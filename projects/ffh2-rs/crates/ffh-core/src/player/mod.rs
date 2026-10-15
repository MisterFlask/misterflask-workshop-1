//! Port of `CvPlayer` (CvPlayer.h / CvPlayer.cpp), plus the player-level `CvPlayerAI`
//! bookkeeping that rules code reads and writes.
//!
//! `Player` holds one field per gameplay data member of `CvPlayer` (UI-only members such
//! as message/popup/diplomacy queues, PBEM flags and the start time are omitted). Field
//! names follow the C++ member names (`m_iFooBar` -> `foo_bar`); where the C++ getter is
//! named differently the field's doc comment names it. Trivial getters/setters are field
//! accesses (`self.players[p].gold += 10` for `changeGold(10)`); everything with logic is
//! an `impl Game` method `player_*` in one of the topic files below. `docs/porting/player.md`
//! maps every C++ function to its Rust file (or to a field / to "skipped").
//!
//! Topic files (each an `impl Game` block):
//! - `turn.rs`             turn flow, alive/turn-active state, identity, score, split empire, AI hooks
//! - `start.rs`            starting plots, free starting units, advanced start
//! - `cities.rs`           found/acquire/raze/disband cities, city names, capital, plot groups, goody huts
//! - `units.rs`            unit/building/project lifecycle and production checks, class counts,
//!                         building effects, golden-age and great-people units
//! - `economy.rs`          gold, commerce, research, upkeep, maintenance, yields, happiness/health
//! - `civics_religion.rs`  civics, anarchy, religion, corporations, votes, traits
//! - `diplomacy_trade.rs`  contact, trade items, deals, bonuses, espionage
//! - `events.rs`           random events and quests

#![allow(unused_imports)]

pub mod civics_religion;
pub mod cities;
pub mod diplomacy_trade;
pub mod economy;
pub mod events;
pub mod start;
pub mod turn;
pub mod units;

pub use diplomacy_trade::DiploEvent;
pub use start::AdvancedStartAction;

use std::collections::BTreeMap;

use crate::defs::INVALID_PLOT_COORD;
use crate::game::Game;
use crate::ids::*;
use crate::setup::{PlayerSetup, SlotStatus};
use crate::structs::*;
use ffh_data::*;

/// C++ `CvPlayer` (+ the `CvPlayerAI` members that rules code touches, prefixed `ai_`).
#[derive(Clone, Debug, Default)]
pub struct Player {
    // ---------------------------------------------------------------- identity
    /// C++ `m_eID` (`getID()`).
    pub id: PlayerId,
    /// C++ `m_ePersonalityType`: leader whose AI personality this player uses.
    pub personality_type: Option<LeaderHeadType>,
    /// C++ `m_eCurrentEra`.
    pub current_era: EraType,
    /// C++ `m_eLastStateReligion`. The state religion is this only while
    /// `state_religion_count > 0` (see `player_get_state_religion`).
    pub last_state_religion: Option<ReligionType>,
    /// C++ `m_eParent`: player this one was split from (`splitEmpire`).
    pub parent: Option<PlayerId>,
    /// C++ `m_eTeamType` (`getTeam()`); mirrors `GameSetup::players[id].team`.
    pub team: TeamId,

    // ---------------------------------------------------------------- int members
    /// C++ `m_iStartingX` (`INVALID_PLOT_COORD` when none; see `player_get_starting_plot`).
    pub starting_x: i32,
    /// C++ `m_iStartingY`.
    pub starting_y: i32,
    pub total_population: i32,
    pub total_land: i32,
    pub total_land_scored: i32,
    /// C++ `m_iGold` (`getGold`/`setGold`/`changeGold`: only UI side effects in C++).
    pub gold: i32,
    /// C++ `m_iGoldPerTurn`: net gold per turn from deals (sum of `gold_per_turn_by_player`).
    pub gold_per_turn: i32,
    /// C++ `m_iAdvancedStartPoints` (-1 = not in advanced start).
    pub advanced_start_points: i32,
    pub golden_age_turns: i32,
    pub num_unit_golden_ages: i32,
    pub strike_turns: i32,
    pub anarchy_turns: i32,
    pub max_anarchy_turns: i32,
    pub anarchy_modifier: i32,
    pub golden_age_modifier: i32,
    /// C++ `m_iGlobalHurryModifier` (`getHurryModifier`/`changeHurryModifier`).
    pub global_hurry_modifier: i32,
    pub great_people_created: i32,
    pub great_generals_created: i32,
    pub great_people_threshold_modifier: i32,
    pub great_generals_threshold_modifier: i32,
    pub great_people_rate_modifier: i32,
    pub great_general_rate_modifier: i32,
    /// C++ `m_iDomesticGreatGeneralRateModifier` (raw member; the C++ getter adds a
    /// define, use `player_get_domestic_great_general_rate_modifier`).
    pub domestic_great_general_rate_modifier: i32,
    pub state_religion_great_people_rate_modifier: i32,
    pub max_global_building_production_modifier: i32,
    pub max_team_building_production_modifier: i32,
    pub max_player_building_production_modifier: i32,
    pub free_experience: i32,
    pub feature_production_modifier: i32,
    pub worker_speed_modifier: i32,
    pub improvement_upgrade_rate_modifier: i32,
    pub military_production_modifier: i32,
    pub space_production_modifier: i32,
    pub city_defense_modifier: i32,
    pub num_nuke_units: i32,
    pub num_outside_units: i32,
    pub base_free_units: i32,
    pub base_free_military_units: i32,
    pub free_units_population_percent: i32,
    pub free_military_units_population_percent: i32,
    pub gold_per_unit: i32,
    pub gold_per_military_unit: i32,
    pub extra_unit_cost: i32,
    pub num_military_units: i32,
    pub happy_per_military_unit: i32,
    pub military_food_production_count: i32,
    pub conscript_count: i32,
    pub max_conscript: i32,
    pub highest_unit_level: i32,
    pub overflow_research: i32,
    pub no_unhealthy_population_count: i32,
    pub exp_in_border_modifier: i32,
    pub building_only_healthy_count: i32,
    pub distance_maintenance_modifier: i32,
    pub num_cities_maintenance_modifier: i32,
    pub corporation_maintenance_modifier: i32,
    /// C++ `m_iTotalMaintenance`, stored x100 (`getTotalMaintenance()` divides by 100,
    /// see `player_get_total_maintenance`; `changeTotalMaintenance` adds raw x100 values).
    pub total_maintenance: i32,
    pub upkeep_modifier: i32,
    pub level_experience_modifier: i32,
    pub extra_health: i32,
    pub building_good_health: i32,
    pub building_bad_health: i32,
    pub extra_happiness: i32,
    pub building_happiness: i32,
    pub largest_city_happiness: i32,
    /// C++ `m_iWarWearinessPercentAnger` (raw; getter honours GAMEOPTION_NO_WAR_WEARINESS,
    /// see `player_get_war_weariness_percent_anger`).
    pub war_weariness_percent_anger: i32,
    pub war_weariness_modifier: i32,
    pub free_specialist: i32,
    pub no_foreign_trade_count: i32,
    pub no_corporations_count: i32,
    pub no_foreign_corporations_count: i32,
    pub coastal_trade_routes: i32,
    pub trade_routes: i32,
    pub revolution_timer: i32,
    pub conversion_timer: i32,
    pub state_religion_count: i32,
    pub no_non_state_religion_spread_count: i32,
    pub state_religion_happiness: i32,
    pub non_state_religion_happiness: i32,
    pub state_religion_unit_production_modifier: i32,
    pub state_religion_building_production_modifier: i32,
    pub state_religion_free_experience: i32,
    /// C++ `m_iCapitalCityID`. May name a city that no longer exists: read it through
    /// `player_get_capital_city` (C++ `getCapitalCity()`), change it through `player_set_capital_city`.
    pub capital_city: Option<CityId>,
    pub cities_lost: i32,
    pub wins_vs_barbs: i32,
    pub assets: i32,
    pub power: i32,
    /// C++ `m_iPopulationScore` (raw; `getPopScore(bCheckVassal)` is `player_get_pop_score`).
    pub population_score: i32,
    /// C++ `m_iLandScore` (raw; see `player_get_land_score`).
    pub land_score: i32,
    pub tech_score: i32,
    pub wonders_score: i32,
    pub combat_experience: i32,
    /// C++ `m_iPopRushHurryCount` (`canPopRush()`).
    pub pop_rush_hurry_count: i32,
    /// C++ `m_iInflationModifier` (changed by events only).
    pub inflation_modifier: i32,

    // ---------------------------------------------------------------- bool members
    pub alive: bool,
    pub ever_alive: bool,
    pub turn_active: bool,
    pub auto_moves: bool,
    pub end_turn: bool,
    pub extended_game: bool,
    pub founded_first_city: bool,
    pub strike: bool,
    /// C++ `m_bHuman` (`isHuman()`), recomputed by `player_update_human`.
    pub human: bool,

    // ---------------------------------------------------------------- BETTER AI (Sephi)
    pub conquest_mode: bool,
    pub favorite_religion: Option<ReligionType>,
    /// C++ `m_iTowerVicFlag` (`getArcaneTowerVictoryFlag`).
    pub tower_vic_flag: i32,
    /// C++ `m_bSumSuiMode` (`AI_isSummonSuicideMode`).
    pub sum_sui_mode: bool,

    // ---------------------------------------------------------------- FFH2 additions
    pub adaptive: bool,
    pub agnostic: bool,
    pub assimilation: bool,
    pub declaring_war: bool,
    /// C++ `m_bDisableHuman` (`getDisableHuman`; set through `player_set_disable_human`).
    pub disable_human: bool,
    pub hide_units: bool,
    pub ignore_food: bool,
    pub insane: bool,
    pub see_invisible: bool,
    pub sprawling: bool,
    /// C++ `m_iAlignment` (`getAlignment`; an AlignmentTypes value, -1 = none). C++ compares
    /// alignments numerically (good < neutral < evil); `Option` ordering matches (None < Some).
    pub alignment: Option<AlignmentType>,
    pub disable_production: i32,
    pub disable_research: i32,
    pub disable_spellcasting: i32,
    pub discover_rand_modifier: i32,
    pub enslavement_chance: i32,
    pub free_xp_from_combat: i32,
    pub heal_change: i32,
    pub heal_change_enemy: i32,
    /// C++ `m_iMaxCities` (-1 = unlimited).
    pub max_cities: i32,
    /// C++ `m_iNoDiplomacyWithEnemies` (count; `isNoDiplomacyWithEnemies()` is `!= 0`).
    pub no_diplomacy_with_enemies: i32,
    pub pillaging_gold: i32,
    pub players_killed: i32,
    /// C++ `m_iRealPlayer`: the real player behind a temporary player (FFH "temp player").
    pub real_player: Option<PlayerId>,
    pub resist_enemy_modify: i32,
    pub resist_modify: i32,
    pub sanctuary_timer: i32,
    pub starting_gold: i32,
    pub summon_duration: i32,
    pub temp_player_timer: i32,
    pub upgrade_cost_modifier: i32,
    /// C++ `m_pbTraits` (`hasTrait()`; changed only through `player_set_has_trait`).
    pub traits: IdxVec<TraitType, bool>,
    /// C++ `m_ppaaiSpecialistTypeExtraCommerce` (FFH).
    pub specialist_type_extra_commerce: IdxVec<SpecialistType, [i32; CommerceType::NUM]>,

    // ---------------------------------------------------------------- yield / commerce arrays
    pub sea_plot_yield: [i32; YieldType::NUM],
    pub yield_rate_modifier: [i32; YieldType::NUM],
    pub capital_yield_rate_modifier: [i32; YieldType::NUM],
    pub extra_yield_threshold: [i32; YieldType::NUM],
    pub trade_yield_modifier: [i32; YieldType::NUM],
    pub free_city_commerce: [i32; CommerceType::NUM],
    pub commerce_percent: [i32; CommerceType::NUM],
    /// C++ `m_aiCommerceRate`, stored x100 (`getCommerceRate()` divides by 100, see
    /// `player_get_commerce_rate`; `changeCommerceRate` adds raw x100 values).
    pub commerce_rate: [i32; CommerceType::NUM],
    pub commerce_rate_modifier: [i32; CommerceType::NUM],
    pub capital_commerce_rate_modifier: [i32; CommerceType::NUM],
    pub state_religion_building_commerce: [i32; CommerceType::NUM],
    pub specialist_extra_commerce: [i32; CommerceType::NUM],
    pub commerce_flexible_count: [i32; CommerceType::NUM],
    /// C++ `m_aiGoldPerTurnByPlayer` (changed only through `player_change_gold_per_turn_by_player`).
    pub gold_per_turn_by_player: IdxVec<PlayerId, i32>,
    pub espionage_spending_weight_against_team: IdxVec<TeamId, i32>,
    pub feat_accomplished: IdxVec<FeatType, bool>,
    pub options: IdxVec<PlayerOptionType, bool>,
    /// C++ `m_szScriptData` (Python script data).
    pub script_data: String,

    // ---------------------------------------------------------------- per-type arrays
    pub bonus_export: IdxVec<BonusType, i32>,
    pub bonus_import: IdxVec<BonusType, i32>,
    pub improvement_count: IdxVec<ImprovementType, i32>,
    pub free_building_count: IdxVec<BuildingType, i32>,
    pub extra_building_happiness: IdxVec<BuildingType, i32>,
    pub extra_building_health: IdxVec<BuildingType, i32>,
    pub feature_happiness: IdxVec<FeatureType, i32>,
    pub unit_class_count: IdxVec<UnitClassType, i32>,
    pub unit_class_making: IdxVec<UnitClassType, i32>,
    pub building_class_count: IdxVec<BuildingClassType, i32>,
    pub building_class_making: IdxVec<BuildingClassType, i32>,
    pub hurry_count: IdxVec<HurryType, i32>,
    pub special_building_not_required_count: IdxVec<SpecialBuildingType, i32>,
    pub has_civic_option_count: IdxVec<CivicOptionType, i32>,
    pub no_civic_upkeep_count: IdxVec<CivicOptionType, i32>,
    pub has_religion_count: IdxVec<ReligionType, i32>,
    /// C++ `m_paiHasCorporationCount` (raw; `getHasCorporationCount` checks
    /// `isActiveCorporation`, see `player_get_has_corporation_count`).
    pub has_corporation_count: IdxVec<CorporationType, i32>,
    pub upkeep_count: IdxVec<UpkeepType, i32>,
    pub specialist_valid_count: IdxVec<SpecialistType, i32>,
    pub researching_tech: IdxVec<TechType, bool>,
    /// C++ `m_pabLoyalMember` (set through `player_set_loyal_member`).
    pub loyal_member: IdxVec<VoteSourceType, bool>,
    /// C++ `m_paeCivics` (`getCivics`; changed only through `player_set_civics`).
    pub civics: IdxVec<CivicOptionType, Option<CivicType>>,
    pub specialist_extra_yield: IdxVec<SpecialistType, [i32; YieldType::NUM]>,
    pub improvement_yield_change: IdxVec<ImprovementType, [i32; YieldType::NUM]>,

    // ---------------------------------------------------------------- lists
    /// C++ `m_triggersFired`.
    pub triggers_fired: Vec<EventTriggerType>,
    /// C++ `m_groupCycle`: selection group cycling order (human unit cycling; the C++ AI
    /// also updates groups in this order).
    pub group_cycle: Vec<GroupId>,
    /// C++ `m_researchQueue` (head = current research).
    pub research_queue: Vec<TechType>,
    /// C++ `m_cityNames` (text keys or literal names added by `addCityName`).
    pub city_names: Vec<String>,
    /// C++ `m_plotGroups` (owner list, creation order; objects live in `Game::plot_groups`).
    pub plot_groups: Vec<PlotGroupId>,
    /// C++ `m_cities` (owner list, creation order; objects live in `Game::cities`).
    pub cities: Vec<CityId>,
    /// C++ `m_units` (owner list, creation order; objects live in `Game::units`).
    pub units: Vec<UnitId>,
    /// C++ `m_selectionGroups` (owner list, creation order; objects live in `Game::groups`).
    pub groups: Vec<GroupId>,
    /// C++ `m_eventsTriggered`: pending triggered events, in creation order.
    pub events_triggered: Vec<EventTriggeredData>,
    /// Next id handed out for `events_triggered` (replaces the C++ free-list id counter).
    pub next_triggered_id: u32,
    /// C++ `m_mapEventsOccured` (hash_map in C++; ordered here for determinism).
    pub events_occured: BTreeMap<EventType, EventTriggeredData>,
    /// C++ `m_mapEventCountdown`.
    pub event_countdown: BTreeMap<EventType, EventTriggeredData>,
    /// C++ `m_aFreeUnitCombatPromotions`.
    pub free_unit_combat_promotions: Vec<(UnitCombatType, PromotionType)>,
    /// C++ `m_aFreeUnitClassPromotions`.
    pub free_unit_class_promotions: Vec<(UnitClassType, PromotionType)>,
    /// C++ `m_aVote` (`getVote`/`setVote`).
    pub votes: Vec<(VoteId, PlayerVote)>,
    /// C++ `m_aUnitExtraCosts` (`getUnitExtraCost`/`setUnitExtraCost`).
    pub unit_extra_costs: Vec<(UnitClassType, i32)>,

    // ---------------------------------------------------------------- score history
    // C++ `m_mapScoreHistory` .. `m_mapEspionageHistory` (hash_map turn -> value), kept as
    // vectors indexed by game turn (missing turns read 0); see `turn_history` / `set_turn_history`.
    /// Score v2 (share of the world in basis points) recorded by `update_score`.
    pub score_history: Vec<i32>,
    pub economy_history: Vec<i32>,
    pub industry_history: Vec<i32>,
    pub agriculture_history: Vec<i32>,
    pub power_history: Vec<i32>,
    pub culture_history: Vec<i32>,
    pub espionage_history: Vec<i32>,

    // ---------------------------------------------------------------- CvPlayerAI bookkeeping used by rules
    /// C++ `CvPlayerAI::m_iCivicTimer` (`AI_get/set/changeCivicTimer`; CvDeal reads/writes it).
    pub ai_civic_timer: i32,
    /// C++ `CvPlayerAI::m_iReligionTimer` (`AI_get/set/changeReligionTimer`).
    pub ai_religion_timer: i32,
    /// C++ `CvPlayerAI::m_aiNumTrainAIUnits` (`AI_changeNumTrainAIUnits`, CvCity).
    pub ai_num_train_ai_units: IdxVec<UnitAIType, i32>,
    /// C++ `CvPlayerAI::m_aiNumAIUnits` (`AI_changeNumAIUnits`, CvUnit; `AI_getNumAIUnits`).
    pub ai_num_ai_units: IdxVec<UnitAIType, i32>,
    /// C++ `CvPlayerAI::m_aiPeacetimeTradeValue` (change through
    /// `player_ai_change_peacetime_trade_value`, which also updates team AI state).
    pub ai_peacetime_trade_value: IdxVec<PlayerId, i32>,
    /// C++ `CvPlayerAI::m_aiPeacetimeGrantValue` (see `player_ai_change_peacetime_grant_value`).
    pub ai_peacetime_grant_value: IdxVec<PlayerId, i32>,
    /// C++ `CvPlayerAI::m_aiGoldTradedTo` (`AI_changeGoldTradedTo`, CvDeal).
    pub ai_gold_traded_to: IdxVec<PlayerId, i32>,
    /// C++ `CvPlayerAI::m_aiAttitudeExtra` (`AI_get/set/changeAttitudeExtra`; FFH Python events).
    pub ai_attitude_extra: IdxVec<PlayerId, i32>,
    /// C++ `CvPlayerAI::m_abFirstContact` (`AI_setFirstContact`, CvTeam/CvPlayer).
    pub ai_first_contact: IdxVec<PlayerId, bool>,
    /// C++ `CvPlayerAI::m_aaiMemoryCount` (`AI_get/changeMemoryCount`): `[other][memory]`.
    pub ai_memory_count: IdxVec<PlayerId, IdxVec<MemoryType, i32>>,
    /// C++ `CvPlayerAI::m_aiBonusValue`: the `AI_baseBonusVal` cache by bonus (-1 = not
    /// computed). Filled from const code in C++, hence `Cell`; reset by
    /// `player_ai_update_bonus_value` (each turn, team merge, empire split). Rules read it
    /// through `AI_foundValue` (start plots, barbarian cities).
    pub ai_bonus_value: Vec<std::cell::Cell<i32>>,
}

impl Player {
    /// C++ `CvPlayer::reset(eID)` + `CvPlayerAI::AI_reset(false)` for slot `id`: every
    /// member at its reset value, arrays sized from the game data and from this game's
    /// player/team counts. Team, human flag and personality come from the slot setup
    /// (C++ `updateTeamType`, `updateHuman`, `GC.getInitCore().getLeader(eID)`).
    ///
    /// The C++ reset also clears other players' entries about this player
    /// (`m_aiGoldPerTurnByPlayer[eID]`, espionage weights against its team and the
    /// CvPlayerAI per-player arrays); that cross-player part belongs to `player_init`.
    pub fn new(d: &GameData, id: PlayerId, setup: &PlayerSetup, num_players: usize, num_teams: usize) -> Player {
        let n_spec = d.specialists.len();
        let n_impr = d.improvements.len();
        let n_bldg = d.buildings.len();
        let n_copt = d.civic_options.len();
        Player {
            id,
            personality_type: setup.leader,
            current_era: EraType(0),
            last_state_religion: None,
            parent: None,
            team: setup.team,

            starting_x: INVALID_PLOT_COORD,
            starting_y: INVALID_PLOT_COORD,
            total_population: 0,
            total_land: 0,
            total_land_scored: 0,
            gold: 0,
            gold_per_turn: 0,
            advanced_start_points: -1,
            golden_age_turns: 0,
            num_unit_golden_ages: 0,
            strike_turns: 0,
            anarchy_turns: 0,
            max_anarchy_turns: 0,
            anarchy_modifier: 0,
            golden_age_modifier: 0,
            global_hurry_modifier: 0,
            great_people_created: 0,
            great_generals_created: 0,
            great_people_threshold_modifier: 0,
            great_generals_threshold_modifier: 0,
            great_people_rate_modifier: 0,
            great_general_rate_modifier: 0,
            domestic_great_general_rate_modifier: 0,
            state_religion_great_people_rate_modifier: 0,
            max_global_building_production_modifier: 0,
            max_team_building_production_modifier: 0,
            max_player_building_production_modifier: 0,
            free_experience: 0,
            feature_production_modifier: 0,
            worker_speed_modifier: 0,
            improvement_upgrade_rate_modifier: 0,
            military_production_modifier: 0,
            space_production_modifier: 0,
            city_defense_modifier: 0,
            num_nuke_units: 0,
            num_outside_units: 0,
            base_free_units: 0,
            base_free_military_units: 0,
            free_units_population_percent: 0,
            free_military_units_population_percent: 0,
            gold_per_unit: 0,
            gold_per_military_unit: 0,
            extra_unit_cost: 0,
            num_military_units: 0,
            happy_per_military_unit: 0,
            military_food_production_count: 0,
            conscript_count: 0,
            max_conscript: 0,
            highest_unit_level: 1,
            overflow_research: 0,
            no_unhealthy_population_count: 0,
            exp_in_border_modifier: 0,
            building_only_healthy_count: 0,
            distance_maintenance_modifier: 0,
            num_cities_maintenance_modifier: 0,
            corporation_maintenance_modifier: 0,
            total_maintenance: 0,
            upkeep_modifier: 0,
            level_experience_modifier: 0,
            extra_health: 0,
            building_good_health: 0,
            building_bad_health: 0,
            extra_happiness: 0,
            building_happiness: 0,
            largest_city_happiness: 0,
            war_weariness_percent_anger: 0,
            war_weariness_modifier: 0,
            free_specialist: 0,
            no_foreign_trade_count: 0,
            no_corporations_count: 0,
            no_foreign_corporations_count: 0,
            coastal_trade_routes: 0,
            trade_routes: 0,
            revolution_timer: 0,
            conversion_timer: 0,
            state_religion_count: 0,
            no_non_state_religion_spread_count: 0,
            state_religion_happiness: 0,
            non_state_religion_happiness: 0,
            state_religion_unit_production_modifier: 0,
            state_religion_building_production_modifier: 0,
            state_religion_free_experience: 0,
            capital_city: None,
            cities_lost: 0,
            wins_vs_barbs: 0,
            assets: 0,
            power: 0,
            population_score: 0,
            land_score: 0,
            tech_score: 0,
            wonders_score: 0,
            combat_experience: 0,
            pop_rush_hurry_count: 0,
            inflation_modifier: 0,

            alive: false,
            ever_alive: false,
            turn_active: false,
            auto_moves: false,
            end_turn: false,
            extended_game: false,
            founded_first_city: false,
            strike: false,
            // C++ updateHuman(): m_bDisableHuman is false right after reset.
            human: setup.human,

            conquest_mode: false,
            favorite_religion: None,
            tower_vic_flag: 0,
            // C++ quirk: reset() declares a local `bool m_bSumSuiMode = false;`, so the member
            // is never reset (left uninitialized on a fresh player). Start it at false.
            sum_sui_mode: false,

            adaptive: false,
            agnostic: false,
            assimilation: false,
            declaring_war: false,
            disable_human: false,
            hide_units: false,
            ignore_food: false,
            insane: false,
            see_invisible: false,
            sprawling: false,
            alignment: None,
            disable_production: 0,
            disable_research: 0,
            disable_spellcasting: 0,
            discover_rand_modifier: 0,
            enslavement_chance: 0,
            free_xp_from_combat: 0,
            heal_change: 0,
            heal_change_enemy: 0,
            max_cities: -1,
            no_diplomacy_with_enemies: 0,
            pillaging_gold: 0,
            players_killed: 0,
            real_player: None,
            resist_enemy_modify: 0,
            resist_modify: 0,
            sanctuary_timer: 0,
            starting_gold: 0,
            summon_duration: 0,
            temp_player_timer: 0,
            upgrade_cost_modifier: 0,
            traits: IdxVec::from_elem(false, d.traits.len()),
            specialist_type_extra_commerce: IdxVec::from_elem([0; CommerceType::NUM], n_spec),

            sea_plot_yield: [0; YieldType::NUM],
            yield_rate_modifier: [0; YieldType::NUM],
            capital_yield_rate_modifier: [0; YieldType::NUM],
            extra_yield_threshold: [0; YieldType::NUM],
            trade_yield_modifier: [0; YieldType::NUM],
            free_city_commerce: [0; CommerceType::NUM],
            commerce_percent: [0; CommerceType::NUM],
            commerce_rate: [0; CommerceType::NUM],
            commerce_rate_modifier: [0; CommerceType::NUM],
            capital_commerce_rate_modifier: [0; CommerceType::NUM],
            state_religion_building_commerce: [0; CommerceType::NUM],
            specialist_extra_commerce: [0; CommerceType::NUM],
            commerce_flexible_count: [0; CommerceType::NUM],
            gold_per_turn_by_player: IdxVec::from_elem(0, num_players),
            espionage_spending_weight_against_team: IdxVec::from_elem(0, num_teams),
            feat_accomplished: IdxVec::from_elem(false, FeatType::NUM),
            options: IdxVec::from_elem(false, PlayerOptionType::NUM),
            script_data: String::new(),

            bonus_export: IdxVec::from_elem(0, d.bonuses.len()),
            bonus_import: IdxVec::from_elem(0, d.bonuses.len()),
            improvement_count: IdxVec::from_elem(0, n_impr),
            free_building_count: IdxVec::from_elem(0, n_bldg),
            extra_building_happiness: IdxVec::from_elem(0, n_bldg),
            extra_building_health: IdxVec::from_elem(0, n_bldg),
            feature_happiness: IdxVec::from_elem(0, d.features.len()),
            unit_class_count: IdxVec::from_elem(0, d.unit_classes.len()),
            unit_class_making: IdxVec::from_elem(0, d.unit_classes.len()),
            building_class_count: IdxVec::from_elem(0, d.building_classes.len()),
            building_class_making: IdxVec::from_elem(0, d.building_classes.len()),
            hurry_count: IdxVec::from_elem(0, d.hurries.len()),
            special_building_not_required_count: IdxVec::from_elem(0, d.special_buildings.len()),
            has_civic_option_count: IdxVec::from_elem(0, n_copt),
            no_civic_upkeep_count: IdxVec::from_elem(0, n_copt),
            has_religion_count: IdxVec::from_elem(0, d.religions.len()),
            has_corporation_count: IdxVec::from_elem(0, d.corporations.len()),
            upkeep_count: IdxVec::from_elem(0, d.upkeeps.len()),
            specialist_valid_count: IdxVec::from_elem(0, n_spec),
            researching_tech: IdxVec::from_elem(false, d.techs.len()),
            loyal_member: IdxVec::from_elem(true, d.vote_sources.len()),
            civics: IdxVec::from_elem(None, n_copt),
            specialist_extra_yield: IdxVec::from_elem([0; YieldType::NUM], n_spec),
            improvement_yield_change: IdxVec::from_elem([0; YieldType::NUM], n_impr),

            triggers_fired: Vec::new(),
            group_cycle: Vec::new(),
            research_queue: Vec::new(),
            city_names: Vec::new(),
            plot_groups: Vec::new(),
            cities: Vec::new(),
            units: Vec::new(),
            groups: Vec::new(),
            events_triggered: Vec::new(),
            next_triggered_id: 0,
            events_occured: BTreeMap::new(),
            event_countdown: BTreeMap::new(),
            free_unit_combat_promotions: Vec::new(),
            free_unit_class_promotions: Vec::new(),
            votes: Vec::new(),
            unit_extra_costs: Vec::new(),

            score_history: Vec::new(),
            economy_history: Vec::new(),
            industry_history: Vec::new(),
            agriculture_history: Vec::new(),
            power_history: Vec::new(),
            culture_history: Vec::new(),
            espionage_history: Vec::new(),

            ai_civic_timer: 0,
            ai_religion_timer: 0,
            ai_num_train_ai_units: IdxVec::from_elem(0, UnitAIType::NUM),
            ai_num_ai_units: IdxVec::from_elem(0, UnitAIType::NUM),
            ai_peacetime_trade_value: IdxVec::from_elem(0, num_players),
            ai_peacetime_grant_value: IdxVec::from_elem(0, num_players),
            ai_gold_traded_to: IdxVec::from_elem(0, num_players),
            ai_attitude_extra: IdxVec::from_elem(0, num_players),
            ai_first_contact: IdxVec::from_elem(false, num_players),
            ai_memory_count: IdxVec::from_fn(num_players, |_| IdxVec::from_elem(0, MemoryType::NUM)),
            ai_bonus_value: (0..d.bonuses.len()).map(|_| std::cell::Cell::new(-1)).collect(),
        }
    }

    /// C++ `getNumCities()`.
    #[inline]
    pub fn num_cities(&self) -> i32 {
        self.cities.len() as i32
    }

    /// C++ `getNumUnits()`.
    #[inline]
    pub fn num_units(&self) -> i32 {
        self.units.len() as i32
    }
}

/// C++ `getScoreHistory(iTurn)` & co: value recorded for `turn`, 0 if none.
pub fn turn_history(history: &[i32], turn: i32) -> i32 {
    if turn < 0 {
        return 0;
    }
    history.get(turn as usize).copied().unwrap_or(0)
}

/// C++ `updateScoreHistory(iTurn, iValue)` & co.
pub fn set_turn_history(history: &mut Vec<i32>, turn: i32, value: i32) {
    if turn < 0 {
        return;
    }
    let t = turn as usize;
    if history.len() <= t {
        history.resize(t + 1, 0);
    }
    history[t] = value;
}

impl Game {
    /// C++ `CvPlayer::init(eID)` (+ `CvPlayerAI::AI_init`): reset the slot
    /// (`self.players[p] = Player::new(..)` plus the cross-player part of `reset`: other
    /// players' `gold_per_turn_by_player[p]`, espionage weights against `p`'s team and
    /// CvPlayerAI arrays indexed by `p`), join the team, and for taken/computer slots:
    /// random personality, initial free units/trade routes/commerce percents, traits,
    /// alignment, global counter limit, max anarchy, extra yield thresholds, initial
    /// civics, event state and the settler extra cost; then AI_init (peace weight,
    /// civic/religion timers).
    pub fn player_init(&mut self, p: PlayerId) {
        let d = self.d;

        //--------------------------------
        // Init saved data: reset(eID), which ends with AI_reset(false).
        reset_player(self, p);

        //--------------------------------
        // Init containers: `Player::new` created them empty.
        // Init non-saved data: setupGraphical() is graphics only.

        //--------------------------------
        // Init other game data
        let team = self.players[p].team;
        self.teams[team].num_members += 1;

        let slot_status = self.setup.players[p.idx()].slot_status;
        if slot_status == SlotStatus::Taken || slot_status == SlotStatus::Computer {
            self.player_set_alive(p, true);

            // Dead code in FfH (CvGame::isOption locks RANDOM_PERSONALITIES off); kept as in C++.
            if self.is_option(GameOptionType::GAMEOPTION_RANDOM_PERSONALITIES) {
                if !self.player_is_barbarian(p) && !self.player_is_minor_civ(p) {
                    let mut best_value = 0;
                    let mut best_personality: Option<LeaderHeadType> = None;

                    for leader in d.leader_heads.keys() {
                        if leader.i() != d.defines.barbarian_leader {
                            // XXX minor civ???
                            let mut value = 1 + self.soren_rand_num(10000, "Choosing Personality");

                            for other in self.civ_player_ids() {
                                if self.players[other].alive && self.players[other].personality_type == Some(leader) {
                                    value /= 2;
                                }
                            }

                            if value > best_value {
                                best_value = value;
                                best_personality = Some(leader);
                            }
                        }
                    }

                    if best_personality.is_some() {
                        self.players[p].personality_type = best_personality;
                    }
                }
            }

            // changeBaseFreeUnits & co: plain changes (their only side effects are UI dirty bits).
            let df = &d.defines;
            self.players[p].base_free_units += df.initial_base_free_units;
            self.players[p].base_free_military_units += df.initial_base_free_military_units;
            self.players[p].free_units_population_percent += df.initial_free_units_population_percent;
            self.players[p].free_military_units_population_percent += df.initial_free_military_units_population_percent;
            self.players[p].gold_per_unit += df.initial_gold_per_unit;
            self.player_change_trade_routes(p, df.initial_trade_routes);
            self.player_change_state_religion_happiness(p, df.initial_state_religion_happiness);
            self.player_change_non_state_religion_happiness(p, df.initial_non_state_religion_happiness);

            for y in 0..YieldType::NUM {
                let y = YieldType::from_usize(y);
                self.player_change_trade_yield_modifier(p, y, d.yields[y].trade_modifier);
            }

            for c in 0..CommerceType::NUM {
                let c = CommerceType::from_usize(c);
                self.player_set_commerce_percent(p, c, d.commerces[c].initial_percent);
            }

            // C++ indexes the info arrays with these unchecked; a taken/computer slot always
            // has a leader and a civilization.
            let leader = self.player_get_leader_type(p).expect("CvPlayer::init: taken/computer slot without a leader");
            let civ = self
                .player_get_civilization_type(p)
                .expect("CvPlayer::init: taken/computer slot without a civilization");

            // FfH: Modified by Kael (the BtS trait effects moved into setHasTrait)
            for t in d.traits.keys() {
                if d.leader_heads[leader].has_trait[t] {
                    self.player_set_has_trait(p, t, true);
                }
            }

            // FfH: Added by Kael 08/14/2007
            if let Some(civ_trait) = d.civilizations[civ].civ_trait {
                self.player_set_has_trait(p, civ_trait, true);
            }
            // setAlignment: plain store (+ UI dirty bit)
            self.players[p].alignment = d.leader_heads[leader].alignment;
            // CvGame::changeGlobalCounterLimit
            self.global_counter_limit += df.global_counter_limit_per_player;
            // FfH: End Add

            self.player_update_max_anarchy_turns(p);

            for y in 0..YieldType::NUM {
                self.player_update_extra_yield_threshold(p, YieldType::from_usize(y));
            }

            for o in d.civic_options.keys() {
                self.player_set_civics(p, o, d.civilizations[civ].civilization_initial_civics[o]);
            }

            for e in d.events.keys() {
                self.player_reset_event_occured(p, e, false);
            }

            for t in d.event_triggers.keys() {
                self.player_reset_trigger_fired(p, t);
            }

            for uc in d.unit_classes.keys() {
                if let Some(unit) = d.civ_unit(civ, uc) {
                    if d.units[unit].found {
                        let cost = self.player_get_new_city_production_value(p);
                        self.player_set_unit_extra_cost(p, uc, cost);
                    }
                }
            }
        }

        ai_init(self, p);
    }
}

/// C++ `CvPlayer::reset(eID)` for a game slot (`bConstructorCall = false`), including `uninit`
/// and the trailing `CvPlayerAI::AI_reset(false)`.
///
/// `Player::new` holds the per-member defaults; this adds what `reset` does outside the
/// player itself: the C++ `removeAll()` of the plot group / city / unit / selection group
/// lists deletes those objects (here: from the game slabs; the lists are empty for every slot
/// C++ (re)initializes in practice), and other players' entries about this player are cleared.
fn reset_player(g: &mut Game, p: PlayerId) {
    let old = std::mem::take(&mut g.players[p]);
    for id in old.plot_groups {
        g.plot_groups.remove(id);
    }
    for id in old.cities {
        g.cities.remove(id);
    }
    for id in old.units {
        g.units.remove(id);
    }
    for id in old.groups {
        g.groups.remove(id);
    }

    let (num_players, num_teams) = (g.num_players(), g.num_teams());
    let fresh = Player::new(g.d, p, &g.setup.players[p.idx()], num_players, num_teams);
    g.players[p] = fresh;

    // C++ placeholders (slots not initialized yet) have no team and are skipped; here their
    // arrays are still empty, so `get_mut` skips them the same way.
    let team = g.players[p].team;
    for other in g.player_ids() {
        // GET_PLAYER(iI).m_aiGoldPerTurnByPlayer[getID()] = 0 (raw store: m_iGoldPerTurn is
        // not adjusted)
        if let Some(v) = g.players[other].gold_per_turn_by_player.get_mut(p) {
            *v = 0;
        }
        // C++ loops teams, then the players of each team: every player with a team gets
        // setEspionageSpendingWeightAgainstTeam(getTeam(), 0) (clamp + compare + UI dirty bit,
        // i.e. a plain store of 0).
        if let Some(v) = g.players[other].espionage_spending_weight_against_team.get_mut(team) {
            *v = 0;
        }
    }

    ai_reset(g, p);
}

/// C++ `CvPlayerAI::AI_reset(false)`, rules-visible part: this player's AI bookkeeping fields
/// (including the `AI_baseBonusVal` cache) and the other players' entries about it. (The other
/// CvPlayerAI members are AI-only state, not kept in `Player`.)
fn ai_reset(g: &mut Game, p: PlayerId) {
    {
        let pl = &mut g.players[p];
        pl.ai_civic_timer = 0;
        pl.ai_religion_timer = 0;
        pl.ai_num_train_ai_units.fill(0);
        pl.ai_num_ai_units.fill(0);
        pl.ai_peacetime_trade_value.fill(0);
        pl.ai_peacetime_grant_value.fill(0);
        pl.ai_gold_traded_to.fill(0);
        pl.ai_attitude_extra.fill(0);
        pl.ai_first_contact.fill(false);
        for memory in pl.ai_memory_count.iter_mut() {
            memory.fill(0);
        }
    }
    // m_aiBonusValue: re-allocated with every entry -1 (the AI_baseBonusVal cache).
    g.player_ai_update_bonus_value(p);

    for other in g.player_ids() {
        let o = &mut g.players[other];
        if let Some(v) = o.ai_peacetime_trade_value.get_mut(p) {
            *v = 0;
        }
        if let Some(v) = o.ai_peacetime_grant_value.get_mut(p) {
            *v = 0;
        }
        if let Some(v) = o.ai_gold_traded_to.get_mut(p) {
            *v = 0;
        }
        if let Some(v) = o.ai_attitude_extra.get_mut(p) {
            *v = 0;
        }
        if let Some(v) = o.ai_first_contact.get_mut(p) {
            *v = false;
        }
        if let Some(memory) = o.ai_memory_count.get_mut(p) {
            memory.fill(0);
        }
    }
}

/// C++ `CvPlayerAI::AI_init()`: `AI_reset(false)` again, then for taken/computer slots the
/// AI weights and timers.
fn ai_init(g: &mut Game, p: PlayerId) {
    ai_reset(g, p);

    //--------------------------------
    // Init other game data
    let slot_status = g.setup.players[p.idx()].slot_status;
    if slot_status == SlotStatus::Taken || slot_status == SlotStatus::Computer {
        let d = g.d;
        let personality = g.players[p].personality_type.expect("CvPlayerAI::AI_init: getPersonalityType() is NO_LEADER");
        let info = &d.leader_heads[personality];
        // AI: AI_setPeaceWeight(getBasePeaceWeight() + rand): the peace weight is AI-only state
        // (not kept); the draw stays so the game RNG matches C++.
        let _peace_weight = info.base_peace_weight + g.soren_rand_num(info.peace_weight_rand, "AI Peace Weight");
        // AI: AI_setEspionageWeight(getEspionageWeight()) (AI-only state).
        g.players[p].ai_religion_timer = 1;
        g.players[p].ai_civic_timer = if g.players[p].max_anarchy_turns == 0 { 1 } else { 2 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::GameSetup;

    #[test]
    fn new_matches_cpp_reset_defaults() {
        let d = ffh_data::global();
        let setup = GameSetup::standard(d, 3, "WORLDSIZE_SMALL", 1);
        let n_players = setup.players.len();
        let n_teams = n_players;
        let pl = Player::new(d, PlayerId(1), &setup.players[1], n_players, n_teams);
        assert_eq!(pl.id, PlayerId(1));
        assert_eq!(pl.team, TeamId(1));
        assert_eq!(pl.starting_x, INVALID_PLOT_COORD);
        assert_eq!(pl.advanced_start_points, -1);
        assert_eq!(pl.highest_unit_level, 1);
        assert_eq!(pl.max_cities, -1);
        assert_eq!(pl.alignment, None);
        assert_eq!(pl.capital_city, None);
        assert_eq!(pl.current_era, EraType(0));
        assert!(pl.loyal_member.iter().all(|&b| b));
        assert!(pl.civics.iter().all(|c| c.is_none()));
        assert_eq!(pl.gold_per_turn_by_player.len(), n_players);
        assert_eq!(pl.espionage_spending_weight_against_team.len(), n_teams);
        assert_eq!(pl.ai_memory_count.len(), n_players);
        assert_eq!(pl.ai_memory_count[PlayerId(0)].len(), MemoryType::NUM);
        assert_eq!(pl.traits.len(), d.traits.len());
        assert_eq!(pl.building_class_count.len(), d.building_classes.len());
        assert_eq!(pl.specialist_extra_yield.len(), d.specialists.len());
    }

    #[test]
    fn turn_history_roundtrip() {
        let mut h = Vec::new();
        assert_eq!(turn_history(&h, 5), 0);
        set_turn_history(&mut h, 5, 42);
        assert_eq!(turn_history(&h, 5), 42);
        assert_eq!(turn_history(&h, 4), 0);
        assert_eq!(turn_history(&h, -1), 0);
    }
}
