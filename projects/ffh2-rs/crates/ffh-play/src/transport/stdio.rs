//! The agent transport (`ffh-play agent`, design §4.1): JSONL over stdin and stdout, on the game
//! thread (no other thread). Each input line is one request of the envelope; each output line is
//! a reply (`{"id", "ok", ..}`) or an event (`{"ev", ..}`):
//!
//! * `control` when the human gets control (at the start, after the end of a turn, a load, or a
//!   command after which the AutoMoves rule continued the queued orders):
//!   `{"ev":"control","turn","year","human","pending","log_next"}`;
//! * `decision` when a blocking decision is raised inside a rules call: the next lines are read
//!   by the decision ([`StdioAnswerer`]): queries are answered, an `answer` closes it, commands
//!   and session operations get `decision_pending`;
//! * `over` (the game ended), `stuck` (the watchdog stopped a turn), `progress` (with
//!   `--progress`: the active players during the AI turns, at most 4 a second), `no_game`
//!   (started without a game), `crashed` (a panic: the message, the last autosave, the journal and
//!   the replay command; exit code 3), `error` (the start failed; exit code 2), `answered` (a
//!   decision the end of input answered with the advisor).
//!
//! At the end of input an open decision takes the advisor's pick; then the agent quits, or with
//! `--on-eof end-turn` first ends the turn (forced) and reports the next control point.

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ffh_core::game::Game;

use super::{decision_event, decision_reply, err_reply, install_panic_hook, take_panic, After, Host, HostOptions, ProtoErr, Request, Start};
use crate::decision::Decision;
use crate::human::{Answer, Answerer, DecisionReader};
use crate::json::Json;
use crate::session::Phase;

/// What the agent does at the end of its input.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OnEof {
    /// Quit (the default).
    Quit,
    /// End the turn (forced), run to the next control point, report it, then quit.
    EndTurn,
}

/// `ffh-play agent`'s settings.
#[derive(Clone, Debug)]
pub struct AgentOptions {
    pub host: HostOptions,
    pub start: Start,
    pub on_eof: OnEof,
    /// `progress` events during the AI turns.
    pub progress: bool,
}

/// Stdin and stdout, shared by the loop and the decision answerer. Each is locked only to read
/// or write one line (never across a rules call).
pub struct Io {
    input: Mutex<Box<dyn BufRead + Send>>,
    output: Mutex<Box<dyn Write + Send>>,
    eof: AtomicBool,
    /// Writing failed (the reader went away).
    closed: AtomicBool,
}

impl Io {
    pub fn new(input: Box<dyn BufRead + Send>, output: Box<dyn Write + Send>) -> Io {
        Io { input: Mutex::new(input), output: Mutex::new(output), eof: AtomicBool::new(false), closed: AtomicBool::new(false) }
    }

    /// The next line without its line end; `None` at the end of input (and after it); an error
    /// for a line that is not UTF-8.
    pub fn read_line(&self) -> Option<Result<String, String>> {
        if self.eof.load(Ordering::Relaxed) {
            return None;
        }
        let mut buf = Vec::new();
        let r = self.input.lock().unwrap_or_else(|e| e.into_inner()).read_until(b'\n', &mut buf);
        match r {
            Ok(0) | Err(_) => {
                self.eof.store(true, Ordering::Relaxed);
                None
            }
            Ok(_) => {
                while buf.last().is_some_and(|&b| b == b'\n' || b == b'\r') {
                    buf.pop();
                }
                Some(String::from_utf8(buf).map_err(|_| "the line is not UTF-8".to_string()))
            }
        }
    }

    /// Writes one line (and flushes).
    pub fn write(&self, j: &Json) {
        let mut out = self.output.lock().unwrap_or_else(|e| e.into_inner());
        if writeln!(out, "{j}").and_then(|_| out.flush()).is_err() {
            self.closed.store(true, Ordering::Relaxed);
        }
    }

    pub fn at_eof(&self) -> bool {
        self.eof.load(Ordering::Relaxed)
    }

    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }
}

/// Answers blocking decisions from the input (design §2.4): prints the `decision` event and
/// reads lines until an `answer` closes it; at the end of input, the advisor's pick.
pub struct StdioAnswerer {
    pub io: Arc<Io>,
}

impl Answerer for StdioAnswerer {
    fn ask(&self, _g: &Game, _d: &Decision) -> Answer {
        Answer::Advisor
    }

    fn ask_with(&self, r: &DecisionReader, d: &Decision) -> Answer {
        self.io.write(&decision_event(d));
        loop {
            let line = match self.io.read_line() {
                None => {
                    let option = d.default_option();
                    let key = d.options.get(option).map_or("", |o| o.key.as_str());
                    self.io.write(&crate::jobj! { "ev" => "answered", "decision" => d.id, "option" => option, "key" => key, "by" => "advisor (end of input)" });
                    return Answer::Advisor;
                }
                Some(Ok(l)) => l,
                Some(Err(e)) => {
                    self.io.write(&err_reply(Json::Null, &ProtoErr::invalid(e)));
                    continue;
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let (id, req) = Request::parse(&line);
            let (reply, answer) = decision_reply(r, d, id, req);
            self.io.write(&reply);
            if let Some(a) = answer {
                return a;
            }
        }
    }
}

/// Runs the agent on `input` / `output` until `quit`, the end of input or a crash. Returns the
/// exit code: 0, 2 (the start failed) or 3 (a panic, reported as a `crashed` event).
pub fn run(input: Box<dyn BufRead + Send>, output: Box<dyn Write + Send>, opts: &AgentOptions) -> i32 {
    install_panic_hook();
    let io = Arc::new(Io::new(input, output));
    let mut host = Host::new(opts.host.clone(), Arc::new(StdioAnswerer { io: io.clone() }));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| serve(&mut host, &io, opts)));
    match r {
        Ok(code) => code,
        Err(_) => {
            let report = host.crashed(&take_panic());
            let mut ev = crate::jobj! { "ev" => "crashed" };
            for (k, v) in report.as_obj().unwrap_or_default() {
                ev.set(k, v.clone());
            }
            io.write(&ev);
            3
        }
    }
}

/// `Host::advance` with the `progress` events.
fn advance(host: &mut Host, io: &Io, progress: bool) -> Json {
    let mut last = Instant::now();
    host.advance(&mut |s| {
        if progress && last.elapsed() >= Duration::from_millis(250) {
            last = Instant::now();
            let g = &s.g;
            let active: Vec<_> = g.player_ids().filter(|&p| g.players[p].turn_active).collect();
            io.write(&crate::jobj! { "ev" => "progress", "turn" => g.game_turn(), "active" => active });
        }
    })
}

fn serve(host: &mut Host, io: &Io, opts: &AgentOptions) -> i32 {
    match host.start(&opts.start) {
        Ok(true) => io.write(&advance(host, io, opts.progress)),
        Ok(false) => io.write(&crate::jobj! { "ev" => "no_game" }),
        Err(e) => {
            io.write(&crate::jobj! { "ev" => "error", "error" => e.code, "detail" => e.detail.as_str() });
            return 2;
        }
    }
    loop {
        if io.closed() {
            return 0;
        }
        let line = match io.read_line() {
            None => break,
            Some(Ok(l)) => l,
            Some(Err(e)) => {
                io.write(&err_reply(Json::Null, &ProtoErr::invalid(e)));
                continue;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let (id, req) = Request::parse(&line);
        let (reply, after) = match req {
            Ok(r) => host.handle(id, r),
            Err(e) => (err_reply(id, &e), After::Nothing),
        };
        io.write(&reply);
        match after {
            After::Nothing => {}
            After::Advance => io.write(&advance(host, io, opts.progress)),
            After::Quit => return 0,
        }
    }
    // The end of input (a decision open then took the advisor's pick).
    if opts.on_eof == OnEof::EndTurn && host.session().is_some_and(|s| matches!(s.phase(), Phase::Human(_))) {
        let (reply, after) = host.handle(Json::Null, Request::Cmd(crate::jobj! { "c" => "end_turn", "force" => true }));
        io.write(&reply);
        if after == After::Advance {
            io.write(&advance(host, io, opts.progress));
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collects what the agent writes.
    #[derive(Clone, Default)]
    struct Out(Arc<Mutex<Vec<u8>>>);

    impl Write for Out {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn agent(input: &str, start: Start) -> (i32, Vec<Json>) {
        let out = Out::default();
        let opts = AgentOptions { host: HostOptions { saves: None, ..HostOptions::default() }, start, on_eof: OnEof::Quit, progress: false };
        let code = run(Box::new(std::io::Cursor::new(input.as_bytes().to_vec())), Box::new(out.clone()), &opts);
        let text = String::from_utf8(out.0.lock().unwrap().clone()).unwrap();
        (code, text.lines().map(|l| Json::parse(l).unwrap_or_else(|e| panic!("{e}: {l}"))).collect())
    }

    /// Without a game: the tables and the new-game lists answer, other requests `no_game`;
    /// malformed lines get `invalid` with the id when there is one.
    #[test]
    fn without_a_game() {
        let input = "{\"id\":1,\"k\":\"q\",\"q\":\"setup\"}\n\n{\"id\":2,\"k\":\"cmd\",\"c\":\"end_turn\"}\r\nnot json\n{\"id\":3,\"k\":\"q\",\"q\":\"view\"}\n\
                     {\"id\":4,\"k\":\"q\",\"q\":\"status\"}\n{\"id\":5,\"k\":\"session\",\"op\":\"save\",\"name\":\"x\"}\n{\"id\":6,\"k\":\"answer\",\"decision\":1,\"option\":0}\n";
        let (code, out) = agent(input, Start::None);
        assert_eq!(code, 0);
        let kinds: Vec<String> = out
            .iter()
            .map(|j| match j.get("ev") {
                Some(ev) => format!("ev:{}", ev.as_str().unwrap()),
                None => j.get("error").and_then(Json::as_str).map_or(format!("ok:{}", j.get("id").unwrap()), |e| format!("{e}:{}", j.get("id").unwrap())),
            })
            .collect();
        assert_eq!(kinds, ["ev:no_game", "ok:1", "no_game:2", "invalid:null", "no_game:3", "ok:4", "no_game:5", "gone:6"]);
        assert!(out[1].get("r").unwrap().req_arr("civilizations").is_ok());
        assert_eq!(out[5].get("r").unwrap().req_str("phase"), Ok("no_game"));
    }

    /// A start that fails reports an `error` event and exit code 2.
    #[test]
    fn a_bad_start_is_an_error() {
        let (code, out) = agent("", Start::New(Json::parse(r#"{"world":"TECH_X"}"#).unwrap()));
        assert_eq!(code, 2);
        assert_eq!(out[0].req_str("ev"), Ok("error"));
        assert!(out[0].req_str("detail").unwrap().contains("world"));
        let (code, out) = agent("", Start::Load("/no/such/save.ffhsave".into()));
        assert_eq!((code, out[0].req_str("error")), (2, Ok("gone")));
    }
}
