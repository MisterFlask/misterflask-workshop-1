//! Saved games resume exactly: a game saved at a turn boundary, loaded (with fresh AI
//! controllers, as in a new process) and continued plays out like the uninterrupted game --
//! same `calculate_sync_checksum` and byte-identical saves at every later turn boundary.
//!
//! Lives in ffh-ai because it needs both the rules (`ffh-core`) and the AI that drives them
//! (ffh-core cannot depend on ffh-ai). The setup mirrors `ffh-cli`'s `run::new_game`.

use std::sync::Arc;

use ffh_core::game::Game;
use ffh_core::mapscript::FractalMapScript;
use ffh_core::setup::GameSetup;
use ffh_data::GameStateType;

fn attach_ai(g: &mut Game) {
    let ai = ffh_ai::ai_controller();
    for p in g.player_ids() {
        g.controllers[p] = ai.clone();
    }
}

fn new_game(seed: u32, civs: usize, world: &str) -> Game {
    let d = ffh_data::global();
    let setup = GameSetup::standard(d, civs, world, seed);
    let mut g = Game::new(d, setup);
    g.map_script = Arc::new(FractalMapScript::default());
    attach_ai(&mut g);
    g.autorun = true;
    g.game_init();
    g
}

fn running(g: &Game) -> bool {
    g.game_state == GameStateType::GAMESTATE_ON
}

/// Plays to the next turn boundary: `update()` until no player's turn is active.
fn play_turn(g: &mut Game) {
    for _ in 0..200_000 {
        g.update();
        if g.at_turn_boundary() || !running(g) {
            return;
        }
    }
    panic!("turn {} does not end", g.game_turn());
}

fn play_to(g: &mut Game, turn: i32) {
    while running(g) && g.game_turn() < turn {
        play_turn(g);
    }
}

/// Save, load into a fresh game with fresh controllers, and check the save round-trips.
fn reload(g: &Game) -> (Game, usize) {
    let bytes = g.save();
    let mut h = Game::load(g.d, &bytes).unwrap_or_else(|e| panic!("load at turn {}: {e}", g.game_turn()));
    assert_saves_equal(&h.save(), &bytes, "save(load(save(g))) != save(g)");
    h.map_script = Arc::new(FractalMapScript::default());
    attach_ai(&mut h);
    (h, bytes.len())
}

fn assert_saves_equal(a: &[u8], b: &[u8], what: &str) {
    if a != b {
        let at = a.iter().zip(b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
        panic!("{what}: {} vs {} bytes, first difference at byte {at}", a.len(), b.len());
    }
}

/// Conflicts so far: cities that changed owner, dead units (proxy for combat).
fn history(g: &Game) -> (usize, usize) {
    let captured = g.cities.iter().filter(|(_, c)| c.previous_owner.is_some() || c.owner != c.original_owner).count();
    let dead_units = g.units.next_id().0 as usize - g.units.len();
    (captured, dead_units)
}

/// Plays `a` to `save_turn`, then continues it and a reloaded copy side by side for
/// `more_turns`, reloading the copy again at each turn in `reload_at`.
fn check_resume(seed: u32, civs: usize, world: &str, save_turn: i32, more_turns: i32, reload_at: &[i32]) {
    let mut a = new_game(seed, civs, world);
    play_to(&mut a, save_turn);
    assert!(running(&a), "game over before turn {save_turn}");
    // A freshly initialized game (before its first update) is also a safe point.
    assert!(save_turn == 0 || a.at_turn_boundary());
    let (mut b, size) = reload(&a);
    let (captured, dead) = history(&a);
    eprintln!(
        "seed {seed} {world}: saved at turn {} ({size} bytes; {} cities, {} units, {captured} cities captured, {dead} units died)",
        a.game_turn(),
        a.cities.len(),
        a.units.len()
    );
    let end = save_turn + more_turns;
    while running(&a) && a.game_turn() < end {
        play_turn(&mut a);
        play_turn(&mut b);
        let t = a.game_turn();
        assert_eq!(b.game_turn(), t, "turn counters diverged");
        assert_eq!(b.calculate_sync_checksum(), a.calculate_sync_checksum(), "sync checksum diverged at turn {t}");
        assert_saves_equal(&b.save(), &a.save(), &format!("saves diverged at turn {t}"));
        if reload_at.contains(&t) {
            b = reload(&b).0;
            let (captured, dead) = history(&b);
            eprintln!("seed {seed} {world}: reloaded at turn {t} ({captured} cities captured, {dead} units died)");
        }
    }
    assert_eq!(a.game_state, b.game_state);
    let (captured, dead) = history(&a);
    eprintln!("seed {seed} {world}: identical through turn {} ({captured} cities captured, {dead} units died)", a.game_turn());
}

#[test]
fn resume_from_turn_zero() {
    check_resume(21, 3, "WORLDSIZE_DUEL", 0, 25, &[]);
}

#[test]
fn resume_small_with_repeated_reloads() {
    check_resume(1, 4, "WORLDSIZE_SMALL", 40, 50, &[45, 46, 70]);
}

#[test]
fn resume_standard_midgame() {
    check_resume(11, 7, "WORLDSIZE_STANDARD", 100, 70, &[130, 165]);
}

#[test]
fn resume_standard_late_game_after_wars() {
    check_resume(3, 7, "WORLDSIZE_STANDARD", 160, 50, &[185]);
}
