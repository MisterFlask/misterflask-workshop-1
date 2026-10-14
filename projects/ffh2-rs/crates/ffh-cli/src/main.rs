//! `ffh`: headless runner for the FFH2 Rust port.
//!
//! Commands:
//!   ffh data                         load data, print a summary and any warnings
//!   ffh lookup <TYPE_NAME>           print the class/index of a type name
//!
//! (Autoplay and benchmark commands are added once the rules engine runs.)

use std::time::Instant;

fn usage() -> ! {
    eprintln!("usage: ffh <data|lookup TYPE> [options]");
    std::process::exit(2)
}

fn cmd_data() {
    let t = Instant::now();
    let d = ffh_data::global();
    println!(
        "loaded data in {:?}: {} units, {} promotions, {} spells, {} buildings, {} techs, {} civs, {} leaders; {} warnings",
        t.elapsed(),
        d.units.len(),
        d.promotions.len(),
        d.spells.len(),
        d.buildings.len(),
        d.techs.len(),
        d.civilizations.len(),
        d.leader_heads.len(),
        d.warnings.len()
    );
    for w in d.warnings.iter().take(50) {
        println!("  warn: {w}");
    }
}

fn cmd_lookup(name: &str) {
    let d = ffh_data::global();
    match d.reg.entry(name) {
        Some(e) => println!("{name}: {}[{}]", e.class, e.index),
        None => println!("{name}: unknown"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        None | Some("data") => cmd_data(),
        Some("lookup") => cmd_lookup(args.get(1).map(|s| s.as_str()).unwrap_or_else(|| usage())),
        Some(_) => usage(),
    }
}
