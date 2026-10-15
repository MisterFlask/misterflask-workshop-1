//! `ffh-play serve` (design §2.1, §2.8, §4.2): the game thread, which owns the session and answers
//! `POST /api` envelopes, the acceptor, and a thread per connection ([`super::http`]).
//!
//! The game thread takes one envelope at a time from a channel shared by every connection. At a
//! control point it handles it ([`Host::handle`]), refreshes the cached view and map and the
//! status ([`Shared`]), then replies; when the game runs (the AI turns after the human's end of
//! turn) it publishes the status at most every 20 ms and answers the envelopes queued meanwhile
//! between turn slices (queries are answered, commands get `not_your_turn`). A blocking
//! decision is answered by [`ChannelAnswerer`], which reads the same channel from inside the
//! rules call: the connection that sent the command waits for its reply while another one
//! answers the decision (design §2.4). The channel's receiver is locked only around a receive,
//! never across a rules call.
//!
//! A panic on the game thread (a rules bug) is caught: the status becomes `crashed` with the
//! message, the last autosave, the journal and the replay command, the session is dropped, and
//! the server stays up so that the tester can load the autosave.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ffh_core::game::Game;

use super::http::{self, HttpCtx};
use super::shared::Shared;
use super::{decision_reply, err_reply, install_panic_hook, running_reply, running_status, take_panic, After, Host, HostOptions, Request, Start};
use crate::decision::Decision;
use crate::human::{Answer, Answerer, DecisionReader};
use crate::json::Json;
use crate::query::Query;

/// A request for the game thread and where its reply goes.
pub struct Envelope {
    pub req: Json,
    pub reply: Sender<Json>,
}

/// Status updates while the game runs, at most this often.
pub const PUBLISH_EVERY: Duration = Duration::from_millis(20);

/// Connections served at once (each has a thread; long-polls hold theirs up to a minute); more
/// get 503.
pub const MAX_CONNECTIONS: usize = 64;

/// On quitting, how long the acceptor waits for the connections still answering.
const QUIT_DRAIN: Duration = Duration::from_secs(2);

/// `ffh-play serve`'s settings.
#[derive(Clone, Debug)]
pub struct ServeOptions {
    /// Where to listen (default 127.0.0.1:8642; port 0 picks a free port).
    pub bind: SocketAddr,
    pub host: HostOptions,
    pub start: Start,
    /// `--assets DIR`: the client files from disk (live editing).
    pub assets: Option<PathBuf>,
}

impl Default for ServeOptions {
    fn default() -> ServeOptions {
        ServeOptions { bind: SocketAddr::from(([127, 0, 0, 1], 8642)), host: HostOptions::default(), start: Start::None, assets: None }
    }
}

/// A running server.
pub struct Server {
    addr: SocketAddr,
    shared: Arc<Shared>,
    acceptor: Option<JoinHandle<()>>,
    game: Option<JoinHandle<()>>,
}

/// The next envelope, or `None` when the server quits or every sender is gone.
fn recv(rx: &Mutex<Receiver<Envelope>>, shared: &Shared) -> Option<Envelope> {
    loop {
        if shared.quitting() {
            return None;
        }
        let r = rx.lock().unwrap_or_else(|e| e.into_inner()).recv_timeout(Duration::from_millis(200));
        match r {
            Ok(e) => return Some(e),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

/// Answers blocking decisions over HTTP (design §2.4): publishes the decision in the status (with
/// the view and map from the deciding player's side), then answers envelopes from inside the
/// rules call until an `answer` closes it.
pub struct ChannelAnswerer {
    rx: Arc<Mutex<Receiver<Envelope>>>,
    shared: Arc<Shared>,
}

impl Answerer for ChannelAnswerer {
    fn ask(&self, _g: &Game, _d: &Decision) -> Answer {
        Answer::Advisor
    }

    fn ask_with(&self, r: &DecisionReader, d: &Decision) -> Answer {
        // `DecisionReader::query` catches a panic (a failed view keeps the previous one).
        self.shared.set_caches(r.query(&Query::View).ok(), r.query(&Query::Map { since: None, epoch: None }).ok());
        let status = r.query(&Query::Status).unwrap_or_else(|_| crate::jobj! { "phase" => "decision", "decision" => d.to_json() });
        self.shared.publish(status.clone());
        loop {
            // The server quits: the advisor's pick lets the rules call finish.
            let Some(env) = recv(&self.rx, &self.shared) else { return Answer::Advisor };
            let (id, req) = Request::from_json(&env.req);
            let (reply, answer) = decision_reply(r, d, id, req);
            let _ = env.reply.send(reply);
            if let Some(a) = answer {
                // The rules call goes on; the game thread publishes again when it is done.
                self.shared.publish(status.with("phase", "running").with("decision", Json::Null));
                return a;
            }
        }
    }
}

/// The game thread's state.
struct GameThread {
    host: Host,
    rx: Arc<Mutex<Receiver<Envelope>>>,
    shared: Arc<Shared>,
}

impl GameThread {
    /// Runs `f` with panics caught: a panic drops the session and publishes the crash report.
    fn guarded(&mut self, f: impl FnOnce(&mut GameThread) -> bool) -> bool {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self))) {
            Ok(quit) => quit,
            Err(_) => {
                // The session may be half-way through a rules call: the host drops it.
                let report = self.host.crashed(&take_panic());
                eprintln!("ffh-play: the game crashed: {report}");
                self.shared.set_crash(Some(report));
                self.shared.clear_caches();
                self.shared.publish(self.host.status());
                false
            }
        }
    }

    /// Caches the view and map and publishes the status (before a reply, so that the client
    /// reads them at once). `after`: what follows; with `After::Advance` the status says
    /// `running` until `advance` reaches the control point.
    fn refresh(&mut self, after: After) {
        match self.host.session() {
            Some(s) => {
                let view = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.query(&Query::View))).ok().and_then(Result::ok);
                let map = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.query(&Query::Map { since: None, epoch: None }))).ok().and_then(Result::ok);
                self.shared.set_caches(view, map);
                self.shared.set_game(self.host.game_info());
                self.shared.set_crash(None);
                self.shared.publish(self.host.status_after(after));
            }
            None => {
                self.shared.clear_caches();
                self.shared.set_crash(self.host.crash().cloned());
                self.shared.publish(self.host.status());
            }
        }
    }

    /// Runs the game to the human's next control point, serving the queued envelopes between
    /// turn slices.
    fn advance(&mut self) {
        let shared = self.shared.clone();
        let rx = self.rx.clone();
        let dir = self.host.game_dir().map(Path::to_path_buf);
        let mut last: Option<Instant> = None;
        self.host.advance(&mut |s| {
            if last.is_none_or(|t| t.elapsed() >= PUBLISH_EVERY) {
                last = Some(Instant::now());
                // Until `advance` returns the human has no control, whatever the predicate says
                // in this slice (the AutoMoves rule may still run the human's missions).
                if let Ok(st) = running_status(s, dir.as_deref()) {
                    shared.set_game(st.get("game").cloned().unwrap_or_default());
                    shared.publish(st);
                }
            }
            loop {
                let next = rx.lock().unwrap_or_else(|e| e.into_inner()).try_recv();
                let Ok(env) = next else { break };
                let (id, req) = Request::from_json(&env.req);
                let _ = env.reply.send(running_reply(s, dir.as_deref(), id, req));
            }
        });
        self.refresh(After::Nothing);
    }

    /// Handles one envelope; `true` to quit.
    fn handle(&mut self, env: Envelope) -> bool {
        let (id, req) = Request::from_json(&env.req);
        let query = matches!(req, Ok(Request::Query(_)));
        let (reply, after) = match req {
            Ok(r) => self.host.handle(id, r),
            Err(e) => (err_reply(id, &e), After::Nothing),
        };
        if !query {
            self.refresh(after);
        }
        let _ = env.reply.send(reply);
        match after {
            After::Nothing => false,
            After::Advance => {
                self.advance();
                false
            }
            After::Quit => true,
        }
    }
}

/// The game thread: start, then envelopes until `quit`. The start's result goes to `started`.
fn game_thread(host: HostOptions, start: Start, rx: Arc<Mutex<Receiver<Envelope>>>, shared: Arc<Shared>, started: Sender<Result<(), String>>) {
    install_panic_hook();
    let answerer = Arc::new(ChannelAnswerer { rx: rx.clone(), shared: shared.clone() });
    let mut gt = GameThread { host: Host::new(host, answerer), rx, shared };
    let mut ok = false;
    gt.guarded(|gt| {
        match gt.host.start(&start) {
            Ok(go) => {
                ok = true;
                let _ = started.send(Ok(()));
                gt.refresh(if go { After::Advance } else { After::Nothing });
                if go {
                    gt.advance();
                }
            }
            Err(e) => {
                let _ = started.send(Err(format!("{}: {}", e.code, e.detail)));
            }
        }
        false
    });
    // A start that failed (or panicked before it was reported) ends the thread.
    drop(started);
    if !ok {
        return;
    }
    loop {
        let Some(env) = recv(&gt.rx, &gt.shared) else { break };
        if gt.guarded(|gt| gt.handle(env)) {
            break;
        }
    }
    gt.shared.quit();
}

impl Server {
    /// Binds, starts the game thread (which starts as `opts.start` says) and the acceptor. Fails
    /// when the address cannot be bound or the start fails (a bad setup or save).
    pub fn start(opts: ServeOptions) -> Result<Server, String> {
        let listener = TcpListener::bind(opts.bind).map_err(|e| format!("{}: {e}", opts.bind))?;
        let addr = listener.local_addr().map_err(|e| e.to_string())?;
        let shared = Arc::new(Shared::new());
        let (tx, rx) = mpsc::channel::<Envelope>();
        let rx = Arc::new(Mutex::new(rx));
        let (started_tx, started_rx) = mpsc::channel();
        let game = {
            let (shared, host, start) = (shared.clone(), opts.host.clone(), opts.start.clone());
            // Rules code recurses (paths, AI): the main thread's stack size, not the 2 MB default.
            std::thread::Builder::new()
                .name("ffh-game".into())
                .stack_size(16 << 20)
                .spawn(move || game_thread(host, start, rx, shared, started_tx))
                .map_err(|e| e.to_string())?
        };
        match started_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                let _ = game.join();
                return Err(e);
            }
            Err(_) => {
                let _ = game.join();
                return Err("the game thread stopped while starting (see the panic message)".into());
            }
        }
        let loopback = addr.ip().is_loopback().then(|| addr.ip());
        let ctx = Arc::new(HttpCtx { shared: shared.clone(), tx, assets: opts.assets.clone(), loopback });
        let acceptor = std::thread::Builder::new()
            .name("ffh-http".into())
            .spawn(move || {
                let open = Arc::new(AtomicUsize::new(0));
                let refusing = Arc::new(AtomicUsize::new(0));
                for stream in listener.incoming() {
                    if ctx.shared.quitting() {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    if open.load(Ordering::Relaxed) >= MAX_CONNECTIONS {
                        // 503, then drained on a short-lived thread (so the acceptor never
                        // waits): closed with its request unread, the connection would be reset
                        // before the client read the answer. Past as many refusals at once, the
                        // connection is just closed.
                        if refusing.load(Ordering::Relaxed) < MAX_CONNECTIONS {
                            refusing.fetch_add(1, Ordering::Relaxed);
                            let mine = refusing.clone();
                            let spawned = std::thread::Builder::new().name("ffh-busy".into()).spawn(move || {
                                http::refuse(stream);
                                mine.fetch_sub(1, Ordering::Relaxed);
                            });
                            if spawned.is_err() {
                                refusing.fetch_sub(1, Ordering::Relaxed);
                            }
                        }
                        continue;
                    }
                    open.fetch_add(1, Ordering::Relaxed);
                    let (ctx, mine) = (ctx.clone(), open.clone());
                    let spawned = std::thread::Builder::new().name("ffh-conn".into()).spawn(move || {
                        http::handle_connection(stream, &ctx);
                        mine.fetch_sub(1, Ordering::Relaxed);
                    });
                    if spawned.is_err() {
                        open.fetch_sub(1, Ordering::Relaxed);
                    }
                }
                // Quitting: the answers in flight (the `quit` request's own among them) are
                // written before the acceptor ends, and with it `Server::wait` and the process
                // (P6: the quit's answer was sometimes lost). Bounded: a connection whose request
                // never comes is not waited for.
                let t0 = Instant::now();
                while open.load(Ordering::Relaxed) > 0 && t0.elapsed() < QUIT_DRAIN {
                    std::thread::sleep(Duration::from_millis(5));
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Server { addr, shared, acceptor: Some(acceptor), game: Some(game) })
    }

    /// The bound address (with the port picked for port 0).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    /// Blocks until the server quits (a `quit` request), then stops the acceptor.
    pub fn wait(mut self) {
        if let Some(g) = self.game.take() {
            let _ = g.join();
        }
        self.stop_acceptor();
    }

    /// Stops the server: the game thread finishes what it is doing (a decision takes the
    /// advisor's pick) and stops; so does the acceptor.
    pub fn shutdown(mut self) {
        self.shared.quit();
        if let Some(g) = self.game.take() {
            let _ = g.join();
        }
        self.stop_acceptor();
    }

    fn stop_acceptor(&mut self) {
        self.shared.quit();
        // Wake the acceptor's `accept`.
        let mut wake = self.addr;
        if wake.ip().is_unspecified() {
            wake.set_ip(if wake.is_ipv4() { [127, 0, 0, 1].into() } else { std::net::Ipv6Addr::LOCALHOST.into() });
        }
        let _ = TcpStream::connect_timeout(&wake, Duration::from_secs(2));
        if let Some(a) = self.acceptor.take() {
            let _ = a.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if self.game.is_some() || self.acceptor.is_some() {
            self.shared.quit();
            if let Some(g) = self.game.take() {
                let _ = g.join();
            }
            self.stop_acceptor();
        }
    }
}
