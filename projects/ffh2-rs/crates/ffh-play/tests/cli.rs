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
