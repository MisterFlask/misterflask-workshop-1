// The static site's page side (docs/WEB.md; the contract's section 5): loaded before the client
// (app/main.js), it makes the page talk to the engine in a Web Worker (engine.js) instead of a
// server, with the client unchanged:
//  - it replaces `fetch`: a request whose URL path is `/api` or under `/api/` (same origin) goes
//    to the engine and comes back as a real `Response` (status, headers, body); others go to the
//    real fetch;
//  - it answers `GET /api/status` (the last status the engine published) and the long-poll
//    `GET /api/wait?since=V&timeout_ms=T` itself (as `transport::shared::Shared` does), so they
//    never wait on a busy engine;
//  - it registers the cross-origin-isolation service worker (../coi-sw.js) and reloads once when
//    that is what isolation needs. Isolated, requests reach the worker through a
//    SharedArrayBuffer queue (engine.js `RequestQueue`) which the engine reads while it runs (in
//    turns and blocking decisions); without isolation they are posted, and the engine answers
//    blocking decisions with the advisor's pick (the page says so once);
//  - it shows the loading progress in a small overlay that goes when the engine is ready;
//  - a dead engine (a panic or a trap) turns the status into `crashed` with the report, as the
//    native server does, and a fresh worker starts, from which the player can load an autosave.
//
// `window.ffhWeb`: `state` ("loading", "ready", "failed"), `isolated`, `flush()` (resolves once
// the saves are in IndexedDB), `status()`, `client`, `build` (web/version.txt); and
// `<html data-ffh-engine="loading | ready | failed">`.
//
// No DOM is touched on import outside a page; the node tests drive `EngineClient` directly.

import { RequestQueue, QUEUE_BYTES } from "./engine.js";

export const JSON_TYPE = "application/json; charset=utf-8";
/** The largest request body (`http::MAX_BODY`). */
export const MAX_BODY = 1 << 20;
/** The longest long-poll (`shared::MAX_WAIT`). */
export const MAX_WAIT_MS = 60000;

const enc = new TextEncoder();
const dec = new TextDecoder();

function jsonResponse(status, value) {
  return new Response(JSON.stringify(value), { status, headers: { "Content-Type": JSON_TYPE, "Cache-Control": "no-store" } });
}

function abortError(signal) {
  return signal?.reason ?? new DOMException("The operation was aborted.", "AbortError");
}

function deferred() {
  let resolve, reject;
  const promise = new Promise((a, b) => {
    resolve = a;
    reject = b;
  });
  promise.catch(() => {});
  return { promise, resolve, reject };
}

/** A query parameter as Rust's `str::parse::<u64>` reads it, or null. */
function parseU64(s) {
  return s !== null && /^\+?\d+$/.test(s) ? Number(s) : null;
}

/** One request for the engine: a line of JSON, `\n`, the body. */
export function frameRequest(token, method, path, query, headers, body) {
  const head = enc.encode(JSON.stringify({ t: token, m: method, p: path, q: query, h: headers }) + "\n");
  const out = new Uint8Array(head.length + body.length);
  out.set(head, 0);
  out.set(body, head.length);
  return out;
}

const NULL_BODY = new Set([101, 103, 204, 205, 304]);

/** The engine's reply (`{"s", "ct", "h"}\n<body>`) as a Response. */
export function replyResponse(buf) {
  const u8 = new Uint8Array(buf);
  const nl = u8.indexOf(10);
  let head = null;
  try {
    head = JSON.parse(dec.decode(nl < 0 ? u8 : u8.subarray(0, nl)));
  } catch {
    /* below */
  }
  const status = head?.s;
  if (!Number.isInteger(status) || status < 200 || status > 599) {
    return jsonResponse(500, { ok: false, error: "engine", detail: "the engine sent a malformed reply" });
  }
  const headers = new Headers();
  if (head.ct) headers.set("Content-Type", head.ct);
  for (const [k, v] of Array.isArray(head.h) ? head.h : []) headers.append(k, v);
  return new Response(NULL_BODY.has(status) ? null : u8.slice(nl + 1), { status, headers });
}

/** The crash report (the status's `crash`, as `transport::crash_report` makes it) from the
 *  engine's panic text (`message (at file:line)`) and the last status (its `game`: the journal
 *  and the last autosave of the session that crashed). */
export function crashReport(text, lastStatus) {
  let message = String(text ?? "?");
  let location = "?";
  const at = /^([\s\S]*) \(at ([^()\n]+:\d+)\)$/.exec(message);
  const std = /^panicked at ([^\n]+?):\n([\s\S]*)$/.exec(message);
  if (at) [, message, location] = at;
  else if (std) [, location, message] = std;
  const game = lastStatus?.game ?? null;
  const autosave = game?.autosave ?? null;
  const journal = game?.journal ?? null;
  const repro = journal ? (autosave ? `ffh-play replay ${journal} --from ${autosave}` : `ffh-play replay ${journal}`) : null;
  return { message, location, autosave, journal, repro };
}

function idOf(body) {
  try {
    return JSON.parse(dec.decode(body))?.id ?? null;
  } catch {
    return null;
  }
}

/** The page's side of the engine: the request routing, the status and the long-polls, the
 *  worker's life. `spawn()` makes a worker: `{post(msg, transfer), onmessage(fn), onerror(fn),
 *  terminate()}`. `ui` (optional): `progress(msg)`, `ready(msg)`, `fail(message)`,
 *  `note(message)`, `crashed(report)`, `restarting()`. */
export class EngineClient {
  constructor({ spawn, isolated = false, wasmUrl, dataUrl, autosaveKeep = 20, ui = {}, now = () => performance.now(), queueBytes = QUEUE_BYTES, maxRestarts = 3 }) {
    this.spawn = spawn;
    this.isolated = isolated;
    this.wasmUrl = String(wasmUrl);
    this.dataUrl = String(dataUrl);
    this.autosaveKeep = autosaveKeep;
    this.ui = ui;
    this.now = now;
    this.queueBytes = queueBytes;
    this.maxRestarts = maxRestarts;
    /** The last status the engine published (its object), and when. */
    this.base = null;
    this.publishedAt = 0;
    /** The page's status version: bumped by every publish (and a crash). */
    this.version = 0;
    /** The crash report while the status is `crashed`. */
    this.crash = null;
    this.crashAt = 0;
    this.crashMessage = null;
    this.waiters = new Set();
    this.firstStatus = deferred();
    /** token -> request; `held`: requests not handed to the worker yet (in order). */
    this.pending = new Map();
    this.held = [];
    this.nextToken = 1;
    this.worker = null;
    this.queue = null;
    this.state = "idle";
    this.failure = null;
    this.flushes = new Map();
    this.flushId = 0;
    this.deaths = [];
  }

  /** Starts a worker (the first, or the next after a crash). */
  start() {
    if (this.worker || this.state === "failed") return;
    this.state = "loading";
    this.queue = this.isolated ? RequestQueue.create(this.queueBytes) : null;
    const w = this.spawn();
    this.worker = w;
    w.onmessage((msg) => {
      if (this.worker === w) this.#onMessage(msg);
    });
    w.onerror((message) => {
      if (this.worker !== w) return;
      if (this.state === "ready") this.#dead({ error: message });
      else this.#fail(message);
    });
    w.post({ t: "init", wasm: this.wasmUrl, data: this.dataUrl, queue: this.queue?.sab ?? null, autosaveKeep: this.autosaveKeep });
    this.#pump();
  }

  /** The status as `GET /api/status` answers it now. */
  current() {
    const now = this.now();
    if (this.crash) {
      return { version: this.version, phase: "crashed", crash: this.crash, ms_in_phase: Math.max(0, Math.round(now - this.crashAt)), map_rev: null };
    }
    if (!this.base) return null;
    const { version: _engineVersion, ...rest } = this.base;
    const st = { version: this.version, ...rest };
    if (typeof rest.ms_in_phase === "number") st.ms_in_phase = rest.ms_in_phase + Math.max(0, Math.round(now - this.publishedAt));
    return st;
  }

  /** Answers one `/api` request (a `Request`). */
  async handle(req) {
    const url = new URL(req.url);
    const signal = req.signal;
    if (signal?.aborted) throw abortError(signal);
    if (req.method === "GET" && url.pathname === "/api/status") return this.#localStatus(signal);
    if (req.method === "GET" && url.pathname === "/api/wait") return this.#localWait(url, signal);
    const body = new Uint8Array(await req.arrayBuffer());
    if (body.length > MAX_BODY) return jsonResponse(413, { ok: false, error: "http", detail: `the body is larger than ${MAX_BODY} bytes` });
    if (this.state === "failed") throw this.#netError();
    const headers = {};
    req.headers.forEach((v, k) => {
      headers[k.toLowerCase()] = v;
    });
    const token = this.nextToken;
    this.nextToken = this.nextToken >= 0xfffffff0 ? 1 : this.nextToken + 1;
    const bytes = frameRequest(token, req.method, url.pathname, url.search.slice(1), headers, body);
    return new Promise((resolve, reject) => {
      const e = { token, bytes, method: req.method, body, resolve, reject, sent: false, signal, onAbort: null };
      if (signal) {
        e.onAbort = () => {
          this.pending.delete(token);
          const i = this.held.indexOf(e);
          if (i >= 0) this.held.splice(i, 1);
          reject(abortError(signal));
        };
        signal.addEventListener("abort", e.onAbort, { once: true });
      }
      this.pending.set(token, e);
      this.held.push(e);
      this.#pump();
    });
  }

  /** Resolves once the files the engine wrote under /saves are in the store. */
  flush() {
    if (!this.worker) return Promise.resolve(false);
    const id = ++this.flushId;
    return new Promise((resolve, reject) => {
      this.flushes.set(id, { resolve, reject });
      this.worker.post({ t: "flush", id });
    });
  }

  #netError() {
    return new TypeError(`ffh-web: the engine is not running (${this.failure ?? "?"})`);
  }

  async #untilFirstStatus(signal) {
    if (this.base || this.crash) return;
    if (this.state === "failed") throw this.#netError();
    if (!signal) return this.firstStatus.promise;
    await new Promise((resolve, reject) => {
      const onAbort = () => reject(abortError(signal));
      signal.addEventListener("abort", onAbort, { once: true });
      this.firstStatus.promise.then(resolve, reject).finally(() => signal.removeEventListener("abort", onAbort));
    });
  }

  async #localStatus(signal) {
    await this.#untilFirstStatus(signal);
    return jsonResponse(200, this.current());
  }

  async #localWait(url, signal) {
    const since = parseU64(url.searchParams.get("since")) ?? 0;
    const ms = Math.min(parseU64(url.searchParams.get("timeout_ms")) ?? 25000, MAX_WAIT_MS);
    const t0 = this.now();
    await this.#untilFirstStatus(signal);
    if (this.version > since) return jsonResponse(200, this.current());
    const left = Math.max(0, ms - (this.now() - t0));
    return new Promise((resolve, reject) => {
      let timer = null;
      let onAbort = null;
      const w = { since };
      const cleanup = () => {
        clearTimeout(timer);
        this.waiters.delete(w);
        if (onAbort) signal.removeEventListener("abort", onAbort);
      };
      w.done = () => {
        cleanup();
        resolve(jsonResponse(200, this.current()));
      };
      timer = setTimeout(w.done, left);
      if (signal) {
        onAbort = () => {
          cleanup();
          reject(abortError(signal));
        };
        signal.addEventListener("abort", onAbort, { once: true });
      }
      this.waiters.add(w);
    });
  }

  #wake() {
    for (const w of [...this.waiters]) if (this.version > w.since) w.done();
  }

  /** Hands the held requests to the worker, in order, while they fit. */
  #pump() {
    if (!this.worker || this.state === "failed") return;
    let pushed = false;
    while (this.held.length) {
      const e = this.held[0];
      if (this.queue) {
        if (e.bytes.length > this.queue.maxMessage) {
          this.held.shift();
          this.#settle(e, jsonResponse(413, { ok: false, error: "http", detail: "the request is too large" }));
          continue;
        }
        if (!this.queue.push(e.bytes)) break;
        pushed = true;
      } else {
        this.worker.post({ t: "req", bytes: e.bytes });
      }
      e.sent = true;
      this.held.shift();
    }
    if (pushed) this.worker.post({ t: "kick" });
  }

  #settle(e, response) {
    this.pending.delete(e.token);
    if (e.onAbort) e.signal.removeEventListener("abort", e.onAbort);
    e.resolve(response);
  }

  #onMessage(msg) {
    switch (msg?.t) {
      case "progress":
        this.ui.progress?.(msg);
        break;
      case "ready":
        this.state = "ready";
        this.ui.ready?.(msg);
        break;
      case "publish":
        this.#publish(msg.status);
        break;
      case "reply": {
        const e = this.pending.get(msg.token);
        if (e) this.#settle(e, replyResponse(msg.bytes));
        break;
      }
      case "crash":
        this.crashMessage = msg.message;
        break;
      case "dead":
        this.#dead(msg);
        break;
      case "fail":
        this.#fail(msg.message);
        break;
      case "note":
        this.ui.note?.(msg.message);
        break;
      case "flushed": {
        const f = this.flushes.get(msg.id);
        if (f) {
          this.flushes.delete(msg.id);
          if (msg.ok) f.resolve(true);
          else f.reject(new Error(msg.error ?? "the saves could not be written"));
        }
        break;
      }
    }
    // A reply (or any progress of the worker) may have made room in the queue.
    this.#pump();
  }

  #publish(text) {
    let st;
    try {
      st = JSON.parse(text);
    } catch (e) {
      console.warn("ffh-web: the engine published a bad status:", e);
      return;
    }
    if (!st || typeof st !== "object") return;
    // After a crash, the restarted engine has no game: the status stays `crashed` (with the
    // report) until a game is started or loaded, as the native host keeps it.
    if (this.crash && st.phase !== "no_game") this.crash = null;
    this.base = st;
    this.publishedAt = this.now();
    this.version++;
    this.firstStatus.resolve();
    this.#wake();
  }

  #dead(msg) {
    const old = this.worker;
    this.worker = null;
    old?.terminate();
    const text = msg.crash ?? this.crashMessage ?? `the engine stopped: ${msg.error ?? "?"}`;
    this.crashMessage = null;
    this.crash = crashReport(text, this.base);
    this.crashAt = this.now();
    this.version++;
    this.firstStatus.resolve();
    this.#wake();
    console.error("ffh-web: the engine crashed:", text);
    this.ui.crashed?.(this.crash);
    // The dead worker wrote the saves before it said so.
    for (const f of this.flushes.values()) f.resolve(true);
    this.flushes.clear();
    // What the dead engine had: a POST gets the reply the native server gives a request the
    // game thread dropped; a GET (answered from caches, it changes nothing) goes to the next
    // engine, as do the requests not handed over yet.
    const again = [];
    for (const e of [...this.pending.values()].sort((a, b) => a.token - b.token)) {
      if (!e.sent) continue;
      if (e.method === "POST") {
        this.#settle(e, jsonResponse(200, { id: idOf(e.body), ok: false, error: "crashed", detail: "the game crashed while handling this request: see /api/status" }));
      } else {
        e.sent = false;
        again.push(e);
      }
    }
    this.held = [...again, ...this.held];
    const now = this.now();
    this.deaths = this.deaths.filter((t) => now - t < 60000);
    this.deaths.push(now);
    if (this.deaths.length > this.maxRestarts) {
      this.#fail(`the engine stopped ${this.deaths.length} times within a minute (last: ${text})`);
      return;
    }
    this.state = "idle";
    this.ui.restarting?.();
    this.start();
  }

  #fail(message) {
    this.state = "failed";
    this.failure = String(message ?? "?");
    const old = this.worker;
    this.worker = null;
    old?.terminate();
    console.error("ffh-web: the engine cannot run:", this.failure);
    this.ui.fail?.(this.failure);
    const err = this.#netError();
    for (const e of [...this.pending.values()]) {
      if (e.onAbort) e.signal.removeEventListener("abort", e.onAbort);
      e.reject(err);
    }
    this.pending.clear();
    this.held = [];
    for (const f of this.flushes.values()) f.reject(err);
    this.flushes.clear();
    this.firstStatus.reject(err);
    // Long-polls answer with what there is (the status as it stood) rather than hang.
    for (const w of [...this.waiters]) w.done();
  }
}

/** Routes `fetch` calls for `/api` and `/api/...` (resolved against `base`, same origin) to
 *  `client.handle`; returns the real fetch. */
export function installFetch(client, { target = globalThis, origin = globalThis.location?.origin, base = globalThis.document?.baseURI ?? globalThis.location?.href } = {}) {
  const realFetch = target.fetch;
  target.fetch = function fetch(input, init) {
    let req;
    try {
      const url = new URL(input instanceof Request ? input.url : String(input), base);
      if (url.origin !== origin || !(url.pathname === "/api" || url.pathname.startsWith("/api/"))) return realFetch(input, init);
      req = new Request(input instanceof Request ? input : url, init);
    } catch {
      return realFetch(input, init);
    }
    return client.handle(req);
  };
  return realFetch;
}

// ---------------------------------------------------------------------------------------------
// The page: the overlay, the service worker, the start.

const RELOAD_KEY = "ffh-web-coi-reload";

function mb(n) {
  return (n / 1048576).toFixed(1);
}

/** The loading overlay (a small card in the middle of the page, gone once the engine is ready)
 *  and the notes (a line at the left, above the client's bottom panel, that goes by itself).
 *  Plain DOM, styled inline with the client's palette variables (with fallbacks, for before its
 *  stylesheet loads). */
class Overlay {
  constructor() {
    this.el = null;
    this.files = {};
    this.restart = false;
  }

  #card() {
    if (this.el) return this.el;
    const el = document.createElement("div");
    el.id = "ffh-web-loading";
    el.setAttribute("role", "status");
    el.setAttribute("aria-live", "polite");
    el.style.cssText =
      "position:fixed;left:50%;top:42%;transform:translate(-50%,-50%);z-index:1000;box-sizing:border-box;" +
      "width:min(360px,calc(100vw - 32px));padding:14px 18px;background:var(--c-soot,#14223e);color:var(--c-bone,#e9dbb3);" +
      "border:1px solid var(--c-ochre,#dba63b);box-shadow:0 0 0 1px var(--c-void,#140d0c),0 10px 30px rgba(0,0,0,.45);" +
      "font:14px/1.4 var(--font,Georgia,serif);transition:opacity .3s ease;pointer-events:none";
    const title = document.createElement("div");
    title.style.cssText = "font-weight:600;margin-bottom:8px";
    const bar = document.createElement("div");
    bar.style.cssText = "height:6px;background:var(--c-coal,#26334f);overflow:hidden";
    const fill = document.createElement("div");
    fill.style.cssText = "height:100%;width:4%;background:var(--c-gold,#f2c74b);transition:width .15s linear";
    bar.append(fill);
    const detail = document.createElement("div");
    detail.style.cssText = "margin-top:6px;font-size:12px;color:var(--c-stone,#adb3bd);min-height:1.4em;overflow-wrap:anywhere";
    el.append(title, bar, detail);
    (document.body ?? document.documentElement).append(el);
    this.el = el;
    this.title = title;
    this.bar = bar;
    this.fill = fill;
    this.detail = detail;
    return el;
  }

  show(title) {
    this.#card();
    this.el.style.opacity = "1";
    this.title.textContent = title;
  }

  progress(msg) {
    this.#card();
    if (msg.stage === "start") {
      this.title.textContent = this.restart ? "Restarting the engine…" : "Starting the engine…";
      this.fill.style.width = "100%";
      return;
    }
    this.files[msg.file] = msg;
    const parts = [];
    let loaded = 0;
    let total = 0;
    for (const [name, label] of [["engine", "Engine"], ["data", "Game data"]]) {
      const f = this.files[name];
      if (!f) continue;
      loaded += f.loaded;
      total += f.total || f.loaded;
      parts.push(f.total ? `${label} ${mb(f.loaded)} / ${mb(f.total)} MB` : `${label} ${mb(f.loaded)} MB`);
    }
    this.detail.textContent = parts.join(" · ");
    if (total) this.fill.style.width = `${Math.max(4, Math.min(100, (100 * loaded) / total))}%`;
  }

  ready() {
    document.documentElement.dataset.ffhEngine = "ready";
    if (!this.el) return;
    const el = this.el;
    this.el = null;
    this.files = {};
    el.style.opacity = "0";
    setTimeout(() => el.remove(), 350);
  }

  restarting() {
    this.restart = true;
    document.documentElement.dataset.ffhEngine = "loading";
    this.show("Restarting the engine…");
    this.detail.textContent = "The game crashed; a fresh engine is starting so you can load an autosave.";
  }

  crashed() {}

  fail(message) {
    document.documentElement.dataset.ffhEngine = "failed";
    this.show("The game could not start");
    this.el.setAttribute("role", "alert");
    this.el.style.pointerEvents = "auto";
    this.bar.style.display = "none";
    this.detail.textContent = message;
    const button = document.createElement("button");
    button.textContent = "Reload";
    button.style.cssText = "margin-top:10px";
    button.addEventListener("click", () => location.reload());
    this.el.append(button);
  }

  note(message, ms = 12000) {
    const el = document.createElement("div");
    el.setAttribute("role", "note");
    el.style.cssText =
      "position:fixed;left:12px;bottom:190px;z-index:1000;max-width:min(440px,calc(100vw - 24px));box-sizing:border-box;padding:6px 28px 6px 10px;" +
      "background:var(--c-soot,#14223e);color:var(--c-stone,#adb3bd);border:1px solid var(--c-iron,#424858);font:12px/1.4 var(--font,Georgia,serif);transition:opacity .3s ease";
    el.textContent = message;
    const close = document.createElement("button");
    close.textContent = "×";
    close.setAttribute("aria-label", "Dismiss");
    close.style.cssText = "position:absolute;right:4px;top:2px;background:none;border:0;color:inherit;font:inherit;font-size:16px;cursor:pointer";
    const remove = () => {
      el.style.opacity = "0";
      setTimeout(() => el.remove(), 350);
    };
    close.addEventListener("click", remove);
    el.append(close);
    (document.body ?? document.documentElement).append(el);
    setTimeout(remove, ms);
  }
}

function browserSpawn(url) {
  return () => {
    const w = new Worker(url, { type: "module", name: "ffh-engine" });
    return {
      post: (msg, transfer) => w.postMessage(msg, transfer ?? []),
      onmessage: (fn) => w.addEventListener("message", (ev) => fn(ev.data)),
      onerror: (fn) => {
        w.addEventListener("error", (ev) => {
          ev.preventDefault?.();
          fn(ev.message || "the engine worker could not be loaded");
        });
        w.addEventListener("messageerror", () => console.warn("ffh-web: a message from the engine could not be read"));
      },
      terminate: () => w.terminate(),
    };
  };
}

/** Makes the page cross-origin isolated through the service worker: registers it and reloads
 *  once it is active (the reload is then served through it, with the isolation headers); a
 *  session flag allows one such reload, so a browser where isolation does not take goes on
 *  without it. Returns "reloading", or why the page goes on without isolation. */
async function ensureIsolation(site) {
  if (!("serviceWorker" in navigator) || !globalThis.isSecureContext) return "no service workers";
  let already;
  try {
    already = sessionStorage.getItem(RELOAD_KEY);
  } catch {
    return "no session storage";
  }
  let reg;
  try {
    reg = await navigator.serviceWorker.register(new URL("coi-sw.js", site), { scope: site.pathname, updateViaCache: "none" });
  } catch (e) {
    return `the service worker could not register (${e?.message ?? e})`;
  }
  if (already === "1") return "isolation did not take after a reload";
  // A browser that blocks service workers may resolve without a registration.
  if (!reg) return "the service worker did not register";
  if (!navigator.serviceWorker.controller && !reg.active) {
    await Promise.race([navigator.serviceWorker.ready, new Promise((r) => setTimeout(r, 5000))]);
  }
  if (!navigator.serviceWorker.controller && !reg.active) return "the service worker did not become active";
  try {
    sessionStorage.setItem(RELOAD_KEY, "1");
  } catch {
    return "no session storage";
  }
  location.reload();
  return "reloading";
}

function main() {
  const site = new URL("../", import.meta.url);
  const isolated = globalThis.crossOriginIsolated === true && typeof SharedArrayBuffer === "function";
  const ui = new Overlay();
  let noted = false;
  const client = new EngineClient({
    spawn: browserSpawn(new URL("engine.js", import.meta.url)),
    isolated,
    wasmUrl: new URL("ffh.wasm.gz", import.meta.url),
    dataUrl: new URL("data.pak.gz", import.meta.url),
    ui: {
      progress: (m) => ui.progress(m),
      ready: (m) => {
        ui.ready();
        if (!m.isolated && !noted) {
          noted = true;
          ui.note("Running without cross-origin isolation: questions the game asks in the middle of a turn (keep or raze a city, offers, free techs) get the advisor's pick.");
        }
      },
      fail: (m) => ui.fail(m),
      note: (m) => ui.note(m),
      restarting: () => ui.restarting(),
    },
  });
  const realFetch = installFetch(client);
  const web = {
    client,
    isolated,
    build: null,
    get state() {
      return client.state;
    },
    flush: () => client.flush(),
    status: () => client.current(),
  };
  globalThis.ffhWeb = web;
  document.documentElement.dataset.ffhEngine = "loading";
  ui.show("Loading the game…");
  realFetch(new URL("version.txt", import.meta.url), { cache: "no-cache" })
    .then((r) => (r.ok ? r.text() : null))
    .then((t) => {
      if (!t) return;
      web.build = t.trim();
      console.info(`ffh-web build ${web.build}`);
    })
    .catch(() => {});
  (async () => {
    if (isolated) {
      try {
        sessionStorage.removeItem(RELOAD_KEY);
      } catch {
        /* nothing to clear */
      }
      // Isolated by the service worker: let it update itself (a byte-wise check, no reload).
      if (navigator.serviceWorker?.controller) navigator.serviceWorker.register(new URL("coi-sw.js", site), { scope: site.pathname, updateViaCache: "none" }).catch(() => {});
    } else {
      const how = await ensureIsolation(site);
      if (how === "reloading") return;
      console.info(`ffh-web: running without cross-origin isolation: ${how}`);
    }
    // One tab at a time: each tab's engine keeps its own copy of the saves and writes it back,
    // so two tabs would overwrite each other's games. (Asked after the isolation reload, which
    // would otherwise hold it across the reload.)
    navigator.locks
      ?.request("ffh-web-saves", { ifAvailable: true }, (lock) => {
        if (!lock) {
          ui.note("The game is open in another tab too: saves made in both tabs may overwrite each other. Close one of them.", 30000);
          return null;
        }
        return new Promise(() => {});
      })
      .catch(() => {});
    client.start();
  })();
}

if (typeof window !== "undefined" && typeof document !== "undefined" && typeof Worker === "function") main();
