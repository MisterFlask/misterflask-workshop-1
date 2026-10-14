//! Port of `CvTeam` (CvTeam.h / CvTeam.cpp) plus the rules-visible bookkeeping of
//! `CvTeamAI` (war plans, contact counters, war success, worst enemy).
//!
//! Methods are `impl Game` blocks split by topic (C++ line ranges per file are listed in
//! `docs/porting/team_group_deal.md`):
//! - [`diplomacy`]: war & peace, contact, open borders, defensive pacts, force peace,
//!   vassals, war weariness, and the `CvTeamAI` war-plan / counter bookkeeping.
//! - [`tech`]: research, techs, tech sharing, bonuses, terrain/route/irrigation effects,
//!   building effects, per-turn processing.
//! - [`misc`]: membership and aggregates over member players, projects, unit/building
//!   class limits, victory, espionage, team merging (permanent alliances).
//!
//! Trivial C++ accessors (`getX`/`isX`/`setX`/`changeX` that only read or write one member,
//! possibly with an `FAssert`) are not methods: use the fields, e.g.
//! `GET_TEAM(t).isAtWar(o)` -> `self.teams[t].at_war[o]`,
//! `changeNumCities(n)` -> `self.teams[t].num_cities += n`,
//! `AI_getWarSuccess(o)` -> `self.teams[t].ai_war_success[o]`.
//! Accessors with logic are methods even when they look trivial; in particular
//! `team_is_open_borders` (FFH: GAMEOPTION_ALWAYS_OPEN_BORDERS), `team_is_has_tech` and
//! `team_get_research_progress` (accept NO_TECH), `team_is_alive`, `team_set_war_weariness`
//! (clamps), `team_ai_set_war_plan`.

#![allow(unused_imports)]
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

pub mod diplomacy;
pub mod misc;
pub mod tech;

/// C++ `CvTeam` (+ `CvTeamAI` members, prefixed `ai_`).
///
/// Per-team arrays (`IdxVec<TeamId, _>`) are sized `Game::num_teams()`; per-type arrays
/// are sized from the data tables. Constructed by [`Team::new`] (C++ `reset`).
#[derive(Clone, Debug, Default)]
pub struct Team {
    /// C++ `m_eID`.
    pub id: TeamId,
    /// C++ `m_iNumMembers` (`getNumMembers` / `changeNumMembers`).
    pub num_members: i32,
    /// C++ `m_iAliveCount`. Use `team_is_alive` / `team_change_alive_count` (frees vassals).
    pub alive_count: i32,
    /// C++ `m_iEverAliveCount`.
    pub ever_alive_count: i32,
    /// C++ `m_iNumCities`.
    pub num_cities: i32,
    /// C++ `m_iTotalPopulation` (raw; `team_get_total_population` adds vassals).
    pub total_population: i32,
    /// C++ `m_iTotalLand` (raw; `team_get_total_land` adds vassals).
    pub total_land: i32,
    /// C++ `m_iNukeInterception`.
    pub nuke_interception: i32,
    /// C++ `m_iExtraWaterSeeFromCount`. Change through `team_change_extra_water_see_from_count`.
    pub extra_water_see_from_count: i32,
    /// C++ `m_iMapTradingCount`.
    pub map_trading_count: i32,
    /// C++ `m_iTechTradingCount`.
    pub tech_trading_count: i32,
    /// C++ `m_iGoldTradingCount`.
    pub gold_trading_count: i32,
    /// C++ `m_iOpenBordersTradingCount`.
    pub open_borders_trading_count: i32,
    /// C++ `m_iDefensivePactTradingCount`.
    pub defensive_pact_trading_count: i32,
    /// C++ `m_iPermanentAllianceTradingCount` (`team_is_permanent_alliance_trading` checks the option).
    pub permanent_alliance_trading_count: i32,
    /// C++ `m_iVassalTradingCount` (`team_is_vassal_state_trading` checks the option).
    pub vassal_trading_count: i32,
    /// C++ `m_iBridgeBuildingCount` (`changeBridgeBuildingCount` only adds a graphics refresh).
    pub bridge_building_count: i32,
    /// C++ `m_iIrrigationCount`. Change through `team_change_irrigation_count`.
    pub irrigation_count: i32,
    /// C++ `m_iIgnoreIrrigationCount`.
    pub ignore_irrigation_count: i32,
    /// C++ `m_iWaterWorkCount`. Change through `team_change_water_work_count`.
    pub water_work_count: i32,
    /// C++ `m_iVassalPower` (land when the vassal agreement was signed).
    pub vassal_power: i32,
    /// C++ `m_iMasterPower` (master's land when the vassal agreement was signed).
    pub master_power: i32,
    /// C++ `m_iEnemyWarWearinessModifier`.
    pub enemy_war_weariness_modifier: i32,
    /// C++ `m_iRiverTradeCount`. Change through `team_change_river_trade_count`.
    pub river_trade_count: i32,
    /// C++ `m_iEspionagePointsEver`.
    pub espionage_points_ever: i32,
    /// C++ `m_bMapCentering` (`setMapCentering` only adds a minimap refresh).
    pub map_centering: bool,
    /// C++ `m_bCapitulated` (`isCapitulated`; meaningful only while a vassal).
    pub capitulated: bool,
    /// C++ `m_bBarbarianAlly` (FfH trait).
    pub barbarian_ally: bool,
    /// C++ `m_bNoCivicAnger` (FfH).
    pub no_civic_anger: bool,

    /// C++ `m_aiStolenVisibilityTimer`. Change through `team_set_stolen_visibility_timer`.
    pub stolen_visibility_timer: IdxVec<TeamId, i32>,
    /// C++ `m_aiWarWeariness`. Write through `team_set_war_weariness` (clamps at 0).
    pub war_weariness: IdxVec<TeamId, i32>,
    /// C++ `m_aiTechShareCount`: index `i` counts "share techs known by `i + 1` met teams"
    /// effects (C++ sized `MAX_TEAMS`; here `num_teams()`). Change through
    /// `team_change_tech_share_count`.
    pub tech_share_count: Vec<i32>,
    /// C++ `m_aiCommerceFlexibleCount` (`changeCommerceFlexibleCount` only adds UI refresh).
    pub commerce_flexible_count: [i32; CommerceType::NUM],
    /// C++ `m_aiExtraMoves`, indexed by `DomainType::idx()`.
    pub extra_moves: [i32; DomainType::NUM],
    /// C++ `m_aiForceTeamVoteEligibilityCount`.
    pub force_team_vote_eligibility_count: IdxVec<VoteSourceType, i32>,

    /// C++ `m_abAtWar` (`isAtWar` / `setAtWar` are plain accessors).
    pub at_war: IdxVec<TeamId, bool>,
    /// C++ `m_abHasMet` (`isHasMet`). Set through `team_make_has_met` / `team_meet`.
    pub has_met: IdxVec<TeamId, bool>,
    /// C++ `m_abPermanentWarPeace`.
    pub permanent_war_peace: IdxVec<TeamId, bool>,
    /// C++ `m_abOpenBorders`: the raw flag. Read through `team_is_open_borders`
    /// (FFH: always true under GAMEOPTION_ALWAYS_OPEN_BORDERS); write through
    /// `team_set_open_borders`.
    pub open_borders: IdxVec<TeamId, bool>,
    /// C++ `m_abDefensivePact`. Write through `team_set_defensive_pact`.
    pub defensive_pact: IdxVec<TeamId, bool>,
    /// C++ `m_abForcePeace`. Write through `team_set_force_peace`.
    pub force_peace: IdxVec<TeamId, bool>,
    /// C++ `m_abVassal`: `vassal[m]` = this team is a vassal of `m`. Write through `team_set_vassal`.
    pub vassal: IdxVec<TeamId, bool>,
    /// C++ `m_abCanLaunch` (`canLaunch` / `setCanLaunch`).
    pub can_launch: IdxVec<VictoryType, bool>,

    /// C++ `m_paiRouteChange`.
    pub route_change: IdxVec<RouteType, i32>,
    /// C++ `m_paiProjectCount`. Change through `team_change_project_count`.
    pub project_count: IdxVec<ProjectType, i32>,
    /// C++ `m_paiProjectMaking`.
    pub project_making: IdxVec<ProjectType, i32>,
    /// C++ `m_paiUnitClassCount`.
    pub unit_class_count: IdxVec<UnitClassType, i32>,
    /// C++ `m_paiBuildingClassCount`.
    pub building_class_count: IdxVec<BuildingClassType, i32>,
    /// C++ `m_paiObsoleteBuildingCount`. Change through `team_change_obsolete_building_count`.
    pub obsolete_building_count: IdxVec<BuildingType, i32>,
    /// C++ `m_paiResearchProgress` (can be negative in FFH). Write through
    /// `team_set_research_progress`; `team_get_research_progress` accepts NO_TECH.
    pub research_progress: IdxVec<TechType, i32>,
    /// C++ `m_paiTechCount` (times a repeatable tech was researched).
    pub tech_count: IdxVec<TechType, i32>,
    /// C++ `m_paiTerrainTradeCount`. Change through `team_change_terrain_trade_count`.
    pub terrain_trade_count: IdxVec<TerrainType, i32>,
    /// C++ `m_aiVictoryCountdown` (-1 = not counting down).
    pub victory_countdown: IdxVec<VictoryType, i32>,

    /// C++ `m_aiEspionagePointsAgainstTeam`. Write through `team_set_espionage_points_against_team`.
    pub espionage_points_against_team: IdxVec<TeamId, i32>,
    /// C++ `m_aiCounterespionageTurnsLeftAgainstTeam`.
    pub counterespionage_turns_left_against_team: IdxVec<TeamId, i32>,
    /// C++ `m_aiCounterespionageModAgainstTeam`.
    pub counterespionage_mod_against_team: IdxVec<TeamId, i32>,

    /// C++ `m_pabHasTech`. Read through `team_is_has_tech` when the tech may be NO_TECH;
    /// write only through `team_set_has_tech`.
    pub has_tech: IdxVec<TechType, bool>,
    /// C++ `m_pabNoTradeTech` (`isNoTradeTech` / `setNoTradeTech`).
    pub no_trade_tech: IdxVec<TechType, bool>,
    /// C++ `m_ppaaiImprovementYieldChange`. Change through `team_change_improvement_yield_change`.
    pub improvement_yield_change: IdxVec<ImprovementType, [i32; YieldType::NUM]>,
    /// C++ `m_aeRevealedBonuses` (ordered, as in C++). See `team_set_force_revealed_bonus`.
    pub revealed_bonuses: Vec<BonusType>,

    // ---- CvTeamAI members read/written by rules code ----
    /// C++ `CvTeamAI::m_eWorstEnemy` (`AI_getWorstEnemy`; updated by `team_ai_update_worst_enemy`).
    pub ai_worst_enemy: Option<TeamId>,
    /// C++ `CvTeamAI::m_aiWarPlanStateCounter`.
    pub ai_war_plan_state_counter: IdxVec<TeamId, i32>,
    /// C++ `CvTeamAI::m_aiAtWarCounter`.
    pub ai_at_war_counter: IdxVec<TeamId, i32>,
    /// C++ `CvTeamAI::m_aiAtPeaceCounter` (read by CvCity foreign trade route yield).
    pub ai_at_peace_counter: IdxVec<TeamId, i32>,
    /// C++ `CvTeamAI::m_aiHasMetCounter`.
    pub ai_has_met_counter: IdxVec<TeamId, i32>,
    /// C++ `CvTeamAI::m_aiOpenBordersCounter`.
    pub ai_open_borders_counter: IdxVec<TeamId, i32>,
    /// C++ `CvTeamAI::m_aiDefensivePactCounter`.
    pub ai_defensive_pact_counter: IdxVec<TeamId, i32>,
    /// C++ `CvTeamAI::m_aiShareWarCounter`.
    pub ai_share_war_counter: IdxVec<TeamId, i32>,
    /// C++ `CvTeamAI::m_aiWarSuccess` (`AI_get/set/changeWarSuccess`; written by combat, deals).
    pub ai_war_success: IdxVec<TeamId, i32>,
    /// C++ `CvTeamAI::m_aiEnemyPeacetimeTradeValue`.
    pub ai_enemy_peacetime_trade_value: IdxVec<TeamId, i32>,
    /// C++ `CvTeamAI::m_aiEnemyPeacetimeGrantValue`.
    pub ai_enemy_peacetime_grant_value: IdxVec<TeamId, i32>,
    /// C++ `CvTeamAI::m_aeWarPlan` (`AI_getWarPlan`). Write through `team_ai_set_war_plan`.
    pub ai_war_plan: IdxVec<TeamId, Option<WarPlanType>>,
}

impl Team {
    /// C++ `CvTeam::reset(eID, false)` + `CvTeamAI::AI_reset(false)` for this team's own
    /// members. (The other teams' entries about `id` are cleared by `Game::team_init`.)
    pub fn new(id: TeamId, num_teams: usize, d: &GameData) -> Team {
        Team {
            id,
            num_members: 0,
            alive_count: 0,
            ever_alive_count: 0,
            num_cities: 0,
            total_population: 0,
            total_land: 0,
            nuke_interception: 0,
            extra_water_see_from_count: 0,
            map_trading_count: 0,
            tech_trading_count: 0,
            gold_trading_count: 0,
            open_borders_trading_count: 0,
            defensive_pact_trading_count: 0,
            permanent_alliance_trading_count: 0,
            vassal_trading_count: 0,
            bridge_building_count: 0,
            irrigation_count: 0,
            ignore_irrigation_count: 0,
            water_work_count: 0,
            vassal_power: 0,
            master_power: 0,
            enemy_war_weariness_modifier: 0,
            river_trade_count: 0,
            espionage_points_ever: 0,
            map_centering: false,
            capitulated: false,
            barbarian_ally: false,
            no_civic_anger: false,

            stolen_visibility_timer: IdxVec::from_elem(0, num_teams),
            war_weariness: IdxVec::from_elem(0, num_teams),
            tech_share_count: vec![0; num_teams],
            commerce_flexible_count: [0; CommerceType::NUM],
            extra_moves: [0; DomainType::NUM],
            force_team_vote_eligibility_count: IdxVec::from_elem(0, d.vote_sources.len()),

            at_war: IdxVec::from_elem(false, num_teams),
            has_met: IdxVec::from_elem(false, num_teams),
            permanent_war_peace: IdxVec::from_elem(false, num_teams),
            open_borders: IdxVec::from_elem(false, num_teams),
            defensive_pact: IdxVec::from_elem(false, num_teams),
            force_peace: IdxVec::from_elem(false, num_teams),
            vassal: IdxVec::from_elem(false, num_teams),
            can_launch: IdxVec::from_elem(false, d.victories.len()),

            route_change: IdxVec::from_elem(0, d.routes.len()),
            project_count: IdxVec::from_elem(0, d.projects.len()),
            project_making: IdxVec::from_elem(0, d.projects.len()),
            unit_class_count: IdxVec::from_elem(0, d.unit_classes.len()),
            building_class_count: IdxVec::from_elem(0, d.building_classes.len()),
            obsolete_building_count: IdxVec::from_elem(0, d.buildings.len()),
            research_progress: IdxVec::from_elem(0, d.techs.len()),
            tech_count: IdxVec::from_elem(0, d.techs.len()),
            terrain_trade_count: IdxVec::from_elem(0, d.terrains.len()),
            victory_countdown: IdxVec::from_elem(-1, d.victories.len()),

            espionage_points_against_team: IdxVec::from_elem(0, num_teams),
            counterespionage_turns_left_against_team: IdxVec::from_elem(0, num_teams),
            counterespionage_mod_against_team: IdxVec::from_elem(0, num_teams),

            has_tech: IdxVec::from_elem(false, d.techs.len()),
            no_trade_tech: IdxVec::from_elem(false, d.techs.len()),
            improvement_yield_change: IdxVec::from_elem([0; YieldType::NUM], d.improvements.len()),
            revealed_bonuses: Vec::new(),

            ai_worst_enemy: None,
            ai_war_plan_state_counter: IdxVec::from_elem(0, num_teams),
            ai_at_war_counter: IdxVec::from_elem(0, num_teams),
            ai_at_peace_counter: IdxVec::from_elem(0, num_teams),
            ai_has_met_counter: IdxVec::from_elem(0, num_teams),
            ai_open_borders_counter: IdxVec::from_elem(0, num_teams),
            ai_defensive_pact_counter: IdxVec::from_elem(0, num_teams),
            ai_share_war_counter: IdxVec::from_elem(0, num_teams),
            ai_war_success: IdxVec::from_elem(0, num_teams),
            ai_enemy_peacetime_trade_value: IdxVec::from_elem(0, num_teams),
            ai_enemy_peacetime_grant_value: IdxVec::from_elem(0, num_teams),
            ai_war_plan: IdxVec::from_elem(None, num_teams),
        }
    }
}

/// C++ `GC.getGameINLINE().isOption(o)` for the options CvTeam reads. FFH's `CvGame::isOption`
/// locks GAMEOPTION_NO_ESPIONAGE (on), GAMEOPTION_RANDOM_PERSONALITIES and
/// GAMEOPTION_PICK_RELIGION (off); none of those is queried through this helper.
/// Replace with the CvGame port's accessor once it exists.
#[inline]
fn is_option(g: &Game, o: GameOptionType) -> bool {
    debug_assert!(
        o != GameOptionType::GAMEOPTION_NO_ESPIONAGE
            && o != GameOptionType::GAMEOPTION_RANDOM_PERSONALITIES
            && o != GameOptionType::GAMEOPTION_PICK_RELIGION,
        "FFH-locked game option: use CvGame::isOption"
    );
    g.setup.option(o)
}

/// Team ids `0..MAX_CIV_TEAMS` (C++ `for (iI = 0; iI < MAX_CIV_TEAMS; iI++)`), sized to this game.
#[inline]
fn civ_team_ids(g: &Game) -> impl Iterator<Item = TeamId> + 'static {
    (0..g.num_civ_teams()).map(|i| TeamId(i as u8))
}

impl Game {
    /// C++ `CvTeam::init(eID)`: `reset(eID)` (this team back to defaults, and every team's
    /// per-team entries about `eID` cleared) followed by `AI_init()` (= `AI_reset(false)`,
    /// same for the `CvTeamAI` arrays). Also used when a team slot is reused mid-game
    /// (C++ `CvGame::addPlayer`).
    pub fn team_init(&mut self, t: TeamId) {
        let n = self.num_teams();
        self.teams[t] = Team::new(t, n, self.d);
        for o in self.team_ids() {
            let k = &mut self.teams[o];
            // CvTeam::reset loop over MAX_TEAMS (kLoopTeam.m_ai...[getID()]).
            k.stolen_visibility_timer[t] = 0;
            k.war_weariness[t] = 0;
            // C++ quirk: m_aiTechShareCount is indexed by a team *count*, not a team id, but
            // reset clears kLoopTeam.m_aiTechShareCount[getID()] anyway.
            k.tech_share_count[t.idx()] = 0;
            k.espionage_points_against_team[t] = 0;
            k.counterespionage_turns_left_against_team[t] = 0;
            k.counterespionage_mod_against_team[t] = 0;
            k.has_met[t] = false;
            k.at_war[t] = false;
            k.permanent_war_peace[t] = false;
            k.open_borders[t] = false;
            k.defensive_pact[t] = false;
            k.force_peace[t] = false;
            k.vassal[t] = false;
            // CvTeamAI::AI_reset loop over MAX_TEAMS.
            k.ai_war_plan_state_counter[t] = 0;
            k.ai_at_war_counter[t] = 0;
            k.ai_at_peace_counter[t] = 0;
            k.ai_has_met_counter[t] = 0;
            k.ai_open_borders_counter[t] = 0;
            k.ai_defensive_pact_counter[t] = 0;
            k.ai_share_war_counter[t] = 0;
            k.ai_war_success[t] = 0;
            k.ai_enemy_peacetime_trade_value[t] = 0;
            k.ai_enemy_peacetime_grant_value[t] = 0;
            k.ai_war_plan[t] = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_team_matches_cpp_reset() {
        let d = ffh_data::global();
        let t = Team::new(TeamId(2), 5, d);
        assert_eq!(t.id, TeamId(2));
        for v in [&t.at_war, &t.has_met, &t.open_borders, &t.vassal] {
            assert_eq!(v.len(), 5);
            assert!(v.iter().all(|&b| !b));
        }
        assert_eq!(t.tech_share_count, vec![0; 5]);
        assert_eq!(t.has_tech.len(), d.techs.len());
        assert_eq!(t.research_progress.len(), d.techs.len());
        assert_eq!(t.project_count.len(), d.projects.len());
        assert_eq!(t.obsolete_building_count.len(), d.buildings.len());
        assert_eq!(t.improvement_yield_change.len(), d.improvements.len());
        assert_eq!(t.force_team_vote_eligibility_count.len(), d.vote_sources.len());
        // C++ reset: m_aiVictoryCountdown[i] = -1, m_aeWarPlan[i] = NO_WARPLAN, m_eWorstEnemy = NO_TEAM.
        assert_eq!(t.victory_countdown.len(), d.victories.len());
        assert!(t.victory_countdown.iter().all(|&v| v == -1));
        assert!(t.ai_war_plan.iter().all(|p| p.is_none()));
        assert_eq!(t.ai_worst_enemy, None);
        assert_eq!((t.alive_count, t.num_members, t.espionage_points_ever), (0, 0, 0));
        assert!(t.revealed_bonuses.is_empty());
    }
}
