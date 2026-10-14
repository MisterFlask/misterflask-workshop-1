//! CvTeam: membership and identity (alive, human, minor, leader, name), aggregates over
//! member players (power, assets, votes, area counts), trading-permission counts,
//! projects, unit/building class limits, victory, espionage, stolen visibility, and team
//! merging on a permanent alliance (`addTeam`, `shareCounters`).

#![allow(unused_variables)]
#![allow(unused_imports)]
use super::{civ_team_ids, is_option};
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Team merging (permanent alliances)
    // ------------------------------------------------------------------

    /// C++ `CvTeam::addTeam`: merge team `other` into `t` (permanent alliance).
    pub fn team_add_team(&mut self, t: TeamId, other: TeamId) {
        todo!("CvTeam::addTeam")
    }

    /// C++ `CvTeam::shareCounters`: take the maximum of `other`'s counters (war weariness,
    /// AI contact counters, war success, ...), and add its project/unit/building class counts
    /// and research progress.
    pub fn team_share_counters(&mut self, t: TeamId, other: TeamId) {
        todo!("CvTeam::shareCounters")
    }

    // ------------------------------------------------------------------
    // Membership and identity
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isAlive` (C++ returns `int`).
    pub fn team_is_alive(&self, t: TeamId) -> bool {
        self.teams[t].alive_count > 0
    }

    /// C++ `CvTeam::changeAliveCount`: when the team dies, its vassals are freed.
    pub fn team_change_alive_count(&mut self, t: TeamId, change: i32) {
        self.teams[t].alive_count += change;

        // free vassals
        if 0 == self.teams[t].alive_count {
            for o in self.team_ids() {
                if o != t {
                    if self.team_is_alive(o) && self.teams[o].vassal[t] {
                        self.team_set_vassal(o, t, false, false);
                    }
                }
            }
        }
    }

    /// C++ `CvTeam::isEverAlive` (C++ returns `int`).
    pub fn team_is_ever_alive(&self, t: TeamId) -> bool {
        self.teams[t].ever_alive_count > 0
    }

    /// C++ `CvTeam::isHuman` (any member player is human).
    pub fn team_is_human(&self, t: TeamId) -> bool {
        todo!("CvTeam::isHuman")
    }

    /// C++ `CvTeam::isBarbarian`.
    pub fn team_is_barbarian(&self, t: TeamId) -> bool {
        t == self.barbarian_team()
    }

    /// C++ `CvTeam::isMinorCiv` (all member players are minor civs).
    pub fn team_is_minor_civ(&self, t: TeamId) -> bool {
        todo!("CvTeam::isMinorCiv")
    }

    /// C++ `CvTeam::getLeaderID`: first alive member, else first member; `None` (C++
    /// `NO_PLAYER`) if the team has no member at all.
    pub fn team_get_leader_id(&self, t: TeamId) -> Option<PlayerId> {
        todo!("CvTeam::getLeaderID")
    }

    /// C++ `CvTeam::getSecretaryID`: first alive human member, else the leader.
    pub fn team_get_secretary_id(&self, t: TeamId) -> Option<PlayerId> {
        todo!("CvTeam::getSecretaryID")
    }

    /// C++ `CvTeam::getHandicapType`: average handicap of the alive members
    /// (STANDARD_HANDICAP if none).
    pub fn team_get_handicap_type(&self, t: TeamId) -> HandicapType {
        todo!("CvTeam::getHandicapType")
    }

    /// C++ `CvTeam::getName`: alive member names joined with "/" (for messages and logs).
    pub fn team_get_name(&self, t: TeamId) -> String {
        todo!("CvTeam::getName")
    }

    /// C++ `CvTeam::setTurnActive` (simultaneous team turns; C++ default `bTurn = true`).
    pub fn team_set_turn_active(&mut self, t: TeamId, value: bool, do_turn: bool) {
        todo!("CvTeam::setTurnActive")
    }

    /// C++ `CvTeam::isTurnActive` (simultaneous team turns).
    pub fn team_is_turn_active(&self, t: TeamId) -> bool {
        todo!("CvTeam::isTurnActive")
    }

    // ------------------------------------------------------------------
    // Aggregates over member players
    // ------------------------------------------------------------------

    /// C++ `CvTeam::getAssets`.
    pub fn team_get_assets(&self, t: TeamId) -> i32 {
        todo!("CvTeam::getAssets")
    }

    /// C++ `CvTeam::getPower`.
    pub fn team_get_power(&self, t: TeamId, include_vassals: bool) -> i32 {
        todo!("CvTeam::getPower")
    }

    /// C++ `CvTeam::getDefensivePower`.
    pub fn team_get_defensive_power(&self, t: TeamId) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) && !self.team_is_a_vassal(o) {
                if t == o || self.teams[t].vassal[o] || self.teams[t].defensive_pact[o] {
                    count += self.team_get_power(o, true);
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getEnemyPower`.
    pub fn team_get_enemy_power(&self, t: TeamId) -> i32 {
        let mut count = 0;

        for o in civ_team_ids(self) {
            if self.team_is_alive(o) {
                if t != o && self.teams[t].at_war[o] {
                    count += self.team_get_power(o, false);
                }
            }
        }

        count
    }

    /// C++ `CvTeam::getNumNukeUnits`.
    pub fn team_get_num_nuke_units(&self, t: TeamId) -> i32 {
        todo!("CvTeam::getNumNukeUnits")
    }

    /// C++ `CvTeam::getTotalPopulation` (C++ default `bCheckVassals = true`).
    pub fn team_get_total_population(&self, t: TeamId, check_vassals: bool) -> i32 {
        let mut vassal_pop = 0;

        if check_vassals {
            if self.team_is_a_vassal(t) {
                return self.teams[t].total_population / 2;
            }

            for o in self.team_ids() {
                if o != t {
                    if self.team_is_alive(o) && self.teams[o].vassal[t] {
                        vassal_pop += self.team_get_total_population(o, false) / 2;
                    }
                }
            }
        }

        self.teams[t].total_population + vassal_pop
    }

    /// C++ `CvTeam::getTotalLand` (C++ default `bCheckVassals = true`).
    pub fn team_get_total_land(&self, t: TeamId, check_vassals: bool) -> i32 {
        let mut vassal_land = 0;

        if check_vassals {
            if self.team_is_a_vassal(t) {
                return self.teams[t].total_land / 2;
            }

            for o in self.team_ids() {
                if o != t {
                    if self.team_is_alive(o) && self.teams[o].vassal[t] {
                        vassal_land += self.team_get_total_land(o, false) / 2;
                    }
                }
            }
        }

        self.teams[t].total_land + vassal_land
    }

    /// C++ `CvTeam::getUnitClassMaking`.
    pub fn team_get_unit_class_making(&self, t: TeamId, unit_class: UnitClassType) -> i32 {
        todo!("CvTeam::getUnitClassMaking")
    }

    /// C++ `CvTeam::getUnitClassCountPlusMaking`.
    pub fn team_get_unit_class_count_plus_making(&self, t: TeamId, unit_class: UnitClassType) -> i32 {
        self.teams[t].unit_class_count[unit_class] + self.team_get_unit_class_making(t, unit_class)
    }

    /// C++ `CvTeam::getBuildingClassMaking`.
    pub fn team_get_building_class_making(&self, t: TeamId, building_class: BuildingClassType) -> i32 {
        todo!("CvTeam::getBuildingClassMaking")
    }

    /// C++ `CvTeam::getBuildingClassCountPlusMaking`.
    pub fn team_get_building_class_count_plus_making(&self, t: TeamId, building_class: BuildingClassType) -> i32 {
        self.teams[t].building_class_count[building_class] + self.team_get_building_class_making(t, building_class)
    }

    /// C++ `CvTeam::getHasReligionCount`.
    pub fn team_get_has_religion_count(&self, t: TeamId, religion: ReligionType) -> i32 {
        todo!("CvTeam::getHasReligionCount")
    }

    /// C++ `CvTeam::getHasCorporationCount`.
    pub fn team_get_has_corporation_count(&self, t: TeamId, corporation: CorporationType) -> i32 {
        todo!("CvTeam::getHasCorporationCount")
    }

    /// C++ `CvTeam::countTotalCulture` (non-const in C++, read-only).
    pub fn team_count_total_culture(&self, t: TeamId) -> i32 {
        todo!("CvTeam::countTotalCulture")
    }

    /// C++ `CvTeam::countNumUnitsByArea`.
    pub fn team_count_num_units_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        todo!("CvTeam::countNumUnitsByArea")
    }

    /// C++ `CvTeam::countNumCitiesByArea`.
    pub fn team_count_num_cities_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        todo!("CvTeam::countNumCitiesByArea")
    }

    /// C++ `CvTeam::countTotalPopulationByArea`.
    pub fn team_count_total_population_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        todo!("CvTeam::countTotalPopulationByArea")
    }

    /// C++ `CvTeam::countPowerByArea`.
    pub fn team_count_power_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        todo!("CvTeam::countPowerByArea")
    }

    /// C++ `CvTeam::countEnemyPowerByArea` (BBAI: counts teams we have any war plan against).
    pub fn team_count_enemy_power_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        todo!("CvTeam::countEnemyPowerByArea")
    }

    /// C++ `CvTeam::countNumAIUnitsByArea`.
    pub fn team_count_num_ai_units_by_area(&self, t: TeamId, area: AreaId, unit_ai: UnitAIType) -> i32 {
        todo!("CvTeam::countNumAIUnitsByArea")
    }

    /// C++ `CvTeam::countEnemyDangerByArea`.
    pub fn team_count_enemy_danger_by_area(&self, t: TeamId, area: AreaId) -> i32 {
        todo!("CvTeam::countEnemyDangerByArea")
    }

    /// C++ `CvTeam::hasHolyCity`.
    pub fn team_has_holy_city(&self, t: TeamId, religion: ReligionType) -> bool {
        todo!("CvTeam::hasHolyCity")
    }

    /// C++ `CvTeam::hasHeadquarters`.
    pub fn team_has_headquarters(&self, t: TeamId, corporation: CorporationType) -> bool {
        todo!("CvTeam::hasHeadquarters")
    }

    /// C++ `CvTeam::hasShrine` (`religion = None` is NO_RELIGION -> false; non-const in C++,
    /// read-only).
    pub fn team_has_shrine(&self, t: TeamId, religion: Option<ReligionType>) -> bool {
        todo!("CvTeam::hasShrine")
    }

    // ------------------------------------------------------------------
    // Votes
    // ------------------------------------------------------------------

    /// C++ `CvTeam::getVotes` (`vote = None` is C++ `NO_VOTE`; FFH counts alive civ players only).
    pub fn team_get_votes(&self, t: TeamId, vote: Option<VoteType>, vote_source: VoteSourceType) -> i32 {
        todo!("CvTeam::getVotes")
    }

    /// C++ `CvTeam::isVotingMember`.
    pub fn team_is_voting_member(&self, t: TeamId, vote_source: VoteSourceType) -> bool {
        self.team_get_votes(t, None, vote_source) > 0
    }

    /// C++ `CvTeam::isFullMember`.
    pub fn team_is_full_member(&self, t: TeamId, vote_source: VoteSourceType) -> bool {
        todo!("CvTeam::isFullMember")
    }

    /// C++ `CvTeam::isForceTeamVoteEligible`.
    pub fn team_is_force_team_vote_eligible(&self, t: TeamId, vote_source: VoteSourceType) -> bool {
        self.teams[t].force_team_vote_eligibility_count[vote_source] > 0 && !self.team_is_minor_civ(t)
    }

    // ------------------------------------------------------------------
    // Trading permissions (counts from techs)
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isMapTrading`.
    pub fn team_is_map_trading(&self, t: TeamId) -> bool {
        self.teams[t].map_trading_count > 0
    }

    /// C++ `CvTeam::isTechTrading`.
    pub fn team_is_tech_trading(&self, t: TeamId) -> bool {
        self.teams[t].tech_trading_count > 0
    }

    /// C++ `CvTeam::isGoldTrading`.
    pub fn team_is_gold_trading(&self, t: TeamId) -> bool {
        self.teams[t].gold_trading_count > 0
    }

    /// C++ `CvTeam::isOpenBordersTrading`.
    pub fn team_is_open_borders_trading(&self, t: TeamId) -> bool {
        self.teams[t].open_borders_trading_count > 0
    }

    /// C++ `CvTeam::isDefensivePactTrading`.
    pub fn team_is_defensive_pact_trading(&self, t: TeamId) -> bool {
        self.teams[t].defensive_pact_trading_count > 0
    }

    /// C++ `CvTeam::isPermanentAllianceTrading` (false unless GAMEOPTION_PERMANENT_ALLIANCES).
    pub fn team_is_permanent_alliance_trading(&self, t: TeamId) -> bool {
        if !is_option(self, GameOptionType::GAMEOPTION_PERMANENT_ALLIANCES) {
            return false;
        }

        self.teams[t].permanent_alliance_trading_count > 0
    }

    /// C++ `CvTeam::isVassalStateTrading` (false under GAMEOPTION_NO_VASSAL_STATES).
    pub fn team_is_vassal_state_trading(&self, t: TeamId) -> bool {
        if is_option(self, GameOptionType::GAMEOPTION_NO_VASSAL_STATES) {
            return false;
        }

        self.teams[t].vassal_trading_count > 0
    }

    /// C++ `CvTeam::isCommerceFlexible`. (`changeCommerceFlexibleCount` is a plain
    /// `commerce_flexible_count[c] += n`; its only side effect is a UI refresh.)
    pub fn team_is_commerce_flexible(&self, t: TeamId, commerce: CommerceType) -> bool {
        self.teams[t].commerce_flexible_count[commerce.idx()] > 0
    }

    // ------------------------------------------------------------------
    // Projects, unit and building classes
    // ------------------------------------------------------------------

    /// C++ `CvTeam::isProjectMaxedOut` (C++ default `iExtra = 0`).
    pub fn team_is_project_maxed_out(&self, t: TeamId, project: ProjectType, extra: i32) -> bool {
        todo!("CvTeam::isProjectMaxedOut")
    }

    /// C++ `CvTeam::changeProjectCount` (project effects; the project art-type bookkeeping
    /// in the C++ is graphics-only and not ported).
    pub fn team_change_project_count(&mut self, t: TeamId, project: ProjectType, change: i32) {
        todo!("CvTeam::changeProjectCount")
    }

    /// C++ `CvTeam::isUnitClassMaxedOut` (C++ default `iExtra = 0`).
    pub fn team_is_unit_class_maxed_out(&self, t: TeamId, unit_class: UnitClassType, extra: i32) -> bool {
        todo!("CvTeam::isUnitClassMaxedOut")
    }

    /// C++ `CvTeam::isBuildingClassMaxedOut` (C++ default `iExtra = 0`).
    pub fn team_is_building_class_maxed_out(&self, t: TeamId, building_class: BuildingClassType, extra: i32) -> bool {
        todo!("CvTeam::isBuildingClassMaxedOut")
    }

    // ------------------------------------------------------------------
    // Victory
    // ------------------------------------------------------------------

    /// C++ `CvTeam::getVictoryDelay`.
    pub fn team_get_victory_delay(&self, t: TeamId, victory: VictoryType) -> i32 {
        todo!("CvTeam::getVictoryDelay")
    }

    /// C++ `CvTeam::getLaunchSuccessRate`.
    pub fn team_get_launch_success_rate(&self, t: TeamId, victory: VictoryType) -> i32 {
        todo!("CvTeam::getLaunchSuccessRate")
    }

    /// C++ `CvTeam::resetVictoryProgress`.
    pub fn team_reset_victory_progress(&mut self, t: TeamId) {
        todo!("CvTeam::resetVictoryProgress")
    }

    // ------------------------------------------------------------------
    // Espionage and stolen visibility
    // ------------------------------------------------------------------

    /// C++ `CvTeam::setEspionagePointsAgainstTeam`.
    pub fn team_set_espionage_points_against_team(&mut self, t: TeamId, other: TeamId, value: i32) {
        if value != self.teams[t].espionage_points_against_team[other] {
            self.teams[t].espionage_points_against_team[other] = value;

            self.team_verify_spy_units_valid_plot(t);
            self.team_verify_spy_units_valid_plot(other);
        }
    }

    /// C++ `CvTeam::changeEspionagePointsAgainstTeam`.
    pub fn team_change_espionage_points_against_team(&mut self, t: TeamId, other: TeamId, change: i32) {
        let v = self.teams[t].espionage_points_against_team[other] + change;
        self.team_set_espionage_points_against_team(t, other, v);
    }

    /// C++ `CvTeam::verifySpyUnitsValidPlot`.
    pub fn team_verify_spy_units_valid_plot(&mut self, t: TeamId) {
        todo!("CvTeam::verifySpyUnitsValidPlot")
    }

    /// C++ `CvTeam::isStolenVisibility`.
    pub fn team_is_stolen_visibility(&self, t: TeamId, other: TeamId) -> bool {
        self.teams[t].stolen_visibility_timer[other] > 0
    }

    /// C++ `CvTeam::setStolenVisibilityTimer` (updates plot stolen-visibility counts).
    pub fn team_set_stolen_visibility_timer(&mut self, t: TeamId, other: TeamId, value: i32) {
        todo!("CvTeam::setStolenVisibilityTimer")
    }

    /// C++ `CvTeam::changeStolenVisibilityTimer`.
    pub fn team_change_stolen_visibility_timer(&mut self, t: TeamId, other: TeamId, change: i32) {
        let v = self.teams[t].stolen_visibility_timer[other] + change;
        self.team_set_stolen_visibility_timer(t, other, v);
    }
}
