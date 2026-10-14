//! CvSelectionGroup: mission queue and mission execution (`pushMission`, `startMission`,
//! `continueMission`, `canStartMission`, `autoMission`), mission timers (`updateTimers`,
//! `updateMission`, `updateMissionTimer`) and `doDelayedDeath`.

#![allow(unused_variables)]
#![allow(unused_imports)]
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Timers
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::updateTimers`: advance combat of a fighting unit, else
    /// `updateMission`; then `doDelayedDeath`.
    pub fn group_update_timers(&mut self, g: GroupId) {
        todo!("CvSelectionGroup::updateTimers")
    }

    /// C++ `CvSelectionGroup::doDelayedDeath`: returns true if the group was killed.
    pub fn group_do_delayed_death(&mut self, g: GroupId) -> bool {
        todo!("CvSelectionGroup::doDelayedDeath")
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
        todo!("CvSelectionGroup::updateMissionTimer")
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
        todo!("CvSelectionGroup::pushMission")
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
        todo!("CvSelectionGroup::autoMission")
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
        todo!("CvSelectionGroup::canStartMission")
    }

    /// C++ `CvSelectionGroup::startMission`.
    pub fn group_start_mission(&mut self, g: GroupId) {
        todo!("CvSelectionGroup::startMission")
    }

    /// C++ `CvSelectionGroup::continueMission` (C++ default `iSteps = 0`; recursive).
    pub fn group_continue_mission(&mut self, g: GroupId, steps: i32) {
        todo!("CvSelectionGroup::continueMission")
    }
}
