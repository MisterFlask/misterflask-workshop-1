//! `ffh`: headless runner for the FFH2 Rust port.
//!
//! Commands:
//!   ffh data                         load data, print a summary and any warnings
//!   ffh lookup <TYPE_NAME>           print the class/index of a type name
//!   ffh autoplay [opts]              play an AI-only game, print per-turn stats and a summary
//!   ffh bench [opts]                 like autoplay, print only timings
//!
//! Options: --seed N  --civs N  --size WORLDSIZE_X  --turns N

mod run;

use std::time::Instant;

fn usage() -> ! {
    eprintln!("usage: ffh <data|lookup TYPE|autoplay|bench> [--seed N] [--civs N] [--size WORLDSIZE_X] [--turns N]");
    std::process::exit(2)
}

fn parse_run_config(args: &[String]) -> run::RunConfig {
    let mut cfg = run::RunConfig::default();
    let mut i = 0;
    while i < args.len() {
        let val = |i: usize| args.get(i + 1).cloned().unwrap_or_else(|| usage());
        match args[i].as_str() {
            "--seed" => cfg.seed = val(i).parse().unwrap_or_else(|_| usage()),
            "--civs" => cfg.civs = val(i).parse().unwrap_or_else(|_| usage()),
            "--size" => cfg.world = val(i),
            "--turns" => cfg.turns = val(i).parse().unwrap_or_else(|_| usage()),
            _ => usage(),
        }
        i += 2;
    }
    cfg
}

fn cmd_autoplay(args: &[String], verbose: bool) {
    let mut cfg = parse_run_config(args);
    cfg.verbose = verbose;
    let t = Instant::now();
    let (mut g, timed) = run::new_game(&cfg);
    let setup_time = t.elapsed();
    let ai_at_setup = timed.total();
    let t = Instant::now();
    let stats = run::autoplay(&mut g, &cfg);
    let play_time = t.elapsed();
    let ai_time = timed.total() - ai_at_setup;
    let n = stats.len().max(1) as f64;
    println!(
        "seed {} civs {} {}: setup {:.1} ms, {} turns in {:.2} s ({:.2} ms/turn, slowest {:.2} ms; {:.0}% inside the AI controller, incl. the moves it orders)",
        cfg.seed,
        cfg.civs,
        cfg.world,
        setup_time.as_secs_f64() * 1000.0,
        stats.len(),
        play_time.as_secs_f64(),
        play_time.as_secs_f64() * 1000.0 / n,
        stats.iter().map(|s| s.elapsed.as_secs_f64() * 1000.0).fold(0.0, f64::max),
        100.0 * ai_time.as_secs_f64() / play_time.as_secs_f64().max(1e-9)
    );
    if let Some(last) = stats.last() {
        println!("final checksum {:08x} at turn {}", last.checksum as u32, last.turn);
    }
    if verbose {
        print!("{}", run::summary(&g));
    }
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
        Some("autoplay") => cmd_autoplay(&args[1..], true),
        Some("bench") => cmd_autoplay(&args[1..], false),
        Some(_) => usage(),
    }
}
