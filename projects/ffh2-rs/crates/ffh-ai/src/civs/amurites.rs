//! Amurites: the arcane civilization (both leaders Arcane; the palace gives Fire, Body and
//! Metamagic mana; Wizards are Mages with a free Spellstaff; the research plan beelines Sorcery).
//!
//! - **Casters.** The generic caster step comes after buildings and its target is (cities + 2) / 4,
//!   so Amurite games had no adept or mage at turn 300, and Mages need a Mage Guild in the
//!   training city, which nothing built (`iAIWeight` 0, no yields). [`Amurites::production`]: the
//!   empire's most productive city, when unthreatened and a unit is affordable, trains casters up
//!   to (cities + 2) / 2, first building there the Mage Guild that Adepts and Mages need
//!   (Knowledge of the Ether; it only has to exist in that one city).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::{best_unit, count_ai, Choice, Role};
use crate::Ai;

pub struct Amurites;

impl CivPlay for Amurites {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        casters(g, c, threat)
    }
}

/// Casters from the empire's most productive city (see the module doc).
fn casters(g: &Game, c: CityId, threat: i32) -> Option<Choice> {
    let p = g.cities[c].owner;
    let pl = &g.players[p];
    if threat > 0 {
        return None;
    }
    let prod = |o: CityId| g.city_yield_rate(o, YieldType::YIELD_PRODUCTION);
    let best = pl.cities.iter().copied().max_by_key(|&o| (prod(o), std::cmp::Reverse(o)))?;
    if best != c {
        return None;
    }
    let n = pl.cities.len() as i32;
    let casters = count_ai(g, p, UnitAIType::UNITAI_MAGE) + count_ai(g, p, UnitAIType::UNITAI_WARWIZARD);
    if casters >= (n + 2) / 2 {
        return None;
    }
    let at_war = crate::diplomacy::preparing_target(g, pl.team).is_some()
        || g.teams.keys().any(|t| t != pl.team && t != g.barbarian_team() && g.teams[t].alive_count > 0 && g.at_war(pl.team, t));
    if !crate::economy::can_afford_unit(g, p, true, at_war) {
        return None;
    }
    let d = g.d;
    if let Some(guild) = d.ty::<BuildingType>("BUILDING_MAGE_GUILD") {
        if g.cities[c].num_real_building[guild] == 0 && g.city_can_construct(c, guild, false, false, false) {
            crate::trace!(g, p, "{}: mage guild for casters", g.cities[c].name);
            return Some(Choice::Construct(guild));
        }
    }
    let u = best_unit(g, c, Role::Mage)?;
    crate::trace!(g, p, "{}: caster {} ({} of {})", g.cities[c].name, d.units[u].base.type_, casters, (n + 2) / 2);
    Some(Choice::Train(u, UnitAIType::UNITAI_MAGE))
}
