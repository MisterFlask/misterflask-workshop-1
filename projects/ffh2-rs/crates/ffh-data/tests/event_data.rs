//! Spot-checks of EventInfo / EventTriggerInfo against values read by hand from
//! data/xml/Events/CIV4EventInfos.xml and CIV4EventTriggerInfos.xml.

use ffh_data::*;

fn data() -> &'static GameData {
    ffh_data::global()
}

fn event(name: &str) -> &'static EventInfo {
    let d = data();
    &d.events[d.t::<EventType>(name)]
}

fn trigger(name: &str) -> &'static EventTriggerInfo {
    let d = data();
    &d.event_triggers[d.t::<EventTriggerType>(name)]
}

fn ev(name: &str) -> EventType {
    data().t::<EventType>(name)
}

fn bc(name: &str) -> BuildingClassType {
    data().t::<BuildingClassType>(name)
}

#[test]
fn loads_without_warnings() {
    let d = data();
    assert!(d.warnings.is_empty(), "{} data warnings:\n{}", d.warnings.len(), d.warnings.join("\n"));
    assert_eq!(d.events.len(), 522);
    assert_eq!(d.event_triggers.len(), 277);
    assert_eq!(d.events[ev("EVENT_CONTINUE")].base.type_, "EVENT_CONTINUE");
}

#[test]
fn trigger_unit_trigger_prereqs() {
    let d = data();
    let t = trigger("EVENTTRIGGER_AMURITE_TRIAL");
    assert_eq!(t.base.type_, "EVENTTRIGGER_AMURITE_TRIAL");
    assert_eq!(t.percent_games_active, 20);
    assert_eq!(t.probability, 200); // <iWeight>
    let classes: Vec<UnitClassType> =
        ["UNITCLASS_ADEPT", "UNITCLASS_MAGE", "UNITCLASS_ARCHMAGE"].iter().map(|n| d.t(n)).collect();
    assert_eq!(t.units_required, classes);
    assert_eq!(t.num_units, 1);
    assert_eq!(t.unit_experience_weight, 1);
    assert_eq!(t.prereq_or_techs, vec![d.t::<TechType>("TECH_KNOWLEDGE_OF_THE_ETHER")]);
    assert!(t.prereq_and_techs.is_empty() && t.obsolete_techs.is_empty());
    assert_eq!(t.events, vec![ev("EVENT_AMURITE_TRIAL_1"), ev("EVENT_AMURITE_TRIAL_2")]);
    assert!(t.recurring && t.show_plot && !t.global && !t.pick_city);
    assert_eq!(t.python_can_do_unit, "canTriggerAmuriteTrialUnit");
    assert_eq!(t.python_can_do, "");
    assert_eq!(t.prereq_civilization_pleased, Some(d.t::<CivilizationType>("CIVILIZATION_AMURITES")));
    assert_eq!(t.max_our_landmass, -1);
    assert_eq!(t.plot_type, None); // <iPlotType>-1
    assert_eq!(t.texts, vec!["TXT_KEY_EVENT_AMURITE_TRIAL"]);
    assert_eq!(t.text_eras, vec![None]);
    assert_eq!(t.world_news, vec!["TXT_KEY_EVENTTRIGGER_AMURITE_TRIAL"]);
}

#[test]
fn trigger_plot_requirements_keep_none_entries() {
    let d = data();
    // Hills desert plot that must have no improvement, no bonus and no route.
    let t = trigger("EVENTTRIGGER_ANCIENT_TEXTS");
    assert_eq!(t.plot_type, Some(PlotType::PLOT_HILLS));
    assert_eq!(t.terrains_required, vec![d.t::<TerrainType>("TERRAIN_DESERT")]);
    assert_eq!(t.improvements_required, vec![None]);
    assert_eq!(t.bonuses_required, vec![None]);
    assert_eq!(t.routes_required, vec![None]);
    assert!(t.features_required.is_empty());
    assert!(t.own_plot);
    assert_eq!(t.num_plots_required, 1);

    let t = trigger("EVENTTRIGGER_FARM_BANDITS");
    assert_eq!(t.improvements_required, vec![Some(d.t::<ImprovementType>("IMPROVEMENT_FARM"))]);
    let bonuses: Vec<Option<BonusType>> =
        ["BONUS_WHEAT", "BONUS_RICE", "BONUS_CORN"].iter().map(|n| Some(d.t(n))).collect();
    assert_eq!(t.bonuses_required, bonuses);
    assert_eq!(t.routes_required, vec![Some(d.t::<RouteType>("ROUTE_ROAD"))]);
    assert_eq!(t.prereq_crime, 10);
    assert_eq!(t.unit_distance_weight, -1);
    assert_eq!(t.events.len(), 4);
    assert_eq!(t.texts.len(), 3);

    // NONE entries in the whole file (counted in the XML): 3 features, 10 improvements,
    // 6 bonuses, 4 routes.
    let nones = |f: fn(&EventTriggerInfo) -> usize| d.event_triggers.iter().map(f).sum::<usize>();
    assert_eq!(nones(|t| t.features_required.iter().filter(|x| x.is_none()).count()), 3);
    assert_eq!(nones(|t| t.improvements_required.iter().filter(|x| x.is_none()).count()), 10);
    assert_eq!(nones(|t| t.bonuses_required.iter().filter(|x| x.is_none()).count()), 6);
    assert_eq!(nones(|t| t.routes_required.iter().filter(|x| x.is_none()).count()), 4);
}

#[test]
fn trigger_python_callbacks_and_prereq_events() {
    let t = trigger("EVENTTRIGGER_ARMAGEDDON_APOCALYPSE");
    assert_eq!(t.python_callback, "doArmageddonApocalypse");
    assert_eq!(t.prereq_global_counter, 100);
    assert_eq!(t.probability, -1);
    assert!(t.global && !t.show_plot);
    assert_eq!(t.events, vec![ev("EVENT_CONTINUE")]);
    // The empty <Text></Text> world-news entry is kept: C++ counts it for the RNG pick.
    assert_eq!(t.world_news, vec![String::new()]);

    let t = trigger("EVENTTRIGGER_CITY_FEUD_ARSON");
    assert_eq!(t.prereq_events, vec![ev("EVENT_CITY_FEUD_START_2")]);
    assert!(t.prereq_event_city); // <bPrereqEventPlot>
    assert!(t.pick_city && t.own_plot);
    assert_eq!(t.min_population, 5);
    assert_eq!(t.python_can_do_city, "canTriggerCityFeud");
    assert_eq!(t.python_callback, "doCityFeudArson");
    assert_eq!(t.events.len(), 3);

    let t = trigger("EVENTTRIGGER_PICK_ALIGNMENT");
    assert_eq!(t.python_can_do, "canTriggerPickAlignment");
    assert_eq!(t.prereq_leader, Some(data().t::<LeaderHeadType>("LEADER_DECIUS")));
}

#[test]
fn trigger_civic_trait_obsolete() {
    let d = data();
    let t = trigger("EVENTTRIGGER_HERESY");
    assert_eq!(t.civic, Some(d.t::<CivicType>("CIVIC_THEOCRACY")));
    assert!(t.pick_religion && t.state_religion && t.global && t.pick_city);
    assert_eq!(t.events.len(), 3);

    let t = trigger("EVENTTRIGGER_SICKNESS");
    assert_eq!(t.obsolete_techs, vec![d.t::<TechType>("TECH_MEDICINE")]);
    assert_eq!(t.probability, 400);
    assert_eq!(t.min_population, 3);

    let t = trigger("EVENTTRIGGER_TRAIT_INSANE");
    assert_eq!(t.prereq_trait, Some(d.t::<TraitType>("TRAIT_INSANE")));
    assert_eq!(t.probability, 0);
    assert!(t.recurring);
    assert_eq!(t.min_difficulty, None);
    assert_eq!(t.other_player_has_tech, None);
}

#[test]
fn trigger_list_totals() {
    let d = data();
    // Counted in the XML: 606 <Event> choices and 294 <TriggerText> entries in all; exactly one
    // world-news entry per trigger, 146 of them empty.
    assert_eq!(d.event_triggers.iter().map(|t| t.events.len()).sum::<usize>(), 606);
    assert_eq!(d.event_triggers.iter().map(|t| t.texts.len()).sum::<usize>(), 294);
    assert!(d.event_triggers.iter().all(|t| t.text_eras.len() == t.texts.len()));
    assert!(d.event_triggers.iter().all(|t| t.world_news.len() == 1));
    assert_eq!(d.event_triggers.iter().filter(|t| t.world_news[0].is_empty()).count(), 146);
}

#[test]
fn event_gold_units_and_promotions() {
    let d = data();
    let e = event("EVENT_CRAB_CAKES_1");
    assert_eq!((e.gold, e.random_gold), (30, 40));
    assert_eq!(e.base.description, "TXT_KEY_EVENT_SALES_TAXES");
    assert_eq!(e.ai_value, 1000);
    assert_eq!(e.max_num_religions, -1);

    let e = event("EVENT_CARNIVAL_STAR_4");
    assert_eq!(e.unit_class, Some(d.t::<UnitClassType>("UNITCLASS_DISCIPLE_FELLOWSHIP_OF_LEAVES")));
    assert_eq!(e.num_units, 1); // <iNumFreeUnits>
    assert_eq!(e.unit_promotion, Some(d.t::<PromotionType>("PROMOTION_MOBILITY1")));
    assert_eq!(e.prereq_state_religion, Some(d.t::<ReligionType>("RELIGION_FELLOWSHIP_OF_LEAVES")));
    assert!(e.city_effect); // <bPickCity>

    let e = event("EVENT_AT_THE_SWORD_1");
    assert_eq!(e.unit_experience, 3);
    assert_eq!(e.unit_name_key, "TXT_KEY_EVENT_UNIT_AT_THE_SWORD");

    let e = event("EVENT_AMURITE_TRIAL_1");
    assert_eq!((e.unit_experience, e.unit_immobile_turns), (4, 2));
    assert_eq!(e.python_callback, "applyAmuriteTrial1");

    let e = event("EVENT_BOWYER_1");
    let archer = d.t::<UnitClassType>("UNITCLASS_ARCHER");
    assert_eq!(e.unit_class_promotion[archer], Some(d.t::<PromotionType>("PROMOTION_COMBAT1")));
    assert_eq!(e.unit_class_promotion.iter().filter(|p| p.is_some()).count(), 1);
    assert!(e.unit_combat_promotion.iter().all(|p| p.is_none()));

    let e = event("EVENT_OVERCOUNCIL_GIFT_3");
    let disciple = d.t::<UnitCombatType>("UNITCOMBAT_DISCIPLE");
    assert_eq!(e.unit_combat_promotion[disciple], Some(d.t::<PromotionType>("PROMOTION_VALOR")));

    let e = event("EVENT_BROTHERS_IN_NEED_2");
    assert_eq!(e.bonus_gift, Some(d.t::<BonusType>("BONUS_COPPER")));
}

#[test]
fn event_follow_up_events() {
    let e = event("EVENT_HEALING_PLANT_2");
    assert_eq!(e.additional_event_chance[ev("EVENT_HEALING_PLANT_4")], 20);
    assert_eq!(e.additional_event_chance[ev("EVENT_HEALING_PLANT_5")], 35);
    assert_eq!(e.additional_event_chance.iter().sum::<i32>(), 55);
    assert_eq!(e.hurry_anger, 1);

    let e = event("EVENT_DUSTBOWL_2");
    assert_eq!(e.clear_event_chance[ev("EVENT_DUSTBOWL_2")], 25);
    assert_eq!(e.improvement_change, -1);
    assert_eq!(e.python_help, "getHelpDustBowl2");

    let e = event("EVENT_LOOTERS_3");
    assert_eq!(e.additional_event_chance[ev("EVENT_LOOTERS_4")], 55);
    assert_eq!(e.espionage_points, -50);
    assert_eq!((e.min_pillage, e.max_pillage), (3, 5));
    assert!(e.other_player_city_effect); // <bPickOtherPlayerCity>
    assert_eq!(e.python_callback, "applyLooters3");
    assert_eq!(e.python_can_do, "canApplyLooters3");
    assert_eq!(e.python_help, "getHelpLooters3");

    let e = event("EVENT_CITY_RUINS_2");
    assert_eq!(e.additional_event_chance[ev("EVENT_CITY_RUINS_3")], 50);
    assert_eq!(e.tech_flavor_value.raw, vec![1; 8]);
    assert_eq!((e.tech_percent, e.tech_cost_percent, e.tech_min_turns_left), (15, 5, 4));

    // Whole-file totals (counted in the XML).
    let d = data();
    let nonzero = |f: fn(&EventInfo) -> usize| d.events.iter().map(f).sum::<usize>();
    assert_eq!(nonzero(|e| e.additional_event_chance.iter().filter(|&&x| x != 0).count()), 7);
    assert_eq!(nonzero(|e| e.clear_event_chance.iter().filter(|&&x| x != 0).count()), 6);
    assert_eq!(nonzero(|e| e.additional_event_time.iter().filter(|&&x| x != 0).count()), 0);
    assert_eq!(nonzero(|e| e.free_specialist_count.iter().filter(|&&x| x != 0).count()), 10);
    assert_eq!(nonzero(|e| e.tech_flavor_value.iter().filter(|&&x| x != 0).count()), 37);
    assert_eq!(nonzero(|e| e.plot_extra_yield.iter().filter(|&&x| x != 0).count()), 24);
    assert_eq!(nonzero(|e| e.unit_combat_promotion.iter().filter(|x| x.is_some()).count()), 2);
    assert_eq!(nonzero(|e| e.unit_class_promotion.iter().filter(|x| x.is_some()).count()), 3);
}

#[test]
fn event_building_changes() {
    let d = data();
    let e = event("EVENT_ANGEL_OF_MERCY_2");
    let infirmary = bc("BUILDINGCLASS_INFIRMARY");
    assert_eq!(e.gold, -100);
    assert_eq!(e.building_happy_changes, vec![(infirmary, 1)]);
    assert_eq!(e.building_health_changes, vec![(infirmary, 1)]);
    assert_eq!(e.building_happy_change(infirmary), 1);
    assert_eq!(e.building_health_change(bc("BUILDINGCLASS_MONUMENT")), 0);

    let e = event("EVENT_DEDICATE_MONUMENT_1");
    let monument = bc("BUILDINGCLASS_MONUMENT");
    assert_eq!(
        e.building_yield_changes,
        vec![BuildingYieldChange { building_class: monument, yield_type: YieldType::YIELD_FOOD, change: 2 }]
    );
    assert_eq!(e.building_yield_change(monument, YieldType::YIELD_FOOD), 2);
    assert_eq!(e.building_yield_change(monument, YieldType::YIELD_PRODUCTION), 0);

    let e = event("EVENT_ELDER_CONFLICT_2");
    let council = bc("BUILDINGCLASS_ELDER_COUNCIL");
    assert_eq!(
        e.building_commerce_changes,
        vec![
            BuildingCommerceChange { building_class: council, commerce_type: CommerceType::COMMERCE_RESEARCH, change: -1 },
            BuildingCommerceChange { building_class: council, commerce_type: CommerceType::COMMERCE_CULTURE, change: 3 },
        ]
    );
    assert_eq!(e.building_commerce_change(council, CommerceType::COMMERCE_CULTURE), 3);
    assert_eq!(e.building_commerce_change(council, CommerceType::COMMERCE_GOLD), 0);

    let e = event("EVENT_CONTROVERSIAL_PHILOSOPHER_3");
    assert_eq!(e.building_class, Some(bc("BUILDINGCLASS_ACADEMY")));
    assert_eq!((e.building_change, e.happy, e.gold), (1, -2, -100));

    let e = event("EVENT_CONTROVERSIAL_PHILOSOPHER_2");
    assert_eq!(e.free_specialist_count[d.t::<SpecialistType>("SPECIALIST_SCIENTIST")], 1);
    assert_eq!(e.happy, -1);

    // Whole-file totals (counted in the XML).
    let total = |f: fn(&EventInfo) -> usize| d.events.iter().map(f).sum::<usize>();
    assert_eq!(total(|e| e.building_yield_changes.len()), 5);
    assert_eq!(total(|e| e.building_commerce_changes.len()), 26);
    assert_eq!(total(|e| e.building_happy_changes.len()), 3);
    assert_eq!(total(|e| e.building_health_changes.len()), 2);
}

#[test]
fn event_misc_effects() {
    let d = data();
    let e = event("EVENT_ANGELIC_PROCESSION_2");
    assert_eq!(e.plot_extra_yield, [0, 0, 1]);
    assert_eq!(e.global_counter, -2);
    assert_eq!(e.prereq_civilization, Some(d.t::<CivilizationType>("CIVILIZATION_ELOHIM")));

    let e = event("EVENT_INSPIRED_MISSION_1");
    assert_eq!((e.max_num_religions, e.convert_own_cities), (1, 4));
    assert_eq!(d.events.iter().filter(|e| e.max_num_religions != -1).count(), 3);

    let e = event("EVENT_FAMINE_3");
    assert_eq!(e.world_news, vec!["TXT_KEY_NEWS_EVENT_FAMINE_3"]);
    assert!(e.gold_to_player && e.city_effect && e.other_player_city_effect);
    assert_eq!((e.gold, e.food_percent), (-10, -100));
    assert_eq!((e.our_attitude_modifier, e.attitude_modifier), (3, 3));
    assert_eq!(d.events.iter().filter(|e| !e.world_news.is_empty()).count(), 4);

    let e = event("EVENT_ANCIENTOLYMPICS_2");
    assert!(e.quest);
    assert_eq!(e.gold, -20);
    assert_eq!(e.python_callback, "doAncientOlympics2");
    assert_eq!(e.python_help, "getHelpModernOlympics");

    let e = event("EVENT_CYCLONE_1");
    assert_eq!(e.python_can_do, "canApplyHurricane1");
    assert_eq!(e.python_callback, "applyHurricane1");
    assert!(d.events.iter().all(|e| e.prereq_corporation.is_none()));
}
