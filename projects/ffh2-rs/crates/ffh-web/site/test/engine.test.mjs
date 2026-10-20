// The page side end to end in node: boot.js's EngineClient (with installFetch, driven by the
// client's own api.js) and engine.js's worker (in a worker thread, with wasi.js) around a stand-in
// engine with the real ABI (mock_engine.rs), downloaded from a local HTTP server: status and
// long-polls answered by the page, Responses with headers and 304, requests pulled while the
// engine runs and inside a blocking decision (isolated) or the advisor's pick (not isolated),
// abort, oversized bodies, saves persisted, a crash (the status `crashed` with the report, the
// pending POST answered, a fresh worker that sees the saves, the crash cleared by a new game),
// and an engine that cannot load. Skipped when rustc or its wasm32-wasip1 target is missing.
import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { Worker } from "node:worker_threads";
import { EngineClient, installFetch, crashReport, MAX_BODY } from "../boot.js";
import * as api from "../../../ffh-play/assets/app/api.js";

const here = fileURLToPath(new URL(".", import.meta.url));
const ORIGIN = "http://page.test";

function rustMissing() {
  const v = spawnSync("rustc", ["--print", "sysroot"], { encoding: "utf8" });
  if (v.error || v.status !== 0) return "rustc not found";
  if (!existsSync(join(v.stdout.trim(), "lib", "rustlib", "wasm32-wasip1"))) return "the wasm32-wasip1 target is not installed";
  return null;
}

// The worker thread: engine.js's workerMain, with a saves store in files (so that it outlives a
// worker, as IndexedDB does).
const SHIM = `
import { workerData, parentPort } from "node:worker_threads";
import { mkdirSync, readdirSync, readFileSync, writeFileSync, rmSync } from "node:fs";
const { workerMain } = await import(workerData.engine);
const dir = workerData.storeDir;
mkdirSync(dir, { recursive: true });
const store = {
  kind: "files",
  async load() {
    return readdirSync(dir).map((f) => {
      const rec = JSON.parse(readFileSync(dir + "/" + f, "utf8"));
      if (rec.data !== undefined) rec.data = Buffer.from(rec.data, "base64");
      return [decodeURIComponent(f), rec];
    });
  },
  async apply(puts, deletes) {
    for (const k of deletes) rmSync(dir + "/" + encodeURIComponent(k), { force: true });
    for (const [k, v] of puts) writeFileSync(dir + "/" + encodeURIComponent(k), JSON.stringify(v.kind === "file" ? { ...v, data: Buffer.from(v.data).toString("base64") } : v));
  },
};
const quiet = { log() {}, warn() {}, error() {} };
workerMain({
  post: (m, t) => parentPort.postMessage(m, t),
  listen: (h) => parentPort.on("message", h),
  openStore: () => store,
  console: quiet,
});
`;

function spawner(storeDir, workers) {
  return () => {
    const w = new Worker(new URL(`data:text/javascript,${encodeURIComponent(SHIM)}`), { workerData: { engine: new URL("../engine.js", import.meta.url).href, storeDir } });
    workers.push(w);
    return {
      post: (m, t) => w.postMessage(m, t),
      onmessage: (fn) => w.on("message", fn),
      onerror: (fn) => w.on("error", (e) => fn(String(e?.message ?? e))),
      terminate: () => w.terminate(),
    };
  };
}

async function until(what, f, ms = 10000) {
  const t0 = Date.now();
  for (;;) {
    const v = await f();
    if (v) return v;
    if (Date.now() - t0 > ms) throw new Error(`timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 10));
  }
}

let server;
let base;
let dir;
let skip = null;

test.before(async () => {
  skip = rustMissing();
  if (skip) return;
  dir = mkdtempSync(join(tmpdir(), "ffh-web-e2e-"));
  const r = spawnSync("rustc", ["--edition", "2021", "-O", "--target", "wasm32-wasip1", "--crate-type", "cdylib", join(here, "mock_engine.rs"), "-o", join(dir, "mock.wasm")], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  const enc = new TextEncoder();
  const pack = Buffer.concat([enc.encode("FFHPAK1\n"), enc.encode("xml/test.xml\n10\nhello data")]);
  const files = {
    "/web/ffh.wasm.gz": gzipSync(readFileSync(join(dir, "mock.wasm")), { level: 9 }),
    // Uncompressed: the worker only gunzips what starts with the gzip magic.
    "/web/data.pak.gz": pack,
  };
  server = createServer((req, res) => {
    const body = files[req.url];
    if (!body) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "Content-Type": "application/octet-stream", "Content-Length": body.length }).end(body);
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  base = `http://127.0.0.1:${server.address().port}`;
});

test.after(() => {
  server?.close();
  if (dir) rmSync(dir, { recursive: true, force: true });
});

/** A page: an EngineClient, `fetch` routed through installFetch, api.js pointed at it. */
function page(name, isolated, workers) {
  const storeDir = join(dir, `store-${name}`);
  const events = [];
  const client = new EngineClient({
    spawn: spawner(storeDir, workers),
    isolated,
    wasmUrl: `${base}/web/ffh.wasm.gz`,
    dataUrl: `${base}/web/data.pak.gz`,
    ui: {
      progress: (m) => events.push(m.stage ?? m.file),
      ready: (m) => events.push(`ready isolated=${m.isolated}`),
      restarting: () => events.push("restarting"),
      note: (m) => events.push(`note ${m}`),
    },
  });
  const target = { fetch: async (input) => new Response(`static ${input}`) };
  installFetch(client, { target, origin: ORIGIN, base: `${ORIGIN}/game/` });
  api.configure({ url: "", fetch: (...a) => target.fetch(...a) });
  return { client, target, events, storeDir };
}

const json = async (r) => [r.status, await r.json()];

for (const isolated of [true, false]) {
  test(`the page and the engine worker (${isolated ? "isolated" : "not isolated"})`, { timeout: 60000 }, async (t) => {
    if (skip) return t.skip(skip);
    const workers = [];
    const { client, target, events, storeDir } = page(isolated ? "iso" : "plain", isolated, workers);
    const f = (path, init) => target.fetch(path, init);
    try {
      client.start();
      // Before the engine is up: the status waits for its first publish.
      const st0 = await api.status();
      assert.equal(st0.phase, "no_game");
      assert.equal(st0.version, 1);
      assert.equal(st0.data, `hello data|blocking=${isolated}`);
      assert.ok(st0.ms_in_phase >= 5);
      await until("ready", () => client.state === "ready");
      assert.deepEqual(events.filter((e) => e === "start" || e.startsWith("ready")), ["start", `ready isolated=${isolated}`]);
      assert.ok(events.includes("engine") && events.includes("data"));

      // Other URLs go to the real fetch; /api under another path or origin too.
      assert.equal(await (await f("app/main.js")).text(), "static app/main.js");
      assert.equal(await (await f("http://elsewhere.test/api/status")).text(), "static http://elsewhere.test/api/status");

      // Responses: status, headers, body; the request as the engine saw it.
      let [s, j] = await json(await f("/api/echo?since=3&epoch=x", { headers: { "X-FFH": "1" } }));
      assert.deepEqual([s, j], [200, { m: "GET", p: "/api/echo", q: "since=3&epoch=x", xffh: true, body: 0 }]);
      const a = await api.get("/api/etag", { etag: "\"old\"" });
      assert.deepEqual(a, { etag: "\"v1\"", body: { tables: 1 } });
      const b = await api.get("/api/etag", { etag: a.etag });
      assert.deepEqual(b, { etag: "\"v1\"", body: null });
      const r304 = await f("/api/etag", { headers: { "If-None-Match": "\"v1\"" } });
      assert.equal(r304.status, 304);
      assert.equal(r304.headers.get("Cache-Control"), "no-cache");
      assert.equal(await api.get("/api/view"), null); // 409: no game
      [s, j] = await json(await f("/api/nothing"));
      assert.deepEqual([s, j.error], [404, "not_found"]);
      [s, j] = await json(await f("/api", { method: "POST", body: "{}" }));
      assert.deepEqual([s, j.error], [403, "forbidden"]);
      [s, j] = await json(await f("/api/echo", { method: "PUT", body: "x" }));
      assert.equal(s, 405);
      [s, j] = await json(await f("/api", { method: "POST", headers: { "X-FFH": "1" }, body: "x".repeat(MAX_BODY + 1) }));
      assert.deepEqual([s, j.error], [413, "http"]);
      assert.deepEqual((await api.call({ op: "ping" })).r, "pong");

      // The long-poll: answered by the page at once for an older version, held until a
      // publish, after its timeout (same version), and aborted.
      let st = await api.status();
      assert.equal((await api.wait(st.version - 1, 5000)).version, st.version);
      const t0 = Date.now();
      assert.equal((await api.wait(st.version, 120)).version, st.version);
      assert.ok(Date.now() - t0 >= 100);
      const ctl = new AbortController();
      const aborted = api.wait(st.version, 20000, ctl.signal);
      setTimeout(() => ctl.abort(), 20);
      await assert.rejects(aborted, (e) => e.name === "AbortError");
      const seen = [];
      const stop = api.watch((x) => seen.push(x.phase), () => {}, { timeoutMs: 2000 });
      const waiting = api.wait(st.version, 20000);
      await api.session("new", { op: "new", name: `g${isolated ? 1 : 2}` });
      st = await waiting;
      assert.equal(st.phase, "human");
      assert.equal(st.game.journal, `/saves/g${isolated ? 1 : 2}/journal.jsonl`);
      await until("the watch to see the game", () => seen.includes("human"));
      stop();
      assert.equal((await api.get("/api/view")).game, `g${isolated ? 1 : 2}`);

      // A request that runs a while: isolated, a GET sent meanwhile is pulled and answered
      // before it ends; not isolated, after.
      const order = [];
      const slow = api.call({ op: "slow", n: 40 }).then((r) => (order.push("slow"), r));
      await until("running", async () => (await api.status()).phase === "running");
      const echo = f("/api/echo?during").then((r) => (order.push("echo"), r.json()));
      const [sr, er] = await Promise.all([slow, echo]);
      assert.equal(er.q, "during");
      if (isolated) {
        assert.deepEqual(order, ["echo", "slow"]);
        assert.equal(sr.r.pulled, 1);
      } else {
        assert.deepEqual(order, ["slow", "echo"]);
        assert.equal(sr.r.pulled, 0);
      }

      // A blocking decision: isolated, the engine waits for the answer and serves requests
      // meanwhile; not isolated, the advisor answers at once.
      const block = api.call({ op: "block" });
      if (isolated) {
        await until("the decision", async () => (await api.status()).phase === "decision");
        assert.equal((await (await f("/api/echo?in-decision")).json()).q, "in-decision");
        const ans = await api.call({ op: "answer" });
        assert.equal(ans.r, "answered");
        assert.equal((await block).r, "decided");
      } else {
        assert.equal((await block).r, "advisor");
      }

      // A request aborted while the engine has it: rejected; the next ones still answered.
      const ctl2 = new AbortController();
      const gone = f("/api", { method: "POST", headers: { "X-FFH": "1" }, body: JSON.stringify({ id: 99, op: "slow", n: 10 }), signal: ctl2.signal });
      setTimeout(() => ctl2.abort(), 5);
      await assert.rejects(gone, (e) => e.name === "AbortError");
      assert.equal((await api.call({ op: "ping" })).r, "pong");

      // Saves: written by the engine, in the store once flushed.
      await api.call({ op: "write", name: "save.txt", text: "kept" });
      assert.equal(await client.flush(), true);
      assert.ok(readdirSync(storeDir).includes(encodeURIComponent("g/save.txt")));

      // A crash: the POST gets the crashed reply, the status turns `crashed` with the report
      // (the autosave and journal of the session), a fresh worker starts and the status stays
      // crashed until a game starts; the saves are there.
      const before = (await api.status()).version;
      const crashed = await api.send({ id: 7, op: "panic" });
      assert.deepEqual(crashed, { id: 7, ok: false, error: "crashed", detail: "the game crashed while handling this request: see /api/status" });
      st = await api.status();
      assert.equal(st.phase, "crashed");
      assert.ok(st.version > before);
      assert.equal(st.crash.message, "mock panic for request 7");
      assert.match(st.crash.location, /^mock_engine\.rs:\d+$/);
      const g = `g${isolated ? 1 : 2}`;
      assert.equal(st.crash.autosave, `/saves/${g}/auto/T0001.ffhsave`);
      assert.equal(st.crash.repro, `ffh-play replay /saves/${g}/journal.jsonl --from /saves/${g}/auto/T0001.ffhsave`);
      await until("the restart", () => client.state === "ready" && workers.length === 2);
      assert.ok(events.includes("restarting"));
      st = await api.status();
      assert.equal(st.phase, "crashed");
      assert.equal((await api.call({ op: "read", name: "save.txt" })).r, "kept");
      const v = st.version;
      await api.call({ op: "new", name: "after" });
      st = await api.status();
      assert.equal(st.phase, "human");
      assert.ok(st.version > v);
      assert.equal(st.crash, undefined);
    } finally {
      for (const w of workers) await w.terminate();
    }
  });
}

test("an engine that cannot load: requests fail like a lost server", { timeout: 30000 }, async (t) => {
  if (skip) return t.skip(skip);
  const workers = [];
  const failures = [];
  const client = new EngineClient({
    spawn: spawner(join(dir, "store-fail"), workers),
    wasmUrl: `${base}/web/missing.wasm.gz`,
    dataUrl: `${base}/web/data.pak.gz`,
    ui: { fail: (m) => failures.push(m) },
  });
  try {
    client.start();
    const r = client.handle(new Request(`${ORIGIN}/api/data`));
    const s = client.handle(new Request(`${ORIGIN}/api/status`));
    await assert.rejects(r, TypeError);
    await assert.rejects(s, TypeError);
    assert.equal(client.state, "failed");
    assert.match(failures[0], /missing\.wasm\.gz: HTTP 404/);
    await assert.rejects(client.handle(new Request(`${ORIGIN}/api/view`)), TypeError);
  } finally {
    for (const w of workers) await w.terminate();
  }
});

test("crash reports from the engine's panic text", () => {
  const game = { game: { journal: "/saves/g/journal.jsonl", autosave: "/saves/g/auto/T0003.ffhsave" } };
  assert.deepEqual(crashReport("index out of bounds (at ffh-core/src/unit.rs:12)", game), {
    message: "index out of bounds",
    location: "ffh-core/src/unit.rs:12",
    autosave: "/saves/g/auto/T0003.ffhsave",
    journal: "/saves/g/journal.jsonl",
    repro: "ffh-play replay /saves/g/journal.jsonl --from /saves/g/auto/T0003.ffhsave",
  });
  assert.deepEqual(crashReport("panicked at src/a.rs:3:9:\nboom", null), { message: "boom", location: "src/a.rs:3:9", autosave: null, journal: null, repro: null });
  assert.equal(crashReport("the engine stopped: RuntimeError: unreachable", null).location, "?");
});
