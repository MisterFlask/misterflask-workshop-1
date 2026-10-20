//! The web host (`transport::web`, docs/WEB.md): what the WebAssembly engine of the static site
//! does, run natively with a fake page. Requests go in framed as the page frames them, replies
//! and statuses come back through the page's callbacks: a new game started with `POST /api`, the
//! cached GETs (view, map deltas, data with its ETag), turns ended with requests pulled between
//! turn slices, a blocking decision (the keep/raze decision of a staged capture) answered through
//! requests pulled from inside the rules call, and a page that cannot deliver requests while the
//! engine runs (the advisor answers). Every request gets exactly one reply.

mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use common::{staged_capture, TempDir};
use ffh_play::json::Json;
use ffh_play::transport::http::{HttpRequest, Response};
use ffh_play::transport::web::{encode_reply, Pulled, WebGame, WebIo, WebOptions, WEB_PUBLISH_EVERY};
use ffh_play::transport::HostOptions;

/// A reply as the page reads it from its frame.
#[derive(Clone, Debug)]
struct Reply {
    token: u32,
    status: u16,
    content_type: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Json {
        Json::parse_bytes(&self.body).unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&self.body)))
    }

    fn header(&self, k: &str) -> Option<&str> {
        self.headers.iter().find(|(h, _)| h.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
    }

    /// The `r` of an ok envelope reply.
    fn ok(&self) -> Json {
        let j = self.json();
        assert_eq!((self.status, j.req::<bool>("ok")), (200, Ok(true)), "{j}");
        j.get("r").cloned().unwrap()
    }

    fn error(&self) -> String {
        self.json().req_str("error").unwrap_or("ok").to_string()
    }
}

/// Decodes a reply frame (`{"s", "ct", "h"}`, `\n`, the body).
fn decode_reply(token: u32, frame: &[u8]) -> Reply {
    let nl = frame.iter().position(|&b| b == b'\n').expect("a reply head");
    let head = Json::parse_bytes(&frame[..nl]).unwrap();
    head.only(&["s", "ct", "h"]).unwrap();
    let headers = head
        .req_arr("h")
        .unwrap()
        .iter()
        .map(|kv| match kv.as_arr() {
            Some([k, v]) => (k.as_str().unwrap().to_string(), v.as_str().unwrap().to_string()),
            _ => panic!("a header is [name, value]: {kv}"),
        })
        .collect();
    Reply { token, status: head.req("s").unwrap(), content_type: head.req_str("ct").unwrap().to_string(), headers, body: frame[nl + 1..].to_vec() }
}

/// What the page queues when the engine pulls and nothing is queued: `(block, the published
/// statuses)` -> requests.
type Script = Box<dyn FnMut(bool, &[Json]) -> Vec<HttpRequest> + Send>;

#[derive(Default)]
struct PageState {
    queue: VecDeque<(u32, HttpRequest)>,
    replies: Vec<Reply>,
    published: Vec<(Instant, Json)>,
    /// `block` of every pull.
    pulls: Vec<bool>,
    /// Every token the page sent (directly or queued).
    sent: Vec<u32>,
    next: u32,
}

/// The fake page.
struct Page {
    st: Mutex<PageState>,
    /// `pull` answers `Unsupported` (a page without cross-origin isolation).
    unsupported: bool,
    script: Mutex<Option<Script>>,
}

impl Page {
    fn new(unsupported: bool) -> Arc<Page> {
        Arc::new(Page { st: Mutex::new(PageState { next: 1, ..Default::default() }), unsupported, script: Mutex::new(None) })
    }

    fn script(&self, s: impl FnMut(bool, &[Json]) -> Vec<HttpRequest> + Send + 'static) {
        *self.script.lock().unwrap() = Some(Box::new(s));
    }

    fn token(&self) -> u32 {
        let mut st = self.st.lock().unwrap();
        let t = st.next;
        st.next += 1;
        st.sent.push(t);
        t
    }

    /// Sends `req` framed and returns its reply (which must have come by the time the call
    /// returns).
    fn send(&self, game: &mut WebGame, req: &HttpRequest) -> Reply {
        let token = self.token();
        game.request_frame(&frame(token, req));
        self.reply_of(token)
    }

    fn reply_of(&self, token: u32) -> Reply {
        let st = self.st.lock().unwrap();
        let mine: Vec<&Reply> = st.replies.iter().filter(|r| r.token == token).collect();
        assert_eq!(mine.len(), 1, "request {token}: {} replies", mine.len());
        mine[0].clone()
    }

    fn last_status(&self) -> Json {
        self.st.lock().unwrap().published.last().expect("a status was published").1.clone()
    }

    fn published(&self) -> usize {
        self.st.lock().unwrap().published.len()
    }

    /// Every request got exactly one reply, and no reply went to an unknown token.
    fn check_one_reply_each(&self) {
        let st = self.st.lock().unwrap();
        for t in &st.sent {
            let n = st.replies.iter().filter(|r| r.token == *t).count();
            assert_eq!(n, 1, "request {t}: {n} replies");
        }
        for r in &st.replies {
            assert!(st.sent.contains(&r.token), "a reply to {}, which was not sent", r.token);
        }
        assert!(st.queue.is_empty(), "requests left in the queue");
        // Versions only grow, and each published status is what `GET /api/status` says then.
        for w in st.published.windows(2) {
            assert!(w[1].1.req::<i64>("version").unwrap() > w[0].1.req::<i64>("version").unwrap());
        }
    }
}

impl WebIo for Page {
    fn reply(&self, token: u32, resp: &Response) {
        let reply = decode_reply(token, &encode_reply(resp));
        self.st.lock().unwrap().replies.push(reply);
    }

    fn publish(&self, status: &Json) {
        self.st.lock().unwrap().published.push((Instant::now(), status.clone()));
    }

    fn pull(&self, block: bool) -> Pulled {
        self.st.lock().unwrap().pulls.push(block);
        if self.unsupported {
            return Pulled::Unsupported;
        }
        if self.st.lock().unwrap().queue.is_empty() {
            let published: Vec<Json> = self.st.lock().unwrap().published.iter().map(|(_, j)| j.clone()).collect();
            let more = self.script.lock().unwrap().as_mut().map(|s| s(block, &published)).unwrap_or_default();
            for req in more {
                let t = self.token();
                self.st.lock().unwrap().queue.push_back((t, req));
            }
        }
        match self.st.lock().unwrap().queue.pop_front() {
            Some((t, req)) => Pulled::Request(t, req),
            None if block => panic!("a blocking pull with nothing to answer (the page would wait forever)"),
            None => Pulled::Empty,
        }
    }
}

/// The page's request frame.
fn frame(token: u32, req: &HttpRequest) -> Vec<u8> {
    let headers = Json::Obj(req.headers.iter().map(|(k, v)| (k.to_ascii_lowercase(), Json::from(v.as_str()))).collect());
    let head = ffh_play::jobj! { "t" => token, "m" => req.method.as_str(), "p" => req.path.as_str(), "q" => req.query.as_str(), "h" => headers };
    let mut f = head.to_string().into_bytes();
    f.push(b'\n');
    f.extend_from_slice(&req.body);
    f
}

fn get_with(target: &str, headers: &[(&str, &str)]) -> HttpRequest {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    HttpRequest { method: "GET".into(), path: path.into(), query: query.into(), headers: headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), body: Vec::new() }
}

fn get(target: &str) -> HttpRequest {
    get_with(target, &[])
}

fn post(j: &Json) -> HttpRequest {
    HttpRequest { method: "POST".into(), path: "/api".into(), query: String::new(), headers: vec![("X-FFH".into(), "1".into())], body: j.to_string().into_bytes() }
}

fn cmd(c: &str) -> Json {
    ffh_play::jobj! { "k" => "cmd", "c" => c }
}

fn phase(st: &Json) -> &str {
    st.req_str("phase").unwrap()
}

fn start(root: &std::path::Path, page: &Arc<Page>, blocking: bool) -> WebGame {
    let opts = WebOptions { host: HostOptions { saves: Some(root.to_path_buf()), autosave_keep: 3, panic_at: None }, blocking };
    WebGame::start(opts, page.clone())
}

/// A page whose blocking pulls take the advisor's pick (an AI offer may open a decision while the
/// AI plays).
fn advisor_answers(page: &Page) {
    page.script(|block, published| {
        if !block {
            return Vec::new();
        }
        let st = published.last().unwrap();
        let id: u32 = st.get("decision").unwrap().req("id").unwrap();
        vec![post(&ffh_play::jobj! { "k" => "answer", "decision" => id, "advisor" => true })]
    });
}

#[test]
fn web_host_new_game_cached_gets_and_turns() {
    let tmp = TempDir::new("web-host-game");
    let page = Page::new(false);
    let mut game = start(&tmp.0.join("saves"), &page, true);
    // The first status: no game.
    let st = page.last_status();
    assert_eq!((phase(&st), st.req::<i64>("version")), ("no_game", Ok(1)));
    let r = page.send(&mut game, &get("/api/status"));
    assert_eq!((r.status, r.content_type.as_str()), (200, "application/json; charset=utf-8"));
    assert_eq!(r.header("Cache-Control"), Some("no-store"));
    assert_eq!(phase(&r.json()), "no_game");
    assert_eq!(page.send(&mut game, &get("/api/view")).status, 409);
    assert_eq!(page.send(&mut game, &get("/api/map")).status, 409);
    // The tables, with their ETag.
    let data = page.send(&mut game, &get("/api/data"));
    assert_eq!(data.status, 200);
    assert!(data.json().get("kinds").is_some());
    let etag = data.header("ETag").unwrap().to_string();
    assert_eq!(data.header("Cache-Control"), Some("no-cache"));
    let again = page.send(&mut game, &get_with("/api/data", &[("If-None-Match", &etag)]));
    assert_eq!((again.status, again.body.len(), again.header("ETag")), (304, 0, Some(etag.as_str())));
    assert_eq!(page.send(&mut game, &get("/api/setup")).status, 200);
    // What `http::route` refuses; the files are the site's.
    for (req, status) in [
        (get("/api"), 405),
        (post(&Json::obj()).with_path("/api/view"), 405),
        (get("/api/nothing"), 404),
        (get("/index.html"), 404),
        (get("/app/main.js"), 404),
        (get("/"), 404),
    ] {
        let r = page.send(&mut game, &req);
        assert_eq!(r.status, status, "{} {}: {}", req.method, req.path, String::from_utf8_lossy(&r.body));
    }
    assert_eq!(page.send(&mut game, &get("/api")).header("Allow"), Some("POST"));
    let mut no_header = post(&ffh_play::jobj! { "k" => "q", "q" => "status" });
    no_header.headers.clear();
    assert_eq!(page.send(&mut game, &no_header).status, 403);
    let mut not_utf8 = post(&Json::obj());
    not_utf8.body = vec![0xff, 0xfe];
    assert_eq!(page.send(&mut game, &not_utf8).status, 400);
    let mut not_json = post(&Json::obj());
    not_json.body = b"{\"k\":".to_vec();
    assert_eq!(page.send(&mut game, &not_json).error(), "invalid");
    // Malformed frames: answered 400 when the token can be read.
    let t = page.token();
    game.request_frame(format!("{{\"t\":{t},\"m\":\"GET\"}}\n").as_bytes());
    assert_eq!(page.reply_of(t).status, 400);
    let before = page.st.lock().unwrap().replies.len();
    game.request_frame(b"no frame at all");
    assert_eq!(page.st.lock().unwrap().replies.len(), before, "no token, no reply");
    // Queries change nothing: no publish.
    let n = page.published();
    assert_eq!(phase(&page.send(&mut game, &post(&ffh_play::jobj! { "k" => "q", "q" => "status" })).ok()), "no_game");
    assert_eq!(page.published(), n);

    // A new game: the reply says `running`, the game runs to the human's first control point
    // before `request` returns.
    advisor_answers(&page);
    let setup = ffh_play::jobj! { "seed" => 3, "world" => "WORLDSIZE_DUEL", "opponents" => 1 };
    let r = page.send(&mut game, &post(&ffh_play::jobj! { "id" => 1, "k" => "session", "op" => "new", "setup" => setup, "name" => "g" }));
    assert_eq!(r.json().req::<i64>("id"), Ok(1));
    assert_eq!(phase(r.json().get("status").unwrap()), "running");
    let st = page.last_status();
    assert_eq!(phase(&st), "human", "{st}");
    let turn: i32 = st.req("turn").unwrap();
    let got = page.send(&mut game, &get("/api/status")).json();
    assert_eq!((got.req::<i64>("version"), phase(&got)), (st.req::<i64>("version"), "human"));
    // The cached view and map, the map deltas, and `/api/wait` at once.
    let view = page.send(&mut game, &get("/api/view")).json();
    assert_eq!(view.req::<i64>("view_version"), st.req::<i64>("view_version"));
    assert!(view.get("me").is_some());
    let map = page.send(&mut game, &get("/api/map")).json();
    let (epoch, rev) = (map.req_str("epoch").unwrap().to_string(), map.req::<i64>("rev").unwrap());
    let delta = page.send(&mut game, &get(&format!("/api/map?since={rev}&epoch={epoch}"))).json();
    assert_eq!(delta.get("full").and_then(Json::as_bool), Some(false), "{delta}");
    let other = page.send(&mut game, &get(&format!("/api/map?since={rev}&epoch=elsewhere"))).json();
    assert_eq!(other.get("full").and_then(Json::as_bool), Some(true));
    let waited = page.send(&mut game, &get("/api/wait?since=0&timeout_ms=60000")).json();
    assert_eq!(waited.req::<i64>("version"), st.req::<i64>("version"));
    assert_eq!(st.get("game").unwrap().req_str("dir"), Ok(tmp.0.join("saves/g").to_str().unwrap()));

    // End the turn; while the AI plays, the page's queued requests are answered between turn
    // slices: GETs from the caches, queries, commands `not_your_turn`, session operations `busy`.
    let mut queued = false;
    page.script(move |block, published| {
        if block {
            let id: u32 = published.last().unwrap().get("decision").unwrap().req("id").unwrap();
            return vec![post(&ffh_play::jobj! { "k" => "answer", "decision" => id, "advisor" => true })];
        }
        if std::mem::replace(&mut queued, true) {
            return Vec::new();
        }
        vec![
            get("/api/status"),
            post(&ffh_play::jobj! { "id" => 21, "k" => "q", "q" => "status" }),
            post(&cmd("end_turn").with("id", 22)),
            post(&ffh_play::jobj! { "id" => 23, "k" => "session", "op" => "save", "name" => "x" }),
            get("/api/view"),
        ]
    });
    // The end turn's token, then the queued requests'.
    let first_pulled = page.st.lock().unwrap().next + 1;
    let n0 = page.published();
    let t0 = Instant::now();
    let r = page.send(&mut game, &post(&cmd("end_turn").with("force", true)));
    let elapsed = t0.elapsed();
    assert_eq!(r.ok().req::<bool>("applied"), Ok(true));
    let pulled: Vec<Reply> = (first_pulled..first_pulled + 5).map(|t| page.reply_of(t)).collect();
    assert_eq!(phase(&pulled[0].json()), "running");
    assert_eq!(phase(&pulled[1].ok()), "running");
    assert_eq!((pulled[2].error(), pulled[3].error()), ("not_your_turn".to_string(), "busy".to_string()));
    assert_eq!(pulled[4].status, 200);
    // The end turn's reply came before the pulled ones (it is sent before the game runs).
    {
        let st = page.st.lock().unwrap();
        let at = |t: u32| st.replies.iter().position(|r| r.token == t).unwrap();
        assert!(at(r.token) < at(first_pulled));
    }
    let st = page.last_status();
    assert_eq!((phase(&st), st.req::<i32>("turn")), ("human", Ok(turn + 1)), "{st}");
    // Running statuses at most every WEB_PUBLISH_EVERY (plus the refreshes around the run and
    // the decisions' opening and closing).
    let decisions = page.st.lock().unwrap().pulls.iter().filter(|b| **b).count();
    let running = page.published() - n0;
    let most = 3 + (elapsed.as_millis() / WEB_PUBLISH_EVERY.as_millis()) as usize + 2 * decisions;
    assert!(running <= most, "{running} statuses in {elapsed:?}");

    // More turns.
    for k in 2..=4 {
        page.send(&mut game, &post(&cmd("end_turn").with("force", true))).ok();
        let st = page.last_status();
        assert_eq!((phase(&st), st.req::<i32>("turn")), ("human", Ok(turn + k)), "{st}");
    }
    // A save, listed.
    page.send(&mut game, &post(&ffh_play::jobj! { "k" => "session", "op" => "save", "name" => "mine" })).ok();
    let saves = page.send(&mut game, &post(&ffh_play::jobj! { "k" => "session", "op" => "saves" })).ok();
    assert!(saves.to_string().contains("mine"), "{saves}");
    // Quit: the game takes no more envelopes; GETs are still answered.
    page.send(&mut game, &post(&ffh_play::jobj! { "k" => "session", "op" => "quit" })).ok();
    assert_eq!(page.send(&mut game, &post(&ffh_play::jobj! { "k" => "q", "q" => "status" })).status, 503);
    assert_eq!(page.send(&mut game, &get("/api/view")).status, 200);
    page.check_one_reply_each();
}

trait WithPath {
    fn with_path(self, p: &str) -> Self;
}

impl WithPath for HttpRequest {
    fn with_path(mut self, p: &str) -> HttpRequest {
        self.path = p.into();
        self
    }
}

/// The keep/raze decision of a capture opens inside the `move`: the engine pulls (blocking) the
/// requests the page queued: GETs from the decision's caches, queries from the deciding side,
/// commands and session operations `decision_pending`, bad answers refused, then the answer;
/// the `move` gets its reply last.
#[test]
fn web_host_a_decision_is_answered_through_pulled_requests() {
    let tmp = TempDir::new("web-host-decision");
    let root = tmp.0.join("saves");
    let staged = staged_capture(&root.join("staged"));
    let page = Page::new(false);
    let mut game = start(&root, &page, true);
    page.send(&mut game, &post(&ffh_play::jobj! { "k" => "session", "op" => "load", "name" => "staged/capture" })).ok();
    let st = page.last_status();
    assert_eq!((phase(&st), st.req::<i32>("turn")), ("human", Ok(staged.turn)));
    let before: i64 = st.req("view_version").unwrap();

    let mut calls = 0;
    let first = page.st.lock().unwrap().next + 1;
    page.script(move |block, published| {
        if !block {
            return Vec::new();
        }
        calls += 1;
        let st = published.last().unwrap();
        assert_eq!(phase(st), "decision", "{st}");
        let d = st.get("decision").unwrap();
        assert_eq!(d.req_str("kind"), Ok("keep_city"));
        let id: u32 = d.req("id").unwrap();
        match calls {
            1 => vec![
                get("/api/status"),
                get("/api/view"),
                post(&ffh_play::jobj! { "k" => "q", "q" => "view" }),
                post(&cmd("end_turn").with("force", true)),
                post(&ffh_play::jobj! { "k" => "session", "op" => "save", "name" => "x" }),
                post(&ffh_play::jobj! { "k" => "answer", "decision" => id, "option" => 9 }),
                post(&ffh_play::jobj! { "k" => "answer", "decision" => id + 1, "option" => 0 }),
                get("/api/wait?since=0"),
                get("/api/data"),
            ],
            2 => vec![post(&ffh_play::jobj! { "id" => "a", "k" => "answer", "decision" => id, "option" => 0 })],
            _ => panic!("pulled again after the answer"),
        }
    });
    let mv = cmd("move").with("id", 7).with("units", vec![staged.unit]).with("x", staged.x).with("y", staged.y);
    let reply = page.send(&mut game, &post(&mv));
    let pulled: Vec<Reply> = (first..first + 10).map(|t| page.reply_of(t)).collect();
    let status = pulled[0].json();
    assert_eq!(phase(&status), "decision");
    let in_decision: i64 = status.req("view_version").unwrap();
    assert!(in_decision > before, "the decision's view is a new one");
    assert_eq!(pulled[1].json().req::<i64>("view_version"), Ok(in_decision));
    assert!(pulled[2].ok().get("me").is_some());
    let errors: Vec<String> = pulled[3..7].iter().map(Reply::error).collect();
    assert_eq!(errors, ["decision_pending", "decision_pending", "invalid", "gone"]);
    assert_eq!(phase(&pulled[7].json()), "decision");
    assert_eq!(pulled[8].status, 200);
    assert_eq!(pulled[9].ok().req_str("key"), Ok("keep"));
    // The move's reply comes last.
    assert_eq!(page.st.lock().unwrap().replies.last().unwrap().token, reply.token);
    let r = reply.ok();
    assert_eq!(reply.json().req::<i64>("id"), Ok(7));
    assert_eq!(r.req::<bool>("applied"), Ok(true));
    let log: Vec<String> = reply.json().req_arr("log").unwrap().iter().map(|e| e.req_str("text").unwrap().to_string()).collect();
    assert!(log.iter().any(|t| t.contains("captured")), "{log:?}");
    // One blocking pull per request (the page queued them in two rounds).
    assert_eq!(page.st.lock().unwrap().pulls.iter().filter(|b| **b).count(), 10);
    // The decision opened and closed in the published statuses; control again after.
    let phases: Vec<String> = page.st.lock().unwrap().published.iter().map(|(_, s)| phase(s).to_string()).collect();
    let open = phases.iter().position(|p| p == "decision").expect("the decision was published");
    assert_eq!(phases[open + 1], "running");
    let st = page.last_status();
    assert_eq!(phase(&st), "human");
    assert!(st.get("decision").unwrap().is_null());
    let view = page.send(&mut game, &get("/api/view")).json();
    assert!(view.req::<i64>("view_version").unwrap() > in_decision);
    let ours = view.req_arr("cities").unwrap().iter().filter(|c| c.req::<i32>("owner") == Ok(0)).count();
    assert_eq!(ours, 2, "the capital and the kept city");
    page.check_one_reply_each();
}

/// A page that cannot deliver requests while the engine runs (`pull` -> unsupported), or an
/// engine started with `blocking: false`: the advisor answers the decision and the `move` gets
/// its reply.
#[test]
fn web_host_without_pulls_the_advisor_answers() {
    let tmp = TempDir::new("web-host-advisor");
    let root = tmp.0.join("saves");
    let staged = staged_capture(&root.join("staged"));
    for (unsupported, blocking) in [(true, true), (false, false)] {
        let page = Page::new(unsupported);
        let mut game = start(&root, &page, blocking);
        page.send(&mut game, &post(&ffh_play::jobj! { "k" => "session", "op" => "load", "name" => "staged/capture" })).ok();
        let mv = cmd("move").with("units", vec![staged.unit]).with("x", staged.x).with("y", staged.y);
        let reply = page.send(&mut game, &post(&mv));
        assert_eq!(reply.ok().req::<bool>("applied"), Ok(true), "unsupported {unsupported}, blocking {blocking}");
        let log: Vec<String> = reply.json().req_arr("log").unwrap().iter().map(|e| e.req_str("text").unwrap().to_string()).collect();
        assert!(log.iter().any(|t| t.contains("captured")), "{log:?}");
        assert_eq!(game.host().session().unwrap().stats().decisions_of("keep_city"), 1);
        let st = page.st.lock().unwrap();
        let decision_published = st.published.iter().any(|(_, s)| phase(s) == "decision");
        if blocking {
            // Asked once (the decision was published, then closed), never again.
            assert_eq!(st.pulls.iter().filter(|b| **b).count(), 1);
            assert!(decision_published);
        } else {
            assert!(st.pulls.is_empty());
            assert!(!decision_published);
        }
        drop(st);
        assert_eq!(phase(&page.last_status()), "human");
        page.check_one_reply_each();
    }
}
