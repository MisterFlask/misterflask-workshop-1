//! CvPlayer: units, buildings and projects -- unit and selection-group lifecycle, what can
//! be trained/constructed/created and at what production cost, unit/building class counts,
//! building effects (`processBuilding`), worker builds and routes, hurry, free promotions,
//! golden-age and great-people units, combat experience, group cycling.
//!
//! `docs/porting/player.md` lists the C++ functions of this file with their CvPlayer.cpp line
//! ranges; all of them are ported. UI-only parts (messages, replay messages, animations) are
//! omitted or go to the game log.

use std::cmp::max;

use crate::defs::*;
use crate::game::Game;
use crate::group::SelectionGroup;
use crate::ids::*;
use crate::unit::Unit;
use crate::utils::{
    is_limited_wonder_class, is_national_unit_class, is_national_wonder_class, is_team_wonder_class, is_world_project,
    is_world_unit_class, is_world_wonder_class,
};
use ffh_data::*;

/// C++ `GROUPFLAG_CONQUEST` (CvDefines.h, Sephi's BETTER AI group flags).
const GROUPFLAG_CONQUEST: i32 = 10;

/// C++ `(UnitClassTypes)GC.getUnitInfo(eUnit).getUnitClassType()`, used unchecked as an array
/// index by the C++ (every unit info names a class).
fn unit_class_of(d: &GameData, unit: UnitType) -> UnitClassType {
    d.units[unit].unit_class_type.expect("CvUnitInfo without a unit class")
}

/// C++ `(BuildingClassTypes)GC.getBuildingInfo(eBuilding).getBuildingClassType()`, used
/// unchecked as an array index by the C++ (every building info names a class).
fn building_class_of(d: &GameData, building: BuildingType) -> BuildingClassType {
    d.buildings[building].building_class_type.expect("CvBuildingInfo without a building class")
}

/// C++ `GC.getCivilizationInfo(getCivilizationType())`: the C++ indexes the info array
/// unchecked; players that own units and cities always have a civilization.
fn civ_of(g: &Game, p: PlayerId) -> CivilizationType {
    g.player_get_civilization_type(p).expect("CvPlayer: player without a civilization")
}

/// The `switch (pLoopUnit->AI_getUnitAIType())` of C++ `CvPlayer::disbandUnit`: the factor the
/// disband value is multiplied by (1 for the cases that only `break`).
fn disband_unit_ai_multiplier(unit_ai: Option<UnitAIType>) -> i32 {
    match unit_ai {
        Some(UnitAIType::UNITAI_UNKNOWN) | Some(UnitAIType::UNITAI_ANIMAL) => 1,

        Some(UnitAIType::UNITAI_SETTLE)
        // BETTER AI (New UNITAI) Sephi
        | Some(UnitAIType::UNITAI_WARWIZARD)
        | Some(UnitAIType::UNITAI_TERRAFORMER)
        | Some(UnitAIType::UNITAI_MANA_UPGRADE)
        | Some(UnitAIType::UNITAI_MAGE) => 20,

        Some(UnitAIType::UNITAI_WORKER) => 10,

        Some(UnitAIType::UNITAI_ATTACK)
        | Some(UnitAIType::UNITAI_ATTACK_CITY)
        | Some(UnitAIType::UNITAI_COLLATERAL)
        | Some(UnitAIType::UNITAI_PILLAGE)
        | Some(UnitAIType::UNITAI_RESERVE)
        | Some(UnitAIType::UNITAI_COUNTER) => 2,

        Some(UnitAIType::UNITAI_CITY_DEFENSE)
        | Some(UnitAIType::UNITAI_CITY_COUNTER)
        | Some(UnitAIType::UNITAI_CITY_SPECIAL)
        | Some(UnitAIType::UNITAI_PARADROP) => 6,

        Some(UnitAIType::UNITAI_EXPLORE) => 15,

        Some(UnitAIType::UNITAI_MISSIONARY) => 8,

        Some(UnitAIType::UNITAI_PROPHET)
        | Some(UnitAIType::UNITAI_ARTIST)
        | Some(UnitAIType::UNITAI_SCIENTIST)
        | Some(UnitAIType::UNITAI_GENERAL)
        | Some(UnitAIType::UNITAI_MERCHANT)
        | Some(UnitAIType::UNITAI_ENGINEER) => 1,

        Some(UnitAIType::UNITAI_SPY) => 12,

        Some(UnitAIType::UNITAI_ICBM) => 4,

        Some(UnitAIType::UNITAI_WORKER_SEA) => 18,

        Some(UnitAIType::UNITAI_ATTACK_SEA) | Some(UnitAIType::UNITAI_RESERVE_SEA) | Some(UnitAIType::UNITAI_ESCORT_SEA) => 1,

        Some(UnitAIType::UNITAI_EXPLORE_SEA) => 25,

        Some(UnitAIType::UNITAI_ASSAULT_SEA)
        | Some(UnitAIType::UNITAI_SETTLER_SEA)
        | Some(UnitAIType::UNITAI_MISSIONARY_SEA)
        | Some(UnitAIType::UNITAI_SPY_SEA)
        | Some(UnitAIType::UNITAI_CARRIER_SEA)
        | Some(UnitAIType::UNITAI_MISSILE_CARRIER_SEA) => 5,

        Some(UnitAIType::UNITAI_PIRATE_SEA) | Some(UnitAIType::UNITAI_ATTACK_AIR) => 1,

        Some(UnitAIType::UNITAI_DEFENSE_AIR) | Some(UnitAIType::UNITAI_CARRIER_AIR) | Some(UnitAIType::UNITAI_MISSILE_AIR) => 3,

        // default: `FAssert(false)` commented out by Sephi (the new UnitAIs were not added).
        _ => 1,
    }
}

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
        // addUnit(): a fresh object in the owner's list. C++ FFreeListTrashArray::add gives the
        // new object its id; `unit_init` then resets the slot (`CvUnit::reset`).
        let mut new_unit = Unit::default();
        new_unit.id = self.units.next_id();
        let u = self.player_add_unit(p, new_unit);

        // C++ casts `getDefaultUnitAIType()` unchecked; every unit info has a <DefaultUnitAI>.
        let unit_ai = match unit_ai {
            Some(unit_ai) => unit_ai,
            None => self.d.units[unit].default_unit_ai_type.expect("CvUnitInfo without a default unit AI"),
        };

        // `eFacingDirection` only orients the unit's graphics entity.
        let _ = facing;
        self.unit_init(u, unit, unit_ai, p, x, y);

        u
    }

    /// C++ `CvPlayer::disbandUnit(bAnnounce)`: disbands the least valuable military-support
    /// unit (gold strike).
    pub fn player_disband_unit(&mut self, p: PlayerId, announce: bool) {
        let d = self.d;

        let mut best_value = MAX_INT;
        let mut best_unit: Option<UnitId> = None;

        for loop_unit in self.players[p].units.clone() {
            let unit_type = self.units[loop_unit].unit_type;
            let info = &d.units[unit_type];

            if !self.unit_has_cargo(loop_unit)
                // FfH: Added by Kael 11/24/2007
                && info.military_support
            {
                if !self.unit_is_golden_age(loop_unit) {
                    if info.production_cost > 0 {
                        let plot = self.unit_plot(loop_unit);
                        if !self.unit_is_military_happiness(loop_unit)
                            || !self.plot_is_city(plot, false, None)
                            || self.plot_plot_count(plot, Some(Game::puf_is_military_happiness), -1, -1, Some(p), None, None, -1, -1) > 1
                        {
                            let mut value = 10000 + self.soren_rand_num(1000, "Disband Unit");

                            value += info.production_cost * 5;

                            value += self.units[loop_unit].experience * 20;
                            value += self.units[loop_unit].level * 100;

                            if self.unit_can_defend(loop_unit, None) && self.plot_is_city(plot, false, None) {
                                value *= 2;
                            }

                            if self.plot_get_team(plot) == Some(self.unit_get_team(loop_unit)) {
                                value *= 3;
                            }

                            value *= disband_unit_ai_multiplier(self.units[loop_unit].ai_type);

                            if info.extra_cost > 0 {
                                value /= info.extra_cost + 1;
                            }

                            if value < best_value {
                                best_value = value;
                                best_unit = Some(loop_unit);
                            }
                        }
                    }
                }
            }
        }

        if let Some(best_unit) = best_unit {
            // UI: TXT_KEY_MISC_UNIT_DISBANDED_NO_MONEY (C++ shows it whatever `bAnnounce`).
            let _ = announce;
            if self.log.enabled {
                let turn = self.game_turn();
                let name = self.unit_get_name(best_unit);
                self.log.msg(turn, Some(p), || format!("{name} disbanded: no money to pay for it"));
            }

            // FAssert(!(pBestUnit->isGoldenAge()))

            self.unit_kill(best_unit, false, None);
        }
    }

    /// C++ `CvPlayer::killUnits` (FFH: strips equipment and immortality; units that cannot
    /// be scrapped betray to the barbarians).
    pub fn player_kill_units(&mut self, p: PlayerId) {
        let d = self.d;

        // C++ walks the live unit list; units killed meanwhile (cargo of a killed transport)
        // are skipped.
        for loop_unit in self.players[p].units.clone() {
            if !self.units.contains(loop_unit) {
                continue;
            }

            // FfH: Modified by Kael 06/13/2008 (was `pLoopUnit->kill(false)`)
            for promotion in d.promotions.keys() {
                if self.units[loop_unit].has_promotion[promotion] {
                    if d.promotions[promotion].equipment {
                        self.unit_set_has_promotion(loop_unit, promotion, false);
                    }
                }
            }
            if self.unit_is_immortal(loop_unit) {
                self.unit_change_immortal(loop_unit, -1);
            }
            if self.unit_can_scrap(loop_unit) {
                self.unit_kill(loop_unit, false, None);
            } else {
                let barbarian = self.barbarian_player();
                self.unit_betray(loop_unit, barbarian);
            }
            // FfH: End Modify
        }
    }

    /// C++ `CvPlayer::verifyUnitStacksValid`.
    pub fn player_verify_unit_stacks_valid(&mut self, p: PlayerId) {
        for loop_unit in self.players[p].units.clone() {
            if !self.units.contains(loop_unit) {
                continue;
            }
            self.unit_verify_stack_valid(loop_unit);
        }
    }

    /// C++ `CvPlayer::updatePlunder(iChange, bUpdatePlotGroups)`: blockading units.
    pub fn player_update_plunder(&mut self, p: PlayerId, change: i32, update_plot_groups: bool) {
        for loop_unit in self.players[p].units.clone() {
            if !self.units.contains(loop_unit) {
                continue;
            }
            if self.units[loop_unit].blockading {
                self.unit_update_plunder(loop_unit, change, update_plot_groups);
            }
        }
    }

    /// C++ `CvPlayer::upgradeAllPrice(eUpgradeUnit, eFromUnit)`.
    pub fn player_upgrade_all_price(&self, p: PlayerId, upgrade_unit: UnitType, from_unit: UnitType) -> i32 {
        let mut price = 0;

        // Loop through units and determine the total power of this player's military (sic)
        for &loop_unit in &self.players[p].units {
            if self.units[loop_unit].unit_type == from_unit {
                if self.unit_can_upgrade(loop_unit, Some(upgrade_unit), true) {
                    price += self.unit_upgrade_price(loop_unit, upgrade_unit);
                }
            }
        }

        price
    }

    /// C++ `CvPlayer::countReligionSpreadUnits(pArea, eReligion)`.
    pub fn player_count_religion_spread_units(&self, p: PlayerId, area: AreaId, religion: ReligionType) -> i32 {
        let d = self.d;
        let mut count = 0;

        for &loop_unit in &self.players[p].units {
            if self.unit_area(loop_unit) == area {
                if d.units[self.units[loop_unit].unit_type].religion_spreads[religion] > 0 {
                    count += 1;
                }
            }
        }

        count
    }

    /// C++ `CvPlayer::countCorporationSpreadUnits(pArea, eCorporation)`.
    pub fn player_count_corporation_spread_units(&self, p: PlayerId, area: AreaId, corporation: CorporationType) -> i32 {
        let d = self.d;
        let mut count = 0;

        for &loop_unit in &self.players[p].units {
            if self.unit_area(loop_unit) == area {
                if d.units[self.units[loop_unit].unit_type].corporation_spreads[corporation] > 0 {
                    count += 1;
                }
            }
        }

        count
    }

    /// C++ `CvPlayer::countGroupFlagUnits(iGroupflag)` (BETTER AI: units with that AI group flag).
    pub fn player_count_group_flag_units(&self, p: PlayerId, group_flag: i32) -> i32 {
        let mut count = 0;

        for &loop_unit in &self.players[p].units {
            if self.units[loop_unit].ai_groupflag == group_flag {
                count += 1;
            }
        }

        count
    }

    /// C++ `CvPlayer::getConquestUnitClassCount(eUnitClass, pArea)` (BETTER AI).
    pub fn player_get_conquest_unit_class_count(&self, p: PlayerId, unit_class: UnitClassType, area: AreaId) -> i32 {
        let mut count = 0;

        for &loop_unit in &self.players[p].units {
            if self.unit_get_unit_class_type(loop_unit) == unit_class
                && area == self.unit_area(loop_unit)
                && self.units[loop_unit].ai_groupflag == GROUPFLAG_CONQUEST
            {
                count += 1;
            }
        }

        count
    }

    // ------------------------------------------------------------------ group cycle

    /// C++ `CvPlayer::updateGroupCycle(pUnit)`: re-inserts the unit's group into the cycle
    /// next to the groups on its plot (or the closest group).
    pub fn player_update_group_cycle(&mut self, p: PlayerId, unit: UnitId) {
        if !self.unit_on_map(unit) {
            return;
        }

        // FAssertMsg(pUnit->getGroup() != NULL): C++ would put the invalid group id in the cycle
        // and crash on the next walk over it.
        let group = self.units[unit].group.expect("CvPlayer::updateGroupCycle: unit has no group");

        self.player_remove_group_cycle(p, group);

        let plot = self.unit_plot(unit);

        let mut before_unit: Option<UnitId> = None;
        let mut after_unit: Option<UnitId> = None;

        for &loop_unit in &self.map.plots[plot].units {
            if self.unit_is_group_head(loop_unit) {
                if loop_unit != unit {
                    if !self.is_before_unit_cycle(loop_unit, unit) {
                        before_unit = Some(loop_unit);
                        break;
                    } else {
                        after_unit = Some(loop_unit);
                    }
                }
            }
        }

        // C++ `pBestSelectionGroupNode`: the cycle position to insert before (None = NULL; a
        // position past the end is the NULL `nextGroupCycleNode` of the last node).
        let mut best_value = MAX_INT;
        let mut best_index: Option<usize> = None;

        let (unit_x, unit_y) = (self.units[unit].x, self.units[unit].y);
        for (index, &loop_group) in self.players[p].group_cycle.iter().enumerate() {
            // FAssertMsg(pLoopSelectionGroup != NULL)
            if let Some(head_unit) = self.group_get_head_unit(loop_group) {
                if let Some(before_unit) = before_unit {
                    if before_unit == head_unit {
                        best_index = Some(index);
                        break;
                    }
                } else if let Some(after_unit) = after_unit {
                    if after_unit == head_unit {
                        best_index = Some(index + 1);
                        break;
                    }
                } else {
                    let value = self.plot_distance(unit_x, unit_y, self.units[head_unit].x, self.units[head_unit].y);

                    if value < best_value {
                        best_value = value;
                        best_index = Some(index);
                    }
                }
            }
        }

        let cycle = &mut self.players[p].group_cycle;
        match best_index {
            // m_groupCycle.insertBefore(pUnit->getGroupID(), pBestSelectionGroupNode)
            Some(index) if index < cycle.len() => cycle.insert(index, group),
            // m_groupCycle.insertAtEnd(pUnit->getGroupID())
            _ => cycle.push(group),
        }
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
        let d = self.d;
        let info = &d.units[unit];
        let unit_class = unit_class_of(d, unit);
        let team = self.players[p].team;

        // FfH: Modified by Kael 05/09/2008 (the `getCivilizationUnits(eUnitClass) != eUnit`
        // check is commented out)

        if !ignore_cost {
            if info.production_cost == -1 {
                return false;
            }
        }

        if self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && self.players[p].human {
            if info.found {
                return false;
            }
        }

        if self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
            if info.spy || info.espionage_points > 0 {
                return false;
            }
        }

        if !self.team_is_has_tech(team, info.prereq_and_tech) {
            return false;
        }

        // C++ scans the NUM_UNIT_AND_TECH_PREREQS slots and skips NO_TECH.
        for &tech in &info.prereq_and_techs {
            if !self.team_is_has_tech(team, Some(tech)) {
                return false;
            }
        }

        if let Some(state_religion) = info.state_religion {
            if self.player_get_state_religion(p) != Some(state_religion) {
                return false;
            }
        }

        if self.is_unit_class_maxed_out(unit_class, 0) {
            return false;
        }

        if self.team_is_unit_class_maxed_out(team, unit_class, 0) {
            return false;
        }

        if self.player_is_unit_class_maxed_out(p, unit_class, 0) {
            return false;
        }

        if !test_visible {
            let continue_extra = if continue_ { -1 } else { 0 };

            if self.is_unit_class_maxed_out(unit_class, self.team_get_unit_class_making(team, unit_class) + continue_extra) {
                return false;
            }

            if self.team_is_unit_class_maxed_out(team, unit_class, self.team_get_unit_class_making(team, unit_class) + continue_extra) {
                return false;
            }

            if self.player_is_unit_class_maxed_out(p, unit_class, self.players[p].unit_class_making[unit_class] + continue_extra) {
                return false;
            }

            if self.is_no_nukes() || !self.nukes_valid {
                if info.nuke_range != -1 {
                    return false;
                }
            }

            if let Some(special_unit) = info.special_unit_type {
                if !self.special_unit_valid[special_unit] {
                    return false;
                }
            }
        }

        // FfH Units: Added by Kael 08/04/2007
        if let Some(prereq_civic) = info.prereq_civic {
            let mut valid = false;
            for i in 0..max(0, d.defines.max_civic_options) as usize {
                // C++ reads `getCivics(i)` unchecked for every i < MAX_CIVIC_OPTIONS.
                if self.players[p].civics.get(CivicOptionType::from_usize(i)) == Some(&Some(prereq_civic)) {
                    valid = true;
                }
            }
            if !valid {
                return false;
            }
        }
        if info.prereq_global_counter != 0 {
            if self.get_global_counter() < info.prereq_global_counter {
                return false;
            }
        }
        if let Some(prereq_alignment) = info.prereq_alignment {
            if self.players[p].alignment != Some(prereq_alignment) {
                return false;
            }
        }
        // FfH: End Add

        true
    }

    /// C++ `CvPlayer::isProductionMaxedUnitClass(eUnitClass)` (game/team/player limits).
    pub fn player_is_production_maxed_unit_class(&self, p: PlayerId, unit_class: UnitClassType) -> bool {
        // (C++ returns false for NO_UNITCLASS; `unit_class` is never none here.)

        if self.is_unit_class_maxed_out(unit_class, 0) {
            return true;
        }

        if self.team_is_unit_class_maxed_out(self.players[p].team, unit_class, 0) {
            return true;
        }

        if self.player_is_unit_class_maxed_out(p, unit_class, 0) {
            return true;
        }

        false
    }

    /// C++ `CvPlayer::getProductionNeeded(UnitTypes)`.
    pub fn player_get_production_needed_unit(&self, p: PlayerId, unit: UnitType) -> i32 {
        let d = self.d;
        let unit_class = unit_class_of(d, unit);
        let pl = &self.players[p];

        let mut production_needed = d.units[unit].production_cost;

        production_needed *= 100 + pl.unit_class_count[unit_class] * d.unit_classes[unit_class].instance_cost_modifier;
        production_needed /= 100;

        production_needed *= d.defines.unit_production_percent;
        production_needed /= 100;

        production_needed *= d.game_speeds[self.game_speed_type()].train_percent;
        production_needed /= 100;

        production_needed *= d.eras[self.start_era()].train_percent;
        production_needed /= 100;

        if !pl.human && !self.player_is_barbarian(p) {
            let handicap = &d.handicaps[self.handicap];
            if is_world_unit_class(d, unit_class) {
                production_needed *= handicap.ai_world_train_percent;
                production_needed /= 100;
            } else {
                production_needed *= handicap.ai_train_percent;
                production_needed /= 100;
            }

            production_needed *= max(0, (handicap.ai_per_era_modifier * pl.current_era.i()) + 100);
            production_needed /= 100;
        }

        production_needed += self.player_get_unit_extra_cost(p, unit_class);

        // Python cost modifier
        if d.defines.use_get_unit_cost_mod_callback != 0 {
            let result = py_get_unit_cost_mod(self, p, unit);

            if result > 1 {
                production_needed *= result;
                production_needed /= 100;
            }
        }

        max(1, production_needed)
    }

    /// C++ `CvPlayer::getProductionModifier(UnitTypes)`.
    pub fn player_get_production_modifier_unit(&self, p: PlayerId, unit: UnitType) -> i32 {
        let d = self.d;
        let info = &d.units[unit];
        let mut multiplier = 0;

        if info.military_production {
            multiplier += self.players[p].military_production_modifier;
        }

        for t in d.traits.keys() {
            if self.players[p].traits[t] {
                multiplier += info.production_traits[t];

                if let Some(special_unit) = info.special_unit_type {
                    multiplier += d.special_units[special_unit].production_traits[t];
                }
            }
        }

        multiplier
    }

    /// C++ `CvPlayer::isUnitClassMaxedOut(eIndex, iExtra = 0)` (national unit limit).
    pub fn player_is_unit_class_maxed_out(&self, p: PlayerId, unit_class: UnitClassType, extra: i32) -> bool {
        let d = self.d;

        if !is_national_unit_class(d, unit_class) {
            return false;
        }

        // FAssertMsg(getUnitClassCount(eIndex) <= getMaxPlayerInstances())

        (self.players[p].unit_class_count[unit_class] + extra) >= d.unit_classes[unit_class].max_player_instances
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
        let mut count = 0;

        for &loop_unit in &self.players[p].units {
            if self.unit_is_golden_age(loop_unit) {
                count += 1;
            }
        }

        count
    }

    /// C++ `CvPlayer::unitsGoldenAgeReady` (distinct golden-age-capable unit types).
    pub fn player_units_golden_age_ready(&self, p: PlayerId) -> i32 {
        let mut unit_used: IdxVec<UnitType, bool> = IdxVec::from_elem(false, self.d.units.len());
        let mut count = 0;

        for &loop_unit in &self.players[p].units {
            let unit_type = self.units[loop_unit].unit_type;
            if !unit_used[unit_type] {
                if self.unit_is_golden_age(loop_unit) {
                    unit_used[unit_type] = true;
                    count += 1;
                }
            }
        }

        count
    }

    /// C++ `CvPlayer::killGoldenAgeUnits(pUnitAlive)`: consumes the units used for a golden
    /// age (closest to `unit_alive`). C++ quirk: dereferences `pUnitAlive` in the distance
    /// computation even though it may be NULL.
    pub fn player_kill_golden_age_units(&mut self, p: PlayerId, unit_alive: Option<UnitId>) {
        let mut unit_used: IdxVec<UnitType, bool> = IdxVec::from_elem(false, self.d.units.len());

        let mut units_required = self.player_units_required_for_golden_age(p);

        if let Some(unit_alive) = unit_alive {
            unit_used[self.units[unit_alive].unit_type] = true;
            units_required -= 1;
        }

        for _ in 0..units_required {
            let mut best_value = 0;
            let mut best_unit: Option<UnitId> = None;

            for &loop_unit in &self.players[p].units {
                if self.unit_is_golden_age(loop_unit) {
                    if !unit_used[self.units[loop_unit].unit_type] {
                        let mut value = 10000;

                        // C++ quirk: `pUnitAlive` is dereferenced unchecked (C++ crashes for NULL;
                        // every C++ caller passes the unit starting the golden age).
                        let alive = unit_alive.expect("CvPlayer::killGoldenAgeUnits: pUnitAlive is NULL");
                        let (loop_x, loop_y) = (self.units[loop_unit].x, self.units[loop_unit].y);
                        let (alive_x, alive_y) = (self.units[alive].x, self.units[alive].y);
                        value /= self.plot_distance(loop_x, loop_y, alive_x, alive_y) + 1;

                        if value > best_value {
                            best_value = value;
                            best_unit = Some(loop_unit);
                        }
                    }
                }
            }

            // FAssert(pBestUnit != NULL)
            if let Some(best_unit) = best_unit {
                unit_used[self.units[best_unit].unit_type] = true;

                // Delayed death: the unit stays in the list but is no longer golden-age capable.
                self.unit_kill(best_unit, true, None);

                // UI: the MISSION_GOLDEN_AGE animation when the plot is visible to the active
                // player (never headless).
            }
        }
    }

    /// C++ `CvPlayer::greatPeopleThreshold(bMilitary = false)`.
    pub fn player_great_people_threshold(&self, p: PlayerId, military: bool) -> i32 {
        let d = self.d;
        let pl = &self.players[p];
        let speed = &d.game_speeds[self.game_speed_type()];

        let mut threshold = if military {
            (d.defines.great_generals_threshold * max(0, pl.great_generals_threshold_modifier + 100)) / 100
        } else {
            (d.defines.great_people_threshold * max(0, pl.great_people_threshold_modifier + 100)) / 100
        };

        threshold *= speed.great_people_percent;
        if military {
            threshold /= max(1, speed.train_percent);
        } else {
            threshold /= 100;
        }

        threshold *= d.eras[self.start_era()].great_people_percent;
        threshold /= 100;

        max(1, threshold)
    }

    /// C++ `CvPlayer::getDomesticGreatGeneralRateModifier`.
    pub fn player_get_domestic_great_general_rate_modifier(&self, p: PlayerId) -> i32 {
        self.d.defines.combat_experience_in_borders_percent + self.players[p].domestic_great_general_rate_modifier
    }

    /// C++ `CvPlayer::createGreatPeople(eGreatPersonUnit, bIncrementThreshold, bIncrementExperience, iX, iY)`.
    pub fn player_create_great_people(&mut self, p: PlayerId, unit: UnitType, increment_threshold: bool, increment_experience: bool, x: i32, y: i32) {
        let d = self.d;

        let great_people_unit = self.player_init_unit(p, unit, x, y, None, None);
        // (C++ `if (NULL == pGreatPeopleUnit) { FAssert(false); return; }`: initUnit always
        // returns the new unit.)

        if increment_threshold {
            self.players[p].great_people_created += 1;

            let created = self.players[p].great_people_created;
            self.players[p].great_people_threshold_modifier += d.defines.great_people_threshold_increase * ((created / 10) + 1);

            // C++ quirk (BtS): the loop includes this player, whose threshold also gets the team
            // increase.
            let team = self.players[p].team;
            for other in self.player_ids() {
                if self.players[other].team == team {
                    let created = self.players[p].great_people_created;
                    self.players[other].great_people_threshold_modifier +=
                        d.defines.great_people_threshold_increase_team * ((created / 10) + 1);
                }
            }
        }

        if increment_experience {
            self.players[p].great_generals_created += 1;

            let created = self.players[p].great_generals_created;
            self.players[p].great_generals_threshold_modifier += d.defines.great_generals_threshold_increase * ((created / 10) + 1);

            // C++ quirk (BtS): as above, this player is part of its team loop.
            let team = self.players[p].team;
            for other in self.player_ids() {
                if self.players[other].team == team {
                    let created = self.players[p].great_generals_created;
                    self.players[other].great_generals_threshold_modifier +=
                        d.defines.great_generals_threshold_increase_team * ((created / 10) + 1);
                }
            }
        }

        // C++ dereferences `pPlot` (`getPlotCity()`) before its NULL check.
        let plot = self.map.plot_xy(x, y).expect("CvPlayer::createGreatPeople: invalid plot");
        let city = self.map.plots[plot].plot_city;

        // UI: replay message TXT_KEY_MISC_GP_BORN / TXT_KEY_MISC_GP_BORN_FIELD and the
        // per-player messages (TXT_KEY_MISC_GP_BORN_SOMEWHERE where the plot is unrevealed).
        if self.log.enabled && self.units.contains(great_people_unit) {
            let turn = self.game_turn();
            let unit_name = self.unit_get_name(great_people_unit);
            let text = match city {
                Some(c) => format!("{unit_name} has been born in {}", self.cities[c].name),
                None => format!("{unit_name} has been born in the field"),
            };
            self.log.msg(turn, Some(p), || text);
        }

        // Python Event
        if let Some(city) = city {
            self.ev_great_person_born(great_people_unit, p, Some(city));
        }
    }

    /// C++ `CvPlayer::setCombatExperience(iExperience)`: creates a great general (warlord) in
    /// the best city when the threshold is reached.
    pub fn player_set_combat_experience(&mut self, p: PlayerId, experience: i32) {
        let d = self.d;

        // FAssert(iExperience >= 0)

        if experience != self.players[p].combat_experience {
            self.players[p].combat_experience = experience;

            if !self.player_is_barbarian(p) {
                let experience_threshold = self.player_great_people_threshold(p, true);
                if self.players[p].combat_experience >= experience_threshold && experience_threshold > 0 {
                    // create great person
                    let mut best_city: Option<CityId> = None;
                    let mut best_value = MAX_INT;
                    for loop_city in self.players[p].cities.clone() {
                        let num_cities = self.players[p].num_cities();
                        let mut value = 4 * self.soren_rand_num(num_cities, "Warlord City Selection");

                        for i in 0..YieldType::NUM {
                            value += self.city_find_yield_rate_rank(loop_city, YieldType::from_usize(i));
                        }
                        value += self.city_find_population_rank(loop_city);

                        if value < best_value {
                            best_city = Some(loop_city);
                            best_value = value;
                        }
                    }

                    if let Some(best_city) = best_city {
                        let num_unit_infos = d.units.len() as i32;
                        let rand_offset = self.soren_rand_num(num_unit_infos, "Warlord Unit Generation");
                        for i in 0..num_unit_infos {
                            let loop_unit = UnitType::from_usize(((i + rand_offset) % num_unit_infos) as usize);
                            if d.units[loop_unit].leader_experience > 0 || d.units[loop_unit].leader_promotion.is_some() {
                                self.city_create_great_people(best_city, loop_unit, false, true);
                                let remaining = self.players[p].combat_experience - experience_threshold;
                                self.player_set_combat_experience(p, remaining);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvPlayer::changeCombatExperience`.
    pub fn player_change_combat_experience(&mut self, p: PlayerId, change: i32) {
        let experience = self.players[p].combat_experience + change;
        self.player_set_combat_experience(p, experience);
    }

    // ------------------------------------------------------------------ buildings and projects

    /// C++ `CvPlayer::canConstruct(eBuilding, bContinue = false, bTestVisible = false, bIgnoreCost = false)`.
    pub fn player_can_construct(&self, p: PlayerId, building: BuildingType, continue_: bool, test_visible: bool, ignore_cost: bool) -> bool {
        let d = self.d;
        let info = &d.buildings[building];
        let building_class = building_class_of(d, building);
        let team = self.players[p].team;

        // FfH: Modified by Kael 07/05/2008 (the `getCivilizationBuildings(eBuildingClass) !=
        // eBuilding` check is commented out)

        if !ignore_cost {
            if info.production_cost == -1 {
                return false;
            }
        }

        if !self.team_is_has_tech(team, info.prereq_and_tech) {
            return false;
        }

        for &tech in &info.prereq_and_techs {
            if let Some(tech) = tech {
                if !self.team_is_has_tech(team, Some(tech)) {
                    return false;
                }
            }
        }

        if self.team_is_obsolete_building(team, building) {
            return false;
        }

        if let Some(special_building) = info.special_building_type {
            if !self.team_is_has_tech(team, d.special_buildings[special_building].tech_prereq) {
                return false;
            }
        }

        if let Some(state_religion) = info.state_religion {
            if self.player_get_state_religion(p) != Some(state_religion) {
                return false;
            }
        }

        if self.count_civ_teams_ever_alive() < info.num_teams_prereq {
            return false;
        }

        if let Some(victory_prereq) = info.victory_prereq {
            if !self.is_victory_valid(victory_prereq) {
                return false;
            }

            if self.player_is_minor_civ(p) {
                return false;
            }

            if self.teams[team].victory_countdown[victory_prereq] >= 0 {
                return false;
            }
        }

        if let Some(max_start_era) = info.max_start_era {
            if self.start_era() > max_start_era {
                return false;
            }
        }

        if self.is_building_class_maxed_out(building_class, 0) {
            return false;
        }

        if self.team_is_building_class_maxed_out(team, building_class, 0) {
            return false;
        }

        if self.player_is_building_class_maxed_out(p, building_class, 0) {
            return false;
        }

        let civ = civ_of(self, p);
        for i in d.building_classes.keys() {
            if let Some(prereq_building) = d.civ_building(civ, i) {
                if self.team_is_obsolete_building(team, prereq_building) {
                    if self.players[p].building_class_count[i] < self.player_get_building_class_prereq_building(p, building, i, 0) {
                        return false;
                    }
                }
            }
        }

        if !test_visible {
            let continue_extra = if continue_ { -1 } else { 0 };

            if self.is_building_class_maxed_out(building_class, self.team_get_building_class_making(team, building_class) + continue_extra) {
                return false;
            }

            if self.team_is_building_class_maxed_out(team, building_class, self.team_get_building_class_making(team, building_class) + continue_extra)
            {
                return false;
            }

            if self.player_is_building_class_maxed_out(p, building_class, self.players[p].building_class_making[building_class] + continue_extra) {
                return false;
            }

            if self.is_no_nukes() {
                if info.allows_nukes {
                    for loop_unit in d.units.keys() {
                        if d.units[loop_unit].nuke_range != -1 {
                            return false;
                        }
                    }
                }
            }

            if let Some(special_building) = info.special_building_type {
                if !self.special_building_valid[special_building] {
                    return false;
                }
            }

            // Xienwolf Tweak 01/04/09: fixes Kuriotates on small maps for certain buildings
            // (was `getNumCities() < getNumCitiesPrereq()`)
            let num_cities = self.players[p].num_cities();
            let max_cities = self.players[p].max_cities;
            if num_cities < info.num_cities_prereq || (max_cities != -1 && (max_cities < info.num_cities_prereq) && (num_cities != max_cities)) {
                return false;
            }

            if self.players[p].highest_unit_level < info.unit_level_prereq {
                return false;
            }

            for i in d.building_classes.keys() {
                let extra = if continue_ { 0 } else { self.players[p].building_class_making[building_class] };
                if self.players[p].building_class_count[i] < self.player_get_building_class_prereq_building(p, building, i, extra) {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvPlayer::canCreate(eProject, bContinue = false, bTestVisible = false)`.
    pub fn player_can_create(&self, p: PlayerId, project: ProjectType, continue_: bool, test_visible: bool) -> bool {
        let d = self.d;
        let info = &d.projects[project];
        let team = self.players[p].team;

        if self.player_is_barbarian(p) {
            return false;
        }

        if info.production_cost == -1 {
            return false;
        }

        if !self.team_is_has_tech(team, info.tech_prereq) {
            return false;
        }

        if let Some(victory_prereq) = info.victory_prereq {
            if !self.is_victory_valid(victory_prereq) {
                return false;
            }

            if self.player_is_minor_civ(p) {
                return false;
            }

            if self.teams[team].victory_countdown[victory_prereq] >= 0 {
                return false;
            }
        }

        if self.is_project_maxed_out(project, 0) {
            return false;
        }

        if self.team_is_project_maxed_out(team, project, 0) {
            return false;
        }

        if !test_visible {
            let continue_extra = if continue_ { -1 } else { 0 };

            if self.is_project_maxed_out(project, self.teams[team].project_making[project] + continue_extra) {
                return false;
            }

            if self.team_is_project_maxed_out(team, project, self.teams[team].project_making[project] + continue_extra) {
                return false;
            }

            if self.is_no_nukes() {
                if info.allows_nukes {
                    for loop_unit in d.units.keys() {
                        if d.units[loop_unit].nuke_range != -1 {
                            return false;
                        }
                    }
                }
            }

            if let Some(anyone_project_prereq) = info.anyone_project_prereq {
                if self.project_created_count[anyone_project_prereq] == 0 {
                    return false;
                }
            }

            for i in d.projects.keys() {
                if self.teams[team].project_count[i] < info.projects_needed[i] {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvPlayer::canMaintain(eProcess, bContinue = false)`.
    pub fn player_can_maintain(&self, p: PlayerId, process: ProcessType, continue_: bool) -> bool {
        // `bContinue` is unused by the C++ as well.
        let _ = continue_;

        if !self.team_is_has_tech(self.players[p].team, self.d.processes[process].tech_prereq) {
            return false;
        }

        true
    }

    /// C++ `CvPlayer::isProductionMaxedBuildingClass(eBuildingClass, bAcquireCity = false)`.
    pub fn player_is_production_maxed_building_class(&self, p: PlayerId, building_class: BuildingClassType, acquire_city: bool) -> bool {
        // (C++ returns false for NO_BUILDINGCLASS; `building_class` is never none here.)

        if !acquire_city {
            if self.is_building_class_maxed_out(building_class, 0) {
                return true;
            }
        }

        if self.team_is_building_class_maxed_out(self.players[p].team, building_class, 0) {
            return true;
        }

        let extra = if acquire_city { self.d.building_classes[building_class].extra_player_instances } else { 0 };
        if self.player_is_building_class_maxed_out(p, building_class, extra) {
            return true;
        }

        false
    }

    /// C++ `CvPlayer::isProductionMaxedProject(eProject)`.
    pub fn player_is_production_maxed_project(&self, p: PlayerId, project: ProjectType) -> bool {
        // (C++ returns false for NO_PROJECT; `project` is never none here.)

        if self.is_project_maxed_out(project, 0) {
            return true;
        }

        if self.team_is_project_maxed_out(self.players[p].team, project, 0) {
            return true;
        }

        false
    }

    /// C++ `CvPlayer::getProductionNeeded(BuildingTypes)`.
    pub fn player_get_production_needed_building(&self, p: PlayerId, building: BuildingType) -> i32 {
        let d = self.d;
        let pl = &self.players[p];

        let mut production_needed = d.buildings[building].production_cost;

        production_needed *= d.defines.building_production_percent;
        production_needed /= 100;

        production_needed *= d.game_speeds[self.game_speed_type()].construct_percent;
        production_needed /= 100;

        production_needed *= d.eras[self.start_era()].construct_percent;
        production_needed /= 100;

        if !pl.human && !self.player_is_barbarian(p) {
            let handicap = &d.handicaps[self.handicap];
            if is_world_wonder_class(d, building_class_of(d, building)) {
                production_needed *= handicap.ai_world_construct_percent;
                production_needed /= 100;
            } else {
                production_needed *= handicap.ai_construct_percent;
                production_needed /= 100;
            }

            production_needed *= max(0, (handicap.ai_per_era_modifier * pl.current_era.i()) + 100);
            production_needed /= 100;
        }

        max(1, production_needed)
    }

    /// C++ `CvPlayer::getProductionNeeded(ProjectTypes)`.
    pub fn player_get_production_needed_project(&self, p: PlayerId, project: ProjectType) -> i32 {
        let d = self.d;
        let pl = &self.players[p];

        let mut production_needed = d.projects[project].production_cost;

        production_needed *= d.defines.project_production_percent;
        production_needed /= 100;

        production_needed *= d.game_speeds[self.game_speed_type()].create_percent;
        production_needed /= 100;

        production_needed *= d.eras[self.start_era()].create_percent;
        production_needed /= 100;

        if !pl.human && !self.player_is_barbarian(p) {
            let handicap = &d.handicaps[self.handicap];
            if is_world_project(d, project) {
                production_needed *= handicap.ai_world_create_percent;
                production_needed /= 100;
            } else {
                production_needed *= handicap.ai_create_percent;
                production_needed /= 100;
            }

            production_needed *= max(0, (handicap.ai_per_era_modifier * pl.current_era.i()) + 100);
            production_needed /= 100;
        }

        max(1, production_needed)
    }

    /// C++ `CvPlayer::getProductionModifier(BuildingTypes)`.
    pub fn player_get_production_modifier_building(&self, p: PlayerId, building: BuildingType) -> i32 {
        let d = self.d;
        let info = &d.buildings[building];
        let pl = &self.players[p];
        let mut multiplier = 0;

        for t in d.traits.keys() {
            if pl.traits[t] {
                multiplier += info.production_traits[t];

                if let Some(special_building) = info.special_building_type {
                    multiplier += d.special_buildings[special_building].production_traits[t];
                }
            }
        }

        let building_class = building_class_of(d, building);

        if is_world_wonder_class(d, building_class) {
            multiplier += pl.max_global_building_production_modifier;
        }

        if is_team_wonder_class(d, building_class) {
            multiplier += pl.max_team_building_production_modifier;
        }

        if is_national_wonder_class(d, building_class) {
            multiplier += pl.max_player_building_production_modifier;
        }

        multiplier
    }

    /// C++ `CvPlayer::getProductionModifier(ProjectTypes)`.
    pub fn player_get_production_modifier_project(&self, p: PlayerId, project: ProjectType) -> i32 {
        let mut multiplier = 0;

        if self.d.projects[project].spaceship {
            multiplier += self.players[p].space_production_modifier;
        }

        multiplier
    }

    /// C++ `CvPlayer::getBuildingClassPrereqBuilding(eBuilding, ePrereqBuildingClass, iExtra = 0)`.
    pub fn player_get_building_class_prereq_building(&self, p: PlayerId, building: BuildingType, prereq_building_class: BuildingClassType, extra: i32) -> i32 {
        let d = self.d;
        let info = &d.buildings[building];

        let mut prereqs = info.prereq_num_of_building_class[prereq_building_class];

        // dont bother with the rest of the calcs if we have no prereqs
        if prereqs < 1 {
            return 0;
        }

        let building_class = building_class_of(d, building);

        prereqs *= max(0, d.worlds[self.setup.world_size].building_class_prereq_modifier + 100);
        prereqs /= 100;

        if !is_limited_wonder_class(d, building_class) {
            prereqs *= self.players[p].building_class_count[building_class] + extra + 1;
        }

        if self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && self.players[p].human {
            prereqs = prereqs.min(1);
        }

        prereqs
    }

    /// C++ `CvPlayer::removeBuildingClass(eBuildingClass)` (FFH: every building of the class
    /// in every city).
    pub fn player_remove_building_class(&mut self, p: PlayerId, building_class: BuildingClassType) {
        let d = self.d;

        // FfH: Modified by Kael 02/26/2009 (was: the civilization's building of the class, in
        // the first city that has it)
        for loop_city in self.players[p].cities.clone() {
            for building in d.buildings.keys() {
                if d.buildings[building].building_class_type == Some(building_class) {
                    if self.cities[loop_city].num_real_building[building] > 0 {
                        self.city_set_num_real_building(loop_city, building, 0);
                    }
                }
            }
        }
        // FfH: End Modify
    }

    /// C++ `CvPlayer::processBuilding(eBuilding, iChange, pArea)`: player- and area-wide
    /// building effects. C++ quirks: `bChange` is uninitialized when `iChange == 0`, and the
    /// FFH resist modifiers are added without being multiplied by `iChange`.
    pub fn player_process_building(&mut self, p: PlayerId, building: BuildingType, change: i32, area: AreaId) {
        let d = self.d;
        let info = &d.buildings[building];

        if let Some(free_building_class) = info.free_building_class {
            let free_building = d.civ_building(civ_of(self, p), free_building_class);
            // BUGFIX (FreeBuilding from Buildings can be -1)
            if let Some(free_building) = free_building {
                self.player_change_free_building_count(p, free_building, change);
            }
        }

        if let Some(civic_option) = info.civic_option {
            self.players[p].has_civic_option_count[civic_option] += change;
        }

        self.players[p].great_people_rate_modifier += info.global_great_people_rate_modifier * change;
        self.players[p].great_general_rate_modifier += info.great_general_rate_modifier * change;
        self.players[p].domestic_great_general_rate_modifier += info.domestic_great_general_rate_modifier * change;
        self.player_change_anarchy_modifier(p, info.anarchy_modifier * change);
        self.players[p].golden_age_modifier += info.golden_age_modifier * change;
        self.players[p].global_hurry_modifier += info.global_hurry_modifier * change;
        self.players[p].free_experience += info.global_free_experience * change;
        self.player_change_war_weariness_modifier(p, info.global_war_weariness_modifier * change);
        self.area_change_free_specialist(area, p, info.area_free_specialist * change);
        self.player_change_free_specialist(p, info.global_free_specialist * change);
        self.player_change_coastal_trade_routes(p, info.coastal_trade_routes * change);
        self.player_change_trade_routes(p, info.global_trade_routes * change);

        // FfH: Added by Kael 04/11/2008
        // C++ quirk: `bool bChange;` is only assigned for a non-zero iChange (callers never pass
        // 0); for 0 the flags below keep their value here.
        let set_value = if change > 0 {
            Some(true)
        } else if change < 0 {
            Some(false)
        } else {
            None
        };
        if info.hide_units {
            if let Some(value) = set_value {
                self.players[p].hide_units = value;
            }
        }
        if info.see_invisible {
            if let Some(value) = set_value {
                self.players[p].see_invisible = value;
            }
        }
        // C++ quirk: not multiplied by iChange, so removing the building adds the modifiers again.
        self.players[p].resist_enemy_modify += info.global_resist_enemy_modify;
        self.players[p].resist_modify += info.global_resist_modify;
        // FfH: End Add

        if info.area_health > 0 {
            self.area_change_building_good_health(area, p, info.area_health * change);
        } else {
            self.area_change_building_bad_health(area, p, info.area_health * change);
        }
        if info.global_health > 0 {
            self.player_change_building_good_health(p, info.global_health * change);
        } else {
            self.player_change_building_bad_health(p, info.global_health * change);
        }
        self.area_change_building_happiness(area, p, info.area_happiness * change);
        self.player_change_building_happiness(p, info.global_happiness * change);
        self.players[p].worker_speed_modifier += info.worker_speed_modifier * change;
        self.players[p].space_production_modifier += info.global_space_production_modifier * change;
        self.players[p].city_defense_modifier += info.all_city_defense_modifier * change;
        let team = self.players[p].team;
        self.area_change_clean_power_count(area, team, if info.area_clean_power { change } else { 0 });
        self.area_change_border_obstacle_count(area, team, if info.area_border_obstacle { change } else { 0 });

        for i in 0..YieldType::NUM {
            let yield_type = YieldType::from_usize(i);
            self.player_change_sea_plot_yield(p, yield_type, info.global_sea_plot_yield_change[i] * change);
            self.area_change_yield_rate_modifier(area, p, yield_type, info.area_yield_modifier[i] * change);
            self.player_change_yield_rate_modifier(p, yield_type, info.global_yield_modifier[i] * change);
        }

        for i in 0..CommerceType::NUM {
            let commerce = CommerceType::from_usize(i);
            self.player_change_commerce_rate_modifier(p, commerce, info.global_commerce_modifier[i] * change);
            self.player_change_specialist_extra_commerce(p, commerce, info.specialist_extra_commerce[i] * change);
            self.player_change_state_religion_building_commerce(p, commerce, info.state_religion_commerce[i] * change);
            self.player_change_commerce_flexible_count(p, commerce, if info.commerce_flexible[i] { change } else { 0 });
        }

        let civ = civ_of(self, p);
        for i in d.building_classes.keys() {
            if let Some(our_building) = d.civ_building(civ, i) {
                self.player_change_extra_building_happiness(p, our_building, info.building_happiness_changes[i] * change);
            }
        }

        for i in d.specialists.keys() {
            for j in 0..YieldType::NUM {
                self.player_change_specialist_extra_yield(p, i, YieldType::from_usize(j), info.specialist_yield_change[i][j] * change);
            }

            // FfH: Added by Kael 11/06/2007
            for j in 0..CommerceType::NUM {
                self.player_change_specialist_type_extra_commerce(p, i, CommerceType::from_usize(j), info.specialist_commerce_change[i][j] * change);
            }
            // FfH: End Add
        }
    }

    /// C++ `CvPlayer::isBuildingClassMaxedOut(eIndex, iExtra = 0)` (national wonder limit).
    pub fn player_is_building_class_maxed_out(&self, p: PlayerId, building_class: BuildingClassType, extra: i32) -> bool {
        let d = self.d;

        if !is_national_wonder_class(d, building_class) {
            return false;
        }

        let class_info = &d.building_classes[building_class];

        // FAssertMsg(getBuildingClassCount(eIndex) <= max player + extra player instances)

        (self.players[p].building_class_count[building_class] + extra) >= (class_info.max_player_instances + class_info.extra_player_instances)
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
        let d = self.d;

        // BUGFIX (CvBuildinginfo::getFreeBuilding works correct for tolerant cities) Sephi
        let building_class = building_class_of(d, building);

        if change != 0 {
            let old_free_building_count = self.players[p].free_building_count[building];

            self.players[p].free_building_count[building] += change;
            // FAssert(getFreeBuildingCount(eIndex) >= 0)

            if old_free_building_count == 0 {
                // FAssertMsg(getFreeBuildingCount(eIndex) > 0)

                for loop_city in self.players[p].cities.clone() {
                    // BUGFIX (Sephi): only cities whose civilization has a building of the class
                    // (was an unconditional `setNumFreeBuilding(eIndex, 1)`).
                    let true_index = d.civ_building(self.city_civilization_type(loop_city), building_class);
                    if true_index.is_some() {
                        self.city_set_num_free_building(loop_city, building, 1);
                    }

                    // C++ quirk: the original unconditional call was left in after the bugfix.
                    self.city_set_num_free_building(loop_city, building, 1);
                }
            } else if self.players[p].free_building_count[building] == 0 {
                // FAssertMsg(iOldFreeBuildingCount > 0)

                for loop_city in self.players[p].cities.clone() {
                    // BUGFIX (Sephi): was an unconditional `setNumFreeBuilding(eIndex, 0)`.
                    let true_index = d.civ_building(self.city_civilization_type(loop_city), building_class);
                    if true_index.is_some() {
                        self.city_set_num_free_building(loop_city, building, 0);
                    }
                }
            }
        }
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
        let pl = &mut self.players[p];

        let old_hurry_count = pl.hurry_count[hurry];
        pl.hurry_count[hurry] += change;
        // FAssert(getHurryCount(eIndex) >= 0)

        // if we just went from 0 to 1 (or the reverse)
        if (old_hurry_count > 0) != (pl.hurry_count[hurry] > 0) {
            // does this hurry reduce population?
            if self.d.hurries[hurry].production_per_population > 0 {
                // C++ quirk: adds iChange (not +-1), so a change by more than one hurry source at
                // a time would skew the count (rules code only changes it by one).
                pl.pop_rush_hurry_count += change;
                // FAssert(m_iPopRushHurryCount >= 0)
            }
        }
    }

    // ------------------------------------------------------------------ worker builds and routes

    /// C++ `CvPlayer::canBuild(pPlot, eBuild, bTestEra = false, bTestVisible = false)`.
    pub fn player_can_build(&self, p: PlayerId, plot: PlotIdx, build: BuildType, test_era: bool, test_visible: bool) -> bool {
        let d = self.d;
        let info = &d.builds[build];
        let team = self.players[p].team;

        if !self.plot_can_build(plot, build, p, test_visible) {
            return false;
        }

        if let Some(tech_prereq) = info.tech_prereq {
            if !self.team_is_has_tech(team, Some(tech_prereq)) {
                if (!test_era && !test_visible) || ((self.players[p].current_era.i() + 1) < opt_to_i32(d.techs[tech_prereq].era)) {
                    return false;
                }
            }
        }

        if !test_visible {
            if let Some(feature) = self.map.plots[plot].feature_type {
                // FfH: Added by Kael 12/16/2007
                if !d.civilizations[civ_of(self, p)].maintain_features[feature] {
                    if !self.team_is_has_tech(team, info.feature_tech[feature]) {
                        return false;
                    }
                }
            }

            if max(0, self.players[p].gold) < self.player_get_build_cost(p, plot, build) {
                return false;
            }
        }

        true
    }

    /// C++ `CvPlayer::getBuildCost(pPlot, eBuild)` (gold cost, inflated; 0 once started).
    pub fn player_get_build_cost(&self, p: PlayerId, plot: PlotIdx, build: BuildType) -> i32 {
        if self.map.plots[plot].build_progress[build] > 0 {
            return 0;
        }

        max(0, self.d.builds[build].cost * (100 + self.player_calculate_inflation_rate(p))) / 100
    }

    /// C++ `CvPlayer::getBestRoute(pPlot = NULL)`.
    pub fn player_get_best_route(&self, p: PlayerId, plot: Option<PlotIdx>) -> Option<RouteType> {
        let d = self.d;

        let mut best_value = 0;
        let mut best_route: Option<RouteType> = None;

        for build in d.builds.keys() {
            if let Some(route) = d.builds[build].route {
                let valid = match plot {
                    Some(plot) => self.map.plots[plot].route_type == Some(route) || self.player_can_build(p, plot, build, false, false),
                    None => self.team_is_has_tech(self.players[p].team, d.builds[build].tech_prereq),
                };
                if valid {
                    let value = d.routes[route].value;

                    if value > best_value {
                        best_value = value;
                        best_route = Some(route);
                    }
                }
            }
        }

        best_route
    }

    /// C++ `CvPlayer::getImprovementUpgradeRate`.
    pub fn player_get_improvement_upgrade_rate(&self, p: PlayerId) -> i32 {
        let mut rate = 1; // XXX (C++)
        rate *= (self.players[p].improvement_upgrade_rate_modifier + 100).max(0);
        rate /= 100;
        rate
    }
}

/// FFH2 Python `CvGameUtils.getUnitCostMod(argsList)` (called through
/// `USE_GET_UNIT_COST_MOD_CALLBACK`, 0 in FFH2's PythonCallbackDefines.xml): always -1, i.e.
/// no change ("Any value > 0 will be used").
fn py_get_unit_cost_mod(_g: &Game, _p: PlayerId, _unit: UnitType) -> i32 {
    -1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;
    use crate::unit::UnitResetInfo;

    const W: i32 = 8;
    const H: i32 = 6;

    /// A game on an 8x6 map (one land area, no wrap) with real `Player` objects (not
    /// `player_init`ed); the two civ slots play the Amurites and the Sheaim.
    fn test_game() -> Game {
        let d = ffh_data::global();
        let mut setup = GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 11);
        setup.players[0].civ = Some(d.t::<CivilizationType>("CIVILIZATION_AMURITES"));
        setup.players[1].civ = Some(d.t::<CivilizationType>("CIVILIZATION_SHEAIM"));
        let mut g = Game::new(d, setup);
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        g.map = Map::new(d, &MapInitData { grid_w: W, grid_h: H, ..Default::default() }, num_players, num_teams);
        let area = g.map_add_area(false);
        for p in g.map.plot_ids() {
            g.map.plots[p].area = Some(area);
        }
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        g
    }

    /// C++ `addUnit` + `reset`, placed at (x, y) directly (no `init`/`setXY`).
    fn add_unit(g: &mut Game, owner: PlayerId, unit_type: UnitType, x: i32, y: i32) -> UnitId {
        let d = g.d;
        let id = g.units.next_id();
        let u = g.player_add_unit(owner, Unit::new(id, unit_type, owner, d, &UnitResetInfo::from_info(&d.units[unit_type])));
        g.units[u].x = x;
        g.units[u].y = y;
        let plot = g.map.plot_idx(x, y).unwrap();
        g.map.plots[plot].units.push(u);
        u
    }

    /// A selection group holding only `u`.
    fn add_group(g: &mut Game, u: UnitId) -> GroupId {
        let owner = g.units[u].owner;
        let id = g.groups.next_id();
        let group = g.player_add_selection_group(owner, SelectionGroup::new(id, owner));
        g.groups[group].units.push(u);
        g.units[u].group = Some(group);
        group
    }

    #[test]
    fn disband_unit_ai_multipliers() {
        assert_eq!(disband_unit_ai_multiplier(None), 1);
        assert_eq!(disband_unit_ai_multiplier(Some(UnitAIType::UNITAI_ANIMAL)), 1);
        assert_eq!(disband_unit_ai_multiplier(Some(UnitAIType::UNITAI_SETTLE)), 20);
        assert_eq!(disband_unit_ai_multiplier(Some(UnitAIType::UNITAI_MAGE)), 20);
        assert_eq!(disband_unit_ai_multiplier(Some(UnitAIType::UNITAI_WORKER)), 10);
        assert_eq!(disband_unit_ai_multiplier(Some(UnitAIType::UNITAI_COUNTER)), 2);
        assert_eq!(disband_unit_ai_multiplier(Some(UnitAIType::UNITAI_PARADROP)), 6);
        assert_eq!(disband_unit_ai_multiplier(Some(UnitAIType::UNITAI_SPY)), 12);
        assert_eq!(disband_unit_ai_multiplier(Some(UnitAIType::UNITAI_EXPLORE_SEA)), 25);
        assert_eq!(disband_unit_ai_multiplier(Some(UnitAIType::UNITAI_MISSILE_AIR)), 3);
        // Sephi's other new UnitAIs fall into the (assert-free) default.
        assert_eq!(disband_unit_ai_multiplier(Some(UnitAIType::UNITAI_HERO)), 1);
    }

    #[test]
    fn hurry_count_tracks_pop_rush() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(0);
        let pop = d.hurries.keys().find(|&h| d.hurries[h].production_per_population > 0).unwrap();
        let gold = d.hurries.keys().find(|&h| d.hurries[h].production_per_population == 0).unwrap();

        g.player_change_hurry_count(p, gold, 1);
        assert!(g.player_can_hurry(p, gold));
        assert!(!g.player_can_pop_rush(p));

        g.player_change_hurry_count(p, pop, 1);
        g.player_change_hurry_count(p, pop, 1);
        assert_eq!(g.players[p].hurry_count[pop], 2);
        assert_eq!(g.players[p].pop_rush_hurry_count, 1);

        g.player_change_hurry_count(p, pop, -1);
        assert!(g.player_can_pop_rush(p));
        g.player_change_hurry_count(p, pop, -1);
        assert!(!g.player_can_hurry(p, pop));
        assert_eq!(g.players[p].pop_rush_hurry_count, 0);
    }

    #[test]
    fn great_people_threshold() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(0);

        // Normal speed, first era: GREAT_PEOPLE_THRESHOLD 100, GREAT_GENERALS_THRESHOLD 30.
        assert_eq!(g.player_great_people_threshold(p, false), 100);
        assert_eq!(g.player_great_people_threshold(p, true), 30);

        g.players[p].great_people_threshold_modifier = 50;
        g.players[p].great_generals_threshold_modifier = -150;
        assert_eq!(g.player_great_people_threshold(p, false), 150);
        assert_eq!(g.player_great_people_threshold(p, true), 1);

        // Marathon (great people 300%, train 200%): generals scale by 300/200.
        g.setup.game_speed = d.t::<GameSpeedType>("GAMESPEED_MARATHON");
        g.players[p].great_generals_threshold_modifier = 0;
        assert_eq!(g.player_great_people_threshold(p, true), 45);
        assert_eq!(g.player_great_people_threshold(p, false), 450);
    }

    #[test]
    fn production_needed_for_a_human_player() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(0);
        g.players[p].human = true;

        let warrior = d.t::<UnitType>("UNIT_WARRIOR");
        assert_eq!(d.units[warrior].production_cost, 25);
        assert_eq!(g.player_get_production_needed_unit(p, warrior), 25);

        let warrior_class = d.units[warrior].unit_class_type.unwrap();
        g.player_set_unit_extra_cost(p, warrior_class, 7);
        assert_eq!(g.player_get_production_needed_unit(p, warrior), 32);

        let tower = d.t::<BuildingType>("BUILDING_TOWER_OF_ALTERATION");
        assert_eq!(g.player_get_production_needed_building(p, tower), 800);

        let project = d.projects.keys().find(|&j| d.projects[j].production_cost > 0).unwrap();
        assert_eq!(g.player_get_production_needed_project(p, project), d.projects[project].production_cost);

        // Quick speed (67%): integer division before the extra cost is added.
        g.setup.game_speed = d.t::<GameSpeedType>("GAMESPEED_QUICK");
        assert_eq!(g.player_get_production_needed_unit(p, warrior), 25 * 67 / 100 + 7);
        assert_eq!(g.player_get_production_needed_building(p, tower), 800 * 67 / 100);
    }

    #[test]
    fn unit_production_modifier() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(0);

        let warrior = d.t::<UnitType>("UNIT_WARRIOR");
        assert!(d.units[warrior].military_production);
        g.players[p].military_production_modifier = 25;
        assert_eq!(g.player_get_production_modifier_unit(p, warrior), 25);

        let traited = d.units.keys().find(|&u| d.units[u].production_traits.iter().any(|&v| v != 0)).unwrap();
        let t = d.traits.keys().find(|&t| d.units[traited].production_traits[t] != 0).unwrap();
        let base = g.player_get_production_modifier_unit(p, traited);
        g.players[p].traits[t] = true;
        assert_eq!(g.player_get_production_modifier_unit(p, traited), base + d.units[traited].production_traits[t]);
    }

    #[test]
    fn process_building_ffh_flags_and_resist_quirk() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(0);
        let area = g.map.plots[PlotIdx(0)].area.unwrap();

        let tower = d.t::<BuildingType>("BUILDING_TOWER_OF_ALTERATION");
        g.player_process_building(p, tower, 1, area);
        assert_eq!((g.players[p].resist_modify, g.players[p].resist_enemy_modify), (20, -20));
        // C++ quirk: the resist modifiers are not multiplied by iChange.
        g.player_process_building(p, tower, -1, area);
        assert_eq!((g.players[p].resist_modify, g.players[p].resist_enemy_modify), (40, -40));

        let nox_noctis = d.t::<BuildingType>("BUILDING_NOX_NOCTIS");
        g.player_process_building(p, nox_noctis, 1, area);
        assert!(g.players[p].hide_units);
        g.player_process_building(p, nox_noctis, -1, area);
        assert!(!g.players[p].hide_units);

        let dies_diei = d.t::<BuildingType>("BUILDING_DIES_DIEI");
        g.player_process_building(p, dies_diei, 1, area);
        assert!(g.players[p].see_invisible);
        // C++ quirk: with iChange == 0 `bChange` is uninitialized; the flag is left alone here.
        g.player_process_building(p, dies_diei, 0, area);
        assert!(g.players[p].see_invisible);
    }

    #[test]
    fn group_cycle_inserts_before_the_closest_group() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(0);
        let warrior = d.t::<UnitType>("UNIT_WARRIOR");
        let a = add_unit(&mut g, p, warrior, 0, 0);
        let b = add_unit(&mut g, p, warrior, 6, 4);
        let c = add_unit(&mut g, p, warrior, 1, 0);
        let (group_a, group_b, group_c) = (add_group(&mut g, a), add_group(&mut g, b), add_group(&mut g, c));

        g.player_update_group_cycle(p, a);
        assert_eq!(g.players[p].group_cycle, vec![group_a]);
        g.player_update_group_cycle(p, b);
        assert_eq!(g.players[p].group_cycle, vec![group_b, group_a]);
        // Closest head: a (distance 1).
        g.player_update_group_cycle(p, c);
        assert_eq!(g.players[p].group_cycle, vec![group_b, group_c, group_a]);

        // Moved next to b: the old entry is removed, then it goes before b.
        g.units[a].x = 7;
        g.units[a].y = 5;
        g.player_update_group_cycle(p, a);
        assert_eq!(g.players[p].group_cycle, vec![group_a, group_b, group_c]);

        // Off the map: nothing changes.
        g.units[c].x = INVALID_PLOT_COORD;
        g.units[c].y = INVALID_PLOT_COORD;
        g.player_update_group_cycle(p, c);
        assert_eq!(g.players[p].group_cycle, vec![group_a, group_b, group_c]);
    }

    #[test]
    fn unit_counts_by_group_flag_and_religion_spread() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(0);
        let area = g.map.plots[PlotIdx(0)].area.unwrap();

        let (spreader, religion) = d
            .units
            .keys()
            .find_map(|u| d.religions.keys().find(|&r| d.units[u].religion_spreads[r] > 0).map(|r| (u, r)))
            .unwrap();
        let warrior = add_unit(&mut g, p, d.t::<UnitType>("UNIT_WARRIOR"), 1, 1);
        add_unit(&mut g, p, spreader, 2, 2);
        assert_eq!(g.player_count_religion_spread_units(p, area, religion), 1);
        assert_eq!(g.player_count_religion_spread_units(PlayerId(1), area, religion), 0);

        g.units[warrior].ai_groupflag = GROUPFLAG_CONQUEST;
        assert_eq!(g.player_count_group_flag_units(p, GROUPFLAG_CONQUEST), 1);
        assert_eq!(g.player_count_group_flag_units(p, GROUPFLAG_NONE), 1);
    }

    #[test]
    fn counters_without_cities() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(0);

        let tower = d.t::<BuildingType>("BUILDING_TOWER_OF_ALTERATION");
        g.player_change_free_building_count(p, tower, 1);
        g.player_change_free_building_count(p, tower, 1);
        assert!(g.player_is_building_free(p, tower));
        g.player_change_free_building_count(p, tower, -2);
        assert!(!g.player_is_building_free(p, tower));

        // The general threshold (30) is reached, but there is no city to place a warlord in:
        // no sync rand is drawn and the experience is kept.
        let rng = g.soren_rand.clone();
        g.player_change_combat_experience(p, 50);
        assert_eq!(g.players[p].combat_experience, 50);
        assert_eq!(g.soren_rand, rng);

        let barbarian = g.barbarian_player();
        g.player_set_combat_experience(barbarian, 500);
        assert_eq!(g.players[barbarian].combat_experience, 500);
    }

    #[test]
    fn build_cost_and_process_tech() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(0);

        // A started build costs nothing more.
        let plot = g.map.plot_idx(3, 3).unwrap();
        let build = d.builds.keys().next().unwrap();
        g.map.plots[plot].build_progress[build] = 1;
        assert_eq!(g.player_get_build_cost(p, plot, build), 0);

        let process = d.processes.keys().find(|&j| d.processes[j].tech_prereq.is_some()).unwrap();
        assert!(!g.player_can_maintain(p, process, false));
        let team = g.players[p].team;
        g.teams[team].has_tech[d.processes[process].tech_prereq.unwrap()] = true;
        assert!(g.player_can_maintain(p, process, false));
    }

    #[test]
    fn early_production_vetoes() {
        let g = test_game();
        let d = g.d;
        let p = PlayerId(0);

        // Units and buildings costing -1 are never produced (unless the cost is ignored).
        let no_cost_unit = d.units.keys().find(|&u| d.units[u].production_cost == -1).unwrap();
        assert!(!g.player_can_train(p, no_cost_unit, false, false, false));
        let dies_diei = d.t::<BuildingType>("BUILDING_DIES_DIEI");
        assert_eq!(d.buildings[dies_diei].production_cost, -1);
        assert!(!g.player_can_construct(p, dies_diei, false, false, false));

        // Missing prerequisite techs.
        let teched_unit = d
            .units
            .keys()
            .find(|&u| d.units[u].production_cost > 0 && !d.units[u].spy && d.units[u].espionage_points == 0 && d.units[u].prereq_and_tech.is_some())
            .unwrap();
        assert!(!g.player_can_train(p, teched_unit, false, false, false));
        let tower = d.t::<BuildingType>("BUILDING_TOWER_OF_ALTERATION");
        assert!(!g.player_can_construct(p, tower, false, false, false));

        // Barbarians never create projects.
        let project = d.projects.keys().next().unwrap();
        assert!(!g.player_can_create(g.barbarian_player(), project, false, false));

        // No prerequisite count for a class the building does not need.
        let other_class = d.buildings[dies_diei].building_class_type.unwrap();
        assert_eq!(d.buildings[tower].prereq_num_of_building_class[other_class], 0);
        assert_eq!(g.player_get_building_class_prereq_building(p, tower, other_class, 0), 0);
    }
}
