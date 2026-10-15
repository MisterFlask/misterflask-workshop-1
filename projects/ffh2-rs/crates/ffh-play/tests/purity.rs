//! T10: purity, the noisy client (design §8). The T1 bot plays the same game twice: quietly,
//! and as a noisy client that issues every allow-listed query after every command, at every
//! control point and inside every blocking decision (view, map, every group's details and
//! actions, 5 path and 5 combat previews, every city screen, techs, civics, religions, players,
//! trade, inspect, text, ascii). Checksums at every control point and the final saves must be
//! equal. A city screen, which fills the city rank caches (F11), leaves the save bytes as they
//! were.

mod common;

use std::sync::Arc;

use common::{checkpoint, new_game};
use ffh_play::bot::{self, Bot};
use ffh_play::human::Answerer;
use ffh_play::query::Query;
use ffh_play::{Session, SessionOptions, Stop};

struct Run {
    checkpoints: Vec<(i32, i32)>,
    save: Vec<u8>,
    queries: usize,
    decision_queries: usize,
    decisions: usize,
}

fn play(seed: u32, turns: i32, noisy: bool) -> Run {
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
    let until = s.g.game_turn() + turns;
    let mut checkpoints = Vec::new();
    let stop = bot::run(&mut s, &mut b, until, |s, _| checkpoints.push(checkpoint(s)));
    assert_eq!(stop, Stop::Human);
    let (save, _) = s.save_bytes().unwrap();
    let (decision_queries, decisions) = probe.map_or((0, 0), |p| (*p.queries.lock().unwrap(), *p.decisions.lock().unwrap()));
    assert_eq!(decisions, if noisy { s.stats().decisions_total() } else { 0 }, "the noisy answerer saw every decision");
    Run { checkpoints, save, queries: b.stats.queries, decision_queries, decisions: s.stats().decisions_total() }
}

#[test]
fn t10_the_noisy_client_changes_nothing() {
    // Seed 18: the bot captures a city at turn 90 (a keep/raze decision). (Seed 50 did at turn
    // 85 until the round-3 AI merge; its human is now conquered at turn 98.)
    let quiet = play(18, 100, false);
    let noisy = play(18, 100, true);
    eprintln!("noisy client: {} queries, {} inside {} decisions", noisy.queries, noisy.decision_queries, noisy.decisions);
    assert!(quiet.decisions >= 1 && noisy.decisions >= 1, "the game should contain a blocking decision");
    assert!(noisy.queries > 5_000 && noisy.decision_queries > 10, "{} queries, {} in decisions", noisy.queries, noisy.decision_queries);
    assert_eq!(quiet.checkpoints.len(), noisy.checkpoints.len());
    for (a, b) in quiet.checkpoints.iter().zip(&noisy.checkpoints) {
        assert_eq!(a, b, "the noisy client's game diverged");
    }
    assert!(quiet.save == noisy.save, "final saves differ");
}

/// A city screen reads happiness, which fills the city rank caches (F11); the query leaves the
/// save bytes as they were. The same reads without the guard do change them (the test is not
/// vacuous).
#[test]
fn a_city_screen_leaves_the_save_unchanged() {
    let ng = new_game(3);
    let mut b = Bot::new(3);
    let mut s = Session::new_game(&ng, b.answerer(), SessionOptions { journal: false, ..Default::default() }).unwrap();
    let until = s.g.game_turn() + 40;
    assert_eq!(bot::run(&mut s, &mut b, until, |_, _| {}), Stop::Human);
    let h = s.human;
    let cities = s.g.players[h].cities.clone();
    assert!(!cities.is_empty());
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
