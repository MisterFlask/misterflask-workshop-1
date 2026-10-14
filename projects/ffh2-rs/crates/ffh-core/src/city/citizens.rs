//! CvCity: city plots and citizens (worked plots, specialists), population and growth,
//! happiness, health, great people, FFH2 plot radius, and the player-settable emphasize /
//! growth-control state of CvCityAI (`AI_setEmphasize`, `AI_stopGrowth`, ...).
//!
//! Citizen *assignment* (`AI_assignWorkingPlots`, `AI_addBestCitizen`,
//! `AI_removeWorstCitizen`) is the governor (`crate::governor`), not ported here.

#![allow(unused_variables)]
#![allow(unused_imports)]
use super::{order_type_data, range, City};
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

impl Game {
    /// C++ `CvCity::createGreatPeople` (CvCity.cpp:1246-1249): forwards to
    /// `CvPlayer::createGreatPeople` at the city's plot.
    pub fn city_create_great_people(&mut self, c: CityId, unit: UnitType, increment_threshold: bool, increment_experience: bool) {
        todo!("CvCity::createGreatPeople")
    }

    /// C++ `CvCity::getCityPlotIndex` (CvCity.cpp:1365-1368, `plotCityXY`): index of `plot`
    /// in the city's 37-plot radius, C++ -1 = `None`.
    pub fn city_plot_index(&self, c: CityId, plot: PlotIdx) -> Option<usize> {
        todo!("CvCity::getCityPlotIndex")
    }

    /// C++ `CvCity::getCityIndexPlot` (CvCity.cpp:1371-1374, `plotCity`): the plot at city
    /// plot index `index` (`None` off the map).
    pub fn city_index_plot(&self, c: CityId, index: usize) -> Option<PlotIdx> {
        todo!("CvCity::getCityIndexPlot")
    }

    /// C++ `CvCity::canWork` (CvCity.cpp:1377-1431).
    pub fn city_can_work(&self, c: CityId, plot: PlotIdx) -> bool {
        todo!("CvCity::canWork")
    }

    /// C++ `CvCity::verifyWorkingPlot` (CvCity.cpp:1434-1455).
    pub fn city_verify_working_plot(&mut self, c: CityId, index: usize) {
        todo!("CvCity::verifyWorkingPlot")
    }

    /// C++ `CvCity::verifyWorkingPlots` (CvCity.cpp:1458-1466).
    pub fn city_verify_working_plots(&mut self, c: CityId) {
        for i in 0..NUM_CITY_PLOTS {
            self.city_verify_working_plot(c, i);
        }
    }

    /// C++ `CvCity::clearWorkingOverride` (CvCity.cpp:1469-1479).
    pub fn city_clear_working_override(&mut self, c: CityId, index: usize) {
        todo!("CvCity::clearWorkingOverride")
    }

    /// C++ `CvCity::countNumImprovedPlots` (CvCity.cpp:1482-1515). C++ defaults
    /// `eImprovement = NO_IMPROVEMENT` (any improvement), `bPotential = false`.
    pub fn city_count_num_improved_plots(&self, c: CityId, improvement: Option<ImprovementType>, potential: bool) -> i32 {
        todo!("CvCity::countNumImprovedPlots")
    }

    /// C++ `CvCity::countNumWaterPlots` (CvCity.cpp:1518-1543).
    pub fn city_count_num_water_plots(&self, c: CityId) -> i32 {
        todo!("CvCity::countNumWaterPlots")
    }

    /// C++ `CvCity::countNumRiverPlots` (CvCity.cpp:1545-1566).
    pub fn city_count_num_river_plots(&self, c: CityId) -> i32 {
        todo!("CvCity::countNumRiverPlots")
    }

    /// C++ `CvCity::canJoin` (CvCity.cpp:2422-2425).
    pub fn city_can_join(&self, c: CityId) -> bool {
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
        todo!("CvCity::processSpecialist")
    }

    /// C++ `CvCity::getOvercrowdingPercentAnger` (CvCity.cpp:4414-4429). C++ default
    /// `iExtra = 0`.
    pub fn city_overcrowding_percent_anger(&self, c: CityId, extra: i32) -> i32 {
        todo!("CvCity::getOvercrowdingPercentAnger")
    }

    /// C++ `CvCity::getNoMilitaryPercentAnger` (CvCity.cpp:4432-4444).
    pub fn city_no_military_percent_anger(&self, c: CityId) -> i32 {
        todo!("CvCity::getNoMilitaryPercentAnger")
    }

    /// C++ `CvCity::getCulturePercentAnger` (CvCity.cpp:4447-4486).
    pub fn city_culture_percent_anger(&self, c: CityId) -> i32 {
        todo!("CvCity::getCulturePercentAnger")
    }

    /// C++ `CvCity::getReligionPercentAnger` (CvCity.cpp:4489-4534).
    pub fn city_religion_percent_anger(&self, c: CityId) -> i32 {
        todo!("CvCity::getReligionPercentAnger")
    }

    /// C++ `CvCity::getHurryPercentAnger` (CvCity.cpp:4537-4545). C++ default `iExtra = 0`.
    pub fn city_hurry_percent_anger(&self, c: CityId, extra: i32) -> i32 {
        todo!("CvCity::getHurryPercentAnger")
    }

    /// C++ `CvCity::getConscriptPercentAnger` (CvCity.cpp:4548-4556). C++ default `iExtra = 0`.
    pub fn city_conscript_percent_anger(&self, c: CityId, extra: i32) -> i32 {
        todo!("CvCity::getConscriptPercentAnger")
    }

    /// C++ `CvCity::getDefyResolutionPercentAnger` (CvCity.cpp:4558-4566). C++ default
    /// `iExtra = 0`.
    pub fn city_defy_resolution_percent_anger(&self, c: CityId, extra: i32) -> i32 {
        todo!("CvCity::getDefyResolutionPercentAnger")
    }

    /// C++ `CvCity::getWarWearinessPercentAnger` (CvCity.cpp:4569-4579).
    pub fn city_war_weariness_percent_anger(&self, c: CityId) -> i32 {
        todo!("CvCity::getWarWearinessPercentAnger")
    }

    /// C++ `CvCity::getLargestCityHappiness` (CvCity.cpp:4582-4590). Uses the population
    /// rank cache (`city_find_population_rank`).
    pub fn city_largest_city_happiness(&self, c: CityId) -> i32 {
        todo!("CvCity::getLargestCityHappiness")
    }

    /// C++ `CvCity::getVassalHappiness` (CvCity.cpp:4592-4608).
    pub fn city_vassal_happiness(&self, c: CityId) -> i32 {
        todo!("CvCity::getVassalHappiness")
    }

    /// C++ `CvCity::getVassalUnhappiness` (CvCity.cpp:4610-4626).
    pub fn city_vassal_unhappiness(&self, c: CityId) -> i32 {
        todo!("CvCity::getVassalUnhappiness")
    }

    /// C++ `CvCity::unhappyLevel` (CvCity.cpp:4629-4675). C++ default `iExtra = 0`.
    pub fn city_unhappy_level(&self, c: CityId, extra: i32) -> i32 {
        todo!("CvCity::unhappyLevel")
    }

    /// C++ `CvCity::happyLevel` (CvCity.cpp:4678-4706).
    pub fn city_happy_level(&self, c: CityId) -> i32 {
        todo!("CvCity::happyLevel")
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
        todo!("CvCity::totalFreeSpecialists")
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
        todo!("CvCity::totalGoodBuildingHealth")
    }

    /// C++ `CvCity::totalBadBuildingHealth` (CvCity.cpp:4781-4789).
    pub fn city_total_bad_building_health(&self, c: CityId) -> i32 {
        todo!("CvCity::totalBadBuildingHealth")
    }

    /// C++ `CvCity::goodHealth` (CvCity.cpp:4792-4842).
    pub fn city_good_health(&self, c: CityId) -> i32 {
        todo!("CvCity::goodHealth")
    }

    /// C++ `CvCity::badHealth` (CvCity.cpp:4845-4907). C++ defaults `bNoAngry = false`,
    /// `iExtra = 0`.
    pub fn city_bad_health(&self, c: CityId, no_angry: bool, extra: i32) -> i32 {
        todo!("CvCity::badHealth")
    }

    /// C++ `CvCity::healthRate` (CvCity.cpp:4910-4913). C++ defaults `bNoAngry = false`,
    /// `iExtra = 0`.
    pub fn city_health_rate(&self, c: CityId, no_angry: bool, extra: i32) -> i32 {
        (self.city_good_health(c) - self.city_bad_health(c, no_angry, extra)).min(0)
    }

    /// C++ `CvCity::foodConsumption` (CvCity.cpp:4916-4932). FFH: civics can change the food
    /// eaten per citizen. C++ defaults `bNoAngry = false`, `iExtra = 0`.
    pub fn city_food_consumption(&self, c: CityId, no_angry: bool, extra: i32) -> i32 {
        todo!("CvCity::foodConsumption")
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
        todo!("CvCity::growthThreshold")
    }

    /// C++ `CvCity::setPopulation` (CvCity.cpp:5475-5537). `AI_setChooseProductionDirty` /
    /// `AI_makeAssignWorkDirty` are bookkeeping fields (`ai_choose_production_dirty`, the
    /// owner's cities' `ai_assign_work_dirty`).
    pub fn city_set_population(&mut self, c: CityId, new_value: i32) {
        todo!("CvCity::setPopulation")
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
        todo!("CvCity::changeSpecialistPopulation")
    }

    /// C++ `CvCity::changeNumGreatPeople` (CvCity.cpp:5603-5612).
    pub fn city_change_num_great_people(&mut self, c: CityId, change: i32) {
        todo!("CvCity::changeNumGreatPeople")
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
        todo!("CvCity::getTotalGreatPeopleRateModifier")
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
        todo!("CvCity::updateFreshWaterHealth")
    }

    /// C++ `CvCity::updateFeatureHealth` (CvCity.cpp:6199-6249).
    pub fn city_update_feature_health(&mut self, c: CityId) {
        todo!("CvCity::updateFeatureHealth")
    }

    /// C++ `CvCity::getBuildingHealth(BuildingTypes)` (CvCity.cpp:6264-6274).
    pub fn city_building_health(&self, c: CityId, building: BuildingType) -> i32 {
        todo!("CvCity::getBuildingHealth")
    }

    /// C++ `CvCity::getBuildingGoodHealth(BuildingTypes)` (CvCity.cpp:6276-6283). (The no-arg
    /// overload is the field `building_good_health`.)
    pub fn city_building_good_health(&self, c: CityId, building: BuildingType) -> i32 {
        todo!("CvCity::getBuildingGoodHealth")
    }

    /// C++ `CvCity::getBuildingBadHealth(BuildingTypes)` (CvCity.cpp:6285-6297). (The no-arg
    /// overload is the field `building_bad_health`.)
    pub fn city_building_bad_health(&self, c: CityId, building: BuildingType) -> i32 {
        todo!("CvCity::getBuildingBadHealth")
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
        todo!("CvCity::updatePowerHealth")
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
        todo!("CvCity::getMilitaryHappiness")
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
        todo!("CvCity::getBuildingHappiness")
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
        todo!("CvCity::updateExtraBuildingHappiness")
    }

    /// C++ `CvCity::updateExtraBuildingHealth` (CvCity.cpp:6600-6636).
    pub fn city_update_extra_building_health(&mut self, c: CityId) {
        todo!("CvCity::updateExtraBuildingHealth")
    }

    /// C++ `CvCity::updateFeatureHappiness` (CvCity.cpp:6651-6709).
    pub fn city_update_feature_happiness(&mut self, c: CityId) {
        todo!("CvCity::updateFeatureHappiness")
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
        todo!("CvCity::getReligionHappiness")
    }

    /// C++ `CvCity::updateReligionHappiness` (CvCity.cpp:6782-6821).
    pub fn city_update_religion_happiness(&mut self, c: CityId) {
        todo!("CvCity::updateReligionHappiness")
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
    pub fn city_flat_defy_resolution_anger_length(&self, c: CityId) -> i32 {
        todo!("CvCity::flatDefyResolutionAngerLength")
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
        todo!("CvCity::isNoUnhealthyPopulation")
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
        todo!("CvCity::isBuildingOnlyHealthy")
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
        todo!("CvCity::setWeLoveTheKingDay")
    }

    /// C++ `CvCity::setCitizensAutomated` (CvCity.cpp:7683-7708). Automating calls the
    /// governor (`AI_assignWorkingPlots`).
    pub fn city_set_citizens_automated(&mut self, c: CityId, new_value: bool) {
        todo!("CvCity::setCitizensAutomated")
    }

    /// C++ `CvCity::getCommerceHappinessByType` (CvCity.cpp:9014-9017).
    pub fn city_commerce_happiness_by_type(&self, c: CityId, commerce: CommerceType) -> i32 {
        todo!("CvCity::getCommerceHappinessByType")
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
        todo!("CvCity::setGreatPeopleUnitRate")
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
        todo!("CvCity::alterSpecialistCount")
    }

    /// C++ `CvCity::isSpecialistValid` (CvCity.cpp:10026-10029). C++ default `iExtra = 0`.
    pub fn city_is_specialist_valid(&self, c: CityId, specialist: SpecialistType, extra: i32) -> bool {
        todo!("CvCity::isSpecialistValid")
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
        todo!("CvCity::getAddedFreeSpecialistCount")
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
        todo!("CvCity::getCurrentStateReligionHappiness")
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
        todo!("CvCity::setWorkingPlot")
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
        todo!("CvCity::alterWorkingPlot")
    }

    /// C++ `CvCity::doGrowth` (CvCity.cpp:11743-11800). Python `doGrowth` callback; growth is
    /// halted by `AI_stopGrowth` (-> `city_ai_stop_growth`). Python event `cityGrowth`.
    pub fn city_do_growth(&mut self, c: CityId) {
        todo!("CvCity::doGrowth")
    }

    /// C++ `CvCity::doGreatPeople` (CvCity.cpp:12218-12279). Python `doGreatPeople` callback;
    /// draws "Great Person".
    pub fn city_do_great_people(&mut self, c: CityId) {
        todo!("CvCity::doGreatPeople")
    }

    /// C++ `CvCity::getBestYieldAvailable` (CvCity.cpp:14344-14380).
    pub fn city_best_yield_available(&self, c: CityId, yield_type: YieldType) -> i32 {
        todo!("CvCity::getBestYieldAvailable")
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
        todo!("CvCity::setPlotRadius")
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
        todo!("CvCityAI::AI_setEmphasize")
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
}
