//! Transports (design §2.1, §4): the JSON envelope a client speaks, and the game-thread side
//! every transport shares.
//!
//! * [`stdio`]: `ffh-play agent`, JSONL over stdin/stdout on the game thread (no extra thread).
//! * `ffh-play serve` (HTTP) follows.
//!
//! The envelope (§4.1) is the same for both: a request is an object with `k` (`cmd`, `q`,
//! `answer`, `session`) and an optional `id` that the reply echoes; a reply is
//! `{"id", "ok": true, "r": ..}` (commands add `log` and `status`) or
//! `{"id", "ok": false, "error": code, "detail": text}`.
//!
//! [`Host`] is the game-thread state of a transport: the session (or none), its game directory,
//! and the session operations (new, load, save, take over, debug, quit, saves). A blocking
//! decision raised inside a rules call is answered by the transport's `Answerer`, which reads the
//! next requests itself ([`decision_reply`]): queries are answered from the decision's side,
//! commands and session operations get `decision_pending`, an `answer` closes it.

pub mod stdio;

use std::path::{Path, PathBuf};
use std::cell::RefCell;
use std::sync::{Arc, Once};

use ffh_core::ids::PlayerId;

use crate::command::{CmdOk, Command};
use crate::decision::Decision;
use crate::human::{Answer, Answerer, DecisionReader};
use crate::json::{FieldError, Json};
use crate::query::{Query, QueryErr};
use crate::save;
use crate::session::{Phase, PlayError, Session, SessionOptions, Stop};
use crate::setup::NewGame;

// ---------------------------------------------------------------------------------------------
// Requests and replies
// ---------------------------------------------------------------------------------------------

/// A protocol error: the `error` code and its `detail`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtoErr {
    pub code: &'static str,
    pub detail: String,
}

impl ProtoErr {
    pub fn new(code: &'static str, detail: impl Into<String>) -> ProtoErr {
        ProtoErr { code, detail: detail.into() }
    }

    pub fn invalid(detail: impl Into<String>) -> ProtoErr {
        ProtoErr::new("invalid", detail)
    }

    fn no_game() -> ProtoErr {
        ProtoErr::new("no_game", "no game is loaded: start one with {\"k\":\"session\",\"op\":\"new\"} or load a save")
    }
}

impl From<FieldError> for ProtoErr {
    fn from(e: FieldError) -> ProtoErr {
        ProtoErr::invalid(e.to_string())
    }
}

impl From<QueryErr> for ProtoErr {
    fn from(e: QueryErr) -> ProtoErr {
        ProtoErr::new(e.code(), e.detail())
    }
}

impl From<crate::command::CmdErr> for ProtoErr {
    fn from(e: crate::command::CmdErr) -> ProtoErr {
        ProtoErr::new(e.code(), e.detail())
    }
}

/// A session operation (`{"k": "session", "op": ..}`).
#[derive(Clone, Debug, PartialEq)]
pub enum SessionOp {
    /// A new game (`setup`: the new-game settings, every member optional; `name`: the game's
    /// directory under the saves root, default `game-<seed>`).
    New { setup: Json, name: Option<String> },
    /// Loads a save of the saves root by name (see [`Host::resolve_save`]).
    Load { name: String },
    /// Saves the game as `name` in its directory.
    Save { name: String },
    /// Takes over slot `player` of an AI-only save (a debugging aid).
    TakeOver { player: PlayerId, name: String },
    /// Debug reveal on or off.
    Debug { reveal: bool },
    Quit,
    /// Every game directory of the saves root with its saves.
    Saves,
}

/// A request (`k`).
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// A command: the request object (`c` and its fields).
    Cmd(Json),
    /// A query: the request object (`q` and its fields).
    Query(Json),
    /// The answer to blocking decision `decision`.
    Answer { decision: u32, answer: Answer },
    Session(SessionOp),
}

impl Request {
    /// Decodes a request object. Returns its `id` (echoed in the reply; `null` when absent)
    /// with the request or the reason it is malformed.
    pub fn from_json(j: &Json) -> (Json, Result<Request, ProtoErr>) {
        let id = j.get("id").cloned().unwrap_or(Json::Null);
        (id, Request::decode(j))
    }

    /// Parses one line (stdio) or body (HTTP).
    pub fn parse(s: &str) -> (Json, Result<Request, ProtoErr>) {
        match Json::parse(s) {
            Ok(j) => Request::from_json(&j),
            Err(e) => (Json::Null, Err(ProtoErr::invalid(format!("not JSON: {e}")))),
        }
    }

    fn decode(j: &Json) -> Result<Request, ProtoErr> {
        if j.as_obj().is_none() {
            return Err(ProtoErr::invalid(format!("a request is an object, got {}", j.kind())));
        }
        if let Some(id) = j.get("id") {
            if !matches!(id, Json::Null | Json::Int(_) | Json::Str(_)) {
                return Err(ProtoErr::invalid(format!("field `id`: expected an integer or a string, got {}", id.kind())));
            }
        }
        Ok(match j.req_str("k")? {
            "cmd" => Request::Cmd(j.clone()),
            "q" => Request::Query(j.clone()),
            "answer" => {
                let decision: u32 = j.req("decision")?;
                let advisor: bool = j.get_or("advisor", false)?;
                let answer = match (j.opt::<usize>("option")?, advisor) {
                    (Some(i), false) => Answer::Option(i),
                    (None, true) => Answer::Advisor,
                    _ => return Err(ProtoErr::invalid("an answer has either `option` (an index) or `\"advisor\": true`")),
                };
                Request::Answer { decision, answer }
            }
            "session" => Request::Session(match j.req_str("op")? {
                "new" => {
                    let setup = match j.opt_value("setup")? {
                        None | Some(Json::Null) => Json::obj(),
                        Some(s) if s.as_obj().is_some() => s.clone(),
                        Some(s) => return Err(FieldError::wrong("setup", "an object", s).into()),
                    };
                    SessionOp::New { setup, name: j.opt("name")? }
                }
                "load" => SessionOp::Load { name: j.req("name")? },
                "save" => SessionOp::Save { name: j.req("name")? },
                "take_over" => SessionOp::TakeOver { player: j.req("player")?, name: j.req("name")? },
                "debug" => SessionOp::Debug { reveal: j.req("reveal")? },
                "quit" => SessionOp::Quit,
                "saves" => SessionOp::Saves,
                other => return Err(ProtoErr::invalid(format!("unknown session op {other:?} (new, load, save, take_over, debug, quit, saves)"))),
            }),
            other => return Err(ProtoErr::invalid(format!("unknown request kind {other:?} (cmd, q, answer, session)"))),
        })
    }
}

/// `{"id", "ok": true, "r": r}`.
pub fn ok_reply(id: Json, r: Json) -> Json {
    crate::jobj! { "id" => id, "ok" => true, "r" => r }
}

/// `{"id", "ok": false, "error", "detail"}`.
pub fn err_reply(id: Json, e: &ProtoErr) -> Json {
    crate::jobj! { "id" => id, "ok" => false, "error" => e.code, "detail" => e.detail.as_str() }
}

fn reply_of(id: Json, r: Result<Json, ProtoErr>) -> Json {
    match r {
        Ok(r) => ok_reply(id, r),
        Err(e) => err_reply(id, &e),
    }
}

/// The reply to a request while blocking decision `d` is open, from inside the rules call
/// (design §2.4): queries are answered from the deciding player's side ([`DecisionReader`]),
/// `answer` with the decision's id closes it (an option must be enabled, or `advisor`), commands
/// and session operations get `decision_pending`. Returns the answer when the request closed
/// the decision.
pub fn decision_reply(r: &DecisionReader, d: &Decision, id: Json, req: Result<Request, ProtoErr>) -> (Json, Option<Answer>) {
    match req {
        Err(e) => (err_reply(id, &e), None),
        Ok(Request::Query(j)) => (reply_of(id, Query::from_json(&j).and_then(|q| r.query(&q)).map_err(ProtoErr::from)), None),
        Ok(Request::Answer { decision, answer }) if decision == d.id => {
            let option = match answer {
                Answer::Option(i) if d.is_valid(i) => i,
                Answer::Option(i) => {
                    let e = ProtoErr::invalid(format!("option {i} is not an enabled option of decision {} (0..{})", d.id, d.options.len()));
                    return (err_reply(id, &e), None);
                }
                Answer::Advisor => d.default_option(),
            };
            let key = d.options.get(option).map_or("", |o| o.key.as_str());
            (ok_reply(id, crate::jobj! { "answered" => d.id, "option" => option, "key" => key }), Some(answer))
        }
        Ok(Request::Answer { decision, .. }) => (err_reply(id, &ProtoErr::new("gone", format!("decision {decision} is not open (decision {} is)", d.id))), None),
        Ok(Request::Cmd(_) | Request::Session(_)) => {
            (err_reply(id, &ProtoErr::new("decision_pending", format!("decision {} ({}) must be answered first", d.id, d.kind.name()))), None)
        }
    }
}

/// The reply to a request while the game runs (the AI turns; the HTTP game thread answers the
/// queued requests between turn slices): queries are answered, commands get `not_your_turn`,
/// session operations `busy`.
pub fn running_reply(s: &Session, id: Json, req: Result<Request, ProtoErr>) -> Json {
    match req {
        Err(e) => err_reply(id, &e),
        Ok(Request::Query(j)) => reply_of(id, Query::from_json(&j).and_then(|q| s.query(&q)).map_err(ProtoErr::from)),
        Ok(Request::Cmd(_)) => err_reply(id, &ProtoErr::from(crate::command::CmdErr::NotYourTurn)),
        Ok(Request::Answer { decision, .. }) => err_reply(id, &ProtoErr::new("gone", format!("decision {decision} is not open"))),
        Ok(Request::Session(_)) => err_reply(id, &ProtoErr::new("busy", "the game is running: session operations wait for the next control point")),
    }
}

/// The `decision` event (stdio) / the decision of `status`.
pub fn decision_event(d: &Decision) -> Json {
    crate::jobj! { "ev" => "decision", "decision" => d.to_json() }
}

/// A log entry of a command reply.
fn log_json(e: &ffh_core::log::LogEntry) -> Json {
    crate::jobj! { "t" => e.turn, "text" => e.text.as_str() }
}

// ---------------------------------------------------------------------------------------------
// Panics
// ---------------------------------------------------------------------------------------------

/// What the last panic said (recorded by the hook [`install_panic_hook`] installs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PanicInfo {
    pub message: String,
    pub location: String,
}

thread_local! {
    /// Per thread: the game thread catches its own panics (other threads, such as the tests'
    /// running in parallel, must not overwrite its report).
    static LAST_PANIC: RefCell<Option<PanicInfo>> = const { RefCell::new(None) };
}

/// Records each panic's message and location (on the panicking thread) for the crash report,
/// then runs the previous hook (which prints it). Installed once per process.
pub fn install_panic_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic>".to_string()
            };
            let location = info.location().map_or("?".to_string(), |l| {
                let f = l.file();
                let f = f.rsplit_once("/crates/").map_or(f.to_string(), |(_, r)| r.to_string());
                format!("{f}:{}", l.line())
            });
            LAST_PANIC.with(|p| *p.borrow_mut() = Some(PanicInfo { message, location }));
            prev(info);
        }));
    });
}

/// The last panic recorded on this thread (taken).
pub fn take_panic() -> PanicInfo {
    LAST_PANIC.with(|p| p.borrow_mut().take()).unwrap_or_else(|| PanicInfo { message: "?".into(), location: "?".into() })
}

/// The crash report of a panic on the game thread (design §2.8): the message, the last
/// autosave and the journal of the session that crashed, and the replay command.
pub fn crash_report(p: &PanicInfo, s: Option<&Session>) -> Json {
    // The session may be half-way through a rules call: read it without trusting its locks.
    let (autosave, journal) = match s {
        Some(s) => {
            let autosave = s.last_autosave().map(|a| a.save.display().to_string());
            let journal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.journal_path())).ok().flatten().map(|p| p.display().to_string());
            (autosave, journal)
        }
        None => (None, None),
    };
    let repro = journal.as_ref().map(|j| match &autosave {
        Some(a) => format!("ffh-play replay {j} --from {a}"),
        None => format!("ffh-play replay {j}"),
    });
    crate::jobj! {
        "message" => p.message.as_str(),
        "location" => p.location.as_str(),
        "autosave" => autosave,
        "journal" => journal,
        "repro" => repro,
    }
}

// ---------------------------------------------------------------------------------------------
// The host: a transport's game-thread state
// ---------------------------------------------------------------------------------------------

/// Where a transport starts.
#[derive(Clone, Debug, PartialEq)]
pub enum Start {
    /// No game: the client starts one (`session new` / `load`).
    None,
    /// A new game with these settings (`NewGame` JSON; every member optional).
    New(Json),
    /// A save given on the command line (any path; its sidecar next to it).
    Load(PathBuf),
    /// An AI-only save given on the command line, slot `player` taken over.
    TakeOver(PathBuf, PlayerId),
}

/// The transport's settings for the sessions it creates.
#[derive(Clone, Debug)]
pub struct HostOptions {
    /// The saves root: each game gets a directory in it (autosaves, journal, manual saves).
    /// `None`: no files (the journal is kept in memory, nothing can be saved or loaded by name).
    pub saves: Option<PathBuf>,
    pub autosave_keep: usize,
    /// A self-test of the crash report: panic when the game reaches this turn (in `advance`).
    pub panic_at: Option<i32>,
}

impl Default for HostOptions {
    fn default() -> HostOptions {
        HostOptions { saves: Some(PathBuf::from("saves")), autosave_keep: SessionOptions::default().autosave_keep, panic_at: None }
    }
}

/// What the transport does after sending a reply.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum After {
    Nothing,
    /// The game runs (a new or loaded game, the human ended the turn): call [`Host::advance`].
    Advance,
    Quit,
}

/// The game-thread state of a transport: the session and its game directory.
pub struct Host {
    session: Option<Session>,
    opts: HostOptions,
    answerer: Arc<dyn Answerer>,
    /// The current game's directory (autosaves, journal, saves).
    game_dir: Option<PathBuf>,
    /// The watchdog's diagnosis when the last `advance` stopped a turn that did not finish.
    stuck: Option<String>,
}

impl Host {
    /// A host whose sessions answer blocking decisions with `answerer` (the transport's).
    pub fn new(opts: HostOptions, answerer: Arc<dyn Answerer>) -> Host {
        Host { session: None, opts, answerer, game_dir: None, stuck: None }
    }

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub fn game_dir(&self) -> Option<&Path> {
        self.game_dir.as_deref()
    }

    /// Drops the session (after a crash: it may be half-way through a rules call).
    pub fn drop_session(&mut self) -> Option<Session> {
        self.game_dir = None;
        self.stuck = None;
        self.session.take()
    }

    fn session_options(&self, dir: Option<PathBuf>) -> SessionOptions {
        SessionOptions { saves_dir: dir, autosave_keep: self.opts.autosave_keep, ..SessionOptions::default() }
    }

    fn install(&mut self, s: Session, dir: Option<PathBuf>) {
        self.session = Some(s);
        self.game_dir = dir;
        self.stuck = None;
    }

    /// Starts as `start` says. `Ok(true)` when a game was started (then call `advance`).
    pub fn start(&mut self, start: &Start) -> Result<bool, ProtoErr> {
        match start {
            Start::None => Ok(false),
            Start::New(setup) => self.new_game(setup, None).map(|_| true),
            Start::Load(path) => {
                let (bytes, sidecar) = save::read_bundle(path).map_err(|e| ProtoErr::new("gone", e))?;
                let dir = self.opts.saves.as_ref().map(|_| game_dir_of(path));
                let s = Session::load(&bytes, sidecar.as_ref(), self.answerer.clone(), self.session_options(dir.clone())).map_err(play_err)?;
                self.install(s, dir);
                Ok(true)
            }
            Start::TakeOver(path, p) => {
                let (bytes, _) = save::read_bundle(path).map_err(|e| ProtoErr::new("gone", e))?;
                let dir = self.opts.saves.as_ref().map(|_| game_dir_of(path));
                let s = Session::take_over(&bytes, *p, self.answerer.clone(), self.session_options(dir.clone())).map_err(play_err)?;
                self.install(s, dir);
                Ok(true)
            }
        }
    }

    /// Handles a request at a control point (or with no game, or a game over or stuck): the
    /// reply, and what to do next.
    pub fn handle(&mut self, id: Json, req: Request) -> (Json, After) {
        match req {
            Request::Cmd(j) => self.command(id, &j),
            Request::Query(j) => (reply_of(id, self.query(&j)), After::Nothing),
            Request::Answer { decision, .. } => (err_reply(id, &ProtoErr::new("gone", format!("decision {decision} is not open"))), After::Nothing),
            Request::Session(op) => match self.session_op(op) {
                Ok((r, after)) => (ok_reply(id, r).with("status", self.status_after(after)), after),
                Err(e) => (err_reply(id, &e), After::Nothing),
            },
        }
    }

    fn query(&self, j: &Json) -> Result<Json, ProtoErr> {
        let q = Query::from_json(j)?;
        match (&self.session, &q) {
            (Some(s), _) => Ok(s.query(&q)?),
            // The static tables and the new-game lists need no game.
            (None, Query::Data) => Ok(crate::view::data::data(ffh_data::global()).clone()),
            (None, Query::Setup) => Ok(crate::setup::setup_lists(ffh_data::global())),
            (None, Query::Status) => Ok(self.status()),
            (None, _) => Err(ProtoErr::no_game()),
        }
    }

    fn command(&mut self, id: Json, j: &Json) -> (Json, After) {
        let Some(s) = &mut self.session else { return (err_reply(id, &ProtoErr::no_game()), After::Nothing) };
        let cmd = match Command::from_json(s.g.d, j) {
            Ok(c) => c,
            Err(e) => return (err_reply(id, &e.into()), After::Nothing),
        };
        match s.execute(&cmd) {
            Ok(CmdOk::Applied { log, accepted }) => {
                // The human may have handed control back (end turn, auto moves) or ended the game.
                let after = if matches!(s.phase(), Phase::Human(_)) { After::Nothing } else { After::Advance };
                let r = crate::jobj! { "applied" => true, "accepted" => accepted };
                let reply = ok_reply(id, r).with("log", Json::Arr(log.iter().map(log_json).collect())).with("status", self.status_after(after));
                (reply, after)
            }
            Ok(CmdOk::NeedsConfirm(c)) => (ok_reply(id, crate::jobj! { "confirm" => c.to_json() }), After::Nothing),
            Err(e) => (err_reply(id, &e.into()), After::Nothing),
        }
    }

    /// Runs the game to the human's next control point or the end (`Session::advance`) and
    /// returns the event to report: `control`, `over` or `stuck`.
    pub fn advance(&mut self, on_slice: &mut dyn FnMut(&Session)) -> Json {
        let Some(s) = &mut self.session else { return crate::jobj! { "ev" => "no_game" } };
        // Fires once: taken here, put back unless it fired (the panic unwinds past the put-back).
        let panic_at = self.opts.panic_at.take();
        let stop = s.advance(&mut |s| {
            if let Some(t) = panic_at.filter(|&t| s.g.game_turn() >= t) {
                panic!("--panic-at {t}: a deliberate panic at turn {} (a self-test of the crash report)", s.g.game_turn());
            }
            on_slice(s)
        });
        self.opts.panic_at = panic_at;
        let s = &*s;
        let (ev, stuck) = match stop {
            Stop::Human => {
                let ev = crate::jobj! {
                    "ev" => "control",
                    "turn" => s.g.game_turn(),
                    "year" => s.g.game_turn_year(),
                    "human" => s.human,
                    "pending" => s.pending().to_json(),
                    "log_next" => s.message_store().total(),
                };
                (ev, None)
            }
            Stop::Over => {
                let status = s.query(&Query::Status).unwrap_or_default();
                let ev = crate::jobj! {
                    "ev" => "over",
                    "turn" => s.g.game_turn(),
                    "human" => s.human,
                    "alive" => s.g.players[s.human].alive,
                    "over" => status.get("over").cloned().unwrap_or(Json::Null),
                };
                (ev, None)
            }
            Stop::Stuck(diag) => (crate::jobj! { "ev" => "stuck", "turn" => s.g.game_turn(), "diagnosis" => diag.as_str() }, Some(diag)),
        };
        self.stuck = stuck;
        ev
    }

    /// The status (design §4.3) without the transport's members: the game's (`status` query)
    /// plus the game directory, journal and last autosave; `phase` is `no_game` without a game
    /// and `stuck` when the watchdog stopped the last turn.
    pub fn status(&self) -> Json {
        let Some(s) = &self.session else { return crate::jobj! { "phase" => "no_game" } };
        let mut j = s.query(&Query::Status).unwrap_or_else(|e| crate::jobj! { "phase" => "running", "error" => e.to_string() });
        if let Some(diag) = &self.stuck {
            j.set("phase", "stuck");
            j.set("stuck", diag.as_str());
        }
        j.set("game", self.game_info());
        j
    }

    /// The status a reply carries: with `After::Advance` the game runs next (`advance`), so the
    /// phase is `running` even where the predicate already gives the human control (a new game,
    /// a save loaded at a control point): a command sent at once waits for the control point.
    pub fn status_after(&self, after: After) -> Json {
        let st = self.status();
        if after == After::Advance && self.session.is_some() {
            st.with("phase", "running")
        } else {
            st
        }
    }

    /// `{dir, journal, autosave}` of the current game.
    pub fn game_info(&self) -> Json {
        let Some(s) = &self.session else { return Json::Null };
        crate::jobj! {
            "dir" => self.game_dir.as_ref().map(|d| d.display().to_string()),
            "journal" => s.journal_path().map(|p| p.display().to_string()),
            "autosave" => s.last_autosave().map(|a| a.save.display().to_string()),
        }
    }

    fn root(&self) -> Result<&Path, ProtoErr> {
        self.opts.saves.as_deref().ok_or_else(|| ProtoErr::new("cannot", "no saves directory (the transport runs with --no-saves)"))
    }

    fn session_op(&mut self, op: SessionOp) -> Result<(Json, After), ProtoErr> {
        match op {
            SessionOp::New { setup, name } => {
                let dir = self.new_game(&setup, name.as_deref())?;
                Ok((crate::jobj! { "game" => dir.map(|d| d.display().to_string()) }, After::Advance))
            }
            SessionOp::Load { name } => {
                let (path, dir) = self.resolve_save(&name)?;
                let (bytes, sidecar) = read_save(&path)?;
                let s = Session::load(&bytes, sidecar.as_ref(), self.answerer.clone(), self.session_options(Some(dir.clone()))).map_err(play_err)?;
                self.session = None;
                self.install(s, Some(dir));
                Ok((crate::jobj! { "loaded" => path.display().to_string() }, After::Advance))
            }
            SessionOp::TakeOver { player, name } => {
                let (path, dir) = self.resolve_save(&name)?;
                let (bytes, _) = read_save(&path)?;
                let s = Session::take_over(&bytes, player, self.answerer.clone(), self.session_options(Some(dir.clone()))).map_err(play_err)?;
                self.session = None;
                self.install(s, Some(dir));
                Ok((crate::jobj! { "taken_over" => path.display().to_string(), "player" => player }, After::Advance))
            }
            SessionOp::Save { name } => {
                let s = self.session.as_ref().ok_or_else(ProtoErr::no_game)?;
                save::sanitize_name(&name).map_err(ProtoErr::invalid)?;
                self.root()?;
                let paths = s.save(&name).map_err(play_err)?;
                Ok((crate::jobj! { "save" => paths.save.display().to_string(), "sidecar" => paths.sidecar.display().to_string() }, After::Nothing))
            }
            SessionOp::Debug { reveal } => {
                let s = self.session.as_ref().ok_or_else(ProtoErr::no_game)?;
                s.set_reveal(reveal);
                Ok((crate::jobj! { "reveal" => reveal }, After::Nothing))
            }
            SessionOp::Quit => Ok((crate::jobj! { "quit" => true }, After::Quit)),
            SessionOp::Saves => Ok((self.saves_json()?, After::Nothing)),
        }
    }

    /// A new game in a new directory `<root>/<name>` (`game-<seed>` by default; `-2`, `-3`, ..
    /// appended when taken). The settings are checked before the directory is made.
    fn new_game(&mut self, setup: &Json, name: Option<&str>) -> Result<Option<PathBuf>, ProtoErr> {
        let d = ffh_data::global();
        let ng = NewGame::from_json(d, setup)?;
        ng.validate(d).map_err(ProtoErr::invalid)?;
        let base = match name {
            Some(n) => save::sanitize_name(n).map_err(ProtoErr::invalid)?.to_string(),
            None => format!("game-{}", ng.seed),
        };
        let dir = match &self.opts.saves {
            None => None,
            Some(root) => Some(unused_dir(root, &base).map_err(|e| ProtoErr::new("cannot", format!("{}: {e}", root.display())))?),
        };
        // The old session goes first (its journal is closed).
        self.session = None;
        let s = Session::new_game(&ng, self.answerer.clone(), self.session_options(dir.clone())).map_err(play_err)?;
        self.install(s, dir.clone());
        Ok(dir)
    }

    /// A save named by a client: `NAME` or `auto/NAME` in the current game's directory,
    /// `GAME/NAME` or `GAME/auto/NAME` in another game's directory of the saves root (a trailing
    /// `.ffhsave` is allowed). Every part is a sanitized name, so no name leaves the root.
    /// Returns the save's path and its game directory.
    pub fn resolve_save(&self, name: &str) -> Result<(PathBuf, PathBuf), ProtoErr> {
        let root = self.root()?;
        let name = name.strip_suffix(".ffhsave").unwrap_or(name);
        let parts: Vec<&str> = name.split('/').collect();
        for p in &parts {
            save::sanitize_name(p).map_err(|e| ProtoErr::invalid(format!("save name {name:?}: {e}")))?;
        }
        let current = || self.game_dir.clone().ok_or_else(|| ProtoErr::invalid(format!("save name {name:?}: no current game; use GAME/NAME")));
        let (dir, sub, file) = match parts.as_slice() {
            [file] => (current()?, None, *file),
            ["auto", file] => (current()?, Some("auto"), *file),
            [game, file] => (root.join(game), None, *file),
            [game, "auto", file] => (root.join(game), Some("auto"), *file),
            _ => return Err(ProtoErr::invalid(format!("save name {name:?}: use NAME, auto/NAME, GAME/NAME or GAME/auto/NAME"))),
        };
        let path = match sub {
            Some(s) => dir.join(s).join(format!("{file}.ffhsave")),
            None => dir.join(format!("{file}.ffhsave")),
        };
        Ok((path, dir))
    }

    /// The `saves` session operation: every game directory of the root with its saves.
    fn saves_json(&self) -> Result<Json, ProtoErr> {
        let root = self.root()?;
        let mut games = Vec::new();
        if let Ok(entries) = std::fs::read_dir(root) {
            let mut dirs: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect();
            dirs.sort();
            for dir in dirs {
                let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                if save::sanitize_name(&name).is_err() {
                    continue;
                }
                let list = save::list_json(Some(&dir));
                let saves = list.get("saves").cloned().unwrap_or_else(Json::arr);
                games.push(crate::jobj! { "game" => name, "dir" => dir.display().to_string(), "saves" => saves });
            }
        }
        Ok(crate::jobj! {
            "root" => root.display().to_string(),
            "current" => self.game_dir.as_ref().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().into_owned()),
            "games" => Json::Arr(games),
        })
    }
}

fn play_err(e: PlayError) -> ProtoErr {
    match e {
        PlayError::Setup(s) => ProtoErr::invalid(s),
        PlayError::Load(s) | PlayError::Io(s) | PlayError::Refused(s) => ProtoErr::new("cannot", s),
    }
}

fn read_save(path: &Path) -> Result<(Vec<u8>, Option<save::Sidecar>), ProtoErr> {
    if !path.is_file() {
        return Err(ProtoErr::new("gone", format!("no save {}", path.display())));
    }
    save::read_bundle(path).map_err(|e| ProtoErr::new("cannot", e))
}

/// The game directory of a save: its directory, or the one above for an autosave in `auto/`.
pub fn game_dir_of(save: &Path) -> PathBuf {
    let dir = save.parent().map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let dir = if dir.as_os_str().is_empty() { PathBuf::from(".") } else { dir };
    if dir.file_name().is_some_and(|n| n == "auto") {
        dir.parent().map_or(dir.clone(), Path::to_path_buf)
    } else {
        dir
    }
}

/// `<root>/<base>`, or `<root>/<base>-2`, `-3`, ... when it exists; created.
fn unused_dir(root: &Path, base: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(root)?;
    let mut n = 1;
    loop {
        let dir = if n == 1 { root.join(base) } else { root.join(format!("{base}-{n}")) };
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::human::AdvisorAnswerer;

    #[test]
    fn requests_decode_with_their_errors() {
        let (id, r) = Request::parse(r#"{"id":12,"k":"cmd","c":"end_turn","force":true}"#);
        assert_eq!(id, Json::Int(12));
        assert!(matches!(r, Ok(Request::Cmd(_))));
        let (id, r) = Request::parse(r#"{"id":"a","k":"answer","decision":3,"option":1}"#);
        assert_eq!(id, Json::Str("a".into()));
        assert_eq!(r, Ok(Request::Answer { decision: 3, answer: Answer::Option(1) }));
        assert_eq!(Request::parse(r#"{"k":"answer","decision":3,"advisor":true}"#).1, Ok(Request::Answer { decision: 3, answer: Answer::Advisor }));
        assert_eq!(Request::parse(r#"{"k":"session","op":"new"}"#).1, Ok(Request::Session(SessionOp::New { setup: Json::obj(), name: None })));
        assert_eq!(
            Request::parse(r#"{"k":"session","op":"take_over","player":2,"name":"g/auto/T0010"}"#).1,
            Ok(Request::Session(SessionOp::TakeOver { player: PlayerId(2), name: "g/auto/T0010".into() }))
        );
        for (bad, why) in [
            ("[1]", "an object"),
            ("{\"k\":\"fly\"}", "unknown request kind"),
            ("{\"id\":[1],\"k\":\"q\",\"q\":\"status\"}", "field `id`"),
            ("{\"k\":\"answer\",\"decision\":1}", "either `option`"),
            ("{\"k\":\"answer\",\"decision\":1,\"option\":0,\"advisor\":true}", "either `option`"),
            ("{\"k\":\"answer\",\"decision\":-1,\"option\":0}", "field `decision`"),
            ("{\"k\":\"session\",\"op\":\"new\",\"setup\":3}", "field `setup`"),
            ("{\"k\":\"session\",\"op\":\"dance\"}", "unknown session op"),
            ("{\"k\":\"session\",\"op\":\"take_over\",\"player\":300,\"name\":\"x\"}", "field `player`"),
            ("{\"k\":1}", "field `k`"),
            ("{\"id\":1,\"k\":\"q\",\"q\":\"status\"", "not JSON"),
        ] {
            let (_, r) = Request::parse(bad);
            let e = r.expect_err(bad);
            assert_eq!(e.code, "invalid", "{bad}");
            assert!(e.detail.contains(why), "{bad}: {}", e.detail);
        }
        let e = err_reply(Json::Int(4), &ProtoErr::new("busy", "later"));
        assert_eq!(e.to_string(), r#"{"id":4,"ok":false,"error":"busy","detail":"later"}"#);
    }

    /// While the game runs (HTTP, between turn slices): queries are answered, commands get
    /// `not_your_turn`, answers `gone`, session operations `busy`.
    #[test]
    fn requests_while_the_game_runs() {
        let d = ffh_data::global();
        let mut ng = NewGame::new(d, 3);
        ng.opponents = 1;
        ng.world = d.t("WORLDSIZE_DUEL");
        let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), SessionOptions::default()).unwrap();
        assert_eq!(s.advance(&mut |_| {}), Stop::Human);
        s.execute(&Command::EndTurn { force: true }).unwrap();
        let mut seen = Vec::new();
        s.advance(&mut |s| {
            if seen.is_empty() {
                for line in [
                    r#"{"id":1,"k":"q","q":"status"}"#,
                    r#"{"id":2,"k":"cmd","c":"end_turn"}"#,
                    r#"{"id":3,"k":"answer","decision":1,"option":0}"#,
                    r#"{"id":4,"k":"session","op":"quit"}"#,
                    r#"{"id":5,"k":"q","q":"nothing"}"#,
                ] {
                    let (id, req) = Request::parse(line);
                    seen.push(running_reply(s, id, req));
                }
            }
        });
        let codes: Vec<&str> = seen.iter().map(|r| r.get("error").and_then(Json::as_str).unwrap_or("ok")).collect();
        assert_eq!(codes, ["ok", "not_your_turn", "gone", "busy", "invalid"]);
        assert!(seen[0].get("r").unwrap().get("turn").is_some());
    }

    #[test]
    fn save_names_stay_in_the_saves_root() {
        let root = std::env::temp_dir().join(format!("ffh-play-host-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut host = Host::new(HostOptions { saves: Some(root.clone()), autosave_keep: 2, panic_at: None }, Arc::new(AdvisorAnswerer));
        // No current game: only GAME/NAME forms.
        assert_eq!(host.resolve_save("g1/x").unwrap(), (root.join("g1/x.ffhsave"), root.join("g1")));
        assert_eq!(host.resolve_save("g1/auto/T0010.ffhsave").unwrap(), (root.join("g1/auto/T0010.ffhsave"), root.join("g1")));
        assert!(host.resolve_save("x").unwrap_err().detail.contains("no current game"));
        host.game_dir = Some(root.join("cur"));
        assert_eq!(host.resolve_save("x").unwrap().0, root.join("cur/x.ffhsave"));
        assert_eq!(host.resolve_save("auto/T0003").unwrap().0, root.join("cur/auto/T0003.ffhsave"));
        for bad in ["../x", "g/../../x", "/etc/passwd", "a/b/c", "g/x/y", ".hidden", "", "a//b", "a\\b", "g/auto/../x", "~/x"] {
            assert_eq!(host.resolve_save(bad).unwrap_err().code, "invalid", "{bad}");
        }
        // Game directories.
        assert_eq!(unused_dir(&root, "game-3").unwrap(), root.join("game-3"));
        assert_eq!(unused_dir(&root, "game-3").unwrap(), root.join("game-3-2"));
        assert_eq!(game_dir_of(&root.join("g/auto/T0001.ffhsave")), root.join("g"));
        assert_eq!(game_dir_of(&root.join("g/x.ffhsave")), root.join("g"));
        assert_eq!(game_dir_of(Path::new("x.ffhsave")), PathBuf::from("."));
        // No saves root: nothing by name.
        host.opts.saves = None;
        assert_eq!(host.resolve_save("g1/x").unwrap_err().code, "cannot");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
