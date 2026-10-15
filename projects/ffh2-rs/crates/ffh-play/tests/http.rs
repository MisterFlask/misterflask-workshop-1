//! T12: HTTP end to end (design §8): `ffh-play serve`'s server on 127.0.0.1:0 in this process, a
//! `TcpStream` client. The client files and tables, a new game played for 10 turns with commands
//! and long-polls, the keep/raze decision of a capture answered on another connection while the
//! command's request waits, the `X-FFH` and `Host` checks, save names that try to leave the saves
//! directory, a crash that leaves the server up, and `quit`.

mod common;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use common::{apply_map, assert_same_map, staged_capture, TempDir};
use ffh_data::*;
use ffh_play::json::Json;
use ffh_play::transport::server::{ServeOptions, Server};
use ffh_play::transport::{HostOptions, Start};

struct Resp {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Resp {
    fn json(&self) -> Json {
        Json::parse_bytes(&self.body).unwrap_or_else(|e| panic!("not JSON ({e}): {}", String::from_utf8_lossy(&self.body)))
    }

    fn header(&self, k: &str) -> Option<&str> {
        self.headers.iter().find(|(h, _)| h.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
    }
}

#[derive(Copy, Clone)]
struct Client {
    addr: SocketAddr,
}

impl Client {
    /// Sends `req` as it is and reads the response to the end (the server closes).
    fn raw(&self, req: &[u8]) -> Resp {
        let mut s = TcpStream::connect(self.addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(300))).unwrap();
        s.write_all(req).unwrap();
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).unwrap();
        let end = buf.windows(4).position(|w| w == b"\r\n\r\n").expect("a response head") + 4;
        let head = std::str::from_utf8(&buf[..end]).unwrap();
        let mut lines = head.split("\r\n");
        let status_line = lines.next().unwrap();
        assert!(status_line.starts_with("HTTP/1.1 "), "{status_line}");
        let status: u16 = status_line[9..12].parse().unwrap();
        let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(": ")).map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let resp = Resp { status, headers, body: buf[end..].to_vec() };
        assert_eq!(resp.header("Connection"), Some("close"));
        assert_eq!(resp.header("Content-Length").map(|l| l.parse::<usize>().unwrap()), Some(if status == 304 { 0 } else { resp.body.len() }));
        resp
    }

    fn get_with(&self, path: &str, extra: &str) -> Resp {
        self.raw(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n{extra}\r\n", self.addr.port()).as_bytes())
    }

    fn get(&self, path: &str) -> Resp {
        self.get_with(path, "")
    }

    /// `POST /api` with the envelope; the reply.
    fn post(&self, j: &Json) -> Json {
        let body = j.to_string();
        let r = self.raw(format!("POST /api HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-FFH: 1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", self.addr.port(), body.len()).as_bytes());
        assert_eq!(r.status, 200);
        r.json()
    }

    /// The reply's `r`, which must be ok.
    fn ok(&self, j: Json) -> Json {
        let r = self.post(&j);
        assert_eq!(r.req::<bool>("ok"), Ok(true), "{j} -> {r}");
        r.get("r").cloned().unwrap()
    }

    fn status(&self) -> Json {
        let r = self.get("/api/status");
        assert_eq!(r.status, 200);
        r.json()
    }

    /// Long-polls `/api/wait` until `pred` holds for the status.
    fn wait_for(&self, what: &str, pred: impl Fn(&Json) -> bool) -> Json {
        let deadline = Instant::now() + Duration::from_secs(300);
        let mut st = self.status();
        while !pred(&st) {
            assert!(Instant::now() < deadline, "waiting for {what}: {st}");
            let v: i64 = st.req("version").unwrap();
            st = self.get(&format!("/api/wait?since={v}&timeout_ms=5000")).json();
        }
        st
    }
}

fn phase(st: &Json) -> &str {
    st.req_str("phase").unwrap()
}

fn start(root: &std::path::Path, panic_at: Option<i32>) -> (Server, Client) {
    let opts = ServeOptions {
        bind: SocketAddr::from(([127, 0, 0, 1], 0)),
        host: HostOptions { saves: Some(root.to_path_buf()), autosave_keep: 5, panic_at },
        start: Start::None,
        assets: None,
    };
    let server = Server::start(opts).expect("server");
    let c = Client { addr: server.addr() };
    (server, c)
}

fn cmd(c: &str) -> Json {
    ffh_play::jobj! { "k" => "cmd", "c" => c }
}

#[test]
fn t12_http_end_to_end() {
    let tmp = TempDir::new("t12-play");
    let root = tmp.0.join("saves");
    let (server, c) = start(&root, None);

    // The client files and the tables.
    let index = c.get("/");
    assert_eq!(index.status, 200);
    assert!(index.header("Content-Type").unwrap().starts_with("text/html"));
    assert!(String::from_utf8_lossy(&index.body).contains("<title>ffh-play</title>"));
    let data = c.get("/api/data");
    assert_eq!(data.status, 200);
    assert!(data.json().get("tables").and_then(|t| t.get("UnitType")).is_some());
    let etag = data.header("ETag").unwrap().to_string();
    assert_eq!(c.get_with("/api/data", &format!("If-None-Match: {etag}\r\n")).status, 304);
    let setup = c.get("/api/setup");
    assert_eq!(setup.status, 200);
    assert!(!setup.json().req_arr("civilizations").unwrap().is_empty());

    // No game yet.
    let st = c.status();
    assert_eq!(phase(&st), "no_game");
    assert_eq!(c.get("/api/view").status, 409);
    assert_eq!(c.get("/api/map").status, 409);
    assert_eq!(c.post(&cmd("end_turn")).req_str("error"), Ok("no_game"));
    // A long-poll with nothing new times out with the same status.
    let v: i64 = c.status().req("version").unwrap();
    assert_eq!(c.get(&format!("/api/wait?since={v}&timeout_ms=50")).json().req::<i64>("version"), Ok(v));

    // The safety checks.
    let body = r#"{"k":"q","q":"status"}"#;
    let port = c.addr.port();
    let no_header = c.raw(format!("POST /api HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes());
    assert_eq!(no_header.status, 403);
    let other_host = c.raw(format!("POST /api HTTP/1.1\r\nHost: evil.example:{port}\r\nX-FFH: 1\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes());
    assert_eq!(other_host.status, 403);
    assert_eq!(c.raw(format!("POST /api HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-FFH: 1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n").as_bytes()).status, 400);
    assert_eq!(c.raw(format!("POST /api HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-FFH: 1\r\n\r\n").as_bytes()).status, 411);
    assert_eq!(c.raw(format!("POST /api HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-FFH: 1\r\nContent-Length: 2000000\r\n\r\n").as_bytes()).status, 413);
    for path in ["/app/../Cargo.toml", "/app/..%2fCargo.toml", "/../../etc/passwd", "/app/.hidden"] {
        assert_eq!(c.get(path).status, 404, "{path}");
    }
    assert_eq!(c.get("/api").status, 405);
    let bad_json = c.raw(format!("POST /api HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-FFH: 1\r\nContent-Length: 5\r\n\r\n{{\"k\":").as_bytes());
    assert_eq!(bad_json.json().req_str("error"), Ok("invalid"));

    // A new game, played for 10 turns.
    let setup = ffh_play::jobj! { "seed" => 5, "world" => "WORLDSIZE_DUEL", "opponents" => 1 };
    let r = c.ok(ffh_play::jobj! { "id" => 1, "k" => "session", "op" => "new", "setup" => setup });
    assert_eq!(r.req_str("game"), Ok(root.join("game-5").to_str().unwrap()));
    let mut st = c.wait_for("the first control point", |st| phase(st) == "human" && st.get("game").is_some_and(|g| !g.get("autosave").unwrap().is_null()));
    let found = MissionType::MISSION_FOUND.idx() as i64;
    let view = c.get("/api/view").json();
    let mut founded = false;
    for g in view.req_arr("groups").unwrap() {
        let units = g.get("units").unwrap().clone();
        let group = c.ok(ffh_play::jobj! { "k" => "q", "q" => "group", "units" => units });
        let action = group.req_arr("actions").unwrap().iter().find(|a| a.req::<i64>("mission") == Ok(found) && a.req::<bool>("enabled") == Ok(true)).cloned();
        if let Some(a) = action {
            let mut cmd = a.get("cmd").unwrap().clone();
            cmd.set("k", "cmd");
            c.ok(cmd);
            founded = true;
            break;
        }
    }
    assert!(founded, "the settler founds the capital");
    let mut researched = 0;
    // The map as a client holds it, kept up to date with the tile deltas (phase 6).
    let mut client_map = c.get("/api/map").json();
    let mut deltas = 0;
    let mut trained = 0;
    for turn in 0..10 {
        assert_eq!(st.req::<i32>("turn"), Ok(turn));
        let pending = c.ok(ffh_play::jobj! { "k" => "q", "q" => "pending" });
        if pending.req::<bool>("research_needed") == Ok(true) {
            let techs = c.ok(ffh_play::jobj! { "k" => "q", "q" => "techs" });
            let t = techs.req_arr("techs").unwrap().iter().find(|t| t.req::<bool>("can_research") == Ok(true) && t.req::<bool>("has") == Ok(false)).unwrap().req::<i64>("tech").unwrap();
            c.ok(cmd("research").with("tech", t));
            researched += 1;
        }
        for city in pending.req_arr("idle_cities").unwrap() {
            let r = c.post(&cmd("push_order").with("city", city.clone()).with("order", "ORDER_TRAIN").with("data", "UNIT_WARRIOR"));
            if r.req::<bool>("ok") == Ok(true) {
                trained += 1;
            }
        }
        let ended = c.ok(cmd("end_turn").with("force", true));
        assert_eq!(ended.req::<bool>("applied"), Ok(true));
        // Served while the AI plays: status and view never wait for the game thread.
        assert_eq!(c.get("/api/view").status, 200);
        st = c.wait_for("the next control point", |st| phase(st) == "human" && st.req::<i32>("turn") == Ok(turn + 1));
        let (rev, epoch) = (client_map.req::<i64>("rev").unwrap(), client_map.req_str("epoch").unwrap().to_string());
        let delta = c.get(&format!("/api/map?since={rev}&epoch={epoch}")).json();
        if delta.req::<bool>("full") == Ok(false) {
            deltas += 1;
            assert!(delta.req_arr("tiles").unwrap().len() < delta.req::<usize>("w").unwrap() * delta.req::<usize>("h").unwrap() / 3);
        }
        client_map = apply_map(&client_map, &delta);
        assert_same_map(&client_map, &c.get("/api/map").json(), "the map after a delta");
        assert_eq!(client_map.get("map_rev"), st.get("map_rev"));
    }
    assert!(researched >= 1 && trained >= 1, "research {researched}, trained {trained}");
    assert!(deltas >= 5, "{deltas} tile deltas in 10 turns");
    // A revision of another epoch (another server, another game) gets the full map.
    assert_eq!(c.get("/api/map?since=1&epoch=0-0").json().req::<bool>("full"), Ok(true));
    let view = c.get("/api/view").json();
    assert_eq!(view.req::<i32>("turn"), Ok(10));
    assert_eq!(view.req::<i64>("view_version"), st.req::<i64>("view_version"));
    assert!(view.req_arr("cities").unwrap().iter().any(|c| c.req::<i32>("owner") == Ok(0)));
    let map = c.get("/api/map").json();
    assert_eq!(map.get("map_rev"), st.get("map_rev"));
    let journal = st.get("game").unwrap().req_str("journal").unwrap().to_string();
    assert!(std::fs::read_to_string(&journal).unwrap().lines().filter(|l| l.contains("\"k\":\"cmd\"")).count() >= 12);

    // Saves: names stay in the saves root.
    let saved = c.ok(ffh_play::jobj! { "k" => "session", "op" => "save", "name" => "mid" });
    assert!(std::path::Path::new(saved.req_str("save").unwrap()).is_file());
    for (op, name) in [("save", "../x"), ("save", "a/b"), ("load", "../../etc/passwd"), ("load", "game-5/../../x"), ("load", "/etc/passwd"), ("take_over", "..")] {
        let mut req = ffh_play::jobj! { "k" => "session", "op" => op, "name" => name };
        if op == "take_over" {
            req.set("player", 1);
        }
        assert_eq!(c.post(&req).req_str("error"), Ok("invalid"), "{op} {name}");
    }
    assert_eq!(c.post(&ffh_play::jobj! { "k" => "session", "op" => "load", "name" => "nosuch/x" }).req_str("error"), Ok("gone"));
    let saves = c.ok(ffh_play::jobj! { "k" => "session", "op" => "saves" });
    let game = &saves.req_arr("games").unwrap()[0];
    assert_eq!(game.req_str("game"), Ok("game-5"));
    assert!(game.req_arr("saves").unwrap().iter().any(|s| s.req_str("name") == Ok("mid")));
    // Loading the save resumes at the same control point.
    c.ok(ffh_play::jobj! { "k" => "session", "op" => "load", "name" => "mid" });
    let st = c.wait_for("the loaded game", |st| phase(st) == "human" && st.req::<i32>("turn") == Ok(10));
    assert_eq!(st.get("game").unwrap().req_str("dir"), Ok(root.join("game-5").to_str().unwrap()));
    // Debug reveal is marked.
    c.ok(ffh_play::jobj! { "k" => "session", "op" => "debug", "reveal" => true });
    assert_eq!(c.get("/api/view").json().req::<bool>("reveal"), Ok(true));

    // Quit stops the server.
    assert_eq!(c.ok(ffh_play::jobj! { "k" => "session", "op" => "quit" }).req::<bool>("quit"), Ok(true));
    server.wait();
    assert!(TcpStream::connect(c.addr).is_err(), "the listener is closed");
}

/// The keep/raze decision of a capture is raised inside the `move`: its request waits while a
/// long-poll sees `phase: decision`, another connection reads the view and is refused a command,
/// and a third answers; then the `move` gets its reply (design §2.4, §8 T12).
#[test]
fn t12_a_decision_inside_a_command_is_answered_on_another_connection() {
    let tmp = TempDir::new("t12-decision");
    let root = tmp.0.join("saves");
    let staged = staged_capture(&root.join("staged"));
    let (server, c) = start(&root, None);
    c.ok(ffh_play::jobj! { "k" => "session", "op" => "load", "name" => "staged/capture" });
    let st = c.wait_for("the loaded game", |st| phase(st) == "human");
    assert_eq!(st.req::<i32>("turn"), Ok(staged.turn));
    let version = |j: &Json| j.req::<i64>("view_version").unwrap();
    let before = version(&st);
    assert_eq!(version(&c.get("/api/view").json()), before);

    let mv = cmd("move").with("id", 7).with("units", vec![staged.unit]).with("x", staged.x).with("y", staged.y);
    let waiting = std::thread::spawn(move || c.post(&mv));
    let st = c.wait_for("the decision", |st| phase(st) == "decision");
    let d = st.get("decision").unwrap();
    assert_eq!(d.req_str("kind"), Ok("keep_city"));
    // The city's name, not its text key (P4 review).
    assert!(!d.req_str("text").unwrap().contains("TXT_KEY"), "{d}");
    let id: u32 = d.req("id").unwrap();
    assert!(!waiting.is_finished(), "the move's request waits for the answer");
    // The decision's view (the city captured) is a new view (P4 review).
    let in_decision = version(&st);
    assert!(in_decision > before, "{st}");
    assert_eq!(version(&c.get("/api/view").json()), in_decision);
    // Other connections meanwhile.
    assert_eq!(c.get("/api/view").status, 200);
    let view = c.ok(ffh_play::jobj! { "k" => "q", "q" => "view" });
    assert!(view.get("me").is_some());
    assert_eq!(c.post(&cmd("end_turn").with("force", true)).req_str("error"), Ok("decision_pending"));
    assert_eq!(c.post(&ffh_play::jobj! { "k" => "session", "op" => "save", "name" => "x" }).req_str("error"), Ok("decision_pending"));
    assert_eq!(c.post(&ffh_play::jobj! { "k" => "answer", "decision" => id, "option" => 9 }).req_str("error"), Ok("invalid"));
    let answered = c.ok(ffh_play::jobj! { "k" => "answer", "decision" => id, "option" => 0 });
    assert_eq!(answered.req_str("key"), Ok("keep"));

    let reply = waiting.join().unwrap();
    assert_eq!(reply.req::<i64>("id"), Ok(7));
    assert_eq!(reply.get("r").unwrap().req::<bool>("applied"), Ok(true), "{reply}");
    let log: Vec<&str> = reply.req_arr("log").unwrap().iter().map(|e| e.req_str("text").unwrap()).collect();
    assert!(log.iter().any(|t| t.contains("captured")), "{log:?}");
    let st = c.wait_for("control again", |st| phase(st) == "human");
    assert!(st.get("decision").unwrap().is_null());
    let view = c.get("/api/view").json();
    assert!(version(&st) > in_decision);
    assert_eq!(version(&view), version(&st));
    let ours = view.req_arr("cities").unwrap().iter().filter(|c| c.req::<i32>("owner") == Ok(0)).count();
    assert_eq!(ours, 2, "the capital and the kept city");
    // Debug reveal changes the view: a new version (P4 review).
    c.ok(ffh_play::jobj! { "k" => "session", "op" => "debug", "reveal" => true });
    let revealed = c.status();
    assert!(version(&revealed) > version(&st));
    assert_eq!(version(&c.get("/api/view").json()), version(&revealed));
    server.shutdown();
}

/// The AutoMoves rule between commands (C++ `updateTestEndTurn`): skipping the last ready unit
/// while another group has queued orders replies `running`, and the orders move their group
/// before the human gets control again in the same turn.
#[test]
fn t12_auto_moves_run_when_the_last_ready_unit_is_skipped() {
    let tmp = TempDir::new("t12-automoves");
    let root = tmp.0.join("saves");
    let staged = common::staged_auto_moves(&root.join("staged"));
    let (server, c) = start(&root, None);
    c.ok(ffh_play::jobj! { "k" => "session", "op" => "load", "name" => "staged/automoves" });
    c.wait_for("the loaded game", |st| phase(st) == "human");
    let at = |view: &Json| {
        let u = view.req_arr("units").unwrap().iter().find(|u| u.req::<u32>("id") == Ok(staged.mover.0)).cloned().unwrap();
        (u.req::<i32>("x").unwrap(), u.req::<i32>("y").unwrap())
    };
    assert_eq!(at(&c.get("/api/view").json()), staged.from);
    let skip = c.post(&cmd("mission").with("units", vec![staged.ready]).with("mission", "MISSION_SKIP"));
    assert_eq!(skip.get("status").unwrap().req_str("phase"), Ok("running"), "{skip}");
    let st = c.wait_for("control again", |st| phase(st) == "human");
    assert_eq!(st.req::<i32>("turn"), Ok(staged.turn));
    assert_ne!(at(&c.get("/api/view").json()), staged.from, "the queued order moved its group");
    server.shutdown();
}

/// A long AI autoplay can be stopped while it runs (P6 review: only killing the server did):
/// `autoplay {turns: 0}` gets `stopping` while other commands get `not_your_turn` and session
/// operations `busy`; the status counts the turns left; the human has control again at its next
/// turn with the autoplay off, and the stop is in the journal. At a control point `turns: 0` is
/// invalid.
#[test]
fn t12_a_running_autoplay_stops() {
    let tmp = TempDir::new("t12-autoplay-stop");
    let (server, c) = start(&tmp.0.join("saves"), None);
    c.ok(ffh_play::jobj! { "k" => "session", "op" => "new", "setup" => ffh_play::jobj! { "seed" => 4, "world" => "WORLDSIZE_DUEL", "opponents" => 1 } });
    let st = c.wait_for("the first turn", |st| phase(st) == "human");
    let from: i32 = st.req("turn").unwrap();
    assert_eq!(st.req::<i32>("ai_auto_play"), Ok(0));
    let r = c.post(&cmd("autoplay").with("turns", 0));
    assert_eq!(r.req_str("error"), Ok("invalid"), "{r}");
    c.ok(cmd("autoplay").with("turns", 10000));
    let st = c.wait_for("the autoplay under way", |st| phase(st) != "human" || st.req::<i32>("turn").unwrap() > from);
    assert_eq!(phase(&st), "running", "{st}");
    let r = c.post(&cmd("end_turn"));
    assert_eq!(r.req_str("error"), Ok("not_your_turn"), "{r}");
    let r = c.post(&ffh_play::jobj! { "k" => "session", "op" => "save", "name" => "x" });
    assert_eq!(r.req_str("error"), Ok("busy"), "{r}");
    let stop = c.ok(cmd("autoplay").with("turns", 0));
    assert_eq!(stop.req::<bool>("stopping"), Ok(true), "{stop}");
    assert!(stop.req::<i32>("turns_left").unwrap() > 9000, "{stop}");
    let st = c.wait_for("control again", |st| phase(st) != "running");
    assert_eq!(phase(&st), "human", "{st}");
    assert_eq!(st.req::<i32>("ai_auto_play"), Ok(0));
    let turn: i32 = st.req("turn").unwrap();
    assert!(turn > from && turn < from + 200, "turn {turn} (from {from})");
    let journal = st.get("game").and_then(|g| g.req_str("journal").ok()).map(std::path::PathBuf::from).expect("the journal");
    let lines = ffh_play::journal::read_file(&journal).unwrap();
    assert!(lines.iter().any(|l| matches!(l, ffh_play::journal::Line::Cmd { cmd, .. } if ffh_play::journal::is_running_cmd(cmd))), "the stop journaled");
    server.shutdown();
}

/// Connection limits and hosts (P4 review): bound to another loopback address, requests naming
/// that address are served; past `MAX_CONNECTIONS` open connections a new one gets a 503 that it
/// can read (not a reset), and once they close the server answers again.
#[test]
fn t12_a_full_server_answers_503_and_its_own_address_is_a_host() {
    let opts = ServeOptions {
        bind: SocketAddr::from(([127, 0, 0, 2], 0)),
        host: HostOptions { saves: None, autosave_keep: 5, panic_at: None },
        start: Start::None,
        assets: None,
    };
    let server = Server::start(opts).expect("server on 127.0.0.2");
    let c = Client { addr: server.addr() };
    let port = c.addr.port();
    let get = |host: &str| c.raw(format!("GET /api/status HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes()).status;
    assert_eq!(get(&format!("127.0.0.2:{port}")), 200);
    assert_eq!(get(&format!("localhost:{port}")), 200);
    assert_eq!(get(&format!("127.0.0.3:{port}")), 403);
    // Idle connections fill the server (each holds a connection thread until its request arrives).
    let idle: Vec<TcpStream> = (0..ffh_play::transport::server::MAX_CONNECTIONS).map(|_| TcpStream::connect(c.addr).unwrap()).collect();
    let busy = c.raw(format!("GET /api/status HTTP/1.1\r\nHost: 127.0.0.2:{port}\r\n\r\n").as_bytes());
    assert_eq!(busy.status, 503);
    assert_eq!(busy.json().req_str("error"), Ok("busy"));
    // A client that reads late: the server's close must not reset the connection (a close with
    // the request unread would, and the reset discards the answer): the request is drained.
    let mut slow = TcpStream::connect(c.addr).unwrap();
    slow.write_all(format!("GET /api/status HTTP/1.1\r\nHost: 127.0.0.2:{port}\r\n\r\n").as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    let mut answer = Vec::new();
    slow.read_to_end(&mut answer).expect("the 503, not a reset");
    assert!(answer.starts_with(b"HTTP/1.1 503 "), "{}", String::from_utf8_lossy(&answer));
    drop(idle);
    let deadline = Instant::now() + Duration::from_secs(30);
    while get(&format!("127.0.0.2:{port}")) != 200 {
        assert!(Instant::now() < deadline, "the server answers again once the idle connections close");
        std::thread::sleep(Duration::from_millis(50));
    }
    server.shutdown();
}

/// A rules panic on the game thread: the status says `crashed` with the report, commands get
/// `no_game`, and loading the autosave plays on.
#[test]
fn t12_a_crash_leaves_the_server_up() {
    let tmp = TempDir::new("t12-crash");
    let root = tmp.0.join("saves");
    let (server, c) = start(&root, Some(1));
    c.ok(ffh_play::jobj! { "k" => "session", "op" => "new", "setup" => ffh_play::jobj! { "seed" => 6, "world" => "WORLDSIZE_DUEL", "opponents" => 1 } });
    c.wait_for("the first control point", |st| phase(st) == "human");
    // The `status` query is the published status's (P4 review): with the game directory.
    let q = c.ok(ffh_play::jobj! { "k" => "q", "q" => "status" });
    assert_eq!(q.get("game"), c.status().get("game"), "{q}");
    c.ok(cmd("end_turn").with("force", true));
    let st = c.wait_for("the crash", |st| phase(st) == "crashed");
    let crash = st.get("crash").unwrap();
    assert!(crash.req_str("message").unwrap().contains("--panic-at 1"));
    let q = c.ok(ffh_play::jobj! { "k" => "q", "q" => "status" });
    assert_eq!((q.req_str("phase"), q.get("crash")), (Ok("crashed"), Some(crash)), "{q}");
    let autosave = crash.req_str("autosave").unwrap();
    assert!(std::path::Path::new(autosave).is_file());
    assert!(crash.req_str("repro").unwrap().starts_with("ffh-play replay "));
    assert_eq!(c.post(&cmd("end_turn")).req_str("error"), Ok("no_game"));
    assert_eq!(c.get("/api/view").status, 409);
    // The autosave by name: GAME/auto/NAME.
    let name = format!("game-6/auto/{}", std::path::Path::new(autosave).file_stem().unwrap().to_str().unwrap());
    c.ok(ffh_play::jobj! { "k" => "session", "op" => "load", "name" => name });
    let st = c.wait_for("the loaded autosave", |st| phase(st) == "human");
    assert!(st.get("crash").is_none());
    assert_eq!(st.req::<i32>("turn"), Ok(1));
    server.shutdown();
}
