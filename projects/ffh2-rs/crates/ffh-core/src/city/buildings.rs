//! CvCity: buildings (`processBuilding`, real/free building counts, per-class yield /
//! commerce / happiness / health changes), bonuses, corporations, power, defense and
//! maintenance.

#![allow(unused_variables)]
#![allow(unused_imports)]
use super::{order_type_data, range, City};
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

impl Game {
    /// C++ `CvCity::getBonusHealth` (CvCity.cpp:3797-3810).
    pub fn city_bonus_health(&self, c: CityId, bonus: BonusType) -> i32 {
        todo!("CvCity::getBonusHealth")
    }

    /// C++ `CvCity::getBonusHappiness` (CvCity.cpp:3813-3826).
    pub fn city_bonus_happiness(&self, c: CityId, bonus: BonusType) -> i32 {
        todo!("CvCity::getBonusHappiness")
    }

    /// C++ `CvCity::getBonusPower` (CvCity.cpp:3829-3851).
    pub fn city_bonus_power(&self, c: CityId, bonus: BonusType, dirty: bool) -> i32 {
        todo!("CvCity::getBonusPower")
    }

    /// C++ `CvCity::getBonusYieldRateModifier(YieldTypes, BonusTypes)` (CvCity.cpp:3854-3867).
    /// (The one-argument overload is the field `bonus_yield_rate_modifier[y]`.)
    pub fn city_bonus_yield_rate_modifier(&self, c: CityId, yield_type: YieldType, bonus: BonusType) -> i32 {
        todo!("CvCity::getBonusYieldRateModifier")
    }

    /// C++ `CvCity::processBonus` (CvCity.cpp:3870-3972). FFH: bonus GP/maintenance/mutation/
    /// research modifiers, capital-only player effects, free promotions.
    pub fn city_process_bonus(&mut self, c: CityId, bonus: BonusType, change: i32) {
        todo!("CvCity::processBonus")
    }

    /// C++ `CvCity::processBuilding` (CvCity.cpp:3975-4206): applies (`change` > 0) or removes
    /// a building's effects to the city, its owner, team, area and the game. C++ default
    /// `bObsolete = false`.
    pub fn city_process_building(&mut self, c: CityId, building: BuildingType, change: i32, obsolete: bool) {
        todo!("CvCity::processBuilding")
    }

    /// C++ `CvCity::isHeadquarters(CorporationTypes)` (CvCity.cpp:4378-4381).
    pub fn city_is_headquarters(&self, c: CityId, corporation: CorporationType) -> bool {
        todo!("CvCity::isHeadquarters")
    }

    /// C++ `CvCity::setHeadquarters` (CvCity.cpp:4383-4396): also grants the corporation's
    /// free unit.
    pub fn city_set_headquarters(&mut self, c: CityId, corporation: CorporationType) {
        todo!("CvCity::setHeadquarters")
    }

    /// C++ `CvCity::isHeadquarters()` (CvCity.cpp:4398-4411): headquarters of any corporation.
    pub fn city_is_any_headquarters(&self, c: CityId) -> bool {
        todo!("CvCity::isHeadquarters")
    }

    /// C++ `CvCity::getNumBuilding` (CvCity.cpp:5242-5247): real + free, capped at
    /// `CITY_MAX_NUM_BUILDINGS`.
    pub fn city_num_building(&self, c: CityId, building: BuildingType) -> i32 {
        todo!("CvCity::getNumBuilding")
    }

    /// C++ `CvCity::getNumActiveBuilding` (CvCity.cpp:5250-5260): 0 if obsolete for the team.
    pub fn city_num_active_building(&self, c: CityId, building: BuildingType) -> i32 {
        todo!("CvCity::getNumActiveBuilding")
    }

    /// C++ `CvCity::hasActiveWorldWonder` (CvCity.cpp:5263-5279).
    pub fn city_has_active_world_wonder(&self, c: CityId) -> bool {
        todo!("CvCity::hasActiveWorldWonder")
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
        todo!("CvCity::changeGovernmentCenterCount")
    }

    /// C++ `CvCity::getMaintenance` (CvCity.cpp:5765-5768). (`getMaintenanceTimes100` is the
    /// field `maintenance`.)
    pub fn city_maintenance(&self, c: CityId) -> i32 {
        self.cities[c].maintenance / 100
    }

    /// C++ `CvCity::updateMaintenance` (CvCity.cpp:5776-5800).
    pub fn city_update_maintenance(&mut self, c: CityId) {
        todo!("CvCity::updateMaintenance")
    }

    /// C++ `CvCity::calculateDistanceMaintenance` (CvCity.cpp:5802-5805).
    pub fn city_calculate_distance_maintenance(&self, c: CityId) -> i32 {
        self.city_calculate_distance_maintenance_times100(c) / 100
    }

    /// C++ `CvCity::calculateDistanceMaintenanceTimes100` (CvCity.cpp:5807-5855).
    pub fn city_calculate_distance_maintenance_times100(&self, c: CityId) -> i32 {
        todo!("CvCity::calculateDistanceMaintenanceTimes100")
    }

    /// C++ `CvCity::calculateNumCitiesMaintenance` (CvCity.cpp:5857-5860).
    pub fn city_calculate_num_cities_maintenance(&self, c: CityId) -> i32 {
        self.city_calculate_num_cities_maintenance_times100(c) / 100
    }

    /// C++ `CvCity::calculateNumCitiesMaintenanceTimes100` (CvCity.cpp:5862-5905).
    pub fn city_calculate_num_cities_maintenance_times100(&self, c: CityId) -> i32 {
        todo!("CvCity::calculateNumCitiesMaintenanceTimes100")
    }

    /// C++ `CvCity::calculateColonyMaintenance` (CvCity.cpp:5908-5911).
    pub fn city_calculate_colony_maintenance(&self, c: CityId) -> i32 {
        self.city_calculate_colony_maintenance_times100(c) / 100
    }

    /// C++ `CvCity::calculateColonyMaintenanceTimes100` (CvCity.cpp:5913-5959). FFH: no
    /// colony maintenance on a landmass with any government center of the owner.
    pub fn city_calculate_colony_maintenance_times100(&self, c: CityId) -> i32 {
        todo!("CvCity::calculateColonyMaintenanceTimes100")
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
        todo!("CvCity::calculateCorporationMaintenanceTimes100")
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
        todo!("CvCity::getAirUnitCapacity")
    }

    /// C++ `CvCity::isPower` (CvCity.cpp:7330-7333).
    pub fn city_is_power(&self, c: CityId) -> bool {
        self.cities[c].power_count > 0 || self.city_is_area_clean_power(c)
    }

    /// C++ `CvCity::isAreaCleanPower` (CvCity.cpp:7336-7344).
    pub fn city_is_area_clean_power(&self, c: CityId) -> bool {
        todo!("CvCity::isAreaCleanPower")
    }

    /// C++ `CvCity::isDirtyPower` (CvCity.cpp:7353-7356).
    pub fn city_is_dirty_power(&self, c: CityId) -> bool {
        self.city_is_power(c) && self.cities[c].dirty_power_count == self.cities[c].power_count && !self.city_is_area_clean_power(c)
    }

    /// C++ `CvCity::changePowerCount` (CvCity.cpp:7359-7394).
    pub fn city_change_power_count(&mut self, c: CityId, change: i32, dirty: bool) {
        todo!("CvCity::changePowerCount")
    }

    /// C++ `CvCity::changeDefenseDamage` (CvCity.cpp:7403-7418): clamped to
    /// `[0, MAX_CITY_DEFENSE_DAMAGE]`; damage marks the city bombarded.
    pub fn city_change_defense_damage(&mut self, c: CityId, change: i32) {
        todo!("CvCity::changeDefenseDamage")
    }

    /// C++ `CvCity::changeDefenseModifier` (CvCity.cpp:7420-7431).
    pub fn city_change_defense_modifier(&mut self, c: CityId, change: i32) {
        todo!("CvCity::changeDefenseModifier")
    }

    /// C++ `CvCity::isBombardable` (CvCity.cpp:7446-7454). C++ allows `pUnit == NULL`.
    pub fn city_is_bombardable(&self, c: CityId, unit: Option<UnitId>) -> bool {
        todo!("CvCity::isBombardable")
    }

    /// C++ `CvCity::getNaturalDefense` (CvCity.cpp:7457-7465).
    pub fn city_natural_defense(&self, c: CityId) -> i32 {
        todo!("CvCity::getNaturalDefense")
    }

    /// C++ `CvCity::getTotalDefense` (CvCity.cpp:7468-7476). FFH: building + natural defense
    /// are added (not maxed), plus the plot's range defense.
    pub fn city_total_defense(&self, c: CityId, ignore_building: bool) -> i32 {
        todo!("CvCity::getTotalDefense")
    }

    /// C++ `CvCity::getDefenseModifier` (CvCity.cpp:7479-7487).
    pub fn city_defense_modifier(&self, c: CityId, ignore_building: bool) -> i32 {
        todo!("CvCity::getDefenseModifier")
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
        todo!("CvCity::getCorporationYieldByCorporation")
    }

    /// C++ `CvCity::getCorporationCommerceByCorporation` (CvCity.cpp:8818-8840).
    pub fn city_corporation_commerce_by_corporation(&self, c: CityId, commerce: CommerceType, corporation: CorporationType) -> i32 {
        todo!("CvCity::getCorporationCommerceByCorporation")
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
        todo!("CvCity::updateCorporationBonus")
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
        todo!("CvCity::changeFreeBonus")
    }

    /// C++ `CvCity::getNumBonuses` (CvCity.cpp:9531-9552). FFH: bonuses banned by vote
    /// (`CvGame::isNoBonus`) count 0 for full members of vote source 0.
    pub fn city_num_bonuses(&self, c: CityId, bonus: BonusType) -> i32 {
        todo!("CvCity::getNumBonuses")
    }

    /// C++ `CvCity::hasBonus` (CvCity.cpp:9555-9558).
    pub fn city_has_bonus(&self, c: CityId, bonus: BonusType) -> bool {
        self.city_num_bonuses(c, bonus) > 0
    }

    /// C++ `CvCity::changeNumBonuses` (CvCity.cpp:9561-9600). FFH: `ModifierPerBonus`
    /// bonuses are processed per copy.
    pub fn city_change_num_bonuses(&mut self, c: CityId, bonus: BonusType, change: i32) {
        todo!("CvCity::changeNumBonuses")
    }

    /// C++ `CvCity::isCorporationBonus` (CvCity.cpp:9610-9633).
    pub fn city_is_corporation_bonus(&self, c: CityId, bonus: BonusType) -> bool {
        todo!("CvCity::isCorporationBonus")
    }

    /// C++ `CvCity::isActiveCorporation` (CvCity.cpp:9635-9663).
    pub fn city_is_active_corporation(&self, c: CityId, corporation: CorporationType) -> bool {
        todo!("CvCity::isActiveCorporation")
    }

    /// C++ `CvCity::setNumRealBuilding` (CvCity.cpp:10434-10449): `setNumRealBuildingTimed`
    /// with the owner and the current game turn (Sephi fix; BTS used the year).
    pub fn city_set_num_real_building(&mut self, c: CityId, building: BuildingType, new_value: i32) {
        todo!("CvCity::setNumRealBuilding")
    }

    /// C++ `CvCity::setNumRealBuildingTimed` (CvCity.cpp:10452-10648). `first` = built here
    /// (not transferred with the city): capital, golden age, global population change,
    /// religions, free techs (AI: `AI_chooseFreeTech` -> `Controller::choose_free_tech`),
    /// wonder messages, nukes, created counts, great wall graphics (omit).
    pub fn city_set_num_real_building_timed(&mut self, c: CityId, building: BuildingType, new_value: i32, first: bool, original_owner: Option<PlayerId>, original_time: i32) {
        todo!("CvCity::setNumRealBuildingTimed")
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
        todo!("CvCity::setHasCorporation")
    }

    /// C++ `CvCity::isValidBuildingLocation` (CvCity.cpp:13112-13142): water / river / area
    /// size requirements.
    pub fn city_is_valid_building_location(&self, c: CityId, building: BuildingType) -> bool {
        todo!("CvCity::isValidBuildingLocation")
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
        todo!("CvCity::setBuildingYieldChange")
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
    pub fn city_set_building_happy_change(&mut self, c: CityId, building_class: BuildingClassType, change: i32) {
        todo!("CvCity::setBuildingHappyChange")
    }

    /// C++ `CvCity::getBuildingHappyChange` (CvCity.cpp:14011-14022).
    pub fn city_building_happy_change(&self, c: CityId, building_class: BuildingClassType) -> i32 {
        self.cities[c].building_happy_change.iter().find(|(bc, _)| *bc == building_class).map_or(0, |&(_, change)| change)
    }

    /// C++ `CvCity::setBuildingHealthChange` (CvCity.cpp:14085-14151, the UNOFFICIAL_PATCH
    /// version; the BTS original above it is commented out).
    pub fn city_set_building_health_change(&mut self, c: CityId, building_class: BuildingClassType, change: i32) {
        todo!("CvCity::setBuildingHealthChange")
    }

    /// C++ `CvCity::getBuildingHealthChange` (CvCity.cpp:14156-14167).
    pub fn city_building_health_change(&self, c: CityId, building_class: BuildingClassType) -> i32 {
        self.cities[c].building_health_change.iter().find(|(bc, _)| *bc == building_class).map_or(0, |&(_, change)| change)
    }

    /// C++ `CvCity::isHasBuildingClass` (CvCity.cpp:14705-14718, FFH).
    pub fn city_is_has_building_class(&self, c: CityId, building_class: BuildingClassType) -> bool {
        todo!("CvCity::isHasBuildingClass")
    }
}
