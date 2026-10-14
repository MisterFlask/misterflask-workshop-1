//! CvPlayer: diplomacy and trade -- contact, diplomacy event consequences, tradeable
//! items and denials, deals, bonuses (availability, export/import), trade routes with
//! other players, forced peace, espionage (spending, missions, spy checks), and the
//! CvPlayerAI peacetime trade/grant bookkeeping that deals update.

#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

/// C++ `DiploEventTypes` (CvEnums.h; not generated since no info table uses it).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DiploEvent {
    /// C++ `DIPLOEVENT_CONTACT`
    Contact,
    /// C++ `DIPLOEVENT_AI_CONTACT`
    AiContact,
    /// C++ `DIPLOEVENT_FAILED_CONTACT`
    FailedContact,
    /// C++ `DIPLOEVENT_GIVE_HELP`
    GiveHelp,
    /// C++ `DIPLOEVENT_REFUSED_HELP`
    RefusedHelp,
    /// C++ `DIPLOEVENT_ACCEPT_DEMAND`
    AcceptDemand,
    /// C++ `DIPLOEVENT_REJECTED_DEMAND`
    RejectedDemand,
    /// C++ `DIPLOEVENT_DEMAND_WAR`
    DemandWar,
    /// C++ `DIPLOEVENT_CONVERT`
    Convert,
    /// C++ `DIPLOEVENT_NO_CONVERT`
    NoConvert,
    /// C++ `DIPLOEVENT_REVOLUTION`
    Revolution,
    /// C++ `DIPLOEVENT_NO_REVOLUTION`
    NoRevolution,
    /// C++ `DIPLOEVENT_JOIN_WAR` (data1 = target team)
    JoinWar,
    /// C++ `DIPLOEVENT_NO_JOIN_WAR`
    NoJoinWar,
    /// C++ `DIPLOEVENT_STOP_TRADING` (data1 = target team)
    StopTrading,
    /// C++ `DIPLOEVENT_NO_STOP_TRADING`
    NoStopTrading,
    /// C++ `DIPLOEVENT_ASK_HELP`
    AskHelp,
    /// C++ `DIPLOEVENT_MADE_DEMAND`
    MadeDemand,
    /// C++ `DIPLOEVENT_RESEARCH_TECH` (data1 = tech)
    ResearchTech,
    /// C++ `DIPLOEVENT_TARGET_CITY` (data1 = city owner, data2 = city id)
    TargetCity,
    /// C++ `DIPLOEVENT_MADE_DEMAND_VASSAL`
    MadeDemandVassal,
}

impl Game {
    // ------------------------------------------------------------------ contact and diplomacy

    /// C++ `CvPlayer::canContact(ePlayer)`.
    pub fn player_can_contact(&self, p: PlayerId, other: PlayerId) -> bool {
        todo!("CvPlayer::canContact")
    }

    /// C++ `CvPlayer::handleDiploEvent(eDiploEvent, ePlayer, iData1, iData2)`: rules
    /// consequences of a diplomacy exchange between `p` (the AI side) and `other`
    /// (memories, first contact, forced peace, war, conversion, revolution, embargo, ...).
    pub fn player_handle_diplo_event(&mut self, p: PlayerId, event: DiploEvent, other: PlayerId, data1: i32, data2: i32) {
        todo!("CvPlayer::handleDiploEvent")
    }

    /// C++ `CvPlayer::isNoDiplomacyWithEnemies` (FFH).
    pub fn player_is_no_diplomacy_with_enemies(&self, p: PlayerId) -> bool {
        self.players[p].no_diplomacy_with_enemies != 0
    }

    /// C++ `CvPlayer::forcePeace(ePlayer)`: implements a peace treaty deal (unless a vassal).
    pub fn player_force_peace(&mut self, p: PlayerId, other: PlayerId) {
        todo!("CvPlayer::forcePeace")
    }

    // ------------------------------------------------------------------ trade items and deals

    /// C++ `CvPlayer::canTradeWith(eWhoTo)`.
    pub fn player_can_trade_with(&self, p: PlayerId, who_to: PlayerId) -> bool {
        todo!("CvPlayer::canTradeWith")
    }

    /// C++ `CvPlayer::canReceiveTradeCity` (not for humans in One City Challenge).
    pub fn player_can_receive_trade_city(&self, p: PlayerId) -> bool {
        todo!("CvPlayer::canReceiveTradeCity")
    }

    /// C++ `CvPlayer::canTradeItem(eWhoTo, item, bTestDenial = false)`.
    pub fn player_can_trade_item(&self, p: PlayerId, who_to: PlayerId, item: &TradeData, test_denial: bool) -> bool {
        todo!("CvPlayer::canTradeItem")
    }

    /// C++ `CvPlayer::getTradeDenial(eWhoTo, item)` (None = NO_DENIAL). The denial reasons
    /// are AI decisions (`AI_*Trade`), to be answered through the controller.
    pub fn player_get_trade_denial(&self, p: PlayerId, who_to: PlayerId, item: &TradeData) -> Option<DenialType> {
        todo!("CvPlayer::getTradeDenial")
    }

    /// C++ `CvPlayer::isTradingWithTeam(eTeam, bIncludeCancelable)`.
    pub fn player_is_trading_with_team(&self, p: PlayerId, team: TeamId, include_cancelable: bool) -> bool {
        todo!("CvPlayer::isTradingWithTeam")
    }

    /// C++ `CvPlayer::canStopTradingWithTeam(eTeam, bContinueNotTrading = false)`.
    pub fn player_can_stop_trading_with_team(&self, p: PlayerId, team: TeamId, continue_not_trading: bool) -> bool {
        todo!("CvPlayer::canStopTradingWithTeam")
    }

    /// C++ `CvPlayer::stopTradingWithTeam(eTeam)`: cancels deals and records the memories.
    pub fn player_stop_trading_with_team(&mut self, p: PlayerId, team: TeamId) {
        todo!("CvPlayer::stopTradingWithTeam")
    }

    /// C++ `CvPlayer::killAllDeals`.
    pub fn player_kill_all_deals(&mut self, p: PlayerId) {
        todo!("CvPlayer::killAllDeals")
    }

    // ------------------------------------------------------------------ bonuses

    /// C++ `CvPlayer::canTradeNetworkWith(ePlayer)` (capitals connected).
    pub fn player_can_trade_network_with(&self, p: PlayerId, other: PlayerId) -> bool {
        todo!("CvPlayer::canTradeNetworkWith")
    }

    /// C++ `CvPlayer::getNumAvailableBonuses(eBonus)` (capital plot group; FFH: banned by the
    /// first vote source's `isNoBonus` for full members).
    pub fn player_get_num_available_bonuses(&self, p: PlayerId, bonus: BonusType) -> i32 {
        todo!("CvPlayer::getNumAvailableBonuses")
    }

    /// C++ `CvPlayer::getNumTradeableBonuses(eBonus)`.
    pub fn player_get_num_tradeable_bonuses(&self, p: PlayerId, bonus: BonusType) -> i32 {
        self.player_get_num_available_bonuses(p, bonus) - self.players[p].bonus_import[bonus]
    }

    /// C++ `CvPlayer::hasBonus(eBonus)` (any city has it).
    pub fn player_has_bonus(&self, p: PlayerId, bonus: BonusType) -> bool {
        todo!("CvPlayer::hasBonus")
    }

    /// C++ `CvPlayer::getNumTradeBonusImports(ePlayer)` (resources traded from `other` in deals).
    pub fn player_get_num_trade_bonus_imports(&self, p: PlayerId, other: PlayerId) -> i32 {
        todo!("CvPlayer::getNumTradeBonusImports")
    }

    /// C++ `CvPlayer::countOwnedBonuses(eBonus)`.
    pub fn player_count_owned_bonuses(&self, p: PlayerId, bonus: BonusType) -> i32 {
        todo!("CvPlayer::countOwnedBonuses")
    }

    /// C++ `CvPlayer::countUnimprovedBonuses(pArea, pFromPlot = NULL)`.
    pub fn player_count_unimproved_bonuses(&self, p: PlayerId, area: AreaId, from_plot: Option<PlotIdx>) -> i32 {
        todo!("CvPlayer::countUnimprovedBonuses")
    }

    /// C++ `CvPlayer::changeBonusExport` (capital plot group bonuses are recounted).
    pub fn player_change_bonus_export(&mut self, p: PlayerId, bonus: BonusType, change: i32) {
        todo!("CvPlayer::changeBonusExport")
    }

    /// C++ `CvPlayer::changeBonusImport` (capital plot group bonuses are recounted).
    pub fn player_change_bonus_import(&mut self, p: PlayerId, bonus: BonusType, change: i32) {
        todo!("CvPlayer::changeBonusImport")
    }

    // ------------------------------------------------------------------ foreign trade routes

    /// C++ `CvPlayer::countPotentialForeignTradeCities(pIgnoreArea = NULL)`.
    pub fn player_count_potential_foreign_trade_cities(&self, p: PlayerId, ignore_area: Option<AreaId>) -> i32 {
        todo!("CvPlayer::countPotentialForeignTradeCities")
    }

    /// C++ `CvPlayer::countPotentialForeignTradeCitiesConnected`.
    pub fn player_count_potential_foreign_trade_cities_connected(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::countPotentialForeignTradeCitiesConnected")
    }

    /// C++ `CvPlayer::canHaveTradeRoutesWith(ePlayer)`.
    pub fn player_can_have_trade_routes_with(&self, p: PlayerId, other: PlayerId) -> bool {
        todo!("CvPlayer::canHaveTradeRoutesWith")
    }

    // ------------------------------------------------------------------ espionage

    /// C++ `CvPlayer::doEspionagePoints`: splits espionage commerce among met teams.
    pub fn player_do_espionage_points(&mut self, p: PlayerId) {
        todo!("CvPlayer::doEspionagePoints")
    }

    /// C++ `CvPlayer::getEspionageSpending(eAgainstTeam)` (-1 if `against` is not a met team).
    pub fn player_get_espionage_spending(&self, p: PlayerId, against: TeamId) -> i32 {
        todo!("CvPlayer::getEspionageSpending")
    }

    /// C++ `CvPlayer::setEspionageSpendingWeightAgainstTeam` (clamped to 0..99).
    pub fn player_set_espionage_spending_weight_against_team(&mut self, p: PlayerId, team: TeamId, value: i32) {
        debug_assert!(value >= 0);
        let value = 0.max(value).min(99);
        self.players[p].espionage_spending_weight_against_team[team] = value;
    }

    /// C++ `CvPlayer::changeEspionageSpendingWeightAgainstTeam`.
    pub fn player_change_espionage_spending_weight_against_team(&mut self, p: PlayerId, team: TeamId, change: i32) {
        let value = self.players[p].espionage_spending_weight_against_team[team] + change;
        self.player_set_espionage_spending_weight_against_team(p, team, value);
    }

    /// C++ `CvPlayer::canDoEspionageMission(eMission, eTargetPlayer, pPlot, iExtraData, pUnit)`.
    pub fn player_can_do_espionage_mission(&self, p: PlayerId, mission: EspionageMissionType, target: Option<PlayerId>, plot: Option<PlotIdx>, extra_data: i32, unit: Option<UnitId>) -> bool {
        todo!("CvPlayer::canDoEspionageMission")
    }

    /// C++ `CvPlayer::getEspionageMissionBaseCost(eMission, eTargetPlayer, pPlot, iExtraData, pSpyUnit)`
    /// (-1 = mission not possible).
    pub fn player_get_espionage_mission_base_cost(&self, p: PlayerId, mission: EspionageMissionType, target: Option<PlayerId>, plot: Option<PlotIdx>, extra_data: i32, spy_unit: Option<UnitId>) -> i32 {
        todo!("CvPlayer::getEspionageMissionBaseCost")
    }

    /// C++ `CvPlayer::getEspionageMissionCost(eMission, eTargetPlayer, pPlot = NULL, iExtraData = -1, pSpyUnit = NULL)`
    /// (-1 = mission not possible).
    pub fn player_get_espionage_mission_cost(&self, p: PlayerId, mission: EspionageMissionType, target: Option<PlayerId>, plot: Option<PlotIdx>, extra_data: i32, spy_unit: Option<UnitId>) -> i32 {
        todo!("CvPlayer::getEspionageMissionCost")
    }

    /// C++ `CvPlayer::getEspionageMissionCostModifier(eMission, eTargetPlayer, pPlot = NULL, iExtraData = -1, pSpyUnit = NULL)`.
    pub fn player_get_espionage_mission_cost_modifier(&self, p: PlayerId, mission: EspionageMissionType, target: Option<PlayerId>, plot: Option<PlotIdx>, extra_data: i32, spy_unit: Option<UnitId>) -> i32 {
        todo!("CvPlayer::getEspionageMissionCostModifier")
    }

    /// C++ `CvPlayer::doEspionageMission(eMission, eTargetPlayer, pPlot, iExtraData, pSpyUnit)`:
    /// performs the mission; true if something happened.
    pub fn player_do_espionage_mission(&mut self, p: PlayerId, mission: EspionageMissionType, target: Option<PlayerId>, plot: Option<PlotIdx>, extra_data: i32, spy_unit: Option<UnitId>) -> bool {
        todo!("CvPlayer::doEspionageMission")
    }

    /// C++ `CvPlayer::canStealTech(eTarget, eTech)`.
    pub fn player_can_steal_tech(&self, p: PlayerId, target: PlayerId, tech: TechType) -> bool {
        todo!("CvPlayer::canStealTech")
    }

    /// C++ `CvPlayer::canForceCivics(eTarget, eCivic)`.
    pub fn player_can_force_civics(&self, p: PlayerId, target: PlayerId, civic: CivicType) -> bool {
        self.player_can_do_civics(target, Some(civic)) && !self.player_is_civic(target, civic) && self.player_is_civic(p, civic)
    }

    /// C++ `CvPlayer::canForceReligion(eTarget, eReligion)`.
    pub fn player_can_force_religion(&self, p: PlayerId, target: PlayerId, religion: ReligionType) -> bool {
        self.player_can_do_religion(target, religion)
            && self.player_get_state_religion(target) != Some(religion)
            && self.player_get_state_religion(p) == Some(religion)
    }

    /// C++ `CvPlayer::canSpyDestroyUnit(eTarget, kUnit)`.
    pub fn player_can_spy_destroy_unit(&self, p: PlayerId, target: PlayerId, unit: UnitId) -> bool {
        todo!("CvPlayer::canSpyDestroyUnit")
    }

    /// C++ `CvPlayer::canSpyBribeUnit(eTarget, kUnit)`.
    pub fn player_can_spy_bribe_unit(&self, p: PlayerId, target: PlayerId, unit: UnitId) -> bool {
        todo!("CvPlayer::canSpyBribeUnit")
    }

    /// C++ `CvPlayer::canSpyDestroyBuilding(eTarget, eBuilding)`.
    pub fn player_can_spy_destroy_building(&self, p: PlayerId, target: PlayerId, building: BuildingType) -> bool {
        todo!("CvPlayer::canSpyDestroyBuilding")
    }

    /// C++ `CvPlayer::canSpyDestroyProject(eTarget, eProject)`.
    pub fn player_can_spy_destroy_project(&self, p: PlayerId, target: PlayerId, project: ProjectType) -> bool {
        todo!("CvPlayer::canSpyDestroyProject")
    }

    /// C++ `CvPlayer::canSpiesEnterBorders(ePlayer)`.
    pub fn player_can_spies_enter_borders(&self, p: PlayerId, other: PlayerId) -> bool {
        todo!("CvPlayer::canSpiesEnterBorders")
    }

    // ------------------------------------------------------------------ CvPlayerAI bookkeeping used by deals

    /// C++ `CvPlayerAI::AI_changePeacetimeTradeValue(eIndex, iChange)`: also raises the
    /// grudge (`AI_changeEnemyPeacetimeTradeValue`) of teams whose worst enemy is `p`'s team
    /// and who have met `other`'s team.
    pub fn player_ai_change_peacetime_trade_value(&mut self, p: PlayerId, other: PlayerId, change: i32) {
        todo!("CvPlayerAI::AI_changePeacetimeTradeValue")
    }

    /// C++ `CvPlayerAI::AI_changePeacetimeGrantValue(eIndex, iChange)` (same team side effect
    /// with `AI_changeEnemyPeacetimeGrantValue`).
    pub fn player_ai_change_peacetime_grant_value(&mut self, p: PlayerId, other: PlayerId, change: i32) {
        todo!("CvPlayerAI::AI_changePeacetimeGrantValue")
    }
}
