//! T3: mid-turn save and resume (design §8). A bot game is played twice: straight through, and
//! with the session saved and reloaded (fresh controllers, the same bot) at control points
//! (turn 10 before acting; turns 37 and 61 after the bot's first commands; the first control
//! point with a deferred event pending; the first one after a blocking decision) and at a turn
//! boundary (an autosave). Both runs must agree at every control point and in the final save;
//! `save(load(save)) == save` at every reload.

mod common;

use std::sync::Arc;

use common::{bot_session, checkpoint, new_game, TempDir};
use ffh_play::human::Answerer;
use ffh_play::save::{read_bundle, Sidecar};
use ffh_play::{Phase, Session, SessionOptions, Stop};

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
    let ng = new_game(seed);
    let (mut s, mut bot) = bot_session(&ng, swap.map(|d| d.0.clone()));
    let opts = SessionOptions { saves_dir: swap.map(|d| d.0.clone()), ..Default::default() };
    let answerer = bot.answerer();
    let mut checkpoints = Vec::new();
    let mut log = Vec::new();
    let (mut event_done, mut decision_done, mut boundary_done) = (false, false, false);
    loop {
        let stop = s.advance(&mut |_| {});
        if stop != Stop::Human || s.g.game_turn() >= end {
            break;
        }
        checkpoints.push(checkpoint(&s));
        let turn = s.g.game_turn();
        if swap.is_some() {
            if turn == 10 {
                reload(&mut s, &answerer, &opts, "control point, before acting", &mut log);
            }
            if !event_done && turn > 10 && !s.pending().events.is_empty() {
                event_done = true;
                reload(&mut s, &answerer, &opts, "deferred event pending", &mut log);
            }
            if !decision_done && s.stats().decisions_total() > 0 {
                decision_done = true;
                reload(&mut s, &answerer, &opts, "after a blocking decision", &mut log);
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
    // Seed 3: the bot captures a city (a keep/raze decision) during the game.
    let end = 150;
    let (a, a_save, _) = run(3, end, None);
    let dir = TempDir::new("resume");
    let (b, b_save, log) = run(3, end, Some(&dir));
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
