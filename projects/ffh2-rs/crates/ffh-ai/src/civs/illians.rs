//! Illians: their ritual projects raise their unique units (the owner's rule: civilizations use
//! their unique mechanics, CLAUDE.md "AI changes"). Wilboman (Hero promotion, an attacker's default
//! AI) is trained as a hero (`production::role_ok`); as an attacker he never won on cost.
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
//!   (5 cities) opens. Holding it for a war, a city in peril or a rival near victory lost share
//!   up to turn 400 (share@200 -48, t -1.4): the setback is worth most early, while rivals grow
//!   fastest.
//!
//! First tried in round 5 (commit 76c1ffd; docs/AI_LOG.md): development seeds 1-40 win 12.5% ->
//! 17.5% (2/0), share at the end +269 (t 1.90); fresh seeds 5001-5064 win 15.6% -> 14.1% (2/3),
//! share at the end -40 (t -0.56): The White Hand came at turns 325-372, when Philosophy came.
//! Back since 2026-10-20 under the owner's rule.

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
