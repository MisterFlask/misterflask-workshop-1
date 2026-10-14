//! T4: journal replay (design §8). A bot game with a journal file and autosaves is replayed
//! from its header and from an autosave at one of its checkpoints: the final saves must be
//! byte-identical. A journal with one stamp changed fails at that line.

mod common;

use std::path::Path;

use common::{bot_session, new_game, TempDir};
use ffh_play::journal::{self, Line};
use ffh_play::save::read_bundle;
use ffh_play::session::{replay, ReplayFrom};
use ffh_play::{bot, SessionOptions, Stop};

#[test]
fn t4_replay_journal() {
    let dir = TempDir::new("replay");
    let ng = new_game(3);
    let (mut s, mut b) = bot_session(&ng, Some(dir.0.clone()));
    let stop = bot::run(&mut s, &mut b, 130, |_, _| {});
    assert_eq!(stop, Stop::Human);
    assert!(s.stats().decisions_total() >= 1, "the game should contain a blocking decision");
    let (final_save, _) = s.save_bytes().unwrap();
    let path = s.journal_path().expect("a journal file");
    let lines = journal::read_file(&path).unwrap();
    let count = |k: &str| lines.iter().filter(|l| l.kind() == k).count();
    eprintln!("journal {}: {} lines, {} commands, {} answers, {} checkpoints", path.display(), lines.len(), count("cmd"), count("answer"), count("checkpoint"));
    assert!(matches!(lines[0], Line::New { .. }));
    assert!(count("answer") >= 1 && count("checkpoint") >= 100);
    drop(s);

    // From the header.
    let r = replay(lines.clone(), None, SessionOptions::default()).unwrap_or_else(|e| panic!("replay: {e}"));
    assert!(r.g.save() == final_save, "replay from the header ends in a different game");

    // From the oldest turn autosave kept (a checkpoint in the middle of the journal), and from
    // the new game's first control point.
    let mut autos: Vec<_> = std::fs::read_dir(dir.0.join("auto"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "ffhsave") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('T')))
        .collect();
    autos.sort();
    let (start, start_sidecar) = read_bundle(&dir.0.join("auto/start.ffhsave")).unwrap();
    let r = replay(lines.clone(), Some(ReplayFrom { save: &start, sidecar: start_sidecar.as_ref() }), SessionOptions::default()).unwrap_or_else(|e| panic!("replay from the start save: {e}"));
    assert!(r.g.save() == final_save, "replay from the start save ends in a different game");
    let (save, sidecar) = read_bundle(&autos[0]).unwrap();
    eprintln!("replaying from {}", autos[0].display());
    let r = replay(lines.clone(), Some(ReplayFrom { save: &save, sidecar: sidecar.as_ref() }), SessionOptions::default()).unwrap_or_else(|e| panic!("replay from a save: {e}"));
    assert!(r.g.save() == final_save, "replay from an autosave ends in a different game");
    // Without the sidecar the checkpoint is found by turn and checksum.
    // (Without it no event popup is pending, so this holds only when none was.)
    if sidecar.as_ref().is_some_and(|sc| sc.pending_events.is_empty()) {
        let r = replay(lines.clone(), Some(ReplayFrom { save: &save, sidecar: None }), SessionOptions::default()).unwrap_or_else(|e| panic!("replay without sidecar: {e}"));
        assert!(r.g.save() == final_save, "replay from an autosave without sidecar ends in a different game");
    }

    // A changed stamp is found at its line.
    let text = std::fs::read_to_string(&path).unwrap();
    let mut raw: Vec<String> = text.lines().map(str::to_string).collect();
    let (i, line) = raw.iter().enumerate().filter(|(_, l)| l.contains("\"k\":\"cmd\"")).nth(40).map(|(i, l)| (i, l.clone())).unwrap();
    let Line::Cmd { stamp, .. } = Line::parse(&line).unwrap() else { panic!() };
    raw[i] = line.replacen(&format!("\"s\":{}", stamp.slice), &format!("\"s\":{}", stamp.slice + 1), 1);
    let bad = journal::parse_lines(&raw).unwrap();
    let err = match replay(bad, None, SessionOptions::default()) {
        Ok(_) => panic!("a corrupted journal replayed"),
        Err(e) => e,
    };
    assert!(err.contains(&format!("journal line {}", i + 1)), "{err}");

    // The pretty printer names types.
    let d = ffh_data::global();
    let printed: Vec<String> = lines.iter().map(|l| journal::pretty(d, l)).collect();
    assert!(printed.iter().any(|l| l.contains("research TECH_")), "{:?}", &printed[..10]);
    assert!(Path::new(&path).exists());
}
