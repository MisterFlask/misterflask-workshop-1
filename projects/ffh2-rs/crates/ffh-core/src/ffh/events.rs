//! Event hooks: port of the rules parts of CvEventManager.py, called where the DLL
//! calls `CvEventReporter::getInstance().<event>(..)`.
//!
//! Call sites in ported rules code must call these at exactly the point the C++
//! calls the reporter (often before the object is destroyed). Bodies that are
//! still empty mean "Python handler not yet ported" (see docs/PORTING.md);
//! handlers that are UI-only in Python stay empty by design.

#![allow(unused_variables)]

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    /// `CvEventReporter::gameEnd` -> `onGameEnd`.
    pub fn ev_game_end(&mut self) {}
    /// `beginGameTurn` -> `onBeginGameTurn`.
    pub fn ev_begin_game_turn(&mut self, game_turn: i32) {}
    /// `endGameTurn` -> `onEndGameTurn`.
    pub fn ev_end_game_turn(&mut self, game_turn: i32) {}
    /// `beginPlayerTurn` -> `onBeginPlayerTurn`.
    pub fn ev_begin_player_turn(&mut self, game_turn: i32, p: PlayerId) {}
    /// `endPlayerTurn` -> `onEndPlayerTurn`.
    pub fn ev_end_player_turn(&mut self, game_turn: i32, p: PlayerId) {}
    /// `firstContact` -> `onFirstContact`.
    pub fn ev_first_contact(&mut self, team1: TeamId, team2: TeamId) {}
    /// `combatResult` -> `onCombatResult`.
    pub fn ev_combat_result(&mut self, winner: UnitId, loser: UnitId) {}
    /// `improvementBuilt` -> `onImprovementBuilt`.
    pub fn ev_improvement_built(&mut self, improvement: ImprovementType, x: i32, y: i32) {}
    /// `improvementDestroyed` -> `onImprovementDestroyed`.
    pub fn ev_improvement_destroyed(&mut self, improvement: ImprovementType, player: Option<PlayerId>, x: i32, y: i32) {}
    /// `routeBuilt` -> `onRouteBuilt`.
    pub fn ev_route_built(&mut self, route: RouteType, x: i32, y: i32) {}
    /// `plotRevealed` -> `onPlotRevealed`.
    pub fn ev_plot_revealed(&mut self, plot: PlotIdx, team: TeamId) {}
    /// `plotFeatureRemoved` -> `onPlotFeatureRemoved`.
    pub fn ev_plot_feature_removed(&mut self, plot: PlotIdx, feature: FeatureType, city: Option<CityId>) {}
    /// `cityBuilt` -> `onCityBuilt`.
    pub fn ev_city_built(&mut self, c: CityId) {}
    /// `cityRazed` -> `onCityRazed` (city still exists when called).
    pub fn ev_city_razed(&mut self, c: CityId, p: PlayerId) {}
    /// `cityAcquired` -> `onCityAcquired`.
    pub fn ev_city_acquired(&mut self, old_owner: PlayerId, p: PlayerId, c: CityId, conquest: bool, trade: bool) {}
    /// `cityAcquiredAndKept` -> `onCityAcquiredAndKept`.
    pub fn ev_city_acquired_and_kept(&mut self, p: PlayerId, c: CityId) {}
    /// `cityLost` -> `onCityLost`.
    pub fn ev_city_lost(&mut self, c: CityId) {}
    /// `cultureExpansion` -> `onCultureExpansion`.
    pub fn ev_culture_expansion(&mut self, c: CityId, p: PlayerId) {}
    /// `cityGrowth` -> `onCityGrowth`.
    pub fn ev_city_growth(&mut self, c: CityId, p: PlayerId) {}
    /// `cityDoTurn` -> `onCityDoTurn`.
    pub fn ev_city_do_turn(&mut self, c: CityId, p: PlayerId) {}
    /// `cityBuildingUnit` -> `onCityBuildingUnit`.
    pub fn ev_city_building_unit(&mut self, c: CityId, unit: UnitType) {}
    /// `cityBuildingBuilding` -> `onCityBuildingBuilding`.
    pub fn ev_city_building_building(&mut self, c: CityId, building: BuildingType) {}
    /// `cityHurry` -> `onCityHurry`.
    pub fn ev_city_hurry(&mut self, c: CityId, hurry: HurryType) {}
    /// `selectionGroupPushMission` -> `onSelectionGroupPushMission`.
    pub fn ev_selection_group_push_mission(&mut self, g: GroupId, mission: MissionType) {}
    /// `unitMove` -> `onUnitMove`.
    pub fn ev_unit_move(&mut self, plot: PlotIdx, u: UnitId, old_plot: Option<PlotIdx>) {}
    /// `unitSetXY` -> `onUnitSetXY`.
    pub fn ev_unit_set_xy(&mut self, plot: PlotIdx, u: UnitId) {}
    /// `unitCreated` -> `onUnitCreated` (USE_ON_UNIT_CREATED_CALLBACK is enabled in FFH2).
    pub fn ev_unit_created(&mut self, u: UnitId) {}
    /// `unitBuilt` -> `onUnitBuilt`.
    pub fn ev_unit_built(&mut self, c: CityId, u: UnitId) {}
    /// `unitKilled` -> `onUnitKilled` (unit still exists when called).
    pub fn ev_unit_killed(&mut self, u: UnitId, attacker: Option<PlayerId>) {}
    /// `unitLost` -> `onUnitLost`.
    pub fn ev_unit_lost(&mut self, u: UnitId) {}
    /// `unitPromoted` -> `onUnitPromoted`.
    pub fn ev_unit_promoted(&mut self, u: UnitId, promotion: PromotionType) {}
    /// `unitPillage` -> `onUnitPillage`.
    pub fn ev_unit_pillage(&mut self, u: UnitId, improvement: Option<ImprovementType>, route: Option<RouteType>, p: PlayerId) {}
    /// `unitSpreadReligionAttempt` -> `onUnitSpreadReligionAttempt`.
    pub fn ev_unit_spread_religion_attempt(&mut self, u: UnitId, religion: ReligionType, success: bool) {}
    /// `unitGifted` -> `onUnitGifted`.
    pub fn ev_unit_gifted(&mut self, u: UnitId, gifting_player: PlayerId, plot: PlotIdx) {}
    /// `unitBuildImprovement` -> `onUnitBuildImprovement`.
    pub fn ev_unit_build_improvement(&mut self, u: UnitId, build: BuildType, finished: bool) {}
    /// `goodyReceived` -> `onGoodyReceived`.
    pub fn ev_goody_received(&mut self, p: PlayerId, plot: PlotIdx, u: Option<UnitId>, goody: GoodyType) {}
    /// `greatPersonBorn` -> `onGreatPersonBorn`.
    pub fn ev_great_person_born(&mut self, u: UnitId, p: PlayerId, c: Option<CityId>) {}
    /// `buildingBuilt` -> `onBuildingBuilt`.
    pub fn ev_building_built(&mut self, c: CityId, building: BuildingType) {}
    /// `projectBuilt` -> `onProjectBuilt`.
    pub fn ev_project_built(&mut self, c: CityId, project: ProjectType) {}
    /// `techAcquired` -> `onTechAcquired`.
    pub fn ev_tech_acquired(&mut self, tech: TechType, team: TeamId, p: Option<PlayerId>, announce: bool) {}
    /// `techSelected` -> `onTechSelected`.
    pub fn ev_tech_selected(&mut self, tech: TechType, p: PlayerId) {}
    /// `religionFounded` -> `onReligionFounded`.
    pub fn ev_religion_founded(&mut self, religion: ReligionType, p: PlayerId) {}
    /// `religionSpread` -> `onReligionSpread`.
    pub fn ev_religion_spread(&mut self, religion: ReligionType, p: PlayerId, c: CityId) {}
    /// `religionRemove` -> `onReligionRemove`.
    pub fn ev_religion_remove(&mut self, religion: ReligionType, p: PlayerId, c: CityId) {}
    /// `corporationFounded` -> `onCorporationFounded`.
    pub fn ev_corporation_founded(&mut self, corp: CorporationType, p: PlayerId) {}
    /// `corporationSpread` -> `onCorporationSpread`.
    pub fn ev_corporation_spread(&mut self, corp: CorporationType, p: PlayerId, c: CityId) {}
    /// `corporationRemove` -> `onCorporationRemove`.
    pub fn ev_corporation_remove(&mut self, corp: CorporationType, p: PlayerId, c: CityId) {}
    /// `goldenAge` -> `onGoldenAge`.
    pub fn ev_golden_age(&mut self, p: PlayerId) {}
    /// `endGoldenAge` -> `onEndGoldenAge`.
    pub fn ev_end_golden_age(&mut self, p: PlayerId) {}
    /// `changeWar` -> `onChangeWar`.
    pub fn ev_change_war(&mut self, war: bool, team: TeamId, other: TeamId) {}
    /// `setPlayerAlive` -> `onSetPlayerAlive`.
    pub fn ev_set_player_alive(&mut self, p: PlayerId, alive: bool) {}
    /// `playerChangeStateReligion` -> `onPlayerChangeStateReligion`.
    pub fn ev_player_change_state_religion(&mut self, p: PlayerId, new: Option<ReligionType>, old: Option<ReligionType>) {}
    /// `playerGoldTrade` -> `onPlayerGoldTrade`.
    pub fn ev_player_gold_trade(&mut self, from: PlayerId, to: PlayerId, amount: i32) {}
    /// `victory` -> `onVictory`.
    pub fn ev_victory(&mut self, winner: TeamId, victory: VictoryType) {}
    /// `vassalState` -> `onVassalState`.
    pub fn ev_vassal_state(&mut self, master: TeamId, vassal: TeamId, is_vassal: bool) {}
}
