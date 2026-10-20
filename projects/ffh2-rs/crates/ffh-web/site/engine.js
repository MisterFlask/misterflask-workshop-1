// The engine worker (docs/WEB.md; the contract's sections 4-6): loads the engine
// (`web/ffh.wasm.gz`) and the game data (`web/data.pak.gz`), mounts the data at /data and the
// saves (from IndexedDB) at /saves in wasi.js's in-memory file system, starts the engine
// (`_initialize`, `ffh_init`) and serves the page's HTTP requests one at a time with
// `ffh_request`. After each batch of requests (the worker idle) the files under /saves that
// changed are written to IndexedDB (database `ffh-play`, store `saves`, key = the path under
// /saves).
//
// Requests arrive through a SharedArrayBuffer queue when the page is cross-origin isolated
// (`RequestQueue`: the page produces, this worker consumes; a postMessage "kick" wakes the worker
// when it is idle, and the engine pulls further requests itself while one runs: `pull`), else
// as postMessage payloads (then `pull` answers -1: nothing can arrive while the engine runs).
//
// This module also runs on the page (boot.js imports `RequestQueue`) and in node (the tests):
// it starts the worker only inside a dedicated worker; `workerMain(env)` is the entry point.
//
// Page -> worker: {t: "init", wasm, data, queue: SharedArrayBuffer | null, autosaveKeep, env}
//                 {t: "req", bytes}   {t: "kick"}   {t: "flush", id}
// Worker -> page: {t: "progress", file: "engine" | "data", loaded, total} | {t: "progress", stage}
//                 {t: "ready", isolated, store}   {t: "fail", message}
//                 {t: "publish", status}   {t: "reply", token, bytes}
//                 {t: "crash", message}   {t: "dead", crash, error}
//                 {t: "flushed", id, ok}   {t: "note", message}

import { MemFS, WASI, WasiExit } from "./wasi.js";

const enc = new TextEncoder();
const dec = new TextDecoder();

// ---------------------------------------------------------------------------------------------
// The request queue

/** The default queue size: a request body may be 1 MiB (`http::MAX_BODY`), plus its head. */
export const QUEUE_BYTES = 1 << 21;

const HEAD = 0;
const TAIL = 1;
const HEADER_BYTES = 16;

/** A single-producer single-consumer queue of byte messages in a SharedArrayBuffer: an Int32
 *  header (the write and read counters, free-running modulo 2^32) and a ring of `capacity`
 *  bytes (a power of two). Each message is a 4-byte little-endian length and its bytes; both may
 *  wrap around the ring's end. The producer (the page's main thread) never blocks: `push`
 *  returns false when the message does not fit now. The consumer (the worker) pops, and may
 *  `wait` (Atomics.wait) until a message comes. */
export class RequestQueue {
  static create(capacity = QUEUE_BYTES) {
    if (typeof SharedArrayBuffer !== "function") throw new Error("SharedArrayBuffer is not available");
    return new RequestQueue(new SharedArrayBuffer(HEADER_BYTES + capacity));
  }

  constructor(sab) {
    this.sab = sab;
    this.i32 = new Int32Array(sab, 0, HEADER_BYTES / 4);
    this.ring = new Uint8Array(sab, HEADER_BYTES);
    this.cap = this.ring.length;
    if (this.cap < 8 || (this.cap & (this.cap - 1)) !== 0) throw new Error(`queue capacity ${this.cap} is not a power of two`);
  }

  /** The largest message that can ever fit. */
  get maxMessage() {
    return this.cap - 4;
  }

  /** Bytes in use. */
  used() {
    return (Atomics.load(this.i32, HEAD) - Atomics.load(this.i32, TAIL)) >>> 0;
  }

  #put(pos, bytes) {
    const i = pos & (this.cap - 1);
    const first = Math.min(bytes.length, this.cap - i);
    this.ring.set(bytes.subarray(0, first), i);
    if (first < bytes.length) this.ring.set(bytes.subarray(first), 0);
  }

  #get(pos, len) {
    const out = new Uint8Array(len);
    const i = pos & (this.cap - 1);
    const first = Math.min(len, this.cap - i);
    out.set(this.ring.subarray(i, i + first), 0);
    if (first < len) out.set(this.ring.subarray(0, len - first), first);
    return out;
  }

  /** Producer: appends a message; false when it does not fit now (or ever: see `maxMessage`). */
  push(bytes) {
    const head = Atomics.load(this.i32, HEAD);
    const tail = Atomics.load(this.i32, TAIL);
    const free = this.cap - ((head - tail) >>> 0);
    const need = 4 + bytes.length;
    if (need > free) return false;
    const len = new Uint8Array(4);
    new DataView(len.buffer).setUint32(0, bytes.length, true);
    this.#put(head, len);
    this.#put(head + 4, bytes);
    Atomics.store(this.i32, HEAD, (head + need) | 0);
    Atomics.notify(this.i32, HEAD);
    return true;
  }

  /** Consumer: the next message (a copy), or null when the queue is empty. */
  pop() {
    const tail = Atomics.load(this.i32, TAIL);
    const head = Atomics.load(this.i32, HEAD);
    if (head === tail) return null;
    const len = new DataView(this.#get(tail, 4).buffer).getUint32(0, true);
    const msg = this.#get(tail + 4, len);
    Atomics.store(this.i32, TAIL, (tail + 4 + len) | 0);
    return msg;
  }

  /** Consumer: waits until the queue is not empty (at most `timeoutMs`); true when it is. */
  wait(timeoutMs = Infinity) {
    const tail = Atomics.load(this.i32, TAIL);
    for (;;) {
      const head = Atomics.load(this.i32, HEAD);
      if (head !== tail) return true;
      if (Atomics.wait(this.i32, HEAD, head, timeoutMs) === "timed-out") return Atomics.load(this.i32, HEAD) !== tail;
    }
  }
}

// ---------------------------------------------------------------------------------------------
// The data pack

/** The files of a data pack (`FFHPAK1\n`, then `<path>\n<length>\n<bytes>` per file):
 *  `[path, bytes]` with `bytes` a view into `pack`. */
export function parsePack(pack) {
  const magic = enc.encode("FFHPAK1\n");
  if (pack.length < magic.length || magic.some((b, i) => pack[i] !== b)) throw new Error("the data pack does not start with FFHPAK1");
  const out = [];
  let pos = magic.length;
  while (pos < pack.length) {
    const nl1 = pack.indexOf(10, pos);
    const nl2 = nl1 < 0 ? -1 : pack.indexOf(10, nl1 + 1);
    if (nl2 < 0) throw new Error(`the data pack is cut at byte ${pos}`);
    const path = dec.decode(pack.subarray(pos, nl1));
    const lenText = dec.decode(pack.subarray(nl1 + 1, nl2));
    if (!/^\d+$/.test(lenText)) throw new Error(`the data pack has a bad length for ${path}`);
    if (!path || path.startsWith("/") || path.split("/").some((c) => c === "" || c === "." || c === "..")) throw new Error(`the data pack has a bad path ${JSON.stringify(path)}`);
    const start = nl2 + 1;
    const end = start + Number(lenText);
    if (end > pack.length) throw new Error(`the data pack is cut in ${path}`);
    out.push([path, pack.subarray(start, end)]);
    pos = end;
  }
  return out;
}

/** Mounts a data pack's files under `root` (their bytes stay in the pack, uncopied). */
export function mountPack(fs, root, pack) {
  const files = parsePack(pack);
  fs.mkdirp(root);
  for (const [path, bytes] of files) fs.writeFile(`${root}/${path}`, bytes, { copy: false });
  return files.length;
}

// ---------------------------------------------------------------------------------------------
// Persistence of /saves

/** A store of the saves: `load()` resolves to `[key, record]` pairs, `apply(puts, deletes)`
 *  writes `[key, record]` pairs and removes keys, all or nothing. A record is
 *  `{kind: "file", data: Uint8Array, mtime}` or `{kind: "dir"}`; the key is the path under the
 *  saves directory (`game/auto/T0001.ffhsave`). This one keeps them in memory (`entries`). */
export function memoryStore(initial = []) {
  const entries = new Map(initial);
  return {
    kind: "memory",
    entries,
    async load() {
      return [...entries];
    },
    async apply(puts, deletes) {
      for (const k of deletes) entries.delete(k);
      for (const [k, v] of puts) entries.set(k, v);
    },
  };
}

function idbRequest(req) {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/** The store in IndexedDB: database `dbName`, object store `storeName` (out-of-line keys). */
export function idbStore(dbName = "ffh-play", storeName = "saves", idb = globalThis.indexedDB) {
  let dbp = null;
  const open = async () => {
    let db = await openDb(dbName, undefined, storeName, idb);
    if (!db.objectStoreNames.contains(storeName)) {
      // The database exists without the store (made by something else): add it.
      const v = db.version + 1;
      db.close();
      db = await openDb(dbName, v, storeName, idb);
    }
    db.onversionchange = () => {
      db.close();
      dbp = null;
    };
    return db;
  };
  const db = () => (dbp ??= open().catch((e) => {
    dbp = null;
    throw e;
  }));
  return {
    kind: "indexeddb",
    async load() {
      const d = await db();
      const st = d.transaction(storeName, "readonly").objectStore(storeName);
      const [keys, values] = await Promise.all([idbRequest(st.getAllKeys()), idbRequest(st.getAll())]);
      return keys.map((k, i) => [String(k), values[i]]);
    },
    async apply(puts, deletes) {
      const d = await db();
      await new Promise((resolve, reject) => {
        const tx = d.transaction(storeName, "readwrite");
        const st = tx.objectStore(storeName);
        for (const k of deletes) st.delete(k);
        for (const [k, v] of puts) st.put(v, k);
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
        tx.onabort = () => reject(tx.error ?? new Error("the IndexedDB transaction was aborted"));
      });
    },
  };
}

function openDb(name, version, storeName, idb) {
  return new Promise((resolve, reject) => {
    const req = version === undefined ? idb.open(name) : idb.open(name, version);
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains(storeName)) req.result.createObjectStore(storeName);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
    req.onblocked = () => reject(new Error(`IndexedDB ${name} is blocked by another tab`));
  });
}

/** Keeps a store in step with the directory `root` of a MemFS: `load()` fills the directory
 *  from the store; `flush()` writes what changed since the last flush (by file identity and
 *  write count, so a file renamed over another, a removed directory tree or a rewritten file
 *  are all seen) and removes what is gone. Flushes run one at a time; a flush asked while one
 *  runs is done after it (once for all who asked). A failed flush is retried by the next one. */
export class SavesPersister {
  constructor(fs, root, store) {
    this.fs = fs;
    this.root = root.replace(/\/$/, "");
    this.store = store;
    /** key -> signature of what the store holds. */
    this.base = new Map();
    this.gen = -1;
    this.chain = Promise.resolve();
    this.next = null;
  }

  async load() {
    const rows = await this.store.load();
    rows.sort((a, b) => a[0].length - b[0].length);
    this.fs.mkdirp(this.root);
    for (const [key, rec] of rows) {
      if (!rec || typeof key !== "string" || key.split("/").some((c) => c === "" || c === "." || c === "..")) continue;
      const path = `${this.root}/${key}`;
      try {
        if (rec.kind === "dir") this.fs.mkdirp(path);
        else if (rec.kind === "file") this.fs.writeFile(path, rec.data instanceof Uint8Array ? rec.data : new Uint8Array(rec.data ?? 0), { copy: false, mtimeMs: rec.mtime });
      } catch (e) {
        console.warn(`saves: cannot restore ${key}: ${e.message ?? e}`);
      }
    }
    this.base = this.#signatures();
    this.gen = this.fs.generation;
    return rows.length;
  }

  #signatures() {
    const out = new Map();
    this.fs.walk(this.root, (rel, node) => out.set(rel, node.kind === "dir" ? "d" : `f${node.ino}:${node.gen}`));
    return out;
  }

  /** What differs from the store now: `{puts, deletes, sigs}` (the records are copies). */
  diff() {
    const sigs = new Map();
    const puts = [];
    this.fs.walk(this.root, (rel, node) => {
      const sig = node.kind === "dir" ? "d" : `f${node.ino}:${node.gen}`;
      sigs.set(rel, sig);
      if (this.base.get(rel) === sig) return;
      puts.push([rel, node.kind === "dir" ? { kind: "dir" } : { kind: "file", data: node.data.slice(0, node.size), mtime: Number(node.mtim / 1000000n) }]);
    });
    const deletes = [...this.base.keys()].filter((k) => !sigs.has(k));
    return { puts, deletes, sigs };
  }

  /** Writes the changes (see the class); resolves to `{puts, deletes}` counts. */
  flush() {
    if (this.next) return this.next;
    const run = this.chain.then(() => {
      this.next = null;
      return this.#flushNow();
    });
    this.next = run;
    this.chain = run.catch(() => {});
    return run;
  }

  async #flushNow() {
    const gen = this.fs.generation;
    if (gen === this.gen) return { puts: 0, deletes: 0 };
    const { puts, deletes, sigs } = this.diff();
    if (puts.length || deletes.length) await this.store.apply(puts, deletes);
    this.base = sigs;
    this.gen = gen;
    return { puts: puts.length, deletes: deletes.length };
  }
}

// ---------------------------------------------------------------------------------------------
// Downloads

/** The bytes at `url`, gunzipped when they are gzip (magic 1f 8b: a server that already
 *  decoded a Content-Encoding hands over the plain bytes); `onProgress(loaded, total)` follows
 *  the download (`total` 0 when unknown). */
export async function fetchBytes(url, onProgress = () => {}, fetchFn = globalThis.fetch, init = {}) {
  const r = await fetchFn(url, init);
  if (!r.ok) throw new Error(`${url}: HTTP ${r.status}`);
  const total = Number(r.headers.get("Content-Length")) || 0;
  let bytes;
  if (r.body && typeof r.body.getReader === "function") {
    const reader = r.body.getReader();
    const chunks = [];
    let loaded = 0;
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
      loaded += value.length;
      onProgress(loaded, Math.max(total, loaded));
    }
    bytes = new Uint8Array(loaded);
    let off = 0;
    for (const c of chunks) {
      bytes.set(c, off);
      off += c.length;
    }
  } else {
    bytes = new Uint8Array(await r.arrayBuffer());
  }
  onProgress(bytes.length, bytes.length);
  if (bytes.length >= 2 && bytes[0] === 0x1f && bytes[1] === 0x8b) bytes = await gunzip(bytes);
  return bytes;
}

export async function gunzip(bytes) {
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

// ---------------------------------------------------------------------------------------------
// The worker

function describe(e) {
  if (e instanceof WasiExit) return `the engine exited (code ${e.code})`;
  return String(e?.message ?? e);
}

/** Runs the worker. `env`: `post(msg, transfer)`, `listen(handler)` (the page's messages, the
 *  message itself), and optionally `fetch`, `openStore()` (the saves store; IndexedDB when the
 *  worker has it, else memory), `console`, `fetchInit` (passed to fetch: the browser revalidates
 *  the engine and the data with `cache: "no-cache"`). */
export function workerMain(env) {
  const post = env.post;
  const log = env.console ?? console;
  let state = "waiting"; // loading, ready, dead
  let queue = null;
  const local = [];
  let busy = false;
  let exp = null;
  let memory = null;
  let fs = null;
  let persister = null;
  let pulled = null;
  let crashMessage = null;
  let lastProgress = 0;
  let persistFailed = false;

  const mem = () => new Uint8Array(memory.buffer);

  function progress(file, loaded, total) {
    const now = Date.now();
    if (loaded < total && now - lastProgress < 80) return;
    lastProgress = now;
    post({ t: "progress", file, loaded, total });
  }

  const ffh = {
    reply(token, ptr, len) {
      const bytes = mem().slice(ptr, ptr + len);
      post({ t: "reply", token, bytes: bytes.buffer }, [bytes.buffer]);
    },
    publish(ptr, len) {
      post({ t: "publish", status: dec.decode(mem().slice(ptr, ptr + len)) });
    },
    pull(block) {
      if (!queue) return -1;
      let msg = queue.pop();
      while (!msg && block) {
        queue.wait();
        msg = queue.pop();
      }
      if (!msg) return 0;
      pulled = msg;
      return msg.length;
    },
    pull_copy(ptr) {
      if (pulled) mem().set(pulled, ptr);
      pulled = null;
    },
    crash(ptr, len) {
      crashMessage = dec.decode(mem().slice(ptr, ptr + len));
      post({ t: "crash", message: crashMessage });
    },
  };

  /** Calls `fn(ptr, len)` with `bytes` copied into the engine's memory, freed after (the
   *  engine borrows the bytes for the call; after a trap nothing more is called). */
  function withBytes(fn, bytes) {
    const ptr = exp.ffh_alloc(bytes.length);
    mem().set(bytes, ptr);
    const r = fn(ptr, bytes.length);
    exp.ffh_free(ptr, bytes.length);
    return r;
  }

  async function init(msg) {
    state = "loading";
    queue = msg.queue ? new RequestQueue(msg.queue) : null;
    fs = new MemFS();
    const openStore = env.openStore ?? (() => (globalThis.indexedDB ? idbStore() : memoryStore()));
    const storeP = Promise.resolve()
      .then(openStore)
      .then(async (store) => {
        const p = new SavesPersister(fs, "/saves", store);
        await p.load();
        return p;
      })
      .catch(async (e) => {
        post({ t: "note", message: `The saves cannot be kept in this browser (${describe(e)}); they last until the page closes.` });
        const p = new SavesPersister(fs, "/saves", memoryStore());
        await p.load();
        return p;
      });
    const fetchFn = env.fetch ?? globalThis.fetch;
    const [wasm, pack] = await Promise.all([
      fetchBytes(msg.wasm, (l, t) => progress("engine", l, t), fetchFn, env.fetchInit ?? {}),
      fetchBytes(msg.data, (l, t) => progress("data", l, t), fetchFn, env.fetchInit ?? {}),
    ]);
    post({ t: "progress", stage: "start" });
    const module = await WebAssembly.compile(wasm);
    mountPack(fs, "/data", pack);
    persister = await storeP;
    fs.takeDirty();
    const wasi = new WASI({
      args: ["ffh-web"],
      env: { FFH_DATA_DIR: "/data", ...(msg.env ?? {}) },
      fs,
      stdout: (line) => log.log(`[engine] ${line}`),
      stderr: (line) => log.warn(`[engine] ${line}`),
    });
    const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: wasi.imports, ffh });
    exp = instance.exports;
    memory = exp.memory;
    wasi.initialize(instance);
    const opts = JSON.stringify({ saves: "/saves", autosave_keep: msg.autosaveKeep ?? 20, blocking: !!queue });
    const rc = withBytes(exp.ffh_init, enc.encode(opts));
    if (rc !== 0) throw new Error(`the engine refused its options ${opts} (ffh_init returned ${rc}; see the console)`);
    state = "ready";
    post({ t: "ready", isolated: !!queue, store: persister.store.kind });
    pump();
  }

  /** Serves the queued requests, one at a time, until none is left; then persists. */
  function pump() {
    if (state !== "ready" || busy) return;
    busy = true;
    try {
      for (;;) {
        const req = local.shift() ?? queue?.pop();
        if (!req) break;
        withBytes(exp.ffh_request, req);
      }
    } catch (e) {
      busy = false;
      die(e);
      return;
    }
    busy = false;
    persist().catch(() => {});
  }

  /** Writes the changed saves to the store; a failure is said once (and retried by the next). */
  function persist() {
    return persister.flush().then(
      (r) => {
        persistFailed = false;
        return r;
      },
      (e) => {
        log.warn("saves: cannot write to the store:", e);
        if (!persistFailed) post({ t: "note", message: `The saves could not be written to the browser's storage (${describe(e)}).` });
        persistFailed = true;
        throw e;
      },
    );
  }

  async function die(e) {
    if (state === "dead") return;
    state = "dead";
    log.error("the engine stopped:", e);
    try {
      // (bounded: a store that hangs must not keep the page from restarting the engine)
      await Promise.race([persist(), new Promise((r) => setTimeout(r, 10000))]);
    } catch {
      /* reported */
    }
    post({ t: "dead", crash: crashMessage, error: describe(e) });
  }

  env.listen((msg) => {
    switch (msg?.t) {
      case "init":
        if (state !== "waiting") return;
        init(msg).catch((e) => {
          state = "dead";
          log.error("the engine could not start:", e);
          post({ t: "fail", message: crashMessage ? `the engine crashed while starting: ${crashMessage}` : describe(e) });
        });
        break;
      case "req":
        local.push(new Uint8Array(msg.bytes));
        pump();
        break;
      case "kick":
        pump();
        break;
      case "flush":
        if (state === "ready" && persister) {
          persist().then(
            () => post({ t: "flushed", id: msg.id, ok: true }),
            (e) => post({ t: "flushed", id: msg.id, ok: false, error: describe(e) }),
          );
        } else {
          post({ t: "flushed", id: msg.id, ok: state !== "dead" && state !== "waiting" ? true : false });
        }
        break;
    }
  });
}

// Inside a dedicated worker: run.
if (typeof WorkerGlobalScope !== "undefined" && globalThis instanceof WorkerGlobalScope) {
  workerMain({
    post: (msg, transfer) => globalThis.postMessage(msg, transfer ?? []),
    listen: (handler) => globalThis.addEventListener("message", (ev) => handler(ev.data)),
    fetchInit: { cache: "no-cache" },
  });
}
