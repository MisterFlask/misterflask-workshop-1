//! CvCity: buildings (`processBuilding`, real/free building counts, per-class yield /
//! commerce / happiness / health changes), bonuses, corporations, power, defense and
//! maintenance.

use super::range;
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::utils::{get_wonder_score, is_national_wonder_class, is_team_wonder_class, is_world_wonder_class};
use ffh_data::*;

/// C++ `(BuildingClassTypes)GC.getBuildingInfo(eBuilding).getBuildingClassType()`: every
/// building info has a class (C++ would index arrays with -1 otherwise).
fn building_class_of(d: &GameData, building: BuildingType) -> BuildingClassType {
    d.buildings[building].building_class_type.expect("building without a building class")
}

/// C++ `for (i = 0; i < GC.getNUM_CORPORATION_PREREQ_BONUSES(); ++i)` over
/// `getPrereqBonus(i)`, skipping `NO_BONUS`: `prereq_bonus` holds the listed bonuses in slot
/// order without the empty slots, so the first `NUM_CORPORATION_PREREQ_BONUSES` entries are
/// exactly the bonuses the C++ loop visits, in the same order.
fn corporation_prereq_bonuses(d: &'static GameData, corporation: CorporationType) -> impl Iterator<Item = BonusType> {
    let num = d.defines.num_corporation_prereq_bonuses.max(0) as usize;
    d.corporations[corporation].prereq_bonus.iter().copied().take(num)
}

/// C++ `CvPlayerAI::AI_chooseFreeTech` (called by `setNumRealBuildingTimed` for AI owners):
/// clears the research queue, picks a tech (C++: Python `AI_chooseTech`, else
/// `AI_bestTech(1, true)`; here the owner's controller decides) and gives it to the team.
fn ai_choose_free_tech(g: &mut Game, p: PlayerId) {
    g.player_clear_research_queue(p);

    let best_tech = g.controller(p).choose_free_tech(g, p);

    if let Some(tech) = best_tech {
        let team = g.players[p].team;
        g.team_set_has_tech(team, tech, true, Some(p), true, true);
    }
}

impl Game {
    /// C++ `CvCity::getBonusHealth` (CvCity.cpp:3797-3810).
    pub fn city_bonus_health(&self, c: CityId, bonus: BonusType) -> i32 {
        let d = self.d;
        let mut health = d.bonuses[bonus].health;

        for building in d.buildings.keys() {
            health += self.city_num_active_building(c, building) * d.buildings[building].bonus_health_changes[bonus];
        }

        health
    }

    /// C++ `CvCity::getBonusHappiness` (CvCity.cpp:3813-3826).
    pub fn city_bonus_happiness(&self, c: CityId, bonus: BonusType) -> i32 {
        let d = self.d;
        let mut happiness = d.bonuses[bonus].happiness;

        for building in d.buildings.keys() {
            happiness += self.city_num_active_building(c, building) * d.buildings[building].bonus_happiness_changes[bonus];
        }

        happiness
    }

    /// C++ `CvCity::getBonusPower` (CvCity.cpp:3829-3851).
    pub fn city_bonus_power(&self, c: CityId, bonus: BonusType, dirty: bool) -> i32 {
        let d = self.d;
        let mut count = 0;

        for building in d.buildings.keys() {
            if self.city_num_active_building(c, building) > 0 {
                let info = &d.buildings[building];
                if info.power_bonus == Some(bonus) {
                    if info.dirty_power == dirty {
                        count += self.city_num_active_building(c, building);
                    }
                }
            }
        }

        count
    }

    /// C++ `CvCity::getBonusYieldRateModifier(YieldTypes, BonusTypes)` (CvCity.cpp:3854-3867).
    /// (The one-argument overload is the field `bonus_yield_rate_modifier[y]`.)
    pub fn city_bonus_yield_rate_modifier(&self, c: CityId, yield_type: YieldType, bonus: BonusType) -> i32 {
        let d = self.d;
        let mut modifier = 0;

        for building in d.buildings.keys() {
            modifier += self.city_num_active_building(c, building) * d.buildings[building].bonus_yield_modifier[bonus][yield_type.idx()];
        }

        modifier
    }

    /// C++ `CvCity::processBonus` (CvCity.cpp:3870-3972). FFH: bonus GP/maintenance/mutation/
    /// research modifiers, capital-only player effects, free promotions.
    pub fn city_process_bonus(&mut self, c: CityId, bonus: BonusType, change: i32) {
        let d = self.d;
        let bonus_info = &d.bonuses[bonus];

        let value = bonus_info.health;
        let mut good_value = value.max(0);
        let mut bad_value = value.min(0);

        for building in d.buildings.keys() {
            let value = d.buildings[building].bonus_health_changes[bonus] * self.city_num_active_building(c, building);

            if value >= 0 {
                good_value += value;
            } else {
                bad_value += value;
            }
        }

        self.city_change_bonus_good_health(c, good_value * change);
        self.city_change_bonus_bad_health(c, bad_value * change);

        let value = bonus_info.happiness;
        let mut good_value = value.max(0);
        let mut bad_value = value.min(0);

        for building in d.buildings.keys() {
            let value = self.city_num_active_building(c, building) * d.buildings[building].bonus_happiness_changes[bonus];

            if value >= 0 {
                good_value += value;
            } else {
                bad_value += value;
            }
        }

        self.city_change_bonus_good_happiness(c, good_value * change);
        self.city_change_bonus_bad_happiness(c, bad_value * change);

        let dirty_power = self.city_bonus_power(c, bonus, true) * change;
        self.city_change_power_count(c, dirty_power, true);
        let clean_power = self.city_bonus_power(c, bonus, false) * change;
        self.city_change_power_count(c, clean_power, false);

        // FfH: Added by Kael 01/27/2008
        if bonus_info.great_people_rate_modifier != 0 {
            // changeGreatPeopleRateModifier
            self.cities[c].great_people_rate_modifier += bonus_info.great_people_rate_modifier * change;
        }
        if bonus_info.maintenance_modifier != 0 {
            self.city_change_maintenance_modifier(c, bonus_info.maintenance_modifier * change);
        }
        if bonus_info.mutate_chance != 0 {
            // changeMutateChance
            self.cities[c].mutate_chance += bonus_info.mutate_chance * change;
        }
        if bonus_info.research_modifier != 0 {
            self.city_change_commerce_rate_modifier(c, CommerceType::COMMERCE_RESEARCH, bonus_info.research_modifier * change);
        }
        if self.city_is_capital(c) {
            let owner = self.cities[c].owner;
            if bonus_info.discover_rand_modifier != 0 {
                self.players[owner].discover_rand_modifier += bonus_info.discover_rand_modifier * change;
            }
            if bonus_info.heal_change != 0 {
                self.players[owner].heal_change += bonus_info.heal_change * change;
            }
            if bonus_info.heal_change_enemy != 0 {
                self.players[owner].heal_change_enemy += bonus_info.heal_change_enemy * change;
            }
        }
        if let Some(promotion) = bonus_info.free_promotion {
            // C++ quirk: the sign comes from `hasBonus` at call time, not from `iChange`:
            // `changeNoBonusCount` calls processBonus(-1) while the city still has the bonus,
            // which adds the promotion instead of removing it, and a `ModifierPerBonus` bonus
            // adds one for every copy gained or lost while the city keeps the bonus.
            if self.city_has_bonus(c, bonus) {
                self.cities[c].free_promotion_count[promotion] += 1;
            } else {
                self.cities[c].free_promotion_count[promotion] -= 1;
            }
        }
        // FfH: End Add

        for i in 0..YieldType::NUM {
            let yield_type = YieldType::from_usize(i);
            let modifier = self.city_bonus_yield_rate_modifier(c, yield_type, bonus) * change;
            self.city_change_bonus_yield_rate_modifier(c, yield_type, modifier);
        }
    }

    /// C++ `CvCity::processBuilding` (CvCity.cpp:3975-4206): applies (`change` > 0) or removes
    /// a building's effects to the city, its owner, team, area and the game. C++ default
    /// `bObsolete = false`.
    pub fn city_process_building(&mut self, c: CityId, building: BuildingType, change: i32, obsolete: bool) {
        let d = self.d;
        let info = &d.buildings[building];
        let building_class = building_class_of(d, building);

        if !self.team_is_obsolete_building(self.city_team(c), building) || obsolete {
            if change > 0 {
                if let Some(corporation) = info.founds_corporation {
                    if !self.is_corporation_founded(corporation) {
                        self.city_set_headquarters(c, corporation);
                    }
                }
            }

            if let Some(bonus) = info.no_bonus {
                self.city_change_no_bonus_count(c, bonus, change);
            }

            if let Some(bonus) = info.free_bonus {
                let num_free_bonuses = self.num_free_bonuses(building);
                self.city_change_free_bonus(c, bonus, num_free_bonuses * change);
            }

            // FfH: Added by Kael 08/04/2007
            // changeCrime
            self.cities[c].crime += info.crime * change;
            // changeUnhappyProduction(iChange * isUnhappyProduction())
            self.cities[c].unhappy_production += change * i32::from(info.unhappy_production);
            if let Some(bonus) = info.free_bonus2 {
                let num_free_bonuses = self.num_free_bonuses(building);
                self.city_change_free_bonus(c, bonus, num_free_bonuses * change);
            }
            if let Some(bonus) = info.free_bonus3 {
                let num_free_bonuses = self.num_free_bonuses(building);
                self.city_change_free_bonus(c, bonus, num_free_bonuses * change);
            }

            // C++ quirk: not multiplied by `iChange`, so removing (or re-processing) the
            // building moves the global counter the same way as adding it.
            self.change_global_counter(info.modify_global_counter);
            if info.resist_magic != 0 {
                // changeResistMagic
                self.cities[c].resist_magic += info.resist_magic * change;
            }
            if info.plot_radius != 0 {
                if change > 0 {
                    self.city_set_plot_radius(c, info.plot_radius);
                } else {
                    self.city_set_plot_radius(c, 2);
                }
            }
            // FfH: End Add

            if let Some(promotion) = info.free_promotion {
                // changeFreePromotionCount
                self.cities[c].free_promotion_count[promotion] += change;
            }

            {
                let city = &mut self.cities[c];
                // changeEspionageDefenseModifier, changeGreatPeopleRateModifier,
                // changeFreeExperience, changeMaxFoodKeptPercent, changeMaxAirlift,
                // changeAirModifier, changeAirUnitCapacity, changeNukeModifier
                city.espionage_defense_modifier += info.espionage_defense_modifier * change;
                city.great_people_rate_modifier += info.great_people_rate_modifier * change;
                city.free_experience += info.free_experience * change;
                city.max_food_kept_percent += info.food_kept * change;
                city.max_airlift += info.airlift * change;
                city.air_modifier += info.air_modifier * change;
                city.air_unit_capacity += info.air_unit_capacity * change;
                city.nuke_modifier += info.nuke_modifier * change;
            }
            self.city_change_free_specialist(c, info.free_specialist * change);
            self.city_change_maintenance_modifier(c, info.maintenance_modifier * change);
            self.city_change_war_weariness_modifier(c, info.war_weariness_modifier * change);
            self.city_change_hurry_anger_modifier(c, info.hurry_anger_modifier * change);
            // changeHealRate
            self.cities[c].heal_rate += info.heal_rate_change * change;
            if info.health > 0 {
                self.city_change_building_good_health(c, info.health * change);
            } else {
                self.city_change_building_bad_health(c, info.health * change);
            }
            if info.happiness > 0 {
                self.city_change_building_good_happiness(c, info.happiness * change);
            } else {
                self.city_change_building_bad_happiness(c, info.happiness * change);
            }
            if let Some(religion) = info.religion_type {
                self.city_change_state_religion_happiness(c, religion, info.state_religion_happiness * change);
            }
            // changeMilitaryProductionModifier, changeSpaceProductionModifier
            self.cities[c].military_production_modifier += info.military_production_modifier * change;
            self.cities[c].space_production_modifier += info.space_production_modifier * change;
            self.city_change_extra_trade_routes(c, info.trade_routes * change);
            self.city_change_trade_route_modifier(c, info.trade_route_modifier * change);
            self.city_change_foreign_trade_route_modifier(c, info.foreign_trade_route_modifier * change);
            self.city_change_power_count(c, if info.power { change } else { 0 }, info.dirty_power);
            self.city_change_government_center_count(c, if info.government_center { change } else { 0 });
            self.city_change_no_unhappiness_count(c, if info.no_unhappiness { change } else { 0 });
            self.city_change_no_unhealthy_population_count(c, if info.no_unhealthy_population { change } else { 0 });
            self.city_change_building_only_healthy_count(c, if info.building_only_healthy { change } else { 0 });

            for i in 0..YieldType::NUM {
                let yield_type = YieldType::from_usize(i);
                self.city_change_sea_plot_yield(c, yield_type, info.sea_plot_yield_change[i] * change);
                self.city_change_river_plot_yield(c, yield_type, info.river_plot_yield_change[i] * change);
                let base_change = (info.yield_change[i] + self.city_building_yield_change(c, building_class, yield_type)) * change;
                self.city_change_base_yield_rate(c, yield_type, base_change);
                self.city_change_yield_rate_modifier(c, yield_type, info.yield_modifier[i] * change);
                self.city_change_power_yield_rate_modifier(c, yield_type, info.power_yield_modifier[i] * change);
            }

            for i in 0..CommerceType::NUM {
                let commerce = CommerceType::from_usize(i);
                self.city_change_commerce_rate_modifier(c, commerce, info.commerce_modifier[i] * change);
                self.city_change_commerce_happiness_per(c, commerce, info.commerce_happiness[i] * change);
            }

            for religion in d.religions.keys() {
                // changeReligionInfluence
                self.cities[c].religion_influence[religion] += info.religion_change[religion] * change;
            }

            for specialist in d.specialists.keys() {
                self.city_change_max_specialist_count(c, specialist, info.specialist_count[specialist] * change);
                self.city_change_free_specialist_count(c, specialist, info.free_specialist_count[specialist] * change);
            }

            for improvement in d.improvements.keys() {
                self.city_change_improvement_free_specialists(c, improvement, info.improvement_free_specialist[improvement] * change);
            }

            for bonus in d.bonuses.keys() {
                if self.city_has_bonus(c, bonus) {
                    if info.bonus_health_changes[bonus] > 0 {
                        self.city_change_bonus_good_health(c, info.bonus_health_changes[bonus] * change);
                    } else {
                        self.city_change_bonus_bad_health(c, info.bonus_health_changes[bonus] * change);
                    }
                    if info.bonus_happiness_changes[bonus] > 0 {
                        self.city_change_bonus_good_happiness(c, info.bonus_happiness_changes[bonus] * change);
                    } else {
                        self.city_change_bonus_bad_happiness(c, info.bonus_happiness_changes[bonus] * change);
                    }

                    if info.power_bonus == Some(bonus) {
                        self.city_change_power_count(c, change, info.dirty_power);
                    }

                    for j in 0..YieldType::NUM {
                        self.city_change_bonus_yield_rate_modifier(c, YieldType::from_usize(j), info.bonus_yield_modifier[bonus][j] * change);
                    }
                }
            }

            for unit_combat in d.unit_combats.keys() {
                // changeUnitCombatFreeExperience
                self.cities[c].unit_combat_free_experience[unit_combat] += info.unit_combat_free_experience[unit_combat] * change;
            }

            for i in 0..DomainType::NUM {
                let domain = DomainType::from_usize(i);
                // changeDomainFreeExperience, changeDomainProductionModifier
                self.cities[c].domain_free_experience[i] += info.domain_free_experience[domain] * change;
                self.cities[c].domain_production_modifier[i] += info.domain_production_modifier[domain] * change;
            }

            self.city_update_extra_building_happiness(c);
            self.city_update_extra_building_health(c);

            let owner = self.cities[c].owner;
            // CvPlayer::changeAssets
            self.players[owner].assets += info.asset_value * change;

            // CvArea::changePower, CvPlayer::changePower
            let area = self.city_area(c);
            self.map.areas[area].power[owner] += info.power_value * change;
            self.players[owner].power += info.power_value * change;

            let team = self.city_team(c);
            for p in self.player_ids() {
                if self.players[p].team == team {
                    if info.team_share || p == owner {
                        let area = self.city_area(c);
                        self.player_process_building(p, building, change, area);
                    }
                }
            }

            self.team_process_building(team, building, change);

            self.process_building(building, change);
        }

        if !obsolete {
            // changeBuildingDefense, changeBuildingBombardDefense (UI dirty bits aside)
            self.cities[c].building_defense += info.defense_modifier * change;
            self.cities[c].building_bombard_defense += info.bombard_defense_modifier * change;

            // changeBaseGreatPeopleRate
            self.cities[c].base_great_people_rate += info.great_people_rate_change * change;

            if let Some(unit_class) = info.great_people_unit_class {
                let great_people_unit = d.civ_unit(self.city_civilization_type(c), unit_class);

                if let Some(great_people_unit) = great_people_unit {
                    self.city_change_great_people_unit_rate(c, great_people_unit, info.great_people_rate_change * change);
                }
            }

            // CvTeam::changeBuildingClassCount, CvPlayer::changeBuildingClassCount
            let team = self.city_team(c);
            let owner = self.cities[c].owner;
            self.teams[team].building_class_count[building_class] += change;
            self.players[owner].building_class_count[building_class] += change;

            self.player_change_wonders_score(owner, get_wonder_score(d, building_class) * change);
        }

        self.city_update_building_commerce(c);

        // UI: setLayoutDirty(true).
    }

    /// C++ `CvCity::isHeadquarters(CorporationTypes)` (CvCity.cpp:4378-4381).
    pub fn city_is_headquarters(&self, c: CityId, corporation: CorporationType) -> bool {
        self.get_headquarters(corporation) == Some(c)
    }

    /// C++ `CvCity::setHeadquarters` (CvCity.cpp:4383-4396): also grants the corporation's
    /// free unit.
    pub fn city_set_headquarters(&mut self, c: CityId, corporation: CorporationType) {
        self.set_headquarters(corporation, Some(c), true);

        if let Some(unit_class) = self.d.corporations[corporation].free_unit_class {
            let free_unit = self.d.civ_unit(self.city_civilization_type(c), unit_class);

            if let Some(free_unit) = free_unit {
                let (owner, x, y) = {
                    let city = &self.cities[c];
                    (city.owner, city.x, city.y)
                };
                self.player_init_unit(owner, free_unit, x, y, None, None);
            }
        }
    }

    /// C++ `CvCity::isHeadquarters()` (CvCity.cpp:4398-4411): headquarters of any corporation.
    pub fn city_is_any_headquarters(&self, c: CityId) -> bool {
        for corporation in self.d.corporations.keys() {
            if self.city_is_headquarters(c, corporation) {
                return true;
            }
        }

        false
    }

    /// C++ `CvCity::getNumBuilding` (CvCity.cpp:5242-5247): real + free, capped at
    /// `CITY_MAX_NUM_BUILDINGS`.
    pub fn city_num_building(&self, c: CityId, building: BuildingType) -> i32 {
        let city = &self.cities[c];
        std::cmp::min(self.d.defines.city_max_num_buildings, city.num_real_building[building] + city.num_free_building[building])
    }

    /// C++ `CvCity::getNumActiveBuilding` (CvCity.cpp:5250-5260): 0 if obsolete for the team.
    pub fn city_num_active_building(&self, c: CityId, building: BuildingType) -> i32 {
        if self.team_is_obsolete_building(self.city_team(c), building) {
            return 0;
        }

        self.city_num_building(c, building)
    }

    /// C++ `CvCity::hasActiveWorldWonder` (CvCity.cpp:5263-5279).
    pub fn city_has_active_world_wonder(&self, c: CityId) -> bool {
        let d = self.d;

        for building in d.buildings.keys() {
            if is_world_wonder_class(d, building_class_of(d, building)) {
                if self.cities[c].num_real_building[building] > 0 && !self.team_is_obsolete_building(self.city_team(c), building) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvCity::getCorporationCount` (CvCity.cpp:5300-5316).
    pub fn city_corporation_count(&self, c: CityId) -> i32 {
        self.cities[c].has_corporation.iter().filter(|&&has| has).count() as i32
    }

    /// C++ `CvCity::isGovernmentCenter` (CvCity.cpp:5747-5750).
    pub fn city_is_government_center(&self, c: CityId) -> bool {
        self.cities[c].government_center_count > 0
    }

    /// C++ `CvCity::changeGovernmentCenterCount` (CvCity.cpp:5753-5762).
    pub fn city_change_government_center_count(&mut self, c: CityId, change: i32) {
        if change != 0 {
            self.cities[c].government_center_count += change;

            let owner = self.cities[c].owner;
            self.player_update_maintenance(owner);
        }
    }

    /// C++ `CvCity::getMaintenance` (CvCity.cpp:5765-5768). (`getMaintenanceTimes100` is the
    /// field `maintenance`.)
    pub fn city_maintenance(&self, c: CityId) -> i32 {
        self.cities[c].maintenance / 100
    }

    /// C++ `CvCity::updateMaintenance` (CvCity.cpp:5776-5800).
    pub fn city_update_maintenance(&mut self, c: CityId) {
        let old_maintenance = self.cities[c].maintenance;

        let mut new_maintenance = 0;

        if !self.city_is_disorder(c) && !self.cities[c].we_love_the_king_day && self.cities[c].population > 0 {
            new_maintenance = (self.city_calculate_base_maintenance_times100(c) * std::cmp::max(0, self.cities[c].maintenance_modifier + 100)) / 100;
        }

        if old_maintenance != new_maintenance {
            self.cities[c].maintenance = new_maintenance;

            // CvPlayer::changeTotalMaintenance
            let owner = self.cities[c].owner;
            self.players[owner].total_maintenance += self.cities[c].maintenance - old_maintenance;
        }
    }

    /// C++ `CvCity::calculateDistanceMaintenance` (CvCity.cpp:5802-5805).
    pub fn city_calculate_distance_maintenance(&self, c: CityId) -> i32 {
        self.city_calculate_distance_maintenance_times100(c) / 100
    }

    /// C++ `CvCity::calculateDistanceMaintenanceTimes100` (CvCity.cpp:5807-5855).
    pub fn city_calculate_distance_maintenance_times100(&self, c: CityId) -> i32 {
        let d = self.d;
        let city = &self.cities[c];
        let owner = city.owner;
        let world = &d.worlds[self.setup.world_size];
        let handicap = &d.handicaps[self.city_handicap_type(c)];

        let mut worst_city_maintenance = 0;
        let mut best_capital_maintenance = MAX_INT;

        for &loop_city in &self.players[owner].cities {
            let (loop_x, loop_y) = (self.cities[loop_city].x, self.cities[loop_city].y);
            let mut temp_maintenance = 100 * (d.defines.max_distance_city_maintenance * self.plot_distance(city.x, city.y, loop_x, loop_y));

            temp_maintenance *= city.population + 7;
            temp_maintenance /= 10;

            temp_maintenance *= std::cmp::max(0, self.players[owner].distance_maintenance_modifier + 100);
            temp_maintenance /= 100;

            temp_maintenance *= world.distance_maintenance_percent;
            temp_maintenance /= 100;

            temp_maintenance *= handicap.distance_maintenance_percent;
            temp_maintenance /= 100;

            temp_maintenance /= self.map.max_plot_distance();

            worst_city_maintenance = std::cmp::max(worst_city_maintenance, temp_maintenance);

            if self.city_is_government_center(loop_city) {
                best_capital_maintenance = std::cmp::min(best_capital_maintenance, temp_maintenance);
            }
        }

        let mut temp_maintenance = std::cmp::min(worst_city_maintenance, best_capital_maintenance);

        // FfH: Added by Kael 11/18/2008
        if city.settlement || self.is_option(GameOptionType::GAMEOPTION_NO_MAINTENANCE) {
            temp_maintenance = 0;
        }
        // FfH: End Add

        temp_maintenance
    }

    /// C++ `CvCity::calculateNumCitiesMaintenance` (CvCity.cpp:5857-5860).
    pub fn city_calculate_num_cities_maintenance(&self, c: CityId) -> i32 {
        self.city_calculate_num_cities_maintenance_times100(c) / 100
    }

    /// C++ `CvCity::calculateNumCitiesMaintenanceTimes100` (CvCity.cpp:5862-5905).
    pub fn city_calculate_num_cities_maintenance_times100(&self, c: CityId) -> i32 {
        let d = self.d;
        let owner = self.cities[c].owner;
        let team = self.city_team(c);
        let handicap = &d.handicaps[self.city_handicap_type(c)];

        let mut num_cities_percent = 100;

        num_cities_percent *= self.cities[c].population + 17;
        num_cities_percent /= 18;

        num_cities_percent *= d.worlds[self.setup.world_size].num_cities_maintenance_percent;
        num_cities_percent /= 100;

        num_cities_percent *= handicap.num_cities_maintenance_percent;
        num_cities_percent /= 100;

        let mut num_vassal_cities = 0;
        for p in self.civ_player_ids() {
            let loop_team = self.players[p].team;
            if loop_team != team && self.teams[loop_team].vassal[team] {
                num_vassal_cities += self.players[p].num_cities();
            }
        }
        num_vassal_cities /= std::cmp::max(1, self.teams[team].num_members);

        // FfH: Modified by Kael 03/29/2009 (settlements do not count)
        let mut num_cities_maintenance =
            ((self.players[owner].num_cities() - self.player_get_num_settlements(owner)) + num_vassal_cities) * num_cities_percent;
        // FfH: End Modify

        num_cities_maintenance = std::cmp::min(num_cities_maintenance, handicap.max_num_cities_maintenance * 100);
        num_cities_maintenance *= std::cmp::max(0, self.players[owner].num_cities_maintenance_modifier + 100);
        num_cities_maintenance /= 100;

        // FfH: Added by Kael 11/18/2008
        if self.cities[c].settlement || self.is_option(GameOptionType::GAMEOPTION_NO_MAINTENANCE) {
            num_cities_maintenance = 0;
        }
        // FfH: End Add

        num_cities_maintenance
    }

    /// C++ `CvCity::calculateColonyMaintenance` (CvCity.cpp:5908-5911).
    pub fn city_calculate_colony_maintenance(&self, c: CityId) -> i32 {
        self.city_calculate_colony_maintenance_times100(c) / 100
    }

    /// C++ `CvCity::calculateColonyMaintenanceTimes100` (CvCity.cpp:5913-5959). FFH: no
    /// colony maintenance on a landmass with any government center of the owner.
    pub fn city_calculate_colony_maintenance_times100(&self, c: CityId) -> i32 {
        if self.is_option(GameOptionType::GAMEOPTION_NO_VASSAL_STATES) {
            return 0;
        }

        let d = self.d;
        let owner = self.cities[c].owner;
        let area = self.city_area(c);

        // FfH: Modified by Kael 10/09/2008 (colony maintenance is determined by the closest
        // palace: none on a landmass with any of the owner's government centers)
        for &loop_city in &self.players[owner].cities {
            if self.city_is_government_center(loop_city) {
                if self.city_area(loop_city) == area {
                    return 0;
                }
            }
        }
        // FfH: End Modify

        let handicap = &d.handicaps[self.city_handicap_type(c)];

        let mut num_cities_percent = 100;

        num_cities_percent *= self.cities[c].population + 17;
        num_cities_percent /= 18;

        num_cities_percent *= d.worlds[self.setup.world_size].colony_maintenance_percent;
        num_cities_percent /= 100;

        num_cities_percent *= handicap.colony_maintenance_percent;
        num_cities_percent /= 100;

        let num_cities = (self.map.areas[area].cities_per_player[owner] - 1) * num_cities_percent;

        let mut maintenance = (num_cities * num_cities) / 100;
        maintenance = std::cmp::min(maintenance, (handicap.max_colony_maintenance * self.city_calculate_distance_maintenance_times100(c)) / 100);

        maintenance
    }

    /// C++ `CvCity::calculateCorporationMaintenance` (CvCity.cpp:5962-5965).
    pub fn city_calculate_corporation_maintenance(&self, c: CityId) -> i32 {
        self.city_calculate_corporation_maintenance_times100(c) / 100
    }

    /// C++ `CvCity::calculateCorporationMaintenanceTimes100()` (CvCity.cpp:5967-5982).
    pub fn city_calculate_corporation_maintenance_times100(&self, c: CityId) -> i32 {
        let mut maintenance = 0;
        for corporation in self.d.corporations.keys() {
            if self.city_is_active_corporation(c, corporation) {
                maintenance += self.city_calculate_corporation_maintenance_times100_for(c, corporation);
            }
        }
        maintenance
    }

    /// C++ `CvCity::calculateCorporationMaintenanceTimes100(CorporationTypes)`
    /// (CvCity.cpp:5984-6027).
    pub fn city_calculate_corporation_maintenance_times100_for(&self, c: CityId, corporation: CorporationType) -> i32 {
        let d = self.d;
        let info = &d.corporations[corporation];
        let owner = self.cities[c].owner;

        let mut maintenance = 0;

        for commerce in 0..CommerceType::NUM {
            maintenance += 100 * info.headquarter_commerce[commerce];
        }

        let mut num_bonuses = 0;
        for bonus in corporation_prereq_bonuses(d, corporation) {
            num_bonuses += self.city_num_bonuses(c, bonus);
        }

        let mut bonus_maintenance = info.maintenance * num_bonuses;
        bonus_maintenance *= d.worlds[self.setup.world_size].corporation_maintenance_percent;
        bonus_maintenance /= 100;
        maintenance += bonus_maintenance;

        maintenance *= self.cities[c].population + 17;
        maintenance /= 18;

        maintenance *= d.handicaps[self.city_handicap_type(c)].corporation_maintenance_percent;
        maintenance /= 100;

        maintenance *= std::cmp::max(0, self.players[owner].corporation_maintenance_modifier + 100);
        maintenance /= 100;

        let inflation = self.player_calculate_inflation_rate(owner) + 100;
        if inflation > 0 {
            maintenance *= 100;
            maintenance /= inflation;
        }

        maintenance
    }

    /// C++ `CvCity::calculateBaseMaintenanceTimes100` (CvCity.cpp:6030-6033).
    pub fn city_calculate_base_maintenance_times100(&self, c: CityId) -> i32 {
        self.city_calculate_distance_maintenance_times100(c)
            + self.city_calculate_num_cities_maintenance_times100(c)
            + self.city_calculate_colony_maintenance_times100(c)
            + self.city_calculate_corporation_maintenance_times100(c)
    }

    /// C++ `CvCity::changeMaintenanceModifier` (CvCity.cpp:6042-6050).
    pub fn city_change_maintenance_modifier(&mut self, c: CityId, change: i32) {
        if change != 0 {
            self.cities[c].maintenance_modifier += change;
            self.city_update_maintenance(c);
        }
    }

    /// C++ `CvCity::getAirUnitCapacity` (CvCity.cpp:7283-7286): the `air_unit_capacity` field
    /// for the owner's team, `CITY_AIR_UNIT_CAPACITY` for other teams.
    pub fn city_air_unit_capacity(&self, c: CityId, team: TeamId) -> i32 {
        if self.city_team(c) == team {
            self.cities[c].air_unit_capacity
        } else {
            self.d.defines.city_air_unit_capacity
        }
    }

    /// C++ `CvCity::isPower` (CvCity.cpp:7330-7333).
    pub fn city_is_power(&self, c: CityId) -> bool {
        self.cities[c].power_count > 0 || self.city_is_area_clean_power(c)
    }

    /// C++ `CvCity::isAreaCleanPower` (CvCity.cpp:7336-7344). (C++ returns false when
    /// `area()` is NULL, which cannot happen for a city's plot.)
    pub fn city_is_area_clean_power(&self, c: CityId) -> bool {
        self.area_is_clean_power(self.city_area(c), self.city_team(c))
    }

    /// C++ `CvCity::isDirtyPower` (CvCity.cpp:7353-7356).
    pub fn city_is_dirty_power(&self, c: CityId) -> bool {
        self.city_is_power(c) && self.cities[c].dirty_power_count == self.cities[c].power_count && !self.city_is_area_clean_power(c)
    }

    /// C++ `CvCity::changePowerCount` (CvCity.cpp:7359-7394).
    pub fn city_change_power_count(&mut self, c: CityId, change: i32, dirty: bool) {
        if change != 0 {
            let old_power = self.city_is_power(c);
            let old_dirty_power = self.city_is_dirty_power(c);

            self.cities[c].power_count += change;
            if dirty {
                self.cities[c].dirty_power_count += change;
            }

            if old_power != self.city_is_power(c) {
                let owner = self.cities[c].owner;
                self.player_invalidate_yield_rank_cache(owner, None);

                self.city_update_commerces(c);

                // UI: setInfoDirty for the active team.
            }

            if old_dirty_power != self.city_is_dirty_power(c) || old_power != self.city_is_power(c) {
                self.city_update_power_health(c);
            }
        }
    }

    /// C++ `CvCity::changeDefenseDamage` (CvCity.cpp:7403-7418): clamped to
    /// `[0, MAX_CITY_DEFENSE_DAMAGE]`; damage marks the city bombarded.
    pub fn city_change_defense_damage(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let max_defense_damage = self.d.defines.max_city_defense_damage;
            let city = &mut self.cities[c];
            city.defense_damage = range(city.defense_damage + change, 0, max_defense_damage);

            if change > 0 {
                // setBombarded(true)
                city.bombarded = true;
            }

            // UI: setInfoDirty, plotAction(PUF_makeInfoBarDirty).
        }
    }

    /// C++ `CvCity::changeDefenseModifier` (CvCity.cpp:7420-7431).
    pub fn city_change_defense_modifier(&mut self, c: CityId, change: i32) {
        if change != 0 {
            let total_defense = self.city_total_defense(c, false);

            if total_defense > 0 {
                let max_defense_damage = self.d.defines.max_city_defense_damage;
                self.city_change_defense_damage(c, (-(max_defense_damage * change)) / total_defense);
            }
        }
    }

    /// C++ `CvCity::isBombardable` (CvCity.cpp:7446-7454). C++ allows `pUnit == NULL`.
    pub fn city_is_bombardable(&self, c: CityId, unit: Option<UnitId>) -> bool {
        if let Some(unit) = unit {
            if !self.unit_is_enemy(unit, Some(self.city_team(c)), None) {
                return false;
            }
        }

        self.city_defense_modifier(c, false) > 0
    }

    /// C++ `CvCity::getNaturalDefense` (CvCity.cpp:7457-7465).
    pub fn city_natural_defense(&self, c: CityId) -> i32 {
        match self.cities[c].culture_level {
            None => 0,
            Some(culture_level) => self.d.culture_levels[culture_level].city_defense_modifier,
        }
    }

    /// C++ `CvCity::getTotalDefense` (CvCity.cpp:7468-7476). FFH: building + natural defense
    /// are added (not maxed), plus the plot's range defense.
    pub fn city_total_defense(&self, c: CityId, ignore_building: bool) -> i32 {
        // FfH: Modified by Kael 03/13/2008
        let owner = self.cities[c].owner;
        (if ignore_building { 0 } else { self.cities[c].building_defense })
            + self.city_natural_defense(c)
            + self.players[owner].city_defense_modifier
            + self.plot_get_range_defense(self.city_plot(c), self.city_team(c), 3, false, true)
    }

    /// C++ `CvCity::getDefenseModifier` (CvCity.cpp:7479-7487).
    pub fn city_defense_modifier(&self, c: CityId, ignore_building: bool) -> i32 {
        if self.city_is_occupation(c) {
            return 0;
        }

        let max_defense_damage = self.d.defines.max_city_defense_damage;
        (self.city_total_defense(c, ignore_building) * (max_defense_damage - self.cities[c].defense_damage)) / max_defense_damage
    }

    /// C++ `CvCity::setCorporationYield` (CvCity.cpp:8768-8784).
    pub fn city_set_corporation_yield(&mut self, c: CityId, yield_type: YieldType, new_value: i32) {
        let old_value = self.cities[c].corporation_yield[yield_type.idx()];
        if old_value != new_value {
            self.cities[c].corporation_yield[yield_type.idx()] = new_value;
            self.city_change_base_yield_rate(c, yield_type, new_value - old_value);
        }
    }

    /// C++ `CvCity::getCorporationYieldByCorporation` (CvCity.cpp:8794-8816).
    pub fn city_corporation_yield_by_corporation(&self, c: CityId, yield_type: YieldType, corporation: CorporationType) -> i32 {
        let d = self.d;
        let mut yield_amount = 0;

        if self.city_is_active_corporation(c, corporation) && !self.city_is_disorder(c) {
            for bonus in corporation_prereq_bonuses(d, corporation) {
                if self.city_num_bonuses(c, bonus) > 0 {
                    yield_amount += (d.corporations[corporation].yield_produced[yield_type.idx()]
                        * self.city_num_bonuses(c, bonus)
                        * d.worlds[self.setup.world_size].corporation_maintenance_percent)
                        / 100;
                }
            }
        }

        (yield_amount + 99) / 100
    }

    /// C++ `CvCity::getCorporationCommerceByCorporation` (CvCity.cpp:8818-8840).
    pub fn city_corporation_commerce_by_corporation(&self, c: CityId, commerce: CommerceType, corporation: CorporationType) -> i32 {
        let d = self.d;
        let mut commerce_amount = 0;

        if self.city_is_active_corporation(c, corporation) && !self.city_is_disorder(c) {
            for bonus in corporation_prereq_bonuses(d, corporation) {
                if self.city_num_bonuses(c, bonus) > 0 {
                    commerce_amount += (d.corporations[corporation].commerce_produced[commerce.idx()]
                        * self.city_num_bonuses(c, bonus)
                        * d.worlds[self.setup.world_size].corporation_maintenance_percent)
                        / 100;
                }
            }
        }

        (commerce_amount + 99) / 100
    }

    /// C++ `CvCity::updateCorporationCommerce` (CvCity.cpp:8842-8858).
    pub fn city_update_corporation_commerce(&mut self, c: CityId, commerce: CommerceType) {
        let mut new_commerce = 0;
        for corporation in self.d.corporations.keys() {
            new_commerce += self.city_corporation_commerce_by_corporation(c, commerce, corporation);
        }
        if self.cities[c].corporation_commerce[commerce.idx()] != new_commerce {
            self.cities[c].corporation_commerce[commerce.idx()] = new_commerce;
            self.city_update_commerce(c, commerce);
        }
    }

    /// C++ `CvCity::updateCorporationYield` (CvCity.cpp:8860-8877).
    pub fn city_update_corporation_yield(&mut self, c: CityId, yield_type: YieldType) {
        let old_yield = self.cities[c].corporation_yield[yield_type.idx()];
        let mut new_yield = 0;
        for corporation in self.d.corporations.keys() {
            new_yield += self.city_corporation_yield_by_corporation(c, yield_type, corporation);
        }
        if old_yield != new_yield {
            self.cities[c].corporation_yield[yield_type.idx()] = new_yield;
            self.city_change_base_yield_rate(c, yield_type, new_yield - old_yield);
        }
    }

    /// C++ `CvCity::updateCorporation` (CvCity.cpp:8880-8897).
    pub fn city_update_corporation(&mut self, c: CityId) {
        self.city_update_corporation_bonus(c);
        self.city_update_building_commerce(c);
        for i in 0..YieldType::NUM {
            self.city_update_corporation_yield(c, YieldType::from_usize(i));
        }
        for i in 0..CommerceType::NUM {
            self.city_update_corporation_commerce(c, CommerceType::from_usize(i));
        }
        self.city_update_maintenance(c);
    }

    /// C++ `CvCity::updateCorporationBonus` (CvCity.cpp:8900-8979): recomputes
    /// `num_corp_produced_bonuses` (iterated to a fixed point) and re-processes bonuses that
    /// appear or disappear.
    pub fn city_update_corporation_bonus(&mut self, c: CityId) {
        let d = self.d;
        let num_bonuses = d.bonuses.len();
        let mut extra_corp_produced_bonuses: Vec<i32> = Vec::with_capacity(num_bonuses);
        let mut last_corp_produced_bonuses: Vec<i32> = Vec::with_capacity(num_bonuses);
        let mut had_bonuses: Vec<bool> = Vec::with_capacity(num_bonuses);

        for bonus in d.bonuses.keys() {
            had_bonuses.push(self.city_has_bonus(c, bonus));
            self.cities[c].num_corp_produced_bonuses[bonus] = 0;
            last_corp_produced_bonuses.push(self.city_num_bonuses(c, bonus));
            extra_corp_produced_bonuses.push(0);
        }

        let team = self.city_team(c);
        let owner = self.cities[c].owner;

        for _iter in 0..d.corporations.len() {
            for corporation in d.corporations.keys() {
                if let Some(bonus_produced) = d.corporations[corporation].bonus_produced {
                    if !self.team_is_bonus_obsolete(team, bonus_produced) {
                        if self.team_is_has_tech(team, d.bonuses[bonus_produced].tech_city_trade) {
                            if self.cities[c].has_corporation[corporation] && self.player_is_active_corporation(owner, corporation) {
                                for bonus_consumed in corporation_prereq_bonuses(d, corporation) {
                                    extra_corp_produced_bonuses[bonus_produced.idx()] += last_corp_produced_bonuses[bonus_consumed.idx()];
                                }
                            }
                        }
                    }
                }
            }

            let mut changed = false;

            for bonus in d.bonuses.keys() {
                let i = bonus.idx();
                if extra_corp_produced_bonuses[i] != 0 {
                    self.cities[c].num_corp_produced_bonuses[bonus] += extra_corp_produced_bonuses[i];

                    changed = true;
                }

                last_corp_produced_bonuses[i] = extra_corp_produced_bonuses[i];
                extra_corp_produced_bonuses[i] = 0;
            }

            if !changed {
                break;
            }
        }

        for bonus in d.bonuses.keys() {
            if had_bonuses[bonus.idx()] != self.city_has_bonus(c, bonus) {
                if self.city_has_bonus(c, bonus) {
                    self.city_process_bonus(c, bonus, 1);
                } else {
                    self.city_process_bonus(c, bonus, -1);
                }
            }
        }
    }

    /// C++ `CvCity::isNoBonus` (CvCity.cpp:9475-9478).
    pub fn city_is_no_bonus(&self, c: CityId, bonus: BonusType) -> bool {
        self.cities[c].no_bonus[bonus] > 0
    }

    /// C++ `CvCity::changeNoBonusCount` (CvCity.cpp:9480-9506).
    pub fn city_change_no_bonus_count(&mut self, c: CityId, bonus: BonusType, change: i32) {
        if change != 0 {
            if self.city_num_bonuses(c, bonus) > 0 {
                self.city_process_bonus(c, bonus, -1);
            }
            self.cities[c].no_bonus[bonus] += change;
            if self.city_num_bonuses(c, bonus) > 0 {
                self.city_process_bonus(c, bonus, 1);
            }
            self.city_update_corporation(c);
            self.cities[c].ai_assign_work_dirty = true;
        }
    }

    /// C++ `CvCity::changeFreeBonus` (CvCity.cpp:9517-9529): removes and re-adds the city
    /// plot's bonuses to its plot group around the change.
    pub fn city_change_free_bonus(&mut self, c: CityId, bonus: BonusType, change: i32) {
        if change != 0 {
            let plot = self.city_plot(c);
            self.plot_update_plot_group_bonus(plot, false);
            self.cities[c].free_bonus[bonus] += change;
            self.plot_update_plot_group_bonus(plot, true);
        }
    }

    /// C++ `CvCity::getNumBonuses` (CvCity.cpp:9531-9552). FFH: bonuses banned by vote
    /// (`CvGame::isNoBonus`) count 0 for full members of vote source 0.
    pub fn city_num_bonuses(&self, c: CityId, bonus: BonusType) -> i32 {
        if self.city_is_no_bonus(c, bonus) {
            return 0;
        }

        // FfH: Added by Kael 11/14/2007
        // C++ tests `isFullMember((VoteSourceTypes)0)` before `isNoBonus(eIndex)`; both are
        // pure, so the cheap game flag is tested first (same result).
        if self.no_bonus[bonus] && self.player_is_full_member(self.cities[c].owner, VoteSourceType(0)) {
            return 0;
        }
        // FfH: End Add

        let city = &self.cities[c];
        city.num_bonuses[bonus] + city.num_corp_produced_bonuses[bonus]
    }

    /// C++ `CvCity::hasBonus` (CvCity.cpp:9555-9558).
    pub fn city_has_bonus(&self, c: CityId, bonus: BonusType) -> bool {
        self.city_num_bonuses(c, bonus) > 0
    }

    /// C++ `CvCity::changeNumBonuses` (CvCity.cpp:9561-9600). FFH: `ModifierPerBonus`
    /// bonuses are processed per copy.
    pub fn city_change_num_bonuses(&mut self, c: CityId, bonus: BonusType, change: i32) {
        if change != 0 {
            let modifier_per_bonus = self.d.bonuses[bonus].modifier_per_bonus;
            let old_has_bonus = self.city_has_bonus(c, bonus);

            self.cities[c].num_bonuses[bonus] += change;

            // FfH: Modified by Kael 08/21/2007
            if old_has_bonus != self.city_has_bonus(c, bonus) && !modifier_per_bonus {
                if self.city_has_bonus(c, bonus) {
                    self.city_process_bonus(c, bonus, 1);
                } else {
                    self.city_process_bonus(c, bonus, -1);
                }
            }

            // FfH: Added by Kael 08/21/2007
            if modifier_per_bonus {
                self.city_process_bonus(c, bonus, change);
            }
            // FfH: End Add

            if self.city_is_corporation_bonus(c, bonus) {
                self.city_update_corporation(c);
            }
        }
    }

    /// C++ `CvCity::isCorporationBonus` (CvCity.cpp:9610-9633).
    pub fn city_is_corporation_bonus(&self, c: CityId, bonus: BonusType) -> bool {
        let d = self.d;
        let owner = self.cities[c].owner;

        for corporation in d.corporations.keys() {
            if self.player_is_active_corporation(owner, corporation) {
                for prereq_bonus in corporation_prereq_bonuses(d, corporation) {
                    if prereq_bonus == bonus && self.cities[c].has_corporation[corporation] {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// C++ `CvCity::isActiveCorporation` (CvCity.cpp:9635-9663).
    pub fn city_is_active_corporation(&self, c: CityId, corporation: CorporationType) -> bool {
        if !self.cities[c].has_corporation[corporation] {
            return false;
        }

        if !self.player_is_active_corporation(self.cities[c].owner, corporation) {
            return false;
        }

        for bonus in corporation_prereq_bonuses(self.d, corporation) {
            if self.city_num_bonuses(c, bonus) > 0 {
                return true;
            }
        }

        false
    }

    /// C++ `CvCity::setNumRealBuilding` (CvCity.cpp:10434-10449): `setNumRealBuildingTimed`
    /// with the owner and the current game turn (Sephi fix; BTS used the year).
    pub fn city_set_num_real_building(&mut self, c: CityId, building: BuildingType, new_value: i32) {
        let owner = self.cities[c].owner;
        let game_turn = self.game_turn();
        self.city_set_num_real_building_timed(c, building, new_value, true, Some(owner), game_turn);
    }

    /// C++ `CvCity::setNumRealBuildingTimed` (CvCity.cpp:10452-10648). `first` = built here
    /// (not transferred with the city): capital, golden age, global population change,
    /// religions, free techs (AI: `AI_chooseFreeTech` -> `Controller::choose_free_tech`),
    /// wonder messages, nukes, created counts, great wall graphics (omit).
    pub fn city_set_num_real_building_timed(&mut self, c: CityId, building: BuildingType, new_value: i32, first: bool, original_owner: Option<PlayerId>, original_time: i32) {
        let d = self.d;
        let info = &d.buildings[building];
        let building_class = building_class_of(d, building);

        let change_num_real_building = new_value - self.cities[c].num_real_building[building];

        if change_num_real_building != 0 {
            let old_num_building = self.city_num_building(c, building);

            self.cities[c].num_real_building[building] = new_value;

            {
                let city = &mut self.cities[c];
                if city.num_real_building[building] > 0 {
                    city.building_original_owner[building] = original_owner;
                    city.building_original_time[building] = original_time;
                } else {
                    city.building_original_owner[building] = None;
                    city.building_original_time[building] = MIN_INT;
                }
            }

            if old_num_building != self.city_num_building(c, building) {
                if self.cities[c].num_real_building[building] > 0 {
                    if info.is_state_religion {
                        for vote_source in d.vote_sources.keys() {
                            if info.vote_source_type == Some(vote_source) {
                                if self.vote_source_religions[vote_source].is_none() {
                                    let owner = self.cities[c].owner;
                                    let state_religion = self.player_get_state_religion(owner);
                                    self.set_vote_source_religion(vote_source, state_religion, true);
                                }
                            }
                        }
                    }
                }

                let change = self.city_num_building(c, building) - old_num_building;
                self.city_process_building(c, building, change, false);
            }

            if !d.building_classes[building_class].no_limit {
                let city = &mut self.cities[c];
                if is_world_wonder_class(d, building_class) {
                    city.num_world_wonders += change_num_real_building;
                } else if is_team_wonder_class(d, building_class) {
                    city.num_team_wonders += change_num_real_building;
                } else if is_national_wonder_class(d, building_class) {
                    city.num_national_wonders += change_num_real_building;
                } else {
                    city.num_buildings += change_num_real_building;
                }
            }

            if change_num_real_building > 0 {
                if first {
                    let owner = self.cities[c].owner;

                    if info.capital {
                        self.player_set_capital_city(owner, Some(c));
                    }

                    // C++ `!gDLL->GetWorldBuilderMode()`: always true headless.
                    if self.final_initialized {
                        if info.golden_age {
                            let golden_age_length = self.player_get_golden_age_length(owner);
                            self.player_change_golden_age_turns(owner, change_num_real_building * (golden_age_length + 1));
                        }

                        if info.global_population_change != 0 {
                            let team = self.city_team(c);
                            for p in self.player_ids() {
                                if self.players[p].alive {
                                    if self.players[p].team == team {
                                        if info.team_share || p == owner {
                                            for loop_city in self.players[p].cities.clone() {
                                                let new_population = std::cmp::max(
                                                    1,
                                                    self.cities[loop_city].population + change_num_real_building * info.global_population_change,
                                                );
                                                self.city_set_population(loop_city, new_population);
                                                // pLoopCity->AI_updateAssignWork(): so subsequent cities don't
                                                // starve with the extra citizen working nothing.
                                                if self.cities[loop_city].ai_assign_work_dirty {
                                                    self.city_assign_working_plots(loop_city);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        for religion in d.religions.keys() {
                            if info.religion_change[religion] > 0 {
                                self.city_set_has_religion(c, religion, true, true, true);
                            }
                        }

                        if info.free_techs > 0 {
                            if !self.city_is_human(c) {
                                for _ in 0..info.free_techs {
                                    for _ in 0..change_num_real_building {
                                        ai_choose_free_tech(self, owner);
                                    }
                                }
                            } else {
                                // C++ text TXT_KEY_MISC_COMPLETED_WONDER_CHOOSE_TECH (popup title).
                                self.player_choose_tech(owner, info.free_techs * change_num_real_building);
                            }
                        }

                        if is_world_wonder_class(d, building_class) {
                            // UI: replay message TXT_KEY_MISC_COMPLETES_WONDER and the
                            // TXT_KEY_MISC_WONDER_COMPLETED(_UNKNOWN) message to every alive player.
                            let turn = self.game_turn();
                            let player_name = self.player_get_name(owner);
                            self.log.msg(turn, Some(owner), || format!("{} completes {}", player_name, d.text.name(&info.base.description)));
                        }
                    }

                    if info.allows_nukes {
                        // makeNukesValid(true)
                        self.nukes_valid = true;
                    }

                    // incrementBuildingClassCreatedCount
                    self.building_class_created_count[building_class] += 1;
                }
            }

            // Graphics: the great wall (AddGreatWall / RemoveGreatWall for
            // `isAreaBorderObstacle` buildings) is omitted.
        }
    }

    /// C++ `CvCity::setNumFreeBuilding` (CvCity.cpp:10659-10677).
    pub fn city_set_num_free_building(&mut self, c: CityId, building: BuildingType, new_value: i32) {
        if self.cities[c].num_free_building[building] != new_value {
            let old_num_building = self.city_num_building(c, building);
            self.cities[c].num_free_building[building] = new_value;
            if old_num_building != self.city_num_building(c, building) {
                // C++ passes iNewValue - iOldNumBuilding (not the change in getNumBuilding).
                self.city_process_building(c, building, new_value - old_num_building, false);
            }
        }
    }

    /// C++ `CvCity::setHasCorporation` (CvCity.cpp:10887-10993). Python events
    /// `corporationSpread` / `corporationRemove`. C++ default `bArrows = true`.
    pub fn city_set_has_corporation(&mut self, c: CityId, corporation: CorporationType, new_value: bool, announce: bool, arrows: bool) {
        if self.cities[c].has_corporation[corporation] != new_value {
            if new_value {
                let mut replaced_headquarters = false;
                for other in self.d.corporations.keys() {
                    if other != corporation && self.cities[c].has_corporation[other] {
                        if self.is_competing_corporation(other, corporation) {
                            if self.get_headquarters(other) == Some(c) {
                                self.replace_corporation(other, corporation);
                                replaced_headquarters = true;
                            } else {
                                self.city_set_has_corporation(c, other, false, false, true);
                            }
                        }
                    }
                }

                if replaced_headquarters {
                    return; // already set the corporation in this city
                }
            }

            self.cities[c].has_corporation[corporation] = new_value;

            let owner = self.cities[c].owner;
            let has_corporation = self.cities[c].has_corporation[corporation];
            self.player_change_has_corporation_count(owner, corporation, if has_corporation { 1 } else { -1 });

            if let Some(headquarters) = self.get_headquarters(corporation) {
                self.city_update_corporation(headquarters);
            }

            self.city_update_corporation(c);

            self.cities[c].ai_assign_work_dirty = true;

            // UI: setInfoDirty(true).

            if self.cities[c].has_corporation[corporation] {
                self.make_corporation_founded(corporation, owner);
            }

            if announce {
                // UI: TXT_KEY_MISC_CORPORATION_SPREAD (+ TXT_KEY_MISC_CORPORATION_SPREAD_BONUS for
                // the owner) to the owner and every alive player with the headquarters;
                // `bArrows` only sets the messages' arrows.
                let _ = arrows;
                let d = self.d;
                let turn = self.game_turn();
                let city_name = self.cities[c].name.clone();
                self.log.msg(turn, Some(owner), || {
                    format!("{} has spread to {}", d.text.name(&d.corporations[corporation].base.description), city_name)
                });
            }

            if new_value {
                // Python Event
                self.ev_corporation_spread(corporation, owner, c);
            } else {
                // Python Event
                self.ev_corporation_remove(corporation, owner, c);
            }
        }
    }

    /// C++ `CvCity::isValidBuildingLocation` (CvCity.cpp:13112-13142): water / river / area
    /// size requirements.
    pub fn city_is_valid_building_location(&self, c: CityId, building: BuildingType) -> bool {
        let info = &self.d.buildings[building];

        // if both the river and water flags are set, we require one of the two conditions, not both
        if info.water {
            if !info.river || !self.plot_is_river(self.city_plot(c)) {
                if !self.city_is_coastal(c, info.min_area_size) {
                    return false;
                }
            }
        } else {
            if self.map.areas[self.city_area(c)].num_tiles < info.min_area_size {
                return false;
            }

            if info.river {
                if !self.plot_is_river(self.city_plot(c)) {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvCity::getBuildingYieldChange` (CvCity.cpp:13754-13765).
    pub fn city_building_yield_change(&self, c: CityId, building_class: BuildingClassType, yield_type: YieldType) -> i32 {
        self.cities[c]
            .building_yield_change
            .iter()
            .find(|ch| ch.building_class == building_class && ch.yield_type == yield_type)
            .map_or(0, |ch| ch.change)
    }

    /// C++ `CvCity::setBuildingYieldChange` (CvCity.cpp:13767-13817).
    pub fn city_set_building_yield_change(&mut self, c: CityId, building_class: BuildingClassType, yield_type: YieldType, change: i32) {
        let d = self.d;

        let found = self.cities[c].building_yield_change.iter().position(|ch| ch.building_class == building_class && ch.yield_type == yield_type);

        if let Some(i) = found {
            let old_change = self.cities[c].building_yield_change[i].change;
            if old_change != change {
                if change == 0 {
                    self.cities[c].building_yield_change.remove(i);
                } else {
                    self.cities[c].building_yield_change[i].change = change;
                }

                if let Some(building) = d.civ_building(self.city_civilization_type(c), building_class) {
                    if self.city_num_active_building(c, building) > 0 {
                        let num_active = self.city_num_active_building(c, building);
                        self.city_change_base_yield_rate(c, yield_type, (change - old_change) * num_active);
                    }
                }
            }

            return;
        }

        if change != 0 {
            self.cities[c].building_yield_change.push(BuildingYieldChange { building_class, yield_type, change });

            if let Some(building) = d.civ_building(self.city_civilization_type(c), building_class) {
                if self.city_num_active_building(c, building) > 0 {
                    let num_active = self.city_num_active_building(c, building);
                    self.city_change_base_yield_rate(c, yield_type, change * num_active);
                }
            }
        }
    }

    /// C++ `CvCity::changeBuildingYieldChange` (CvCity.cpp:13819-13822).
    pub fn city_change_building_yield_change(&mut self, c: CityId, building_class: BuildingClassType, yield_type: YieldType, change: i32) {
        let new_value = self.city_building_yield_change(c, building_class, yield_type) + change;
        self.city_set_building_yield_change(c, building_class, yield_type, new_value);
    }

    /// C++ `CvCity::getBuildingCommerceChange` (CvCity.cpp:13824-13835).
    pub fn city_building_commerce_change(&self, c: CityId, building_class: BuildingClassType, commerce: CommerceType) -> i32 {
        self.cities[c]
            .building_commerce_change
            .iter()
            .find(|ch| ch.building_class == building_class && ch.commerce_type == commerce)
            .map_or(0, |ch| ch.change)
    }

    /// C++ `CvCity::setBuildingCommerceChange` (CvCity.cpp:13837-13871).
    pub fn city_set_building_commerce_change(&mut self, c: CityId, building_class: BuildingClassType, commerce: CommerceType, change: i32) {
        let changes = &mut self.cities[c].building_commerce_change;
        let mut updated = false;
        if let Some(i) = changes.iter().position(|ch| ch.building_class == building_class && ch.commerce_type == commerce) {
            if changes[i].change != change {
                if change == 0 {
                    changes.remove(i);
                } else {
                    changes[i].change = change;
                }
                updated = true;
            }
        } else if change != 0 {
            changes.push(BuildingCommerceChange { building_class, commerce_type: commerce, change });
            updated = true;
        }
        if updated {
            self.city_update_building_commerce(c);
        }
    }

    /// C++ `CvCity::changeBuildingCommerceChange` (CvCity.cpp:13873-13876).
    pub fn city_change_building_commerce_change(&mut self, c: CityId, building_class: BuildingClassType, commerce: CommerceType, change: i32) {
        let new_value = self.city_building_commerce_change(c, building_class, commerce) + change;
        self.city_set_building_commerce_change(c, building_class, commerce, new_value);
    }

    /// C++ `CvCity::setBuildingHappyChange` (CvCity.cpp:13939-14006, the UNOFFICIAL_PATCH
    /// version; the BTS original above it is commented out).
    ///
    /// C++ quirk: the change is only recorded while the class's building is active here. An
    /// existing entry is erased first and re-added only if the building is active, so a
    /// change made while the building is missing or obsolete is lost.
    pub fn city_set_building_happy_change(&mut self, c: CityId, building_class: BuildingClassType, change: i32) {
        let d = self.d;

        let found = self.cities[c].building_happy_change.iter().position(|&(bc, _)| bc == building_class);

        if let Some(i) = found {
            if self.cities[c].building_happy_change[i].1 != change {
                let old_change = self.cities[c].building_happy_change[i].1;

                self.cities[c].building_happy_change.remove(i);

                if let Some(building) = d.civ_building(self.city_civilization_type(c), building_class) {
                    if self.city_num_active_building(c, building) > 0 {
                        if old_change > 0 {
                            self.city_change_building_good_happiness(c, -old_change);
                        } else if old_change < 0 {
                            self.city_change_building_bad_happiness(c, -old_change);
                        }

                        if change != 0 {
                            self.cities[c].building_happy_change.push((building_class, change));
                            if change > 0 {
                                self.city_change_building_good_happiness(c, change);
                            } else if change < 0 {
                                self.city_change_building_bad_happiness(c, change);
                            }
                        }
                    }
                }
            }

            return;
        }

        if change != 0 {
            if let Some(building) = d.civ_building(self.city_civilization_type(c), building_class) {
                if self.city_num_active_building(c, building) > 0 {
                    self.cities[c].building_happy_change.push((building_class, change));

                    if change > 0 {
                        self.city_change_building_good_happiness(c, change);
                    } else if change < 0 {
                        self.city_change_building_bad_happiness(c, change);
                    }
                }
            }
        }
    }

    /// C++ `CvCity::getBuildingHappyChange` (CvCity.cpp:14011-14022).
    pub fn city_building_happy_change(&self, c: CityId, building_class: BuildingClassType) -> i32 {
        self.cities[c].building_happy_change.iter().find(|(bc, _)| *bc == building_class).map_or(0, |&(_, change)| change)
    }

    /// C++ `CvCity::setBuildingHealthChange` (CvCity.cpp:14085-14151, the UNOFFICIAL_PATCH
    /// version; the BTS original above it is commented out).
    ///
    /// C++ quirk: as in `setBuildingHappyChange`, the change is only recorded while the
    /// class's building is active here.
    pub fn city_set_building_health_change(&mut self, c: CityId, building_class: BuildingClassType, change: i32) {
        let d = self.d;

        let found = self.cities[c].building_health_change.iter().position(|&(bc, _)| bc == building_class);

        if let Some(i) = found {
            if self.cities[c].building_health_change[i].1 != change {
                let old_change = self.cities[c].building_health_change[i].1;

                self.cities[c].building_health_change.remove(i);

                if let Some(building) = d.civ_building(self.city_civilization_type(c), building_class) {
                    if self.city_num_active_building(c, building) > 0 {
                        if old_change > 0 {
                            self.city_change_building_good_health(c, -old_change);
                        } else if old_change < 0 {
                            self.city_change_building_bad_health(c, -old_change);
                        }

                        if change != 0 {
                            self.cities[c].building_health_change.push((building_class, change));
                            if change > 0 {
                                self.city_change_building_good_health(c, change);
                            } else if change < 0 {
                                self.city_change_building_bad_health(c, change);
                            }
                        }
                    }
                }
            }

            return;
        }

        if change != 0 {
            if let Some(building) = d.civ_building(self.city_civilization_type(c), building_class) {
                if self.city_num_active_building(c, building) > 0 {
                    self.cities[c].building_health_change.push((building_class, change));

                    if change > 0 {
                        self.city_change_building_good_health(c, change);
                    } else if change < 0 {
                        self.city_change_building_bad_health(c, change);
                    }
                }
            }
        }
    }

    /// C++ `CvCity::getBuildingHealthChange` (CvCity.cpp:14156-14167).
    pub fn city_building_health_change(&self, c: CityId, building_class: BuildingClassType) -> i32 {
        self.cities[c].building_health_change.iter().find(|(bc, _)| *bc == building_class).map_or(0, |&(_, change)| change)
    }

    /// C++ `CvCity::isHasBuildingClass` (CvCity.cpp:14705-14718, FFH).
    pub fn city_is_has_building_class(&self, c: CityId, building_class: BuildingClassType) -> bool {
        let d = self.d;

        for building in d.buildings.keys() {
            if self.city_num_building(c, building) > 0 {
                if d.buildings[building].building_class_type == Some(building_class) {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::City;
    use crate::setup::GameSetup;

    /// A game with one city (inserted directly, not `init`ed) owned by player 0.
    fn game_with_city() -> (Game, CityId) {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 3));
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        let id = g.cities.next_id();
        let c = g.cities.insert(City::new(d, num_players, num_teams, id, PlayerId(0), 1, 1, 0));
        (g, c)
    }

    #[test]
    fn every_building_has_a_class() {
        let d = ffh_data::global();
        for building in d.buildings.keys() {
            let class = building_class_of(d, building);
            assert!(class.idx() < d.building_classes.len());
        }
    }

    #[test]
    fn num_building_adds_real_and_free_up_to_the_cap() {
        let (mut g, c) = game_with_city();
        let cap = g.d.defines.city_max_num_buildings;
        let building = BuildingType(0);
        assert_eq!(g.city_num_building(c, building), 0);
        g.cities[c].num_free_building[building] = 1;
        assert_eq!(g.city_num_building(c, building), std::cmp::min(cap, 1));
        g.cities[c].num_real_building[building] = 1;
        assert_eq!(g.city_num_building(c, building), std::cmp::min(cap, 2));
    }

    #[test]
    fn has_building_class_counts_real_and_free_buildings() {
        let (mut g, c) = game_with_city();
        let d = g.d;
        let building = BuildingType(5);
        let class = building_class_of(d, building);
        assert!(!g.city_is_has_building_class(c, class));
        g.cities[c].num_free_building[building] = 1;
        assert!(g.city_is_has_building_class(c, class));
        g.cities[c].num_free_building[building] = 0;
        g.cities[c].num_real_building[building] = 1;
        assert!(g.city_is_has_building_class(c, class));
    }

    #[test]
    fn num_bonuses_adds_corporation_bonuses_unless_banned() {
        let (mut g, c) = game_with_city();
        let bonus = BonusType(0);
        g.cities[c].num_bonuses[bonus] = 2;
        g.cities[c].num_corp_produced_bonuses[bonus] = 1;
        assert_eq!(g.city_num_bonuses(c, bonus), 3);
        assert!(g.city_has_bonus(c, bonus));
        g.cities[c].no_bonus[bonus] = 1;
        assert!(g.city_is_no_bonus(c, bonus));
        assert_eq!(g.city_num_bonuses(c, bonus), 0);
        assert!(!g.city_has_bonus(c, bonus));
    }

    #[test]
    fn defense_damage_is_clamped_and_marks_bombarded() {
        let (mut g, c) = game_with_city();
        let max = g.d.defines.max_city_defense_damage;
        g.city_change_defense_damage(c, 30);
        assert_eq!(g.cities[c].defense_damage, std::cmp::min(30, max));
        assert!(g.cities[c].bombarded);
        g.cities[c].bombarded = false;
        g.city_change_defense_damage(c, -(max + 50));
        assert_eq!(g.cities[c].defense_damage, 0);
        assert!(!g.cities[c].bombarded, "repairs do not mark the city bombarded");
        g.city_change_defense_damage(c, max + 10);
        assert_eq!(g.cities[c].defense_damage, max);
        g.cities[c].bombarded = false;
        g.city_change_defense_damage(c, 0);
        assert!(!g.cities[c].bombarded);
    }

    #[test]
    fn natural_defense_comes_from_the_culture_level() {
        let (mut g, c) = game_with_city();
        assert_eq!(g.city_natural_defense(c), 0);
        for level in g.d.culture_levels.keys() {
            g.cities[c].culture_level = Some(level);
            assert_eq!(g.city_natural_defense(c), g.d.culture_levels[level].city_defense_modifier);
        }
    }

    #[test]
    fn maintenance_is_stored_times_100() {
        let (mut g, c) = game_with_city();
        g.cities[c].maintenance = 1234;
        assert_eq!(g.city_maintenance(c), 12);
    }
}
