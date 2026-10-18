//! Sheaim: both leaders are Summoner (Tebryn also Arcane), the Sundered trait gives their adepts
//! unholy resistance and their summons Stigmata, the palace gives Death, Chaos and Fire mana, and
//! the Training Yard is blocked (no Champions or other tier-3 melee). Casters are their designed
//! substitute, but adepts need a Mage Guild in the training city (`PrereqBuildingClass`), whose
//! generic value is about 0, so the generic AI never had one.
//!
//! What the AI does: once it knows Knowledge of the Ether, its most productive safe city builds a
//! Mage Guild (one per four cities); at war, or preparing one, guild cities train adepts up to
//! one per two cities. Adepts follow the army (`units::caster`), pick Death first (Raise Skeleton:
//! a permanent undead unit summoned next to the enemy every turn by the end-of-turn summon cast),
//! then Fire (Fireball).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::{best_unit, count_ai, Choice, Role};
use crate::Ai;

pub struct Sheaim;

impl CivPlay for Sheaim {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        if threat > 0 {
            return None;
        }
        mage_guild(g, c).or_else(|| adept(g, c))
    }

    fn promotion_value(&self, g: &Game, u: UnitId, promo: PromotionType, value: i32) -> i32 {
        if !matches!(g.units[u].ai_type, Some(UnitAIType::UNITAI_MAGE) | Some(UnitAIType::UNITAI_WARWIZARD)) {
            return value;
        }
        let name = g.d.promotions[promo].base.type_.as_str();
        match name {
            "PROMOTION_DEATH1" | "PROMOTION_DEATH2" => value + 300,
            "PROMOTION_CHANNELING2" => value + 200,
            "PROMOTION_FIRE1" | "PROMOTION_FIRE2" => value + 100,
            _ => value,
        }
    }
}

/// The Mage Guild in our most productive cities: one per four cities.
fn mage_guild(g: &Game, c: CityId) -> Option<Choice> {
    let d = g.d;
    let p = g.cities[c].owner;
    let civ = g.player_get_civilization_type(p)?;
    let bc = d.ty::<BuildingClassType>("BUILDINGCLASS_MAGE_GUILD")?;
    let b = d.civ_building(civ, bc)?;
    if g.city_is_has_building_class(c, bc) || !g.city_can_construct(c, b, false, false, false) {
        return None;
    }
    let cities = &g.players[p].cities;
    let have = cities.iter().filter(|&&o| g.city_is_has_building_class(o, bc) || g.city_production_building(o) == Some(b)).count() as i32;
    if have >= (cities.len() as i32 / 4).max(1) {
        return None;
    }
    // Only the most productive city that can build it.
    let prod = |o: CityId| g.city_yield_rate(o, YieldType::YIELD_PRODUCTION);
    let best = cities
        .iter()
        .copied()
        .filter(|&o| !g.city_is_has_building_class(o, bc) && g.city_can_construct(o, b, false, false, false))
        .max_by_key(|&o| (prod(o), std::cmp::Reverse(o)))?;
    if best != c || g.city_production_turns_left_building(c, b, 0) > 25 {
        return None;
    }
    crate::trace!(g, p, "{}: Mage Guild (sheaim)", g.cities[c].name);
    Some(Choice::Construct(b))
}

/// Adepts at war (or preparing one) from a guild city: up to one per two cities.
fn adept(g: &Game, c: CityId) -> Option<Choice> {
    let p = g.cities[c].owner;
    if !super::world_spell::war_or_preparing(g, p) {
        return None;
    }
    let mages = count_ai(g, p, UnitAIType::UNITAI_MAGE) + count_ai(g, p, UnitAIType::UNITAI_WARWIZARD);
    let target = (g.players[p].cities.len() as i32 / 2).clamp(1, 6);
    if mages >= target || !crate::economy::can_afford_unit(g, p, true, true) {
        return None;
    }
    let u = best_unit(g, c, Role::Mage)?;
    if g.city_production_turns_left_unit(c, u, 0) > 10 {
        return None;
    }
    crate::trace!(g, p, "{}: {} (sheaim caster {}/{})", g.cities[c].name, g.d.units[u].base.type_, mages, target);
    Some(Choice::Train(u, UnitAIType::UNITAI_MAGE))
}
