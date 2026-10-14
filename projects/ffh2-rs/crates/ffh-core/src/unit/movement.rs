//! CvUnit movement: territory/area entry, `canMoveInto`, `move`, paths, loading and cargo, moves
//! left, visibility and invisibility, enemy tests, jump/withdraw to a valid plot.
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges. Every `todo!()` names the C++ function still to port.

#![allow(unused_variables, unused_imports)]

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::unit::{CombatDetails, DefenderCombatValues, Unit, UnitResetInfo};
use ffh_data::*;

impl Game {
    /// C++ `CvUnit::getPathEndTurnPlot` (CvUnit.cpp:2914-2917).
    /// C++ `getGroup()->getPathEndTurnPlot()` (last path generated for the group).
    pub fn unit_get_path_end_turn_plot(&self, u: UnitId) -> Option<PlotIdx> {
        todo!("CvUnit::getPathEndTurnPlot")
    }

    /// C++ `CvUnit::generatePath` (CvUnit.cpp:2920-2923).
    /// C++ `getGroup()->generatePath(plot(), pToPlot, iFlags, bReuse, piPathTurns)`; `flags` are `MOVE_*`.
    pub fn unit_generate_path(
        &self,
        u: UnitId,
        to: PlotIdx,
        flags: i32,
        reuse: bool,
        path_turns: Option<&mut i32>,
    ) -> bool {
        todo!("CvUnit::generatePath")
    }

    /// C++ `CvUnit::canEnterTerritory` (CvUnit.cpp:2926-2987).
    pub fn unit_can_enter_territory(&self, u: UnitId, team: Option<TeamId>, ignore_right_of_passage: bool) -> bool {
        todo!("CvUnit::canEnterTerritory")
    }

    /// C++ `CvUnit::canEnterArea` (CvUnit.cpp:2990-3009).
    pub fn unit_can_enter_area(
        &self,
        u: UnitId,
        team: Option<TeamId>,
        area: Option<AreaId>,
        ignore_right_of_passage: bool,
    ) -> bool {
        todo!("CvUnit::canEnterArea")
    }

    /// C++ `CvUnit::getDeclareWarMove` (CvUnit.cpp:3013-3057).
    /// Returns the team to declare war on to move into `plot` (human players).
    pub fn unit_get_declare_war_move(&self, u: UnitId, plot: PlotIdx) -> Option<TeamId> {
        todo!("CvUnit::getDeclareWarMove")
    }

    /// C++ `CvUnit::willRevealByMove` (CvUnit.cpp:3059-3078).
    pub fn unit_will_reveal_by_move(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::willRevealByMove")
    }

    /// C++ `CvUnit::canMoveInto` (CvUnit.cpp:3080-3444).
    /// Ends with the `unitCannotMoveInto` Python callback when `USE_UNIT_CANNOT_MOVE_INTO_CALLBACK` is set.
    pub fn unit_can_move_into(
        &self,
        u: UnitId,
        plot: PlotIdx,
        attack: bool,
        declare_war: bool,
        ignore_load: bool,
    ) -> bool {
        todo!("CvUnit::canMoveInto")
    }

    /// C++ `CvUnit::canMoveOrAttackInto` (CvUnit.cpp:3447-3450).
    pub fn unit_can_move_or_attack_into(&self, u: UnitId, plot: PlotIdx, declare_war: bool) -> bool {
        todo!("CvUnit::canMoveOrAttackInto")
    }

    /// C++ `CvUnit::canMoveThrough` (CvUnit.cpp:3453-3456).
    pub fn unit_can_move_through(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canMoveThrough")
    }

    /// C++ `CvUnit::move` (CvUnit.cpp:3598-3645).
    /// C++ `bShow` (animate the move) is dropped. The `OnUnitChangeTo` feature change is rules;
    /// the bird effect (async rand) is graphics.
    pub fn unit_move(&mut self, u: UnitId, plot: PlotIdx) {
        todo!("CvUnit::move")
    }

    /// C++ `CvUnit::jumpToNearestValidPlot` (CvUnit.cpp:3648-3727).
    /// Returns false if the unit was killed (no valid plot).
    pub fn unit_jump_to_nearest_valid_plot(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::jumpToNearestValidPlot")
    }

    /// C++ `CvUnit::canLoadUnit` (CvUnit.cpp:4094-4164).
    pub fn unit_can_load_unit(&self, u: UnitId, transport: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canLoadUnit")
    }

    /// C++ `CvUnit::loadUnit` (CvUnit.cpp:4167-4175).
    pub fn unit_load_unit(&mut self, u: UnitId, transport: UnitId) {
        todo!("CvUnit::loadUnit")
    }

    /// C++ `CvUnit::shouldLoadOnMove` (CvUnit.cpp:4177-4233).
    pub fn unit_should_load_on_move(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::shouldLoadOnMove")
    }

    /// C++ `CvUnit::canLoad` (CvUnit.cpp:4236-4255).
    pub fn unit_can_load(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canLoad")
    }

    /// C++ `CvUnit::load` (CvUnit.cpp:4258-4296).
    pub fn unit_load(&mut self, u: UnitId) {
        todo!("CvUnit::load")
    }

    /// C++ `CvUnit::canUnload` (CvUnit.cpp:4299-4337).
    pub fn unit_can_unload(&self, u: UnitId) -> bool {
        todo!("CvUnit::canUnload")
    }

    /// C++ `CvUnit::unload` (CvUnit.cpp:4340-4348).
    pub fn unit_unload(&mut self, u: UnitId) {
        todo!("CvUnit::unload")
    }

    /// C++ `CvUnit::canUnloadAll` (CvUnit.cpp:4351-4359).
    pub fn unit_can_unload_all(&self, u: UnitId) -> bool {
        self.units[u].cargo != 0
    }

    /// C++ `CvUnit::unloadAll` (CvUnit.cpp:4362-4384).
    pub fn unit_unload_all(&mut self, u: UnitId) {
        todo!("CvUnit::unloadAll")
    }

    /// C++ `CvUnit::getInvisibleType` (CvUnit.cpp:8519-8557).
    /// FfH: hidden by the plot owner (`isHideUnits`), invisible-from-promotion, else the
    /// `invisible_type` field.
    pub fn unit_get_invisible_type(&self, u: UnitId) -> Option<InvisibleType> {
        todo!("CvUnit::getInvisibleType")
    }

    /// C++ `CvUnit::getNumSeeInvisibleTypes` (CvUnit.cpp:8559-8570).
    pub fn unit_get_num_see_invisible_types(&self, u: UnitId) -> i32 {
        todo!("CvUnit::getNumSeeInvisibleTypes")
    }

    /// C++ `CvUnit::getSeeInvisibleType` (CvUnit.cpp:8572-8583).
    pub fn unit_get_see_invisible_type(&self, u: UnitId, i: i32) -> InvisibleType {
        todo!("CvUnit::getSeeInvisibleType")
    }

    /// C++ `CvUnit::visibilityRange` (CvUnit.cpp:8604-8618).
    pub fn unit_visibility_range(&self, u: UnitId) -> i32 {
        todo!("CvUnit::visibilityRange")
    }

    /// C++ `CvUnit::baseMoves` (CvUnit.cpp:8621-8624).
    pub fn unit_base_moves(&self, u: UnitId) -> i32 {
        todo!("CvUnit::baseMoves")
    }

    /// C++ `CvUnit::maxMoves` (CvUnit.cpp:8627-8630).
    pub fn unit_max_moves(&self, u: UnitId) -> i32 {
        todo!("CvUnit::maxMoves")
    }

    /// C++ `CvUnit::movesLeft` (CvUnit.cpp:8633-8636).
    pub fn unit_moves_left(&self, u: UnitId) -> i32 {
        todo!("CvUnit::movesLeft")
    }

    /// C++ `CvUnit::canMove` (CvUnit.cpp:8639-8657).
    pub fn unit_can_move(&self, u: UnitId) -> bool {
        todo!("CvUnit::canMove")
    }

    /// C++ `CvUnit::hasMoved` (CvUnit.cpp:8660-8663).
    pub fn unit_has_moved(&self, u: UnitId) -> bool {
        self.units[u].moves > 0
    }

    /// C++ `CvUnit::canCoexistWithEnemyUnit` (CvUnit.cpp:8885-8903).
    pub fn unit_can_coexist_with_enemy_unit(&self, u: UnitId, team: Option<TeamId>) -> bool {
        todo!("CvUnit::canCoexistWithEnemyUnit")
    }

    /// C++ `CvUnit::alwaysInvisible` (CvUnit.cpp:10165-10168).
    pub fn unit_always_invisible(&self, u: UnitId) -> bool {
        todo!("CvUnit::alwaysInvisible")
    }

    /// C++ `CvUnit::canMoveImpassable` (CvUnit.cpp:10198-10209).
    pub fn unit_can_move_impassable(&self, u: UnitId) -> bool {
        todo!("CvUnit::canMoveImpassable")
    }

    /// C++ `CvUnit::canMoveAllTerrain` (CvUnit.cpp:10211-10222).
    pub fn unit_can_move_all_terrain(&self, u: UnitId) -> bool {
        todo!("CvUnit::canMoveAllTerrain")
    }

    /// C++ `CvUnit::flatMovementCost` (CvUnit.cpp:10224-10235).
    pub fn unit_flat_movement_cost(&self, u: UnitId) -> bool {
        todo!("CvUnit::flatMovementCost")
    }

    /// C++ `CvUnit::ignoreTerrainCost` (CvUnit.cpp:10238-10249).
    pub fn unit_ignore_terrain_cost(&self, u: UnitId) -> bool {
        todo!("CvUnit::ignoreTerrainCost")
    }

    /// C++ `CvUnit::isNeverInvisible` (CvUnit.cpp:10252-10255).
    pub fn unit_is_never_invisible(&self, u: UnitId) -> bool {
        todo!("CvUnit::isNeverInvisible")
    }

    /// C++ `CvUnit::isInvisible` (CvUnit.cpp:10258-10323).
    /// C++ `isInvisible(eTeam, bDebug, bCheckCargo)`: `bDebug` (debug-mode view) is dropped;
    /// C++ default `bCheckCargo` is true.
    pub fn unit_is_invisible(&self, u: UnitId, team: TeamId, check_cargo: bool) -> bool {
        todo!("CvUnit::isInvisible")
    }

    /// C++ `CvUnit::isInvisibleInPlot` (CvUnit.cpp:10326-10380).
    /// FfH variant of `isInvisible` for a given plot; `bDebug` dropped.
    pub fn unit_is_invisible_in_plot(&self, u: UnitId, team: TeamId, plot: Option<PlotIdx>, check_cargo: bool) -> bool {
        todo!("CvUnit::isInvisibleInPlot")
    }

    /// C++ `CvUnit::specialCargo` (CvUnit.cpp:10561-10564).
    pub fn unit_special_cargo(&self, u: UnitId) -> Option<SpecialUnitType> {
        todo!("CvUnit::specialCargo")
    }

    /// C++ `CvUnit::domainCargo` (CvUnit.cpp:10567-10570).
    pub fn unit_domain_cargo(&self, u: UnitId) -> Option<DomainType> {
        todo!("CvUnit::domainCargo")
    }

    /// C++ `CvUnit::changeCargoSpace` (CvUnit.cpp:10578-10586).
    pub fn unit_change_cargo_space(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].cargo_capacity += change;
        }
    }

    /// C++ `CvUnit::isFull` (CvUnit.cpp:10588-10591).
    pub fn unit_is_full(&self, u: UnitId) -> bool {
        let unit = &self.units[u];
        unit.cargo >= unit.cargo_capacity
    }

    /// C++ `CvUnit::cargoSpaceAvailable` (CvUnit.cpp:10594-10613).
    /// C++ defaults: `NO_SPECIALUNIT`, `NO_DOMAIN`.
    pub fn unit_cargo_space_available(
        &self,
        u: UnitId,
        special_cargo: Option<SpecialUnitType>,
        domain_cargo: Option<DomainType>,
    ) -> i32 {
        todo!("CvUnit::cargoSpaceAvailable")
    }

    /// C++ `CvUnit::hasCargo` (CvUnit.cpp:10616-10619).
    pub fn unit_has_cargo(&self, u: UnitId) -> bool {
        self.units[u].cargo > 0
    }

    /// C++ `CvUnit::canCargoAllMove` (CvUnit.cpp:10622-10650).
    pub fn unit_can_cargo_all_move(&self, u: UnitId) -> bool {
        todo!("CvUnit::canCargoAllMove")
    }

    /// C++ `CvUnit::canCargoEnterArea` (CvUnit.cpp:10652-10673).
    pub fn unit_can_cargo_enter_area(
        &self,
        u: UnitId,
        team: Option<TeamId>,
        area: Option<AreaId>,
        ignore_right_of_passage: bool,
    ) -> bool {
        todo!("CvUnit::canCargoEnterArea")
    }

    /// C++ `CvUnit::getUnitAICargo` (CvUnit.cpp:10675-10690).
    pub fn unit_get_unit_ai_cargo(&self, u: UnitId, unit_ai: UnitAIType) -> i32 {
        todo!("CvUnit::getUnitAICargo")
    }

    /// C++ `CvUnit::changeMoves` (CvUnit.cpp:11771-11774).
    pub fn unit_change_moves(&mut self, u: UnitId, change: i32) {
        // C++ `setMoves(getMoves() + iChange)`; the rest of setMoves is interface updates.
        self.units[u].moves += change;
    }

    /// C++ `CvUnit::finishMoves` (CvUnit.cpp:11777-11780).
    pub fn unit_finish_moves(&mut self, u: UnitId) {
        todo!("CvUnit::finishMoves")
    }

    /// C++ `CvUnit::changeCargo` (CvUnit.cpp:11876-11880).
    pub fn unit_change_cargo(&mut self, u: UnitId, change: i32) {
        self.units[u].cargo += change;
    }

    /// C++ `CvUnit::getCargoUnits` (CvUnit.cpp:11882-11902).
    /// Units on this unit's plot whose transport is this unit, in plot order.
    pub fn unit_get_cargo_units(&self, u: UnitId) -> Vec<UnitId> {
        todo!("CvUnit::getCargoUnits")
    }

    /// C++ `CvUnit::changeExtraVisibilityRange` (CvUnit.cpp:12100-12111).
    pub fn unit_change_extra_visibility_range(&mut self, u: UnitId, change: i32) {
        todo!("CvUnit::changeExtraVisibilityRange")
    }

    /// C++ `CvUnit::getImmobileTimer` (CvUnit.cpp:12516-12527).
    pub fn unit_get_immobile_timer(&self, u: UnitId) -> i32 {
        // FfH: held units are immobile.
        if self.unit_is_held(u) {
            return 999;
        }
        self.units[u].immobile_timer
    }

    /// C++ `CvUnit::setImmobileTimer` (CvUnit.cpp:12529-12551).
    /// FfH: casts the delayed spell when the timer reaches 0.
    pub fn unit_set_immobile_timer(&mut self, u: UnitId, value: i32) {
        todo!("CvUnit::setImmobileTimer")
    }

    /// C++ `CvUnit::changeImmobileTimer` (CvUnit.cpp:12553-12559).
    pub fn unit_change_immobile_timer(&mut self, u: UnitId, change: i32) {
        todo!("CvUnit::changeImmobileTimer")
    }

    /// C++ `CvUnit::isCargo` (CvUnit.cpp:12918-12921).
    pub fn unit_is_cargo(&self, u: UnitId) -> bool {
        // C++ `getTransportUnit() != NULL` (`::getUnit` yields NULL for a dead unit).
        self.units[u].transport_unit.map_or(false, |t| self.units.contains(t))
    }

    /// C++ `CvUnit::setTransportUnit` (CvUnit.cpp:12924-12978).
    pub fn unit_set_transport_unit(&mut self, u: UnitId, transport: Option<UnitId>) {
        todo!("CvUnit::setTransportUnit")
    }

    /// C++ `CvUnit::isTerrainDoubleMove` (CvUnit.cpp:13064-13069).
    pub fn unit_is_terrain_double_move(&self, u: UnitId, terrain: TerrainType) -> bool {
        self.units[u].terrain_double_move_count[terrain] > 0
    }

    /// C++ `CvUnit::changeTerrainDoubleMoveCount` (CvUnit.cpp:13072-13078).
    pub fn unit_change_terrain_double_move_count(&mut self, u: UnitId, terrain: TerrainType, change: i32) {
        self.units[u].terrain_double_move_count[terrain] += change;
    }

    /// C++ `CvUnit::isFeatureDoubleMove` (CvUnit.cpp:13089-13094).
    pub fn unit_is_feature_double_move(&self, u: UnitId, feature: FeatureType) -> bool {
        self.units[u].feature_double_move_count[feature] > 0
    }

    /// C++ `CvUnit::changeFeatureDoubleMoveCount` (CvUnit.cpp:13097-13103).
    pub fn unit_change_feature_double_move_count(&mut self, u: UnitId, feature: FeatureType, change: i32) {
        self.units[u].feature_double_move_count[feature] += change;
    }

    /// C++ `CvUnit::potentialWarAction` (CvUnit.cpp:13651-13672).
    /// Reads `getGroup()->AI_isDeclareWar(pPlot)` and the team war plan (AI bookkeeping).
    pub fn unit_potential_war_action(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::potentialWarAction")
    }

    /// C++ `CvUnit::getOpenPlot` (CvUnit.cpp:16728-16763).
    /// FfH: nearest empty plot valid for this unit (used by `betray`).
    pub fn unit_get_open_plot(&self, u: UnitId) -> Option<PlotIdx> {
        todo!("CvUnit::getOpenPlot")
    }

    /// C++ `CvUnit::withdrawlToNearestValidPlot` (CvUnit.cpp:16805-16864).
    /// FfH: move to the best adjacent valid plot; if none, kill the unit when `kill`.
    pub fn unit_withdrawl_to_nearest_valid_plot(&mut self, u: UnitId, kill: bool) -> bool {
        todo!("CvUnit::withdrawlToNearestValidPlot")
    }

    /// C++ `CvUnit::isEnemy` (CvUnit.cpp:18011-18029).
    /// `plot` `None` = the unit's own plot (C++ default NULL).
    pub fn unit_is_enemy(&self, u: UnitId, team: Option<TeamId>, plot: Option<PlotIdx>) -> bool {
        todo!("CvUnit::isEnemy")
    }

    /// C++ `CvUnit::isPotentialEnemy` (CvUnit.cpp:18031-18039).
    pub fn unit_is_potential_enemy(&self, u: UnitId, team: Option<TeamId>, plot: Option<PlotIdx>) -> bool {
        todo!("CvUnit::isPotentialEnemy")
    }

    /// C++ `CvUnit::isAlwaysHostile` (CvUnit.cpp:18339-18360).
    pub fn unit_is_always_hostile(&self, u: UnitId, plot: Option<PlotIdx>) -> bool {
        todo!("CvUnit::isAlwaysHostile")
    }

    /// C++ `CvUnit::verifyStackValid` (CvUnit.cpp:18362-18373).
    pub fn unit_verify_stack_valid(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::verifyStackValid")
    }
}
