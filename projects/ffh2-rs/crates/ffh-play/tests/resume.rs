//! T3: mid-turn save and resume (design §8). A bot game is played twice: straight through, and
//! with the session saved and reloaded (fresh controllers, the same bot) at control points
//! (turn 10 before acting; turns 37 and 61 after the bot's first commands; the first control
//! point with a deferred event pending; the first one after a blocking decision) and at a turn
//! boundary (an autosave). Both runs must agree at every control point and in the final save;
//! `save(load(save)) == save` at every reload.

mod common;

use std::sync::Arc;

use common::{bot_session, checkpoint, construct_capture, peaceful_game, TempDir};
use ffh_play::human::Answerer;
use ffh_play::save::{read_bundle, Sidecar};
use ffh_play::{CmdOk, Command, Phase, Session, SessionOptions, Stop};

/// Saves `s` and replaces it with a session loaded from that save.
fn reload(s: &mut Session, answerer: &Arc<dyn Answerer>, opts: &SessionOptions, why: &str, log: &mut Vec<String>) {
    let (bytes, sidecar) = s.save_bytes().unwrap_or_else(|e| panic!("save ({why}): {e}"));
    let before = checkpoint(s);
    let pending = s.pending();
    let n = Session::load(&bytes, Some(&sidecar), answerer.clone(), SessionOptions { journal: false, ..Default::default() }).unwrap();
    assert!(n.g.save() == bytes, "save(load(save)) != save ({why})");
    *s = Session::load(&bytes, Some(&sidecar), answerer.clone(), opts.clone()).unwrap();
    assert_eq!(checkpoint(s), before);
    assert_eq!(s.pending(), pending, "pending items changed by the reload ({why})");
    log.push(format!("turn {}: {why}", before.0));
}

/// Replaces `s` (at a control point) with the last autosave, run forward to the same point.
fn reload_boundary(s: &mut Session, answerer: &Arc<dyn Answerer>, opts: &SessionOptions, log: &mut Vec<String>) {
    let here = checkpoint(s);
    let path = s.last_autosave().expect("an autosave").save.clone();
    let (bytes, sidecar): (Vec<u8>, Option<Sidecar>) = read_bundle(&path).unwrap();
    let mut n = Session::load(&bytes, sidecar.as_ref(), answerer.clone(), opts.clone()).unwrap();
    assert!(n.g.at_turn_boundary());
    assert_eq!(n.advance(&mut |_| {}), Stop::Human);
    assert_eq!(checkpoint(&n), here, "resumed from {} to a different control point", path.display());
    *s = n;
    log.push(format!("turn {}: from the autosave {}", here.0, path.display()));
}

fn run(seed: u32, end: i32, swap: Option<&TempDir>) -> (Vec<(i32, i32)>, Vec<u8>, Vec<String>) {
    // Always-peace, barbarian-free: no AI can declare war on or eliminate the human and no victory
    // can fire, so the bot game runs the full `end` turns and every control point is reached for
    // ANY seed and under any AI/content change (the reload kinds that just need a long varied game:
    // before acting, a deferred event pending, the autosave boundary, mid-turn).
    let ng = peaceful_game(seed);
    let (mut s, mut bot) = bot_session(&ng, swap.map(|d| d.0.clone()));
    let opts = SessionOptions { saves_dir: swap.map(|d| d.0.clone()), ..Default::default() };
    let answerer = bot.answerer();
    let mut checkpoints = Vec::new();
    let mut log = Vec::new();
    let (mut event_done, mut boundary_done, mut captured) = (false, false, false);
    loop {
        let stop = s.advance(&mut |_| {});
        if stop != Stop::Human || s.g.game_turn() >= end {
            break;
        }
        checkpoints.push(checkpoint(&s));
        let turn = s.g.game_turn();
        // For the "after a blocking decision" reload: construct a city capture at a known control
        // point so a `keep_city` decision is raised and answered deterministically, with no reliance
        // on the AI/content producing a capture or any other decision. Done identically in BOTH runs
        // (so they stay in lockstep); peace is restored at once so the always-peace game still runs
        // the full length. The reload happens right here, straight after the decision (a reload
        // recreates the session's stats, so this cannot lean on a `decisions_total()` check at a
        // later control point -- an intervening reload would have zeroed it).
        if !captured && turn >= 30 {
            captured = true;
            let h = s.human;
            let (warrior, cx, cy) = construct_capture(&mut s);
            let before = s.stats().decisions_total();
            match s.execute(&Command::Move { units: vec![warrior], x: cx, y: cy, append: false, declare_war: None }) {
                Ok(CmdOk::Applied { .. }) => {}
                other => panic!("capture move: {other:?}"),
            }
            assert!(s.stats().decisions_total() > before, "the constructed capture raised no blocking decision");
            let my = s.g.players[h].team;
            for t in s.g.civ_team_ids().collect::<Vec<_>>() {
                if t != my && s.g.at_war(my, t) {
                    s.g.team_make_peace(my, t, false);
                }
            }
            if swap.is_some() {
                reload(&mut s, &answerer, &opts, "after a blocking decision", &mut log);
            }
        }
        if swap.is_some() {
            if turn == 10 {
                reload(&mut s, &answerer, &opts, "control point, before acting", &mut log);
            }
            if !event_done && turn > 10 && !s.pending().events.is_empty() {
                event_done = true;
                reload(&mut s, &answerer, &opts, "deferred event pending", &mut log);
            }
            if !boundary_done && turn >= 75 {
                boundary_done = true;
                reload_boundary(&mut s, &answerer, &opts, &mut log);
            }
            if turn == 37 || turn == 61 {
                bot.play_with(&mut s, &mut |s| {
                    assert!(matches!(s.phase(), Phase::Human(_)));
                    reload(s, &answerer, &opts, "mid-turn, after commands", &mut log);
                });
                continue;
            }
        }
        bot.play(&mut s);
    }
    let (save, _) = s.save_bytes().unwrap();
    (checkpoints, save, log)
}

#[test]
fn t3_save_and_resume_mid_turn() {
    // The base game is made peaceful (see `run`) so it runs the full 150 turns for any seed, and a
    // city capture is constructed at a known control point so the keep/raze decision occurs
    // deterministically -- no reliance on a seed producing a particular emergent game. The two runs
    // (straight through, and with saves/reloads) must still agree at every control point and in the
    // final save, and `save(load(save)) == save` must hold at every reload.
    let end = 150;
    let (a, a_save, _) = run(22, end, None);
    let dir = TempDir::new("resume");
    let (b, b_save, log) = run(22, end, Some(&dir));
    eprintln!("reloads: {log:#?}");
    for what in ["before acting", "deferred event", "blocking decision", "autosave", "mid-turn"] {
        assert!(log.iter().any(|l| l.contains(what)), "no reload {what:?}: {log:?}");
    }
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x, y, "the resumed game diverged");
    }
    assert!(a_save == b_save, "final saves differ");
}
