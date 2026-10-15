//! T13 (design §8, §2.7): the session's performance, `ffh-play bench`, and the `#[ignore]`d
//! `tests/bench.rs`.
//!
//! * **AI time per turn** (design §2.7's bench guard): the AI turns of a game the test bot plays
//!   (`advance` after its end of turn) against `ffh bench` on the same seed, size and number of
//!   civilizations (budget +10%; skipped without the `ffh` binary). These are two different
//!   games (`ffh bench` has an AI in slot 0, the bot plays weakly), so this guard is loose; the
//!   next measurement isolates the session's own cost.
//! * **Session overhead on one trajectory.** From a save at the human's first control point the
//!   AI plays every slot for `turns` turns (`autoplay`: the human slot through its
//!   `HumanController` and the advisor) three ways: as a bare `Game::update()` loop with the
//!   same controllers; through `Session::advance` with the journal in memory and no autosave;
//!   and with the journal and the autosaves in a directory. All end with the same checksum.
//!   The budget (+10%) is on the session's own work measured inside its runs
//!   ([`AdvanceProfile`]: the phase predicate, the slot-move check, the log drain and its fog,
//!   the turn-boundary checkpoint and its checksum, the per-slice callback) against the
//!   `update()` slices of the same runs: both are read around each slice, so host load reaches
//!   both alike (P6 review). The three ways are also timed whole, interleaved, `overhead_runs`
//!   rounds, the best of each reported with the spread: separate runs of the same trajectory
//!   differ by up to a third on a loaded host, even in the thread's CPU time
//!   (`/proc/thread-self/schedstat`, which leaves out the time other processes run but not their
//!   effect on caches, memory bandwidth and clock speed), so their ratio is reported, not
//!   budgeted. The autosave (`Game::save` and the file write) is reported, not budgeted.
//! * **Command latency**: `execute` (with its settle) plus the `view` query, for commands at a
//!   control point of a standard map at turn 200 (reached by autoplay; budget 50 ms).
//! * **View build** on a huge map (budget 10 ms), and the **QueryGuard** (budget 1 ms).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::bot::Bot;
use crate::command::{CmdOk, Command};
use crate::guard::QueryGuard;
use crate::human::{AdvisorAnswerer, HumanController, SessionShared};
use crate::journal::Journal;
use crate::json::Json;
use crate::query::{Query, UiState};
use crate::session::{phase, AdvanceProfile, Phase, Session, SessionOptions, Stop};
use crate::setup::{self, NewGame};

/// What `bench` measures (the defaults are T13's).
#[derive(Clone, Debug)]
pub struct BenchOptions {
    pub seed: u32,
    /// The AI-time games: world size, opponents, turns.
    pub world: String,
    pub opponents: usize,
    pub turns: i32,
    /// Rounds of the three overhead runs (raw, session, session with autosaves), interleaved.
    pub overhead_runs: usize,
    /// Turns the test bot plays for the bot-human figure (0: skipped).
    pub bot_turns: i32,
    /// The command-latency game: a map of this size at this turn.
    pub latency_world: String,
    pub latency_turn: i32,
    /// The view-build game.
    pub view_world: String,
    pub view_opponents: usize,
    pub view_turn: i32,
    /// The `ffh` binary for `ffh bench` (its check is skipped when absent).
    pub ffh: Option<PathBuf>,
    /// Print progress to stderr.
    pub verbose: bool,
}

impl Default for BenchOptions {
    fn default() -> BenchOptions {
        BenchOptions {
            seed: 1,
            world: "WORLDSIZE_STANDARD".into(),
            opponents: 5,
            turns: 120,
            overhead_runs: 5,
            bot_turns: 120,
            latency_world: "WORLDSIZE_STANDARD".into(),
            latency_turn: 200,
            view_world: "WORLDSIZE_HUGE".into(),
            view_opponents: 8,
            view_turn: 100,
            ffh: None,
            verbose: false,
        }
    }
}

impl BenchOptions {
    /// A quick run (small maps, few turns): a smoke test of the bench itself.
    pub fn quick() -> BenchOptions {
        BenchOptions {
            world: "WORLDSIZE_SMALL".into(),
            opponents: 3,
            turns: 20,
            overhead_runs: 2,
            bot_turns: 10,
            latency_world: "WORLDSIZE_SMALL".into(),
            latency_turn: 30,
            view_world: "WORLDSIZE_SMALL".into(),
            view_opponents: 3,
            view_turn: 10,
            ..BenchOptions::default()
        }
    }
}

/// The figures (milliseconds unless named otherwise).
#[derive(Clone, Debug, Default)]
pub struct BenchReport {
    /// The AI turns of the test bot's game, and `ffh bench` (budget +10%).
    pub bot_ai_ms_per_turn: Option<f64>,
    pub ffh_bench_ms_per_turn: Option<f64>,
    /// The same AI turns as a bare update loop, through `Session::advance` without autosaves,
    /// and with them, per turn: the best of `overhead_runs`, and the slowest (the spread).
    pub raw_ms_per_turn: f64,
    pub session_ms_per_turn: f64,
    pub session_saves_ms_per_turn: f64,
    pub raw_max_ms_per_turn: f64,
    pub session_max_ms_per_turn: f64,
    /// `session / raw` of the best runs, and with the autosaves (reported: host load moves it).
    pub overhead: f64,
    pub overhead_saves: f64,
    /// Inside the session runs without autosaves (`AdvanceProfile`, wall time, median over the
    /// runs): the `update()` slices and the session's own work, per turn, and `own / update`
    /// (budget 0.10); the autosave per turn in the runs with autosaves.
    pub update_ms_per_turn: f64,
    pub own_ms_per_turn: f64,
    pub own_share: f64,
    pub autosave_ms_per_turn: f64,
    /// The times are CPU time (else wall time).
    pub cpu_time: bool,
    /// Every run ended with the same checksum (the same trajectory).
    pub same_game: bool,
    /// The seeds of the latency and view games (the next seed when the AI lost the human's
    /// civilization on the way).
    pub latency_seed: u32,
    pub view_seed: u32,
    /// Command latency (execute + settle + view): the commands timed, median and slowest.
    pub commands: usize,
    pub command_median_ms: f64,
    pub command_max_ms: f64,
    pub command_slowest: String,
    /// The view and the full map on the view game, median and slowest of 10.
    pub view_plots: usize,
    pub view_median_ms: f64,
    pub view_max_ms: f64,
    pub map_median_ms: f64,
    /// `QueryGuard::new` + drop, and the `status` query, mean over 1000 (microseconds).
    pub guard_us: f64,
    pub status_us: f64,
}

impl BenchReport {
    pub fn to_json(&self) -> Json {
        crate::jobj! {
            "bot_ai_ms_per_turn" => self.bot_ai_ms_per_turn,
            "ffh_bench_ms_per_turn" => self.ffh_bench_ms_per_turn,
            "raw_ms_per_turn" => self.raw_ms_per_turn,
            "session_ms_per_turn" => self.session_ms_per_turn,
            "session_saves_ms_per_turn" => self.session_saves_ms_per_turn,
            "raw_max_ms_per_turn" => self.raw_max_ms_per_turn,
            "session_max_ms_per_turn" => self.session_max_ms_per_turn,
            "overhead" => self.overhead,
            "overhead_saves" => self.overhead_saves,
            "update_ms_per_turn" => self.update_ms_per_turn,
            "own_ms_per_turn" => self.own_ms_per_turn,
            "own_share" => self.own_share,
            "autosave_ms_per_turn" => self.autosave_ms_per_turn,
            "cpu_time" => self.cpu_time,
            "same_game" => self.same_game,
            "latency_seed" => self.latency_seed,
            "view_seed" => self.view_seed,
            "commands" => self.commands,
            "command_median_ms" => self.command_median_ms,
            "command_max_ms" => self.command_max_ms,
            "command_slowest" => self.command_slowest.as_str(),
            "view_plots" => self.view_plots,
            "view_median_ms" => self.view_median_ms,
            "view_max_ms" => self.view_max_ms,
            "map_median_ms" => self.map_median_ms,
            "guard_us" => self.guard_us,
            "status_us" => self.status_us,
        }
    }

    /// The T13 budgets that are not met (empty when all are).
    pub fn failures(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let (Some(bot), Some(ffh)) = (self.bot_ai_ms_per_turn, self.ffh_bench_ms_per_turn) {
            if bot > ffh * 1.10 {
                out.push(format!("the AI turns of the bot's game {bot:.2} ms/turn against `ffh bench` {ffh:.2} ms/turn (budget +10%)"));
            }
        }
        if !self.same_game {
            out.push("the session and the bare loop played different games".to_string());
        }
        if self.own_share > 0.10 {
            out.push(format!("the session's own work {:.1}% of the update time (budget 10%)", self.own_share * 100.0));
        }
        if self.commands == 0 {
            out.push("no command was timed".to_string());
        }
        if self.command_max_ms >= 50.0 {
            out.push(format!("slowest command {:.1} ms ({}; budget 50 ms)", self.command_max_ms, self.command_slowest));
        }
        if self.view_median_ms >= 10.0 {
            out.push(format!("view build {:.1} ms (budget 10 ms)", self.view_median_ms));
        }
        if self.guard_us >= 1000.0 {
            out.push(format!("QueryGuard {:.0} µs (budget 1 ms)", self.guard_us));
        }
        out
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn median(v: &mut [f64]) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

/// This thread's CPU time (Linux `/proc/thread-self/schedstat`: nanoseconds on a CPU).
fn cpu_now() -> Option<Duration> {
    let s = std::fs::read_to_string("/proc/thread-self/schedstat").ok()?;
    let ns: u64 = s.split_whitespace().next()?.parse().ok()?;
    (ns > 0).then(|| Duration::from_nanos(ns))
}

/// A stopwatch of this thread's CPU time, or of wall time where that cannot be read.
struct Watch {
    wall: Instant,
    cpu: Option<Duration>,
}

impl Watch {
    fn start() -> Watch {
        Watch { wall: Instant::now(), cpu: cpu_now() }
    }

    fn elapsed(&self) -> Duration {
        match (self.cpu, cpu_now()) {
            (Some(a), Some(b)) => b.saturating_sub(a),
            _ => self.wall.elapsed(),
        }
    }
}

/// A scratch directory for the autosaves and journals, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!("ffh-bench-{tag}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch directory");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn new_game(seed: u32, world: &str, opponents: usize) -> NewGame {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, seed);
    ng.world = d.t::<WorldSizeType>(world);
    ng.opponents = opponents;
    ng
}

fn opts(dir: Option<&Path>) -> SessionOptions {
    SessionOptions { saves_dir: dir.map(Path::to_path_buf), ..SessionOptions::default() }
}

/// A session of `ng` at the human's control point of turn `turn` (the AI played the human's
/// slot until then). The AI may lose the human's civilization on the way (the game is over):
/// the next seeds are tried then. Returns the session and its seed.
fn session_at(ng: &NewGame, turn: i32) -> (Session, u32) {
    let mut ng = ng.clone();
    for _ in 0..8 {
        let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), opts(None)).expect("new game");
        assert_eq!(s.advance(&mut |_| {}), Stop::Human, "the first control point");
        let now = s.g.game_turn();
        if turn <= now || autoplay(&mut s, turn - now) == Stop::Human {
            return (s, ng.seed);
        }
        ng.seed += 1;
    }
    panic!("the human's civilization did not reach turn {turn} in 8 seeds from {}", ng.seed - 8);
}

/// `autoplay {turns}` and the AI turns that follow, to the next control point (or the end).
fn autoplay(s: &mut Session, turns: i32) -> Stop {
    let ok = s.execute(&Command::AutoPlay { turns });
    assert!(matches!(ok, Ok(CmdOk::Applied { .. })), "autoplay: {ok:?}");
    let stop = s.advance(&mut |_| {});
    assert!(!matches!(stop, Stop::Stuck(_)), "autoplay to turn {}: {stop:?}", s.g.game_turn());
    stop
}

/// `turns` of autoplay from save `start` through the session (a callback per slice, as a
/// transport has; with `saves`, the journal and the autosaves in a scratch directory, else the
/// journal in memory and no autosave). Returns the time, the final checksum and turn, and where
/// `advance` spent its time.
fn session_run(start: &[u8], sidecar: &crate::save::Sidecar, turns: i32, saves: bool) -> (Duration, i32, i32, AdvanceProfile) {
    let scratch = Scratch::new("overhead");
    let mut s = Session::load(start, Some(sidecar), Arc::new(AdvisorAnswerer), opts(saves.then_some(scratch.0.as_path()))).expect("load");
    s.profile = Some(AdvanceProfile::default());
    let t0 = Watch::start();
    let ok = s.execute(&Command::AutoPlay { turns });
    assert!(matches!(ok, Ok(CmdOk::Applied { .. })), "autoplay: {ok:?}");
    let stop = s.advance(&mut |s| {
        std::hint::black_box(s.g.game_turn());
    });
    let time = t0.elapsed();
    assert!(matches!(stop, Stop::Human | Stop::Over), "autoplay: {stop:?}");
    (time, s.g.calculate_sync_checksum(), s.g.game_turn(), s.profile.unwrap_or_default())
}

/// The same as a bare update loop: the same controllers (the human slot's HumanController
/// delegates to its own advisor while autoplay disables the human), no session.
fn raw_run(start: &[u8], human: PlayerId, turns: i32) -> (Duration, i32, i32) {
    let mut g = Game::load(ffh_data::global(), start).expect("load");
    let shared = Arc::new(SessionShared::new(Journal::off()));
    let ui: Arc<Mutex<UiState>> = Arc::default();
    let hc = Arc::new(HumanController::with_ui(Arc::new(ffh_ai::Ai::new()), Arc::new(AdvisorAnswerer), shared, ui));
    setup::attach_controllers(&mut g, human, ffh_ai::ai_controller(), hc);
    setup::configure_human(&mut g, human);
    let t0 = Watch::start();
    g.set_ai_auto_play(turns);
    loop {
        g.update();
        g.log.entries.clear();
        if !matches!(phase(&g, human), Phase::Running) {
            break;
        }
    }
    (t0.elapsed(), g.calculate_sync_checksum(), g.game_turn())
}

/// `ffh bench` on the same seed, size and civilizations: its "N ms/turn" (wall time; another
/// game).
fn ffh_bench(ffh: &Path, o: &BenchOptions) -> Option<f64> {
    let out = std::process::Command::new(ffh)
        .args(["bench", "--seed", &o.seed.to_string(), "--civs", &(o.opponents + 1).to_string(), "--size", &o.world, "--turns", &o.bot_turns.to_string()])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let i = text.find(" ms/turn")?;
    let start = text[..i].rfind('(')? + 1;
    text[start..i].trim().parse().ok()
}

/// The AI time per turn of a game the test bot plays (`advance` after its end of turn; wall
/// time, as `ffh bench`'s).
fn bot_game(o: &BenchOptions) -> f64 {
    let mut bot = Bot::new(o.seed);
    let mut s = Session::new_game(&new_game(o.seed, &o.world, o.opponents), bot.answerer(), opts(None)).expect("new game");
    let mut ai = Duration::ZERO;
    let mut turns = 0;
    loop {
        let t0 = Instant::now();
        let stop = s.advance(&mut |_| {});
        // The first advance is the game's start (no AI turn).
        if s.g.game_turn() > 0 {
            ai += t0.elapsed();
            turns += 1;
        }
        if stop != Stop::Human || s.g.game_turn() >= o.bot_turns {
            break;
        }
        bot.play(&mut s);
    }
    ms(ai) / turns.max(1) as f64
}

/// Command latency at a control point of `s`: for the human's groups, an order the `group`
/// query offers (skip, sleep, fortify, sentry, wake) or a move to a neighbouring plot, and a
/// production change per city; each timed (wall time, as a player waits for it) with the `view`
/// query after it.
fn command_latency(s: &mut Session, max: usize) -> (Vec<f64>, String) {
    let h = s.human;
    let d = s.g.d;
    let mut times = Vec::new();
    let mut slowest = (0.0, String::new());
    let mut run = |s: &mut Session, cmd: Command, times: &mut Vec<f64>| {
        let t0 = Instant::now();
        let r = s.execute(&cmd);
        let view = s.query(&Query::View);
        let t = ms(t0.elapsed());
        if matches!(r, Ok(CmdOk::Applied { .. })) && view.is_ok() {
            times.push(t);
            if t > slowest.0 {
                slowest = (t, cmd.describe(d));
            }
        }
    };
    let groups: Vec<GroupId> = s.read(|g| g.players[h].groups.clone());
    let wanted = ["MISSION_SKIP", "MISSION_FORTIFY", "MISSION_SLEEP", "MISSION_SENTRY", "COMMAND_WAKE"];
    for (n, grp) in groups.into_iter().enumerate() {
        if times.len() >= max || !matches!(s.phase(), Phase::Human(_)) {
            break;
        }
        let units: Option<Vec<UnitId>> = s.read(|g| g.groups.get(grp).map(|gr| gr.units.clone()));
        let Some(units) = units.filter(|u| !u.is_empty()) else { continue };
        let Ok(reply) = s.query(&Query::Group { units: units.clone() }) else { continue };
        let actions = reply.get("actions").and_then(Json::as_arr).unwrap_or_default().to_vec();
        // Every other group moves to a neighbouring plot it can enter (path, settle).
        if n % 2 == 1 {
            let target = s.read(|g| {
                let u = units[0];
                let (x, y) = (g.units[u].x, g.units[u].y);
                [(0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1)]
                    .iter()
                    .filter_map(|&(dx, dy)| g.plot_xy(x, y, dx, dy))
                    .find(|&p| g.unit_can_move_into(u, p, false, false, false) && g.map.plots[p].units.is_empty())
                    .map(|p| g.map.xy(p))
            });
            if let (Some((x, y)), true) = (target, s.read(|g| g.unit_moves_left(units[0]) > 0)) {
                run(s, Command::Move { units: units.clone(), x, y, append: false, declare_war: Some(false) }, &mut times);
                continue;
            }
        }
        let name = |a: &Json, key: &str| -> Option<&'static str> {
            let i = a.get(key).and_then(Json::as_i64)? as usize;
            Some(if key == "mission" { crate::names::name(d, MissionType::from_usize(i)) } else { crate::names::name(d, CommandType::from_usize(i)) })
        };
        let pick = wanted.iter().find_map(|w| {
            actions.iter().find(|a| a.get("enabled").and_then(Json::as_bool) == Some(true) && (name(a, "mission") == Some(w) || name(a, "command") == Some(w)))
        });
        if let Some(cmd) = pick.and_then(|a| a.get("cmd")).and_then(|c| Command::from_json(d, c).ok()) {
            run(s, cmd, &mut times);
        }
    }
    let cities: Vec<CityId> = s.read(|g| g.players[h].cities.clone());
    for c in cities.into_iter().take(max / 2) {
        if !matches!(s.phase(), Phase::Human(_)) {
            break;
        }
        let Ok(city) = s.query(&Query::City { city: c }) else { continue };
        let pick = city.get("buildable").and_then(Json::as_arr).and_then(|b| b.iter().find(|x| x.get("enabled").and_then(Json::as_bool) == Some(true)).cloned());
        if let Some(b) = pick {
            let order = OrderType::from_usize(b.get("order").and_then(Json::as_i64).unwrap_or(0) as usize);
            let data = b.get("type").and_then(Json::as_i64).unwrap_or(0) as i32;
            run(s, Command::PushOrder { city: c, order, data, alt: false, shift: true, ctrl: false }, &mut times);
        }
    }
    (times, slowest.1)
}

/// Runs the measurements of [`BenchOptions`].
pub fn run(o: &BenchOptions) -> BenchReport {
    let say = |m: &str| {
        if o.verbose {
            eprintln!("bench: {m}");
        }
    };
    let mut r = BenchReport::default();

    // 1. The AI time per turn: the bot's game and `ffh bench`.
    if o.bot_turns > 0 {
        say(&format!("the test bot's game: {} turns, {}, {} opponents, seed {}", o.bot_turns, o.world, o.opponents, o.seed));
        r.bot_ai_ms_per_turn = Some(bot_game(o));
        if let Some(ffh) = o.ffh.as_deref().filter(|p| p.is_file()) {
            say("ffh bench");
            r.ffh_bench_ms_per_turn = ffh_bench(ffh, o);
        }
    }

    // 2. The session overhead on one trajectory: `overhead_runs` interleaved rounds of the three
    // ways; the budget is on the session's own share inside its runs.
    let rounds = o.overhead_runs.max(1);
    say(&format!("session overhead: {} turns of autoplay, {rounds} rounds", o.turns));
    let (s, _) = session_at(&new_game(o.seed, &o.world, o.opponents), 0);
    let (start, sidecar) = s.save_bytes().expect("save at the first control point");
    let (human, from) = (s.human, s.g.game_turn());
    drop(s);
    let (mut raw, mut sess, mut saves) = (Vec::new(), Vec::new(), Vec::new());
    let (mut shares, mut updates, mut owns, mut autosaves) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut ends = Vec::new();
    for _ in 0..rounds {
        let (t, sum, end) = raw_run(&start, human, o.turns);
        raw.push(t);
        ends.push((sum, end));
        let (t, sum, end, p) = session_run(&start, &sidecar, o.turns, false);
        sess.push(t);
        ends.push((sum, end));
        shares.push(p.own.as_secs_f64() / p.update.as_secs_f64().max(1e-9));
        updates.push(ms(p.update));
        owns.push(ms(p.own));
        let (t, sum, end, p) = session_run(&start, &sidecar, o.turns, true);
        saves.push(t);
        ends.push((sum, end));
        autosaves.push(ms(p.autosave));
    }
    let turns = (ends[0].1 - from).max(1) as f64;
    let best = |v: &[Duration]| v.iter().copied().min().unwrap_or_default();
    let worst = |v: &[Duration]| v.iter().copied().max().unwrap_or_default();
    r.cpu_time = cpu_now().is_some();
    r.raw_ms_per_turn = ms(best(&raw)) / turns;
    r.session_ms_per_turn = ms(best(&sess)) / turns;
    r.session_saves_ms_per_turn = ms(best(&saves)) / turns;
    r.raw_max_ms_per_turn = ms(worst(&raw)) / turns;
    r.session_max_ms_per_turn = ms(worst(&sess)) / turns;
    r.overhead = best(&sess).as_secs_f64() / best(&raw).as_secs_f64().max(1e-9);
    r.overhead_saves = best(&saves).as_secs_f64() / best(&raw).as_secs_f64().max(1e-9);
    r.own_share = median(&mut shares);
    r.update_ms_per_turn = median(&mut updates) / turns;
    r.own_ms_per_turn = median(&mut owns) / turns;
    r.autosave_ms_per_turn = median(&mut autosaves) / turns;
    r.same_game = ends.iter().all(|e| *e == ends[0]);

    // 3. Command latency.
    say(&format!("command latency: {} at turn {}", o.latency_world, o.latency_turn));
    let (mut s, seed) = session_at(&new_game(o.seed, &o.latency_world, o.opponents), o.latency_turn);
    r.latency_seed = seed;
    let (mut times, slowest) = command_latency(&mut s, 40);
    r.commands = times.len();
    r.command_max_ms = times.iter().copied().fold(0.0, f64::max);
    r.command_median_ms = median(&mut times);
    r.command_slowest = slowest;
    drop(s);

    // 4. The view on a big map, the guard.
    say(&format!("view build: {} with {} opponents at turn {}", o.view_world, o.view_opponents, o.view_turn));
    let (s, seed) = session_at(&new_game(o.seed, &o.view_world, o.view_opponents), o.view_turn);
    r.view_seed = seed;
    r.view_plots = s.g.map.num_plots();
    let mut views = Vec::new();
    let mut maps = Vec::new();
    for _ in 0..10 {
        let t0 = Instant::now();
        std::hint::black_box(s.query(&Query::View).expect("view"));
        views.push(ms(t0.elapsed()));
        let t0 = Instant::now();
        std::hint::black_box(s.query(&Query::Map { since: None, epoch: None }).expect("map"));
        maps.push(ms(t0.elapsed()));
    }
    r.view_max_ms = views.iter().copied().fold(0.0, f64::max);
    r.view_median_ms = median(&mut views);
    r.map_median_ms = median(&mut maps);
    let n = 1000;
    let t0 = Instant::now();
    for _ in 0..n {
        let g = QueryGuard::new(&s.g);
        std::hint::black_box(&g);
    }
    r.guard_us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
    let t0 = Instant::now();
    for _ in 0..n {
        std::hint::black_box(s.query(&Query::Status).expect("status"));
    }
    r.status_us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
    r
}

/// The report for people.
pub fn render(o: &BenchOptions, r: &BenchReport) -> String {
    let mut out = String::new();
    let opt = |v: Option<f64>| v.map_or("-".to_string(), |v| format!("{v:.2} ms/turn"));
    out += &format!(
        "AI turns, {} {} opponents seed {}, {} turns: the test bot's game {} against `ffh bench` {} (budget +10%; two different games)\n",
        o.world,
        o.opponents,
        o.seed,
        o.bot_turns,
        opt(r.bot_ai_ms_per_turn),
        opt(r.ffh_bench_ms_per_turn)
    );
    out += &format!(
        "the same {} turns of autoplay, inside the session's runs (wall time, median of {}): update {:.2} ms/turn, the session's own work {:.3} ms/turn ({:.1}% of it, budget 10%), the autosave {:.2} ms/turn{}\n",
        o.turns,
        o.overhead_runs.max(1),
        r.update_ms_per_turn,
        r.own_ms_per_turn,
        r.own_share * 100.0,
        r.autosave_ms_per_turn,
        if r.same_game { "; the same game each time" } else { "; DIFFERENT GAMES" }
    );
    out += &format!(
        "  whole runs ({} time, best of {}, interleaved; not budgeted: host load moves them): bare update loop {:.2} ms/turn (slowest {:.2}); session {:.2} ms/turn ({:+.1}%; slowest {:.2}); with autosaves {:.2} ms/turn ({:+.1}%)\n",
        if r.cpu_time { "CPU" } else { "wall" },
        o.overhead_runs.max(1),
        r.raw_ms_per_turn,
        r.raw_max_ms_per_turn,
        r.session_ms_per_turn,
        (r.overhead - 1.0) * 100.0,
        r.session_max_ms_per_turn,
        r.session_saves_ms_per_turn,
        (r.overhead_saves - 1.0) * 100.0,
    );
    out += &format!(
        "commands, {} seed {} at turn {}: {} timed (execute + settle + view), median {:.2} ms, slowest {:.2} ms ({}) (budget 50 ms)\n",
        o.latency_world, r.latency_seed, o.latency_turn, r.commands, r.command_median_ms, r.command_max_ms, r.command_slowest
    );
    out += &format!(
        "view, {} ({} plots, {} opponents, seed {}) at turn {}: median {:.2} ms, slowest {:.2} ms (budget 10 ms); full map {:.2} ms\n",
        o.view_world, r.view_plots, o.view_opponents, r.view_seed, o.view_turn, r.view_median_ms, r.view_max_ms, r.map_median_ms
    );
    out += &format!("QueryGuard {:.1} µs (budget 1 ms); status query {:.1} µs\n", r.guard_us, r.status_us);
    out
}
