//! CvGameUtils.py rules callbacks. FFH2 0.41 enables (PythonCallbackDefines.xml):
//! USE_CANNOT_DO_CIVIC, USE_CANNOT_CONSTRUCT, USE_CANNOT_RESEARCH,
//! USE_GET_BUILDING_COST_MOD, USE_CAN_BUILD, USE_CANNOT_TRAIN, USE_ON_UNIT_CREATED.
//! The DLL also calls some CvGameUtils functions unconditionally; those whose FFH2
//! Python contains rules logic are ported here too: doPillageGold, doCityCaptureGold,
//! getExperienceNeeded, cannotCreate, calculateScore, isVictoryTest. All remaining
//! hooks are disabled or return "use default" in FFH2, so the C++ default behaviour
//! applies and they are not called.
//!
//! AI-only branches (`if not pPlayer.isHuman(): ...` build restrictions in cannotTrain,
//! cannotConstruct, cannotCreate, cannotResearch, canBuild) are AI policy, not rules:
//! they are omitted here and left to the new AI (docs/PORTING.md, "Deliberate deviations").
//!
//! NOTE: Python 2 `/` on ints is floor division; use `py_div` for faithful results.

#![allow(unused_variables)]

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    /// `cannotDoCivic(ePlayer, eCivic)`.
    pub fn py_cannot_do_civic(&self, p: PlayerId, civic: CivicType) -> bool {
        false
    }
    /// `cannotConstruct(pCity, eBuilding, bContinue, bTestVisible, bIgnoreCost)`.
    pub fn py_cannot_construct(&self, c: CityId, building: BuildingType, continue_: bool, test_visible: bool, ignore_cost: bool) -> bool {
        false
    }
    /// `cannotResearch(ePlayer, eTech, bTrade)`.
    pub fn py_cannot_research(&self, p: PlayerId, tech: TechType, trade: bool) -> bool {
        false
    }
    /// `getBuildingCostMod(ePlayer, iCityID, eBuilding)`: percentage modifier (-1/0 = none).
    pub fn py_get_building_cost_mod(&self, p: PlayerId, c: CityId, building: BuildingType) -> i32 {
        -1
    }
    /// `canBuild(pPlot, iBuild, iTestVisible, iPlayer)`: -1 = use default, 0 = cannot, 1 = can.
    pub fn py_can_build(&self, plot: PlotIdx, build: BuildType, test_visible: bool, p: PlayerId) -> i32 {
        -1
    }
    /// `cannotTrain(pCity, eUnit, bContinue, bTestVisible, bIgnoreCost, bIgnoreUpgrades)`.
    pub fn py_cannot_train(&self, c: CityId, unit: UnitType, continue_: bool, test_visible: bool, ignore_cost: bool, ignore_upgrades: bool) -> bool {
        false
    }

    /// `doPillageGold(pPlot, pUnit)` (always called by `CvUnit::pillage`).
    pub fn py_do_pillage_gold(&mut self, plot: PlotIdx, u: UnitId) -> i32 {
        todo!("CvGameUtils.doPillageGold")
    }
    /// `doCityCaptureGold(pOldCity)` (always called by `CvPlayer::acquireCity`).
    pub fn py_do_city_capture_gold(&mut self, old_city: CityId) -> i32 {
        todo!("CvGameUtils.doCityCaptureGold")
    }
    /// `getExperienceNeeded(iLevel, iOwner)` (always called by `CvUnit::experienceNeeded`).
    pub fn py_get_experience_needed(&self, level: i32, owner: PlayerId) -> i32 {
        todo!("CvGameUtils.getExperienceNeeded")
    }
    /// `cannotCreate(pCity, eProject, bContinue, bTestVisible)` (always called by `CvCity::canCreate`).
    pub fn py_cannot_create(&self, c: CityId, project: ProjectType, continue_: bool, test_visible: bool) -> bool {
        todo!("CvGameUtils.cannotCreate")
    }
    /// `calculateScore(ePlayer, bFinal, bVictory)` (always called by `CvPlayer::calculateScore`).
    pub fn py_calculate_score(&self, p: PlayerId, final_: bool, victory: bool) -> i32 {
        todo!("CvGameUtils.calculateScore")
    }
    /// `isVictoryTest()` (always called by `CvGame::testVictory`).
    pub fn py_is_victory_test(&self) -> bool {
        todo!("CvGameUtils.isVictoryTest")
    }
}

/// Python 2 integer division (floor), for faithful ports of Python arithmetic.
#[inline]
pub fn py_div(a: i32, b: i32) -> i32 {
    let q = a / b;
    if (a % b != 0) && ((a < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}

#[cfg(test)]
mod tests {
    use super::py_div;

    #[test]
    fn floor_division() {
        assert_eq!(py_div(7, 2), 3);
        assert_eq!(py_div(-7, 2), -4);
        assert_eq!(py_div(7, -2), -4);
        assert_eq!(py_div(-150, 100), -2);
        assert_eq!(py_div(-200, 100), -2);
    }
}
