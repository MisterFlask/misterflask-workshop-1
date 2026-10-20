// engine.js `RequestQueue`: the SharedArrayBuffer queue from the page (producer) to the engine
// worker (consumer): wrap-around of the ring and of the 32-bit counters, full and empty, large
// messages, and many messages against a consumer in another thread that blocks in
// Atomics.wait (as the engine's `pull(1)` does).
import test from "node:test";
import assert from "node:assert/strict";
import { Worker } from "node:worker_threads";
import { RequestQueue } from "../engine.js";

function msg(n, seed) {
  const b = new Uint8Array(n);
  for (let i = 0; i < n; i++) b[i] = (seed * 31 + i * 7) & 0xff;
  return b;
}

test("push and pop in order; full and empty", () => {
  const q = RequestQueue.create(64);
  assert.equal(q.maxMessage, 60);
  assert.equal(q.pop(), null);
  assert.ok(q.push(msg(10, 1)));
  assert.ok(q.push(msg(0, 2)));
  assert.ok(q.push(msg(20, 3)));
  assert.equal(q.used(), 14 + 4 + 24);
  // 22 bytes free: a 20-byte message needs 24.
  assert.equal(q.push(msg(20, 4)), false);
  assert.ok(q.push(msg(18, 5)));
  assert.equal(q.used(), 64);
  assert.equal(q.push(msg(0, 6)), false);
  assert.deepEqual(q.pop(), msg(10, 1));
  assert.deepEqual(q.pop(), msg(0, 2));
  assert.deepEqual(q.pop(), msg(20, 3));
  assert.deepEqual(q.pop(), msg(18, 5));
  assert.equal(q.pop(), null);
  assert.equal(q.used(), 0);
});

test("messages wrap around the ring's end (the length prefix too)", () => {
  const q = RequestQueue.create(32);
  // Every start offset modulo 32, with lengths that put the prefix and the body across the end.
  for (let round = 0; round < 200; round++) {
    const n = (round * 5) % 29;
    const a = msg(n, round);
    const b = msg(28 - n - 4 > 0 ? 28 - n - 4 : 0, round + 1000);
    assert.ok(q.push(a), `round ${round}`);
    if (4 + n + 4 + b.length <= 32) assert.ok(q.push(b));
    assert.deepEqual(q.pop(), a);
    if (4 + n + 4 + b.length <= 32) assert.deepEqual(q.pop(), b);
    assert.equal(q.pop(), null);
  }
});

test("the 32-bit counters wrap", () => {
  const q = RequestQueue.create(64);
  // Start just below 2^32 (as Int32: -20).
  q.i32[0] = -20;
  q.i32[1] = -20;
  for (let i = 0; i < 50; i++) {
    const m = msg(i % 50, i);
    assert.ok(q.push(m));
    assert.equal(q.used(), 4 + m.length);
    assert.deepEqual(q.pop(), m);
  }
  assert.ok(q.i32[0] > 0, "the write counter went past 2^32");
});

test("large messages: the largest that fits, and one too large", () => {
  const q = RequestQueue.create(1 << 16);
  const big = msg(q.maxMessage, 9);
  assert.ok(q.push(big));
  assert.equal(q.push(new Uint8Array(0)), false);
  assert.deepEqual(q.pop(), big);
  assert.equal(q.push(msg(q.maxMessage + 1, 1)), false);
  // Off-centre: half the ring used and freed first, so the big message wraps.
  assert.ok(q.push(msg(30000, 2)));
  assert.equal(q.pop().length, 30000);
  assert.ok(q.push(big));
  assert.deepEqual(q.pop(), big);
});

test("wait: times out when empty, returns when a message comes", () => {
  const q = RequestQueue.create(64);
  const t0 = Date.now();
  assert.equal(q.wait(30), false);
  assert.ok(Date.now() - t0 >= 25);
  q.push(msg(3, 3));
  assert.equal(q.wait(1000), true);
});

const CONSUMER = `
import { workerData, parentPort } from "node:worker_threads";
const { RequestQueue } = await import(workerData.engine);
const q = new RequestQueue(workerData.sab);
let n = 0, bytes = 0, bad = 0, sum = 0;
for (;;) {
  let m = q.pop();
  // Like the engine's pull(1): block until a message comes.
  while (!m) { q.wait(); m = q.pop(); }
  if (m.length === 1 && m[0] === 255) break;
  const seed = new DataView(m.buffer).getUint32(0, true);
  if (seed !== n) bad++;
  for (let i = 4; i < m.length; i++) if (m[i] !== ((seed * 31 + i * 7) & 0xff)) { bad++; break; }
  for (let i = 0; i < m.length; i++) sum = (sum + m[i]) % 1000000007;
  n++; bytes += m.length;
  // Now and then a slow consumer, so that the producer meets a full queue.
  if (n % 500 === 0) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 5);
}
parentPort.postMessage({ n, bytes, bad, sum });
`;

test("many messages to a consumer thread that blocks in Atomics.wait", { timeout: 60000 }, async () => {
  const q = RequestQueue.create(1 << 12);
  const w = new Worker(new URL(`data:text/javascript,${encodeURIComponent(CONSUMER)}`), { workerData: { sab: q.sab, engine: new URL("../engine.js", import.meta.url).href } });
  const done = new Promise((resolve, reject) => {
    w.on("message", resolve);
    w.on("error", reject);
  });
  const N = 20000;
  let sent = 0;
  let bytes = 0;
  let sum = 0;
  let full = 0;
  let rand = 12345;
  const next = () => (rand = (rand * 1103515245 + 12345) >>> 0);
  while (sent < N) {
    // Sizes from 4 bytes to a quarter of the ring, a few up to the largest.
    const r = next();
    const n = r % 97 === 0 ? q.maxMessage : 4 + (r % 1024);
    const m = msg(n, sent);
    new DataView(m.buffer).setUint32(0, sent, true);
    if (!q.push(m)) {
      // The page never blocks: it holds the request and retries later.
      full++;
      await new Promise((res) => setImmediate(res));
      continue;
    }
    for (let i = 0; i < m.length; i++) sum = (sum + m[i]) % 1000000007;
    sent++;
    bytes += n;
  }
  while (!q.push(new Uint8Array([255]))) await new Promise((res) => setImmediate(res));
  const got = await done;
  await w.terminate();
  assert.deepEqual(got, { n: N, bytes, bad: 0, sum });
  assert.ok(full > 0, "the producer met a full queue");
});
