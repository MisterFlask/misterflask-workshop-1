//! Spot checks for the unit-related info tables (port of CvUnitInfo, CvUnitClassInfo,
//! CvPromotionInfo, CvSpellInfo, CvBuildInfo, CvSpecialUnitInfo, CvAutomateInfo, CvCommandInfo,
//! CvMissionInfo). Expected values were read off data/xml/Units/*.xml.

use ffh_data::*;

fn data() -> &'static GameData {
    ffh_data::global()
}

fn unit(name: &str) -> &'static UnitInfo {
    let d = data();
    &d.units[d.t::<UnitType>(name)]
}

fn promo(name: &str) -> &'static PromotionInfo {
    let d = data();
    &d.promotions[d.t::<PromotionType>(name)]
}

fn spell(name: &str) -> &'static SpellInfo {
    let d = data();
    &d.spells[d.t::<SpellType>(name)]
}

/// Indices set to `true` in a bool array, as sorted type names.
fn set_names<I: InfoId>(v: &IdxVec<I, bool>) -> Vec<&'static str> {
    let d = data();
    let mut names: Vec<&str> = v.iter_enumerated().filter(|(_, b)| **b).map(|(i, _)| d.type_name(i)).collect();
    names.sort_unstable();
    names
}

#[test]
fn loads_without_warnings() {
    let d = data();
    for w in &d.warnings {
        eprintln!("warning: {w}");
    }
    assert!(d.warnings.is_empty(), "{} data warnings (printed above)", d.warnings.len());
}

#[test]
fn table_sizes() {
    let d = data();
    assert_eq!(d.units.len(), 325);
    assert_eq!(d.unit_classes.len(), 247);
    assert_eq!(d.promotions.len(), 261);
    assert_eq!(d.spells.len(), 267);
    assert_eq!(d.builds.len(), 35);
    assert_eq!(d.special_units.len(), 3);
    assert_eq!(d.missions.len(), MissionType::NUM);
    assert_eq!(d.commands.len(), CommandType::NUM);
    assert_eq!(d.automates.len(), AutomateType::NUM);
}

#[test]
fn dense_arrays_are_sized_by_their_key_table() {
    let d = data();
    for u in d.units.iter() {
        assert_eq!(u.free_promotions.len(), d.promotions.len());
        assert_eq!(u.unit_ai_type.len(), UnitAIType::NUM);
        assert_eq!(u.domain_modifier.len(), DomainType::NUM);
        assert_eq!(u.upgrade_unit_class.len(), d.unit_classes.len());
        assert_eq!(u.builds.len(), d.builds.len());
        assert_eq!(u.terrain_passable_tech.len(), d.terrains.len());
        assert_eq!(u.feature_impassable.len(), d.features.len());
        assert_eq!(u.flavor_value.len(), d.reg.count::<FlavorType>());
        assert_eq!(u.damage_type_combat.len(), DamageType::NUM);
        // C++ fixed arrays of NUM_UNIT_AND_TECH_PREREQS / NUM_UNIT_PREREQ_OR_BONUSES slots.
        assert!(u.prereq_and_techs.len() <= d.defines.num_unit_and_tech_prereqs as usize);
        assert!(u.prereq_or_bonuses.len() <= d.defines.num_unit_prereq_or_bonuses as usize);
    }
    for p in d.promotions.iter() {
        assert_eq!(p.unit_combat.len(), d.unit_combats.len());
        assert_eq!(p.damage_type_resist.len(), DamageType::NUM);
        assert_eq!(p.bonus_affinity.len(), d.bonuses.len());
    }
    for b in d.builds.iter() {
        assert_eq!(b.feature_tech.len(), d.features.len());
        assert_eq!(b.feature_remove.len(), d.features.len());
    }
}

#[test]
fn every_unit_has_a_class_and_domain() {
    let d = data();
    for (i, u) in d.units.iter_enumerated() {
        assert!(u.unit_class_type.is_some(), "{} has no unit class", d.type_name(i));
        assert!(u.domain_type.is_some(), "{} has no domain", d.type_name(i));
    }
}

#[test]
fn warrior() {
    let d = data();
    let u = unit("UNIT_WARRIOR");
    assert_eq!(u.base.type_, "UNIT_WARRIOR");
    assert_eq!(u.base.description, "TXT_KEY_UNIT_WARRIOR");
    assert_eq!(u.unit_class_type, Some(d.t::<UnitClassType>("UNITCLASS_WARRIOR")));
    assert_eq!(u.unit_combat_type, Some(d.t::<UnitCombatType>("UNITCOMBAT_MELEE")));
    assert_eq!(u.domain_type, Some(DomainType::DOMAIN_LAND));
    assert_eq!(u.default_unit_ai_type, Some(UnitAIType::UNITAI_ATTACK));
    assert_eq!(u.combat, 3);
    assert_eq!(u.combat_defense, 3);
    assert_eq!(u.production_cost, 25);
    assert_eq!(u.moves, 1);
    assert_eq!(u.city_defense_modifier, 25);
    assert_eq!(u.combat_limit, 100);
    assert_eq!(u.min_area_size, -1);
    assert_eq!(u.nuke_range, -1);
    assert_eq!(u.tier, 1);
    assert_eq!(u.weapon_tier, 1);
    assert_eq!(u.prereq_and_tech, None);
    assert!(u.prereq_and_techs.is_empty());
    assert!(u.military_happiness && u.pillage && !u.found);
    assert_eq!(set_names(&u.upgrade_unit_class), ["UNITCLASS_ARCHER", "UNITCLASS_AXEMAN"]);
    assert_eq!(set_names(&u.unit_ai_type), ["UNITAI_ATTACK", "UNITAI_PILLAGE"]);
    assert!(u.see_invisible_types.is_empty());
    assert!(u.unit_names.is_empty());
}

#[test]
fn chariot_prereqs_and_modifiers() {
    let d = data();
    let u = unit("UNIT_CHARIOT");
    assert_eq!(u.unit_combat_type, Some(d.t::<UnitCombatType>("UNITCOMBAT_MOUNTED")));
    assert_eq!(u.prereq_and_tech, Some(d.t::<TechType>("TECH_TRADE")));
    assert_eq!(u.prereq_and_techs, vec![d.t::<TechType>("TECH_CONSTRUCTION")]);
    // <PrereqBonuses> lists BONUS_HORSE, BONUS_NIGHTMARE, NONE, NONE: the NONE slots are dropped.
    assert_eq!(
        u.prereq_or_bonuses,
        vec![d.t::<BonusType>("BONUS_HORSE"), d.t::<BonusType>("BONUS_NIGHTMARE")]
    );
    assert_eq!(u.prereq_and_bonus, None);
    assert_eq!(u.prereq_building, Some(d.t::<BuildingType>("BUILDING_SIEGE_WORKSHOP")));
    assert_eq!(u.production_cost, 120);
    assert_eq!(u.combat, 5);
    assert_eq!(u.moves, 3);
    assert_eq!(u.withdrawal_probability, 25);
    assert!(u.no_defensive_bonus);
    assert_eq!(u.feature_attack_modifier[d.t::<FeatureType>("FEATURE_FOREST")], -25);
    assert_eq!(u.feature_attack_modifier[d.t::<FeatureType>("FEATURE_JUNGLE")], -25);
    assert_eq!(u.feature_attack_modifier[d.t::<FeatureType>("FEATURE_FOREST_ANCIENT")], 0);
    assert_eq!(set_names(&u.upgrade_unit_class), ["UNITCLASS_KNIGHT"]);
}

#[test]
fn angel_of_death_ffh_fields() {
    let d = data();
    let u = unit("UNIT_ANGEL_OF_DEATH");
    assert_eq!(u.prereq_civ, Some(d.t::<CivilizationType>("CIVILIZATION_MERCURIANS")));
    assert_eq!(u.py_post_combat_won, "postCombatReducePopulation(pCaster,pOpponent)");
    assert_eq!(u.py_post_combat_lost, "");
    assert_eq!(u.see_invisible_types, vec![d.t::<InvisibleType>("INVISIBLE_ANIMAL")]);
    assert_eq!(
        set_names(&u.free_promotions),
        ["PROMOTION_ANGEL", "PROMOTION_INVISIBLE", "PROMOTION_MARKSMAN", "PROMOTION_VILE_TOUCH"]
    );
    assert_eq!(u.damage_type_combat[DamageType::DAMAGE_HOLY], 3);
    assert_eq!(u.damage_type_combat[DamageType::DAMAGE_FIRE], 0);
    assert_eq!(u.combat, 8);
    assert_eq!(u.combat_defense, 6);
    assert_eq!(u.min_level, 4);
    assert_eq!(u.first_strikes, 1);
    assert!(u.first_strike_immune && u.investigate && u.rival_territory);
}

#[test]
fn air_elemental_create_from_combat() {
    let d = data();
    let u = unit("UNIT_AIR_ELEMENTAL");
    assert_eq!(u.unit_create_from_combat, Some(d.t::<UnitType>("UNIT_LIGHTNING_ELEMENTAL")));
    assert_eq!(u.unit_create_from_combat_chance, 100);
    assert_eq!(u.unit_convert_from_combat, None);
    assert_eq!(u.bonus_affinity[d.t::<BonusType>("BONUS_MANA_AIR")], 1);
    assert_eq!(u.damage_type_combat[DamageType::DAMAGE_LIGHTNING], 2);
    assert_eq!(u.production_cost, -1);
    assert_eq!(u.culture_garrison_value, 6);
    assert_eq!(u.power_value, 20);
    assert!(u.free_promotions[d.t::<PromotionType>("PROMOTION_FLYING")]);
}

#[test]
fn galley_and_settler() {
    let d = data();
    let g = unit("UNIT_GALLEY");
    assert_eq!(g.domain_type, Some(DomainType::DOMAIN_SEA));
    assert_eq!(g.prereq_and_tech, Some(d.t::<TechType>("TECH_SAILING")));
    assert!(g.mech_unit);
    assert!(g.terrain_impassable[d.t::<TerrainType>("TERRAIN_OCEAN")]);
    assert!(!g.terrain_impassable[d.t::<TerrainType>("TERRAIN_COAST")]);
    assert_eq!(g.domain_cargo, Some(DomainType::DOMAIN_LAND));
    assert_eq!(g.special_cargo, None);
    assert_eq!(g.cargo_space, 0);
    assert_eq!(g.min_area_size, 20);

    let s = unit("UNIT_SETTLER");
    assert_eq!(s.production_traits[d.t::<TraitType>("TRAIT_EXPANSIVE")], 100);
    assert!(s.food_production && s.found && s.only_defensive);
    assert_eq!(s.unit_capture_class_type, Some(d.t::<UnitClassType>("UNITCLASS_WORKER")));
    assert_eq!(s.default_unit_ai_type, Some(UnitAIType::UNITAI_SETTLE));
    assert_eq!(s.advanced_start_cost_increase, 50);
}

#[test]
fn great_artist() {
    let d = data();
    let u = unit("UNIT_ARTIST");
    assert_eq!(u.unit_names.len(), 20);
    assert_eq!(u.unit_names[0], "TXT_KEY_UNIT_ARTIST_1");
    assert_eq!(u.unit_names[19], "TXT_KEY_UNIT_ARTIST_20");
    assert_eq!(u.special_unit_type, Some(d.t::<SpecialUnitType>("SPECIALUNIT_PEOPLE")));
    assert_eq!(set_names(&u.great_peoples), ["SPECIALIST_GREAT_ARTIST"]);
    assert_eq!(set_names(&u.buildings), ["BUILDING_SONG_OF_AUTUMN"]);
    assert_eq!(u.flavor_value[d.t::<FlavorType>("FLAVOR_CULTURE")], 3);
    assert_eq!(u.flavor_value[d.t::<FlavorType>("FLAVOR_GROWTH")], 2);
    assert_eq!(u.flavor_value[d.t::<FlavorType>("FLAVOR_SCIENCE")], 1);
    assert_eq!(u.flavor_value[d.t::<FlavorType>("FLAVOR_MILITARY")], 0);
    assert_eq!(u.base_discover, 1000);
    assert_eq!(u.discover_multiplier, 2);
    assert_eq!(u.great_work_culture, 4000);
    assert!(u.golden_age);
}

#[test]
fn misc_ffh_unit_references() {
    let d = data();
    assert_eq!(unit("UNIT_BALOR").upgrade_civ, Some(d.t::<CivilizationType>("CIVILIZATION_INFERNAL")));
    assert_eq!(unit("UNIT_CHALID").diplo_vote_type, Some(d.t::<VoteSourceType>("DIPLOVOTE_OVERCOUNCIL")));
    assert_eq!(unit("UNIT_BABY_SPIDER").unit_convert_from_combat, Some(d.t::<UnitType>("UNIT_GIANT_SPIDER")));
    assert_eq!(unit("UNIT_DRUID").prereq_alignment, Some(AlignmentType::ALIGNMENT_NEUTRAL));
    assert_eq!(unit("EQUIPMENT_ATHAME").equipment_promotion, Some(d.t::<PromotionType>("PROMOTION_ATHAME")));
    assert_eq!(unit("EQUIPMENT_ATHAME").unit_capture_class_type, Some(d.t::<UnitClassType>("EQUIPMENTCLASS_ATHAME")));
    assert!(unit("UNIT_ASSASSIN").ai_block_patrol);
    assert!(!unit("UNIT_WARRIOR").ai_block_patrol);
    assert_eq!(unit("UNIT_ACHERON").py_post_combat_lost, "postCombatAcheron(pCaster,pOpponent)");
}

#[test]
fn unit_classes() {
    let d = data();
    let herne = &d.unit_classes[d.t::<UnitClassType>("UNITCLASS_HERNE")];
    assert_eq!(herne.max_global_instances, 1);
    assert_eq!(herne.max_team_instances, -1);
    assert_eq!(herne.max_player_instances, -1);
    assert_eq!(herne.instance_cost_modifier, 0);
    assert_eq!(herne.default_unit_index, Some(d.t::<UnitType>("UNIT_HERNE")));
    let ira = &d.unit_classes[d.t::<UnitClassType>("UNITCLASS_IRA")];
    assert_eq!(ira.max_player_instances, 4);
    // Every default unit belongs to the class that names it, except one FFH2 0.41 data quirk:
    // UNITCLASS_POLAR_BEAR's DefaultUnit is UNIT_POLAR_BEAR, whose <Class> is UNITCLASS_BEAR.
    let mismatched: Vec<&str> = d
        .unit_classes
        .iter_enumerated()
        .filter(|(c, info)| matches!(info.default_unit_index, Some(u) if d.units[u].unit_class_type != Some(*c)))
        .map(|(c, _)| d.type_name(c))
        .collect();
    assert_eq!(mismatched, ["UNITCLASS_POLAR_BEAR"]);
    assert_eq!(unit("UNIT_POLAR_BEAR").unit_class_type, Some(d.t::<UnitClassType>("UNITCLASS_BEAR")));
}

#[test]
fn promotions() {
    let d = data();
    let c1 = promo("PROMOTION_COMBAT1");
    assert_eq!(c1.combat_percent, 20);
    assert_eq!(c1.combat_percent_defense, 20);
    assert_eq!(c1.spell_damage_modify, 5);
    assert!(c1.mutation);
    assert_eq!(c1.promotion_summon_perk, Some(d.t::<PromotionType>("PROMOTION_EMPOWER1")));
    assert_eq!(c1.promotion_next_level, Some(d.t::<PromotionType>("PROMOTION_COMBAT2")));
    assert_eq!(c1.prereq_promotion, None);
    assert!(c1.unit_combat[d.t::<UnitCombatType>("UNITCOMBAT_MELEE")]);
    assert!(c1.unit_combat[d.t::<UnitCombatType>("UNITCOMBAT_SIEGE")]);
    assert_eq!(set_names(&c1.unit_combat).len(), 10);

    let fear = promo("PROMOTION_FEAR");
    assert_eq!(fear.prereq_promotion, Some(d.t::<PromotionType>("PROMOTION_COMBAT5")));
    assert_eq!(fear.prereq_or_promotion1, Some(d.t::<PromotionType>("PROMOTION_DEMON")));
    assert_eq!(fear.prereq_or_promotion2, Some(d.t::<PromotionType>("PROMOTION_UNDEAD")));
    assert_eq!(fear.promotion_prereq_or3, Some(d.t::<PromotionType>("PROMOTION_NIGHTMARE")));
    assert_eq!(fear.promotion_prereq_or4, None);
    assert!(fear.fear);
    assert_eq!(fear.ai_weight, 100);

    let slay = promo("PROMOTION_DEMON_SLAYING");
    assert_eq!(slay.promotion_combat_type, Some(d.t::<PromotionType>("PROMOTION_DEMON")));
    assert_eq!(slay.promotion_combat_mod, 40);
    assert_eq!(slay.tech_prereq, Some(d.t::<TechType>("TECH_WAY_OF_THE_WISE")));

    let wood = promo("PROMOTION_WOODSMAN1");
    assert_eq!(wood.feature_attack_percent[d.t::<FeatureType>("FEATURE_FOREST")], 30);
    assert_eq!(wood.feature_defense_percent[d.t::<FeatureType>("FEATURE_JUNGLE")], 30);
    assert_eq!(wood.terrain_defense_percent[d.t::<TerrainType>("TERRAIN_GRASS")], 0);

    let medic = promo("PROMOTION_MEDIC1");
    assert_eq!(medic.same_tile_heal_change, 10);
    assert_eq!(medic.tech_prereq, Some(d.t::<TechType>("TECH_NEVER")));

    let hero = promo("PROMOTION_HERO");
    assert_eq!(hero.free_xp_per_turn, 1);
    assert_eq!(hero.min_level, -1);
    assert_eq!(hero.resist_magic, 20);

    assert_eq!(promo("PROMOTION_FIRE2").promotion_prereq_and, Some(d.t::<PromotionType>("PROMOTION_CHANNELING2")));
    assert_eq!(promo("PROMOTION_BOARDING").capture_unit_combat, Some(d.t::<UnitCombatType>("UNITCOMBAT_NAVAL")));

    let crown = promo("PROMOTION_CROWN_OF_BRILLANCE");
    assert_eq!(crown.py_per_turn, "effectCrownOfBrillance(pCaster)");
    assert_eq!(crown.expire_chance, 25);
    assert!(crown.dispellable && crown.see_invisible);

    // No FFH2 promotion sets bLeader, so no promotion is forced graphical-only by it.
    assert!(d.promotions.iter().all(|p| !p.leader));
}

#[test]
fn spells() {
    let d = data();
    let s = spell("SPELL_RAISE_SKELETON");
    assert_eq!(s.promotion_prereq1, Some(d.t::<PromotionType>("PROMOTION_DEATH1")));
    assert_eq!(s.promotion_prereq2, Some(d.t::<PromotionType>("PROMOTION_CHANNELING1")));
    assert_eq!(s.create_unit_type, Some(d.t::<UnitType>("UNIT_SKELETON")));
    assert_eq!(s.create_unit_num, 1);
    assert!(s.permanent_unit_create && s.allow_ai && s.has_casted);
    assert_eq!(s.py_result, "spellRaiseSkeleton(pCaster)");
    assert_eq!(s.py_requirement, "");

    let b = spell("SPELL_CALL_BLIZZARD");
    assert_eq!(b.unit_class_prereq, Some(d.t::<UnitClassType>("UNITCLASS_PRIEST_OF_WINTER")));
    assert_eq!(b.py_result, "spellCallBlizzard(pCaster)");
    assert_eq!(b.py_requirement, "reqCallBlizzard(pCaster)");
    assert_eq!(b.ai_weight, 50);

    let c = spell("SPELL_CONTAGION");
    assert_eq!(c.unit_class_prereq, Some(d.t::<UnitClassType>("UNITCLASS_MARY")));
    assert_eq!(c.range, 1);
    assert_eq!(c.damage, 10);
    assert_eq!(c.damage_limit, 40);
    assert_eq!(c.damage_type, Some(DamageType::DAMAGE_DEATH));
    assert_eq!(c.add_promotion_type1, Some(d.t::<PromotionType>("PROMOTION_PLAGUED")));
    assert!(c.causes_war && c.resistable && c.immune_team && c.immune_not_alive);
    assert!(!c.immune_enemy);

    let k = spell("SPELL_FOUND_TEMPLE_OF_KILMORPH");
    assert_eq!(k.create_building_type, Some(d.t::<BuildingType>("BUILDING_TEMPLE_OF_KILMORPH")));
    assert_eq!(k.spread_religion, Some(d.t::<ReligionType>("RELIGION_RUNES_OF_KILMORPH")));
    assert!(k.sacrifice_caster && k.in_city_only && k.ability);
    assert!(k.base.graphical_only);

    let f = spell("SPELL_ADD_TO_FREAK_SHOW_DWARF");
    assert_eq!(f.civilization_prereq, Some(d.t::<CivilizationType>("CIVILIZATION_BALSERAPHS")));
    assert_eq!(f.create_building_type, Some(d.t::<BuildingType>("BUILDING_DWARF_CAGE")));
    assert!(f.ignore_has_casted && f.in_borders_only);
    assert_eq!(f.ai_weight, 30);

    assert_eq!(spell("SPELL_ARENA_BATTLE").building_prereq, Some(d.t::<BuildingType>("BUILDING_ARENA")));
    let h = spell("SPELL_HASTE");
    assert_eq!(h.add_promotion_type1, Some(d.t::<PromotionType>("PROMOTION_HASTED")));
    assert_eq!(h.remove_promotion_type1, Some(d.t::<PromotionType>("PROMOTION_SLOW")));
    assert_eq!(spell("SPELL_ADD_TO_CITY").change_population, 1);
}

#[test]
fn builds_feature_structs() {
    let d = data();
    let farm = &d.builds[d.t::<BuildType>("BUILD_FARM")];
    assert_eq!(farm.tech_prereq, Some(d.t::<TechType>("TECH_AGRICULTURE")));
    assert_eq!(farm.time, 500);
    assert_eq!(farm.cost, 0);
    assert!(!farm.kill);
    assert_eq!(farm.improvement, Some(d.t::<ImprovementType>("IMPROVEMENT_FARM")));
    assert_eq!(farm.route, None);
    let jungle = d.t::<FeatureType>("FEATURE_JUNGLE");
    let forest = d.t::<FeatureType>("FEATURE_FOREST");
    let ancient = d.t::<FeatureType>("FEATURE_FOREST_ANCIENT");
    assert_eq!(farm.feature_tech[jungle], Some(d.t::<TechType>("TECH_BRONZE_WORKING")));
    assert_eq!(farm.feature_time[jungle], 400);
    assert_eq!(farm.feature_production[jungle], 0);
    assert!(farm.feature_remove[jungle]);
    assert_eq!(farm.feature_tech[forest], Some(d.t::<TechType>("TECH_MINING")));
    assert_eq!(farm.feature_time[forest], 250);
    assert_eq!(farm.feature_production[forest], 20);
    assert_eq!(farm.feature_time[ancient], 500);
    assert_eq!(farm.feature_production[ancient], 30);
    // Features without a <FeatureStruct> keep the SetFeatureStruct defaults.
    let flood = d.t::<FeatureType>("FEATURE_FLOOD_PLAINS");
    assert_eq!(farm.feature_tech[flood], None);
    assert_eq!(farm.feature_time[flood], 0);
    assert!(!farm.feature_remove[flood]);

    let road = &d.builds[d.t::<BuildType>("BUILD_ROAD")];
    assert_eq!(road.route, Some(d.t::<RouteType>("ROUTE_ROAD")));
    assert_eq!(road.improvement, None);
    assert_eq!(road.time, 200);
    assert_eq!(road.tech_prereq, Some(d.t::<TechType>("TECH_EXPLORATION")));
    assert!(road.feature_tech.iter().all(|t| t.is_none()));

    assert!(d.builds[d.t::<BuildType>("BUILD_FISHING_BOATS")].kill);
}

#[test]
fn missions_commands_automates() {
    let d = data();
    let mv = &d.missions[MissionType::MISSION_MOVE_TO];
    assert_eq!(mv.base.type_, "MISSION_MOVE_TO");
    assert_eq!(mv.time, 4);
    assert!(mv.target && !mv.build && !mv.visible);
    let build = &d.missions[MissionType::MISSION_BUILD];
    assert!(build.build);
    assert_eq!(build.time, 0);
    let pillage = &d.missions[MissionType::MISSION_PILLAGE];
    assert_eq!(pillage.time, 10);
    assert!(pillage.visible);

    assert!(d.commands[CommandType::COMMAND_UPGRADE].all);
    assert!(!d.commands[CommandType::COMMAND_PROMOTION].all);
    let del = &d.commands[CommandType::COMMAND_DELETE];
    assert!(del.confirm_command && del.visible);
    // The entry inside the nested <CommandInfos> container keeps its enum slot.
    assert_eq!(d.commands[CommandType::COMMAND_AUTOMATE_SPELL].base.type_, "COMMAND_AUTOMATE_SPELL");
    assert!(d.commands.iter().all(|c| c.automate.is_none()));

    for (i, a) in d.automates.iter_enumerated() {
        assert_eq!(a.command, Some(CommandType::COMMAND_AUTOMATE));
        assert_eq!(a.automate, Some(i), "automate entries are in AutomateTypes order");
        assert!(a.visible);
        assert_eq!(a.base.type_, "", "CIV4AutomateInfos.xml has no <Type>");
    }
    assert_eq!(d.automates[AutomateType::AUTOMATE_TERRAFORMING].base.description, "TXT_KEY_ACTION_AUTOMATE_TERRAFORMING");
}

#[test]
fn special_units() {
    let d = data();
    let spell_unit = &d.special_units[d.t::<SpecialUnitType>("SPECIALUNIT_SPELL")];
    assert!(spell_unit.valid && spell_unit.city_load);
    let people = &d.special_units[d.t::<SpecialUnitType>("SPECIALUNIT_PEOPLE")];
    assert!(people.valid && !people.city_load);
    // C++ quirk: the XML's <CarrierUnitAIs> is never read (C++ looks for <CarrierUnitAITypes>).
    for s in d.special_units.iter() {
        assert_eq!(s.carrier_unit_ai_type.len(), UnitAIType::NUM);
        assert!(s.carrier_unit_ai_type.iter().all(|b| !b));
        assert!(s.production_traits.iter().all(|v| *v == 0));
    }
}
