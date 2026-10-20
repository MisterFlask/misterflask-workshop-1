//! `ffh-play`: human play for ffh2-rs (docs/PLAY_DESIGN.md, docs/HUMAN.md).
//!
//! Commands:
//!   ffh-play kinds                     list every named kind (info tables, GlobalTypes lists,
//!                                      hard-coded C++ enums) with its source and count
//!   ffh-play names [KIND] [--pretty]   JSON of the type names of one kind (`UnitType` or
//!                                      `units`), or `{kind: [names]}` for all kinds
//!   ffh-play lookup REF..              resolve type references: a type name (`UNIT_WARRIOR`)
//!                                      or `KIND:INDEX` (`UnitType:44`); one JSON line each,
//!                                      exit code 1 if any does not resolve
//!   ffh-play bot [--seed N] [--turns T] [--opponents N] [--world WORLDSIZE_X] [--saves DIR] [--quiet]
//!                                      a scripted human plays the human slot (design §8 T1);
//!                                      with --saves: journal and autosaves in DIR
//!   ffh-play replay JOURNAL [--from SAVE]
//!                                      replay a journal from its header (a new game), from the
//!                                      save its load / take-over header names, or from an
//!                                      autosave at one of its checkpoints; exit code 1 at a
//!                                      divergence
//!   ffh-play journal print JOURNAL     the journal as lines for people
//!   ffh-play soak run [SETTINGS] [--saves DIR] [--resume SAVE] [--check-every N] [--save-check-every N]
//!                                      the AI plays the human slot for one game (design P2):
//!                                      --seed N --size tiny|WORLDSIZE_X --civ kuriotates|CIVILIZATION_X
//!                                      --opponents N --options raging,aggressive|GAMEOPTION_X,..
//!                                      --turns N --speed --handicap --era --spawn-turn N; a
//!                                      panic is caught and reported with its repro command
//!                                      (exit 101; stuck 4; other failure 5); --panic-at T
//!                                      panics at turn T (a self-test of the report)
//!   ffh-play soak [--seeds A..B] [--sizes ..] [--civs ..] [--options default,raging,..] [--speeds ..] [--turns N]
//!                 [--jobs N] [--timeout SECS] [--saves DIR] [--keep-saves] [--out FILE]
//!                 [--filter TEXT] [--list] [--check-every N] [--save-check-every N]
//!                                      the soak matrix (default: design §7 P2's; duplicates dropped), each
//!                                      game in a child `soak run` (its repro has the check
//!                                      settings); summary on stdout, exit 1 on failures
//!   ffh-play agent [GAME] [--on-eof quit|end-turn] [--progress]
//!                                      JSONL over stdin/stdout (design §4.1): requests in, replies
//!                                      and events out; starts a new game unless --no-game,
//!                                      --load or --take-over; exit 3 after a crash
//!   ffh-play serve [GAME] [--port N] [--bind ADDR] [--assets DIR]
//!                                      the HTTP server (design §4.2) on 127.0.0.1:8642 with the
//!                                      browser client (P5); no game until the client starts
//!                                      one, or GAME says so
//!   ffh-play stage capture|attack|automoves|load|trade|automate|election|settlement|corrupt DIR
//!                                      write a save at a staged situation into DIR (testing the
//!                                      clients): `capture` (a warrior next to an undefended
//!                                      enemy city: moving in raises the keep/raze decision),
//!                                      `attack` (a warrior next to a barbarian warrior: the
//!                                      attack odds), `automoves` (skipping the one ready unit runs the
//!                                      AutoMoves rule), `load` (a warrior and two triremes in a
//!                                      coastal city: the transport picker); prints the save and
//!                                      ids as JSON; writes only DIR/KIND.ffhsave and its sidecar
//!   ffh-play bench [--quick] [--seed N] [--turns T] [--runs R] [--world W] [--opponents N] [--ffh PATH] [--json]
//!                                      T13 (design §8): the session overhead on the AI turns,
//!                                      command latency, the view on a huge map, the QueryGuard;
//!                                      exit code 1 when a budget is missed
//!   ffh-play export-site OUT [--data DIR]
//!                                      the static site's files from this binary (docs/WEB.md):
//!                                      OUT/index.html (loading web/boot.js), the client (app/),
//!                                      the art (art/) and the game data pack (web/data.pak, from
//!                                      DIR or the default data directory)
//!   GAME: [--saves DIR | --no-saves] [--autosaves N] [--setup JSON] [--seed N] [--opponents N]
//!         [--world W] [--civ C] [--leader L] [--handicap H] [--speed S] [--new] [--no-game]
//!         [--load SAVE] [--take-over SAVE --player P] [--panic-at T]
//!                                      saves: a directory per game in DIR (default ./saves);
//!                                      the new-game flags set members of --setup; --panic-at T
//!                                      panics at turn T (a self-test of the crash report)

use ffh_play::jobj;
use ffh_play::json::Json;
use ffh_play::bot::Bot;
use ffh_play::names::{self, Source, KINDS};
use ffh_play::session::ReplayFrom;
use ffh_play::soak::{self, batch, Matrix, RunOptions, SoakGame};
use ffh_play::transport::server::{ServeOptions, Server};
use ffh_play::transport::stdio::{AgentOptions, OnEof};
use ffh_play::transport::{HostOptions, Start};
use ffh_play::{NewGame, Session, SessionOptions, Stop};

fn usage() -> ! {
    eprintln!(
        "usage: ffh-play kinds | names [KIND] [--pretty] | lookup NAME|KIND:INDEX ..\n\
         \x20      | bot [--seed N] [--turns T] [--opponents N] [--world WORLDSIZE_X] [--saves DIR] [--quiet]\n\
         \x20      | replay JOURNAL [--from SAVE] | journal print JOURNAL\n\
         \x20      | soak run [SETTINGS] [--saves DIR] [--resume SAVE] [--check-every N] [--save-check-every N]\n\
         \x20      | soak [--seeds A..B] [--sizes ..] [--civs ..] [--options ..] [--speeds ..] [--turns N] [--jobs N] [--timeout SECS]\n\
         \x20             [--saves DIR] [--keep-saves] [--out FILE] [--filter TEXT] [--list]\n\
         \x20      | agent [GAME] [--on-eof quit|end-turn] [--progress]\n\
         \x20      | serve [GAME] [--port N] [--bind ADDR] [--assets DIR] | stage capture|attack|automoves|load|trade|automate|election|settlement|corrupt DIR\n\
         \x20      | bench [--quick] [--seed N] [--turns T] [--runs R] [--world W] [--opponents N] [--ffh PATH] [--json]\n\
         \x20      | export-site OUT [--data DIR]\n\
         \x20  GAME: [--saves DIR | --no-saves] [--autosaves N] [--setup JSON] [--seed N] [--opponents N] [--world W] [--civ C]\n\
         \x20        [--leader L] [--handicap H] [--speed S] [--new] [--no-game] [--load SAVE] [--take-over SAVE --player P]\n\
         \x20        [--panic-at T]   (see the source header)"
    );
    std::process::exit(2)
}

/// The value after flag `args[*i]`.
fn flag_value<'a>(args: &'a [String], i: &mut usize) -> &'a str {
    *i += 1;
    args.get(*i).map(String::as_str).unwrap_or_else(|| usage())
}

fn cmd_bot(args: &[String]) {
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, 1);
    let mut turns = 100;
    let mut opts = SessionOptions { journal: false, ..Default::default() };
    let mut quiet = false;
    let mut verbose = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => ng.seed = flag_value(args, &mut i).parse().unwrap_or_else(|_| usage()),
            "--turns" => turns = flag_value(args, &mut i).parse().unwrap_or_else(|_| usage()),
            "--opponents" => ng.opponents = flag_value(args, &mut i).parse().unwrap_or_else(|_| usage()),
            "--world" => {
                let w = flag_value(args, &mut i);
                ng.world = d.ty(w).unwrap_or_else(|| {
                    eprintln!("unknown world size {w:?}");
                    std::process::exit(2)
                })
            }
            "--saves" => {
                opts.saves_dir = Some(flag_value(args, &mut i).into());
                opts.journal = true;
            }
            "--quiet" => quiet = true,
            "--verbose" => verbose = true,
            _ => usage(),
        }
        i += 1;
    }
    let mut bot = Bot::new(ng.seed);
    let mut s = Session::new_game(&ng, bot.answerer(), opts).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let until = s.g.game_turn() + turns;
    let t0 = std::time::Instant::now();
    let stop = ffh_play::bot::run(&mut s, &mut bot, until, |s, _| {
        if !quiet {
            let h = s.human;
            let pl = &s.g.players[h];
            println!(
                "turn {:4} cities {:2} units {:3} techs {:3} gold {:5} messages {}",
                s.g.game_turn(),
                pl.cities.len(),
                pl.units.len(),
                s.g.teams[pl.team].has_tech.iter().filter(|&&b| b).count(),
                pl.gold,
                s.messages().len()
            );
            if verbose {
                // What an agent reads: the `text` query.
                if let Ok(t) = s.query(&ffh_play::Query::Text) {
                    for line in t.get("text").and_then(Json::as_str).unwrap_or_default().lines() {
                        println!("    {line}");
                    }
                }
            }
        }
    });
    println!("stopped: {stop:?} at turn {} after {:.1} s", s.g.game_turn(), t0.elapsed().as_secs_f64());
    println!("bot: {:?}", bot.stats);
    println!("session: {:?}", s.stats());
    if let Some(p) = s.journal_path() {
        println!("journal: {}", p.display());
    }
    if let Some(p) = s.last_autosave() {
        println!("last autosave: {}", p.save.display());
    }
    if matches!(stop, Stop::Stuck(_)) {
        std::process::exit(1);
    }
}

fn cmd_replay(args: &[String]) {
    let mut journal = None;
    let mut from = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--from" => from = Some(flag_value(args, &mut i).to_string()),
            a if journal.is_none() && !a.starts_with('-') => journal = Some(a.to_string()),
            _ => usage(),
        }
        i += 1;
    }
    let Some(journal) = journal else { usage() };
    let fail = |e: String| -> ! {
        eprintln!("replay: {e}");
        std::process::exit(1)
    };
    let lines = ffh_play::journal::read_file(std::path::Path::new(&journal)).unwrap_or_else(|e| fail(e));
    let bundle = from.map(|f| ffh_play::save::read_bundle(std::path::Path::new(&f)).unwrap_or_else(|e| fail(e)));
    let from = bundle.as_ref().map(|(save, sidecar)| ReplayFrom { save, sidecar: sidecar.as_ref() });
    let n = lines.len();
    let s = ffh_play::session::replay(lines, from, SessionOptions::default()).unwrap_or_else(|e| fail(e));
    println!(
        "replayed {n} journal lines: turn {} slice {}, checksum {:08x}",
        s.g.game_turn(),
        s.g.turn_slice,
        s.g.calculate_sync_checksum() as u32
    );
}

fn cmd_journal(args: &[String]) {
    let [sub, path] = args else { usage() };
    if sub != "print" {
        usage();
    }
    let d = ffh_data::global();
    let lines = ffh_play::journal::read_file(std::path::Path::new(path)).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    for l in &lines {
        println!("{}", ffh_play::journal::pretty(d, l));
    }
}

/// `soak run`: one game in this process (the batch's child).
fn cmd_soak_run(args: &[String]) {
    let d = ffh_data::global();
    let mut opts = RunOptions::default();
    let mut settings = Vec::new();
    let fail = |e: String| -> ! {
        eprintln!("soak run: {e}");
        std::process::exit(2)
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--saves" => opts.saves = Some(flag_value(args, &mut i).into()),
            "--resume" => opts.resume = Some(flag_value(args, &mut i).into()),
            k @ "--check-every" => opts.check_every = soak::parse_every(k, flag_value(args, &mut i)).unwrap_or_else(|e| fail(e)),
            k @ "--save-check-every" => opts.save_check_every = soak::parse_every(k, flag_value(args, &mut i)).unwrap_or_else(|e| fail(e)),
            "--panic-at" => opts.panic_at = Some(flag_value(args, &mut i).parse().unwrap_or_else(|_| usage())),
            _ => {
                settings.push(args[i].clone());
                settings.push(flag_value(args, &mut i).to_string());
            }
        }
        i += 1;
    }
    let game = SoakGame::parse(d, &settings).unwrap_or_else(|e| fail(e));
    // The game and the check settings (a check runs only on the turns they name); not
    // `--panic-at`, a self-test of this report.
    let mut repro = format!("ffh-play soak run {}", game.to_args(d).into_iter().chain(opts.check_args()).collect::<Vec<_>>().join(" "));
    let r = soak::run_caught(&game, &opts, &mut |s| println!("PROGRESS\t{}", s.g.game_turn()));
    if let Some(dir) = &opts.saves {
        if let Some(a) = soak::latest_autosave(dir) {
            repro += &format!(" --resume {}", a.display());
        }
    } else if let Some(r) = &opts.resume {
        repro += &format!(" --resume {}", r.display());
    }
    match r {
        Ok(Ok(o)) => {
            let st = &o.stats;
            println!("TURN\t{}", o.turn);
            println!("END\t{} ({:.1} s, checksum {:08x}, save {})", o.end_text(d), o.secs, o.checksum as u32, o.save_fnv);
            println!("CITIES\t{}\t{}", o.human_cities, o.ai_cities.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","));
            println!(
                "STATS\thuman p{} {}{}: cities {} vs AI {:?}; turns {} prod {} auto {} research {} events {} (closed {}) votes {} left open {} (research {}) gate {}; decisions {:?} events deferred {} research AI {} moved {}; checks {}+{}{}",
                o.human.0,
                o.human_civ.map_or("?", |c| d.type_name(c)),
                match game.civ() {
                    Some(c) if Some(c) != o.human_civ => format!(" (started as {})", d.type_name(c)),
                    _ => String::new(),
                },
                o.human_cities,
                o.ai_cities,
                st.turns_played,
                st.productions,
                st.automated_cities,
                st.research,
                st.events,
                st.events_unanswerable,
                st.votes,
                st.popups_left,
                st.research_left_open,
                st.gate_confirms,
                o.session.decisions,
                o.session.events_deferred,
                o.session.research_ai_picks,
                o.session.reassigned,
                st.checks,
                st.save_checks,
                st.taken_over_at.map_or(String::new(), |t| format!("; taken over at {t}"))
            );
            if let soak::End::Stuck(diag) = &o.end {
                println!("STUCK\t{diag}");
                println!("REPRO\t{repro}");
                std::process::exit(4);
            }
        }
        Ok(Err(e)) => {
            println!("ERROR\t{e}");
            println!("REPRO\t{repro}");
            std::process::exit(5);
        }
        Err(c) => {
            println!("TURN\t{}", c.turn);
            println!("PANIC\t{} at {}", c.message.replace('\n', " / "), c.location);
            for f in &c.frames {
                println!("BT\t{f}");
            }
            println!("REPRO\t{repro}");
            std::process::exit(101);
        }
    }
}

/// `soak [matrix]`: the soak matrix, each game in a child process.
fn cmd_soak(args: &[String]) {
    if args.first().map(String::as_str) == Some("run") {
        return cmd_soak_run(&args[1..]);
    }
    let d = ffh_data::global();
    let mut m = Matrix::design(d);
    let mut o = batch::BatchOptions {
        jobs: std::thread::available_parallelism().map_or(1, |n| n.get()),
        timeout: std::time::Duration::from_secs(1800),
        saves: None,
        keep_saves: false,
        extra: Vec::new(),
    };
    let mut out = None;
    let mut filters: Vec<String> = Vec::new();
    let mut list = false;
    let fail = |e: String| -> ! {
        eprintln!("soak: {e}");
        std::process::exit(2)
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seeds" => m.seeds = batch::parse_seeds(flag_value(args, &mut i)).unwrap_or_else(|e| fail(e)),
            "--sizes" => {
                m.sizes = flag_value(args, &mut i).split(',').map(|s| soak::lenient(d, "WORLDSIZE_", s)).collect::<Result<_, _>>().unwrap_or_else(|e| fail(e))
            }
            "--civs" => {
                m.civs = flag_value(args, &mut i).split(',').map(|s| soak::lenient(d, "CIVILIZATION_", s)).collect::<Result<_, _>>().unwrap_or_else(|e| fail(e))
            }
            "--options" => {
                m.options = flag_value(args, &mut i).split(',').map(|s| soak::parse_options(d, s)).collect::<Result<_, _>>().unwrap_or_else(|e| fail(e))
            }
            "--speeds" => {
                m.speeds = flag_value(args, &mut i).split(',').map(|s| soak::lenient(d, "GAMESPEED_", s)).collect::<Result<_, _>>().unwrap_or_else(|e| fail(e))
            }
            "--turns" => m.turns = flag_value(args, &mut i).parse().unwrap_or_else(|_| usage()),
            "--jobs" => o.jobs = flag_value(args, &mut i).parse().unwrap_or_else(|_| usage()),
            "--timeout" => o.timeout = std::time::Duration::from_secs(flag_value(args, &mut i).parse().unwrap_or_else(|_| usage())),
            "--saves" => o.saves = Some(flag_value(args, &mut i).into()),
            "--keep-saves" => o.keep_saves = true,
            "--out" => out = Some(flag_value(args, &mut i).to_string()),
            "--filter" => filters.push(flag_value(args, &mut i).to_string()),
            "--list" => list = true,
            k @ ("--check-every" | "--save-check-every") => {
                // Checked here, so that a bad value is a usage error, not a failure of every game.
                let n = soak::parse_every(k, flag_value(args, &mut i)).unwrap_or_else(|e| fail(e));
                o.extra.extend([k.to_string(), n.to_string()]);
            }
            _ => usage(),
        }
        i += 1;
    }
    if m.turns < 1 {
        fail("--turns must be at least 1".into());
    }
    let mut games = m.games(d);
    games.retain(|g| filters.iter().all(|f| g.describe(d).contains(f.as_str())));
    // Every game's settings as its child reads them (e.g. a game option a new game refuses).
    for g in &games {
        if let Err(e) = SoakGame::parse(d, &g.to_args(d)) {
            fail(format!("{}: {e}", g.describe(d)));
        }
    }
    if list {
        for g in &games {
            println!("ffh-play soak run {}", g.to_args(d).join(" "));
        }
        return;
    }
    // The TSV file is created before the games run: a bad path must not cost the batch.
    let mut out = out.map(|path| {
        let f = std::fs::File::create(&path).unwrap_or_else(|e| fail(format!("{path}: {e}")));
        (path, f)
    });
    let exe = std::env::current_exe().unwrap_or_else(|e| fail(format!("current_exe: {e}")));
    let t0 = std::time::Instant::now();
    let results = batch::run_all(d, &exe, games, &o);
    print!("{}", batch::summary(d, &results, t0.elapsed()));
    let failed = results.iter().any(|r| r.status != "ok");
    if let Some((path, f)) = &mut out {
        use std::io::Write;
        if let Err(e) = f.write_all(batch::tsv(d, &results).as_bytes()) {
            eprintln!("soak: {path}: {e}");
            // Failed games decide the exit code first.
            std::process::exit(if failed { 1 } else { 2 });
        }
    }
    if failed {
        std::process::exit(1);
    }
}

/// The game options `agent` and `serve` share (see the source header).
struct GameArgs {
    host: HostOptions,
    /// `--setup` with the new-game flags merged in.
    setup: Json,
    /// A new-game flag was given (`--new`, `--setup`, `--seed`, ...).
    new: bool,
    no_game: bool,
    load: Option<std::path::PathBuf>,
    take_over: Option<std::path::PathBuf>,
    player: Option<u8>,
}

impl GameArgs {
    fn new() -> GameArgs {
        GameArgs { host: HostOptions::default(), setup: Json::obj(), new: false, no_game: false, load: None, take_over: None, player: None }
    }

    /// Takes the flag at `args[*i]` (and its value) if it is a game option.
    fn take(&mut self, args: &[String], i: &mut usize) -> bool {
        let fail = |e: String| -> ! {
            eprintln!("ffh-play: {e}");
            std::process::exit(2)
        };
        let flag = args[*i].as_str();
        match flag {
            "--saves" => self.host.saves = Some(flag_value(args, i).into()),
            "--no-saves" => self.host.saves = None,
            "--autosaves" => self.host.autosave_keep = flag_value(args, i).parse().unwrap_or_else(|_| usage()),
            "--panic-at" => self.host.panic_at = Some(flag_value(args, i).parse().unwrap_or_else(|_| usage())),
            "--setup" => {
                let j = Json::parse(flag_value(args, i)).unwrap_or_else(|e| fail(format!("--setup: {e}")));
                if j.as_obj().is_none() {
                    fail("--setup: expected a JSON object".into());
                }
                for (k, v) in j.as_obj().unwrap_or_default() {
                    self.setup.set(k, v.clone());
                }
                self.new = true;
            }
            "--seed" | "--opponents" => {
                let v: i64 = flag_value(args, i).parse().unwrap_or_else(|_| usage());
                self.setup.set(&flag[2..], v);
                self.new = true;
            }
            "--world" | "--civ" | "--leader" | "--handicap" | "--speed" => {
                let v = flag_value(args, i).to_string();
                self.setup.set(&flag[2..], v);
                self.new = true;
            }
            "--new" => self.new = true,
            "--no-game" => self.no_game = true,
            "--load" => self.load = Some(flag_value(args, i).into()),
            "--take-over" => self.take_over = Some(flag_value(args, i).into()),
            "--player" => self.player = Some(flag_value(args, i).parse().unwrap_or_else(|_| usage())),
            _ => return false,
        }
        true
    }

    /// How to start; `default_new`: a new game when nothing is said.
    fn start(&self, default_new: bool) -> Start {
        let picked = [self.new, self.no_game, self.load.is_some(), self.take_over.is_some()].iter().filter(|&&b| b).count();
        if picked > 1 || self.player.is_some() != self.take_over.is_some() {
            eprintln!("ffh-play: give one of the new-game flags, --no-game, --load SAVE or --take-over SAVE --player P");
            std::process::exit(2);
        }
        if let Some(p) = &self.load {
            Start::Load(p.clone())
        } else if let (Some(p), Some(player)) = (&self.take_over, self.player) {
            Start::TakeOver(p.clone(), ffh_core::ids::PlayerId(player))
        } else if self.new || (default_new && !self.no_game) {
            Start::New(self.setup.clone())
        } else {
            Start::None
        }
    }
}

fn cmd_agent(args: &[String]) {
    let mut game = GameArgs::new();
    let mut on_eof = OnEof::Quit;
    let mut progress = false;
    let mut i = 0;
    while i < args.len() {
        if !game.take(args, &mut i) {
            match args[i].as_str() {
                "--on-eof" => {
                    on_eof = match flag_value(args, &mut i) {
                        "quit" => OnEof::Quit,
                        "end-turn" => OnEof::EndTurn,
                        _ => usage(),
                    }
                }
                "--progress" => progress = true,
                _ => usage(),
            }
        }
        i += 1;
    }
    let opts = AgentOptions { start: game.start(true), host: game.host, on_eof, progress };
    let stdin = std::io::BufReader::new(std::io::stdin());
    let code = ffh_play::transport::stdio::run(Box::new(stdin), Box::new(std::io::stdout()), &opts);
    std::process::exit(code);
}

fn cmd_serve(args: &[String]) {
    let mut game = GameArgs::new();
    let mut opts = ServeOptions::default();
    let mut i = 0;
    while i < args.len() {
        if !game.take(args, &mut i) {
            match args[i].as_str() {
                "--port" => opts.bind.set_port(flag_value(args, &mut i).parse().unwrap_or_else(|_| usage())),
                "--bind" => {
                    let v = flag_value(args, &mut i);
                    opts.bind = match v.parse::<std::net::SocketAddr>() {
                        Ok(a) => a,
                        Err(_) => std::net::SocketAddr::new(v.parse().unwrap_or_else(|_| usage()), opts.bind.port()),
                    }
                }
                "--assets" => opts.assets = Some(flag_value(args, &mut i).into()),
                _ => usage(),
            }
        }
        i += 1;
    }
    opts.start = game.start(false);
    let saves = game.host.saves.as_ref().map_or("none (--no-saves)".to_string(), |d| d.display().to_string());
    opts.host = game.host;
    let server = Server::start(opts).unwrap_or_else(|e| {
        eprintln!("ffh-play serve: {e}");
        std::process::exit(2)
    });
    println!("ffh-play: serving http://{}/ (saves: {saves}); stop with Ctrl-C or the quit request", server.addr());
    server.wait();
}

fn cmd_kinds() {
    let d = ffh_data::global();
    for k in KINDS {
        let source = match k.source {
            Source::Table { field, class } => format!("table {field} ({class})"),
            Source::Global { class } => format!("GlobalTypes {class}"),
            Source::Enum => "C++ enum".to_string(),
        };
        println!("{:<24} {:>5}  {source}", k.kind, (k.count)(d));
    }
}

fn cmd_names(args: &[String]) {
    let d = ffh_data::global();
    let mut pretty = false;
    let mut kind = None;
    for a in args {
        match a.as_str() {
            "--pretty" => pretty = true,
            k if kind.is_none() && !k.starts_with('-') => kind = Some(k),
            _ => usage(),
        }
    }
    let j = match kind {
        None => names::names_json(d),
        Some(k) => match names::kind(k) {
            Some(k) => names::kind_names_json(d, k),
            None => {
                eprintln!("unknown kind {k:?} (see `ffh-play kinds`)");
                std::process::exit(1)
            }
        },
    };
    println!("{}", if pretty { j.to_pretty() } else { j.to_string() });
}

/// One type reference -> `{"ref","kind","index","name","display","text_key"}` or `{"ref","error"}`.
fn lookup_one(d: &ffh_data::GameData, r: &str) -> Result<Json, String> {
    let (k, i) = if let Some((kind, idx)) = r.split_once(':') {
        let k = names::kind(kind).ok_or_else(|| format!("unknown kind {kind:?}"))?;
        let i: usize = idx.parse().map_err(|_| format!("bad index {idx:?}"))?;
        if i >= (k.count)(d) {
            return Err(format!("{} index {i} out of range 0..{}", k.kind, (k.count)(d)));
        }
        (k, i)
    } else {
        match names::find(d, r).as_slice() {
            [] => return Err("unknown type name".into()),
            [(k, i)] => (*k, *i),
            many => {
                let kinds: Vec<&str> = many.iter().map(|(k, _)| k.kind).collect();
                return Err(format!("ambiguous: {}", kinds.join(", ")));
            }
        }
    };
    Ok(jobj! {
        "ref" => r,
        "kind" => k.kind,
        "index" => i,
        "name" => (k.name)(d, i),
        "display" => k.display(d, i).into_owned(),
        "text_key" => (k.text_key)(d, i),
    })
}

fn cmd_lookup(args: &[String]) {
    if args.is_empty() {
        usage();
    }
    let d = ffh_data::global();
    let mut failed = false;
    for r in args {
        match lookup_one(d, r) {
            Ok(j) => println!("{j}"),
            Err(e) => {
                failed = true;
                println!("{}", jobj! { "ref" => r.as_str(), "error" => e });
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}

/// `ffh-play stage capture|attack|automoves|load|trade|automate|election|settlement|corrupt DIR`: a save
/// at a staged situation (testing the clients).
fn cmd_stage(args: &[String]) {
    let [kind, dir] = args else { usage() };
    let dir = std::path::Path::new(dir);
    let out = match kind.as_str() {
        "capture" => ffh_play::stage::capture(dir).map(|s| s.to_json()),
        "attack" => ffh_play::stage::attack(dir).map(|s| s.to_json()),
        "automoves" => ffh_play::stage::auto_moves(dir).map(|s| s.to_json()),
        "load" => ffh_play::stage::load(dir).map(|s| s.to_json()),
        "trade" => ffh_play::stage::trade(dir).map(|s| s.to_json()),
        "automate" => ffh_play::stage::automate(dir).map(|s| s.to_json()),
        "election" => ffh_play::stage::election(dir).map(|s| s.to_json()),
        "settlement" => ffh_play::stage::settlement(dir).map(|s| s.to_json()),
        "corrupt" => ffh_play::stage::corrupt(dir).map(|s| s.to_json()),
        _ => usage(),
    };
    match out {
        Ok(j) => println!("{j}"),
        Err(e) => {
            eprintln!("ffh-play stage {kind}: {e}");
            std::process::exit(1)
        }
    }
}

/// `ffh-play bench [--quick] [--seed N] [--turns T] [--runs R] [--world W] [--opponents N]
/// [--ffh PATH] [--json]`: T13 (design §8): the session overhead on the AI turns (`--runs`
/// rounds of the three ways), command latency, the view build on a huge map, the QueryGuard.
/// Exit code 1 when a budget is missed.
fn cmd_bench(args: &[String]) {
    let mut o = if args.iter().any(|a| a == "--quick") { ffh_play::bench::BenchOptions::quick() } else { ffh_play::bench::BenchOptions::default() };
    // `ffh bench` for the reference figure: next to this binary unless --ffh says otherwise.
    o.ffh = std::env::current_exe().ok().map(|p| p.with_file_name("ffh"));
    o.verbose = true;
    let mut json = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--quick" => {}
            "--json" => json = true,
            "--seed" => o.seed = flag_value(args, &mut i).parse().unwrap_or_else(|_| usage()),
            "--turns" => o.turns = flag_value(args, &mut i).parse().unwrap_or_else(|_| usage()),
            "--runs" => o.overhead_runs = flag_value(args, &mut i).parse().unwrap_or_else(|_| usage()),
            "--opponents" => o.opponents = flag_value(args, &mut i).parse().unwrap_or_else(|_| usage()),
            "--world" => o.world = flag_value(args, &mut i).to_string(),
            "--ffh" => o.ffh = Some(flag_value(args, &mut i).into()),
            _ => usage(),
        }
        i += 1;
    }
    let d = ffh_data::global();
    if d.ty::<ffh_data::WorldSizeType>(&o.world).is_none() || o.turns < 1 || o.overhead_runs < 1 {
        usage();
    }
    let r = ffh_play::bench::run(&o);
    if json {
        println!("{}", r.to_json());
    } else {
        print!("{}", ffh_play::bench::render(&o, &r));
    }
    let failures = r.failures();
    for f in &failures {
        eprintln!("bench: over budget: {f}");
    }
    if !failures.is_empty() {
        std::process::exit(1);
    }
}

/// `ffh-play export-site OUT [--data DIR]`: the static site's files that come from this binary
/// (`ffh_play::site`).
fn cmd_export_site(args: &[String]) {
    let mut out = None;
    let mut data = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--data" => data = Some(std::path::PathBuf::from(flag_value(args, &mut i))),
            a if out.is_none() && !a.starts_with('-') => out = Some(std::path::PathBuf::from(a)),
            _ => usage(),
        }
        i += 1;
    }
    let Some(out) = out else { usage() };
    let data = data.unwrap_or_else(ffh_data::default_data_dir);
    match ffh_play::site::export_site(&out, &data) {
        Ok(e) => println!("ffh-play export-site: {} files in {} (data pack: {} files, {} bytes, from {})", e.files, out.display(), e.data_files, e.pack_bytes, data.display()),
        Err(e) => {
            eprintln!("ffh-play export-site: {e}");
            std::process::exit(1)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { usage() };
    match cmd.as_str() {
        "bench" => cmd_bench(&args[1..]),
        "kinds" => cmd_kinds(),
        "names" => cmd_names(&args[1..]),
        "lookup" => cmd_lookup(&args[1..]),
        "bot" => cmd_bot(&args[1..]),
        "replay" => cmd_replay(&args[1..]),
        "journal" => cmd_journal(&args[1..]),
        "soak" => cmd_soak(&args[1..]),
        "agent" => cmd_agent(&args[1..]),
        "serve" => cmd_serve(&args[1..]),
        "stage" => cmd_stage(&args[1..]),
        "export-site" => cmd_export_site(&args[1..]),
        _ => usage(),
    }
}
