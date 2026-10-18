//! Hippus.
//!
//! **Warcry** (world spell, `spellWarcry`): every unit with a unit combat gets Warcry (+1
//! strength, +1 defense, +1 move, Blitz; a 5% chance a turn to wear off, about 20 turns). The
//! AI-only gate of the Python allows it at war with a civ with 25 units or more; cast at once, it
//! fell on the declaration turn, while the stacks were still gathering (operations spend about
//! half their turns gathering), so much of the promotion wore off before any fight. Held for the
//! fight: cast when our war operation steps next to its target or strikes, when one of our cities
//! is in peril in a civ war, or after 20 turns of war without either.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::{held, CivPlay};
use crate::view::View;
use crate::Ai;

pub struct Hippus;

impl CivPlay for Hippus {
    fn turn(&self, _ai: &Ai, g: &mut Game, v: &mut View) {
        warcry(g, v);
    }

    fn spell_value(&self, g: &Game, _u: UnitId, spell: SpellType, value: i32) -> i32 {
        if Some(spell) == held::spell(g, "SPELL_WARCRY") {
            return 0;
        }
        value
    }
}

fn warcry(g: &mut Game, v: &mut View) {
    let p = v.p;
    let Some(spell) = held::spell(g, "SPELL_WARCRY") else { return };
    if held::caster(g, p, spell).is_none() {
        return;
    }
    let why = if held::war_op_striking(g, v) {
        "strike"
    } else if held::civ_peril(v) {
        "a city in peril"
    } else if held::civ_war_turns(g, v) >= held::speed_turns(g, 20) {
        "fallback"
    } else {
        return;
    };
    held::cast(g, p, spell, why);
}
