//! Khazad. Their siege engines (Trebuchet: Catapult class, 6 strength, 100% collateral, 80%
//! withdrawal; Dwarven Cannon) need a Siege Workshop and their knight (Hornguard, 12 strength, no
//! horses) a Stable -- specific buildings (`PrereqBuilding`), which the generic production never
//! builds, so the Khazad never had either. [`Khazad::production`] builds the Siege Workshop in our
//! most productive city when our operation wants siege engines and no city can train one (as
//! `campaign::siege_building` does for class-gated engines), and a Stable where it opens the
//! Hornguard (as `production::enabler_building` does for class-gated units).
//!
//! Saving gold for their Dwarven Vaults (`cf_do_turn_khazad`) was tried and measured: more
//! population, fewer techs, the same score by turn 300 (see docs/AI.md).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::{best_unit, Choice, Role};
use crate::Ai;

pub struct Khazad;

impl CivPlay for Khazad {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        siege_workshop(g, c, threat).or_else(|| super::unit_building(g, c, threat, &["UNIT_HORNGUARD"])).map(Choice::Construct)
    }
}

/// The Siege Workshop when our operation wants siege engines, no city of ours can train one and
/// none is building the workshop, in our most productive city.
fn siege_workshop(g: &Game, c: CityId, threat: i32) -> Option<BuildingType> {
    let p = g.cities[c].owner;
    let pl = &g.players[p];
    if threat > 0 || g.cities[c].settlement || !crate::economy::can_afford_unit(g, p, true, false) {
        return None;
    }
    let f = crate::campaign::needed_force(g, p)?;
    if f.siege <= 0 || pl.cities.iter().any(|&o| best_unit(g, o, Role::Siege).is_some()) {
        return None;
    }
    let prod = |o: CityId| g.city_yield_rate(o, YieldType::YIELD_PRODUCTION);
    if pl.cities.iter().any(|&o| o != c && !g.city_is_occupation(o) && (prod(o), std::cmp::Reverse(o)) > (prod(c), std::cmp::Reverse(c))) {
        return None;
    }
    let civ = g.player_get_civilization_type(p)?;
    let u = g.d.ty::<UnitType>("UNIT_TREBUCHET")?;
    let b = g.d.units[u].prereq_building?;
    let class = g.d.units[u].unit_class_type?;
    if g.d.civ_unit(civ, class) != Some(u) || !g.player_can_train(p, u, false, false, false) || g.city_num_building(c, b) > 0 || !g.city_can_construct(c, b, false, false, false) {
        return None;
    }
    if pl.cities.iter().any(|&o| g.city_production_building(o) == Some(b)) {
        return None;
    }
    crate::trace!(g, p, "{}: the campaign wants {:?}: {}", g.cities[c].name, f, g.d.buildings[b].base.type_);
    Some(b)
}
