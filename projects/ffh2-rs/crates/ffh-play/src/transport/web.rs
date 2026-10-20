//! The web host (docs/WEB.md): what `ffh-play serve`'s game thread does, synchronously, for a
//! page that runs the engine itself (`crates/ffh-web`, compiled to WebAssembly and run in a Web
//! Worker). There are no threads and no sockets: the page hands over one HTTP request at a time
//! ([`WebGame::request`]) and the host answers through a [`WebIo`]:
//!
//! * `reply(token, response)`: the answer to request `token` (every request gets exactly one,
//!   possibly before the request that pulled it is answered);
//! * `publish(status)`: the status, as `GET /api/status` returns it, each time it changes;
//! * `pull(block)`: the next request the page queued while one runs: between turn slices
//!   (`block = false`) and inside a blocking decision (`block = true`, which waits for one).
//!   [`Pulled::Unsupported`]: the page cannot deliver requests while the engine runs (no
//!   cross-origin isolation): blocking decisions take the advisor's pick and nothing is served
//!   between slices.
//!
//! The answers are [`http::route`]'s for `/api`: GETs from [`Shared`] ([`http::api_get`];
//! `/api/wait` answers the current status at once: the page holds long-polls itself), `POST /api`
//! as the game thread handles an envelope ([`Host::handle`], refresh the caches, publish, reply,
//! then `advance` when `After::Advance`, publishing at most every [`WEB_PUBLISH_EVERY`] and serving
//! pulled requests between slices with [`running_reply`]); inside a blocking decision
//! [`WebAnswerer`] serves them as `ChannelAnswerer` does ([`decision_reply`]). Other paths get 404:
//! the site serves the client's files.
//!
//! Panics are not caught here: on wasm a panic aborts (`crates/ffh-web` reports it from a panic
//! hook and the page restarts the engine).
//!
//! Framing (the page's side of `crates/ffh-web`): a request is one line of JSON
//! (`{"t": token, "m", "p", "q", "h": {name: value}}`) then `\n` and the body bytes
//! ([`decode_request`]); a reply is `{"s": status, "ct": content type, "h": [[name, value]..]}`,
//! `\n` and the body ([`encode_reply`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::http::{self, HttpRequest, Response};
use super::server::{decision_closed, publish_decision, publish_running, refresh_shared};
use super::shared::Shared;
use super::{decision_reply, err_reply, running_reply, After, Host, HostOptions, Request};
use crate::decision::Decision;
use crate::human::{Answer, Answerer, DecisionReader};
use crate::json::Json;
use crate::session::SessionOptions;

/// Status updates while the game runs, at most this often (each one crosses to the page).
pub const WEB_PUBLISH_EVERY: Duration = Duration::from_millis(100);

/// What `pull` returned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pulled {
    /// A request and its token.
    Request(u32, HttpRequest),
    /// None queued (only when not blocking).
    Empty,
    /// The page cannot deliver requests while the engine runs.
    Unsupported,
}

/// The page's side: where replies and statuses go, where queued requests come from.
pub trait WebIo: Send + Sync {
    /// The reply to request `token`.
    fn reply(&self, token: u32, resp: &Response);
    /// The status (with its `version`) after it changed.
    fn publish(&self, status: &Json);
    /// The next queued request; `block`: wait until one comes (inside a blocking decision).
    fn pull(&self, block: bool) -> Pulled;
}

/// The web host's settings (`ffh_init`'s options).
#[derive(Clone, Debug)]
pub struct WebOptions {
    pub host: HostOptions,
    /// Blocking decisions wait for the player's answer (requests pulled with `block = true`).
    /// `false`: the advisor answers them and nothing is pulled between turn slices either.
    pub blocking: bool,
}

impl Default for WebOptions {
    fn default() -> WebOptions {
        WebOptions { host: HostOptions { saves: Some(PathBuf::from("/saves")), autosave_keep: SessionOptions::default().autosave_keep, panic_at: None }, blocking: true }
    }
}

impl WebOptions {
    /// `{"saves": "/saves" | null, "autosave_keep": N, "blocking": true|false}`; every member is
    /// optional (defaults: `/saves`, the session's autosave ring, blocking), others are refused.
    pub fn from_json(j: &Json) -> Result<WebOptions, String> {
        if j.as_obj().is_none() {
            return Err(format!("the options are an object, got {}", j.kind()));
        }
        j.only(&["saves", "autosave_keep", "blocking"]).map_err(|e| e.to_string())?;
        let mut o = WebOptions::default();
        match j.get("saves") {
            None => {}
            Some(Json::Null) => o.host.saves = None,
            Some(Json::Str(s)) if !s.is_empty() => o.host.saves = Some(PathBuf::from(s)),
            Some(v) => return Err(format!("field `saves`: expected a directory or null, got {}", v.kind())),
        }
        if let Some(n) = j.opt::<usize>("autosave_keep").map_err(|e| e.to_string())? {
            o.host.autosave_keep = n;
        }
        o.blocking = j.get_or("blocking", true).map_err(|e| e.to_string())?;
        Ok(o)
    }
}

/// A frame that is not a request: why, and its token when it could be read (the page is then
/// answered 400).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameError {
    pub token: Option<u32>,
    pub msg: String,
}

impl FrameError {
    /// The reply to a malformed request.
    pub fn response(&self) -> Response {
        Response::error(400, "http", self.msg.clone())
    }
}

/// Decodes a request frame: `{"t": token, "m": "GET"|"POST", "p": "/api/view", "q": "since=3",
/// "h": {"if-none-match": "\"abc\""}}`, `\n`, the body (`q` and `h` optional).
pub fn decode_request(frame: &[u8]) -> Result<(u32, HttpRequest), FrameError> {
    let fail = |token, msg: String| FrameError { token, msg };
    let nl = frame.iter().position(|&b| b == b'\n').ok_or_else(|| fail(None, "a request frame is a JSON line, `\\n`, then the body".into()))?;
    let head = Json::parse_bytes(&frame[..nl]).map_err(|e| fail(None, format!("the request head is not JSON: {e}")))?;
    let token: u32 = head.req("t").map_err(|e| fail(None, e.to_string()))?;
    let bad = |msg: String| fail(Some(token), msg);
    head.only(&["t", "m", "p", "q", "h"]).map_err(|e| bad(e.to_string()))?;
    let method = head.req_str("m").map_err(|e| bad(e.to_string()))?;
    if method.is_empty() || !method.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(bad(format!("bad method {method:?}")));
    }
    let path = head.req_str("p").map_err(|e| bad(e.to_string()))?;
    if !path.starts_with('/') {
        return Err(bad("the path must start with /".into()));
    }
    let query = head.opt_str("q").map_err(|e| bad(e.to_string()))?.unwrap_or_default();
    let mut headers = Vec::new();
    match head.get("h") {
        None | Some(Json::Null) => {}
        Some(Json::Obj(members)) => {
            for (k, v) in members {
                let v = v.as_str().ok_or_else(|| bad(format!("header {k:?}: expected a string, got {}", v.kind())))?;
                headers.push((k.clone(), v.to_string()));
            }
        }
        Some(v) => return Err(bad(format!("field `h`: expected an object, got {}", v.kind()))),
    }
    let req = HttpRequest { method: method.to_string(), path: path.to_string(), query: query.to_string(), headers, body: frame[nl + 1..].to_vec() };
    Ok((token, req))
}

/// Encodes a reply: `{"s": status, "ct": type, "h": [[name, value]..]}`, `\n`, the body (none
/// for 304). As the native server, `Cache-Control: no-store` unless the response says otherwise.
pub fn encode_reply(resp: &Response) -> Vec<u8> {
    let mut h: Vec<Json> = resp.headers.iter().map(|(k, v)| crate::jarr![*k, v.as_str()]).collect();
    if !resp.headers.iter().any(|(k, _)| *k == "Cache-Control") {
        h.push(crate::jarr!["Cache-Control", "no-store"]);
    }
    let head = crate::jobj! { "s" => resp.status, "ct" => resp.content_type, "h" => Json::Arr(h) };
    let mut out = head.to_string().into_bytes();
    out.push(b'\n');
    if resp.status != 304 {
        out.extend_from_slice(resp.body.bytes());
    }
    out
}

/// What the host does with a request.
enum Routed {
    /// Answered (from the caches, or refused).
    Done(Response),
    /// `POST /api` with this envelope: the game's.
    Post(Json),
}

/// [`http::route`] for the web host: the API only (the site serves the files), `/api/wait` at
/// once.
fn route(req: &HttpRequest, shared: &Shared) -> Routed {
    let path = req.path.as_str();
    let not_found = || Response::error(404, "not_found", format!("no {path}"));
    if path != "/api" && !path.starts_with("/api/") {
        return Routed::Done(not_found());
    }
    if let Err(refused) = http::check_method(req) {
        return Routed::Done(refused);
    }
    match path {
        "/api" => match http::post_envelope(req) {
            Ok(j) => Routed::Post(j),
            Err(refused) => Routed::Done(refused),
        },
        // The page answers long-polls itself (it holds the published status).
        "/api/wait" => Routed::Done(Response::json(200, &shared.status())),
        _ => Routed::Done(http::api_get(req, shared).unwrap_or_else(not_found)),
    }
}

/// The shared status and the page.
struct Out {
    shared: Arc<Shared>,
    io: Arc<dyn WebIo>,
    blocking: bool,
}

impl Out {
    /// After a publish to `shared`: the status goes to the page.
    fn published(&self) {
        self.io.publish(&self.shared.status());
    }

    fn pull(&self, block: bool) -> Pulled {
        if self.blocking {
            self.io.pull(block)
        } else {
            Pulled::Unsupported
        }
    }
}

/// Answers blocking decisions from the page ([`super::server::ChannelAnswerer`]'s logic): the
/// decision's status and caches are published, then requests are pulled (blocking) until an
/// `answer` closes it; GETs are answered from the caches, `POST /api` by [`decision_reply`].
/// When the page cannot deliver requests ([`Pulled::Unsupported`], or not `blocking`), the
/// advisor answers.
pub struct WebAnswerer {
    out: Arc<Out>,
}

impl Answerer for WebAnswerer {
    fn ask(&self, _g: &ffh_core::game::Game, _d: &Decision) -> Answer {
        Answer::Advisor
    }

    fn ask_with(&self, r: &DecisionReader, d: &Decision) -> Answer {
        let out = &self.out;
        if !out.blocking {
            return Answer::Advisor;
        }
        let status = publish_decision(r, d, &out.shared);
        out.published();
        loop {
            let (token, req) = match out.pull(true) {
                Pulled::Request(token, req) => (token, req),
                // Not with `block`; asked again (the page may wake the engine without one).
                Pulled::Empty => continue,
                Pulled::Unsupported => {
                    out.shared.publish(decision_closed(status));
                    out.published();
                    return Answer::Advisor;
                }
            };
            let (resp, answer) = match route(&req, &out.shared) {
                Routed::Done(resp) => (resp, None),
                Routed::Post(j) => {
                    let (id, req) = Request::from_json(&j);
                    let (reply, answer) = decision_reply(r, d, id, req);
                    (Response::json(200, &reply), answer)
                }
            };
            out.io.reply(token, &resp);
            if let Some(a) = answer {
                // The rules call goes on; the host publishes again when it is done.
                out.shared.publish(decision_closed(status));
                out.published();
                return a;
            }
        }
    }
}

/// The web host: the transport state ([`Host`]), the published status and caches ([`Shared`]),
/// and the page ([`WebIo`]).
pub struct WebGame {
    host: Host,
    out: Arc<Out>,
    /// A `quit` was answered: the game no longer takes envelopes (as the native server stops).
    quit: bool,
}

impl WebGame {
    /// The host (no game: the page starts one); publishes the first status.
    pub fn start(opts: WebOptions, io: Arc<dyn WebIo>) -> WebGame {
        let out = Arc::new(Out { shared: Arc::new(Shared::new()), io, blocking: opts.blocking });
        let answerer = Arc::new(WebAnswerer { out: out.clone() });
        let mut g = WebGame { host: Host::new(opts.host, answerer), out, quit: false };
        g.refresh(After::Nothing);
        g
    }

    pub fn host(&self) -> &Host {
        &self.host
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.out.shared
    }

    /// One request frame ([`decode_request`]); a malformed one is answered 400 when its token can
    /// be read (else only reported on stderr: there is no one to answer).
    pub fn request_frame(&mut self, frame: &[u8]) {
        match decode_request(frame) {
            Ok((token, req)) => self.request(token, &req),
            Err(e) => match e.token {
                Some(token) => self.out.io.reply(token, &e.response()),
                None => eprintln!("ffh-web: a request without a token: {}", e.msg),
            },
        }
    }

    /// One request: answered through the page's `reply` (with requests pulled meanwhile, when the
    /// game runs or a decision is open).
    pub fn request(&mut self, token: u32, req: &HttpRequest) {
        let j = match route(req, &self.out.shared) {
            Routed::Done(resp) => return self.out.io.reply(token, &resp),
            Routed::Post(_) if self.quit => {
                return self.out.io.reply(token, &Response::error(503, "stopped", "the game has quit (reload the page)"));
            }
            Routed::Post(j) => j,
        };
        // As the game thread handles an envelope (`server::GameThread::handle`).
        let (id, req) = Request::from_json(&j);
        let query = matches!(req, Ok(Request::Query(_)));
        let (reply, after) = match req {
            Ok(r) => self.host.handle(id, r),
            Err(e) => (err_reply(id, &e), After::Nothing),
        };
        if !query {
            self.refresh(after);
        }
        self.out.io.reply(token, &Response::json(200, &reply));
        match after {
            After::Nothing => {}
            After::Advance => self.advance(),
            After::Quit => {
                self.quit = true;
                self.out.shared.quit();
            }
        }
    }

    /// Caches the view and map and publishes the status (`server::refresh_shared`).
    fn refresh(&mut self, after: After) {
        refresh_shared(&self.host, &self.out.shared, after);
        self.out.published();
    }

    /// Runs the game to the human's next control point, publishing at most every
    /// [`WEB_PUBLISH_EVERY`] and serving the requests the page queued between turn slices.
    fn advance(&mut self) {
        let out = self.out.clone();
        let dir = self.host.game_dir().map(Path::to_path_buf);
        let mut last: Option<Instant> = None;
        let mut pulling = true;
        self.host.advance(&mut |s| {
            if last.is_none_or(|t| t.elapsed() >= WEB_PUBLISH_EVERY) {
                last = Some(Instant::now());
                if publish_running(s, dir.as_deref(), &out.shared) {
                    out.published();
                }
            }
            while pulling {
                match out.pull(false) {
                    Pulled::Request(token, req) => {
                        let resp = match route(&req, &out.shared) {
                            Routed::Done(resp) => resp,
                            Routed::Post(j) => {
                                let (id, req) = Request::from_json(&j);
                                Response::json(200, &running_reply(s, dir.as_deref(), id, req))
                            }
                        };
                        out.io.reply(token, &resp);
                    }
                    Pulled::Empty => break,
                    Pulled::Unsupported => pulling = false,
                }
            }
        });
        self.refresh(After::Nothing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_decode_and_encode() {
        let (t, r) = decode_request(b"{\"t\":7,\"m\":\"GET\",\"p\":\"/api/map\",\"q\":\"since=3&epoch=e-1\",\"h\":{\"if-none-match\":\"\\\"x\\\"\"}}\n").unwrap();
        assert_eq!(t, 7);
        assert_eq!((r.method.as_str(), r.path.as_str(), r.param("since"), r.param("epoch")), ("GET", "/api/map", Some("3"), Some("e-1")));
        assert_eq!(r.header("If-None-Match"), Some("\"x\""));
        assert!(r.body.is_empty());
        let (t, r) = decode_request(b"{\"t\":8,\"m\":\"POST\",\"p\":\"/api\",\"h\":{\"x-ffh\":\"1\"}}\n{\"k\":\"q\",\n\"q\":\"status\"}").unwrap();
        assert_eq!((t, r.query.as_str(), r.body.as_slice()), (8, "", b"{\"k\":\"q\",\n\"q\":\"status\"}".as_slice()));
        for (bad, token) in [
            (&b"{\"t\":1,\"m\":\"GET\",\"p\":\"/api\"}"[..], None),
            (b"[1]\n", None),
            (b"{\"m\":\"GET\",\"p\":\"/api\"}\n", None),
            (b"{\"t\":-1,\"m\":\"GET\",\"p\":\"/api\"}\n", None),
            (b"{\"t\":2,\"m\":\"get\",\"p\":\"/api\"}\n", Some(2)),
            (b"{\"t\":3,\"m\":\"GET\",\"p\":\"api\"}\n", Some(3)),
            (b"{\"t\":4,\"m\":\"GET\",\"p\":\"/api\",\"h\":{\"a\":1}}\n", Some(4)),
            (b"{\"t\":5,\"m\":\"GET\",\"p\":\"/api\",\"x\":1}\n", Some(5)),
        ] {
            let e = decode_request(bad).unwrap_err();
            assert_eq!(e.token, token, "{}: {}", String::from_utf8_lossy(bad), e.msg);
            assert_eq!(e.response().status, 400);
        }
        let resp = Response::json(200, &crate::jobj! { "a" => 1 });
        assert_eq!(encode_reply(&resp), b"{\"s\":200,\"ct\":\"application/json; charset=utf-8\",\"h\":[[\"Cache-Control\",\"no-store\"]]}\n{\"a\":1}");
        let mut not_modified = Response::json(304, &Json::Null);
        not_modified.headers = vec![("ETag", "\"e\"".into()), ("Cache-Control", "no-cache".into())];
        assert_eq!(encode_reply(&not_modified), b"{\"s\":304,\"ct\":\"application/json; charset=utf-8\",\"h\":[[\"ETag\",\"\\\"e\\\"\"],[\"Cache-Control\",\"no-cache\"]]}\n");
    }

    #[test]
    fn options_parse() {
        let o = WebOptions::from_json(&Json::parse(r#"{"saves":"/s","autosave_keep":3,"blocking":false}"#).unwrap()).unwrap();
        assert_eq!((o.host.saves.as_deref(), o.host.autosave_keep, o.blocking), (Some(Path::new("/s")), 3, false));
        let o = WebOptions::from_json(&Json::parse(r#"{"saves":null}"#).unwrap()).unwrap();
        assert_eq!((o.host.saves, o.blocking), (None, true));
        let o = WebOptions::from_json(&Json::obj()).unwrap();
        assert_eq!(o.host.saves.as_deref(), Some(Path::new("/saves")));
        for bad in [r#"[]"#, r#"{"saves":3}"#, r#"{"saves":""}"#, r#"{"autosave_keep":-1}"#, r#"{"blocking":1}"#, r#"{"save":"/s"}"#] {
            assert!(WebOptions::from_json(&Json::parse(bad).unwrap()).is_err(), "{bad}");
        }
    }
}
