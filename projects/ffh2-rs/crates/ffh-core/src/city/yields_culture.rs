//! CvCity: yields, commerce, trade routes, culture and culture level, religion, espionage
//! visibility, the rank caches, and owner/location helpers (`getTeam`, `plot`, `area`,
//! `isHuman`, `isCapital`, `isDisorder`, ...).

use std::cmp::{max, min};

use super::range;
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

/// Python `CvGameUtils.doCulture(argsList)`, called unconditionally by `CvCity::doCulture`
/// (true = the Python handled it and the DLL skips its own culture growth). FFH2 returns False.
fn py_do_culture(_g: &Game, _c: CityId) -> bool {
    false
}

/// Python `CvGameUtils.doPlotCulture(argsList)`, called unconditionally by
/// `CvCity::doPlotCulture` (true = skip the DLL's plot culture). FFH2 returns False.
fn py_do_plot_culture(_g: &Game, _c: CityId, _update: bool, _player: PlayerId, _culture_rate: i32) -> bool {
    false
}

/// Python `CvGameUtils.doReligion(argsList)`, called unconditionally by `CvCity::doReligion`
/// (true = skip the DLL's religion spread). FFH2 returns False.
fn py_do_religion(_g: &Game, _c: CityId) -> bool {
    false
}

/// C++ `(BuildingClassTypes)GC.getBuildingInfo(eBuilding).getBuildingClassType()`: every
/// building has a class (checked by the ffh-data tests).
fn building_class(d: &GameData, building: BuildingType) -> BuildingClassType {
    d.buildings[building].building_class_type.expect("building without a building class")
}

impl Game {
    /// C++ `CvCity::updateYield` (CvCity.cpp:1213-1227): recomputes the yields of the city's
    /// plots.
    pub fn city_update_yield(&mut self, c: CityId) {
        for i in 0..NUM_CITY_PLOTS {
            if let Some(loop_plot) = self.city_index_plot(c, i) {
                self.plot_update_yield(loop_plot);
            }
        }
    }

    // ---- rank caches ----
    //
    // C++ breaks ties by `CvCity::getID()`. Within one player's FFreeListTrashArray the ids grow
    // with creation order (`m_iCurrentID + iIndex`, `m_iCurrentID` += FLTA_MAX_BUCKETS per add),
    // and so do the global `CityId`s, so comparing `CityId`s ranks the owner's cities the same.

    /// C++ `CvCity::findPopulationRank` (CvCity.cpp:1569-1592): rank among the owner's
    /// cities, cached in `population_rank` (C++ `mutable`).
    pub fn city_find_population_rank(&self, c: CityId) -> i32 {
        let city = &self.cities[c];
        if !city.population_rank_valid.get() {
            let mut rank = 1;

            for &loop_city in &self.players[city.owner].cities {
                let loop_population = self.cities[loop_city].population;
                if (loop_population > city.population) || ((loop_population == city.population) && (loop_city < c)) {
                    rank += 1;
                }
            }

            // shenanigans are to get around the const check
            city.population_rank_valid.set(true);
            city.population_rank.set(rank);
        }

        city.population_rank.get()
    }

    /// C++ `CvCity::findBaseYieldRateRank` (CvCity.cpp:1595-1619), cached in `base_yield_rank`.
    pub fn city_find_base_yield_rate_rank(&self, c: CityId, yield_type: YieldType) -> i32 {
        let y = yield_type.idx();
        let city = &self.cities[c];
        if !city.base_yield_rank_valid[y].get() {
            let rate = city.base_yield_rate[y];

            let mut rank = 1;

            for &loop_city in &self.players[city.owner].cities {
                let loop_rate = self.cities[loop_city].base_yield_rate[y];
                if (loop_rate > rate) || ((loop_rate == rate) && (loop_city < c)) {
                    rank += 1;
                }
            }

            city.base_yield_rank_valid[y].set(true);
            city.base_yield_rank[y].set(rank);
        }

        city.base_yield_rank[y].get()
    }

    /// C++ `CvCity::findYieldRateRank` (CvCity.cpp:1622-1646), cached in `yield_rank`.
    pub fn city_find_yield_rate_rank(&self, c: CityId, yield_type: YieldType) -> i32 {
        let y = yield_type.idx();
        let city = &self.cities[c];
        if !city.yield_rank_valid[y].get() {
            let rate = self.city_yield_rate(c, yield_type);

            let mut rank = 1;

            for &loop_city in &self.players[city.owner].cities {
                let loop_rate = self.city_yield_rate(loop_city, yield_type);
                if (loop_rate > rate) || ((loop_rate == rate) && (loop_city < c)) {
                    rank += 1;
                }
            }

            city.yield_rank_valid[y].set(true);
            city.yield_rank[y].set(rank);
        }

        city.yield_rank[y].get()
    }

    /// C++ `CvCity::findCommerceRateRank` (CvCity.cpp:1649-1673), cached in `commerce_rank`.
    pub fn city_find_commerce_rate_rank(&self, c: CityId, commerce: CommerceType) -> i32 {
        let i = commerce.idx();
        let city = &self.cities[c];
        if !city.commerce_rank_valid[i].get() {
            let rate = self.city_commerce_rate_times100(c, commerce);

            let mut rank = 1;

            for &loop_city in &self.players[city.owner].cities {
                let loop_rate = self.city_commerce_rate_times100(loop_city, commerce);
                if (loop_rate > rate) || ((loop_rate == rate) && (loop_city < c)) {
                    rank += 1;
                }
            }

            city.commerce_rank_valid[i].set(true);
            city.commerce_rank[i].set(rank);
        }

        city.commerce_rank[i].get()
    }

    // ---- owner helpers ----

    /// C++ `CvCity::getHandicapType` (CvCity.cpp:4253-4256): the owner's handicap.
    pub fn city_handicap_type(&self, c: CityId) -> HandicapType {
        self.player_get_handicap_type(self.cities[c].owner).expect("city owner without a handicap")
    }

    /// C++ `CvCity::getCivilizationType` (CvCity.cpp:4259-4274). FFH: an assimilating owner
    /// uses the city's own civilization (`civ`).
    pub fn city_civilization_type(&self, c: CityId) -> CivilizationType {
        let city = &self.cities[c];

        // FfH: Added by Kael 07/05/2008
        if self.players[city.owner].assimilation {
            if let Some(civ) = city.civ {
                return civ;
            }
        }
        // FfH: End Add

        self.player_get_civilization_type(city.owner).expect("city owner without a civilization")
    }

    /// C++ `CvCity::getPersonalityType` (CvCity.cpp:4277-4280): the owner's personality.
    pub fn city_personality_type(&self, c: CityId) -> LeaderHeadType {
        self.players[self.cities[c].owner].personality_type.expect("city owner without a personality")
    }

    /// C++ `CvCity::hasTrait` (CvCity.cpp:4309-4312).
    pub fn city_has_trait(&self, c: CityId, trait_type: TraitType) -> bool {
        self.players[self.cities[c].owner].traits[trait_type]
    }

    /// C++ `CvCity::isBarbarian` (CvCity.cpp:4315-4318).
    pub fn city_is_barbarian(&self, c: CityId) -> bool {
        self.player_is_barbarian(self.cities[c].owner)
    }

    /// C++ `CvCity::isHuman` (CvCity.cpp:4321-4324).
    pub fn city_is_human(&self, c: CityId) -> bool {
        self.players[self.cities[c].owner].human
    }

    /// C++ `CvCity::isVisible` (CvCity.cpp:4327-4330): the city plot is visible to `team`.
    /// (`bDebug` only matters in debug mode, which is UI: never on.)
    pub fn city_is_visible(&self, c: CityId, team: TeamId, _debug: bool) -> bool {
        self.plot_is_visible(self.city_plot(c), team)
    }

    /// C++ `CvCity::isCapital` (CvCity.cpp:4333-4336).
    pub fn city_is_capital(&self, c: CityId) -> bool {
        self.player_get_capital_city(self.cities[c].owner) == Some(c)
    }

    /// C++ `CvCity::isCoastal` (CvCity.cpp:4339-4342).
    pub fn city_is_coastal(&self, c: CityId, min_water_size: i32) -> bool {
        self.plot_is_coastal_land(self.city_plot(c), min_water_size)
    }

    /// C++ `CvCity::isDisorder` (CvCity.cpp:4345-4353). FFH: also while the owner's
    /// production is disabled.
    pub fn city_is_disorder(&self, c: CityId) -> bool {
        let owner = self.cities[c].owner;

        // FfH: Modified by Kael 10/03/2008
        self.city_is_occupation(c) || self.player_is_anarchy(owner) || self.players[owner].disable_production > 0
        // FfH: End Modify
    }

    /// C++ `CvCity::isHolyCity(ReligionTypes)` (CvCity.cpp:4356-4359).
    pub fn city_is_holy_city(&self, c: CityId, religion: ReligionType) -> bool {
        self.get_holy_city(religion) == Some(c)
    }

    /// C++ `CvCity::isHolyCity()` (CvCity.cpp:4362-4375): holy city of any religion.
    pub fn city_is_any_holy_city(&self, c: CityId) -> bool {
        for religion in self.d.religions.keys() {
            if self.city_is_holy_city(c, religion) {
                return true;
            }
        }

        false
    }

    // ---- culture strength (revolts) ----

    /// C++ `CvCity::cultureDistance` (CvCity.cpp:5161-5164).
    pub fn city_culture_distance(&self, _c: CityId, dx: i32, dy: i32) -> i32 {
        max(1, self.plot_distance(0, 0, dx, dy))
    }

    /// C++ `CvCity::cultureStrength` (CvCity.cpp:5167-5212): revolt strength of `player`.
    pub fn city_culture_strength(&self, c: CityId, player: PlayerId) -> i32 {
        let d = self.d;
        let city = &self.cities[c];
        let owner = city.owner;

        let mut strength = 1;

        strength += city.highest_population * 2;

        for i in 0..DirectionType::NUM {
            if let Some(loop_plot) = self.plot_direction(city.x, city.y, DirectionType::from_usize(i)) {
                if self.map.plots[loop_plot].owner == Some(player) {
                    strength += opt_to_i32(self.current_era()) + 1;
                }
            }
        }

        let plot = &self.map.plots[self.city_plot(c)];
        strength *= max(
            0,
            ((d.defines.revolt_total_culture_modifier * (plot.culture[player] - plot.culture[owner] + 1)) / (plot.culture[player] + 1)) + 100,
        );
        strength /= 100;

        if let Some(state_religion) = self.player_get_state_religion(player) {
            if city.has_religion[state_religion] {
                strength *= max(0, d.defines.revolt_offense_state_religion_modifier + 100);
                strength /= 100;
            }
        }

        if let Some(state_religion) = self.player_get_state_religion(owner) {
            if city.has_religion[state_religion] {
                strength *= max(0, d.defines.revolt_defense_state_religion_modifier + 100);
                strength /= 100;
            }
        }

        strength
    }

    /// C++ `CvCity::cultureGarrison` (CvCity.cpp:5215-5239).
    pub fn city_culture_garrison(&self, c: CityId, player: PlayerId) -> i32 {
        let mut garrison = 1;

        for &loop_unit in &self.map.plots[self.city_plot(c)].units {
            garrison += self.d.units[self.units[loop_unit].unit_type].culture_garrison_value;
        }

        if self.at_war(self.players[player].team, self.city_team(c)) {
            garrison *= 2;
        }

        garrison
    }

    /// C++ `CvCity::getReligionCount` (CvCity.cpp:5282-5298).
    pub fn city_religion_count(&self, c: CityId) -> i32 {
        self.cities[c].has_religion.iter().filter(|&&has| has).count() as i32
    }

    // ---- location ----

    /// C++ `CvCity::at(int, int)` (CvCity.cpp:5356-5359).
    pub fn city_at(&self, c: CityId, x: i32, y: i32) -> bool {
        let city = &self.cities[c];
        city.x == x && city.y == y
    }

    /// C++ `CvCity::at(CvPlot*)` (CvCity.cpp:5362-5365).
    pub fn city_at_plot(&self, c: CityId, plot: PlotIdx) -> bool {
        self.city_plot(c) == plot
    }

    /// C++ `CvCity::plot` (CvCity.cpp:5368-5371): the city's plot.
    pub fn city_plot(&self, c: CityId) -> PlotIdx {
        let city = &self.cities[c];
        self.map.plot_soren(city.x, city.y).expect("city at INVALID_PLOT_COORD")
    }

    /// C++ `CvCity::plotGroup` (CvCity.cpp:5374-5377).
    pub fn city_plot_group(&self, c: CityId, player: PlayerId) -> Option<PlotGroupId> {
        self.plot_get_plot_group(self.city_plot(c), player)
    }

    /// C++ `CvCity::isConnectedTo` (CvCity.cpp:5380-5383).
    pub fn city_is_connected_to(&self, c: CityId, other: CityId) -> bool {
        self.plot_is_connected_to(self.city_plot(c), other)
    }

    /// C++ `CvCity::isConnectedToCapital` (CvCity.cpp:5386-5389). C++ default
    /// `ePlayer = NO_PLAYER` (= the plot's owner, resolved by `CvPlot`).
    pub fn city_is_connected_to_capital(&self, c: CityId, player: Option<PlayerId>) -> bool {
        self.plot_is_connected_to_capital(self.city_plot(c), player)
    }

    /// C++ `CvCity::area` (CvCity.cpp:5397-5400); also C++ `CvCity::getArea`
    /// (CvCity.cpp:5392-5395), which returns the same area's id.
    pub fn city_area(&self, c: CityId) -> AreaId {
        self.plot_area(self.city_plot(c)).expect("CvCity::area() is NULL (areas not calculated)")
    }

    /// C++ `CvCity::waterArea` (CvCity.cpp:5403-5406).
    pub fn city_water_area(&self, c: CityId) -> Option<AreaId> {
        self.plot_water_area(self.city_plot(c))
    }

    /// C++ `CvCity::getRallyPlot` (CvCity.cpp:5409-5412): the plot at (`rally_x`, `rally_y`).
    pub fn city_rally_plot(&self, c: CityId) -> Option<PlotIdx> {
        let city = &self.cities[c];
        self.map.plot_soren(city.rally_x, city.rally_y)
    }

    /// C++ `CvCity::setRallyPlot` (CvCity.cpp:5415-5435). `None` clears it.
    pub fn city_set_rally_plot(&mut self, c: CityId, plot: Option<PlotIdx>) {
        if self.city_rally_plot(c) != plot {
            let (x, y) = match plot {
                Some(p) => self.map.xy(p),
                None => (INVALID_PLOT_COORD, INVALID_PLOT_COORD),
            };
            let city = &mut self.cities[c];
            city.rally_x = x;
            city.rally_y = y;

            // isCitySelected(): ColoredPlots dirty bit (UI).
        }
    }

    // ---- trade route modifiers ----

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
        self.players[self.cities[c].owner].team
    }

    // ---- culture level ----

    /// C++ `CvCity::getCultureThreshold()` (CvCity.cpp:7825-7828): culture needed for the
    /// next level.
    pub fn city_culture_threshold(&self, c: CityId) -> i32 {
        self.city_culture_threshold_for_level(self.cities[c].culture_level)
    }

    /// C++ static `CvCity::getCultureThreshold(CultureLevelTypes)` (CvCity.cpp:7830-7838).
    pub fn city_culture_threshold_for_level(&self, level: Option<CultureLevelType>) -> i32 {
        let Some(level) = level else {
            return 1;
        };

        let next_level = min(level.idx() as i32 + 1, self.d.culture_levels.len() as i32 - 1);
        max(1, self.culture_threshold(CultureLevelType::from_usize(next_level as usize)))
    }

    /// C++ `CvCity::setCultureLevel` (CvCity.cpp:7841-7971): updates the plots' culture range
    /// counts. Python event `cultureExpansion`.
    pub fn city_set_culture_level(&mut self, c: CityId, new_value: Option<CultureLevelType>, update_plot_groups: bool) {
        let old_value = self.cities[c].culture_level;

        if old_value != new_value {
            self.cities[c].culture_level = new_value;

            // C++ compares the levels as ints (NO_CULTURELEVEL = -1). Neither the owner nor the
            // culture level can change inside the loops (`CvPlot::updateCulture` never changes a
            // city plot's owner), so they are read once.
            let old_level = opt_to_i32(old_value);
            let new_level = opt_to_i32(new_value);
            let (x, y, owner) = {
                let city = &self.cities[c];
                (city.x, city.y, city.owner)
            };

            if old_value.is_some() {
                for dx in -old_level..=old_level {
                    for dy in -old_level..=old_level {
                        let culture_range = self.city_culture_distance(c, dx, dy);

                        if culture_range > new_level {
                            if culture_range <= old_level {
                                debug_assert!(culture_range <= self.d.culture_levels.len() as i32);

                                if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                                    let level = CultureLevelType::from_usize(culture_range as usize);
                                    self.plot_change_culture_range_cities(loop_plot, owner, level, -1, update_plot_groups);
                                }
                            }
                        }
                    }
                }
            }

            if new_value.is_some() {
                for dx in -new_level..=new_level {
                    for dy in -new_level..=new_level {
                        let culture_range = self.city_culture_distance(c, dx, dy);

                        if culture_range > old_level {
                            if culture_range <= new_level {
                                debug_assert!(culture_range <= self.d.culture_levels.len() as i32);

                                if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                                    let level = CultureLevelType::from_usize(culture_range as usize);
                                    self.plot_change_culture_range_cities(loop_plot, owner, level, 1, update_plot_groups);
                                }
                            }
                        }
                    }
                }
            }

            if self.final_initialized {
                if (new_level > old_level) && (new_level > 1) {
                    // TXT_KEY_MISC_BORDERS_EXPANDED (and, at the last culture level,
                    // TXT_KEY_MISC_CULTURE_LEVEL to every alive player): messages only.
                    let turn = self.game_turn();
                    self.log.msg(turn, Some(owner), || format!("The borders of {} have expanded", self.cities[c].name));

                    // ONEVENT - Culture growth
                    self.ev_culture_expansion(c, owner);

                    // Stop Build Culture
                    // UNOFFICIAL_PATCH 07/17/09 (jdog5000): only for a human owner without
                    // automated production (for the AI the timing gave extra overflow hammers).
                    if self.city_is_human(c) && !self.cities[c].production_automated {
                        if self.city_is_production_process(c) {
                            if let Some(process) = self.city_production_process(c) {
                                if self.d.processes[process].production_to_commerce_modifier[CommerceType::COMMERCE_CULTURE.idx()] > 0 {
                                    self.city_pop_order(c, 0, false, true);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvCity::updateCultureLevel` (CvCity.cpp:7974-7996).
    pub fn city_update_culture_level(&mut self, c: CityId, update_plot_groups: bool) {
        if self.cities[c].culture_update_timer > 0 {
            return;
        }

        let mut culture_level = CultureLevelType::from_usize(0);

        if !self.city_is_occupation(c) {
            let owner = self.cities[c].owner;
            for i in (1..self.d.culture_levels.len()).rev() {
                let level = CultureLevelType::from_usize(i);
                if self.cities[c].culture[owner] >= 100 * self.culture_threshold(level) {
                    culture_level = level;
                    break;
                }
            }
        }

        self.city_set_culture_level(c, Some(culture_level), update_plot_groups);
    }

    // ---- yields ----

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
        let y = yield_type.idx();
        let city = &self.cities[c];
        let owner = city.owner;

        let mut modifier = city.yield_rate_modifier[y];

        modifier += city.bonus_yield_rate_modifier[y];

        if self.city_is_power(c) {
            modifier += city.power_yield_rate_modifier[y];
        }

        if let Some(area) = self.plot_area(self.city_plot(c)) {
            modifier += self.map.areas[area].yield_rate_modifier[owner][y];
        }

        modifier += self.players[owner].yield_rate_modifier[y];

        if self.city_is_capital(c) {
            modifier += self.players[owner].capital_yield_rate_modifier[y];
        }

        modifier += extra;

        // note: player->invalidateYieldRankCache() must be called for anything that is checked here
        // so if any extra checked things are added here, the cache needs to be invalidated

        // FfH: Added by Kael 11/08/2007
        if city.settlement {
            modifier -= 75;
        }
        // FfH: End Add

        max(0, modifier + 100)
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
        if change != 0 {
            self.cities[c].yield_rate_modifier[yield_type.idx()] += change;

            let owner = self.cities[c].owner;
            self.player_invalidate_yield_rank_cache(owner, Some(yield_type));

            if yield_type == YieldType::YIELD_COMMERCE {
                self.city_update_commerces(c);
            }

            self.cities[c].ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changePowerYieldRateModifier` (CvCity.cpp:8178-8202).
    pub fn city_change_power_yield_rate_modifier(&mut self, c: CityId, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.cities[c].power_yield_rate_modifier[yield_type.idx()] += change;

            let owner = self.cities[c].owner;
            self.player_invalidate_yield_rank_cache(owner, Some(yield_type));

            if yield_type == YieldType::YIELD_COMMERCE {
                self.city_update_commerces(c);
            }

            self.cities[c].ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeBonusYieldRateModifier` (CvCity.cpp:8213-8237).
    pub fn city_change_bonus_yield_rate_modifier(&mut self, c: CityId, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.cities[c].bonus_yield_rate_modifier[yield_type.idx()] += change;

            let owner = self.cities[c].owner;
            self.player_invalidate_yield_rank_cache(owner, Some(yield_type));

            if yield_type == YieldType::YIELD_COMMERCE {
                self.city_update_commerces(c);
            }

            self.cities[c].ai_assign_work_dirty = true;
        }
    }

    // ---- trade ----

    /// C++ `CvCity::totalTradeModifier` (CvCity.cpp:8248-8277). C++ default
    /// `pOtherCity = NULL`.
    pub fn city_total_trade_modifier(&self, c: CityId, other: Option<CityId>) -> i32 {
        let d = self.d;
        let mut modifier = 100;

        modifier += self.cities[c].trade_route_modifier;

        modifier += self.city_population_trade_modifier(c);

        if self.city_is_connected_to_capital(c, None) {
            modifier += d.defines.capital_trade_modifier;
        }

        if let Some(other) = other {
            // C++ compares the `CvArea` pointers.
            if self.plot_area(self.city_plot(c)) != self.plot_area(self.city_plot(other)) {
                modifier += d.defines.overseas_trade_modifier;
            }

            if self.city_team(c) != self.city_team(other) {
                modifier += self.cities[c].foreign_trade_route_modifier;

                modifier += self.city_peace_trade_modifier(c, self.city_team(other));
            }
        }

        modifier
    }

    /// C++ `CvCity::getPopulationTradeModifier` (CvCity.cpp:8279-8282).
    pub fn city_population_trade_modifier(&self, c: CityId) -> i32 {
        let defines = &self.d.defines;
        max(0, (self.cities[c].population + defines.our_population_trade_modifier_offset) * defines.our_population_trade_modifier)
    }

    /// C++ `CvCity::getPeaceTradeModifier` (CvCity.cpp:8284-8302).
    pub fn city_peace_trade_modifier(&self, c: CityId, team: TeamId) -> i32 {
        let defines = &self.d.defines;
        let our_team = self.city_team(c);
        debug_assert!(team != our_team);

        if self.at_war(team, our_team) {
            return 0;
        }

        let peace_turns = min(defines.foreign_trade_full_credit_peace_turns, self.teams[our_team].ai_at_peace_counter[team]);

        if self.elapsed_game_turns <= peace_turns {
            return defines.foreign_trade_modifier;
        }

        (defines.foreign_trade_modifier * peace_turns) / max(1, defines.foreign_trade_full_credit_peace_turns)
    }

    /// C++ `CvCity::getBaseTradeProfit` (CvCity.cpp:8304-8314).
    pub fn city_base_trade_profit(&self, c: CityId, other: CityId) -> i32 {
        let d = self.d;
        let city = &self.cities[c];
        let other_city = &self.cities[other];

        let mut profit = min(
            other_city.population * d.defines.their_population_trade_percent,
            self.plot_distance(city.x, city.y, other_city.x, other_city.y) * d.worlds[self.setup.world_size].trade_profit_percent,
        );

        profit *= d.defines.trade_profit_percent;
        profit /= 100;

        profit = max(100, profit);

        profit
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
        let trade_yield_modifier = self.players[self.cities[c].owner].trade_yield_modifier[yield_type.idx()];
        if (trade_profit > 0) && (trade_yield_modifier > 0) {
            (trade_profit * trade_yield_modifier) / 100
        } else {
            0
        }
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
        let city = &self.cities[c];
        (city.specialist_count[specialist] + city.free_specialist_count[specialist]) * self.players[city.owner].specialist_extra_yield[specialist][yield_type.idx()]
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

    // ---- commerce ----

    /// C++ `CvCity::getCommerceRate` (CvCity.cpp:8416-8419).
    pub fn city_commerce_rate(&self, c: CityId, commerce: CommerceType) -> i32 {
        self.city_commerce_rate_times100(c, commerce) / 100
    }

    /// C++ `CvCity::getCommerceRateTimes100` (CvCity.cpp:8421-8440): the `commerce_rate`
    /// field, with espionage folded into culture under the no-espionage option.
    pub fn city_commerce_rate_times100(&self, c: CityId, commerce: CommerceType) -> i32 {
        let city = &self.cities[c];
        let mut rate = city.commerce_rate[commerce.idx()];
        if self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
            if commerce == CommerceType::COMMERCE_CULTURE {
                rate += city.commerce_rate[CommerceType::COMMERCE_ESPIONAGE.idx()];
            } else if commerce == CommerceType::COMMERCE_ESPIONAGE {
                rate = 0;
            }
        }

        rate
    }

    /// C++ `CvCity::getCommerceFromPercent` (CvCity.cpp:8443-8455).
    pub fn city_commerce_from_percent(&self, c: CityId, commerce: CommerceType, yield_rate: i32) -> i32 {
        let mut total = (yield_rate * self.players[self.cities[c].owner].commerce_percent[commerce.idx()]) / 100;

        if commerce == CommerceType::COMMERCE_GOLD {
            total += yield_rate
                - total
                - self.city_commerce_from_percent(c, CommerceType::COMMERCE_RESEARCH, yield_rate)
                - self.city_commerce_from_percent(c, CommerceType::COMMERCE_CULTURE, yield_rate)
                - self.city_commerce_from_percent(c, CommerceType::COMMERCE_ESPIONAGE, yield_rate);
        }

        total
    }

    /// C++ `CvCity::getBaseCommerceRate` (CvCity.cpp:8458-8461).
    pub fn city_base_commerce_rate(&self, c: CityId, commerce: CommerceType) -> i32 {
        self.city_base_commerce_rate_times100(c, commerce) / 100
    }

    /// C++ `CvCity::getBaseCommerceRateTimes100` (CvCity.cpp:8463-8480). FFH: per-specialist
    /// type extra commerce.
    pub fn city_base_commerce_rate_times100(&self, c: CityId, commerce: CommerceType) -> i32 {
        let i = commerce.idx();
        let city = &self.cities[c];
        let player = &self.players[city.owner];

        let mut base_commerce_rate = self.city_commerce_from_percent(c, commerce, self.city_yield_rate(c, YieldType::YIELD_COMMERCE) * 100);

        // FfH: Modified by Kael 12/22/2007
        for specialist in self.d.specialists.keys() {
            base_commerce_rate += 100
                * ((city.specialist_count[specialist] + city.free_specialist_count[specialist])
                    * (player.specialist_extra_commerce[i] + player.specialist_type_extra_commerce[specialist][i]));
        }
        // FfH: End Modify

        base_commerce_rate += 100 * (city.building_commerce[i] + city.specialist_commerce[i] + city.religion_commerce[i] + city.corporation_commerce[i] + player.free_city_commerce[i]);

        base_commerce_rate
    }

    /// C++ `CvCity::getTotalCommerceRateModifier` (CvCity.cpp:8483-8486).
    pub fn city_total_commerce_rate_modifier(&self, c: CityId, commerce: CommerceType) -> i32 {
        let i = commerce.idx();
        let city = &self.cities[c];
        let player = &self.players[city.owner];
        let capital_modifier = if self.city_is_capital(c) { player.capital_commerce_rate_modifier[i] } else { 0 };
        max(0, city.commerce_rate_modifier[i] + player.commerce_rate_modifier[i] + capital_modifier + 100)
    }

    /// C++ `CvCity::updateCommerce(CommerceTypes)` (CvCity.cpp:8489-8524).
    pub fn city_update_commerce(&mut self, c: CityId, commerce: CommerceType) {
        let i = commerce.idx();
        let old_commerce = self.cities[c].commerce_rate[i];

        let new_commerce = if self.city_is_disorder(c) {
            0
        } else {
            let mut new_commerce = (self.city_base_commerce_rate_times100(c, commerce) * self.city_total_commerce_rate_modifier(c, commerce)) / 100;
            new_commerce += self.city_yield_rate(c, YieldType::YIELD_PRODUCTION) * self.cities[c].production_to_commerce_modifier[i];
            new_commerce
        };

        if old_commerce != new_commerce {
            self.cities[c].commerce_rate[i] = new_commerce;
            // (no >= 0 assert: some buildings can give negative commerce, Sephi)

            let owner = self.cities[c].owner;
            self.player_invalidate_commerce_rank_cache(owner, Some(commerce));

            // CvPlayer::changeCommerceRate (times 100; otherwise only a UI dirty bit)
            self.players[owner].commerce_rate[i] += new_commerce - old_commerce;

            // isCitySelected(): InfoPane / CityScreen dirty bits (UI).
        }
    }

    /// C++ `CvCity::updateCommerce()` (CvCity.cpp:8527-8535).
    pub fn city_update_commerces(&mut self, c: CityId) {
        let owner = self.cities[c].owner;
        self.player_invalidate_yield_rank_cache(owner, None);

        for i in 0..CommerceType::NUM {
            self.city_update_commerce(c, CommerceType::from_usize(i));
        }
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
        let d = self.d;
        let i = commerce.idx();
        let city = &self.cities[c];
        let owner = city.owner;

        let mut total = 0;

        let num_building = self.city_num_building(c, building);
        if num_building > 0 {
            let kb = &d.buildings[building];
            if !kb.commerce_change_original_owner[i] || (city.building_original_owner[building] == Some(owner)) {
                total += kb.obsolete_safe_commerce_change[i] * num_building;

                let num_active_building = self.city_num_active_building(c, building);
                if num_active_building > 0 {
                    total += (kb.commerce_change[i] + self.city_building_commerce_change(c, building_class(d, building), commerce)) * num_active_building;

                    if let Some(religion) = kb.religion_type {
                        if Some(religion) == self.player_get_state_religion(owner) {
                            total += self.players[owner].state_religion_building_commerce[i] * num_active_building;
                        }
                    }

                    if let Some(religion) = kb.global_religion_commerce {
                        total += (d.religions[religion].global_religion_commerce[i] * self.count_religion_levels(religion)) * num_active_building;
                    }

                    if let Some(corporation) = kb.global_corporation_commerce {
                        total += (d.corporations[corporation].headquarter_commerce[i] * self.count_corporation_levels(corporation)) * num_active_building;
                    }
                }

                // ADDON (Fix CommerceChangeDoubleTime) Sephi: was
                // `getGameTurnYear() - getBuildingOriginalTime(eBuilding) >= getCommerceChangeDoubleTime(eIndex)`.
                let needed_turn = kb.commerce_change_double_time[i] * d.game_speeds[self.game_speed_type()].construct_percent / 100;

                if (kb.commerce_change_double_time[i] != 0)
                    && (city.building_original_time[building] != MIN_INT)
                    && ((self.game_turn() - city.building_original_time[building]) >= needed_turn)
                {
                    return total * 2;
                }

                return total;
            }
        }

        0
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
        let i = commerce.idx();
        let state_religion = self.player_get_state_religion(self.cities[c].owner);

        let mut total = 0;

        if (state_religion == Some(religion)) || state_religion.is_none() {
            if self.cities[c].has_religion[religion] {
                total += self.d.religions[religion].state_religion_commerce[i];

                if self.city_is_holy_city(c, religion) {
                    total += self.d.religions[religion].holy_city_commerce[i];
                }
            }
        }

        total
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

    // ---- culture ----

    /// C++ `CvCity::getCulture` (CvCity.cpp:9084-9089). (`getCultureTimes100` is the field
    /// `culture[p]`.)
    pub fn city_culture(&self, c: CityId, player: PlayerId) -> i32 {
        self.cities[c].culture[player] / 100
    }

    /// C++ `CvCity::countTotalCultureTimes100` (CvCity.cpp:9099-9115): over alive players.
    pub fn city_count_total_culture_times100(&self, c: CityId) -> i32 {
        let mut total_culture = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                total_culture += self.cities[c].culture[p];
            }
        }

        total_culture
    }

    /// C++ `CvCity::findHighestCulture` (CvCity.cpp:9118-9143). C++ `NO_PLAYER` = `None`.
    pub fn city_find_highest_culture(&self, c: CityId) -> Option<PlayerId> {
        let mut best_value = 0;
        let mut best_player = None;

        for p in self.player_ids() {
            if self.players[p].alive {
                let value = self.cities[c].culture[p];

                if value > best_value {
                    best_value = value;
                    best_player = Some(p);
                }
            }
        }

        best_player
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
        let mut team_culture_percent = 0;

        for p in self.player_ids() {
            if self.players[p].alive {
                if self.players[p].team == team {
                    team_culture_percent += self.city_calculate_culture_percent(c, p);
                }
            }
        }

        team_culture_percent
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
        let mut best_modifier = 0;

        for &loop_unit in &self.map.plots[self.city_plot(c)].units {
            let revolt_protection = self.units[loop_unit].revolt_protection;
            if revolt_protection > best_modifier {
                best_modifier = revolt_protection;
            }
        }
        best_modifier = range(best_modifier, 0, 100);

        (self.d.defines.revolt_test_prob * (100 - best_modifier)) / 100
    }

    // ---- revealed / espionage visibility ----

    /// C++ `CvCity::isRevealed` (CvCity.cpp:9290-9303): the `revealed` field, or true for
    /// `debug` queries in debug mode (`CvGame::isDebugMode`). Rules code passes
    /// `debug = false`; debug mode is UI and never on in this port.
    pub fn city_is_revealed(&self, c: CityId, team: TeamId, _debug: bool) -> bool {
        self.cities[c].revealed[team]
    }

    /// C++ `CvCity::setEspionageVisibility` (CvCity.cpp:9345-9358): updates the city plot's
    /// sight around the change.
    pub fn city_set_espionage_visibility(&mut self, c: CityId, team: TeamId, new_value: bool, update_plot_groups: bool) {
        if self.cities[c].espionage_visibility[team] != new_value {
            let plot = self.city_plot(c);
            self.plot_update_sight(plot, false, update_plot_groups);

            self.cities[c].espionage_visibility[team] = new_value;

            self.plot_update_sight(plot, true, update_plot_groups);
        }
    }

    /// C++ `CvCity::updateEspionageVisibility` (CvCity.cpp:9360-9404).
    pub fn city_update_espionage_visibility(&mut self, c: CityId, update_plot_groups: bool) {
        let d = self.d;
        let missions: Vec<EspionageMissionType> = d
            .espionage_missions
            .keys()
            .filter(|&m| d.espionage_missions[m].passive && d.espionage_missions[m].visibility_level > 0)
            .collect();

        for team in self.civ_team_ids() {
            let mut visibility = false;

            if team != self.city_team(c) {
                if self.city_is_revealed(c, team, false) {
                    let owner = self.cities[c].owner;
                    let plot = self.city_plot(c);
                    for p in self.civ_player_ids() {
                        let player = &self.players[p];
                        if player.alive && player.team == team {
                            for &mission in &missions {
                                if self.player_can_do_espionage_mission(p, mission, Some(owner), Some(plot), -1, None) {
                                    visibility = true;
                                    break;
                                }
                            }

                            if visibility {
                                break;
                            }
                        }
                    }
                }
            }

            self.city_set_espionage_visibility(c, team, visibility, update_plot_groups);
        }
    }

    // ---- religion ----

    /// C++ `CvCity::setHasReligion` (CvCity.cpp:10688-10810). FFH: disabled religions,
    /// founding a religion makes this its holy city, global counter. Python events
    /// `religionSpread` / `religionRemove`. C++ default `bArrows = true`.
    pub fn city_set_has_religion(&mut self, c: CityId, religion: ReligionType, new_value: bool, _announce: bool, _arrows: bool) {
        let d = self.d;

        // FfH: Added by Kael 10/29/2007
        if self.is_religion_disabled(religion) {
            return;
        }
        // FfH: End Add

        if self.cities[c].has_religion[religion] != new_value {
            for vote_source in d.vote_sources.keys() {
                self.city_process_vote_source_bonus(c, vote_source, false);
            }

            // FfH: Modified by Kael 11/03/2007 (a religion can change the city's sight)
            let plot = self.city_plot(c);
            self.plot_update_sight(plot, false, true);
            self.cities[c].has_religion[religion] = new_value;
            self.plot_update_sight(plot, true, true);
            // FfH: End Modify

            for vote_source in d.vote_sources.keys() {
                self.city_process_vote_source_bonus(c, vote_source, true);
            }

            let owner = self.cities[c].owner;
            let change = if self.cities[c].has_religion[religion] { 1 } else { -1 };
            self.player_change_has_religion_count(owner, religion, change);

            self.city_update_maintenance(c);
            self.city_update_religion_happiness(c);
            self.city_update_religion_commerces(c);

            self.cities[c].ai_assign_work_dirty = true;

            if self.cities[c].has_religion[religion] {
                // FfH: Added by Kael 12/18/2008
                if !self.is_religion_founded(religion) {
                    self.set_holy_city(religion, Some(c), true);
                }
                // FfH: End Add

                let owner = self.cities[c].owner;
                self.make_religion_founded(religion, owner);

                // bAnnounce: UI only. TXT_KEY_MISC_RELIGION_SPREAD messages (unless this is the
                // holy city) to alive players that have the city revealed and own it, have the
                // religion as state religion or its holy city (FfH: and `canSeeReligion`); and,
                // for a human owner whose first city with the religion this is, the
                // BUTTONPOPUP_CHANGERELIGION popup if it `canConvert` without a state religion
                // (the controller converts through `player_convert` on its own turn).
            }

            let owner = self.cities[c].owner;
            if new_value {
                // FfH: Added by Kael 08/13/2007
                self.change_global_counter(d.religions[religion].global_counter_spread);
                // FfH: End Add

                // Python Event
                self.ev_religion_spread(religion, owner, c);
            } else {
                // FfH: Added by Kael 08/13/2007
                self.change_global_counter(-1 * d.religions[religion].global_counter_spread);
                // FfH: End Add

                // Python Event
                self.ev_religion_remove(religion, owner, c);
            }
        }
    }

    /// C++ `CvCity::processVoteSourceBonus` (CvCity.cpp:10813-10876).
    pub fn city_process_vote_source_bonus(&mut self, c: CityId, vote_source: VoteSourceType, active: bool) {
        let d = self.d;
        let owner = self.cities[c].owner;

        if !self.players[owner].loyal_member[vote_source] {
            return;
        }

        if self.is_diplo_vote(vote_source) {
            let religion = self.vote_source_religions[vote_source];
            let vote_source_info = &d.vote_sources[vote_source];

            if let Some(specialist) = vote_source_info.free_specialist {
                if religion.is_none_or(|r| self.cities[c].has_religion[r]) {
                    self.city_change_free_specialist_count(c, specialist, if active { 1 } else { -1 });
                }
            }

            if let Some(religion) = religion.filter(|&r| self.cities[c].has_religion[r]) {
                for i in 0..YieldType::NUM {
                    let mut change = vote_source_info.religion_yield[i];
                    if !active {
                        change = -change;
                    }

                    if 0 != change {
                        for building in d.buildings.keys() {
                            if d.buildings[building].religion_type == Some(religion) {
                                self.city_change_building_yield_change(c, building_class(d, building), YieldType::from_usize(i), change);
                            }
                        }
                    }
                }

                for i in 0..CommerceType::NUM {
                    let mut change = vote_source_info.religion_commerce[i];
                    if !active {
                        change = -change;
                    }

                    if 0 != change {
                        for building in d.buildings.keys() {
                            if d.buildings[building].religion_type == Some(religion) {
                                self.city_change_building_commerce_change(c, building_class(d, building), CommerceType::from_usize(i), change);
                            }
                        }
                    }
                }
            }
        }
    }

    // ---- trade routes ----

    /// C++ `CvCity::getTradeCity` (CvCity.cpp:10996-11001): `None` if the slot is empty or
    /// the city no longer exists (C++ `getCity(IDInfo)` returns NULL).
    pub fn city_trade_city(&self, c: CityId, index: usize) -> Option<CityId> {
        self.cities[c].trade_cities[index].filter(|&t| self.cities.contains(t))
    }

    /// C++ `CvCity::getTradeRoutes` (CvCity.cpp:11004-11017).
    pub fn city_trade_routes(&self, c: CityId) -> i32 {
        let owner = self.cities[c].owner;

        let mut trade_routes = self.trade_routes;
        trade_routes += self.players[owner].trade_routes;
        if self.city_is_coastal(c, self.d.defines.min_water_size_for_ocean) {
            trade_routes += self.players[owner].coastal_trade_routes;
        }
        trade_routes += self.cities[c].extra_trade_routes;

        min(trade_routes, self.d.defines.max_trade_routes)
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
        let max_trade_routes = self.d.defines.max_trade_routes.max(0) as usize;
        let mut best_value = vec![0; max_trade_routes];

        self.city_clear_trade_routes(c);

        if !self.city_is_disorder(c) && !self.cities[c].plundered {
            let trade_routes = self.city_trade_routes(c).max(0) as usize;
            debug_assert!(trade_routes <= max_trade_routes);

            let owner = self.cities[c].owner;
            for p in self.player_ids() {
                if self.player_can_have_trade_routes_with(owner, p) {
                    // (the loop changes no city list: iterate by index)
                    for n in 0..self.players[p].cities.len() {
                        let loop_city = self.players[p].cities[n];
                        if loop_city != c {
                            if !self.cities[loop_city].trade_route[owner] || (self.city_team(c) == self.players[p].team) {
                                if self.city_plot_group(loop_city, owner) == self.city_plot_group(c, owner)
                                    || self.d.defines.ignore_plot_group_for_trade_routes != 0
                                {
                                    let value = self.city_calculate_trade_profit(c, loop_city);

                                    for j in 0..trade_routes {
                                        if value > best_value[j] {
                                            for k in ((j + 1)..trade_routes).rev() {
                                                best_value[k] = best_value[k - 1];
                                                self.cities[c].trade_cities[k] = self.cities[c].trade_cities[k - 1];
                                            }

                                            best_value[j] = value;
                                            self.cities[c].trade_cities[j] = Some(loop_city);

                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let owner = self.cities[c].owner;
        let mut trade_profit = 0;

        for i in 0..max_trade_routes {
            if let Some(loop_city) = self.city_trade_city(c, i) {
                self.cities[loop_city].trade_route[owner] = true;

                trade_profit += self.city_calculate_trade_profit(c, loop_city);
            }
        }

        for i in 0..YieldType::NUM {
            let yield_type = YieldType::from_usize(i);
            let trade_yield = self.city_calculate_trade_yield(c, yield_type, trade_profit);
            self.city_set_trade_yield(c, yield_type, trade_yield); // XXX could take this out if handled when CvPlotGroup changes...
        }
    }

    // ---- turn processing: culture and religion ----

    /// C++ `CvCity::doCulture` (CvCity.cpp:11803-11817). Python `doCulture` callback.
    pub fn city_do_culture(&mut self, c: CityId) {
        if py_do_culture(self, c) {
            return;
        }

        let owner = self.cities[c].owner;
        let culture_rate = self.city_commerce_rate_times100(c, CommerceType::COMMERCE_CULTURE);
        self.city_change_culture_times100(c, owner, culture_rate, false, true);
    }

    /// C++ `CvCity::doPlotCulture` (CvCity.cpp:11820-11886): spreads `player`'s culture to
    /// the plots in range. Python `doPlotCulture` callback.
    pub fn city_do_plot_culture(&mut self, c: CityId, update: bool, player: PlayerId, culture_rate: i32) {
        // C++ CultureLevelTypes as an int: starts at (CultureLevelTypes)0; NO_CULTURELEVEL = -1.
        let mut culture_level: i32 = 0;

        if py_do_plot_culture(self, c, update, player, culture_rate) {
            return;
        }

        if self.cities[c].owner == player {
            culture_level = opt_to_i32(self.cities[c].culture_level);
        } else {
            for i in (1..self.d.culture_levels.len()).rev() {
                if self.cities[c].culture[player] >= 100 * self.culture_threshold(CultureLevelType::from_usize(i)) {
                    culture_level = i as i32;
                    break;
                }
            }
        }

        let free_culture_rate = self.d.defines.city_free_culture_growth_factor;
        if self.cities[c].culture[player] > 0 {
            if culture_level != -1 {
                let (x, y) = (self.cities[c].x, self.cities[c].y);
                for dx in -culture_level..=culture_level {
                    for dy in -culture_level..=culture_level {
                        let culture_range = self.city_culture_distance(c, dx, dy);

                        if culture_range <= culture_level {
                            if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                                if self.plot_is_potential_city_work_for_area(loop_plot, self.city_area(c)) {
                                    let change = ((culture_level - culture_range) * free_culture_rate) + culture_rate + 1;
                                    let update_plot = update || !self.plot_is_owned(loop_plot);
                                    self.plot_change_culture(loop_plot, player, change, update_plot);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvCity::doReligion` (CvCity.cpp:12145-12215): spread of religions into a city
    /// without any; draws "Religion Spread". Python `doReligion` callback.
    pub fn city_do_religion(&mut self, c: CityId) {
        let d = self.d;

        if py_do_religion(self, c) {
            return;
        }

        if self.city_religion_count(c) == 0 {
            for religion in d.religions.keys() {
                if !self.cities[c].has_religion[religion] {
                    let owner = self.cities[c].owner;
                    if (Some(religion) == self.player_get_state_religion(owner)) || !self.player_is_no_non_state_religion_spread(owner) {
                        let mut rand_threshold = 0;

                        for p in self.player_ids() {
                            if self.players[p].alive {
                                for &loop_city in &self.players[p].cities {
                                    if self.city_is_connected_to(loop_city, c) {
                                        let mut spread = self.cities[loop_city].religion_influence[religion];

                                        spread *= d.religions[religion].spread_factor;

                                        if spread > 0 {
                                            let (city, other) = (&self.cities[c], &self.cities[loop_city]);
                                            let distance = self.plot_distance(city.x, city.y, other.x, other.y);
                                            spread /= max(1, ((d.defines.religion_spread_distance_divisor * distance) / self.map.max_plot_distance()) - 5);

                                            //iSpread /= (getReligionCount() + 1);

                                            // FfH: Added by Kael 07/30/2008
                                            if self.players[owner].agnostic {
                                                spread /= 2;
                                            }
                                            // FfH: End Add

                                            rand_threshold = max(rand_threshold, spread);
                                        }
                                    }
                                }
                            }
                        }

                        if self.soren_rand_num(d.defines.religion_spread_rand, "Religion Spread") < rand_threshold {
                            self.city_set_has_religion(c, religion, true, true, true);
                            break;
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvCity::hasShrine` (CvCity.cpp:13693-13711).
    pub fn city_has_shrine(&self, c: CityId, religion: ReligionType) -> bool {
        let mut has_shrine = false;

        // note, for normal XML, this count will be one, there is only one shrine of each religion
        let shrine_building_count = self.get_shrine_building_count(Some(religion));
        for i in 0..shrine_building_count {
            // (always Some for i < count)
            if let Some(building) = self.get_shrine_building(i, Some(religion)) {
                if self.city_num_building(c, building) > 0 {
                    has_shrine = true;
                    break;
                }
            }
        }

        has_shrine
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
        let city = &self.cities[c];
        (city.specialist_count[specialist] + city.free_specialist_count[specialist]) * self.players[city.owner].specialist_type_extra_commerce[specialist][commerce.idx()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::City;
    use crate::map::tests::{assign_test_areas, at, game_from_ascii};
    use crate::player::Player;
    use crate::unit::{Unit, UnitResetInfo};

    /// A duel game on a 10x5 map whose southern row (y = 0) is a 10-tile ocean, with every
    /// player at its `reset` state (own team) and players 0 and 1 alive.
    fn test_game() -> Game {
        let rows = ["##########", "##########", "##########", "##########", ".........."];
        let mut g = game_from_ascii(&rows, false, false);
        assign_test_areas(&mut g);
        let d = g.d;
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        for p in [PlayerId(0), PlayerId(1)] {
            g.players[p].alive = true;
        }
        g
    }

    /// A city of `owner` at (x, y) with `population`, registered with its owner and plot
    /// (without `city_init`).
    fn add_city(g: &mut Game, owner: PlayerId, x: i32, y: i32, population: i32) -> CityId {
        let d = g.d;
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        let c = g.cities.insert_with(|id| City::new(d, num_players, num_teams, id, owner, x, y, 0));
        g.players[owner].cities.push(c);
        g.cities[c].population = population;
        let p = at(g, x, y);
        g.map.plots[p].plot_city = Some(c);
        g.map.plots[p].owner = Some(owner);
        c
    }

    fn set_war(g: &mut Game, a: TeamId, b: TeamId, war: bool) {
        g.teams[a].at_war[b] = war;
        g.teams[b].at_war[a] = war;
    }

    #[test]
    fn population_rank_is_cached_until_invalidated() {
        let mut g = test_game();
        let p0 = PlayerId(0);
        let a = add_city(&mut g, p0, 1, 1, 3);
        let b = add_city(&mut g, p0, 4, 1, 5);
        let c = add_city(&mut g, p0, 1, 3, 3);
        add_city(&mut g, PlayerId(1), 7, 3, 9); // other owners do not count
        assert_eq!(g.city_find_population_rank(b), 1);
        assert_eq!(g.city_find_population_rank(a), 2, "ties: the older city first");
        assert_eq!(g.city_find_population_rank(c), 3);
        // the cache is real state: a stale rank survives until invalidated
        g.cities[c].population = 6;
        assert_eq!(g.city_find_population_rank(c), 3);
        g.city_invalidate_population_rank_cache(c);
        assert_eq!(g.city_find_population_rank(c), 1);
    }

    #[test]
    fn base_yield_and_commerce_rate_ranks() {
        let mut g = test_game();
        let p0 = PlayerId(0);
        let a = add_city(&mut g, p0, 1, 1, 1);
        let b = add_city(&mut g, p0, 4, 1, 1);
        let food = YieldType::YIELD_FOOD;
        g.cities[a].base_yield_rate[food.idx()] = 4;
        g.cities[b].base_yield_rate[food.idx()] = 4;
        assert_eq!(g.city_find_base_yield_rate_rank(a, food), 1);
        assert_eq!(g.city_find_base_yield_rate_rank(b, food), 2);
        g.cities[b].base_yield_rate[food.idx()] = 7;
        g.city_invalidate_yield_rank_cache(b, Some(food));
        assert_eq!(g.city_find_base_yield_rate_rank(b, food), 1);
        assert_eq!(g.city_find_base_yield_rate_rank(a, food), 1, "stale until invalidated");
        g.city_invalidate_yield_rank_cache(a, None);
        assert_eq!(g.city_find_base_yield_rate_rank(a, food), 2);

        let culture = CommerceType::COMMERCE_CULTURE;
        g.cities[a].commerce_rate = [0, 0, 100, 60];
        g.cities[b].commerce_rate = [0, 0, 150, 0];
        // espionage counts as culture: 160 > 150
        assert_eq!(g.city_find_commerce_rate_rank(a, culture), 1);
        assert_eq!(g.city_find_commerce_rate_rank(b, culture), 2);
    }

    #[test]
    fn commerce_rate_folds_espionage_into_culture() {
        let mut g = test_game();
        let a = add_city(&mut g, PlayerId(0), 1, 1, 1);
        g.cities[a].commerce_rate = [250, 300, 120, 80];
        // FfH locks GAMEOPTION_NO_ESPIONAGE on
        assert_eq!(g.city_commerce_rate_times100(a, CommerceType::COMMERCE_GOLD), 250);
        assert_eq!(g.city_commerce_rate_times100(a, CommerceType::COMMERCE_RESEARCH), 300);
        assert_eq!(g.city_commerce_rate_times100(a, CommerceType::COMMERCE_CULTURE), 200);
        assert_eq!(g.city_commerce_rate_times100(a, CommerceType::COMMERCE_ESPIONAGE), 0);
        assert_eq!(g.city_commerce_rate(a, CommerceType::COMMERCE_GOLD), 2);
    }

    #[test]
    fn commerce_from_percent_gives_the_remainder_to_gold() {
        let mut g = test_game();
        let a = add_city(&mut g, PlayerId(0), 1, 1, 1);
        g.players[PlayerId(0)].commerce_percent = [30, 60, 10, 0];
        assert_eq!(g.city_commerce_from_percent(a, CommerceType::COMMERCE_RESEARCH, 700), 420);
        assert_eq!(g.city_commerce_from_percent(a, CommerceType::COMMERCE_CULTURE, 700), 70);
        assert_eq!(g.city_commerce_from_percent(a, CommerceType::COMMERCE_ESPIONAGE, 700), 0);
        assert_eq!(g.city_commerce_from_percent(a, CommerceType::COMMERCE_GOLD, 700), 210);
        // truncation: research 3 * 60% = 1, culture 0, gold 0 + the 2 left over
        assert_eq!(g.city_commerce_from_percent(a, CommerceType::COMMERCE_GOLD, 3), 2);
    }

    #[test]
    fn total_commerce_rate_modifier() {
        let mut g = test_game();
        let p0 = PlayerId(0);
        let a = add_city(&mut g, p0, 1, 1, 1);
        let gold = CommerceType::COMMERCE_GOLD;
        g.cities[a].commerce_rate_modifier[gold.idx()] = 10;
        g.players[p0].commerce_rate_modifier[gold.idx()] = 20;
        g.players[p0].capital_commerce_rate_modifier[gold.idx()] = 50;
        assert_eq!(g.city_total_commerce_rate_modifier(a, gold), 130);
        g.players[p0].capital_city = Some(a);
        assert!(g.city_is_capital(a));
        assert_eq!(g.city_total_commerce_rate_modifier(a, gold), 180);
        g.cities[a].commerce_rate_modifier[gold.idx()] = -500;
        assert_eq!(g.city_total_commerce_rate_modifier(a, gold), 0);
    }

    #[test]
    fn culture_shares() {
        let mut g = test_game();
        let (p0, p1, p2) = (PlayerId(0), PlayerId(1), PlayerId(2));
        assert!(!g.players[p2].alive);
        let a = add_city(&mut g, p0, 1, 1, 1);
        assert_eq!(g.city_find_highest_culture(a), None);
        assert_eq!(g.city_calculate_culture_percent(a, p0), 0);
        g.cities[a].culture[p0] = 300;
        g.cities[a].culture[p1] = 100;
        g.cities[a].culture[p2] = 400; // not alive: ignored
        assert_eq!(g.city_count_total_culture_times100(a), 400);
        assert_eq!(g.city_culture(a, p0), 3);
        assert_eq!(g.city_find_highest_culture(a), Some(p0));
        assert_eq!(g.city_calculate_culture_percent(a, p1), 25);
        assert_eq!(g.city_calculate_team_culture_percent(a, g.players[p0].team), 75);
        assert_eq!(g.city_calculate_team_culture_percent(a, g.players[p2].team), 0);
        // ties keep the first player
        g.cities[a].culture[p1] = 300;
        assert_eq!(g.city_find_highest_culture(a), Some(p0));
    }

    #[test]
    fn culture_distance_and_threshold() {
        let mut g = test_game();
        let a = add_city(&mut g, PlayerId(0), 4, 2, 1);
        assert_eq!(g.city_culture_distance(a, 0, 0), 1);
        assert_eq!(g.city_culture_distance(a, 1, 1), 1);
        assert_eq!(g.city_culture_distance(a, 2, 1), 2);
        assert_eq!(g.city_culture_distance(a, -2, -2), 3);
        assert_eq!(g.city_culture_distance(a, 3, 3), 4);
        assert_eq!(g.cities[a].culture_level, None);
        assert_eq!(g.city_culture_threshold(a), 1);
        assert_eq!(g.city_culture_threshold_for_level(None), 1);
    }

    #[test]
    fn location_and_owner_helpers() {
        let mut g = test_game();
        let d = g.d;
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let coastal = add_city(&mut g, p0, 2, 1, 1);
        let inland = add_city(&mut g, p1, 6, 3, 1);
        let plot = at(&g, 2, 1);
        assert_eq!(g.city_plot(coastal), plot);
        assert!(g.city_at_plot(coastal, plot));
        assert!(!g.city_at_plot(inland, plot));
        assert_eq!(g.city_area(coastal), g.map.plots[plot].area.unwrap());
        assert_eq!(g.city_area(coastal), g.city_area(inland));
        assert_eq!(g.city_water_area(coastal), g.map.plots[at(&g, 2, 0)].area);
        assert_eq!(g.city_water_area(inland), None);
        assert!(g.city_is_coastal(coastal, d.defines.min_water_size_for_ocean));
        assert!(!g.city_is_coastal(coastal, 11));
        assert!(!g.city_is_coastal(inland, -1));
        assert_eq!(g.city_team(inland), g.players[p1].team);
        assert!(!g.city_is_human(coastal));
        assert!(!g.city_is_barbarian(coastal));
        assert!(!g.city_is_capital(coastal));
        let religion = ReligionType(0);
        assert!(!g.city_is_any_holy_city(inland));
        g.holy_city[religion] = Some(inland);
        assert!(g.city_is_holy_city(inland, religion));
        assert!(!g.city_is_holy_city(coastal, religion));
        assert!(g.city_is_any_holy_city(inland));
        let trait_type = TraitType(0);
        g.players[p1].traits[trait_type] = true;
        assert!(g.city_has_trait(inland, trait_type));
        assert!(!g.city_has_trait(coastal, trait_type));
    }

    #[test]
    fn rally_plot() {
        let mut g = test_game();
        let a = add_city(&mut g, PlayerId(0), 1, 1, 1);
        assert_eq!(g.city_rally_plot(a), None);
        g.city_set_rally_plot(a, Some(at(&g, 3, 2)));
        assert_eq!((g.cities[a].rally_x, g.cities[a].rally_y), (3, 2));
        assert_eq!(g.city_rally_plot(a), Some(at(&g, 3, 2)));
        g.city_set_rally_plot(a, None);
        assert_eq!((g.cities[a].rally_x, g.cities[a].rally_y), (INVALID_PLOT_COORD, INVALID_PLOT_COORD));
        assert_eq!(g.city_rally_plot(a), None);
    }

    #[test]
    fn disorder() {
        let mut g = test_game();
        let p0 = PlayerId(0);
        let a = add_city(&mut g, p0, 1, 1, 1);
        assert!(!g.city_is_disorder(a));
        g.cities[a].occupation_timer = 2;
        assert!(g.city_is_disorder(a));
        g.cities[a].occupation_timer = 0;
        g.players[p0].anarchy_turns = 1;
        assert!(g.city_is_disorder(a));
        g.players[p0].anarchy_turns = 0;
        g.players[p0].disable_production = 1;
        assert!(g.city_is_disorder(a), "FfH: disabled production");
    }

    #[test]
    fn trade_routes_count() {
        let mut g = test_game();
        let p0 = PlayerId(0);
        let coastal = add_city(&mut g, p0, 2, 1, 1);
        let inland = add_city(&mut g, p0, 6, 3, 1);
        g.trade_routes = 1;
        g.players[p0].trade_routes = 1;
        g.players[p0].coastal_trade_routes = 2;
        g.cities[coastal].extra_trade_routes = 1;
        g.cities[inland].extra_trade_routes = 1;
        assert_eq!(g.city_trade_routes(coastal), 5);
        assert_eq!(g.city_trade_routes(inland), 3);
        g.cities[inland].extra_trade_routes = 20;
        assert_eq!(g.city_trade_routes(inland), g.d.defines.max_trade_routes);
    }

    #[test]
    fn trade_profit_and_modifiers() {
        let mut g = test_game();
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let (t0, t1) = (g.players[p0].team, g.players[p1].team);
        let a = add_city(&mut g, p0, 0, 1, 4);
        let b = add_city(&mut g, p1, 6, 4, 30);

        // OUR_POPULATION_TRADE_MODIFIER 5, _OFFSET -10
        assert_eq!(g.city_population_trade_modifier(a), 0);
        assert_eq!(g.city_population_trade_modifier(b), 100);

        // distance 6 + 3/2 = 7: min(30 * 50, 7 * 80 (duel)) * 20% = 112
        assert_eq!(g.city_base_trade_profit(a, b), 112);
        // min(4 * 50, 7 * 80) * 20% = 40, at least 100
        assert_eq!(g.city_base_trade_profit(b, a), 100);

        // FOREIGN_TRADE_MODIFIER 150 over FOREIGN_TRADE_FULL_CREDIT_PEACE_TURNS 50
        g.teams[t0].ai_at_peace_counter[t1] = 20;
        g.elapsed_game_turns = 10;
        assert_eq!(g.city_peace_trade_modifier(a, t1), 150);
        g.elapsed_game_turns = 100;
        assert_eq!(g.city_peace_trade_modifier(a, t1), 60);
        g.teams[t0].ai_at_peace_counter[t1] = 80;
        assert_eq!(g.city_peace_trade_modifier(a, t1), 150);
        set_war(&mut g, t0, t1, true);
        assert_eq!(g.city_peace_trade_modifier(a, t1), 0);
        set_war(&mut g, t0, t1, false);

        // no capital (not connected), same area: 100 + foreign 10 + peace 150
        g.cities[a].foreign_trade_route_modifier = 10;
        assert_eq!(g.city_total_trade_modifier(a, Some(b)), 260);
        assert_eq!(g.city_total_trade_modifier(a, None), 100);
        assert_eq!(g.city_calculate_trade_profit(a, b), 112 * 260 / 10000);

        let commerce = YieldType::YIELD_COMMERCE;
        assert_eq!(g.city_calculate_trade_yield(a, commerce, 250), 0);
        g.players[p0].trade_yield_modifier[commerce.idx()] = 100;
        assert_eq!(g.city_calculate_trade_yield(a, commerce, 250), 250);
        assert_eq!(g.city_calculate_trade_yield(a, commerce, 0), 0);
        g.players[p0].trade_yield_modifier[commerce.idx()] = 50;
        assert_eq!(g.city_calculate_trade_yield(a, commerce, 5), 2);
    }

    #[test]
    fn specialist_extras() {
        let mut g = test_game();
        let p0 = PlayerId(0);
        let a = add_city(&mut g, p0, 1, 1, 1);
        let (s0, s1) = (SpecialistType(0), SpecialistType(1));
        g.cities[a].specialist_count[s0] = 2;
        g.cities[a].free_specialist_count[s0] = 1;
        g.cities[a].specialist_count[s1] = 1;
        g.players[p0].specialist_extra_yield[s0][YieldType::YIELD_FOOD.idx()] = 2;
        assert_eq!(g.city_extra_specialist_yield(a, YieldType::YIELD_FOOD, s0), 6);
        assert_eq!(g.city_extra_specialist_yield(a, YieldType::YIELD_FOOD, s1), 0);
        let gold = CommerceType::COMMERCE_GOLD;
        g.players[p0].specialist_type_extra_commerce[s0][gold.idx()] = 3;
        g.players[p0].specialist_type_extra_commerce[s1][gold.idx()] = 5;
        assert_eq!(g.city_extra_specialist_commerce_by_specialist(a, gold, s0), 9);
        assert_eq!(g.city_extra_specialist_commerce(a, gold), 14);
    }

    #[test]
    fn religion_commerce() {
        let mut g = test_game();
        let d = g.d;
        let p0 = PlayerId(0);
        let a = add_city(&mut g, p0, 1, 1, 1);
        let (r0, r1) = (ReligionType(0), ReligionType(1));
        let gold = CommerceType::COMMERCE_GOLD;
        let state = d.religions[r0].state_religion_commerce[gold.idx()];
        let holy = d.religions[r0].holy_city_commerce[gold.idx()];
        assert_eq!(g.city_religion_commerce_by_religion(a, gold, r0), 0);
        g.cities[a].has_religion[r0] = true;
        assert_eq!(g.city_religion_count(a), 1);
        // no state religion: every religion counts
        assert_eq!(g.city_religion_commerce_by_religion(a, gold, r0), state);
        g.holy_city[r0] = Some(a);
        assert_eq!(g.city_religion_commerce_by_religion(a, gold, r0), state + holy);
        // another state religion: nothing
        g.players[p0].state_religion_count = 1;
        g.players[p0].last_state_religion = Some(r1);
        assert_eq!(g.city_religion_commerce_by_religion(a, gold, r0), 0);
        g.players[p0].last_state_religion = Some(r0);
        assert_eq!(g.city_religion_commerce_by_religion(a, gold, r0), state + holy);
    }

    #[test]
    fn revolt_probability_and_garrison() {
        let mut g = test_game();
        let d = g.d;
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let a = add_city(&mut g, p0, 1, 1, 1);
        let plot = g.city_plot(a);
        // REVOLT_TEST_PROB 10
        assert_eq!(g.city_revolt_test_probability(a), 10);
        assert_eq!(g.city_culture_garrison(a, p1), 1);

        let warrior = d.t::<UnitType>("UNIT_WARRIOR");
        let axeman = d.t::<UnitType>("UNIT_AXEMAN");
        let add = |g: &mut Game, unit_type: UnitType, revolt_protection: i32| {
            let u = g.units.insert_with(|id| Unit::new(id, unit_type, p0, d, &UnitResetInfo::default()));
            g.units[u].revolt_protection = revolt_protection;
            g.map.plots[plot].units.push(u);
        };
        add(&mut g, warrior, 30);
        add(&mut g, axeman, 20);
        assert_eq!(g.city_revolt_test_probability(a), 10 * 70 / 100);
        add(&mut g, warrior, 150);
        assert_eq!(g.city_revolt_test_probability(a), 0, "protection is capped at 100");

        let garrison = 1 + 2 * d.units[warrior].culture_garrison_value + d.units[axeman].culture_garrison_value;
        assert_eq!(g.city_culture_garrison(a, p1), garrison);
        let (t0, t1) = (g.players[p0].team, g.players[p1].team);
        set_war(&mut g, t0, t1, true);
        assert_eq!(g.city_culture_garrison(a, p1), 2 * garrison);
    }
}
