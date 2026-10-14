//! T5: AI-as-human soak smoke (design §8; the short CI variant of P2's soak). The human slot is
//! played by an `ffh_ai::Ai` through the session: production and unit moves by the AI on the
//! human's behalf (`choose_production`, `play_turn`), research, events and votes answered
//! through the commands with the AI's choice, blocking decisions by the advisor. The human-only
//! rules branches run every turn; the invariants are checked every 5 turns.

mod common;

use std::sync::Arc;

use common::{check, new_game};
use ffh_ai::Ai;
use ffh_core::controller::Controller;
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::PlayerVote;
use ffh_play::command::{CmdOk, Command};
use ffh_play::human::AdvisorAnswerer;
use ffh_play::{Session, SessionOptions, Stop};

fn fingerprint(g: &Game, h: PlayerId) -> Vec<(UnitId, i32, i32, i32)> {
    g.players[h].units.iter().map(|&u| (u, g.units[u].x, g.units[u].y, g.units[u].moves)).collect()
}

fn exec(s: &mut Session, c: Command) {
    match s.execute(&c) {
        Ok(CmdOk::Applied { .. }) => {}
        other => panic!("turn {}: {c:?} -> {other:?}", s.g.game_turn()),
    }
}

fn soak(seed: u32, turns: i32) {
    let ng = new_game(seed);
    let mut s = Session::new_game(&ng, Arc::new(AdvisorAnswerer), SessionOptions { journal: false, ..Default::default() }).unwrap();
    let ai = Ai::new();
    let h = s.human;
    let until = s.g.game_turn() + turns;
    let mut played = 0;
    'game: loop {
        let stop = s.advance(&mut |_| {});
        assert!(!matches!(stop, Stop::Stuck(_)), "seed {seed}: {stop:?}");
        if stop != Stop::Human || s.g.game_turn() >= until {
            break;
        }
        let turn = s.g.game_turn();
        if turn % 5 == 0 {
            s.read(check::check);
        }
        for c in s.pending().idle_cities {
            ai.choose_production(&mut s.g, c);
        }
        let p = s.pending();
        if p.research_needed {
            if let Some(t) = s.read(|g| ai.choose_research(g, h)) {
                exec(&mut s, Command::Research { tech: Some(t), shift: false });
            }
        }
        for e in &p.events {
            let options: Vec<_> = e.options.iter().filter(|o| o.enabled).map(|o| o.event).collect();
            let pick = s.read(|g| {
                let data = g.player_get_event_triggered(h, e.triggered).cloned().unwrap();
                ai.choose_event(g, h, &data, &options)
            });
            exec(&mut s, Command::Event { triggered: e.triggered, event: pick.unwrap_or(options[0]) });
        }
        for v in &p.votes {
            let choice = s.read(|g| {
                let data = g.vote_triggered(v.vote).cloned().unwrap();
                ai.diplo_vote(g, h, &data.vote_option, data.vote_source)
            });
            let choice = if v.choices.contains(&choice) { choice } else { *v.choices.iter().find(|&&c| c == PlayerVote::PLAYER_VOTE_ABSTAIN).unwrap_or(&v.choices[0]) };
            exec(&mut s, Command::Vote { vote: v.vote, choice });
        }
        // The AI moves the human's units while it moves something.
        for _ in 0..20 {
            let before = fingerprint(&s.g, h);
            ai.play_turn(&mut s.g, h);
            if s.advance(&mut |_| {}) != Stop::Human {
                break 'game;
            }
            if fingerprint(&s.g, h) == before {
                break;
            }
        }
        exec(&mut s, Command::EndTurn { force: true });
        played += 1;
    }
    s.read(check::check);
    let pl = &s.g.players[h];
    let others: Vec<usize> = s.g.civ_player_ids().filter(|&p| p != h && s.g.players[p].alive).map(|p| s.g.players[p].cities.len()).collect();
    eprintln!(
        "seed {seed}: turn {}, {played} turns played, cities {} (AI players: {others:?}), units {}, {:?}",
        s.g.game_turn(),
        pl.cities.len(),
        pl.units.len(),
        s.stats()
    );
    assert!(played >= turns / 2, "seed {seed}: only {played} turns");
}

#[test]
fn t5_ai_as_human_soak_smoke() {
    for seed in [1, 2] {
        soak(seed, 150);
    }
}
