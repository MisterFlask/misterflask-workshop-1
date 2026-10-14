//! CvPlot, topic "units": the plot's unit list, unit queries (`plotCount`/`plotCheck` with
//! `PUF_*` predicates), defenders, movement cost, defense modifiers, nukes, sight (line of
//! sight and sight sources), plot events and unit training requirements.

#![allow(unused_variables)]
#![allow(unused_imports)]

use super::{ConstPlotUnitFunc, PlotUnitFunc};
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Unit list (`getNumUnits`, `getUnitByIndex`, `headUnitNode`, ... are `units` field ops)
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isUnit`.
    #[inline]
    pub fn plot_is_unit(&self, p: PlotIdx) -> bool {
        !self.map.plots[p].units.is_empty()
    }

    /// C++ `CvPlot::addUnit` (`bUpdate` dropped: center unit / flag refresh only). Inserts the
    /// unit before the first unit it sorts before (`isBeforeUnitCycle`).
    pub fn plot_add_unit(&mut self, p: PlotIdx, unit: UnitId) {
        todo!("CvPlot::addUnit")
    }

    /// C++ `CvPlot::removeUnit` (`bUpdate` dropped: center unit / flag refresh only).
    pub fn plot_remove_unit(&mut self, p: PlotIdx, unit: UnitId) {
        let units = &mut self.map.plots[p].units;
        if let Some(i) = units.iter().position(|&u| u == unit) {
            units.remove(i);
        }
    }

    /// C++ `CvPlot::verifyUnitValidPlot`: bumps units that can no longer stay here.
    pub fn plot_verify_unit_valid_plot(&mut self, p: PlotIdx) {
        todo!("CvPlot::verifyUnitValidPlot")
    }

    /// C++ `CvPlot::plotAction` (C++ defaults: data -1, owner/team none).
    pub fn plot_plot_action(&mut self, p: PlotIdx, func: PlotUnitFunc, data1: i32, data2: i32, owner: Option<PlayerId>, team: Option<TeamId>) {
        todo!("CvPlot::plotAction")
    }

    /// C++ `CvPlot::plotCount`: units of `owner`/`team` passing `func_a` and `func_b`
    /// (a None predicate always passes).
    #[allow(clippy::too_many_arguments)]
    pub fn plot_plot_count(
        &self,
        p: PlotIdx,
        func_a: Option<ConstPlotUnitFunc>,
        data1_a: i32,
        data2_a: i32,
        owner: Option<PlayerId>,
        team: Option<TeamId>,
        func_b: Option<ConstPlotUnitFunc>,
        data1_b: i32,
        data2_b: i32,
    ) -> i32 {
        todo!("CvPlot::plotCount")
    }

    /// C++ `CvPlot::plotCheck`: first unit of `owner`/`team` passing `func_a` and `func_b`
    /// (`func_a` is required, a None `func_b` always passes).
    #[allow(clippy::too_many_arguments)]
    pub fn plot_plot_check(
        &self,
        p: PlotIdx,
        func_a: ConstPlotUnitFunc,
        data1_a: i32,
        data2_a: i32,
        owner: Option<PlayerId>,
        team: Option<TeamId>,
        func_b: Option<ConstPlotUnitFunc>,
        data1_b: i32,
        data2_b: i32,
    ) -> Option<UnitId> {
        todo!("CvPlot::plotCheck")
    }

    /// C++ `CvPlot::getNumAnimalUnits` (FFH).
    pub fn plot_get_num_animal_units(&self, p: PlotIdx) -> i32 {
        todo!("CvPlot::getNumAnimalUnits")
    }

    /// C++ `CvPlot::getUnitPower` (C++ default `eOwner = NO_PLAYER`).
    pub fn plot_get_unit_power(&self, p: PlotIdx, owner: Option<PlayerId>) -> i32 {
        todo!("CvPlot::getUnitPower")
    }

    // ------------------------------------------------------------------
    // Defenders and enemies
    // ------------------------------------------------------------------

    /// C++ `CvPlot::getBestDefender` (C++ defaults: no attacking player/attacker, all tests off).
    #[allow(clippy::too_many_arguments)]
    pub fn plot_get_best_defender(
        &self,
        p: PlotIdx,
        owner: Option<PlayerId>,
        attacking_player: Option<PlayerId>,
        attacker: Option<UnitId>,
        test_at_war: bool,
        test_potential_enemy: bool,
        test_can_move: bool,
    ) -> Option<UnitId> {
        todo!("CvPlot::getBestDefender")
    }

    /// C++ `CvPlot::isInvestigate`.
    pub fn plot_is_investigate(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isInvestigate")
    }

    /// C++ `CvPlot::isVisibleEnemyDefender`.
    pub fn plot_is_visible_enemy_defender(&self, p: PlotIdx, unit: UnitId) -> bool {
        todo!("CvPlot::isVisibleEnemyDefender")
    }

    /// C++ `CvPlot::getVisibleEnemyDefender`.
    pub fn plot_get_visible_enemy_defender(&self, p: PlotIdx, player: PlayerId) -> Option<UnitId> {
        todo!("CvPlot::getVisibleEnemyDefender")
    }

    /// C++ `CvPlot::getNumDefenders`.
    pub fn plot_get_num_defenders(&self, p: PlotIdx, player: PlayerId) -> i32 {
        todo!("CvPlot::getNumDefenders")
    }

    /// C++ `CvPlot::getNumVisibleEnemyDefenders`.
    pub fn plot_get_num_visible_enemy_defenders(&self, p: PlotIdx, unit: UnitId) -> i32 {
        todo!("CvPlot::getNumVisibleEnemyDefenders")
    }

    /// C++ `CvPlot::getNumVisiblePotentialEnemyDefenders`.
    pub fn plot_get_num_visible_potential_enemy_defenders(&self, p: PlotIdx, unit: UnitId) -> i32 {
        todo!("CvPlot::getNumVisiblePotentialEnemyDefenders")
    }

    /// C++ `CvPlot::isVisibleEnemyUnit(PlayerTypes)`.
    pub fn plot_is_visible_enemy_unit(&self, p: PlotIdx, player: PlayerId) -> bool {
        todo!("CvPlot::isVisibleEnemyUnit(PlayerTypes)")
    }

    /// C++ `CvPlot::isVisibleEnemyUnit(const CvUnit*)`.
    pub fn plot_is_visible_enemy_unit_for_unit(&self, p: PlotIdx, unit: UnitId) -> bool {
        todo!("CvPlot::isVisibleEnemyUnit(const CvUnit*)")
    }

    /// C++ `CvPlot::getNumVisibleUnits`.
    pub fn plot_get_num_visible_units(&self, p: PlotIdx, player: PlayerId) -> i32 {
        todo!("CvPlot::getNumVisibleUnits")
    }

    /// C++ `CvPlot::isVisibleOtherUnit`.
    pub fn plot_is_visible_other_unit(&self, p: PlotIdx, player: PlayerId) -> bool {
        todo!("CvPlot::isVisibleOtherUnit")
    }

    /// C++ `CvPlot::isFighting`.
    pub fn plot_is_fighting(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isFighting")
    }

    /// C++ `CvPlot::isFriendlyCity`.
    pub fn plot_is_friendly_city(&self, p: PlotIdx, unit: UnitId, check_improvement: bool) -> bool {
        todo!("CvPlot::isFriendlyCity")
    }

    /// C++ `CvPlot::isEnemyCity`.
    pub fn plot_is_enemy_city(&self, p: PlotIdx, unit: UnitId) -> bool {
        todo!("CvPlot::isEnemyCity")
    }

    // ------------------------------------------------------------------
    // Domains, movement, defense
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isValidDomainForLocation`.
    pub fn plot_is_valid_domain_for_location(&self, p: PlotIdx, unit: UnitId) -> bool {
        todo!("CvPlot::isValidDomainForLocation")
    }

    /// C++ `CvPlot::isValidDomainForAction`.
    pub fn plot_is_valid_domain_for_action(&self, p: PlotIdx, unit: UnitId) -> bool {
        todo!("CvPlot::isValidDomainForAction")
    }

    /// C++ `CvPlot::isValidRoute`.
    pub fn plot_is_valid_route(&self, p: PlotIdx, unit: UnitId) -> bool {
        todo!("CvPlot::isValidRoute")
    }

    /// C++ `CvPlot::movementCost`: cost for `unit` to enter this plot from `from_plot`.
    pub fn plot_movement_cost(&self, p: PlotIdx, unit: UnitId, from_plot: PlotIdx) -> i32 {
        todo!("CvPlot::movementCost")
    }

    /// C++ `CvPlot::getExtraMovePathCost` (`CvGame::getPlotExtraCost` at this plot).
    pub fn plot_get_extra_move_path_cost(&self, p: PlotIdx) -> i32 {
        todo!("CvPlot::getExtraMovePathCost")
    }

    /// C++ `CvPlot::changeExtraMovePathCost` (`CvGame::changePlotExtraCost` at this plot).
    pub fn plot_change_extra_move_path_cost(&mut self, p: PlotIdx, change: i32) {
        todo!("CvPlot::changeExtraMovePathCost")
    }

    /// C++ `CvPlot::defenseModifier` (`bHelp` dropped: it reads the active team's revealed
    /// improvement for help text).
    pub fn plot_defense_modifier(&self, p: PlotIdx, defender: Option<TeamId>, ignore_building: bool) -> i32 {
        todo!("CvPlot::defenseModifier")
    }

    /// C++ `CvPlot::getRangeDefense` (FFH): best defense/range-defense modifier of friendly
    /// improvements within `range` (`final_` = C++ `bFinal`: use final upgrades).
    pub fn plot_get_range_defense(&self, p: PlotIdx, defender: TeamId, range: i32, final_: bool, exclude_center: bool) -> i32 {
        todo!("CvPlot::getRangeDefense")
    }

    /// C++ `CvPlot::countNumAirUnits`.
    pub fn plot_count_num_air_units(&self, p: PlotIdx, team: TeamId) -> i32 {
        todo!("CvPlot::countNumAirUnits")
    }

    /// C++ `CvPlot::airUnitSpaceAvailable`.
    pub fn plot_air_unit_space_available(&self, p: PlotIdx, team: TeamId) -> i32 {
        todo!("CvPlot::airUnitSpaceAvailable")
    }

    /// C++ `CvPlot::isEspionageCounterSpy`.
    pub fn plot_is_espionage_counter_spy(&self, p: PlotIdx, team: TeamId) -> bool {
        todo!("CvPlot::isEspionageCounterSpy")
    }

    /// C++ `CvPlot::nukeExplosion` (C++ default `pNukeUnit = NULL`).
    pub fn plot_nuke_explosion(&mut self, p: PlotIdx, range: i32, nuke_unit: Option<UnitId>) {
        todo!("CvPlot::nukeExplosion")
    }

    // ------------------------------------------------------------------
    // Sight
    // ------------------------------------------------------------------

    /// C++ `CvPlot::seeFromLevel`.
    pub fn plot_see_from_level(&self, p: PlotIdx, team: TeamId) -> i32 {
        todo!("CvPlot::seeFromLevel")
    }

    /// C++ `CvPlot::seeThroughLevel`.
    pub fn plot_see_through_level(&self, p: PlotIdx) -> i32 {
        todo!("CvPlot::seeThroughLevel")
    }

    /// C++ `CvPlot::changeAdjacentSight`: adds/removes `team`'s sight of radius `range`
    /// around this plot (line of sight; `unit` supplies facing and see-invisible types).
    pub fn plot_change_adjacent_sight(&mut self, p: PlotIdx, team: TeamId, range: i32, increment: bool, unit: Option<UnitId>, update_plot_groups: bool) {
        todo!("CvPlot::changeAdjacentSight")
    }

    /// C++ `CvPlot::canSeePlot` (C++ returns false for a NULL plot).
    pub fn plot_can_see_plot(&self, p: PlotIdx, other: PlotIdx, team: TeamId, range: i32, facing: Option<DirectionType>) -> bool {
        todo!("CvPlot::canSeePlot")
    }

    /// C++ `CvPlot::canSeeDisplacementPlot` (recursive line-of-sight test).
    #[allow(clippy::too_many_arguments)]
    pub fn plot_can_see_displacement_plot(
        &self,
        p: PlotIdx,
        team: TeamId,
        dx: i32,
        dy: i32,
        original_dx: i32,
        original_dy: i32,
        first_plot: bool,
        outer_ring: bool,
    ) -> bool {
        todo!("CvPlot::canSeeDisplacementPlot")
    }

    /// C++ `CvPlot::shouldProcessDisplacementPlot`: is (dx, dy) inside the sight cone of
    /// `facing` (None = no facing: always)? Reads only its arguments (`range` is unused in
    /// the C++ too).
    #[inline]
    pub fn plot_should_process_displacement_plot(&self, p: PlotIdx, dx: i32, dy: i32, range: i32, facing: Option<DirectionType>) -> bool {
        should_process_displacement(dx, dy, facing)
    }

    /// C++ `CvPlot::updateSight`: applies/removes all sight sources on this plot (FFH holy
    /// city religions, vassals, espionage, ownership, units, recon).
    pub fn plot_update_sight(&mut self, p: PlotIdx, increment: bool, update_plot_groups: bool) {
        todo!("CvPlot::updateSight")
    }

    /// C++ `CvPlot::updateSeeFromSight`: `updateSight` on every plot that could see this one.
    pub fn plot_update_see_from_sight(&mut self, p: PlotIdx, increment: bool, update_plot_groups: bool) {
        todo!("CvPlot::updateSeeFromSight")
    }

    // ------------------------------------------------------------------
    // Events and training
    // ------------------------------------------------------------------

    /// C++ `CvPlot::canTrigger`.
    pub fn plot_can_trigger(&self, p: PlotIdx, trigger: EventTriggerType, player: PlayerId) -> bool {
        todo!("CvPlot::canTrigger")
    }

    /// C++ `CvPlot::canApplyEvent`.
    pub fn plot_can_apply_event(&self, p: PlotIdx, event: EventType) -> bool {
        todo!("CvPlot::canApplyEvent")
    }

    /// C++ `CvPlot::applyEvent`.
    pub fn plot_apply_event(&mut self, p: PlotIdx, event: EventType) {
        todo!("CvPlot::applyEvent")
    }

    /// C++ `CvPlot::canTrain`: plot/city requirements for training `unit` here
    /// (`continue_` = C++ `bContinue`, unused by the C++ body).
    pub fn plot_can_train(&self, p: PlotIdx, unit: UnitType, continue_: bool, test_visible: bool) -> bool {
        todo!("CvPlot::canTrain")
    }
}

/// Body of C++ `CvPlot::shouldProcessDisplacementPlot`, with the same single-precision float
/// ops as the C++ (`atan2` on floats, `60 * (float)M_PI / 180`).
fn should_process_displacement(dx: i32, dy: i32, facing: Option<DirectionType>) -> bool {
    let Some(facing) = facing else {
        return true;
    };
    if dx == 0 && dy == 0 {
        // always process this plot
        return true;
    }
    //                                    N       NE      E       SE       S        SW        W        NW
    const DISPLACEMENTS: [[i32; 2]; 8] = [[0, 1], [1, 1], [1, 0], [1, -1], [0, -1], [-1, -1], [-1, 0], [-1, 1]];
    let direction_x = DISPLACEMENTS[facing.idx()][0];
    let direction_y = DISPLACEMENTS[facing.idx()][1];

    // compute angle off of direction
    let cross_product = direction_x * dy - direction_y * dx;
    let dot_product = direction_x * dx + direction_y * dy;

    let theta = (cross_product as f32).atan2(dot_product as f32);
    let mut spread = 60.0_f32 * std::f32::consts::PI / 180.0;
    if dx.abs() <= 1 && dy.abs() <= 1 {
        // close plots use wider spread
        spread = 90.0_f32 * std::f32::consts::PI / 180.0;
    }
    theta >= -spread / 2.0 && theta <= spread / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_cone_boundaries() {
        let n = Some(DirectionType::DIRECTION_NORTH);
        assert!(should_process_displacement(0, 1, n));
        // 45 degrees off for a close plot is on the 90-degree cone boundary (inclusive).
        assert!(should_process_displacement(1, 1, n));
        assert!(should_process_displacement(-1, 1, n));
        assert!(!should_process_displacement(1, 0, n));
        assert!(!should_process_displacement(0, -2, n));
        // far plots use the 60-degree cone
        assert!(should_process_displacement(1, 3, n));
        assert!(!should_process_displacement(2, 2, n));
        assert!(should_process_displacement(0, 0, n));
        assert!(should_process_displacement(5, -5, None));
    }
}
