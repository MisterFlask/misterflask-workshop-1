//! Illians.
//!
//! **Stasis** (world spell, `spellStasis`): every other player, barbarians included, gets 20
//! turns (normal speed) without production or research: no hammers, no commerce of any kind,
//! every city in disorder. The AI-only gate of the Python lets the AI cast it with 5 cities or
//! more. Held for the fight: cast when our war operation against a civ marches (its target can
//! train no defender while we close in), when one of our cities is in peril in a civ war, when a
//! rival is that close to a victory, or once we have had 5 cities for twice the effect's
//! duration without any of these.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::{held, CivPlay};
use crate::production::Choice;
use crate::view::View;
use crate::Ai;

pub struct Illians;

impl CivPlay for Illians {
    fn turn(&self, _ai: &Ai, g: &mut Game, v: &mut View) {
        stasis(g, v);
    }

    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        if variant() & 2 == 0 || threat > 0 {
            return None;
        }
        ritual(g, c)
    }

    fn spell_value(&self, g: &Game, _u: UnitId, spell: SpellType, value: i32) -> i32 {
        if Some(spell) == held::spell(g, "SPELL_STASIS") && variant() & 1 != 0 {
            return 0;
        }
        value
    }
}

fn variant() -> i32 {
    static V: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("FFH_DEV_ILL").ok().and_then(|s| s.parse().ok()).unwrap_or(3))
}

fn stasis(g: &mut Game, v: &mut View) {
    let p = v.p;
    if variant() & 1 == 0 {
        return;
    }
    let Some(spell) = held::spell(g, "SPELL_STASIS") else { return };
    if held::caster(g, p, spell).is_none() {
        return;
    }
    let duration = held::speed_turns(g, 20);
    let war_in_view = !v.civ_enemies.is_empty() || crate::diplomacy::preparing_target(g, v.team).is_some();
    let why = if variant() & 4 != 0 && !war_in_view {
        "no war in view"
    } else if held::war_op_moving(g, v) {
        "war operation under way"
    } else if held::civ_peril(v) {
        "a city in peril"
    } else if crate::victory::plan(g, p).rival.is_some_and(|(_, r)| r.eta <= 2 * duration) {
        "a rival near victory"
    } else if held::turn_with_cities(g, p, 5).is_some_and(|t| g.game_turn() - t >= 2 * duration) {
        "fallback"
    } else {
        return;
    };
    held::cast(g, p, spell, why);
}

/// The White Hand (three Priests of Winter for 400 hammers) and Stir from Slumber (Drifa, a
/// dragon) in our most productive city as soon as it can build them.
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
