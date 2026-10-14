//! `ffh-play`: human play for ffh2-rs (docs/PLAY_DESIGN.md, docs/HUMAN.md).
//!
//! Commands (serve, agent, soak and bench come with later phases):
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

use ffh_play::jobj;
use ffh_play::json::Json;
use ffh_play::bot::Bot;
use ffh_play::names::{self, Source, KINDS};
use ffh_play::session::ReplayFrom;
use ffh_play::{NewGame, Session, SessionOptions, Stop};

fn usage() -> ! {
    eprintln!(
        "usage: ffh-play kinds | names [KIND] [--pretty] | lookup NAME|KIND:INDEX ..\n\
         \x20      | bot [--seed N] [--turns T] [--opponents N] [--world WORLDSIZE_X] [--saves DIR] [--quiet]\n\
         \x20      | replay JOURNAL [--from SAVE] | journal print JOURNAL"
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
                s.read(|g| {
                    for &c in &pl.cities {
                        let city = &g.cities[c];
                        let head = city.order_queue.first().map_or("-".to_string(), |o| format!("{:?} {}", o.order_type, o.data1));
                        let (prod, need) = if city.order_queue.is_empty() { (0, 0) } else { (g.city_production(c), g.city_production_needed(c)) };
                        println!("    city {} size {} {head} {prod}/{need} (+{})", city.name, city.population, g.city_current_production_difference(c, false, true));
                    }
                    for &u in &pl.units {
                        let unit = &g.units[u];
                        println!("    unit {} {} at ({}, {}) group {:?}", u.0, g.d.type_name(unit.unit_type), unit.x, unit.y, unit.group);
                    }
                });
                for e in s.pending().events {
                    println!("    event {}: {}", e.triggered.0, e.text);
                    for o in &e.options {
                        println!("      {} {}: {}", if o.enabled { "+" } else { "-" }, names::name(s.g.d, o.event), o.effects.join(", "));
                    }
                }
                for m in s.messages().iter().rev().take(3) {
                    println!("    msg t{}: {}", m.turn, m.text);
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { usage() };
    match cmd.as_str() {
        "kinds" => cmd_kinds(),
        "names" => cmd_names(&args[1..]),
        "lookup" => cmd_lookup(&args[1..]),
        "bot" => cmd_bot(&args[1..]),
        "replay" => cmd_replay(&args[1..]),
        "journal" => cmd_journal(&args[1..]),
        _ => usage(),
    }
}
