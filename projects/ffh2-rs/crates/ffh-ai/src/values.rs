//! Static (game-state independent) valuations derived from game data, computed once.
//! These are AI heuristics, not rules.

use std::sync::OnceLock;

use ffh_data::*;

/// Per-type static values.
pub struct StaticValues {
    /// Rough military strength of a unit type (combat x mobility x extras), 0 for non-combat.
    pub unit_power: IdxVec<UnitType, i32>,
    /// Rough economic value of a building type, independent of the city.
    pub building_value: IdxVec<BuildingType, i32>,
    /// What a tech unlocks, by category, for research choice.
    pub tech_unlocks: IdxVec<TechType, TechUnlocks>,
    /// Techs that list this tech as an AND or OR prerequisite.
    pub tech_children: IdxVec<TechType, Vec<TechType>>,
}

#[derive(Clone, Debug, Default)]
pub struct TechUnlocks {
    pub units: Vec<UnitType>,
    pub buildings: Vec<BuildingType>,
    pub builds: Vec<BuildType>,
    pub civics: Vec<CivicType>,
    pub religion: Option<ReligionType>,
    /// Best unit power unlocked (for military research weighting).
    pub best_unit_power: i32,
}

fn unit_power(u: &UnitInfo) -> i32 {
    if u.combat <= 0 {
        return 0;
    }
    let mut p = u.combat * 100;
    p += p * (u.moves.max(1) - 1) / 4;
    p += p * (u.first_strikes * 2 + u.chance_first_strikes) / 20;
    p += p * u.withdrawal_probability / 200;
    p
}

fn building_value(b: &BuildingInfo) -> i32 {
    let mut v = 0;
    v += b.yield_change[0] * 30 + b.yield_change[1] * 25 + b.yield_change[2] * 12;
    v += b.yield_modifier[0] * 2 + b.yield_modifier[1] * 3 + b.yield_modifier[2] * 2;
    v += b.commerce_change.iter().sum::<i32>() * 12;
    v += b.commerce_modifier.iter().sum::<i32>() * 2;
    v += b.happiness * 40 + b.health * 25;
    v += b.area_happiness * 30 + b.global_happiness * 60;
    v += b.defense_modifier;
    v += b.free_experience * 10;
    v += b.great_people_rate_change * 15;
    v += b.trade_routes * 30;
    v += b.food_kept / 2;
    v += b.ai_weight;
    v
}

impl StaticValues {
    fn build(d: &GameData) -> StaticValues {
        let unit_power = IdxVec::from_fn(d.units.len(), |u: UnitType| unit_power(&d.units[u]));
        let building_value = IdxVec::from_fn(d.buildings.len(), |b: BuildingType| building_value(&d.buildings[b]));
        let mut tech_unlocks: IdxVec<TechType, TechUnlocks> = IdxVec::from_elem(TechUnlocks::default(), d.techs.len());
        for (u, info) in d.units.iter_enumerated() {
            if let Some(t) = info.prereq_and_tech {
                let tu = &mut tech_unlocks[t];
                tu.units.push(u);
                tu.best_unit_power = tu.best_unit_power.max(unit_power[u]);
            }
        }
        for (b, info) in d.buildings.iter_enumerated() {
            if let Some(t) = info.prereq_and_tech {
                tech_unlocks[t].buildings.push(b);
            }
        }
        for (r, info) in d.religions.iter_enumerated() {
            if let Some(t) = info.tech_prereq {
                tech_unlocks[t].religion = Some(r);
            }
        }
        for (c, info) in d.civics.iter_enumerated() {
            if let Some(t) = info.tech_prereq {
                tech_unlocks[t].civics.push(c);
            }
        }
        for (b, info) in d.builds.iter_enumerated() {
            if let Some(t) = info.tech_prereq {
                tech_unlocks[t].builds.push(b);
            }
        }
        let mut tech_children: IdxVec<TechType, Vec<TechType>> = IdxVec::from_elem(Vec::new(), d.techs.len());
        for (t, info) in d.techs.iter_enumerated() {
            for &pre in info.prereq_and_techs.iter().chain(info.prereq_or_techs.iter()) {
                if !tech_children[pre].contains(&t) {
                    tech_children[pre].push(t);
                }
            }
        }
        StaticValues { unit_power, building_value, tech_unlocks, tech_children }
    }
}

static VALUES: OnceLock<StaticValues> = OnceLock::new();

pub fn values(d: &GameData) -> &'static StaticValues {
    VALUES.get_or_init(|| StaticValues::build(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_sane() {
        let d = ffh_data::global();
        let v = values(d);
        let warrior = d.t::<UnitType>("UNIT_WARRIOR");
        let axe = d.t::<UnitType>("UNIT_AXEMAN");
        assert!(v.unit_power[axe] > v.unit_power[warrior]);
        let worker = d.t::<UnitType>("UNIT_WORKER");
        assert_eq!(v.unit_power[worker], 0);
        assert!(v.tech_unlocks.iter().any(|t| t.religion.is_some()));
    }
}
