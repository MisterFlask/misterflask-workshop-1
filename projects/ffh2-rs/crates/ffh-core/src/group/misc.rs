//! CvSelectionGroup: lifetime (`kill`, `doTurn`), activity and automation, busy/ready
//! checks, commands, unit membership (add/remove/merge/split, head unit, unit index),
//! owner/team, position (plot, x/y, area, domain), aggregates over the group's units
//! (moves, cargo, fight/defend, invisibility, unit AI counts), and the
//! `CvSelectionGroupAI` mission-AI / group-flag bookkeeping.

#![allow(unused_variables)]
#![allow(unused_imports)]
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Lifetime, turn
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::kill`: remove the (empty) group from its owner
    /// (`removeGroupCycle`, `deleteSelectionGroup`).
    pub fn group_kill(&mut self, g: GroupId) {
        todo!("CvSelectionGroup::kill")
    }

    /// C++ `CvSelectionGroup::doTurn`: units' turns, waking (hold / healed / sentry alert),
    /// force-update of AI-controlled groups, danger checks for human missions.
    pub fn group_do_turn(&mut self, g: GroupId) {
        todo!("CvSelectionGroup::doTurn")
    }

    /// C++ `CvSelectionGroup::sentryAlert` (protected; FFH "Alertness": uses the unit with the
    /// best visibility range): an enemy unit is visible within sentry range.
    pub fn group_sentry_alert(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::sentryAlert")
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
        todo!("CvSelectionGroup::setAutomateType")
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
        todo!("CvSelectionGroup::isBusy")
    }

    /// C++ `CvSelectionGroup::isCargoBusy`: a unit carried by this group is busy.
    pub fn group_is_cargo_busy(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::isCargoBusy")
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
        todo!("CvSelectionGroup::canDoCommand")
    }

    /// C++ `CvSelectionGroup::canEverDoCommand` (C++ `bUseCache` UI cache not ported: the
    /// COMMAND_UPGRADE cache branch is never taken).
    pub fn group_can_ever_do_command(&self, g: GroupId, command: CommandType, data1: i32, data2: i32, test_visible: bool) -> bool {
        todo!("CvSelectionGroup::canEverDoCommand")
    }

    // ------------------------------------------------------------------
    // Owner, control
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::isHuman` (owner is human; non-const in C++, read-only).
    pub fn group_is_human(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::isHuman")
    }

    /// C++ `CvSelectionGroup::getTeam` (the owner's team).
    pub fn group_get_team(&self, g: GroupId) -> TeamId {
        todo!("CvSelectionGroup::getTeam")
    }

    /// C++ `CvSelectionGroup::isAIControl` (FfH: any unit is AI-controlled, i.e. has a
    /// promotion with `bAIControl`).
    pub fn group_is_ai_control(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::isAIControl")
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
        todo!("CvSelectionGroup::addUnit")
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
        todo!("CvSelectionGroup::mergeIntoGroup")
    }

    /// C++ `CvSelectionGroup::splitGroup(iSplitSize, pNewHeadUnit, ppOtherGroup)`: returns
    /// `(group of at most split_size units, remainder group (C++ *ppOtherGroup))`. Returns
    /// `(Some(g), None)` if the group is already small enough, `(None, None)` on bad input.
    pub fn group_split_group(&mut self, g: GroupId, split_size: i32, new_head_unit: Option<UnitId>) -> (Option<GroupId>, Option<GroupId>) {
        todo!("CvSelectionGroup::splitGroup")
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
        todo!("CvSelectionGroup::getHeadUnitAI")
    }

    /// C++ `CvSelectionGroup::getHeadOwner`.
    pub fn group_get_head_owner(&self, g: GroupId) -> Option<PlayerId> {
        todo!("CvSelectionGroup::getHeadOwner")
    }

    /// C++ `CvSelectionGroup::getHeadTeam`.
    pub fn group_get_head_team(&self, g: GroupId) -> Option<TeamId> {
        todo!("CvSelectionGroup::getHeadTeam")
    }

    // ------------------------------------------------------------------
    // Position (all from the head unit)
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::getX` (INVALID_PLOT_COORD for an empty group).
    pub fn group_get_x(&self, g: GroupId) -> i32 {
        todo!("CvSelectionGroup::getX")
    }

    /// C++ `CvSelectionGroup::getY` (INVALID_PLOT_COORD for an empty group).
    pub fn group_get_y(&self, g: GroupId) -> i32 {
        todo!("CvSelectionGroup::getY")
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
        todo!("CvSelectionGroup::plot")
    }

    /// C++ `CvSelectionGroup::area` and `getArea` (the area id; C++ `getArea` returns 0
    /// (`NULL`) for an empty group, here `None`).
    pub fn group_area(&self, g: GroupId) -> Option<AreaId> {
        todo!("CvSelectionGroup::area + CvSelectionGroup::getArea")
    }

    /// C++ `CvSelectionGroup::getDomainType` (`None` = NO_DOMAIN for an empty group).
    pub fn group_get_domain_type(&self, g: GroupId) -> Option<DomainType> {
        todo!("CvSelectionGroup::getDomainType")
    }

    // ------------------------------------------------------------------
    // Aggregates over the group's units
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::baseMoves` (minimum over units; MAX_INT for an empty group).
    pub fn group_base_moves(&self, g: GroupId) -> i32 {
        todo!("CvSelectionGroup::baseMoves")
    }

    /// C++ `CvSelectionGroup::isFull` (cargo space; special-cargo two-pass rule).
    pub fn group_is_full(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::isFull")
    }

    /// C++ `CvSelectionGroup::hasCargo`.
    pub fn group_has_cargo(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::hasCargo")
    }

    /// C++ `CvSelectionGroup::getCargo` (total cargo carried).
    pub fn group_get_cargo(&self, g: GroupId) -> i32 {
        todo!("CvSelectionGroup::getCargo")
    }

    /// C++ `CvSelectionGroup::canAllMove` (false for an empty group).
    pub fn group_can_all_move(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::canAllMove")
    }

    /// C++ `CvSelectionGroup::canAnyMove`.
    pub fn group_can_any_move(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::canAnyMove")
    }

    /// C++ `CvSelectionGroup::hasMoved`.
    pub fn group_has_moved(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::hasMoved")
    }

    /// C++ `CvSelectionGroup::canFight`.
    pub fn group_can_fight(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::canFight")
    }

    /// C++ `CvSelectionGroup::canDefend`.
    pub fn group_can_defend(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::canDefend")
    }

    /// C++ `CvSelectionGroup::visibilityRange`. C++ quirk: declared `bool`, so the computed
    /// maximum range is returned as `range != 0`.
    pub fn group_visibility_range(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::visibilityRange")
    }

    /// C++ `CvSelectionGroup::unloadAll`.
    pub fn group_unload_all(&mut self, g: GroupId) {
        todo!("CvSelectionGroup::unloadAll")
    }

    /// C++ `CvSelectionGroup::alwaysInvisible` (false for an empty group).
    pub fn group_always_invisible(&self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::alwaysInvisible")
    }

    /// C++ `CvSelectionGroup::isInvisible` (all units invisible to `team`; false if empty).
    pub fn group_is_invisible(&self, g: GroupId, team: TeamId) -> bool {
        todo!("CvSelectionGroup::isInvisible")
    }

    /// C++ `CvSelectionGroup::countNumUnitAIType` (`unit_ai = None` = NO_UNITAI counts all).
    pub fn group_count_num_unit_ai_type(&self, g: GroupId, unit_ai: Option<UnitAIType>) -> i32 {
        todo!("CvSelectionGroup::countNumUnitAIType")
    }

    /// C++ `CvSelectionGroup::hasWorker`.
    pub fn group_has_worker(&self, g: GroupId) -> bool {
        self.group_count_num_unit_ai_type(g, Some(UnitAIType::UNITAI_WORKER)) > 0
            || self.group_count_num_unit_ai_type(g, Some(UnitAIType::UNITAI_WORKER_SEA)) > 0
    }

    /// C++ `CvSelectionGroup::setBlockading` (each unit's `setBlockading`, which updates plunder).
    pub fn group_set_blockading(&mut self, g: GroupId, start: bool) {
        todo!("CvSelectionGroup::setBlockading")
    }

    // ------------------------------------------------------------------
    // CvSelectionGroupAI bookkeeping read/written by rules code
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroupAI::AI_getMissionAIPlot` (`plotSorenINLINE(m_iMissionAIX, m_iMissionAIY)`).
    pub fn group_ai_get_mission_ai_plot(&self, g: GroupId) -> Option<PlotIdx> {
        todo!("CvSelectionGroupAI::AI_getMissionAIPlot")
    }

    /// C++ `CvSelectionGroupAI::AI_setMissionAI` (stores the plot as `ai_mission_ai_x/y`).
    pub fn group_ai_set_mission_ai(&mut self, g: GroupId, mission_ai: Option<MissionAIType>, plot: Option<PlotIdx>, unit: Option<UnitId>) {
        todo!("CvSelectionGroupAI::AI_setMissionAI")
    }

    /// C++ `CvSelectionGroupAI::AI_getMissionAIUnit` (`getUnit(m_missionAIUnit)`: None once the
    /// unit is gone).
    pub fn group_ai_get_mission_ai_unit(&self, g: GroupId) -> Option<UnitId> {
        self.groups[g].ai_mission_ai_unit.filter(|&u| self.units.contains(u))
    }

    /// C++ `CvSelectionGroupAI::AI_getGroupflag` (FFH BETTER AI: the head unit's group flag,
    /// `GROUPFLAG_*` in `defs.rs`).
    pub fn group_ai_get_groupflag(&self, g: GroupId) -> i32 {
        todo!("CvSelectionGroupAI::AI_getGroupflag")
    }

    /// C++ `CvSelectionGroupAI::AI_setGroupflag` (sets the flag on every unit).
    pub fn group_ai_set_groupflag(&mut self, g: GroupId, flag: i32) {
        todo!("CvSelectionGroupAI::AI_setGroupflag")
    }
}
