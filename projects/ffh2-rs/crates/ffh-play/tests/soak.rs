//! T5: AI-as-human soak smoke (design §8; the short CI variant of P2's soak, `ffh-play soak`).
//! The human slot is played by an `ffh_ai::Ai` through the session (`ffh_play::soak`): production
//! and unit moves by the AI on the human's behalf, research, events and votes answered through
//! the commands with the AI's choice (or left open one turn in seven), blocking decisions by the
//! advisor. The human-only rules branches run every turn; the invariants are checked every 5
//! turns and the save round trip every 25.

mod common;

use common::TempDir;
use ffh_core::game::Game;
use ffh_data::*;
use ffh_play::soak::{self, End, Outcome, RunOptions, SoakGame};

fn game(seed: u32, world: &str, turns: i32) -> SoakGame {
    let d = ffh_data::global();
    let mut g = SoakGame::new(d, seed);
    g.ng.world = d.t::<WorldSizeType>(world);
    g.ng.opponents = soak::default_opponents(d, g.ng.world);
    g.turns = turns;
    g
}

fn play(g: &SoakGame, opts: &RunOptions) -> Outcome {
    soak::run(g, opts, &mut |_| {}).unwrap_or_else(|e| panic!("{}: {e}", g.describe(ffh_data::global())))
}

#[test]
fn t5_ai_as_human_soak_smoke() {
    let d = ffh_data::global();
    let mut popups = 0;
    let (mut research_left_open, mut research_ai_picks) = (0, 0);
    for seed in [1, 2] {
        let mut g = game(seed, "WORLDSIZE_SMALL", 150);
        g.ng.opponents = 3;
        let o = play(&g, &RunOptions::default());
        eprintln!("{}: {} turn {} {:?} {:?} cities {} vs {:?}", g.describe(d), o.end_text(d), o.turn, o.stats, o.session, o.human_cities, o.ai_cities);
        assert_eq!(o.end, End::Turns, "seed {seed}");
        assert_eq!(o.turn, 150);
        assert!(o.stats.turns_played >= 75, "seed {seed}: {:?}", o.stats);
        assert!(o.stats.checks >= 30 && o.stats.save_checks >= 6, "{:?}", o.stats);
        assert!(o.stats.research > 0 && o.stats.productions > 0 && o.stats.gate_confirms > 0, "{:?}", o.stats);
        popups += o.stats.events + o.stats.popups_left;
        research_left_open += o.stats.research_left_open;
        research_ai_picks += o.session.research_ai_picks;
    }
    assert!(popups > 0, "no deferred popup in two games");
    // A research popup left open on a quiet turn reaches the forced end turn open (the AI's
    // own research choice is cleared), where C++ makes the AI_chooseResearch pick.
    assert!(research_left_open > 0 && research_ai_picks >= research_left_open, "research left open {research_left_open}, AI picks {research_ai_picks}");
}

/// The repro of a failure: a game resumed from one of its autosaves ends exactly as the
/// uninterrupted game (the soak driver keeps no state the save does not have).
#[test]
fn soak_resumes_exactly_from_an_autosave() {
    let dir = TempDir::new("soak-resume");
    let mut g = game(3, "WORLDSIZE_TINY", 40);
    let full = play(&g, &RunOptions::default());
    g.turns = 25;
    play(&g, &RunOptions { saves: Some(dir.0.clone()), ..RunOptions::default() });
    let autosave = soak::latest_autosave(&dir.0).expect("autosaves");
    assert!(autosave.ends_with("auto/T0024.ffhsave"), "{}", autosave.display());
    g.turns = 40;
    let resumed = play(&g, &RunOptions { resume: Some(autosave), ..RunOptions::default() });
    assert_eq!((resumed.turn, resumed.checksum, &resumed.save_fnv), (full.turn, full.checksum, &full.save_fnv));
}

/// The human-only option sets: the one-city challenge, and the FfH challenges, whose High to Low
/// moves a leading human to the last civ at turn 50 (`reassignPlayerAdvanced`; with this seed
/// it does, and before P2 the session then waited for the old slot: `Stuck`).
#[test]
fn human_only_options() {
    let d = ffh_data::global();
    for (set, seed) in [("occ", 1), ("challenges", 2)] {
        let mut g = game(seed, "WORLDSIZE_TINY", 60);
        g.ng.civ = Some(d.t("CIVILIZATION_KURIOTATES"));
        g.ng.options = soak::parse_options(d, set).unwrap();
        let o = play(&g, &RunOptions::default());
        assert_eq!(o.end, End::Turns, "{set}");
        if set == "occ" {
            assert!(o.human_cities <= 1, "one city: {}", o.human_cities);
        } else if o.session.reassigned > 0 {
            assert_ne!(o.human, ffh_core::ids::PlayerId(0), "the session follows the human");
        }
    }
}

/// Infernals and Mercurians (not playable at the start) are founded by player 0 of an AI-only
/// game and taken over at the next turn boundary, with the human's handicap (FfH's
/// `reassignPlayerAdvanced` gives the slot the human founder's).
#[test]
fn spawned_civilizations_are_taken_over() {
    let d = ffh_data::global();
    for civ in soak::SPAWNED_CIVS {
        let mut g = game(4, "WORLDSIZE_TINY", 45);
        g.spawn = Some(d.t::<CivilizationType>(civ));
        g.spawn_turn = 15;
        g.ng.handicap = d.t("HANDICAP_PRINCE");
        let dir = TempDir::new(&format!("soak-spawn-{civ}"));
        let o = play(&g, &RunOptions { saves: Some(dir.0.clone()), ..RunOptions::default() });
        assert_eq!(o.end, End::Turns, "{civ}");
        assert_eq!(o.stats.taken_over_at, Some(15), "{civ}");
        assert!(o.stats.turns_played >= 25, "{civ}: {:?}", o.stats);
        let (save, sidecar) = ffh_play::save::read_bundle(&soak::latest_autosave(&dir.0).unwrap()).unwrap();
        let g2 = Game::load(d, &save).unwrap();
        let h = sidecar.unwrap().human;
        assert_eq!(h, o.human);
        assert_eq!(g2.player_get_civilization_type(h), Some(d.t(civ)), "{civ}");
        assert!(g2.setup.players[h.0 as usize].human && g2.num_human_players() == 1);
        assert_eq!(g2.player_get_handicap_type(h), Some(g.ng.handicap), "{civ}: the human's handicap");
        assert_eq!(g2.player_get_handicap_type(ffh_core::ids::PlayerId(0)), Some(d.t("HANDICAP_NOBLE")), "{civ}: the founder is an AI");
        if civ == "CIVILIZATION_MERCURIANS" {
            // The Mercurians join the builder's team.
            assert_eq!(g2.players[h].team, g2.players[ffh_core::ids::PlayerId(0)].team);
        }
    }
}

/// When the rules move the human inside `update()` (High to Low at turn 50: slot 0's turn
/// starts while it is human, then it is reassigned and plays that turn as an AI in the same
/// update), the rest of that update is not the human's: C++ moves the active player at once
/// (CvInitCore.cpp:474-482), so the old slot's messages from its AI turn reach nobody. The
/// session drains such an update for the new slot.
/// (Which slot leads at turn 50, and so whether High to Low fires, is emergent and drifts with
/// the AI and content, so the test scans seeds for a game that triggers the move rather than
/// pinning one -- see the rationale in the body.)
#[test]
fn a_slot_move_inside_an_update_drains_for_the_new_slot() {
    let d = ffh_data::global();
    // High to Low only reassigns the human when its slot leads the standings at turn 50
    // (`player_rank == 0`, CvGame::doTurn), which is emergent from how the game plays out: the AI
    // and any added content both shift the standings, and adding content also reshuffles the RNG
    // so a pinned seed reproduces a different game. So no fixed seed reliably reproduces the move
    // (seed 12 happens to today; seed 2 did before the round-3b AI merge). The driver callback is
    // read-only, so the move cannot be forced mid-run either. Instead, scan a bounded range of
    // seeds for any game whose standings put the human slot on top at turn 50 -- the seed only
    // shapes a believable board -- exercise the drain invariant on every game that moves the
    // human, and take the first game where High to Low moves the human off slot 0 at turn 50 and
    // leaves it there. If NONE in range does, fail loudly: that means High to Low has stopped
    // picking the human slot at all (a regression in `reassignPlayerAdvanced` or in the standings
    // that pick the leader), not that a golden seed drifted.
    for seed in 1..=200 {
        let mut g = game(seed, "WORLDSIZE_TINY", 52);
        g.ng.civ = Some(d.t("CIVILIZATION_BANNOR"));
        g.ng.options = soak::parse_options(d, "challenges").unwrap();
        let mut prev = None;
        let mut moved = None;
        let o = soak::run(&g, &RunOptions::default(), &mut |s| {
            let (h, turn) = (s.human, s.g.game_turn());
            if let Some(old) = prev.filter(|&p| p != h) {
                moved = Some((old, h, turn));
            }
            prev = Some(h);
            // The drain invariant (the real subject), checked on every game that moves the human:
            // once the human has been reassigned, none of the old slot's messages from its AI
            // turns after the move reach the (new) human slot. True for any seed and any move
            // turn, so it is not a seed-specific outcome.
            if let Some((old, _, at)) = moved {
                let leaked: Vec<String> = s.messages().iter().filter(|m| m.player == Some(old.0) && m.turn >= at).map(|m| format!("t{} {}", m.turn, m.text)).collect();
                assert!(leaked.is_empty(), "seed {seed}: the old slot's messages after the move reached the human: {leaked:?}");
            }
        })
        .unwrap();
        // The scenario this test is about: High to Low moved the human off slot 0 at turn 50 and
        // the human stayed there (a single clean move, so `o.human` is the new slot). Take the
        // first such game; the drain was already asserted above as it ran.
        if moved == Some((ffh_core::ids::PlayerId(0), o.human, 50)) {
            let (old, new, at) = moved.unwrap();
            eprintln!("seed {seed}: High to Low moved {old:?} -> {new:?} at turn {at}");
            assert_eq!((old, o.human, at), (ffh_core::ids::PlayerId(0), new, 50));
            return;
        }
    }
    panic!(
        "no seed in 1..=200 (challenges, Bannor, TINY, 52 turns) had High to Low move the human \
         from slot 0 at turn 50: the mechanism has stopped picking the human slot entirely"
    );
}
