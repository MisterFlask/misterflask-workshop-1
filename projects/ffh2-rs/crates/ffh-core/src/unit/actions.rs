//! CvUnit commands and actions: `canDoCommand`/`doCommand`, automation, scrap/gift,
//! hold/sleep/fortify/patrol/heal (and heal rates), airlift, nuke, recon, paradrop, air bomb,
//! bombard, pillage, plunder and blockades.
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
    /// C++ `CvUnit::canDoCommand` (CvUnit.cpp:2649-2785).
    /// C++ non-const only because `canGift`/`canCast` are; it changes nothing. C++ defaults:
    /// `bTestVisible = false`, `bTestBusy = true`. COMMAND_LOAD_UNIT data is (owner, per-player
    /// unit id) in C++; the port decides how to encode a `UnitId`.
    pub fn unit_can_do_command(
        &self,
        u: UnitId,
        command: CommandType,
        data1: i32,
        data2: i32,
        test_visible: bool,
        test_busy: bool,
    ) -> bool {
        todo!("CvUnit::canDoCommand")
    }

    /// C++ `CvUnit::doCommand` (CvUnit.cpp:2788-2905).
    /// COMMAND_HOTKEY only sets the interface hot key (`setHotKeyNumber`, not ported).
    pub fn unit_do_command(&mut self, u: UnitId, command: CommandType, data1: i32, data2: i32) {
        todo!("CvUnit::doCommand")
    }

    /// C++ `CvUnit::canAutomate` (CvUnit.cpp:3730-3820).
    pub fn unit_can_automate(&self, u: UnitId, automate: Option<AutomateType>) -> bool {
        todo!("CvUnit::canAutomate")
    }

    /// C++ `CvUnit::automate` (CvUnit.cpp:3920-3928).
    pub fn unit_automate(&mut self, u: UnitId, automate: Option<AutomateType>) {
        todo!("CvUnit::automate")
    }

    /// C++ `CvUnit::canScrap` (CvUnit.cpp:3931-3950).
    pub fn unit_can_scrap(&self, u: UnitId) -> bool {
        todo!("CvUnit::canScrap")
    }

    /// C++ `CvUnit::scrap` (CvUnit.cpp:3953-3974).
    pub fn unit_scrap(&mut self, u: UnitId) {
        todo!("CvUnit::scrap")
    }

    /// C++ `CvUnit::canGift` (CvUnit.cpp:3977-4054).
    /// FfH 0.41 disables gifting: the C++ returns false before any other check.
    /// C++ non-const (asks `AI_acceptUnit`) but changes nothing.
    pub fn unit_can_gift(&self, u: UnitId, test_visible: bool, test_transport: bool) -> bool {
        todo!("CvUnit::canGift")
    }

    /// C++ `CvUnit::gift` (CvUnit.cpp:4057-4091).
    pub fn unit_gift(&mut self, u: UnitId, test_transport: bool) {
        todo!("CvUnit::gift")
    }

    /// C++ `CvUnit::canHold` (CvUnit.cpp:4387-4390).
    pub fn unit_can_hold(&self, u: UnitId, plot: PlotIdx) -> bool {
        true
    }

    /// C++ `CvUnit::canSleep` (CvUnit.cpp:4393-4406).
    pub fn unit_can_sleep(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canSleep")
    }

    /// C++ `CvUnit::canFortify` (CvUnit.cpp:4409-4422).
    pub fn unit_can_fortify(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canFortify")
    }

    /// C++ `CvUnit::canAirPatrol` (CvUnit.cpp:4425-4443).
    pub fn unit_can_air_patrol(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canAirPatrol")
    }

    /// C++ `CvUnit::canSeaPatrol` (CvUnit.cpp:4446-4469).
    pub fn unit_can_sea_patrol(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canSeaPatrol")
    }

    /// C++ `CvUnit::canHeal` (CvUnit.cpp:4501-4519).
    pub fn unit_can_heal(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canHeal")
    }

    /// C++ `CvUnit::canSentry` (CvUnit.cpp:4522-4535).
    pub fn unit_can_sentry(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canSentry")
    }

    /// C++ `CvUnit::healRate` (CvUnit.cpp:4538-4673).
    pub fn unit_heal_rate(&self, u: UnitId, plot: PlotIdx) -> i32 {
        todo!("CvUnit::healRate")
    }

    /// C++ `CvUnit::healTurns` (CvUnit.cpp:4676-4703).
    /// `MAX_INT` when the unit cannot heal on `plot`.
    pub fn unit_heal_turns(&self, u: UnitId, plot: PlotIdx) -> i32 {
        todo!("CvUnit::healTurns")
    }

    /// C++ `CvUnit::doHeal` (CvUnit.cpp:4706-4709).
    pub fn unit_do_heal(&mut self, u: UnitId) {
        todo!("CvUnit::doHeal")
    }

    /// C++ `CvUnit::canAirlift` (CvUnit.cpp:4712-4744).
    pub fn unit_can_airlift(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canAirlift")
    }

    /// C++ `CvUnit::canAirliftAt` (CvUnit.cpp:4747-4782).
    pub fn unit_can_airlift_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        todo!("CvUnit::canAirliftAt")
    }

    /// C++ `CvUnit::airlift` (CvUnit.cpp:4785-4815).
    pub fn unit_airlift(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        todo!("CvUnit::airlift")
    }

    /// C++ `CvUnit::isNukeVictim` (CvUnit.cpp:4818-4855).
    pub fn unit_is_nuke_victim(&self, u: UnitId, plot: PlotIdx, team: TeamId) -> bool {
        todo!("CvUnit::isNukeVictim")
    }

    /// C++ `CvUnit::canNuke` (CvUnit.cpp:4858-4866).
    pub fn unit_can_nuke(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canNuke")
    }

    /// C++ `CvUnit::canNukeAt` (CvUnit.cpp:4869-4904).
    pub fn unit_can_nuke_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        todo!("CvUnit::canNukeAt")
    }

    /// C++ `CvUnit::nuke` (CvUnit.cpp:4907-5084).
    pub fn unit_nuke(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        todo!("CvUnit::nuke")
    }

    /// C++ `CvUnit::canRecon` (CvUnit.cpp:5087-5105).
    /// `plot` is unused by the C++ and `canAutomate` passes NULL.
    pub fn unit_can_recon(&self, u: UnitId, plot: Option<PlotIdx>) -> bool {
        todo!("CvUnit::canRecon")
    }

    /// C++ `CvUnit::canReconAt` (CvUnit.cpp:5109-5123).
    pub fn unit_can_recon_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        todo!("CvUnit::canReconAt")
    }

    /// C++ `CvUnit::recon` (CvUnit.cpp:5126-5155).
    pub fn unit_recon(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        todo!("CvUnit::recon")
    }

    /// C++ `CvUnit::canParadrop` (CvUnit.cpp:5158-5176).
    pub fn unit_can_paradrop(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canParadrop")
    }

    /// C++ `CvUnit::canParadropAt` (CvUnit.cpp:5180-5222).
    pub fn unit_can_paradrop_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        todo!("CvUnit::canParadropAt")
    }

    /// C++ `CvUnit::paradrop` (CvUnit.cpp:5225-5260).
    pub fn unit_paradrop(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        todo!("CvUnit::paradrop")
    }

    /// C++ `CvUnit::canAirBomb` (CvUnit.cpp:5263-5281).
    pub fn unit_can_air_bomb(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canAirBomb")
    }

    /// C++ `CvUnit::canAirBombAt` (CvUnit.cpp:5284-5337).
    pub fn unit_can_air_bomb_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        todo!("CvUnit::canAirBombAt")
    }

    /// C++ `CvUnit::airBomb` (CvUnit.cpp:5340-5431).
    pub fn unit_air_bomb(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        todo!("CvUnit::airBomb")
    }

    /// C++ `CvUnit::bombardTarget` (CvUnit.cpp:5434-5470).
    pub fn unit_bombard_target(&self, u: UnitId, plot: PlotIdx) -> Option<CityId> {
        todo!("CvUnit::bombardTarget")
    }

    /// C++ `CvUnit::canBombard` (CvUnit.cpp:5473-5496).
    pub fn unit_can_bombard(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canBombard")
    }

    /// C++ `CvUnit::bombard` (CvUnit.cpp:5499-5560).
    pub fn unit_bombard(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::bombard")
    }

    /// C++ `CvUnit::canPillage` (CvUnit.cpp:5563-5620).
    pub fn unit_can_pillage(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canPillage")
    }

    /// C++ `CvUnit::pillage` (CvUnit.cpp:5623-5750).
    /// Pillage gold comes from the `doPillageGold` Python callback (port in `crate::ffh`).
    pub fn unit_pillage(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::pillage")
    }

    /// C++ `CvUnit::canPlunder` (CvUnit.cpp:5753-5789).
    pub fn unit_can_plunder(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        todo!("CvUnit::canPlunder")
    }

    /// C++ `CvUnit::plunder` (CvUnit.cpp:5792-5806).
    pub fn unit_plunder(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::plunder")
    }

    /// C++ `CvUnit::updatePlunder` (CvUnit.cpp:5809-5854).
    pub fn unit_update_plunder(&mut self, u: UnitId, change: i32, update_plot_groups: bool) {
        todo!("CvUnit::updatePlunder")
    }

    /// C++ `CvUnit::isAutomated` (CvUnit.cpp:10004-10007).
    /// C++ `getGroup()->isAutomated()`.
    pub fn unit_is_automated(&self, u: UnitId) -> bool {
        todo!("CvUnit::isAutomated")
    }

    /// C++ `CvUnit::isWaiting` (CvUnit.cpp:10010-10013).
    /// C++ `getGroup()->isWaiting()`.
    pub fn unit_is_waiting(&self, u: UnitId) -> bool {
        todo!("CvUnit::isWaiting")
    }

    /// C++ `CvUnit::setBlockading` (CvUnit.cpp:12677-12685).
    pub fn unit_set_blockading(&mut self, u: UnitId, value: bool) {
        todo!("CvUnit::setBlockading")
    }

    /// C++ `CvUnit::collectBlockadeGold` (CvUnit.cpp:12687-12728).
    pub fn unit_collect_blockade_gold(&mut self, u: UnitId) {
        todo!("CvUnit::collectBlockadeGold")
    }
}
