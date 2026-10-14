//! CvPlayer: cities -- founding, acquiring (conquest/trade), razing and disbanding,
//! city names, the capital, per-player city/plot-group lists, city counts, and goody huts
//! (exploration rewards).

#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::city::City;
use crate::game::Game;
use crate::ids::*;
use crate::plot_group::PlotGroup;
use crate::structs::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------ city / plot group lists

    /// C++ `CvPlayer::addCity`: stores a new city object and appends it to the owner list.
    /// (C++ returns an uninitialized object that the caller then `init`s.)
    pub fn player_add_city(&mut self, p: PlayerId, city: City) -> CityId {
        let id = self.cities.insert(city);
        self.players[p].cities.push(id);
        id
    }

    /// C++ `CvPlayer::deleteCity(iID)`: destroys the city object and removes it from the owner list.
    pub fn player_delete_city(&mut self, p: PlayerId, c: CityId) {
        self.cities.remove(c);
        self.players[p].cities.retain(|&x| x != c);
    }

    /// C++ `CvPlayer::addPlotGroup`.
    pub fn player_add_plot_group(&mut self, p: PlayerId, plot_group: PlotGroup) -> PlotGroupId {
        let id = self.plot_groups.insert(plot_group);
        self.players[p].plot_groups.push(id);
        id
    }

    /// C++ `CvPlayer::deletePlotGroup(iID)`.
    pub fn player_delete_plot_group(&mut self, p: PlayerId, pg: PlotGroupId) {
        self.plot_groups.remove(pg);
        self.players[p].plot_groups.retain(|&x| x != pg);
    }

    /// C++ `CvPlayer::initPlotGroup(pPlot)`: new plot group owned by `p` seeded at `plot`.
    pub fn player_init_plot_group(&mut self, p: PlayerId, plot: PlotIdx) -> PlotGroupId {
        todo!("CvPlayer::initPlotGroup")
    }

    /// C++ `CvPlayer::updatePlotGroups`: recalculates every plot group of the player (only
    /// once the game is final-initialized), then trade routes.
    pub fn player_update_plot_groups(&mut self, p: PlayerId) {
        todo!("CvPlayer::updatePlotGroups")
    }

    // ------------------------------------------------------------------ founding / acquiring / razing

    /// C++ `CvPlayer::initCity(iX, iY, bBumpUnits, bUpdatePlotGroups)`: creates and inits a city.
    pub fn player_init_city(&mut self, p: PlayerId, x: i32, y: i32, bump_units: bool, update_plot_groups: bool) -> CityId {
        todo!("CvPlayer::initCity")
    }

    /// C++ `CvPlayer::acquireCity(pCity, bConquest, bTrade, bUpdatePlotGroups)`: transfers
    /// `old_city` to `p` (kills it and re-creates it with the carried-over state); on
    /// conquest the raze decision goes to the controller (C++ `AI_conquerCity`) or a popup.
    pub fn player_acquire_city(&mut self, p: PlayerId, old_city: CityId, conquest: bool, trade: bool, update_plot_groups: bool) {
        todo!("CvPlayer::acquireCity")
    }

    /// C++ `CvPlayer::killCities`.
    pub fn player_kill_cities(&mut self, p: PlayerId) {
        todo!("CvPlayer::killCities")
    }

    /// C++ `CvPlayer::canFound(iX, iY, bTestVisible = false)`.
    pub fn player_can_found(&self, p: PlayerId, x: i32, y: i32, test_visible: bool) -> bool {
        todo!("CvPlayer::canFound")
    }

    /// C++ `CvPlayer::found(iX, iY)`: founds a city (barbarian defenders, free start-era
    /// buildings, advanced-start Creative culture, Python `cityBuilt`).
    pub fn player_found(&mut self, p: PlayerId, x: i32, y: i32) {
        todo!("CvPlayer::found")
    }

    /// C++ `CvPlayer::canRaze(pCity)` (also Python `canRazeCity`).
    pub fn player_can_raze(&self, p: PlayerId, city: CityId) -> bool {
        todo!("CvPlayer::canRaze")
    }

    /// C++ `CvPlayer::raze(pCity)`: memories, FFH global counter and equipment units, then `disband`.
    pub fn player_raze(&mut self, p: PlayerId, city: CityId) {
        todo!("CvPlayer::raze")
    }

    /// C++ `CvPlayer::disband(pCity)`.
    pub fn player_disband(&mut self, p: PlayerId, city: CityId) {
        todo!("CvPlayer::disband")
    }

    /// C++ `CvPlayer::pickConqueredCityOwner(kCity)`: the liberation player if it is our
    /// vassal, else `p`.
    pub fn player_pick_conquered_city_owner(&self, p: PlayerId, city: CityId) -> PlayerId {
        todo!("CvPlayer::pickConqueredCityOwner")
    }

    // ------------------------------------------------------------------ city names

    /// C++ `CvPlayer::getNewCityName` (const in C++, but draws from the game RNG).
    pub fn player_get_new_city_name(&mut self, p: PlayerId) -> String {
        todo!("CvPlayer::getNewCityName")
    }

    /// C++ `CvPlayer::getCivilizationCityName(szBuffer, eCivilization)`: first valid city
    /// name of `civ` (None = the C++ buffer is left unchanged). Const in C++ but barbarian
    /// and minor civs draw a random offset from the game RNG.
    pub fn player_get_civilization_city_name(&mut self, p: PlayerId, civ: CivilizationType) -> Option<String> {
        todo!("CvPlayer::getCivilizationCityName")
    }

    /// C++ `CvPlayer::isCityNameValid(szName, bTestDestroyed = true)`.
    pub fn player_is_city_name_valid(&self, p: PlayerId, name: &str, test_destroyed: bool) -> bool {
        todo!("CvPlayer::isCityNameValid")
    }

    /// C++ `CvPlayer::getCityName(iIndex)`: entry `index` of `city_names`, "" if out of range.
    pub fn player_get_city_name(&self, p: PlayerId, index: i32) -> String {
        if index < 0 {
            return String::new();
        }
        self.players[p].city_names.get(index as usize).cloned().unwrap_or_default()
    }

    // ------------------------------------------------------------------ capital

    /// C++ `CvPlayer::getCapitalCity`: `capital_city` if that city still exists (C++ looks
    /// the id up in the owner's city list, so a deleted capital reads as NULL). City ids
    /// are never reused and a city never changes owner without being re-created.
    pub fn player_get_capital_city(&self, p: PlayerId) -> Option<CityId> {
        self.players[p].capital_city.filter(|&c| self.cities.contains(c))
    }

    /// C++ `CvPlayer::setCapitalCity(pNewCapitalCity)`: plot group bonuses, maintenance,
    /// trade routes and both cities' commerce.
    pub fn player_set_capital_city(&mut self, p: PlayerId, city: Option<CityId>) {
        todo!("CvPlayer::setCapitalCity")
    }

    /// C++ `CvPlayer::findNewCapital`: moves the capital building to the best other city.
    pub fn player_find_new_capital(&mut self, p: PlayerId) {
        todo!("CvPlayer::findNewCapital")
    }

    /// C++ `CvPlayer::getNumGovernmentCenters`.
    pub fn player_get_num_government_centers(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::getNumGovernmentCenters")
    }

    // ------------------------------------------------------------------ counts over cities

    /// C++ `CvPlayer::countNumCoastalCities`.
    pub fn player_count_num_coastal_cities(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::countNumCoastalCities")
    }

    /// C++ `CvPlayer::countNumCoastalCitiesByArea(pArea)`.
    pub fn player_count_num_coastal_cities_by_area(&self, p: PlayerId, area: AreaId) -> i32 {
        todo!("CvPlayer::countNumCoastalCitiesByArea")
    }

    /// C++ `CvPlayer::countTotalCulture` (sum of own culture in own cities, /100).
    pub fn player_count_total_culture(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::countTotalCulture")
    }

    /// C++ `CvPlayer::countCityFeatures(eFeature)`.
    pub fn player_count_city_features(&self, p: PlayerId, feature: FeatureType) -> i32 {
        todo!("CvPlayer::countCityFeatures")
    }

    /// C++ `CvPlayer::countNumBuildings(eBuilding)`.
    pub fn player_count_num_buildings(&self, p: PlayerId, building: BuildingType) -> i32 {
        todo!("CvPlayer::countNumBuildings")
    }

    /// C++ `CvPlayer::countNumCitiesConnectedToCapital`.
    pub fn player_count_num_cities_connected_to_capital(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::countNumCitiesConnectedToCapital")
    }

    /// C++ `CvPlayer::getNumBuilding(iBuilding)` (FFH: real buildings of a type over all cities).
    pub fn player_get_num_building(&self, p: PlayerId, building: BuildingType) -> i32 {
        todo!("CvPlayer::getNumBuilding")
    }

    /// C++ `CvPlayer::getNumSettlements` (FFH: cities that are settlements).
    pub fn player_get_num_settlements(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::getNumSettlements")
    }

    // ------------------------------------------------------------------ goody huts

    /// C++ `CvPlayer::canReceiveGoody(pPlot, eGoody, pUnit)`.
    pub fn player_can_receive_goody(&self, p: PlayerId, plot: PlotIdx, goody: GoodyType, unit: Option<UnitId>) -> bool {
        todo!("CvPlayer::canReceiveGoody")
    }

    /// C++ `CvPlayer::receiveGoody(pPlot, eGoody, pUnit)`.
    pub fn player_receive_goody(&mut self, p: PlayerId, plot: PlotIdx, goody: GoodyType, unit: Option<UnitId>) {
        todo!("CvPlayer::receiveGoody")
    }

    /// C++ `CvPlayer::doGoody(pPlot, pUnit)` (Python `doGoody` override, then handicap goodies).
    pub fn player_do_goody(&mut self, p: PlayerId, plot: PlotIdx, unit: Option<UnitId>) {
        todo!("CvPlayer::doGoody")
    }
}
