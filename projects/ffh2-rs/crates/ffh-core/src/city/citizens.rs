//! CvCity: city plots and citizens (worked plots, specialists), population and growth,
//! happiness, health, great people, FFH2 plot radius, and the player-settable emphasize /
//! growth-control state of CvCityAI (`AI_setEmphasize`, `AI_stopGrowth`, ...).
//!
//! Citizen *assignment* (`AI_assignWorkingPlots`) is the governor (`crate::governor`). The
//! player's citizen buttons (`alterSpecialistCount`, `alterWorkingPlot`) also call
//! `AI_addBestCitizen` / `AI_removeWorstCitizen`; their stand-ins
//! (`city_ai_add_best_citizen`, `city_ai_remove_worst_citizen`) live in `governor.rs`, keep
//! the C++ structure of those two functions and value the candidates the way the governor does.

use super::range;
use crate::defs::*;
use crate::game::Game;
use crate::governor::Weights;
use crate::ids::*;
use crate::utils::get_population_power;
use ffh_data::*;

impl Game {
    /// C++ `CvCity::createGreatPeople` (CvCity.cpp:1246-1249): forwards to
    /// `CvPlayer::createGreatPeople` at the city's plot.
    pub fn city_create_great_people(&mut self, c: CityId, unit: UnitType, increment_threshold: bool, increment_experience: bool) {
        let city = &self.cities[c];
        let (owner, x, y) = (city.owner, city.x, city.y);
        self.player_create_great_people(owner, unit, increment_threshold, increment_experience, x, y);
    }

    /// C++ `CvCity::getCityPlotIndex` (CvCity.cpp:1365-1368, `plotCityXY`): index of `plot`
    /// in the city's 37-plot radius, C++ -1 = `None`.
    pub fn city_plot_index(&self, c: CityId, plot: PlotIdx) -> Option<usize> {
        self.plot_city_xy_for(c, plot)
    }

    /// C++ `CvCity::getCityIndexPlot` (CvCity.cpp:1371-1374, `plotCity`): the plot at city
    /// plot index `index` (`None` off the map).
    pub fn city_index_plot(&self, c: CityId, index: usize) -> Option<PlotIdx> {
        let city = &self.cities[c];
        self.plot_city(city.x, city.y, index)
    }

    /// C++ `CvCity::canWork` (CvCity.cpp:1377-1431).
    pub fn city_can_work(&self, c: CityId, plot: PlotIdx) -> bool {
        // C++ `getWorkingCity()` resolves the stored id (NULL for a destroyed city); `c` is alive,
        // so comparing the stored id is the same test.
        if self.map.plots[plot].working_city != Some(c) {
            return false;
        }

        let owner = self.cities[c].owner;
        if self.plot_plot_check(plot, Game::puf_can_siege, owner.i(), -1, None, None, None, -1, -1).is_some() {
            return false;
        }

        if self.plot_is_water(plot) {
            let team = self.city_team(c);
            if !self.team_is_water_work(team) {
                return false;
            }

            if self.map.plots[plot].blockaded_count[team] > 0 {
                return false;
            }
            // (BTS: the adjacent-siege test below this point is commented out, replaced by the
            // blockade count above.)
        }

        if !self.plot_has_yield(plot) {
            return false;
        }

        true
    }

    /// C++ `CvCity::verifyWorkingPlot` (CvCity.cpp:1434-1455).
    pub fn city_verify_working_plot(&mut self, c: CityId, index: usize) {
        if self.cities[c].working_plot[index] {
            if let Some(plot) = self.city_index_plot(c, index) {
                if !self.city_can_work(c, plot) {
                    self.city_set_working_plot(c, index, false);

                    self.cities[c].ai_assign_work_dirty = true;
                }
            }
        }
    }

    /// C++ `CvCity::verifyWorkingPlots` (CvCity.cpp:1458-1466).
    pub fn city_verify_working_plots(&mut self, c: CityId) {
        for i in 0..NUM_CITY_PLOTS {
            self.city_verify_working_plot(c, i);
        }
    }

    /// C++ `CvCity::clearWorkingOverride` (CvCity.cpp:1469-1479).
    pub fn city_clear_working_override(&mut self, c: CityId, index: usize) {
        if let Some(plot) = self.city_index_plot(c, index) {
            self.plot_set_working_city_override(plot, None);
        }
    }

    /// C++ `CvCity::countNumImprovedPlots` (CvCity.cpp:1482-1515). C++ defaults
    /// `eImprovement = NO_IMPROVEMENT` (any improvement), `bPotential = false`.
    pub fn city_count_num_improved_plots(&self, c: CityId, improvement: Option<ImprovementType>, potential: bool) -> i32 {
        let mut count = 0;

        for i in 0..NUM_CITY_PLOTS {
            let Some(loop_plot) = self.city_index_plot(c, i) else {
                continue;
            };
            if self.map.plots[loop_plot].working_city != Some(c) {
                continue;
            }
            match improvement {
                Some(imp) => {
                    if self.map.plots[loop_plot].improvement_type == Some(imp)
                        || (potential && self.plot_can_have_improvement(loop_plot, imp, Some(self.city_team(c)), false))
                    {
                        count += 1;
                    }
                }
                None => {
                    if self.map.plots[loop_plot].improvement_type.is_some() {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// C++ `CvCity::countNumWaterPlots` (CvCity.cpp:1518-1543).
    pub fn city_count_num_water_plots(&self, c: CityId) -> i32 {
        let mut count = 0;

        for i in 0..NUM_CITY_PLOTS {
            if let Some(loop_plot) = self.city_index_plot(c, i) {
                if self.map.plots[loop_plot].working_city == Some(c) && self.plot_is_water(loop_plot) {
                    count += 1;
                }
            }
        }

        count
    }

    /// C++ `CvCity::countNumRiverPlots` (CvCity.cpp:1545-1566).
    pub fn city_count_num_river_plots(&self, c: CityId) -> i32 {
        let mut count = 0;

        for i in 0..NUM_CITY_PLOTS {
            if let Some(loop_plot) = self.city_index_plot(c, i) {
                if self.map.plots[loop_plot].working_city == Some(c) && self.plot_is_river(loop_plot) {
                    count += 1;
                }
            }
        }

        count
    }

    /// C++ `CvCity::canJoin` (CvCity.cpp:2422-2425).
    pub fn city_can_join(&self, _c: CityId) -> bool {
        true
    }

    /// C++ `CvCity::getFoodTurnsLeft` (CvCity.cpp:2428-2448).
    pub fn city_food_turns_left(&self, c: CityId) -> i32 {
        let food_left = self.city_growth_threshold(c) - self.cities[c].food;
        let food_difference = self.city_food_difference(c, true);
        if food_difference <= 0 {
            return food_left;
        }
        let mut turns_left = food_left / food_difference;
        if turns_left * food_difference < food_left {
            turns_left += 1;
        }
        turns_left.max(1)
    }

    /// C++ `CvCity::processSpecialist` (CvCity.cpp:4220-4250).
    pub fn city_process_specialist(&mut self, c: CityId, specialist: SpecialistType, change: i32) {
        let d = self.d;
        let info = &d.specialists[specialist];

        if let Some(gp_class) = info.great_people_unit_class {
            let civ = self.city_civilization_type(c);
            let gp_unit = d.civilizations[civ].civilization_units(gp_class, d.unit_classes[gp_class].default_unit_index);

            if let Some(gp_unit) = gp_unit {
                self.city_change_great_people_unit_rate(c, gp_unit, info.great_people_rate_change * change);
            }
        }

        // changeBaseGreatPeopleRate
        self.cities[c].base_great_people_rate += info.great_people_rate_change * change;

        for i in 0..YieldType::NUM {
            self.city_change_base_yield_rate(c, YieldType::from_usize(i), info.yield_change[i] * change);
        }

        for i in 0..CommerceType::NUM {
            self.city_change_specialist_commerce(c, CommerceType::from_usize(i), info.commerce_change[i] * change);
        }

        self.city_update_extra_specialist_yields(c);

        // changeSpecialistFreeExperience
        self.cities[c].specialist_free_experience += info.experience * change;
    }

    /// C++ `CvCity::getOvercrowdingPercentAnger` (CvCity.cpp:4414-4429). C++ default
    /// `iExtra = 0`.
    pub fn city_overcrowding_percent_anger(&self, c: CityId, extra: i32) -> i32 {
        let population = self.cities[c].population;
        let mut anger = 0;

        let overcrowding = population + extra;

        if overcrowding > 0 {
            anger += ((overcrowding * self.d.defines.percent_anger_divisor) / (population + extra).max(1)) + 1;
        }

        anger
    }

    /// C++ `CvCity::getNoMilitaryPercentAnger` (CvCity.cpp:4432-4444).
    pub fn city_no_military_percent_anger(&self, c: CityId) -> i32 {
        let mut anger = 0;

        if self.cities[c].military_happiness_units == 0 {
            anger += self.d.defines.no_military_percent_anger;
        }

        anger
    }

    /// C++ `CvCity::getCulturePercentAnger` (CvCity.cpp:4447-4486).
    pub fn city_culture_percent_anger(&self, c: CityId) -> i32 {
        let defines = &self.d.defines;
        let plot = self.city_plot(c);

        let total_culture = self.plot_count_total_culture(plot);

        if total_culture == 0 {
            return 0;
        }

        let team = self.city_team(c);
        let mut angry_culture: i32 = 0;

        for p in self.player_ids() {
            if !self.players[p].alive {
                continue;
            }
            let loop_team = self.players[p].team;
            if loop_team == team {
                continue;
            }
            let mut culture = self.map.plots[plot].culture[p];

            if culture > 0 {
                if self.at_war(loop_team, team) {
                    // C++ quirk: 32-bit int arithmetic, wraps on overflow (MSVC).
                    culture = culture.wrapping_mul((defines.at_war_culture_anger_modifier + 100).max(0));
                    culture /= 100;
                }

                angry_culture = angry_culture.wrapping_add(culture);
            }
        }

        // C++ quirk: 32-bit int arithmetic, wraps on overflow (MSVC).
        defines.culture_percent_anger.wrapping_mul(angry_culture) / total_culture
    }

    /// C++ `CvCity::getReligionPercentAnger` (CvCity.cpp:4489-4534).
    pub fn city_religion_percent_anger(&self, c: CityId) -> i32 {
        if self.num_cities == 0 {
            return 0;
        }

        let religion_count = self.city_religion_count(c);
        if religion_count == 0 {
            return 0;
        }

        let team = self.city_team(c);
        let mut count = 0;

        for p in self.player_ids() {
            if self.players[p].alive && self.at_war(self.players[p].team, team) {
                if let Some(state_religion) = self.player_get_state_religion(p) {
                    if self.cities[c].has_religion[state_religion] {
                        count += self.players[p].has_religion_count[state_religion];
                    }
                }
            }
        }

        let mut anger = self.d.defines.religion_percent_anger;

        anger *= count;
        anger /= self.num_cities;

        anger /= religion_count;

        anger
    }

    /// C++ `CvCity::getHurryPercentAnger` (CvCity.cpp:4537-4545). C++ default `iExtra = 0`.
    pub fn city_hurry_percent_anger(&self, c: CityId, extra: i32) -> i32 {
        let timer = self.cities[c].hurry_anger_timer;
        if timer == 0 {
            return 0;
        }

        let defines = &self.d.defines;
        ((((timer - 1) / self.city_flat_hurry_anger_length(c)) + 1) * defines.hurry_pop_anger * defines.percent_anger_divisor)
            / (self.cities[c].population + extra).max(1)
            + 1
    }

    /// C++ `CvCity::getConscriptPercentAnger` (CvCity.cpp:4548-4556). C++ default `iExtra = 0`.
    pub fn city_conscript_percent_anger(&self, c: CityId, extra: i32) -> i32 {
        let timer = self.cities[c].conscript_anger_timer;
        if timer == 0 {
            return 0;
        }

        let defines = &self.d.defines;
        ((((timer - 1) / self.city_flat_conscript_anger_length(c)) + 1) * defines.conscript_pop_anger * defines.percent_anger_divisor)
            / (self.cities[c].population + extra).max(1)
            + 1
    }

    /// C++ `CvCity::getDefyResolutionPercentAnger` (CvCity.cpp:4558-4566). C++ default
    /// `iExtra = 0`.
    pub fn city_defy_resolution_percent_anger(&self, c: CityId, extra: i32) -> i32 {
        let timer = self.cities[c].defy_resolution_anger_timer;
        if timer == 0 {
            return 0;
        }

        let defines = &self.d.defines;
        ((((timer - 1) / self.city_flat_defy_resolution_anger_length(c)) + 1)
            * defines.defy_resolution_pop_anger
            * defines.percent_anger_divisor)
            / (self.cities[c].population + extra).max(1)
            + 1
    }

    /// C++ `CvCity::getWarWearinessPercentAnger` (CvCity.cpp:4569-4579).
    pub fn city_war_weariness_percent_anger(&self, c: CityId) -> i32 {
        let owner = self.cities[c].owner;
        let mut anger = self.player_get_war_weariness_percent_anger(owner);

        anger *= (self.cities[c].war_weariness_modifier + self.players[owner].war_weariness_modifier + 100).max(0);
        anger /= 100;

        anger
    }

    /// C++ `CvCity::getLargestCityHappiness` (CvCity.cpp:4582-4590). Uses the population
    /// rank cache (`city_find_population_rank`).
    pub fn city_largest_city_happiness(&self, c: CityId) -> i32 {
        if self.city_find_population_rank(c) <= self.d.worlds[self.setup.world_size].target_num_cities {
            return self.players[self.cities[c].owner].largest_city_happiness;
        }

        0
    }

    /// C++ `CvCity::getVassalHappiness` (CvCity.cpp:4592-4608).
    pub fn city_vassal_happiness(&self, c: CityId) -> i32 {
        let team = self.city_team(c);
        let mut happy = 0;

        for t in self.team_ids() {
            if team != t && self.teams[t].vassal[team] {
                happy += self.d.defines.vassal_happiness;
            }
        }

        happy
    }

    /// C++ `CvCity::getVassalUnhappiness` (CvCity.cpp:4610-4626).
    pub fn city_vassal_unhappiness(&self, c: CityId) -> i32 {
        let team = self.city_team(c);
        let mut unhappy = 0;

        for t in self.team_ids() {
            if team != t && self.teams[team].vassal[t] {
                // C++ uses VASSAL_HAPPINESS here too.
                unhappy += self.d.defines.vassal_happiness;
            }
        }

        unhappy
    }

    /// C++ `CvCity::unhappyLevel` (CvCity.cpp:4629-4675). C++ default `iExtra = 0`.
    pub fn city_unhappy_level(&self, c: CityId, extra: i32) -> i32 {
        let mut unhappiness = 0;

        if !self.city_is_no_unhappiness(c) {
            let owner = self.cities[c].owner;
            let mut anger_percent = 0;

            anger_percent += self.city_overcrowding_percent_anger(c, extra);
            anger_percent += self.city_no_military_percent_anger(c);
            anger_percent += self.city_culture_percent_anger(c);
            anger_percent += self.city_religion_percent_anger(c);
            anger_percent += self.city_hurry_percent_anger(c, extra);
            anger_percent += self.city_conscript_percent_anger(c, extra);
            anger_percent += self.city_defy_resolution_percent_anger(c, extra);
            anger_percent += self.city_war_weariness_percent_anger(c);

            for civic in self.d.civics.keys() {
                anger_percent += self.player_get_civic_percent_anger(owner, civic, false);
            }

            let city = &self.cities[c];
            let player = &self.players[owner];

            unhappiness = (anger_percent * (city.population + extra)) / self.d.defines.percent_anger_divisor;

            unhappiness -= self.city_largest_city_happiness(c).min(0);
            unhappiness -= self.city_military_happiness(c).min(0);
            unhappiness -= self.city_current_state_religion_happiness(c).min(0);
            unhappiness -= city.building_bad_happiness.min(0);
            unhappiness -= city.extra_building_bad_happiness.min(0);
            unhappiness -= city.feature_bad_happiness.min(0);
            unhappiness -= city.bonus_bad_happiness.min(0);
            unhappiness -= city.religion_bad_happiness.min(0);
            unhappiness -= self.city_commerce_happiness(c).min(0);
            unhappiness -= self.map.areas[self.city_area(c)].building_happiness[owner].min(0);
            unhappiness -= player.building_happiness.min(0);
            unhappiness -= (city.extra_happiness + player.extra_happiness).min(0);
            unhappiness -= self.d.handicaps[self.city_handicap_type(c)].happy_bonus.min(0);
            unhappiness += self.city_vassal_unhappiness(c).max(0);
            unhappiness += city.espionage_happiness_counter.max(0);
        }

        unhappiness.max(0)
    }

    /// C++ `CvCity::happyLevel` (CvCity.cpp:4678-4706).
    pub fn city_happy_level(&self, c: CityId) -> i32 {
        let city = &self.cities[c];
        let owner = city.owner;
        let player = &self.players[owner];
        let mut happiness = 0;

        happiness += self.city_largest_city_happiness(c).max(0);
        happiness += self.city_military_happiness(c).max(0);
        happiness += self.city_current_state_religion_happiness(c).max(0);
        happiness += city.building_good_happiness.max(0);
        happiness += city.extra_building_good_happiness.max(0);
        happiness += city.feature_good_happiness.max(0);
        happiness += city.bonus_good_happiness.max(0);
        happiness += city.religion_good_happiness.max(0);
        happiness += self.city_commerce_happiness(c).max(0);
        happiness += self.map.areas[self.city_area(c)].building_happiness[owner].max(0);
        happiness += player.building_happiness.max(0);
        happiness += (city.extra_happiness + player.extra_happiness).max(0);
        happiness += self.d.handicaps[self.city_handicap_type(c)].happy_bonus.max(0);
        happiness += self.city_vassal_happiness(c).max(0);

        if city.happiness_timer > 0 {
            happiness += self.d.defines.temp_happy;
        }

        happiness.max(0)
    }

    /// C++ `CvCity::angryPopulation` (CvCity.cpp:4709-4714). C++ default `iExtra = 0`.
    pub fn city_angry_population(&self, c: CityId, extra: i32) -> i32 {
        range(self.city_unhappy_level(c, extra) - self.city_happy_level(c), 0, self.cities[c].population + extra)
    }

    /// C++ `CvCity::visiblePopulation` (CvCity.cpp:4717-4720).
    pub fn city_visible_population(&self, c: CityId) -> i32 {
        self.cities[c].population - self.city_angry_population(c, 0) - self.cities[c].working_population
    }

    /// C++ `CvCity::totalFreeSpecialists` (CvCity.cpp:4723-4743).
    pub fn city_total_free_specialists(&self, c: CityId) -> i32 {
        let mut count = 0;
        if self.cities[c].population > 0 {
            let owner = self.cities[c].owner;
            count += self.cities[c].free_specialist;
            count += self.map.areas[self.city_area(c)].free_specialist[owner];
            count += self.players[owner].free_specialist;

            for improvement in self.d.improvements.keys() {
                let num_specialists_per_improvement = self.cities[c].improvement_free_specialists[improvement];
                if num_specialists_per_improvement > 0 {
                    count += num_specialists_per_improvement * self.city_count_num_improved_plots(c, Some(improvement), false);
                }
            }
        }

        count
    }

    /// C++ `CvCity::extraPopulation` (CvCity.cpp:4746-4749).
    pub fn city_extra_population(&self, c: CityId) -> i32 {
        self.city_visible_population(c) + self.city_extra_free_specialists(c).min(0)
    }

    /// C++ `CvCity::extraSpecialists` (CvCity.cpp:4752-4755).
    pub fn city_extra_specialists(&self, c: CityId) -> i32 {
        self.city_visible_population(c) + self.city_extra_free_specialists(c)
    }

    /// C++ `CvCity::extraFreeSpecialists` (CvCity.cpp:4758-4761).
    pub fn city_extra_free_specialists(&self, c: CityId) -> i32 {
        self.city_total_free_specialists(c) - self.cities[c].specialist_population
    }

    /// C++ `CvCity::unhealthyPopulation` (CvCity.cpp:4764-4772). C++ defaults
    /// `bNoAngry = false`, `iExtra = 0`.
    pub fn city_unhealthy_population(&self, c: CityId, no_angry: bool, extra: i32) -> i32 {
        if self.city_is_no_unhealthy_population(c) {
            return 0;
        }
        let angry = if no_angry { self.city_angry_population(c, extra) } else { 0 };
        (self.cities[c].population + extra - angry).max(0)
    }

    /// C++ `CvCity::totalGoodBuildingHealth` (CvCity.cpp:4775-4778).
    pub fn city_total_good_building_health(&self, c: CityId) -> i32 {
        let city = &self.cities[c];
        let owner = city.owner;
        city.building_good_health
            + self.map.areas[self.city_area(c)].building_good_health[owner]
            + self.players[owner].building_good_health
            + city.extra_building_good_health
    }

    /// C++ `CvCity::totalBadBuildingHealth` (CvCity.cpp:4781-4789).
    pub fn city_total_bad_building_health(&self, c: CityId) -> i32 {
        if !self.city_is_building_only_healthy(c) {
            let city = &self.cities[c];
            let owner = city.owner;
            return city.building_bad_health
                + self.map.areas[self.city_area(c)].building_bad_health[owner]
                + self.players[owner].building_bad_health
                + city.extra_building_bad_health;
        }

        0
    }

    /// C++ `CvCity::goodHealth` (CvCity.cpp:4792-4842).
    pub fn city_good_health(&self, c: CityId) -> i32 {
        let city = &self.cities[c];
        let mut total_health = 0;

        let mut health = city.fresh_water_good_health;
        if health > 0 {
            total_health += health;
        }

        health = city.feature_good_health;
        if health > 0 {
            total_health += health;
        }

        health = city.power_good_health;
        if health > 0 {
            total_health += health;
        }

        health = city.bonus_good_health;
        if health > 0 {
            total_health += health;
        }

        health = self.city_total_good_building_health(c);
        if health > 0 {
            total_health += health;
        }

        health = self.players[city.owner].extra_health + city.extra_health;
        if health > 0 {
            total_health += health;
        }

        health = self.d.handicaps[self.city_handicap_type(c)].health_bonus;
        if health > 0 {
            total_health += health;
        }

        total_health
    }

    /// C++ `CvCity::badHealth` (CvCity.cpp:4845-4907). C++ defaults `bNoAngry = false`,
    /// `iExtra = 0`.
    pub fn city_bad_health(&self, c: CityId, no_angry: bool, extra: i32) -> i32 {
        let city = &self.cities[c];
        let mut total_health = 0;

        let mut health = city.espionage_health_counter;
        if health > 0 {
            total_health -= health;
        }

        health = city.fresh_water_bad_health;
        if health < 0 {
            total_health += health;
        }

        health = city.feature_bad_health;
        if health < 0 {
            total_health += health;
        }

        health = city.power_bad_health;
        if health < 0 {
            total_health += health;
        }

        health = city.bonus_bad_health;
        if health < 0 {
            total_health += health;
        }

        health = self.city_total_bad_building_health(c);
        if health < 0 {
            total_health += health;
        }

        health = self.players[city.owner].extra_health + city.extra_health;
        if health < 0 {
            total_health += health;
        }

        health = self.d.handicaps[self.city_handicap_type(c)].health_bonus;
        if health < 0 {
            total_health += health;
        }

        // C++ quirk (BTS): the extra building bad health is already part of
        // `totalBadBuildingHealth`, so it counts twice (and still counts once when the city
        // only gets healthy effects from buildings).
        health = city.extra_building_bad_health;
        if health < 0 {
            total_health += health;
        }

        self.city_unhealthy_population(c, no_angry, extra) - total_health
    }

    /// C++ `CvCity::healthRate` (CvCity.cpp:4910-4913). C++ defaults `bNoAngry = false`,
    /// `iExtra = 0`.
    pub fn city_health_rate(&self, c: CityId, no_angry: bool, extra: i32) -> i32 {
        (self.city_good_health(c) - self.city_bad_health(c, no_angry, extra)).min(0)
    }

    /// C++ `CvCity::foodConsumption` (CvCity.cpp:4916-4932). FFH: civics can change the food
    /// eaten per citizen. C++ defaults `bNoAngry = false`, `iExtra = 0`.
    pub fn city_food_consumption(&self, c: CityId, no_angry: bool, extra: i32) -> i32 {
        // FfH: Modified by Kael 09/02/2007 (so civics can modify the amount of food consumed per
        // population): the last civic option whose civic sets a value wins.
        let d = self.d;
        let owner = self.cities[c].owner;
        let mut food = d.defines.food_consumption_per_population;
        for option in d.civic_options.keys() {
            // C++ reads GC.getCivicInfo(getCivics(i)) unchecked; a player always holds a civic in
            // every option once initialized (every FFH2 civilization lists its InitialCivics).
            if let Some(civic) = self.players[owner].civics[option] {
                if d.civics[civic].food_consumption_per_population != 0 {
                    food = d.civics[civic].food_consumption_per_population;
                }
            }
        }
        let angry = if no_angry { self.city_angry_population(c, extra) } else { 0 };
        ((self.cities[c].population + extra) - angry) * food - self.city_health_rate(c, no_angry, extra)
    }

    /// C++ `CvCity::foodDifference` (CvCity.cpp:4935-4962). C++ default `bBottom = true`.
    pub fn city_food_difference(&self, c: CityId, bottom: bool) -> i32 {
        if self.city_is_disorder(c) {
            return 0;
        }
        let food_rate = self.city_yield_rate(c, YieldType::YIELD_FOOD);
        let mut difference = if self.city_is_food_production(c) {
            (food_rate - self.city_food_consumption(c, false, 0)).min(0)
        } else {
            food_rate - self.city_food_consumption(c, false, 0)
        };
        if bottom && self.cities[c].population == 1 && self.cities[c].food == 0 {
            difference = difference.max(0);
        }
        difference
    }

    /// C++ `CvCity::growthThreshold` (CvCity.cpp:4965-4968).
    pub fn city_growth_threshold(&self, c: CityId) -> i32 {
        let city = &self.cities[c];
        self.player_get_growth_threshold(city.owner, city.population)
    }

    /// C++ `CvCity::setPopulation` (CvCity.cpp:5475-5537). `AI_setChooseProductionDirty` /
    /// `AI_makeAssignWorkDirty` are bookkeeping fields (`ai_choose_production_dirty`, the
    /// owner's cities' `ai_assign_work_dirty`).
    pub fn city_set_population(&mut self, c: CityId, new_value: i32) {
        let old_population = self.cities[c].population;

        if old_population != new_value {
            self.cities[c].population = new_value;

            let owner = self.cities[c].owner;
            self.player_invalidate_population_rank_cache(owner);

            let population = self.cities[c].population;
            if population > self.cities[c].highest_population {
                self.cities[c].highest_population = population;
            }

            let change = population - old_population;
            let area = self.city_area(c);
            self.area_change_population_per_player(area, owner, change);
            self.player_change_total_population(owner, change);
            let team = self.city_team(c);
            self.teams[team].total_population += change;
            self.total_population += change;

            // CvArea::changePower
            if old_population > 0 {
                self.map.areas[area].power[owner] -= get_population_power(old_population);
            }
            if population > 0 {
                self.map.areas[area].power[owner] += get_population_power(population);
            }

            let plot = self.city_plot(c);
            self.plot_update_yield(plot);

            self.city_update_maintenance(c);

            if (old_population == 1 && population > 1)
                || (population == 1 && old_population > 1)
                || (population > old_population && self.players[owner].num_cities() <= 2)
            {
                if !self.city_is_human(c) {
                    self.cities[c].ai_choose_production_dirty = true;
                }
            }

            self.player_ai_make_assign_work_dirty(owner);

            // UI: info/layout dirty bits, plot info bars, selection buttons / city screen.
        }
    }

    /// C++ `CvCity::changePopulation` (CvCity.cpp:5540-5543).
    pub fn city_change_population(&mut self, c: CityId, change: i32) {
        let new_value = self.cities[c].population + change;
        self.city_set_population(c, new_value);
    }

    /// C++ `CvCity::getRealPopulation` (CvCity.cpp:5546-5549): `pow(pop, 2.8f) * 1000` in
    /// float math, as C++.
    pub fn city_real_population(&self, c: CityId) -> i32 {
        // C++ quirk: `long` is 32-bit on MSVC, so the product wraps for huge cities.
        ((self.cities[c].population as f32).powf(2.8f32) as i32).wrapping_mul(1000)
    }

    /// C++ `CvCity::changeSpecialistPopulation` (CvCity.cpp:5583-5594).
    pub fn city_change_specialist_population(&mut self, c: CityId, change: i32) {
        if change != 0 {
            self.cities[c].specialist_population += change;

            let owner = self.cities[c].owner;
            self.player_invalidate_yield_rank_cache(owner, None);

            self.city_update_commerces(c);
        }
    }

    /// C++ `CvCity::changeNumGreatPeople` (CvCity.cpp:5603-5612).
    pub fn city_change_num_great_people(&mut self, c: CityId, change: i32) {
        if change != 0 {
            self.cities[c].num_great_people += change;

            self.city_update_commerces(c);
        }
    }

    /// C++ `CvCity::getGreatPeopleRate` (CvCity.cpp:5621-5629).
    pub fn city_great_people_rate(&self, c: CityId) -> i32 {
        if self.city_is_disorder(c) {
            return 0;
        }
        (self.cities[c].base_great_people_rate * self.city_total_great_people_rate_modifier(c)) / 100
    }

    /// C++ `CvCity::getTotalGreatPeopleRateModifier` (CvCity.cpp:5632-5654).
    pub fn city_total_great_people_rate_modifier(&self, c: CityId) -> i32 {
        let owner = self.cities[c].owner;
        let player = &self.players[owner];

        let mut modifier = self.cities[c].great_people_rate_modifier;

        modifier += player.great_people_rate_modifier;

        if let Some(state_religion) = self.player_get_state_religion(owner) {
            if self.cities[c].has_religion[state_religion] {
                modifier += player.state_religion_great_people_rate_modifier;
            }
        }

        if self.player_is_golden_age(owner) {
            modifier += self.d.defines.golden_age_great_people_modifier;
        }

        (modifier + 100).max(0)
    }

    /// C++ `CvCity::changeWarWearinessModifier` (CvCity.cpp:6059-6067).
    pub fn city_change_war_weariness_modifier(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.war_weariness_modifier += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeHurryAngerModifier` (CvCity.cpp:6076-6094): rescales a running
    /// hurry anger timer to the new modifier.
    pub fn city_change_hurry_anger_modifier(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            let mut ratio = 0;
            if city.hurry_anger_timer > 0 {
                ratio = (100 * (city.hurry_anger_timer - 1)) / (100 + city.hurry_anger_modifier).max(1);
            }
            city.hurry_anger_modifier += change;
            if city.hurry_anger_timer > 0 {
                city.hurry_anger_timer = (ratio * (100 + city.hurry_anger_modifier).max(1)) / 100 + 1;
            }
        }
    }

    /// C++ `CvCity::updateFreshWaterHealth` (CvCity.cpp:6150-6184).
    pub fn city_update_fresh_water_health(&mut self, c: CityId) {
        let mut new_good_health = 0;
        let mut new_bad_health = 0;

        let plot = self.city_plot(c);
        if self.plot_is_fresh_water(plot) {
            let change = self.d.defines.fresh_water_health_change;
            if change > 0 {
                new_good_health += change;
            } else {
                new_bad_health += change;
            }
        }

        let city = &mut self.cities[c];
        if city.fresh_water_good_health != new_good_health || city.fresh_water_bad_health != new_bad_health {
            city.fresh_water_good_health = new_good_health;
            city.fresh_water_bad_health = new_bad_health;

            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::updateFeatureHealth` (CvCity.cpp:6199-6249).
    pub fn city_update_feature_health(&mut self, c: CityId) {
        let d = self.d;
        let mut new_good_health = 0;
        let mut new_bad_health = 0;

        for i in 0..NUM_CITY_PLOTS {
            if let Some(loop_plot) = self.city_index_plot(c, i) {
                if let Some(feature) = self.map.plots[loop_plot].feature_type {
                    let health_percent = d.features[feature].health_percent;
                    if health_percent > 0 {
                        new_good_health += health_percent;
                    } else {
                        new_bad_health += health_percent;
                    }
                }
            }
        }

        new_good_health /= 100;
        new_bad_health /= 100;

        let city = &mut self.cities[c];
        if city.feature_good_health != new_good_health || city.feature_bad_health != new_bad_health {
            city.feature_good_health = new_good_health;
            city.feature_bad_health = new_bad_health;

            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::getBuildingHealth(BuildingTypes)` (CvCity.cpp:6264-6274).
    pub fn city_building_health(&self, c: CityId, building: BuildingType) -> i32 {
        let mut health = self.city_building_good_health(c, building);

        if !self.city_is_building_only_healthy(c) {
            health += self.city_building_bad_health(c, building);
        }

        health
    }

    /// C++ `CvCity::getBuildingGoodHealth(BuildingTypes)` (CvCity.cpp:6276-6283). (The no-arg
    /// overload is the field `building_good_health`.)
    pub fn city_building_good_health(&self, c: CityId, building: BuildingType) -> i32 {
        let info = &self.d.buildings[building];
        let owner = self.cities[c].owner;

        let mut health = info.health.max(0);
        health += self.city_building_class_health_change(c, info.building_class_type).max(0);
        health += self.players[owner].extra_building_health[building].max(0);

        health
    }

    /// C++ `CvCity::getBuildingBadHealth(BuildingTypes)` (CvCity.cpp:6285-6297). (The no-arg
    /// overload is the field `building_bad_health`.)
    pub fn city_building_bad_health(&self, c: CityId, building: BuildingType) -> i32 {
        if self.city_is_building_only_healthy(c) {
            return 0;
        }

        let info = &self.d.buildings[building];
        let owner = self.cities[c].owner;

        let mut health = info.health.min(0);
        health += self.city_building_class_health_change(c, info.building_class_type).min(0);
        health += self.players[owner].extra_building_health[building].min(0);

        health
    }

    /// C++ `CvCity::changeBuildingGoodHealth` (CvCity.cpp:6299-6313).
    pub fn city_change_building_good_health(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.building_good_health += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeBuildingBadHealth` (CvCity.cpp:6316-6330).
    pub fn city_change_building_bad_health(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.building_bad_health += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::updatePowerHealth` (CvCity.cpp:6345-6393).
    pub fn city_update_power_health(&mut self, c: CityId) {
        let mut new_good_health = 0;
        let mut new_bad_health = 0;

        if self.city_is_power(c) {
            let power_health = self.d.defines.power_health_change;
            if power_health > 0 {
                new_good_health += power_health;
            } else {
                new_bad_health += power_health;
            }
        }

        if self.city_is_dirty_power(c) {
            let dirty_power_health = self.d.defines.dirty_power_health_change;
            if dirty_power_health > 0 {
                new_good_health += dirty_power_health;
            } else {
                new_bad_health += dirty_power_health;
            }
        }

        let city = &mut self.cities[c];
        if city.power_good_health != new_good_health || city.power_bad_health != new_bad_health {
            city.power_good_health = new_good_health;
            city.power_bad_health = new_bad_health;

            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeBonusGoodHealth` (CvCity.cpp:6408-6424).
    pub fn city_change_bonus_good_health(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.bonus_good_health += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeBonusBadHealth` (CvCity.cpp:6427-6443).
    pub fn city_change_bonus_bad_health(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.bonus_bad_health += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::getMilitaryHappiness` (CvCity.cpp:6452-6455).
    pub fn city_military_happiness(&self, c: CityId) -> i32 {
        let city = &self.cities[c];
        city.military_happiness_units * self.players[city.owner].happy_per_military_unit
    }

    /// C++ `CvCity::changeMilitaryHappinessUnits` (CvCity.cpp:6458-6467).
    pub fn city_change_military_happiness_units(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.military_happiness_units += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::getBuildingHappiness(BuildingTypes)` (CvCity.cpp:6482-6507).
    pub fn city_building_happiness(&self, c: CityId, building: BuildingType) -> i32 {
        let info = &self.d.buildings[building];
        let owner = self.cities[c].owner;
        let player = &self.players[owner];

        let mut happiness = info.happiness;

        if let Some(religion) = info.religion_type {
            if Some(religion) == self.player_get_state_religion(owner) {
                happiness += info.state_religion_happiness;
            }
        }

        happiness += player.extra_building_happiness[building];

        for i in 0..CommerceType::NUM {
            happiness += (info.commerce_happiness[i] * player.commerce_percent[i]) / 100;
        }

        // C++ getBuildingHappyChange((BuildingClassTypes)getBuildingClassType()): no change is
        // stored for NO_BUILDINGCLASS.
        happiness += info.building_class_type.map_or(0, |class| self.city_building_happy_change(c, class));

        happiness
    }

    /// C++ `CvCity::changeBuildingGoodHappiness` (CvCity.cpp:6510-6519).
    pub fn city_change_building_good_happiness(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.building_good_happiness += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeBuildingBadHappiness` (CvCity.cpp:6522-6531).
    pub fn city_change_building_bad_happiness(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.building_bad_happiness += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::updateExtraBuildingHappiness` (CvCity.cpp:6546-6585).
    pub fn city_update_extra_building_happiness(&mut self, c: CityId) {
        let owner = self.cities[c].owner;
        let mut new_good = 0;
        let mut new_bad = 0;

        for building in self.d.buildings.keys() {
            let change = self.city_num_active_building(c, building) * self.players[owner].extra_building_happiness[building];

            if change > 0 {
                new_good += change;
            } else {
                new_bad += change;
            }
        }

        let city = &mut self.cities[c];
        if city.extra_building_good_happiness != new_good {
            city.extra_building_good_happiness = new_good;

            city.ai_assign_work_dirty = true;
        }

        if city.extra_building_bad_happiness != new_bad {
            city.extra_building_bad_happiness = new_bad;

            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::updateExtraBuildingHealth` (CvCity.cpp:6600-6636).
    pub fn city_update_extra_building_health(&mut self, c: CityId) {
        let owner = self.cities[c].owner;
        let mut new_good = 0;
        let mut new_bad = 0;

        for building in self.d.buildings.keys() {
            let change = self.city_num_active_building(c, building) * self.players[owner].extra_building_health[building];

            if change > 0 {
                new_good += change;
            } else {
                new_bad += change;
            }
        }

        let city = &mut self.cities[c];
        if city.extra_building_good_health != new_good {
            city.extra_building_good_health = new_good;

            city.ai_assign_work_dirty = true;
        }

        if city.extra_building_bad_health != new_bad {
            city.extra_building_bad_health = new_bad;

            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::updateFeatureHappiness` (CvCity.cpp:6651-6709).
    pub fn city_update_feature_happiness(&mut self, c: CityId) {
        let d = self.d;
        let owner = self.cities[c].owner;
        let mut new_good = 0;
        let mut new_bad = 0;

        for i in 0..NUM_CITY_PLOTS {
            let Some(loop_plot) = self.city_index_plot(c, i) else {
                continue;
            };

            if let Some(feature) = self.map.plots[loop_plot].feature_type {
                let happy = self.players[owner].feature_happiness[feature];
                if happy > 0 {
                    new_good += happy;
                } else {
                    new_bad += happy;
                }
            }

            if let Some(improvement) = self.map.plots[loop_plot].improvement_type {
                let happy = d.improvements[improvement].happiness;
                if happy > 0 {
                    new_good += happy;
                } else {
                    new_bad += happy;
                }
            }
        }

        let city = &mut self.cities[c];
        if city.feature_good_happiness != new_good {
            city.feature_good_happiness = new_good;

            city.ai_assign_work_dirty = true;
        }

        if city.feature_bad_happiness != new_bad {
            city.feature_bad_happiness = new_bad;

            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeBonusGoodHappiness` (CvCity.cpp:6724-6733).
    pub fn city_change_bonus_good_happiness(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.bonus_good_happiness += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeBonusBadHappiness` (CvCity.cpp:6736-6745).
    pub fn city_change_bonus_bad_happiness(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.bonus_bad_happiness += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::getReligionHappiness(ReligionTypes)` (CvCity.cpp:6760-6779).
    pub fn city_religion_happiness(&self, c: CityId, religion: ReligionType) -> i32 {
        let mut happiness = 0;

        if self.cities[c].has_religion[religion] {
            let owner = self.cities[c].owner;
            if Some(religion) == self.player_get_state_religion(owner) {
                happiness += self.players[owner].state_religion_happiness;
            } else {
                happiness += self.players[owner].non_state_religion_happiness;
            }
        }

        happiness
    }

    /// C++ `CvCity::updateReligionHappiness` (CvCity.cpp:6782-6821).
    pub fn city_update_religion_happiness(&mut self, c: CityId) {
        let mut new_good = 0;
        let mut new_bad = 0;

        for religion in self.d.religions.keys() {
            let change = self.city_religion_happiness(c, religion);

            if change > 0 {
                new_good += change;
            } else {
                new_bad += change;
            }
        }

        let city = &mut self.cities[c];
        if city.religion_good_happiness != new_good {
            city.religion_good_happiness = new_good;

            city.ai_assign_work_dirty = true;
        }

        if city.religion_bad_happiness != new_bad {
            city.religion_bad_happiness = new_bad;

            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeExtraHappiness` (CvCity.cpp:6830-6838).
    pub fn city_change_extra_happiness(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.extra_happiness += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeExtraHealth` (CvCity.cpp:6846-6854).
    pub fn city_change_extra_health(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.extra_health += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeHurryAngerTimer` (CvCity.cpp:6864-6873).
    pub fn city_change_hurry_anger_timer(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.hurry_anger_timer += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeConscriptAngerTimer` (CvCity.cpp:6882-6891).
    pub fn city_change_conscript_anger_timer(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.conscript_anger_timer += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeDefyResolutionAngerTimer` (CvCity.cpp:6899-6908).
    pub fn city_change_defy_resolution_anger_timer(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.defy_resolution_anger_timer += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::flatDefyResolutionAngerLength` (CvCity.cpp:6910-6920).
    pub fn city_flat_defy_resolution_anger_length(&self, _c: CityId) -> i32 {
        let mut anger = self.d.defines.defy_resolution_anger_divisor;

        anger *= self.d.game_speeds[self.game_speed_type()].hurry_conscript_anger_percent;
        anger /= 100;

        anger.max(1)
    }

    /// C++ `CvCity::changeHappinessTimer` (CvCity.cpp:6929-6938).
    pub fn city_change_happiness_timer(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.happiness_timer += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::isNoUnhappiness` (CvCity.cpp:6947-6950).
    pub fn city_is_no_unhappiness(&self, c: CityId) -> bool {
        self.cities[c].no_unhappiness_count > 0
    }

    /// C++ `CvCity::changeNoUnhappinessCount` (CvCity.cpp:6953-6962).
    pub fn city_change_no_unhappiness_count(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.no_unhappiness_count += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::isNoUnhealthyPopulation` (CvCity.cpp:6971-6979).
    pub fn city_is_no_unhealthy_population(&self, c: CityId) -> bool {
        if self.player_is_no_unhealthy_population(self.cities[c].owner) {
            return true;
        }

        self.cities[c].no_unhealthy_population_count > 0
    }

    /// C++ `CvCity::changeNoUnhealthyPopulationCount` (CvCity.cpp:6982-6991).
    pub fn city_change_no_unhealthy_population_count(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.no_unhealthy_population_count += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::isBuildingOnlyHealthy` (CvCity.cpp:7000-7008).
    pub fn city_is_building_only_healthy(&self, c: CityId) -> bool {
        if self.player_is_building_only_healthy(self.cities[c].owner) {
            return true;
        }

        self.cities[c].building_only_healthy_count > 0
    }

    /// C++ `CvCity::changeBuildingOnlyHealthyCount` (CvCity.cpp:7011-7020).
    pub fn city_change_building_only_healthy_count(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.building_only_healthy_count += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeFreeSpecialist` (CvCity.cpp:7312-7321).
    pub fn city_change_free_specialist(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.free_specialist += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::setWeLoveTheKingDay` (CvCity.cpp:7642-7674).
    pub fn city_set_we_love_the_king_day(&mut self, c: CityId, new_value: bool) {
        if self.cities[c].we_love_the_king_day != new_value {
            self.cities[c].we_love_the_king_day = new_value;

            self.city_update_maintenance(c);

            let d = self.d;
            let owner = self.cities[c].owner;
            let mut civic = None;

            for i in d.civics.keys() {
                if self.player_is_civic(owner, i) && !d.civics[i].we_love_the_king.is_empty() {
                    civic = Some(i);
                    break;
                }
            }

            if let Some(civic) = civic {
                // C++: TXT_KEY_CITY_CELEBRATE message to the owner (UI; sent whether the
                // celebration starts or ends) -> game log.
                let turn = self.game_turn();
                let name = &self.cities[c].name;
                self.log.msg(turn, Some(owner), || {
                    format!("{} celebrates \"We Love the {} Day\"", name, d.text.name(&d.civics[civic].we_love_the_king))
                });
            }
        }
    }

    /// C++ `CvCity::setCitizensAutomated` (CvCity.cpp:7683-7708). Automating calls the
    /// governor (`AI_assignWorkingPlots`).
    pub fn city_set_citizens_automated(&mut self, c: CityId, new_value: bool) {
        if self.cities[c].citizens_automated != new_value {
            self.cities[c].citizens_automated = new_value;

            if self.cities[c].citizens_automated {
                // AI_assignWorkingPlots
                self.city_assign_working_plots(c);
            } else {
                for specialist in self.d.specialists.keys() {
                    self.city_set_force_specialist_count(c, specialist, 0);
                }
            }

            // UI: selection buttons dirty bit.
        }
    }

    /// C++ `CvCity::getCommerceHappinessByType` (CvCity.cpp:9014-9017).
    pub fn city_commerce_happiness_by_type(&self, c: CityId, commerce: CommerceType) -> i32 {
        let city = &self.cities[c];
        (city.commerce_happiness_per[commerce.idx()] * self.players[city.owner].commerce_percent[commerce.idx()]) / 100
    }

    /// C++ `CvCity::getCommerceHappiness` (CvCity.cpp:9020-9033).
    pub fn city_commerce_happiness(&self, c: CityId) -> i32 {
        let mut happiness = 0;
        for i in 0..CommerceType::NUM {
            happiness += self.city_commerce_happiness_by_type(c, CommerceType::from_usize(i));
        }
        happiness
    }

    /// C++ `CvCity::changeCommerceHappinessPer` (CvCity.cpp:9036-9048).
    pub fn city_change_commerce_happiness_per(&mut self, c: CityId, commerce: CommerceType, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.commerce_happiness_per[commerce.idx()] += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::setGreatPeopleUnitRate` (CvCity.cpp:9848-9859): ignored for espionage
    /// great people under the no-espionage option.
    pub fn city_set_great_people_unit_rate(&mut self, c: CityId, unit: UnitType, new_value: i32) {
        if self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) && self.d.units[unit].espionage_points > 0 {
            return;
        }

        self.cities[c].great_people_unit_rate[unit] = new_value;
    }

    /// C++ `CvCity::changeGreatPeopleUnitRate` (CvCity.cpp:9862-9865).
    pub fn city_change_great_people_unit_rate(&mut self, c: CityId, unit: UnitType, change: i32) {
        let new_value = self.cities[c].great_people_unit_rate[unit] + change;
        self.city_set_great_people_unit_rate(c, unit, new_value);
    }

    /// C++ `CvCity::setSpecialistCount` (CvCity.cpp:9899-9921).
    pub fn city_set_specialist_count(&mut self, c: CityId, specialist: SpecialistType, new_value: i32) {
        let old_value = self.cities[c].specialist_count[specialist];
        if old_value != new_value {
            self.cities[c].specialist_count[specialist] = new_value;
            self.city_change_specialist_population(c, new_value - old_value);
            self.city_process_specialist(c, specialist, new_value - old_value);
        }
    }

    /// C++ `CvCity::changeSpecialistCount` (CvCity.cpp:9924-9927).
    pub fn city_change_specialist_count(&mut self, c: CityId, specialist: SpecialistType, change: i32) {
        let new_value = self.cities[c].specialist_count[specialist] + change;
        self.city_set_specialist_count(c, specialist, new_value);
    }

    /// C++ `CvCity::alterSpecialistCount` (CvCity.cpp:9930-10015): the player's specialist
    /// +/- buttons (forced specialists when automated; governor calls otherwise).
    pub fn city_alter_specialist_count(&mut self, c: CityId, specialist: SpecialistType, change: i32) {
        if change == 0 {
            return;
        }

        if self.cities[c].citizens_automated && self.cities[c].force_specialist_count[specialist] + change < 0 {
            self.city_set_citizens_automated(c, false);
        }

        if self.cities[c].citizens_automated {
            self.city_change_force_specialist_count(c, specialist, change);
        } else if change > 0 {
            for _ in 0..change {
                // C++ quirk: a citizen is freed before the validity test, and stays idle if the
                // specialist then turns out not to be valid.
                if self.city_extra_population(c) > 0 || self.city_ai_remove_worst_citizen(c, Some(specialist)) {
                    if self.city_is_specialist_valid(c, specialist, 1) {
                        self.city_change_specialist_count(c, specialist, 1);
                    }
                }
            }
        } else {
            let default_specialist = opt_from_i32::<SpecialistType>(self.d.defines.default_specialist);

            for _ in 0..-change {
                if self.cities[c].specialist_count[specialist] <= 0 {
                    continue;
                }

                self.city_change_specialist_count(c, specialist, -1);

                match default_specialist {
                    Some(default) if default != specialist => {
                        self.city_change_specialist_count(c, default, 1);
                    }
                    _ => {
                        if self.city_extra_free_specialists(c) > 0 {
                            self.city_ai_add_best_citizen(c, false, true);
                        } else {
                            let mut num_can_work_plots = 0;
                            for i in 0..NUM_CITY_PLOTS {
                                if i != CITY_HOME_PLOT && !self.cities[c].working_plot[i] {
                                    if let Some(loop_plot) = self.city_index_plot(c, i) {
                                        if self.city_can_work(c, loop_plot) {
                                            num_can_work_plots += 1;
                                        }
                                    }
                                }
                            }

                            if num_can_work_plots > 0 {
                                self.city_ai_add_best_citizen(c, true, false);
                            } else {
                                self.city_ai_add_best_citizen(c, false, true);
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvCity::isSpecialistValid` (CvCity.cpp:10026-10029). C++ default `iExtra = 0`.
    pub fn city_is_specialist_valid(&self, c: CityId, specialist: SpecialistType, extra: i32) -> bool {
        let city = &self.cities[c];
        (city.specialist_count[specialist] + extra) <= city.max_specialist_count[specialist]
            || self.player_is_specialist_valid(city.owner, specialist)
            || specialist.i() == self.d.defines.default_specialist
    }

    /// C++ `CvCity::changeMaxSpecialistCount` (CvCity.cpp:10032-10043).
    pub fn city_change_max_specialist_count(&mut self, c: CityId, specialist: SpecialistType, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.max_specialist_count[specialist] = (city.max_specialist_count[specialist] + change).max(0);
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::isSpecialistForced` (CvCity.cpp:10054-10067).
    pub fn city_is_specialist_forced(&self, c: CityId) -> bool {
        self.cities[c].force_specialist_count.iter().any(|&n| n > 0)
    }

    /// C++ `CvCity::setForceSpecialistCount` (CvCity.cpp:10070-10086).
    pub fn city_set_force_specialist_count(&mut self, c: CityId, specialist: SpecialistType, new_value: i32) {
        let city = &mut self.cities[c];
        if city.force_specialist_count[specialist] != new_value {
            city.force_specialist_count[specialist] = new_value.max(0);
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeForceSpecialistCount` (CvCity.cpp:10089-10092).
    pub fn city_change_force_specialist_count(&mut self, c: CityId, specialist: SpecialistType, change: i32) {
        let new_value = self.cities[c].force_specialist_count[specialist] + change;
        self.city_set_force_specialist_count(c, specialist, new_value);
    }

    /// C++ `CvCity::getAddedFreeSpecialistCount` (CvCity.cpp:10102-10117): free specialists
    /// not coming from buildings.
    pub fn city_added_free_specialist_count(&self, c: CityId, specialist: SpecialistType) -> i32 {
        let d = self.d;
        let mut num_added_specialists = self.cities[c].free_specialist_count[specialist];

        for building in d.buildings.keys() {
            let free = d.buildings[building].free_specialist_count[specialist];
            if free > 0 {
                num_added_specialists -= self.city_num_active_building(c, building) * free;
            }
        }

        num_added_specialists.max(0)
    }

    /// C++ `CvCity::setFreeSpecialistCount` (CvCity.cpp:10119-10141).
    pub fn city_set_free_specialist_count(&mut self, c: CityId, specialist: SpecialistType, new_value: i32) {
        let old_value = self.cities[c].free_specialist_count[specialist];
        if old_value != new_value {
            self.cities[c].free_specialist_count[specialist] = new_value;
            self.city_change_num_great_people(c, new_value - old_value);
            self.city_process_specialist(c, specialist, new_value - old_value);
        }
    }

    /// C++ `CvCity::changeFreeSpecialistCount` (CvCity.cpp:10143-10146).
    pub fn city_change_free_specialist_count(&mut self, c: CityId, specialist: SpecialistType, change: i32) {
        let new_value = self.cities[c].free_specialist_count[specialist] + change;
        self.city_set_free_specialist_count(c, specialist, new_value);
    }

    /// C++ `CvCity::changeImprovementFreeSpecialists` (CvCity.cpp:10155-10164).
    pub fn city_change_improvement_free_specialists(&mut self, c: CityId, improvement: ImprovementType, change: i32) {
        if change != 0 {
            let count = &mut self.cities[c].improvement_free_specialists[improvement];
            *count = (*count + change).max(0);
        }
    }

    /// C++ `CvCity::getCurrentStateReligionHappiness` (CvCity.cpp:10183-10191).
    pub fn city_current_state_religion_happiness(&self, c: CityId) -> i32 {
        match self.player_get_state_religion(self.cities[c].owner) {
            Some(state_religion) => self.cities[c].state_religion_happiness[state_religion],
            None => 0,
        }
    }

    /// C++ `CvCity::changeStateReligionHappiness` (CvCity.cpp:10202-10213).
    pub fn city_change_state_religion_happiness(&mut self, c: CityId, religion: ReligionType, change: i32) {
        if change != 0 {
            let city = &mut self.cities[c];
            city.state_religion_happiness[religion] += change;
            city.ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::isWorkingPlot(const CvPlot*)` (CvCity.cpp:10292-10304). (The index
    /// overload is the field `working_plot[i]`.)
    pub fn city_is_working_plot(&self, c: CityId, plot: PlotIdx) -> bool {
        match self.city_plot_index(c, plot) {
            Some(i) => self.cities[c].working_plot[i],
            None => false,
        }
    }

    /// C++ `CvCity::setWorkingPlot(int, bool)` (CvCity.cpp:10307-10366).
    pub fn city_set_working_plot(&mut self, c: CityId, index: usize, new_value: bool) {
        if self.cities[c].working_plot[index] == new_value {
            return;
        }

        self.cities[c].working_plot[index] = new_value;

        if let Some(plot) = self.city_index_plot(c, index) {
            if self.cities[c].working_plot[index] {
                if index != CITY_HOME_PLOT {
                    self.cities[c].working_population += 1;
                }

                for i in 0..YieldType::NUM {
                    let plot_yield = self.map.plots[plot].yields[i];
                    self.city_change_base_yield_rate(c, YieldType::from_usize(i), plot_yield);
                }

                // C++ pPlot->updatePlotBuilder(): graphics.
            } else {
                if index != CITY_HOME_PLOT {
                    self.cities[c].working_population -= 1;
                }

                for i in 0..YieldType::NUM {
                    let plot_yield = self.map.plots[plot].yields[i];
                    self.city_change_base_yield_rate(c, YieldType::from_usize(i), -plot_yield);
                }
            }

            // UI: pPlot->updateSymbolDisplay() for the active team / debug mode.
        }

        // UI: info pane, city screen and colored plots dirty bits.
    }

    /// C++ `CvCity::setWorkingPlot(CvPlot*, bool)` (CvCity.cpp:10369-10372).
    pub fn city_set_working_plot_at(&mut self, c: CityId, plot: PlotIdx, new_value: bool) {
        // C++ passes -1 on (UB); a plot outside the radius is a caller bug.
        let index = self.city_plot_index(c, plot).expect("CvCity::setWorkingPlot: plot outside the city radius");
        self.city_set_working_plot(c, index, new_value);
    }

    /// C++ `CvCity::alterWorkingPlot` (CvCity.cpp:10375-10423): the player clicking a city
    /// plot (governor calls `AI_addBestCitizen` / `AI_removeWorstCitizen`).
    pub fn city_alter_working_plot(&mut self, c: CityId, index: usize) {
        if index == CITY_HOME_PLOT {
            self.city_set_citizens_automated(c, true);
            return;
        }

        let Some(plot) = self.city_index_plot(c, index) else {
            return;
        };

        if self.city_can_work(c, plot) {
            self.city_set_citizens_automated(c, false);

            if self.cities[c].working_plot[index] {
                self.city_set_working_plot(c, index, false);

                match opt_from_i32::<SpecialistType>(self.d.defines.default_specialist) {
                    Some(default) => self.city_change_specialist_count(c, default, 1),
                    None => {
                        self.city_ai_add_best_citizen(c, false, true);
                    }
                }
            } else if self.city_extra_population(c) > 0 || self.city_ai_remove_worst_citizen(c, None) {
                self.city_set_working_plot(c, index, true);
            }
        } else if self.map.plots[plot].owner == Some(self.cities[c].owner) {
            self.plot_set_working_city_override(plot, Some(c));
        }
    }

    /// C++ `CvCity::doGrowth` (CvCity.cpp:11743-11800). Python `doGrowth` callback; growth is
    /// halted by `AI_stopGrowth` (-> `city_ai_stop_growth`). Python event `cityGrowth`.
    pub fn city_do_growth(&mut self, c: CityId) {
        // Python CvGameUtils.doGrowth returns False in FFH2 (see ffh/game_utils.rs), so the C++
        // default always runs.

        let diff = self.city_food_difference(c, true);

        // changeFood / changeFoodKept
        self.cities[c].food += diff;
        self.cities[c].food_kept += diff;

        let max_food_kept = (self.city_growth_threshold(c) * self.cities[c].max_food_kept_percent) / 100;
        self.cities[c].food_kept = range(self.cities[c].food_kept, 0, max_food_kept);

        if self.cities[c].food >= self.city_growth_threshold(c) {
            // GrowthControl (Jean Elcard, merged by Sephi): AI_stopGrowth() replaces
            // AI_isEmphasizeAvoidGrowth().
            if self.city_ai_stop_growth(c) {
                let threshold = self.city_growth_threshold(c);
                self.cities[c].food = threshold;
            } else {
                let food_used = (self.city_growth_threshold(c) - self.cities[c].food_kept).max(0);
                self.cities[c].food -= food_used;
                self.city_change_population(c, 1);

                // ONEVENT - City growth
                let owner = self.cities[c].owner;
                self.ev_city_growth(c, owner);
            }
        } else if self.cities[c].food < 0 {
            // changeFood(-(getFood()))
            self.cities[c].food = 0;

            if self.cities[c].population > 1 {
                self.city_change_population(c, -1);
            }
        }
    }

    /// C++ `CvCity::doGreatPeople` (CvCity.cpp:12218-12279). Python `doGreatPeople` callback;
    /// draws "Great Person".
    pub fn city_do_great_people(&mut self, c: CityId) {
        // Python CvGameUtils.doGreatPeople returns False in FFH2 (see ffh/game_utils.rs), so the
        // C++ default always runs.

        if self.city_is_disorder(c) {
            return;
        }

        let rate = self.city_great_people_rate(c);
        self.cities[c].great_people_progress += rate;

        for unit in self.d.units.keys() {
            let city = &mut self.cities[c];
            city.great_people_unit_progress[unit] += city.great_people_unit_rate[unit];
        }

        let owner = self.cities[c].owner;
        if self.cities[c].great_people_progress >= self.player_great_people_threshold(owner, false) {
            let total_great_people_unit_progress: i32 = self.cities[c].great_people_unit_progress.iter().sum();

            let mut great_people_unit_rand = self.soren_rand_num(total_great_people_unit_progress, "Great Person");

            let mut great_people_unit = None;
            for unit in self.d.units.keys() {
                let progress = self.cities[c].great_people_unit_progress[unit];
                if great_people_unit_rand < progress {
                    great_people_unit = Some(unit);
                    break;
                } else {
                    great_people_unit_rand -= progress;
                }
            }

            if let Some(great_people_unit) = great_people_unit {
                let threshold = self.player_great_people_threshold(owner, false);
                self.cities[c].great_people_progress -= threshold;

                self.cities[c].great_people_unit_progress.fill(0);

                self.city_create_great_people(c, great_people_unit, true, false);
            }
        }
    }

    /// C++ `CvCity::getBestYieldAvailable` (CvCity.cpp:14344-14380).
    pub fn city_best_yield_available(&self, c: CityId, yield_type: YieldType) -> i32 {
        let mut best_yield_available = 0;

        for j in 0..NUM_CITY_PLOTS {
            if j != CITY_HOME_PLOT && !self.cities[c].working_plot[j] {
                if let Some(plot) = self.city_index_plot(c, j) {
                    if self.city_can_work(c, plot) {
                        let plot_yield = self.map.plots[plot].yields[yield_type.idx()];
                        if plot_yield > best_yield_available {
                            best_yield_available = plot_yield;
                        }
                    }
                }
            }
        }

        for specialist in self.d.specialists.keys() {
            if self.city_is_specialist_valid(c, specialist, 1) {
                let specialist_yield = self.d.specialists[specialist].yield_change[yield_type.idx()];
                if specialist_yield > best_yield_available {
                    best_yield_available = specialist_yield;
                }
            }
        }

        best_yield_available
    }

    /// C++ `CvCity::getNumCityPlots` (CvCity.cpp:14613-14620, FFH): 37 at radius 3, else 21.
    pub fn city_num_city_plots(&self, c: CityId) -> i32 {
        if self.cities[c].plot_radius == 3 {
            37
        } else {
            21
        }
    }

    /// C++ `CvCity::setPlotRadius` (CvCity.cpp:14627-14655, FFH). C++ quirk: the loop that
    /// frees worked outer plots tests `iNewValue < getPlotRadius()` after the assignment, so
    /// it never runs.
    pub fn city_set_plot_radius(&mut self, c: CityId, new_value: i32) {
        if new_value == self.cities[c].plot_radius {
            return;
        }

        self.cities[c].plot_radius = new_value;

        // C++ quirk: compared after the assignment above, so this is always false.
        if new_value < self.cities[c].plot_radius {
            for i in 8..NUM_CITY_PLOTS {
                // That workers in outlying plots are not stuck there...
                if self.cities[c].working_plot[i] {
                    self.city_set_working_plot(c, i, false);
                }
            }
        }

        let (x, y) = (self.cities[c].x, self.cities[c].y);
        for i in 8..NUM_CITY_PLOTS {
            if let Some(loop_plot) = self.plot_city(x, y, i) {
                self.plot_update_working_city(loop_plot);
            }
        }

        self.city_update_feature_health(c);
        self.city_update_feature_happiness(c);
    }

    // ---- CvCityAI: player-settable emphasize and FFH2 growth control ----

    /// C++ `CvCityAI::AI_isEmphasizeAvoidGrowth` (CvCityAI.cpp:4861-4864).
    pub fn city_ai_is_emphasize_avoid_growth(&self, c: CityId) -> bool {
        self.cities[c].ai_emphasize_avoid_growth_count > 0
    }

    /// C++ `CvCityAI::AI_isEmphasizeAvoidAngryCitizens` (CvCityAI.cpp:4876-4879).
    pub fn city_ai_is_emphasize_avoid_angry_citizens(&self, c: CityId) -> bool {
        self.cities[c].ai_emphasize_avoid_angry_citizens_count > 0
    }

    /// C++ `CvCityAI::AI_isEmphasizeAvoidUnhealthyCitizens` (CvCityAI.cpp:4886-4889).
    pub fn city_ai_is_emphasize_avoid_unhealthy_citizens(&self, c: CityId) -> bool {
        self.cities[c].ai_emphasize_avoid_unhealthy_citizens_count > 0
    }

    /// C++ `CvCityAI::AI_isEmphasizeGreatPeople` (CvCityAI.cpp:4900-4903).
    pub fn city_ai_is_emphasize_great_people(&self, c: CityId) -> bool {
        self.cities[c].ai_emphasize_great_people_count > 0
    }

    /// C++ `CvCityAI::AI_isEmphasizeYield` (CvCityAI.cpp:4995-4998).
    pub fn city_ai_is_emphasize_yield(&self, c: CityId, yield_type: YieldType) -> bool {
        self.cities[c].ai_emphasize_yield_count[yield_type.idx()] > 0
    }

    /// C++ `CvCityAI::AI_isEmphasizeCommerce` (CvCityAI.cpp:5009-5012). (C++ quirk:
    /// `AI_getEmphasizeCommerceCount` returns `count > 0`; the result here is the same.)
    pub fn city_ai_is_emphasize_commerce(&self, c: CityId, commerce: CommerceType) -> bool {
        self.cities[c].ai_emphasize_commerce_count[commerce.idx()] > 0
    }

    /// C++ `CvCityAI::AI_setEmphasize` (CvCityAI.cpp:5024-5091): sets an emphasize flag and
    /// keeps the per-kind counts (`EmphasizeInfo` avoid growth / avoid angry / avoid unhealthy
    /// / great people / yield / commerce changes), then reassigns citizens (governor).
    pub fn city_ai_set_emphasize(&mut self, c: CityId, emphasize: EmphasizeType, new_value: bool) {
        if self.cities[c].ai_emphasize[emphasize] == new_value {
            return;
        }

        let info = &self.d.emphasizes[emphasize];
        let city = &mut self.cities[c];
        city.ai_emphasize[emphasize] = new_value;
        let delta = if city.ai_emphasize[emphasize] { 1 } else { -1 };

        if info.avoid_growth {
            city.ai_emphasize_avoid_growth_count += delta;
        }

        // GrowthControl (Jean Elcard, merged by Sephi): switch growth control on or off.
        // Multiple switches allowed.
        if info.avoid_angry_citizens {
            city.ai_emphasize_avoid_angry_citizens_count += delta;
        }
        if info.avoid_unhealthy_citizens {
            city.ai_emphasize_avoid_unhealthy_citizens_count += delta;
        }

        if info.great_people {
            city.ai_emphasize_great_people_count += delta;
        }

        for i in 0..YieldType::NUM {
            if info.yield_change[i] != 0 {
                city.ai_emphasize_yield_count[i] += delta;
            }
        }

        for i in 0..CommerceType::NUM {
            if info.commerce_change[i] != 0 {
                city.ai_emphasize_commerce_count[i] += delta;
            }
        }

        // AI_assignWorkingPlots
        self.city_assign_working_plots(c);

        // UI: selection buttons dirty bit.
    }

    /// C++ `CvCityAI::AI_stopGrowth` (CvCityAI.cpp:432-488, FFH2 GrowthControl): whether
    /// `doGrowth` keeps the city at its size.
    pub fn city_ai_stop_growth(&self, c: CityId) -> bool {
        if self.city_ai_is_emphasize_avoid_growth(c) {
            return true;
        }
        if self.city_is_food_production(c) {
            return true;
        }
        if self.city_is_disorder(c) {
            return true;
        }
        if self.city_ai_is_emphasize_avoid_angry_citizens(c) {
            if self.city_unhappy_level(c, 0) > self.city_happy_level(c) {
                return true;
            }
            if self.city_food_turns_left(c) == 1 {
                let happy_faces_left = self.city_happy_level(c) - self.city_unhappy_level(c, 0);
                if happy_faces_left < 1 {
                    return true;
                }
            }
        }
        if self.city_ai_is_emphasize_avoid_unhealthy_citizens(c) {
            if self.city_bad_health(c, false, 0) > self.city_good_health(c) {
                return true;
            }
            if self.city_food_turns_left(c) == 1 {
                let healthy_faces_left = self.city_good_health(c) - self.city_bad_health(c, false, 0);
                if healthy_faces_left < 1 {
                    return true;
                }
            }
        }
        false
    }

    // ---- private helpers ----

    /// C++ `getBuildingHealthChange((BuildingClassTypes)kBuilding.getBuildingClassType())`: no
    /// change is stored for NO_BUILDINGCLASS.
    fn city_building_class_health_change(&self, c: CityId, class: Option<BuildingClassType>) -> i32 {
        class.map_or(0, |class| self.city_building_health_change(c, class))
    }

}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::City;
    use crate::player::Player;
    use crate::setup::GameSetup;

    const P0: PlayerId = PlayerId(0);

    /// A game with real `Player` objects (not `player_init`ed) and one city of player 0 with
    /// every member at its reset default (`city_init` is not run).
    fn test_city() -> (Game, CityId) {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 7));
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        let id = g.cities.next_id();
        let c = g.player_add_city(P0, City::new(d, num_players, num_teams, id, P0, 2, 1, 0));
        (g, c)
    }

    #[test]
    fn percent_anger_formulas() {
        let (mut g, c) = test_city();
        let defines = &g.d.defines;
        g.cities[c].population = 5;

        assert_eq!(g.city_overcrowding_percent_anger(c, 0), defines.percent_anger_divisor + 1);
        assert_eq!(g.city_overcrowding_percent_anger(c, -5), 0);

        assert_eq!(g.city_no_military_percent_anger(c), defines.no_military_percent_anger);
        g.cities[c].military_happiness_units = 1;
        assert_eq!(g.city_no_military_percent_anger(c), 0);

        let length = g.city_flat_defy_resolution_anger_length(c);
        let speed = &g.d.game_speeds[g.game_speed_type()];
        assert_eq!(length, (defines.defy_resolution_anger_divisor * speed.hurry_conscript_anger_percent / 100).max(1));
        assert_eq!(g.city_defy_resolution_percent_anger(c, 0), 0);
        // Two anger periods running (the timer counts down from `2 * length`).
        g.cities[c].defy_resolution_anger_timer = length + 1;
        assert_eq!(
            g.city_defy_resolution_percent_anger(c, 0),
            (2 * defines.defy_resolution_pop_anger * defines.percent_anger_divisor) / 5 + 1
        );
        assert_eq!(
            g.city_defy_resolution_percent_anger(c, 5),
            (2 * defines.defy_resolution_pop_anger * defines.percent_anger_divisor) / 10 + 1
        );
    }

    #[test]
    fn specialist_validity() {
        let (mut g, c) = test_city();
        let d = g.d;
        let default = opt_from_i32::<SpecialistType>(d.defines.default_specialist).expect("DEFAULT_SPECIALIST");
        let other = d.specialists.keys().find(|&s| s != default).unwrap();

        // The default specialist is always valid; others need room under the city maximum or
        // the owner's unlimited permission.
        assert!(g.city_is_specialist_valid(c, default, 1));
        assert!(!g.city_is_specialist_valid(c, other, 1));
        assert!(g.city_is_specialist_valid(c, other, 0));
        g.city_change_max_specialist_count(c, other, 1);
        assert!(g.cities[c].ai_assign_work_dirty);
        assert!(g.city_is_specialist_valid(c, other, 1));
        assert!(!g.city_is_specialist_valid(c, other, 2));
        g.players[P0].specialist_valid_count[other] = 1;
        assert!(g.city_is_specialist_valid(c, other, 2));
    }

    #[test]
    fn alter_specialist_count_forces_specialists_while_automated() {
        let (mut g, c) = test_city();
        let s = g.d.specialists.keys().next().unwrap();
        assert!(g.cities[c].citizens_automated);

        g.city_alter_specialist_count(c, s, 2);
        assert_eq!(g.cities[c].force_specialist_count[s], 2);
        g.city_alter_specialist_count(c, s, -1);
        assert_eq!(g.cities[c].force_specialist_count[s], 1);
        assert!(g.cities[c].citizens_automated);

        // Going below zero turns automation off (clearing every forced count); the city has no
        // such specialist to remove, so nothing else changes.
        g.city_alter_specialist_count(c, s, -2);
        assert!(!g.cities[c].citizens_automated);
        assert!(!g.city_is_specialist_forced(c));
        assert_eq!(g.cities[c].specialist_count[s], 0);
    }

    #[test]
    fn religion_happiness_follows_the_state_religion() {
        let (mut g, c) = test_city();
        let r = g.d.religions.keys().next().unwrap();
        g.players[P0].state_religion_happiness = 2;
        g.players[P0].non_state_religion_happiness = -1;

        assert_eq!(g.city_religion_happiness(c, r), 0);
        g.cities[c].has_religion[r] = true;
        assert_eq!(g.city_religion_happiness(c, r), -1);
        g.city_update_religion_happiness(c);
        assert_eq!((g.cities[c].religion_good_happiness, g.cities[c].religion_bad_happiness), (0, -1));

        g.players[P0].last_state_religion = Some(r);
        g.players[P0].state_religion_count = 1;
        assert_eq!(g.city_religion_happiness(c, r), 2);
        g.cities[c].ai_assign_work_dirty = false;
        g.city_update_religion_happiness(c);
        assert_eq!((g.cities[c].religion_good_happiness, g.cities[c].religion_bad_happiness), (2, 0));
        assert!(g.cities[c].ai_assign_work_dirty);

        g.cities[c].state_religion_happiness[r] = 3;
        assert_eq!(g.city_current_state_religion_happiness(c), 3);
    }

    #[test]
    fn great_people_rate_modifier() {
        let (mut g, c) = test_city();
        g.cities[c].great_people_rate_modifier = 50;
        g.players[P0].great_people_rate_modifier = 25;
        assert_eq!(g.city_total_great_people_rate_modifier(c), 175);

        g.players[P0].golden_age_turns = 1;
        assert_eq!(g.city_total_great_people_rate_modifier(c), 175 + g.d.defines.golden_age_great_people_modifier);

        g.cities[c].great_people_rate_modifier = -1000;
        assert_eq!(g.city_total_great_people_rate_modifier(c), 0);
    }

    #[test]
    fn building_health_splits_good_and_bad() {
        let (mut g, c) = test_city();
        let d = g.d;
        let (b, info) = d.buildings.iter_enumerated().find(|(_, b)| b.health < 0 && b.building_class_type.is_some()).unwrap();

        assert_eq!(g.city_building_good_health(c, b), 0);
        assert_eq!(g.city_building_bad_health(c, b), info.health);
        g.players[P0].extra_building_health[b] = 3;
        assert_eq!(g.city_building_good_health(c, b), 3);
        assert_eq!(g.city_building_health(c, b), 3 + info.health);

        // Only healthy effects of buildings: the bad part disappears.
        g.cities[c].building_only_healthy_count = 1;
        assert_eq!(g.city_building_bad_health(c, b), 0);
        assert_eq!(g.city_building_health(c, b), 3);
    }
}
