//! Smoke tests of the `ffh-play` binary (P0 commands).

use std::process::Command;

use ffh_play::json::Json;
use ffh_play::names::KINDS;

fn run(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ffh-play")).args(args).output().expect("run ffh-play");
    (out.status.code().unwrap_or(-1), String::from_utf8(out.stdout).expect("UTF-8 output"))
}

#[test]
fn lookup_prints_one_json_line_per_reference() {
    let (code, out) = run(&["lookup", "UNIT_WARRIOR", "UnitType:0", "units:0", "ORDER_TRAIN", "NO_SUCH_TYPE", "UnitType:100000"]);
    assert_eq!(code, 1, "a failed reference makes the exit code 1");
    let lines: Vec<Json> = out.lines().map(|l| Json::parse(l).unwrap_or_else(|e| panic!("{e}: {l}"))).collect();
    assert_eq!(lines.len(), 6);
    assert_eq!(lines[0].req_str("kind"), Ok("UnitType"));
    assert_eq!(lines[0].req_str("name"), Ok("UNIT_WARRIOR"));
    assert_eq!(lines[0].req_str("display"), Ok("Warrior"));
    assert_eq!(lines[1].req::<i64>("index"), Ok(0));
    assert_eq!(lines[1].get("name"), lines[2].get("name"));
    assert_eq!(lines[3].req_str("kind"), Ok("OrderType"));
    assert_eq!(lines[3].get("text_key"), Some(&Json::Null));
    assert_eq!(lines[4].req_str("error"), Ok("unknown type name"));
    assert!(lines[5].req_str("error").unwrap().contains("out of range"));

    let (code, _) = run(&["lookup", "TECH_NECROMANCY"]);
    assert_eq!(code, 0);

    // Display names are the full description (not cut at a ':'), or the type name when the
    // text has an unfilled placeholder.
    let (code, out) = run(&["lookup", "VOTE_LIBERTY", "MEMORY_SOMNIUM_POSITIVE", "MEMORY_SOMNIUM_NEGATIVE"]);
    assert_eq!(code, 0);
    let display: Vec<String> =
        out.lines().map(|l| Json::parse(l).unwrap().req::<String>("display").unwrap()).collect();
    assert_eq!(display, ["Global Civic: Liberty", "Somnium Positive", "Somnium Negative"]);
}

#[test]
fn names_and_kinds() {
    let (code, out) = run(&["names", "units"]);
    assert_eq!(code, 0);
    let units = Json::parse(out.trim()).unwrap();
    assert!(units.as_arr().unwrap().iter().any(|n| n.as_str() == Some("UNIT_WARRIOR")));

    let (code, out) = run(&["names", "--pretty"]);
    assert_eq!(code, 0);
    let all = Json::parse(&out).unwrap();
    assert_eq!(all.as_obj().unwrap().len(), KINDS.len());
    assert_eq!(all.get("UnitType"), Some(&units));

    let (code, out) = run(&["kinds"]);
    assert_eq!(code, 0);
    assert_eq!(out.lines().count(), KINDS.len());

    assert_eq!(run(&["names", "NoSuchKind"]).0, 1);
    assert_eq!(run(&[]).0, 2);
    assert_eq!(run(&["no-such-command"]).0, 2);
}

/// `bot` with a saves directory, then `replay` (from the header and from an autosave) and
/// `journal print` of its journal.
#[test]
fn bot_replay_and_journal_print() {
    let dir = std::env::temp_dir().join(format!("ffh-play-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let d = dir.to_str().unwrap();
    let (code, out) = run(&["bot", "--seed", "2", "--turns", "12", "--world", "WORLDSIZE_DUEL", "--opponents", "1", "--saves", d, "--quiet"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("stopped: Human at turn 12"), "{out}");
    let journal = dir.join("journal.jsonl");
    let (code, out) = run(&["replay", journal.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.starts_with("replayed "), "{out}");
    let auto = dir.join("auto/T0005.ffhsave");
    let (code, out2) = run(&["replay", journal.to_str().unwrap(), "--from", auto.to_str().unwrap()]);
    assert_eq!(code, 0, "{out2}");
    assert_eq!(out, out2, "the same end from the header and from the autosave");
    let (code, out) = run(&["journal", "print", journal.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(out.lines().next().unwrap().starts_with("new game seed 2"), "{out}");
    assert!(out.contains("-- checkpoint (turn 5)"), "{out}");
    assert_eq!(run(&["replay", dir.join("missing.jsonl").to_str().unwrap()]).0, 1);
    assert_eq!(run(&["journal", "show", journal.to_str().unwrap()]).0, 2);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The value of the first `KEY\t...` line of a `soak run` output.
fn field<'a>(out: &'a str, key: &str) -> Option<&'a str> {
    out.lines().find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('\t')))
}

/// `soak run` (a game, a deliberate panic with its report and repro, the repro resumed),
/// `soak --list` and a small `soak` batch.
#[test]
fn soak_run_report_and_batch() {
    let dir = std::env::temp_dir().join(format!("ffh-play-cli-soak-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let game = ["--seed", "5", "--size", "duel", "--opponents", "1", "--civ", "sheaim", "--turns", "16"];
    let (code, out) = run(&[&["soak", "run"][..], &game].concat());
    assert_eq!(code, 0, "{out}");
    assert_eq!(field(&out, "TURN"), Some("16"));
    let end = field(&out, "END").unwrap().to_string();
    assert!(end.starts_with("ON ("), "{out}");
    assert!(field(&out, "STATS").unwrap().contains("CIVILIZATION_SHEAIM"), "{out}");

    // A panic: reported with the workspace frames and the command that resumes the game.
    let saves = dir.join("panic");
    let (code, out) = run(&[&["soak", "run"][..], &game, &["--saves", saves.to_str().unwrap(), "--panic-at", "9"]].concat());
    assert_eq!(code, 101, "{out}");
    assert_eq!(field(&out, "TURN"), Some("9"));
    assert!(field(&out, "PANIC").unwrap().starts_with("soak self-test: a deliberate panic at turn 9 at crates/ffh-play/src/soak.rs:"), "{out}");
    assert!(field(&out, "BT").unwrap().starts_with("ffh_play::soak::run "), "{out}");
    let repro = field(&out, "REPRO").unwrap();
    let autosave = saves.join("auto/T0008.ffhsave");
    assert_eq!(
        repro,
        format!("ffh-play soak run --seed 5 --size WORLDSIZE_DUEL --civ CIVILIZATION_SHEAIM --turns 16 --resume {}", autosave.display())
    );
    let args: Vec<&str> = repro.split(' ').skip(1).collect();
    let (code, out) = run(&args);
    assert_eq!(code, 0, "{out}");
    // The resumed game ends as the uninterrupted one (state, checksum, save hash; not the time).
    let strip = |e: &str| e.split_once(" s, ").map(|(a, b)| (a.split(" (").next().unwrap().to_string(), b.to_string())).unwrap();
    assert_eq!(strip(field(&out, "END").unwrap()), strip(&end));

    // The matrix.
    let (code, out) = run(&["soak", "--list"]);
    assert_eq!(code, 0);
    assert_eq!(out.lines().count(), 144, "design §7 P2: 2 seeds x 3 sizes x 6 civs x 4 option sets");
    let (code, out) = run(&["soak", "--list", "--seeds", "1..3", "--sizes", "duel", "--civs", "bannor,infernal", "--options", "default,raging+aggressive", "--filter", "INFERNAL"]);
    assert_eq!(code, 0);
    assert_eq!(out.lines().count(), 6, "{out}");
    assert!(out.lines().all(|l| l.starts_with("ffh-play soak run --seed ") && l.contains("--civ CIVILIZATION_INFERNAL")), "{out}");
    let batch = dir.join("batch");
    let tsv = dir.join("soak.tsv");
    let (code, out) = run(&[
        "soak", "--seeds", "1..2", "--sizes", "duel", "--civs", "calabim", "--options", "default", "--turns", "10", "--jobs", "2", "--saves",
        batch.to_str().unwrap(), "--out", tsv.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("== soak summary: 2 games in ") && out.contains("\nno failures"), "{out}");
    let tsv = std::fs::read_to_string(&tsv).unwrap();
    assert_eq!(tsv.lines().count(), 3, "{tsv}");
    assert!(tsv.lines().skip(1).all(|l| l.split('\t').nth(1) == Some("ok")), "{tsv}");
    assert_eq!(std::fs::read_dir(&batch).map_or(0, |d| d.count()), 0, "saves of games that passed are removed");
    // Usage errors.
    assert_eq!(run(&["soak", "run", "--size", "enormous"]).0, 2);
    assert_eq!(run(&["soak", "--seeds", "3..1"]).0, 2);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A repro keeps the check settings that decide on which turns checks run (`soak run`'s own
/// and those a batch passes to its games); arguments are checked before any game runs.
#[test]
fn soak_repro_keeps_the_checks_and_arguments_are_checked_first() {
    let dir = std::env::temp_dir().join(format!("ffh-play-cli-soak-checks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let saves = dir.join("run");
    let (code, out) =
        run(&["soak", "run", "--size", "duel", "--turns", "6", "--check-every", "1", "--save-check-every", "2", "--panic-at", "4", "--saves", saves.to_str().unwrap()]);
    assert_eq!(code, 101, "{out}");
    let expected = format!("ffh-play soak run --seed 1 --size WORLDSIZE_DUEL --turns 6 --check-every 1 --save-check-every 2 --resume {}", saves.join("auto/T0003.ffhsave").display());
    assert_eq!(field(&out, "REPRO"), Some(expected.as_str()), "{out}");
    // Numbers out of their range are refused, not wrapped.
    for bad in [&["--turns", "4294967297"][..], &["--seed", "-1"], &["--check-every", "x"], &["--save-check-every", "-1"]] {
        let (code, out) = run(&[&["soak", "run", "--size", "duel"][..], bad].concat());
        assert_eq!(code, 2, "{bad:?}: {out}");
    }
    // The batch: usage errors before any game runs.
    let one = ["soak", "--seeds", "1", "--sizes", "duel", "--civs", "calabim", "--options", "default", "--turns", "2", "--jobs", "1"];
    for bad in [&["--out", "/nonexistent/dir/x.tsv"][..], &["--turns", "0"], &["--check-every", "x"], &["--options", "GAMEOPTION_ADVANCED_START"]] {
        let (code, out) = run(&[&one[..], bad].concat());
        assert_eq!(code, 2, "{bad:?}: {out}");
        assert!(!out.contains("soak summary"), "{bad:?}: a game ran: {out}");
    }
    // A failed game's repro has the check settings the batch gave it (a timeout of 0 s fails
    // the game at once).
    let (code, out) = run(&[&one[..], &["--timeout", "0", "--check-every", "1"]].concat());
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("timeout") && out.contains("  ffh-play soak run --seed 1 --size WORLDSIZE_DUEL --civ CIVILIZATION_CALABIM --turns 2 --check-every 1\n"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
