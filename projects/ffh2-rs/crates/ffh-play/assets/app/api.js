// The protocol over HTTP (design §4.1-4.2): `POST /api` envelopes and the cached GETs.
// No DOM here: the node smoke client (tools/web-api-smoke.mjs) and the tests import this module.

let base = "";
let fetchFn = (...a) => globalThis.fetch(...a);
let nextId = 1;

/** Point the module at a server (`http://127.0.0.1:8642`; "" = this page's origin) and,
 *  for tests, at another `fetch`. */
export function configure({ url, fetch } = {}) {
  if (url !== undefined) base = url.replace(/\/$/, "");
  if (fetch) fetchFn = fetch;
}

/** A refused request: the reply's `error` code (`invalid`, `not_your_turn`, `cannot`, ...) and
 *  its `detail`; `reply` is the whole reply. */
export class ApiError extends Error {
  constructor(reply) {
    super(`${reply.error}: ${reply.detail ?? ""}`);
    this.name = "ApiError";
    this.code = reply.error;
    this.detail = reply.detail ?? "";
    this.reply = reply;
  }
}

/** A transport failure (the server is gone, an HTTP error without a protocol reply). */
export class NetError extends Error {
  constructor(msg, status = 0) {
    super(msg);
    this.name = "NetError";
    this.status = status;
  }
}

async function readJson(r, what) {
  const text = await r.text();
  try {
    return JSON.parse(text);
  } catch {
    throw new NetError(`${what}: HTTP ${r.status}, not JSON: ${text.slice(0, 120)}`, r.status);
  }
}

/** Sends one envelope and returns the whole reply (`{id, ok, r | error, detail, log?,
 *  status?}`); never throws for a protocol refusal. */
export async function send(req) {
  const body = { ...req };
  if (body.id === undefined) body.id = nextId++;
  let r;
  try {
    r = await fetchFn(`${base}/api`, {
      method: "POST",
      headers: { "X-FFH": "1", "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
  } catch (e) {
    throw new NetError(`POST /api: ${e.message ?? e}`);
  }
  const j = await readJson(r, "POST /api");
  if (r.status !== 200 && j.ok === undefined) throw new NetError(`POST /api: HTTP ${r.status}: ${j.detail ?? ""}`, r.status);
  return j;
}

/** Like `send`, but a refusal throws an `ApiError`; returns the reply. */
export async function call(req) {
  const j = await send(req);
  if (!j.ok) throw new ApiError(j);
  return j;
}

/** A query's result (`r`). */
export async function query(q, fields = {}) {
  return (await call({ k: "q", q, ...fields })).r;
}

/** A command; returns the whole reply (`r.applied` or `r.confirm`, `log`, `status`). */
export function cmd(c, fields = {}) {
  return call({ k: "cmd", c, ...fields });
}

/** A session operation (`new`, `load`, `save`, `saves`, `debug`, `take_over`, `quit`). */
export function session(op, fields = {}) {
  return call({ k: "session", op, ...fields });
}

/** Answers blocking decision `decision` with option `option`, or with the advisor's pick
 *  when `option` is null. */
export function answer(decision, option) {
  return call(option === null || option === undefined ? { k: "answer", decision, advisor: true } : { k: "answer", decision, option });
}

/** A GET of the server's cached JSON (`/api/status`, `/api/view`, ...); `null` for 409 (no
 *  game). With `etag` (`/api/data`), `{etag, body}` and `body: null` on 304. */
export async function get(path, { etag, signal } = {}) {
  let r;
  try {
    r = await fetchFn(`${base}${path}`, { headers: etag ? { "If-None-Match": etag } : {}, signal });
  } catch (e) {
    if (e?.name === "AbortError") throw e;
    throw new NetError(`GET ${path}: ${e.message ?? e}`);
  }
  if (r.status === 409) return null;
  if (etag !== undefined) {
    const tag = r.headers.get("ETag");
    if (r.status === 304) return { etag: tag ?? etag, body: null };
    if (r.status !== 200) throw new NetError(`GET ${path}: HTTP ${r.status}`, r.status);
    return { etag: tag, body: await readJson(r, `GET ${path}`) };
  }
  if (r.status !== 200) throw new NetError(`GET ${path}: HTTP ${r.status}`, r.status);
  return readJson(r, `GET ${path}`);
}

/** The status once its version is above `since` (the long-poll), or after `timeoutMs`. */
export function wait(since, timeoutMs = 25000, signal) {
  return get(`/api/wait?since=${since}&timeout_ms=${timeoutMs}`, { signal });
}

/** Watches the status: calls `onStatus(status)` for every new version and `onError(e)` while
 *  the server cannot be reached (then retries). Returns a function that stops it. */
export function watch(onStatus, onError = () => {}, { timeoutMs = 25000, retryMs = 1000 } = {}) {
  let stopped = false;
  const ctl = typeof AbortController === "function" ? new AbortController() : null;
  (async () => {
    let version = -1;
    while (!stopped) {
      try {
        const st = version < 0 ? await get("/api/status") : await wait(version, timeoutMs, ctl?.signal);
        if (stopped) break;
        if (st && st.version !== version) {
          version = st.version;
          onStatus(st);
        }
      } catch (e) {
        if (stopped) break;
        onError(e);
        await new Promise((res) => setTimeout(res, retryMs));
        version = -1;
      }
    }
  })();
  return () => {
    stopped = true;
    ctl?.abort();
  };
}

/** The current status at once. */
export function status() {
  return get("/api/status");
}
