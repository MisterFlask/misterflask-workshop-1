//! FFH2 AI: a new design (see docs/AI.md), not a port of the C++ AI.
//!
//! [`Ai`] implements [`ffh_core::Controller`]. Each turn (`play_turn`) it builds a
//! [`view::View`] (enemies, danger map), manages the empire (research, civics, religion,
//! sliders), diplomacy, then gives every group that needs orders one decision through the
//! rules' mission API, and finally casts end-of-turn spells as FfH's AI does. City
//! production is chosen when the rules ask (`choose_production`).
//!
//! All decisions are deterministic functions of the game state: the AI draws no random
//! numbers and its caches are recomputed whenever the turn changes.

use std::sync::{Arc, Mutex};

use ffh_core::controller::{Controller, ControllerRef};
use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::structs::*;
use ffh_data::*;

pub mod casting;
pub mod diplomacy;
pub mod empire;
pub mod production;
pub mod reach;
pub mod research;
pub mod sites;
pub mod units;
pub mod values;
pub mod view;

/// The AI controller. One instance may serve any number of players.
#[derive(Default)]
pub struct Ai {
    pub(crate) sites: Mutex<sites::SiteCaches>,
}

impl Ai {
    pub fn new() -> Ai {
        Ai::default()
    }
}

pub fn ai_controller() -> ControllerRef {
    Arc::new(Ai::new())
}

impl Controller for Ai {
    fn name(&self) -> &str {
        "ffh-ai"
    }

    fn play_turn(&self, g: &mut Game, p: PlayerId) {
        if !g.players[p].alive {
            return;
        }
        let mut v = view::View::new(g, p);
        if !v.barbarian {
            empire::manage(g, p);
            diplomacy::manage(g, &v);
            // War declarations change the picture.
            v = view::View::new(g, p);
        }
        units::play(self, g, &mut v);
        casting::end_turn(g, p);
        // Spells may have summoned units or used moves: make sure nothing is left ready.
        units::finalize(g, p);
    }

    fn choose_production(&self, g: &mut Game, c: CityId) {
        production::choose(self, g, c);
    }

    fn choose_research(&self, g: &Game, p: PlayerId) -> Option<TechType> {
        research::choose(g, p)
    }

    fn raze_conquered_city(&self, g: &Game, p: PlayerId, c: CityId) -> bool {
        diplomacy::raze_conquered_city(g, p, c)
    }

    fn consider_offer(&self, g: &Game, p: PlayerId, from: PlayerId, they_give: &[TradeData], we_give: &[TradeData]) -> bool {
        diplomacy::consider_offer(g, p, from, they_give, we_give)
    }

    fn choose_religion(&self, g: &Game, p: PlayerId) -> Option<ReligionType> {
        g.players[p].personality_type.and_then(|l| g.d.leader_heads[l].favorite_religion)
    }
}
