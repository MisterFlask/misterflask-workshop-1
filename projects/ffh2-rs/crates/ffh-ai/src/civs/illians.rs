//! Illians.
//!
//! - **Ritual projects** (`CIV4ProjectInfo.xml`, effects in `ffh/events.rs`): The White Hand
//!   (Philosophy, 400 hammers: three Priests of Winter, strength 5 + 2 cold, Medic, Channeling
//!   II, Ice) and Stir from Slumber (Divine Essence, 900 hammers: Drifa, a dragon; the rules
//!   allow it once a player has been eliminated). The generic AI reaches projects only when no
//!   building has any value left, so The White Hand came very late or never, and it never
//!   builds Stir from Slumber. Our most productive city builds them as soon as it can (not while
//!   threatened or broke).
//! - **Stasis** (world spell: every other player, barbarians included, gets 20 turns without
//!   production or research) is cast by the generic end-of-turn choice the turn its AI gate
//!   (5 cities) opens. Holding it for a war, a city in peril or a rival near victory was measured
//!   and lost share up to turn 400 (docs/civ-ai-b.md): the setback is worth most early, while
//!   rivals grow fastest.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::Choice;
use crate::Ai;

pub struct Illians;

impl CivPlay for Illians {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        if threat > 0 {
            return None;
        }
        ritual(g, c)
    }
}

/// The White Hand and Stir from Slumber in our most productive city as soon as it can build
/// them.
fn ritual(g: &Game, c: CityId) -> Option<Choice> {
    let p = g.cities[c].owner;
    let best = g.players[p].cities.iter().copied().max_by_key(|&o| (g.city_yield_rate(o, YieldType::YIELD_PRODUCTION), std::cmp::Reverse(o)));
    if best != Some(c) || crate::production::financial_trouble(g, p) {
        return None;
    }
    for name in ["PROJECT_THE_WHITE_HAND", "PROJECT_STIR_FROM_SLUMBER"] {
        let Some(pr) = g.d.ty::<ProjectType>(name) else { continue };
        if g.city_can_create(c, pr, false, false) {
            return Some(Choice::Create(pr));
        }
    }
    None
}
