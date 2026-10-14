//! CvCity: build checks (`canTrain`/`canConstruct`/`canCreate`/`canMaintain`), production
//! queries and modifiers, hurry and conscription.
//!
//! The build queue itself (`pushOrder`/`popOrder`, `doProduction`, ...) is in `turn.rs`.

use super::{ai_choose_production, building_class_of, order_type_data, unit_class_of, unit_domain_of};
use crate::defs::MAX_INT;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use crate::utils::{is_limited_project, is_limited_unit_class, is_limited_wonder_class, is_national_wonder_class, is_team_wonder_class, is_world_unit_class, is_world_wonder_class};
use ffh_data::*;

impl Game {
    /// C++ `CvCity::chooseProduction` (CvCity.cpp:1330-1362). C++ opens the
    /// choose-production popup for the human owner (the arguments are the item just
    /// finished, shown as a hint). In this port the decision goes to the owner's controller
    /// (`Controller::choose_production`).
    pub fn city_choose_production(&mut self, c: CityId, train_unit: Option<UnitType>, construct_building: Option<BuildingType>, create_project: Option<ProjectType>, finish: bool, front: bool) {
        // C++ queues a BUTTONPOPUP_CHOOSEPRODUCTION popup (data: this city, the order type and
        // item just finished as a hint, bFinish, bFront = popup position). Those are display
        // hints only; the owner's answer (an order pushed through `city_push_order`) is the
        // controller's decision.
        let _ = (train_unit, construct_building, create_project, finish, front);
        let owner = self.cities[c].owner;
        self.controller(owner).choose_production(self, c);
    }

    /// C++ `CvCity::allUpgradesAvailable` (CvCity.cpp:1677-1747). Recursive; C++ default
    /// `iUpgradeCount = 0`.
    pub fn city_all_upgrades_available(&self, c: CityId, unit: UnitType, upgrade_count: i32) -> Option<UnitType> {
        let d = self.d;

        if upgrade_count > d.unit_classes.len() as i32 {
            return None;
        }

        let mut upgrade_unit = None;
        let mut upgrade_found = false;
        let mut upgrade_available = false;
        let mut upgrade_unavailable = false;

        for unit_class in d.unit_classes.keys() {
            if d.units[unit].upgrade_unit_class[unit_class] {
                if let Some(loop_unit) = d.civ_unit(self.city_civilization_type(c), unit_class) {
                    upgrade_found = true;

                    match self.city_all_upgrades_available(c, loop_unit, upgrade_count + 1) {
                        Some(temp_unit) => {
                            upgrade_unit = Some(temp_unit);
                            upgrade_available = true;
                        }
                        None => upgrade_unavailable = true,
                    }
                }
            }
        }

        if upgrade_count > 0 {
            if upgrade_found && upgrade_available {
                return upgrade_unit;
            }

            if self.city_can_train(c, unit, false, false, false, true) {
                return Some(unit);
            }
        } else if upgrade_found && !upgrade_unavailable {
            return upgrade_unit;
        }

        None
    }

    /// C++ `CvCity::isWorldWondersMaxed` (CvCity.cpp:1750-1768).
    pub fn city_is_world_wonders_maxed(&self, c: CityId) -> bool {
        if self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && self.city_is_human(c) {
            return false;
        }

        let max = self.d.defines.max_world_wonders_per_city;
        if max == -1 {
            return false;
        }

        self.cities[c].num_world_wonders >= max
    }

    /// C++ `CvCity::isTeamWondersMaxed` (CvCity.cpp:1771-1789).
    pub fn city_is_team_wonders_maxed(&self, c: CityId) -> bool {
        if self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && self.city_is_human(c) {
            return false;
        }

        let max = self.d.defines.max_team_wonders_per_city;
        if max == -1 {
            return false;
        }

        self.cities[c].num_team_wonders >= max
    }

    /// C++ `CvCity::isNationalWondersMaxed` (CvCity.cpp:1792-1807).
    pub fn city_is_national_wonders_maxed(&self, c: CityId) -> bool {
        let defines = &self.d.defines;
        let max_num_wonders = if self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && self.city_is_human(c) {
            defines.max_national_wonders_per_city_for_occ
        } else {
            defines.max_national_wonders_per_city
        };

        if max_num_wonders == -1 {
            return false;
        }

        self.cities[c].num_national_wonders >= max_num_wonders
    }

    /// C++ `CvCity::isBuildingsMaxed` (CvCity.cpp:1810-1828).
    pub fn city_is_buildings_maxed(&self, c: CityId) -> bool {
        if self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && self.city_is_human(c) {
            return false;
        }

        let max = self.d.defines.max_buildings_per_city;
        if max == -1 {
            return false;
        }

        self.cities[c].num_buildings >= max
    }

    /// C++ `CvCity::canTrain(UnitTypes, ...)` (CvCity.cpp:1832-1850). FFH: min level and
    /// settlement checks, then `canUpgrade`. (C++ returns false for `NO_UNIT`; callers holding
    /// an `Option` check it first.)
    pub fn city_can_train(&self, c: CityId, unit: UnitType, cont: bool, test_visible: bool, ignore_cost: bool, ignore_upgrades: bool) -> bool {
        let min_level = self.d.units[unit].min_level;
        if min_level > 0 && (self.city_is_human(c) || !self.is_option(GameOptionType::GAMEOPTION_AI_NO_MINIMUM_LEVEL) || min_level == 1) {
            return false;
        }
        if self.cities[c].settlement {
            return false;
        }
        self.city_can_upgrade(c, unit, cont, test_visible, ignore_cost, ignore_upgrades)
    }

    /// C++ `CvCity::canUpgrade` (CvCity.cpp:1852-1968): the BTS `canTrain` body (Python
    /// `canTrain`/`cannotTrain` callbacks, player/plot checks, FFH civ/world-unit rules).
    pub fn city_can_upgrade(&self, c: CityId, unit: UnitType, cont: bool, test_visible: bool, ignore_cost: bool, ignore_upgrades: bool) -> bool {
        let d = self.d;
        let owner = self.cities[c].owner;

        if d.defines.use_can_train_callback != 0 && py_can_train(self, c, unit, cont, test_visible, ignore_cost, ignore_upgrades) {
            return true;
        }

        if !self.player_can_train(owner, unit, cont, test_visible, ignore_cost) {
            return false;
        }

        if !ignore_upgrades
            && self.city_all_upgrades_available(c, unit, 0).is_some()
            // FfH: Added by Kael 08/04/2007
            && !d.units[unit].never_obsolete
        {
            return false;
        }

        if !self.plot_can_train(self.city_plot(c), unit, cont, test_visible) {
            return false;
        }

        // FfH: Added by Kael 09/26/2007
        let info = &d.units[unit];
        if let Some(prereq_civ) = info.prereq_civ {
            if prereq_civ != self.city_civilization_type(c) {
                return false;
            }
        }
        if self.is_option(GameOptionType::GAMEOPTION_NO_SETTLERS) && info.found {
            return false;
        }
        let unit_class = unit_class_of(d, unit);
        let owner_civ_unit = self.player_get_civilization_type(owner).and_then(|civ| d.civ_unit(civ, unit_class));
        if is_world_unit_class(d, unit_class) {
            if owner_civ_unit != Some(unit) {
                return false;
            }
        } else {
            match info.upgrade_civ {
                None => {
                    if !(d.civ_unit(self.city_civilization_type(c), unit_class) == Some(unit) || owner_civ_unit == Some(unit)) {
                        return false;
                    }
                }
                Some(upgrade_civ) => {
                    if d.civ_unit(upgrade_civ, unit_class) != Some(unit) {
                        return false;
                    }
                }
            }
        }
        // FfH: End Add

        if d.defines.use_cannot_train_callback != 0 && self.py_cannot_train(c, unit, cont, test_visible, ignore_cost, ignore_upgrades) {
            return false;
        }

        true
    }

    /// C++ `CvCity::canTrain(UnitCombatTypes)` (CvCity.cpp:1970-1989).
    pub fn city_can_train_combat(&self, c: CityId, combat: UnitCombatType) -> bool {
        let d = self.d;
        for unit_class in d.unit_classes.keys() {
            if let Some(unit) = d.civ_unit(self.city_civilization_type(c), unit_class) {
                if d.units[unit].unit_combat_type == Some(combat) && self.city_can_train(c, unit, false, false, false, false) {
                    return true;
                }
            }
        }
        false
    }

    /// C++ `CvCity::canConstruct` (CvCity.cpp:1992-2309). Python `canConstruct` /
    /// `cannotConstruct` callbacks. (C++ returns false for `NO_BUILDING`.)
    pub fn city_can_construct(&self, c: CityId, building: BuildingType, cont: bool, test_visible: bool, ignore_cost: bool) -> bool {
        let d = self.d;
        let defines = &d.defines;
        let city = &self.cities[c];
        let owner = city.owner;
        let info = &d.buildings[building];

        // FfH: Modified by Kael 08/24/2007
        if city.settlement {
            return false;
        }

        if defines.use_can_construct_callback != 0 && py_can_construct(self, c, building, cont, test_visible, ignore_cost) {
            return true;
        }

        if !self.player_can_construct(owner, building, cont, test_visible, ignore_cost) {
            return false;
        }

        if self.city_num_building(c, building) >= defines.city_max_num_buildings {
            return false;
        }

        if info.is_prereq_religion && self.city_religion_count(c) > 0 {
            return false;
        }

        if info.is_state_religion {
            match self.player_get_state_religion(owner) {
                Some(state_religion) if city.has_religion[state_religion] => {}
                _ => return false,
            }
        }

        if let Some(religion) = info.prereq_religion {
            if !city.has_religion[religion] {
                return false;
            }
        }

        if let Some(corporation) = info.prereq_corporation {
            if !city.has_corporation[corporation] {
                return false;
            }
        }

        if let Some(corporation) = info.founds_corporation {
            if self.is_corporation_founded(corporation) {
                return false;
            }

            for loop_corporation in d.corporations.keys() {
                if self.city_is_headquarters(c, loop_corporation) && self.is_competing_corporation(loop_corporation, corporation) {
                    return false;
                }
            }
        }

        if !self.city_is_valid_building_location(c, building) {
            return false;
        }

        if info.government_center && self.city_is_government_center(c) {
            return false;
        }

        if !test_visible {
            if !cont && self.city_first_building_order(c, building).is_some() {
                return false;
            }

            let building_class = building_class_of(d, building);
            if !d.building_classes[building_class].no_limit {
                if is_world_wonder_class(d, building_class) {
                    if self.city_is_world_wonders_maxed(c) {
                        return false;
                    }
                } else if is_team_wonder_class(d, building_class) {
                    if self.city_is_team_wonders_maxed(c) {
                        return false;
                    }
                } else if is_national_wonder_class(d, building_class) {
                    if self.city_is_national_wonders_maxed(c) {
                        return false;
                    }
                } else if self.city_is_buildings_maxed(c) {
                    return false;
                }
            }

            if let Some(religion) = info.holy_city {
                if !self.city_is_holy_city(c, religion) {
                    return false;
                }
            }

            if let Some(bonus) = info.prereq_and_bonus {
                if !self.city_has_bonus(c, bonus) {
                    return false;
                }
            }

            if let Some(corporation) = info.founds_corporation {
                if self.is_corporation_founded(corporation) {
                    return false;
                }

                if self.player_is_no_corporations(owner) {
                    return false;
                }

                let num_prereq_bonuses = defines.num_corporation_prereq_bonuses.max(0) as usize;
                let valid = d.corporations[corporation].prereq_bonus.iter().take(num_prereq_bonuses).any(|&bonus| self.city_has_bonus(c, bonus));
                if !valid {
                    return false;
                }
            }

            let plot = self.city_plot(c);
            if self.plot_get_latitude(plot) > info.max_latitude {
                return false;
            }

            if self.plot_get_latitude(plot) < info.min_latitude {
                return false;
            }

            // FfH: Modified by Kael 08/24/2007 (the buildings' OR bonus requirements are AND
            // requirements)
            for bonus in info.prereq_or_bonuses.iter().flatten() {
                if !self.city_has_bonus(c, *bonus) {
                    return false;
                }
            }

            for loop_class in d.building_classes.keys() {
                if info.building_class_needed_in_city[loop_class] {
                    if let Some(prereq_building) = d.civ_building(self.city_civilization_type(c), loop_class) {
                        if self.city_num_building(c, prereq_building) == 0 {
                            return false;
                        }
                    }
                }
            }
        }

        // FfH: Added by Kael 08/04/2007
        if let Some(prereq_civ) = info.prereq_civ {
            if prereq_civ != self.city_civilization_type(c) {
                return false;
            }
        }

        if let Some(prereq_trait) = info.prereq_trait {
            if !self.city_has_trait(c, prereq_trait) {
                return false;
            }
        }
        if info.plot_radius != 0 && info.plot_radius == city.plot_radius {
            return false;
        }
        let building_class = building_class_of(d, building);
        if d.civ_building(self.city_civilization_type(c), building_class) != Some(building) {
            return false;
        }
        // FfH: End Add

        if defines.use_cannot_construct_callback != 0 && self.py_cannot_construct(c, building, cont, test_visible, ignore_cost) {
            return false;
        }

        true
    }

    /// C++ `CvCity::canCreate` (CvCity.cpp:2312-2374). Python `canCreate`/`cannotCreate`.
    pub fn city_can_create(&self, c: CityId, project: ProjectType, cont: bool, test_visible: bool) -> bool {
        let info = &self.d.projects[project];

        // FfH: Added by Kael 04/11/2008
        if self.is_option(GameOptionType::GAMEOPTION_NO_PROJECTS) {
            return false;
        }
        if self.cities[c].settlement {
            return false;
        }
        if let Some(prereq_civ) = info.prereq_civilization {
            if prereq_civ != self.city_civilization_type(c) {
                return false;
            }
        }
        if info.prereq_global_counter != 0 && info.prereq_global_counter > self.get_global_counter() {
            return false;
        }
        // FfH: End Add

        if py_can_create(self, c, project, cont, test_visible) {
            return true;
        }

        if !self.player_can_create(self.cities[c].owner, project, cont, test_visible) {
            return false;
        }

        if self.py_cannot_create(c, project, cont, test_visible) {
            return false;
        }

        true
    }

    /// C++ `CvCity::canMaintain` (CvCity.cpp:2377-2419). Python `canMaintain`/`cannotMaintain`.
    pub fn city_can_maintain(&self, c: CityId, process: ProcessType, cont: bool) -> bool {
        // FfH: Added by Kael 11/08/2007
        if self.cities[c].settlement {
            return false;
        }

        if py_can_maintain(self, c, process, cont) {
            return true;
        }

        if !self.player_can_maintain(self.cities[c].owner, process, cont) {
            return false;
        }

        if py_cannot_maintain(self, c, process, cont) {
            return false;
        }

        true
    }

    /// C++ `CvCity::isProduction` (CvCity.cpp:2451-2454).
    pub fn city_is_production(&self, c: CityId) -> bool {
        !self.cities[c].order_queue.is_empty()
    }

    /// C++ `CvCity::isProductionLimited` (CvCity.cpp:2457-2487).
    pub fn city_is_production_limited(&self, c: CityId) -> bool {
        let d = self.d;
        let Some(o) = self.cities[c].head_order() else { return false };
        match o.order_type {
            OrderType::ORDER_TRAIN => is_limited_unit_class(d, unit_class_of(d, order_type_data(o))),
            OrderType::ORDER_CONSTRUCT => is_limited_wonder_class(d, building_class_of(d, order_type_data(o))),
            OrderType::ORDER_CREATE => is_limited_project(d, order_type_data(o)),
            _ => false,
        }
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
        let d = self.d;
        let city = &self.cities[c];
        let player = &self.players[city.owner];

        let mut experience = city.free_experience;
        experience += player.free_experience;

        if let Some(unit) = unit {
            if let Some(unit_combat) = d.units[unit].unit_combat_type {
                experience += city.unit_combat_free_experience[unit_combat];
            }
            experience += city.domain_free_experience[unit_domain_of(d, unit).idx()];

            experience += city.specialist_free_experience;
        }

        if let Some(state_religion) = self.player_get_state_religion(city.owner) {
            if city.has_religion[state_religion] {
                experience += player.state_religion_free_experience;
            }
        }

        experience.max(0)
    }

    /// C++ `CvCity::addProductionExperience` (CvCity.cpp:2601-2621).
    pub fn city_add_production_experience(&mut self, c: CityId, unit: UnitId, conscript: bool) {
        if self.unit_can_acquire_promotion_any(unit) {
            let experience = self.city_production_experience(c, Some(self.units[unit].unit_type)) / if conscript { 2 } else { 1 };
            self.unit_change_experience(unit, experience, -1, false, false, false);
        }

        for promotion in self.d.promotions.keys() {
            if self.city_is_free_promotion(c, promotion) {
                if let Some(unit_combat) = self.unit_get_unit_combat_type(unit) {
                    if self.d.promotions[promotion].unit_combat[unit_combat] {
                        self.unit_set_has_promotion(unit, promotion, true);
                    }
                }
            }
        }
        self.unit_test_promotion_ready(unit);
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
        let info = &self.d.units[unit];
        if info.food_production {
            return true;
        }

        if self.player_is_military_food_production(self.cities[c].owner) && info.military_production {
            return true;
        }

        false
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
        self.player_get_production_needed_unit(self.cities[c].owner, unit)
    }

    /// C++ `CvCity::getProductionNeeded(BuildingTypes)` (CvCity.cpp:3079-3101). Python
    /// `getBuildingCostMod` callback.
    pub fn city_production_needed_building(&self, c: CityId, building: BuildingType) -> i32 {
        let owner = self.cities[c].owner;
        let mut production_needed = self.player_get_production_needed_building(owner, building);

        // Python cost modifier
        if self.d.defines.use_get_building_cost_mod_callback != 0 {
            let result = self.py_get_building_cost_mod(owner, c, building);

            // C++ quirk: the result is applied as a percentage, but FFH2's Python returns a
            // cost (Gambling House with a gambling ring: iCost / 4), so the house costs
            // iCost / 4 percent of its normal cost.
            if result > 1 {
                production_needed *= result;
                production_needed /= 100;
            }
        }

        production_needed
    }

    /// C++ `CvCity::getProductionNeeded(ProjectTypes)` (CvCity.cpp:3103-3106).
    pub fn city_production_needed_project(&self, c: CityId, project: ProjectType) -> i32 {
        self.player_get_production_needed_project(self.cities[c].owner, project)
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
    pub fn city_production_turns_left_raw(&self, _c: CityId, production_needed: i32, production: i32, first_production_difference: i32, production_difference: i32) -> i32 {
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
        let d = self.d;
        let city = &self.cities[c];
        let info = &d.units[unit];

        let mut multiplier = self.player_get_production_modifier_unit(city.owner, unit);

        multiplier += city.domain_production_modifier[unit_domain_of(d, unit).idx()];

        if info.military_production {
            multiplier += city.military_production_modifier;
        }

        for bonus in d.bonuses.keys() {
            if self.city_has_bonus(c, bonus) {
                multiplier += info.bonus_production_modifier[bonus];
            }
        }

        if let Some(state_religion) = self.player_get_state_religion(city.owner) {
            if city.has_religion[state_religion] {
                multiplier += self.players[city.owner].state_religion_unit_production_modifier;
            }
        }

        // FfH: Modified by Kael 02/11/2009 (was `std::max(0, iMultiplier)`)
        multiplier
    }

    /// C++ `CvCity::getProductionModifier(BuildingTypes)` (CvCity.cpp:3340-3361).
    pub fn city_production_modifier_building(&self, c: CityId, building: BuildingType) -> i32 {
        let d = self.d;
        let city = &self.cities[c];

        let mut multiplier = self.player_get_production_modifier_building(city.owner, building);

        for bonus in d.bonuses.keys() {
            if self.city_has_bonus(c, bonus) {
                multiplier += d.buildings[building].bonus_production_modifier[bonus];
            }
        }

        if let Some(state_religion) = self.player_get_state_religion(city.owner) {
            if city.has_religion[state_religion] {
                multiplier += self.players[city.owner].state_religion_building_production_modifier;
            }
        }

        multiplier.max(0)
    }

    /// C++ `CvCity::getProductionModifier(ProjectTypes)` (CvCity.cpp:3364-3385).
    pub fn city_production_modifier_project(&self, c: CityId, project: ProjectType) -> i32 {
        let d = self.d;
        let city = &self.cities[c];

        let mut multiplier = self.player_get_production_modifier_project(city.owner, project);

        // FfH: Modified by Kael 08/31/2007 (the "spaceship modifier" applies to all rituals,
        // not only to `isSpaceship()` projects)
        multiplier += city.space_production_modifier;

        for bonus in d.bonuses.keys() {
            if self.city_has_bonus(c, bonus) {
                multiplier += d.projects[project].bonus_production_modifier[bonus];
            }
        }

        multiplier.max(0)
    }

    /// C++ `CvCity::getProductionDifference` (CvCity.cpp:3388-3413): production per turn
    /// towards an item (FFH: disabled production, unhappy production).
    pub fn city_production_difference(&self, c: CityId, production_needed: i32, production: i32, production_modifier: i32, food_production: bool, overflow: bool) -> i32 {
        // C++ takes the needed/current production but does not use them.
        let _ = (production_needed, production);

        if self.city_is_disorder(c) {
            return 0;
        }

        let food = if food_production { (self.city_yield_rate(c, YieldType::YIELD_FOOD) - self.city_food_consumption(c, true, 0)).max(0) } else { 0 };

        let city = &self.cities[c];
        let overflow_production = if overflow { city.overflow_production + city.feature_production } else { 0 };

        // FfH: Modified by Kael 10/13/2007
        if self.players[city.owner].disable_production > 0 {
            return 0;
        }
        let mut unhappy_production = 0;
        if self.city_is_unhappy_production(c) {
            unhappy_production += self.city_unhappy_level(c, 0);
        }
        ((city.base_yield_rate[YieldType::YIELD_PRODUCTION.idx()] + overflow_production + unhappy_production) * self.city_base_yield_rate_modifier(c, YieldType::YIELD_PRODUCTION, production_modifier)) / 100 + food
        // FfH: End Modify
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
        let owner = self.cities[c].owner;

        if !self.player_can_hurry(owner, hurry) {
            return false;
        }

        if self.city_is_disorder(c) {
            return false;
        }

        if self.city_production(c) >= self.city_production_needed(c) {
            return false;
        }

        if !test_visible {
            if !self.city_is_production_unit(c) && !self.city_is_production_building(c) {
                return false;
            }

            if self.players[owner].gold < self.city_hurry_gold(c, hurry) {
                return false;
            }

            if self.city_max_hurry_population(c) < self.city_hurry_population(c, hurry) {
                return false;
            }
        }

        // FfH: Added by Kael 11/08/2007
        if self.cities[c].settlement {
            return false;
        }

        true
    }

    /// C++ `CvCity::canHurryUnit` (CvCity.cpp:3493-3521).
    pub fn city_can_hurry_unit(&self, c: CityId, hurry: HurryType, unit: UnitType, ignore_new: bool) -> bool {
        let owner = self.cities[c].owner;

        if !self.player_can_hurry(owner, hurry) {
            return false;
        }

        if self.city_is_disorder(c) {
            return false;
        }

        if self.cities[c].unit_production[unit] >= self.city_production_needed_unit(c, unit) {
            return false;
        }

        if self.players[owner].gold < self.city_hurry_gold_for_cost(c, hurry, self.city_hurry_cost_unit(c, false, unit, ignore_new)) {
            return false;
        }

        if self.city_max_hurry_population(c) < self.city_hurry_population_for_cost(c, hurry, self.city_hurry_cost_unit(c, true, unit, ignore_new)) {
            return false;
        }

        true
    }

    /// C++ `CvCity::canHurryBuilding` (CvCity.cpp:3523-3551).
    pub fn city_can_hurry_building(&self, c: CityId, hurry: HurryType, building: BuildingType, ignore_new: bool) -> bool {
        let owner = self.cities[c].owner;

        if !self.player_can_hurry(owner, hurry) {
            return false;
        }

        if self.city_is_disorder(c) {
            return false;
        }

        if self.cities[c].building_production[building] >= self.city_production_needed_building(c, building) {
            return false;
        }

        if self.players[owner].gold < self.city_hurry_gold_for_cost(c, hurry, self.city_hurry_cost_building(c, false, building, ignore_new)) {
            return false;
        }

        if self.city_max_hurry_population(c) < self.city_hurry_population_for_cost(c, hurry, self.city_hurry_cost_building(c, true, building, ignore_new)) {
            return false;
        }

        true
    }

    /// C++ `CvCity::hurry` (CvCity.cpp:3554-3583). Python event `cityHurry`.
    pub fn city_hurry(&mut self, c: CityId, hurry: HurryType) {
        if !self.city_can_hurry(c, hurry, false) {
            return;
        }

        let hurry_gold = self.city_hurry_gold(c, hurry);
        let hurry_population = self.city_hurry_population(c, hurry);
        let hurry_anger_length = self.city_hurry_anger_length(c, hurry);

        let production = self.city_hurry_production(c, hurry);
        self.city_change_production(c, production);

        let owner = self.cities[c].owner;
        self.players[owner].gold -= hurry_gold;
        self.city_change_population(c, -hurry_population);

        self.city_change_hurry_anger_timer(c, hurry_anger_length);

        // UI: selection buttons dirty bit for the active player's selected city.

        // Python Event
        self.ev_city_hurry(c, hurry);
    }

    /// C++ `CvCity::getConscriptUnit` (CvCity.cpp:3586-3630). Python `getConscriptUnitType`
    /// callback may override the choice.
    pub fn city_conscript_unit(&self, c: CityId) -> Option<UnitType> {
        let d = self.d;
        let mut best_value = 0;
        let mut best_unit = None;

        for unit_class in d.unit_classes.keys() {
            if let Some(loop_unit) = d.civ_unit(self.city_civilization_type(c), unit_class) {
                if self.city_can_train(c, loop_unit, false, false, false, false) {
                    let value = d.units[loop_unit].conscription_value;

                    if value > best_value {
                        best_value = value;
                        best_unit = Some(loop_unit);
                    }
                }
            }
        }

        // Allow the player to determine the conscripted unit type
        let conscript_unit = py_get_conscript_unit_type(self, self.cities[c].owner);

        if conscript_unit != -1 {
            best_unit = opt_from_i32(conscript_unit);
        }

        best_unit
    }

    /// C++ `CvCity::getConscriptPopulation` (CvCity.cpp:3633-3650).
    pub fn city_conscript_population(&self, c: CityId) -> i32 {
        let Some(conscript_unit) = self.city_conscript_unit(c) else { return 0 };

        let per_cost = self.d.defines.conscript_population_per_cost;
        if per_cost == 0 {
            return 0;
        }

        (self.d.units[conscript_unit].production_cost / per_cost).max(1)
    }

    /// C++ `CvCity::conscriptMinCityPopulation` (CvCity.cpp:3653-3662).
    pub fn city_conscript_min_city_population(&self, c: CityId) -> i32 {
        let mut population = self.d.defines.conscript_min_city_population;

        population += self.city_conscript_population(c);

        population
    }

    /// C++ `CvCity::flatConscriptAngerLength` (CvCity.cpp:3665-3675).
    pub fn city_flat_conscript_anger_length(&self, _c: CityId) -> i32 {
        let mut anger = self.d.defines.conscript_anger_divisor;

        anger *= self.d.game_speeds[self.game_speed_type()].hurry_conscript_anger_percent;
        anger /= 100;

        anger.max(1)
    }

    /// C++ `CvCity::canConscript` (CvCity.cpp:3678-3723).
    pub fn city_can_conscript(&self, c: CityId) -> bool {
        if self.city_is_disorder(c) {
            return false;
        }

        if self.cities[c].drafted {
            return false;
        }

        let owner = self.cities[c].owner;
        if self.players[owner].conscript_count >= self.players[owner].max_conscript {
            return false;
        }

        if self.cities[c].population <= self.city_conscript_population(c) {
            return false;
        }

        if self.cities[c].population < self.city_conscript_min_city_population(c) {
            return false;
        }

        if self.plot_calculate_team_culture_percent(self.city_plot(c), self.city_team(c)) < self.d.defines.conscript_min_culture_percent {
            return false;
        }

        if self.city_conscript_unit(c).is_none() {
            return false;
        }

        // FfH: Added by Kael 11/08/2007
        if self.cities[c].settlement {
            return false;
        }

        true
    }

    /// C++ `CvCity::initConscriptedUnit` (CvCity.cpp:3725-3767). The UnitAI is picked with
    /// `CvPlayerAI::AI_unitValue` (AI: a controller/AI-crate decision in this port).
    pub fn city_init_conscripted_unit(&mut self, c: CityId) -> Option<UnitId> {
        let conscript_unit = self.city_conscript_unit(c)?;

        // AI: C++ picks the first of UNITAI_ATTACK, UNITAI_CITY_DEFENSE, UNITAI_CITY_COUNTER,
        // UNITAI_CITY_SPECIAL with `AI_unitValue(eConscriptUnit, eUnitAI, area()) > 0`, else
        // NO_UNITAI. AI valuations are not ported: NO_UNITAI, i.e. the unit's default UnitAI.
        let unit_ai = None;

        let (owner, x, y) = {
            let city = &self.cities[c];
            (city.owner, city.x, city.y)
        };
        let unit = self.player_init_unit(owner, conscript_unit, x, y, unit_ai, None);

        self.city_add_production_experience(c, unit, true);

        // setMoves(0)
        self.units[unit].moves = 0;

        Some(unit)
    }

    /// C++ `CvCity::conscript` (CvCity.cpp:3770-3794).
    pub fn city_conscript(&mut self, c: CityId) {
        if !self.city_can_conscript(c) {
            return;
        }

        let conscript_population = self.city_conscript_population(c);
        self.city_change_population(c, -conscript_population);
        let anger_length = self.city_flat_conscript_anger_length(c);
        self.city_change_conscript_anger_timer(c, anger_length);

        self.cities[c].drafted = true;

        let owner = self.cities[c].owner;
        self.players[owner].conscript_count += 1;

        // UI: the active player selects the new unit.
        let _unit = self.city_init_conscripted_unit(c);
    }

    /// C++ `CvCity::processProcess` (CvCity.cpp:4209-4217).
    pub fn city_process_process(&mut self, c: CityId, process: ProcessType, change: i32) {
        let modifiers = self.d.processes[process].production_to_commerce_modifier;
        for (i, &modifier) in modifiers.iter().enumerate() {
            self.city_change_production_to_commerce_modifier(c, CommerceType::from_usize(i), modifier * change);
        }
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
        self.city_hurry_cost_modifier_raw(c, self.d.units[unit].hurry_cost_modifier, self.cities[c].unit_production[unit], ignore_new)
    }

    /// C++ `CvCity::getHurryCostModifier(BuildingTypes, bool)` (CvCity.cpp:5011-5014).
    pub fn city_hurry_cost_modifier_building(&self, c: CityId, building: BuildingType, ignore_new: bool) -> i32 {
        self.city_hurry_cost_modifier_raw(c, self.d.buildings[building].hurry_cost_modifier, self.cities[c].building_production[building], ignore_new)
    }

    /// C++ `CvCity::getHurryCostModifier(int, int, bool)` (CvCity.cpp:5016-5032).
    pub fn city_hurry_cost_modifier_raw(&self, c: CityId, base_modifier: i32, production: i32, ignore_new: bool) -> i32 {
        let mut modifier = 100;
        modifier *= (base_modifier + 100).max(0);
        modifier /= 100;

        if production == 0 && !ignore_new {
            modifier *= (self.d.defines.new_hurry_modifier + 100).max(0);
            modifier /= 100;
        }

        modifier *= (self.players[self.cities[c].owner].global_hurry_modifier + 100).max(0);
        modifier /= 100;

        modifier
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
    pub fn city_hurry_gold_for_cost(&self, _c: CityId, hurry: HurryType, hurry_cost: i32) -> i32 {
        let gold_per_production = self.d.hurries[hurry].gold_per_production;
        if gold_per_production == 0 {
            return 0;
        }

        let gold = hurry_cost * gold_per_production;

        gold.max(1)
    }

    /// C++ `CvCity::hurryPopulation` (CvCity.cpp:5093-5096).
    pub fn city_hurry_population(&self, c: CityId, hurry: HurryType) -> i32 {
        self.city_hurry_population_for_cost(c, hurry, self.city_hurry_cost(c, true))
    }

    /// C++ `CvCity::getHurryPopulation(HurryTypes, int iHurryCost)` (CvCity.cpp:5098-5108).
    pub fn city_hurry_population_for_cost(&self, _c: CityId, hurry: HurryType, hurry_cost: i32) -> i32 {
        if self.d.hurries[hurry].production_per_population == 0 {
            return 0;
        }

        let population = (hurry_cost - 1) / self.production_per_population(Some(hurry));

        (population + 1).max(1)
    }

    /// C++ `CvCity::hurryProduction` (CvCity.cpp:5110-5125).
    pub fn city_hurry_production(&self, c: CityId, hurry: HurryType) -> i32 {
        if self.d.hurries[hurry].production_per_population > 0 {
            let extra = self.city_hurry_population(c, hurry) * self.production_per_population(Some(hurry));
            (100 * self.city_extra_production_difference(c, extra)) / self.city_hurry_cost_modifier(c, false).max(1)
        } else {
            self.city_production_left(c)
        }
    }

    /// C++ `CvCity::flatHurryAngerLength` (CvCity.cpp:5128-5139).
    pub fn city_flat_hurry_anger_length(&self, c: CityId) -> i32 {
        let mut anger = self.d.defines.hurry_anger_divisor;
        anger *= self.d.game_speeds[self.game_speed_type()].hurry_conscript_anger_percent;
        anger /= 100;
        anger *= (100 + self.cities[c].hurry_anger_modifier).max(0);
        anger /= 100;

        anger.max(1)
    }

    /// C++ `CvCity::hurryAngerLength` (CvCity.cpp:5142-5152).
    pub fn city_hurry_anger_length(&self, c: CityId, hurry: HurryType) -> i32 {
        if self.d.hurries[hurry].anger {
            self.city_flat_hurry_anger_length(c)
        } else {
            0
        }
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
        if self.cities[c].production_automated != new_value {
            self.cities[c].production_automated = new_value;

            // C++: only for the active player's selected city (interface): selection buttons
            // dirty bit, and with `bNewValue && bClear` (all three modifier keys) the queue is
            // cleared so that the AI chooses again. Headless there is no selected city.
            let _ = clear;

            if !self.city_is_production(c) {
                ai_choose_production(self, c);
            }
        }
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

// ---------------------------------------------------------------------------------------------
// CvGameUtils.py callbacks that CvCity calls but whose FFH2 bodies are constant ("use the
// default" answers), so they have no hook in `crate::ffh`.
// ---------------------------------------------------------------------------------------------

/// Python `CvGameUtils.canTrain` (called only with `USE_CAN_TRAIN_CALLBACK`, off in FFH2):
/// FFH2 returns False.
fn py_can_train(_g: &Game, _c: CityId, _unit: UnitType, _cont: bool, _test_visible: bool, _ignore_cost: bool, _ignore_upgrades: bool) -> bool {
    false
}

/// Python `CvGameUtils.canConstruct` (called only with `USE_CAN_CONSTRUCT_CALLBACK`, off in
/// FFH2): FFH2 returns False.
fn py_can_construct(_g: &Game, _c: CityId, _building: BuildingType, _cont: bool, _test_visible: bool, _ignore_cost: bool) -> bool {
    false
}

/// Python `CvGameUtils.canCreate` (always called by `CvCity::canCreate`): FFH2 returns False.
fn py_can_create(_g: &Game, _c: CityId, _project: ProjectType, _cont: bool, _test_visible: bool) -> bool {
    false
}

/// Python `CvGameUtils.canMaintain` (always called by `CvCity::canMaintain`): FFH2 returns False.
fn py_can_maintain(_g: &Game, _c: CityId, _process: ProcessType, _cont: bool) -> bool {
    false
}

/// Python `CvGameUtils.cannotMaintain` (always called by `CvCity::canMaintain`): FFH2 returns
/// False.
fn py_cannot_maintain(_g: &Game, _c: CityId, _process: ProcessType, _cont: bool) -> bool {
    false
}

/// Python `CvGameUtils.getConscriptUnitType(iPlayer)` (always called by
/// `CvCity::getConscriptUnit`): FFH2 returns -1 ("uses default system").
fn py_get_conscript_unit_type(_g: &Game, _p: PlayerId) -> i32 {
    -1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::City;
    use crate::player::Player;
    use crate::setup::GameSetup;

    /// A two-civ game (players `Player::new`ed, no map) with one city of player 0.
    fn game_with_city() -> (Game, CityId) {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 5));
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        let id = g.cities.next_id();
        let c = g.player_add_city(PlayerId(0), City::new(d, num_players, num_teams, id, PlayerId(0), 3, 3, 0));
        (g, c)
    }

    #[test]
    fn hurry_cost_modifier_matches_cpp() {
        let (mut g, c) = game_with_city();
        let new_hurry = g.d.defines.new_hurry_modifier;
        // Base +50%, production already started: 100 * 150 / 100 = 150.
        assert_eq!(g.city_hurry_cost_modifier_raw(c, 50, 10, false), 150);
        // Nothing produced yet: the NEW_HURRY_MODIFIER applies unless ignored.
        assert_eq!(g.city_hurry_cost_modifier_raw(c, 50, 0, false), 150 * (new_hurry + 100).max(0) / 100);
        assert_eq!(g.city_hurry_cost_modifier_raw(c, 50, 0, true), 150);
        // The owner's hurry modifier, clamped at -100%.
        g.players[PlayerId(0)].global_hurry_modifier = -25;
        assert_eq!(g.city_hurry_cost_modifier_raw(c, 0, 5, false), 75);
        g.players[PlayerId(0)].global_hurry_modifier = -150;
        assert_eq!(g.city_hurry_cost_modifier_raw(c, 0, 5, false), 0);
        // Integer division at each step: 100 * 33 / 100 = 33, then 33 * 133 / 100 = 43.
        g.players[PlayerId(0)].global_hurry_modifier = 33;
        assert_eq!(g.city_hurry_cost_modifier_raw(c, -67, 5, false), 43);
    }

    #[test]
    fn hurry_gold_and_anger_lengths() {
        let (mut g, c) = game_with_city();
        let d = g.d;
        let gold_hurry = d.hurries.keys().find(|&h| d.hurries[h].gold_per_production > 0).expect("a gold hurry");
        let per = d.hurries[gold_hurry].gold_per_production;
        assert_eq!(g.city_hurry_gold_for_cost(c, gold_hurry, 7), (7 * per).max(1));
        // At least one gold even for a free hurry.
        assert_eq!(g.city_hurry_gold_for_cost(c, gold_hurry, 0), 1);
        if let Some(pop_hurry) = d.hurries.keys().find(|&h| d.hurries[h].gold_per_production == 0) {
            assert_eq!(g.city_hurry_gold_for_cost(c, pop_hurry, 50), 0);
        }

        let percent = d.game_speeds[g.game_speed_type()].hurry_conscript_anger_percent;
        let base = d.defines.hurry_anger_divisor * percent / 100;
        assert_eq!(g.city_flat_hurry_anger_length(c), base.max(1));
        g.cities[c].hurry_anger_modifier = 50;
        assert_eq!(g.city_flat_hurry_anger_length(c), (base * 150 / 100).max(1));
        g.cities[c].hurry_anger_modifier = -200;
        assert_eq!(g.city_flat_hurry_anger_length(c), 1);
        for h in d.hurries.keys() {
            let expect = if d.hurries[h].anger { g.city_flat_hurry_anger_length(c) } else { 0 };
            assert_eq!(g.city_hurry_anger_length(c, h), expect);
        }

        let conscript = (d.defines.conscript_anger_divisor * percent / 100).max(1);
        assert_eq!(g.city_flat_conscript_anger_length(c), conscript);
    }

    #[test]
    fn production_queue_queries() {
        let (mut g, c) = game_with_city();
        assert!(!g.city_is_production(c));
        assert!(!g.city_is_production_limited(c));
        assert_eq!(g.city_production(c), 0);
        assert_eq!(g.city_production_needed(c), MAX_INT);
        assert_eq!(g.city_hurry_cost_modifier(c, false), 100);

        let process = ProcessType(0);
        g.cities[c].order_queue.push(OrderData { order_type: OrderType::ORDER_MAINTAIN, data1: process.i(), data2: -1, save: false });
        assert!(g.city_is_production(c) && g.city_is_production_process(c));
        assert!(!g.city_is_production_unit(c) && !g.city_is_production_building(c) && !g.city_is_production_project(c));
        assert!(!g.city_is_production_limited(c));
        assert!(!g.city_is_food_production(c));
        assert_eq!(g.city_production_process(c), Some(process));
        assert_eq!(g.city_production_unit(c), None);
        assert_eq!(g.city_production_needed(c), MAX_INT);
        assert_eq!(g.city_production_turns_left(c), MAX_INT);
        assert_eq!(g.city_general_production_turns_left(c), 0);
        assert_eq!(g.city_hurry_cost_modifier(c, false), 100);
        // Production changes on a process are ignored.
        g.city_change_production(c, 5);
        assert_eq!(g.city_production(c), 0);
    }

    #[test]
    fn free_promotions_and_unhappy_production() {
        let (mut g, c) = game_with_city();
        let promotion = PromotionType(0);
        assert!(!g.city_is_free_promotion(c, promotion));
        g.cities[c].free_promotion_count[promotion] = 2;
        assert!(g.city_is_free_promotion(c, promotion));
        assert!(!g.city_is_unhappy_production(c));
        g.cities[c].unhappy_production = -1;
        assert!(g.city_is_unhappy_production(c));
        assert_eq!(g.city_max_hurry_population(c), 0);
        g.cities[c].population = 7;
        assert_eq!(g.city_max_hurry_population(c), 3);
    }
}
