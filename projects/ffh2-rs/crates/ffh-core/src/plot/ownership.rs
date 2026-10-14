//! CvPlot, topic "ownership": owner and team, culture and culture ranges, city radius,
//! plot city and working city, AI found values, plot groups and the trade network,
//! visibility counts and revealed state.

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Owner
    // ------------------------------------------------------------------

    /// C++ `CvPlot::getTeam`: the owner's team (None if unowned).
    pub fn plot_get_team(&self, p: PlotIdx) -> Option<TeamId> {
        todo!("CvPlot::getTeam")
    }

    /// C++ `CvPlot::isOwned`.
    #[inline]
    pub fn plot_is_owned(&self, p: PlotIdx) -> bool {
        self.map.plots[p].owner.is_some()
    }

    /// C++ `CvPlot::isBarbarian`.
    #[inline]
    pub fn plot_is_barbarian(&self, p: PlotIdx) -> bool {
        self.map.plots[p].owner == Some(self.barbarian_player())
    }

    /// C++ `CvPlot::setOwner`: city revolts (acquireCity), land/area/plot-group bookkeeping,
    /// sight, revealed owners, goodies and first contact.
    pub fn plot_set_owner(&mut self, p: PlotIdx, new_value: Option<PlayerId>, check_units: bool, update_plot_group: bool) {
        todo!("CvPlot::setOwner")
    }

    /// C++ `CvPlot::isOwnershipScore`.
    #[inline]
    pub fn plot_is_ownership_score(&self, p: PlotIdx) -> bool {
        self.map.plots[p].ownership_duration >= self.d.defines.ownership_score_duration_threshold
    }

    /// C++ `CvPlot::setOwnershipDuration` (updates the owner's scored land).
    pub fn plot_set_ownership_duration(&mut self, p: PlotIdx, new_value: i32) {
        todo!("CvPlot::setOwnershipDuration")
    }

    /// C++ `CvPlot::changeOwnershipDuration`.
    pub fn plot_change_ownership_duration(&mut self, p: PlotIdx, change: i32) {
        let v = self.map.plots[p].ownership_duration + change;
        self.plot_set_ownership_duration(p, v);
    }

    /// C++ `CvPlot::isForceUnowned`.
    #[inline]
    pub fn plot_is_force_unowned(&self, p: PlotIdx) -> bool {
        self.map.plots[p].force_unowned_timer > 0
    }

    /// C++ `CvPlot::isAdjacentOwned`.
    pub fn plot_is_adjacent_owned(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isAdjacentOwned")
    }

    /// C++ `CvPlot::isAdjacentPlayer` (C++ default `bLandOnly = false`).
    pub fn plot_is_adjacent_player(&self, p: PlotIdx, player: PlayerId, land_only: bool) -> bool {
        todo!("CvPlot::isAdjacentPlayer")
    }

    /// C++ `CvPlot::isAdjacentTeam` (C++ default `bLandOnly = false`).
    pub fn plot_is_adjacent_team(&self, p: PlotIdx, team: TeamId, land_only: bool) -> bool {
        todo!("CvPlot::isAdjacentTeam")
    }

    // ------------------------------------------------------------------
    // Culture
    // ------------------------------------------------------------------

    /// C++ `CvPlot::calculateCulturalOwner`.
    pub fn plot_calculate_cultural_owner(&self, p: PlotIdx) -> Option<PlayerId> {
        todo!("CvPlot::calculateCulturalOwner")
    }

    /// C++ `CvPlot::updateCulture`: non-city plots go to their cultural owner.
    pub fn plot_update_culture(&mut self, p: PlotIdx, bump_units: bool, update_plot_groups: bool) {
        todo!("CvPlot::updateCulture")
    }

    /// C++ `CvPlot::doCulture` (protected): city revolts and culture flips, then `updateCulture`.
    pub fn plot_do_culture(&mut self, p: PlotIdx) {
        todo!("CvPlot::doCulture")
    }

    /// C++ `CvPlot::countTotalCulture` (alive players).
    pub fn plot_count_total_culture(&self, p: PlotIdx) -> i32 {
        todo!("CvPlot::countTotalCulture")
    }

    /// C++ `CvPlot::countFriendlyCulture`: culture of `team`, its vassals and open-border partners.
    pub fn plot_count_friendly_culture(&self, p: PlotIdx, team: TeamId) -> i32 {
        todo!("CvPlot::countFriendlyCulture")
    }

    /// C++ `CvPlot::findHighestCultureTeam`.
    pub fn plot_find_highest_culture_team(&self, p: PlotIdx) -> Option<TeamId> {
        todo!("CvPlot::findHighestCultureTeam")
    }

    /// C++ `CvPlot::findHighestCulturePlayer`.
    pub fn plot_find_highest_culture_player(&self, p: PlotIdx) -> Option<PlayerId> {
        todo!("CvPlot::findHighestCulturePlayer")
    }

    /// C++ `CvPlot::calculateCulturePercent`.
    pub fn plot_calculate_culture_percent(&self, p: PlotIdx, player: PlayerId) -> i32 {
        todo!("CvPlot::calculateCulturePercent")
    }

    /// C++ `CvPlot::calculateTeamCulturePercent`.
    pub fn plot_calculate_team_culture_percent(&self, p: PlotIdx, team: TeamId) -> i32 {
        todo!("CvPlot::calculateTeamCulturePercent")
    }

    /// C++ `CvPlot::setCulture` (`getCulture(e)` is the `culture` field).
    pub fn plot_set_culture(&mut self, p: PlotIdx, player: PlayerId, new_value: i32, update: bool, update_plot_groups: bool) {
        todo!("CvPlot::setCulture")
    }

    /// C++ `CvPlot::changeCulture`.
    pub fn plot_change_culture(&mut self, p: PlotIdx, player: PlayerId, change: i32, update: bool) {
        if change != 0 {
            let v = self.map.plots[p].culture[player] + change;
            self.plot_set_culture(p, player, v, update, true);
        }
    }

    /// C++ `CvPlot::isWithinCultureRange`.
    pub fn plot_is_within_culture_range(&self, p: PlotIdx, player: PlayerId) -> bool {
        self.map.plots[p].culture_range_cities[player].iter().any(|&n| n > 0)
    }

    /// C++ `CvPlot::getNumCultureRangeCities`.
    pub fn plot_get_num_culture_range_cities(&self, p: PlotIdx, player: PlayerId) -> i32 {
        self.map.plots[p].culture_range_cities[player].iter().sum()
    }

    /// C++ `CvPlot::isCultureRangeCity`.
    #[inline]
    pub fn plot_is_culture_range_city(&self, p: PlotIdx, player: PlayerId, range: CultureLevelType) -> bool {
        self.map.plots[p].culture_range_cities[player][range] > 0
    }

    /// C++ `CvPlot::changeCultureRangeCities`.
    pub fn plot_change_culture_range_cities(&mut self, p: PlotIdx, player: PlayerId, range: CultureLevelType, change: i32, update_plot_groups: bool) {
        todo!("CvPlot::changeCultureRangeCities")
    }

    // ------------------------------------------------------------------
    // Cities
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isCityRadius` (C++ returns int).
    #[inline]
    pub fn plot_is_city_radius(&self, p: PlotIdx) -> bool {
        self.map.plots[p].city_radius_count > 0
    }

    /// C++ `CvPlot::isPlayerCityRadius`.
    #[inline]
    pub fn plot_is_player_city_radius(&self, p: PlotIdx, player: PlayerId) -> bool {
        self.map.plots[p].player_city_radius_count[player] > 0
    }

    /// C++ `CvPlot::isWithinTeamCityRadius` (C++ default `eIgnorePlayer = NO_PLAYER`).
    pub fn plot_is_within_team_city_radius(&self, p: PlotIdx, team: TeamId, ignore_player: Option<PlayerId>) -> bool {
        todo!("CvPlot::isWithinTeamCityRadius")
    }

    /// C++ `CvPlot::isCity` (C++ defaults `bCheckImprovement = false`, `eForTeam = NO_TEAM`):
    /// a city, or (`check_improvement`) an improvement that acts as a city for `for_team`.
    pub fn plot_is_city(&self, p: PlotIdx, check_improvement: bool, for_team: Option<TeamId>) -> bool {
        todo!("CvPlot::isCity")
    }

    /// C++ `CvPlot::setPlotCity` (`getPlotCity()` is the `plot_city` field).
    pub fn plot_set_plot_city(&mut self, p: PlotIdx, city: Option<CityId>) {
        todo!("CvPlot::setPlotCity")
    }

    /// C++ `CvPlot::isOccupation`.
    pub fn plot_is_occupation(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isOccupation")
    }

    /// C++ `CvPlot::isBeingWorked`.
    pub fn plot_is_being_worked(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isBeingWorked")
    }

    /// C++ `CvPlot::updateWorkingCity` (`getWorkingCity()` is the `working_city` field).
    pub fn plot_update_working_city(&mut self, p: PlotIdx) {
        todo!("CvPlot::updateWorkingCity")
    }

    /// C++ `CvPlot::setWorkingCityOverride`.
    pub fn plot_set_working_city_override(&mut self, p: PlotIdx, city: Option<CityId>) {
        todo!("CvPlot::setWorkingCityOverride")
    }

    /// C++ `CvPlot::getFoundValue`: cached `AI_foundValue` (or the `getCityFoundValue`
    /// Python callback) when the cache holds -1; also raises the area's best found value.
    /// Used by starting-plot selection. Non-const in C++ (fills the cache).
    pub fn plot_get_found_value(&mut self, p: PlotIdx, player: PlayerId) -> i32 {
        todo!("CvPlot::getFoundValue")
    }

    /// C++ `CvPlot::isBestAdjacentFound` (non-const in C++ but read-only; UI action recommendation).
    pub fn plot_is_best_adjacent_found(&self, p: PlotIdx, player: PlayerId) -> bool {
        todo!("CvPlot::isBestAdjacentFound")
    }

    // ------------------------------------------------------------------
    // Plot groups and the trade network
    // ------------------------------------------------------------------

    /// C++ `CvPlot::getPlotGroup`: `player`'s plot group containing this plot (ids of deleted
    /// groups read as None, like C++ `CvPlayer::getPlotGroup`).
    #[inline]
    pub fn plot_get_plot_group(&self, p: PlotIdx, player: PlayerId) -> Option<PlotGroupId> {
        self.map.plots[p].plot_group[player].filter(|&g| self.plot_groups.contains(g))
    }

    /// C++ `CvPlot::getOwnerPlotGroup`.
    pub fn plot_get_owner_plot_group(&self, p: PlotIdx) -> Option<PlotGroupId> {
        let owner = self.map.plots[p].owner?;
        self.plot_get_plot_group(p, owner)
    }

    /// C++ `CvPlot::setPlotGroup` (moves the plot city's bonuses between groups).
    pub fn plot_set_plot_group(&mut self, p: PlotIdx, player: PlayerId, group: Option<PlotGroupId>) {
        todo!("CvPlot::setPlotGroup")
    }

    /// C++ `CvPlot::updatePlotGroup()`: all alive players.
    pub fn plot_update_plot_group(&mut self, p: PlotIdx) {
        todo!("CvPlot::updatePlotGroup()")
    }

    /// C++ `CvPlot::updatePlotGroup(PlayerTypes, bool)` (C++ default `bRecalculate = true`).
    pub fn plot_update_plot_group_player(&mut self, p: PlotIdx, player: PlayerId, recalculate: bool) {
        todo!("CvPlot::updatePlotGroup(PlayerTypes, bool)")
    }

    /// C++ `CvPlot::updatePlotGroupBonus`: adds/removes this plot's bonuses (city free bonuses,
    /// capital imports/exports, the plot bonus) to/from the owner's plot group.
    pub fn plot_update_plot_group_bonus(&mut self, p: PlotIdx, add: bool) {
        todo!("CvPlot::updatePlotGroupBonus")
    }

    /// C++ `CvPlot::isConnectedTo`.
    pub fn plot_is_connected_to(&self, p: PlotIdx, city: CityId) -> bool {
        todo!("CvPlot::isConnectedTo")
    }

    /// C++ `CvPlot::isConnectedToCapital` (C++ default `ePlayer = NO_PLAYER` = owner).
    pub fn plot_is_connected_to_capital(&self, p: PlotIdx, player: Option<PlayerId>) -> bool {
        todo!("CvPlot::isConnectedToCapital")
    }

    /// C++ `CvPlot::getPlotGroupConnectedBonus`.
    pub fn plot_get_plot_group_connected_bonus(&self, p: PlotIdx, player: PlayerId, bonus: BonusType) -> i32 {
        match self.plot_get_plot_group(p, player) {
            Some(g) => self.plot_groups[g].num_bonuses[bonus],
            None => 0,
        }
    }

    /// C++ `CvPlot::isPlotGroupConnectedBonus`.
    #[inline]
    pub fn plot_is_plot_group_connected_bonus(&self, p: PlotIdx, player: PlayerId, bonus: BonusType) -> bool {
        self.plot_get_plot_group_connected_bonus(p, player, bonus) > 0
    }

    /// C++ `CvPlot::isAdjacentPlotGroupConnectedBonus`.
    pub fn plot_is_adjacent_plot_group_connected_bonus(&self, p: PlotIdx, player: PlayerId, bonus: BonusType) -> bool {
        todo!("CvPlot::isAdjacentPlotGroupConnectedBonus")
    }

    /// C++ `CvPlot::isTradeNetworkImpassable`.
    pub fn plot_is_trade_network_impassable(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isTradeNetworkImpassable")
    }

    /// C++ `CvPlot::isRiverNetwork`.
    pub fn plot_is_river_network(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isRiverNetwork")
    }

    /// C++ `CvPlot::isNetworkTerrain`.
    pub fn plot_is_network_terrain(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isNetworkTerrain")
    }

    /// C++ `CvPlot::isBonusNetwork`.
    pub fn plot_is_bonus_network(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isBonusNetwork")
    }

    /// C++ `CvPlot::isTradeNetwork`.
    pub fn plot_is_trade_network(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isTradeNetwork")
    }

    /// C++ `CvPlot::isTradeNetworkConnected`.
    pub fn plot_is_trade_network_connected(&self, p: PlotIdx, other: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isTradeNetworkConnected")
    }

    /// C++ `CvPlot::changeBlockadedCount` (`getBlockadedCount` is the `blockaded_count` field).
    pub fn plot_change_blockaded_count(&mut self, p: PlotIdx, team: TeamId, change: i32) {
        todo!("CvPlot::changeBlockadedCount")
    }

    // ------------------------------------------------------------------
    // Visibility and revealed state
    // (`bDebug` arguments are dropped: debug mode is a UI feature and never on in rules;
    // `getVisibilityCount`, `isRevealed`, `getRevealedOwner`, `getRevealedImprovementType`,
    // `getRevealedRouteType` and their UI-only setters are fields.)
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isVisible` (`bDebug` dropped).
    #[inline]
    pub fn plot_is_visible(&self, p: PlotIdx, team: TeamId) -> bool {
        let pl = &self.map.plots[p];
        pl.visibility_count[team] > 0 || pl.stolen_visibility_count[team] > 0
    }

    /// C++ `CvPlot::isVisibleToCivTeam`: visible to any alive non-barbarian team.
    pub fn plot_is_visible_to_civ_team(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isVisibleToCivTeam")
    }

    /// C++ `CvPlot::isVisibleToWatchingHuman`.
    pub fn plot_is_visible_to_watching_human(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isVisibleToWatchingHuman")
    }

    /// C++ `CvPlot::isAdjacentVisible` (`bDebug` dropped).
    pub fn plot_is_adjacent_visible(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isAdjacentVisible")
    }

    /// C++ `CvPlot::isAdjacentNonvisible`.
    pub fn plot_is_adjacent_nonvisible(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isAdjacentNonvisible")
    }

    /// C++ `CvPlot::changeVisibilityCount`: reveals on becoming visible, meets the owner,
    /// propagates stolen visibility.
    pub fn plot_change_visibility_count(&mut self, p: PlotIdx, team: TeamId, change: i32, see_invisible: Option<InvisibleType>, update_plot_groups: bool) {
        todo!("CvPlot::changeVisibilityCount")
    }

    /// C++ `CvPlot::changeStolenVisibilityCount`.
    pub fn plot_change_stolen_visibility_count(&mut self, p: PlotIdx, team: TeamId, change: i32) {
        todo!("CvPlot::changeStolenVisibilityCount")
    }

    /// C++ `CvPlot::isInvisibleVisible`.
    #[inline]
    pub fn plot_is_invisible_visible(&self, p: PlotIdx, team: TeamId, invisible: InvisibleType) -> bool {
        self.map.plots[p].invisible_visibility_count[team][invisible] > 0
    }

    /// C++ `CvPlot::getRevealedTeam` (`bDebug` dropped).
    pub fn plot_get_revealed_team(&self, p: PlotIdx, team: TeamId) -> Option<TeamId> {
        todo!("CvPlot::getRevealedTeam")
    }

    /// C++ `CvPlot::updateRevealedOwner`.
    pub fn plot_update_revealed_owner(&mut self, p: PlotIdx, team: TeamId) {
        todo!("CvPlot::updateRevealedOwner")
    }

    /// C++ `CvPlot::setRevealed`.
    pub fn plot_set_revealed(&mut self, p: PlotIdx, team: TeamId, new_value: bool, terrain_only: bool, from_team: Option<TeamId>, update_plot_group: bool) {
        todo!("CvPlot::setRevealed")
    }

    /// C++ `CvPlot::isAdjacentRevealed`.
    pub fn plot_is_adjacent_revealed(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isAdjacentRevealed")
    }

    /// C++ `CvPlot::isAdjacentNonrevealed`.
    pub fn plot_is_adjacent_nonrevealed(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isAdjacentNonrevealed")
    }
}
