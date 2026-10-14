//! Data spot-checks for the terrain tables (yields, terrains, features, bonus classes,
//! bonuses, improvements, routes) and the basic info tables. Every expected value was
//! read from the vendored XML under `data/xml/`.

use ffh_data::*;

fn ty<T: InfoId>(name: &str) -> T {
    global().t::<T>(name)
}

/// Type names of the entries set in a per-type bool array, in table order.
fn set_names<T: InfoId>(v: &IdxVec<T, bool>) -> Vec<&'static str> {
    let d = global();
    v.iter_enumerated().filter(|(_, &on)| on).map(|(t, _)| d.type_name(t)).collect()
}

#[test]
fn data_loads_without_warnings() {
    let d = global();
    assert!(d.warnings.is_empty(), "{} data warnings:\n  {}", d.warnings.len(), d.warnings.join("\n  "));
}

#[test]
fn table_sizes() {
    let d = global();
    let sizes = [
        ("yields", d.yields.len(), 3),
        ("terrains", d.terrains.len(), 14),
        ("features", d.features.len(), 20),
        ("bonus_classes", d.bonus_classes.len(), 8),
        ("bonuses", d.bonuses.len(), 58),
        ("improvements", d.improvements.len(), 81),
        ("routes", d.routes.len(), 2),
        ("alignments", d.alignments.len(), 3),
        ("attitudes", d.attitudes.len(), 5),
        ("calendars", d.calendars.len(), 7),
        ("damages", d.damages.len(), 8),
        ("denials", d.denials.len(), 29),
        ("domains", d.domains.len(), 4),
        ("invisibles", d.invisibles.len(), 3),
        ("memories", d.memories.len(), 36),
        ("months", d.months.len(), 12),
        ("seasons", d.seasons.len(), 4),
        ("unit_ais", d.unit_ais.len(), 49),
        ("unit_combats", d.unit_combats.len(), 10),
    ];
    for (name, got, want) in sizes {
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn yields() {
    let d = global();
    let food = &d.yields[YieldType::YIELD_FOOD];
    assert_eq!(food.base.type_, "YIELD_FOOD");
    assert_eq!((food.hills_change, food.peak_change, food.lake_change), (-1, 0, 1));
    assert_eq!((food.min_city, food.golden_age_yield, food.ai_weight_percent), (2, 0, 100));
    let prod = &d.yields[YieldType::YIELD_PRODUCTION];
    assert_eq!((prod.hills_change, prod.min_city, prod.ai_weight_percent), (1, 1, 110));
    assert_eq!((prod.golden_age_yield, prod.golden_age_yield_threshold), (1, 1));
    let commerce = &d.yields[YieldType::YIELD_COMMERCE];
    assert_eq!((commerce.trade_modifier, commerce.ai_weight_percent), (100, 80));
}

#[test]
fn terrains() {
    let d = global();
    let grass = &d.terrains[ty::<TerrainType>("TERRAIN_GRASS")];
    assert_eq!(grass.yield_, [2, 0, 0]);
    assert_eq!(grass.river_yield_change, [0, 0, 1]);
    assert_eq!(grass.hills_yield_change, [0, 0, 0]);
    assert_eq!((grass.movement_cost, grass.see_from_level, grass.see_through_level), (1, 1, 1));
    assert!(grass.found && grass.normalize && !grass.water && !grass.impassable);
    assert_eq!((grass.plot_counter_up, grass.terrain_up), (9, d.ty("TERRAIN_BROKEN_LANDS")));
    assert_eq!((grass.plot_counter_down, grass.terrain_down), (0, None));
    assert_eq!(grass.civilization_yield_type, None);

    let desert = &d.terrains[ty::<TerrainType>("TERRAIN_DESERT")];
    assert_eq!(desert.yield_, [0, 0, 0], "<Yields/> is all zero");
    assert_eq!((desert.movement_cost, desert.build_modifier, desert.defense_modifier), (2, 25, -25));
    assert_eq!(desert.terrain_up, d.ty("TERRAIN_BURNING_SANDS"));
    assert_eq!(desert.civilization_yield_type, d.ty("CIVILIZATION_MALAKIM"));
    assert_eq!(desert.civilization_yield_change, [0, 0, 1]);

    let snow = &d.terrains[ty::<TerrainType>("TERRAIN_SNOW")];
    assert_eq!(snow.civilization_yield_type, d.ty("CIVILIZATION_ILLIANS"));
    assert_eq!(snow.civilization_yield_change, [2, 0, 0]);

    let coast = &d.terrains[ty::<TerrainType>("TERRAIN_COAST")];
    assert!(coast.water);
    assert_eq!(coast.defense_modifier, 10);
    assert!(d.terrains[ty::<TerrainType>("TERRAIN_PEAK")].base.graphical_only);
}

#[test]
fn features() {
    let d = global();
    let jungle = &d.features[ty::<FeatureType>("FEATURE_JUNGLE")];
    assert_eq!(jungle.yield_change, [-1, 0, 0]);
    assert_eq!((jungle.movement_cost, jungle.see_through_change, jungle.defense_modifier), (2, 1, 25));
    assert_eq!((jungle.health_percent, jungle.growth_probability), (-25, 16));
    assert_eq!(jungle.advanced_start_remove_cost, 40);
    assert!(jungle.flammable && !jungle.impassable);
    assert_eq!(set_names(&jungle.terrain), ["TERRAIN_GRASS", "TERRAIN_MARSH"]);

    let forest = &d.features[ty::<FeatureType>("FEATURE_FOREST")];
    assert_eq!(forest.yield_change, [0, 1, 0]);
    assert_eq!((forest.health_percent, forest.defense_modifier, forest.growth_probability), (50, 25, 8));
    assert_eq!(forest.feature_upgrade, d.ty("FEATURE_FOREST_ANCIENT"));

    let ancient = &d.features[ty::<FeatureType>("FEATURE_FOREST_ANCIENT")];
    assert_eq!((ancient.yield_change, ancient.defense_modifier), ([1, 1, 0], 50));
    assert_eq!(ancient.prereq_state_religion, d.ty("RELIGION_FELLOWSHIP_OF_LEAVES"));
    assert_eq!(ancient.python_on_move, "onMoveAncientForest(pCaster, pPlot)");
    assert_eq!(ancient.feature_upgrade, None);

    let flood_plains = &d.features[ty::<FeatureType>("FEATURE_FLOOD_PLAINS")];
    assert_eq!((flood_plains.yield_change, flood_plains.river_yield_change), ([3, 0, 0], [0, 0, 1]));
    assert_eq!((flood_plains.health_percent, flood_plains.appearance_probability), (-40, 10000));
    assert!(flood_plains.requires_river);

    let oasis = &d.features[ty::<FeatureType>("FEATURE_OASIS")];
    assert_eq!((oasis.yield_change, oasis.appearance_probability), ([3, 0, 2], 500));
    assert!(oasis.adds_fresh_water && oasis.no_city && oasis.no_improvement && oasis.requires_flatlands);
    assert!(oasis.no_coast && oasis.no_river && oasis.no_adjacent);

    let fallout = &d.features[ty::<FeatureType>("FEATURE_FALLOUT")];
    assert_eq!((fallout.yield_change, fallout.health_percent), ([-3, -3, -3], -50));
    assert!(fallout.impassable);

    assert_eq!(d.features[ty::<FeatureType>("FEATURE_FLAMES")].require_resist, Some(DamageType::DAMAGE_FIRE));
    let door = &d.features[ty::<FeatureType>("FEATURE_DOOR_WEST_CLOSED")];
    assert_eq!((door.movement_cost, door.on_unit_change_to), (5, d.ty("FEATURE_DOOR_WEST_OPEN")));
    assert_eq!(d.features.iter().filter(|f| f.on_unit_change_to.is_some()).count(), 2);
    assert_eq!(d.features[ty::<FeatureType>("FEATURE_WALLS")].see_through_change, 9);
}

#[test]
fn bonus_classes() {
    let d = global();
    let unique = |name: &str| d.bonus_classes[ty::<BonusClassType>(name)].unique_range;
    assert_eq!((unique("BONUSCLASS_GENERAL"), unique("BONUSCLASS_GRAIN"), unique("BONUSCLASS_RUSH")), (0, 5, 3));
}

#[test]
fn bonuses() {
    let d = global();
    let corn = &d.bonuses[ty::<BonusType>("BONUS_CORN")];
    assert_eq!(corn.bonus_class_type, d.ty("BONUSCLASS_GRAIN"));
    assert_eq!((corn.tech_reveal, corn.tech_city_trade, corn.tech_obsolete), (None, d.ty("TECH_AGRICULTURE"), None));
    assert_eq!(corn.yield_change, [1, 0, 0]);
    assert_eq!((corn.health, corn.happiness, corn.placement_order, corn.const_appearance), (1, 0, 4, 50));
    assert_eq!((corn.min_area_size, corn.min_latitude, corn.max_latitude, corn.tiles_per), (3, 0, 90, 16));
    let rands = (corn.rand_appearance1, corn.rand_appearance2, corn.rand_appearance3, corn.rand_appearance4);
    assert_eq!(rands, (25, 25, 0, 0));
    assert!(corn.one_area && corn.flatlands && corn.no_river_side && corn.normalize && !corn.hills);
    assert_eq!(set_names(&corn.terrain), ["TERRAIN_GRASS"]);

    let iron = &d.bonuses[ty::<BonusType>("BONUS_IRON")];
    assert_eq!(iron.bonus_class_type, d.ty("BONUSCLASS_RUSH"));
    assert_eq!((iron.tech_reveal, iron.tech_city_trade), (d.ty("TECH_SMELTING"), d.ty("TECH_IRON_WORKING")));
    assert_eq!((iron.yield_change, iron.ai_trade_modifier, iron.const_appearance), ([0, 1, 0], 10, 100));
    assert_eq!((iron.percent_per_player, iron.unique_range, iron.rand_appearance1), (75, 7, 10));
    assert!(iron.hills && iron.flatlands && !iron.one_area);

    let banana = &d.bonuses[ty::<BonusType>("BONUS_BANANA")];
    assert_eq!(set_names(&banana.feature), ["FEATURE_JUNGLE"]);
    assert_eq!(set_names(&banana.feature_terrain), ["TERRAIN_GRASS"]);
    let dye = &d.bonuses[ty::<BonusType>("BONUS_DYE")];
    assert_eq!((dye.happiness, dye.group_range, dye.group_rand), (1, 1, 50));
    assert_eq!(d.bonuses[ty::<BonusType>("BONUS_CRAB")].min_latitude, 40);

    // FfH mana and special bonuses.
    let fire = &d.bonuses[ty::<BonusType>("BONUS_MANA_FIRE")];
    assert_eq!((fire.damage_type, fire.ai_trade_modifier, fire.placement_order), (Some(DamageType::DAMAGE_FIRE), 90, -1));
    assert_eq!(fire.tech_city_trade, d.ty("TECH_KNOWLEDGE_OF_THE_ETHER"));
    let entropy = &d.bonuses[ty::<BonusType>("BONUS_MANA_ENTROPY")];
    assert!(entropy.modifier_per_bonus);
    assert_eq!((entropy.bad_attitude, entropy.heal_change_enemy), (-2, -5));
    assert_eq!(entropy.damage_type, Some(DamageType::DAMAGE_UNHOLY));
    let mana = |name: &str| &d.bonuses[ty::<BonusType>(name)];
    assert_eq!(mana("BONUS_MANA_BODY").heal_change, 5);
    assert_eq!(mana("BONUS_MANA_CHAOS").mutate_chance, 2);
    assert_eq!(mana("BONUS_MANA_EARTH").discover_rand_modifier, 20);
    assert_eq!(mana("BONUS_MANA_LAW").maintenance_modifier, -5);
    assert_eq!(mana("BONUS_MANA_MIND").research_modifier, 3);
    assert_eq!(mana("BONUS_MANA_SPIRIT").great_people_rate_modifier, 5);
    assert_eq!(d.bonuses[ty::<BonusType>("BONUS_NIGHTMARE")].free_promotion, d.ty("PROMOTION_NIGHTMARE"));
    assert_eq!(corn.free_promotion, None);
}

#[test]
fn improvements() {
    let d = global();
    let farm = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_FARM")];
    assert_eq!(farm.prereq_nature_yield, [1, 0, 0]);
    assert_eq!(farm.irrigated_yield_change, [1, 0, 0]);
    assert_eq!(farm.yield_change, [0, 0, 0], "no <YieldChanges> -> zeros");
    assert!(farm.fresh_water_makes_valid && farm.requires_flatlands && farm.requires_irrigation && farm.carries_irrigation);
    assert!(!farm.acts_as_city && !farm.permanent);
    assert_eq!((farm.advanced_start_cost, farm.pillage_gold), (30, 5));
    assert_eq!(
        set_names(&farm.terrain_makes_valid),
        ["TERRAIN_GRASS", "TERRAIN_PLAINS", "TERRAIN_TUNDRA", "TERRAIN_SNOW", "TERRAIN_BROKEN_LANDS", "TERRAIN_FIELDS_OF_PERDITION"]
    );
    assert!(set_names(&farm.feature_makes_valid).is_empty());
    // BonusTypeStructs -> per-bonus CvImprovementBonusInfo.
    assert_eq!(farm.improvement_bonus.len(), d.bonuses.len());
    assert_eq!(
        farm.improvement_bonus[ty::<BonusType>("BONUS_CORN")],
        ImprovementBonusInfo { discover_rand: 0, bonus_makes_valid: true, bonus_trade: true, yield_change: [2, 0, 0] }
    );
    assert_eq!(farm.improvement_bonus[ty::<BonusType>("BONUS_RICE")].yield_change, [1, 0, 0]);
    assert_eq!(farm.improvement_bonus[ty::<BonusType>("BONUS_IRON")], ImprovementBonusInfo::default());
    assert_eq!(farm.improvement_bonus.iter().filter(|b| b.bonus_makes_valid).count(), 3);
    // TechYieldChanges (2D).
    assert_eq!(farm.tech_yield_changes.len(), d.techs.len());
    assert_eq!(farm.tech_yield_changes[ty::<TechType>("TECH_SANITATION")], [1, 0, 0]);
    assert_eq!(farm.tech_yield_changes.iter().filter(|y| **y != [0, 0, 0]).count(), 1);
    // RouteYieldChanges (2D): sized by routes; every FFH2 improvement leaves it empty.
    assert_eq!(farm.route_yield_changes.len(), d.routes.len());
    assert_eq!(farm.route_yield_changes[ty::<RouteType>("ROUTE_ROAD")], [0, 0, 0]);
    assert!(d.improvements.iter().all(|i| i.route_yield_changes.iter().all(|y| *y == [0, 0, 0])));

    let mine = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_MINE")];
    assert_eq!(mine.yield_change, [0, 2, 0]);
    assert_eq!(mine.tech_yield_changes[ty::<TechType>("TECH_BLASTING_POWDER")], [0, 1, 0]);
    assert_eq!(
        mine.improvement_bonus[ty::<BonusType>("BONUS_COPPER")],
        ImprovementBonusInfo { discover_rand: 10000, bonus_makes_valid: true, bonus_trade: true, yield_change: [0, 1, 0] }
    );
    assert_eq!(mine.improvement_bonus[ty::<BonusType>("BONUS_GEMS")].yield_change, [0, -1, 5]);

    let cottage = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_COTTAGE")];
    assert_eq!((cottage.yield_change, cottage.upgrade_time), ([0, 0, 1], 10));
    assert_eq!((cottage.improvement_upgrade, cottage.improvement_pillage), (d.ty("IMPROVEMENT_HAMLET"), None));

    let castle = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_CASTLE")];
    assert_eq!((castle.improvement_pillage, castle.improvement_upgrade), (d.ty("IMPROVEMENT_FORT"), d.ty("IMPROVEMENT_CITADEL")));
    assert_eq!((castle.range, castle.range_defense_modifier, castle.heal_rate_change), (2, 10, 10));
    assert_eq!((castle.defense_modifier, castle.upgrade_time), (25, 40));
    assert!(castle.outside_borders);

    let lumbermill = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_LUMBERMILL")];
    assert_eq!((lumbermill.yield_change, lumbermill.river_side_yield_change), ([0, 1, 0], [0, 0, 1]));
    assert!(lumbermill.requires_feature);
    assert_eq!(set_names(&lumbermill.feature_makes_valid), ["FEATURE_FOREST"]);

    let hut = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_GOODY_HUT")];
    assert!(hut.goody);
    assert_eq!((hut.tiles_per_goody, hut.goody_unique_range), (60, 3));

    // FfH fields.
    let barrow = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_BARROW")];
    assert_eq!((barrow.spawn_unit_type, barrow.appearance_probability), (d.ty("UNIT_SKELETON"), 75));
    assert!(barrow.permanent && barrow.outside_borders);
    let enclave = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_ENCLAVE")];
    assert_eq!(enclave.prereq_civilization, d.ty("CIVILIZATION_KURIOTATES"));
    let guardian = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_GUARDIAN")];
    assert_eq!(guardian.python_at_range, "atRangeGuardian(pCaster, pPlot)");
    assert!(guardian.requires_peak && guardian.unique);
    let mirror = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_MIRROR_OF_HEAVEN")];
    assert_eq!((mirror.visibility_change, mirror.bonus_convert), (6, d.ty("BONUS_MANA_SUN")));
    assert_eq!(mirror.python_on_move, "onMoveMirrorOfHeaven(pCaster, pPlot)");
    let smoke = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_SMOKE")];
    assert_eq!((smoke.feature_upgrade, smoke.defense_modifier), (d.ty("FEATURE_FLAMES"), -10));
    let cove = &d.improvements[ty::<ImprovementType>("IMPROVEMENT_PIRATE_COVE")];
    assert!(cove.acts_as_city && cove.water);
    assert_eq!(set_names(&cove.terrain_makes_valid), ["TERRAIN_COAST"]);
}

#[test]
fn routes() {
    let d = global();
    let road = &d.routes[ty::<RouteType>("ROUTE_ROAD")];
    assert_eq!((road.value, road.advanced_start_cost, road.advanced_start_cost_increase), (1, 12, 0));
    assert_eq!((road.movement_cost, road.flat_movement_cost), (30, 30));
    assert_eq!((road.prereq_bonus, road.yield_change), (None, [0, 0, 0]));
    assert_eq!(road.tech_movement_change.len(), d.techs.len());
    assert_eq!(road.tech_movement_change[ty::<TechType>("TECH_ENGINEERING")], -10);
    assert_eq!(road.tech_movement_change.iter().filter(|c| **c != 0).count(), 1);
    // Sized by GC.getNUM_ROUTE_PREREQ_OR_BONUSES().
    assert_eq!(road.prereq_or_bonus, vec![None; d.defines.num_route_prereq_or_bonuses as usize]);

    let railroad = &d.routes[ty::<RouteType>("ROUTE_RAILROAD")];
    assert_eq!((railroad.value, railroad.advanced_start_cost), (2, -1));
    assert_eq!((railroad.movement_cost, railroad.flat_movement_cost), (20, 6));
    assert!(railroad.tech_movement_change.iter().all(|c| *c == 0));
    assert_eq!(railroad.prereq_or_bonus.len(), d.defines.num_route_prereq_or_bonuses as usize);
}

#[test]
fn unit_combats_and_damage_types() {
    let d = global();
    let combats: Vec<&str> = d.unit_combats.iter().map(|u| u.base.type_.as_str()).collect();
    assert_eq!(
        combats,
        [
            "UNITCOMBAT_RECON", "UNITCOMBAT_ARCHER", "UNITCOMBAT_MOUNTED", "UNITCOMBAT_MELEE", "UNITCOMBAT_SIEGE",
            "UNITCOMBAT_ADEPT", "UNITCOMBAT_DISCIPLE", "UNITCOMBAT_ANIMAL", "UNITCOMBAT_NAVAL", "UNITCOMBAT_BEAST",
        ]
    );
    let melee = &d.unit_combats[ty::<UnitCombatType>("UNITCOMBAT_MELEE")];
    assert_eq!(melee.base.description, "TXT_KEY_UNITCOMBAT_MELEE");
    assert_eq!(melee.base.button, ",Art/Interface/Buttons/Promotions/Shock.dds,Art/Interface/Buttons/Promotions_Atlas.dds,4,5");

    let fire = &d.damages[DamageType::DAMAGE_FIRE];
    assert_eq!(fire.base.type_, "DAMAGE_FIRE");
    assert_eq!(fire.base.description, "TXT_KEY_DAMAGE_FIRE");
    assert_eq!(fire.base.button, "Art/Interface/Buttons/Fire.dds");
    assert_eq!(d.damages[DamageType::DAMAGE_UNHOLY].base.type_, "DAMAGE_UNHOLY");
}

/// The C++ enums in CvEnums.h hard-code these table orders (lib.rs checks some of them
/// at load; this covers every basic table that has one).
#[test]
fn basic_tables_match_cpp_enum_order() {
    fn check<T: InfoId>(d: &GameData, cpp: &[&str]) {
        let names: Vec<&str> = d.reg.names_of(T::CLASS).iter().map(String::as_str).collect();
        assert_eq!(names, cpp, "{} order", T::CLASS);
    }
    let d = global();
    check::<AlignmentType>(d, AlignmentType::NAMES);
    check::<AttitudeType>(d, AttitudeType::NAMES);
    check::<CalendarType>(d, CalendarType::NAMES);
    check::<DamageType>(d, DamageType::NAMES);
    check::<DenialType>(d, DenialType::NAMES);
    check::<DomainType>(d, DomainType::NAMES);
    check::<MemoryType>(d, MemoryType::NAMES);
    check::<UnitAIType>(d, UnitAIType::NAMES);
    check::<YieldType>(d, YieldType::NAMES);

    // Each table's InfoBase agrees with the registry.
    assert_eq!(d.alignments[AlignmentType::ALIGNMENT_EVIL].base.type_, "ALIGNMENT_EVIL");
    assert_eq!(d.alignments[AlignmentType::ALIGNMENT_EVIL].base.description, "TXT_KEY_ALIGNMENT_EVIL");
    assert_eq!(d.domains[DomainType::DOMAIN_LAND].base.type_, "DOMAIN_LAND");
    assert_eq!(d.unit_ais[UnitAIType::UNITAI_ATTACK].base.type_, "UNITAI_ATTACK");
    assert_eq!(d.calendars[CalendarType::CALENDAR_MONTHS].base.type_, "CALENDAR_MONTHS");
    let seasons: Vec<&str> = d.seasons.iter().map(|s| s.base.type_.as_str()).collect();
    assert_eq!(seasons, ["SEASON_WINTER", "SEASON_SPRING", "SEASON_SUMMER", "SEASON_FALL"]);
    let invisibles: Vec<&str> = d.invisibles.iter().map(|s| s.base.type_.as_str()).collect();
    assert_eq!(invisibles, ["INVISIBLE_LAND", "INVISIBLE_ANIMAL", "INVISIBLE_ALL"]);
    assert_eq!(d.months.raw.first().map(|m| m.base.type_.as_str()), Some("MONTH_JANUARY"));
    assert_eq!(d.months.raw.last().map(|m| m.base.type_.as_str()), Some("MONTH_DECEMBER"));
}
