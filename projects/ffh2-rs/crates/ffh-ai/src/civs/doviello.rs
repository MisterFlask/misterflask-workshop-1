//! Doviello.
//!
//! - **Wild Hunt** (world spell, `spellWildHunt`): a wolf (strength 2, more for strong units)
//!   appears on the plot of every unit of ours with a base strength. The AI-only gate of the
//!   Python allows it at war with a civ with 20 units or more; cast at once it fell on the
//!   declaration turn, the wolves appeared in our cities and, as `UNITAI_ANIMAL` units, wandered
//!   off like barbarian animals. Held for the fight: cast when our war operation steps next to
//!   its target or strikes (the wolves appear in the stacks there), when one of our cities is in
//!   peril in a civ war, or after 20 turns of war. The new wolves become army units
//!   (`UNITAI_ATTACK`) and join the army stack on their plot: they are expendable attackers in
//!   the strike and the first units disbanded at peace.
//! - **Upgrade spells** (Worker or Slave to Beastman, Beastman to Sons of Asena, Sons of Asena to
//!   Battlemaster; the only Doviello upgrade path): a worker (75 hammers) is not turned into a
//!   Beastman (25 hammers) unless it is the only way to save it (enemies next to it), and no
//!   upgrade spell takes the treasury below the reserve the generic upgrades keep.
//! - **War Machine** (the Doviello hero siege unit, strength 19, Machinery) and **Bison Rider**
//!   (strength 12, Warhorses, no horse needed) need a Siege Workshop and a Stable, buildings
//!   the generic AI never builds for units it cannot see (`PrereqBuilding`): the most productive
//!   city builds a Siege Workshop and trains the War Machine once Machinery is known, and the
//!   better half of the cities build a Stable once Warhorses is known.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::{held, CivPlay};
use crate::production::Choice;
use crate::view::View;
use crate::Ai;

pub struct Doviello;

impl CivPlay for Doviello {
    fn turn(&self, _ai: &Ai, g: &mut Game, v: &mut View) {
        wild_hunt(g, v);
    }

    fn spell_value(&self, g: &Game, u: UnitId, spell: SpellType, value: i32) -> i32 {
        let name = g.d.spells[spell].base.type_.as_str();
        if name == "SPELL_WILD_HUNT" && dev(1) {
            return 0;
        }
        if name.starts_with("SPELL_UPGRADE_DOVIELLO") {
            if !upgrade_ok(g, u, spell) {
                return 0;
            }
        }
        value
    }

    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        if threat > 0 {
            return None;
        }
        if !dev(8) {
            return None;
        }
        war_machine(g, c).or_else(|| stable(g, c))
    }
}

fn dev(bit: i32) -> bool {
    static V: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("FFH_DEV_DOV").ok().and_then(|s| s.parse().ok()).unwrap_or(15)) & bit != 0
}

fn wild_hunt(g: &mut Game, v: &mut View) {
    let p = v.p;
    if !dev(1) {
        return;
    }
    let Some(spell) = held::spell(g, "SPELL_WILD_HUNT") else { return };
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
    let max_before = g.players[p].units.iter().copied().max();
    if !held::cast(g, p, spell, why) {
        return;
    }
    // The wolves become army units, in the army stack on their plot if there is one.
    let Some(wolf) = g.d.ty::<UnitType>("UNIT_WOLF") else { return };
    let new: Vec<UnitId> = g.players[p].units.iter().copied().filter(|&n| max_before.map_or(true, |m| n > m) && g.units[n].unit_type == wolf).collect();
    for n in new {
        g.unit_ai_set_unit_ai_type(n, UnitAIType::UNITAI_ATTACK);
        let here = g.unit_plot(n);
        let stack = g.map.plots[here].units.iter().copied().find(|&o| o != n && g.units[o].owner == p && g.units[o].unit_type != wolf && crate::military::is_army_unit(g, o)).and_then(|o| g.units[o].group);
        if let Some(grp) = stack {
            g.unit_join_group(n, Some(grp), false);
        }
    }
}

/// Upgrade spells: never below the generic upgrade reserve (`units::upkeep`: 20 + 3 per city);
/// a worker only when enemies stand next to it (it would be captured).
fn upgrade_ok(g: &Game, u: UnitId, spell: SpellType) -> bool {
    let p = g.units[u].owner;
    let reserve = 20 + 3 * g.players[p].cities.len() as i32;
    if dev(4) && g.players[p].gold - g.d.spells[spell].cost < reserve {
        return false;
    }
    let worker = g.d.units[g.units[u].unit_type].unit_class_type == g.d.ty::<UnitClassType>("UNITCLASS_WORKER");
    if worker && dev(2) {
        let (x, y) = (g.units[u].x, g.units[u].y);
        let threatened = (-1..=1).any(|i| (-1..=1).any(|j| g.plot_xy(x, y, i, j).is_some_and(|q| g.plot_is_visible_enemy_unit_for_unit(q, u))));
        return threatened;
    }
    true
}

/// The most productive of our cities (ties: the lower id).
fn best_city(g: &Game, p: PlayerId) -> Option<CityId> {
    g.players[p].cities.iter().copied().max_by_key(|&c| (g.city_yield_rate(c, YieldType::YIELD_PRODUCTION), std::cmp::Reverse(c)))
}

/// The War Machine: trained where it can be; else a Siege Workshop in the most productive city
/// once Machinery is known and no War Machine exists.
fn war_machine(g: &Game, c: CityId) -> Option<Choice> {
    let d = g.d;
    let p = g.cities[c].owner;
    let unit = d.ty::<UnitType>("UNIT_WAR_MACHINE")?;
    let class = d.units[unit].unit_class_type?;
    if g.player_get_unit_class_count_plus_making(p, class) > 0 || g.player_is_unit_class_maxed_out(p, class, 0) {
        return None;
    }
    if g.city_can_train(c, unit, false, false, false, false) {
        return Some(Choice::Train(unit, UnitAIType::UNITAI_ATTACK));
    }
    let team = g.players[p].team;
    if !g.team_is_has_tech(team, d.units[unit].prereq_and_tech) || best_city(g, p) != Some(c) {
        return None;
    }
    let ws = d.units[unit].prereq_building?;
    let ws_class = d.buildings[ws].building_class_type?;
    if g.player_get_building_class_count_plus_making(p, ws_class) > 0 || !g.city_can_construct(c, ws, false, false, false) {
        return None;
    }
    Some(Choice::Construct(ws))
}

/// A Stable for Bison Riders in the better half of our cities (by production) once Warhorses
/// is known.
fn stable(g: &Game, c: CityId) -> Option<Choice> {
    let d = g.d;
    let p = g.cities[c].owner;
    let unit = d.ty::<UnitType>("UNIT_BISON_RIDER")?;
    let team = g.players[p].team;
    if !g.team_is_has_tech(team, d.units[unit].prereq_and_tech) {
        return None;
    }
    let st = d.units[unit].prereq_building?;
    if g.city_num_building(c, st) > 0 || !g.city_can_construct(c, st, false, false, false) {
        return None;
    }
    let prod = |o: CityId| g.city_yield_rate(o, YieldType::YIELD_PRODUCTION);
    let better = g.players[p].cities.iter().filter(|&&o| (prod(o), std::cmp::Reverse(o)) > (prod(c), std::cmp::Reverse(c))).count();
    let n = g.players[p].cities.len();
    if better * 2 >= n {
        return None;
    }
    Some(Choice::Construct(st))
}
