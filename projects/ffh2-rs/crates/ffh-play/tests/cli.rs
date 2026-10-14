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
