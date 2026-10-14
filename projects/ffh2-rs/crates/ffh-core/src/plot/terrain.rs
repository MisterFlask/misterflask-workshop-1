//! CvPlot, topic "terrain": terrain/feature/bonus/improvement/route changes and validity,
//! builds, goodies, turn processing (`doTurn`, `doFeature`, `doImprovement`), FFH plot
//! counter and temporary terrain, pirate coves.
//!
//! (`setPlotType` lives in `geography.rs` with the area code it drives.)

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Turn processing
    // ------------------------------------------------------------------

    /// C++ `CvPlot::doTurn`: timers, improvement duration/upgrade, FFH lair spawns and
    /// feature upgrades, `doFeature`, `doCulture`, `verifyUnitValidPlot`, FFH temp terrain expiry.
    pub fn plot_do_turn(&mut self, p: PlotIdx) {
        todo!("CvPlot::doTurn")
    }

    /// C++ `CvPlot::doImprovement` (worked plots): bonus discovery, then `doImprovementUpgrade`.
    pub fn plot_do_improvement(&mut self, p: PlotIdx) {
        todo!("CvPlot::doImprovement")
    }

    /// C++ `CvPlot::doImprovementUpgrade` (protected).
    pub fn plot_do_improvement_upgrade(&mut self, p: PlotIdx) {
        todo!("CvPlot::doImprovementUpgrade")
    }

    /// C++ `CvPlot::doFeature` (protected): FFH feature upgrades and flames, feature
    /// disappearance and growth.
    pub fn plot_do_feature(&mut self, p: PlotIdx) {
        todo!("CvPlot::doFeature")
    }

    /// C++ `CvPlot::erase`: kills the plot's units and city, clears bonus, improvement, route,
    /// feature, FFH temporary terrain and rivers.
    pub fn plot_erase(&mut self, p: PlotIdx) {
        todo!("CvPlot::erase")
    }

    // ------------------------------------------------------------------
    // Plot type and terrain
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isWater`.
    #[inline]
    pub fn plot_is_water(&self, p: PlotIdx) -> bool {
        self.map.plots[p].plot_type == PlotType::PLOT_OCEAN
    }

    /// C++ `CvPlot::isFlatlands`.
    #[inline]
    pub fn plot_is_flatlands(&self, p: PlotIdx) -> bool {
        self.map.plots[p].plot_type == PlotType::PLOT_LAND
    }

    /// C++ `CvPlot::isHills`.
    #[inline]
    pub fn plot_is_hills(&self, p: PlotIdx) -> bool {
        self.map.plots[p].plot_type == PlotType::PLOT_HILLS
    }

    /// C++ `CvPlot::isPeak`.
    #[inline]
    pub fn plot_is_peak(&self, p: PlotIdx) -> bool {
        self.map.plots[p].plot_type == PlotType::PLOT_PEAK
    }

    /// C++ `CvPlot::isImpassable`: peaks, or impassable terrain/feature.
    pub fn plot_is_impassable(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isImpassable")
    }

    /// C++ `CvPlot::setTerrainType` (`bRebuildGraphics` dropped: graphics only).
    /// Switches the plot type when the terrain's water flag differs.
    pub fn plot_set_terrain_type(&mut self, p: PlotIdx, terrain: TerrainType, recalculate: bool) {
        todo!("CvPlot::setTerrainType")
    }

    /// C++ `CvPlot::setTempTerrainType` (FFH): remembers the real terrain and starts the timer.
    pub fn plot_set_temp_terrain_type(&mut self, p: PlotIdx, terrain: TerrainType, timer: i32) {
        todo!("CvPlot::setTempTerrainType")
    }

    /// C++ `CvPlot::changePlotCounter` (FFH hell terrain): clamps to 0..=100 and moves the
    /// terrain up/down its terrain chain.
    pub fn plot_change_plot_counter(&mut self, p: PlotIdx, change: i32) {
        todo!("CvPlot::changePlotCounter")
    }

    // ------------------------------------------------------------------
    // Features
    // ------------------------------------------------------------------

    /// C++ `CvPlot::canHaveFeature` (None = NO_FEATURE: always true).
    pub fn plot_can_have_feature(&self, p: PlotIdx, feature: Option<FeatureType>) -> bool {
        todo!("CvPlot::canHaveFeature")
    }

    /// C++ `CvPlot::setFeatureType` (the art-only `iVariety` argument is dropped).
    pub fn plot_set_feature_type(&mut self, p: PlotIdx, feature: Option<FeatureType>) {
        todo!("CvPlot::setFeatureType")
    }

    // ------------------------------------------------------------------
    // Bonuses
    // ------------------------------------------------------------------

    /// C++ `CvPlot::canHaveBonus` (None = NO_BONUS: always true).
    pub fn plot_can_have_bonus(&self, p: PlotIdx, bonus: Option<BonusType>, ignore_latitude: bool) -> bool {
        todo!("CvPlot::canHaveBonus")
    }

    /// C++ `CvPlot::getBonusType(eTeam)`: the bonus if `team` has its reveal tech (or it is
    /// force-revealed). `team == None` is the plain `bonus_type` field.
    pub fn plot_get_bonus_type(&self, p: PlotIdx, team: Option<TeamId>) -> Option<BonusType> {
        todo!("CvPlot::getBonusType")
    }

    /// C++ `CvPlot::getNonObsoleteBonusType`.
    pub fn plot_get_non_obsolete_bonus_type(&self, p: PlotIdx, team: TeamId) -> Option<BonusType> {
        todo!("CvPlot::getNonObsoleteBonusType")
    }

    /// C++ `CvPlot::setBonusType`.
    pub fn plot_set_bonus_type(&mut self, p: PlotIdx, bonus: Option<BonusType>) {
        todo!("CvPlot::setBonusType")
    }

    // ------------------------------------------------------------------
    // Improvements and goodies
    // ------------------------------------------------------------------

    /// C++ `CvPlot::canHaveImprovement` (C++ defaults: `eTeam = NO_TEAM`, `bPotential = false`).
    pub fn plot_can_have_improvement(&self, p: PlotIdx, improvement: ImprovementType, team: Option<TeamId>, potential: bool) -> bool {
        todo!("CvPlot::canHaveImprovement")
    }

    /// C++ `CvPlot::setImprovementType`.
    pub fn plot_set_improvement_type(&mut self, p: PlotIdx, improvement: Option<ImprovementType>) {
        todo!("CvPlot::setImprovementType")
    }

    /// C++ `CvPlot::getUpgradeTimeLeft`.
    pub fn plot_get_upgrade_time_left(&self, p: PlotIdx, improvement: ImprovementType, player: Option<PlayerId>) -> i32 {
        todo!("CvPlot::getUpgradeTimeLeft")
    }

    /// C++ `CvPlot::isGoody` (C++ default `eTeam = NO_TEAM`).
    pub fn plot_is_goody(&self, p: PlotIdx, team: Option<TeamId>) -> bool {
        todo!("CvPlot::isGoody")
    }

    /// C++ `CvPlot::isRevealedGoody` (FFH: lairs and equipment count as goodies at war).
    pub fn plot_is_revealed_goody(&self, p: PlotIdx, team: Option<TeamId>) -> bool {
        todo!("CvPlot::isRevealedGoody")
    }

    /// C++ `CvPlot::removeGoody`.
    pub fn plot_remove_goody(&mut self, p: PlotIdx) {
        self.plot_set_improvement_type(p, None);
    }

    /// C++ static `CvPlot::c_aePirateCoveTypes` set-up (first `CvPlot` constructor): the
    /// `PIRATE_COVE_IMPROVEMENT` define and its upgrade chain, stored sorted in
    /// `Map::pirate_cove_types`.
    pub fn plot_init_pirate_cove_types(&mut self) {
        todo!("CvPlot::CvPlot (c_aePirateCoveTypes)")
    }

    /// C++ `CvPlot::isPirateCove` (BETTER AI).
    pub fn plot_is_pirate_cove(&self, p: PlotIdx) -> bool {
        match self.map.plots[p].improvement_type {
            Some(i) => self.map.pirate_cove_types.binary_search(&i).is_ok(),
            None => false,
        }
    }

    /// C++ `CvPlot::isPirateCoveValid` (BETTER AI; AI-only callers).
    pub fn plot_is_pirate_cove_valid(&self, p: PlotIdx, player: PlayerId) -> bool {
        todo!("CvPlot::isPirateCoveValid")
    }

    // ------------------------------------------------------------------
    // Routes
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isRoute`.
    #[inline]
    pub fn plot_is_route(&self, p: PlotIdx) -> bool {
        self.map.plots[p].route_type.is_some()
    }

    /// C++ `CvPlot::setRouteType`.
    pub fn plot_set_route_type(&mut self, p: PlotIdx, route: Option<RouteType>, update_plot_groups: bool) {
        todo!("CvPlot::setRouteType")
    }

    /// C++ `CvPlot::updateCityRoute`.
    pub fn plot_update_city_route(&mut self, p: PlotIdx, update_plot_group: bool) {
        todo!("CvPlot::updateCityRoute")
    }

    // ------------------------------------------------------------------
    // Builds
    // ------------------------------------------------------------------

    /// C++ `CvPlot::canBuild` (calls the `canBuild` Python callback when enabled).
    pub fn plot_can_build(&self, p: PlotIdx, build: BuildType, player: PlayerId, test_visible: bool) -> bool {
        todo!("CvPlot::canBuild")
    }

    /// C++ `CvPlot::getBuildTime`.
    pub fn plot_get_build_time(&self, p: PlotIdx, build: BuildType) -> i32 {
        todo!("CvPlot::getBuildTime")
    }

    /// C++ `CvPlot::getBuildTurnsLeft`.
    pub fn plot_get_build_turns_left(&self, p: PlotIdx, build: BuildType, now_extra: i32, then_extra: i32) -> i32 {
        todo!("CvPlot::getBuildTurnsLeft")
    }

    /// C++ `CvPlot::getFeatureProduction`: production from clearing the feature and the city
    /// that receives it (the C++ `CvCity** ppCity` out-parameter).
    pub fn plot_get_feature_production(&self, p: PlotIdx, build: BuildType, team: TeamId) -> (i32, Option<CityId>) {
        todo!("CvPlot::getFeatureProduction")
    }

    /// C++ `CvPlot::changeBuildProgress`: true if the build finished.
    pub fn plot_change_build_progress(&mut self, p: PlotIdx, build: BuildType, change: i32, team: TeamId) -> bool {
        todo!("CvPlot::changeBuildProgress")
    }

    /// C++ `CvPlot::isBuilding` (FFH; AI-only callers): a unit of `team` is building `build`
    /// within `range`.
    pub fn plot_is_building(&self, p: PlotIdx, build: BuildType, team: TeamId, range: i32, exclude_center: bool) -> bool {
        todo!("CvPlot::isBuilding")
    }
}
