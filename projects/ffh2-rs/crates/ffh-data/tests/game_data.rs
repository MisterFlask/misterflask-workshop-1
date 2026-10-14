//! Data spot-checks for the GameInfo tables ported in `infos/game.rs`.
//! Every expected value was read by hand from `data/xml/Gameinfo/*.xml`.

use ffh_data::*;

fn d() -> &'static GameData {
    ffh_data::global()
}

#[test]
fn loads_without_warnings() {
    let d = d();
    assert!(d.warnings.is_empty(), "{} data warnings:\n{}", d.warnings.len(), d.warnings.join("\n"));
}

#[test]
fn table_sizes() {
    let d = d();
    assert_eq!(d.commerces.len(), CommerceType::NUM);
    assert_eq!(d.game_speeds.len(), 4);
    assert_eq!(d.worlds.len(), WorldSizeType::NUM);
    assert_eq!(d.climates.len(), 5);
    assert_eq!(d.sea_levels.len(), 3);
    assert_eq!(d.eras.len(), 10);
    assert_eq!(d.handicaps.len(), 9);
    assert_eq!(d.game_options.len(), GameOptionType::NUM);
    assert_eq!(d.goodies.len(), 54);
    assert_eq!(d.hurries.len(), 2);
    assert_eq!(d.upkeeps.len(), 3);
    assert_eq!(d.culture_levels.len(), 7);
    assert_eq!(d.victories.len(), 8);
    assert_eq!(d.processes.len(), 3);
    assert_eq!(d.emphasizes.len(), 8);
}

#[test]
fn world_order_matches_cpp_enum() {
    // C++ hard-codes WORLDSIZE_DUEL..WORLDSIZE_HUGE (CvEnums.h WorldSizeTypes).
    let d = d();
    let names: Vec<&str> = d.worlds.iter().map(|w| w.base.type_.as_str()).collect();
    assert_eq!(names, WorldSizeType::NAMES);
}

#[test]
fn commerces() {
    let d = d();
    let gold = &d.commerces[CommerceType::COMMERCE_GOLD];
    assert_eq!((gold.initial_percent, gold.ai_weight_percent, gold.flexible_percent), (0, 100, false));
    let research = &d.commerces[CommerceType::COMMERCE_RESEARCH];
    assert_eq!((research.initial_percent, research.ai_weight_percent, research.flexible_percent), (100, 125, true));
    let culture = &d.commerces[CommerceType::COMMERCE_CULTURE];
    assert_eq!((culture.initial_happiness, culture.ai_weight_percent), (10, 25));
    assert_eq!(d.commerces[CommerceType::COMMERCE_ESPIONAGE].ai_weight_percent, 0);
}

#[test]
fn game_speeds() {
    let d = d();
    let normal = &d.game_speeds[d.t::<GameSpeedType>("GAMESPEED_NORMAL")];
    assert_eq!((normal.train_percent, normal.construct_percent, normal.research_percent), (100, 100, 100));
    assert_eq!((normal.growth_percent, normal.hurry_percent), (100, 100));
    assert_eq!((normal.inflation_percent, normal.inflation_offset), (30, -90));
    assert_eq!(normal.num_turn_increments(), 1);
    assert_eq!(normal.game_turn_info, vec![GameTurnInfo { month_increment: 12, num_game_turns_per_increment: 690 }]);

    let marathon = &d.game_speeds[d.t::<GameSpeedType>("GAMESPEED_MARATHON")];
    assert_eq!((marathon.train_percent, marathon.construct_percent, marathon.research_percent), (200, 300, 300));
    assert_eq!((marathon.anarchy_percent, marathon.barb_percent, marathon.golden_age_percent), (200, 400, 200));
    assert_eq!((marathon.hurry_percent, marathon.inflation_offset, marathon.victory_delay_percent), (33, -270, 300));
    assert_eq!(marathon.game_turn_info[0].num_game_turns_per_increment, 1800);

    let quick = &d.game_speeds[d.t::<GameSpeedType>("GAMESPEED_QUICK")];
    assert_eq!((quick.train_percent, quick.golden_age_percent, quick.hurry_percent), (67, 80, 150));
    assert_eq!(quick.game_turn_info, vec![GameTurnInfo { month_increment: 12, num_game_turns_per_increment: 480 }]);
}

#[test]
fn world_sizes() {
    let d = d();
    let standard = &d.worlds[WorldSizeType::WORLDSIZE_STANDARD];
    assert_eq!((standard.grid_width, standard.grid_height), (21, 13));
    assert_eq!((standard.target_num_cities, standard.default_players), (5, 7));
    assert_eq!((standard.num_free_building_bonuses, standard.research_percent), (5, 130));
    assert_eq!(standard.war_weariness_modifier, -10);
    assert_eq!((standard.max_cities_mod, standard.unique_feature_chance), (0, 0));

    let huge = &d.worlds[WorldSizeType::WORLDSIZE_HUGE];
    assert_eq!((huge.grid_width, huge.grid_height, huge.target_num_cities), (32, 20, 6));
    assert_eq!((huge.max_cities_mod, huge.unique_feature_chance), (2, 10));
    assert_eq!(huge.corporation_maintenance_percent, 50);

    let duel = &d.worlds[WorldSizeType::WORLDSIZE_DUEL];
    assert_eq!((duel.grid_width, duel.grid_height), (10, 6));
    assert_eq!((duel.max_cities_mod, duel.unique_feature_chance), (-1, -15));
    assert_eq!((duel.terrain_grain_change, duel.feature_grain_change), (-1, -1));
    assert_eq!((duel.max_conscript_modifier, duel.num_cities_anarchy_percent), (-50, 11));
}

#[test]
fn climates_and_sea_levels() {
    let d = d();
    let temperate = &d.climates[d.t::<ClimateType>("CLIMATE_TEMPERATE")];
    assert_eq!((temperate.jungle_latitude, temperate.hill_range, temperate.peak_percent), (5, 5, 25));
    assert_eq!((temperate.ice_latitude, temperate.rand_ice_latitude), (0.95, 0.25));
    let rocky = &d.climates[d.t::<ClimateType>("CLIMATE_ROCKY")];
    assert_eq!((rocky.hill_range, rocky.peak_percent), (7, 35));
    assert_eq!((rocky.snow_latitude_change, rocky.tundra_latitude_change), (-0.025, -0.05));
    let arid = &d.climates[d.t::<ClimateType>("CLIMATE_ARID")];
    assert_eq!(arid.desert_percent_change, 20);
    assert_eq!((arid.desert_bottom_latitude_change, arid.desert_top_latitude_change), (-0.1, 0.1));

    let sea: Vec<i32> = d.sea_levels.iter().map(|s| s.sea_level_change).collect();
    assert_eq!(sea, vec![-8, 0, 6]);
}

#[test]
fn eras() {
    let d = d();
    let ancient = &d.eras[d.t::<EraType>("ERA_ANCIENT")];
    assert_eq!((ancient.start_percent, ancient.starting_unit_multiplier), (0, 1));
    assert_eq!((ancient.event_chance_per_turn, ancient.anarchy_percent, ancient.train_percent), (4, 50, 100));
    let classical = &d.eras[d.t::<EraType>("ERA_CLASSICAL")];
    assert_eq!((classical.start_percent, classical.starting_gold, classical.advanced_start_points), (10, 10, 1200));
    assert_eq!((classical.starting_defense_units, classical.starting_worker_units), (1, 1));
    assert!(!classical.no_goodies);
    let medieval = &d.eras[d.t::<EraType>("ERA_MEDIEVAL")];
    assert!(medieval.no_goodies && !medieval.no_animals && !medieval.no_barb_units && !medieval.no_barb_cities);
    assert_eq!((medieval.start_percent, medieval.starting_gold), (20, 25));
    assert_eq!(d.eras[d.t::<EraType>("ERA_RUNE")].advanced_start_points, -1);
}

#[test]
fn handicap_noble() {
    let d = d();
    let h = &d.handicaps[d.t::<HandicapType>("HANDICAP_NOBLE")];
    assert_eq!((h.starting_location_percent, h.animal_attack_prob, h.free_units), (40, 85, 8));
    assert_eq!((h.num_cities_maintenance_percent, h.max_num_cities_maintenance), (75, 5));
    assert_eq!((h.inflation_percent, h.civic_upkeep_percent), (65, 80));
    assert_eq!((h.health_bonus, h.happy_bonus, h.attitude_change), (2, 4, -1));
    assert_eq!((h.unowned_water_tiles_per_barbarian_unit, h.barbarian_city_creation_prob), (1100, 6));
    assert_eq!((h.barbarian_combat_modifier, h.ai_barbarian_combat_modifier), (0, -25));
    assert_eq!(h.ai_declare_war_prob, 100);
    assert_eq!((h.ai_construct_percent, h.ai_world_construct_percent), (85, 85));
    assert_eq!((h.ai_war_weariness_percent, h.ai_advanced_start_percent), (70, 160));
    assert_eq!(h.lair_spawn_rate, 3);
    assert_eq!(h.ai_free_xp, 0, "<iAIFreeXP> is absent: C++ default 0");
    assert_eq!(h.free_techs.len(), d.techs.len());
    assert!(h.free_techs.iter().all(|&b| !b));
    assert!(h.ai_free_techs.iter().all(|&b| !b));

    // Goodies: weighted list in XML order, duplicates kept.
    let g = |n: &str| d.t::<GoodyType>(n);
    assert_eq!(h.num_goodies(), 20);
    assert_eq!(h.goodies[..3], [g("GOODY_HIGH_GOLD"); 3]);
    assert_eq!(h.goodies.iter().filter(|&&x| x == g("GOODY_LOW_GOLD")).count(), 4);
    assert_eq!(h.goodies.last(), Some(&g("GOODY_BARBARIANS_STRONG")));
}

#[test]
fn handicap_settler_and_deity() {
    let d = d();
    let tech = |n: &str| d.t::<TechType>(n);

    let settler = &d.handicaps[d.t::<HandicapType>("HANDICAP_SETTLER")];
    assert_eq!((settler.free_units, settler.barbarian_combat_modifier), (24, -30));
    assert_eq!((settler.ai_unit_supply_percent, settler.lair_spawn_rate), (50, 2));
    for t in ["TECH_EXPLORATION", "TECH_AGRICULTURE", "TECH_CRAFTING"] {
        assert!(settler.free_techs[tech(t)], "{t}");
    }
    assert_eq!(settler.free_techs.iter().filter(|&&b| b).count(), 3);
    assert!(settler.ai_free_techs.iter().all(|&b| !b));

    let deity = &d.handicaps[d.t::<HandicapType>("HANDICAP_DEITY")];
    assert_eq!((deity.ai_free_xp, deity.lair_spawn_rate, deity.ai_train_percent), (8, 6, 75));
    assert_eq!(
        (deity.ai_starting_unit_multiplier, deity.ai_starting_defense_units, deity.ai_starting_worker_units, deity.ai_starting_explore_units),
        (1, 4, 1, 1)
    );
    assert!(deity.ai_free_techs[tech("TECH_ANCIENT_CHANTS")]);
    assert_eq!(deity.ai_free_techs.iter().filter(|&&b| b).count(), 4);
    assert!(deity.free_techs.iter().all(|&b| !b));
    assert_eq!(deity.num_goodies(), 20);
    assert_eq!(deity.goodies.iter().filter(|&&x| x == d.t::<GoodyType>("GOODY_BARBARIANS_STRONG")).count(), 6);
}

#[test]
fn game_options() {
    let d = d();
    let no_espionage = &d.game_options[GameOptionType::GAMEOPTION_NO_ESPIONAGE];
    assert_eq!(no_espionage.base.type_, "GAMEOPTION_NO_ESPIONAGE");
    assert!(no_espionage.default && !no_espionage.visible);
    let advanced_start = &d.game_options[GameOptionType::GAMEOPTION_ADVANCED_START];
    assert!(!advanced_start.default && advanced_start.visible);
    assert_eq!(d.game_options.iter().filter(|o| o.default).count(), 1);
    assert_eq!(d.game_options.iter().filter(|o| o.visible).count(), 52);
}

#[test]
fn goodies() {
    let d = d();
    let g = |n: &str| &d.goodies[d.t::<GoodyType>(n)];
    let class = |n: &str| Some(d.t::<UnitClassType>(n));

    let low = g("GOODY_LOW_GOLD");
    assert_eq!((low.gold, low.gold_rand1, low.gold_rand2), (20, 21, 21));
    assert_eq!((low.unit_class_type, low.barbarian_unit_class), (None, None));
    let high = g("GOODY_HIGH_GOLD");
    assert_eq!((high.gold, high.gold_rand1, high.gold_rand2), (20, 51, 51));
    let map = g("GOODY_MAP");
    assert_eq!((map.map_offset, map.map_range, map.map_prob), (4, 4, 80));
    assert_eq!(g("GOODY_WARRIOR").unit_class_type, class("UNITCLASS_WARRIOR"));
    assert_eq!(g("GOODY_EXPERIENCE").experience, 5);
    let healing = g("GOODY_HEALING");
    assert_eq!((healing.healing, healing.damage_prereq), (100, 60));
    assert!(g("GOODY_TECH").tech && !g("GOODY_TECH").bad);

    let strong = g("GOODY_BARBARIANS_STRONG");
    assert!(strong.bad);
    assert_eq!(strong.unit_class_type, None);
    assert_eq!(strong.barbarian_unit_class, class("UNITCLASS_WARRIOR"));
    assert_eq!((strong.barbarian_unit_prob, strong.min_barbarians), (40, 2));

    assert_eq!(g("GOODY_EXPLORE_LAIR_ITEM_HEALING_SALVE").unit_class_type, class("EQUIPMENTCLASS_HEALING_SALVE"));
    let vault = g("GOODY_EXPLORE_LAIR_TREASURE_VAULT");
    assert_eq!((vault.gold, vault.gold_rand1, vault.gold_rand2), (40, 101, 101));
}

#[test]
fn hurries_and_upkeeps() {
    let d = d();
    let pop = &d.hurries[d.t::<HurryType>("HURRY_POPULATION")];
    assert_eq!((pop.gold_per_production, pop.production_per_population, pop.anger), (0, 30, true));
    let gold = &d.hurries[d.t::<HurryType>("HURRY_GOLD")];
    assert_eq!((gold.gold_per_production, gold.production_per_population, gold.anger), (3, 0, false));

    let upkeeps: Vec<(i32, i32)> = d.upkeeps.iter().map(|u| (u.population_percent, u.city_percent)).collect();
    assert_eq!(upkeeps, vec![(8, 40), (12, 50), (16, 60)]);
}

#[test]
fn culture_level_thresholds() {
    let d = d();
    let speed = |n: &str| d.t::<GameSpeedType>(n);
    let legendary = &d.culture_levels[d.t::<CultureLevelType>("CULTURELEVEL_LEGENDARY")];
    assert_eq!(legendary.city_defense_modifier, 50);
    assert_eq!(legendary.speed_threshold.len(), d.game_speeds.len());
    assert_eq!(legendary.speed_threshold[speed("GAMESPEED_MARATHON")], 150000);
    assert_eq!(legendary.speed_threshold[speed("GAMESPEED_EPIC")], 75000);
    assert_eq!(legendary.speed_threshold[speed("GAMESPEED_NORMAL")], 50000);
    assert_eq!(legendary.speed_threshold[speed("GAMESPEED_QUICK")], 25000);

    let normal = speed("GAMESPEED_NORMAL");
    let thresholds: Vec<i32> = d.culture_levels.iter().map(|c| c.speed_threshold[normal]).collect();
    assert_eq!(thresholds, vec![0, 0, 10, 100, 500, 5000, 50000]);
    let defense: Vec<i32> = d.culture_levels.iter().map(|c| c.city_defense_modifier).collect();
    assert_eq!(defense, vec![0, 0, 10, 20, 30, 40, 50]);
}

#[test]
fn victories() {
    let d = d();
    let v = |n: &str| &d.victories[d.t::<VictoryType>(n)];
    let cultural = v("VICTORY_CULTURAL");
    assert_eq!(cultural.city_culture, Some(d.t::<CultureLevelType>("CULTURELEVEL_LEGENDARY")));
    assert_eq!(cultural.num_culture_cities, 3);
    let domination = v("VICTORY_DOMINATION");
    assert_eq!((domination.population_percent_lead, domination.land_percent, domination.min_land_percent), (25, 78, 51));
    assert_eq!(domination.city_culture, None, "<CityCulture>NONE</CityCulture>");
    let score = v("VICTORY_SCORE");
    assert!(score.target_score && score.permanent && !score.end_score && !score.conquest);
    assert!(v("VICTORY_TIME").end_score);
    assert!(v("VICTORY_CONQUEST").conquest);
    assert_eq!(v("VICTORY_RELIGIOUS").religion_percent, 80);
    assert!(d.victories.iter().all(|x| !x.diplo_vote && x.victory_delay_turns == 0));
}

#[test]
fn processes() {
    let d = d();
    let p = |n: &str| &d.processes[d.t::<ProcessType>(n)];
    assert_eq!(p("PROCESS_WEALTH").tech_prereq, Some(d.t::<TechType>("TECH_MATHEMATICS")));
    assert_eq!(p("PROCESS_WEALTH").production_to_commerce_modifier, [50, 0, 0, 0]);
    assert_eq!(p("PROCESS_RESEARCH").tech_prereq, Some(d.t::<TechType>("TECH_WRITING")));
    assert_eq!(p("PROCESS_RESEARCH").production_to_commerce_modifier, [0, 50, 0, 0]);
    assert_eq!(p("PROCESS_CULTURE").tech_prereq, Some(d.t::<TechType>("TECH_DRAMA")));
    assert_eq!(p("PROCESS_CULTURE").production_to_commerce_modifier[CommerceType::COMMERCE_CULTURE.idx()], 50);
}

#[test]
fn emphasizes() {
    let d = d();
    let e = |n: &str| &d.emphasizes[d.t::<EmphasizeType>(n)];
    assert_eq!(e("EMPHASIZE_FOOD").yield_change, [1, 0, 0]);
    assert_eq!(e("EMPHASIZE_PRODUCTION").yield_change, [0, 1, 0]);
    assert_eq!(e("EMPHASIZE_COMMERCE").yield_change, [0, 0, 1]);
    // The XML lists 3 commerce values; espionage keeps the InitList 0.
    assert_eq!(e("EMPHASIZE_RESEARCH").commerce_change, [0, 1, 0, 0]);
    assert!(e("EMPHASIZE_GREAT_PEOPLE").great_people);
    assert!(e("EMPHASIZE_AVOID_GROWTH").avoid_growth);
    assert!(e("EMPHASIZE_AVOID_ANGRY_CITIZENS").avoid_angry_citizens);
    assert!(e("EMPHASIZE_AVOID_UNHEALTHY_CITIZENS").avoid_unhealthy_citizens);
    let food = e("EMPHASIZE_FOOD");
    assert!(!food.avoid_growth && !food.great_people && !food.avoid_angry_citizens && !food.avoid_unhealthy_citizens);
}
