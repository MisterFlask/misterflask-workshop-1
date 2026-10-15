//! CvPlayer: economy -- gold (unit cost/supply, maintenance, civic upkeep is in
//! `civics_religion.rs`, inflation), commerce percents and rates, research (rate, queue,
//! per-turn progress), golden ages, yields, happiness/health modifiers, specialists, and
//! the "update every city" helpers.
//!
//! CvGameUtils hooks reached from here: `cannotResearch` (USE_CANNOT_RESEARCH_CALLBACK, on in
//! FFH2) is `py_cannot_research`. The others never change the C++ result in FFH2, so they are
//! not called: `isPlayerResearch` (returns True) and `canResearch` (returns False) are also
//! disabled by their USE_*_CALLBACK defines; `getExtraCost` returns 0; `doGold`, `doResearch`
//! and `skipResearchPopup` return False (= run the C++ default).

#![allow(unused_imports)]

use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

/// Out-parameters of C++ `CvPlayer::calculateUnitCost(int& iFreeUnits, ...)`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitCostBreakdown {
    pub free_units: i32,
    pub free_military_units: i32,
    pub paid_units: i32,
    pub paid_military_units: i32,
    pub base_unit_cost: i32,
    pub military_cost: i32,
    pub extra_cost: i32,
    /// The C++ return value.
    pub cost: i32,
}

/// Out-parameters of C++ `CvPlayer::calculateUnitSupply(int& iPaidUnits, int& iBaseSupplyCost)`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitSupplyBreakdown {
    pub paid_units: i32,
    pub base_supply_cost: i32,
    /// The C++ return value.
    pub supply: i32,
}

impl Game {
    /// C++ `GC.getHandicapInfo(getHandicapType())`: the player's own difficulty. Every player
    /// slot has one (C++ would index the table with -1 for NO_HANDICAP).
    fn player_handicap_info(&self, p: PlayerId) -> &'static HandicapInfo {
        let d = self.d;
        let handicap = self.player_get_handicap_type(p).expect("player without a handicap");
        &d.handicaps[handicap]
    }

    /// C++ `std::max(0, (GC.getHandicapInfo(GC.getGameINLINE().getHandicapType()).getAIPerEraModifier()
    /// * getCurrentEra()) + 100)`: the per-era AI factor (percent) of the game difficulty.
    fn player_ai_per_era_percent(&self, p: PlayerId) -> i32 {
        let ai_per_era_modifier = self.d.handicaps[self.handicap].ai_per_era_modifier;
        0.max(ai_per_era_modifier * self.players[p].current_era.i() + 100)
    }

    // ------------------------------------------------------------------ "update every city" helpers

    /// C++ `CvPlayer::updateYield`.
    pub fn player_update_yield(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_yield(c);
        }
    }

    /// C++ `CvPlayer::updateMaintenance`.
    pub fn player_update_maintenance(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_maintenance(c);
        }
    }

    /// C++ `CvPlayer::updatePowerHealth`.
    pub fn player_update_power_health(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_power_health(c);
        }
    }

    /// C++ `CvPlayer::updateExtraBuildingHappiness`.
    pub fn player_update_extra_building_happiness(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_extra_building_happiness(c);
        }
    }

    /// C++ `CvPlayer::updateExtraBuildingHealth`.
    pub fn player_update_extra_building_health(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_extra_building_health(c);
        }
    }

    /// C++ `CvPlayer::updateFeatureHappiness`.
    pub fn player_update_feature_happiness(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_feature_happiness(c);
        }
    }

    /// C++ `CvPlayer::updateReligionHappiness`.
    pub fn player_update_religion_happiness(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_religion_happiness(c);
        }
    }

    /// C++ `CvPlayer::updateExtraSpecialistYield`.
    pub fn player_update_extra_specialist_yield(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_extra_specialist_yields(c);
        }
    }

    /// C++ `CvPlayer::updateCommerce(eCommerce)`.
    pub fn player_update_commerce_type(&mut self, p: PlayerId, commerce: CommerceType) {
        for c in self.players[p].cities.clone() {
            self.city_update_commerce(c, commerce);
        }
    }

    /// C++ `CvPlayer::updateCommerce()`.
    pub fn player_update_commerce(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_commerces(c);
        }
    }

    /// C++ `CvPlayer::updateBuildingCommerce`.
    pub fn player_update_building_commerce(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_building_commerce(c);
        }
    }

    /// C++ `CvPlayer::updateReligionCommerce`.
    pub fn player_update_religion_commerce(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_religion_commerces(c);
        }
    }

    /// C++ `CvPlayer::updateCorporation`.
    pub fn player_update_corporation(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_update_corporation(c);
        }
    }

    /// C++ `CvPlayer::updateCityPlotYield`: recomputes the yield of every city's own plot.
    pub fn player_update_city_plot_yield(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            let plot = self.city_plot(c);
            self.plot_update_yield(plot);
        }
    }

    /// C++ `CvPlayer::updateCitySight(bIncrement, bUpdatePlotGroups)`: applies/removes the
    /// sight of every city's own plot.
    pub fn player_update_city_sight(&mut self, p: PlayerId, increment: bool, update_plot_groups: bool) {
        for c in self.players[p].cities.clone() {
            let plot = self.city_plot(c);
            self.plot_update_sight(plot, increment, update_plot_groups);
        }
    }

    /// C++ `CvPlayer::updateTradeRoutes`: clears all routes, then updates cities in order of
    /// decreasing total trade modifier.
    pub fn player_update_trade_routes(&mut self, p: PlayerId) {
        let cities = self.players[p].cities.clone();

        for &c in &cities {
            self.city_clear_trade_routes(c);
        }

        // C++ `CLinkList<int> cityList`: each city is inserted before the first listed city
        // with a smaller total trade modifier (so ties keep city order), else at the end.
        let mut city_list: Vec<CityId> = Vec::with_capacity(cities.len());

        for &c in &cities {
            let total_trade_modifier = self.city_total_trade_modifier(c, None);

            let pos = city_list.iter().position(|&list_city| total_trade_modifier > self.city_total_trade_modifier(list_city, None));

            match pos {
                Some(i) => city_list.insert(i, c),
                None => city_list.push(c),
            }
        }

        for c in city_list {
            self.city_update_trade_routes(c);
        }
    }

    /// C++ `CvPlayer::invalidatePopulationRankCache`.
    pub fn player_invalidate_population_rank_cache(&mut self, p: PlayerId) {
        for c in self.players[p].cities.clone() {
            self.city_invalidate_population_rank_cache(c);
        }
    }

    /// C++ `CvPlayer::invalidateYieldRankCache(eYield = NO_YIELD)` (every yield is invalidated).
    pub fn player_invalidate_yield_rank_cache(&mut self, p: PlayerId, _yield_type: Option<YieldType>) {
        // C++ quirk: eYield is ignored; every city invalidates all yields (`invalidateYieldRankCache()`).
        for c in self.players[p].cities.clone() {
            self.city_invalidate_yield_rank_cache(c, None);
        }
    }

    /// C++ `CvPlayer::invalidateCommerceRankCache(eCommerce = NO_COMMERCE)` (every commerce is invalidated).
    pub fn player_invalidate_commerce_rank_cache(&mut self, p: PlayerId, _commerce: Option<CommerceType>) {
        // C++ quirk: eCommerce is ignored; every city invalidates all commerce types.
        for c in self.players[p].cities.clone() {
            self.city_invalidate_commerce_rank_cache(c, None);
        }
    }

    // ------------------------------------------------------------------ totals over cities

    /// C++ `CvPlayer::calculateTotalYield(eYield)`.
    pub fn player_calculate_total_yield(&self, p: PlayerId, yield_type: YieldType) -> i32 {
        let mut total = 0;
        for &c in &self.players[p].cities {
            total += self.city_yield_rate(c, yield_type);
        }
        total
    }

    /// C++ `CvPlayer::calculateTotalExports(eYield)`: trade yield of routes to foreign cities.
    pub fn player_calculate_total_exports(&self, p: PlayerId, yield_type: YieldType) -> i32 {
        let mut total_exports = 0;

        for &c in &self.players[p].cities {
            for trade_loop in 0..self.city_trade_routes(c) {
                if let Some(trade_city) = self.city_trade_city(c, trade_loop as usize) {
                    if self.cities[trade_city].owner != p {
                        total_exports += self.city_calculate_trade_yield(c, yield_type, self.city_calculate_trade_profit(c, trade_city));
                    }
                }
            }
        }

        total_exports
    }

    /// C++ `CvPlayer::calculateTotalImports(eYield)`: trade yield of other civs' routes to
    /// this player's cities.
    pub fn player_calculate_total_imports(&self, p: PlayerId, yield_type: YieldType) -> i32 {
        let mut total_imports = 0;

        // Loop through players
        for other in self.civ_player_ids() {
            if other != p {
                for &c in &self.players[other].cities {
                    for trade_loop in 0..self.city_trade_routes(c) {
                        if let Some(trade_city) = self.city_trade_city(c, trade_loop as usize) {
                            if self.cities[trade_city].owner == p {
                                total_imports += self.city_calculate_trade_yield(c, yield_type, self.city_calculate_trade_profit(c, trade_city));
                            }
                        }
                    }
                }
            }
        }

        total_imports
    }

    /// C++ `CvPlayer::calculateTotalCityHappiness`.
    pub fn player_calculate_total_city_happiness(&self, p: PlayerId) -> i32 {
        let mut total = 0;
        for &c in &self.players[p].cities {
            total += self.city_happy_level(c);
        }
        total
    }

    /// C++ `CvPlayer::calculateTotalCityUnhappiness`.
    pub fn player_calculate_total_city_unhappiness(&self, p: PlayerId) -> i32 {
        let mut total = 0;
        for &c in &self.players[p].cities {
            total += self.city_unhappy_level(c, 0);
        }
        total
    }

    /// C++ `CvPlayer::calculateTotalCityHealthiness`.
    pub fn player_calculate_total_city_healthiness(&self, p: PlayerId) -> i32 {
        let mut total = 0;
        for &c in &self.players[p].cities {
            total += self.city_good_health(c);
        }
        total
    }

    /// C++ `CvPlayer::calculateTotalCityUnhealthiness`.
    pub fn player_calculate_total_city_unhealthiness(&self, p: PlayerId) -> i32 {
        let mut total = 0;
        for &c in &self.players[p].cities {
            total += self.city_bad_health(c, false, 0);
        }
        total
    }

    // ------------------------------------------------------------------ gold: costs and income

    /// C++ `CvPlayer::calculateUnitCost(int& iFreeUnits, ...)`: unit support cost with its
    /// breakdown (FFH: units with a duration are free).
    pub fn player_calculate_unit_cost_breakdown(&self, p: PlayerId) -> UnitCostBreakdown {
        let d = self.d;
        let pl = &self.players[p];
        let handicap = self.player_handicap_info(p);

        let mut free_units = handicap.free_units;

        free_units += pl.base_free_units;
        free_units += (pl.total_population * pl.free_units_population_percent) / 100;

        //FfH: Added by Kael 06/20/2010
        for &u in &pl.units {
            if self.units[u].duration > 0 {
                free_units += 1;
            }
        }
        //FfH: End Add

        let mut free_military_units = pl.base_free_military_units;
        free_military_units += (pl.total_population * pl.free_military_units_population_percent) / 100;

        if !pl.human {
            if self.team_has_met_human(pl.team) {
                free_units += pl.num_cities(); // XXX
                free_military_units += pl.num_cities(); // XXX
            }
        }

        let paid_units = 0.max(pl.num_units() - free_units);
        let paid_military_units = 0.max(pl.num_military_units - free_military_units);

        let base_unit_cost = paid_units * pl.gold_per_unit;
        let military_cost = paid_military_units * pl.gold_per_military_unit;
        let extra_cost = pl.extra_unit_cost;

        let mut support = military_cost + base_unit_cost + extra_cost;

        support *= handicap.unit_cost_percent;
        support /= 100;

        if !pl.human && !self.player_is_barbarian(p) {
            support *= d.handicaps[self.handicap].ai_unit_cost_percent;
            support /= 100;

            support *= self.player_ai_per_era_percent(p);
            support /= 100;
        }

        debug_assert!(support >= 0);

        UnitCostBreakdown {
            free_units,
            free_military_units,
            paid_units,
            paid_military_units,
            base_unit_cost,
            military_cost,
            extra_cost,
            cost: 0.max(support),
        }
    }

    /// C++ `CvPlayer::calculateUnitCost()` (0 in anarchy, with FFH disabled production or
    /// GAMEOPTION_NO_MAINTENANCE).
    pub fn player_calculate_unit_cost(&self, p: PlayerId) -> i32 {
        if self.player_is_anarchy(p) {
            return 0;
        }

        //FfH: Added by Kael 10/03/2008
        if self.players[p].disable_production > 0 || self.is_option(GameOptionType::GAMEOPTION_NO_MAINTENANCE) {
            return 0;
        }
        //FfH: End Add

        self.player_calculate_unit_cost_breakdown(p).cost
    }

    /// C++ `CvPlayer::calculateUnitSupply()` (0 in anarchy, with FFH disabled production or
    /// GAMEOPTION_NO_MAINTENANCE).
    pub fn player_calculate_unit_supply(&self, p: PlayerId) -> i32 {
        if self.player_is_anarchy(p) {
            return 0;
        }

        //FfH: Added by Kael 10/03/2008
        if self.players[p].disable_production > 0 || self.is_option(GameOptionType::GAMEOPTION_NO_MAINTENANCE) {
            return 0;
        }
        //FfH: End Add

        self.player_calculate_unit_supply_breakdown(p).supply
    }

    /// C++ `CvPlayer::calculateUnitSupply(int& iPaidUnits, int& iBaseSupplyCost)`.
    pub fn player_calculate_unit_supply_breakdown(&self, p: PlayerId) -> UnitSupplyBreakdown {
        let d = self.d;

        let paid_units = 0.max(self.players[p].num_outside_units - d.defines.initial_free_outside_units);

        let mut base_supply_cost = paid_units * d.defines.initial_outside_unit_gold_percent;
        base_supply_cost /= 100;

        let mut supply = base_supply_cost;

        if !self.players[p].human && !self.player_is_barbarian(p) {
            supply *= d.handicaps[self.handicap].ai_unit_supply_percent;
            supply /= 100;

            supply *= self.player_ai_per_era_percent(p);
            supply /= 100;
        }

        debug_assert!(supply >= 0);

        UnitSupplyBreakdown { paid_units, base_supply_cost, supply }
    }

    /// C++ `CvPlayer::calculatePreInflatedCosts` (units + supply + maintenance + civic upkeep
    /// + Python `getExtraCost`).
    pub fn player_calculate_pre_inflated_costs(&self, p: PlayerId) -> i32 {
        // Python getExtraCost(ePlayer): FFH2's CvGameUtils returns 0.
        let extra_cost = 0;

        self.player_calculate_unit_cost(p)
            + self.player_calculate_unit_supply(p)
            + self.player_get_total_maintenance(p)
            + self.player_get_civic_upkeep(p, None, false)
            + extra_cost
    }

    /// C++ `CvPlayer::calculateInflationRate`.
    pub fn player_calculate_inflation_rate(&self, p: PlayerId) -> i32 {
        //FfH: Added by Kael 11/18/2008
        if self.is_option(GameOptionType::GAMEOPTION_NO_INFLATION) {
            return 0;
        }
        //FfH: End Add

        let d = self.d;
        let speed = &d.game_speeds[self.game_speed_type()];

        let mut turns = (self.game_turn() + self.elapsed_game_turns) / 2;

        if self.max_turns() > 0 {
            turns = self.max_turns().min(turns);
        }

        turns += speed.inflation_offset;

        if turns <= 0 {
            return 0;
        }

        let mut inflation_per_turn_times10000 = speed.inflation_percent;
        inflation_per_turn_times10000 *= self.player_handicap_info(p).inflation_percent;
        inflation_per_turn_times10000 /= 100;

        let mut modifier = self.players[p].inflation_modifier;
        if !self.players[p].human && !self.player_is_barbarian(p) {
            let mut ai_modifier = d.handicaps[self.handicap].ai_inflation_percent;
            ai_modifier *= self.player_ai_per_era_percent(p);
            ai_modifier /= 100;

            modifier += ai_modifier - 100;
        }

        inflation_per_turn_times10000 *= 0.max(100 + modifier);
        inflation_per_turn_times10000 /= 100;

        // Keep up to second order terms in binomial series
        let mut rate_percent = (turns * inflation_per_turn_times10000) / 100;
        // C++ quirk: the 32-bit product (evaluated left to right) can overflow late in long
        // games; MSVC wraps it.
        rate_percent += turns
            .wrapping_mul(turns - 1)
            .wrapping_mul(inflation_per_turn_times10000)
            .wrapping_mul(inflation_per_turn_times10000)
            / 2000000;

        // C++: FAssert(iRatePercent >= 0) -- debug-only, and the wrapped product can break it.
        rate_percent
    }

    /// C++ `CvPlayer::calculateInflatedCosts`.
    pub fn player_calculate_inflated_costs(&self, p: PlayerId) -> i32 {
        let mut costs = self.player_calculate_pre_inflated_costs(p);

        costs *= 0.max(self.player_calculate_inflation_rate(p) + 100);
        costs /= 100;

        costs
    }

    /// C++ `CvPlayer::calculateBaseNetGold`.
    pub fn player_calculate_base_net_gold(&self, p: PlayerId) -> i32 {
        let mut net_gold = self.player_get_commerce_rate(p, CommerceType::COMMERCE_GOLD) + self.players[p].gold_per_turn;
        net_gold -= self.player_calculate_inflated_costs(p);
        net_gold
    }

    /// C++ `CvPlayer::calculateGoldRate`.
    pub fn player_calculate_gold_rate(&self, p: PlayerId) -> i32 {
        if self.player_is_commerce_flexible(p, CommerceType::COMMERCE_RESEARCH) {
            self.player_calculate_base_net_gold(p)
        } else {
            0.min(self.player_calculate_base_net_research(p, None) + self.player_calculate_base_net_gold(p))
        }
    }

    /// C++ `CvPlayer::calculateTotalCommerce`.
    pub fn player_calculate_total_commerce(&self, p: PlayerId) -> i32 {
        let mut total = self.player_calculate_base_net_gold(p) + self.player_calculate_base_net_research(p, None);
        for i in 0..CommerceType::NUM {
            let c = CommerceType::from_usize(i);
            if c != CommerceType::COMMERCE_GOLD && c != CommerceType::COMMERCE_RESEARCH {
                total += self.player_get_commerce_rate(p, c);
            }
        }
        total
    }

    /// C++ `CvPlayer::doGold` (Python `doGold` override; gold strike and unit disbanding).
    pub fn player_do_gold(&mut self, p: PlayerId) {
        // Python doGold(ePlayer): FFH2's CvGameUtils returns False, so the C++ default runs.

        let gold_change = self.player_calculate_gold_rate(p);

        // C++: FAssert(isHuman() || isBarbarian() || ((getGold() + iGoldChange) >= 0) || isAnarchy())
        // -- debug-only, and it fails whenever even a 100% gold rate cannot pay the costs.

        // changeGold(iGoldChange)
        self.players[p].gold += gold_change;

        let mut strike = false;

        if self.players[p].gold < 0 {
            // setGold(0)
            self.players[p].gold = 0;

            if !self.player_is_barbarian(p) && self.players[p].num_cities() > 0 {
                strike = true;
            }
        }

        if strike {
            // setStrike(true): the "units on strike" message is shown only to the active player (UI).
            self.players[p].strike = true;
            // changeStrikeTurns(1)
            self.players[p].strike_turns += 1;

            if self.players[p].strike_turns > 1 {
                let disband_unit = self.players[p].strike_turns / 2; // XXX mod?

                for _ in 0..disband_unit {
                    self.player_disband_unit(p, true);

                    if self.player_calculate_gold_rate(p) >= 0 {
                        break;
                    }
                }
            }
        } else {
            // setStrike(false)
            self.players[p].strike = false;
        }
    }

    /// C++ `CvPlayer::verifyGoldCommercePercent`: raises the gold rate until the treasury
    /// does not go negative.
    pub fn player_verify_gold_commerce_percent(&mut self, p: PlayerId) {
        while self.players[p].gold + self.player_calculate_gold_rate(p) < 0 {
            let step = self.d.defines.commerce_percent_change_increments;
            self.player_change_commerce_percent(p, CommerceType::COMMERCE_GOLD, step);

            if self.players[p].commerce_percent[CommerceType::COMMERCE_GOLD.idx()] == 100 {
                break;
            }
        }
    }

    /// C++ `CvPlayer::getTotalMaintenance` (the member is stored x100).
    pub fn player_get_total_maintenance(&self, p: PlayerId) -> i32 {
        self.players[p].total_maintenance / 100
    }

    /// C++ `CvPlayer::changeDistanceMaintenanceModifier`.
    pub fn player_change_distance_maintenance_modifier(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].distance_maintenance_modifier += change;
            self.player_update_maintenance(p);
        }
    }

    /// C++ `CvPlayer::changeNumCitiesMaintenanceModifier`.
    pub fn player_change_num_cities_maintenance_modifier(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].num_cities_maintenance_modifier += change;
            self.player_update_maintenance(p);
        }
    }

    /// C++ `CvPlayer::changeCorporationMaintenanceModifier`.
    pub fn player_change_corporation_maintenance_modifier(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].corporation_maintenance_modifier += change;
            self.player_update_maintenance(p);
        }
    }

    /// C++ `CvPlayer::changeGoldPerTurnByPlayer`: gold-per-turn deal payments.
    pub fn player_change_gold_per_turn_by_player(&mut self, p: PlayerId, other: PlayerId, change: i32) {
        if change != 0 {
            let pl = &mut self.players[p];
            pl.gold_per_turn += change;
            pl.gold_per_turn_by_player[other] += change;

            if !pl.human {
                // AI: AI_doCommerce() (re-balances the commerce sliders)
            }
        }
    }

    // ------------------------------------------------------------------ commerce

    /// C++ `CvPlayer::setCommercePercent`: clamps to 0..100 and re-balances the other
    /// commerce percents so they total 100.
    pub fn player_set_commerce_percent(&mut self, p: PlayerId, commerce: CommerceType, new_value: i32) {
        let ci = commerce.idx();
        let old_value = self.players[p].commerce_percent[ci];
        // C++ range(iNewValue, 0, 100)
        self.players[p].commerce_percent[ci] = new_value.max(0).min(100);

        if old_value != self.players[p].commerce_percent[ci] {
            let percents = &mut self.players[p].commerce_percent;
            let mut total: i32 = percents.iter().sum();

            for i in 0..CommerceType::NUM {
                if i != ci {
                    if total != 100 {
                        let adjustment = percents[i].min(total - 100);
                        percents[i] -= adjustment;
                        total -= adjustment;
                    } else {
                        break;
                    }
                }
            }

            debug_assert_eq!(total, 100);

            self.player_update_commerce(p);
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeCommercePercent`.
    pub fn player_change_commerce_percent(&mut self, p: PlayerId, commerce: CommerceType, change: i32) {
        let new_value = self.players[p].commerce_percent[commerce.idx()] + change;
        self.player_set_commerce_percent(p, commerce, new_value);
    }

    /// C++ `CvPlayer::getCommerceRate(eIndex)`: the x100 member divided by 100; with
    /// GAMEOPTION_NO_ESPIONAGE (always on in FFH, see `CvGame::isOption`) espionage counts
    /// as culture; 0 while FFH research is disabled.
    pub fn player_get_commerce_rate(&self, p: PlayerId, commerce: CommerceType) -> i32 {
        let pl = &self.players[p];

        let mut rate = pl.commerce_rate[commerce.idx()];
        if self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
            if commerce == CommerceType::COMMERCE_CULTURE {
                rate += pl.commerce_rate[CommerceType::COMMERCE_ESPIONAGE.idx()];
            } else if commerce == CommerceType::COMMERCE_ESPIONAGE {
                rate = 0;
            }
        }

        //FfH: Added by Kael 08/28/2008 (every commerce type, not only research, reads 0)
        if pl.disable_research > 0 {
            return 0;
        }
        //FfH: End Add

        rate / 100
    }

    /// C++ `CvPlayer::changeCommerceRateModifier`.
    pub fn player_change_commerce_rate_modifier(&mut self, p: PlayerId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.players[p].commerce_rate_modifier[commerce.idx()] += change;
            self.player_update_commerce_type(p, commerce);
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeCapitalCommerceRateModifier`.
    pub fn player_change_capital_commerce_rate_modifier(&mut self, p: PlayerId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.players[p].capital_commerce_rate_modifier[commerce.idx()] += change;

            if let Some(capital_city) = self.player_get_capital_city(p) {
                self.city_update_commerces(capital_city);

                // AI_setAssignWorkDirty(true)
                self.cities[capital_city].ai_assign_work_dirty = true;
            }
        }
    }

    /// C++ `CvPlayer::changeFreeCityCommerce`.
    pub fn player_change_free_city_commerce(&mut self, p: PlayerId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.players[p].free_city_commerce[commerce.idx()] += change;
            debug_assert!(self.players[p].free_city_commerce[commerce.idx()] >= 0);
            self.player_update_commerce_type(p, commerce);
        }
    }

    /// C++ `CvPlayer::changeStateReligionBuildingCommerce`.
    pub fn player_change_state_religion_building_commerce(&mut self, p: PlayerId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.players[p].state_religion_building_commerce[commerce.idx()] += change;
            // C++ quirk: the C++ FAssert can fail for a player added to a team mid-game (bTeamShare
            // buildings, see `player_change_trade_routes`).
            self.player_update_commerce_type(p, commerce);
        }
    }

    /// C++ `CvPlayer::changeSpecialistExtraCommerce`.
    pub fn player_change_specialist_extra_commerce(&mut self, p: PlayerId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.players[p].specialist_extra_commerce[commerce.idx()] += change;
            // C++ quirk: can go negative (the C++ FAssert fails) for a player added to a team
            // mid-game, who loses the team's `bTeamShare` buildings without having had them
            // (see `player_change_trade_routes`).
            self.player_update_commerce_type(p, commerce);
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeSpecialistTypeExtraCommerce` (FFH).
    pub fn player_change_specialist_type_extra_commerce(&mut self, p: PlayerId, specialist: SpecialistType, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.players[p].specialist_type_extra_commerce[specialist][commerce.idx()] += change;
            self.player_update_commerce(p);
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::isCommerceFlexible(eIndex)`.
    pub fn player_is_commerce_flexible(&self, p: PlayerId, commerce: CommerceType) -> bool {
        let pl = &self.players[p];

        if !pl.founded_first_city {
            return false;
        }

        if commerce == CommerceType::COMMERCE_ESPIONAGE {
            if 0 == self.team_get_has_met_civ_count(pl.team, true) || self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
                return false;
            }
        }

        self.d.commerces[commerce].flexible_percent
            || pl.commerce_flexible_count[commerce.idx()] > 0
            || self.team_is_commerce_flexible(pl.team, commerce)
    }

    /// C++ `CvPlayer::changeCommerceFlexibleCount`.
    pub fn player_change_commerce_flexible_count(&mut self, p: PlayerId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.players[p].commerce_flexible_count[commerce.idx()] += change;
            // C++ quirk: the C++ FAssert can fail for a player added to a team mid-game (bTeamShare
            // buildings, see `player_change_trade_routes`).

            if !self.player_is_commerce_flexible(p, commerce) {
                self.player_set_commerce_percent(p, commerce, 0);
            }
        }
    }

    /// C++ `CvPlayer::specialistCommerce(eSpecialist, eCommerce)` (FFH adds the
    /// per-specialist-type extra commerce).
    pub fn player_specialist_commerce(&self, p: PlayerId, specialist: SpecialistType, commerce: CommerceType) -> i32 {
        let ci = commerce.idx();
        let pl = &self.players[p];

        //FfH: Modified by Kael 11/08/2007
        self.d.specialists[specialist].commerce_change[ci] + pl.specialist_extra_commerce[ci] + pl.specialist_type_extra_commerce[specialist][ci]
        //FfH: End Modify
    }

    // ------------------------------------------------------------------ research

    /// C++ `CvPlayer::chooseTech(iDiscover, szText, bFront)`: in C++ a tech-chooser popup
    /// for humans (`iDiscover` free techs); route the choice through the controller.
    ///
    /// The popup (`CvDLLButtonPopup::launchChooseTechPopup`) lists the techs `canResearch`
    /// accepts; a research choice (`iDiscover == 0`) is cancelled while the player already
    /// researches something or the game is over (`OnFocus`). The pick runs as `CvNetResearch`
    /// (CvMessageData.cpp): a free tech is granted (`setHasTech`) and the popup reopens for
    /// the remaining free techs; a research pick replaces the queue (`pushResearch(eTech, true)`).
    /// `szText` (popup header) and `bFront` (popup order) are UI only.
    ///
    /// The popup goes through `CvPlayer::addPopup`, which deletes it for a non-human player, so
    /// for AI players the call does nothing (e.g. `CvTeam::setHasTech`'s "research next" popup
    /// or the Iron Orb event's free tech).
    pub fn player_choose_tech(&mut self, p: PlayerId, discover: i32) {
        // `CvPlayer::addPopup` deletes popups for non-human players, so for an AI player this
        // does nothing (e.g. Python `chooseTech(1, ..)` in the Iron Orb event grants no tech).
        if !self.players[p].human {
            return;
        }
        // Python skipResearchPopup(ePlayer): FFH2's CvGameUtils returns False.
        let team = self.players[p].team;

        if discover > 0 {
            // AI_chooseFreeTech's decision for humans: the controller picks a free tech.
            let Some(tech) = self.controller(p).choose_free_tech(self, p) else {
                return;
            };
            if !self.player_can_research(p, tech, false) {
                return;
            }

            // CvNetResearch::Execute (m_iDiscover > 0)
            self.team_set_has_tech(team, tech, true, Some(p), true, true);

            if discover > 1 {
                // C++ reopens the popup for the active player, who made this choice.
                self.player_choose_tech(p, discover - 1);
            }
        } else {
            // OnFocus: the research popup is cancelled while research is set or the game is over.
            if self.player_get_current_research(p).is_some() || self.game_state == GameStateType::GAMESTATE_OVER {
                return;
            }

            let Some(tech) = self.controller(p).choose_research(self, p) else {
                return;
            };

            // CvNetResearch::Execute (m_iDiscover == 0, no shift key: replace the queue)
            if self.player_can_ever_research(p, tech) {
                if self.team_is_has_tech(team, Some(tech)) || self.players[p].researching_tech[tech] {
                    self.player_clear_research_queue(p);
                }
                self.player_push_research(p, tech, true);
            }
        }
    }

    /// C++ `CvPlayer::calculateResearchModifier(eTech)` (100 for NO_TECH).
    pub fn player_calculate_research_modifier(&self, p: PlayerId, tech: Option<TechType>) -> i32 {
        let mut modifier = 100;

        let Some(tech) = tech else {
            return modifier;
        };

        let d = self.d;
        let team = self.players[p].team;

        let mut known_count = 0;
        let mut possible_known_count = 0;

        for t in self.civ_team_ids() {
            if self.team_is_alive(t) {
                if self.teams[team].has_met[t] {
                    if self.team_is_has_tech(t, Some(tech)) {
                        known_count += 1;
                    }
                }

                possible_known_count += 1;
            }
        }

        if possible_known_count > 0 {
            modifier += (d.defines.tech_cost_total_known_team_modifier * known_count) / possible_known_count;
        }

        let mut possible_paths = 0;
        let mut unknown_paths = 0;

        for &prereq in &d.techs[tech].prereq_or_techs {
            if !self.team_is_has_tech(team, Some(prereq)) {
                unknown_paths += 1;
            }

            possible_paths += 1;
        }

        debug_assert!(possible_paths >= unknown_paths, "The number of possible paths is expected to match or exceed the number of unknown ones");

        modifier += (possible_paths - unknown_paths) * d.defines.tech_cost_known_prereq_modifier;

        modifier
    }

    /// C++ `CvPlayer::calculateBaseNetResearch(eTech = NO_TECH)` (NO_TECH = current research).
    pub fn player_calculate_base_net_research(&self, p: PlayerId, tech: Option<TechType>) -> i32 {
        let research_tech = if tech.is_some() { tech } else { self.player_get_current_research(p) };
        ((self.d.defines.base_research_rate + self.player_get_commerce_rate(p, CommerceType::COMMERCE_RESEARCH))
            * self.player_calculate_research_modifier(p, research_tech))
            / 100
    }

    /// C++ `CvPlayer::calculateResearchRate(eTech = NO_TECH)`.
    pub fn player_calculate_research_rate(&self, p: PlayerId, tech: Option<TechType>) -> i32 {
        if self.player_is_commerce_flexible(p, CommerceType::COMMERCE_RESEARCH) {
            self.player_calculate_base_net_research(p, tech)
        } else {
            1.max(self.player_calculate_base_net_research(p, tech) + self.player_calculate_base_net_gold(p))
        }
    }

    /// C++ `CvPlayer::isResearch` (Python `isPlayerResearch`; requires a founded first city).
    pub fn player_is_research(&self, p: PlayerId) -> bool {
        // Python isPlayerResearch(ePlayer) only with USE_IS_PLAYER_RESEARCH_CALLBACK (0 in FFH2);
        // FFH2's CvGameUtils returns True anyway, which never vetoes.

        if !self.players[p].founded_first_city {
            return false;
        }

        true
    }

    /// C++ `CvPlayer::canEverResearch(eTech)`.
    pub fn player_can_ever_research(&self, p: PlayerId, tech: TechType) -> bool {
        let d = self.d;

        if d.techs[tech].disable {
            return false;
        }

        let civ = self.player_get_civilization_type(p).expect("player without a civilization");
        if d.civilizations[civ].civilization_disable_techs[tech] {
            return false;
        }

        //FfH Techs: Added by Kael 08/09/2007
        if let Some(prereq_religion) = d.techs[tech].prereq_religion {
            if self.player_get_state_religion(p) != Some(prereq_religion) {
                return false;
            }
        }
        //FfH: End Add

        if d.defines.use_cannot_research_callback != 0 {
            // Python cannotResearch(ePlayer, eTech, False)
            if self.py_cannot_research(p, tech, false) {
                return false;
            }
        }

        true
    }

    /// C++ `CvPlayer::canResearch(eTech, bTrade = false)`.
    pub fn player_can_research(&self, p: PlayerId, tech: TechType, trade: bool) -> bool {
        // Python canResearch(ePlayer, eTech, bTrade) only with USE_CAN_RESEARCH_CALLBACK (0 in
        // FFH2); FFH2's CvGameUtils returns False anyway (= no override).

        let d = self.d;

        if !self.player_is_research(p) && self.players[p].advanced_start_points < 0 {
            return false;
        }

        let team = self.players[p].team;

        if self.team_is_has_tech(team, Some(tech)) {
            return false;
        }

        let mut found_possible = false;
        let mut found_valid = false;

        for &prereq in &d.techs[tech].prereq_or_techs {
            found_possible = true;

            if self.team_is_has_tech(team, Some(prereq)) {
                if !trade || self.is_option(GameOptionType::GAMEOPTION_NO_TECH_BROKERING) || !self.teams[team].no_trade_tech[prereq] {
                    found_valid = true;
                    break;
                }
            }
        }

        if found_possible && !found_valid {
            return false;
        }

        for &prereq in &d.techs[tech].prereq_and_techs {
            if !self.team_is_has_tech(team, Some(prereq)) {
                return false;
            }

            if trade && !self.is_option(GameOptionType::GAMEOPTION_NO_TECH_BROKERING) && self.teams[team].no_trade_tech[prereq] {
                return false;
            }
        }

        if !self.player_can_ever_research(p, tech) {
            return false;
        }

        true
    }

    /// C++ `CvPlayer::getCurrentResearch` (head of the research queue).
    pub fn player_get_current_research(&self, p: PlayerId) -> Option<TechType> {
        self.players[p].research_queue.first().copied()
    }

    /// C++ `CvPlayer::isCurrentResearchRepeat`.
    pub fn player_is_current_research_repeat(&self, p: PlayerId) -> bool {
        match self.player_get_current_research(p) {
            Some(current_research) => self.d.techs[current_research].repeat,
            None => false,
        }
    }

    /// C++ `CvPlayer::isNoResearchAvailable`.
    pub fn player_is_no_research_available(&self, p: PlayerId) -> bool {
        if self.player_get_current_research(p).is_some() {
            return false;
        }

        for tech in self.d.techs.keys() {
            if self.player_can_research(p, tech, false) {
                return false;
            }
        }

        true
    }

    /// C++ `CvPlayer::getResearchTurnsLeft(eTech, bOverflow)`.
    pub fn player_get_research_turns_left(&self, p: PlayerId, tech: TechType, overflow: bool) -> i32 {
        let turns_left = self.player_get_research_turns_left_times100(p, tech, overflow);
        // C++ quirk: MAX_INT + 99 overflows (to a negative value, clamped to 1 below).
        let turns_left = turns_left.wrapping_add(99) / 100; // round up
        1.max(turns_left)
    }

    /// C++ `CvPlayer::getResearchTurnsLeftTimes100(eTech, bOverflow)` (MAX_INT if no research).
    pub fn player_get_research_turns_left_times100(&self, p: PlayerId, tech: TechType, overflow: bool) -> i32 {
        let team = self.players[p].team;

        let mut research_rate = 0;
        let mut overflow_research = 0;

        for other in self.player_ids() {
            if self.players[other].alive {
                if self.players[other].team == team {
                    if other == p || self.player_get_current_research(other) == Some(tech) {
                        research_rate += self.player_calculate_research_rate(other, Some(tech));
                        // C++: the modifier is this player's (`calculateResearchModifier(eTech)`).
                        overflow_research += (self.players[other].overflow_research * self.player_calculate_research_modifier(p, Some(tech))) / 100;
                    }
                }
            }
        }

        if research_rate == 0 {
            return i32::MAX;
        }

        let mut research_left = self.team_get_research_left(team, tech);

        if overflow {
            research_left -= overflow_research;
        }

        research_left *= 100;

        let mut turns_left = research_left / research_rate;

        if turns_left * research_rate < research_left {
            turns_left += 1;
        }

        1.max(turns_left)
    }

    /// C++ `CvPlayer::findPathLength(eTech, bCost = true)`: research cost (or number of
    /// techs) still needed for `tech`, through its AND prerequisites and the cheapest OR one.
    pub fn player_find_path_length(&self, p: PlayerId, tech: TechType, cost: bool) -> i32 {
        let d = self.d;
        let team = self.players[p].team;

        if self.team_is_has_tech(team, Some(tech)) || self.players[p].researching_tech[tech] {
            // We have this tech, no reason to add this to the pre-reqs
            // Base case return 0, we know it...
            return 0;
        }

        let mut path_length = 0;

        // Cycle through the and paths and add up their tech lengths
        for &prereq in &d.techs[tech].prereq_and_techs {
            path_length += self.player_find_path_length(p, prereq, cost);
        }

        let mut shortest_or: Option<TechType> = None;
        let mut shortest_path = i32::MAX;
        // Find the shortest OR tech
        for &prereq in &d.techs[tech].prereq_or_techs {
            // Recursively find the path length (takes into account all ANDs)
            let num_steps = self.player_find_path_length(p, prereq, cost);

            // If the prereq is a valid tech and its the current shortest, mark it as such
            if num_steps < shortest_path {
                shortest_or = Some(prereq);
                shortest_path = num_steps;
            }
        }

        // If the shortest OR is a valid tech, add the steps to it...
        if shortest_or.is_some() {
            path_length += shortest_path;
        }

        path_length + if cost { self.team_get_research_cost(team, tech) } else { 1 }
    }

    /// C++ `CvPlayer::getQueuePosition(eTech)`: 1-based position in the research queue, -1 if absent.
    pub fn player_get_queue_position(&self, p: PlayerId, tech: TechType) -> i32 {
        match self.players[p].research_queue.iter().position(|&t| t == tech) {
            Some(i) => i as i32 + 1,
            None => -1,
        }
    }

    /// C++ `CvPlayer::clearResearchQueue`.
    pub fn player_clear_research_queue(&mut self, p: PlayerId) {
        let pl = &mut self.players[p];
        pl.research_queue.clear();
        pl.researching_tech.fill(false);
    }

    /// C++ `CvPlayer::pushResearch(eTech, bClear = false)`: queues `tech` and its missing
    /// prerequisites (shortest OR path); Python `techSelected` event.
    pub fn player_push_research(&mut self, p: PlayerId, tech: TechType, clear: bool) -> bool {
        let d = self.d;
        let team = self.players[p].team;

        if self.team_is_has_tech(team, Some(tech)) || self.players[p].researching_tech[tech] {
            // We have this tech, no reason to add this to the pre-reqs
            return true;
        }

        if !self.player_can_ever_research(p, tech) {
            return false;
        }

        // Pop the entire queue...
        if clear {
            self.player_clear_research_queue(p);
        }

        // Add in all the pre-reqs for the and techs...
        for &prereq in &d.techs[tech].prereq_and_techs {
            if !self.player_push_research(p, prereq, false) {
                return false;
            }
        }

        // Will return the shortest path of all the or techs.  Tie breaker goes to the first one...
        let mut shortest_or: Option<TechType> = None;
        let mut shortest_path = i32::MAX;
        let mut or_prereq_found = false;
        // Cycle through all the OR techs
        for &prereq in &d.techs[tech].prereq_or_techs {
            or_prereq_found = true;

            // If the pre-req exists, and we have it, it is the shortest path, get out, we're done
            if self.team_is_has_tech(team, Some(prereq)) {
                shortest_or = Some(prereq);
                break;
            }

            if self.player_can_ever_research(p, prereq) {
                // Find the length of the path to this pre-req
                let num_steps = self.player_find_path_length(p, prereq, true);

                // If this pre-req is a valid tech, and its the shortest current path, set it as such
                if num_steps < shortest_path {
                    shortest_or = Some(prereq);
                    shortest_path = num_steps;
                }
            }
        }

        // If the shortest path tech is valid, push it (and its children) on to the research queue recursively
        if let Some(shortest_or) = shortest_or {
            if !self.player_push_research(p, shortest_or, false) {
                return false;
            }
        } else if or_prereq_found {
            return false;
        }

        // Insert this tech at the end of the queue
        self.players[p].research_queue.push(tech);

        // setResearchingTech(eTech, true)
        self.players[p].researching_tech[tech] = true;

        // (dirty bits for the active team: UI)

        // ONEVENT - Tech selected (any)
        self.ev_tech_selected(tech, p);

        true
    }

    /// C++ `CvPlayer::popResearch(eTech)`.
    pub fn player_pop_research(&mut self, p: PlayerId, tech: TechType) {
        let pl = &mut self.players[p];
        if let Some(i) = pl.research_queue.iter().position(|&t| t == tech) {
            pl.research_queue.remove(i);
        }
        pl.researching_tech[tech] = false;
    }

    /// C++ `CvPlayer::doResearch` (Python `doResearch` override; research choice through
    /// the controller, overflow, team research progress).
    pub fn player_do_research(&mut self, p: PlayerId) {
        // Python doResearch(ePlayer): FFH2's CvGameUtils returns False, so the C++ default runs.

        if self.player_is_research(p) {
            let mut force_research_choice = false;

            if self.player_get_current_research(p).is_none() {
                if self.active_player == Some(p) {
                    self.player_choose_tech(p, 0);
                }

                if self.elapsed_game_turns > 4 {
                    // AI_chooseResearch() (for every player, humans included, with no popup):
                    // the controller's `ai_choose_research` decides and its tech is pushed (C++
                    // `pushResearch(eBestTech)`).
                    // AI: AI_chooseResearch also returns early while getDisableResearch() > 0,
                    // clears the (empty) queue and first tries a teammate's current research
                    // and Python AI_chooseTech -- choice policy, left to the controller.
                    // AI_chooseResearch returns at once while research is disabled (FfH Stasis,
                    // CvPlayerAI.cpp:5241), keeping the stored research for later.
                    if self.players[p].disable_research == 0 {
                        if let Some(tech) = self.controller(p).ai_choose_research(self, p) {
                            self.player_push_research(p, tech, false);
                        }
                    }

                    force_research_choice = true;
                }
            }

            let current_tech = self.player_get_current_research(p);
            match current_tech {
                None => {
                    let overflow = (100 * self.player_calculate_research_rate(p, None))
                        / 1.max(self.player_calculate_research_modifier(p, current_tech));
                    // changeOverflowResearch(iOverflow)
                    self.players[p].overflow_research += overflow;
                }
                Some(current_tech) => {
                    let overflow_research =
                        (self.players[p].overflow_research * self.player_calculate_research_modifier(p, Some(current_tech))) / 100;
                    // setOverflowResearch(0)
                    self.players[p].overflow_research = 0;
                    let team = self.players[p].team;
                    let change = self.player_calculate_research_rate(p, None) + overflow_research;
                    self.team_change_research_progress(team, current_tech, change, p);
                }
            }

            if force_research_choice {
                self.player_clear_research_queue(p);
            }
        }
    }

    /// C++ `CvPlayer::getTechFreeUnit(eTech)`.
    pub fn player_get_tech_free_unit(&self, p: PlayerId, tech: TechType) -> Option<UnitType> {
        let d = self.d;

        let unit_class = d.techs[tech].first_free_unit_class?;

        let civ = self.player_get_civilization_type(p).expect("player without a civilization");
        let unit = d.civ_unit(civ, unit_class)?;

        if d.units[unit].espionage_points > 0 && self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
            return None;
        }

        Some(unit)
    }

    /// C++ `CvPlayer::isHasTech(iTech)` (FFH shortcut for the team's `isHasTech`).
    pub fn player_is_has_tech(&self, p: PlayerId, tech: TechType) -> bool {
        self.team_is_has_tech(self.players[p].team, Some(tech))
    }

    // ------------------------------------------------------------------ golden age

    /// C++ `CvPlayer::isGoldenAge`.
    pub fn player_is_golden_age(&self, p: PlayerId) -> bool {
        self.players[p].golden_age_turns > 0
    }

    /// C++ `CvPlayer::changeGoldenAgeTurns`: ends anarchy when a golden age starts, updates
    /// yields, Python golden age events.
    pub fn player_change_golden_age_turns(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            let old_golden_age = self.player_is_golden_age(p);

            self.players[p].golden_age_turns += change;
            debug_assert!(self.players[p].golden_age_turns >= 0);

            if old_golden_age != self.player_is_golden_age(p) {
                if self.player_is_golden_age(p) {
                    let anarchy_turns = self.players[p].anarchy_turns;
                    self.player_change_anarchy_turns(p, -anarchy_turns);
                }

                self.player_update_yield(p);

                if self.player_is_golden_age(p) {
                    // Replay message TXT_KEY_MISC_PLAYER_GOLDEN_AGE_BEGINS (replay: not ported).
                    let turn = self.game_turn();
                    let name = self.player_get_name(p);
                    self.log.msg(turn, Some(p), || format!("{name} begins a golden age"));

                    self.ev_golden_age(p);
                } else {
                    self.ev_end_golden_age(p);
                }

                // (TXT_KEY_MISC_PLAYER_GOLDEN_AGE_HAS_BEGUN / _ENDED messages to every alive player
                // that has met this team: UI.)
            }
        }
    }

    /// C++ `CvPlayer::getGoldenAgeLength`.
    pub fn player_get_golden_age_length(&self, p: PlayerId) -> i32 {
        (self.golden_age_length() * 0.max(100 + self.players[p].golden_age_modifier)) / 100
    }

    // ------------------------------------------------------------------ yields

    /// C++ `CvPlayer::changeSeaPlotYield`.
    pub fn player_change_sea_plot_yield(&mut self, p: PlayerId, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.players[p].sea_plot_yield[yield_type.idx()] += change;
            self.player_update_yield(p);
        }
    }

    /// C++ `CvPlayer::changeYieldRateModifier`.
    pub fn player_change_yield_rate_modifier(&mut self, p: PlayerId, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.players[p].yield_rate_modifier[yield_type.idx()] += change;

            self.player_invalidate_yield_rank_cache(p, Some(yield_type));

            if yield_type == YieldType::YIELD_COMMERCE {
                self.player_update_commerce(p);
            }

            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeCapitalYieldRateModifier`.
    pub fn player_change_capital_yield_rate_modifier(&mut self, p: PlayerId, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.players[p].capital_yield_rate_modifier[yield_type.idx()] += change;

            self.player_invalidate_yield_rank_cache(p, Some(yield_type));

            if let Some(capital_city) = self.player_get_capital_city(p) {
                if yield_type == YieldType::YIELD_COMMERCE {
                    self.city_update_commerces(capital_city);
                }

                // AI_setAssignWorkDirty(true)
                self.cities[capital_city].ai_assign_work_dirty = true;

                // (setInfoDirty for the active team: UI)
            }
        }
    }

    /// C++ `CvPlayer::updateExtraYieldThreshold(eIndex)` (best trait threshold).
    pub fn player_update_extra_yield_threshold(&mut self, p: PlayerId, yield_type: YieldType) {
        let d = self.d;
        let yi = yield_type.idx();

        let mut best_value = 0;

        for t in d.traits.keys() {
            if self.players[p].traits[t] {
                let threshold = d.traits[t].extra_yield_threshold[yi];
                if threshold > 0 {
                    if best_value == 0 || threshold < best_value {
                        best_value = threshold;
                    }
                }
            }
        }

        if self.players[p].extra_yield_threshold[yi] != best_value {
            self.players[p].extra_yield_threshold[yi] = best_value;
            debug_assert!(self.players[p].extra_yield_threshold[yi] >= 0);

            self.player_update_yield(p);
        }
    }

    /// C++ `CvPlayer::changeTradeYieldModifier`.
    pub fn player_change_trade_yield_modifier(&mut self, p: PlayerId, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.players[p].trade_yield_modifier[yield_type.idx()] += change;
            self.player_update_trade_routes(p);
        }
    }

    /// C++ `CvPlayer::changeImprovementYieldChange`.
    pub fn player_change_improvement_yield_change(&mut self, p: PlayerId, improvement: ImprovementType, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.players[p].improvement_yield_change[improvement][yield_type.idx()] += change;
            self.player_update_yield(p);
        }
    }

    /// C++ `CvPlayer::changeTradeRoutes`.
    pub fn player_change_trade_routes(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].trade_routes += change;
            // C++ quirk: the C++ `FAssert(>= 0)` can fail: `CvPlayer::setTeam` (FfH
            // `addPlayerAdvanced` puts the Mercurians on their summoner's team) does not process the
            // team's `bTeamShare` buildings for the new member, who still loses them with the
            // team's cities (e.g. Basium converting a teammate's Great Lighthouse city).
            self.player_update_trade_routes(p);
        }
    }

    /// C++ `CvPlayer::changeCoastalTradeRoutes`.
    pub fn player_change_coastal_trade_routes(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].coastal_trade_routes += change;
            // See `player_change_trade_routes` (C++ quirk: can go negative).
            self.player_update_trade_routes(p);
        }
    }

    /// C++ `CvPlayer::isNoForeignTrade`.
    pub fn player_is_no_foreign_trade(&self, p: PlayerId) -> bool {
        self.players[p].no_foreign_trade_count > 0
    }

    /// C++ `CvPlayer::changeNoForeignTradeCount` (updates the trade routes of the whole game).
    pub fn player_change_no_foreign_trade_count(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].no_foreign_trade_count += change;
            debug_assert!(self.players[p].no_foreign_trade_count >= 0);

            self.update_trade_routes();
        }
    }

    // ------------------------------------------------------------------ corporations

    /// C++ `CvPlayer::isNoCorporations`.
    pub fn player_is_no_corporations(&self, p: PlayerId) -> bool {
        self.players[p].no_corporations_count > 0
    }

    /// C++ `CvPlayer::changeNoCorporationsCount`.
    pub fn player_change_no_corporations_count(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].no_corporations_count += change;
            debug_assert!(self.players[p].no_corporations_count >= 0);
            self.player_update_corporation(p);
        }
    }

    /// C++ `CvPlayer::isNoForeignCorporations`.
    pub fn player_is_no_foreign_corporations(&self, p: PlayerId) -> bool {
        self.players[p].no_foreign_corporations_count > 0
    }

    /// C++ `CvPlayer::changeNoForeignCorporationsCount`.
    pub fn player_change_no_foreign_corporations_count(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].no_foreign_corporations_count += change;
            debug_assert!(self.players[p].no_foreign_corporations_count >= 0);
            self.player_update_corporation(p);
        }
    }

    // ------------------------------------------------------------------ happiness and health

    /// C++ `CvPlayer::changeExtraHealth`.
    pub fn player_change_extra_health(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].extra_health += change;
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeBuildingGoodHealth`.
    pub fn player_change_building_good_health(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].building_good_health += change;
            // C++ quirk: the C++ FAssert can fail for a player added to a team mid-game (bTeamShare
            // buildings, see `player_change_trade_routes`).
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeBuildingBadHealth`.
    pub fn player_change_building_bad_health(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].building_bad_health += change;
            // C++ quirk: the C++ FAssert can fail for a player added to a team mid-game (bTeamShare
            // buildings, see `player_change_trade_routes`).
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeExtraHappiness`.
    pub fn player_change_extra_happiness(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].extra_happiness += change;
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeBuildingHappiness`.
    pub fn player_change_building_happiness(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].building_happiness += change;
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeLargestCityHappiness`.
    pub fn player_change_largest_city_happiness(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].largest_city_happiness += change;
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeHappyPerMilitaryUnit`.
    pub fn player_change_happy_per_military_unit(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].happy_per_military_unit += change;
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeFeatureHappiness`.
    pub fn player_change_feature_happiness(&mut self, p: PlayerId, feature: FeatureType, change: i32) {
        if change != 0 {
            self.players[p].feature_happiness[feature] += change;
            self.player_update_feature_happiness(p);
        }
    }

    /// C++ `CvPlayer::isMilitaryFoodProduction`.
    pub fn player_is_military_food_production(&self, p: PlayerId) -> bool {
        self.players[p].military_food_production_count > 0
    }

    /// C++ `CvPlayer::isNoUnhealthyPopulation`.
    pub fn player_is_no_unhealthy_population(&self, p: PlayerId) -> bool {
        self.players[p].no_unhealthy_population_count > 0
    }

    /// C++ `CvPlayer::changeNoUnhealthyPopulationCount`.
    pub fn player_change_no_unhealthy_population_count(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].no_unhealthy_population_count += change;
            debug_assert!(self.players[p].no_unhealthy_population_count >= 0);
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::isBuildingOnlyHealthy`.
    pub fn player_is_building_only_healthy(&self, p: PlayerId) -> bool {
        self.players[p].building_only_healthy_count > 0
    }

    /// C++ `CvPlayer::changeBuildingOnlyHealthyCount`.
    pub fn player_change_building_only_healthy_count(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].building_only_healthy_count += change;
            debug_assert!(self.players[p].building_only_healthy_count >= 0);
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::getWarWearinessPercentAnger` (0 with GAMEOPTION_NO_WAR_WEARINESS).
    pub fn player_get_war_weariness_percent_anger(&self, p: PlayerId) -> i32 {
        //FfH: Added by Kael 11/17/2008
        if self.is_option(GameOptionType::GAMEOPTION_NO_WAR_WEARINESS) {
            return 0;
        }
        //FfH: End Add

        self.players[p].war_weariness_percent_anger
    }

    /// C++ `CvPlayer::updateWarWearinessPercentAnger`.
    pub fn player_update_war_weariness_percent_anger(&mut self, p: PlayerId) {
        let mut new_war_weariness_percent_anger = 0;

        if !self.player_is_barbarian(p) && !self.player_is_minor_civ(p) {
            let team = self.players[p].team;

            for t in self.civ_team_ids() {
                if self.team_is_alive(t) && !self.team_is_minor_civ(t) {
                    if self.teams[t].at_war[team] {
                        new_war_weariness_percent_anger +=
                            (self.teams[team].war_weariness[t] * 0.max(100 + self.teams[t].enemy_war_weariness_modifier)) / 10000;
                    }
                }
            }
        }

        new_war_weariness_percent_anger = self.player_get_modified_war_weariness_percent_anger(p, new_war_weariness_percent_anger);

        // C++ quirk: compares against the getter, which reads 0 under GAMEOPTION_NO_WAR_WEARINESS,
        // so with that option a new value of 0 never resets a stale member.
        if self.player_get_war_weariness_percent_anger(p) != new_war_weariness_percent_anger {
            self.players[p].war_weariness_percent_anger = new_war_weariness_percent_anger;

            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::getModifiedWarWearinessPercentAnger(iWarWearinessPercentAnger)`.
    pub fn player_get_modified_war_weariness_percent_anger(&self, p: PlayerId, war_weariness_percent_anger: i32) -> i32 {
        let d = self.d;
        let mut anger = war_weariness_percent_anger;

        anger *= d.defines.base_war_weariness_multiplier;

        if self.is_option(GameOptionType::GAMEOPTION_ALWAYS_WAR) || self.is_option(GameOptionType::GAMEOPTION_NO_CHANGING_WAR_PEACE) {
            anger *= 0.max(d.defines.forced_war_war_weariness_modifier + 100);
            anger /= 100;
        }

        if self.is_game_multi_player() {
            anger *= 0.max(d.defines.multiplayer_war_weariness_modifier + 100);
            anger /= 100;
        }

        anger *= 0.max(d.worlds[self.setup.world_size].war_weariness_modifier + 100);
        anger /= 100;

        if !self.players[p].human && !self.player_is_barbarian(p) && !self.player_is_minor_civ(p) {
            anger *= d.handicaps[self.handicap].ai_war_weariness_percent;
            anger /= 100;

            anger *= self.player_ai_per_era_percent(p);
            anger /= 100;
        }

        anger
    }

    /// C++ `CvPlayer::changeWarWearinessModifier`.
    pub fn player_change_war_weariness_modifier(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].war_weariness_modifier += change;
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    // ------------------------------------------------------------------ specialists

    /// C++ `CvPlayer::changeFreeSpecialist`.
    pub fn player_change_free_specialist(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].free_specialist += change;
            // C++ quirk: the C++ FAssert can fail for a player added to a team mid-game (bTeamShare
            // buildings, see `player_change_trade_routes`).
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::isSpecialistValid`.
    pub fn player_is_specialist_valid(&self, p: PlayerId, specialist: SpecialistType) -> bool {
        self.players[p].specialist_valid_count[specialist] > 0
    }

    /// C++ `CvPlayer::changeSpecialistValidCount`.
    pub fn player_change_specialist_valid_count(&mut self, p: PlayerId, specialist: SpecialistType, change: i32) {
        if change != 0 {
            self.players[p].specialist_valid_count[specialist] += change;
            debug_assert!(self.players[p].specialist_valid_count[specialist] >= 0);
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeSpecialistExtraYield`.
    pub fn player_change_specialist_extra_yield(&mut self, p: PlayerId, specialist: SpecialistType, yield_type: YieldType, change: i32) {
        if change != 0 {
            self.players[p].specialist_extra_yield[specialist][yield_type.idx()] += change;
            // C++ quirk: the C++ FAssert can fail for a player added to a team mid-game (bTeamShare
            // buildings, see `player_change_trade_routes`).
            self.player_update_extra_specialist_yield(p);
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::specialistYield(eSpecialist, eYield)`.
    pub fn player_specialist_yield(&self, p: PlayerId, specialist: SpecialistType, yield_type: YieldType) -> i32 {
        let yi = yield_type.idx();
        self.d.specialists[specialist].yield_change[yi] + self.players[p].specialist_extra_yield[specialist][yi]
    }

    // ------------------------------------------------------------------ growth

    /// C++ `CvPlayer::getGrowthThreshold(iPopulation)`: food needed to grow at `population`.
    pub fn player_get_growth_threshold(&self, p: PlayerId, population: i32) -> i32 {
        let d = self.d;

        let mut threshold = d.defines.base_city_growth_threshold + population * d.defines.city_growth_multiplier;

        threshold *= d.game_speeds[self.game_speed_type()].growth_percent;
        threshold /= 100;

        threshold *= d.eras[self.start_era()].growth_percent;
        threshold /= 100;

        if !self.players[p].human && !self.player_is_barbarian(p) {
            threshold *= d.handicaps[self.handicap].ai_growth_percent;
            threshold /= 100;

            threshold *= self.player_ai_per_era_percent(p);
            threshold /= 100;
        }

        1.max(threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Player;
    use crate::setup::GameSetup;
    use crate::unit::Unit;

    /// A game whose first `civs.len()` slots hold freshly reset players (C++ `reset`), alive
    /// on their own teams, with the given civilizations/leaders; other slots stay placeholders.
    fn game(civs: &[(&str, &str)]) -> Game {
        let d = ffh_data::global();
        let mut setup = GameSetup::standard(d, civs.len(), "WORLDSIZE_DUEL", 7);
        for (i, (civ, leader)) in civs.iter().enumerate() {
            setup.players[i].civ = Some(d.t::<CivilizationType>(civ));
            setup.players[i].leader = Some(d.t::<LeaderHeadType>(leader));
        }
        let mut g = Game::new(d, setup);
        let (n_players, n_teams) = (g.num_players(), g.num_teams());
        for i in 0..civs.len() {
            let p = PlayerId(i as u8);
            g.players[p] = Player::new(d, p, &g.setup.players[i], n_players, n_teams);
            g.players[p].alive = true;
            g.players[p].ever_alive = true;
            let t = g.players[p].team;
            g.teams[t].num_members = 1;
            g.teams[t].alive_count = 1;
            g.teams[t].ever_alive_count = 1;
        }
        g
    }

    fn bannor() -> Game {
        game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL")])
    }

    fn tech(g: &Game, name: &str) -> TechType {
        g.d.t::<TechType>(name)
    }

    #[test]
    fn growth_threshold_scales_with_speed_era_and_ai_difficulty() {
        let mut g = bannor();
        let d = g.d;
        let p = PlayerId(0);
        g.handicap = d.t::<HandicapType>("HANDICAP_DEITY");
        let speed = d.game_speeds[g.game_speed_type()].growth_percent;
        let era = d.eras[g.start_era()].growth_percent;
        let base = |pop: i32| ((d.defines.base_city_growth_threshold + pop * d.defines.city_growth_multiplier) * speed / 100) * era / 100;

        g.players[p].human = true;
        assert_eq!(g.player_get_growth_threshold(p, 3), base(3).max(1));

        g.players[p].human = false;
        g.players[p].current_era = EraType(2);
        let h = &d.handicaps[g.handicap];
        let ai = (base(3) * h.ai_growth_percent / 100) * (h.ai_per_era_modifier * 2 + 100).max(0) / 100;
        assert_eq!(g.player_get_growth_threshold(p, 3), ai.max(1));
        assert_ne!(ai, base(3), "Deity AIs grow faster");
    }

    #[test]
    fn inflation_rate_keeps_second_order_terms() {
        let mut g = bannor();
        let d = g.d;
        let p = PlayerId(0);
        g.players[p].human = true;
        let speed = &d.game_speeds[g.game_speed_type()];
        let per_turn = speed.inflation_percent * d.handicaps[g.setup.players[0].handicap.unwrap()].inflation_percent / 100;
        let expected = |turns: i32| turns * per_turn / 100 + turns * (turns - 1) * per_turn * per_turn / 2000000;

        // (game turn + elapsed turns) / 2 + offset <= 0: no inflation yet
        g.setup.game_turn = 0;
        g.elapsed_game_turns = 0;
        assert!(speed.inflation_offset <= 0);
        assert_eq!(g.player_calculate_inflation_rate(p), 0);

        g.setup.game_turn = 400;
        g.elapsed_game_turns = 300;
        let turns = 350 + speed.inflation_offset;
        assert!(turns > 0);
        assert_eq!(g.player_calculate_inflation_rate(p), expected(turns));

        // max turns cap the turn count
        g.setup.max_turns = 200;
        assert_eq!(g.player_calculate_inflation_rate(p), expected(200 + speed.inflation_offset));
        g.setup.max_turns = 0;

        // events' inflation modifier
        g.players[p].inflation_modifier = 50;
        let per_turn_mod = per_turn * 150 / 100;
        assert_eq!(
            g.player_calculate_inflation_rate(p),
            turns * per_turn_mod / 100 + turns * (turns - 1) * per_turn_mod * per_turn_mod / 2000000
        );

        // the 32-bit product wraps instead of panicking
        g.setup.game_turn = 20000;
        g.elapsed_game_turns = 20000;
        let _ = g.player_calculate_inflation_rate(p);

        g.setup.options[GameOptionType::GAMEOPTION_NO_INFLATION] = true;
        assert_eq!(g.player_calculate_inflation_rate(p), 0);
    }

    #[test]
    fn unit_supply_and_its_exemptions() {
        let mut g = bannor();
        let d = g.d;
        let p = PlayerId(0);
        g.players[p].human = true;
        g.players[p].num_outside_units = d.defines.initial_free_outside_units + 7;
        let base = 7 * d.defines.initial_outside_unit_gold_percent / 100;
        assert_eq!(
            g.player_calculate_unit_supply_breakdown(p),
            UnitSupplyBreakdown { paid_units: 7, base_supply_cost: base, supply: base }
        );
        assert_eq!(g.player_calculate_unit_supply(p), base);

        g.players[p].human = false;
        let h = &d.handicaps[g.handicap];
        assert_eq!(g.player_calculate_unit_supply(p), (base * h.ai_unit_supply_percent / 100) * (h.ai_per_era_modifier * 0 + 100) / 100);

        g.players[p].anarchy_turns = 1;
        assert_eq!(g.player_calculate_unit_supply(p), 0);
        g.players[p].anarchy_turns = 0;
        g.players[p].disable_production = 1;
        assert_eq!(g.player_calculate_unit_supply(p), 0);
        g.players[p].disable_production = 0;
        g.setup.options[GameOptionType::GAMEOPTION_NO_MAINTENANCE] = true;
        assert_eq!(g.player_calculate_unit_supply(p), 0);
    }

    #[test]
    fn unit_cost_counts_free_and_summoned_units() {
        let mut g = bannor();
        let d = g.d;
        let p = PlayerId(0);
        // 16 units, 3 of them summons (a duration: free in FfH)
        for i in 0..16 {
            let unit = Unit { duration: if i < 3 { 2 } else { 0 }, ..Unit::default() };
            let u = g.units.insert(unit);
            g.players[p].units.push(u);
        }
        let pl = &mut g.players[p];
        pl.human = true;
        pl.base_free_units = 1;
        pl.total_population = 9;
        pl.free_units_population_percent = 25; // 9 * 25 / 100 = 2
        pl.num_military_units = 5;
        pl.base_free_military_units = 2;
        pl.gold_per_unit = 2;
        pl.gold_per_military_unit = 3;
        pl.extra_unit_cost = 1;

        let h = &d.handicaps[g.setup.players[0].handicap.unwrap()];
        let free_units = h.free_units + 1 + 2 + 3;
        let paid_units = 0.max(16 - free_units);
        let support = (3 * 3 + paid_units * 2 + 1) * h.unit_cost_percent / 100;
        let b = g.player_calculate_unit_cost_breakdown(p);
        assert_eq!(
            b,
            UnitCostBreakdown {
                free_units,
                free_military_units: 2,
                paid_units,
                paid_military_units: 3,
                base_unit_cost: paid_units * 2,
                military_cost: 9,
                extra_cost: 1,
                cost: support,
            }
        );
        assert!(paid_units > 0);
        assert_eq!(g.player_calculate_unit_cost(p), support);

        g.players[p].anarchy_turns = 1;
        assert_eq!(g.player_calculate_unit_cost(p), 0);
        g.players[p].anarchy_turns = 0;
        g.players[p].disable_production = 3;
        assert_eq!(g.player_calculate_unit_cost(p), 0);
    }

    #[test]
    fn commerce_rate_is_x100_and_folds_espionage_into_culture() {
        let mut g = bannor();
        let p = PlayerId(0);
        g.players[p].commerce_rate = [199, 450, 250, 120];
        assert_eq!(g.player_get_commerce_rate(p, CommerceType::COMMERCE_GOLD), 1);
        assert_eq!(g.player_get_commerce_rate(p, CommerceType::COMMERCE_RESEARCH), 4);
        // GAMEOPTION_NO_ESPIONAGE is always on in FfH
        assert_eq!(g.player_get_commerce_rate(p, CommerceType::COMMERCE_CULTURE), 3);
        assert_eq!(g.player_get_commerce_rate(p, CommerceType::COMMERCE_ESPIONAGE), 0);

        // FfH: while research is disabled every commerce rate reads 0
        g.players[p].disable_research = 2;
        for i in 0..CommerceType::NUM {
            assert_eq!(g.player_get_commerce_rate(p, CommerceType::from_usize(i)), 0);
        }
    }

    #[test]
    fn commerce_flexibility() {
        let mut g = bannor();
        let p = PlayerId(0);
        let t = g.players[p].team;
        assert!(!g.player_is_commerce_flexible(p, CommerceType::COMMERCE_RESEARCH), "no city founded yet");

        g.players[p].founded_first_city = true;
        // CIV4CommerceInfo.xml: only research has bFlexiblePercent
        assert!(g.player_is_commerce_flexible(p, CommerceType::COMMERCE_RESEARCH));
        assert!(!g.player_is_commerce_flexible(p, CommerceType::COMMERCE_GOLD));
        assert!(!g.player_is_commerce_flexible(p, CommerceType::COMMERCE_CULTURE));

        g.players[p].commerce_flexible_count[CommerceType::COMMERCE_CULTURE.idx()] = 1;
        assert!(g.player_is_commerce_flexible(p, CommerceType::COMMERCE_CULTURE));
        g.teams[t].commerce_flexible_count[CommerceType::COMMERCE_GOLD.idx()] = 1;
        assert!(g.player_is_commerce_flexible(p, CommerceType::COMMERCE_GOLD));

        // espionage: no civ met, and GAMEOPTION_NO_ESPIONAGE
        g.players[p].commerce_flexible_count[CommerceType::COMMERCE_ESPIONAGE.idx()] = 1;
        assert!(!g.player_is_commerce_flexible(p, CommerceType::COMMERCE_ESPIONAGE));
    }

    #[test]
    fn can_research_follows_prerequisites_trade_and_religion() {
        let mut g = bannor();
        let d = g.d;
        let p = PlayerId(0);
        let t = g.players[p].team;
        let crafting = tech(&g, "TECH_CRAFTING");
        let mining = tech(&g, "TECH_MINING");

        // no city and not in advanced start
        assert!(!g.player_can_research(p, crafting, false));
        g.players[p].advanced_start_points = 0;
        assert!(g.player_can_research(p, crafting, false));
        g.players[p].advanced_start_points = -1;

        g.players[p].founded_first_city = true;
        assert!(g.player_can_research(p, crafting, false));
        assert!(!g.player_can_research(p, mining, false), "OR prerequisite Crafting unknown");

        g.teams[t].has_tech[crafting] = true;
        assert!(!g.player_can_research(p, crafting, false), "already known");
        assert!(g.player_can_research(p, mining, false));

        // traded techs cannot be used as prerequisites for trading unless brokering is off
        g.teams[t].no_trade_tech[crafting] = true;
        assert!(!g.player_can_research(p, mining, true));
        assert!(g.player_can_research(p, mining, false));
        g.setup.options[GameOptionType::GAMEOPTION_NO_TECH_BROKERING] = true;
        assert!(g.player_can_research(p, mining, true));

        assert!(!g.player_can_ever_research(p, tech(&g, "TECH_NEVER")), "bDisable");

        // FfH: religion techs need the state religion
        let hidden_paths = tech(&g, "TECH_HIDDEN_PATHS");
        assert!(!g.player_can_ever_research(p, hidden_paths));
        g.players[p].last_state_religion = Some(d.t::<ReligionType>("RELIGION_FELLOWSHIP_OF_LEAVES"));
        g.players[p].state_religion_count = 1;
        assert!(g.player_can_ever_research(p, hidden_paths));

        // CvGameUtils.cannotResearch: Seafaring is for the Lanun only
        assert!(!g.player_can_ever_research(p, tech(&g, "TECH_SEAFARING")));
    }

    #[test]
    fn find_path_length_counts_missing_techs() {
        let mut g = bannor();
        let p = PlayerId(0);
        let t = g.players[p].team;
        let mining = tech(&g, "TECH_MINING");
        let archery = tech(&g, "TECH_ARCHERY"); // OR Hunting, Mining
        let earthmother = tech(&g, "TECH_WAY_OF_THE_EARTHMOTHER"); // AND Mining, Mysticism

        assert_eq!(g.player_find_path_length(p, mining, false), 2); // Crafting, Mining
        assert_eq!(g.player_find_path_length(p, archery, false), 3); // Exploration, Hunting, Archery
        assert_eq!(g.player_find_path_length(p, earthmother, false), 5);

        // known or queued techs count 0
        let (crafting, ancient_chants) = (tech(&g, "TECH_CRAFTING"), tech(&g, "TECH_ANCIENT_CHANTS"));
        g.teams[t].has_tech[crafting] = true;
        g.players[p].researching_tech[ancient_chants] = true;
        assert_eq!(g.player_find_path_length(p, earthmother, false), 3); // Mining, Mysticism, Earthmother
        g.players[p].researching_tech[earthmother] = true;
        assert_eq!(g.player_find_path_length(p, earthmother, false), 0);
    }

    #[test]
    fn push_research_queues_missing_prerequisites() {
        let mut g = bannor();
        let p = PlayerId(0);
        let t = g.players[p].team;
        g.players[p].founded_first_city = true;
        let crafting = tech(&g, "TECH_CRAFTING");
        let mining = tech(&g, "TECH_MINING");
        let mysticism = tech(&g, "TECH_MYSTICISM");
        let earthmother = tech(&g, "TECH_WAY_OF_THE_EARTHMOTHER");
        let agriculture = tech(&g, "TECH_AGRICULTURE");
        let husbandry = tech(&g, "TECH_ANIMAL_HUSBANDRY");
        for known in [crafting, tech(&g, "TECH_ANCIENT_CHANTS"), agriculture] {
            g.teams[t].has_tech[known] = true;
        }

        assert!(g.player_push_research(p, earthmother, false));
        assert_eq!(g.players[p].research_queue, vec![mining, mysticism, earthmother]);
        assert!(g.players[p].researching_tech[mining] && g.players[p].researching_tech[earthmother]);
        assert_eq!(g.player_get_current_research(p), Some(mining));
        assert_eq!(g.player_get_queue_position(p, mysticism), 2);

        // known or already queued techs change nothing (not even with bClear)
        assert!(g.player_push_research(p, crafting, false));
        assert!(g.player_push_research(p, mysticism, true));
        assert_eq!(g.players[p].research_queue.len(), 3);

        // bClear replaces the queue
        assert!(g.player_push_research(p, husbandry, true));
        assert_eq!(g.players[p].research_queue, vec![husbandry]);
        assert!(!g.players[p].researching_tech[mining]);

        // a tech the player can never research is refused
        assert!(!g.player_push_research(p, tech(&g, "TECH_HIDDEN_PATHS"), false));
        assert_eq!(g.players[p].research_queue, vec![husbandry]);

        g.player_pop_research(p, husbandry);
        assert_eq!(g.player_get_current_research(p), None);
        assert!(!g.players[p].researching_tech[husbandry]);
    }

    #[test]
    fn research_modifier_counts_met_teams_and_known_prerequisites() {
        let mut g = game(&[("CIVILIZATION_BANNOR", "LEADER_SABATHIEL"), ("CIVILIZATION_KHAZAD", "LEADER_ARTURUS")]);
        let d = g.d;
        let p = PlayerId(0);
        let (t0, t1) = (g.players[p].team, g.players[PlayerId(1)].team);
        let mining = tech(&g, "TECH_MINING");
        let known_prereq = d.defines.tech_cost_known_prereq_modifier;

        assert_eq!(g.player_calculate_research_modifier(p, None), 100);
        assert_eq!(g.player_calculate_research_modifier(p, Some(mining)), 100);

        let crafting = tech(&g, "TECH_CRAFTING");
        g.teams[t0].has_tech[crafting] = true;
        assert_eq!(g.player_calculate_research_modifier(p, Some(mining)), 100 + known_prereq);

        // a team that knows the tech counts only once met (2 alive civ teams)
        g.teams[t1].has_tech[mining] = true;
        assert_eq!(g.player_calculate_research_modifier(p, Some(mining)), 100 + known_prereq);
        g.teams[t0].has_met[t1] = true;
        assert_eq!(
            g.player_calculate_research_modifier(p, Some(mining)),
            100 + d.defines.tech_cost_total_known_team_modifier / 2 + known_prereq
        );
    }

    #[test]
    fn modified_war_weariness() {
        let mut g = bannor();
        let d = g.d;
        let p = PlayerId(0);
        let world = (d.worlds[g.setup.world_size].war_weariness_modifier + 100).max(0);
        let base = 7 * d.defines.base_war_weariness_multiplier;

        g.players[p].human = true;
        assert_eq!(g.player_get_modified_war_weariness_percent_anger(p, 7), base * world / 100);

        g.setup.options[GameOptionType::GAMEOPTION_ALWAYS_WAR] = true;
        let forced = base * (d.defines.forced_war_war_weariness_modifier + 100).max(0) / 100;
        assert_eq!(g.player_get_modified_war_weariness_percent_anger(p, 7), forced * world / 100);
        g.setup.options[GameOptionType::GAMEOPTION_ALWAYS_WAR] = false;

        g.players[p].human = false;
        let h = &d.handicaps[g.handicap];
        assert_eq!(
            g.player_get_modified_war_weariness_percent_anger(p, 7),
            ((base * world / 100) * h.ai_war_weariness_percent / 100) * (h.ai_per_era_modifier * 0 + 100) / 100
        );

        g.players[p].war_weariness_percent_anger = 30;
        assert_eq!(g.player_get_war_weariness_percent_anger(p), 30);
        g.setup.options[GameOptionType::GAMEOPTION_NO_WAR_WEARINESS] = true;
        assert_eq!(g.player_get_war_weariness_percent_anger(p), 0);
    }

    /// Picks the same tech for every research and free-tech choice.
    struct PickTech(TechType);

    impl crate::controller::Controller for PickTech {
        fn choose_research(&self, _g: &Game, _p: PlayerId) -> Option<TechType> {
            Some(self.0)
        }
    }

    #[test]
    fn choose_tech_is_a_human_only_popup() {
        let mut g = bannor();
        let p = PlayerId(0);
        let t = g.players[p].team;
        g.players[p].founded_first_city = true;
        let crafting = tech(&g, "TECH_CRAFTING");
        g.controllers[p] = std::sync::Arc::new(PickTech(crafting));

        // CvPlayer::addPopup deletes an AI player's popup: no research, no free tech
        g.players[p].human = false;
        g.player_choose_tech(p, 0);
        assert_eq!(g.player_get_current_research(p), None);
        g.player_choose_tech(p, 1);
        assert!(!g.teams[t].has_tech[crafting]);

        // a human picks the next research
        g.players[p].human = true;
        g.player_choose_tech(p, 0);
        assert_eq!(g.players[p].research_queue, vec![crafting]);
    }

    #[test]
    fn specialists_repeat_research_and_tech_free_units() {
        let mut g = bannor();
        let d = g.d;
        let p = PlayerId(0);

        let s = SpecialistType(0);
        let other = SpecialistType(1);
        let food = YieldType::YIELD_FOOD;
        let gold = CommerceType::COMMERCE_GOLD;
        g.players[p].specialist_extra_yield[s][food.idx()] = 2;
        g.players[p].specialist_extra_commerce[gold.idx()] = 1;
        g.players[p].specialist_type_extra_commerce[s][gold.idx()] = 3;
        assert_eq!(g.player_specialist_yield(p, s, food), d.specialists[s].yield_change[food.idx()] + 2);
        assert_eq!(g.player_specialist_commerce(p, s, gold), d.specialists[s].commerce_change[gold.idx()] + 4);
        assert_eq!(g.player_specialist_commerce(p, other, gold), d.specialists[other].commerce_change[gold.idx()] + 1);

        assert!(!g.player_is_current_research_repeat(p));
        g.players[p].research_queue = vec![tech(&g, "TECH_FUTURE_TECH")];
        assert!(g.player_is_current_research_repeat(p));

        let civ = d.t::<CivilizationType>("CIVILIZATION_BANNOR");
        let arcane_lore = tech(&g, "TECH_ARCANE_LORE"); // FirstFreeUnitClass UNITCLASS_SCIENTIST
        let free_unit = g.player_get_tech_free_unit(p, arcane_lore);
        assert!(free_unit.is_some());
        assert_eq!(free_unit, d.civ_unit(civ, d.t::<UnitClassType>("UNITCLASS_SCIENTIST")));
        assert_eq!(g.player_get_tech_free_unit(p, tech(&g, "TECH_AGRICULTURE")), None);
    }
}
