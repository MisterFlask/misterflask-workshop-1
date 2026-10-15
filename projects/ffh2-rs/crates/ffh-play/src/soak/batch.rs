//! The soak matrix and the batch runner (`ffh-play soak`): every game in a child
//! `ffh-play soak run` process with a timeout, so panics, aborts, stack overflows and hangs are
//! all reported, with the command that reproduces each failure.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ffh_data::*;

use super::{default_opponents, latest_autosave, lenient, options_label, parse_options, SoakGame, SPAWNED_CIVS};
use crate::names;

/// The matrix of design §7 P2: seeds × sizes × civs × option sets (× game speeds).
#[derive(Clone, Debug)]
pub struct Matrix {
    pub seeds: Vec<u32>,
    pub sizes: Vec<WorldSizeType>,
    pub civs: Vec<CivilizationType>,
    pub options: Vec<Vec<GameOptionType>>,
    /// Game speeds (default: the new game's, normal). A quicker speed reaches the late game
    /// in fewer turns.
    pub speeds: Vec<GameSpeedType>,
    pub turns: i32,
}

impl Matrix {
    /// 2 seeds × {tiny, small, standard} × {Kuriotates, Infernal, Sheaim, Mercurians, Bannor,
    /// Calabim} × {default, raging barbarians, `occ`, `challenges`}, 300 turns. The design's
    /// first matrix had "aggressive AI" for the last two: no rules or AI code of the port reads
    /// that option (C++ uses it only in the AI and in the Python war scripts the new AI replaces),
    /// so its games were the default ones (same checksums); the human-only [`OPTION_SETS`] take
    /// its place.
    pub fn design(d: &GameData) -> Matrix {
        fn t<T: InfoId + names::Named>(d: &GameData, p: &str, s: &str) -> T {
            lenient(d, p, s).expect("matrix type")
        }
        Matrix {
            seeds: vec![1, 2],
            sizes: ["tiny", "small", "standard"].iter().map(|s| t(d, "WORLDSIZE_", s)).collect(),
            civs: ["kuriotates", "infernal", "sheaim", "mercurians", "bannor", "calabim"].iter().map(|s| t(d, "CIVILIZATION_", s)).collect(),
            options: ["default", "raging_barbarians", "occ", "challenges"].iter().map(|o| parse_options(d, o).expect("matrix options")).collect(),
            speeds: vec![SoakGame::new(d, 1).ng.speed],
            turns: 300,
        }
    }

    /// The games, each once (a seed, size, civ, option set or speed given twice adds nothing:
    /// a game is deterministic).
    pub fn games(&self, d: &GameData) -> Vec<SoakGame> {
        let mut v: Vec<SoakGame> = Vec::new();
        for &size in &self.sizes {
            for &civ in &self.civs {
                for opts in &self.options {
                    for &speed in &self.speeds {
                        for &seed in &self.seeds {
                            let mut g = SoakGame::new(d, seed);
                            g.ng.world = size;
                            g.ng.opponents = default_opponents(d, size);
                            if SPAWNED_CIVS.contains(&d.type_name(civ)) {
                                g.spawn = Some(civ);
                            } else {
                                g.ng.civ = Some(civ);
                            }
                            g.ng.options = opts.clone();
                            g.ng.speed = speed;
                            g.turns = self.turns;
                            if !v.contains(&g) {
                                v.push(g);
                            }
                        }
                    }
                }
            }
        }
        v
    }
}

/// Seeds: `1..3` (inclusive, as shell brace ranges) or `1,2,5`.
pub fn parse_seeds(s: &str) -> Result<Vec<u32>, String> {
    let bad = || format!("bad seeds {s:?} (A..B or A,B,..)");
    if let Some((a, b)) = s.split_once("..") {
        let (a, b): (u32, u32) = (a.parse().map_err(|_| bad())?, b.parse().map_err(|_| bad())?);
        if a > b {
            return Err(bad());
        }
        return Ok((a..=b).collect());
    }
    s.split(',').map(|x| x.parse().map_err(|_| bad())).collect()
}

/// The outcome of one child process of the batch.
#[derive(Clone, Debug)]
pub struct Done {
    pub game: SoakGame,
    /// `ok`, `panic`, `stuck`, `timeout`, `crash`, `error`.
    pub status: String,
    pub turn: String,
    pub end: String,
    pub detail: String,
    /// `STATS` line (human vs AI cities, counters).
    pub stats: String,
    pub human_cities: Option<usize>,
    pub ai_cities: Vec<usize>,
    pub secs: f64,
    /// The `soak run` line that reproduces it (with `--resume` when an autosave was left).
    pub repro: String,
}

/// Batch settings.
#[derive(Clone, Debug)]
pub struct BatchOptions {
    pub jobs: usize,
    pub timeout: Duration,
    /// Each game's autosaves in `<saves>/<game>/` (kept for failures only, unless `keep_saves`).
    pub saves: Option<PathBuf>,
    pub keep_saves: bool,
    /// Extra arguments for every `soak run` (e.g. `--check-every 1`); also in every repro.
    pub extra: Vec<String>,
}

/// Runs every game in a child `exe soak run ...` process, `jobs` at a time; progress on stderr.
pub fn run_all(d: &GameData, exe: &Path, games: Vec<SoakGame>, o: &BatchOptions) -> Vec<Done> {
    let n = games.len();
    eprintln!("{n} soak games, {} jobs, timeout {} s", o.jobs, o.timeout.as_secs());
    let games = Arc::new(games);
    let next = Arc::new(AtomicUsize::new(0));
    let results: Arc<Mutex<Vec<Option<Done>>>> = Arc::new(Mutex::new(vec![None; n]));
    let t0 = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..o.jobs.max(1) {
            let (games, next, results) = (games.clone(), next.clone(), results.clone());
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= games.len() {
                    break;
                }
                let done = run_child(d, exe, &games[i], o);
                eprintln!(
                    "[{:4}/{n} {:6.0}s] {:7} turn {:>4} {:>6.1}s  {}  {}",
                    i + 1,
                    t0.elapsed().as_secs_f64(),
                    done.status,
                    done.turn,
                    done.secs,
                    done.game.describe(d),
                    if done.status == "ok" { format!("{} {}", done.end, done.stats) } else { done.detail.lines().next().unwrap_or("").to_string() }
                );
                results.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(done);
            });
        }
    });
    let mut results = results.lock().unwrap_or_else(|e| e.into_inner());
    results.drain(..).map(|r| r.expect("every game ran")).collect()
}

/// One `soak run` child with a timeout.
fn run_child(d: &GameData, exe: &Path, game: &SoakGame, o: &BatchOptions) -> Done {
    let t = Instant::now();
    // The game and the run options that decide its checks: the repro has both.
    let run_args: Vec<String> = game.to_args(d).into_iter().chain(o.extra.iter().cloned()).collect();
    let mut args: Vec<String> = vec!["soak".into(), "run".into()];
    args.extend(run_args.iter().cloned());
    let saves = o.saves.as_ref().map(|s| s.join(game.file_name(d)));
    if let Some(dir) = &saves {
        let _ = std::fs::remove_dir_all(dir);
        args.push("--saves".into());
        args.push(dir.display().to_string());
    }
    let mut done = Done {
        game: game.clone(),
        status: String::new(),
        turn: "?".into(),
        end: String::new(),
        detail: String::new(),
        stats: String::new(),
        human_cities: None,
        ai_cities: Vec::new(),
        secs: 0.0,
        repro: format!("ffh-play soak run {}", run_args.join(" ")),
    };
    let child = std::process::Command::new(exe).args(&args).env("RUST_BACKTRACE", "0").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            done.status = "error".into();
            done.detail = format!("spawn {}: {e}", exe.display());
            return done;
        }
    };
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let out_thread = std::thread::spawn(move || {
        let mut v = String::new();
        let _ = stdout.read_to_string(&mut v);
        v
    });
    let err_thread = std::thread::spawn(move || {
        let mut v = String::new();
        let _ = stderr.read_to_string(&mut v);
        v
    });
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) => {}
            Err(_) => break None,
        }
        if t.elapsed() > o.timeout {
            let _ = child.kill();
            timed_out = true;
            break child.wait().ok();
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let out = out_thread.join().unwrap_or_default();
    let err = err_thread.join().unwrap_or_default();
    done.secs = t.elapsed().as_secs_f64();
    let mut panic_lines = Vec::new();
    let mut progress = String::new();
    for line in out.lines() {
        let (k, v) = line.split_once('\t').unwrap_or((line, ""));
        match k {
            "TURN" => done.turn = v.to_string(),
            "PROGRESS" => progress = v.to_string(),
            "END" => done.end = v.to_string(),
            "STATS" => done.stats = v.to_string(),
            "CITIES" => {
                let mut f = v.split('\t');
                done.human_cities = f.next().and_then(|x| x.parse().ok());
                done.ai_cities = f.next().unwrap_or("").split(',').filter_map(|x| x.parse().ok()).collect();
            }
            "PANIC" | "BT" | "STUCK" | "ERROR" => panic_lines.push(v.to_string()),
            _ => {}
        }
    }
    if done.turn == "?" && !progress.is_empty() {
        done.turn = progress;
    }
    let ok_exit = status.is_some_and(|s| s.success());
    done.status = if timed_out {
        done.detail = format!("killed after {} s", o.timeout.as_secs());
        "timeout"
    } else if out.lines().any(|l| l.starts_with("PANIC\t")) {
        "panic"
    } else if out.lines().any(|l| l.starts_with("STUCK\t")) {
        "stuck"
    } else if out.lines().any(|l| l.starts_with("ERROR\t")) {
        "error"
    } else if !ok_exit {
        let tail: Vec<&str> = err.lines().rev().take(6).collect();
        done.detail = format!("{}: {}", status.map_or("?".to_string(), |s| s.to_string()), tail.into_iter().rev().collect::<Vec<_>>().join(" | "));
        "crash"
    } else {
        "ok"
    }
    .to_string();
    if !panic_lines.is_empty() {
        done.detail = panic_lines.join("\n");
    }
    if let Some(dir) = &saves {
        if done.status == "ok" && !o.keep_saves {
            let _ = std::fs::remove_dir_all(dir);
        } else if done.status != "ok" {
            if let Some(a) = latest_autosave(dir) {
                done.repro += &format!(" --resume {}", a.display());
            }
        }
    }
    done
}

/// The batch summary (stdout): counts per civ, size and option set, the human's cities against
/// the AI civs', and every distinct failure with its repro commands.
pub fn summary(d: &GameData, results: &[Done], elapsed: Duration) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "\n== soak summary: {} games in {:.0} s ==", results.len(), elapsed.as_secs_f64());
    type Key = fn(&GameData, &SoakGame) -> String;
    let by: [(&str, Key); 3] = [
        ("civ", |d, g| g.civ().map_or("RANDOM".into(), |c| d.type_name(c).trim_start_matches("CIVILIZATION_").to_string())),
        ("size", |d, g| d.type_name(g.ng.world).trim_start_matches("WORLDSIZE_").to_string()),
        ("options", |d, g| if g.ng.options.is_empty() { "default".into() } else { options_label(d, &g.ng.options) }),
    ];
    for (title, key) in by {
        let mut keys: Vec<String> = Vec::new();
        for r in results {
            let k = key(d, &r.game);
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        let _ = writeln!(s, "\n{:<20} {:>5} {:>4} {:>5} {:>5} {:>7} {:>5}  {:>9} {:>9}  ends", title, "games", "ok", "panic", "stuck", "timeout", "other", "human_cit", "ai_cit");
        for k in &keys {
            let rs: Vec<&Done> = results.iter().filter(|r| &key(d, &r.game) == k).collect();
            let c = |st: &str| rs.iter().filter(|r| r.status == st).count();
            let other = rs.len() - c("ok") - c("panic") - c("stuck") - c("timeout");
            let (hc, ac) = city_means(&rs);
            let mut ends: Vec<(String, usize)> = Vec::new();
            for r in rs.iter().filter(|r| r.status == "ok") {
                let e = r.end.split(" (").next().unwrap_or("").split_whitespace().take(2).collect::<Vec<_>>().join(" ");
                match ends.iter_mut().find(|(x, _)| *x == e) {
                    Some(x) => x.1 += 1,
                    None => ends.push((e, 1)),
                }
            }
            let ends: Vec<String> = ends.iter().map(|(e, n)| format!("{e}={n}")).collect();
            let _ = writeln!(s, "{:<20} {:>5} {:>4} {:>5} {:>5} {:>7} {:>5}  {:>9.1} {:>9.1}  {}", k, rs.len(), c("ok"), c("panic"), c("stuck"), c("timeout"), other, hc, ac, ends.join(" "));
        }
    }
    let all: Vec<&Done> = results.iter().collect();
    let (hc, ac) = city_means(&all);
    let _ = writeln!(s, "\nhuman (AI-as-human) cities at the end: mean {hc:.2}; AI civs: mean {ac:.2} (games that ran to the end)");
    let failed: Vec<&Done> = results.iter().filter(|r| r.status != "ok").collect();
    if failed.is_empty() {
        let _ = writeln!(s, "\nno failures");
        return s;
    }
    let mut groups: Vec<(String, Vec<&Done>)> = Vec::new();
    for r in &failed {
        let k = r.detail.lines().next().unwrap_or("").to_string();
        match groups.iter_mut().find(|(x, _)| *x == k) {
            Some(x) => x.1.push(r),
            None => groups.push((k, vec![r])),
        }
    }
    let _ = writeln!(s, "\n== {} failures, {} distinct ==", failed.len(), groups.len());
    for (k, rs) in &groups {
        let _ = writeln!(s, "\n[{} x {}] {}", rs.len(), rs[0].status, k);
        for line in rs[0].detail.lines().skip(1) {
            let _ = writeln!(s, "    {line}");
        }
        for r in rs.iter().take(6) {
            let _ = writeln!(s, "  turn {:>4}  {}", r.turn, r.repro);
        }
        if rs.len() > 6 {
            let _ = writeln!(s, "  ... {} more", rs.len() - 6);
        }
    }
    s
}

/// Mean cities of the human and of the AI civs over finished games with city counts.
fn city_means(rs: &[&Done]) -> (f64, f64) {
    let with: Vec<&&Done> = rs.iter().filter(|r| r.status == "ok" && r.human_cities.is_some()).collect();
    if with.is_empty() {
        return (0.0, 0.0);
    }
    let h = with.iter().map(|r| r.human_cities.unwrap_or(0) as f64).sum::<f64>() / with.len() as f64;
    let ai: Vec<f64> = with.iter().flat_map(|r| r.ai_cities.iter().map(|&c| c as f64)).collect();
    (h, if ai.is_empty() { 0.0 } else { ai.iter().sum::<f64>() / ai.len() as f64 })
}

/// The TSV of a batch (`--out`).
pub fn tsv(d: &GameData, results: &[Done]) -> String {
    let mut s = String::from("settings\tstatus\tturn\tend\tsecs\thuman_cities\tai_cities\tstats\tdetail\trepro\n");
    for r in results {
        s += &format!(
            "{}\t{}\t{}\t{}\t{:.1}\t{}\t{}\t{}\t{}\t{}\n",
            r.game.describe(d),
            r.status,
            r.turn,
            r.end,
            r.secs,
            r.human_cities.map_or(String::new(), |c| c.to_string()),
            r.ai_cities.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","),
            r.stats,
            r.detail.replace('\n', " | ").replace('\t', " "),
            r.repro
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds() {
        assert_eq!(parse_seeds("1..3").unwrap(), [1, 2, 3]);
        assert_eq!(parse_seeds("4,9").unwrap(), [4, 9]);
        assert!(parse_seeds("3..1").is_err() && parse_seeds("x").is_err());
    }

    #[test]
    fn the_design_matrix() {
        let d = ffh_data::global();
        let m = Matrix::design(d);
        let games = m.games(d);
        assert_eq!(games.len(), 2 * 3 * 6 * 4);
        assert_eq!(games.iter().filter(|g| g.spawn.is_some()).count(), 2 * 3 * 2 * 4);
        assert_eq!(games.iter().filter(|g| g.ng.options.len() == 4).count(), 36, "the challenges");
        assert!(games.iter().all(|g| g.turns == 300 && g.ng.validate(d).is_ok()));
        let names: Vec<String> = games.iter().map(|g| g.file_name(d)).collect();
        assert!(names.iter().enumerate().all(|(i, n)| !names[..i].contains(n)), "file names are unique");
    }

    /// Each game of a batch gets its own save directory: also games whose descriptions share
    /// the first 55 characters or differ only in the speed; a repeated seed adds no game.
    #[test]
    fn save_directories_are_unique() {
        let d = ffh_data::global();
        let mut m = Matrix::design(d);
        m.seeds = vec![1, 1];
        m.sizes = vec![lenient(d, "WORLDSIZE_", "standard").unwrap()];
        m.civs = vec![lenient(d, "CIVILIZATION_", "kuriotates").unwrap()];
        m.options = ["raging+aggressive+no_tech_trading", "raging+aggressive+no_tech_brokering"].iter().map(|o| parse_options(d, o).unwrap()).collect();
        m.speeds = ["normal", "quick"].iter().map(|s| lenient(d, "GAMESPEED_", s).unwrap()).collect();
        m.turns = 1;
        let games = m.games(d);
        assert_eq!(games.len(), 4, "the repeated seed is dropped");
        let (a, b) = (games[0].describe(d), games[2].describe(d));
        assert_eq!(a.chars().take(60).collect::<String>(), b.chars().take(60).collect::<String>(), "{a} / {b}");
        let names: Vec<String> = games.iter().map(|g| g.file_name(d)).collect();
        assert!(names.iter().enumerate().all(|(i, n)| !names[..i].contains(n)), "{names:?}");
        assert!(names.iter().all(|n| n.len() <= 64 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')), "{names:?}");
        assert!(games[1].describe(d).contains(" QUICK "), "{}", games[1].describe(d));
    }
}
