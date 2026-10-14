//! CvPlayer: game start -- initial gold/research state, free starting units, starting
//! area/plot selection, and the advanced-start point economy.

#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

/// C++ `AdvancedStartActionTypes` (CvEnums.h; not generated since no info table uses it).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AdvancedStartAction {
    /// C++ `ADVANCEDSTARTACTION_EXIT`
    Exit,
    /// C++ `ADVANCEDSTARTACTION_UNIT` (data = UnitTypes)
    Unit,
    /// C++ `ADVANCEDSTARTACTION_CITY`
    City,
    /// C++ `ADVANCEDSTARTACTION_POP`
    Pop,
    /// C++ `ADVANCEDSTARTACTION_CULTURE`
    Culture,
    /// C++ `ADVANCEDSTARTACTION_BUILDING` (data = BuildingTypes)
    Building,
    /// C++ `ADVANCEDSTARTACTION_IMPROVEMENT` (data = ImprovementTypes)
    Improvement,
    /// C++ `ADVANCEDSTARTACTION_ROUTE` (data = RouteTypes)
    Route,
    /// C++ `ADVANCEDSTARTACTION_TECH` (data = TechTypes)
    Tech,
    /// C++ `ADVANCEDSTARTACTION_VISIBILITY`
    Visibility,
    /// C++ `ADVANCEDSTARTACTION_AUTOMATE`
    Automate,
}

impl Game {
    // ------------------------------------------------------------------ initial state and units

    /// C++ `CvPlayer::initFreeState`: starting gold (handicap + start era + FFH trait
    /// starting gold) and an empty research queue.
    pub fn player_init_free_state(&mut self, p: PlayerId) {
        todo!("CvPlayer::initFreeState")
    }

    /// C++ `CvPlayer::initFreeUnits`: advanced-start points and starting visibility, or the
    /// civilization's free units plus era/handicap defense, worker and explore units.
    pub fn player_init_free_units(&mut self, p: PlayerId) {
        todo!("CvPlayer::initFreeUnits")
    }

    /// C++ `CvPlayer::addFreeUnitAI`: adds `count` units of the best trainable unit for
    /// `unit_ai` (best by `AI_unitValue`, through the controller).
    pub fn player_add_free_unit_ai(&mut self, p: PlayerId, unit_ai: UnitAIType, count: i32) {
        todo!("CvPlayer::addFreeUnitAI")
    }

    /// C++ `CvPlayer::addFreeUnit(eUnit, eUnitAI = NO_UNITAI)`: places a starting unit near
    /// the starting plot (FFH: settlers get STARTING_SETTLER_PROMOTION).
    pub fn player_add_free_unit(&mut self, p: PlayerId, unit: UnitType, unit_ai: Option<UnitAIType>) {
        todo!("CvPlayer::addFreeUnit")
    }

    // ------------------------------------------------------------------ starting plot

    /// C++ `CvPlayer::startingPlotRange`.
    pub fn player_starting_plot_range(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::startingPlotRange")
    }

    /// C++ `CvPlayer::startingPlotWithinRange`: always false since BtS ("XXX changes to
    /// AI_foundValue ... make this function redundant but it is still called from Python").
    pub fn player_starting_plot_within_range(&self, p: PlayerId, plot: PlotIdx, other: PlayerId, range: i32, pass: i32) -> bool {
        false
    }

    /// C++ `CvPlayer::startingPlotDistanceFactor`.
    pub fn player_starting_plot_distance_factor(&self, p: PlayerId, plot: PlotIdx, other: PlayerId, range: i32) -> i32 {
        todo!("CvPlayer::startingPlotDistanceFactor")
    }

    /// C++ `CvPlayer::findStartingArea`: best land area id, None if it does not matter
    /// (C++ -1; the map-script override `findStartingArea` is replaced by this port's map
    /// generators).
    pub fn player_find_starting_area(&self, p: PlayerId) -> Option<AreaId> {
        todo!("CvPlayer::findStartingArea")
    }

    /// C++ `CvPlayer::findStartingPlot(bRandomize = false)`: best plot by found value
    /// (C++ `AI_updateFoundValues(true)` + `CvPlot::getFoundValue`).
    pub fn player_find_starting_plot(&mut self, p: PlayerId, randomize: bool) -> Option<PlotIdx> {
        todo!("CvPlayer::findStartingPlot")
    }

    /// C++ `CvPlayer::findBestFoundValue`: best `CvArea::getBestFoundValue` over all areas.
    pub fn player_find_best_found_value(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::findBestFoundValue")
    }

    /// C++ `CvPlayer::getStartingPlot` (`plotSorenINLINE(m_iStartingX, m_iStartingY)`).
    pub fn player_get_starting_plot(&self, p: PlayerId) -> Option<PlotIdx> {
        todo!("CvPlayer::getStartingPlot")
    }

    /// C++ `CvPlayer::setStartingPlot(pNewValue, bUpdateStartDist)`: updates the areas'
    /// starting plot counts (and min original start distances).
    pub fn player_set_starting_plot(&mut self, p: PlayerId, plot: Option<PlotIdx>, update_start_dist: bool) {
        todo!("CvPlayer::setStartingPlot")
    }

    /// C++ `CvPlayer::getNewCityProductionValue`: production value of a new city's free
    /// start-era buildings and population (advanced start, settler extra cost).
    pub fn player_get_new_city_production_value(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::getNewCityProductionValue")
    }

    // ------------------------------------------------------------------ advanced start

    /// C++ `CvPlayer::doAdvancedStartAction(eAction, iX, iY, iData, bAdd)`: buys (or sells)
    /// a unit/city/pop/culture/building/improvement/route/tech/visibility with
    /// advanced-start points; `data` is the type index the action needs.
    pub fn player_do_advanced_start_action(&mut self, p: PlayerId, action: AdvancedStartAction, x: i32, y: i32, data: i32, add: bool) {
        todo!("CvPlayer::doAdvancedStartAction")
    }

    /// C++ `CvPlayer::getAdvancedStartUnitCost(eUnit, bAdd, pPlot = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_unit_cost(&self, p: PlayerId, unit: UnitType, add: bool, plot: Option<PlotIdx>) -> i32 {
        todo!("CvPlayer::getAdvancedStartUnitCost")
    }

    /// C++ `CvPlayer::getAdvancedStartCityCost(bAdd, pPlot = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_city_cost(&self, p: PlayerId, add: bool, plot: Option<PlotIdx>) -> i32 {
        todo!("CvPlayer::getAdvancedStartCityCost")
    }

    /// C++ `CvPlayer::getAdvancedStartPopCost(bAdd, pCity = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_pop_cost(&self, p: PlayerId, add: bool, city: Option<CityId>) -> i32 {
        todo!("CvPlayer::getAdvancedStartPopCost")
    }

    /// C++ `CvPlayer::getAdvancedStartCultureCost(bAdd, pCity = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_culture_cost(&self, p: PlayerId, add: bool, city: Option<CityId>) -> i32 {
        todo!("CvPlayer::getAdvancedStartCultureCost")
    }

    /// C++ `CvPlayer::getAdvancedStartBuildingCost(eBuilding, bAdd, pCity = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_building_cost(&self, p: PlayerId, building: BuildingType, add: bool, city: Option<CityId>) -> i32 {
        todo!("CvPlayer::getAdvancedStartBuildingCost")
    }

    /// C++ `CvPlayer::getAdvancedStartImprovementCost(eImprovement, bAdd, pPlot = NULL)`;
    /// -1 = not possible (also for NO_IMPROVEMENT).
    pub fn player_get_advanced_start_improvement_cost(&self, p: PlayerId, improvement: Option<ImprovementType>, add: bool, plot: Option<PlotIdx>) -> i32 {
        todo!("CvPlayer::getAdvancedStartImprovementCost")
    }

    /// C++ `CvPlayer::getAdvancedStartRouteCost(eRoute, bAdd, pPlot = NULL)`; -1 = not
    /// possible (also for NO_ROUTE).
    pub fn player_get_advanced_start_route_cost(&self, p: PlayerId, route: Option<RouteType>, add: bool, plot: Option<PlotIdx>) -> i32 {
        todo!("CvPlayer::getAdvancedStartRouteCost")
    }

    /// C++ `CvPlayer::getAdvancedStartTechCost(eTech, bAdd)`; -1 = not possible (also for NO_TECH).
    pub fn player_get_advanced_start_tech_cost(&self, p: PlayerId, tech: Option<TechType>, add: bool) -> i32 {
        todo!("CvPlayer::getAdvancedStartTechCost")
    }

    /// C++ `CvPlayer::getAdvancedStartVisibilityCost(bAdd, pPlot = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_visibility_cost(&self, p: PlayerId, add: bool, plot: Option<PlotIdx>) -> i32 {
        todo!("CvPlayer::getAdvancedStartVisibilityCost")
    }
}
