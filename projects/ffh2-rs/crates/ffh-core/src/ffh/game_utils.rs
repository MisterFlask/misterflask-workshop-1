//! CvGameUtils.py rules callbacks. FFH2 0.41 enables (PythonCallbackDefines.xml):
//! USE_CANNOT_DO_CIVIC, USE_CANNOT_CONSTRUCT, USE_CANNOT_RESEARCH,
//! USE_GET_BUILDING_COST_MOD, USE_CAN_BUILD, USE_CANNOT_TRAIN, USE_ON_UNIT_CREATED.
//! All other CvGameUtils hooks are disabled or return "use default" in FFH2, so the
//! C++ default behaviour applies and they are not called.

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
}
