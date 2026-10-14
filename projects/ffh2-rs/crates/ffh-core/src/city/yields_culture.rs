//! CvCity: yields, commerce, trade routes, culture and culture level, religion, espionage
//! visibility, the rank caches, and owner/location helpers (`getTeam`, `plot`, `area`,
//! `isHuman`, `isCapital`, `isDisorder`, ...).

#![allow(unused_variables)]
#![allow(unused_imports)]
use super::{order_type_data, range, City};
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

impl Game {
    /// C++ `CvCity::updateYield` (CvCity.cpp:1213-1227): recomputes the yields of the city's
    /// plots.
    pub fn city_update_yield(&mut self, c: CityId) {
        todo!("CvCity::updateYield")
    }

    /// C++ `CvCity::findPopulationRank` (CvCity.cpp:1569-1592): rank among the owner's
    /// cities, cached in `population_rank` (C++ `mutable`).
    pub fn city_find_population_rank(&self, c: CityId) -> i32 {
        todo!("CvCity::findPopulationRank")
    }

    /// C++ `CvCity::findBaseYieldRateRank` (CvCity.cpp:1595-1619), cached in `base_yield_rank`.
    pub fn city_find_base_yield_rate_rank(&self, c: CityId, yield_type: YieldType) -> i32 {
        todo!("CvCity::findBaseYieldRateRank")
    }

    /// C++ `CvCity::findYieldRateRank` (CvCity.cpp:1622-1646), cached in `yield_rank`.
    pub fn city_find_yield_rate_rank(&self, c: CityId, yield_type: YieldType) -> i32 {
        todo!("CvCity::findYieldRateRank")
    }

    /// C++ `CvCity::findCommerceRateRank` (CvCity.cpp:1649-1673), cached in `commerce_rank`.
    pub fn city_find_commerce_rate_rank(&self, c: CityId, commerce: CommerceType) -> i32 {
        todo!("CvCity::findCommerceRateRank")
    }

    /// C++ `CvCity::getHandicapType` (CvCity.cpp:4253-4256): the owner's handicap.
    pub fn city_handicap_type(&self, c: CityId) -> HandicapType {
        todo!("CvCity::getHandicapType")
    }

    /// C++ `CvCity::getCivilizationType` (CvCity.cpp:4259-4274). FFH: an assimilating owner
    /// uses the city's own civilization (`civ`).
    pub fn city_civilization_type(&self, c: CityId) -> CivilizationType {
        todo!("CvCity::getCivilizationType")
    }

    /// C++ `CvCity::getPersonalityType` (CvCity.cpp:4277-4280): the owner's personality.
    pub fn city_personality_type(&self, c: CityId) -> LeaderHeadType {
        todo!("CvCity::getPersonalityType")
    }

    /// C++ `CvCity::hasTrait` (CvCity.cpp:4309-4312).
    pub fn city_has_trait(&self, c: CityId, trait_type: TraitType) -> bool {
        todo!("CvCity::hasTrait")
    }

    /// C++ `CvCity::isBarbarian` (CvCity.cpp:4315-4318).
    pub fn city_is_barbarian(&self, c: CityId) -> bool {
        todo!("CvCity::isBarbarian")
    }

    /// C++ `CvCity::isHuman` (CvCity.cpp:4321-4324).
    pub fn city_is_human(&self, c: CityId) -> bool {
        todo!("CvCity::isHuman")
    }

    /// C++ `CvCity::isVisible` (CvCity.cpp:4327-4330): the city plot is visible to `team`.
    pub fn city_is_visible(&self, c: CityId, team: TeamId, debug: bool) -> bool {
        todo!("CvCity::isVisible")
    }

    /// C++ `CvCity::isCapital` (CvCity.cpp:4333-4336).
    pub fn city_is_capital(&self, c: CityId) -> bool {
        todo!("CvCity::isCapital")
    }

    /// C++ `CvCity::isCoastal` (CvCity.cpp:4339-4342).
    pub fn city_is_coastal(&self, c: CityId, min_water_size: i32) -> bool {
        todo!("CvCity::isCoastal")
    }

    /// C++ `CvCity::isDisorder` (CvCity.cpp:4345-4353). FFH: also while the owner's
    /// production is disabled.
    pub fn city_is_disorder(&self, c: CityId) -> bool {
        todo!("CvCity::isDisorder")
    }

    /// C++ `CvCity::isHolyCity(ReligionTypes)` (CvCity.cpp:4356-4359).
    pub fn city_is_holy_city(&self, c: CityId, religion: ReligionType) -> bool {
        todo!("CvCity::isHolyCity")
    }

    /// C++ `CvCity::isHolyCity()` (CvCity.cpp:4362-4375): holy city of any religion.
    pub fn city_is_any_holy_city(&self, c: CityId) -> bool {
        todo!("CvCity::isHolyCity")
    }

    /// C++ `CvCity::cultureDistance` (CvCity.cpp:5161-5164).
    pub fn city_culture_distance(&self, c: CityId, dx: i32, dy: i32) -> i32 {
        todo!("CvCity::cultureDistance")
    }

    /// C++ `CvCity::cultureStrength` (CvCity.cpp:5167-5212): revolt strength of `player`.
    pub fn city_culture_strength(&self, c: CityId, player: PlayerId) -> i32 {
        todo!("CvCity::cultureStrength")
    }

    /// C++ `CvCity::cultureGarrison` (CvCity.cpp:5215-5239).
    pub fn city_culture_garrison(&self, c: CityId, player: PlayerId) -> i32 {
        todo!("CvCity::cultureGarrison")
    }

    /// C++ `CvCity::getReligionCount` (CvCity.cpp:5282-5298).
    pub fn city_religion_count(&self, c: CityId) -> i32 {
        self.cities[c].has_religion.iter().filter(|&&has| has).count() as i32
    }

    /// C++ `CvCity::at(int, int)` (CvCity.cpp:5356-5359).
    pub fn city_at(&self, c: CityId, x: i32, y: i32) -> bool {
        let city = &self.cities[c];
        city.x == x && city.y == y
    }

    /// C++ `CvCity::at(CvPlot*)` (CvCity.cpp:5362-5365).
    pub fn city_at_plot(&self, c: CityId, plot: PlotIdx) -> bool {
        todo!("CvCity::at")
    }

    /// C++ `CvCity::plot` (CvCity.cpp:5368-5371): the city's plot.
    pub fn city_plot(&self, c: CityId) -> PlotIdx {
        todo!("CvCity::plot")
    }

    /// C++ `CvCity::plotGroup` (CvCity.cpp:5374-5377).
    pub fn city_plot_group(&self, c: CityId, player: PlayerId) -> Option<PlotGroupId> {
        todo!("CvCity::plotGroup")
    }

    /// C++ `CvCity::isConnectedTo` (CvCity.cpp:5380-5383).
    pub fn city_is_connected_to(&self, c: CityId, other: CityId) -> bool {
        todo!("CvCity::isConnectedTo")
    }

    /// C++ `CvCity::isConnectedToCapital` (CvCity.cpp:5386-5389). C++ default
    /// `ePlayer = NO_PLAYER` (= the plot's owner, resolved by `CvPlot`).
    pub fn city_is_connected_to_capital(&self, c: CityId, player: Option<PlayerId>) -> bool {
        todo!("CvCity::isConnectedToCapital")
    }

    /// C++ `CvCity::area` (CvCity.cpp:5397-5400); also C++ `CvCity::getArea`
    /// (CvCity.cpp:5392-5395), which returns the same area's id.
    pub fn city_area(&self, c: CityId) -> AreaId {
        todo!("CvCity::area")
    }

    /// C++ `CvCity::waterArea` (CvCity.cpp:5403-5406).
    pub fn city_water_area(&self, c: CityId) -> Option<AreaId> {
        todo!("CvCity::waterArea")
    }

    /// C++ `CvCity::getRallyPlot` (CvCity.cpp:5409-5412): the plot at (`rally_x`, `rally_y`).
    pub fn city_rally_plot(&self, c: CityId) -> Option<PlotIdx> {
        todo!("CvCity::getRallyPlot")
    }

    /// C++ `CvCity::setRallyPlot` (CvCity.cpp:5415-5435). `None` clears it.
    pub fn city_set_rally_plot(&mut self, c: CityId, plot: Option<PlotIdx>) {
        todo!("CvCity::setRallyPlot")
    }

    /// C++ `CvCity::changeExtraTradeRoutes` (CvCity.cpp:7150-7159).
    pub fn city_change_extra_trade_routes(&mut self, c: CityId, change: i32) {
        if change != 0 {
            self.cities[c].extra_trade_routes += change;
            self.city_update_trade_routes(c);
        }
    }

    /// C++ `CvCity::changeTradeRouteModifier` (CvCity.cpp:7167-7175).
    pub fn city_change_trade_route_modifier(&mut self, c: CityId, change: i32) {
        if change != 0 {
            self.cities[c].trade_route_modifier += change;
            self.city_update_trade_routes(c);
        }
    }

    /// C++ `CvCity::changeForeignTradeRouteModifier` (CvCity.cpp:7182-7190).
    pub fn city_change_foreign_trade_route_modifier(&mut self, c: CityId, change: i32) {
        if change != 0 {
            self.cities[c].foreign_trade_route_modifier += change;
            self.city_update_trade_routes(c);
        }
    }

    /// C++ `CvCity::setPlundered` (CvCity.cpp:7625-7633).
    pub fn city_set_plundered(&mut self, c: CityId, new_value: bool) {
        if new_value != self.cities[c].plundered {
            self.cities[c].plundered = new_value;
            self.city_update_trade_routes(c);
        }
    }

    /// C++ `CvCity::getTeam` (CvCity.cpp:7813-7816): the owner's team.
    pub fn city_team(&self, c: CityId) -> TeamId {
        todo!("CvCity::getTeam")
    }

    /// C++ `CvCity::getCultureThreshold()` (CvCity.cpp:7825-7828): culture needed for the
    /// next level.
    pub fn city_culture_threshold(&self, c: CityId) -> i32 {
        todo!("CvCity::getCultureThreshold")
    }

    /// C++ static `CvCity::getCultureThreshold(CultureLevelTypes)` (CvCity.cpp:7830-7838).
    pub fn city_culture_threshold_for_level(&self, level: Option<CultureLevelType>) -> i32 {
        todo!("CvCity::getCultureThreshold")
    }

    /// C++ `CvCity::setCultureLevel` (CvCity.cpp:7841-7971): updates the plots' culture range
    /// counts. Python event `cultureExpansion`.
    pub fn city_set_culture_level(&mut self, c: CityId, new_value: Option<CultureLevelType>, update_plot_groups: bool) {
        todo!("CvCity::setCultureLevel")
    }

    /// C++ `CvCity::updateCultureLevel` (CvCity.cpp:7974-7996).
    pub fn city_update_culture_level(&mut self, c: CityId, update_plot_groups: bool) {
        todo!("CvCity::updateCultureLevel")
    }

    /// C++ `CvCity::changeSeaPlotYield` (CvCity.cpp:8007-8019).
    pub fn city_change_sea_plot_yield(&mut self, c: CityId, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.cities[c].sea_plot_yield[yield_type.idx()] += change;
            self.city_update_yield(c);
        }
    }

    /// C++ `CvCity::changeRiverPlotYield` (CvCity.cpp:8030-8042).
    pub fn city_change_river_plot_yield(&mut self, c: CityId, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.cities[c].river_plot_yield[yield_type.idx()] += change;
            self.city_update_yield(c);
        }
    }

    /// C++ `CvCity::getBaseYieldRateModifier` (CvCity.cpp:8053-8091). FFH: settlements -75%.
    /// C++ default `iExtra = 0`.
    pub fn city_base_yield_rate_modifier(&self, c: CityId, yield_type: YieldType, extra: i32) -> i32 {
        todo!("CvCity::getBaseYieldRateModifier")
    }

    /// C++ `CvCity::getYieldRate` (CvCity.cpp:8094-8097).
    pub fn city_yield_rate(&self, c: CityId, yield_type: YieldType) -> i32 {
        (self.cities[c].base_yield_rate[yield_type.idx()] * self.city_base_yield_rate_modifier(c, yield_type, 0)) / 100
    }

    /// C++ `CvCity::setBaseYieldRate` (CvCity.cpp:8100-8126).
    pub fn city_set_base_yield_rate(&mut self, c: CityId, yield_type: YieldType, new_value: i32) {
        if self.cities[c].base_yield_rate[yield_type.idx()] != new_value {
            self.cities[c].base_yield_rate[yield_type.idx()] = new_value;
            self.city_update_commerces(c);
        }
    }

    /// C++ `CvCity::changeBaseYieldRate` (CvCity.cpp:8129-8132).
    pub fn city_change_base_yield_rate(&mut self, c: CityId, yield_type: YieldType, change: i32) {
        let new_value = self.cities[c].base_yield_rate[yield_type.idx()] + change;
        self.city_set_base_yield_rate(c, yield_type, new_value);
    }

    /// C++ `CvCity::changeYieldRateModifier` (CvCity.cpp:8143-8167).
    pub fn city_change_yield_rate_modifier(&mut self, c: CityId, yield_type: YieldType, change: i32) {
        todo!("CvCity::changeYieldRateModifier")
    }

    /// C++ `CvCity::changePowerYieldRateModifier` (CvCity.cpp:8178-8202).
    pub fn city_change_power_yield_rate_modifier(&mut self, c: CityId, yield_type: YieldType, change: i32) {
        todo!("CvCity::changePowerYieldRateModifier")
    }

    /// C++ `CvCity::changeBonusYieldRateModifier` (CvCity.cpp:8213-8237).
    pub fn city_change_bonus_yield_rate_modifier(&mut self, c: CityId, yield_type: YieldType, change: i32) {
        todo!("CvCity::changeBonusYieldRateModifier")
    }

    /// C++ `CvCity::totalTradeModifier` (CvCity.cpp:8248-8277). C++ default
    /// `pOtherCity = NULL`.
    pub fn city_total_trade_modifier(&self, c: CityId, other: Option<CityId>) -> i32 {
        todo!("CvCity::totalTradeModifier")
    }

    /// C++ `CvCity::getPopulationTradeModifier` (CvCity.cpp:8279-8282).
    pub fn city_population_trade_modifier(&self, c: CityId) -> i32 {
        todo!("CvCity::getPopulationTradeModifier")
    }

    /// C++ `CvCity::getPeaceTradeModifier` (CvCity.cpp:8284-8302).
    pub fn city_peace_trade_modifier(&self, c: CityId, team: TeamId) -> i32 {
        todo!("CvCity::getPeaceTradeModifier")
    }

    /// C++ `CvCity::getBaseTradeProfit` (CvCity.cpp:8304-8314).
    pub fn city_base_trade_profit(&self, c: CityId, other: CityId) -> i32 {
        todo!("CvCity::getBaseTradeProfit")
    }

    /// C++ `CvCity::calculateTradeProfit` (CvCity.cpp:8316-8324).
    pub fn city_calculate_trade_profit(&self, c: CityId, other: CityId) -> i32 {
        let mut profit = self.city_base_trade_profit(c, other);
        profit *= self.city_total_trade_modifier(c, Some(other));
        profit /= 10000;
        profit
    }

    /// C++ `CvCity::calculateTradeYield` (CvCity.cpp:8327-8337).
    pub fn city_calculate_trade_yield(&self, c: CityId, yield_type: YieldType, trade_profit: i32) -> i32 {
        todo!("CvCity::calculateTradeYield")
    }

    /// C++ `CvCity::setTradeYield` (CvCity.cpp:8340-8356).
    pub fn city_set_trade_yield(&mut self, c: CityId, yield_type: YieldType, new_value: i32) {
        let old_value = self.cities[c].trade_yield[yield_type.idx()];
        if old_value != new_value {
            self.cities[c].trade_yield[yield_type.idx()] = new_value;
            self.city_change_base_yield_rate(c, yield_type, new_value - old_value);
        }
    }

    /// C++ `CvCity::getExtraSpecialistYield(YieldTypes, SpecialistTypes)`
    /// (CvCity.cpp:8367-8374). (The one-argument overload is the field
    /// `extra_specialist_yield[y]`.)
    pub fn city_extra_specialist_yield(&self, c: CityId, yield_type: YieldType, specialist: SpecialistType) -> i32 {
        todo!("CvCity::getExtraSpecialistYield")
    }

    /// C++ `CvCity::updateExtraSpecialistYield(YieldTypes)` (CvCity.cpp:8377-8402).
    pub fn city_update_extra_specialist_yield(&mut self, c: CityId, yield_type: YieldType) {
        let old_yield = self.cities[c].extra_specialist_yield[yield_type.idx()];
        let mut new_yield = 0;
        for specialist in self.d.specialists.keys() {
            new_yield += self.city_extra_specialist_yield(c, yield_type, specialist);
        }
        if old_yield != new_yield {
            self.cities[c].extra_specialist_yield[yield_type.idx()] = new_yield;
            self.city_change_base_yield_rate(c, yield_type, new_yield - old_yield);
        }
    }

    /// C++ `CvCity::updateExtraSpecialistYield()` (CvCity.cpp:8405-8413).
    pub fn city_update_extra_specialist_yields(&mut self, c: CityId) {
        for i in 0..YieldType::NUM {
            self.city_update_extra_specialist_yield(c, YieldType::from_usize(i));
        }
    }

    /// C++ `CvCity::getCommerceRate` (CvCity.cpp:8416-8419).
    pub fn city_commerce_rate(&self, c: CityId, commerce: CommerceType) -> i32 {
        self.city_commerce_rate_times100(c, commerce) / 100
    }

    /// C++ `CvCity::getCommerceRateTimes100` (CvCity.cpp:8421-8440): the `commerce_rate`
    /// field, with espionage folded into culture under the no-espionage option.
    pub fn city_commerce_rate_times100(&self, c: CityId, commerce: CommerceType) -> i32 {
        todo!("CvCity::getCommerceRateTimes100")
    }

    /// C++ `CvCity::getCommerceFromPercent` (CvCity.cpp:8443-8455).
    pub fn city_commerce_from_percent(&self, c: CityId, commerce: CommerceType, yield_rate: i32) -> i32 {
        todo!("CvCity::getCommerceFromPercent")
    }

    /// C++ `CvCity::getBaseCommerceRate` (CvCity.cpp:8458-8461).
    pub fn city_base_commerce_rate(&self, c: CityId, commerce: CommerceType) -> i32 {
        self.city_base_commerce_rate_times100(c, commerce) / 100
    }

    /// C++ `CvCity::getBaseCommerceRateTimes100` (CvCity.cpp:8463-8480). FFH: per-specialist
    /// type extra commerce.
    pub fn city_base_commerce_rate_times100(&self, c: CityId, commerce: CommerceType) -> i32 {
        todo!("CvCity::getBaseCommerceRateTimes100")
    }

    /// C++ `CvCity::getTotalCommerceRateModifier` (CvCity.cpp:8483-8486).
    pub fn city_total_commerce_rate_modifier(&self, c: CityId, commerce: CommerceType) -> i32 {
        todo!("CvCity::getTotalCommerceRateModifier")
    }

    /// C++ `CvCity::updateCommerce(CommerceTypes)` (CvCity.cpp:8489-8524).
    pub fn city_update_commerce(&mut self, c: CityId, commerce: CommerceType) {
        todo!("CvCity::updateCommerce")
    }

    /// C++ `CvCity::updateCommerce()` (CvCity.cpp:8527-8535).
    pub fn city_update_commerces(&mut self, c: CityId) {
        todo!("CvCity::updateCommerce")
    }

    /// C++ `CvCity::changeProductionToCommerceModifier` (CvCity.cpp:8546-8557).
    pub fn city_change_production_to_commerce_modifier(&mut self, c: CityId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.cities[c].production_to_commerce_modifier[commerce.idx()] += change;
            self.city_update_commerce(c, commerce);
        }
    }

    /// C++ `CvCity::getBuildingCommerceByBuilding` (CvCity.cpp:8568-8640). Sephi fix: the
    /// commerce-doubling time is in game turns scaled by the speed's construct percent.
    pub fn city_building_commerce_by_building(&self, c: CityId, commerce: CommerceType, building: BuildingType) -> i32 {
        todo!("CvCity::getBuildingCommerceByBuilding")
    }

    /// C++ `CvCity::updateBuildingCommerce` (CvCity.cpp:8643-8665).
    pub fn city_update_building_commerce(&mut self, c: CityId) {
        for i in 0..CommerceType::NUM {
            let commerce = CommerceType::from_usize(i);
            let mut new_building_commerce = 0;
            for building in self.d.buildings.keys() {
                new_building_commerce += self.city_building_commerce_by_building(c, commerce, building);
            }
            if self.cities[c].building_commerce[i] != new_building_commerce {
                self.cities[c].building_commerce[i] = new_building_commerce;
                self.city_update_commerce(c, commerce);
            }
        }
    }

    /// C++ `CvCity::changeSpecialistCommerce` (CvCity.cpp:8676-8688).
    pub fn city_change_specialist_commerce(&mut self, c: CityId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.cities[c].specialist_commerce[commerce.idx()] += change;
            self.city_update_commerce(c, commerce);
        }
    }

    /// C++ `CvCity::getReligionCommerceByReligion` (CvCity.cpp:8699-8724).
    pub fn city_religion_commerce_by_religion(&self, c: CityId, commerce: CommerceType, religion: ReligionType) -> i32 {
        todo!("CvCity::getReligionCommerceByReligion")
    }

    /// C++ `CvCity::updateReligionCommerce(CommerceTypes)` (CvCity.cpp:8728-8747).
    pub fn city_update_religion_commerce(&mut self, c: CityId, commerce: CommerceType) {
        let mut new_religion_commerce = 0;
        for religion in self.d.religions.keys() {
            new_religion_commerce += self.city_religion_commerce_by_religion(c, commerce, religion);
        }
        if self.cities[c].religion_commerce[commerce.idx()] != new_religion_commerce {
            self.cities[c].religion_commerce[commerce.idx()] = new_religion_commerce;
            self.city_update_commerce(c, commerce);
        }
    }

    /// C++ `CvCity::updateReligionCommerce()` (CvCity.cpp:8750-8758).
    pub fn city_update_religion_commerces(&mut self, c: CityId) {
        for i in 0..CommerceType::NUM {
            self.city_update_religion_commerce(c, CommerceType::from_usize(i));
        }
    }

    /// C++ `CvCity::changeCommerceRateModifier` (CvCity.cpp:8990-9003).
    pub fn city_change_commerce_rate_modifier(&mut self, c: CityId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.cities[c].commerce_rate_modifier[commerce.idx()] += change;
            self.city_update_commerce(c, commerce);
            self.cities[c].ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::getCulture` (CvCity.cpp:9084-9089). (`getCultureTimes100` is the field
    /// `culture[p]`.)
    pub fn city_culture(&self, c: CityId, player: PlayerId) -> i32 {
        self.cities[c].culture[player] / 100
    }

    /// C++ `CvCity::countTotalCultureTimes100` (CvCity.cpp:9099-9115): over alive players.
    pub fn city_count_total_culture_times100(&self, c: CityId) -> i32 {
        todo!("CvCity::countTotalCultureTimes100")
    }

    /// C++ `CvCity::findHighestCulture` (CvCity.cpp:9118-9143). C++ `NO_PLAYER` = `None`.
    pub fn city_find_highest_culture(&self, c: CityId) -> Option<PlayerId> {
        todo!("CvCity::findHighestCulture")
    }

    /// C++ `CvCity::calculateCulturePercent` (CvCity.cpp:9146-9158).
    pub fn city_calculate_culture_percent(&self, c: CityId, player: PlayerId) -> i32 {
        let total_culture = self.city_count_total_culture_times100(c);
        if total_culture > 0 {
            return (self.cities[c].culture[player] * 100) / total_culture;
        }
        0
    }

    /// C++ `CvCity::calculateTeamCulturePercent` (CvCity.cpp:9161-9180).
    pub fn city_calculate_team_culture_percent(&self, c: CityId, team: TeamId) -> i32 {
        todo!("CvCity::calculateTeamCulturePercent")
    }

    /// C++ `CvCity::setCulture` (CvCity.cpp:9183-9186).
    pub fn city_set_culture(&mut self, c: CityId, player: PlayerId, new_value: i32, plots: bool, update_plot_groups: bool) {
        self.city_set_culture_times100(c, player, 100 * new_value, plots, update_plot_groups);
    }

    /// C++ `CvCity::setCultureTimes100` (CvCity.cpp:9188-9205).
    pub fn city_set_culture_times100(&mut self, c: CityId, player: PlayerId, new_value: i32, plots: bool, update_plot_groups: bool) {
        if self.cities[c].culture[player] != new_value {
            self.cities[c].culture[player] = new_value;
            self.city_update_culture_level(c, update_plot_groups);
            if plots {
                self.city_do_plot_culture(c, true, player, 0);
            }
        }
    }

    /// C++ `CvCity::changeCulture` (CvCity.cpp:9208-9211).
    pub fn city_change_culture(&mut self, c: CityId, player: PlayerId, change: i32, plots: bool, update_plot_groups: bool) {
        let new_value = self.cities[c].culture[player] + 100 * change;
        self.city_set_culture_times100(c, player, new_value, plots, update_plot_groups);
    }

    /// C++ `CvCity::changeCultureTimes100` (CvCity.cpp:9213-9216).
    pub fn city_change_culture_times100(&mut self, c: CityId, player: PlayerId, change: i32, plots: bool, update_plot_groups: bool) {
        let new_value = self.cities[c].culture[player] + change;
        self.city_set_culture_times100(c, player, new_value, plots, update_plot_groups);
    }

    /// C++ `CvCity::getRevoltTestProbability` (CvCity.cpp:9235-9253).
    pub fn city_revolt_test_probability(&self, c: CityId) -> i32 {
        todo!("CvCity::getRevoltTestProbability")
    }

    /// C++ `CvCity::isRevealed` (CvCity.cpp:9290-9303): the `revealed` field, or true for
    /// `debug` queries in debug mode (`CvGame::isDebugMode`). Rules code passes
    /// `debug = false`.
    pub fn city_is_revealed(&self, c: CityId, team: TeamId, debug: bool) -> bool {
        todo!("CvCity::isRevealed")
    }

    /// C++ `CvCity::setEspionageVisibility` (CvCity.cpp:9345-9358): updates the city plot's
    /// sight around the change.
    pub fn city_set_espionage_visibility(&mut self, c: CityId, team: TeamId, new_value: bool, update_plot_groups: bool) {
        todo!("CvCity::setEspionageVisibility")
    }

    /// C++ `CvCity::updateEspionageVisibility` (CvCity.cpp:9360-9404).
    pub fn city_update_espionage_visibility(&mut self, c: CityId, update_plot_groups: bool) {
        todo!("CvCity::updateEspionageVisibility")
    }

    /// C++ `CvCity::setHasReligion` (CvCity.cpp:10688-10810). FFH: disabled religions,
    /// founding a religion makes this its holy city, global counter. Python events
    /// `religionSpread` / `religionRemove`. C++ default `bArrows = true`.
    pub fn city_set_has_religion(&mut self, c: CityId, religion: ReligionType, new_value: bool, announce: bool, arrows: bool) {
        todo!("CvCity::setHasReligion")
    }

    /// C++ `CvCity::processVoteSourceBonus` (CvCity.cpp:10813-10876).
    pub fn city_process_vote_source_bonus(&mut self, c: CityId, vote_source: VoteSourceType, active: bool) {
        todo!("CvCity::processVoteSourceBonus")
    }

    /// C++ `CvCity::getTradeCity` (CvCity.cpp:10996-11001): `None` if the slot is empty or
    /// the city no longer exists (C++ `getCity(IDInfo)` returns NULL).
    pub fn city_trade_city(&self, c: CityId, index: usize) -> Option<CityId> {
        self.cities[c].trade_cities[index].filter(|&t| self.cities.contains(t))
    }

    /// C++ `CvCity::getTradeRoutes` (CvCity.cpp:11004-11017).
    pub fn city_trade_routes(&self, c: CityId) -> i32 {
        todo!("CvCity::getTradeRoutes")
    }

    /// C++ `CvCity::clearTradeRoutes` (CvCity.cpp:11020-11036).
    pub fn city_clear_trade_routes(&mut self, c: CityId) {
        let owner = self.cities[c].owner;
        for i in 0..self.cities[c].trade_cities.len() {
            if let Some(trade_city) = self.city_trade_city(c, i) {
                self.cities[trade_city].trade_route[owner] = false;
            }
            self.cities[c].trade_cities[i] = None;
        }
    }

    /// C++ `CvCity::updateTradeRoutes` (CvCity.cpp:11040-11123).
    pub fn city_update_trade_routes(&mut self, c: CityId) {
        todo!("CvCity::updateTradeRoutes")
    }

    /// C++ `CvCity::doCulture` (CvCity.cpp:11803-11817). Python `doCulture` callback.
    pub fn city_do_culture(&mut self, c: CityId) {
        todo!("CvCity::doCulture")
    }

    /// C++ `CvCity::doPlotCulture` (CvCity.cpp:11820-11886): spreads `player`'s culture to
    /// the plots in range. Python `doPlotCulture` callback.
    pub fn city_do_plot_culture(&mut self, c: CityId, update: bool, player: PlayerId, culture_rate: i32) {
        todo!("CvCity::doPlotCulture")
    }

    /// C++ `CvCity::doReligion` (CvCity.cpp:12145-12215): spread of religions into a city
    /// without any; draws "Religion Spread". Python `doReligion` callback.
    pub fn city_do_religion(&mut self, c: CityId) {
        todo!("CvCity::doReligion")
    }

    /// C++ `CvCity::hasShrine` (CvCity.cpp:13693-13711).
    pub fn city_has_shrine(&self, c: CityId, religion: ReligionType) -> bool {
        todo!("CvCity::hasShrine")
    }

    /// C++ `CvCity::invalidatePopulationRankCache` (CvCity.cpp:13713-13716).
    pub fn city_invalidate_population_rank_cache(&mut self, c: CityId) {
        self.cities[c].population_rank_valid.set(false);
    }

    /// C++ `CvCity::invalidateYieldRankCache` (CvCity.cpp:13718-13735). C++ default
    /// `eYield = NO_YIELD` (= all yields).
    pub fn city_invalidate_yield_rank_cache(&mut self, c: CityId, yield_type: Option<YieldType>) {
        let city = &self.cities[c];
        match yield_type {
            None => {
                for i in 0..YieldType::NUM {
                    city.base_yield_rank_valid[i].set(false);
                    city.yield_rank_valid[i].set(false);
                }
            }
            Some(y) => {
                city.base_yield_rank_valid[y.idx()].set(false);
                city.yield_rank_valid[y.idx()].set(false);
            }
        }
    }

    /// C++ `CvCity::invalidateCommerceRankCache` (CvCity.cpp:13737-13752). C++ default
    /// `eCommerce = NO_COMMERCE` (= all commerce types).
    pub fn city_invalidate_commerce_rank_cache(&mut self, c: CityId, commerce: Option<CommerceType>) {
        let city = &self.cities[c];
        match commerce {
            None => {
                for i in 0..CommerceType::NUM {
                    city.commerce_rank_valid[i].set(false);
                }
            }
            Some(ct) => city.commerce_rank_valid[ct.idx()].set(false),
        }
    }

    /// C++ `CvCity::getExtraSpecialistCommerce(CommerceTypes)` (CvCity.cpp:14720-14728, FFH).
    pub fn city_extra_specialist_commerce(&self, c: CityId, commerce: CommerceType) -> i32 {
        let mut count = 0;
        for specialist in self.d.specialists.keys() {
            count += self.city_extra_specialist_commerce_by_specialist(c, commerce, specialist);
        }
        count
    }

    /// C++ `CvCity::getExtraSpecialistCommerce(CommerceTypes, SpecialistTypes)`
    /// (CvCity.cpp:14730-14733, FFH).
    pub fn city_extra_specialist_commerce_by_specialist(&self, c: CityId, commerce: CommerceType, specialist: SpecialistType) -> i32 {
        todo!("CvCity::getExtraSpecialistCommerce")
    }
}
