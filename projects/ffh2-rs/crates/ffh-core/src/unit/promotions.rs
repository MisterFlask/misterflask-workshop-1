//! CvUnit promotions, experience and levels, leaders (great generals), upgrades, and the unit
//! characteristics they modify: promotion-driven counters (`changeBlitzCount`, `changeExtraMoves`,
//! ...), hit points and base strength, combat modifiers, first strikes, XP values, interception,
//! fortification and the queries derived from the unit info (`getDomainType`, `isAnimal`, ...).
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges; all of them are ported. UI-only parts (dirty bits, sounds, promotion glow, entity
//! reloads) are omitted with `UI:`/graphics notes; AI-only parts with `// AI:` notes.

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::utils::{is_promotion_valid, is_world_unit_class};
use ffh_data::*;

/// C++ `GROUPFLAG_PERMDEFENSE` (CvDefines.h, Sephi's BETTER AI group flags; read by `canUpgrade`).
const GROUPFLAG_PERMDEFENSE: i32 = 2;
/// C++ `GROUPFLAG_PERMDEFENSE_NEW` (CvDefines.h; read by `canUpgrade`).
const GROUPFLAG_PERMDEFENSE_NEW: i32 = 3;

/// C++ `(UnitClassTypes)kUnitInfo.getUnitClassType()`, used unchecked as an array index by the
/// C++ (every unit info names a class).
fn info_unit_class(info: &UnitInfo) -> UnitClassType {
    info.unit_class_type.expect("CvUnitInfo without a unit class")
}

/// C++ `GET_PLAYER(getOwnerINLINE()).getUnit(iID)`: the unit `id` if it is alive and belongs to
/// `u`'s owner (C++ looks the id up in the owner's unit list), else `None` (NULL). Unit ids are
/// global here, so the owner check stands in for the per-player id space.
fn owner_unit(g: &Game, u: UnitId, id: UnitId) -> Option<UnitId> {
    match g.units.get(id) {
        Some(unit) if unit.owner == g.units[u].owner => Some(id),
        _ => None,
    }
}

/// C++ `getGroup()`: the unit's selection group, NULL without a group or once it was deleted.
fn unit_group_of(g: &Game, u: UnitId) -> Option<GroupId> {
    g.units[u].group.filter(|&grp| g.groups.contains(grp))
}

/// Python `CvGameUtils.getUpgradePriceOverride(iPlayer, iUnitID, iUnitTypeUpgrade)` (always
/// called by `CvUnit::upgradePrice`): FFH2 returns -1 ("any value 0 or above will be used"),
/// i.e. "use the default price".
fn py_get_upgrade_price_override(_g: &Game, _player: PlayerId, _u: UnitId, _unit_type: UnitType) -> i32 {
    -1
}

impl Game {
    /// C++ `CvUnit::canPromote` (CvUnit.cpp:7717-7776).
    /// C++ `iLeaderUnitId` is a unit id of the same owner, -1 = none.
    pub fn unit_can_promote(&self, u: UnitId, promotion: Option<PromotionType>, leader_unit: Option<UnitId>) -> bool {
        let d = self.d;

        if let Some(leader_id) = leader_unit {
            if leader_id == u {
                return false;
            }

            // The command is always possible if it's coming from a Warlord unit that gives just experience points
            // (C++ `NO_UNIT != pWarlord->getUnitType()` always holds for a live unit.)
            if let Some(warlord) = owner_unit(self, u, leader_id) {
                let warlord_info = &d.units[self.units[warlord].unit_type];
                if warlord_info.leader_experience > 0
                    && warlord_info.leader_promotion.is_none()
                    && self.unit_can_acquire_promotion_any(u)
                {
                    return true;
                }
            }
        }

        let Some(promotion) = promotion else {
            return false;
        };

        if !self.unit_can_acquire_promotion(u, promotion) {
            return false;
        }

        // FfH Units: Added by Kael 08/04/2007
        if self.units[u].free_promotion_pick > 0 {
            return true;
        }
        // FfH: End Add

        if d.promotions[promotion].leader {
            if let Some(leader_id) = leader_unit {
                if let Some(warlord) = owner_unit(self, u, leader_id) {
                    return d.units[self.units[warlord].unit_type].leader_promotion == Some(promotion);
                }
            }
            return false;
        } else if !self.units[u].promotion_ready {
            return false;
        }

        true
    }

    /// C++ `CvUnit::promote` (CvUnit.cpp:7778-7835).
    /// C++ quirk: `canPromote` accepts NO_PROMOTION when a warlord only gives experience, and
    /// `promote` then indexes the promotion table with it. Keep the call order; guard the lookup.
    pub fn unit_promote(&mut self, u: UnitId, promotion: Option<PromotionType>, leader_unit: Option<UnitId>) {
        let d = self.d;

        if !self.unit_can_promote(u, promotion, leader_unit) {
            return;
        }

        if let Some(leader_id) = leader_unit {
            if let Some(warlord) = owner_unit(self, u, leader_id) {
                self.unit_give_experience(warlord);
                if !self.units[warlord].name.is_empty() {
                    let name = self.unit_get_name_key(warlord);
                    self.unit_set_name(u, name);
                }

                //update graphics models
                self.units[u].leader_unit_type = Some(self.units[warlord].unit_type);
                // Graphics: `reloadEntity()`.
            }
        }

        // C++ quirk: `GC.getPromotionInfo(NO_PROMOTION).isLeader()` reads outside the promotion
        // table (only reachable through a warlord that gives just experience, which no FFH2 unit
        // is); a missing promotion reads as "not a leader promotion" here.
        let is_leader = promotion.map_or(false, |p| d.promotions[p].leader);

        // FfH Units: Modified by Kael 08/04/2007 (also requires no free promotion pick)
        if !is_leader && self.units[u].free_promotion_pick == 0 {
            self.unit_change_level(u, 1);
            let damage = self.units[u].damage;
            self.unit_change_damage(u, -(damage / 2), None);
        }

        // FfH: Added by Kael 01/09/2008
        if self.units[u].free_promotion_pick > 0 {
            self.unit_change_free_promotion_pick(u, -1);
        }
        // FfH: End Add

        // C++ quirk: `setHasPromotion(NO_PROMOTION, true)` would index the promotion arrays with
        // -1; with no promotion there is nothing to set.
        if let Some(promotion) = promotion {
            self.unit_set_has_promotion(u, promotion, true);
            // C++ keeps using `this`; a promotion effect that killed the unit (an invisibility
            // change withdrawing it with nowhere to go) would be a use-after-free there.
            if !self.units.contains(u) {
                return;
            }
        }

        self.unit_test_promotion_ready(u);

        // UI: the promotion sound for the selected unit, info pane / info bar dirty bits.

        // SIGNATURE: `ev_unit_promoted` takes a `PromotionType`; C++ reports `unitPromoted(this,
        // NO_PROMOTION)` for an experience-only warlord (no FFH2 unit is one), skipped here.
        if let Some(promotion) = promotion {
            self.ev_unit_promoted(u, promotion);
        }
    }

    /// C++ `CvUnit::lead` (CvUnit.cpp:7837-7877).
    /// `unit` `None` (C++ -1) opens the lead-unit popup for humans and returns false.
    pub fn unit_lead(&mut self, u: UnitId, unit: Option<UnitId>) -> bool {
        // C++ `canLead(plot(), iUnitId)` (the plot is only dereferenced for `iUnitId == -1`).
        let plot = self.unit_plot(u);
        if self.unit_can_lead(u, plot, unit) == 0 {
            return false;
        }

        let leader_promotion = self.d.units[self.units[u].unit_type].leader_promotion;

        let Some(unit_id) = unit else {
            // UI: `BUTTONPOPUP_LEADUNIT` popup for the owner; the human's pick comes back as
            // `COMMAND_LEAD` with the chosen unit, i.e. `unit_lead(u, Some(target))`.
            return false;
        };

        let target = owner_unit(self, u, unit_id);
        let Some(target) = target.filter(|&t| self.unit_can_promote(t, leader_promotion, Some(u))) else {
            return false;
        };

        self.unit_join_group(target, None, true);

        self.unit_promote(target, leader_promotion, Some(u));

        // Graphics: `NotifyEntity(MISSION_LEAD)` when the plot is visible to the active team.

        self.unit_kill(u, true, None);

        true
    }

    /// C++ `CvUnit::canLead` (CvUnit.cpp:7880-7920).
    /// Returns the number of units that could be led (C++ `int canLead`).
    pub fn unit_can_lead(&self, u: UnitId, plot: PlotIdx, unit: Option<UnitId>) -> i32 {
        if self.units[u].death_delay {
            return 0;
        }

        // C++ `NO_UNIT == getUnitType()`: never true for a live unit.

        let mut num_units = 0;
        let leader_promotion = self.d.units[self.units[u].unit_type].leader_promotion;
        let owner = self.units[u].owner;

        match unit {
            None => {
                for &loop_unit in &self.map.plots[plot].units {
                    // C++ `::getUnit(IDInfo)` is NULL for a dead unit.
                    if !self.units.contains(loop_unit) {
                        continue;
                    }
                    if loop_unit != u
                        && self.units[loop_unit].owner == owner
                        && self.unit_can_promote(loop_unit, leader_promotion, Some(u))
                    {
                        num_units += 1;
                    }
                }
            }
            Some(unit_id) => {
                if let Some(target) = owner_unit(self, u, unit_id) {
                    if target != u && self.unit_can_promote(target, leader_promotion, Some(u)) {
                        num_units = 1;
                    }
                }
            }
        }
        num_units
    }

    /// C++ `CvUnit::canGiveExperience` (CvUnit.cpp:7923-7943).
    pub fn unit_can_give_experience(&self, u: UnitId, plot: PlotIdx) -> i32 {
        let mut num_units = 0;

        // C++ also tests `NO_UNIT != getUnitType()` (always true for a live unit).
        if self.d.units[self.units[u].unit_type].leader_experience > 0 {
            let owner = self.units[u].owner;
            for &loop_unit in &self.map.plots[plot].units {
                if self.units.contains(loop_unit)
                    && loop_unit != u
                    && self.units[loop_unit].owner == owner
                    && self.unit_can_acquire_promotion_any(loop_unit)
                {
                    num_units += 1;
                }
            }
        }

        num_units
    }

    /// C++ `CvUnit::giveExperience` (CvUnit.cpp:7945-7980).
    pub fn unit_give_experience(&mut self, u: UnitId) -> bool {
        if let Some(plot) = self.unit_plot_opt(u) {
            let num_units = self.unit_can_give_experience(u, plot);
            if num_units > 0 {
                let total_experience = self.unit_get_stack_experience_to_give(u, num_units);

                let min_experience_per_unit = total_experience / num_units;
                let remainder = total_experience % num_units;

                let owner = self.units[u].owner;
                let mut i = 0;
                for loop_unit in self.map.plots[plot].units.clone() {
                    if self.units.contains(loop_unit)
                        && loop_unit != u
                        && self.units[loop_unit].owner == owner
                        && self.unit_can_acquire_promotion_any(loop_unit)
                    {
                        let experience = if i < remainder { min_experience_per_unit + 1 } else { min_experience_per_unit };
                        self.unit_change_experience(loop_unit, experience, -1, false, false, false);
                        self.unit_test_promotion_ready(loop_unit);
                    }

                    // C++ quirk: `i` counts every unit on the plot (the leader and units that get
                    // nothing included), so only receivers at plot positions below the remainder
                    // get the extra point and part of the remainder can be lost.
                    i += 1;
                }

                return true;
            }
        }

        false
    }

    /// C++ `CvUnit::getStackExperienceToGive` (CvUnit.cpp:7982-7985).
    pub fn unit_get_stack_experience_to_give(&self, u: UnitId, num_units: i32) -> i32 {
        let leader_experience = self.d.units[self.units[u].unit_type].leader_experience;
        (leader_experience
            * (100 + std::cmp::min(50, (num_units - 1) * self.d.defines.warlord_extra_experience_per_unit_percent)))
            / 100
    }

    /// C++ `CvUnit::upgradePrice` (CvUnit.cpp:7987-8027).
    /// Starts with the `getUpgradePriceOverride` Python callback (FFH2: always "use the default").
    pub fn unit_upgrade_price(&self, u: UnitId, unit_type: UnitType) -> i32 {
        let owner = self.units[u].owner;

        let result = py_get_upgrade_price_override(self, owner, u, unit_type);
        if result >= 0 {
            return result;
        }

        if self.unit_is_barbarian(u) {
            return 0;
        }

        let defines = &self.d.defines;
        let mut price = defines.base_unit_upgrade_cost;

        price += std::cmp::max(
            0,
            self.player_get_production_needed_unit(owner, unit_type)
                - self.player_get_production_needed_unit(owner, self.units[u].unit_type),
        ) * defines.unit_upgrade_cost_per_production;

        if !self.unit_is_human(u) && !self.unit_is_barbarian(u) {
            let handicap = &self.d.handicaps[self.handicap];
            price *= handicap.ai_unit_upgrade_percent;
            price /= 100;

            price *= std::cmp::max(0, (handicap.ai_per_era_modifier * self.players[owner].current_era.i()) + 100);
            price /= 100;
        }

        price -= (price * self.units[u].upgrade_discount) / 100;

        // FfH Traits: Added by Kael 08/02/2007
        price += (price * self.players[owner].upgrade_cost_modifier) / 100;
        // FfH: End Add

        price
    }

    /// C++ `CvUnit::upgradeAvailable` (CvUnit.cpp:8030-8064).
    /// Recursive; C++ default `iCount = 0`.
    pub fn unit_upgrade_available(
        &self,
        u: UnitId,
        from_unit: UnitType,
        to_unit_class: UnitClassType,
        count: i32,
    ) -> bool {
        let d = self.d;
        let num_unit_class_infos = d.unit_classes.len() as i32;

        if count > num_unit_class_infos {
            return false;
        }

        let from_unit_info = &d.units[from_unit];

        if from_unit_info.upgrade_unit_class[to_unit_class] {
            return true;
        }

        for class in d.unit_classes.keys() {
            if from_unit_info.upgrade_unit_class[class] {
                if let Some(loop_unit) = d.civ_unit(self.unit_get_civilization_type(u), class) {
                    if self.unit_upgrade_available(u, loop_unit, to_unit_class, count + 1) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// C++ `CvUnit::canUpgrade` (CvUnit.cpp:8067-8138).
    pub fn unit_can_upgrade(&self, u: UnitId, unit_type: Option<UnitType>, test_visible: bool) -> bool {
        let Some(unit_type) = unit_type else {
            return false;
        };

        if !self.unit_is_ready_for_upgrade(u) {
            return false;
        }

        let owner = self.units[u].owner;
        if !test_visible {
            if self.players[owner].gold < self.unit_upgrade_price(u, unit_type) {
                return false;
            }
        }

        let info = &self.d.units[unit_type];

        // FfH Units: Added by Kael 05/24/2008
        if self.units[u].level < info.min_level {
            if self.unit_is_human(u) || !self.is_option(GameOptionType::GAMEOPTION_AI_NO_MINIMUM_LEVEL) {
                return false;
            }
        }
        if info.disable_upgrade_to {
            return false;
        }
        if self.player_is_unit_class_maxed_out(owner, info_unit_class(info), 0) {
            return false;
        }
        if !self.unit_is_human(u) {
            //added so the AI wont spam UNTIAI_MISSIONARY priests by upgradign disciples
            if self.units[u].ai_type == Some(UnitAIType::UNITAI_MISSIONARY) && self.units[u].level < 2 {
                return false;
            }
        }
        // FfH: End Add

        // BETTER AI (Smarter Upgrading) Sephi 0.41k
        if !self.unit_is_human(u) {
            let group_flag = self.units[u].ai_groupflag;
            if group_flag == GROUPFLAG_PERMDEFENSE || group_flag == GROUPFLAG_PERMDEFENSE_NEW {
                if info.no_defensive_bonus {
                    return false;
                }
            }
        }
        // END

        if self.unit_has_upgrade_to(u, Some(unit_type), false) {
            return true;
        }

        false
    }

    /// C++ `CvUnit::isReadyForUpgrade` (CvUnit.cpp:8140-8153).
    pub fn unit_is_ready_for_upgrade(&self, u: UnitId) -> bool {
        if !self.unit_can_move(u) {
            return false;
        }

        if self.plot_get_team(self.unit_plot(u)) != Some(self.unit_get_team(u)) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::hasUpgrade` (CvUnit.cpp:8159-8162).
    /// C++ overload `hasUpgrade(bool bSearch)`.
    pub fn unit_has_upgrade(&self, u: UnitId, search: bool) -> bool {
        self.unit_get_upgrade_city(u, search).is_some()
    }

    /// C++ `CvUnit::hasUpgrade` (CvUnit.cpp:8168-8171).
    /// C++ overload `hasUpgrade(UnitTypes eUnit, bool bSearch)`.
    pub fn unit_has_upgrade_to(&self, u: UnitId, unit_type: Option<UnitType>, search: bool) -> bool {
        self.unit_get_upgrade_city_for(u, unit_type, search, None).is_some()
    }

    /// C++ `CvUnit::getUpgradeCity` (CvUnit.cpp:8178-8214).
    /// C++ overload `getUpgradeCity(bool bSearch)`. Ranks candidate unit types with
    /// `CvPlayerAI::AI_unitValue` (AI); only AI code calls it.
    ///
    /// AI: C++ only tries the unit types whose `AI_unitValue(eUnit, AI_getUnitAIType(), area())`
    /// is above the unit's own type's. AI valuations are not part of the rules port (the AI crate
    /// ranks upgrades itself through [`Game::unit_get_upgrade_city_for`]), so every other unit
    /// type is a candidate here, in type order; the search itself is the C++ one.
    pub fn unit_get_upgrade_city(&self, u: UnitId, search: bool) -> Option<CityId> {
        let current = self.units[u].unit_type;

        let mut best_search_value = MAX_INT;
        let mut best_upgrade_city = None;

        for unit_type in self.d.units.keys() {
            // AI: `iNewValue > iCurrentValue` (the unit's own type never beats itself).
            if unit_type == current {
                continue;
            }

            let mut search_value = 0;
            let upgrade_city = self.unit_get_upgrade_city_for(u, Some(unit_type), search, Some(&mut search_value));
            if let Some(upgrade_city) = upgrade_city {
                // if not searching or close enough, then this match will do
                if !search || search_value < 16 {
                    return Some(upgrade_city);
                }

                if search_value < best_search_value {
                    best_search_value = search_value;
                    best_upgrade_city = Some(upgrade_city);
                }
            }
        }

        best_upgrade_city
    }

    /// C++ `CvUnit::getUpgradeCity` (CvUnit.cpp:8222-8428).
    /// C++ overload `getUpgradeCity(UnitTypes eUnit, bool bSearch, int* iSearchValue)`.
    /// Sephi speed tweak: only cities with squared distance < 10 are path-tested.
    pub fn unit_get_upgrade_city_for(
        &self,
        u: UnitId,
        unit_type: Option<UnitType>,
        search: bool,
        mut search_value: Option<&mut i32>,
    ) -> Option<CityId> {
        let Some(unit_type) = unit_type else {
            // C++ `return false;` (NULL).
            return None;
        };

        let d = self.d;
        let owner = self.units[u].owner;
        let my_info = &d.units[self.units[u].unit_type];
        let k_unit_info = &d.units[unit_type];
        let unit_class = info_unit_class(k_unit_info);
        let assimilation = self.players[owner].assimilation;

        // FfH: Modified by Kael 05/09/2008
        match my_info.upgrade_civ {
            None => {
                if !assimilation {
                    // C++ `kPlayer.getCivilizationType()` (= the unit's civilization).
                    if d.civ_unit(self.unit_get_civilization_type(u), unit_class) != Some(unit_type) {
                        return None;
                    }
                }
            }
            Some(upgrade_civ) => {
                if d.civ_unit(upgrade_civ, unit_class) != Some(unit_type) {
                    return None;
                }
            }
        }
        // FfH: End Modify

        if !self.unit_upgrade_available(u, self.units[u].unit_type, unit_class, 0) {
            return None;
        }

        if k_unit_info.cargo_space < self.units[u].cargo {
            return None;
        }

        let plot = self.unit_plot(u);
        for &loop_unit in &self.map.plots[plot].units {
            // C++ `pLoopUnit->getTransportUnit() == this`.
            if self.units[loop_unit].transport_unit == Some(u) {
                if let Some(special_cargo) = k_unit_info.special_cargo {
                    if Some(special_cargo) != self.unit_get_special_unit_type(loop_unit) {
                        return None;
                    }
                }

                if let Some(domain_cargo) = k_unit_info.domain_cargo {
                    if domain_cargo != self.unit_get_domain_type(loop_unit) {
                        return None;
                    }
                }
            }
        }

        // sea units must be built on the coast
        let coastal_only = self.unit_get_domain_type(u) == DomainType::DOMAIN_SEA;

        // results
        let mut best_value = MAX_INT;
        let mut best_city = None;

        // if search is true, check every city for our team
        if search {
            // air units can travel any distance
            let ignore_distance = self.unit_get_domain_type(u) == DomainType::DOMAIN_AIR;

            let team = self.unit_get_team(u);
            let area = self.unit_area(u);
            let (x, y) = (self.units[u].x, self.units[u].y);

            // check every player on our team's cities
            for loop_player in self.player_ids() {
                // is this player on our team?
                if self.players[loop_player].alive && self.players[loop_player].team == team {
                    for &loop_city in &self.players[loop_player].cities {
                        // if coastal only, then make sure we are coast
                        // (C++ only computes `pWaterArea` when `bCoastalOnly`.)
                        let mut water_area = None;
                        let coast_ok = if coastal_only {
                            water_area = self.city_water_area(loop_city);
                            water_area.is_some_and(|a| !self.area_is_lake(a))
                        } else {
                            true
                        };
                        if coast_ok {
                            // can this city tran this unit?
                            // FfH Units: Modified by Kael 05/24/2008 (`canUpgrade` instead of `canTrain`)
                            if self.city_can_upgrade(loop_city, unit_type, false, false, true, false) {
                                // if we do not care about distance, then the first match will do
                                if ignore_distance {
                                    // if we do not care about distance, then return 1 for value
                                    if let Some(sv) = search_value.as_deref_mut() {
                                        *sv = 1;
                                    }

                                    return Some(loop_city);
                                }

                                let (city_x, city_y) = (self.cities[loop_city].x, self.cities[loop_city].y);
                                let mut value = self.plot_distance(x, y, city_x, city_y);

                                // if not same area, not as good (lower numbers are better)
                                if area != self.city_area(loop_city) && (!coastal_only || Some(area) != water_area) {
                                    value *= 16;
                                }

                                // if we cannot path there, not as good (lower numbers are better)
                                // SPEED TWEAK Sephi: we only check for cities not that far away.
                                // C++ quirk: the distance is the raw (unwrapped) coordinate
                                // difference between the city's plot and the unit's plot.
                                let x_dist = city_x - x;
                                let y_dist = city_y - y;
                                if (x_dist * x_dist) + (y_dist * y_dist) < 10 {
                                    let city_plot = self.city_plot(loop_city);
                                    if !self.unit_generate_path(u, city_plot, 0, true, None) {
                                        value *= 16;
                                    }
                                } else {
                                    value *= 16;
                                }

                                if value < best_value {
                                    best_value = value;
                                    best_city = Some(loop_city);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // find the closest city
            let (x, y) = (self.units[u].x, self.units[u].y);
            let team = self.unit_get_team(u);
            let closest_city = self.map_find_city(x, y, None, Some(team), true, coastal_only, None, None, None);
            if let Some(closest_city) = closest_city {
                // if we can train, then return this city (otherwise it will return NULL)

                // FfH Units: Modified by Kael 08/07/2007
                if assimilation && my_info.upgrade_civ.is_none() {
                    if d.civ_unit(self.city_civilization_type(closest_city), unit_class) != Some(unit_type)
                        && d.civ_unit(self.unit_get_civilization_type(u), unit_class) != Some(unit_type)
                    {
                        return None;
                    }
                }
                if self.city_can_upgrade(closest_city, unit_type, false, false, true, false) {
                    // FfH: End Add
                    // did not search, always return 1 for search value
                    best_value = 1;

                    best_city = Some(closest_city);
                }
            }
        }

        // return the best value, if non-NULL
        if let Some(sv) = search_value {
            *sv = best_value;
        }

        best_city
    }

    /// C++ `CvUnit::upgrade` (CvUnit.cpp:8430-8470).
    pub fn unit_upgrade(&mut self, u: UnitId, unit_type: Option<UnitType>) {
        if !self.unit_can_upgrade(u, unit_type, false) {
            return;
        }
        // `canUpgrade` rejects NO_UNIT.
        let Some(unit_type) = unit_type else {
            return;
        };

        let owner = self.units[u].owner;
        let price = self.unit_upgrade_price(u, unit_type);
        // `changeGold(-(upgradePrice(eUnit)))` (the rest of setGold is interface updates).
        self.players[owner].gold += -price;

        // FfH: Modified by Kael 04/18/2009 (missionaries that could not spread the state religion
        // any more become reserve units)
        let mut unit_ai = self.units[u].ai_type;
        if unit_ai == Some(UnitAIType::UNITAI_MISSIONARY) {
            let religion = self.player_get_state_religion(owner);
            if religion.map_or(true, |r| self.d.units[unit_type].religion_spreads[r] == 0) {
                unit_ai = Some(UnitAIType::UNITAI_RESERVE);
            }
        }
        let (x, y) = (self.units[u].x, self.units[u].y);
        let upgrade_unit = self.player_init_unit(owner, unit_type, x, y, unit_ai, None);
        // FfH: End Modify

        let group = unit_group_of(self, u);
        self.unit_join_group(upgrade_unit, group, true);

        self.unit_convert(upgrade_unit, u);
        // C++ keeps using `pUpgradeUnit`; a promotion effect applied by `convert` that killed it
        // would be a use-after-free there.
        if !self.units.contains(upgrade_unit) {
            return;
        }

        self.unit_finish_moves(upgrade_unit);

        if self.units[upgrade_unit].leader_unit_type.is_none() {
            let max_experience = self.d.defines.max_experience_after_upgrade;
            if self.units[upgrade_unit].experience > max_experience {
                self.unit_set_experience(upgrade_unit, max_experience, -1);
            }
        }
    }

    /// C++ `CvUnit::getHandicapType` (CvUnit.cpp:8473-8476).
    pub fn unit_get_handicap_type(&self, u: UnitId) -> HandicapType {
        // C++ `GET_PLAYER(getOwnerINLINE()).getHandicapType()` (CvInitCore always holds one).
        self.player_get_handicap_type(self.units[u].owner)
            .expect("CvUnit::getHandicapType: owner without a handicap")
    }

    /// C++ `CvUnit::getCivilizationType` (CvUnit.cpp:8479-8482).
    pub fn unit_get_civilization_type(&self, u: UnitId) -> CivilizationType {
        // C++ `GET_PLAYER(getOwnerINLINE()).getCivilizationType()` (a unit's owner always has a
        // civilization; C++ would index the civilization table with NO_CIVILIZATION otherwise).
        self.player_get_civilization_type(self.units[u].owner)
            .expect("CvUnit::getCivilizationType: owner without a civilization")
    }

    /// C++ `CvUnit::getSpecialUnitType` (CvUnit.cpp:8494-8497).
    pub fn unit_get_special_unit_type(&self, u: UnitId) -> Option<SpecialUnitType> {
        self.d.units[self.units[u].unit_type].special_unit_type
    }

    /// C++ `CvUnit::getCaptureUnitType` (CvUnit.cpp:8500-8504).
    pub fn unit_get_capture_unit_type(&self, u: UnitId, civilization: CivilizationType) -> Option<UnitType> {
        match self.d.units[self.units[u].unit_type].unit_capture_class_type {
            None => None,
            Some(capture_class) => self.d.civ_unit(civilization, capture_class),
        }
    }

    /// C++ `CvUnit::getUnitCombatType` (CvUnit.cpp:8507-8510).
    pub fn unit_get_unit_combat_type(&self, u: UnitId) -> Option<UnitCombatType> {
        self.d.units[self.units[u].unit_type].unit_combat_type
    }

    /// C++ `CvUnit::getDomainType` (CvUnit.cpp:8513-8516).
    pub fn unit_get_domain_type(&self, u: UnitId) -> DomainType {
        // Every FFH2 unit info names a domain.
        self.d.units[self.units[u].unit_type].domain_type.expect("CvUnitInfo without a domain")
    }

    /// C++ `CvUnit::flavorValue` (CvUnit.cpp:8586-8589).
    pub fn unit_flavor_value(&self, u: UnitId, flavor: FlavorType) -> i32 {
        self.d.units[self.units[u].unit_type].flavor_value[flavor]
    }

    /// C++ `CvUnit::isBarbarian` (CvUnit.cpp:8592-8595).
    pub fn unit_is_barbarian(&self, u: UnitId) -> bool {
        // C++ `GET_PLAYER(getOwnerINLINE()).isBarbarian()` == `getID() == BARBARIAN_PLAYER`.
        self.units[u].owner == self.barbarian_player()
    }

    /// C++ `CvUnit::isHuman` (CvUnit.cpp:8598-8601).
    pub fn unit_is_human(&self, u: UnitId) -> bool {
        // C++ `GET_PLAYER(getOwnerINLINE()).isHuman()` (`m_bHuman`).
        self.players[self.units[u].owner].human
    }

    /// C++ `CvUnit::airRange` (CvUnit.cpp:8666-8669).
    pub fn unit_air_range(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].air_range + self.units[u].extra_air_range
    }

    /// C++ `CvUnit::nukeRange` (CvUnit.cpp:8672-8675).
    pub fn unit_nuke_range(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].nuke_range
    }

    /// C++ `CvUnit::isAnimal` (CvUnit.cpp:8799-8802).
    pub fn unit_is_animal(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].animal
    }

    /// C++ `CvUnit::isNoBadGoodies` (CvUnit.cpp:8805-8808).
    pub fn unit_is_no_bad_goodies(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].no_bad_goodies
    }

    /// C++ `CvUnit::isOnlyDefensive` (CvUnit.cpp:8811-8822).
    /// FfH: `only_defensive > 0` or the unit info flag.
    pub fn unit_is_only_defensive(&self, u: UnitId) -> bool {
        // FfH Promotions: Added by Kael 08/14/2007
        if self.units[u].only_defensive > 0 {
            return true;
        }
        // FfH: End Add

        self.d.units[self.units[u].unit_type].only_defensive
    }

    /// C++ `CvUnit::isNoCapture` (CvUnit.cpp:8825-8836).
    /// FfH: hidden-nationality units never capture.
    pub fn unit_is_no_capture(&self, u: UnitId) -> bool {
        // FfH: Added by Kael 10/25/2007
        if self.unit_is_hidden_nationality(u) {
            return true;
        }
        // FfH: End Add

        self.d.units[self.units[u].unit_type].no_capture
    }

    /// C++ `CvUnit::isRivalTerritory` (CvUnit.cpp:8839-8842).
    pub fn unit_is_rival_territory(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].rival_territory
    }

    /// C++ `CvUnit::isMilitaryHappiness` (CvUnit.cpp:8845-8848).
    pub fn unit_is_military_happiness(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].military_happiness
    }

    /// C++ `CvUnit::isInvestigate` (CvUnit.cpp:8851-8854).
    pub fn unit_is_investigate(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].investigate
    }

    /// C++ `CvUnit::isCounterSpy` (CvUnit.cpp:8857-8860).
    pub fn unit_is_counter_spy(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].counter_spy
    }

    /// C++ `CvUnit::isSpy` (CvUnit.cpp:8863-8866).
    pub fn unit_is_spy(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].spy
    }

    /// C++ `CvUnit::isFound` (CvUnit.cpp:8869-8872).
    pub fn unit_is_found(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].found
    }

    /// C++ `CvUnit::isGoldenAge` (CvUnit.cpp:8875-8883).
    pub fn unit_is_golden_age(&self, u: UnitId) -> bool {
        if self.units[u].death_delay {
            return false;
        }

        self.d.units[self.units[u].unit_type].golden_age
    }

    /// C++ `CvUnit::maxHitPoints` (CvUnit.cpp:8929-8932).
    pub fn unit_max_hit_points(&self, _u: UnitId) -> i32 {
        self.d.defines.max_hit_points
    }

    /// C++ `CvUnit::currHitPoints` (CvUnit.cpp:8935-8938).
    pub fn unit_curr_hit_points(&self, u: UnitId) -> i32 {
        self.unit_max_hit_points(u) - self.units[u].damage
    }

    /// C++ `CvUnit::isHurt` (CvUnit.cpp:8941-8944).
    pub fn unit_is_hurt(&self, u: UnitId) -> bool {
        self.units[u].damage > 0
    }

    /// C++ `CvUnit::isDead` (CvUnit.cpp:8947-8950).
    pub fn unit_is_dead(&self, u: UnitId) -> bool {
        self.units[u].damage >= self.unit_max_hit_points(u)
    }

    /// C++ `CvUnit::baseCombatStr` (CvUnit.cpp:8958-8971).
    pub fn unit_base_combat_str(&self, u: UnitId) -> i32 {
        // FfH: base strength plus damage-type strength, floored at 0.
        let unit = &self.units[u];
        (unit.base_combat + unit.total_damage_type_combat).max(0)
    }

    /// C++ `CvUnit::baseCombatStrDefense` (CvUnit.cpp:8979-8987).
    pub fn unit_base_combat_str_defense(&self, u: UnitId) -> i32 {
        let unit = &self.units[u];
        (unit.base_combat_defense + unit.total_damage_type_combat).max(0)
    }

    /// C++ `CvUnit::isFortifyable` (CvUnit.cpp:10016-10024).
    pub fn unit_is_fortifyable(&self, u: UnitId) -> bool {
        if !self.unit_can_fight(u)
            || self.unit_no_defensive_bonus(u)
            || (self.unit_get_domain_type(u) != DomainType::DOMAIN_LAND
                && self.unit_get_domain_type(u) != DomainType::DOMAIN_IMMOBILE)
        {
            return false;
        }

        true
    }

    /// C++ `CvUnit::fortifyModifier` (CvUnit.cpp:10027-10044).
    pub fn unit_fortify_modifier(&self, u: UnitId) -> i32 {
        if !self.unit_is_fortifyable(u) {
            return 0;
        }

        // FfH: Modified by Kael 10/26/2007 (doubled by the double fortify bonus)
        let mut i = self.units[u].fortify_turns * self.d.defines.fortify_modifier_per_turn;
        if self.unit_is_double_fortify_bonus(u) {
            i *= 2;
        }
        i
    }

    /// C++ `CvUnit::experienceNeeded` (CvUnit.cpp:10047-10065).
    /// C++ asks the `getExperienceNeeded` Python callback (port in `crate::ffh`).
    pub fn unit_experience_needed(&self, u: UnitId) -> i32 {
        // Use python to determine pillage amounts... (sic: CvGameUtils `getExperienceNeeded`)
        let unit = &self.units[u];
        self.py_get_experience_needed(unit.level, unit.owner)
    }

    /// C++ `CvUnit::attackXPValue` (CvUnit.cpp:10068-10081).
    pub fn unit_attack_xp_value(&self, u: UnitId) -> i32 {
        // FfH: Modified by Kael 02/12/2009
        let mut xp = self.d.units[self.units[u].unit_type].xp_value_attack;
        if self.unit_is_animal(u) || self.unit_is_barbarian(u) {
            xp = xp * self.d.defines.barbarian_experience_modifier / 100;
        }
        xp
    }

    /// C++ `CvUnit::defenseXPValue` (CvUnit.cpp:10084-10097).
    pub fn unit_defense_xp_value(&self, u: UnitId) -> i32 {
        // FfH: Modified by Kael 02/12/2009
        let mut xp = self.d.units[self.units[u].unit_type].xp_value_defense;
        if self.unit_is_animal(u) || self.unit_is_barbarian(u) {
            xp = xp * self.d.defines.barbarian_experience_modifier / 100;
        }
        xp
    }

    /// C++ `CvUnit::maxXPValue` (CvUnit.cpp:10100-10129).
    pub fn unit_max_xp_value(&self, u: UnitId) -> i32 {
        let mut max_value = MAX_INT;

        // FfH: Modified by Kael 02/12/2009 (world units have no XP cap)
        if !is_world_unit_class(self.d, self.unit_get_unit_class_type(u)) {
            if self.unit_is_animal(u) {
                max_value = std::cmp::min(max_value, self.d.defines.animal_max_xp_value);
            }
            if self.unit_is_barbarian(u) {
                max_value = std::cmp::min(max_value, self.d.defines.barbarian_max_xp_value);
            }
        }
        // FfH: End Modify

        max_value
    }

    /// C++ `CvUnit::firstStrikes` (CvUnit.cpp:10132-10135).
    pub fn unit_first_strikes(&self, u: UnitId) -> i32 {
        std::cmp::max(0, self.d.units[self.units[u].unit_type].first_strikes + self.units[u].extra_first_strikes)
    }

    /// C++ `CvUnit::chanceFirstStrikes` (CvUnit.cpp:10138-10141).
    pub fn unit_chance_first_strikes(&self, u: UnitId) -> i32 {
        std::cmp::max(
            0,
            self.d.units[self.units[u].unit_type].chance_first_strikes + self.units[u].extra_chance_first_strikes,
        )
    }

    /// C++ `CvUnit::maxFirstStrikes` (CvUnit.cpp:10144-10147).
    pub fn unit_max_first_strikes(&self, u: UnitId) -> i32 {
        self.unit_first_strikes(u) + self.unit_chance_first_strikes(u)
    }

    /// C++ `CvUnit::immuneToFirstStrikes` (CvUnit.cpp:10171-10174).
    pub fn unit_immune_to_first_strikes(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].first_strike_immune || self.units[u].immune_to_first_strikes_count > 0
    }

    /// C++ `CvUnit::noDefensiveBonus` (CvUnit.cpp:10177-10180).
    pub fn unit_no_defensive_bonus(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].no_defensive_bonus
    }

    /// C++ `CvUnit::ignoreBuildingDefense` (CvUnit.cpp:10183-10195).
    /// FfH: the unit info flag or `ignore_building_defense != 0`.
    pub fn unit_ignore_building_defense(&self, u: UnitId) -> bool {
        // FfH: Modifed by Kael 11/17/2007
        if self.d.units[self.units[u].unit_type].ignore_building_defense {
            return true;
        }
        self.units[u].ignore_building_defense != 0
    }

    /// C++ `CvUnit::isNukeImmune` (CvUnit.cpp:10384-10387).
    pub fn unit_is_nuke_immune(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].nuke_immune
    }

    /// C++ `CvUnit::maxInterceptionProbability` (CvUnit.cpp:10390-10393).
    pub fn unit_max_interception_probability(&self, u: UnitId) -> i32 {
        std::cmp::max(0, self.d.units[self.units[u].unit_type].interception_probability + self.units[u].extra_intercept)
    }

    /// C++ `CvUnit::currInterceptionProbability` (CvUnit.cpp:10396-10406).
    pub fn unit_curr_interception_probability(&self, u: UnitId) -> i32 {
        if self.unit_get_domain_type(u) != DomainType::DOMAIN_AIR {
            self.unit_max_interception_probability(u)
        } else {
            (self.unit_max_interception_probability(u) * self.unit_curr_hit_points(u)) / self.unit_max_hit_points(u)
        }
    }

    /// C++ `CvUnit::evasionProbability` (CvUnit.cpp:10409-10412).
    pub fn unit_evasion_probability(&self, u: UnitId) -> i32 {
        std::cmp::max(0, self.d.units[self.units[u].unit_type].evasion_probability + self.units[u].extra_evasion)
    }

    /// C++ `CvUnit::withdrawalProbability` (CvUnit.cpp:10415-10430).
    pub fn unit_withdrawal_probability(&self, u: UnitId) -> i32 {
        if self.unit_get_domain_type(u) == DomainType::DOMAIN_LAND && self.plot_is_water(self.unit_plot(u)) {
            return 0;
        }

        // FfH: Added by Kael 04/06/2009
        if self.unit_get_immobile_timer(u) > 0 {
            return 0;
        }
        // FfH: End Add

        std::cmp::max(0, self.d.units[self.units[u].unit_type].withdrawal_probability + self.units[u].extra_withdrawal)
    }

    /// C++ `CvUnit::collateralDamage` (CvUnit.cpp:10433-10436).
    pub fn unit_collateral_damage(&self, u: UnitId) -> i32 {
        std::cmp::max(0, self.d.units[self.units[u].unit_type].collateral_damage)
    }

    /// C++ `CvUnit::collateralDamageLimit` (CvUnit.cpp:10439-10442).
    pub fn unit_collateral_damage_limit(&self, u: UnitId) -> i32 {
        std::cmp::max(
            0,
            self.d.units[self.units[u].unit_type].collateral_damage_limit * self.d.defines.max_hit_points / 100,
        )
    }

    /// C++ `CvUnit::collateralDamageMaxUnits` (CvUnit.cpp:10445-10448).
    pub fn unit_collateral_damage_max_units(&self, u: UnitId) -> i32 {
        std::cmp::max(0, self.d.units[self.units[u].unit_type].collateral_damage_max_units)
    }

    /// C++ `CvUnit::cityAttackModifier` (CvUnit.cpp:10451-10454).
    pub fn unit_city_attack_modifier(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].city_attack_modifier + self.units[u].extra_city_attack_percent
    }

    /// C++ `CvUnit::cityDefenseModifier` (CvUnit.cpp:10457-10460).
    pub fn unit_city_defense_modifier(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].city_defense_modifier + self.units[u].extra_city_defense_percent
    }

    /// C++ `CvUnit::animalCombatModifier` (CvUnit.cpp:10463-10466).
    pub fn unit_animal_combat_modifier(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].animal_combat_modifier
    }

    /// C++ `CvUnit::hillsAttackModifier` (CvUnit.cpp:10469-10472).
    pub fn unit_hills_attack_modifier(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].hills_attack_modifier + self.units[u].extra_hills_attack_percent
    }

    /// C++ `CvUnit::hillsDefenseModifier` (CvUnit.cpp:10475-10478).
    pub fn unit_hills_defense_modifier(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].hills_defense_modifier + self.units[u].extra_hills_defense_percent
    }

    /// C++ `CvUnit::terrainAttackModifier` (CvUnit.cpp:10481-10486).
    pub fn unit_terrain_attack_modifier(&self, u: UnitId, terrain: TerrainType) -> i32 {
        self.d.units[self.units[u].unit_type].terrain_attack_modifier[terrain]
            + self.units[u].extra_terrain_attack_percent[terrain]
    }

    /// C++ `CvUnit::terrainDefenseModifier` (CvUnit.cpp:10489-10494).
    pub fn unit_terrain_defense_modifier(&self, u: UnitId, terrain: TerrainType) -> i32 {
        self.d.units[self.units[u].unit_type].terrain_defense_modifier[terrain]
            + self.units[u].extra_terrain_defense_percent[terrain]
    }

    /// C++ `CvUnit::featureAttackModifier` (CvUnit.cpp:10497-10502).
    pub fn unit_feature_attack_modifier(&self, u: UnitId, feature: FeatureType) -> i32 {
        self.d.units[self.units[u].unit_type].feature_attack_modifier[feature]
            + self.units[u].extra_feature_attack_percent[feature]
    }

    /// C++ `CvUnit::featureDefenseModifier` (CvUnit.cpp:10504-10509).
    pub fn unit_feature_defense_modifier(&self, u: UnitId, feature: FeatureType) -> i32 {
        self.d.units[self.units[u].unit_type].feature_defense_modifier[feature]
            + self.units[u].extra_feature_defense_percent[feature]
    }

    /// C++ `CvUnit::unitClassAttackModifier` (CvUnit.cpp:10511-10516).
    pub fn unit_unit_class_attack_modifier(&self, u: UnitId, unit_class: UnitClassType) -> i32 {
        self.d.units[self.units[u].unit_type].unit_class_attack_modifier[unit_class]
    }

    /// C++ `CvUnit::unitClassDefenseModifier` (CvUnit.cpp:10519-10524).
    pub fn unit_unit_class_defense_modifier(&self, u: UnitId, unit_class: UnitClassType) -> i32 {
        self.d.units[self.units[u].unit_type].unit_class_defense_modifier[unit_class]
    }

    /// C++ `CvUnit::unitCombatModifier` (CvUnit.cpp:10527-10532).
    pub fn unit_unit_combat_modifier(&self, u: UnitId, unit_combat: UnitCombatType) -> i32 {
        self.d.units[self.units[u].unit_type].unit_combat_modifier[unit_combat]
            + self.units[u].extra_unit_combat_modifier[unit_combat]
    }

    /// C++ `CvUnit::domainModifier` (CvUnit.cpp:10535-10540).
    pub fn unit_domain_modifier(&self, u: UnitId, domain: DomainType) -> i32 {
        self.d.units[self.units[u].unit_type].domain_modifier[domain] + self.units[u].extra_domain_modifier[domain.idx()]
    }

    /// C++ `CvUnit::bombardRate` (CvUnit.cpp:10543-10546).
    pub fn unit_bombard_rate(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].bombard_rate + self.units[u].extra_bombard_rate
    }

    /// C++ `CvUnit::airBombBaseRate` (CvUnit.cpp:10549-10552).
    pub fn unit_air_bomb_base_rate(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].bomb_rate
    }

    /// C++ `CvUnit::airBombCurrRate` (CvUnit.cpp:10555-10558).
    pub fn unit_air_bomb_curr_rate(&self, u: UnitId) -> i32 {
        (self.unit_air_bomb_base_rate(u) * self.unit_curr_hit_points(u)) / self.unit_max_hit_points(u)
    }

    /// C++ `CvUnit::setExperience` (CvUnit.cpp:11788-11800).
    pub fn unit_set_experience(&mut self, u: UnitId, value: i32, max: i32) {
        // `max == -1` means no maximum (C++ default); the info pane update is interface-only.
        let max = if max == -1 { MAX_INT } else { max };
        let unit = &mut self.units[u];
        if unit.experience != value && unit.experience < max {
            unit.experience = max.min(value);
        }
    }

    /// C++ `CvUnit::changeExperience` (CvUnit.cpp:11802-11840).
    /// C++ defaults: `iMax = -1`, `bFromCombat = false`, `bInBorders = false`, `bUpdateGlobal = false`.
    pub fn unit_change_experience(
        &mut self,
        u: UnitId,
        change: i32,
        max: i32,
        from_combat: bool,
        in_borders: bool,
        update_global: bool,
    ) {
        let mut unit_experience = change;

        if from_combat {
            let owner = self.units[u].owner;

            let mut combat_experience_mod = 100 + self.players[owner].great_general_rate_modifier;

            if in_borders {
                let exp_in_border_modifier = self.players[owner].exp_in_border_modifier;
                combat_experience_mod +=
                    self.player_get_domestic_great_general_rate_modifier(owner) + exp_in_border_modifier;
                unit_experience += (change * exp_in_border_modifier) / 100;
            }

            if update_global {
                self.player_change_combat_experience(owner, (change * combat_experience_mod) / 100);
            }

            if self.units[u].experience_percent != 0 {
                unit_experience *= std::cmp::max(0, 100 + self.units[u].experience_percent);
                unit_experience /= 100;
            }

            // FfH: Added by Kael 05/17/2008
            if self.is_option(GameOptionType::GAMEOPTION_SLOWER_XP) {
                unit_experience += 1;
                unit_experience /= 2;
            }
            // FfH: End Add
        }

        let value = self.units[u].experience + unit_experience;
        self.unit_set_experience(u, value, max);
    }

    /// C++ `CvUnit::setLevel` (CvUnit.cpp:11847-11864).
    pub fn unit_set_level(&mut self, u: UnitId, value: i32) {
        if self.units[u].level != value {
            self.units[u].level = value;

            let owner = self.units[u].owner;
            let level = self.units[u].level;
            if level > self.players[owner].highest_unit_level {
                // `setHighestUnitLevel` (trivial setter).
                self.players[owner].highest_unit_level = level;
            }

            // UI: info pane dirty bit for the selected unit.
        }
    }

    /// C++ `CvUnit::changeLevel` (CvUnit.cpp:11866-11869).
    pub fn unit_change_level(&mut self, u: UnitId, change: i32) {
        let value = self.units[u].level + change;
        self.unit_set_level(u, value);
    }

    /// C++ `CvUnit::setFortifyTurns` (CvUnit.cpp:11971-11980).
    pub fn unit_set_fortify_turns(&mut self, u: UnitId, value: i32) {
        // C++ `range(iNewValue, 0, MAX_FORTIFY_TURNS)`; the info bar update is interface-only.
        let max = self.d.defines.max_fortify_turns;
        let value = if value < 0 {
            0
        } else if value > max {
            max
        } else {
            value
        };
        self.units[u].fortify_turns = value;
    }

    /// C++ `CvUnit::changeFortifyTurns` (CvUnit.cpp:11982-11985).
    pub fn unit_change_fortify_turns(&mut self, u: UnitId, change: i32) {
        let value = self.units[u].fortify_turns + change;
        self.unit_set_fortify_turns(u, value);
    }

    /// C++ `CvUnit::isBlitz` (CvUnit.cpp:11992-11995).
    pub fn unit_is_blitz(&self, u: UnitId) -> bool {
        self.units[u].blitz_count > 0
    }

    /// C++ `CvUnit::changeBlitzCount` (CvUnit.cpp:11997-12001).
    pub fn unit_change_blitz_count(&mut self, u: UnitId, change: i32) {
        self.units[u].blitz_count += change;
    }

    /// C++ `CvUnit::isAmphib` (CvUnit.cpp:12008-12011).
    pub fn unit_is_amphib(&self, u: UnitId) -> bool {
        self.units[u].amphib_count > 0
    }

    /// C++ `CvUnit::changeAmphibCount` (CvUnit.cpp:12013-12017).
    pub fn unit_change_amphib_count(&mut self, u: UnitId, change: i32) {
        self.units[u].amphib_count += change;
    }

    /// C++ `CvUnit::isRiver` (CvUnit.cpp:12024-12027).
    pub fn unit_is_river(&self, u: UnitId) -> bool {
        self.units[u].river_count > 0
    }

    /// C++ `CvUnit::changeRiverCount` (CvUnit.cpp:12029-12033).
    pub fn unit_change_river_count(&mut self, u: UnitId, change: i32) {
        self.units[u].river_count += change;
    }

    /// C++ `CvUnit::isEnemyRoute` (CvUnit.cpp:12040-12043).
    pub fn unit_is_enemy_route(&self, u: UnitId) -> bool {
        self.units[u].enemy_route_count > 0
    }

    /// C++ `CvUnit::changeEnemyRouteCount` (CvUnit.cpp:12045-12049).
    pub fn unit_change_enemy_route_count(&mut self, u: UnitId, change: i32) {
        self.units[u].enemy_route_count += change;
    }

    /// C++ `CvUnit::isAlwaysHeal` (CvUnit.cpp:12056-12059).
    pub fn unit_is_always_heal(&self, u: UnitId) -> bool {
        self.units[u].always_heal_count > 0
    }

    /// C++ `CvUnit::changeAlwaysHealCount` (CvUnit.cpp:12061-12065).
    pub fn unit_change_always_heal_count(&mut self, u: UnitId, change: i32) {
        self.units[u].always_heal_count += change;
    }

    /// C++ `CvUnit::isHillsDoubleMove` (CvUnit.cpp:12072-12075).
    pub fn unit_is_hills_double_move(&self, u: UnitId) -> bool {
        self.units[u].hills_double_move_count > 0
    }

    /// C++ `CvUnit::changeHillsDoubleMoveCount` (CvUnit.cpp:12077-12081).
    pub fn unit_change_hills_double_move_count(&mut self, u: UnitId, change: i32) {
        self.units[u].hills_double_move_count += change;
    }

    /// C++ `CvUnit::changeImmuneToFirstStrikesCount` (CvUnit.cpp:12088-12092).
    pub fn unit_change_immune_to_first_strikes_count(&mut self, u: UnitId, change: i32) {
        self.units[u].immune_to_first_strikes_count += change;
    }

    /// C++ `CvUnit::changeExtraMoves` (CvUnit.cpp:12119-12123).
    /// Can become negative (Sephi 0.41k: mutations).
    pub fn unit_change_extra_moves(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_moves += change;
    }

    /// C++ `CvUnit::changeExtraMoveDiscount` (CvUnit.cpp:12132-12136).
    pub fn unit_change_extra_move_discount(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_move_discount += change;
    }

    /// C++ `CvUnit::changeExtraAirRange` (CvUnit.cpp:12143-12146).
    pub fn unit_change_extra_air_range(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_air_range += change;
    }

    /// C++ `CvUnit::changeExtraIntercept` (CvUnit.cpp:12153-12156).
    pub fn unit_change_extra_intercept(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_intercept += change;
    }

    /// C++ `CvUnit::changeExtraEvasion` (CvUnit.cpp:12163-12166).
    pub fn unit_change_extra_evasion(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_evasion += change;
    }

    /// C++ `CvUnit::changeExtraFirstStrikes` (CvUnit.cpp:12173-12177).
    pub fn unit_change_extra_first_strikes(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_first_strikes += change;
    }

    /// C++ `CvUnit::changeExtraChanceFirstStrikes` (CvUnit.cpp:12184-12188).
    pub fn unit_change_extra_chance_first_strikes(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_chance_first_strikes += change;
    }

    /// C++ `CvUnit::changeExtraWithdrawal` (CvUnit.cpp:12197-12201).
    pub fn unit_change_extra_withdrawal(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_withdrawal += change;
    }

    /// C++ `CvUnit::changeExtraCollateralDamage` (CvUnit.cpp:12208-12212).
    pub fn unit_change_extra_collateral_damage(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_collateral_damage += change;
    }

    /// C++ `CvUnit::changeExtraBombardRate` (CvUnit.cpp:12219-12223).
    pub fn unit_change_extra_bombard_rate(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_bombard_rate += change;
    }

    /// C++ `CvUnit::changeExtraEnemyHeal` (CvUnit.cpp:12230-12234).
    pub fn unit_change_extra_enemy_heal(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_enemy_heal += change;
    }

    /// C++ `CvUnit::changeExtraNeutralHeal` (CvUnit.cpp:12241-12245).
    pub fn unit_change_extra_neutral_heal(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_neutral_heal += change;
    }

    /// C++ `CvUnit::changeExtraFriendlyHeal` (CvUnit.cpp:12253-12257).
    pub fn unit_change_extra_friendly_heal(&mut self, u: UnitId, change: i32) {
        self.units[u].extra_friendly_heal += change;
    }

    /// C++ `CvUnit::changeSameTileHeal` (CvUnit.cpp:12264-12268).
    pub fn unit_change_same_tile_heal(&mut self, u: UnitId, change: i32) {
        self.units[u].same_tile_heal += change;
    }

    /// C++ `CvUnit::changeAdjacentTileHeal` (CvUnit.cpp:12275-12279).
    pub fn unit_change_adjacent_tile_heal(&mut self, u: UnitId, change: i32) {
        self.units[u].adjacent_tile_heal += change;
    }

    /// C++ `CvUnit::getExtraCombatPercent` (CvUnit.cpp:12281-12294).
    /// FfH: `extra_combat_percent` plus `combat_percent_in_borders` inside own borders.
    pub fn unit_get_extra_combat_percent(&self, u: UnitId) -> i32 {
        // FfH: Modified by Kael 10/26/2007
        let unit = &self.units[u];
        let mut i = unit.extra_combat_percent;
        if self.map.plots[self.unit_plot(u)].owner == Some(unit.owner) {
            i += unit.combat_percent_in_borders;
        }
        i
    }

    /// C++ `CvUnit::changeExtraCombatPercent` (CvUnit.cpp:12296-12304).
    pub fn unit_change_extra_combat_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].extra_combat_percent += change;
        }
    }

    /// C++ `CvUnit::changeExtraCityAttackPercent` (CvUnit.cpp:12311-12319).
    pub fn unit_change_extra_city_attack_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].extra_city_attack_percent += change;
        }
    }

    /// C++ `CvUnit::changeExtraCityDefensePercent` (CvUnit.cpp:12326-12334).
    pub fn unit_change_extra_city_defense_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].extra_city_defense_percent += change;
        }
    }

    /// C++ `CvUnit::changeExtraHillsAttackPercent` (CvUnit.cpp:12341-12349).
    pub fn unit_change_extra_hills_attack_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].extra_hills_attack_percent += change;
        }
    }

    /// C++ `CvUnit::changeExtraHillsDefensePercent` (CvUnit.cpp:12356-12364).
    pub fn unit_change_extra_hills_defense_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].extra_hills_defense_percent += change;
        }
    }

    /// C++ `CvUnit::changeRevoltProtection` (CvUnit.cpp:12371-12379).
    pub fn unit_change_revolt_protection(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].revolt_protection += change;
        }
    }

    /// C++ `CvUnit::changeCollateralDamageProtection` (CvUnit.cpp:12386-12394).
    pub fn unit_change_collateral_damage_protection(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].collateral_damage_protection += change;
        }
    }

    /// C++ `CvUnit::changePillageChange` (CvUnit.cpp:12401-12409).
    pub fn unit_change_pillage_change(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].pillage_change += change;
        }
    }

    /// C++ `CvUnit::changeUpgradeDiscount` (CvUnit.cpp:12416-12424).
    pub fn unit_change_upgrade_discount(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].upgrade_discount += change;
        }
    }

    /// C++ `CvUnit::changeExperiencePercent` (CvUnit.cpp:12431-12439).
    pub fn unit_change_experience_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].experience_percent += change;
        }
    }

    /// C++ `CvUnit::changeKamikazePercent` (CvUnit.cpp:12446-12454).
    pub fn unit_change_kamikaze_percent(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].kamikaze_percent += change;
        }
    }

    /// C++ `CvUnit::setPromotionReady` (CvUnit.cpp:12591-12621).
    /// On becoming ready (unless UNITAI_TERRAFORMER) the group stops automation, clears its
    /// missions and wakes up. The promotion glow is graphics.
    pub fn unit_set_promotion_ready(&mut self, u: UnitId, value: bool) {
        if self.units[u].promotion_ready != value {
            self.units[u].promotion_ready = value;

            // ADDON (automatic terraforming) Sephi: terraformers keep working.
            if self.units[u].promotion_ready && self.units[u].ai_type != Some(UnitAIType::UNITAI_TERRAFORMER) {
                // C++ dereferences `getGroup()` for each call (a unit on the map always has a
                // group, and none of these calls moves the unit to another group).
                let group = unit_group_of(self, u).expect("CvUnit::setPromotionReady: unit has no group");
                self.group_set_automate_type(group, None);
                self.group_clear_mission_queue(group);
                self.group_set_activity_type(group, ActivityType::ACTIVITY_AWAKE);
            }

            // Graphics: `showPromotionGlow`; UI: selection buttons dirty bit.
        }
    }

    /// C++ `CvUnit::testPromotionReady` (CvUnit.cpp:12624-12627).
    pub fn unit_test_promotion_ready(&mut self, u: UnitId) {
        let ready = self.units[u].experience >= self.unit_experience_needed(u) && self.unit_can_acquire_promotion_any(u);
        self.unit_set_promotion_ready(u, ready);
    }

    /// C++ `CvUnit::getUnitClassType` (CvUnit.cpp:12813-12816).
    pub fn unit_get_unit_class_type(&self, u: UnitId) -> UnitClassType {
        info_unit_class(&self.d.units[self.units[u].unit_type])
    }

    /// C++ `CvUnit::changeExtraDomainModifier` (CvUnit.cpp:12989-12994).
    pub fn unit_change_extra_domain_modifier(&mut self, u: UnitId, domain: DomainType, change: i32) {
        self.units[u].extra_domain_modifier[domain.idx()] += change;
    }

    /// C++ `CvUnit::changeExtraTerrainAttackPercent` (CvUnit.cpp:13114-13125).
    pub fn unit_change_extra_terrain_attack_percent(&mut self, u: UnitId, terrain: TerrainType, change: i32) {
        if change != 0 {
            self.units[u].extra_terrain_attack_percent[terrain] += change;
        }
    }

    /// C++ `CvUnit::changeExtraTerrainDefensePercent` (CvUnit.cpp:13135-13146).
    pub fn unit_change_extra_terrain_defense_percent(&mut self, u: UnitId, terrain: TerrainType, change: i32) {
        if change != 0 {
            self.units[u].extra_terrain_defense_percent[terrain] += change;
        }
    }

    /// C++ `CvUnit::changeExtraFeatureAttackPercent` (CvUnit.cpp:13156-13167).
    pub fn unit_change_extra_feature_attack_percent(&mut self, u: UnitId, feature: FeatureType, change: i32) {
        if change != 0 {
            self.units[u].extra_feature_attack_percent[feature] += change;
        }
    }

    /// C++ `CvUnit::changeExtraFeatureDefensePercent` (CvUnit.cpp:13177-13188).
    pub fn unit_change_extra_feature_defense_percent(&mut self, u: UnitId, feature: FeatureType, change: i32) {
        if change != 0 {
            self.units[u].extra_feature_defense_percent[feature] += change;
        }
    }

    /// C++ `CvUnit::changeExtraUnitCombatModifier` (CvUnit.cpp:13198-13203).
    pub fn unit_change_extra_unit_combat_modifier(&mut self, u: UnitId, unit_combat: UnitCombatType, change: i32) {
        self.units[u].extra_unit_combat_modifier[unit_combat] += change;
    }

    /// C++ `CvUnit::canAcquirePromotion` (CvUnit.cpp:13206-13347).
    pub fn unit_can_acquire_promotion(&self, u: UnitId, promotion: PromotionType) -> bool {
        let d = self.d;
        let unit = &self.units[u];
        let info = &d.promotions[promotion];

        if unit.has_promotion[promotion] {
            return false;
        }

        if let Some(prereq) = info.prereq_promotion {
            if !unit.has_promotion[prereq] {
                return false;
            }
        }

        // FfH: Modified by Kael 07/30/2007 (replaces the BTS PrereqOrPromotion1/2 test)
        if info.min_level == -1 {
            return false;
        }
        if info.race {
            return false;
        }
        if info.equipment {
            return false;
        }
        if info.min_level > unit.level {
            return false;
        }
        if let Some(prereq_and) = info.promotion_prereq_and {
            if !unit.has_promotion[prereq_and] {
                return false;
            }
        }
        if let Some(prereq_or1) = info.prereq_or_promotion1 {
            let mut valid = false;
            if unit.has_promotion[prereq_or1] {
                valid = true;
            }
            if let Some(prereq_or2) = info.prereq_or_promotion2 {
                if unit.has_promotion[prereq_or2] {
                    valid = true;
                }
            }
            if let Some(prereq_or3) = info.promotion_prereq_or3 {
                if unit.has_promotion[prereq_or3] {
                    valid = true;
                }
            }
            if let Some(prereq_or4) = info.promotion_prereq_or4 {
                if unit.has_promotion[prereq_or4] {
                    valid = true;
                }
            }
            if !valid {
                return false;
            }
        }
        if let Some(bonus) = info.bonus_prereq {
            if !self.player_has_bonus(unit.owner, bonus) {
                return false;
            }
        }
        for other in d.promotions.keys() {
            if unit.has_promotion[other] {
                let other_info = &d.promotions[other];
                if other_info.promotion_immune1 == Some(promotion) {
                    return false;
                }
                if other_info.promotion_immune2 == Some(promotion) {
                    return false;
                }
                if other_info.promotion_immune3 == Some(promotion) {
                    return false;
                }
            }
        }
        if info.prereq_alive {
            if !self.unit_is_alive(u) {
                return false;
            }
        }
        // FfH: End Add

        if let Some(tech) = info.tech_prereq {
            if !self.team_is_has_tech(self.unit_get_team(u), Some(tech)) {
                return false;
            }
        }

        if let Some(religion) = info.state_religion_prereq {
            if self.player_get_state_religion(unit.owner) != Some(religion) {
                return false;
            }
        }

        if !self.unit_is_promotion_valid(u, promotion) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::isPromotionValid` (CvUnit.cpp:13349-13383).
    /// Starts with `::isPromotionValid(ePromotion, getUnitType(), true)` (CvGameCoreUtils).
    pub fn unit_is_promotion_valid(&self, u: UnitId, promotion: PromotionType) -> bool {
        let d = self.d;
        let unit_type = self.units[u].unit_type;

        if !is_promotion_valid(d, promotion, unit_type, true) {
            return false;
        }

        let promotion_info = &d.promotions[promotion];

        // FfH: Modified by Kael 10/28/2008 (only promotions that raise withdrawal are capped)
        if promotion_info.withdrawal_change > 0 {
            if promotion_info.withdrawal_change + d.units[unit_type].withdrawal_probability + self.units[u].extra_withdrawal
                > d.defines.max_withdrawal_probability
            {
                return false;
            }
        }
        // FfH: End Modify

        if promotion_info.intercept_change + self.unit_max_interception_probability(u)
            > d.defines.max_interception_probability
        {
            return false;
        }

        if promotion_info.evasion_change + self.unit_evasion_probability(u) > d.defines.max_evasion_probability {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canAcquirePromotionAny` (CvUnit.cpp:13386-13399).
    pub fn unit_can_acquire_promotion_any(&self, u: UnitId) -> bool {
        for promotion in self.d.promotions.keys() {
            if self.unit_can_acquire_promotion(u, promotion) {
                return true;
            }
        }

        false
    }

    /// C++ `CvUnit::setHasPromotion` (CvUnit.cpp:13410-13620).
    /// Applies/removes every promotion effect through the `unit_change_*` methods (the FfH
    /// part includes race, weapons art style/group size (graphics), terraformer update).
    pub fn unit_set_has_promotion(&mut self, u: UnitId, promotion: PromotionType, value: bool) {
        let d = self.d;

        // FfH: Added by Kael 07/28/2008 (a promotion the unit is immune to is never gained)
        if value {
            for other in d.promotions.keys() {
                if self.units[u].has_promotion[other] {
                    let other_info = &d.promotions[other];
                    if other_info.promotion_immune1 == Some(promotion) {
                        return;
                    }
                    if other_info.promotion_immune2 == Some(promotion) {
                        return;
                    }
                    if other_info.promotion_immune3 == Some(promotion) {
                        return;
                    }
                }
            }
        }
        // FfH: End Add

        if self.units[u].has_promotion[promotion] != value {
            self.units[u].has_promotion[promotion] = value;

            let change = if self.units[u].has_promotion[promotion] { 1 } else { -1 };
            let info = &d.promotions[promotion];
            let flag = |b: bool| if b { change } else { 0 };

            self.unit_change_blitz_count(u, flag(info.blitz));
            self.unit_change_amphib_count(u, flag(info.amphib));
            self.unit_change_river_count(u, flag(info.river));
            self.unit_change_enemy_route_count(u, flag(info.enemy_route));
            self.unit_change_always_heal_count(u, flag(info.always_heal));
            self.unit_change_hills_double_move_count(u, flag(info.hills_double_move));
            self.unit_change_immune_to_first_strikes_count(u, flag(info.immune_to_first_strikes));

            self.unit_change_extra_visibility_range(u, info.visibility_change * change);
            self.unit_change_extra_moves(u, info.moves_change * change);
            self.unit_change_extra_move_discount(u, info.move_discount_change * change);
            self.unit_change_extra_air_range(u, info.air_range_change * change);
            self.unit_change_extra_intercept(u, info.intercept_change * change);
            self.unit_change_extra_evasion(u, info.evasion_change * change);
            self.unit_change_extra_first_strikes(u, info.first_strikes_change * change);
            self.unit_change_extra_chance_first_strikes(u, info.chance_first_strikes_change * change);
            self.unit_change_extra_withdrawal(u, info.withdrawal_change * change);
            self.unit_change_extra_collateral_damage(u, info.collateral_damage_change * change);
            self.unit_change_extra_bombard_rate(u, info.bombard_rate_change * change);
            self.unit_change_extra_enemy_heal(u, info.enemy_heal_change * change);
            self.unit_change_extra_neutral_heal(u, info.neutral_heal_change * change);
            self.unit_change_extra_friendly_heal(u, info.friendly_heal_change * change);
            self.unit_change_same_tile_heal(u, info.same_tile_heal_change * change);
            self.unit_change_adjacent_tile_heal(u, info.adjacent_tile_heal_change * change);
            self.unit_change_extra_combat_percent(u, info.combat_percent * change);
            self.unit_change_extra_city_attack_percent(u, info.city_attack_percent * change);
            self.unit_change_extra_city_defense_percent(u, info.city_defense_percent * change);
            self.unit_change_extra_hills_attack_percent(u, info.hills_attack_percent * change);
            self.unit_change_extra_hills_defense_percent(u, info.hills_defense_percent * change);
            self.unit_change_revolt_protection(u, info.revolt_protection * change);
            self.unit_change_collateral_damage_protection(u, info.collateral_damage_protection * change);
            self.unit_change_pillage_change(u, info.pillage_change * change);
            self.unit_change_upgrade_discount(u, info.upgrade_discount * change);
            self.unit_change_experience_percent(u, info.experience_percent * change);
            self.unit_change_kamikaze_percent(u, info.kamikaze_percent * change);
            self.unit_change_cargo_space(u, info.cargo_change * change);

            // FfH: Added by Kael 07/30/2007
            if info.ai_control && value {
                self.unit_join_group(u, None, true);
            }
            self.unit_change_ai_control(u, flag(info.ai_control));
            self.unit_change_alive(u, flag(info.not_alive));
            self.unit_change_base_combat_str(u, info.extra_combat_str * change);
            self.unit_change_base_combat_str_defense(u, info.extra_combat_defense * change);
            self.unit_change_better_defender_than_percent(u, info.better_defender_than_percent * change);
            self.unit_change_boarding(u, flag(info.boarding));
            self.unit_change_combat_heal_percent(u, info.combat_heal_percent * change);
            self.unit_change_combat_percent_in_borders(u, info.combat_percent_in_borders * change);
            self.unit_change_combat_percent_global_counter(u, info.combat_percent_global_counter * change);
            self.unit_change_defensive_strike_chance(u, info.defensive_strike_chance * change);
            self.unit_change_defensive_strike_damage(u, info.defensive_strike_damage * change);
            self.unit_change_double_fortify_bonus(u, flag(info.double_fortify_bonus));
            self.unit_change_fear(u, flag(info.fear));
            self.unit_change_flying(u, flag(info.flying));
            self.unit_change_gold_from_combat(u, info.gold_from_combat * change);
            self.unit_change_held(u, flag(info.held));
            self.unit_change_hidden_nationality(u, flag(info.hidden_nationality));
            self.unit_change_ignore_building_defense(u, flag(info.ignore_building_defense));
            self.unit_change_immortal(u, flag(info.immortal));
            self.unit_change_immune_to_capture(u, flag(info.immune_to_capture));
            self.unit_change_immune_to_defensive_strike(u, flag(info.immune_to_defensive_strike));
            self.unit_change_immune_to_fear(u, flag(info.immune_to_fear));
            self.unit_change_immune_to_magic(u, flag(info.immune_to_magic));
            self.unit_change_invisible_from_promotion(u, flag(info.invisible));
            // C++ keeps using `this`: losing invisibility next to a visible enemy withdraws the
            // unit and kills it (`kill(false)`) when there is no valid plot, a use-after-free there.
            if !self.units.contains(u) {
                return;
            }
            self.unit_change_only_defensive(u, flag(info.only_defensive));
            self.unit_change_resist(u, info.resist_magic * change);
            self.unit_change_resist_modify(u, info.caster_resist_modify * change);
            self.unit_change_see_invisible(u, flag(info.see_invisible));
            self.unit_change_spell_caster_xp(u, info.spell_caster_xp * change);
            self.unit_change_spell_damage_modify(u, info.spell_damage_modify * change);
            self.unit_change_target_weakest_unit(u, flag(info.target_weakest_unit));
            self.unit_change_target_weakest_unit_counter(u, flag(info.target_weakest_unit_counter));
            self.unit_change_twincast(u, flag(info.twincast));
            self.unit_change_water_walking(u, flag(info.water_walking));
            self.unit_change_work_rate_modify(u, info.work_rate_modify * change);
            self.change_global_counter(info.modify_global_counter * change);
            if info.combat_limit != 0 {
                self.unit_calc_combat_limit(u);
            }
            if promotion.i() == d.defines.mutated_promotion && value {
                self.unit_mutate(u);
                // C++ keeps using `this` (a mutation's effects could kill the unit, see above).
                if !self.units.contains(u) {
                    return;
                }
            }
            if info.race {
                if value {
                    self.unit_set_race(u, Some(promotion));
                } else {
                    self.unit_set_race(u, None);
                }
                // C++ keeps using `this` (`setRace` removes the old race promotion, see above).
                if !self.units.contains(u) {
                    return;
                }
            }
            for damage_type in d.damages.keys() {
                self.unit_change_damage_type_combat(u, damage_type, info.damage_type_combat[damage_type] * change);
            }
            for bonus in d.bonuses.keys() {
                self.unit_change_bonus_affinity(u, bonus, info.bonus_affinity[bonus] * change);
            }
            for damage_type in d.damages.keys() {
                self.unit_change_damage_type_resist(u, damage_type, info.damage_type_resist[damage_type] * change);
            }
            // FfH: End Add
            for terrain in d.terrains.keys() {
                self.unit_change_extra_terrain_attack_percent(u, terrain, info.terrain_attack_percent[terrain] * change);
                self.unit_change_extra_terrain_defense_percent(u, terrain, info.terrain_defense_percent[terrain] * change);
                self.unit_change_terrain_double_move_count(u, terrain, flag(info.terrain_double_move[terrain]));
            }

            for feature in d.features.keys() {
                self.unit_change_extra_feature_attack_percent(u, feature, info.feature_attack_percent[feature] * change);
                self.unit_change_extra_feature_defense_percent(u, feature, info.feature_defense_percent[feature] * change);
                self.unit_change_feature_double_move_count(u, feature, flag(info.feature_double_move[feature]));
            }

            for unit_combat in d.unit_combats.keys() {
                self.unit_change_extra_unit_combat_modifier(
                    u,
                    unit_combat,
                    info.unit_combat_modifier_percent[unit_combat] * change,
                );
            }

            for i in 0..DomainType::NUM {
                let domain = DomainType::from_usize(i);
                self.unit_change_extra_domain_modifier(u, domain, info.domain_modifier_percent[domain] * change);
            }

            // UI: selection buttons / info pane dirty bits. Graphics: `updatePromotionLayers`.

            // FfH: Added by Kael 07/04/2009
            // Graphics: the promotion's unit art style (`setUnitArtStyleType`) and group size
            // (`setGroupSize`, the number of figures drawn) with `reloadEntity()`; neither is read
            // by rules code (PromotionInfo does not load them).
            self.unit_update_terraformer(u);
            // FfH: End Add
        }
    }

    /// C++ `CvUnit::isSuicide` (CvUnit.cpp:18041-18044).
    pub fn unit_is_suicide(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].suicide || self.units[u].kamikaze_percent != 0
    }

    /// C++ `CvUnit::getDropRange` (CvUnit.cpp:18046-18049).
    pub fn unit_get_drop_range(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].drop_range
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;
    use crate::unit::{Unit, UnitResetInfo};

    const W: i32 = 6;
    const H: i32 = 4;

    /// A game on a 6x4 land map (one area) with real `Player` objects (not `player_init`ed);
    /// every civ slot plays the Bannor (default units for the warrior/axeman/champion classes).
    fn test_game() -> Game {
        let d = ffh_data::global();
        let mut setup = GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 7);
        let bannor = d.t::<CivilizationType>("CIVILIZATION_BANNOR");
        for slot in setup.players.iter_mut() {
            slot.civ = Some(bannor);
        }
        let mut g = Game::new(d, setup);
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        g.map = Map::new(d, &MapInitData { grid_w: W, grid_h: H, ..Default::default() }, num_players, num_teams);
        let area = g.map_add_area(false);
        for p in g.map.plot_ids() {
            g.map.plots[p].area = Some(area);
            g.map.plots[p].plot_type = PlotType::PLOT_LAND;
        }
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        g
    }

    /// C++ `CvPlayer::addUnit` + `reset`, with the coordinates set directly (no `init`/`setXY`).
    fn add_unit(g: &mut Game, owner: PlayerId, unit_type: &str, x: i32, y: i32) -> UnitId {
        let d = g.d;
        let t = d.t::<UnitType>(unit_type);
        let id = g.units.next_id();
        let u = g.player_add_unit(owner, Unit::new(id, t, owner, d, &UnitResetInfo::from_info(&d.units[t])));
        g.units[u].x = x;
        g.units[u].y = y;
        u
    }

    #[test]
    fn unit_info_queries() {
        let mut g = test_game();
        let d = g.d;
        let archer = add_unit(&mut g, PlayerId(0), "UNIT_ARCHER", 1, 1);
        let info = &d.units[g.units[archer].unit_type];

        assert_eq!(g.unit_get_domain_type(archer), DomainType::DOMAIN_LAND);
        assert_eq!(g.unit_get_unit_class_type(archer), d.t::<UnitClassType>("UNITCLASS_ARCHER"));
        assert_eq!(g.unit_get_unit_combat_type(archer), info.unit_combat_type);
        assert_eq!(g.unit_get_civilization_type(archer), d.t::<CivilizationType>("CIVILIZATION_BANNOR"));
        assert!(!g.unit_is_human(archer) && !g.unit_is_barbarian(archer));

        // First strikes: info + promotions, floored at 0.
        assert_eq!(g.unit_first_strikes(archer), info.first_strikes);
        g.units[archer].extra_first_strikes = 2;
        g.units[archer].extra_chance_first_strikes = 1;
        assert_eq!(g.unit_first_strikes(archer), info.first_strikes + 2);
        assert_eq!(g.unit_max_first_strikes(archer), info.first_strikes + 2 + info.chance_first_strikes + 1);
        g.units[archer].extra_first_strikes = -10;
        assert_eq!(g.unit_first_strikes(archer), 0);
        assert!(!g.unit_immune_to_first_strikes(archer));
        g.units[archer].immune_to_first_strikes_count = 1;
        assert!(g.unit_immune_to_first_strikes(archer));

        // Combat modifiers add the promotion counters to the unit info values.
        g.units[archer].extra_city_defense_percent = 10;
        assert_eq!(g.unit_city_defense_modifier(archer), info.city_defense_modifier + 10);
        g.units[archer].extra_hills_defense_percent = 5;
        assert_eq!(g.unit_hills_defense_modifier(archer), info.hills_defense_modifier + 5);
        let terrain = TerrainType(0);
        g.units[archer].extra_terrain_attack_percent[terrain] = 15;
        assert_eq!(g.unit_terrain_attack_modifier(archer, terrain), info.terrain_attack_modifier[terrain] + 15);
        g.unit_change_extra_domain_modifier(archer, DomainType::DOMAIN_SEA, 20);
        assert_eq!(
            g.unit_domain_modifier(archer, DomainType::DOMAIN_SEA),
            info.domain_modifier[DomainType::DOMAIN_SEA] + 20
        );

        // FfH counters that turn unit info flags on.
        assert!(!g.unit_is_only_defensive(archer));
        g.units[archer].only_defensive = 1;
        assert!(g.unit_is_only_defensive(archer));
        assert!(!g.unit_ignore_building_defense(archer));
        g.units[archer].ignore_building_defense = 1;
        assert!(g.unit_ignore_building_defense(archer));
        assert!(!g.unit_is_suicide(archer));
        g.units[archer].kamikaze_percent = 50;
        assert!(g.unit_is_suicide(archer));
        assert!(!g.unit_is_no_capture(archer) || info.no_capture);
        g.units[archer].hidden_nationality = 1;
        assert!(g.unit_is_no_capture(archer));

        // Delayed death disables golden ages.
        let prophet = d.units.keys().find(|&t| d.units[t].golden_age).unwrap();
        let name = d.type_name(prophet).to_string();
        let gp = add_unit(&mut g, PlayerId(0), &name, 2, 2);
        assert!(g.unit_is_golden_age(gp));
        g.units[gp].death_delay = true;
        assert!(!g.unit_is_golden_age(gp));
    }

    #[test]
    fn interception_collateral_and_bombs() {
        let mut g = test_game();
        let d = g.d;

        // Air units intercept and bomb in proportion to their hit points.
        let hawk = add_unit(&mut g, PlayerId(0), "UNIT_HAWK", 1, 1);
        let info = &d.units[g.units[hawk].unit_type];
        assert_eq!(g.unit_get_domain_type(hawk), DomainType::DOMAIN_AIR);
        g.units[hawk].extra_intercept = 50;
        g.units[hawk].damage = 40;
        let max = info.interception_probability + 50;
        assert_eq!(g.unit_max_interception_probability(hawk), max);
        assert_eq!(g.unit_curr_interception_probability(hawk), max * 60 / 100);
        assert_eq!(g.unit_air_bomb_curr_rate(hawk), info.bomb_rate * 60 / 100);

        // Other domains intercept at full strength; probabilities are floored at 0.
        let warrior = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);
        g.units[warrior].extra_intercept = 30;
        g.units[warrior].damage = 50;
        assert_eq!(g.unit_curr_interception_probability(warrior), g.unit_max_interception_probability(warrior));
        g.units[warrior].extra_evasion = -100;
        assert_eq!(g.unit_evasion_probability(warrior), 0);

        let catapult = add_unit(&mut g, PlayerId(0), "UNIT_CATAPULT", 1, 1);
        let info = &d.units[g.units[catapult].unit_type];
        assert!(info.collateral_damage > 0);
        assert_eq!(g.unit_collateral_damage(catapult), info.collateral_damage);
        assert_eq!(
            g.unit_collateral_damage_limit(catapult),
            info.collateral_damage_limit * d.defines.max_hit_points / 100
        );
        assert_eq!(g.unit_collateral_damage_max_units(catapult), info.collateral_damage_max_units);
        g.units[catapult].extra_bombard_rate = 5;
        assert_eq!(g.unit_bombard_rate(catapult), info.bombard_rate + 5);
    }

    #[test]
    fn withdrawal_and_fortification() {
        let mut g = test_game();
        let d = g.d;
        let catapult = add_unit(&mut g, PlayerId(0), "UNIT_CATAPULT", 2, 2);
        let base = d.units[g.units[catapult].unit_type].withdrawal_probability;
        assert!(base > 0);
        g.units[catapult].extra_withdrawal = 5;
        assert_eq!(g.unit_withdrawal_probability(catapult), base + 5);

        // Immobile (or held) units cannot withdraw.
        g.units[catapult].immobile_timer = 1;
        assert_eq!(g.unit_withdrawal_probability(catapult), 0);
        g.units[catapult].immobile_timer = 0;
        g.units[catapult].held = 1;
        assert_eq!(g.unit_withdrawal_probability(catapult), 0);
        g.units[catapult].held = 0;

        // Land units on water never withdraw.
        let plot = g.unit_plot(catapult);
        g.map.plots[plot].plot_type = PlotType::PLOT_OCEAN;
        assert_eq!(g.unit_withdrawal_probability(catapult), 0);

        // Fortification: land units that can fight and get defensive bonuses.
        let warrior = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);
        g.unit_set_fortify_turns(warrior, 3);
        assert!(g.unit_is_fortifyable(warrior));
        assert_eq!(g.unit_fortify_modifier(warrior), 3 * d.defines.fortify_modifier_per_turn);
        g.units[warrior].double_fortify_bonus = 1;
        assert_eq!(g.unit_fortify_modifier(warrior), 6 * d.defines.fortify_modifier_per_turn);

        let trireme = add_unit(&mut g, PlayerId(0), "UNIT_TRIREME", 0, 0);
        g.units[trireme].fortify_turns = 3;
        assert!(!g.unit_is_fortifyable(trireme));
        assert_eq!(g.unit_fortify_modifier(trireme), 0);

        let no_bonus = d
            .units
            .keys()
            .find(|&t| {
                d.units[t].no_defensive_bonus && d.units[t].domain_type == Some(DomainType::DOMAIN_LAND) && d.units[t].combat > 0
            })
            .unwrap();
        let name = d.type_name(no_bonus).to_string();
        let rider = add_unit(&mut g, PlayerId(0), &name, 1, 2);
        g.units[rider].fortify_turns = 3;
        assert!(g.unit_no_defensive_bonus(rider));
        assert_eq!(g.unit_fortify_modifier(rider), 0);
    }

    #[test]
    fn extra_combat_percent_in_own_borders() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 3, 1);
        g.unit_change_extra_combat_percent(u, 10);
        g.units[u].combat_percent_in_borders = 25;
        let plot = g.unit_plot(u);

        assert_eq!(g.unit_get_extra_combat_percent(u), 10);
        g.map.plots[plot].owner = Some(PlayerId(1));
        assert_eq!(g.unit_get_extra_combat_percent(u), 10);
        g.map.plots[plot].owner = Some(PlayerId(0));
        assert_eq!(g.unit_get_extra_combat_percent(u), 35);
    }

    #[test]
    fn experience_and_levels() {
        let mut g = test_game();
        let owner = PlayerId(0);
        let u = add_unit(&mut g, owner, "UNIT_WARRIOR", 0, 0);

        // Plain experience; `max` caps it and stops further gains.
        g.unit_change_experience(u, 5, -1, false, false, false);
        assert_eq!(g.units[u].experience, 5);
        g.unit_change_experience(u, 10, 8, false, false, false);
        assert_eq!(g.units[u].experience, 8);
        g.unit_change_experience(u, 10, 8, false, false, false);
        assert_eq!(g.units[u].experience, 8);

        // Combat experience: in-borders bonus, then the unit's experience percent.
        g.units[u].experience = 0;
        g.players[owner].exp_in_border_modifier = 50;
        g.unit_change_experience(u, 4, -1, true, false, false);
        assert_eq!(g.units[u].experience, 4);
        g.unit_change_experience(u, 4, -1, true, true, false);
        assert_eq!(g.units[u].experience, 4 + 6);
        g.units[u].experience = 0;
        g.unit_change_experience_percent(u, 50);
        g.unit_change_experience(u, 4, -1, true, true, false);
        assert_eq!(g.units[u].experience, (4 + 2) * 150 / 100);
        // Non-combat experience ignores the modifiers.
        g.unit_change_experience(u, 3, -1, false, true, false);
        assert_eq!(g.units[u].experience, 9 + 3);

        // GAMEOPTION_SLOWER_XP halves combat experience, rounding up.
        g.setup.options[GameOptionType::GAMEOPTION_SLOWER_XP] = true;
        g.units[u].experience = 0;
        g.units[u].experience_percent = 0;
        g.unit_change_experience(u, 5, -1, true, false, false);
        assert_eq!(g.units[u].experience, 3);

        // Levels track the owner's highest unit level.
        assert_eq!(g.units[u].level, 1);
        assert_eq!(g.players[owner].highest_unit_level, 1);
        g.unit_change_level(u, 2);
        assert_eq!(g.units[u].level, 3);
        assert_eq!(g.players[owner].highest_unit_level, 3);
        g.unit_set_level(u, 2);
        assert_eq!(g.players[owner].highest_unit_level, 3);

        // CvGameUtils.getExperienceNeeded: level^2 + 1, plus the owner's modifier rounded up.
        assert_eq!(g.unit_experience_needed(u), 5);
        g.players[owner].level_experience_modifier = 50;
        assert_eq!(g.unit_experience_needed(u), 5 + (5 * 50 + 99) / 100);
    }

    #[test]
    fn xp_values_for_animals_and_barbarians() {
        let mut g = test_game();
        let d = g.d;
        let modifier = d.defines.barbarian_experience_modifier;
        let barbarian = g.barbarian_player();

        let warrior = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 0, 0);
        let info = &d.units[g.units[warrior].unit_type];
        assert_eq!(g.unit_attack_xp_value(warrior), info.xp_value_attack);
        assert_eq!(g.unit_defense_xp_value(warrior), info.xp_value_defense);

        let barb = add_unit(&mut g, barbarian, "UNIT_WARRIOR", 1, 0);
        assert!(g.unit_is_barbarian(barb));
        assert_eq!(g.unit_attack_xp_value(barb), info.xp_value_attack * modifier / 100);
        assert_eq!(g.unit_defense_xp_value(barb), info.xp_value_defense * modifier / 100);

        let wolf = add_unit(&mut g, PlayerId(0), "UNIT_WOLF", 2, 0);
        assert!(g.unit_is_animal(wolf));
        let wolf_info = &d.units[g.units[wolf].unit_type];
        assert_eq!(g.unit_attack_xp_value(wolf), wolf_info.xp_value_attack * modifier / 100);

        // Barbarians upgrade for free.
        let axeman = d.t::<UnitType>("UNIT_AXEMAN");
        assert_eq!(g.unit_upgrade_price(barb, axeman), 0);
    }

    #[test]
    fn promotion_prerequisites() {
        let mut g = test_game();
        let d = g.d;
        let p = |name: &str| d.t::<PromotionType>(name);
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);

        // Prereq promotion, MinLevel -1, race and equipment promotions are never picked.
        assert!(!g.unit_can_acquire_promotion(u, p("PROMOTION_COMBAT2")));
        assert!(!g.unit_can_acquire_promotion(u, p("PROMOTION_BRONZE_WEAPONS")));
        assert!(!g.unit_can_acquire_promotion(u, p("PROMOTION_ORC")));
        assert!(!g.unit_can_acquire_promotion(u, p("PROMOTION_INFERNAL_GRIMOIRE")));
        // Tech prereq (TECH_WARFARE), checked after the FfH tests.
        assert!(!g.unit_can_acquire_promotion(u, p("PROMOTION_CITY_GARRISON1")));
        // A promotion the unit already has.
        g.units[u].has_promotion[p("PROMOTION_COMBAT1")] = true;
        assert!(!g.unit_can_acquire_promotion(u, p("PROMOTION_COMBAT1")));

        // canPromote: no promotion, or the unit itself as the leader.
        assert!(!g.unit_can_promote(u, None, None));
        assert!(!g.unit_can_promote(u, Some(p("PROMOTION_COMBAT2")), Some(u)));
    }

    #[test]
    fn set_has_promotion_respects_immunities() {
        let mut g = test_game();
        let d = g.d;
        let immune = d.t::<PromotionType>("PROMOTION_IMMUNE_DISEASE");
        let diseased = d.t::<PromotionType>("PROMOTION_DISEASED");
        assert_eq!(d.promotions[immune].promotion_immune1, Some(diseased));
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);
        g.units[u].has_promotion[immune] = true;

        // Gaining a promotion the unit is immune to does nothing.
        g.unit_set_has_promotion(u, diseased, true);
        assert!(!g.units[u].has_promotion[diseased]);
        assert!(!g.unit_can_acquire_promotion(u, diseased));

        // Removing a promotion the unit does not have changes nothing.
        let before = (g.units[u].extra_combat_percent, g.units[u].blitz_count, g.units[u].base_combat);
        g.unit_set_has_promotion(u, d.t::<PromotionType>("PROMOTION_COMBAT1"), false);
        assert_eq!(before, (g.units[u].extra_combat_percent, g.units[u].blitz_count, g.units[u].base_combat));
    }

    #[test]
    fn promotion_ready_flag() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);

        // Terraformers become ready without touching their group's missions.
        g.units[u].ai_type = Some(UnitAIType::UNITAI_TERRAFORMER);
        g.unit_set_promotion_ready(u, true);
        assert!(g.units[u].promotion_ready);
        g.unit_set_promotion_ready(u, false);
        assert!(!g.units[u].promotion_ready);

        // testPromotionReady: not enough experience (level 1 needs 2) clears the flag.
        g.units[u].promotion_ready = true;
        g.units[u].experience = 1;
        g.unit_test_promotion_ready(u);
        assert!(!g.units[u].promotion_ready);
    }

    #[test]
    fn leaders_need_leader_units() {
        let mut g = test_game();
        let d = g.d;
        // No FFH2 unit is a great general (no leader promotion or leader experience).
        assert!(d.units.iter().all(|info| info.leader_promotion.is_none() && info.leader_experience == 0));

        let leader = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 2, 2);
        let other = add_unit(&mut g, PlayerId(0), "UNIT_AXEMAN", 2, 2);
        let plot = g.unit_plot(leader);
        g.map.plots[plot].units = vec![leader, other];

        assert_eq!(g.unit_can_lead(leader, plot, None), 0);
        assert_eq!(g.unit_can_lead(leader, plot, Some(other)), 0);
        assert!(!g.unit_lead(leader, Some(other)));
        assert!(g.units.contains(leader));
        assert_eq!(g.unit_can_give_experience(leader, plot), 0);
        assert_eq!(g.unit_get_stack_experience_to_give(leader, 2), 0);
        assert!(!g.unit_give_experience(leader));

        g.units[leader].death_delay = true;
        assert_eq!(g.unit_can_lead(leader, plot, None), 0);
    }

    #[test]
    fn upgrade_paths_and_capture() {
        let mut g = test_game();
        let d = g.d;
        let class = |name: &str| d.t::<UnitClassType>(name);
        let warrior_type = d.t::<UnitType>("UNIT_WARRIOR");
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);

        // Direct upgrade class, and through the Bannor axeman (warrior -> axeman -> champion).
        assert!(g.unit_upgrade_available(u, warrior_type, class("UNITCLASS_AXEMAN"), 0));
        assert!(g.unit_upgrade_available(u, warrior_type, class("UNITCLASS_CHAMPION"), 0));
        assert!(!g.unit_upgrade_available(u, warrior_type, class("UNITCLASS_WARRIOR"), 0));
        // The recursion depth limit.
        let limit = d.unit_classes.len() as i32;
        assert!(!g.unit_upgrade_available(u, warrior_type, class("UNITCLASS_AXEMAN"), limit + 1));

        // NO_UNIT never upgrades.
        assert!(!g.unit_can_upgrade(u, None, false));
        assert!(!g.unit_has_upgrade_to(u, None, true));
        let mut search_value = -7;
        assert_eq!(g.unit_get_upgrade_city_for(u, None, true, Some(&mut search_value)), None);
        assert_eq!(search_value, -7);

        // Capture classes go through the capturing civilization's units.
        let bannor = d.t::<CivilizationType>("CIVILIZATION_BANNOR");
        let settler = add_unit(&mut g, PlayerId(0), "UNIT_SETTLER", 2, 1);
        assert_eq!(g.unit_get_capture_unit_type(settler, bannor), d.civ_unit(bannor, class("UNITCLASS_WORKER")));
        assert!(g.unit_get_capture_unit_type(settler, bannor).is_some());
        assert_eq!(g.unit_get_capture_unit_type(u, bannor), None);
    }
}
