//! CvSelectionGroup: lifetime (`kill`, `doTurn`), activity and automation, busy/ready
//! checks, commands, unit membership (add/remove/merge/split, head unit, unit index),
//! owner/team, position (plot, x/y, area, domain), aggregates over the group's units
//! (moves, cargo, fight/defend, invisibility, unit AI counts), and the
//! `CvSelectionGroupAI` mission-AI / group-flag bookkeeping.
//!
//! C++ loops over `m_units` fetch the next node before acting on the current unit. Loops
//! here whose body can change group membership iterate over a copy of `units` and skip the
//! units that left the group (or died) in the meantime, which is what the C++ walk does (it
//! would read a freed node if the very next unit left).
//!
//! Some `CvUnitAI` / `CvPlayerAI` functions these rules depend on are AI code; they are
//! reproduced as private helpers at the end of this file: `ai_group_first_val` /
//! `ai_group_second_val` (the head-unit order of `addUnit`, exact ports), `ai_plot_danger`
//! (`AI_getPlotDanger(..) > 0` in `doTurn`) and `ai_unit_ai_valid` (`AI_unitValue(..) > 0` in
//! `mergeIntoGroup`, approximated).

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

/// C++ `GROUPFLAG_CONQUEST` (CvDefines.h, Sephi's BETTER AI group flags; read by
/// `CvUnitAI::AI_groupFirstVal`).
const GROUPFLAG_CONQUEST: i32 = 10;

impl Game {
    // ------------------------------------------------------------------
    // Lifetime, turn
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::kill`: remove the (empty) group from its owner
    /// (`removeGroupCycle`, `deleteSelectionGroup`).
    pub fn group_kill(&mut self, g: GroupId) {
        let owner = self.groups[g].owner;

        // C++ FAssert(getOwnerINLINE() != NO_PLAYER) and FAssertMsg(getID() != INVALID_INDEX):
        // always true here.
        debug_assert!(self.groups[g].units.is_empty(), "The number of units is expected to be 0");

        self.player_remove_group_cycle(owner, g);

        self.player_delete_selection_group(owner, g);
    }

    /// C++ `CvSelectionGroup::doTurn`: units' turns, waking (hold / healed / sentry alert),
    /// force-update of AI-controlled groups, danger checks for human missions.
    pub fn group_do_turn(&mut self, g: GroupId) {
        // C++ FAssert(getOwnerINLINE() != NO_PLAYER): always true here.
        if !self.groups[g].units.is_empty() {
            let mut hurt = false;

            // do unit's turns (checking for damage)
            for loop_unit in self.groups[g].units.clone() {
                if !unit_in_group(self, loop_unit, g) {
                    continue;
                }

                self.unit_do_turn(loop_unit);

                // C++ keeps reading `pLoopUnit` (a unit deleted by its own turn would be a
                // use-after-free there); here such a unit does not count as hurt.
                if self.units.contains(loop_unit) && self.unit_is_hurt(loop_unit) {
                    hurt = true;
                }
            }

            // C++ would go on with a deleted group (no unit turn deletes groups in practice).
            if !self.groups.contains(g) {
                return;
            }

            let activity = self.groups[g].activity_type;

            // wake unit if skipped last turn
            //		or healing and automated or no longer hurt (automated healing is one turn at a time)
            //		or on sentry and there is danger
            // Alertness (Pep, imported by Xienwolf, merged by Sephi): also wakes a healing group
            // holding the best defender of its plot when danger is near.
            if activity == ActivityType::ACTIVITY_HOLD
                || (activity == ActivityType::ACTIVITY_HEAL && (self.group_ai_is_controlled(g) || !hurt))
                || ((activity == ActivityType::ACTIVITY_SENTRY
                    || (activity == ActivityType::ACTIVITY_HEAL && holds_best_defender(self, g)))
                    && self.group_sentry_alert(g))
            {
                self.group_set_activity_type(g, ActivityType::ACTIVITY_AWAKE);
            }

            if self.group_ai_is_controlled(g) {
                // `AI_getPlotDanger(plot(), 2) > 0`: see `ai_plot_danger`.
                if self.groups[g].activity_type != ActivityType::ACTIVITY_MISSION
                    || (!self.group_can_fight(g) && group_plot_danger(self, g))
                {
                    // setForceUpdate(true): AI bookkeeping (the group's AI re-plans this turn).
                    self.groups[g].force_update = true;
                }
            } else if self.groups[g].activity_type == ActivityType::ACTIVITY_MISSION {
                let mut non_spy = false;
                for &loop_unit in &self.groups[g].units {
                    if !self.unit_is_spy(loop_unit) {
                        non_spy = true;
                        break;
                    }
                }

                if non_spy && group_plot_danger(self, g) {
                    self.group_clear_mission_queue(g);
                }
            }

            // C++ `if (isHuman() && isMPOption(MPOPTION_SIMULTANEOUS_TURNS))`: raise the mission
            // timer to the units' MIN_TIMER_UNIT_DOUBLE_MOVES wait and cycle the interface
            // selection -- multiplayer only; MPOPTION_SIMULTANEOUS_TURNS is never set here.
        }

        self.group_do_delayed_death(g);
    }

    /// C++ `CvSelectionGroup::sentryAlert` (protected; FFH "Alertness": uses the unit with the
    /// best visibility range): an enemy unit is visible within sentry range.
    pub fn group_sentry_alert(&self, g: GroupId) -> bool {
        let mut max_range = 0;

        // Alertness (Pep, imported by Xienwolf, merged by Sephi): ensures that the best visibility
        // range in the tile is used.
        let mut index = -1;

        for &loop_unit in &self.groups[g].units {
            let range = self.unit_visibility_range(loop_unit) + 1;

            if range > max_range {
                max_range = range;
                // Alertness
                index = self.group_get_unit_index(g, loop_unit, -1);
            }
        }

        // Alertness: originally `getHeadUnit()`. C++ quirk: no unit is used (no alert) when every
        // unit's range is below 0.
        let head_unit = if index == -1 { None } else { self.group_get_unit_at(g, index) };

        if let Some(head_unit) = head_unit {
            let (head_x, head_y) = (self.units[head_unit].x, self.units[head_unit].y);
            for x in -max_range..=max_range {
                for y in -max_range..=max_range {
                    if let Some(p) = self.plot_xy(head_x, head_y, x, y) {
                        if self.plot_can_see_plot(self.unit_plot(head_unit), p, self.unit_get_team(head_unit), max_range - 1, None) {
                            if self.plot_is_visible_enemy_unit_for_unit(p, head_unit) {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    // ------------------------------------------------------------------
    // Activity and automation
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::setActivityType`.
    pub fn group_set_activity_type(&mut self, g: GroupId, activity: ActivityType) {
        let old_activity = self.groups[g].activity_type;

        if old_activity != activity {
            // UI: airCircle(false) when leaving ACTIVITY_INTERCEPT (graphics only).

            self.group_set_blockading(g, false);

            self.groups[g].activity_type = activity;

            // UI: airCircle(true) for ACTIVITY_INTERCEPT; NotifyEntity(MISSION_IDLE) when not on
            // a mission; plot flag and interface dirty bits.
        }
    }

    /// C++ `CvSelectionGroup::isAutomated` (Xienwolf tweak: also true for human groups with
    /// FfH AI-controlled units). Non-const in C++, read-only.
    pub fn group_is_automated(&self, g: GroupId) -> bool {
        self.groups[g].automate_type.is_some() || (self.group_is_human(g) && self.group_is_ai_control(g))
    }

    /// C++ `CvSelectionGroup::setAutomateType` (`None` = NO_AUTOMATE; cancelling automation
    /// also cancels it on cargo groups).
    pub fn group_set_automate_type(&mut self, g: GroupId, automate: Option<AutomateType>) {
        // C++ FAssert(getOwnerINLINE() != NO_PLAYER): always true here.
        if self.groups[g].automate_type != automate {
            self.groups[g].automate_type = automate;

            self.group_clear_mission_queue(g);
            self.group_set_activity_type(g, ActivityType::ACTIVITY_AWAKE);

            // if canceling automation, cancel on cargo as well
            if automate.is_none() {
                if let Some(plot) = self.group_plot(g) {
                    // C++ walks the live plot list; stopping automation moves no unit.
                    for cargo_unit in self.map.plots[plot].units.clone() {
                        if let Some(transport_unit) = unit_transport(self, cargo_unit) {
                            if unit_group(self, transport_unit) == Some(g) {
                                // C++ dereferences `pCargoUnit->getGroup()`.
                                let cargo_group = unit_group(self, cargo_unit)
                                    .expect("CvSelectionGroup::setAutomateType: cargo unit without a group");
                                self.group_set_automate_type(cargo_group, None);
                                self.group_set_activity_type(cargo_group, ActivityType::ACTIVITY_AWAKE);
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvSelectionGroup::isWaiting`.
    pub fn group_is_waiting(&self, g: GroupId) -> bool {
        let a = self.groups[g].activity_type;
        a == ActivityType::ACTIVITY_HOLD
            || a == ActivityType::ACTIVITY_SLEEP
            || a == ActivityType::ACTIVITY_HEAL
            || a == ActivityType::ACTIVITY_SENTRY
            || a == ActivityType::ACTIVITY_PATROL
            || a == ActivityType::ACTIVITY_PLUNDER
            || a == ActivityType::ACTIVITY_INTERCEPT
    }

    // ------------------------------------------------------------------
    // Busy / ready, commands
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::isBusy`: mission timer running or a unit in combat
    /// (non-const in C++, read-only).
    pub fn group_is_busy(&self, g: GroupId) -> bool {
        let group = &self.groups[g];

        if group.units.is_empty() {
            return false;
        }

        if group.mission_timer > 0 {
            return true;
        }

        // C++ `pPlot = plot();` is never read.

        for &loop_unit in &group.units {
            // C++ `if (pLoopUnit != NULL)`
            if self.units.contains(loop_unit) && self.unit_is_combat(loop_unit) {
                return true;
            }
        }

        false
    }

    /// C++ `CvSelectionGroup::isCargoBusy`: a unit carried by this group is busy.
    pub fn group_is_cargo_busy(&self, g: GroupId) -> bool {
        let units = &self.groups[g].units;

        if units.is_empty() {
            return false;
        }

        let plot = self.group_plot(g);

        for &loop_unit1 in units {
            // C++ `if (pLoopUnit1 != NULL)`
            if self.units.contains(loop_unit1) && self.units[loop_unit1].cargo > 0 {
                // C++ dereferences `pPlot`.
                let plot = plot.expect("CvSelectionGroup::isCargoBusy: group without a plot");
                for &loop_unit2 in &self.map.plots[plot].units {
                    if self.units[loop_unit2].transport_unit == Some(loop_unit1) {
                        // C++ dereferences `pLoopUnit2->getGroup()`.
                        let cargo_group = unit_group(self, loop_unit2)
                            .expect("CvSelectionGroup::isCargoBusy: cargo unit without a group");
                        if self.group_is_busy(cargo_group) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// C++ `CvSelectionGroup::readyToSelect` (C++ default `bAny = false`).
    pub fn group_ready_to_select(&self, g: GroupId, any: bool) -> bool {
        self.group_ready_to_move(g, any) && !self.group_is_automated(g)
    }

    /// C++ `CvSelectionGroup::readyToMove` (C++ default `bAny = false`).
    pub fn group_ready_to_move(&self, g: GroupId, any: bool) -> bool {
        (if any { self.group_can_any_move(g) } else { self.group_can_all_move(g) })
            && self.groups[g].mission_queue.is_empty()
            && self.groups[g].activity_type == ActivityType::ACTIVITY_AWAKE
            && !self.group_is_busy(g)
            && !self.group_is_cargo_busy(g)
    }

    /// C++ `CvSelectionGroup::readyToAuto`.
    pub fn group_ready_to_auto(&self, g: GroupId) -> bool {
        self.group_can_all_move(g) && !self.groups[g].mission_queue.is_empty()
    }

    /// C++ `CvSelectionGroup::canDoCommand` (C++ `bUseCache` UI cache not ported).
    pub fn group_can_do_command(&self, g: GroupId, command: CommandType, data1: i32, data2: i32, test_visible: bool) -> bool {
        // C++ `bUseCache` (the interface's `m_bIsBusyCache`) is not ported: always `isBusy()`.
        if self.group_is_busy(g) {
            return false;
        }

        if !self.group_can_ever_do_command(g, command, data1, data2, test_visible) {
            return false;
        }

        for &loop_unit in &self.groups[g].units {
            if self.unit_can_do_command(loop_unit, command, data1, data2, test_visible, false) {
                return true;
            }
        }

        false
    }

    /// C++ `CvSelectionGroup::canEverDoCommand` (C++ `bUseCache` UI cache not ported: the
    /// COMMAND_UPGRADE cache branch is never taken).
    pub fn group_can_ever_do_command(&self, g: GroupId, command: CommandType, data1: i32, data2: i32, test_visible: bool) -> bool {
        // `data1`, `data2` and `test_visible` are only passed on by the C++ upgrade-cache branch.
        let _ = (data1, data2, test_visible);

        if command == CommandType::COMMAND_LOAD {
            // C++ dereferences `plot()`.
            let plot = self.group_plot(g).expect("CvSelectionGroup::canEverDoCommand: group without a plot");
            for &loop_unit in &self.map.plots[plot].units {
                if !self.unit_is_full(loop_unit) {
                    return true;
                }
            }

            //no cargo space on this plot
            return false;
        } else if command == CommandType::COMMAND_UNLOAD {
            for &loop_unit in &self.groups[g].units {
                if self.unit_is_cargo(loop_unit) {
                    return true;
                }
            }

            //no loaded unit
            return false;
        }

        // C++ COMMAND_UPGRADE with `bUseCache`: asks the interface's different-unit cache (not
        // ported); without the cache it falls through to `true` like every other command.

        true
    }

    // ------------------------------------------------------------------
    // Owner, control
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::isHuman` (owner is human; non-const in C++, read-only).
    pub fn group_is_human(&self, g: GroupId) -> bool {
        // C++ returns true for NO_PLAYER (ownerless UI groups, not ported).
        self.players[self.groups[g].owner].human
    }

    /// C++ `CvSelectionGroup::getTeam` (the owner's team).
    pub fn group_get_team(&self, g: GroupId) -> TeamId {
        // C++ returns NO_TEAM for NO_PLAYER (ownerless UI groups, not ported).
        self.players[self.groups[g].owner].team
    }

    /// C++ `CvSelectionGroup::isAIControl` (FfH: any unit is AI-controlled, i.e. has a
    /// promotion with `bAIControl`).
    pub fn group_is_ai_control(&self, g: GroupId) -> bool {
        // C++ `if (getNumUnits() > 0)` around the loop.
        for &loop_unit in &self.groups[g].units {
            if self.unit_is_ai_control(loop_unit) {
                return true;
            }
        }

        false
    }

    /// C++ `CvSelectionGroupAI::AI_isControlled`: the AI moves this group (non-human owner or
    /// automated). Non-const in C++, read-only.
    pub fn group_ai_is_controlled(&self, g: GroupId) -> bool {
        !self.group_is_human(g) || self.group_is_automated(g)
    }

    // ------------------------------------------------------------------
    // Unit membership
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::clearUnits`.
    pub fn group_clear_units(&mut self, g: GroupId) {
        while !self.groups[g].units.is_empty() {
            self.group_delete_unit_node(g, 0);
        }
    }

    /// C++ `CvSelectionGroup::addUnit`: inserts `u` ordered by `AI_groupFirstVal` /
    /// `AI_groupSecondVal` (which decides the head unit). Returns false if the unit cannot
    /// join (`CvUnit::canJoinGroup`).
    pub fn group_add_unit(&mut self, g: GroupId, u: UnitId, minimal_change: bool) -> bool {
        // C++ `canJoinGroup(pUnit->plot(), this)`: NULL plot while `init` -> `setXY` puts a unit
        // that is not on the map yet into its first group.
        if !self.unit_can_join_group_opt(u, self.unit_plot_opt(u), g) {
            return false;
        }

        // Before the first unit with a lower `AI_groupFirstVal`, or an equal one and a lower
        // `AI_groupSecondVal`; else at the end. (C++ evaluates the values per comparison, so
        // none for an empty group.)
        let mut insert_index = None;
        if !self.groups[g].units.is_empty() {
            let first_val = ai_group_first_val(self, u);

            for (index, &loop_unit) in self.groups[g].units.iter().enumerate() {
                let loop_first_val = ai_group_first_val(self, loop_unit);
                if first_val > loop_first_val
                    || (first_val == loop_first_val && ai_group_second_val(self, u) > ai_group_second_val(self, loop_unit))
                {
                    insert_index = Some(index);
                    break;
                }
            }
        }

        match insert_index {
            Some(index) => self.groups[g].units.insert(index, u),
            None => self.groups[g].units.push(u),
        }

        // C++ `if (!bMinimalChange && getOwnerINLINE() == NO_PLAYER)`: MISSION_MULTI_SELECT entity
        // notifications for the interface selection group (graphics; ownerless groups are not
        // ported).
        let _ = minimal_change;

        true
    }

    /// C++ `CvSelectionGroup::removeUnit`.
    pub fn group_remove_unit(&mut self, g: GroupId, u: UnitId) {
        if let Some(index) = self.groups[g].units.iter().position(|&x| x == u) {
            self.group_delete_unit_node(g, index);
        }
    }

    /// C++ `CvSelectionGroup::deleteUnitNode`, with the node given by its index in `units`
    /// (the C++ return value, the next node, is the same index afterwards). Removing a unit
    /// stops automation and missions, and wakes the group unless it sleeps/intercepts/
    /// patrols/plunders.
    pub fn group_delete_unit_node(&mut self, g: GroupId, index: usize) {
        // C++: `if (getOwnerINLINE() != NO_PLAYER)` -- always true here (see `SelectionGroup::owner`).
        self.group_set_automate_type(g, None);
        self.group_clear_mission_queue(g);

        let a = self.groups[g].activity_type;
        if !(a == ActivityType::ACTIVITY_SLEEP
            || a == ActivityType::ACTIVITY_INTERCEPT
            || a == ActivityType::ACTIVITY_PATROL
            || a == ActivityType::ACTIVITY_PLUNDER)
        {
            self.group_set_activity_type(g, ActivityType::ACTIVITY_AWAKE);
        }

        self.groups[g].units.remove(index);
    }

    /// C++ `CvSelectionGroup::mergeIntoGroup`: move all units into `into`, keeping its head
    /// unit AI (may change unit AIs, via `CvPlayerAI::AI_unitValue`).
    pub fn group_merge_into_group(&mut self, g: GroupId, into: GroupId) {
        // C++ `CvPlayerAI& kPlayer = GET_PLAYER(getOwnerINLINE())` is only used for
        // `AI_unitValue` (see `ai_unit_ai_valid`).

        // merge groups, but make sure we do not change the head unit AI
        // this means that if a new unit is going to become the head, change its AI to match, if possible
        // AI_setUnitAIType removes the unit from the current group (at least currently), so we have to be careful in the loop here
        // so, loop until we have not changed unit AIs
        loop {
            let mut changed_unit_ai = false;

            // loop over all the units, moving them to the new group,
            // stopping if we had to change a unit AI, because doing so removes that unit from our group, so we have to start over
            for loop_unit in self.groups[g].units.clone() {
                if changed_unit_ai {
                    break;
                }

                // C++ `if (pLoopUnit != NULL)` on the live list.
                if !unit_in_group(self, loop_unit, g) {
                    continue;
                }

                let unit_ai = self.units[loop_unit].ai_type;

                // if the unitAIs are different, and the loop unit has a higher val, then the group unitAI would change
                // change this UnitAI to the old group UnitAI if possible
                let new_head_unit = self.group_get_head_unit(into);
                let new_head_unit_ai = self.group_get_head_unit_ai(into);
                if let Some(new_head_unit) = new_head_unit {
                    if unit_ai != new_head_unit_ai && ai_group_first_val(self, loop_unit) > ai_group_first_val(self, new_head_unit) {
                        // non-zero AI_unitValue means that this UnitAI is valid for this unit (that is the check used everywhere)
                        // (A head unit always has a UnitAI once `init` is done.)
                        if let Some(new_head_unit_ai) = new_head_unit_ai {
                            if ai_unit_ai_valid(self, self.units[loop_unit].unit_type, new_head_unit_ai) {
                                // this will remove pLoopUnit from the current group
                                self.unit_ai_set_unit_ai_type(loop_unit, new_head_unit_ai);

                                changed_unit_ai = true;
                            }
                        }
                    }
                }

                self.unit_join_group(loop_unit, Some(into), true);
            }

            if !changed_unit_ai {
                break;
            }
        }
    }

    /// C++ `CvSelectionGroup::splitGroup(iSplitSize, pNewHeadUnit, ppOtherGroup)`: returns
    /// `(group of at most split_size units, remainder group (C++ *ppOtherGroup))`. Returns
    /// `(Some(g), None)` if the group is already small enough, `(None, None)` on bad input.
    ///
    /// Split up each unit AI type as evenly as possible. `g` is left empty (not killed).
    pub fn group_split_group(&mut self, g: GroupId, split_size: i32, new_head_unit: Option<UnitId>) -> (Option<GroupId>, Option<GroupId>) {
        // C++ FAssertMsg(iSplitSize > 0, "non-positive splitGroup size")
        if split_size <= 0 {
            return (None, None);
        }

        // are we already small enough?
        if self.groups[g].units.len() as i32 <= split_size {
            return (Some(g), None);
        }

        let old_head_unit = self.groups[g].units[0];
        // C++ FAssertMsg(pOldHeadUnit != NULL, "non-zero group without head unit")
        if !self.units.contains(old_head_unit) {
            return (None, None);
        }

        let old_head_ai = self.units[old_head_unit].ai_type;

        // if pNewHeadUnit NULL, then we will use our current head to head the new split group of target size
        let new_head_unit = new_head_unit.unwrap_or(old_head_unit);

        // the AI of the new head (the remainder will get the AI of the old head)
        // UnitAITypes eNewHeadAI = pNewHeadUnit->AI_getUnitAIType();

        // pRemainderHeadUnit is the head unit of the group that contains the remainder of units
        let mut remainder_head_unit = None;

        // if the new head is not the old head, then make the old head the remainder head
        let spliting_head = old_head_unit == new_head_unit;
        if !spliting_head {
            remainder_head_unit = Some(old_head_unit);
        }

        // try to find remainder head with same AI as head, if we cannot find one, we will split the rest of the group up
        if remainder_head_unit.is_none() {
            // loop over all the units
            for &loop_unit in &self.groups[g].units {
                if self.units.contains(loop_unit) && loop_unit != new_head_unit && self.units[loop_unit].ai_type == old_head_ai {
                    remainder_head_unit = Some(loop_unit);
                    break;
                }
            }
        }

        // make the new group for the new head
        self.unit_join_group(new_head_unit, None, true);
        let split_group = unit_group(self, new_head_unit).expect("CvSelectionGroup::splitGroup: join resulted in NULL group");

        // make a new group for the remainder, if non-null
        let mut remainder_group = None;
        if let Some(remainder_head_unit) = remainder_head_unit {
            self.unit_join_group(remainder_head_unit, None, true);
            remainder_group = unit_group(self, remainder_head_unit);
            // C++ FAssertMsg(pRemainderGroup != NULL, "join resulted in NULL group")
        }

        // loop until this group is empty, trying to move different AI types each time

        // C++ `bool abUnitAIField[NUM_UNITAI_TYPES]`, indexed by the unit AI. The extra last slot
        // stands for NO_UNITAI, which C++ would index out of bounds (every unit has a UnitAI
        // once `init` is done).
        let ai_slot = |unit_ai: Option<UnitAIType>| unit_ai.map_or(UnitAIType::NUM, |a| a.idx());
        let mut unit_ai_field = [false; UnitAIType::NUM + 1];

        while !self.groups[g].units.is_empty() {
            let num_units_before = self.groups[g].units.len();

            let mut target_unit_ai: Option<UnitAIType> = None;

            // loop over all the units, find the next different UnitAI and move one of each
            let mut destination_split = (self.groups[split_group].units.len() as i32) < split_size;
            for loop_unit in self.groups[g].units.clone() {
                // C++ `if (pLoopUnit != NULL)` on the live list.
                if !unit_in_group(self, loop_unit, g) {
                    continue;
                }

                let loop_unit_ai = self.units[loop_unit].ai_type;

                // if we have not found a new type to move, is this a new unitai?
                // note, if there eventually are unitAIs above 31, we will just always move those, which is fine
                if target_unit_ai.is_none() && !unit_ai_field[ai_slot(loop_unit_ai)] {
                    target_unit_ai = loop_unit_ai;
                    unit_ai_field[ai_slot(loop_unit_ai)] = true;
                }

                // is this the right UnitAI?
                if loop_unit_ai == target_unit_ai {
                    // move this unit to the appropriate group
                    if destination_split {
                        self.unit_join_group(loop_unit, Some(split_group), true);
                    } else {
                        self.unit_join_group(loop_unit, remainder_group, true);
                        // (if pRemainderGroup NULL, it gets its own group)
                        // C++ quirk: if the join was refused (`canJoinGroup`), this is the group
                        // being split.
                        remainder_group = unit_group(self, loop_unit);
                    }

                    // if we moved to remainder, try for next unit AI
                    if !destination_split {
                        target_unit_ai = None;

                        destination_split = (self.groups[split_group].units.len() as i32) < split_size;
                    } else {
                        // next unit goes to the remainder group
                        destination_split = false;
                    }
                }
            }

            // clear bitfield, all types are valid again
            unit_ai_field = [false; UnitAIType::NUM + 1];

            // Not in C++: when every unit left was refused by its destination (`canJoinGroup`,
            // e.g. an FfH AI-controlled or hidden-nationality unit), C++ repeats the same pass
            // forever; stop instead and leave those units in this group.
            if self.groups[g].units.len() == num_units_before {
                break;
            }
        }

        // C++ FAssertMsg(pSplitGroup->getNumUnits() <= iSplitSize, "somehow our split group is too large")

        (Some(split_group), remainder_group)
    }

    /// C++ `CvSelectionGroup::getUnitIndex` (C++ default `maxIndex = -1`): position of `u`, or
    /// -1 (also -1 once `max_index` units were checked).
    pub fn group_get_unit_index(&self, g: GroupId, u: UnitId, max_index: i32) -> i32 {
        let mut index = 0;

        for &x in &self.groups[g].units {
            if x == u {
                return index;
            }

            index += 1;

            //early out if not interested beyond maxIndex
            if max_index >= 0 && index >= max_index {
                return -1;
            }
        }

        -1
    }

    /// C++ `CvSelectionGroup::getHeadUnit`.
    pub fn group_get_head_unit(&self, g: GroupId) -> Option<UnitId> {
        self.groups[g].units.first().copied()
    }

    /// C++ `CvSelectionGroup::getUnitAt`.
    pub fn group_get_unit_at(&self, g: GroupId, index: i32) -> Option<UnitId> {
        let units = &self.groups[g].units;
        if index >= units.len() as i32 {
            return None;
        }
        // C++ quirk: a negative index returns the head unit (the node-advance loop never runs).
        units.get(index.max(0) as usize).copied()
    }

    /// C++ `CvSelectionGroupAI::isHeadUnit` (FFH BETTER AI).
    pub fn group_is_head_unit(&self, g: GroupId, u: UnitId) -> bool {
        self.group_get_head_unit(g) == Some(u)
    }

    /// C++ `CvSelectionGroup::getHeadUnitAI` (`None` = NO_UNITAI for an empty group).
    pub fn group_get_head_unit_ai(&self, g: GroupId) -> Option<UnitAIType> {
        self.group_get_head_unit(g).and_then(|head_unit| self.units[head_unit].ai_type)
    }

    /// C++ `CvSelectionGroup::getHeadOwner`.
    pub fn group_get_head_owner(&self, g: GroupId) -> Option<PlayerId> {
        self.group_get_head_unit(g).map(|head_unit| self.units[head_unit].owner)
    }

    /// C++ `CvSelectionGroup::getHeadTeam`.
    pub fn group_get_head_team(&self, g: GroupId) -> Option<TeamId> {
        self.group_get_head_unit(g).map(|head_unit| self.unit_get_team(head_unit))
    }

    // ------------------------------------------------------------------
    // Position (all from the head unit)
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::getX` (INVALID_PLOT_COORD for an empty group).
    pub fn group_get_x(&self, g: GroupId) -> i32 {
        self.group_get_head_unit(g).map_or(INVALID_PLOT_COORD, |head_unit| self.units[head_unit].x)
    }

    /// C++ `CvSelectionGroup::getY` (INVALID_PLOT_COORD for an empty group).
    pub fn group_get_y(&self, g: GroupId) -> i32 {
        self.group_get_head_unit(g).map_or(INVALID_PLOT_COORD, |head_unit| self.units[head_unit].y)
    }

    /// C++ `CvSelectionGroup::at`.
    pub fn group_at(&self, g: GroupId, x: i32, y: i32) -> bool {
        self.group_get_x(g) == x && self.group_get_y(g) == y
    }

    /// C++ `CvSelectionGroup::atPlot` (`plot() == pPlot`; `None` matches an empty group).
    pub fn group_at_plot(&self, g: GroupId, plot: Option<PlotIdx>) -> bool {
        self.group_plot(g) == plot
    }

    /// C++ `CvSelectionGroup::plot` (None for an empty group).
    pub fn group_plot(&self, g: GroupId) -> Option<PlotIdx> {
        // C++ `getHeadUnit()->plot()` (also NULL while the head unit is off the map).
        self.group_get_head_unit(g).and_then(|head_unit| self.unit_plot_opt(head_unit))
    }

    /// C++ `CvSelectionGroup::area` and `getArea` (the area id; C++ `getArea` returns 0
    /// (`NULL`) for an empty group, here `None`).
    pub fn group_area(&self, g: GroupId) -> Option<AreaId> {
        self.group_get_head_unit(g).map(|head_unit| self.unit_area(head_unit))
    }

    /// C++ `CvSelectionGroup::getDomainType` (`None` = NO_DOMAIN for an empty group).
    pub fn group_get_domain_type(&self, g: GroupId) -> Option<DomainType> {
        self.group_get_head_unit(g).map(|head_unit| self.unit_get_domain_type(head_unit))
    }

    // ------------------------------------------------------------------
    // Aggregates over the group's units
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::baseMoves` (minimum over units; MAX_INT for an empty group).
    pub fn group_base_moves(&self, g: GroupId) -> i32 {
        let mut best_value = MAX_INT;

        for &loop_unit in &self.groups[g].units {
            let value = self.unit_base_moves(loop_unit);

            if value < best_value {
                best_value = value;
            }
        }

        best_value
    }

    /// C++ `CvSelectionGroup::isFull` (cargo space; special-cargo two-pass rule).
    pub fn group_is_full(&self, g: GroupId) -> bool {
        let units = &self.groups[g].units;

        if !units.is_empty() {
            // do two passes, the first pass, we ignore units with speical cargo
            let mut special_cargo_count = 0;
            let mut cargo_count = 0;

            // first pass, count but ignore special cargo units
            for &loop_unit in units {
                // C++ `cargoSpace()`
                if self.units[loop_unit].cargo_capacity > 0 {
                    cargo_count += 1;
                }

                if self.unit_special_cargo(loop_unit).is_some() {
                    special_cargo_count += 1;
                } else if !self.unit_is_full(loop_unit) {
                    return false;
                }
            }

            // if every unit in the group has special cargo, then check those, otherwise, consider ourselves full
            if special_cargo_count >= cargo_count {
                for &loop_unit in units {
                    if !self.unit_is_full(loop_unit) {
                        return false;
                    }
                }
            }

            return true;
        }

        false
    }

    /// C++ `CvSelectionGroup::hasCargo`.
    pub fn group_has_cargo(&self, g: GroupId) -> bool {
        self.groups[g].units.iter().any(|&loop_unit| self.unit_has_cargo(loop_unit))
    }

    /// C++ `CvSelectionGroup::getCargo` (total cargo carried).
    pub fn group_get_cargo(&self, g: GroupId) -> i32 {
        self.groups[g].units.iter().map(|&loop_unit| self.units[loop_unit].cargo).sum()
    }

    /// C++ `CvSelectionGroup::canAllMove` (false for an empty group).
    pub fn group_can_all_move(&self, g: GroupId) -> bool {
        let units = &self.groups[g].units;

        if !units.is_empty() {
            for &loop_unit in units {
                // C++ FAssertMsg(pLoopUnit != NULL, "existing node, but NULL unit") and
                // `if (pLoopUnit != NULL && !(pLoopUnit->canMove()))`.
                if self.units.contains(loop_unit) && !self.unit_can_move(loop_unit) {
                    return false;
                }
            }

            return true;
        }

        false
    }

    /// C++ `CvSelectionGroup::canAnyMove`.
    pub fn group_can_any_move(&self, g: GroupId) -> bool {
        self.groups[g].units.iter().any(|&loop_unit| self.unit_can_move(loop_unit))
    }

    /// C++ `CvSelectionGroup::hasMoved`.
    pub fn group_has_moved(&self, g: GroupId) -> bool {
        self.groups[g].units.iter().any(|&loop_unit| self.unit_has_moved(loop_unit))
    }

    /// C++ `CvSelectionGroup::canFight`.
    pub fn group_can_fight(&self, g: GroupId) -> bool {
        self.groups[g].units.iter().any(|&loop_unit| self.unit_can_fight(loop_unit))
    }

    /// C++ `CvSelectionGroup::canDefend`.
    pub fn group_can_defend(&self, g: GroupId) -> bool {
        self.groups[g].units.iter().any(|&loop_unit| self.unit_can_defend(loop_unit, None))
    }

    /// C++ `CvSelectionGroup::visibilityRange`. C++ quirk: declared `bool`, so the computed
    /// maximum range is returned as `range != 0`.
    pub fn group_visibility_range(&self, g: GroupId) -> bool {
        let mut max_range = 0;

        for &loop_unit in &self.groups[g].units {
            let range = self.unit_visibility_range(loop_unit);
            if range > max_range {
                max_range = range;
            }
        }

        // C++ quirk: `return iMaxRange;` from a `bool` function.
        max_range != 0
    }

    /// C++ `CvSelectionGroup::unloadAll`.
    pub fn group_unload_all(&mut self, g: GroupId) {
        for loop_unit in self.groups[g].units.clone() {
            // C++ `if (pLoopUnit != NULL)` on the live list.
            if unit_in_group(self, loop_unit, g) {
                self.unit_unload_all(loop_unit);
            }
        }
    }

    /// C++ `CvSelectionGroup::alwaysInvisible` (false for an empty group).
    pub fn group_always_invisible(&self, g: GroupId) -> bool {
        let units = &self.groups[g].units;

        if !units.is_empty() {
            for &loop_unit in units {
                if !self.unit_always_invisible(loop_unit) {
                    return false;
                }
            }

            return true;
        }

        false
    }

    /// C++ `CvSelectionGroup::isInvisible` (all units invisible to `team`; false if empty).
    pub fn group_is_invisible(&self, g: GroupId, team: TeamId) -> bool {
        let units = &self.groups[g].units;

        if !units.is_empty() {
            for &loop_unit in units {
                // C++ `isInvisible(eTeam, false)`: `bDebug = false`, default `bCheckCargo = true`.
                if !self.unit_is_invisible(loop_unit, team, true) {
                    return false;
                }
            }

            return true;
        }

        false
    }

    /// C++ `CvSelectionGroup::countNumUnitAIType` (`unit_ai = None` = NO_UNITAI counts all).
    pub fn group_count_num_unit_ai_type(&self, g: GroupId, unit_ai: Option<UnitAIType>) -> i32 {
        // C++ FAssertMsg(headUnitNode() != NULL, ...) (debug builds only; an empty group counts 0).
        let mut count = 0;

        for &loop_unit in &self.groups[g].units {
            // count all units if NO_UNITAI passed in
            if unit_ai.is_none() || self.units[loop_unit].ai_type == unit_ai {
                count += 1;
            }
        }

        count
    }

    /// C++ `CvSelectionGroup::hasWorker`.
    pub fn group_has_worker(&self, g: GroupId) -> bool {
        self.group_count_num_unit_ai_type(g, Some(UnitAIType::UNITAI_WORKER)) > 0
            || self.group_count_num_unit_ai_type(g, Some(UnitAIType::UNITAI_WORKER_SEA)) > 0
    }

    /// C++ `CvSelectionGroup::setBlockading` (each unit's `setBlockading`, which updates plunder).
    pub fn group_set_blockading(&mut self, g: GroupId, start: bool) {
        // C++ reads the next node after the call; `setBlockading` changes no group.
        for loop_unit in self.groups[g].units.clone() {
            self.unit_set_blockading(loop_unit, start);
        }
    }

    // ------------------------------------------------------------------
    // CvSelectionGroupAI bookkeeping read/written by rules code
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroupAI::AI_getMissionAIPlot` (`plotSorenINLINE(m_iMissionAIX, m_iMissionAIY)`).
    pub fn group_ai_get_mission_ai_plot(&self, g: GroupId) -> Option<PlotIdx> {
        let group = &self.groups[g];
        self.map.plot_soren(group.ai_mission_ai_x, group.ai_mission_ai_y)
    }

    /// C++ `CvSelectionGroupAI::AI_setMissionAI` (stores the plot as `ai_mission_ai_x/y`).
    pub fn group_ai_set_mission_ai(&mut self, g: GroupId, mission_ai: Option<MissionAIType>, plot: Option<PlotIdx>, unit: Option<UnitId>) {
        let (x, y) = match plot {
            Some(plot) => self.map.xy(plot),
            None => (INVALID_PLOT_COORD, INVALID_PLOT_COORD),
        };

        let group = &mut self.groups[g];
        group.ai_mission_ai_type = mission_ai;

        group.ai_mission_ai_x = x;
        group.ai_mission_ai_y = y;

        // C++ `m_missionAIUnit = pNewUnit->getIDInfo()` or `m_missionAIUnit.reset()`.
        group.ai_mission_ai_unit = unit;
    }

    /// C++ `CvSelectionGroupAI::AI_getMissionAIUnit` (`getUnit(m_missionAIUnit)`: None once the
    /// unit is gone).
    pub fn group_ai_get_mission_ai_unit(&self, g: GroupId) -> Option<UnitId> {
        self.groups[g].ai_mission_ai_unit.filter(|&u| self.units.contains(u))
    }

    /// C++ `CvSelectionGroupAI::AI_getGroupflag` (FFH BETTER AI: the head unit's group flag,
    /// `GROUPFLAG_*` in `defs.rs`).
    pub fn group_ai_get_groupflag(&self, g: GroupId) -> i32 {
        // C++ dereferences `getHeadUnit()`.
        let head_unit = self.group_get_head_unit(g).expect("CvSelectionGroupAI::AI_getGroupflag: empty group");
        self.units[head_unit].ai_groupflag
    }

    /// C++ `CvSelectionGroupAI::AI_setGroupflag` (sets the flag on every unit).
    pub fn group_ai_set_groupflag(&mut self, g: GroupId, flag: i32) {
        for index in 0..self.groups[g].units.len() {
            let loop_unit = self.groups[g].units[index];
            // C++ `pLoopUnit->AI_setGroupflag(newflag)`
            self.units[loop_unit].ai_groupflag = flag;
        }
    }
}

// ----------------------------------------------------------------------
// Private helpers
// ----------------------------------------------------------------------

/// Whether `u` is alive and still in `g`'s unit list (and `g` still exists): the units a C++
/// walk over `m_units` still reaches after earlier iterations changed the group.
fn unit_in_group(game: &Game, u: UnitId, g: GroupId) -> bool {
    game.groups.contains(g) && game.units.contains(u) && game.groups[g].units.contains(&u)
}

/// C++ `CvUnit::getGroup()`: `GET_PLAYER(owner).getSelectionGroup(m_iGroupID)`, NULL without a
/// group or once the group was deleted.
fn unit_group(game: &Game, u: UnitId) -> Option<GroupId> {
    game.units[u].group.filter(|&g| game.groups.contains(g))
}

/// C++ `CvUnit::getTransportUnit()`: `::getUnit(m_transportUnit)`, NULL once the transport is gone.
fn unit_transport(game: &Game, u: UnitId) -> Option<UnitId> {
    game.units[u].transport_unit.filter(|&t| game.units.contains(t))
}

/// C++ `plot()->getBestDefender(getOwnerINLINE())->getGroup() == this` (Alertness, in
/// `CvSelectionGroup::doTurn`): the group holds its owner's best defender on its plot. C++
/// dereferences both pointers; neither is NULL for a group with units (they stand on the plot).
fn holds_best_defender(game: &Game, g: GroupId) -> bool {
    let owner = game.groups[g].owner;
    let plot = game.group_plot(g).expect("CvSelectionGroup::doTurn: healing group without a plot");
    let best_defender = game
        .plot_get_best_defender(plot, Some(owner), None, None, false, false, false)
        .expect("CvSelectionGroup::doTurn: no best defender on the group's plot");
    unit_group(game, best_defender) == Some(g)
}

/// C++ `GET_PLAYER(getOwnerINLINE()).AI_getPlotDanger(plot(), 2) > 0` in
/// `CvSelectionGroup::doTurn` (C++ dereferences `plot()`; a group on a mission has units).
fn group_plot_danger(game: &Game, g: GroupId) -> bool {
    let plot = game.group_plot(g).expect("CvSelectionGroup::doTurn: AI_getPlotDanger(plot()) without a plot");
    ai_plot_danger(game, game.groups[g].owner, plot, 2)
}

/// C++ `CvUnitAI::AI_groupFirstVal` (BETTER AI, Sephi: summons last, conquest stacks and heroes
/// first): primary key of the head-unit order (`CvSelectionGroup::addUnit` inserts a unit before
/// the first unit with a lower value). Rules-visible: the head unit gives the group's position,
/// and group loops (e.g. `doTurn`) run the units in this order.
fn ai_group_first_val(game: &Game, u: UnitId) -> i32 {
    let unit = &game.units[u];

    // BETTER AI (improved logic which unit becomes head of a group) Sephi
    if unit.duration > 0 {
        return 1;
    }

    if unit.ai_groupflag == GROUPFLAG_CONQUEST {
        return 25;
    }
    // END

    // NO_UNITAI (between `reset` and `AI_init`, when `init` -> `setXY` groups a new unit) takes
    // the C++ `default: FAssert(false)` case.
    let Some(unit_ai) = unit.ai_type else {
        return 0;
    };

    match unit_ai {
        // C++ FAssert(false)
        UnitAIType::UNITAI_UNKNOWN | UnitAIType::UNITAI_ANIMAL => 0,
        UnitAIType::UNITAI_SETTLE => 21,
        UnitAIType::UNITAI_WORKER => 20,
        UnitAIType::UNITAI_ATTACK => {
            if game.unit_collateral_damage(u) > 0 {
                17
            } else if game.unit_withdrawal_probability(u) > 0 {
                15
            } else {
                13
            }
        }
        UnitAIType::UNITAI_ATTACK_CITY => {
            if game.unit_bombard_rate(u) > 0 {
                19
            } else if game.unit_collateral_damage(u) > 0 {
                18
            } else if game.unit_withdrawal_probability(u) > 0 {
                16
            } else {
                14
            }
        }
        UnitAIType::UNITAI_COLLATERAL => 7,
        UnitAIType::UNITAI_PILLAGE => 12,
        UnitAIType::UNITAI_RESERVE => 6,
        UnitAIType::UNITAI_COUNTER => 5,
        UnitAIType::UNITAI_CITY_DEFENSE => 3,
        UnitAIType::UNITAI_CITY_COUNTER => 2,
        // BETTER AI (New UNITAI) Sephi: the mage types share UNITAI_CITY_SPECIAL's value.
        UnitAIType::UNITAI_CITY_SPECIAL
        | UnitAIType::UNITAI_MAGE
        | UnitAIType::UNITAI_TERRAFORMER
        | UnitAIType::UNITAI_MANA_UPGRADE
        | UnitAIType::UNITAI_WARWIZARD => 3,
        // BETTER AI (New UNITAI) Sephi
        UnitAIType::UNITAI_BARBSMASHER | UnitAIType::UNITAI_FEASTING | UnitAIType::UNITAI_MEDIC => 3,
        UnitAIType::UNITAI_HERO => 100, //Heroes don't like to get pushed around
        UnitAIType::UNITAI_PARADROP => 4,
        UnitAIType::UNITAI_EXPLORE => 8,
        UnitAIType::UNITAI_MISSIONARY => 10,
        UnitAIType::UNITAI_PROPHET
        | UnitAIType::UNITAI_ARTIST
        | UnitAIType::UNITAI_SCIENTIST
        | UnitAIType::UNITAI_GENERAL
        | UnitAIType::UNITAI_MERCHANT
        | UnitAIType::UNITAI_ENGINEER => 11,
        UnitAIType::UNITAI_SPY => 9,
        UnitAIType::UNITAI_ICBM => 0,
        UnitAIType::UNITAI_WORKER_SEA => 8,
        UnitAIType::UNITAI_ATTACK_SEA => 3,
        UnitAIType::UNITAI_RESERVE_SEA => 2,
        UnitAIType::UNITAI_ESCORT_SEA => 1,
        UnitAIType::UNITAI_EXPLORE_SEA => 5,
        UnitAIType::UNITAI_ASSAULT_SEA => 11,
        UnitAIType::UNITAI_SETTLER_SEA => 9,
        UnitAIType::UNITAI_MISSIONARY_SEA => 9,
        UnitAIType::UNITAI_SPY_SEA => 10,
        UnitAIType::UNITAI_CARRIER_SEA => 7,
        UnitAIType::UNITAI_MISSILE_CARRIER_SEA => 6,
        UnitAIType::UNITAI_PIRATE_SEA => 4,
        UnitAIType::UNITAI_ATTACK_AIR
        | UnitAIType::UNITAI_DEFENSE_AIR
        | UnitAIType::UNITAI_CARRIER_AIR
        | UnitAIType::UNITAI_MISSILE_AIR => 0,
        UnitAIType::UNITAI_ATTACK_CITY_LEMMING => 1,
        // C++ default: FAssert(false)
        _ => 0,
    }
}

/// C++ `CvUnitAI::AI_groupSecondVal`: tie-breaker of the head-unit order (see
/// `ai_group_first_val`).
fn ai_group_second_val(game: &Game, u: UnitId) -> i32 {
    if game.unit_get_domain_type(u) == DomainType::DOMAIN_AIR {
        game.unit_air_base_combat_str(u)
    } else {
        game.unit_base_combat_str(u)
    }
}

/// Stand-in for C++ `CvPlayerAI::AI_getPlotDanger(pPlot, iRange) > 0` (C++ default
/// `bTestMoves = true`) for player `p`. `CvSelectionGroup::doTurn` uses it to force an update
/// of AI-controlled groups that cannot fight (AI bookkeeping) and to cancel the missions of
/// human groups (rules-visible).
///
/// AI: `AI_getPlotDanger` is CvPlayerAI code and is not ported as a player method. This private
/// helper approximates it with the same tests, reduced to "is there any danger" (it stops at the
/// first one, which is all a `> 0` test needs): within `range` plots of `plot` and in its area,
/// a visible enemy unit that can attack -- an enemy of `p`'s team (`isEnemy`) that can attack,
/// is not invisible to that team (`isInvisible(eTeam, false)`), can move or attack into `plot`,
/// and reaches it (`baseMoves()`, +1 when it stands on a valid route, >= the step distance);
/// or, for a non-human `p` and a plot without a city, an adjacent plot (or a route plot two steps
/// away) owned by a team at war with `p`'s (C++ "border danger").
fn ai_plot_danger(game: &Game, p: PlayerId, plot: PlotIdx, range: i32) -> bool {
    let team = game.players[p].team;
    let plot_area = game.map.plots[plot].area;
    let (x, y) = game.map.xy(plot);

    let mut border_danger = 0;

    for dx in -range..=range {
        for dy in -range..=range {
            let Some(loop_plot) = game.plot_xy(x, y, dx, dy) else {
                continue;
            };

            if game.map.plots[loop_plot].area != plot_area {
                continue;
            }

            let (loop_x, loop_y) = game.map.xy(loop_plot);
            let distance = game.step_distance(x, y, loop_x, loop_y);
            if game.at_war_opt(game.plot_get_team(loop_plot), Some(team)) {
                if distance == 1 {
                    border_danger += 1;
                } else if distance == 2 && game.plot_is_route(loop_plot) {
                    border_danger += 1;
                }
            }

            for &loop_unit in &game.map.plots[loop_plot].units {
                if game.unit_is_enemy(loop_unit, Some(team), None)
                    && game.unit_can_attack(loop_unit)
                    && !game.unit_is_invisible(loop_unit, team, true)
                    && game.unit_can_move_or_attack_into(loop_unit, plot, false)
                {
                    let danger_range = game.unit_base_moves(loop_unit) + i32::from(game.plot_is_valid_route(loop_plot, loop_unit));
                    if danger_range >= distance {
                        return true;
                    }
                }
            }
        }
    }

    border_danger > 0 && !game.players[p].human && !game.plot_is_city(plot, false, None)
}

/// Stand-in for C++ `CvPlayerAI::AI_unitValue(eUnit, eUnitAI, NULL) > 0`, which
/// `CvSelectionGroup::mergeIntoGroup` uses as "this UnitAI is valid for this unit type".
///
/// AI: `AI_unitValue` is AI valuation code and is not ported. This keeps the part of its
/// validity test that the unit info states directly: the UnitAI is listed in the unit's
/// `UnitAIs` and not in its `NotUnitAIs`. (C++ also requires the UnitAI's domain to match,
/// accepts unlisted UnitAIs that the unit's abilities suit, and may still value a valid one
/// at 0.)
fn ai_unit_ai_valid(game: &Game, unit_type: UnitType, unit_ai: UnitAIType) -> bool {
    let info = &game.d.units[unit_type];
    info.unit_ai_type[unit_ai] && !info.not_unit_ai_type[unit_ai]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::SelectionGroup;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;
    use crate::unit::{Unit, UnitResetInfo};

    const W: i32 = 6;
    const H: i32 = 4;

    /// A game on a 6x4 map (one land area) with real `Player` objects (not `player_init`ed).
    fn test_game() -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 7));
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

    /// C++ `CvPlayer::addUnit` + `reset`, placed on (x, y) directly (no `init`/`setXY`).
    fn add_unit(g: &mut Game, owner: PlayerId, unit_type: &str, x: i32, y: i32) -> UnitId {
        let d = g.d;
        let t = d.t::<UnitType>(unit_type);
        let id = g.units.next_id();
        let u = g.player_add_unit(owner, Unit::new(id, t, owner, d, &UnitResetInfo::from_info(&d.units[t])));
        g.units[u].x = x;
        g.units[u].y = y;
        let plot = g.map.plot_idx(x, y).unwrap();
        g.map.plots[plot].units.push(u);
        u
    }

    /// A new group of `owner` (`addSelectionGroup` + `init`) holding `units` in this order
    /// (attached directly, bypassing `joinGroup`).
    fn add_group(g: &mut Game, owner: PlayerId, units: &[UnitId]) -> GroupId {
        let id = g.groups.next_id();
        let grp = g.player_add_selection_group(owner, SelectionGroup::new(id, owner));
        for &u in units {
            g.groups[grp].units.push(u);
            g.units[u].group = Some(grp);
        }
        grp
    }

    #[test]
    fn empty_group_queries() {
        let mut g = test_game();
        let grp = add_group(&mut g, PlayerId(0), &[]);
        assert_eq!((g.group_get_x(grp), g.group_get_y(grp)), (INVALID_PLOT_COORD, INVALID_PLOT_COORD));
        assert_eq!(g.group_plot(grp), None);
        assert!(g.group_at_plot(grp, None));
        assert_eq!(g.group_area(grp), None);
        assert_eq!(g.group_get_domain_type(grp), None);
        assert_eq!(g.group_get_head_unit_ai(grp), None);
        assert_eq!((g.group_get_head_owner(grp), g.group_get_head_team(grp)), (None, None));
        assert_eq!(g.group_base_moves(grp), MAX_INT);
        assert!(!g.group_is_busy(grp) && !g.group_is_cargo_busy(grp));
        assert!(!g.group_can_all_move(grp) && !g.group_can_any_move(grp));
        assert!(!g.group_is_full(grp) && !g.group_always_invisible(grp));
        assert!(!g.group_is_invisible(grp, TeamId(1)));
        assert!(!g.group_sentry_alert(grp));
        assert!(!g.group_visibility_range(grp));
        assert_eq!(g.group_count_num_unit_ai_type(grp, None), 0);
    }

    #[test]
    fn head_unit_position_and_owner() {
        let mut g = test_game();
        let p = PlayerId(1);
        let a = add_unit(&mut g, p, "UNIT_WARRIOR", 2, 1);
        let b = add_unit(&mut g, p, "UNIT_WARRIOR", 2, 1);
        g.units[a].ai_type = Some(UnitAIType::UNITAI_ATTACK);
        g.units[b].ai_type = Some(UnitAIType::UNITAI_CITY_DEFENSE);
        let grp = add_group(&mut g, p, &[a, b]);
        let plot = g.map.plot_idx(2, 1).unwrap();

        assert_eq!((g.group_get_x(grp), g.group_get_y(grp)), (2, 1));
        assert!(g.group_at(grp, 2, 1) && g.group_at_plot(grp, Some(plot)));
        assert_eq!(g.group_plot(grp), Some(plot));
        assert_eq!(g.group_area(grp), g.map.plots[plot].area);
        assert_eq!(g.group_get_head_unit_ai(grp), Some(UnitAIType::UNITAI_ATTACK));
        assert_eq!(g.group_get_head_owner(grp), Some(p));
        assert_eq!(g.group_get_head_team(grp), Some(g.players[p].team));
        assert_eq!(g.group_get_team(grp), g.players[p].team);

        assert!(!g.group_is_human(grp));
        assert!(g.group_ai_is_controlled(grp));
        g.players[p].human = true;
        assert!(g.group_is_human(grp) && !g.group_is_automated(grp));
        assert!(!g.group_ai_is_controlled(grp));

        // FfH AI-controlled units make a human group automated (Xienwolf tweak).
        assert!(!g.group_is_ai_control(grp));
        g.units[b].ai_control = 1;
        assert!(g.group_is_ai_control(grp) && g.group_is_automated(grp) && g.group_ai_is_controlled(grp));
    }

    #[test]
    fn kill_removes_the_group() {
        let mut g = test_game();
        let p = PlayerId(0);
        let grp = add_group(&mut g, p, &[]);
        let other = add_group(&mut g, p, &[]);
        g.players[p].group_cycle = vec![other, grp];

        g.group_kill(grp);
        assert!(!g.groups.contains(grp) && g.groups.contains(other));
        assert_eq!(g.players[p].groups, vec![other]);
        assert_eq!(g.players[p].group_cycle, vec![other]);
    }

    #[test]
    fn unit_ai_counts() {
        let mut g = test_game();
        let p = PlayerId(0);
        let units: Vec<UnitId> = (0..3).map(|_| add_unit(&mut g, p, "UNIT_WARRIOR", 1, 1)).collect();
        g.units[units[0]].ai_type = Some(UnitAIType::UNITAI_ATTACK);
        g.units[units[1]].ai_type = Some(UnitAIType::UNITAI_ATTACK);
        g.units[units[2]].ai_type = Some(UnitAIType::UNITAI_EXPLORE);
        let grp = add_group(&mut g, p, &units);

        assert_eq!(g.group_count_num_unit_ai_type(grp, None), 3);
        assert_eq!(g.group_count_num_unit_ai_type(grp, Some(UnitAIType::UNITAI_ATTACK)), 2);
        assert_eq!(g.group_count_num_unit_ai_type(grp, Some(UnitAIType::UNITAI_SETTLE)), 0);
        assert!(!g.group_has_worker(grp));
        g.units[units[2]].ai_type = Some(UnitAIType::UNITAI_WORKER_SEA);
        assert!(g.group_has_worker(grp));
    }

    #[test]
    fn cargo_totals_and_commands() {
        let mut g = test_game();
        let p = PlayerId(0);
        let ship = add_unit(&mut g, p, "UNIT_WARRIOR", 3, 2);
        let passenger = add_unit(&mut g, p, "UNIT_WARRIOR", 3, 2);
        let grp = add_group(&mut g, p, &[ship]);
        let cargo_grp = add_group(&mut g, p, &[passenger]);

        assert_eq!(g.group_get_cargo(grp), 0);
        assert!(!g.group_has_cargo(grp));
        // COMMAND_LOAD needs a unit with free cargo space on the plot.
        assert!(!g.group_can_ever_do_command(cargo_grp, CommandType::COMMAND_LOAD, -1, -1, false));
        g.units[ship].cargo_capacity = 2;
        assert!(g.group_can_ever_do_command(cargo_grp, CommandType::COMMAND_LOAD, -1, -1, false));

        g.units[ship].cargo = 1;
        g.units[passenger].transport_unit = Some(ship);
        assert_eq!(g.group_get_cargo(grp), 1);
        assert!(g.group_has_cargo(grp));

        // COMMAND_UNLOAD needs a loaded unit in the group; other commands pass.
        assert!(g.group_can_ever_do_command(cargo_grp, CommandType::COMMAND_UNLOAD, -1, -1, false));
        assert!(!g.group_can_ever_do_command(grp, CommandType::COMMAND_UNLOAD, -1, -1, false));
        assert!(g.group_can_ever_do_command(grp, CommandType::COMMAND_UPGRADE, -1, -1, false));

        // A busy group can do no command.
        g.groups[cargo_grp].mission_timer = 1;
        assert!(g.group_is_busy(cargo_grp));
        assert!(!g.group_can_do_command(cargo_grp, CommandType::COMMAND_UNLOAD, -1, -1, false));
    }

    #[test]
    fn cancelling_automation_reaches_cargo_groups() {
        let mut g = test_game();
        let p = PlayerId(0);
        let ship = add_unit(&mut g, p, "UNIT_WARRIOR", 3, 2);
        let passenger = add_unit(&mut g, p, "UNIT_WARRIOR", 3, 2);
        let bystander = add_unit(&mut g, p, "UNIT_WARRIOR", 3, 2);
        g.units[passenger].transport_unit = Some(ship);
        let grp = add_group(&mut g, p, &[ship]);
        let cargo_grp = add_group(&mut g, p, &[passenger]);
        let other_grp = add_group(&mut g, p, &[bystander]);

        let explore = Some(AutomateType::AUTOMATE_EXPLORE);
        for x in [grp, cargo_grp, other_grp] {
            g.group_set_automate_type(x, explore);
            assert_eq!(g.groups[x].automate_type, explore);
            assert!(g.group_is_automated(x));
        }

        g.group_set_automate_type(grp, None);
        assert_eq!(g.groups[grp].automate_type, None);
        assert_eq!(g.groups[cargo_grp].automate_type, None, "the transported group stops too");
        assert_eq!(g.groups[other_grp].automate_type, explore);
    }

    #[test]
    fn mission_ai_and_group_flags() {
        let mut g = test_game();
        let p = PlayerId(0);
        let a = add_unit(&mut g, p, "UNIT_WARRIOR", 0, 0);
        let b = add_unit(&mut g, p, "UNIT_WARRIOR", 0, 0);
        let grp = add_group(&mut g, p, &[a, b]);
        let mission_ai = MissionAIType(1);
        let plot = g.map.plot_idx(4, 3).unwrap();

        g.group_ai_set_mission_ai(grp, Some(mission_ai), Some(plot), Some(b));
        assert_eq!(g.groups[grp].ai_mission_ai_type, Some(mission_ai));
        assert_eq!((g.groups[grp].ai_mission_ai_x, g.groups[grp].ai_mission_ai_y), (4, 3));
        assert_eq!(g.group_ai_get_mission_ai_plot(grp), Some(plot));
        assert_eq!(g.group_ai_get_mission_ai_unit(grp), Some(b));

        g.group_ai_set_mission_ai(grp, None, None, None);
        assert_eq!(g.groups[grp].ai_mission_ai_type, None);
        assert_eq!((g.groups[grp].ai_mission_ai_x, g.groups[grp].ai_mission_ai_y), (INVALID_PLOT_COORD, INVALID_PLOT_COORD));
        assert_eq!(g.group_ai_get_mission_ai_plot(grp), None);
        assert_eq!(g.group_ai_get_mission_ai_unit(grp), None);

        assert_eq!(g.group_ai_get_groupflag(grp), GROUPFLAG_NONE);
        g.group_ai_set_groupflag(grp, GROUPFLAG_CONQUEST);
        assert_eq!((g.units[a].ai_groupflag, g.units[b].ai_groupflag), (GROUPFLAG_CONQUEST, GROUPFLAG_CONQUEST));
        g.units[a].ai_groupflag = GROUPFLAG_NONE;
        assert_eq!(g.group_ai_get_groupflag(grp), GROUPFLAG_NONE, "the head unit's flag");
    }

    #[test]
    fn group_first_val_order_keys() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 0, 0);

        // NO_UNITAI (a unit grouped during `init`) -> 0.
        assert_eq!(ai_group_first_val(&g, u), 0);
        for (unit_ai, value) in [
            (UnitAIType::UNITAI_SETTLE, 21),
            (UnitAIType::UNITAI_WORKER, 20),
            (UnitAIType::UNITAI_HERO, 100),
            (UnitAIType::UNITAI_MAGE, 3),
            (UnitAIType::UNITAI_EXPLORE, 8),
            (UnitAIType::UNITAI_ASSAULT_SEA, 11),
            (UnitAIType::UNITAI_ATTACK_AIR, 0),
            (UnitAIType::UNITAI_ATTACK_CITY_LEMMING, 1),
        ] {
            g.units[u].ai_type = Some(unit_ai);
            assert_eq!(ai_group_first_val(&g, u), value, "{unit_ai:?}");
        }

        // BETTER AI: conquest stacks, then summons (duration) override the UnitAI.
        g.units[u].ai_groupflag = GROUPFLAG_CONQUEST;
        assert_eq!(ai_group_first_val(&g, u), 25);
        g.units[u].duration = 3;
        assert_eq!(ai_group_first_val(&g, u), 1);
    }

    #[test]
    fn add_unit_to_empty_group() {
        let mut g = test_game();
        let p = PlayerId(0);
        let u = add_unit(&mut g, p, "UNIT_WARRIOR", 1, 1);
        g.units[u].ai_type = Some(UnitAIType::UNITAI_ATTACK);
        let grp = add_group(&mut g, p, &[]);

        // A group about to be split takes no unit; another player's group neither.
        g.groups[grp].ai_force_separate = true;
        assert!(!g.group_add_unit(grp, u, false));
        g.groups[grp].ai_force_separate = false;
        let foreign = add_group(&mut g, PlayerId(1), &[]);
        assert!(!g.group_add_unit(foreign, u, false));
        assert!(g.groups[foreign].units.is_empty());

        assert!(g.group_add_unit(grp, u, false));
        assert_eq!(g.groups[grp].units, vec![u]);
        assert!(g.group_is_head_unit(grp, u));

        // Off the map (as during `init`), the join check gets no plot.
        let v = add_unit(&mut g, p, "UNIT_WARRIOR", 1, 1);
        g.units[v].x = INVALID_PLOT_COORD;
        g.units[v].y = INVALID_PLOT_COORD;
        let new_grp = add_group(&mut g, p, &[]);
        assert!(g.group_add_unit(new_grp, v, true));
        assert_eq!(g.groups[new_grp].units, vec![v]);
    }

    #[test]
    fn split_group_trivial_cases() {
        let mut g = test_game();
        let p = PlayerId(0);
        let a = add_unit(&mut g, p, "UNIT_WARRIOR", 0, 0);
        let b = add_unit(&mut g, p, "UNIT_WARRIOR", 0, 0);
        let grp = add_group(&mut g, p, &[a, b]);

        assert_eq!(g.group_split_group(grp, 0, None), (None, None));
        assert_eq!(g.group_split_group(grp, -1, None), (None, None));
        assert_eq!(g.group_split_group(grp, 2, None), (Some(grp), None));
        assert_eq!(g.group_split_group(grp, 5, Some(b)), (Some(grp), None));
        assert_eq!(g.groups[grp].units, vec![a, b]);
    }

    #[test]
    fn plot_danger_from_borders() {
        let mut g = test_game();
        let d = g.d;
        let p = PlayerId(0);
        let enemy = PlayerId(1);
        let (team, enemy_team) = (g.players[p].team, g.players[enemy].team);
        let plot = g.map.plot_idx(2, 1).unwrap();

        // No units and no enemy territory around.
        assert!(!ai_plot_danger(&g, p, plot, 2));

        // Territory of a team at war two plots away counts only with a route on it.
        let far = g.map.plot_idx(4, 1).unwrap();
        g.map.plots[far].owner = Some(enemy);
        assert!(!ai_plot_danger(&g, p, plot, 2), "not at war");
        g.teams[team].at_war[enemy_team] = true;
        g.teams[enemy_team].at_war[team] = true;
        assert!(!ai_plot_danger(&g, p, plot, 2), "distance 2 without a route");
        g.map.plots[far].route_type = Some(d.t::<RouteType>("ROUTE_ROAD"));
        assert!(ai_plot_danger(&g, p, plot, 2));
        assert!(!ai_plot_danger(&g, p, plot, 1), "out of range");

        // Adjacent enemy territory.
        g.map.plots[far].route_type = None;
        let near = g.map.plot_idx(3, 2).unwrap();
        g.map.plots[near].owner = Some(enemy);
        assert!(ai_plot_danger(&g, p, plot, 2));
        assert!(ai_plot_danger(&g, p, plot, 1));

        // Border danger is ignored for human players, and for plots in another area.
        g.players[p].human = true;
        assert!(!ai_plot_danger(&g, p, plot, 2));
        g.players[p].human = false;
        let other_area = g.map_add_area(false);
        g.map.plots[near].area = Some(other_area);
        assert!(!ai_plot_danger(&g, p, plot, 1));
    }

    #[test]
    fn unit_ai_validity_from_unit_info() {
        let g = test_game();
        let d = g.d;
        let warrior = d.t::<UnitType>("UNIT_WARRIOR");
        let info = &d.units[warrior];
        for unit_ai in (0..UnitAIType::NUM).map(UnitAIType::from_usize) {
            assert_eq!(ai_unit_ai_valid(&g, warrior, unit_ai), info.unit_ai_type[unit_ai] && !info.not_unit_ai_type[unit_ai]);
        }
        assert!((0..UnitAIType::NUM).map(UnitAIType::from_usize).any(|a| ai_unit_ai_valid(&g, warrior, a)));
    }
}
