//! The AI-as-human soak (design §7 P2): `ffh_ai::Ai` plays the human slot through the session,
//! so the rules' human-only branches (deferred and blocking decisions, human mission timers,
//! the active-player paths, the human governor, take-over) run for whole games, with the
//! invariants of ffh-stress's `check.rs` checked as it goes.
//!
//! At every control point the driver plays the human's turn the way design §8 T5 describes:
//! * production: every other city (odd ids) is set to automated production with the
//!   `do_task SET_AUTOMATED_PRODUCTION` command (the human automation path: the rules then ask
//!   the human's controller, which asks its advisor); idle manual cities get the AI's
//!   `choose_production` (the deferred CHOOSEPRODUCTION popup, answered);
//! * research, event popups and diplomatic votes are answered with the AI's choice through the
//!   `research`, `event` and `vote` commands (a refusal of what `pending` offers is a failure),
//!   except on one turn in [`QUIET_EVERY`], when they are left open and the end-turn gate is
//!   forced (C++ then closes the event popups, gives a vote its extra turn then abstains, and
//!   makes the AI research pick after turn 4: human-only paths too). The research the AI's own
//!   empire management sets on such a turn (its first `play_turn` fills an empty queue) is
//!   cleared with `research {tech: null}` before the end turn, so the popup really is open;
//!   popups are read again before each answer (one answer can change what the next offers);
//! * blocking decisions (keep/raze, free tech, election, ...) take the advisor's pick
//!   ([`AdvisorAnswerer`]);
//! * the AI moves the human's units (`Controller::play_turn`, which uses the rules' mission API
//!   as a human would), repeated while it moves something, at most 20 times, the session
//!   running the human's missions in between;
//! * `end_turn`, then `end_turn {force}` if the end-turn gate asks.
//!
//! Checks: `check.rs` every [`RunOptions::check_every`] turns, at most one active player
//! turn at a time, and `save(load(save)) == save` (plus the sidecar's JSON round trip) every
//! [`RunOptions::save_check_every`] turns. A refused command, a stuck turn (the watchdog) or a
//! failed check is a failure.
//!
//! Infernals and Mercurians are not playable at the start (`bPlayable` 0): FfH spawns them
//! mid-game (Infernal Pact, Mercurian Gate) and offers a human founder control of them
//! (`reassignPlayer`, which the port has no hook for). Their soak plays an AI-only game, makes
//! player 0 found them at [`SoakGame::spawn_turn`] (the tech or the wonder is granted, which
//! runs the FfH Python port that spawns them), and takes the new player over at the next turn
//! boundary (`Session::take_over`).
//!
//! Every game is deterministic, so the repro of a failure is the game's own command line
//! (`ffh-play soak run ...`), or, with autosaves, the same line with `--resume <autosave>`.
//! `ffh-play soak` (batch) runs a matrix of games, each in a child process, so aborts, stack
//! overflows and hangs are caught too.

use std::backtrace::Backtrace;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use ffh_ai::Ai;
use ffh_core::controller::{Controller, ControllerRef};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::mapscript::FractalMapScript;
use ffh_core::setup::SetupOptions;
use ffh_core::structs::PlayerVote;
use ffh_data::*;

use crate::command::{CmdOk, Command, Confirm};
use crate::human::{AdvisorAnswerer, Answerer, Stats};
use crate::names;
use crate::save::{self, Sidecar};
use crate::session::{Phase, Session, SessionOptions, Stop};
use crate::setup::NewGame;

pub mod batch;

pub use batch::{BatchOptions, Done, Matrix};

/// ffh-stress's invariant checks, reused without editing ffh-stress (design F20).
#[path = "../../ffh-stress/src/check.rs"]
pub mod check;

/// Civilizations a soak takes over when they appear (not playable at the start).
pub const SPAWNED_CIVS: [&str; 2] = ["CIVILIZATION_INFERNAL", "CIVILIZATION_MERCURIANS"];

/// On game turns `QUIET_TURN` modulo `QUIET_EVERY` the deferred popups are left open.
pub const QUIET_EVERY: i32 = 7;
pub const QUIET_TURN: i32 = 3;

/// One soak game.
#[derive(Clone, Debug, PartialEq)]
pub struct SoakGame {
    /// The new-game settings (`ng.civ` is the human's civilization, `None` = random).
    pub ng: NewGame,
    /// A civilization from [`SPAWNED_CIVS`] the human takes over when it appears (then `ng` is
    /// an AI-only game of `ng.opponents + 1` civs whose player 0 founds it).
    pub spawn: Option<CivilizationType>,
    /// Game turn at which player 0 founds `spawn`.
    pub spawn_turn: i32,
    /// Game turns played (from the game's start turn).
    pub turns: i32,
}

impl SoakGame {
    /// Defaults: [`NewGame::new`] with the world's default number of civs, 300 turns.
    pub fn new(d: &GameData, seed: u32) -> SoakGame {
        let mut ng = NewGame::new(d, seed);
        ng.opponents = default_opponents(d, ng.world);
        SoakGame { ng, spawn: None, spawn_turn: 30, turns: 300 }
    }

    /// The human's civilization (requested), if any.
    pub fn civ(&self) -> Option<CivilizationType> {
        self.spawn.or(self.ng.civ)
    }

    /// Settings from command-line arguments (see `ffh-play soak run`); type names may be
    /// shortened (`tiny`, `kuriotates`, `raging`).
    pub fn parse(d: &GameData, args: &[String]) -> Result<SoakGame, String> {
        let mut g = SoakGame::new(d, 1);
        let mut opponents = None;
        let mut i = 0;
        while i < args.len() {
            let v = args.get(i + 1).ok_or_else(|| format!("{} needs a value", args[i]))?;
            // Each number in its own type: an out-of-range value is an error, not a wrapped one.
            fn num<T: std::str::FromStr>(flag: &str, v: &str) -> Result<T, String> {
                v.parse::<T>().map_err(|_| format!("{flag}: not a number in range: {v}"))
            }
            let flag = args[i].as_str();
            match flag {
                "--seed" => g.ng.seed = num::<u32>(flag, v)?,
                "--size" => g.ng.world = lenient(d, "WORLDSIZE_", v)?,
                "--civ" => {
                    let c: CivilizationType = lenient(d, "CIVILIZATION_", v)?;
                    if SPAWNED_CIVS.contains(&d.type_name(c)) {
                        g.spawn = Some(c);
                        g.ng.civ = None;
                    } else {
                        g.spawn = None;
                        g.ng.civ = Some(c);
                    }
                }
                "--opponents" => opponents = Some(num::<usize>(flag, v)?),
                "--options" => g.ng.options = parse_options(d, v)?,
                "--turns" => g.turns = num::<i32>(flag, v)?,
                "--speed" => g.ng.speed = lenient(d, "GAMESPEED_", v)?,
                "--handicap" => g.ng.handicap = lenient(d, "HANDICAP_", v)?,
                "--era" => g.ng.era = lenient(d, "ERA_", v)?,
                "--spawn-turn" => g.spawn_turn = num::<i32>(flag, v)?,
                a => return Err(format!("unknown soak setting {a}")),
            }
            i += 2;
        }
        g.ng.opponents = opponents.unwrap_or_else(|| default_opponents(d, g.ng.world));
        if g.turns < 1 {
            return Err("--turns must be at least 1".into());
        }
        g.ng.validate(d)?;
        Ok(g)
    }

    /// The command-line form (inverse of [`SoakGame::parse`], with full type names).
    pub fn to_args(&self, d: &GameData) -> Vec<String> {
        let base = SoakGame::new(d, self.ng.seed);
        let mut a: Vec<String> = Vec::new();
        let mut push = |k: &str, v: String| {
            a.push(k.into());
            a.push(v);
        };
        push("--seed", self.ng.seed.to_string());
        push("--size", d.type_name(self.ng.world).to_string());
        if let Some(c) = self.civ() {
            push("--civ", d.type_name(c).to_string());
        }
        if self.ng.opponents != default_opponents(d, self.ng.world) {
            push("--opponents", self.ng.opponents.to_string());
        }
        if !self.ng.options.is_empty() {
            push("--options", self.ng.options.iter().map(|&o| d.type_name(o)).collect::<Vec<_>>().join(","));
        }
        push("--turns", self.turns.to_string());
        if self.ng.speed != base.ng.speed {
            push("--speed", d.type_name(self.ng.speed).to_string());
        }
        if self.ng.handicap != base.ng.handicap {
            push("--handicap", d.type_name(self.ng.handicap).to_string());
        }
        if self.ng.era != base.ng.era {
            push("--era", d.type_name(self.ng.era).to_string());
        }
        if self.spawn.is_some() && self.spawn_turn != base.spawn_turn {
            push("--spawn-turn", self.spawn_turn.to_string());
        }
        a
    }

    /// One line for people (prefixes dropped): `s1 TINYx3 KURIOTATES [RAGING_BARBARIANS] 300`,
    /// plus the speed, handicap, era and spawn turn where they are not the defaults
    /// (`s1 TINYx3 BANNOR QUICK DEITY 400`).
    pub fn describe(&self, d: &GameData) -> String {
        let short = |s: &str| s.split_once('_').map_or(s.to_string(), |(_, r)| r.to_string());
        let base = SoakGame::new(d, self.ng.seed);
        let mut out = format!(
            "s{} {}x{} {}",
            self.ng.seed,
            short(d.type_name(self.ng.world)),
            self.ng.opponents + 1,
            self.civ().map_or("RANDOM".to_string(), |c| short(d.type_name(c)))
        );
        if !self.ng.options.is_empty() {
            out += &format!(" [{}]", options_label(d, &self.ng.options));
        }
        if self.ng.speed != base.ng.speed {
            out += &format!(" {}", short(d.type_name(self.ng.speed)));
        }
        if self.ng.handicap != base.ng.handicap {
            out += &format!(" {}", short(d.type_name(self.ng.handicap)));
        }
        if self.ng.era != base.ng.era {
            out += &format!(" {}", short(d.type_name(self.ng.era)));
        }
        if self.spawn.is_some() && self.spawn_turn != base.spawn_turn {
            out += &format!(" spawn@{}", self.spawn_turn);
        }
        out + &format!(" {}", self.turns)
    }

    /// A file-name form of [`SoakGame::describe`], cut to 55 characters, then 8 hex digits of
    /// the FNV-1a hash of [`SoakGame::to_args`] (`s1-tinyx3-kuriotates-raging_barbarians-300-…`):
    /// different settings get different names, also where the description is cut or does not
    /// show a setting (speed, handicap, era, spawn turn).
    pub fn file_name(&self, d: &GameData) -> String {
        let mut s: String = self.describe(d).chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c.to_ascii_lowercase() } else { '-' }).collect();
        while s.contains("--") {
            s = s.replace("--", "-");
        }
        let base: String = s.trim_matches('-').chars().take(55).collect();
        let hash = crate::journal::fnv_hex(self.to_args(d).join(" ").as_bytes());
        format!("{}-{}", base.trim_end_matches('-'), &hash[..8])
    }
}

/// AI opponents of a world size's default game (`iDefaultPlayers` civs in all).
pub fn default_opponents(d: &GameData, w: WorldSizeType) -> usize {
    (d.worlds[w].default_players.max(2) as usize) - 1
}

/// A type by name: the exact type name, `PREFIX` + the upper-cased text, or the only type whose
/// name contains it (`tiny` -> WORLDSIZE_TINY, `mercurian` -> CIVILIZATION_MERCURIANS).
pub fn lenient<T: InfoId + names::Named>(d: &GameData, prefix: &str, s: &str) -> Result<T, String> {
    if let Some(t) = d.ty::<T>(s) {
        return Ok(t);
    }
    let up = s.to_ascii_uppercase();
    if let Some(t) = d.ty::<T>(&format!("{prefix}{up}")) {
        return Ok(t);
    }
    let all: Vec<T> = (0..T::count(d)).map(T::from_usize).filter(|&t| d.type_name(t).contains(&up)).collect();
    match all.as_slice() {
        [t] => Ok(*t),
        [] => Err(format!("unknown {} {s:?}", T::KIND)),
        many => Err(format!("{s:?} is ambiguous: {}", many.iter().map(|&t| d.type_name(t)).collect::<Vec<_>>().join(", "))),
    }
}

/// Named sets of game options that only act on human players (no-ops in AI-only games, so only
/// a human soak runs their rules branches).
pub const OPTION_SETS: [(&str, &[&str]); 2] = [
    // One-city challenge (no settlers, conquered cities razed, ...) and no healing.
    ("occ", &["GAMEOPTION_ONE_CITY_CHALLENGE", "GAMEOPTION_NO_HEALING_FOR_HUMANS"]),
    // FfH challenges: the last civ cut every 50 turns, the leading human moved to the last civ
    // (`reassignPlayerAdvanced`), the difficulty raised every 50 turns or with the human's rank.
    (
        "challenges",
        &["GAMEOPTION_CHALLENGE_CUT_LOSERS", "GAMEOPTION_CHALLENGE_HIGH_TO_LOW", "GAMEOPTION_CHALLENGE_INCREASING_DIFFICULTY", "GAMEOPTION_FLEXIBLE_DIFFICULTY"],
    ),
];

/// Short names of game options (`RAGING_BARBARIANS,AGGRESSIVE_AI`), or the [`OPTION_SETS`] name.
pub fn options_label(d: &GameData, options: &[GameOptionType]) -> String {
    let names: Vec<&str> = options.iter().map(|&o| d.type_name(o)).collect();
    match OPTION_SETS.iter().find(|(_, set)| *set == names.as_slice()) {
        Some((name, _)) => name.to_string(),
        None => names.iter().map(|n| n.trim_start_matches("GAMEOPTION_")).collect::<Vec<_>>().join(","),
    }
}

/// Game options: `default` (none), or names or [`OPTION_SETS`] joined by `,` or `+`
/// (`raging+aggressive`, `occ`).
pub fn parse_options(d: &GameData, s: &str) -> Result<Vec<GameOptionType>, String> {
    let mut v = Vec::new();
    for o in s.split([',', '+']).filter(|x| !x.is_empty() && *x != "default") {
        match OPTION_SETS.iter().find(|(name, _)| *name == o) {
            Some((_, set)) => v.extend(set.iter().map(|n| d.t::<GameOptionType>(n))),
            None => v.push(lenient::<GameOptionType>(d, "GAMEOPTION_", o)?),
        }
    }
    let mut out: Vec<GameOptionType> = Vec::new();
    for o in v {
        if !out.contains(&o) {
            out.push(o);
        }
    }
    Ok(out)
}

// ---- one game ----------------------------------------------------------------------------------

/// How a game is run.
#[derive(Clone, Debug)]
pub struct RunOptions {
    /// Autosaves (the newest 3) in `<saves>/auto/`; `None`: no files.
    pub saves: Option<PathBuf>,
    /// Continue from this save (with its sidecar, if any) instead of starting the game.
    pub resume: Option<PathBuf>,
    /// `check.rs` every N game turns (0 = never).
    pub check_every: i32,
    /// `save(load(save)) == save` every N game turns (0 = never).
    pub save_check_every: i32,
    /// A deliberate panic at the first control point of this game turn (a self-test of the
    /// crash report).
    pub panic_at: Option<i32>,
}

impl Default for RunOptions {
    fn default() -> RunOptions {
        RunOptions { saves: None, resume: None, check_every: 5, save_check_every: 25, panic_at: None }
    }
}

impl RunOptions {
    /// The check settings that are not the defaults, as `soak run` arguments. They belong in a
    /// repro: a check runs only on the turns they name.
    pub fn check_args(&self) -> Vec<String> {
        let def = RunOptions::default();
        let mut a = Vec::new();
        if self.check_every != def.check_every {
            a.extend(["--check-every".to_string(), self.check_every.to_string()]);
        }
        if self.save_check_every != def.save_check_every {
            a.extend(["--save-check-every".to_string(), self.save_check_every.to_string()]);
        }
        a
    }
}

/// A `--check-every` / `--save-check-every` value: game turns, 0 = never.
pub fn parse_every(flag: &str, v: &str) -> Result<i32, String> {
    v.parse::<i32>().ok().filter(|&n| n >= 0).ok_or_else(|| format!("{flag}: expected a number of turns (0 = never), got {v:?}"))
}

/// What the AI-as-human did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SoakStats {
    /// Human turns played (control points that ended with `end_turn`).
    pub turns_played: i32,
    pub productions: usize,
    pub automated_cities: usize,
    pub research: usize,
    pub events: usize,
    /// Event popups left unanswered (no option doable): closed by the forced end turn.
    pub events_unanswerable: usize,
    pub votes: usize,
    /// Elections answered (the human was secretary general).
    pub elections: usize,
    /// Deferred popups left open on the quiet turns (research, events, votes).
    pub popups_left: usize,
    /// Of those, research popups (still open at the forced end turn: C++ then makes the
    /// AI_chooseResearch pick after turn 4, `Stats::research_ai_picks`).
    pub research_left_open: usize,
    /// `end_turn` answered with the gate's confirmation (then forced).
    pub gate_confirms: usize,
    pub checks: usize,
    pub save_checks: usize,
    /// The game turn of the take-over (spawned civilizations).
    pub taken_over_at: Option<i32>,
}

/// How a game ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    /// The turns were played.
    Turns,
    /// Game over (a victory, or the human was eliminated).
    Over { human_alive: bool, victory: Option<VictoryType>, winner: Option<TeamId> },
    /// A turn did not finish (the session's watchdog): diagnostics.
    Stuck(String),
}

/// The result of one game.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub end: End,
    pub turn: i32,
    pub checksum: i32,
    /// FNV-1a of the final `Game::save()` (resume and determinism checks).
    pub save_fnv: String,
    /// The human's slot and civilization at the end (FfH may have moved the human).
    pub human: PlayerId,
    pub human_civ: Option<CivilizationType>,
    pub stats: SoakStats,
    pub session: Stats,
    /// The human's and the AI civs' cities at the end (strength comparison).
    pub human_cities: usize,
    pub ai_cities: Vec<usize>,
    pub secs: f64,
}

impl Outcome {
    /// The `END` line: `ON`, `OVER <victory> winner=<team>`, `HUMAN-DEAD`, `STUCK`.
    pub fn end_text(&self, d: &GameData) -> String {
        match &self.end {
            End::Turns => "ON".into(),
            End::Over { human_alive: false, .. } => "HUMAN-DEAD".into(),
            End::Over { victory, winner, .. } => format!(
                "OVER {} winner={}",
                victory.map_or("NO_VICTORY".to_string(), |v| d.type_name(v).trim_start_matches("VICTORY_").to_string()),
                winner.map_or("-".to_string(), |t| t.0.to_string())
            ),
            End::Stuck(_) => "STUCK".into(),
        }
    }
}

/// Game turn being played (for panic reports).
static TURN: AtomicI32 = AtomicI32::new(-1);

/// Plays `game` (panics propagate; see [`run_caught`]). `on_turn` is called at the human's
/// first control point of every game turn.
pub fn run(game: &SoakGame, opts: &RunOptions, on_turn: &mut dyn FnMut(&Session)) -> Result<Outcome, String> {
    let t0 = Instant::now();
    let sopts = SessionOptions {
        saves_dir: opts.saves.clone(),
        autosave_keep: if opts.saves.is_some() { 3 } else { 0 },
        // The AI moves the human's units through rules calls, not commands: a journal would not
        // replay. A failure is reproduced by re-running the game (or resuming an autosave).
        journal: false,
        ..SessionOptions::default()
    };
    let answerer: Arc<dyn Answerer> = Arc::new(AdvisorAnswerer);
    let mut stats = SoakStats::default();
    let mut s = if let Some(path) = &opts.resume {
        let (bytes, sidecar) = save::read_bundle(path)?;
        Session::load(&bytes, sidecar.as_ref(), answerer, sopts).map_err(|e| e.to_string())?
    } else if let Some(civ) = game.spawn {
        let (bytes, p, turn) = spawn_civ(game, civ)?;
        stats.taken_over_at = Some(turn);
        Session::take_over(&bytes, p, answerer, sopts).map_err(|e| e.to_string())?
    } else {
        Session::new_game(&game.ng, answerer, sopts).map_err(|e| e.to_string())?
    };
    let until = s.g.start_turn + game.turns;
    let mut driver = Driver { ai: Ai::new(), stats, last_turn: i32::MIN };
    let end = loop {
        let stop = s.advance(&mut |s| TURN.store(s.g.game_turn(), Ordering::Relaxed));
        TURN.store(s.g.game_turn(), Ordering::Relaxed);
        match stop {
            Stop::Stuck(diag) => break End::Stuck(diag),
            Stop::Over => break End::Over { human_alive: s.g.players[s.human].alive, victory: s.g.victory, winner: s.g.winner },
            Stop::Human => {}
        }
        let turn = s.g.game_turn();
        if turn != driver.last_turn {
            driver.last_turn = turn;
            driver.turn_checks(&s, opts)?;
            on_turn(&s);
            if opts.panic_at == Some(turn) {
                panic!("soak self-test: a deliberate panic at turn {turn}");
            }
            if turn >= until {
                break End::Turns;
            }
        }
        driver.play(&mut s)?;
    };
    if end == End::Turns {
        s.read(check::check);
        driver.stats.checks += 1;
    }
    let h = s.human;
    let ai_cities = s.g.civ_player_ids().filter(|&p| p != h && s.g.players[p].alive).map(|p| s.g.players[p].cities.len()).collect();
    Ok(Outcome {
        end,
        turn: s.g.game_turn(),
        checksum: s.g.calculate_sync_checksum(),
        save_fnv: crate::journal::fnv_hex(&s.g.save()),
        human: h,
        human_civ: s.g.player_get_civilization_type(h),
        human_cities: s.g.players[h].cities.len(),
        ai_cities,
        stats: driver.stats,
        session: s.stats(),
        secs: t0.elapsed().as_secs_f64(),
    })
}

/// The AI-as-human.
struct Driver {
    ai: Ai,
    stats: SoakStats,
    /// Game turn of the last control point seen.
    last_turn: i32,
}

impl Driver {
    /// The checks of a new game turn (at its first control point).
    fn turn_checks(&mut self, s: &Session, opts: &RunOptions) -> Result<(), String> {
        let turn = s.g.game_turn();
        if s.g.num_game_turn_active > 1 {
            return Err(format!("turn {turn}: {} player turns active at once", s.g.num_game_turn_active));
        }
        if opts.check_every > 0 && turn % opts.check_every == 0 {
            s.read(check::check);
            self.stats.checks += 1;
        }
        if opts.save_check_every > 0 && turn % opts.save_check_every == 0 {
            save_round_trip(s)?;
            self.stats.save_checks += 1;
        }
        Ok(())
    }

    /// Executes `cmd`, which the AI-as-human took from what the session offers: anything but
    /// `Applied` is a failure (the end-turn gate is handled by the caller).
    fn exec(&mut self, s: &mut Session, cmd: Command) -> Result<CmdOk, String> {
        match s.execute(&cmd) {
            Ok(ok) => Ok(ok),
            Err(e) => Err(format!("turn {}: {} refused: {} {}", s.g.game_turn(), cmd.describe(s.g.d), e.code(), e.detail())),
        }
    }

    fn applied(&mut self, s: &mut Session, cmd: Command) -> Result<(), String> {
        match self.exec(s, cmd.clone())? {
            CmdOk::Applied { .. } => Ok(()),
            CmdOk::NeedsConfirm(c) => Err(format!("turn {}: {} asks for confirmation: {c:?}", s.g.game_turn(), cmd.describe(s.g.d))),
        }
    }

    /// The human's turn at a control point (module docs). Returns at the next control point's
    /// caller loop: after `end_turn`, or early when a command leaves the human without control.
    fn play(&mut self, s: &mut Session) -> Result<(), String> {
        let h = s.human;
        // Still the human's control point, and the same human (the rules may move the human
        // to another slot: then the next control point is the new slot's).
        let at_control = |s: &Session| s.phase() == Phase::Human(h);
        // Production: odd city ids automated (do_task), the others chosen when idle.
        let to_automate: Vec<CityId> = s.g.players[h].cities.iter().copied().filter(|c| c.0 % 2 == 1 && !s.g.cities[*c].production_automated).collect();
        for c in to_automate {
            if !at_control(s) {
                return Ok(());
            }
            let cmd = Command::DoTask { city: c, task: TaskType::TASK_SET_AUTOMATED_PRODUCTION, data1: -1, data2: -1, option: true, alt: false, shift: false, ctrl: false };
            self.applied(s, cmd)?;
            self.stats.automated_cities += 1;
        }
        for c in s.pending().idle_cities {
            self.ai.choose_production(&mut s.g, c);
            self.stats.productions += 1;
        }
        // Research, events, votes: the deferred popups, answered through their commands; on
        // turns `QUIET_TURN` mod `QUIET_EVERY` left open, so the end-turn gate is forced with
        // what C++ does to unanswered popups (events closed, a vote's extra turn then abstain,
        // the AI's research pick after turn 4).
        let p = s.pending();
        let quiet = s.g.game_turn() % QUIET_EVERY == QUIET_TURN;
        if quiet {
            self.stats.popups_left += p.popups();
        } else {
            self.answer_popups(s, p.research_needed)?;
        }
        if !at_control(s) {
            return Ok(());
        }
        // The AI moves the human's units while it moves something; the session runs the
        // missions (human mission timers) in between.
        for _ in 0..20 {
            if !at_control(s) {
                return Ok(());
            }
            let before = fingerprint(&s.g, h);
            self.ai.play_turn(&mut s.g, h);
            if s.advance(&mut |s| TURN.store(s.g.game_turn(), Ordering::Relaxed)) != Stop::Human {
                return Ok(());
            }
            if fingerprint(&s.g, h) == before {
                break;
            }
        }
        if !at_control(s) {
            return Ok(());
        }
        // A research popup left open stays open: the AI's first `play_turn` of a turn (its
        // empire management) sets research with a rules call when the queue is empty, which a
        // human with the popup open has not done. Undo it as the human would, with CvNetResearch
        // and NO_TECH (`clearResearchQueue`, CvMessageData.cpp:383-385), so that the forced end
        // turn runs `doResearch` with no research (the AI_chooseResearch pick after turn 4).
        if quiet && p.research_needed {
            if s.g.player_get_current_research(h).is_some() {
                self.applied(s, Command::Research { tech: None, shift: false })?;
                if !at_control(s) {
                    return Ok(());
                }
            }
            self.stats.research_left_open += 1;
        }
        // End the turn; the gate's confirmation (popups left open, ready units) is forced.
        if let CmdOk::NeedsConfirm(c) = self.exec(s, Command::EndTurn { force: false })? {
            if !matches!(c, Confirm::EndTurn { .. }) {
                return Err(format!("turn {}: end_turn asks {c:?}", s.g.game_turn()));
            }
            self.stats.gate_confirms += 1;
            self.applied(s, Command::EndTurn { force: true })?;
        }
        self.stats.turns_played += 1;
        Ok(())
    }

    /// Research (when `research_needed`), event popups and votes, with the AI's choices. Each
    /// answer can change what the other popups offer (gold spent, a unit gone, a new popup
    /// launched by an event's outcome), and the commands check their gates when applied, so the
    /// pending popups are read again before each answer.
    fn answer_popups(&mut self, s: &mut Session, research_needed: bool) -> Result<(), String> {
        let h = s.human;
        // Still the human's control point, and the same human (the rules may move the human
        // to another slot: then the next control point is the new slot's).
        let at_control = |s: &Session| s.phase() == Phase::Human(h);
        if research_needed && at_control(s) {
            if let Some(t) = s.read(|g| self.ai.choose_research(g, h)) {
                self.applied(s, Command::Research { tech: Some(t), shift: false })?;
                self.stats.research += 1;
            }
        }
        // Popups already answered or found with no doable option (those stay open).
        let mut seen: Vec<TriggeredId> = Vec::new();
        while at_control(s) {
            let Some(e) = s.pending().events.into_iter().find(|e| !seen.contains(&e.triggered)) else { break };
            seen.push(e.triggered);
            let options: Vec<EventType> = e.options.iter().filter(|o| o.enabled).map(|o| o.event).collect();
            if options.is_empty() {
                self.stats.events_unanswerable += 1;
                continue;
            }
            let pick = s.read(|g| g.player_get_event_triggered(h, e.triggered).cloned().and_then(|data| self.ai.choose_event(g, h, &data, &options)));
            let event = pick.filter(|ev| options.contains(ev)).unwrap_or(options[0]);
            self.applied(s, Command::Event { triggered: e.triggered, event })?;
            self.stats.events += 1;
        }
        let mut seen: Vec<VoteId> = Vec::new();
        while at_control(s) {
            let Some(v) = s.pending().votes.into_iter().find(|v| !seen.contains(&v.vote)) else { break };
            seen.push(v.vote);
            let pick = s.read(|g| g.vote_triggered(v.vote).cloned().map(|data| self.ai.diplo_vote(g, h, &data.vote_option, data.vote_source)));
            let choice = match pick {
                Some(c) if v.choices.contains(&c) => c,
                _ => *v.choices.iter().find(|&&c| c == PlayerVote::PLAYER_VOTE_ABSTAIN).unwrap_or(&v.choices[0]),
            };
            self.applied(s, Command::Vote { vote: v.vote, choice })?;
            self.stats.votes += 1;
        }
        // Elections (the human is secretary general): the AI's pick (`AI_chooseElection`).
        while at_control(s) {
            let Some(e) = s.pending().elections.into_iter().next() else { break };
            let pick = s.read(|g| g.vote_selection(e.selection).cloned().and_then(|vs| self.ai.choose_election(g, h, &vs)));
            self.applied(s, Command::Election { selection: e.selection, option: pick.filter(|&i| i < e.options.len()) })?;
            self.stats.elections += 1;
        }
        Ok(())
    }
}

/// Positions and moves of `h`'s units ("did the AI move something").
fn fingerprint(g: &Game, h: PlayerId) -> Vec<(UnitId, i32, i32, i32)> {
    g.players[h].units.iter().map(|&u| (u, g.units[u].x, g.units[u].y, g.units[u].moves)).collect()
}

/// `save(load(save)) == save` for the game and the sidecar's JSON round trip.
fn save_round_trip(s: &Session) -> Result<(), String> {
    let (bytes, sidecar) = s.save_bytes().map_err(|e| e.to_string())?;
    let turn = s.g.game_turn();
    let g = Game::load(ffh_data::global(), &bytes).map_err(|e| format!("turn {turn}: the save does not load: {e}"))?;
    if g.save() != bytes {
        return Err(format!("turn {turn}: save(load(save)) != save"));
    }
    if Sidecar::from_json(&sidecar.to_json()).as_ref() != Ok(&sidecar) {
        return Err(format!("turn {turn}: the sidecar does not round-trip"));
    }
    Ok(())
}

/// The AI-only part of a spawned civilization's soak: plays `game.ng`'s settings with AI on
/// every slot (and one more civ) until `spawn_turn`, makes player 0 found `civ` (Infernal: the
/// Infernal Pact tech, whose `onTechAcquired` port spawns Hyborem's Infernals; Mercurians: the
/// Mercurian Gate in its capital, `onBuildingBuilt`), then plays to the next turn boundary.
/// Returns that save, the new player and the game turn.
pub fn spawn_civ(game: &SoakGame, civ: CivilizationType) -> Result<(Vec<u8>, PlayerId, i32), String> {
    let d = ffh_data::global();
    let ng = &game.ng;
    ng.validate(d)?;
    if ng.options.iter().any(|&o| d.type_name(o) == "GAMEOPTION_NO_HYBOREM_OR_BASIUM") {
        return Err("GAMEOPTION_NO_HYBOREM_OR_BASIUM: Infernals and Mercurians never appear".into());
    }
    let mut o = SetupOptions::new(d, ng.opponents + 1, "WORLDSIZE_SMALL", ng.seed);
    o.world_size = ng.world;
    o.game_speed = ng.speed;
    o.handicap = ng.handicap;
    o.era = ng.era;
    o.climate = ng.climate;
    o.sea_level = ng.sea_level;
    o.options = ng.options.clone();
    o.disabled_victories = ng.disabled_victories.clone();
    o.max_turns = ng.max_turns;
    let mut g = Game::new_with_options(d, &o);
    g.map_script = Arc::new(FractalMapScript::default());
    let ai: ControllerRef = ffh_ai::ai_controller();
    for p in g.player_ids() {
        g.controllers[p] = ai.clone();
    }
    // AI-only: the game goes on whoever is alive (as ffh-cli's autoplay).
    g.autorun = true;
    g.game_init();
    let founder = PlayerId(0);
    let mut spawned = None;
    let mut slices = 0u64;
    loop {
        TURN.store(g.game_turn(), Ordering::Relaxed);
        if g.game_state != GameStateType::GAMESTATE_ON {
            return Err(format!("the game ended at turn {} before {} appeared", g.game_turn(), d.type_name(civ)));
        }
        if g.at_turn_boundary() {
            if let Some(p) = spawned {
                if g.players[p].alive {
                    // The take-over stands for FfH's `reassignPlayerAdvanced(founder, p)`, which
                    // gives the slot the human founder's handicap: the human's handicap of a new
                    // game (`setup::build_game`), while every AI slot has the setup's default.
                    g.setup.players[p.idx()].handicap = Some(ng.handicap);
                    return Ok((g.save(), p, g.game_turn()));
                }
            } else if g.game_turn() >= game.spawn_turn {
                if !g.players[founder].alive {
                    return Err(format!("player 0 died before turn {}", game.spawn_turn));
                }
                let before: Vec<PlayerId> = g.player_ids().filter(|&p| g.players[p].ever_alive).collect();
                found_spawned_civ(&mut g, founder, civ)?;
                spawned = g.player_ids().find(|&p| !before.contains(&p) && g.player_get_civilization_type(p) == Some(civ));
                if spawned.is_none() {
                    return Err(format!("{} did not appear at turn {} (no open player slot?)", d.type_name(civ), g.game_turn()));
                }
                continue;
            }
        }
        g.update();
        slices += 1;
        if slices > 50_000_000 {
            return Err("the AI-only game did not progress".into());
        }
    }
}

/// Player `p` founds `civ` as the FfH Python does when its trigger happens.
fn found_spawned_civ(g: &mut Game, p: PlayerId, civ: CivilizationType) -> Result<(), String> {
    let d = g.d;
    match d.type_name(civ) {
        "CIVILIZATION_INFERNAL" => {
            let pact = d.t::<TechType>("TECH_INFERNAL_PACT");
            let team = g.players[p].team;
            if g.team_ids().any(|t| g.team_is_has_tech(t, Some(pact))) {
                return Err("the Infernal Pact is already known".into());
            }
            // CvTeam::setHasTech -> onTechAcquired: the first team with the pact spawns them.
            g.team_set_has_tech(team, pact, true, Some(p), true, true);
        }
        "CIVILIZATION_MERCURIANS" => {
            let gate = d.t::<BuildingType>("BUILDING_MERCURIAN_GATE");
            let city = g.player_get_capital_city(p).ok_or("player 0 has no capital for the Mercurian Gate")?;
            // CvCity::popOrder: setNumRealBuilding, then the buildingBuilt event.
            g.city_set_num_real_building(city, gate, 1);
            g.ev_building_built(city, gate);
        }
        other => return Err(format!("{other} is not spawned mid-game")),
    }
    Ok(())
}

// ---- panics --------------------------------------------------------------------------------------

/// A caught panic.
#[derive(Clone, Debug)]
pub struct Crash {
    pub message: String,
    /// `file:line` (from `crates/`).
    pub location: String,
    /// Frames of the workspace crates (`fn (file:line)`).
    pub frames: Vec<String>,
    pub turn: i32,
}

static PANIC: Mutex<Option<Crash>> = Mutex::new(None);

/// Keeps the frames of the workspace crates (`fn (file:line)`) of a backtrace.
fn ffh_frames(bt: &Backtrace) -> Vec<String> {
    let text = bt.to_string();
    let mut frames = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(l) = lines.next() {
        let Some((_, sym)) = l.trim().split_once(": ") else { continue };
        let at = match lines.peek() {
            Some(n) if n.trim().starts_with("at ") => lines.next().unwrap_or_default().trim().trim_start_matches("at ").to_string(),
            _ => String::new(),
        };
        // The frames below `run_caught` are its callers (the CLI).
        if sym.starts_with("ffh_play::soak::run_caught") {
            break;
        }
        let ours = ["ffh_core", "ffh_ai", "ffh_data", "ffh_play", "<ffh"].iter().any(|c| sym.starts_with(c));
        if ours && !sym.contains("soak::ffh_frames") && !sym.contains("soak::install_hook") && !at.starts_with("/rustc/") {
            let at = at.rsplit_once("/crates/").map_or(at.clone(), |(_, r)| r.to_string());
            let sym = sym.rsplit_once("::h").filter(|(_, h)| h.len() == 16).map_or(sym, |(s, _)| s);
            frames.push(format!("{sym} ({at})"));
        }
    }
    frames.truncate(16);
    frames
}

/// Records every panic (message, location, workspace frames) for [`run_caught`].
pub fn install_hook() {
    panic::set_hook(Box::new(|info| {
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
        let turn = TURN.load(Ordering::Relaxed);
        eprintln!("panic at turn {turn}: {message} at {location}");
        let frames = ffh_frames(&Backtrace::force_capture());
        *PANIC.lock().unwrap_or_else(|e| e.into_inner()) = Some(Crash { message, location, frames, turn });
    }));
}

/// [`run`] with panics caught (installs the panic hook).
pub fn run_caught(game: &SoakGame, opts: &RunOptions, on_turn: &mut dyn FnMut(&Session)) -> Result<Result<Outcome, String>, Crash> {
    install_hook();
    TURN.store(-1, Ordering::Relaxed);
    panic::catch_unwind(AssertUnwindSafe(|| run(game, opts, on_turn))).map_err(|_| {
        PANIC.lock().unwrap_or_else(|e| e.into_inner()).take().unwrap_or(Crash {
            message: "?".into(),
            location: "?".into(),
            frames: Vec::new(),
            turn: TURN.load(Ordering::Relaxed),
        })
    })
}

/// The newest autosave in `<saves>/auto/` (`T####.ffhsave`).
pub fn latest_autosave(saves: &Path) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(saves.join("auto"))
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "ffhsave") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('T')))
        .collect();
    v.sort_by_key(|p| {
        let n = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        (n.len(), n)
    });
    v.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_names() {
        let d = ffh_data::global();
        let args: Vec<String> =
            ["--seed", "7", "--size", "tiny", "--civ", "kuriotates", "--options", "raging+aggressive", "--turns", "40"].iter().map(|s| s.to_string()).collect();
        let g = SoakGame::parse(d, &args).unwrap();
        assert_eq!(g.ng.seed, 7);
        assert_eq!(d.type_name(g.ng.world), "WORLDSIZE_TINY");
        assert_eq!(g.ng.opponents, 2);
        assert_eq!(g.ng.civ.map(|c| d.type_name(c)), Some("CIVILIZATION_KURIOTATES"));
        assert_eq!(g.ng.options.iter().map(|&o| d.type_name(o)).collect::<Vec<_>>(), ["GAMEOPTION_RAGING_BARBARIANS", "GAMEOPTION_AGGRESSIVE_AI"]);
        assert_eq!(SoakGame::parse(d, &g.to_args(d)).unwrap(), g);
        assert_eq!(g.describe(d), "s7 TINYx3 KURIOTATES [RAGING_BARBARIANS,AGGRESSIVE_AI] 40");
        let occ = parse_options(d, "default,occ+GAMEOPTION_ONE_CITY_CHALLENGE").unwrap();
        assert_eq!(occ.iter().map(|&o| d.type_name(o)).collect::<Vec<_>>(), ["GAMEOPTION_ONE_CITY_CHALLENGE", "GAMEOPTION_NO_HEALING_FOR_HUMANS"]);
        assert_eq!(parse_options(d, "default").unwrap(), []);
        let name = g.file_name(d);
        assert!(name.starts_with("s7-tinyx3-kuriotates-raging_barbarians-aggressive_ai-40-") && name.len() == 55 + 1 + 8, "{name}");
        let mut quick = g.clone();
        quick.ng.speed = d.t("GAMESPEED_QUICK");
        assert_eq!(quick.describe(d), "s7 TINYx3 KURIOTATES [RAGING_BARBARIANS,AGGRESSIVE_AI] QUICK 40");
        assert_ne!(quick.file_name(d), name);
        // Spawned civilizations, an explicit opponent count.
        let args: Vec<String> = ["--civ", "mercurian", "--opponents", "3", "--spawn-turn", "12"].iter().map(|s| s.to_string()).collect();
        let g = SoakGame::parse(d, &args).unwrap();
        assert_eq!((g.spawn.map(|c| d.type_name(c)), g.ng.civ, g.ng.opponents, g.spawn_turn), (Some("CIVILIZATION_MERCURIANS"), None, 3, 12));
        assert_eq!(SoakGame::parse(d, &g.to_args(d)).unwrap(), g);
        // Errors.
        let bad_numbers = [&["--seed", "-1"][..], &["--seed", "4294967296"], &["--turns", "4294967297"], &["--opponents", "-2"], &["--spawn-turn", "x"]];
        for bad in [&["--size", "enormous"][..], &["--civ", "a"], &["--turns", "0"], &["--seed"], &["--color", "red"], &["--options", "GAMEOPTION_ADVANCED_START"]].into_iter().chain(bad_numbers) {
            let args: Vec<String> = bad.iter().map(|s| s.to_string()).collect();
            assert!(SoakGame::parse(d, &args).is_err(), "{bad:?}");
        }
    }
}
