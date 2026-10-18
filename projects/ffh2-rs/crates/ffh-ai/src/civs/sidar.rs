//! Sidar. Into the Mist (`SPELL_INTO_THE_MIST`, world spell; for the AI only with 40+ units) gives
//! every unit `PROMOTION_HIDDEN`: invisible to other teams (except in our own cities) and +1 first
//! strike, until the unit fights or casts. Units trained later do not get it. The end-of-turn
//! spell choice cast it the first turn the gate opened, usually at peace, and the army lost it
//! against barbarians before any war. The AI holds it for a war: it casts it when it is at war
//! with a civilization or preparing one, so the army marches unseen (the enemy cannot attack or
//! count it) and opens with an extra first strike; with no war by then it casts it once it has
//! half again the gate's 40 units, for the protection of workers and settlers.
//!
//! Measured (full games, seeds 1-40, Sidar in slot 0, against the generic play): win 5.0% ->
//! 7.5% (1 seed won only by the new side, 0 by the base), world share at the end 1333 -> 1407
//! (t 0.94), at turn 300 1531 -> 1529 (t -0.41: the spell mostly falls after turn 300).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::{world_spell, CivPlay};
use crate::view::View;
use crate::Ai;

pub struct Sidar;

const SPELL: &str = "SPELL_INTO_THE_MIST";

impl CivPlay for Sidar {
    fn spell_value(&self, g: &Game, _u: UnitId, spell: SpellType, value: i32) -> i32 {
        if world_spell::is(g, spell, SPELL) {
            return 0;
        }
        value
    }

    fn turn(&self, _ai: &Ai, g: &mut Game, v: &mut View) {
        let p = v.p;
        if g.players[p].feat_accomplished[FeatType::FEAT_GLOBAL_SPELL] {
            return;
        }
        if world_spell::war_or_preparing(g, p) || g.players[p].num_units() >= 60 {
            world_spell::cast(g, p, SPELL);
        }
    }
}
