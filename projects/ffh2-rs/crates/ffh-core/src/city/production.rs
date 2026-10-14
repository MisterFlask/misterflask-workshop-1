//! CvCity: build checks (`canTrain`/`canConstruct`/`canCreate`/`canMaintain`), production
//! queries and modifiers, hurry and conscription.
//!
//! The build queue itself (`pushOrder`/`popOrder`, `doProduction`, ...) is in `turn.rs`.

#![allow(unused_variables)]
#![allow(unused_imports)]
use super::{order_type_data, range, City};
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

impl Game {
    /// C++ `CvCity::chooseProduction` (CvCity.cpp:1330-1362). C++ opens the
    /// choose-production popup for the human owner (the arguments are the item just
    /// finished, shown as a hint). In this port the decision goes to the owner's controller
    /// (`Controller::choose_production`).
    pub fn city_choose_production(&mut self, c: CityId, train_unit: Option<UnitType>, construct_building: Option<BuildingType>, create_project: Option<ProjectType>, finish: bool, front: bool) {
        todo!("CvCity::chooseProduction")
    }

    /// C++ `CvCity::allUpgradesAvailable` (CvCity.cpp:1677-1747). Recursive; C++ default
    /// `iUpgradeCount = 0`.
    pub fn city_all_upgrades_available(&self, c: CityId, unit: UnitType, upgrade_count: i32) -> Option<UnitType> {
        todo!("CvCity::allUpgradesAvailable")
    }

    /// C++ `CvCity::isWorldWondersMaxed` (CvCity.cpp:1750-1768).
    pub fn city_is_world_wonders_maxed(&self, c: CityId) -> bool {
        todo!("CvCity::isWorldWondersMaxed")
    }

    /// C++ `CvCity::isTeamWondersMaxed` (CvCity.cpp:1771-1789).
    pub fn city_is_team_wonders_maxed(&self, c: CityId) -> bool {
        todo!("CvCity::isTeamWondersMaxed")
    }

    /// C++ `CvCity::isNationalWondersMaxed` (CvCity.cpp:1792-1807).
    pub fn city_is_national_wonders_maxed(&self, c: CityId) -> bool {
        todo!("CvCity::isNationalWondersMaxed")
    }

    /// C++ `CvCity::isBuildingsMaxed` (CvCity.cpp:1810-1828).
    pub fn city_is_buildings_maxed(&self, c: CityId) -> bool {
        todo!("CvCity::isBuildingsMaxed")
    }

    /// C++ `CvCity::canTrain(UnitTypes, ...)` (CvCity.cpp:1832-1850). FFH: min level and
    /// settlement checks, then `canUpgrade`. (C++ returns false for `NO_UNIT`; callers holding
    /// an `Option` check it first.)
    pub fn city_can_train(&self, c: CityId, unit: UnitType, cont: bool, test_visible: bool, ignore_cost: bool, ignore_upgrades: bool) -> bool {
        todo!("CvCity::canTrain")
    }

    /// C++ `CvCity::canUpgrade` (CvCity.cpp:1852-1968): the BTS `canTrain` body (Python
    /// `canTrain`/`cannotTrain` callbacks, player/plot checks, FFH civ/world-unit rules).
    pub fn city_can_upgrade(&self, c: CityId, unit: UnitType, cont: bool, test_visible: bool, ignore_cost: bool, ignore_upgrades: bool) -> bool {
        todo!("CvCity::canUpgrade")
    }

    /// C++ `CvCity::canTrain(UnitCombatTypes)` (CvCity.cpp:1970-1989).
    pub fn city_can_train_combat(&self, c: CityId, combat: UnitCombatType) -> bool {
        todo!("CvCity::canTrain")
    }

    /// C++ `CvCity::canConstruct` (CvCity.cpp:1992-2309). Python `canConstruct` /
    /// `cannotConstruct` callbacks. (C++ returns false for `NO_BUILDING`.)
    pub fn city_can_construct(&self, c: CityId, building: BuildingType, cont: bool, test_visible: bool, ignore_cost: bool) -> bool {
        todo!("CvCity::canConstruct")
    }

    /// C++ `CvCity::canCreate` (CvCity.cpp:2312-2374). Python `canCreate`/`cannotCreate`.
    pub fn city_can_create(&self, c: CityId, project: ProjectType, cont: bool, test_visible: bool) -> bool {
        todo!("CvCity::canCreate")
    }

    /// C++ `CvCity::canMaintain` (CvCity.cpp:2377-2419). Python `canMaintain`/`cannotMaintain`.
    pub fn city_can_maintain(&self, c: CityId, process: ProcessType, cont: bool) -> bool {
        todo!("CvCity::canMaintain")
    }

    /// C++ `CvCity::isProduction` (CvCity.cpp:2451-2454).
    pub fn city_is_production(&self, c: CityId) -> bool {
        !self.cities[c].order_queue.is_empty()
    }

    /// C++ `CvCity::isProductionLimited` (CvCity.cpp:2457-2487).
    pub fn city_is_production_limited(&self, c: CityId) -> bool {
        todo!("CvCity::isProductionLimited")
    }

    /// C++ `CvCity::isProductionUnit` (CvCity.cpp:2490-2500).
    pub fn city_is_production_unit(&self, c: CityId) -> bool {
        self.cities[c].head_order().map_or(false, |o| o.order_type == OrderType::ORDER_TRAIN)
    }

    /// C++ `CvCity::isProductionBuilding` (CvCity.cpp:2503-2513).
    pub fn city_is_production_building(&self, c: CityId) -> bool {
        self.cities[c].head_order().map_or(false, |o| o.order_type == OrderType::ORDER_CONSTRUCT)
    }

    /// C++ `CvCity::isProductionProject` (CvCity.cpp:2516-2526).
    pub fn city_is_production_project(&self, c: CityId) -> bool {
        self.cities[c].head_order().map_or(false, |o| o.order_type == OrderType::ORDER_CREATE)
    }

    /// C++ `CvCity::isProductionProcess` (CvCity.cpp:2529-2539).
    pub fn city_is_production_process(&self, c: CityId) -> bool {
        self.cities[c].head_order().map_or(false, |o| o.order_type == OrderType::ORDER_MAINTAIN)
    }

    /// C++ `CvCity::canContinueProduction` (CvCity.cpp:2542-2568).
    pub fn city_can_continue_production(&self, c: CityId, order: &OrderData) -> bool {
        match order.order_type {
            OrderType::ORDER_TRAIN => self.city_can_train(c, order_type_data(order), true, false, false, false),
            OrderType::ORDER_CONSTRUCT => self.city_can_construct(c, order_type_data(order), true, false, false),
            OrderType::ORDER_CREATE => self.city_can_create(c, order_type_data(order), true, false),
            OrderType::ORDER_MAINTAIN => self.city_can_maintain(c, order_type_data(order), true),
            _ => false,
        }
    }

    /// C++ `CvCity::getProductionExperience` (CvCity.cpp:2571-2598). C++ default
    /// `eUnit = NO_UNIT`.
    pub fn city_production_experience(&self, c: CityId, unit: Option<UnitType>) -> i32 {
        todo!("CvCity::getProductionExperience")
    }

    /// C++ `CvCity::addProductionExperience` (CvCity.cpp:2601-2621).
    pub fn city_add_production_experience(&mut self, c: CityId, unit: UnitId, conscript: bool) {
        todo!("CvCity::addProductionExperience")
    }

    /// C++ `CvCity::getProductionUnit` (CvCity.cpp:2624-2648).
    pub fn city_production_unit(&self, c: CityId) -> Option<UnitType> {
        let o = self.cities[c].head_order()?;
        (o.order_type == OrderType::ORDER_TRAIN).then(|| order_type_data(o))
    }

    /// C++ `CvCity::getProductionUnitAI` (CvCity.cpp:2651-2675).
    pub fn city_production_unit_ai(&self, c: CityId) -> Option<UnitAIType> {
        let o = self.cities[c].head_order()?;
        if o.order_type == OrderType::ORDER_TRAIN {
            opt_from_i32(o.data2)
        } else {
            None
        }
    }

    /// C++ `CvCity::getProductionBuilding` (CvCity.cpp:2678-2704).
    pub fn city_production_building(&self, c: CityId) -> Option<BuildingType> {
        let o = self.cities[c].head_order()?;
        (o.order_type == OrderType::ORDER_CONSTRUCT).then(|| order_type_data(o))
    }

    /// C++ `CvCity::getProductionProject` (CvCity.cpp:2707-2733).
    pub fn city_production_project(&self, c: CityId) -> Option<ProjectType> {
        let o = self.cities[c].head_order()?;
        (o.order_type == OrderType::ORDER_CREATE).then(|| order_type_data(o))
    }

    /// C++ `CvCity::getProductionProcess` (CvCity.cpp:2736-2760).
    pub fn city_production_process(&self, c: CityId) -> Option<ProcessType> {
        let o = self.cities[c].head_order()?;
        (o.order_type == OrderType::ORDER_MAINTAIN).then(|| order_type_data(o))
    }

    /// C++ `CvCity::getGeneralProductionTurnsLeft` (CvCity.cpp:2797-2828).
    pub fn city_general_production_turns_left(&self, c: CityId) -> i32 {
        let Some(o) = self.cities[c].head_order() else { return 0 };
        match o.order_type {
            OrderType::ORDER_TRAIN => self.city_production_turns_left_unit(c, order_type_data(o), 0),
            OrderType::ORDER_CONSTRUCT => self.city_production_turns_left_building(c, order_type_data(o), 0),
            OrderType::ORDER_CREATE => self.city_production_turns_left_project(c, order_type_data(o), 0),
            _ => 0,
        }
    }

    /// C++ `CvCity::isFoodProduction()` (CvCity.cpp:2865-2889).
    pub fn city_is_food_production(&self, c: CityId) -> bool {
        match self.city_production_unit(c) {
            Some(u) => self.city_is_food_production_unit(c, u),
            None => false,
        }
    }

    /// C++ `CvCity::isFoodProduction(UnitTypes)` (CvCity.cpp:2892-2908).
    pub fn city_is_food_production_unit(&self, c: CityId, unit: UnitType) -> bool {
        todo!("CvCity::isFoodProduction")
    }

    /// C++ `CvCity::getFirstUnitOrder` (CvCity.cpp:2911-2933). C++ -1 = `None`.
    pub fn city_first_unit_order(&self, c: CityId, unit: UnitType) -> Option<usize> {
        self.cities[c].order_queue.iter().position(|o| o.order_type == OrderType::ORDER_TRAIN && o.data1 == unit.i())
    }

    /// C++ `CvCity::getFirstBuildingOrder` (CvCity.cpp:2936-2958). C++ -1 = `None`.
    pub fn city_first_building_order(&self, c: CityId, building: BuildingType) -> Option<usize> {
        self.cities[c].order_queue.iter().position(|o| o.order_type == OrderType::ORDER_CONSTRUCT && o.data1 == building.i())
    }

    /// C++ `CvCity::getFirstProjectOrder` (CvCity.cpp:2961-2983). C++ -1 = `None`.
    pub fn city_first_project_order(&self, c: CityId, project: ProjectType) -> Option<usize> {
        self.cities[c].order_queue.iter().position(|o| o.order_type == OrderType::ORDER_CREATE && o.data1 == project.i())
    }

    /// C++ `CvCity::getNumTrainUnitAI` (CvCity.cpp:2986-3006).
    pub fn city_num_train_unit_ai(&self, c: CityId, unit_ai: UnitAIType) -> i32 {
        self.cities[c].order_queue.iter().filter(|o| o.order_type == OrderType::ORDER_TRAIN && o.data2 == unit_ai.i()).count() as i32
    }

    /// C++ `CvCity::getProduction` (CvCity.cpp:3009-3039): progress on the head order.
    pub fn city_production(&self, c: CityId) -> i32 {
        let city = &self.cities[c];
        let Some(o) = city.head_order() else { return 0 };
        match o.order_type {
            OrderType::ORDER_TRAIN => city.unit_production[order_type_data::<UnitType>(o)],
            OrderType::ORDER_CONSTRUCT => city.building_production[order_type_data::<BuildingType>(o)],
            OrderType::ORDER_CREATE => city.project_production[order_type_data::<ProjectType>(o)],
            _ => 0,
        }
    }

    /// C++ `CvCity::getProductionNeeded()` (CvCity.cpp:3042-3072). `MAX_INT` when there is
    /// no head order or it is a process.
    pub fn city_production_needed(&self, c: CityId) -> i32 {
        let Some(o) = self.cities[c].head_order() else { return MAX_INT };
        match o.order_type {
            OrderType::ORDER_TRAIN => self.city_production_needed_unit(c, order_type_data(o)),
            OrderType::ORDER_CONSTRUCT => self.city_production_needed_building(c, order_type_data(o)),
            OrderType::ORDER_CREATE => self.city_production_needed_project(c, order_type_data(o)),
            _ => MAX_INT,
        }
    }

    /// C++ `CvCity::getProductionNeeded(UnitTypes)` (CvCity.cpp:3074-3077).
    pub fn city_production_needed_unit(&self, c: CityId, unit: UnitType) -> i32 {
        todo!("CvCity::getProductionNeeded")
    }

    /// C++ `CvCity::getProductionNeeded(BuildingTypes)` (CvCity.cpp:3079-3101). Python
    /// `getBuildingCostMod` callback.
    pub fn city_production_needed_building(&self, c: CityId, building: BuildingType) -> i32 {
        todo!("CvCity::getProductionNeeded")
    }

    /// C++ `CvCity::getProductionNeeded(ProjectTypes)` (CvCity.cpp:3103-3106).
    pub fn city_production_needed_project(&self, c: CityId, project: ProjectType) -> i32 {
        todo!("CvCity::getProductionNeeded")
    }

    /// C++ `CvCity::getProductionTurnsLeft()` (CvCity.cpp:3108-3138). `MAX_INT` when there is
    /// no head order or it is a process.
    pub fn city_production_turns_left(&self, c: CityId) -> i32 {
        let Some(o) = self.cities[c].head_order() else { return MAX_INT };
        match o.order_type {
            OrderType::ORDER_TRAIN => self.city_production_turns_left_unit(c, order_type_data(o), 0),
            OrderType::ORDER_CONSTRUCT => self.city_production_turns_left_building(c, order_type_data(o), 0),
            OrderType::ORDER_CREATE => self.city_production_turns_left_project(c, order_type_data(o), 0),
            _ => MAX_INT,
        }
    }

    /// C++ `CvCity::getProductionTurnsLeft(UnitTypes, int iNum)` (CvCity.cpp:3141-3161).
    /// `num` is the position in the queue being asked about (0 = head).
    pub fn city_production_turns_left_unit(&self, c: CityId, unit: UnitType, num: i32) -> i32 {
        let mut production = 0;
        if self.city_first_unit_order(c, unit).map_or(true, |i| i as i32 == num) {
            production += self.cities[c].unit_production[unit];
        }
        let production_needed = self.city_production_needed_unit(c, unit);
        let production_modifier = self.city_production_modifier_unit(c, unit);
        let food_production = self.city_is_food_production_unit(c, unit);
        let first_difference = self.city_production_difference(c, production_needed, production, production_modifier, food_production, num == 0);
        let difference = self.city_production_difference(c, production_needed, production, production_modifier, food_production, false);
        self.city_production_turns_left_raw(c, production_needed, production, first_difference, difference)
    }

    /// C++ `CvCity::getProductionTurnsLeft(BuildingTypes, int iNum)` (CvCity.cpp:3164-3185).
    pub fn city_production_turns_left_building(&self, c: CityId, building: BuildingType, num: i32) -> i32 {
        let mut production = 0;
        if self.city_first_building_order(c, building).map_or(true, |i| i as i32 == num) {
            production += self.cities[c].building_production[building];
        }
        let production_needed = self.city_production_needed_building(c, building);
        let production_modifier = self.city_production_modifier_building(c, building);
        let first_difference = self.city_production_difference(c, production_needed, production, production_modifier, false, num == 0);
        let difference = self.city_production_difference(c, production_needed, production, production_modifier, false, false);
        self.city_production_turns_left_raw(c, production_needed, production, first_difference, difference)
    }

    /// C++ `CvCity::getProductionTurnsLeft(ProjectTypes, int iNum)` (CvCity.cpp:3188-3208).
    pub fn city_production_turns_left_project(&self, c: CityId, project: ProjectType, num: i32) -> i32 {
        let mut production = 0;
        if self.city_first_project_order(c, project).map_or(true, |i| i as i32 == num) {
            production += self.cities[c].project_production[project];
        }
        let production_needed = self.city_production_needed_project(c, project);
        let production_modifier = self.city_production_modifier_project(c, project);
        let first_difference = self.city_production_difference(c, production_needed, production, production_modifier, false, num == 0);
        let difference = self.city_production_difference(c, production_needed, production, production_modifier, false, false);
        self.city_production_turns_left_raw(c, production_needed, production, first_difference, difference)
    }

    /// C++ `CvCity::getProductionTurnsLeft(int, int, int, int)` (CvCity.cpp:3211-3233).
    pub fn city_production_turns_left_raw(&self, c: CityId, production_needed: i32, production: i32, first_production_difference: i32, production_difference: i32) -> i32 {
        let production_left = (production_needed - production - first_production_difference).max(0);
        if production_difference == 0 {
            return production_left + 1;
        }
        let mut turns_left = production_left / production_difference;
        if turns_left * production_difference < production_left {
            turns_left += 1;
        }
        turns_left += 1;
        turns_left.max(1)
    }

    /// C++ `CvCity::setProduction` (CvCity.cpp:3236-3250): sets progress on the head order.
    pub fn city_set_production(&mut self, c: CityId, new_value: i32) {
        let city = &mut self.cities[c];
        let Some((order_type, data1)) = city.head_order().map(|o| (o.order_type, o.data1)) else { return };
        match order_type {
            OrderType::ORDER_TRAIN => city.unit_production[UnitType::from_usize(data1 as usize)] = new_value,
            OrderType::ORDER_CONSTRUCT => city.building_production[BuildingType::from_usize(data1 as usize)] = new_value,
            OrderType::ORDER_CREATE => city.project_production[ProjectType::from_usize(data1 as usize)] = new_value,
            _ => {}
        }
    }

    /// C++ `CvCity::changeProduction` (CvCity.cpp:3253-3267).
    pub fn city_change_production(&mut self, c: CityId, change: i32) {
        let city = &mut self.cities[c];
        let Some((order_type, data1)) = city.head_order().map(|o| (o.order_type, o.data1)) else { return };
        match order_type {
            OrderType::ORDER_TRAIN => city.unit_production[UnitType::from_usize(data1 as usize)] += change,
            OrderType::ORDER_CONSTRUCT => city.building_production[BuildingType::from_usize(data1 as usize)] += change,
            OrderType::ORDER_CREATE => city.project_production[ProjectType::from_usize(data1 as usize)] += change,
            _ => {}
        }
    }

    /// C++ `CvCity::getProductionModifier()` (CvCity.cpp:3270-3300).
    pub fn city_production_modifier(&self, c: CityId) -> i32 {
        let Some(o) = self.cities[c].head_order() else { return 0 };
        match o.order_type {
            OrderType::ORDER_TRAIN => self.city_production_modifier_unit(c, order_type_data(o)),
            OrderType::ORDER_CONSTRUCT => self.city_production_modifier_building(c, order_type_data(o)),
            OrderType::ORDER_CREATE => self.city_production_modifier_project(c, order_type_data(o)),
            _ => 0,
        }
    }

    /// C++ `CvCity::getProductionModifier(UnitTypes)` (CvCity.cpp:3303-3337). FFH: not
    /// clamped at 0.
    pub fn city_production_modifier_unit(&self, c: CityId, unit: UnitType) -> i32 {
        todo!("CvCity::getProductionModifier")
    }

    /// C++ `CvCity::getProductionModifier(BuildingTypes)` (CvCity.cpp:3340-3361).
    pub fn city_production_modifier_building(&self, c: CityId, building: BuildingType) -> i32 {
        todo!("CvCity::getProductionModifier")
    }

    /// C++ `CvCity::getProductionModifier(ProjectTypes)` (CvCity.cpp:3364-3385).
    pub fn city_production_modifier_project(&self, c: CityId, project: ProjectType) -> i32 {
        todo!("CvCity::getProductionModifier")
    }

    /// C++ `CvCity::getProductionDifference` (CvCity.cpp:3388-3413): production per turn
    /// towards an item (FFH: disabled production, unhappy production).
    pub fn city_production_difference(&self, c: CityId, production_needed: i32, production: i32, production_modifier: i32, food_production: bool, overflow: bool) -> i32 {
        todo!("CvCity::getProductionDifference")
    }

    /// C++ `CvCity::getCurrentProductionDifference` (CvCity.cpp:3416-3419).
    pub fn city_current_production_difference(&self, c: CityId, ignore_food: bool, overflow: bool) -> i32 {
        let food_production = !ignore_food && self.city_is_food_production(c);
        self.city_production_difference(c, self.city_production_needed(c), self.city_production(c), self.city_production_modifier(c), food_production, overflow)
    }

    /// C++ `CvCity::getExtraProductionDifference(int)` (CvCity.cpp:3422-3425).
    pub fn city_extra_production_difference(&self, c: CityId, extra: i32) -> i32 {
        self.city_extra_production_difference_modifier(c, extra, self.city_production_modifier(c))
    }

    /// C++ `CvCity::getExtraProductionDifference(int, UnitTypes)` (CvCity.cpp:3427-3430).
    pub fn city_extra_production_difference_unit(&self, c: CityId, extra: i32, unit: UnitType) -> i32 {
        self.city_extra_production_difference_modifier(c, extra, self.city_production_modifier_unit(c, unit))
    }

    /// C++ `CvCity::getExtraProductionDifference(int, BuildingTypes)` (CvCity.cpp:3432-3435).
    pub fn city_extra_production_difference_building(&self, c: CityId, extra: i32, building: BuildingType) -> i32 {
        self.city_extra_production_difference_modifier(c, extra, self.city_production_modifier_building(c, building))
    }

    /// C++ `CvCity::getExtraProductionDifference(int, ProjectTypes)` (CvCity.cpp:3437-3440).
    pub fn city_extra_production_difference_project(&self, c: CityId, extra: i32, project: ProjectType) -> i32 {
        self.city_extra_production_difference_modifier(c, extra, self.city_production_modifier_project(c, project))
    }

    /// C++ `CvCity::getExtraProductionDifference(int, int iModifier)` (CvCity.cpp:3442-3445).
    pub fn city_extra_production_difference_modifier(&self, c: CityId, extra: i32, modifier: i32) -> i32 {
        (extra * self.city_base_yield_rate_modifier(c, YieldType::YIELD_PRODUCTION, modifier)) / 100
    }

    /// C++ `CvCity::canHurry` (CvCity.cpp:3448-3491).
    pub fn city_can_hurry(&self, c: CityId, hurry: HurryType, test_visible: bool) -> bool {
        todo!("CvCity::canHurry")
    }

    /// C++ `CvCity::canHurryUnit` (CvCity.cpp:3493-3521).
    pub fn city_can_hurry_unit(&self, c: CityId, hurry: HurryType, unit: UnitType, ignore_new: bool) -> bool {
        todo!("CvCity::canHurryUnit")
    }

    /// C++ `CvCity::canHurryBuilding` (CvCity.cpp:3523-3551).
    pub fn city_can_hurry_building(&self, c: CityId, hurry: HurryType, building: BuildingType, ignore_new: bool) -> bool {
        todo!("CvCity::canHurryBuilding")
    }

    /// C++ `CvCity::hurry` (CvCity.cpp:3554-3583). Python event `cityHurry`.
    pub fn city_hurry(&mut self, c: CityId, hurry: HurryType) {
        todo!("CvCity::hurry")
    }

    /// C++ `CvCity::getConscriptUnit` (CvCity.cpp:3586-3630). Python `getConscriptUnitType`
    /// callback may override the choice.
    pub fn city_conscript_unit(&self, c: CityId) -> Option<UnitType> {
        todo!("CvCity::getConscriptUnit")
    }

    /// C++ `CvCity::getConscriptPopulation` (CvCity.cpp:3633-3650).
    pub fn city_conscript_population(&self, c: CityId) -> i32 {
        todo!("CvCity::getConscriptPopulation")
    }

    /// C++ `CvCity::conscriptMinCityPopulation` (CvCity.cpp:3653-3662).
    pub fn city_conscript_min_city_population(&self, c: CityId) -> i32 {
        todo!("CvCity::conscriptMinCityPopulation")
    }

    /// C++ `CvCity::flatConscriptAngerLength` (CvCity.cpp:3665-3675).
    pub fn city_flat_conscript_anger_length(&self, c: CityId) -> i32 {
        todo!("CvCity::flatConscriptAngerLength")
    }

    /// C++ `CvCity::canConscript` (CvCity.cpp:3678-3723).
    pub fn city_can_conscript(&self, c: CityId) -> bool {
        todo!("CvCity::canConscript")
    }

    /// C++ `CvCity::initConscriptedUnit` (CvCity.cpp:3725-3767). The UnitAI is picked with
    /// `CvPlayerAI::AI_unitValue` (AI: a controller/AI-crate decision in this port).
    pub fn city_init_conscripted_unit(&mut self, c: CityId) -> Option<UnitId> {
        todo!("CvCity::initConscriptedUnit")
    }

    /// C++ `CvCity::conscript` (CvCity.cpp:3770-3794).
    pub fn city_conscript(&mut self, c: CityId) {
        todo!("CvCity::conscript")
    }

    /// C++ `CvCity::processProcess` (CvCity.cpp:4209-4217).
    pub fn city_process_process(&mut self, c: CityId, process: ProcessType, change: i32) {
        todo!("CvCity::processProcess")
    }

    /// C++ `CvCity::productionLeft` (CvCity.cpp:4971-4974).
    pub fn city_production_left(&self, c: CityId) -> i32 {
        self.city_production_needed(c) - self.city_production(c)
    }

    /// C++ `CvCity::getHurryCostModifier(bool)` (CvCity.cpp:4976-5004). C++ default
    /// `bIgnoreNew = false`.
    pub fn city_hurry_cost_modifier(&self, c: CityId, ignore_new: bool) -> i32 {
        let Some(o) = self.cities[c].head_order() else { return 100 };
        match o.order_type {
            OrderType::ORDER_TRAIN => self.city_hurry_cost_modifier_unit(c, order_type_data(o), ignore_new),
            OrderType::ORDER_CONSTRUCT => self.city_hurry_cost_modifier_building(c, order_type_data(o), ignore_new),
            _ => 100,
        }
    }

    /// C++ `CvCity::getHurryCostModifier(UnitTypes, bool)` (CvCity.cpp:5006-5009).
    pub fn city_hurry_cost_modifier_unit(&self, c: CityId, unit: UnitType, ignore_new: bool) -> i32 {
        todo!("CvCity::getHurryCostModifier")
    }

    /// C++ `CvCity::getHurryCostModifier(BuildingTypes, bool)` (CvCity.cpp:5011-5014).
    pub fn city_hurry_cost_modifier_building(&self, c: CityId, building: BuildingType, ignore_new: bool) -> i32 {
        todo!("CvCity::getHurryCostModifier")
    }

    /// C++ `CvCity::getHurryCostModifier(int, int, bool)` (CvCity.cpp:5016-5032).
    pub fn city_hurry_cost_modifier_raw(&self, c: CityId, base_modifier: i32, production: i32, ignore_new: bool) -> i32 {
        todo!("CvCity::getHurryCostModifier")
    }

    /// C++ `CvCity::hurryCost` (CvCity.cpp:5035-5038).
    pub fn city_hurry_cost(&self, c: CityId, extra: bool) -> i32 {
        self.city_hurry_cost_raw(c, extra, self.city_production_left(c), self.city_hurry_cost_modifier(c, false), self.city_production_modifier(c))
    }

    /// C++ `CvCity::getHurryCost(bool, UnitTypes, bool)` (CvCity.cpp:5040-5045).
    pub fn city_hurry_cost_unit(&self, c: CityId, extra: bool, unit: UnitType, ignore_new: bool) -> i32 {
        let production_left = self.city_production_needed_unit(c, unit) - self.cities[c].unit_production[unit];
        self.city_hurry_cost_raw(c, extra, production_left, self.city_hurry_cost_modifier_unit(c, unit, ignore_new), self.city_production_modifier_unit(c, unit))
    }

    /// C++ `CvCity::getHurryCost(bool, BuildingTypes, bool)` (CvCity.cpp:5047-5052).
    pub fn city_hurry_cost_building(&self, c: CityId, extra: bool, building: BuildingType, ignore_new: bool) -> i32 {
        let production_left = self.city_production_needed_building(c, building) - self.cities[c].building_production[building];
        self.city_hurry_cost_raw(c, extra, production_left, self.city_hurry_cost_modifier_building(c, building, ignore_new), self.city_production_modifier_building(c, building))
    }

    /// C++ `CvCity::getHurryCost(bool, int, int, int)` (CvCity.cpp:5054-5071).
    pub fn city_hurry_cost_raw(&self, c: CityId, extra: bool, production_left: i32, hurry_modifier: i32, modifier: i32) -> i32 {
        let mut production = (production_left * hurry_modifier + 99) / 100; // round up
        if extra {
            let extra_production = self.city_extra_production_difference_modifier(c, production, modifier);
            if extra_production > 0 {
                let adjusted_prod = production * production;
                // round up
                production = (adjusted_prod + (extra_production - 1)) / extra_production;
            }
        }
        production.max(0)
    }

    /// C++ `CvCity::hurryGold` (CvCity.cpp:5073-5076).
    pub fn city_hurry_gold(&self, c: CityId, hurry: HurryType) -> i32 {
        self.city_hurry_gold_for_cost(c, hurry, self.city_hurry_cost(c, false))
    }

    /// C++ `CvCity::getHurryGold(HurryTypes, int iHurryCost)` (CvCity.cpp:5078-5090).
    pub fn city_hurry_gold_for_cost(&self, c: CityId, hurry: HurryType, hurry_cost: i32) -> i32 {
        todo!("CvCity::getHurryGold")
    }

    /// C++ `CvCity::hurryPopulation` (CvCity.cpp:5093-5096).
    pub fn city_hurry_population(&self, c: CityId, hurry: HurryType) -> i32 {
        self.city_hurry_population_for_cost(c, hurry, self.city_hurry_cost(c, true))
    }

    /// C++ `CvCity::getHurryPopulation(HurryTypes, int iHurryCost)` (CvCity.cpp:5098-5108).
    pub fn city_hurry_population_for_cost(&self, c: CityId, hurry: HurryType, hurry_cost: i32) -> i32 {
        todo!("CvCity::getHurryPopulation")
    }

    /// C++ `CvCity::hurryProduction` (CvCity.cpp:5110-5125).
    pub fn city_hurry_production(&self, c: CityId, hurry: HurryType) -> i32 {
        todo!("CvCity::hurryProduction")
    }

    /// C++ `CvCity::flatHurryAngerLength` (CvCity.cpp:5128-5139).
    pub fn city_flat_hurry_anger_length(&self, c: CityId) -> i32 {
        todo!("CvCity::flatHurryAngerLength")
    }

    /// C++ `CvCity::hurryAngerLength` (CvCity.cpp:5142-5152).
    pub fn city_hurry_anger_length(&self, c: CityId, hurry: HurryType) -> i32 {
        todo!("CvCity::hurryAngerLength")
    }

    /// C++ `CvCity::maxHurryPopulation` (CvCity.cpp:5155-5158).
    pub fn city_max_hurry_population(&self, c: CityId) -> i32 {
        self.cities[c].population / 2
    }

    /// C++ `CvCity::changeOverflowProduction` (CvCity.cpp:7093-7098): stores `change`
    /// un-modified by `production_modifier` (it is re-applied when spent).
    pub fn city_change_overflow_production(&mut self, c: CityId, change: i32, production_modifier: i32) {
        let overflow = (100 * change) / self.city_base_yield_rate_modifier(c, YieldType::YIELD_PRODUCTION, production_modifier).max(1);
        self.cities[c].overflow_production += overflow;
    }

    /// C++ `CvCity::setProductionAutomated` (CvCity.cpp:7717-7739). Calls
    /// `AI_chooseProduction` (-> `Controller::choose_production`) when the queue is empty.
    pub fn city_set_production_automated(&mut self, c: CityId, new_value: bool, clear: bool) {
        todo!("CvCity::setProductionAutomated")
    }

    /// C++ `CvCity::isFreePromotion` (CvCity.cpp:10241-10244).
    pub fn city_is_free_promotion(&self, c: CityId, promotion: PromotionType) -> bool {
        self.cities[c].free_promotion_count[promotion] > 0
    }

    /// C++ `CvCity::isUnhappyProduction` (CvCity.cpp:14657-14660, FFH).
    pub fn city_is_unhappy_production(&self, c: CityId) -> bool {
        self.cities[c].unhappy_production != 0
    }
}
