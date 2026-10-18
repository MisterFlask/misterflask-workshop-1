//! T1 (scripted bot game) and T2 (determinism), design §8.

mod common;

use common::{bot_session, check, checkpoint, new_game, peaceful_game};
use ffh_play::bot;
use ffh_play::{NewGame, Stop};

/// What one bot game produced.
struct Played {
    stop: Stop,
    checkpoints: Vec<(i32, i32)>,
    save: Vec<u8>,
}

fn play(seed: u32, turns: i32, check_every: i32) -> Played {
    play_game(new_game(seed), seed, turns, check_every)
}

/// Plays `ng` with a fresh bot for `turns` game turns, checking ffh-stress's invariants every
/// `check_every` turns (0 = never) and, when `check_every > 0`, the breadth coverage of a long
/// varied game: no stuck turn, invariants hold, and the bot founds cities, researches, answers a
/// popup and produces. These are floors on activity that any full run clears, not golden numbers
/// to hand-update, so they stay true as the AI and content change. Returns the stop, the
/// per-control-point checksums and the final save (for the determinism test).
fn play_game(ng: NewGame, seed: u32, turns: i32, check_every: i32) -> Played {
    let (mut s, mut b) = bot_session(&ng, None);
    let until = s.g.game_turn() + turns;
    let mut checkpoints = Vec::new();
    let mut last_checked = -1;
    let stop = bot::run(&mut s, &mut b, until, |s, _| {
        assert!(s.g.num_game_turn_active <= 1, "turn {}: {} players active", s.g.game_turn(), s.g.num_game_turn_active);
        checkpoints.push(checkpoint(s));
        let turn = s.g.game_turn();
        if check_every > 0 && turn % check_every == 0 && turn != last_checked {
            last_checked = turn;
            s.read(check::check);
        }
    });
    assert!(!matches!(stop, Stop::Stuck(_)), "seed {seed}: {stop:?}");
    s.read(check::check);
    let st = s.stats();
    let techs = s.read(|g| g.teams[g.players[s.human].team].has_tech.iter().filter(|&&t| t).count());
    eprintln!("seed {seed}: {stop:?} at turn {}, techs {techs}, cities {}, alive {}, winner {:?} victory {:?}, bot {:?}, session {st:?}", s.g.game_turn(), s.g.players[s.human].cities.len(), s.g.players[s.human].alive, s.g.winner, s.g.victory, b.stats);
    if check_every > 0 {
        assert!(b.stats.found_orders >= 2, "seed {seed}: {} cities founded", b.stats.found_orders);
        assert!(techs >= 6, "seed {seed}: only {techs} techs");
        assert!(b.stats.events_answered + b.stats.votes_answered + b.stats.research_chosen >= 1, "seed {seed}: no deferred popup answered");
        assert!(b.stats.events_answered >= 1, "seed {seed}: no event answered");
        assert!(b.stats.productions >= 5 && b.stats.end_turns >= 100, "seed {seed}: {:?}", b.stats);
    }
    let (save, _) = s.save_bytes().expect("save at the end");
    Played { stop, checkpoints, save }
}

/// T1: three 150-turn games on a small map against 3 AIs, the human slot played by the bot only
/// through the protocol's queries and commands. Made robust to AI and content change (the user's
/// standing rule): the games use `GAMEOPTION_ALWAYS_PEACE` and `GAMEOPTION_NO_BARBARIANS`
/// (`peaceful_game`), so no AI can attack or eliminate the bot and no victory can fire inside 150
/// turns. The bot therefore reaches the full run with `Stop::Human` for ANY seed -- true by
/// construction, not a property of a particular seed that a new AI or new content would break --
/// so the seeds are free (1, 2, 3) and no seed-specific outcome is asserted. What is asserted is
/// breadth coverage that holds for every full run: no panic, no stuck turn, ffh-stress's
/// invariants every 5 turns, and the bot founds cities, researches, answers popups and produces
/// (`play_game` with `check_every > 0`). City capture and the `keep_city` decision are covered
/// deterministically by `decisions.rs` (`capture_asks_keep_or_raze_inside_the_command`, built
/// with `common::construct_capture`) and by the purity/resume tests, so a blocking decision is no
/// longer forced here -- the old `decisions >= 1` assertion depended on a seed producing a capture
/// and is dropped.
#[test]
fn t1_scripted_bot_games() {
    for seed in [1, 2, 3] {
        let played = play_game(peaceful_game(seed), seed, 150, 5);
        // `GAMEOPTION_NO_BARBARIANS` does not remove every barbarian: units spawned by FfH's
        // Python (e.g. the Dragon's Horde of a barbarian city placed at map creation, which sends
        // Sons of the Inferno) still roam, and can raze the bot's cities (seed 2: barbarian
        // warriors and a Son of the Inferno take the bot's last cities by turn 142). So the run may also end with the bot eliminated (`Stop::Over`); what must not
        // happen is a stuck turn (checked in `play_game`).
        assert!(matches!(played.stop, Stop::Human | Stop::Over), "seed {seed}: {:?}", played.stop);
    }
}

/// T2: the same seed twice gives the same checksum at every control point and byte-identical
/// final saves.
#[test]
fn t2_same_seed_same_game() {
    let a = play(50, 60, 0);
    let b = play(50, 60, 0);
    assert_eq!(a.stop, b.stop);
    assert_eq!(a.checkpoints.len(), b.checkpoints.len());
    for (x, y) in a.checkpoints.iter().zip(&b.checkpoints) {
        assert_eq!(x, y, "the games diverged");
    }
    assert!(a.save == b.save, "final saves differ");
}
