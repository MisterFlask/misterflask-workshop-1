//! T10: purity, the noisy client (design §8). The T1 bot plays the same game twice: quietly,
//! and as a noisy client that issues every allow-listed query after every command, at every
//! control point and inside every blocking decision (view, map, every group's details and
//! actions, 5 path and 5 combat previews, every city screen, techs, civics, religions, players,
//! trade, inspect, text, ascii). Checksums at every control point and the final saves must be
//! equal. A city screen, which fills the city rank caches (F11), leaves the save bytes as they
//! were.

mod common;

use std::sync::Arc;

use common::{checkpoint, new_game, peaceful_game};
use ffh_play::bot::{self, Bot};
use ffh_play::command::{CmdOk, Command};
use ffh_play::human::Answerer;
use ffh_play::query::Query;
use ffh_play::{Phase, Session, SessionOptions, Stop};

struct Run {
    checkpoints: Vec<(i32, i32)>,
    save: Vec<u8>,
    queries: usize,
    decision_queries: usize,
    decisions: usize,
}

/// One T10 run: `warmup` bot turns build a believable board, then a CONSTRUCTED capture (pattern
/// B) drives the blocking decision the noisy client must read through. Both the warm-up and the
/// capture are identical for `quiet` and `noisy` (queries are read-only), so the two runs must
/// agree at every checkpoint and in the final save; the game ending or the AI capturing a city on
/// its own is never assumed. Checkpoints are the control points of the warm-up plus the control
/// point reached before the capture and the one right after it.
fn play(seed: u32, warmup: i32, noisy: bool) -> Run {
    let ng = new_game(seed);
    let mut b = Bot::new(seed);
    b.noisy = noisy;
    let (probe, answerer): (Option<Arc<bot::NoisyAnswerer>>, Arc<dyn Answerer>) = if noisy {
        let (probe, a) = b.noisy_answerer();
        (Some(probe), a)
    } else {
        (None, b.answerer())
    };
    let mut s = Session::new_game(&ng, answerer, SessionOptions::default()).unwrap();
    let mut checkpoints = Vec::new();
    // Warm-up: a handful of bot turns. No game ends this early (no victory, no elimination) for
    // any seed, so the run reaches its until-turn with `Stop::Human` by construction -- not a
    // seed-specific outcome. The noisy bot's reads after every command and at every control point
    // give the "many queries" coverage below.
    let until = s.g.game_turn() + warmup;
    assert_eq!(bot::run(&mut s, &mut b, until, |s, _| checkpoints.push(checkpoint(s))), Stop::Human);
    assert_eq!(s.phase(), Phase::Human(s.human));
    checkpoints.push(checkpoint(&s));
    // Construct a deterministic capture of an undefended AI city and drive the `Move` that raises
    // the `keep_city` decision inside the command -- no combat RNG, so the capture does not depend
    // on the seed, the AI or the content; the seed only shaped the warm-up board.
    let (warrior, cx, cy) = common::construct_capture(&mut s);
    let before = s.stats().decisions_of("keep_city");
    match s.execute(&Command::Move { units: vec![warrior], x: cx, y: cy, append: false, declare_war: None }) {
        Ok(CmdOk::Applied { .. }) => {}
        other => panic!("the capture move was not applied: {other:?}"),
    }
    assert_eq!(s.stats().decisions_of("keep_city"), before + 1, "the capture raised one keep_city decision");
    checkpoints.push(checkpoint(&s));
    let (save, _) = s.save_bytes().unwrap();
    let (decision_queries, decisions) = probe.map_or((0, 0), |p| (*p.queries.lock().unwrap(), *p.decisions.lock().unwrap()));
    assert_eq!(decisions, if noisy { s.stats().decisions_total() } else { 0 }, "the noisy answerer saw every decision");
    Run { checkpoints, save, queries: b.stats.queries, decision_queries, decisions: s.stats().decisions_total() }
}

/// The noisy client changes nothing: quiet and noisy drive the SAME warm-up and the SAME
/// constructed capture, so every control-point checksum and the final save must be equal, and the
/// noisy answerer must have seen the (constructed) blocking decision and issued read-only queries
/// inside it. The capture is constructed (pattern B), not fished out of a particular seed's
/// emergent game, so the test holds for any seed and under any AI/content change.
#[test]
fn t10_the_noisy_client_changes_nothing() {
    let quiet = play(22, 12, false);
    let noisy = play(22, 12, true);
    eprintln!("noisy client: {} queries, {} inside {} decisions", noisy.queries, noisy.decision_queries, noisy.decisions);
    // Coverage (true by construction: the capture always raises a keep_city decision), not a
    // pinned number: the noisy client probed heavily, including inside the decision.
    assert!(quiet.decisions >= 1 && noisy.decisions >= 1, "the game should contain a blocking decision");
    assert!(noisy.queries > 200 && noisy.decision_queries > 10, "{} queries, {} in decisions", noisy.queries, noisy.decision_queries);
    // The equality invariants: the noisy client is read-only, so the two games never diverge.
    assert_eq!(quiet.checkpoints.len(), noisy.checkpoints.len());
    for (a, b) in quiet.checkpoints.iter().zip(&noisy.checkpoints) {
        assert_eq!(a, b, "the noisy client's game diverged");
    }
    assert!(quiet.save == noisy.save, "final saves differ");
}

/// A city screen reads happiness, which fills the city rank caches (F11); the query leaves the
/// save bytes as they were. The same reads without the guard do change them (the test is not
/// vacuous). Pattern A: [`peaceful_game`] (`GAMEOPTION_ALWAYS_PEACE` + `GAMEOPTION_NO_BARBARIANS`)
/// so the bot game reaches the until-turn with `Stop::Human` for any seed, and the peaceful bot
/// keeps the capital it founds early -- the city screen we probe has a city by construction, not
/// by a seed-specific outcome.
#[test]
fn a_city_screen_leaves_the_save_unchanged() {
    let ng = peaceful_game(3);
    let mut b = Bot::new(3);
    let mut s = Session::new_game(&ng, b.answerer(), SessionOptions { journal: false, ..Default::default() }).unwrap();
    let until = s.g.game_turn() + 40;
    // True by construction (always-peace, no barbarians): the run reaches the until-turn.
    assert_eq!(bot::run(&mut s, &mut b, until, |_, _| {}), Stop::Human);
    let h = s.human;
    let cities = s.g.players[h].cities.clone();
    assert!(!cities.is_empty(), "a peaceful bot founds and keeps at least one city to open a city screen on");
    let invalidate = |s: &Session| {
        for (_, c) in s.g.cities.iter() {
            c.population_rank_valid.set(false);
            for v in c.base_yield_rank_valid.iter().chain(&c.yield_rank_valid).chain(&c.commerce_rank_valid) {
                v.set(false);
            }
        }
    };
    invalidate(&s);
    let before = s.g.save();
    for &c in &cities {
        s.query(&Query::City { city: c }).unwrap();
    }
    s.query(&Query::View).unwrap();
    s.query(&Query::Text).unwrap();
    assert!(before == s.g.save(), "a city screen changed the save");
    for &c in &cities {
        let _ = s.g.city_largest_city_happiness(c);
    }
    assert!(before != s.g.save(), "unguarded happiness reads should fill the rank caches");
}
