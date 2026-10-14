//! CvPlayer: economy -- gold (unit cost/supply, maintenance, civic upkeep is in
//! `civics_religion.rs`, inflation), commerce percents and rates, research (rate, queue,
//! per-turn progress), golden ages, yields, happiness/health modifiers, specialists, and
//! the "update every city" helpers.

#![allow(unused_imports)]
#![allow(unused_variables)]

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
    // ------------------------------------------------------------------ "update every city" helpers

    /// C++ `CvPlayer::updateYield`.
    pub fn player_update_yield(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateYield")
    }

    /// C++ `CvPlayer::updateMaintenance`.
    pub fn player_update_maintenance(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateMaintenance")
    }

    /// C++ `CvPlayer::updatePowerHealth`.
    pub fn player_update_power_health(&mut self, p: PlayerId) {
        todo!("CvPlayer::updatePowerHealth")
    }

    /// C++ `CvPlayer::updateExtraBuildingHappiness`.
    pub fn player_update_extra_building_happiness(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateExtraBuildingHappiness")
    }

    /// C++ `CvPlayer::updateExtraBuildingHealth`.
    pub fn player_update_extra_building_health(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateExtraBuildingHealth")
    }

    /// C++ `CvPlayer::updateFeatureHappiness`.
    pub fn player_update_feature_happiness(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateFeatureHappiness")
    }

    /// C++ `CvPlayer::updateReligionHappiness`.
    pub fn player_update_religion_happiness(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateReligionHappiness")
    }

    /// C++ `CvPlayer::updateExtraSpecialistYield`.
    pub fn player_update_extra_specialist_yield(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateExtraSpecialistYield")
    }

    /// C++ `CvPlayer::updateCommerce(eCommerce)`.
    pub fn player_update_commerce_type(&mut self, p: PlayerId, commerce: CommerceType) {
        todo!("CvPlayer::updateCommerce")
    }

    /// C++ `CvPlayer::updateCommerce()`.
    pub fn player_update_commerce(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateCommerce")
    }

    /// C++ `CvPlayer::updateBuildingCommerce`.
    pub fn player_update_building_commerce(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateBuildingCommerce")
    }

    /// C++ `CvPlayer::updateReligionCommerce`.
    pub fn player_update_religion_commerce(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateReligionCommerce")
    }

    /// C++ `CvPlayer::updateCorporation`.
    pub fn player_update_corporation(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateCorporation")
    }

    /// C++ `CvPlayer::updateCityPlotYield`.
    pub fn player_update_city_plot_yield(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateCityPlotYield")
    }

    /// C++ `CvPlayer::updateCitySight(bIncrement, bUpdatePlotGroups)`.
    pub fn player_update_city_sight(&mut self, p: PlayerId, increment: bool, update_plot_groups: bool) {
        todo!("CvPlayer::updateCitySight")
    }

    /// C++ `CvPlayer::updateTradeRoutes`: clears all routes, then updates cities in order of
    /// decreasing total trade modifier.
    pub fn player_update_trade_routes(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateTradeRoutes")
    }

    /// C++ `CvPlayer::invalidatePopulationRankCache`.
    pub fn player_invalidate_population_rank_cache(&mut self, p: PlayerId) {
        todo!("CvPlayer::invalidatePopulationRankCache")
    }

    /// C++ `CvPlayer::invalidateYieldRankCache(eYield = NO_YIELD)` (every yield is invalidated).
    pub fn player_invalidate_yield_rank_cache(&mut self, p: PlayerId, yield_type: Option<YieldType>) {
        todo!("CvPlayer::invalidateYieldRankCache")
    }

    /// C++ `CvPlayer::invalidateCommerceRankCache(eCommerce = NO_COMMERCE)` (every commerce is invalidated).
    pub fn player_invalidate_commerce_rank_cache(&mut self, p: PlayerId, commerce: Option<CommerceType>) {
        todo!("CvPlayer::invalidateCommerceRankCache")
    }

    // ------------------------------------------------------------------ totals over cities

    /// C++ `CvPlayer::calculateTotalYield(eYield)`.
    pub fn player_calculate_total_yield(&self, p: PlayerId, yield_type: YieldType) -> i32 {
        todo!("CvPlayer::calculateTotalYield")
    }

    /// C++ `CvPlayer::calculateTotalExports(eYield)`.
    pub fn player_calculate_total_exports(&self, p: PlayerId, yield_type: YieldType) -> i32 {
        todo!("CvPlayer::calculateTotalExports")
    }

    /// C++ `CvPlayer::calculateTotalImports(eYield)`.
    pub fn player_calculate_total_imports(&self, p: PlayerId, yield_type: YieldType) -> i32 {
        todo!("CvPlayer::calculateTotalImports")
    }

    /// C++ `CvPlayer::calculateTotalCityHappiness`.
    pub fn player_calculate_total_city_happiness(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::calculateTotalCityHappiness")
    }

    /// C++ `CvPlayer::calculateTotalCityUnhappiness`.
    pub fn player_calculate_total_city_unhappiness(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::calculateTotalCityUnhappiness")
    }

    /// C++ `CvPlayer::calculateTotalCityHealthiness`.
    pub fn player_calculate_total_city_healthiness(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::calculateTotalCityHealthiness")
    }

    /// C++ `CvPlayer::calculateTotalCityUnhealthiness`.
    pub fn player_calculate_total_city_unhealthiness(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::calculateTotalCityUnhealthiness")
    }

    // ------------------------------------------------------------------ gold: costs and income

    /// C++ `CvPlayer::calculateUnitCost(int& iFreeUnits, ...)`: unit support cost with its
    /// breakdown (FFH: units with a duration are free).
    pub fn player_calculate_unit_cost_breakdown(&self, p: PlayerId) -> UnitCostBreakdown {
        todo!("CvPlayer::calculateUnitCost")
    }

    /// C++ `CvPlayer::calculateUnitCost()` (0 in anarchy, with FFH disabled production or
    /// GAMEOPTION_NO_MAINTENANCE).
    pub fn player_calculate_unit_cost(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::calculateUnitCost")
    }

    /// C++ `CvPlayer::calculateUnitSupply()` (0 in anarchy, with FFH disabled production or
    /// GAMEOPTION_NO_MAINTENANCE).
    pub fn player_calculate_unit_supply(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::calculateUnitSupply")
    }

    /// C++ `CvPlayer::calculateUnitSupply(int& iPaidUnits, int& iBaseSupplyCost)`.
    pub fn player_calculate_unit_supply_breakdown(&self, p: PlayerId) -> UnitSupplyBreakdown {
        todo!("CvPlayer::calculateUnitSupply")
    }

    /// C++ `CvPlayer::calculatePreInflatedCosts` (units + supply + maintenance + civic upkeep
    /// + Python `getExtraCost`).
    pub fn player_calculate_pre_inflated_costs(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::calculatePreInflatedCosts")
    }

    /// C++ `CvPlayer::calculateInflationRate`.
    pub fn player_calculate_inflation_rate(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::calculateInflationRate")
    }

    /// C++ `CvPlayer::calculateInflatedCosts`.
    pub fn player_calculate_inflated_costs(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::calculateInflatedCosts")
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
        todo!("CvPlayer::doGold")
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
        todo!("CvPlayer::getCommerceRate")
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
        todo!("CvPlayer::changeCapitalCommerceRateModifier")
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
            debug_assert!(self.players[p].state_religion_building_commerce[commerce.idx()] >= 0);
            self.player_update_commerce_type(p, commerce);
        }
    }

    /// C++ `CvPlayer::changeSpecialistExtraCommerce`.
    pub fn player_change_specialist_extra_commerce(&mut self, p: PlayerId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.players[p].specialist_extra_commerce[commerce.idx()] += change;
            debug_assert!(self.players[p].specialist_extra_commerce[commerce.idx()] >= 0);
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
        todo!("CvPlayer::isCommerceFlexible")
    }

    /// C++ `CvPlayer::changeCommerceFlexibleCount`.
    pub fn player_change_commerce_flexible_count(&mut self, p: PlayerId, commerce: CommerceType, change: i32) {
        if change != 0 {
            self.players[p].commerce_flexible_count[commerce.idx()] += change;
            debug_assert!(self.players[p].commerce_flexible_count[commerce.idx()] >= 0);

            if !self.player_is_commerce_flexible(p, commerce) {
                self.player_set_commerce_percent(p, commerce, 0);
            }
        }
    }

    /// C++ `CvPlayer::specialistCommerce(eSpecialist, eCommerce)` (FFH adds the
    /// per-specialist-type extra commerce).
    pub fn player_specialist_commerce(&self, p: PlayerId, specialist: SpecialistType, commerce: CommerceType) -> i32 {
        todo!("CvPlayer::specialistCommerce")
    }

    // ------------------------------------------------------------------ research

    /// C++ `CvPlayer::chooseTech(iDiscover, szText, bFront)`: in C++ a tech-chooser popup
    /// for humans (`iDiscover` free techs); route the choice through the controller.
    pub fn player_choose_tech(&mut self, p: PlayerId, discover: i32) {
        todo!("CvPlayer::chooseTech")
    }

    /// C++ `CvPlayer::calculateResearchModifier(eTech)` (100 for NO_TECH).
    pub fn player_calculate_research_modifier(&self, p: PlayerId, tech: Option<TechType>) -> i32 {
        todo!("CvPlayer::calculateResearchModifier")
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
        todo!("CvPlayer::isResearch")
    }

    /// C++ `CvPlayer::canEverResearch(eTech)`.
    pub fn player_can_ever_research(&self, p: PlayerId, tech: TechType) -> bool {
        todo!("CvPlayer::canEverResearch")
    }

    /// C++ `CvPlayer::canResearch(eTech, bTrade = false)`.
    pub fn player_can_research(&self, p: PlayerId, tech: TechType, trade: bool) -> bool {
        todo!("CvPlayer::canResearch")
    }

    /// C++ `CvPlayer::getCurrentResearch` (head of the research queue).
    pub fn player_get_current_research(&self, p: PlayerId) -> Option<TechType> {
        self.players[p].research_queue.first().copied()
    }

    /// C++ `CvPlayer::isCurrentResearchRepeat`.
    pub fn player_is_current_research_repeat(&self, p: PlayerId) -> bool {
        todo!("CvPlayer::isCurrentResearchRepeat")
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
        todo!("CvPlayer::getResearchTurnsLeftTimes100")
    }

    /// C++ `CvPlayer::findPathLength(eTech, bCost = true)`.
    pub fn player_find_path_length(&self, p: PlayerId, tech: TechType, cost: bool) -> i32 {
        todo!("CvPlayer::findPathLength")
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
        todo!("CvPlayer::pushResearch")
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
        todo!("CvPlayer::doResearch")
    }

    /// C++ `CvPlayer::getTechFreeUnit(eTech)`.
    pub fn player_get_tech_free_unit(&self, p: PlayerId, tech: TechType) -> Option<UnitType> {
        todo!("CvPlayer::getTechFreeUnit")
    }

    /// C++ `CvPlayer::isHasTech(iTech)` (FFH shortcut for the team's `isHasTech`).
    pub fn player_is_has_tech(&self, p: PlayerId, tech: TechType) -> bool {
        todo!("CvPlayer::isHasTech")
    }

    // ------------------------------------------------------------------ golden age

    /// C++ `CvPlayer::isGoldenAge`.
    pub fn player_is_golden_age(&self, p: PlayerId) -> bool {
        self.players[p].golden_age_turns > 0
    }

    /// C++ `CvPlayer::changeGoldenAgeTurns`: ends anarchy when a golden age starts, updates
    /// yields, Python golden age events.
    pub fn player_change_golden_age_turns(&mut self, p: PlayerId, change: i32) {
        todo!("CvPlayer::changeGoldenAgeTurns")
    }

    /// C++ `CvPlayer::getGoldenAgeLength`.
    pub fn player_get_golden_age_length(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::getGoldenAgeLength")
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
        todo!("CvPlayer::changeCapitalYieldRateModifier")
    }

    /// C++ `CvPlayer::updateExtraYieldThreshold(eIndex)` (best trait threshold).
    pub fn player_update_extra_yield_threshold(&mut self, p: PlayerId, yield_type: YieldType) {
        todo!("CvPlayer::updateExtraYieldThreshold")
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
            debug_assert!(self.players[p].trade_routes >= 0);
            self.player_update_trade_routes(p);
        }
    }

    /// C++ `CvPlayer::changeCoastalTradeRoutes`.
    pub fn player_change_coastal_trade_routes(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].coastal_trade_routes += change;
            debug_assert!(self.players[p].coastal_trade_routes >= 0);
            self.player_update_trade_routes(p);
        }
    }

    /// C++ `CvPlayer::isNoForeignTrade`.
    pub fn player_is_no_foreign_trade(&self, p: PlayerId) -> bool {
        self.players[p].no_foreign_trade_count > 0
    }

    /// C++ `CvPlayer::changeNoForeignTradeCount` (updates the trade routes of the whole game).
    pub fn player_change_no_foreign_trade_count(&mut self, p: PlayerId, change: i32) {
        todo!("CvPlayer::changeNoForeignTradeCount")
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
            debug_assert!(self.players[p].building_good_health >= 0);
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::changeBuildingBadHealth`.
    pub fn player_change_building_bad_health(&mut self, p: PlayerId, change: i32) {
        if change != 0 {
            self.players[p].building_bad_health += change;
            debug_assert!(self.players[p].building_bad_health <= 0);
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
        todo!("CvPlayer::getWarWearinessPercentAnger")
    }

    /// C++ `CvPlayer::updateWarWearinessPercentAnger`.
    pub fn player_update_war_weariness_percent_anger(&mut self, p: PlayerId) {
        todo!("CvPlayer::updateWarWearinessPercentAnger")
    }

    /// C++ `CvPlayer::getModifiedWarWearinessPercentAnger(iWarWearinessPercentAnger)`.
    pub fn player_get_modified_war_weariness_percent_anger(&self, p: PlayerId, war_weariness_percent_anger: i32) -> i32 {
        todo!("CvPlayer::getModifiedWarWearinessPercentAnger")
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
            debug_assert!(self.players[p].free_specialist >= 0);
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
            debug_assert!(self.players[p].specialist_extra_yield[specialist][yield_type.idx()] >= 0);
            self.player_update_extra_specialist_yield(p);
            self.player_ai_make_assign_work_dirty(p);
        }
    }

    /// C++ `CvPlayer::specialistYield(eSpecialist, eYield)`.
    pub fn player_specialist_yield(&self, p: PlayerId, specialist: SpecialistType, yield_type: YieldType) -> i32 {
        todo!("CvPlayer::specialistYield")
    }

    // ------------------------------------------------------------------ growth

    /// C++ `CvPlayer::getGrowthThreshold(iPopulation)`: food needed to grow at `population`.
    pub fn player_get_growth_threshold(&self, p: PlayerId, population: i32) -> i32 {
        todo!("CvPlayer::getGrowthThreshold")
    }
}
