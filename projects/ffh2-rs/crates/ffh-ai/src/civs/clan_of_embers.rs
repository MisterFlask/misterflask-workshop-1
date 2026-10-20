//! Clan of Embers.
//!
//! - **Warrens** (`BUILDING_WARRENS`, Masonry, 180 hammers): every unit finished in the city that
//!   is not a world, national, siege or naval unit gets a free copy (`ev_unit_built`; settlers and
//!   workers too). Its effect lives in Python, so the generic estimate is 0 and it was never
//!   built. [`ClanOfEmbers::building_value`] values it as the hammers it adds: the copies are worth
//!   the share of the city's production that goes into units.
//! - **Rantine** (hero) can cast Convert City in a barbarian city whose barbarian units are no
//!   stronger than him (base strength): the city becomes ours, without war. The Clan is at peace
//!   with the barbarians (`TRAIT_BARBARIAN`), so its armies never go there; [`ClanOfEmbers::unit`]
//!   walks Rantine to the nearest such city while no civ war needs him.
//! - **For the Horde** (world spell, barbarian player with 60+ units): half the barbarian orcs
//!   become ours where they stand. Cast at war or while preparing one, when the units have a war
//!   to fight ([`ClanOfEmbers::spell_value`]).
//! - **Shamans** (the Clan's Adept, no Mage Guild needed): the Clan keeps casters,
//!   [`super::unique_casters`], the owner's rule that civilizations use their unique units.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::Choice;
use crate::units::{alive, hold, move_to};
use crate::view::View;
use crate::Ai;

pub struct ClanOfEmbers;

impl CivPlay for ClanOfEmbers {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        super::unique_casters(g, c, threat)
    }

    fn spell_value(&self, g: &Game, u: UnitId, spell: SpellType, value: i32) -> i32 {
        if g.d.spells[spell].base.type_ != "SPELL_FOR_THE_HORDE" {
            return value;
        }
        // The converted orcs are worth most with a war to fight; at peace they cost unit support.
        let p = g.units[u].owner;
        let team = g.players[p].team;
        let barb = g.barbarian_team();
        let war = g.teams.keys().any(|t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t));
        if war || crate::diplomacy::preparing_target(g, team).is_some() {
            value
        } else {
            0
        }
    }

    fn building_value(&self, g: &Game, c: CityId, b: BuildingType, value: i32) -> i32 {
        if g.d.buildings[b].base.type_ != "BUILDING_WARRENS" {
            return value;
        }
        // One hammer a turn is worth 150 in `building_value` (yield weight 15 x 10). The copies
        // double what the city puts into units, about half of its production.
        let prod = g.city_yield_rate(c, YieldType::YIELD_PRODUCTION).max(0);
        value + prod * 150 / 2
    }

    fn unit(&self, _ai: &Ai, g: &mut Game, v: &mut View, grp: GroupId, head: UnitId) -> bool {
        if g.d.units[g.units[head].unit_type].base.type_ != "UNIT_RANTINE" {
            return false;
        }
        rantine(g, v, grp, head)
    }
}

/// Rantine: to the nearest barbarian city he can convert, and cast there. Only while no civ is at
/// war with us (his place is then with the army) and he is healthy.
fn rantine(g: &mut Game, v: &mut View, grp: GroupId, u: UnitId) -> bool {
    let p = v.p;
    let Some(spell) = g.d.ty::<SpellType>("SPELL_CONVERT_CITY_RANTINE") else { return false };
    if g.unit_can_cast(u, spell, false) {
        crate::trace!(g, p, "rantine: convert city at {:?}", g.map.xy(g.unit_plot(u)));
        g.unit_cast(u, spell);
        if alive(g, grp) && g.group_ready_to_move(grp, true) {
            hold(g, grp);
        }
        return true;
    }
    let barb = g.barbarian_player();
    if !v.civ_enemies.is_empty() || g.at_war(v.team, g.players[barb].team) || g.units[u].damage > 20 {
        return false;
    }
    let strength = g.unit_base_combat_str(u);
    let reach = v.flood.run(g, u, 15, false);
    let target = reach.iter().find(|r| {
        let q = r.plot;
        let pl = &g.map.plots[q];
        g.plot_is_city(q, false, None)
            && pl.owner == Some(barb)
            && pl.revealed[v.team]
            && pl.units.iter().all(|&o| g.units[o].owner != barb || !g.plot_is_visible(q, v.team) || g.unit_base_combat_str(o) <= strength)
    });
    let Some(r) = target.copied() else { return false };
    crate::trace!(g, p, "rantine at {:?}: to barbarian city {:?} ({} turns)", g.map.xy(g.unit_plot(u)), g.map.xy(r.plot), r.turns);
    move_to(g, grp, r.plot, 0);
    if alive(g, grp) && g.units.contains(u) && g.unit_plot(u) == r.plot && g.unit_can_cast(u, spell, false) {
        crate::trace!(g, p, "rantine: convert city at {:?}", g.map.xy(r.plot));
        g.unit_cast(u, spell);
    }
    if alive(g, grp) && g.group_ready_to_move(grp, true) {
        hold(g, grp);
    }
    true
}
