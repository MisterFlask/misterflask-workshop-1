//! T4: journal replay (design §8). A bot game with a journal file and autosaves is replayed
//! from its header and from an autosave at one of its checkpoints: the final saves must be
//! byte-identical. A journal with one stamp changed fails at that line. Journals of loaded and
//! taken-over sessions replay from the save their header names.

mod common;

use std::path::Path;

use common::{bot_session, new_game, peaceful_game, TempDir};
use ffh_play::journal::{self, Line};
use ffh_play::save::read_bundle;
use ffh_play::session::{replay, ReplayFrom};
use ffh_play::{bot, Session, SessionOptions, Stop};

#[test]
fn t4_replay_journal() {
    let dir = TempDir::new("replay");
    // Always-peace, barbarian-free (see `common::peaceful_game`): no AI can declare war on or
    // eliminate the human and no victory can fire, so the bot game runs the full 130 turns and
    // `bot::run` reaches the until-turn with `Stop::Human` for ANY seed and under any AI/content
    // change. The subject of this test is the journal-replay invariant (replay reproduces the same
    // final save), asserted below; we only need a long, varied, journaled game to exercise it.
    let ng = peaceful_game(22);
    let (mut s, mut b) = bot_session(&ng, Some(dir.0.clone()));
    let stop = bot::run(&mut s, &mut b, 130, |_, _| {});
    // True by construction (an always-peace, barbarian-free game cannot end before the until-turn).
    assert_eq!(stop, Stop::Human);
    // No assertion that a blocking decision was journaled: whether one occurs depends on the
    // AI/content (FfH locks religion-pick off, free techs and trades depend on content, and a
    // capture cannot be forced here -- construct_capture mutates the game directly, outside the
    // journal, so a replay from the header, a fresh new game, would diverge from it). The journal's
    // command/checkpoint coverage below, and the byte-identical replays, are the subject.
    let (final_save, _) = s.save_bytes().unwrap();
    let path = s.journal_path().expect("a journal file");
    let lines = journal::read_file(&path).unwrap();
    let count = |k: &str| lines.iter().filter(|l| l.kind() == k).count();
    eprintln!("journal {}: {} lines, {} commands, {} answers, {} checkpoints", path.display(), lines.len(), count("cmd"), count("answer"), count("checkpoint"));
    assert!(matches!(lines[0], Line::New { .. }));
    // A full run's worth of journaled commands and turn-boundary checkpoints (coverage that the
    // game ran the whole length and the inputs were recorded, not a particular emergent outcome).
    assert!(count("cmd") >= 100 && count("checkpoint") >= 100);
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

/// The AI autoplay stopped while it runs (`SessionShared::request_stop_autoplay`, what the HTTP
/// transport does on `autoplay {turns: 0}`; P6 review): the human has control at its next turn,
/// the stop is journaled at its turn slice, and the journal replays to the same game (the replay
/// applies the stop at that slice).
#[test]
fn an_autoplay_stopped_while_it_runs_replays() {
    use ffh_play::human::AdvisorAnswerer;
    use ffh_play::{CmdOk, Command, NewGame};
    let dir = TempDir::new("autoplay-stop");
    let d = ffh_data::global();
    let mut ng = NewGame::new(d, 4);
    ng.opponents = 1;
    ng.world = d.t("WORLDSIZE_DUEL");
    let opts = SessionOptions { saves_dir: Some(dir.0.clone()), ..SessionOptions::default() };
    let mut s = Session::new_game(&ng, std::sync::Arc::new(AdvisorAnswerer), opts).unwrap();
    assert_eq!(s.advance(&mut |_| {}), Stop::Human);
    let from = s.g.game_turn();
    assert!(matches!(s.execute(&Command::AutoPlay { turns: 400 }), Ok(CmdOk::Applied { .. })));
    let mut asked = None;
    let stop = s.advance(&mut |s| {
        if asked.is_none() && s.g.game_turn() >= from + 3 && !s.g.players[s.human].turn_active {
            s.shared.request_stop_autoplay();
            asked = Some(s.g.game_turn());
        }
    });
    assert_eq!(stop, Stop::Human);
    let asked = asked.expect("the stop asked");
    assert_eq!(s.g.ai_auto_play, 0);
    assert!(s.g.players[s.human].human);
    assert_eq!(s.g.game_turn(), asked + 1, "control at the human's next turn");
    let (final_save, _) = s.save_bytes().unwrap();
    let lines = journal::read_file(&s.journal_path().unwrap()).unwrap();
    let stops: Vec<_> = lines.iter().filter(|l| matches!(l, Line::Cmd { cmd, .. } if journal::is_running_cmd(cmd))).collect();
    assert_eq!(stops.len(), 1, "{stops:?}");
    drop(s);
    let r = replay(lines, None, SessionOptions::default()).unwrap_or_else(|e| panic!("replay: {e}"));
    assert!(r.g.save() == final_save, "the replay ends in a different game");
    assert_eq!(r.g.ai_auto_play, 0);
}

/// A journal that starts with a `load` header replays from the save that header names (with its
/// sidecar, as the session loaded it), and one that starts with `take_over` from the AI save it
/// names; replaying either from its header alone asks for that save.
#[test]
fn t4_replay_load_and_take_over_journals() {
    let dir = TempDir::new("replay-load");
    // Load: a bot game saved at turn 8, loaded into a new saves directory, played to turn 14.
    let ng = new_game(4);
    let (mut s, mut b) = bot_session(&ng, Some(dir.0.join("first")));
    assert_eq!(bot::run(&mut s, &mut b, 8, |_, _| {}), Stop::Human);
    let paths = s.save("t8").unwrap();
    drop(s);
    let (save, sidecar) = read_bundle(&paths.save).unwrap();
    let opts = SessionOptions { saves_dir: Some(dir.0.join("loaded")), ..Default::default() };
    let mut s = Session::load(&save, sidecar.as_ref(), b.answerer(), opts).unwrap();
    assert_eq!(bot::run(&mut s, &mut b, 14, |_, _| {}), Stop::Human);
    let (final_save, _) = s.save_bytes().unwrap();
    let lines = journal::read_file(&s.journal_path().unwrap()).unwrap();
    drop(s);
    assert!(matches!(lines[0], Line::Load { .. }));
    assert!(lines.iter().any(|l| l.kind() == "cmd"));
    let err = replay(lines.clone(), None, SessionOptions::default()).err().expect("a load journal needs its save");
    assert!(err.contains("--from"), "{err}");
    let r = replay(lines.clone(), Some(ReplayFrom { save: &save, sidecar: sidecar.as_ref() }), SessionOptions::default())
        .unwrap_or_else(|e| panic!("replay from the loaded save: {e}"));
    assert!(r.g.save() == final_save, "replay from the loaded save ends in a different game");
    // An autosave of the loaded session is at one of its checkpoints.
    let auto = dir.0.join("loaded/auto/T0012.ffhsave");
    let (save12, sidecar12) = read_bundle(&auto).unwrap();
    let r = replay(lines.clone(), Some(ReplayFrom { save: &save12, sidecar: sidecar12.as_ref() }), SessionOptions::default())
        .unwrap_or_else(|e| panic!("replay from {}: {e}", auto.display()));
    assert!(r.g.save() == final_save);

    // Take over: slot 1 of an AI-only duel at a turn boundary, played for 4 turns.
    let d = ffh_data::global();
    let o = ffh_core::setup::SetupOptions::new(d, 2, "WORLDSIZE_DUEL", 21);
    let mut g = ffh_core::game::Game::new_with_options(d, &o);
    g.map_script = std::sync::Arc::new(ffh_core::mapscript::FractalMapScript::default());
    let ai = ffh_ai::ai_controller();
    for p in g.player_ids() {
        g.controllers[p] = ai.clone();
    }
    g.autorun = true;
    g.game_init();
    while g.game_turn() < 6 || !g.at_turn_boundary() {
        g.update();
    }
    let ai_save = g.save();
    let mut b = bot::Bot::new(21);
    let opts = SessionOptions { saves_dir: Some(dir.0.join("taken")), ..Default::default() };
    let mut s = Session::take_over(&ai_save, ffh_core::ids::PlayerId(1), b.answerer(), opts).unwrap();
    assert_eq!(bot::run(&mut s, &mut b, 10, |_, _| {}), Stop::Human);
    let (final_save, _) = s.save_bytes().unwrap();
    let lines = journal::read_file(&s.journal_path().unwrap()).unwrap();
    drop(s);
    assert!(matches!(lines[0], Line::TakeOver { .. }));
    assert!(replay(lines.clone(), None, SessionOptions::default()).is_err());
    let r = replay(lines, Some(ReplayFrom { save: &ai_save, sidecar: None }), SessionOptions::default())
        .unwrap_or_else(|e| panic!("replay from the AI save: {e}"));
    assert_eq!(r.human, ffh_core::ids::PlayerId(1));
    assert!(r.g.save() == final_save, "replay from the AI save ends in a different game");
}
