//! Data spot-checks for SpecialBuildingInfo, BuildingClassInfo, BuildingInfo, TraitInfo,
//! CivilizationInfo and LeaderHeadInfo. Every expected value was read from the vendored
//! XML (data/xml/Buildings, data/xml/Civilizations), or is the C++ default for an absent tag.

use ffh_data::*;

fn d() -> &'static GameData {
    global()
}

fn building(name: &str) -> &'static BuildingInfo {
    &d().buildings[d().t::<BuildingType>(name)]
}

fn building_class(name: &str) -> &'static BuildingClassInfo {
    &d().building_classes[d().t::<BuildingClassType>(name)]
}

fn civ(name: &str) -> &'static CivilizationInfo {
    &d().civilizations[d().t::<CivilizationType>(name)]
}

fn leader(name: &str) -> &'static LeaderHeadInfo {
    &d().leader_heads[d().t::<LeaderHeadType>(name)]
}

fn trait_(name: &str) -> &'static TraitInfo {
    &d().traits[d().t::<TraitType>(name)]
}

fn bt(name: &str) -> BuildingType {
    d().t(name)
}
fn bct(name: &str) -> BuildingClassType {
    d().t(name)
}
fn uct(name: &str) -> UnitClassType {
    d().t(name)
}
fn ut(name: &str) -> UnitType {
    d().t(name)
}
fn bonus(name: &str) -> BonusType {
    d().t(name)
}
fn spec(name: &str) -> SpecialistType {
    d().t(name)
}
fn tech(name: &str) -> TechType {
    d().t(name)
}
fn trt(name: &str) -> TraitType {
    d().t(name)
}
fn flavor(name: &str) -> FlavorType {
    d().t(name)
}

#[test]
fn loads_without_warnings() {
    let w = &d().warnings;
    for x in w {
        println!("warning: {x}");
    }
    assert!(w.is_empty(), "{} data warnings:\n{}", w.len(), w.join("\n"));
}

#[test]
fn table_sizes() {
    assert_eq!(d().special_buildings.len(), 1);
    assert_eq!(d().building_classes.len(), 131);
    assert_eq!(d().buildings.len(), 170);
    assert_eq!(d().traits.len(), 29);
    assert_eq!(d().civilizations.len(), 23);
    assert_eq!(d().leader_heads.len(), 69);
    for b in d().buildings.iter() {
        assert!(b.building_class_type.is_some(), "{} has no class", b.base.type_);
        // Arrays the C++ sizes with NUM_* enum constants rather than GC.getNumXInfos().
        assert_eq!(b.domain_free_experience.len(), DomainType::NUM);
        assert_eq!(b.domain_production_modifier.len(), DomainType::NUM);
    }
    for l in d().leader_heads.iter() {
        assert_eq!(l.contact_rand.len(), ContactType::NUM);
        assert_eq!(l.contact_delay.len(), ContactType::NUM);
        assert_eq!(l.memory_decay_rand.len(), MemoryType::NUM);
        assert_eq!(l.memory_attitude_percent.len(), MemoryType::NUM);
        assert_eq!(l.no_war_attitude_prob.len(), AttitudeType::NUM);
        assert_eq!(l.unit_ai_weight_modifier.len(), UnitAIType::NUM);
        assert_eq!(l.has_trait.len(), d().traits.len());
    }
    for c in d().civilizations.iter() {
        assert_eq!(c.civilization_building_overrides.len(), d().building_classes.len());
        assert_eq!(c.civilization_unit_overrides.len(), d().unit_classes.len());
        assert_eq!(c.civilization_initial_civics.len(), d().civic_options.len());
    }
}

#[test]
fn fixed_prereq_lists_match_global_defines() {
    // The reader mirrors these two defines; they must agree with GlobalDefines(Alt).xml.
    let defs = &d().defines;
    for b in d().buildings.iter() {
        assert_eq!(b.prereq_and_techs.len() as i32, defs.num_building_and_tech_prereqs);
        assert_eq!(b.prereq_or_bonuses.len() as i32, defs.num_building_prereq_or_bonuses);
    }
    assert_eq!(defs.num_building_prereq_or_bonuses, 5, "GlobalDefinesAlt.xml overrides 4 -> 5");
}

#[test]
fn academy_commerce_and_defaults() {
    let b = building("BUILDING_ACADEMY");
    assert_eq!(b.building_class_type, Some(bct("BUILDINGCLASS_ACADEMY")));
    assert_eq!(b.production_cost, -1);
    assert_eq!(b.advanced_start_cost, -1);
    assert_eq!(b.min_area_size, -1);
    assert_eq!(b.conquest_probability, 100);
    assert_eq!(b.max_latitude, 90);
    assert_eq!(b.asset_value, 6);
    assert_eq!(b.commerce_modifier, [0, 50, 0, 0]);
    assert_eq!(b.obsolete_safe_commerce_change, [0, 0, 4, 0]);
    assert_eq!(b.commerce_change, [0; 4]);
    assert_eq!(b.flavor_value[flavor("FLAVOR_SCIENCE")], 10);
    assert_eq!(b.flavor_value[flavor("FLAVOR_MILITARY")], 0);
    assert_eq!(b.prereq_and_tech, None);
    assert_eq!(b.free_promotion, None);
    assert!(!b.capital && !b.victory_building);
}

#[test]
fn palace_sidar_yields_free_bonuses_and_2d_arrays() {
    let b = building("BUILDING_PALACE_SIDAR");
    assert_eq!(b.building_class_type, Some(bct("BUILDINGCLASS_PALACE")));
    assert_eq!(b.production_cost, 160);
    assert_eq!(b.hurry_cost_modifier, 50);
    assert_eq!(b.num_cities_prereq, 4);
    assert_eq!(b.yield_change, [0, 1, 8]);
    assert_eq!(b.obsolete_safe_commerce_change, [0, 0, 2, 0]);
    assert_eq!(b.happiness, 1);
    assert_eq!(b.crime, -10);
    assert!(b.capital && b.government_center && b.never_capture && b.nuke_immune);
    assert_eq!(b.free_bonus, Some(bonus("BONUS_MANA_SPIRIT")));
    assert_eq!(b.num_free_bonuses, 1);
    assert_eq!(b.free_bonus2, Some(bonus("BONUS_MANA_ENCHANTMENT")));
    assert_eq!(b.free_bonus3, Some(bonus("BONUS_MANA_SHADOW")));
    // SpecialistYieldChanges
    assert_eq!(b.specialist_yield_change.len(), d().specialists.len());
    assert_eq!(b.specialist_yield_change[spec("SPECIALIST_ENGINEER")], [0, 1, 0]);
    assert_eq!(b.specialist_yield_change[spec("SPECIALIST_GREAT_ENGINEER")], [0, 1, 0]);
    assert_eq!(b.specialist_yield_change[spec("SPECIALIST_PRIEST")], [0, 0, 0]);
    // SpecialistCommerceChanges (FfH)
    assert_eq!(b.specialist_commerce_change[spec("SPECIALIST_MERCHANT")], [1, 0, 0, 0]);
    assert_eq!(b.specialist_commerce_change[spec("SPECIALIST_SCIENTIST")], [0, 1, 0, 0]);
    assert_eq!(b.specialist_commerce_change[spec("SPECIALIST_GREAT_ARTIST")], [0, 0, 1, 0]);
    assert_eq!(b.specialist_commerce_change[spec("SPECIALIST_ENGINEER")], [0; 4]);
}

#[test]
fn great_library_prereqs_and_pair_lists() {
    let b = building("BUILDING_GREAT_LIBRARY");
    assert_eq!(b.prereq_and_tech, Some(tech("TECH_WRITING")));
    assert_eq!(b.production_cost, 350);
    assert_eq!(b.great_people_unit_class, Some(uct("UNITCLASS_SCIENTIST")));
    assert_eq!(b.great_people_rate_change, 2);
    assert!(b.team_share);
    let library = bct("BUILDINGCLASS_LIBRARY");
    assert_eq!(b.prereq_num_of_building_class[library], 3);
    assert!(b.building_class_needed_in_city[library]);
    assert!(!b.building_class_needed_in_city[bct("BUILDINGCLASS_ACADEMY")]);
    assert_eq!(b.free_specialist_count[spec("SPECIALIST_SCIENTIST")], 1);
    // The XML value tag is misspelled (<iProductonModifier>); pair values are positional.
    assert_eq!(b.bonus_production_modifier[bonus("BONUS_MARBLE")], 100);
    assert_eq!(b.commerce_change_double_time, [0, 0, 1000, 0]);
    assert_eq!(b.commerce_change_original_owner, [false, false, true, false]);
    assert_eq!(b.commerce_flexible, [false; 4]);
}

#[test]
fn tower_of_alteration_prereq_or_bonuses() {
    let b = building("BUILDING_TOWER_OF_ALTERATION");
    assert_eq!(b.prereq_and_tech, Some(tech("TECH_ALTERATION")));
    assert_eq!(
        b.prereq_or_bonuses,
        vec![
            Some(bonus("BONUS_MANA_BODY")),
            Some(bonus("BONUS_MANA_LIFE")),
            Some(bonus("BONUS_MANA_ENCHANTMENT")),
            Some(bonus("BONUS_MANA_NATURE")),
            None,
        ]
    );
    assert_eq!(b.prereq_and_techs, vec![None, None, None]);
    assert_eq!(b.production_cost, 800);
    assert_eq!(b.power_value, 8);
    assert_eq!(b.global_resist_enemy_modify, -20);
    assert_eq!(b.global_resist_modify, 20);
}

#[test]
fn tower_of_eyes_free_promotion_and_free_building() {
    let b = building("BUILDING_TOWER_OF_EYES");
    assert_eq!(b.free_promotion, Some(d().t::<PromotionType>("PROMOTION_SENTRY")));
    assert_eq!(b.free_building_class, Some(bct("BUILDINGCLASS_DUNGEON")));
    assert_eq!(b.prereq_and_tech, Some(tech("TECH_OPTICS")));
    assert_eq!(b.great_people_unit_class, Some(uct("UNITCLASS_COMMANDER")));
    assert_eq!(b.obsolete_safe_commerce_change, [0, 0, 3, 0]);
}

#[test]
fn brewery_bonus_yield_modifiers() {
    let b = building("BUILDING_BREWERY");
    assert_eq!(b.prereq_and_tech, Some(tech("TECH_CRAFTING")));
    assert_eq!(b.free_bonus, Some(bonus("BONUS_ALE")));
    assert_eq!(b.num_free_bonuses, 2);
    assert!(b.river);
    assert_eq!(b.bonus_yield_modifier.len(), d().bonuses.len());
    for name in ["BONUS_CORN", "BONUS_RICE", "BONUS_WHEAT"] {
        assert_eq!(b.bonus_yield_modifier[bonus(name)], [0, 0, 10], "{name}");
    }
    assert_eq!(b.bonus_yield_modifier[bonus("BONUS_MARBLE")], [0, 0, 0]);
}

#[test]
fn temple_of_leaves_religion_and_specialists() {
    let b = building("BUILDING_TEMPLE_OF_LEAVES");
    let leaves = d().t::<ReligionType>("RELIGION_FELLOWSHIP_OF_LEAVES");
    assert_eq!(b.special_building_type, Some(d().t::<SpecialBuildingType>("SPECIALBUILDING_TEMPLE")));
    assert_eq!(b.religion_type, Some(leaves));
    assert_eq!(b.prereq_religion, Some(leaves));
    assert!(!b.is_prereq_religion && !b.is_state_religion);
    assert_eq!(b.state_religion, None);
    assert_eq!(b.prereq_and_tech, Some(tech("TECH_WAY_OF_THE_FORESTS")));
    assert_eq!(b.health, 1);
    assert_eq!(b.commerce_modifier, [0, 0, 20, 0]);
    assert_eq!(b.specialist_count[spec("SPECIALIST_ARTIST")], 1);
    assert_eq!(b.specialist_count[spec("SPECIALIST_PRIEST")], 1);
    assert_eq!(b.specialist_count[spec("SPECIALIST_SCIENTIST")], 0);
    assert_eq!(b.bonus_happiness_changes[bonus("BONUS_INCENSE")], 1);
}

#[test]
fn palisade_production_traits_and_defense() {
    let b = building("BUILDING_PALISADE");
    assert_eq!(b.production_traits[trt("TRAIT_DEFENDER")], 100);
    assert_eq!(b.production_traits[trt("TRAIT_SPIRITUAL")], 0);
    assert_eq!(b.defense_modifier, 10);
    assert_eq!(b.production_cost, 40);
    assert!(b.center_in_city);
}

#[test]
fn prereq_civ_is_never_resolved_cxx_quirk() {
    let b = building("BUILDING_ADVENTURERS_GUILD");
    assert_eq!(b.building_class_type, Some(bct("BUILDINGCLASS_CIV_BUILDING1")));
    assert_eq!(b.prereq_civ, None, "C++ never calls CvBuildingInfo::readPass3");
    assert_eq!(b.prereq_civ_xml, Some(d().t::<CivilizationType>("CIVILIZATION_GRIGORI")));
    assert_eq!(b.domain_free_experience[DomainType::DOMAIN_LAND], 2);
    assert_eq!(b.domain_free_experience[DomainType::DOMAIN_SEA], 0);
    assert!(d().buildings.iter().all(|b| b.prereq_civ.is_none()));
    assert_eq!(d().buildings.iter().filter(|b| b.prereq_civ_xml.is_some()).count(), 15);
}

#[test]
fn building_classes() {
    let palace = building_class("BUILDINGCLASS_PALACE");
    assert_eq!(palace.max_global_instances, -1);
    assert_eq!(palace.max_team_instances, -1);
    assert_eq!(palace.max_player_instances, 1);
    assert_eq!(palace.extra_player_instances, 1);
    assert!(palace.no_limit && !palace.monument);
    assert_eq!(palace.default_building_index, Some(bt("BUILDING_PALACE_BANNOR")));

    let gl = building_class("BUILDINGCLASS_GREAT_LIBRARY");
    assert_eq!(gl.max_global_instances, 1);
    assert_eq!(gl.max_player_instances, -1);
    assert_eq!(gl.default_building_index, Some(bt("BUILDING_GREAT_LIBRARY")));

    assert_eq!(building_class("BUILDINGCLASS_CIV_BUILDING1").default_building_index, None);

    let mastery = building_class("BUILDINGCLASS_TOWER_OF_MASTERY");
    assert_eq!(mastery.victory_threshold[d().t::<VictoryType>("VICTORY_TOWER_OF_MASTERY")], 1);
    assert_eq!(mastery.max_player_instances, 1);
    assert_eq!(gl.victory_threshold.iter().sum::<i32>(), 0);
}

#[test]
fn special_building_temple() {
    let s = &d().special_buildings[d().t::<SpecialBuildingType>("SPECIALBUILDING_TEMPLE")];
    assert!(s.valid);
    assert_eq!(s.production_traits[trt("TRAIT_SPIRITUAL")], 100);
    assert_eq!(s.production_traits[trt("TRAIT_DEFENDER")], 0);
    assert_eq!((s.obsolete_tech, s.tech_prereq, s.tech_prereq_anyone), (None, None, None));
}

#[test]
fn traits() {
    let phil = trait_("TRAIT_PHILOSOPHICAL");
    assert_eq!(phil.great_people_rate_modifier, 100);
    assert_eq!(phil.max_anarchy, -1);
    assert_eq!(phil.max_cities, -1);
    assert!(phil.selectable);
    assert_eq!(phil.short_description, "TXT_KEY_TRAIT_PHILOSOPHICAL_SHORT");

    let aggr = trait_("TRAIT_AGGRESSIVE");
    let combat1 = d().t::<PromotionType>("PROMOTION_COMBAT1");
    assert!(aggr.free_promotion[combat1]);
    assert_eq!(aggr.free_promotion.iter().filter(|&&b| b).count(), 1);
    assert_eq!(aggr.free_promotion_unit_combat.iter().filter(|&&b| b).count(), 6);
    assert!(aggr.free_promotion_unit_combat[d().t::<UnitCombatType>("UNITCOMBAT_MELEE")]);
    assert!(!aggr.free_promotion_unit_combat[d().t::<UnitCombatType>("UNITCOMBAT_SIEGE")]);

    assert_eq!(trait_("TRAIT_FINANCIAL").extra_yield_threshold, [0, 0, 2]);
    assert_eq!(trait_("TRAIT_EXPANSIVE").health, 3);

    let sprawl = trait_("TRAIT_SPRAWLING");
    assert!(sprawl.sprawling && !sprawl.selectable);
    assert_eq!(sprawl.happiness, 3);
    assert_eq!(sprawl.max_cities, 3);
}

#[test]
fn khazad_unique_units_buildings_and_start() {
    let c = civ("CIVILIZATION_KHAZAD");
    let classes = &d().building_classes;
    // Unique units / buildings.
    assert_eq!(c.civilization_unit_overrides[uct("UNITCLASS_ARCHER")], Some(Some(ut("UNIT_DWARVEN_SLINGER"))));
    assert_eq!(c.civilization_units(uct("UNITCLASS_ARCHER"), Some(ut("UNIT_ARCHER"))), Some(ut("UNIT_DWARVEN_SLINGER")));
    assert_eq!(c.civilization_units(uct("UNITCLASS_HORSEMAN"), Some(ut("UNIT_HORSEMAN"))), Some(ut("UNIT_BOAR_RIDER")));
    // Not overridden: the class default (UNITCLASS_WARRIOR's DefaultUnit is UNIT_WARRIOR).
    assert_eq!(c.civilization_unit_overrides[uct("UNITCLASS_WARRIOR")], None);
    assert_eq!(c.civilization_units(uct("UNITCLASS_WARRIOR"), Some(ut("UNIT_WARRIOR"))), Some(ut("UNIT_WARRIOR")));
    assert_eq!(c.civilization_unit_overrides.iter().filter(|o| o.is_some()).count(), 52);
    assert_eq!(c.civilization_buildings(bct("BUILDINGCLASS_FORGE"), classes), Some(bt("BUILDING_DWARVEN_SMITHY")));
    assert_eq!(c.civilization_buildings(bct("BUILDINGCLASS_PALACE"), classes), Some(bt("BUILDING_PALACE_KHAZAD")));
    assert_eq!(c.civilization_buildings(bct("BUILDINGCLASS_LIBRARY"), classes), Some(bt("BUILDING_LIBRARY")));
    // Starting units, techs, buildings, civics.
    assert_eq!(c.civilization_free_units_class[uct("UNITCLASS_SETTLER")], 1);
    assert_eq!(c.civilization_free_units_class[uct("UNITCLASS_WARRIOR")], 2);
    assert_eq!(c.civilization_free_units_class[uct("UNITCLASS_SCOUT")], 0);
    assert!(c.civilization_free_techs[tech("TECH_CRAFTING")]);
    assert!(!c.civilization_free_techs[tech("TECH_EXPLORATION")]);
    assert!(c.civilization_free_building_class[bct("BUILDINGCLASS_PALACE")]);
    let civics: Vec<_> = c.civilization_initial_civics.iter().map(|o| o.map(|x| d().type_name(x))).collect();
    assert_eq!(
        civics,
        [Some("CIVIC_DESPOTISM"), Some("CIVIC_RELIGION"), Some("CIVIC_TRIBALISM"), Some("CIVIC_DECENTRALIZATION"), Some("CIVIC_NO_MEMBERSHIP")]
    );
    // FfH fields and leaders.
    assert_eq!(c.default_race, Some(d().t::<PromotionType>("PROMOTION_DWARF")));
    assert_eq!(c.hero, Some(ut("UNIT_MAROS")));
    assert_eq!(c.civ_trait, None);
    let leaders: Vec<_> = c.leaders.iter_enumerated().filter(|(_, &b)| b).map(|(l, _)| d().type_name(l)).collect();
    assert_eq!(leaders, ["LEADER_ARTURUS", "LEADER_KANDROS"]);
    assert!(c.playable && c.ai_playable);
    assert_eq!(c.city_names.len(), 23);
    assert_eq!(c.city_names[0], "TXT_KEY_CITY_KHAZAD_1");
}

#[test]
fn grigori_and_bannor_class_overrides() {
    let classes = &d().building_classes;
    let grigori = civ("CIVILIZATION_GRIGORI");
    let bannor = civ("CIVILIZATION_BANNOR");
    let civ1 = bct("BUILDINGCLASS_CIV_BUILDING1");
    let pagan = bct("BUILDINGCLASS_PAGAN_TEMPLE");
    // Unique building in a class whose default is NONE.
    assert_eq!(grigori.civilization_buildings(civ1, classes), Some(bt("BUILDING_ADVENTURERS_GUILD")));
    assert_eq!(bannor.civilization_buildings(civ1, classes), None);
    // Explicit NONE override beats the class default.
    assert_eq!(grigori.civilization_building_overrides[pagan], Some(None));
    assert_eq!(grigori.civilization_buildings(pagan, classes), None);
    assert_eq!(bannor.civilization_buildings(pagan, classes), Some(bt("BUILDING_PAGAN_TEMPLE")));
    // Bannor removes the adventurer unit (UnitType NONE).
    let adv = uct("UNITCLASS_ADVENTURER");
    assert_eq!(bannor.civilization_unit_overrides[adv], Some(None));
    assert_eq!(bannor.civilization_units(adv, Some(ut("UNIT_ADVENTURER"))), None);
    assert_eq!(grigori.civilization_units(uct("UNITCLASS_CHAMPION"), None), Some(ut("UNIT_DRAGON_SLAYER")));
    // Starting setup.
    assert_eq!(bannor.civ_trait, Some(trt("TRAIT_GUARDSMAN")));
    assert_eq!(bannor.hero, Some(ut("UNIT_DONAL")));
    assert_eq!(bannor.default_race, None);
    assert!(bannor.civilization_free_techs[tech("TECH_EXPLORATION")]);
    assert_eq!(bannor.civilization_free_units_class[uct("UNITCLASS_SCOUT")], 1);
    assert_eq!(grigori.civ_trait, Some(trt("TRAIT_AGNOSTIC")));
    assert_eq!(grigori.hero, None);
    let opt1 = CivicOptionType(1);
    assert_eq!(grigori.civilization_initial_civics[opt1], Some(d().t::<CivicType>("CIVIC_PACIFISM")));
    assert_eq!(bannor.short_description_key, "TXT_KEY_CIV_BANNOR_SHORT_DESC");
    assert_eq!(bannor.adjective_key, "TXT_KEY_CIV_BANNOR_ADJECTIVE");
    // Infernals are not playable; their unique units still map.
    let inf = civ("CIVILIZATION_INFERNAL");
    assert!(!inf.playable && !inf.ai_playable);
    assert_eq!(inf.civilization_units(uct("UNITCLASS_ADEPT"), None), Some(ut("UNIT_IMP")));
    assert_eq!(inf.civilization_buildings(bct("BUILDINGCLASS_AQUEDUCT"), classes), None);
    assert_eq!(inf.default_race, Some(d().t::<PromotionType>("PROMOTION_DEMON")));
}

#[test]
fn civ_ai_defaults_and_derivative() {
    // No civ sets the Better AI tags, so the hard-coded C++ defaults apply everywhere.
    for c in d().civilizations.iter() {
        assert_eq!(c.best_defender1, Some(uct("UNITCLASS_ARCHER")), "{}", c.base.type_);
        assert_eq!(c.best_defender2, Some(uct("UNITCLASS_LONGBOWMAN")));
        assert_eq!(c.best_defender3, Some(uct("UNITCLASS_ARQUEBUS")));
        assert_eq!(c.best_patrol1, Some(uct("UNITCLASS_HUNTER")));
        assert_eq!(c.best_patrol2, Some(uct("UNITCLASS_RANGER")));
        assert_eq!(c.best_patrol3, None);
        assert_eq!(c.derivative_civ, None);
        // Every civ has <Buildings>, so no class is forced to NONE wholesale.
        assert!(c.civilization_building_overrides.iter().any(|o| o.is_none()));
    }
}

#[test]
fn leader_sabathiel_personality() {
    let l = leader("LEADER_SABATHIEL");
    let traits: Vec<_> = l.has_trait.iter_enumerated().filter(|(_, &b)| b).map(|(t, _)| d().type_name(t)).collect();
    assert_eq!(traits, ["TRAIT_ORGANIZED", "TRAIT_CHARISMATIC"]);
    assert_eq!(l.alignment, Some(AlignmentType::ALIGNMENT_GOOD));
    assert!(!l.female);
    assert_eq!(l.favorite_civic, Some(d().t::<CivicType>("CIVIC_THEOCRACY")));
    assert_eq!(l.favorite_religion, None);
    assert_eq!(l.favorite_wonder, Some(bt("BUILDING_AQUAE_SUCELLUS")));
    assert_eq!(l.favorite_unit_combat, Some(d().t::<UnitCombatType>("UNITCOMBAT_MELEE")));
    assert_eq!(l.base_attitude, 2);
    assert_eq!(l.build_unit_prob, 30);
    assert_eq!(l.raze_city_prob, 10);
    assert_eq!(l.limited_war_rand, 180);
    assert_eq!(l.at_war_attitude_divisor, -5);
    assert_eq!(l.same_religion_attitude_change, 4);
    assert_eq!(l.attitude_bad_bonus, 2);
    assert_eq!(l.somnium_aggressiveness, 35);
    assert_eq!(l.economy_tech_value, 130);
    assert!(l.altar_victory && l.religion_victory && !l.culture_victory);
    assert_eq!(l.tech_refuse_attitude_threshold, Some(AttitudeType::ATTITUDE_ANNOYED));
    assert_eq!(l.happiness_bonus_refuse_attitude_threshold, None);
    assert_eq!(l.convert_religion_refuse_attitude_threshold, Some(AttitudeType::ATTITUDE_FRIENDLY));
    assert_eq!(l.favorite_early_mil_tech, Some(tech("TECH_BRONZE_WORKING")));
    assert_eq!(l.favorite_early_tech2, Some(tech("TECH_ORDERS_FROM_HEAVEN")));
    assert_eq!(l.favorite_early_tech3, None);
    assert_eq!(l.favorite_early_religion, Some(d().t::<ReligionType>("RELIGION_RUNES_OF_KILMORPH")));
    assert_eq!(l.favorite_early_wonder, Some(bct("BUILDINGCLASS_FORM_OF_THE_TITAN")));
    assert_eq!(l.favorite_late_wonder, Some(bct("BUILDINGCLASS_TEMPLE_OF_TEMPORENCE")));
    // Flavors and arrays.
    assert_eq!(l.flavor_value[flavor("FLAVOR_MILITARY")], 30);
    assert_eq!(l.flavor_value[flavor("FLAVOR_SCIENCE")], -5);
    assert_eq!(l.flavor_value[flavor("FLAVOR_GOLD")], 0);
    assert_eq!(l.contact_rand[ContactType::CONTACT_RELIGION_PRESSURE], 1);
    assert_eq!(l.contact_rand[ContactType::CONTACT_CIVIC_PRESSURE], 500);
    assert_eq!(l.contact_delay[ContactType::CONTACT_RELIGION_PRESSURE], 50);
    assert_eq!(l.contact_delay[ContactType::CONTACT_DEMAND_TRIBUTE], 100);
    assert_eq!(l.contact_delay[ContactType::CONTACT_PEACE_TREATY], 10);
    assert_eq!(l.memory_decay_rand[MemoryType::MEMORY_GIVE_HELP], 200);
    assert_eq!(l.memory_decay_rand[MemoryType::MEMORY_DECLARED_WAR], 0);
    assert_eq!(l.memory_attitude_percent[MemoryType::MEMORY_DECLARED_WAR], -300);
    assert_eq!(l.memory_attitude_percent[MemoryType::MEMORY_VOTED_FOR_US], 200);
    assert_eq!(l.no_war_attitude_prob[AttitudeType::ATTITUDE_ANNOYED], 10);
    assert_eq!(l.no_war_attitude_prob[AttitudeType::ATTITUDE_FRIENDLY], 100);
    assert_eq!(l.no_war_attitude_prob[AttitudeType::ATTITUDE_FURIOUS], 0);
    assert_eq!(l.improvement_weight_modifier[d().t::<ImprovementType>("IMPROVEMENT_MANA_CHAOS")], -40);
    assert_eq!(l.improvement_weight_modifier[d().t::<ImprovementType>("IMPROVEMENT_CITADEL")], 40);
    assert_eq!(l.religion_weight_modifier[d().t::<ReligionType>("RELIGION_THE_ORDER")], 100);
    assert_eq!(l.religion_weight_modifier[d().t::<ReligionType>("RELIGION_COUNCIL_OF_ESUS")], -10);
    assert!(l.unit_ai_weight_modifier.iter().all(|&w| w == 0));
    // Absent tags take the C++ read() defaults.
    assert_eq!(l.max_war_rand, 50);
    assert_eq!(l.at_peace_attitude_divisor, 10);
    assert_eq!(l.at_peace_attitude_change_limit, 5);
    assert_eq!(l.same_religion_attitude_divisor, 30);
    assert_eq!(l.different_religion_attitude_divisor, -30);
    assert_eq!(l.different_religion_attitude_change_limit, -5);
    assert_eq!(l.defensive_pact_attitude_divisor, 50);
    assert_eq!(l.share_war_attitude_change, 2);
    assert_eq!(l.wonder_construct_rand, 0);
}

#[test]
fn leader_read_defaults() {
    let random_good = leader("LEADER_RANDOM_GOOD");
    assert_eq!(random_good.build_unit_prob, 25);
    assert_eq!(random_good.at_war_attitude_divisor, 10);
    assert_eq!(random_good.ai_patrol_group_size, 100);
    assert_eq!(random_good.ai_value_mage, 100);
    assert_eq!(random_good.favorite_civic, Some(d().t::<CivicType>("CIVIC_FOREIGN_TRADE")));
    assert_eq!(random_good.vassal_power_modifier, 20);
    let decius = leader("LEADER_DECIUS");
    assert_eq!(decius.economy_tech_value, 100);
    assert_eq!(decius.ai_patrol_group_size, 100);
    let capria = leader("LEADER_CAPRIA");
    assert!(capria.female);
    assert_eq!(capria.at_peace_attitude_change_limit, 4);
    assert!(capria.has_trait[trt("TRAIT_INDUSTRIOUS")] && capria.has_trait[trt("TRAIT_SPIRITUAL")]);
}

#[test]
fn leader_favorite_wonder_class_index_cxx_quirk() {
    // FindInInfoClass is class-agnostic: a BUILDING_* name in a building-class field
    // stores the building's index, which C++/Python then use as a building class.
    let as_class = |b: BuildingType| BuildingClassType(b.0);
    let capria = leader("LEADER_CAPRIA");
    assert_eq!(capria.favorite_early_wonder, Some(bct("BUILDINGCLASS_FORM_OF_THE_TITAN")));
    assert_eq!(capria.favorite_late_wonder, Some(as_class(bt("BUILDING_AQUAE_SUCELLUS"))));
    let thessa = leader("LEADER_THESSA");
    assert_eq!(thessa.favorite_early_wonder, Some(as_class(bt("BUILDING_CATACOMB_LIBRALUS"))));
    assert_eq!(thessa.favorite_late_wonder, Some(as_class(bt("BUILDING_GREAT_LIBRARY"))));
}
