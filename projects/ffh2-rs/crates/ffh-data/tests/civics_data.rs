//! Data spot-checks for the infos in `infos/civics.rs` (specialists, religions,
//! corporations, civic options, civics, votes, vote sources, espionage missions,
//! projects, techs). Expected values are taken from the vendored XML.

use ffh_data::*;

fn d() -> &'static GameData {
    ffh_data::global()
}

fn tech(name: &str) -> TechType {
    d().t::<TechType>(name)
}
fn unit_class(name: &str) -> UnitClassType {
    d().t::<UnitClassType>(name)
}
fn flavor(name: &str) -> FlavorType {
    d().t::<FlavorType>(name)
}
fn alignment(name: &str) -> AlignmentType {
    d().t::<AlignmentType>(name)
}
fn civic(name: &str) -> &'static CivicInfo {
    &d().civics[d().t::<CivicType>(name)]
}

#[test]
fn loads_without_warnings() {
    let w = &d().warnings;
    assert!(w.is_empty(), "{} load warnings:\n{}", w.len(), w.join("\n"));
}

#[test]
fn table_sizes_and_types() {
    let d = d();
    assert_eq!(d.specialists.len(), 11);
    assert_eq!(d.religions.len(), 7);
    assert_eq!(d.corporations.len(), 0, "FFH2 ships an empty corporation file");
    assert_eq!(d.civic_options.len(), 5);
    assert_eq!(d.civics.len(), 31);
    assert_eq!(d.votes.len(), 21);
    assert_eq!(d.vote_sources.len(), 2);
    assert_eq!(d.espionage_missions.len(), 1);
    assert_eq!(d.projects.len(), 17);
    assert_eq!(d.techs.len(), 86);
    // every entry's <Type> is the registered name at its index
    for (t, info) in d.techs.iter_enumerated() {
        assert_eq!(d.type_name(t), info.base.type_);
    }
    for (c, info) in d.civics.iter_enumerated() {
        assert_eq!(d.type_name(c), info.base.type_);
    }
    // dense arrays are sized by their key table
    let c = &d.civics[CivicType(0)];
    assert_eq!(c.building_happiness_changes.len(), d.building_classes.len());
    assert_eq!(c.feature_happiness_changes.len(), d.features.len());
    assert_eq!(c.hurry.len(), d.hurries.len());
    assert_eq!(c.specialist_valid.len(), d.specialists.len());
    assert_eq!(c.special_building_not_required.len(), d.special_buildings.len());
    assert_eq!(c.improvement_yield_changes.len(), d.improvements.len());
    assert_eq!(d.civic_options[CivicOptionType(0)].trait_no_upkeep.len(), d.traits.len());
    assert_eq!(d.techs[TechType(0)].terrain_trade.len(), d.terrains.len());
    assert_eq!(d.techs[TechType(0)].domain_extra_moves.len(), DomainType::NUM);
    assert_eq!(d.techs[TechType(0)].flavor_value.len(), d.reg.count::<FlavorType>());
    assert_eq!(d.votes[VoteType(0)].force_civic.len(), d.civics.len());
    assert_eq!(d.votes[VoteType(0)].vote_source_type.len(), d.vote_sources.len());
}

#[test]
fn tech_prereq_lists_fit_cpp_arrays() {
    // C++ stores these in fixed arrays of NUM_OR/AND_TECH_PREREQS entries.
    let d = d();
    let (n_or, n_and) = (d.defines.num_or_tech_prereqs as usize, d.defines.num_and_tech_prereqs as usize);
    for t in d.techs.iter() {
        assert!(t.prereq_or_techs.len() <= n_or, "{}", t.base.type_);
        assert!(t.prereq_and_techs.len() <= n_and, "{}", t.base.type_);
    }
}

#[test]
fn tech_sorcery() {
    let t = &d().techs[tech("TECH_SORCERY")];
    assert_eq!(t.research_cost, 2000);
    assert_eq!(t.era, Some(d().t::<EraType>("ERA_CLASSICAL")));
    assert_eq!(
        t.prereq_or_techs,
        vec![tech("TECH_ALTERATION"), tech("TECH_DIVINATION"), tech("TECH_ELEMENTALISM"), tech("TECH_NECROMANCY")]
    );
    assert_eq!(t.prereq_and_techs, vec![tech("TECH_WRITING")]);
    assert_eq!(t.flavor_value[flavor("FLAVOR_SCIENCE")], 80);
    assert_eq!(t.flavor_value[flavor("FLAVOR_CULTURE")], 10);
    assert_eq!(t.flavor_value[flavor("FLAVOR_GROWTH")], 10);
    assert_eq!(t.flavor_value[flavor("FLAVOR_MILITARY")], 0);
    assert_eq!((t.ai_trade_modifier, t.asset_value, t.power_value), (10, 40, 4));
    assert!(t.trade && t.goody_tech && !t.repeat && !t.disable);
    assert_eq!(t.advisor_type, "ADVISOR_SCIENCE");
}

#[test]
fn tech_sailing_and_seafaring() {
    let t = &d().techs[tech("TECH_SAILING")];
    assert_eq!(t.research_cost, 320);
    assert_eq!(t.ai_trade_modifier, -10);
    assert_eq!(t.prereq_or_techs, vec![tech("TECH_FISHING")]);
    assert!(t.prereq_and_techs.is_empty());
    assert!(t.terrain_trade[d().t::<TerrainType>("TERRAIN_COAST")]);
    assert_eq!(t.terrain_trade.iter().filter(|&&b| b).count(), 1);
    assert!(t.water);
    assert_eq!(t.flavor_value[flavor("FLAVOR_GOLD")], 30);
    assert_eq!(t.flavor_value[flavor("FLAVOR_MILITARY")], 30);

    let s = &d().techs[tech("TECH_SEAFARING")];
    assert_eq!(s.domain_extra_moves[DomainType::DOMAIN_SEA], 1);
    assert_eq!(s.domain_extra_moves[DomainType::DOMAIN_LAND], 0);
    assert_eq!(s.research_cost, -1);
    assert_eq!(s.era, Some(d().t::<EraType>("ERA_ANCIENT")));
}

#[test]
fn tech_misc_fields() {
    let drama = &d().techs[tech("TECH_DRAMA")];
    assert_eq!(drama.commerce_flexible, [false, false, true, false]);
    assert_eq!(drama.first_free_unit_class, Some(unit_class("UNITCLASS_ARTIST")));
    assert_eq!(drama.prereq_and_techs, vec![tech("TECH_EDUCATION")]);

    let cos = &d().techs[tech("TECH_CORRUPTION_OF_SPIRIT")];
    assert_eq!(cos.prereq_and_techs, vec![tech("TECH_WAY_OF_THE_WICKED"), tech("TECH_KNOWLEDGE_OF_THE_ETHER")]);
    assert!(cos.prereq_or_techs.is_empty());
    assert_eq!(cos.preferred_alignment, Some(alignment("ALIGNMENT_EVIL")));
    assert_eq!(cos.prereq_religion, None);

    let arete = &d().techs[tech("TECH_ARETE")];
    assert_eq!(arete.prereq_religion, d().ty::<ReligionType>("RELIGION_RUNES_OF_KILMORPH"));
    assert!(arete.prereq_religion.is_some());

    let future = &d().techs[tech("TECH_FUTURE_TECH")];
    assert!(future.repeat);
    assert_eq!((future.health, future.happiness, future.research_cost), (1, 1, 10000));

    let honor = &d().techs[tech("TECH_HONOR")];
    assert!(honor.defensive_pact_trading);
    assert_eq!(honor.preferred_alignment, Some(alignment("ALIGNMENT_GOOD")));
    assert_eq!(honor.ai_weight, 4);

    assert!(d().techs[tech("TECH_FEUDALISM")].vassal_state_trading);
    assert!(d().techs[tech("TECH_NEVER")].disable);
    assert_eq!(d().techs[tech("TECH_TRADE")].trade_routes, 1);
    let construction = &d().techs[tech("TECH_CONSTRUCTION")];
    assert!(construction.bridge_building && construction.irrigation);
}

#[test]
fn civics_government() {
    let despotism = civic("CIVIC_DESPOTISM");
    assert_eq!(despotism.civic_option_type, d().ty::<CivicOptionType>("CIVICOPTION_GOVERNMENT"));
    assert!(despotism.civic_option_type.is_some());
    assert_eq!(despotism.upkeep, d().ty::<UpkeepType>("UPKEEP_LOW"));
    assert!(despotism.upkeep.is_some());
    assert_eq!(despotism.tech_prereq, None);
    assert_eq!(despotism.ai_weight, -1000);
    assert_eq!(despotism.num_cities_maintenance_modifier, 25);
    assert_eq!(despotism.war_weariness_modifier, -50);
    assert_eq!(despotism.we_love_the_king, "TXT_KEY_CIVIC_WE_LOVE_DESPOT");

    let aristocracy = civic("CIVIC_ARISTOCRACY");
    assert_eq!(aristocracy.tech_prereq, Some(tech("TECH_CODE_OF_LAWS")));
    assert_eq!(aristocracy.distance_maintenance_modifier, -40);
    let farm = d().t::<ImprovementType>("IMPROVEMENT_FARM");
    assert_eq!(aristocracy.improvement_yield_changes[farm], [-1, 0, 2]);
    let changed: Vec<ImprovementType> = aristocracy
        .improvement_yield_changes
        .iter_enumerated()
        .filter(|(_, y)| **y != [0, 0, 0])
        .map(|(i, _)| i)
        .collect();
    assert_eq!(changed, vec![farm]);

    let god_king = civic("CIVIC_GOD_KING");
    assert_eq!(god_king.upkeep, d().ty::<UpkeepType>("UPKEEP_HIGH"));
    assert_eq!(god_king.capital_yield_modifier, [0, 50, 0]);
    assert_eq!(god_king.capital_commerce_modifier, [50, 0, 0, 0]);
    assert_eq!(god_king.yield_modifier, [0, 0, 0]);
}

#[test]
fn civics_labor_economy_and_ffh_fields() {
    let slavery = civic("CIVIC_SLAVERY");
    assert_eq!(slavery.civic_option_type, d().ty::<CivicOptionType>("CIVICOPTION_LABOR"));
    assert!(slavery.hurry[d().t::<HurryType>("HURRY_POPULATION")]);
    assert!(!slavery.hurry[d().t::<HurryType>("HURRY_GOLD")]);
    assert_eq!(slavery.improvement_yield_changes[d().t::<ImprovementType>("IMPROVEMENT_QUARRY")], [0, 1, 0]);
    assert_eq!(slavery.enslavement_chance, 25);
    assert_eq!(slavery.block_alignment, Some(alignment("ALIGNMENT_GOOD")));
    assert_eq!(slavery.we_love_the_king, "");

    let caste = civic("CIVIC_CASTE_SYSTEM");
    assert_eq!(caste.specialist_extra_commerce, [0, 1, 2, 0]);
    assert_eq!(caste.worker_speed_modifier, 50);
    assert_eq!(caste.largest_city_happiness, -1);

    let guilds = civic("CIVIC_GUILDS");
    for s in ["SPECIALIST_ARTIST", "SPECIALIST_ENGINEER", "SPECIALIST_MERCHANT", "SPECIALIST_SCIENTIST"] {
        assert!(guilds.specialist_valid[d().t::<SpecialistType>(s)], "{s}");
    }
    assert!(!guilds.specialist_valid[d().t::<SpecialistType>("SPECIALIST_PRIEST")]);

    let guardian = civic("CIVIC_GUARDIAN_OF_NATURE");
    assert_eq!(guardian.building_happiness_changes[d().t::<BuildingClassType>("BUILDINGCLASS_GROVE")], 2);
    assert_eq!(guardian.feature_happiness_changes[d().t::<FeatureType>("FEATURE_FOREST")], 1);
    assert_eq!(guardian.feature_happiness_changes[d().t::<FeatureType>("FEATURE_FOREST_ANCIENT")], 1);
    assert_eq!(guardian.extra_health, 5);
    assert_eq!(guardian.military_production_modifier, -10);
    assert_eq!(guardian.prereq_religion, d().ty::<ReligionType>("RELIGION_FELLOWSHIP_OF_LEAVES"));
    assert!(guardian.prereq_religion.is_some());

    let crusade = civic("CIVIC_CRUSADE");
    assert_eq!(crusade.prereq_civilization, d().ty::<CivilizationType>("CIVILIZATION_BANNOR"));
    assert!(crusade.prereq_civilization.is_some());
    assert_eq!(civic("CIVIC_DESPOTISM").prereq_civilization, None);
}

#[test]
fn civic_options() {
    let d = d();
    let names: Vec<&str> = d.civic_options.iter().map(|o| o.base.type_.as_str()).collect();
    assert_eq!(
        names,
        [
            "CIVICOPTION_GOVERNMENT",
            "CIVICOPTION_CULTURAL_VALUES",
            "CIVICOPTION_LABOR",
            "CIVICOPTION_ECONOMY",
            "CIVICOPTION_MEMBERSHIP"
        ]
    );
    assert!(d.civic_options.iter().all(|o| o.trait_no_upkeep.iter().all(|&b| !b)));
}

#[test]
fn religions() {
    let leaves = &d().religions[d().t::<ReligionType>("RELIGION_FELLOWSHIP_OF_LEAVES")];
    assert_eq!(leaves.tech_prereq, Some(tech("TECH_WAY_OF_THE_FORESTS")));
    assert_eq!(leaves.free_unit_class, Some(unit_class("UNITCLASS_DISCIPLE_FELLOWSHIP_OF_LEAVES")));
    assert_eq!((leaves.num_free_units, leaves.spread_factor), (1, 100));
    assert_eq!(leaves.global_religion_commerce, [1, 0, 0, 0]);
    assert_eq!(leaves.holy_city_commerce, [0, 0, 4, 0]);
    assert_eq!(leaves.state_religion_commerce, [0, 0, 1, 0]);
    assert_eq!(leaves.adjective_key, "TXT_KEY_RELIGION_FELLOWSHIP_OF_LEAVES_ADJECTIVE");
    assert_eq!(leaves.alignment, Some(alignment("ALIGNMENT_NEUTRAL")));
    assert_eq!(leaves.alignment_best, Some(alignment("ALIGNMENT_GOOD")));
    assert_eq!(leaves.alignment_worst, Some(alignment("ALIGNMENT_EVIL")));
    assert_eq!(leaves.religion_hero1, Some(unit_class("UNITCLASS_KITHRA")));
    assert_eq!(leaves.religion_hero2, Some(unit_class("UNITCLASS_YVAIN")));
    assert_eq!(leaves.religion_tech1, Some(tech("TECH_WAY_OF_THE_FORESTS")));
    assert_eq!(leaves.religion_tech2, Some(tech("TECH_HIDDEN_PATHS")));

    let veil = &d().religions[d().t::<ReligionType>("RELIGION_THE_ASHEN_VEIL")];
    assert_eq!(veil.tech_prereq, Some(tech("TECH_CORRUPTION_OF_SPIRIT")));
    assert_eq!((veil.global_counter_found, veil.global_counter_spread, veil.spread_factor), (5, 1, 150));

    let esus = &d().religions[d().t::<ReligionType>("RELIGION_COUNCIL_OF_ESUS")];
    assert!(esus.hidden && esus.sneak_attack && esus.update_sight);
    assert_eq!(esus.free_unit_class, Some(unit_class("UNITCLASS_NIGHTWATCH")));
    // absent <ReligionHero2>/<ReligionTech2> -> NO_UNITCLASS / NO_TECH
    assert_eq!((esus.religion_hero2, esus.religion_tech2), (None, None));
}

#[test]
fn specialists() {
    let sp = |n: &str| &d().specialists[d().t::<SpecialistType>(n)];
    let citizen = sp("SPECIALIST_CITIZEN");
    assert_eq!(citizen.yield_change, [0, 1, 0]);
    assert_eq!(citizen.commerce_change, [0, 0, 0, 0]);
    assert_eq!(citizen.great_people_unit_class, None);

    let priest = sp("SPECIALIST_PRIEST");
    assert_eq!(priest.yield_change, [0, 1, 0]);
    assert_eq!(priest.commerce_change, [1, 0, 0, 0]);
    assert_eq!(priest.great_people_unit_class, Some(unit_class("UNITCLASS_PROPHET")));
    assert_eq!(priest.great_people_rate_change, 3);

    let scientist = sp("SPECIALIST_SCIENTIST");
    assert_eq!(scientist.yield_change, [0, 0, 0]);
    assert_eq!(scientist.commerce_change, [0, 3, 0, 0]);
    assert_eq!(scientist.great_people_unit_class, Some(unit_class("UNITCLASS_SCIENTIST")));

    let great_artist = sp("SPECIALIST_GREAT_ARTIST");
    assert_eq!(great_artist.commerce_change, [3, 0, 12, 0]);
    assert_eq!((great_artist.great_people_rate_change, great_artist.experience), (0, 0));
    assert!(d().specialists.iter().all(|s| s.flavor_value.iter().all(|&f| f == 0)));
}

#[test]
fn projects() {
    let pj = |n: &str| &d().projects[d().t::<ProjectType>(n)];
    let bane = pj("PROJECT_BANE_DIVINE");
    assert_eq!(bane.tech_prereq, Some(tech("TECH_MALEVOLENT_DESIGNS")));
    assert_eq!((bane.max_global_instances, bane.max_team_instances), (1, -1));
    assert_eq!(bane.production_cost, 10000);
    assert_eq!(bane.bonus_production_modifier[d().t::<BonusType>("BONUS_MANA_DEATH")], 100);
    assert_eq!(bane.bonus_production_modifier[d().t::<BonusType>("BONUS_MANA_ENTROPY")], 100);
    assert_eq!(bane.bonus_production_modifier[d().t::<BonusType>("BONUS_MANA_LIFE")], 0);
    assert_eq!((bane.ai_weight, bane.prereq_global_counter), (300, 70));
    assert_eq!((bane.victory_prereq, bane.anyone_project_prereq, bane.prereq_civilization), (None, None, None));

    let ascension = pj("PROJECT_ASCENSION");
    assert_eq!(ascension.anyone_project_prereq, d().ty::<ProjectType>("PROJECT_THE_DRAW"));
    assert!(ascension.anyone_project_prereq.is_some());
    assert_eq!(ascension.prereq_civilization, d().ty::<CivilizationType>("CIVILIZATION_ILLIANS"));
    assert!(ascension.prereq_civilization.is_some());
    assert_eq!((ascension.production_cost, ascension.ai_weight), (1200, 1000));

    let elegy = pj("PROJECT_ELEGY_OF_THE_SHEAIM");
    assert_eq!(elegy.max_global_instances, -1);
    assert_eq!(elegy.modify_global_counter, 5);
    assert_eq!(pj("PROJECT_HALLOWING_OF_THE_ELOHIM").modify_global_counter, -5);

    assert!(d().projects.iter().all(|p| p.projects_needed.len() == d().projects.len()
        && p.victory_threshold.len() == d().victories.len()
        && p.victory_min_threshold.raw == p.victory_threshold.raw));
}

#[test]
fn votes_and_vote_sources() {
    let d = d();
    let over = d.t::<VoteSourceType>("DIPLOVOTE_OVERCOUNCIL");
    let under = d.t::<VoteSourceType>("DIPLOVOTE_UNDERCOUNCIL");
    let vote = |n: &str| &d.votes[d.t::<VoteType>(n)];

    let sg = vote("VOTE_SECRETARY_GENERAL");
    assert!(sg.secretary_general && sg.civ_voting && !sg.city_voting);
    assert_eq!((sg.population_threshold, sg.min_voters), (50, 2));
    assert!(sg.vote_source_type[over] && !sg.vote_source_type[under]);

    let liberty = vote("VOTE_LIBERTY");
    assert!(liberty.force_civic[d.t::<CivicType>("CIVIC_LIBERTY")]);
    assert_eq!(liberty.force_civic.iter().filter(|&&b| b).count(), 1);

    let codes = vote("VOTE_DEVELOP_SECRET_CODES");
    assert!(codes.no_outside_tech_trades);
    assert_eq!((codes.cost, codes.free_units), (25, 1));
    assert_eq!(codes.free_unit_class, Some(unit_class("UNITCLASS_SCIENTIST")));
    assert!(codes.vote_source_type[under] && !codes.vote_source_type[over]);

    let gambling = vote("VOTE_GAMBLING_RING");
    assert!(gambling.gambling_ring);
    assert_eq!((gambling.cost, gambling.crime), (25, 5));
    assert_eq!(vote("VOTE_SMUGGLING_RING").trade_routes, 1);
    assert_eq!(vote("VOTE_NO_DEATH_MANA").no_bonus, d.ty::<BonusType>("BONUS_MANA_DEATH"));
    assert!(vote("VOTE_NO_DEATH_MANA").no_bonus.is_some());
    assert_eq!(vote("VOTE_FUND_DISSIDENTS").py_result, "voteFundDissidents()");
    assert_eq!(sg.py_result, "");

    let oc = &d.vote_sources[over];
    assert_eq!(oc.vote_interval, 10);
    assert_eq!(oc.civic, d.ty::<CivicType>("CIVIC_OVERCOUNCIL"));
    assert!(oc.civic.is_some());
    assert_eq!(oc.free_specialist, None);
    assert_eq!((oc.religion_yield, oc.religion_commerce), ([0; 3], [0; 4]));
    assert_eq!(d.vote_sources[under].vote_interval, 15);
    assert_eq!(d.vote_sources[under].civic, d.ty::<CivicType>("CIVIC_UNDERCOUNCIL"));
}

#[test]
fn espionage_mission() {
    let m = &d().espionage_missions[EspionageMissionType(0)];
    assert_eq!(m.base.type_, "ESPIONAGEMISSION_SEE_DEMOGRAPHICS");
    assert_eq!(m.cost, 30);
    assert!(m.passive && !m.two_phases && !m.targets_city && !m.see_demographics);
    assert_eq!(m.tech_prereq, None);
    assert_eq!((m.buy_tech_cost_factor, m.counterespionage_mod, m.difficulty_mod), (0, 0, 0));
}
