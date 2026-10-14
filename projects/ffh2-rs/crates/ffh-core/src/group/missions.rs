//! CvSelectionGroup: mission queue and mission execution (`pushMission`, `startMission`,
//! `continueMission`, `canStartMission`, `autoMission`), mission timers (`updateTimers`,
//! `updateMission`, `updateMissionTimer`) and `doDelayedDeath`.
//!
//! Headless, there is no active player: the active-player UI branches (selection cycling,
//! sounds, camera) never run. Mission timers follow the C++ rules (`updateMissionTimer`): they
//! stay 0 unless a human player watches the plot (`isVisibleToWatchingHuman`) and the group is
//! human or a human shows its moves (`showMoves`).
//!
//! C++ reads `headMissionQueueNode()->m_data` afresh at every use; the ports re-read the head
//! mission wherever the queue can have changed in between (unit actions, moves, combat).

use super::movement::{head_mission, is_group_member, mission_target_unit, num_units, units_snapshot};
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::path::ai_get_plot_danger;
use crate::structs::*;
use ffh_data::*;

/// C++ `CvSelectionGroup::showMoves`: a human player watches this group's moves (friendly or
/// enemy moves option). Only `updateMissionTimer` uses it in rules code.
fn show_moves(g: &Game, grp: GroupId) -> bool {
    // isMPOption(MPOPTION_SIMULTANEOUS_TURNS) is never set (no multiplayer).
    if g.is_simultaneous_team_turns() {
        return false;
    }

    for p in g.civ_player_ids() {
        let player = &g.players[p];
        if player.alive && player.human {
            if let Some(head) = g.group_get_head_unit(grp) {
                if g.unit_is_enemy(head, Some(player.team), None) {
                    if player.options[PlayerOptionType::PLAYEROPTION_SHOW_ENEMY_MOVES] {
                        return true;
                    }
                } else if player.options[PlayerOptionType::PLAYEROPTION_SHOW_FRIENDLY_MOVES] {
                    return true;
                }
            }
        }
    }

    false
}

/// `(BuildTypes)iData1` of a build mission (C++ only asserts that it is a valid build).
fn mission_build(g: &Game, data1: i32) -> Option<BuildType> {
    opt_from_i32::<BuildType>(data1).filter(|b| b.idx() < g.d.builds.len())
}

/// `iData1` of `MISSION_LEAD`: the unit to lead (C++ `iUnitId`, -1 = any unit on the plot).
fn mission_lead_unit(data1: i32) -> Option<UnitId> {
    if data1 < 0 {
        None
    } else {
        Some(UnitId(data1 as u32))
    }
}

impl Game {
    // ------------------------------------------------------------------
    // Timers
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::updateTimers`: advance combat of a fighting unit, else
    /// `updateMission`; then `doDelayedDeath`.
    pub fn group_update_timers(&mut self, g: GroupId) {
        debug_assert!(self.groups.contains(g));

        if num_units(self, g) > 0 {
            let mut combat = false;

            for u in units_snapshot(self, g) {
                if !is_group_member(self, u, g) {
                    continue;
                }

                if self.unit_is_combat(u) {
                    if self.units[u].air_combat {
                        self.unit_update_air_combat(u);
                    } else {
                        self.unit_update_combat(u);
                    }

                    combat = true;
                    break;
                }
            }

            if !combat && self.groups.contains(g) {
                self.group_update_mission(g);
            }
        }

        if self.groups.contains(g) {
            self.group_do_delayed_death(g);
        }
    }

    /// C++ `CvSelectionGroup::doDelayedDeath`: returns true if the group was killed.
    pub fn group_do_delayed_death(&mut self, g: GroupId) -> bool {
        if self.group_is_busy(g) {
            return false;
        }

        for u in units_snapshot(self, g) {
            if !is_group_member(self, u, g) {
                continue;
            }

            self.unit_do_delayed_death(u);
        }

        if !self.groups.contains(g) {
            // Deleted meanwhile (C++ would hold a dangling pointer).
            return true;
        }

        if num_units(self, g) == 0 {
            self.group_kill(g);
            return true;
        }

        false
    }

    /// C++ `CvSelectionGroup::updateMission`: count the mission timer down and continue the
    /// mission when it expires.
    pub fn group_update_mission(&mut self, g: GroupId) {
        if self.groups[g].mission_timer > 0 {
            // changeMissionTimer(-1)
            self.groups[g].mission_timer -= 1;

            if self.groups[g].mission_timer == 0 {
                if self.groups[g].activity_type == ActivityType::ACTIVITY_MISSION {
                    self.group_continue_mission(g, 0);
                } else {
                    // UI: changeCycleSelectionCounter(1) for the active player when nothing is selected.
                }
            }
        }
    }

    /// C++ `CvSelectionGroup::updateMissionTimer` (C++ default `iSteps = 0`): animation delay
    /// before the mission continues (0 for AI groups whose moves are not shown).
    pub fn group_update_mission_timer(&mut self, g: GroupId, steps: i32) {
        let time = if !self.group_is_human(g) && !show_moves(self, g) {
            0
        } else if let Some(head) = head_mission(self, g) {
            let mut time = self.d.missions[head.mission_type].time;

            if head.mission_type == MissionType::MISSION_MOVE_TO
                || head.mission_type == MissionType::MISSION_ROUTE_TO
                || head.mission_type == MissionType::MISSION_MOVE_TO_UNIT
            {
                let target_plot = if head.mission_type == MissionType::MISSION_MOVE_TO_UNIT {
                    mission_target_unit(self, head.data1, head.data2).and_then(|target| self.unit_plot_opt(target))
                } else {
                    self.map.plot_xy(head.data1, head.data2)
                };

                if self.group_at_plot(g, target_plot) {
                    time += steps;
                } else {
                    time = time.min(2);
                }
            }

            // GET_PLAYER(isNetworkMultiPlayer() ? owner : activePlayer).isOption(PLAYEROPTION_QUICK_MOVES):
            // headless there is no active player, so no quick moves.
            let option_player = if self.is_network_multi_player() { Some(self.groups[g].owner) } else { self.active_player };
            let quick_moves = option_player.is_some_and(|p| self.players[p].options[PlayerOptionType::PLAYEROPTION_QUICK_MOVES]);
            if self.group_is_human(g) && (self.group_is_automated(g) || quick_moves) {
                time = time.min(1);
            }

            time
        } else {
            0
        };

        // setMissionTimer(iTime)
        debug_assert!(time >= 0);
        self.groups[g].mission_timer = time;
    }

    // ------------------------------------------------------------------
    // Mission queue
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::pushMission` (C++ defaults: `iData1 = -1, iData2 = -1,
    /// iFlags = 0, bAppend = false, bManual = false, eMissionAI = NO_MISSIONAI,
    /// pMissionAIPlot = NULL, pMissionAIUnit = NULL`).
    #[allow(clippy::too_many_arguments)]
    pub fn group_push_mission(
        &mut self,
        g: GroupId,
        mission: MissionType,
        data1: i32,
        data2: i32,
        flags: i32,
        append: bool,
        manual: bool,
        mission_ai: Option<MissionAIType>,
        mission_ai_plot: Option<PlotIdx>,
        mission_ai_unit: Option<UnitId>,
    ) {
        if !append {
            if self.group_is_busy(g) {
                return;
            }

            self.group_clear_mission_queue(g);
        }

        if manual {
            self.group_set_automate_type(g, None);
        }

        let data = MissionData { mission_type: mission, data1, data2, flags, push_turn: self.game_turn() };

        self.group_ai_set_mission_ai(g, mission_ai, mission_ai_plot, mission_ai_unit);

        self.group_insert_at_end_mission_queue(g, data, !append);

        if manual {
            // UI: action sound and setHasMovedUnit for the active player.

            if !self.groups.contains(g) {
                return;
            }

            self.ev_selection_group_push_mission(g, mission);

            if self.groups.contains(g) {
                self.group_do_delayed_death(g);
            }
        }
    }

    /// C++ `CvSelectionGroup::popMission`: delete the last queued mission.
    pub fn group_pop_mission(&mut self, g: GroupId) {
        let len = self.groups[g].mission_queue.len();
        if len > 0 {
            self.group_delete_mission_queue_node(g, len - 1);
        }
    }

    /// C++ `CvSelectionGroup::autoMission`: start or continue the head mission (human groups
    /// with visible units clear their queue when in danger).
    pub fn group_auto_mission(&mut self, g: GroupId) {
        if num_units(self, g) > 0 {
            if head_mission(self, g).is_some() {
                if !self.group_is_busy(g) {
                    let mut visible_human = false;
                    if self.group_is_human(g) {
                        visible_human = self.groups[g].units.iter().any(|&u| !self.unit_always_invisible(u));
                    }

                    let owner = self.groups[g].owner;
                    if visible_human && self.group_plot(g).is_some_and(|plot| ai_get_plot_danger(self, owner, plot, 1, true) > 0) {
                        self.group_clear_mission_queue(g);
                    } else if self.groups[g].activity_type == ActivityType::ACTIVITY_MISSION {
                        self.group_continue_mission(g, 0);
                    } else {
                        self.group_start_mission(g);
                    }
                }
            }
        }

        if self.groups.contains(g) {
            self.group_do_delayed_death(g);
        }
    }

    /// C++ `CvSelectionGroup::clearMissionQueue`.
    pub fn group_clear_mission_queue(&mut self, g: GroupId) {
        self.group_deactivate_head_mission(g);

        self.groups[g].mission_queue.clear();

        // UI: Waypoints / SelectionButtons / InfoPane dirty bits for the active player.
    }

    /// C++ `CvSelectionGroup::insertAtEndMissionQueue` (C++ default `bStart = true`).
    pub fn group_insert_at_end_mission_queue(&mut self, g: GroupId, mission: MissionData, start: bool) {
        self.groups[g].mission_queue.push(mission);

        if self.groups[g].mission_queue.len() == 1 && start {
            self.group_activate_head_mission(g);
        }

        // UI: Waypoints / SelectionButtons / InfoPane dirty bits for the active player.
    }

    /// C++ `CvSelectionGroup::deleteMissionQueueNode`, with the node given by its index in
    /// `mission_queue` (the C++ return value, the next node, is the same index afterwards).
    pub fn group_delete_mission_queue_node(&mut self, g: GroupId, index: usize) {
        if index == 0 {
            self.group_deactivate_head_mission(g);
        }

        self.groups[g].mission_queue.remove(index);

        // C++: `if (pNextMissionNode == headMissionQueueNode()) activateHeadMission();` -- true
        // exactly when the head was deleted (including the now-empty queue: NULL == NULL).
        if index == 0 {
            self.group_activate_head_mission(g);
        }

        // UI: Waypoints / SelectionButtons / InfoPane dirty bits for the active player.
    }

    /// C++ `CvSelectionGroup::getMissionType(iNode)`: `None` (C++ -1) if there is no such node.
    pub fn group_get_mission_type(&self, g: GroupId, node: i32) -> Option<MissionType> {
        if node < 0 {
            return None;
        }
        self.groups[g].mission_queue.get(node as usize).map(|m| m.mission_type)
    }

    /// C++ `CvSelectionGroup::getMissionData1(iNode)`: -1 if there is no such node.
    pub fn group_get_mission_data1(&self, g: GroupId, node: i32) -> i32 {
        if node < 0 {
            return -1;
        }
        self.groups[g].mission_queue.get(node as usize).map_or(-1, |m| m.data1)
    }

    /// C++ `CvSelectionGroup::getMissionData2(iNode)`: -1 if there is no such node.
    pub fn group_get_mission_data2(&self, g: GroupId, node: i32) -> i32 {
        if node < 0 {
            return -1;
        }
        self.groups[g].mission_queue.get(node as usize).map_or(-1, |m| m.data2)
    }

    /// C++ `CvSelectionGroup::activateHeadMission` (protected).
    pub fn group_activate_head_mission(&mut self, g: GroupId) {
        if !self.groups[g].mission_queue.is_empty() {
            if !self.group_is_busy(g) {
                self.group_start_mission(g);
            }
        }
    }

    /// C++ `CvSelectionGroup::deactivateHeadMission` (protected).
    pub fn group_deactivate_head_mission(&mut self, g: GroupId) {
        if !self.groups[g].mission_queue.is_empty() {
            if self.groups[g].activity_type == ActivityType::ACTIVITY_MISSION {
                self.group_set_activity_type(g, ActivityType::ACTIVITY_AWAKE);
            }

            // setMissionTimer(0)
            self.groups[g].mission_timer = 0;

            // UI: changeCycleSelectionCounter(1) if the active player's group is selected.
        }
    }

    // ------------------------------------------------------------------
    // Mission execution
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::canStartMission` (`plot = None` means the group's plot; the C++
    /// `bUseCache` UI cache is not ported; non-const in C++, read-only).
    pub fn group_can_start_mission(&self, g: GroupId, mission: MissionType, data1: i32, data2: i32, plot: Option<PlotIdx>, test_visible: bool) -> bool {
        if self.group_is_busy(g) {
            return false;
        }

        // `if (pPlot == NULL) pPlot = plot();` (NULL only for an empty group: the loop below
        // does not run then).
        let Some(plot) = plot.or_else(|| self.group_plot(g)) else {
            return false;
        };

        for &loop_unit in &self.groups[g].units {
            match mission {
                MissionType::MISSION_MOVE_TO => {
                    if !self.plot_at(plot, data1, data2) {
                        return true;
                    }
                }

                MissionType::MISSION_ROUTE_TO => {
                    if !self.plot_at(plot, data1, data2) || self.group_get_best_build_route(g, plot).0.is_some() {
                        return true;
                    }
                }

                MissionType::MISSION_MOVE_TO_UNIT => {
                    debug_assert!(data1 >= 0, "iData1 should be a valid Player");
                    if let Some(target) = mission_target_unit(self, data1, data2) {
                        if !self.unit_at_plot(target, plot) {
                            return true;
                        }
                    }
                }

                MissionType::MISSION_SKIP => {
                    if self.unit_can_hold(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_SLEEP => {
                    if self.unit_can_sleep(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_FORTIFY => {
                    if self.unit_can_fortify(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_AIRPATROL => {
                    if self.unit_can_air_patrol(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_SEAPATROL => {
                    if self.unit_can_sea_patrol(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_HEAL => {
                    if self.unit_can_heal(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_SENTRY => {
                    if self.unit_can_sentry(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_AIRLIFT => {
                    if self.unit_can_airlift_at(loop_unit, plot, data1, data2) {
                        return true;
                    }
                }

                MissionType::MISSION_NUKE => {
                    if self.unit_can_nuke_at(loop_unit, plot, data1, data2) {
                        return true;
                    }
                }

                MissionType::MISSION_RECON => {
                    if self.unit_can_recon_at(loop_unit, plot, data1, data2) {
                        return true;
                    }
                }

                MissionType::MISSION_PARADROP => {
                    if self.unit_can_paradrop_at(loop_unit, plot, data1, data2) {
                        return true;
                    }
                }

                MissionType::MISSION_AIRBOMB => {
                    if self.unit_can_air_bomb_at(loop_unit, plot, data1, data2) {
                        return true;
                    }
                }

                MissionType::MISSION_BOMBARD => {
                    if self.unit_can_bombard(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_RANGE_ATTACK => {
                    if self.unit_can_range_strike_at(loop_unit, plot, data1, data2) {
                        return true;
                    }
                }

                MissionType::MISSION_PLUNDER => {
                    if self.unit_can_plunder(loop_unit, plot, test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_PILLAGE => {
                    if self.unit_can_pillage(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_SABOTAGE => {
                    if self.unit_can_sabotage(loop_unit, plot, test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_DESTROY => {
                    if self.unit_can_destroy(loop_unit, plot, test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_STEAL_PLANS => {
                    if self.unit_can_steal_plans(loop_unit, plot, test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_FOUND => {
                    if self.unit_can_found(loop_unit, plot, test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_SPREAD => {
                    if self.unit_can_spread(loop_unit, plot, opt_from_i32(data1), test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_SPREAD_CORPORATION => {
                    if self.unit_can_spread_corporation(loop_unit, plot, opt_from_i32(data1), test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_JOIN => {
                    if self.unit_can_join(loop_unit, plot, opt_from_i32(data1)) {
                        return true;
                    }
                }

                MissionType::MISSION_CONSTRUCT => {
                    if self.unit_can_construct(loop_unit, plot, opt_from_i32(data1), test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_DISCOVER => {
                    if self.unit_can_discover(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_HURRY => {
                    if self.unit_can_hurry(loop_unit, plot, test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_TRADE => {
                    if self.unit_can_trade(loop_unit, plot, test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_GREAT_WORK => {
                    if self.unit_can_great_work(loop_unit, plot) {
                        return true;
                    }
                }

                MissionType::MISSION_INFILTRATE => {
                    if self.unit_can_infiltrate(loop_unit, plot, test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_GOLDEN_AGE => {
                    //this means to play the animation only
                    if data1 != -1 {
                        return true;
                    }

                    if self.unit_can_golden_age(loop_unit, plot, test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_BUILD => {
                    debug_assert!(mission_build(self, data1).is_some(), "Invalid Build");
                    if let Some(build) = mission_build(self, data1) {
                        if self.unit_can_build(loop_unit, plot, build, test_visible) {
                            return true;
                        }
                    }
                }

                MissionType::MISSION_LEAD => {
                    if self.unit_can_lead(loop_unit, plot, mission_lead_unit(data1)) != 0 {
                        return true;
                    }
                }

                MissionType::MISSION_ESPIONAGE => {
                    if self.unit_can_espionage(loop_unit, plot, test_visible) {
                        return true;
                    }
                }

                MissionType::MISSION_DIE_ANIMATION => {
                    return false;
                }

                MissionType::MISSION_BEGIN_COMBAT
                | MissionType::MISSION_END_COMBAT
                | MissionType::MISSION_AIRSTRIKE
                | MissionType::MISSION_SURRENDER
                | MissionType::MISSION_IDLE
                | MissionType::MISSION_DIE
                | MissionType::MISSION_DAMAGE
                | MissionType::MISSION_MULTI_SELECT
                | MissionType::MISSION_MULTI_DESELECT => {}

                _ => debug_assert!(false, "canStartMission: unexpected mission {mission:?}"),
            }
        }

        false
    }

    /// C++ `CvSelectionGroup::startMission`.
    pub fn group_start_mission(&mut self, g: GroupId) {
        debug_assert!(!self.group_is_busy(g));
        debug_assert!(!self.groups[g].mission_queue.is_empty());

        let owner = self.groups[g].owner;

        // !isMPOption(MPOPTION_SIMULTANEOUS_TURNS): always (no multiplayer).
        if !self.players[owner].turn_active {
            // UI: changeCycleSelectionCounter(1) for the active player's selected group.
            return;
        }

        if self.group_can_all_move(g) {
            self.group_set_activity_type(g, ActivityType::ACTIVITY_MISSION);
        } else {
            self.group_set_activity_type(g, ActivityType::ACTIVITY_HOLD);
        }

        let mut delete = false;
        let mut action = false;
        let mut nuke = false;

        let Some(head) = head_mission(self, g) else {
            // C++ FAssert(headMissionQueueNode() != NULL) and dereferences it.
            return;
        };

        let group_plot = self.group_plot(g);
        if !self.group_can_start_mission(g, head.mission_type, head.data1, head.data2, group_plot, false) {
            delete = true;
        } else {
            debug_assert!(
                self.players[owner].turn_active || self.players[owner].human,
                "It's expected that either the turn is active for this player or the player is human"
            );

            match head.mission_type {
                MissionType::MISSION_MOVE_TO => {
                    // Alertness (Pep, imported by Xienwolf): prevents annoying accidental attacks
                    // when moving into non-visible tiles.
                    if self.group_is_human(g) {
                        if let Some(target) = self.map.plot_xy(head.data1, head.data2) {
                            let team = self.group_get_team(g);
                            // isVisible(getTeam(), false); isRevealed(getTeam(), true) (debug mode is never on)
                            self.groups[g].last_plot_visible = self.plot_is_visible(target, team);
                            self.groups[g].last_plot_revealed = self.map.plots[target].revealed[team];
                        }
                    }
                }

                MissionType::MISSION_ROUTE_TO | MissionType::MISSION_MOVE_TO_UNIT => {}

                MissionType::MISSION_SKIP => {
                    self.group_set_activity_type(g, ActivityType::ACTIVITY_HOLD);
                    delete = true;
                }

                MissionType::MISSION_SLEEP => {
                    self.group_set_activity_type(g, ActivityType::ACTIVITY_SLEEP);
                    delete = true;
                }

                MissionType::MISSION_FORTIFY => {
                    self.group_set_activity_type(g, ActivityType::ACTIVITY_SLEEP);
                    delete = true;
                }

                MissionType::MISSION_PLUNDER => {
                    self.group_set_activity_type(g, ActivityType::ACTIVITY_PLUNDER);
                    delete = true;
                }

                MissionType::MISSION_AIRPATROL => {
                    self.group_set_activity_type(g, ActivityType::ACTIVITY_INTERCEPT);
                    delete = true;
                }

                MissionType::MISSION_SEAPATROL => {
                    self.group_set_activity_type(g, ActivityType::ACTIVITY_PATROL);
                    delete = true;
                }

                MissionType::MISSION_HEAL => {
                    self.group_set_activity_type(g, ActivityType::ACTIVITY_HEAL);
                    delete = true;
                }

                MissionType::MISSION_SENTRY => {
                    self.group_set_activity_type(g, ActivityType::ACTIVITY_SENTRY);
                    delete = true;
                }

                MissionType::MISSION_AIRLIFT
                | MissionType::MISSION_NUKE
                | MissionType::MISSION_RECON
                | MissionType::MISSION_PARADROP
                | MissionType::MISSION_AIRBOMB
                | MissionType::MISSION_BOMBARD
                | MissionType::MISSION_RANGE_ATTACK
                | MissionType::MISSION_PILLAGE
                | MissionType::MISSION_SABOTAGE
                | MissionType::MISSION_DESTROY
                | MissionType::MISSION_STEAL_PLANS
                | MissionType::MISSION_FOUND
                | MissionType::MISSION_SPREAD
                | MissionType::MISSION_SPREAD_CORPORATION
                | MissionType::MISSION_JOIN
                | MissionType::MISSION_CONSTRUCT
                | MissionType::MISSION_DISCOVER
                | MissionType::MISSION_HURRY
                | MissionType::MISSION_TRADE
                | MissionType::MISSION_GREAT_WORK
                | MissionType::MISSION_INFILTRATE
                | MissionType::MISSION_GOLDEN_AGE
                | MissionType::MISSION_BUILD
                | MissionType::MISSION_LEAD
                | MissionType::MISSION_ESPIONAGE
                | MissionType::MISSION_DIE_ANIMATION => {}

                _ => debug_assert!(false, "startMission: unexpected mission {:?}", head.mission_type),
            }

            // UI: NotifyEntity(mission) for sleep/fortify/plunder/heal/sentry (bNotify).

            for u in units_snapshot(self, g) {
                if !is_group_member(self, u, g) {
                    continue;
                }

                if self.unit_can_move(u) {
                    // C++ reads headMissionQueueNode() for every unit (non-NULL: checked below).
                    let Some(mission) = head_mission(self, g) else {
                        break;
                    };
                    let (data1, data2) = (mission.data1, mission.data2);

                    // MISSION_ESPIONAGE: `pUnitNode = NULL`, one unit at a time.
                    let mut last_unit = false;

                    match mission.mission_type {
                        MissionType::MISSION_MOVE_TO
                        | MissionType::MISSION_ROUTE_TO
                        | MissionType::MISSION_MOVE_TO_UNIT
                        | MissionType::MISSION_SKIP
                        | MissionType::MISSION_SLEEP
                        | MissionType::MISSION_FORTIFY
                        | MissionType::MISSION_AIRPATROL
                        | MissionType::MISSION_SEAPATROL
                        | MissionType::MISSION_HEAL
                        | MissionType::MISSION_SENTRY => {}

                        MissionType::MISSION_AIRLIFT => {
                            if self.unit_airlift(u, data1, data2) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_NUKE => {
                            if self.unit_nuke(u, data1, data2) {
                                action = true;

                                if self.map.plot_xy(data1, data2).is_some_and(|p| self.plot_is_visible_to_watching_human(p)) {
                                    nuke = true;
                                }
                            }
                        }

                        MissionType::MISSION_RECON => {
                            if self.unit_recon(u, data1, data2) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_PARADROP => {
                            if self.unit_paradrop(u, data1, data2) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_AIRBOMB => {
                            if self.unit_air_bomb(u, data1, data2) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_BOMBARD => {
                            if self.unit_bombard(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_RANGE_ATTACK => {
                            if self.unit_range_strike(u, data1, data2) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_PILLAGE => {
                            if self.unit_pillage(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_PLUNDER => {
                            if self.unit_plunder(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_SABOTAGE => {
                            if self.unit_sabotage(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_DESTROY => {
                            if self.unit_destroy(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_STEAL_PLANS => {
                            if self.unit_steal_plans(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_FOUND => {
                            if self.unit_found(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_SPREAD => {
                            if self.unit_spread(u, opt_from_i32(data1)) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_SPREAD_CORPORATION => {
                            if self.unit_spread_corporation(u, opt_from_i32(data1)) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_JOIN => {
                            if self.unit_join(u, opt_from_i32(data1)) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_CONSTRUCT => {
                            if self.unit_construct(u, opt_from_i32(data1)) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_DISCOVER => {
                            if self.unit_discover(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_HURRY => {
                            if self.unit_hurry(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_TRADE => {
                            if self.unit_trade(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_GREAT_WORK => {
                            if self.unit_great_work(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_INFILTRATE => {
                            if self.unit_infiltrate(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_GOLDEN_AGE => {
                            //just play animation, not golden age - JW
                            if data1 != -1 {
                                // UI: the golden age animation (CvMissionDefinition, AddMission,
                                // NotifyEntity).
                                action = true;
                            } else if self.unit_golden_age(u) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_BUILD => {}

                        MissionType::MISSION_LEAD => {
                            if self.unit_lead(u, mission_lead_unit(data1)) {
                                action = true;
                            }
                        }

                        MissionType::MISSION_ESPIONAGE => {
                            if self.unit_espionage(u, opt_from_i32(data1), data2) {
                                action = true;
                            }
                            last_unit = true; // allow one unit at a time to do espionage
                        }

                        MissionType::MISSION_DIE_ANIMATION => {
                            action = true;
                        }

                        _ => debug_assert!(false, "startMission: unexpected mission {:?}", mission.mission_type),
                    }

                    if num_units(self, g) == 0 {
                        break;
                    }

                    if head_mission(self, g).is_none() {
                        break;
                    }

                    if last_unit {
                        break;
                    }
                }
            }
        }

        if num_units(self, g) > 0 && head_mission(self, g).is_some() {
            if action {
                if self.group_is_human(g) {
                    if self.group_plot(g).is_some_and(|p| self.plot_is_visible_to_watching_human(p)) {
                        self.group_update_mission_timer(g, 0);
                    }
                }
            }

            if nuke {
                self.groups[g].mission_timer = self.d.missions[MissionType::MISSION_NUKE].time;
            }

            if !self.group_is_busy(g) {
                if delete {
                    // UI: changeCycleSelectionCounter for the active player's selected group.

                    self.group_delete_mission_queue_node(g, 0);
                } else if self.groups[g].activity_type == ActivityType::ACTIVITY_MISSION {
                    self.group_continue_mission(g, 0);
                }
            }
        }
    }

    /// C++ `CvSelectionGroup::continueMission` (C++ default `iSteps = 0`; recursive in C++: the
    /// tail call `continueMission(iSteps + 1)` is a loop here).
    pub fn group_continue_mission(&mut self, g: GroupId, steps: i32) {
        let mut steps = steps;

        loop {
            debug_assert!(!self.group_is_busy(g));
            debug_assert!(self.groups[g].activity_type == ActivityType::ACTIVITY_MISSION);

            let Some(head) = head_mission(self, g) else {
                // just in case...
                self.group_set_activity_type(g, ActivityType::ACTIVITY_AWAKE);
                return;
            };

            let mut done = false;
            let mut action = false;

            if head.push_turn == self.game_turn() || head.flags & MOVE_THROUGH_ENEMY != 0 {
                if head.mission_type == MissionType::MISSION_MOVE_TO {
                    let (attacked, _failed_already_fighting) = self.group_attack(g, head.data1, head.data2, head.flags);
                    if attacked {
                        done = true;
                    }
                }
            }

            if !self.groups.contains(g) {
                return;
            }

            // extra crash protection, should never happen (but a previous bug in groupAttack was
            // causing a NULL here)
            let Some(head) = head_mission(self, g) else {
                self.group_set_activity_type(g, ActivityType::ACTIVITY_AWAKE);
                return;
            };

            if !done {
                if num_units(self, g) > 0 {
                    if self.group_can_all_move(g) {
                        match head.mission_type {
                            MissionType::MISSION_MOVE_TO => {
                                if self.group_get_domain_type(g) == Some(DomainType::DOMAIN_AIR) {
                                    self.group_path_to(g, head.data1, head.data2, head.flags);
                                    done = true;
                                } else if self.group_path_to(g, head.data1, head.data2, head.flags) {
                                    action = true;

                                    if num_units(self, g) > 0 {
                                        if !self.group_can_all_move(g) {
                                            if let Some(mission) = head_mission(self, g) {
                                                // C++ dereferences plotINLINE(iData1, iData2).
                                                if let Some(plot) = self.map.plot_xy(mission.data1, mission.data2) {
                                                    if self.group_amphib_move(g, plot, mission.flags) {
                                                        action = false;
                                                        done = true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    done = true;
                                }
                            }

                            MissionType::MISSION_ROUTE_TO => {
                                if self.group_road_to(g, head.data1, head.data2, head.flags) {
                                    action = true;
                                } else {
                                    done = true;
                                }
                            }

                            MissionType::MISSION_MOVE_TO_UNIT => 'move_to_unit: {
                                if self.group_get_head_unit_ai(g) == Some(UnitAIType::UNITAI_CITY_DEFENSE) {
                                    if let Some(plot) = self.group_plot(g) {
                                        let our_city = self.map.plots[plot].plot_city.is_some()
                                            && self.plot_get_team(plot) == Some(self.group_get_team(g));
                                        if our_city {
                                            let owner = self.groups[g].owner;
                                            // C++ dereferences getBestDefender(getOwnerINLINE()).
                                            let best_defender =
                                                self.plot_get_best_defender(plot, Some(owner), None, None, false, false, false);
                                            if best_defender.and_then(|u| self.units[u].group) == Some(g) {
                                                action = false;
                                                done = true;
                                                break 'move_to_unit;
                                            }
                                        }
                                    }
                                }

                                let Some(target) = mission_target_unit(self, head.data1, head.data2) else {
                                    done = true;
                                    break 'move_to_unit;
                                };

                                let mission_ai = self.groups[g].ai_mission_ai_type;
                                if mission_ai != Some(MissionAIType::MISSIONAI_SHADOW)
                                    && mission_ai != Some(MissionAIType::MISSIONAI_GROUP)
                                {
                                    let owner = self.groups[g].owner;
                                    if let Some(plot) = self.group_plot(g) {
                                        if !self.plot_is_owned(plot) || self.map.plots[plot].owner == Some(owner) {
                                            let mission_plot = self.units[target]
                                                .group
                                                .filter(|&target_group| self.groups.contains(target_group))
                                                .and_then(|target_group| self.group_ai_get_mission_ai_plot(target_group));
                                            if let Some(mission_plot) = mission_plot {
                                                if let Some(mission_team) = self.plot_get_team(mission_plot) {
                                                    if self.plot_is_owned(mission_plot)
                                                        && self.unit_is_potential_enemy(target, Some(mission_team), Some(mission_plot))
                                                    {
                                                        action = false;
                                                        done = true;
                                                        break 'move_to_unit;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                let (target_x, target_y) = (self.units[target].x, self.units[target].y);
                                if self.group_path_to(g, target_x, target_y, head.flags) {
                                    action = true;
                                } else {
                                    done = true;
                                }
                            }

                            MissionType::MISSION_SKIP
                            | MissionType::MISSION_SLEEP
                            | MissionType::MISSION_FORTIFY
                            | MissionType::MISSION_PLUNDER
                            | MissionType::MISSION_AIRPATROL
                            | MissionType::MISSION_SEAPATROL
                            | MissionType::MISSION_HEAL
                            | MissionType::MISSION_SENTRY => {
                                debug_assert!(false, "continueMission: {:?} is not a continued mission", head.mission_type);
                            }

                            MissionType::MISSION_AIRLIFT
                            | MissionType::MISSION_NUKE
                            | MissionType::MISSION_RECON
                            | MissionType::MISSION_PARADROP
                            | MissionType::MISSION_AIRBOMB
                            | MissionType::MISSION_BOMBARD
                            | MissionType::MISSION_RANGE_ATTACK
                            | MissionType::MISSION_PILLAGE
                            | MissionType::MISSION_SABOTAGE
                            | MissionType::MISSION_DESTROY
                            | MissionType::MISSION_STEAL_PLANS
                            | MissionType::MISSION_FOUND
                            | MissionType::MISSION_SPREAD
                            | MissionType::MISSION_SPREAD_CORPORATION
                            | MissionType::MISSION_JOIN
                            | MissionType::MISSION_CONSTRUCT
                            | MissionType::MISSION_DISCOVER
                            | MissionType::MISSION_HURRY
                            | MissionType::MISSION_TRADE
                            | MissionType::MISSION_GREAT_WORK
                            | MissionType::MISSION_INFILTRATE
                            | MissionType::MISSION_GOLDEN_AGE
                            | MissionType::MISSION_LEAD
                            | MissionType::MISSION_ESPIONAGE
                            | MissionType::MISSION_DIE_ANIMATION => {}

                            MissionType::MISSION_BUILD => match mission_build(self, head.data1) {
                                Some(build) => {
                                    if !self.group_build(g, build) {
                                        done = true;
                                    }
                                }
                                // C++ asserts a valid build (an invalid one would index past the infos).
                                None => done = true,
                            },

                            _ => debug_assert!(false, "continueMission: unexpected mission {:?}", head.mission_type),
                        }
                    }
                }
            }

            if !self.groups.contains(g) {
                return;
            }

            if num_units(self, g) > 0 {
                if let Some(head) = head_mission(self, g) {
                    if !done {
                        match head.mission_type {
                            MissionType::MISSION_MOVE_TO => {
                                if self.group_at(g, head.data1, head.data2) {
                                    done = true;
                                }
                            }

                            MissionType::MISSION_ROUTE_TO => {
                                if self.group_at(g, head.data1, head.data2) {
                                    if let Some(plot) = self.group_plot(g) {
                                        if self.group_get_best_build_route(g, plot).0.is_none() {
                                            done = true;
                                        }
                                    }
                                }
                            }

                            MissionType::MISSION_MOVE_TO_UNIT => {
                                let target = mission_target_unit(self, head.data1, head.data2);
                                if target.map_or(true, |target| self.group_at_plot(g, self.unit_plot_opt(target))) {
                                    done = true;
                                }
                            }

                            MissionType::MISSION_SKIP
                            | MissionType::MISSION_SLEEP
                            | MissionType::MISSION_FORTIFY
                            | MissionType::MISSION_PLUNDER
                            | MissionType::MISSION_AIRPATROL
                            | MissionType::MISSION_SEAPATROL
                            | MissionType::MISSION_HEAL
                            | MissionType::MISSION_SENTRY => {
                                debug_assert!(false, "continueMission: {:?} is not a continued mission", head.mission_type);
                            }

                            MissionType::MISSION_AIRLIFT
                            | MissionType::MISSION_NUKE
                            | MissionType::MISSION_RECON
                            | MissionType::MISSION_PARADROP
                            | MissionType::MISSION_AIRBOMB
                            | MissionType::MISSION_BOMBARD
                            | MissionType::MISSION_RANGE_ATTACK
                            | MissionType::MISSION_PILLAGE
                            | MissionType::MISSION_SABOTAGE
                            | MissionType::MISSION_DESTROY
                            | MissionType::MISSION_STEAL_PLANS
                            | MissionType::MISSION_FOUND
                            | MissionType::MISSION_SPREAD
                            | MissionType::MISSION_SPREAD_CORPORATION
                            | MissionType::MISSION_JOIN
                            | MissionType::MISSION_CONSTRUCT
                            | MissionType::MISSION_DISCOVER
                            | MissionType::MISSION_HURRY
                            | MissionType::MISSION_TRADE
                            | MissionType::MISSION_GREAT_WORK
                            | MissionType::MISSION_INFILTRATE
                            | MissionType::MISSION_GOLDEN_AGE
                            | MissionType::MISSION_LEAD
                            | MissionType::MISSION_ESPIONAGE
                            | MissionType::MISSION_DIE_ANIMATION => {
                                done = true;
                            }

                            MissionType::MISSION_BUILD => {
                                // XXX what happens if two separate worker groups are both building the mine...
                                // (C++: a `plot()->getBuildType()` check is commented out)
                            }

                            _ => debug_assert!(false, "continueMission: unexpected mission {:?}", head.mission_type),
                        }
                    }
                }
            }

            if num_units(self, g) > 0 && head_mission(self, g).is_some() {
                if action {
                    if done || !self.group_can_all_move(g) {
                        if self.group_plot(g).is_some_and(|p| self.plot_is_visible_to_watching_human(p)) {
                            self.group_update_mission_timer(g, steps);

                            // UI: camera lookAt for the active player when `showMoves()`.
                        }
                    }
                }

                if done {
                    if !self.group_is_busy(g) {
                        // UI: changeCycleSelectionCounter for the active player's selected group.

                        // UNOFFICIAL_PATCH 08/04/09 jdog5000 (player interface): human move
                        // missions keep their node while the group cannot move any more and
                        // further missions are queued.
                        let head_type = head_mission(self, g).map(|m| m.mission_type);
                        if !self.group_is_human(g) || head_type != Some(MissionType::MISSION_MOVE_TO) {
                            self.group_delete_mission_queue_node(g, 0);
                        } else if self.group_can_all_move(g) || self.groups[g].mission_queue.len() == 1 {
                            self.group_delete_mission_queue_node(g, 0);
                        }
                    }
                } else if self.group_can_all_move(g) {
                    // continueMission(iSteps + 1)
                    steps += 1;
                    continue;
                } else {
                    // UI: `if (!isBusy())` changeCycleSelectionCounter(1) for the active player.
                }
            }

            return;
        }
    }
}
