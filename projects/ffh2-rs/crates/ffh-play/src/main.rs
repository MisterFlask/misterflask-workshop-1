//! `ffh-play`: human play for ffh2-rs (docs/PLAY_DESIGN.md, docs/HUMAN.md).
//!
//! Commands (phase P0; serve, agent, bot, soak, replay, journal and bench come with later phases):
//!   ffh-play kinds                     list every named kind (info tables, GlobalTypes lists,
//!                                      hard-coded C++ enums) with its source and count
//!   ffh-play names [KIND] [--pretty]   JSON of the type names of one kind (`UnitType` or
//!                                      `units`), or `{kind: [names]}` for all kinds
//!   ffh-play lookup REF..              resolve type references: a type name (`UNIT_WARRIOR`)
//!                                      or `KIND:INDEX` (`UnitType:44`); one JSON line each,
//!                                      exit code 1 if any does not resolve

use ffh_play::jobj;
use ffh_play::json::Json;
use ffh_play::names::{self, Source, KINDS};

fn usage() -> ! {
    eprintln!("usage: ffh-play kinds | names [KIND] [--pretty] | lookup NAME|KIND:INDEX ..");
    std::process::exit(2)
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
        _ => usage(),
    }
}
