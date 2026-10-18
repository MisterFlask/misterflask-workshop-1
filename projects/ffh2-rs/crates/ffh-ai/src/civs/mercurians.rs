//! Mercurians (join mid-game when a civilization completes the Mercurian Gate: they join its team
//! and Basium converts the gate city). They start with a Settler, but their one city stands in the
//! builder's culture, where `sites.rs` (sites within 7 plots of our cities) finds nothing, so the
//! settler waited all game paying unit support. When no site is on its landmass 20 turns after it
//! appeared, the AI disbands it.
//!
//! Measured over the 50 games (of 80 full games, Malakim and Svartalfar forced, seeds 1-40) where
//! the Mercurians appear on both sides: settlers left at the end 44 -> 5, world share 377 -> 420
//! (t 1.33), cities 2.48 -> 2.80, population 19.6 -> 21.4.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::view::View;
use crate::Ai;

pub struct Mercurians;

impl CivPlay for Mercurians {
    fn unit(&self, ai: &Ai, g: &mut Game, v: &mut View, _grp: GroupId, head: UnitId) -> bool {
        if g.units[head].ai_type != Some(UnitAIType::UNITAI_SETTLE) || g.game_turn() - g.units[head].game_turn_created < 20 {
            return false;
        }
        let area = g.map.plots[g.unit_plot(head)].area;
        if crate::sites::sites(ai, g, v.p).iter().any(|&(q, _)| g.map.plots[q].area == area) || !g.unit_can_scrap(head) {
            return false;
        }
        crate::trace!(g, v.p, "disbanding a settler with no site");
        g.unit_scrap(head);
        true
    }
}
