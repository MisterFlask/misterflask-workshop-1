//! Calabim: their vampires fight and feed (the owner's rule: civilizations use their unique
//! mechanics, CLAUDE.md "AI changes").
//!
//! - **Vampires** (`UNIT_VAMPIRE`, the Calabim Champion, and the Vampire Lord): the rules put
//!   every Vampire on `UNITAI_FEASTING` when it is created (`ev_unit_created`, also on a Moroi's
//!   upgrade), a unit AI type this AI has no logic for: such units were left out of stacks, siege
//!   forces and the attacker count. [`Calabim::turn`] puts them back on `UNITAI_ATTACK` (AI
//!   bookkeeping, as production does for garrisons).
//! - **Feast** (a unit with the Vampire promotion in our city of size 3+): the city loses a
//!   citizen and gets 3 turns of hurry anger, the unit gains population - 2 experience. FfH's
//!   estimate is negative (-50 for the population), so it was never cast. Cast in a city with an
//!   unhappy citizen to spare (above its happy cap), by units below 30 experience (FfH's own
//!   rule in `AI_feastingmove`).
//! - **Gift Vampirism** (Vampires): the Vampire promotion (+10% strength, +5% healing, Feast) for
//!   every level 6+ unit (level 4+ Moroi) on the plot; its effect is Python only (value 0), so it
//!   was never cast. Valued as a promotion for each such unit.
//!
//! First tried in round 5 (commit efb8d23, with River of Blood held for a war, which is not
//! back): development seeds 1-40 win 7.7% -> 7.7%, share at the end +109 (t 0.59); fresh seeds
//! 5001-5064 win 10.9% -> 4.7% (2/6 seeds won by one side only), share at the end -77 (t -0.61);
//! vampires were rare (0.1 a game; docs/AI_LOG.md). Back since 2026-10-20 under the owner's rule.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::view::View;
use crate::Ai;

pub struct Calabim;

impl CivPlay for Calabim {
    fn turn(&self, _ai: &Ai, g: &mut Game, v: &mut View) {
        let feasting = UnitAIType::UNITAI_FEASTING;
        for u in g.players[v.p].units.clone() {
            if g.units[u].ai_type == Some(feasting) {
                g.unit_ai_set_unit_ai_type(u, UnitAIType::UNITAI_ATTACK);
            }
        }
    }

    fn spell_value(&self, g: &Game, u: UnitId, spell: SpellType, value: i32) -> i32 {
        match g.d.spells[spell].base.type_.as_str() {
            "SPELL_FEAST" => feast_value(g, u),
            "SPELL_GIFT_VAMPIRISM" => gift_value(g, u),
            _ => value,
        }
    }
}

/// Feast in a city with a citizen above its happy cap (it does not work, so losing it costs
/// nothing but the hurry anger, which falls on a citizen that is unhappy anyway).
fn feast_value(g: &Game, u: UnitId) -> i32 {
    let Some(c) = g.map.plots[g.unit_plot(u)].plot_city else { return 0 };
    let xp = g.cities[c].population - 2;
    if g.units[u].experience >= 30 || xp < 3 || g.city_happy_level(c) - g.city_unhappy_level(c, 0) >= 0 {
        return 0;
    }
    xp * 50
}

/// The Vampire promotion for each level 6+ unit (level 4+ Moroi) on the caster's plot.
fn gift_value(g: &Game, u: UnitId) -> i32 {
    let d = g.d;
    let vampire = d.t::<PromotionType>("PROMOTION_VAMPIRE");
    let moroi = d.t::<UnitType>("UNIT_MOROI");
    let p = g.units[u].owner;
    let n = g.map.plots[g.unit_plot(u)]
        .units
        .iter()
        .filter(|&&o| {
            let unit = &g.units[o];
            unit.owner == p && g.unit_is_alive(o) && !unit.has_promotion[vampire] && (unit.level >= 6 || unit.level >= 4 && unit.unit_type == moroi)
        })
        .count() as i32;
    n * crate::units::promotion_value(g, u, vampire).max(50)
}
