#!/usr/bin/env node
// The smoke test of the static site (docs/WEB.md; what tools/web-site.sh builds): serves the site
// with a tiny static server below a path, as GitHub Pages does (`/misterflask-workshop-1/`), opens
// it in headless Chromium (Playwright) and plays: the engine loads (WebAssembly in a worker; by
// default the server sends no isolation headers, so the service worker coi-sw.js has to make the
// page cross-origin isolated, with one reload), a new game starts from the new-game screen (tiny
// map, fixed seed), the map is drawn with the art, a few turns end through the end-turn gate with
// the popups answered, the game is saved, the page is reloaded with `?renderer=iso` (a fresh
// engine: the save comes back from IndexedDB) and the save loaded, the isometric renderer draws
// it, and one more turn ends. Fails on any page error, console error, failed request or HTTP
// error; screenshots go to --out.
//
//   node tools/web-site-smoke.mjs --site DIR [--port N] [--base /path/] [--isolated] [--no-sw]
//        [--turns 3] [--seed 5] [--out DIR] [--headed]
//
//   --site DIR    the site (tools/web-site.sh DIR)
//   --port N      the static server's port (default: a free one)
//   --base PATH   the path the site is served below (default /misterflask-workshop-1/)
//   --isolated    the server sends COOP/COEP itself (the page is isolated without the service worker)
//   --no-sw       a browser without service workers (navigator.serviceWorker removed): the page
//                 runs without isolation (the fallback: the advisor answers every blocking
//                 decision)
//   --serve       only serve the site (prints the URL) until interrupted; no browser
//
// The browser: the `playwright` package, resolved normally, then in node's global modules and
// $NODE_PATH (as tools/web-smoke.mjs does). Without it the test fails (exit 1): CI installs it.
// Exit 2 on bad arguments.

import http from "node:http";
import { createRequire } from "node:module";
import fs from "node:fs";
import path from "node:path";
import { ROOT, parseArgs, until } from "./web-lib.mjs";

let args;
try {
  args = parseArgs(process.argv.slice(2), { site: "v", port: "v", base: "v", isolated: "flag", "no-sw": "flag", turns: "v", seed: "v", out: "v", headed: "flag", serve: "flag" });
  if (!args.site) throw new Error("--site DIR is required");
  for (const k of ["port", "turns", "seed"]) if (args[k] !== undefined && !/^\d+$/.test(args[k])) throw new Error(`--${k} needs a number, not ${args[k]}`);
  if (args.base !== undefined && !/^\/([^?#]*\/)?$/.test(args.base)) throw new Error(`--base needs a path that starts and ends with "/", not ${args.base}`);
} catch (e) {
  console.error(`web-site-smoke: ${e.message}`);
  console.error("usage: node tools/web-site-smoke.mjs --site DIR [--port N] [--base /path/] [--isolated] [--no-sw] [--turns 3] [--seed 5] [--out DIR] [--headed] [--serve]");
  process.exit(2);
}
const SITE = path.resolve(args.site);
const BASE = args.base ?? "/misterflask-workshop-1/";
const PORT = Number(args.port ?? 0);
const TURNS = Number(args.turns ?? 3);
const SEED = Number(args.seed ?? 5);
const OUT = path.resolve(args.out ?? path.join(ROOT, "target", "web-site-smoke"));
const VIEW = { width: 1280, height: 800 };
/** The page should end up cross-origin isolated: by the server's headers or by the service worker. */
const WANT_ISOLATED = !!args.isolated || !args["no-sw"];

// ------------------------------------------------------------------------------------------
// The static server: GitHub Pages' behaviour that matters here (files below a path, a directory
// is its index.html, `/base` redirects to `/base/`, .gz files as application/gzip with no
// Content-Encoding, no isolation headers).

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".txt": "text/plain; charset=utf-8",
  ".wasm": "application/wasm",
  ".gz": "application/gzip",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".ico": "image/x-icon",
};

/** Serves `dir` below `base` on 127.0.0.1:`port`; resolves with `{url, close(), served}`. */
function serveSite({ dir, base = "/", port = 0, isolated = false }) {
  const served = { files: 0, missing: [] };
  const server = http.createServer((req, res) => {
    const head = (status, headers = {}) => {
      if (isolated) {
        headers["Cross-Origin-Opener-Policy"] = "same-origin";
        headers["Cross-Origin-Embedder-Policy"] = "require-corp";
        headers["Cross-Origin-Resource-Policy"] = "same-origin";
      }
      res.writeHead(status, headers);
    };
    const notFound = (what) => {
      served.missing.push(what);
      head(404, { "Content-Type": "text/plain; charset=utf-8" });
      res.end(req.method === "HEAD" ? undefined : `404: ${what}\n`);
    };
    if (req.method !== "GET" && req.method !== "HEAD") {
      head(405, { Allow: "GET, HEAD", "Content-Type": "text/plain; charset=utf-8" });
      return res.end("405\n");
    }
    let pathname;
    try {
      pathname = decodeURIComponent(new URL(req.url, "http://localhost").pathname);
    } catch {
      head(400, { "Content-Type": "text/plain; charset=utf-8" });
      return res.end("400\n");
    }
    if (pathname === base.slice(0, -1) && base !== "/") {
      head(301, { Location: base });
      return res.end();
    }
    if (!pathname.startsWith(base)) return notFound(pathname);
    let rel = pathname.slice(base.length);
    if (rel === "" || rel.endsWith("/")) rel += "index.html";
    const file = path.resolve(dir, rel);
    const inside = path.relative(dir, file);
    if (inside.startsWith("..") || path.isAbsolute(inside) || rel.includes("\0")) return notFound(pathname);
    let st;
    try {
      st = fs.statSync(file);
    } catch {
      return notFound(pathname);
    }
    if (st.isDirectory()) {
      head(301, { Location: `${pathname}/` });
      return res.end();
    }
    if (!st.isFile()) return notFound(pathname);
    served.files++;
    head(200, { "Content-Type": TYPES[path.extname(file).toLowerCase()] ?? "application/octet-stream", "Content-Length": st.size, "Cache-Control": "no-cache" });
    if (req.method === "HEAD") return res.end();
    fs.createReadStream(file).pipe(res);
  });
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      const { port: p } = server.address();
      resolve({
        url: `http://127.0.0.1:${p}${base}`,
        served,
        close: () =>
          new Promise((r) => {
            server.closeAllConnections?.();
            server.close(() => r());
          }),
      });
    });
  });
}

// ------------------------------------------------------------------------------------------
// The browser.

async function loadPlaywright() {
  try {
    return await import("playwright");
  } catch {
    /* not installed locally */
  }
  const require = createRequire(import.meta.url);
  const roots = [path.join(path.dirname(process.execPath), "..", "lib", "node_modules"), ...(process.env.NODE_PATH ?? "").split(path.delimiter).filter(Boolean)];
  for (const r of roots) {
    try {
      return require(require.resolve("playwright", { paths: [r] }));
    } catch {
      /* next */
    }
  }
  return null;
}

const problems = [];
const steps = [];
let page = null;
let shotN = 0;

async function shot(name) {
  const file = path.join(OUT, `${String(++shotN).padStart(2, "0")}-${name}.png`);
  await page.screenshot({ path: file });
  steps.push(`screenshot ${path.relative(process.cwd(), file)}`);
}

/** Evaluates `expr` in the page; null while the page navigates (the isolation reload). */
async function ev(expr) {
  try {
    return await page.evaluate(expr);
  } catch (e) {
    if (/Execution context was destroyed|Cannot find context|navigat|Target page, context or browser has been closed/i.test(String(e?.message))) return null;
    throw e;
  }
}
const until2 = (expr, what, ms = 30000) => until(() => (pageError(), ev(expr)), what, ms, 100);
const S = (expr) => ev(`(() => { const S = window.ffh && window.ffh.S; return ${expr}; })()`);
const modalTitles = async () => (await ev(`[...document.querySelectorAll("#modals .backdrop:not(.under) .modal h2")].map((e) => e.textContent)`)) ?? [];
const dialog = (title, ms = 30000) => until(async () => (await modalTitles()).includes(title), `the dialog ${title}`, ms);

/** Clicks the element `sel` (its centre) with the mouse. */
async function click(sel) {
  const r = await ev(`(() => { const e = document.querySelector(${JSON.stringify(sel)}); if (!e) return null; e.scrollIntoView({block: "nearest"}); const r = e.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2, disabled: !!e.disabled }; })()`);
  if (!r) throw new Error(`no element ${sel}`);
  if (r.disabled) throw new Error(`${sel} is disabled`);
  await page.mouse.click(r.x, r.y);
}

/** Clicks the button of the top dialog whose text is `text`. */
async function clickButton(text) {
  const ok = await ev(`(() => {
    const b = [...document.querySelectorAll("#modals .backdrop:not(.under) .modal-buttons button")].find((b) => b.textContent.trim() === ${JSON.stringify(text)});
    if (!b) return false;
    b.dataset.smoke = "target";
    return true;
  })()`);
  if (!ok) throw new Error(`no button "${text}" in the top dialog`);
  await click(`button[data-smoke="target"]`);
  await ev(`document.querySelector('button[data-smoke="target"]')?.removeAttribute("data-smoke")`);
}

/** The engine's crash, as the status reports it, or null. */
async function crashed() {
  const st = await S(`S && S.status && S.status.phase === "crashed" ? JSON.stringify(S.status.crash ?? S.status) : null`);
  return st ? `the engine crashed: ${st}` : null;
}

/** A page error so far (the waits stop at one instead of timing out). */
function pageError() {
  const e = problems.find((p) => p.startsWith("page error:"));
  if (e) throw new Error(e);
}

/** Loads the page at `url` and waits for the engine and the client (through the isolation reload). */
async function boot(url, what) {
  await page.goto(url);
  const state = await until(async () => {
    pageError();
    const s = await ev(`window.ffhWeb ? window.ffhWeb.state : null`);
    return s === "ready" || s === "failed" ? s : null;
  }, `the engine to load (${what})`, 180000, 200);
  if (state === "failed") {
    const text = await ev(`document.body.innerText.slice(-2000)`);
    throw new Error(`the engine failed to load (${what}): ${text}`);
  }
  await until2("!!(window.ffh && window.ffh.ready)", `the client to boot (${what})`, 60000);
  return ev(`({ isolated: globalThis.crossOriginIsolated === true, web: window.ffhWeb.isolated, sw: !!(navigator.serviceWorker && navigator.serviceWorker.controller), build: window.ffhWeb.build })`);
}

async function waitHuman(what, ms = 120000) {
  await until(async () => {
    pageError();
    const c = await crashed();
    if (c) throw new Error(c);
    return S(`!!S && !!S.status && S.status.phase === "human" && !!S.view && S.view.turn === S.status.turn`);
  }, what, ms, 100);
  await new Promise((r) => setTimeout(r, 250));
}

/** Answers what the turn start shows: the deferred popups (first enabled choice) and the
 *  blocking decision (keep / first option); done once no dialog shows twice in a row. */
async function answerPopups() {
  let calm = 0;
  for (let k = 0; k < 16; k++) {
    await new Promise((r) => setTimeout(r, 300));
    const titles = await modalTitles();
    if (!titles.length) {
      if (++calm >= 2) return;
      continue;
    }
    calm = 0;
    const top = "#modals .backdrop:not(.under)";
    const has = (sel) => ev(`!!document.querySelector(${JSON.stringify(`${top} ${sel}`)})`);
    if (await has(".modal.decision")) await click(`${top} .modal.decision button.option:not([disabled])`);
    else if (await has("button.build[data-tech]")) await click(`${top} button.build[data-tech]`);
    else if (await has("button.build[data-order]:not([disabled])")) await click(`${top} button.build[data-order]:not([disabled])`);
    else if (await has("button.option:not([disabled])")) await click(`${top} button.option:not([disabled])`);
    else await page.keyboard.press("Escape");
    steps.push(`answered "${titles.at(-1)}"`);
  }
}

/** Ends `n` turns with the End turn button (the gate when units wait: end anyway). */
async function endTurns(n) {
  for (let k = 0; k < n; k++) {
    const turn = await S("S.status.turn");
    const t0 = Date.now();
    await click("#end-turn");
    await new Promise((r) => setTimeout(r, 400));
    if ((await modalTitles()).includes("End the turn?")) await click("#modals .modal-buttons button:not(.primary):last-child");
    await until(async () => {
      const c = await crashed();
      if (c) throw new Error(c);
      return S(`!!S.status && (S.status.turn > ${turn} || S.status.phase === "decision")`);
    }, `turn ${turn + 1}`, 180000, 100);
    await waitHuman(`turn ${turn + 1}`);
    await answerPopups();
    steps.push(`ended turn ${turn} (${((Date.now() - t0) / 1000).toFixed(1)} s)`);
  }
}

async function newGame() {
  await dialog("New game", 60000);
  await shot("new-game");
  await ev(`(() => {
    const set = (id, v) => { const e = document.getElementById(id); e.value = v; e.dispatchEvent(new Event("change")); };
    set("ng-world", "WORLDSIZE_TINY"); set("ng-seed", "${SEED}"); set("ng-opponents", "2");
  })()`);
  const t0 = Date.now();
  await clickButton("Start");
  await waitHuman("the first turn", 180000);
  steps.push(`new game (tiny, seed ${SEED}, 2 opponents): turn ${await S("S.status.turn")} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  await answerPopups();
}

/** The map is drawn with the art: the art loaded (its bundle, relative to the page), the view is
 *  drawn in full, the middle of the map is not blank. */
async function artMap() {
  await until2("!!(window.ffh.look.art || window.ffh.look.error)", "the art to load", 60000);
  const err = await ev("window.ffh.look.error ? String(window.ffh.look.error.message || window.ffh.look.error) : null");
  if (err) throw new Error(`the art did not load: ${err}`);
  if ((await ev("window.ffh.mapView.renderer()")) !== "art") throw new Error("the map is not drawn with the art");
  await until2("window.ffh.mapView.art.settled()", "the art map drawn in full", 60000);
  const colours = await ev(`(() => {
    const c = window.ffh.mapView.canvas;
    const g = c.getContext("2d", { willReadFrequently: true });
    const w = Math.min(240, c.width >> 1), h = Math.min(240, c.height >> 1);
    const d = g.getImageData((c.width - w) >> 1, (c.height - h) >> 1, w, h).data;
    const seen = new Set();
    for (let i = 0; i < d.length; i += 4) seen.add((d[i] << 16) | (d[i + 1] << 8) | d[i + 2]);
    return seen.size;
  })()`);
  if (colours < 8) throw new Error(`the art map is nearly blank (${colours} colours in the middle of the view)`);
  await shot("art-map");
  steps.push(`art map: ${colours} colours mid-view`);
}

/** Saves the game as `smoke` and waits until the engine has written it to IndexedDB. */
async function save() {
  await click("#btn-save");
  await dialog("Save the game");
  await ev(`document.getElementById("save-name").select()`);
  await page.keyboard.type("smoke");
  await page.keyboard.press("Enter");
  await until2(`[...document.querySelectorAll("#toasts .toast")].some((t) => t.textContent.startsWith("Saved"))`, "the saved toast");
  const turn = await S("S.status.turn");
  await ev("window.ffhWeb.flush()");
  // The engine's IndexedDB (database ffh-play, store saves, keys: paths under /saves).
  const keys = await ev(`new Promise((resolve, reject) => {
    const open = indexedDB.open("ffh-play");
    open.onerror = () => reject(open.error);
    open.onsuccess = () => {
      const db = open.result;
      if (!db.objectStoreNames.contains("saves")) return (db.close(), resolve([]));
      const rq = db.transaction("saves", "readonly").objectStore("saves").getAllKeys();
      rq.onerror = () => reject(rq.error);
      rq.onsuccess = () => (db.close(), resolve(rq.result.map(String)));
    };
  })`);
  if (!keys.some((k) => /(^|\/)smoke(\.[^/]*)?$/.test(k))) throw new Error(`the save is not in IndexedDB: ${JSON.stringify(keys)}`);
  steps.push(`saved as smoke at turn ${turn} (${keys.length} files in IndexedDB)`);
  return turn;
}

/** After a reload (a fresh engine, no game): loads `smoke` from the new-game screen. */
async function loadSmoke(turn) {
  await dialog("New game", 60000);
  await clickButton("Load a game…");
  const sel = `#modals button.save[data-save$="/smoke"]`;
  await until2(`!!document.querySelector(${JSON.stringify(sel)})`, "the save in the load screen (from IndexedDB)", 30000);
  await shot("load-screen");
  await click(sel);
  await waitHuman("the loaded game");
  const now = await S("S.status.turn");
  if (now !== turn) throw new Error(`the loaded game is at turn ${now}, saved at ${turn}`);
  await answerPopups();
  steps.push(`loaded smoke after a reload: turn ${now}`);
}

/** The isometric renderer draws the game: its sprites (app/iso/*.png, relative URLs) load and the
 *  frame is not blank. */
async function isoRenderer() {
  if ((await ev("window.ffh.mapView.renderer()")) !== "iso") throw new Error("?renderer=iso did not choose the isometric renderer");
  await until2("(() => { const im = window.ffh.mapView.iso.sprites.images; return im.size > 3 && [...im.values()].every((e) => e.ok); })()", "the isometric sprites to load", 60000);
  const colours = await ev(`(() => {
    const mv = window.ffh.mapView;
    mv.draw();
    const d = mv.ctx.getImageData(0, 0, mv.canvas.width, mv.canvas.height).data;
    const seen = new Set();
    for (let i = 0; i < d.length; i += 4 * 37) seen.add(((d[i] >> 3) << 10) | ((d[i + 1] >> 3) << 5) | (d[i + 2] >> 3));
    return seen.size;
  })()`);
  if (colours < 64) throw new Error(`the isometric frame is nearly blank (${colours} colours)`);
  await shot("renderer-iso");
  steps.push(`?renderer=iso: a frame of ${colours} colours`);
}

function describe(info) {
  return `isolated ${info.isolated ? "yes" : "no"}${info.sw ? " (service worker)" : ""}, build ${info.build ?? "?"}`;
}

async function main() {
  if (!fs.existsSync(path.join(SITE, "index.html"))) {
    console.error(`web-site-smoke: no site at ${SITE} (no index.html): build it with tools/web-site.sh ${args.site}`);
    return 1;
  }
  for (const f of ["web/boot.js", "web/engine.js", "web/wasi.js", "web/ffh.wasm.gz", "web/data.pak.gz", "coi-sw.js"]) {
    if (!fs.existsSync(path.join(SITE, f))) problems.push(`the site has no ${f}`);
  }
  const server = await serveSite({ dir: SITE, base: BASE, port: PORT, isolated: !!args.isolated });
  if (args.serve) {
    console.log(`web-site-smoke: serving ${SITE} at ${server.url}${args.isolated ? " (with COOP/COEP)" : ""}; Ctrl-C stops`);
    await new Promise((r) => process.once("SIGINT", r));
    await server.close();
    return 0;
  }
  if (problems.length) {
    for (const p of problems) console.error(`FAIL: ${p}`);
    await server.close();
    return 1;
  }
  const pw = await loadPlaywright();
  if (!pw) {
    console.error("web-site-smoke: the playwright package was not found (npm i playwright, or put its node_modules in NODE_PATH; then npx playwright install chromium)");
    await server.close();
    return 1;
  }
  fs.mkdirSync(OUT, { recursive: true });
  for (const f of fs.readdirSync(OUT)) if (f.endsWith(".png")) fs.rmSync(path.join(OUT, f));
  let browser = null;
  try {
    browser = await pw.chromium.launch({ headless: !args.headed });
    const context = await browser.newContext({ viewport: VIEW });
    // No service workers, as in a browser without them (Playwright's `serviceWorkers: "block"`
    // instead makes register() resolve with undefined, which no browser does).
    if (args["no-sw"]) await context.addInitScript(() => delete Navigator.prototype.serviceWorker);
    page = await context.newPage();
    page.on("console", (m) => m.type() === "error" && problems.push(`console: ${m.text()}`));
    page.on("pageerror", (e) => problems.push(`page error: ${e.message}`));
    // A navigation (the isolation reload, the iso reload) aborts the page's requests: not a failure.
    page.on("requestfailed", (r) => r.failure()?.errorText !== "net::ERR_ABORTED" && problems.push(`request failed: ${r.url()} ${r.failure()?.errorText ?? ""}`));
    page.on("response", (r) => r.status() >= 400 && problems.push(`HTTP ${r.status()}: ${r.url()}`));
    const mode = `${args.isolated ? "COOP/COEP from the server" : "no isolation headers"}, ${args["no-sw"] ? "no service workers" : "service workers allowed"}`;
    console.log(`web-site-smoke: Chromium ${browser.version()}; ${server.url} (${mode}); screenshots in ${OUT}`);
    const t0 = Date.now();

    const info = await boot(server.url, "first visit");
    steps.push(`engine and client ready in ${((Date.now() - t0) / 1000).toFixed(1)} s: ${describe(info)}`);
    if (WANT_ISOLATED && !info.isolated) throw new Error(`the page is not cross-origin isolated (${args.isolated ? "the server sends COOP/COEP" : "the service worker should have made it so"}): ${describe(info)}`);
    if (!WANT_ISOLATED && info.isolated) throw new Error("the page is isolated without service workers or isolation headers");
    if (info.web !== info.isolated) throw new Error(`ffhWeb.isolated is ${info.web}, crossOriginIsolated ${info.isolated}`);
    if (!args.isolated && !args["no-sw"] && !info.sw) throw new Error("no service worker controls the page");
    await until2("!!window.ffhWeb.build", "web/version.txt", 10000);

    await newGame();
    await artMap();
    await endTurns(TURNS);
    const turn = await save();

    const iso = await boot(`${server.url}?renderer=iso`, "reload with ?renderer=iso");
    if (WANT_ISOLATED && !iso.isolated) throw new Error(`the reloaded page is not cross-origin isolated: ${describe(iso)}`);
    steps.push(`reloaded with ?renderer=iso: ${describe(iso)}`);
    await loadSmoke(turn);
    await isoRenderer();
    await endTurns(1);
    console.log(`web-site-smoke: ${steps.length} steps in ${((Date.now() - t0) / 1000).toFixed(1)} s; ${server.served.files} files served`);
  } catch (e) {
    problems.push(`step failed: ${e.message}`);
    try {
      await page?.screenshot({ path: path.join(OUT, "failure.png") });
      problems.push(`see ${path.join(OUT, "failure.png")}`);
    } catch {
      /* the browser is gone */
    }
  } finally {
    await browser?.close().catch(() => {});
    await server.close();
  }
  for (const s of steps) console.log(`  ${s}`);
  if (server.served.missing.length) console.log(`  not found: ${[...new Set(server.served.missing)].join(" ")}`);
  if (problems.length) {
    for (const p of problems) console.error(`FAIL: ${p}`);
    return 1;
  }
  console.log("web-site-smoke: ok");
  return 0;
}

process.exit(await main());
