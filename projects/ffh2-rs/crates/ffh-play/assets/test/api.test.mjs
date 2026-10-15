// node --test: the HTTP protocol layer (app/api.js) against a fake fetch: the envelope, the
// X-FFH header, request ids, refusals, 409 and ETags, and the long-poll loop.
import test from "node:test";
import assert from "node:assert/strict";
import * as api from "../app/api.js";

function fakeFetch(handler) {
  const calls = [];
  const f = async (url, init = {}) => {
    calls.push({ url, init });
    // Like real I/O, a reply comes in a later turn of the event loop.
    await new Promise((r) => setImmediate(r));
    const { status = 200, body = {}, headers = {} } = await handler(url, init, calls.length);
    return {
      status,
      headers: { get: (k) => headers[k] ?? null },
      text: async () => (typeof body === "string" ? body : JSON.stringify(body)),
    };
  };
  return { f, calls };
}

test("envelopes: header, ids, replies and refusals", async () => {
  const { f, calls } = fakeFetch((url, init) => {
    const req = JSON.parse(init.body);
    if (req.k === "cmd" && req.c === "end_turn") return { body: { id: req.id, ok: false, error: "not_your_turn", detail: "the game runs" } };
    return { body: { id: req.id, ok: true, r: { echo: req } } };
  });
  api.configure({ url: "http://h:1/", fetch: f });
  const r = await api.query("plot", { x: 1, y: 2 });
  assert.deepEqual({ ...r.echo, id: 0 }, { k: "q", q: "plot", x: 1, y: 2, id: 0 });
  assert.equal(calls[0].url, "http://h:1/api");
  assert.equal(calls[0].init.method, "POST");
  assert.equal(calls[0].init.headers["X-FFH"], "1");
  const a = JSON.parse(calls[0].init.body).id;
  await api.query("status");
  assert.equal(JSON.parse(calls[1].init.body).id, a + 1);
  // A refusal throws an ApiError with the code; `send` returns it.
  await assert.rejects(api.cmd("end_turn"), (e) => e instanceof api.ApiError && e.code === "not_your_turn" && e.detail === "the game runs");
  const raw = await api.send({ k: "cmd", c: "end_turn", id: "mine" });
  assert.equal(raw.ok, false);
  assert.equal(JSON.parse(calls[3].init.body).id, "mine");
  // Answers: an option or the advisor.
  await api.answer(17, 2);
  assert.deepEqual(JSON.parse(calls[4].init.body).option, 2);
  await api.answer(17, null);
  assert.equal(JSON.parse(calls[5].init.body).advisor, true);
  await api.session("save", { name: "x" });
  assert.deepEqual(JSON.parse(calls[6].init.body).op, "save");
});

test("transport failures are NetErrors", async () => {
  api.configure({ url: "", fetch: async () => { throw new Error("refused"); } });
  await assert.rejects(api.query("status"), (e) => e instanceof api.NetError && /refused/.test(e.message));
  const { f } = fakeFetch(() => ({ status: 503, body: "busy" }));
  api.configure({ fetch: f });
  await assert.rejects(api.query("status"), (e) => e instanceof api.NetError && e.status === 503);
});

test("GETs: 409 is no game; ETags", async () => {
  const { f, calls } = fakeFetch((url, init) => {
    if (url.endsWith("/api/view")) return { status: 409, body: { ok: false, error: "no_game" } };
    if (url.endsWith("/api/data")) {
      if (init.headers?.["If-None-Match"] === '"t1"') return { status: 304, body: "", headers: { ETag: '"t1"' } };
      return { body: { kinds: [] }, headers: { ETag: '"t1"' } };
    }
    return { body: { version: 4, phase: "human" } };
  });
  api.configure({ url: "", fetch: f });
  assert.equal(await api.get("/api/view"), null);
  assert.deepEqual(await api.get("/api/data", { etag: "" }), { etag: '"t1"', body: { kinds: [] } });
  assert.deepEqual(await api.get("/api/data", { etag: '"t1"' }), { etag: '"t1"', body: null });
  assert.equal((await api.status()).version, 4);
  assert.equal(calls.at(-1).url, "/api/status");
});

test("watch: status first, then long-polls since the version; retries after errors", async () => {
  let n = 0;
  const seen = [];
  const errors = [];
  const { f, calls } = fakeFetch((url) => {
    n++;
    if (n === 3) throw new Error("dropped");
    if (url === "/api/status") return { body: { version: n === 1 ? 1 : 5, phase: "human" } };
    const since = Number(/since=(\d+)/.exec(url)[1]);
    return { body: { version: Math.min(since + 1, 6), phase: "running" } };
  });
  api.configure({ url: "", fetch: f });
  const stop = api.watch((st) => seen.push(st.version), (e) => errors.push(e), { retryMs: 5 });
  await new Promise((r) => setTimeout(r, 120));
  stop();
  assert.equal(calls[0].url, "/api/status");
  assert.match(calls[1].url, /^\/api\/wait\?since=1&timeout_ms=25000$/);
  assert.equal(errors.length, 1);
  // After the error it starts again from /api/status.
  assert.equal(calls[3].url, "/api/status");
  assert.deepEqual(seen.slice(0, 4), [1, 2, 5, 6]);
});
