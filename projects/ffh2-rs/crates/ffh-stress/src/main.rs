//! `ffh-stress`: plays batches of AI-only games over a matrix of game settings (speed,
//! handicap, era, world size, number of civs, climate, sea level, game options, teams, victory
//! settings) and reports every crash.
//!
//! Commands:
//!   ffh-stress run [settings]        play one game in this process; a panic is caught
//!                                    (`catch_unwind`) and reported with its location and the
//!                                    ffh frames of its backtrace
//!   ffh-stress batch [opts]          play every game of a matrix, each in a child `run`
//!                                    process (so aborts, stack overflows and hangs are caught
//!                                    too), N at a time, then print a summary
//!   ffh-stress list [opts]           print the games of a matrix (one `run` command each)
//!
//! FFH_STRESS_LOG=1 makes `run` print the game log (LOG lines) turn by turn.
//! `run` cross-checks the rules' redundant bookkeeping (`check.rs`) every 5 turns
//! (FFH_STRESS_CHECK=N: every N turns, 0 = never).
//!
//! Settings (`run`): --seed N --civs N --size WORLDSIZE_X --speed GAMESPEED_X
//!   --handicap HANDICAP_X (game handicap) --player-handicap HANDICAP_X (AI slots)
//!   --era ERA_X --climate CLIMATE_X --sea SEALEVEL_X --options GAMEOPTION_A,GAMEOPTION_B
//!   --teams N (consecutive civs in teams of N) --turns N --max-turns N
//!   --target-score N (score victory at N basis points of the world: 5000 = half; 0 = off)
//!   --max-city-elimination N --no-victory VICTORY_A,VICTORY_B --spare N
//!
//! Batch options: --matrix NAME[,NAME..] (speed handicap era world civs climate sea options
//!   teams victory random all; default all) --seeds 1,2 --turns N --jobs N --timeout SECS
//!   --random N (games in the random matrix) --out FILE (TSV of all results)
//!   --filter TEXT (only games whose settings contain TEXT)

mod check;
mod matrix;
mod play;
mod settings;

use std::io::{BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use settings::Settings;

fn usage() -> ! {
    eprintln!("usage: ffh-stress <run [settings] | batch [opts] | list [opts]> (see the source header)");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => {
            let s = Settings::parse(&args[1..]).unwrap_or_else(|e| {
                eprintln!("{e}");
                usage()
            });
            let r = play::run_caught(&s);
            print!("{}", r.to_lines());
            std::process::exit(if r.panic.is_some() { 101 } else { 0 });
        }
        Some("batch") => batch(&args[1..], false),
        Some("list") => batch(&args[1..], true),
        _ => usage(),
    }
}

struct BatchOpts {
    matrices: Vec<String>,
    seeds: Vec<u32>,
    turns: i32,
    jobs: usize,
    timeout: Duration,
    random: usize,
    out: Option<String>,
    filter: Vec<String>,
}

fn parse_batch(args: &[String]) -> BatchOpts {
    let mut o = BatchOpts {
        matrices: vec!["all".into()],
        seeds: vec![1, 2],
        turns: 400,
        jobs: std::thread::available_parallelism().map_or(1, |n| n.get()),
        timeout: Duration::from_secs(1800),
        random: 40,
        out: None,
        filter: Vec::new(),
    };
    let mut i = 0;
    while i < args.len() {
        let val = args.get(i + 1).cloned().unwrap_or_else(|| usage());
        match args[i].as_str() {
            "--matrix" => o.matrices = val.split(',').map(String::from).collect(),
            "--seeds" => o.seeds = val.split(',').map(|s| s.parse().unwrap_or_else(|_| usage())).collect(),
            "--turns" => o.turns = val.parse().unwrap_or_else(|_| usage()),
            "--jobs" => o.jobs = val.parse().unwrap_or_else(|_| usage()),
            "--timeout" => o.timeout = Duration::from_secs(val.parse().unwrap_or_else(|_| usage())),
            "--random" => o.random = val.parse().unwrap_or_else(|_| usage()),
            "--out" => o.out = Some(val),
            "--filter" => o.filter.push(val),
            _ => usage(),
        }
        i += 2;
    }
    o
}

/// Outcome of one child process.
struct Done {
    settings: Settings,
    status: String,
    turn: String,
    end: String,
    detail: String,
    secs: f64,
}

fn batch(args: &[String], list_only: bool) {
    let o = parse_batch(args);
    let d = ffh_data::global();
    let mut games = Vec::new();
    for m in &o.matrices {
        games.extend(matrix::build(d, m, &o.seeds, o.turns, o.random).unwrap_or_else(|e| {
            eprintln!("{e}");
            usage()
        }));
    }
    games.retain(|g| o.filter.iter().all(|f| g.describe().contains(f.as_str())));
    if list_only {
        for g in &games {
            println!("ffh-stress run {}", g.to_args().join(" "));
        }
        return;
    }
    let exe = std::env::current_exe().expect("current_exe");
    let n = games.len();
    eprintln!("{n} games, {} jobs, timeout {} s", o.jobs, o.timeout.as_secs());
    let games = Arc::new(games);
    let next = Arc::new(AtomicUsize::new(0));
    let results: Arc<Mutex<Vec<Option<Done>>>> = Arc::new(Mutex::new((0..n).map(|_| None).collect()));
    let t0 = Instant::now();
    let mut workers = Vec::new();
    for _ in 0..o.jobs.max(1) {
        let (games, next, results, exe) = (games.clone(), next.clone(), results.clone(), exe.clone());
        let timeout = o.timeout;
        workers.push(std::thread::spawn(move || loop {
            let i = next.fetch_add(1, Ordering::SeqCst);
            if i >= games.len() {
                break;
            }
            let done = run_child(&exe, &games[i], timeout);
            eprintln!(
                "[{:4}/{n} {:6.0}s] {:7} turn {:>5} {:>7.1}s  {}  {}",
                i + 1,
                t0.elapsed().as_secs_f64(),
                done.status,
                done.turn,
                done.secs,
                done.settings.describe(),
                if done.status == "ok" { done.end.clone() } else { done.detail.lines().next().unwrap_or("").to_string() }
            );
            results.lock().unwrap()[i] = Some(done);
        }));
    }
    for w in workers {
        w.join().unwrap();
    }
    let results: Vec<Done> = std::mem::take(&mut *results.lock().unwrap()).into_iter().map(Option::unwrap).collect();
    summary(&results, t0.elapsed());
    if let Some(path) = &o.out {
        let mut f = std::fs::File::create(path).expect("create --out file");
        writeln!(f, "group\tsettings\tstatus\tturn\tend\tsecs\tdetail\tcommand").unwrap();
        for r in &results {
            writeln!(
                f,
                "{}\t{}\t{}\t{}\t{}\t{:.1}\t{}\tffh-stress run {}",
                r.settings.group,
                r.settings.describe(),
                r.status,
                r.turn,
                r.end,
                r.secs,
                r.detail.replace('\n', " | ").replace('\t', " "),
                r.settings.to_args().join(" ")
            )
            .unwrap();
        }
    }
}

/// Runs `ffh-stress run <settings>` with a timeout and parses its result lines.
fn run_child(exe: &std::path::Path, s: &Settings, timeout: Duration) -> Done {
    let t = Instant::now();
    let mut child = Command::new(exe)
        .arg("run")
        .args(s.to_args())
        .env("RUST_BACKTRACE", "0")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ffh-stress run");
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out_thread = std::thread::spawn(move || {
        let mut v = String::new();
        let _ = stdout.read_to_string(&mut v);
        v
    });
    let err_thread = std::thread::spawn(move || {
        let mut v = String::new();
        let _ = BufReader::new(&mut stderr).read_to_string(&mut v);
        v
    });
    let mut timed_out = false;
    let status = loop {
        if let Some(st) = child.try_wait().expect("wait child") {
            break st;
        }
        if t.elapsed() > timeout {
            let _ = child.kill();
            timed_out = true;
            break child.wait().expect("wait child");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let out = out_thread.join().unwrap_or_default();
    let err = err_thread.join().unwrap_or_default();
    let secs = t.elapsed().as_secs_f64();
    let mut done = Done { settings: s.clone(), status: String::new(), turn: "?".into(), end: String::new(), detail: String::new(), secs };
    let mut panic_lines = Vec::new();
    let mut progress = String::new();
    let mut victory = String::new();
    for line in out.lines() {
        let mut f = line.splitn(2, '\t');
        match (f.next(), f.next()) {
            (Some("TURN"), Some(v)) => done.turn = v.to_string(),
            (Some("END"), Some(v)) => done.end = v.to_string(),
            (Some("PANIC"), Some(v)) | (Some("BT"), Some(v)) => panic_lines.push(v.to_string()),
            (Some("PROGRESS"), Some(v)) => progress = v.to_string(),
            (Some("VICTORY"), Some(v)) => victory = v.replace('\t', " "),
            _ => {}
        }
    }
    if timed_out {
        done.status = "timeout".into();
        done.detail = format!("killed after {} s", timeout.as_secs());
    } else if !panic_lines.is_empty() {
        done.status = "panic".into();
        done.detail = panic_lines.join("\n");
    } else if !status.success() {
        done.status = "crash".into();
        let tail: Vec<&str> = err.lines().rev().take(6).collect();
        done.detail = format!("{status}: {}", tail.into_iter().rev().collect::<Vec<_>>().join(" | "));
    } else {
        done.status = "ok".into();
    }
    if !victory.is_empty() {
        // "<turn> <state> <victory> winner=..": the end state at the victory, then the end.
        let (vt, rest) = victory.split_once(' ').unwrap_or(("?", ""));
        let short: String = rest.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
        done.end = format!("{short} @{vt}; then {}", done.end);
    }
    if done.turn == "?" && !progress.is_empty() {
        done.turn = progress;
    }
    done
}

fn summary(results: &[Done], elapsed: Duration) {
    println!("\n== summary: {} games in {:.0} s ==", results.len(), elapsed.as_secs_f64());
    let mut groups: Vec<String> = Vec::new();
    for r in results {
        if !groups.contains(&r.settings.group) {
            groups.push(r.settings.group.clone());
        }
    }
    println!("{:<10} {:>5} {:>5} {:>6} {:>7} {:>6}  ends", "group", "games", "ok", "panic", "timeout", "crash");
    for gname in &groups {
        let rs: Vec<&Done> = results.iter().filter(|r| &r.settings.group == gname).collect();
        let c = |s: &str| rs.iter().filter(|r| r.status == s).count();
        let mut ends: Vec<(String, usize)> = Vec::new();
        for r in rs.iter().filter(|r| r.status == "ok") {
            // "ON", or the victory type ("EXTENDED CONQUEST ..." -> "CONQUEST").
            let w: Vec<&str> = r.end.split_whitespace().take(2).collect();
            let k = if w.first() == Some(&"ON") { "ON".to_string() } else { w.get(1).unwrap_or(&"?").to_string() };
            match ends.iter_mut().find(|(e, _)| *e == k) {
                Some(e) => e.1 += 1,
                None => ends.push((k, 1)),
            }
        }
        let ends: Vec<String> = ends.iter().map(|(e, n)| format!("{e}={n}")).collect();
        println!("{:<10} {:>5} {:>5} {:>6} {:>7} {:>6}  {}", gname, rs.len(), c("ok"), c("panic"), c("timeout"), c("crash"), ends.join(" "));
    }
    let failed: Vec<&Done> = results.iter().filter(|r| r.status != "ok").collect();
    if failed.is_empty() {
        println!("\nno failures");
        return;
    }
    // Failures grouped by their first detail line (panic message + location).
    let mut keys: Vec<(String, Vec<&Done>)> = Vec::new();
    for r in &failed {
        let k = r.detail.lines().next().unwrap_or("").to_string();
        match keys.iter_mut().find(|(kk, _)| *kk == k) {
            Some(e) => e.1.push(r),
            None => keys.push((k, vec![r])),
        }
    }
    println!("\n== {} failures, {} distinct ==", failed.len(), keys.len());
    for (k, rs) in &keys {
        println!("\n[{} x {}] {}", rs.len(), rs[0].status, k);
        for line in rs[0].detail.lines().skip(1) {
            println!("    {line}");
        }
        for r in rs.iter().take(6) {
            println!("  turn {:>5}  ffh-stress run {}", r.turn, r.settings.to_args().join(" "));
        }
        if rs.len() > 6 {
            println!("  ... {} more", rs.len() - 6);
        }
    }
}
