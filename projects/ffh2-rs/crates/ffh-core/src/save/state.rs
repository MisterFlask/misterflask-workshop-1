//! [`Persist`] for every game-state type: ids and info-type indices, the CvStructs.h structs,
//! the game objects (`Unit`, `City`, `Player`, `Team`, `Plot`, `Area`, `PlotGroup`,
//! `SelectionGroup`, `Deal`), `Map`, `GameSetup` and the log. `Game` itself is in `mod.rs`;
//! `PathFinder` in `path_finder.rs`.
//!
//! Each struct is listed field by field in declaration order (`persist_struct!` destructures
//! it exhaustively): adding a field to one of these structs breaks the build here until the
//! field is added to its list (and `FORMAT_VERSION` is bumped).

use super::io::{persist_newtype, persist_struct};
use super::{LoadError, Persist, Reader, Writer};

use crate::area::Area;
use crate::city::City;
use crate::deal::Deal;
use crate::group::SelectionGroup;
use crate::ids::*;
use crate::log::{GameLog, LogEntry};
use crate::map::Map;
use crate::player::Player;
use crate::plot::Plot;
use crate::plot_group::PlotGroup;
use crate::rng::Rng;
use crate::setup::{GameSetup, PlayerSetup, SlotStatus};
use crate::structs::*;
use crate::team::Team;
use crate::unit::Unit;
use ffh_data::*;

// ---- typed indices (ffh-data `define_idx!`, inner u16) and entity ids ----

persist_newtype!(
    ActivityType, AdvisorType, AlignmentType, AreaAIType, ArtStyleType, AttitudeType, AutomateType,
    BonusClassType, BonusType, BuildType, BuildingClassType, BuildingType, CalendarType,
    CardinalDirectionType, CitySizeType, CivicOptionType, CivicType, CivilizationType, ClimateType,
    CommandType, CommerceType, ContactType, ControlType, CorporationType, CultureLevelType,
    DamageType, DenialType, DiploCommentType, DiplomacyPowerType, DirectionType, DomainType,
    EmphasizeType, EraType, EspionageMissionType, EventTriggerType, EventType, FeatType,
    FeatureType, FlavorType, GameOptionType, GameSpeedType, GameStateType, GoodyType, HandicapType,
    HurryType, ImprovementType, InterfaceMessageType, InvisibleType, LeaderHeadType, MemoryType,
    MissionAIType, MissionType, MonthType, OrderType, PlayerOptionType, PlotType, ProbabilityType,
    ProcessType, ProjectType, PromotionType, ReligionType, RouteType, SeaLevelType, SeasonType,
    SpecialBuildingType, SpecialUnitType, SpecialistType, SpellType, TaskType, TechType,
    TerrainType, TradeableItem, TraitType, UnitAIType, UnitArtStyleType, UnitClassType,
    UnitCombatType, UnitType, UpkeepType, VictoryType, VoteSourceType, VoteType, WarPlanType,
    WorldSizeType, YieldType,
);

persist_newtype!(
    PlayerId, TeamId, UnitId, CityId, GroupId, AreaId, PlotGroupId, DealId, PlotIdx, TriggeredId, VoteId,
    PlayerVote,
);

// ---- CvStructs.h ----

persist_struct!(XYCoords { x, y });
persist_struct!(OrderData { order_type, data1, data2, save });
persist_struct!(MissionData { mission_type, data1, data2, flags, push_turn });
persist_struct!(TradeData { item_type, data, offering, hidden });
persist_struct!(EventTriggeredData {
    id, trigger, turn, player, city, plot_x, plot_y, unit, other_player, other_player_city, religion,
    corporation, building, text, global_text,
});
persist_struct!(VoteSelectionSubData { vote, player, city, other_player, text });
persist_struct!(VoteSelectionData { id, vote_source, vote_options });
persist_struct!(VoteTriggeredData { id, vote_source, vote_option });
persist_struct!(PlotExtraYield { x, y, extra_yield });
persist_struct!(PlotExtraCost { x, y, cost });
persist_struct!(BuildingYieldChange { building_class, yield_type, change });
persist_struct!(BuildingCommerceChange { building_class, commerce_type, change });

// ---- RNG, setup, log ----

/// `CvRandom` is its seed (private field; `Rng::new` / `Rng::seed` round-trip it).
impl Persist for Rng {
    fn write(&self, w: &mut Writer) {
        self.seed().write(w);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        Ok(Rng::new(u32::read(r)?))
    }
}

impl Persist for SlotStatus {
    fn write(&self, w: &mut Writer) {
        w.u8(match self {
            SlotStatus::Open => 0,
            SlotStatus::Computer => 1,
            SlotStatus::Closed => 2,
            SlotStatus::Taken => 3,
        });
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        Ok(match r.u8()? {
            0 => SlotStatus::Open,
            1 => SlotStatus::Computer,
            2 => SlotStatus::Closed,
            3 => SlotStatus::Taken,
            b => return Err(LoadError::Corrupt(format!("slot status {b}"))),
        })
    }
}

persist_struct!(PlayerSetup { civ, leader, team, handicap, art_style, slot_status, human, leader_name, civ_description });
persist_struct!(GameSetup {
    world_size, climate, sea_level, era, game_speed, calendar, options, victories, game_turn, max_turns,
    target_score, max_city_elimination, num_advanced_start_points, sync_rand_seed, map_rand_seed,
    map_script, players,
});
persist_struct!(LogEntry { turn, player, text });
persist_struct!(GameLog { enabled, entries, cap });

// ---- game objects ----

persist_struct!(Unit {
    id, group, x, y, last_move_turn, recon_x, recon_y, game_turn_created, damage, moves, experience,
    level, cargo, cargo_capacity, attack_plot_x, attack_plot_y, combat_timer, combat_first_strikes,
    fortify_turns, blitz_count, amphib_count, river_count, enemy_route_count, always_heal_count,
    hills_double_move_count, immune_to_first_strikes_count, extra_visibility_range, extra_moves,
    extra_move_discount, extra_air_range, extra_intercept, extra_evasion, extra_first_strikes,
    extra_chance_first_strikes, extra_withdrawal, extra_collateral_damage, extra_bombard_rate,
    extra_enemy_heal, extra_neutral_heal, extra_friendly_heal, same_tile_heal, adjacent_tile_heal,
    extra_combat_percent, extra_city_attack_percent, extra_city_defense_percent,
    extra_hills_attack_percent, extra_hills_defense_percent, revolt_protection,
    collateral_damage_protection, pillage_change, upgrade_discount, experience_percent,
    kamikaze_percent, base_combat, immobile_timer, made_attack, made_interception, promotion_ready,
    death_delay, blockading, air_combat, flee_withdrawl, has_casted, ignore_hide, terraformer, alive,
    ai_control, boarding, defensive_strike_chance, defensive_strike_damage, double_fortify_bonus,
    fear, flying, held, hidden_nationality, ignore_building_defense, immortal, immune_to_capture,
    immune_to_defensive_strike, immune_to_fear, immune_to_magic, invisible, only_defensive,
    see_invisible, summoner, target_weakest_unit, target_weakest_unit_counter, twincast,
    water_walking, base_combat_defense, better_defender_than_percent, combat_heal_percent,
    combat_limit, combat_percent_in_borders, combat_percent_global_counter, delayed_spell, duration,
    free_promotion_pick, gold_from_combat, invisible_type, race, religion, resist, resist_modify,
    scenario_counter, spell_caster_xp, spell_damage_modify, total_damage_type_combat,
    work_rate_modify, bonus_affinity, bonus_affinity_amount, damage_type_combat, damage_type_resist,
    auto_spell_cast, owner, capturing_player, unit_type, leader_unit_type, combat_unit,
    transport_unit, extra_domain_modifier, name, script_data, has_promotion,
    terrain_double_move_count, feature_double_move_count, extra_terrain_attack_percent,
    extra_terrain_defense_percent, extra_feature_attack_percent, extra_feature_defense_percent,
    extra_unit_combat_modifier, ai_birthmark, ai_type, ai_groupflag, ai_origin_x, ai_origin_y,
    ai_suicide_summon, ai_allowed_perm_defense, ai_allowed_patrol, ai_allowed_explore,
});

persist_struct!(City {
    id, x, y, rally_x, rally_y, game_turn_founded, game_turn_acquired, population, highest_population,
    working_population, specialist_population, num_great_people, base_great_people_rate,
    great_people_rate_modifier, great_people_progress, num_world_wonders, num_team_wonders,
    num_national_wonders, num_buildings, government_center_count, maintenance, maintenance_modifier,
    war_weariness_modifier, hurry_anger_modifier, heal_rate, espionage_health_counter,
    espionage_happiness_counter, fresh_water_good_health, fresh_water_bad_health,
    feature_good_health, feature_bad_health, building_good_health, building_bad_health,
    power_good_health, power_bad_health, bonus_good_health, bonus_bad_health, hurry_anger_timer,
    conscript_anger_timer, defy_resolution_anger_timer, happiness_timer, military_happiness_units,
    building_good_happiness, building_bad_happiness, extra_building_good_happiness,
    extra_building_bad_happiness, extra_building_good_health, extra_building_bad_health,
    feature_good_happiness, feature_bad_happiness, bonus_good_happiness, bonus_bad_happiness,
    religion_good_happiness, religion_bad_happiness, extra_happiness, extra_health,
    no_unhappiness_count, no_unhealthy_population_count, building_only_healthy_count, food,
    food_kept, max_food_kept_percent, overflow_production, feature_production,
    military_production_modifier, space_production_modifier, extra_trade_routes,
    trade_route_modifier, foreign_trade_route_modifier, building_defense, building_bombard_defense,
    free_experience, curr_airlift, max_airlift, air_modifier, air_unit_capacity, nuke_modifier,
    free_specialist, power_count, dirty_power_count, defense_damage, last_defense_damage,
    occupation_timer, culture_update_timer, specialist_free_experience, espionage_defense_modifier,
    never_lost, bombarded, drafted, airlift_targeted, we_love_the_king_day, citizens_automated,
    production_automated, plundered, owner, previous_owner, original_owner, culture_level,
    settlement, civ, crime, mutate_chance, plot_radius, resist_magic, unhappy_production,
    sea_plot_yield, river_plot_yield, base_yield_rate, yield_rate_modifier,
    power_yield_rate_modifier, bonus_yield_rate_modifier, trade_yield, corporation_yield,
    extra_specialist_yield, commerce_rate, production_to_commerce_modifier, building_commerce,
    specialist_commerce, religion_commerce, corporation_commerce, commerce_rate_modifier,
    commerce_happiness_per, domain_free_experience, domain_production_modifier, culture,
    num_revolts, ever_owned, trade_route, revealed, espionage_visibility, name, script_data,
    no_bonus, free_bonus, num_bonuses, num_corp_produced_bonuses, project_production,
    building_production, building_production_time, building_original_owner,
    building_original_time, unit_production, unit_production_time, great_people_unit_rate,
    great_people_unit_progress, specialist_count, max_specialist_count, force_specialist_count,
    free_specialist_count, improvement_free_specialists, religion_influence,
    state_religion_happiness, unit_combat_free_experience, free_promotion_count,
    num_real_building, num_free_building, working_plot, has_religion, has_corporation,
    trade_cities, order_queue, events_occured, building_yield_change, building_commerce_change,
    building_happy_change, building_health_change, population_rank, population_rank_valid,
    base_yield_rank, base_yield_rank_valid, yield_rank, yield_rank_valid, commerce_rank,
    commerce_rank_valid, ai_assign_work_dirty, ai_choose_production_dirty, ai_emphasize,
    ai_emphasize_avoid_growth_count, ai_emphasize_great_people_count, ai_emphasize_yield_count,
    ai_emphasize_commerce_count, ai_emphasize_avoid_angry_citizens_count,
    ai_emphasize_avoid_unhealthy_citizens_count,
});

persist_struct!(Player {
    id, personality_type, current_era, last_state_religion, parent, team, starting_x, starting_y,
    total_population, total_land, total_land_scored, gold, gold_per_turn, advanced_start_points,
    golden_age_turns, num_unit_golden_ages, strike_turns, anarchy_turns, max_anarchy_turns,
    anarchy_modifier, golden_age_modifier, global_hurry_modifier, great_people_created,
    great_generals_created, great_people_threshold_modifier, great_generals_threshold_modifier,
    great_people_rate_modifier, great_general_rate_modifier, domestic_great_general_rate_modifier,
    state_religion_great_people_rate_modifier, max_global_building_production_modifier,
    max_team_building_production_modifier, max_player_building_production_modifier,
    free_experience, feature_production_modifier, worker_speed_modifier,
    improvement_upgrade_rate_modifier, military_production_modifier, space_production_modifier,
    city_defense_modifier, num_nuke_units, num_outside_units, base_free_units,
    base_free_military_units, free_units_population_percent,
    free_military_units_population_percent, gold_per_unit, gold_per_military_unit, extra_unit_cost,
    num_military_units, happy_per_military_unit, military_food_production_count, conscript_count,
    max_conscript, highest_unit_level, overflow_research, no_unhealthy_population_count,
    exp_in_border_modifier, building_only_healthy_count, distance_maintenance_modifier,
    num_cities_maintenance_modifier, corporation_maintenance_modifier, total_maintenance,
    upkeep_modifier, level_experience_modifier, extra_health, building_good_health,
    building_bad_health, extra_happiness, building_happiness, largest_city_happiness,
    war_weariness_percent_anger, war_weariness_modifier, free_specialist, no_foreign_trade_count,
    no_corporations_count, no_foreign_corporations_count, coastal_trade_routes, trade_routes,
    revolution_timer, conversion_timer, state_religion_count, no_non_state_religion_spread_count,
    state_religion_happiness, non_state_religion_happiness,
    state_religion_unit_production_modifier, state_religion_building_production_modifier,
    state_religion_free_experience, capital_city, cities_lost, wins_vs_barbs, assets, power,
    population_score, land_score, tech_score, wonders_score, combat_experience,
    pop_rush_hurry_count, inflation_modifier, alive, ever_alive, turn_active, auto_moves, end_turn,
    extended_game, founded_first_city, strike, human, conquest_mode, favorite_religion,
    tower_vic_flag, sum_sui_mode, adaptive, agnostic, assimilation, declaring_war, disable_human,
    hide_units, ignore_food, insane, see_invisible, sprawling, alignment, disable_production,
    disable_research, disable_spellcasting, discover_rand_modifier, enslavement_chance,
    free_xp_from_combat, heal_change, heal_change_enemy, max_cities, no_diplomacy_with_enemies,
    pillaging_gold, players_killed, real_player, resist_enemy_modify, resist_modify,
    sanctuary_timer, starting_gold, summon_duration, temp_player_timer, upgrade_cost_modifier,
    traits, specialist_type_extra_commerce, sea_plot_yield, yield_rate_modifier,
    capital_yield_rate_modifier, extra_yield_threshold, trade_yield_modifier, free_city_commerce,
    commerce_percent, commerce_rate, commerce_rate_modifier, capital_commerce_rate_modifier,
    state_religion_building_commerce, specialist_extra_commerce, commerce_flexible_count,
    gold_per_turn_by_player, espionage_spending_weight_against_team, feat_accomplished, options,
    script_data, bonus_export, bonus_import, improvement_count, free_building_count,
    extra_building_happiness, extra_building_health, feature_happiness, unit_class_count,
    unit_class_making, building_class_count, building_class_making, hurry_count,
    special_building_not_required_count, has_civic_option_count, no_civic_upkeep_count,
    has_religion_count, has_corporation_count, upkeep_count, specialist_valid_count,
    researching_tech, loyal_member, civics, specialist_extra_yield, improvement_yield_change,
    triggers_fired, group_cycle, research_queue, city_names, plot_groups, cities, units, groups,
    events_triggered, next_triggered_id, events_occured, event_countdown,
    free_unit_combat_promotions, free_unit_class_promotions, votes, unit_extra_costs,
    score_history, economy_history, industry_history, agriculture_history, power_history,
    culture_history, espionage_history, ai_civic_timer, ai_religion_timer, ai_num_train_ai_units,
    ai_num_ai_units, ai_peacetime_trade_value, ai_peacetime_grant_value, ai_gold_traded_to,
    ai_attitude_extra, ai_first_contact, ai_memory_count, ai_bonus_value,
});

persist_struct!(Team {
    id, num_members, alive_count, ever_alive_count, num_cities, total_population, total_land,
    nuke_interception, extra_water_see_from_count, map_trading_count, tech_trading_count,
    gold_trading_count, open_borders_trading_count, defensive_pact_trading_count,
    permanent_alliance_trading_count, vassal_trading_count, bridge_building_count,
    irrigation_count, ignore_irrigation_count, water_work_count, vassal_power, master_power,
    enemy_war_weariness_modifier, river_trade_count, espionage_points_ever, map_centering,
    capitulated, barbarian_ally, no_civic_anger, stolen_visibility_timer, war_weariness,
    tech_share_count, commerce_flexible_count, extra_moves, force_team_vote_eligibility_count,
    at_war, has_met, permanent_war_peace, open_borders, defensive_pact, force_peace, vassal,
    can_launch, route_change, project_count, project_making, unit_class_count,
    building_class_count, obsolete_building_count, research_progress, tech_count,
    terrain_trade_count, victory_countdown, espionage_points_against_team,
    counterespionage_turns_left_against_team, counterespionage_mod_against_team, has_tech,
    no_trade_tech, improvement_yield_change, revealed_bonuses, ai_worst_enemy,
    ai_war_plan_state_counter, ai_at_war_counter, ai_at_peace_counter, ai_has_met_counter,
    ai_open_borders_counter, ai_defensive_pact_counter, ai_share_war_counter, ai_war_success,
    ai_enemy_peacetime_trade_value, ai_enemy_peacetime_grant_value, ai_war_plan,
});

persist_struct!(Plot {
    x, y, area, ownership_duration, improvement_duration, upgrade_progress, force_unowned_timer,
    city_radius_count, river_id, min_original_start_dist, recon_count, river_crossing_count,
    starting_plot, hills, n_of_river, w_of_river, irrigated, potential_city_work, owner, plot_type,
    terrain_type, feature_type, bonus_type, improvement_type, route_type, river_ns_direction,
    river_we_direction, plot_city, working_city, working_city_override, yields, culture,
    found_value, player_city_radius_count, plot_group, visibility_count, stolen_visibility_count,
    blockaded_count, revealed_owner, river_crossing, revealed, revealed_improvement_type,
    revealed_route_type, script_data, build_progress, culture_range_cities,
    invisible_visibility_count, units, move_disabled_ai, move_disabled_human, build_disabled,
    found_disabled, python_active, min_level, plot_counter, portal_exit_x, portal_exit_y,
    temp_terrain_timer, real_terrain_type,
});

persist_struct!(Area {
    id, num_tiles, num_owned_tiles, num_river_edges, num_units, num_cities, total_population,
    num_starting_plots, water, units_per_player, animals_per_player, cities_per_player,
    population_per_player, building_good_health, building_bad_health, building_happiness,
    free_specialist, power, best_found_value, num_revealed_tiles, clean_power_count,
    border_obstacle_count, area_ai_type, target_cities, yield_rate_modifier, num_train_ai_units,
    num_ai_units, num_bonuses, num_improvements,
});

persist_struct!(PlotGroup { id, owner, num_bonuses, plots });

persist_struct!(SelectionGroup {
    id, mission_timer, force_update, owner, activity_type, automate_type, units, mission_queue,
    last_plot_visible, last_plot_revealed, ai_mission_ai_x, ai_mission_ai_y, ai_force_separate,
    ai_mission_ai_type, ai_mission_ai_unit, ai_group_attack, ai_group_attack_x, ai_group_attack_y,
});

persist_struct!(Deal { id, initial_game_turn, first_player, second_player, first_trades, second_trades });

persist_struct!(Map {
    grid_width, grid_height, land_plots, owned_plots, top_latitude, bottom_latitude, next_river_id,
    wrap_x, wrap_y, num_bonus, num_bonus_on_land, plots, areas, pirate_cove_types,
});
