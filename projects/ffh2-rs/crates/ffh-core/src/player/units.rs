//! CvPlayer: units, buildings and projects -- unit and selection-group lifecycle, what can
//! be trained/constructed/created and at what production cost, unit/building class counts,
//! building effects (`processBuilding`), worker builds and routes, hurry, free promotions,
//! golden-age and great-people units, combat experience, group cycling.

#![allow(unused_imports)]
#![allow(unused_variables)]

use crate::game::Game;
use crate::group::SelectionGroup;
use crate::ids::*;
use crate::structs::*;
use crate::unit::Unit;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------ unit / group lists

    /// C++ `CvPlayer::addUnit`: stores a new unit object and appends it to the owner list.
    /// (C++ returns an uninitialized object that the caller then `init`s.)
    pub fn player_add_unit(&mut self, p: PlayerId, unit: Unit) -> UnitId {
        let id = self.units.insert(unit);
        self.players[p].units.push(id);
        id
    }

    /// C++ `CvPlayer::deleteUnit(iID)`.
    pub fn player_delete_unit(&mut self, p: PlayerId, u: UnitId) {
        self.units.remove(u);
        self.players[p].units.retain(|&x| x != u);
    }

    /// C++ `CvPlayer::addSelectionGroup`.
    pub fn player_add_selection_group(&mut self, p: PlayerId, group: SelectionGroup) -> GroupId {
        let id = self.groups.insert(group);
        self.players[p].groups.push(id);
        id
    }

    /// C++ `CvPlayer::deleteSelectionGroup(iID)`.
    pub fn player_delete_selection_group(&mut self, p: PlayerId, g: GroupId) {
        let removed = self.groups.remove(g).is_some();
        debug_assert!(removed, "could not find group, delete failed");
        self.players[p].groups.retain(|&x| x != g);
    }

    // ------------------------------------------------------------------ unit lifecycle

    /// C++ `CvPlayer::initUnit(eUnit, iX, iY, eUnitAI = NO_UNITAI, eFacingDirection = NO_DIRECTION)`:
    /// creates and inits a unit (default unit AI from the unit info when `unit_ai` is None).
    pub fn player_init_unit(&mut self, p: PlayerId, unit: UnitType, x: i32, y: i32, unit_ai: Option<UnitAIType>, facing: Option<DirectionType>) -> UnitId {
        todo!("CvPlayer::initUnit")
    }

    /// C++ `CvPlayer::disbandUnit(bAnnounce)`: disbands the least valuable military-support
    /// unit (gold strike).
    pub fn player_disband_unit(&mut self, p: PlayerId, announce: bool) {
        todo!("CvPlayer::disbandUnit")
    }

    /// C++ `CvPlayer::killUnits` (FFH: strips equipment and immortality; units that cannot
    /// be scrapped betray to the barbarians).
    pub fn player_kill_units(&mut self, p: PlayerId) {
        todo!("CvPlayer::killUnits")
    }

    /// C++ `CvPlayer::verifyUnitStacksValid`.
    pub fn player_verify_unit_stacks_valid(&mut self, p: PlayerId) {
        todo!("CvPlayer::verifyUnitStacksValid")
    }

    /// C++ `CvPlayer::updatePlunder(iChange, bUpdatePlotGroups)`: blockading units.
    pub fn player_update_plunder(&mut self, p: PlayerId, change: i32, update_plot_groups: bool) {
        todo!("CvPlayer::updatePlunder")
    }

    /// C++ `CvPlayer::upgradeAllPrice(eUpgradeUnit, eFromUnit)`.
    pub fn player_upgrade_all_price(&self, p: PlayerId, upgrade_unit: UnitType, from_unit: UnitType) -> i32 {
        todo!("CvPlayer::upgradeAllPrice")
    }

    /// C++ `CvPlayer::countReligionSpreadUnits(pArea, eReligion)`.
    pub fn player_count_religion_spread_units(&self, p: PlayerId, area: AreaId, religion: ReligionType) -> i32 {
        todo!("CvPlayer::countReligionSpreadUnits")
    }

    /// C++ `CvPlayer::countCorporationSpreadUnits(pArea, eCorporation)`.
    pub fn player_count_corporation_spread_units(&self, p: PlayerId, area: AreaId, corporation: CorporationType) -> i32 {
        todo!("CvPlayer::countCorporationSpreadUnits")
    }

    /// C++ `CvPlayer::countGroupFlagUnits(iGroupflag)` (BETTER AI: units with that AI group flag).
    pub fn player_count_group_flag_units(&self, p: PlayerId, group_flag: i32) -> i32 {
        todo!("CvPlayer::countGroupFlagUnits")
    }

    /// C++ `CvPlayer::getConquestUnitClassCount(eUnitClass, pArea)` (BETTER AI).
    pub fn player_get_conquest_unit_class_count(&self, p: PlayerId, unit_class: UnitClassType, area: AreaId) -> i32 {
        todo!("CvPlayer::getConquestUnitClassCount")
    }

    // ------------------------------------------------------------------ group cycle

    /// C++ `CvPlayer::updateGroupCycle(pUnit)`: re-inserts the unit's group into the cycle
    /// next to the groups on its plot (or the closest group).
    pub fn player_update_group_cycle(&mut self, p: PlayerId, unit: UnitId) {
        todo!("CvPlayer::updateGroupCycle")
    }

    /// C++ `CvPlayer::removeGroupCycle(iID)`.
    pub fn player_remove_group_cycle(&mut self, p: PlayerId, g: GroupId) {
        let cycle = &mut self.players[p].group_cycle;
        if let Some(i) = cycle.iter().position(|&x| x == g) {
            cycle.remove(i);
        }
    }

    // ------------------------------------------------------------------ training units

    /// C++ `CvPlayer::canTrain(eUnit, bContinue = false, bTestVisible = false, bIgnoreCost = false)`.
    pub fn player_can_train(&self, p: PlayerId, unit: UnitType, continue_: bool, test_visible: bool, ignore_cost: bool) -> bool {
        todo!("CvPlayer::canTrain")
    }

    /// C++ `CvPlayer::isProductionMaxedUnitClass(eUnitClass)` (game/team/player limits).
    pub fn player_is_production_maxed_unit_class(&self, p: PlayerId, unit_class: UnitClassType) -> bool {
        todo!("CvPlayer::isProductionMaxedUnitClass")
    }

    /// C++ `CvPlayer::getProductionNeeded(UnitTypes)`.
    pub fn player_get_production_needed_unit(&self, p: PlayerId, unit: UnitType) -> i32 {
        todo!("CvPlayer::getProductionNeeded")
    }

    /// C++ `CvPlayer::getProductionModifier(UnitTypes)`.
    pub fn player_get_production_modifier_unit(&self, p: PlayerId, unit: UnitType) -> i32 {
        todo!("CvPlayer::getProductionModifier")
    }

    /// C++ `CvPlayer::isUnitClassMaxedOut(eIndex, iExtra = 0)` (national unit limit).
    pub fn player_is_unit_class_maxed_out(&self, p: PlayerId, unit_class: UnitClassType, extra: i32) -> bool {
        todo!("CvPlayer::isUnitClassMaxedOut")
    }

    /// C++ `CvPlayer::getUnitClassCountPlusMaking`.
    pub fn player_get_unit_class_count_plus_making(&self, p: PlayerId, unit_class: UnitClassType) -> i32 {
        let pl = &self.players[p];
        pl.unit_class_count[unit_class] + pl.unit_class_making[unit_class]
    }

    /// C++ `CvPlayer::getUnitExtraCost(eUnitClass)`.
    pub fn player_get_unit_extra_cost(&self, p: PlayerId, unit_class: UnitClassType) -> i32 {
        for &(uc, cost) in &self.players[p].unit_extra_costs {
            if uc == unit_class {
                return cost;
            }
        }
        0
    }

    /// C++ `CvPlayer::setUnitExtraCost(eUnitClass, iCost)` (0 removes the entry).
    pub fn player_set_unit_extra_cost(&mut self, p: PlayerId, unit_class: UnitClassType, cost: i32) {
        let costs = &mut self.players[p].unit_extra_costs;
        if let Some(i) = costs.iter().position(|&(uc, _)| uc == unit_class) {
            if cost == 0 {
                costs.remove(i);
            } else {
                costs[i].1 = cost;
            }
            return;
        }
        if cost != 0 {
            costs.push((unit_class, cost));
        }
    }

    // ------------------------------------------------------------------ free promotions

    /// C++ `CvPlayer::isFreePromotion(UnitCombatTypes, PromotionTypes)`.
    pub fn player_is_free_promotion_unit_combat(&self, p: PlayerId, unit_combat: UnitCombatType, promotion: PromotionType) -> bool {
        self.players[p].free_unit_combat_promotions.iter().any(|&(uc, pr)| uc == unit_combat && pr == promotion)
    }

    /// C++ `CvPlayer::setFreePromotion(UnitCombatTypes, PromotionTypes, bFree)`.
    pub fn player_set_free_promotion_unit_combat(&mut self, p: PlayerId, unit_combat: UnitCombatType, promotion: PromotionType, free: bool) {
        let list = &mut self.players[p].free_unit_combat_promotions;
        if let Some(i) = list.iter().position(|&(uc, pr)| uc == unit_combat && pr == promotion) {
            if !free {
                list.remove(i);
            }
            return;
        }
        if free {
            list.push((unit_combat, promotion));
        }
    }

    /// C++ `CvPlayer::isFreePromotion(UnitClassTypes, PromotionTypes)`.
    pub fn player_is_free_promotion_unit_class(&self, p: PlayerId, unit_class: UnitClassType, promotion: PromotionType) -> bool {
        self.players[p].free_unit_class_promotions.iter().any(|&(uc, pr)| uc == unit_class && pr == promotion)
    }

    /// C++ `CvPlayer::setFreePromotion(UnitClassTypes, PromotionTypes, bFree)`.
    pub fn player_set_free_promotion_unit_class(&mut self, p: PlayerId, unit_class: UnitClassType, promotion: PromotionType, free: bool) {
        let list = &mut self.players[p].free_unit_class_promotions;
        if let Some(i) = list.iter().position(|&(uc, pr)| uc == unit_class && pr == promotion) {
            if !free {
                list.remove(i);
            }
            return;
        }
        if free {
            list.push((unit_class, promotion));
        }
    }

    // ------------------------------------------------------------------ golden age units / great people

    /// C++ `CvPlayer::unitsRequiredForGoldenAge`.
    pub fn player_units_required_for_golden_age(&self, p: PlayerId) -> i32 {
        self.d.defines.base_golden_age_units + self.players[p].num_unit_golden_ages * self.d.defines.golden_age_units_multiplier
    }

    /// C++ `CvPlayer::unitsGoldenAgeCapable`.
    pub fn player_units_golden_age_capable(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::unitsGoldenAgeCapable")
    }

    /// C++ `CvPlayer::unitsGoldenAgeReady` (distinct golden-age-capable unit types).
    pub fn player_units_golden_age_ready(&self, p: PlayerId) -> i32 {
        todo!("CvPlayer::unitsGoldenAgeReady")
    }

    /// C++ `CvPlayer::killGoldenAgeUnits(pUnitAlive)`: consumes the units used for a golden
    /// age (closest to `unit_alive`). C++ quirk: dereferences `pUnitAlive` in the distance
    /// computation even though it may be NULL.
    pub fn player_kill_golden_age_units(&mut self, p: PlayerId, unit_alive: Option<UnitId>) {
        todo!("CvPlayer::killGoldenAgeUnits")
    }

    /// C++ `CvPlayer::greatPeopleThreshold(bMilitary = false)`.
    pub fn player_great_people_threshold(&self, p: PlayerId, military: bool) -> i32 {
        todo!("CvPlayer::greatPeopleThreshold")
    }

    /// C++ `CvPlayer::getDomesticGreatGeneralRateModifier`.
    pub fn player_get_domestic_great_general_rate_modifier(&self, p: PlayerId) -> i32 {
        self.d.defines.combat_experience_in_borders_percent + self.players[p].domestic_great_general_rate_modifier
    }

    /// C++ `CvPlayer::createGreatPeople(eGreatPersonUnit, bIncrementThreshold, bIncrementExperience, iX, iY)`.
    pub fn player_create_great_people(&mut self, p: PlayerId, unit: UnitType, increment_threshold: bool, increment_experience: bool, x: i32, y: i32) {
        todo!("CvPlayer::createGreatPeople")
    }

    /// C++ `CvPlayer::setCombatExperience(iExperience)`: creates a great general (warlord) in
    /// the best city when the threshold is reached.
    pub fn player_set_combat_experience(&mut self, p: PlayerId, experience: i32) {
        todo!("CvPlayer::setCombatExperience")
    }

    /// C++ `CvPlayer::changeCombatExperience`.
    pub fn player_change_combat_experience(&mut self, p: PlayerId, change: i32) {
        let experience = self.players[p].combat_experience + change;
        self.player_set_combat_experience(p, experience);
    }

    // ------------------------------------------------------------------ buildings and projects

    /// C++ `CvPlayer::canConstruct(eBuilding, bContinue = false, bTestVisible = false, bIgnoreCost = false)`.
    pub fn player_can_construct(&self, p: PlayerId, building: BuildingType, continue_: bool, test_visible: bool, ignore_cost: bool) -> bool {
        todo!("CvPlayer::canConstruct")
    }

    /// C++ `CvPlayer::canCreate(eProject, bContinue = false, bTestVisible = false)`.
    pub fn player_can_create(&self, p: PlayerId, project: ProjectType, continue_: bool, test_visible: bool) -> bool {
        todo!("CvPlayer::canCreate")
    }

    /// C++ `CvPlayer::canMaintain(eProcess, bContinue = false)`.
    pub fn player_can_maintain(&self, p: PlayerId, process: ProcessType, continue_: bool) -> bool {
        todo!("CvPlayer::canMaintain")
    }

    /// C++ `CvPlayer::isProductionMaxedBuildingClass(eBuildingClass, bAcquireCity = false)`.
    pub fn player_is_production_maxed_building_class(&self, p: PlayerId, building_class: BuildingClassType, acquire_city: bool) -> bool {
        todo!("CvPlayer::isProductionMaxedBuildingClass")
    }

    /// C++ `CvPlayer::isProductionMaxedProject(eProject)`.
    pub fn player_is_production_maxed_project(&self, p: PlayerId, project: ProjectType) -> bool {
        todo!("CvPlayer::isProductionMaxedProject")
    }

    /// C++ `CvPlayer::getProductionNeeded(BuildingTypes)`.
    pub fn player_get_production_needed_building(&self, p: PlayerId, building: BuildingType) -> i32 {
        todo!("CvPlayer::getProductionNeeded")
    }

    /// C++ `CvPlayer::getProductionNeeded(ProjectTypes)`.
    pub fn player_get_production_needed_project(&self, p: PlayerId, project: ProjectType) -> i32 {
        todo!("CvPlayer::getProductionNeeded")
    }

    /// C++ `CvPlayer::getProductionModifier(BuildingTypes)`.
    pub fn player_get_production_modifier_building(&self, p: PlayerId, building: BuildingType) -> i32 {
        todo!("CvPlayer::getProductionModifier")
    }

    /// C++ `CvPlayer::getProductionModifier(ProjectTypes)`.
    pub fn player_get_production_modifier_project(&self, p: PlayerId, project: ProjectType) -> i32 {
        todo!("CvPlayer::getProductionModifier")
    }

    /// C++ `CvPlayer::getBuildingClassPrereqBuilding(eBuilding, ePrereqBuildingClass, iExtra = 0)`.
    pub fn player_get_building_class_prereq_building(&self, p: PlayerId, building: BuildingType, prereq_building_class: BuildingClassType, extra: i32) -> i32 {
        todo!("CvPlayer::getBuildingClassPrereqBuilding")
    }

    /// C++ `CvPlayer::removeBuildingClass(eBuildingClass)` (FFH: every building of the class
    /// in every city).
    pub fn player_remove_building_class(&mut self, p: PlayerId, building_class: BuildingClassType) {
        todo!("CvPlayer::removeBuildingClass")
    }

    /// C++ `CvPlayer::processBuilding(eBuilding, iChange, pArea)`: player- and area-wide
    /// building effects. C++ quirks: `bChange` is uninitialized when `iChange == 0`, and the
    /// FFH resist modifiers are added without being multiplied by `iChange`.
    pub fn player_process_building(&mut self, p: PlayerId, building: BuildingType, change: i32, area: AreaId) {
        todo!("CvPlayer::processBuilding")
    }

    /// C++ `CvPlayer::isBuildingClassMaxedOut(eIndex, iExtra = 0)` (national wonder limit).
    pub fn player_is_building_class_maxed_out(&self, p: PlayerId, building_class: BuildingClassType, extra: i32) -> bool {
        todo!("CvPlayer::isBuildingClassMaxedOut")
    }

    /// C++ `CvPlayer::getBuildingClassCountPlusMaking`.
    pub fn player_get_building_class_count_plus_making(&self, p: PlayerId, building_class: BuildingClassType) -> i32 {
        let pl = &self.players[p];
        pl.building_class_count[building_class] + pl.building_class_making[building_class]
    }

    /// C++ `CvPlayer::isBuildingFree`.
    pub fn player_is_building_free(&self, p: PlayerId, building: BuildingType) -> bool {
        self.players[p].free_building_count[building] > 0
    }

    /// C++ `CvPlayer::changeFreeBuildingCount`: grants/removes the free building in every
    /// city when the count crosses zero. C++ quirk: the Sephi bugfix still calls
    /// `setNumFreeBuilding(eIndex, 1)` unconditionally after the civ check.
    pub fn player_change_free_building_count(&mut self, p: PlayerId, building: BuildingType, change: i32) {
        todo!("CvPlayer::changeFreeBuildingCount")
    }

    /// C++ `CvPlayer::changeExtraBuildingHappiness`.
    pub fn player_change_extra_building_happiness(&mut self, p: PlayerId, building: BuildingType, change: i32) {
        if change != 0 {
            self.players[p].extra_building_happiness[building] += change;
            self.player_update_extra_building_happiness(p);
        }
    }

    /// C++ `CvPlayer::changeExtraBuildingHealth`.
    pub fn player_change_extra_building_health(&mut self, p: PlayerId, building: BuildingType, change: i32) {
        if change != 0 {
            self.players[p].extra_building_health[building] += change;
            self.player_update_extra_building_health(p);
        }
    }

    /// C++ `CvPlayer::isSpecialBuildingNotRequired`.
    pub fn player_is_special_building_not_required(&self, p: PlayerId, special_building: SpecialBuildingType) -> bool {
        self.players[p].special_building_not_required_count[special_building] > 0
    }

    // ------------------------------------------------------------------ hurry

    /// C++ `CvPlayer::canHurry(eIndex)`.
    pub fn player_can_hurry(&self, p: PlayerId, hurry: HurryType) -> bool {
        self.players[p].hurry_count[hurry] > 0
    }

    /// C++ `CvPlayer::canPopRush`.
    pub fn player_can_pop_rush(&self, p: PlayerId) -> bool {
        self.players[p].pop_rush_hurry_count > 0
    }

    /// C++ `CvPlayer::changeHurryCount`: also tracks hurries that cost population.
    pub fn player_change_hurry_count(&mut self, p: PlayerId, hurry: HurryType, change: i32) {
        todo!("CvPlayer::changeHurryCount")
    }

    // ------------------------------------------------------------------ worker builds and routes

    /// C++ `CvPlayer::canBuild(pPlot, eBuild, bTestEra = false, bTestVisible = false)`.
    pub fn player_can_build(&self, p: PlayerId, plot: PlotIdx, build: BuildType, test_era: bool, test_visible: bool) -> bool {
        todo!("CvPlayer::canBuild")
    }

    /// C++ `CvPlayer::getBuildCost(pPlot, eBuild)` (gold cost, inflated; 0 once started).
    pub fn player_get_build_cost(&self, p: PlayerId, plot: PlotIdx, build: BuildType) -> i32 {
        todo!("CvPlayer::getBuildCost")
    }

    /// C++ `CvPlayer::getBestRoute(pPlot = NULL)`.
    pub fn player_get_best_route(&self, p: PlayerId, plot: Option<PlotIdx>) -> Option<RouteType> {
        todo!("CvPlayer::getBestRoute")
    }

    /// C++ `CvPlayer::getImprovementUpgradeRate`.
    pub fn player_get_improvement_upgrade_rate(&self, p: PlayerId) -> i32 {
        let mut rate = 1; // XXX (C++)
        rate *= (self.players[p].improvement_upgrade_rate_modifier + 100).max(0);
        rate /= 100;
        rate
    }
}
