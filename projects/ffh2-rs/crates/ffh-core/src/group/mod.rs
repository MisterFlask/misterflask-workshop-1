//! Port of `CvSelectionGroup` (CvSelectionGroup.h / CvSelectionGroup.cpp) plus the
//! rules-visible parts of `CvSelectionGroupAI` (mission AI, force separate, group attack,
//! controlled/declare-war checks, best group attacker).
//!
//! Methods are `impl Game` blocks split by topic (C++ line ranges per file are listed in
//! `docs/porting/team_group_deal.md`):
//! - [`missions`]: mission queue, `pushMission` / `startMission` / `continueMission` /
//!   `canStartMission`, mission timers (`updateTimers`, `updateMission`), `doDelayedDeath`.
//! - [`movement`]: pathing (`generatePath`, path plots, `lastMissionPlot`), `groupMove`,
//!   `groupPathTo`, `groupAttack`, `groupDeclareWar`, amphibious moves, transport,
//!   build/route missions, can-move/enter checks, best group attacker.
//! - [`misc`]: `kill`, `doTurn`, activity/automation, busy/ready checks, commands, unit
//!   membership (add/remove/merge/split, head unit), position, aggregates over the units,
//!   mission-AI / group-flag bookkeeping.
//!
//! Naming: `CvSelectionGroup::groupMove` etc. drop their redundant prefix:
//! `group_move`, `group_attack`, `group_path_to`, `group_road_to`, `group_build`,
//! `group_declare_war`, `group_amphib_move`.
//!
//! Trivial C++ accessors are fields: `getNumUnits()` -> `self.groups[g].units.len()`,
//! `headUnitNode()`/`nextUnitNode()` -> iterate `units`, `getLengthMissionQueue()` ->
//! `mission_queue.len()`, `headMissionQueueNode()` -> `mission_queue.first()`,
//! `getMissionFromQueue(i)` -> `mission_queue.get(i)`, `getMissionTimer()` /
//! `setMissionTimer()` / `changeMissionTimer()` -> `mission_timer`, `AI_getMissionAIType()` ->
//! `ai_mission_ai_type`, `AI_isForceSeparate()` / `AI_makeForceSeparate()` -> `ai_force_separate`.

#![allow(unused_imports)]
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

pub mod misc;
pub mod missions;
pub mod movement;

/// C++ `CvSelectionGroup` (+ `CvSelectionGroupAI` members, prefixed `ai_`).
///
/// The C++ UI action cache (`m_bIsBusyCache`, `m_aDifferentUnitCache`, `setupActionCache`)
/// is not ported; `bUseCache` parameters are dropped.
#[derive(Clone, Debug, Default)]
pub struct SelectionGroup {
    /// C++ `m_iID`.
    pub id: GroupId,
    /// C++ `m_iMissionTimer` (`get/set/changeMissionTimer` are plain accessors).
    pub mission_timer: i32,
    /// C++ `m_bForceUpdate` (`isForceUpdate` / `setForceUpdate`).
    pub force_update: bool,
    /// C++ `m_eOwner`. Never NO_PLAYER here: C++ only uses ownerless groups for the UI
    /// selection list, which is not ported.
    pub owner: PlayerId,
    /// C++ `m_eActivityType`. Write through `group_set_activity_type`.
    pub activity_type: ActivityType,
    /// C++ `m_eAutomateType` (`None` = NO_AUTOMATE). Write through `group_set_automate_type`;
    /// `group_is_automated` also covers FfH AI-controlled units of human players.
    pub automate_type: Option<AutomateType>,
    /// C++ `m_units` (ordered; head unit first). Add/remove through `group_add_unit` /
    /// `group_remove_unit` (`CvUnit::joinGroup`).
    pub units: Vec<UnitId>,
    /// C++ `m_missionQueue` (head mission first). Modify through `group_push_mission`,
    /// `group_insert_at_end_mission_queue`, `group_delete_mission_queue_node`,
    /// `group_clear_mission_queue` (they start/stop missions).
    pub mission_queue: Vec<MissionData>,
    /// C++ `m_lastPlotVisible` (FFH "Alertness" addon: whether the MISSION_MOVE_TO target was
    /// visible when a human group's mission started; `groupAttack` refuses to attack into a
    /// plot that was not). `setLastPathPlotVisibility` / `isLastPathPlotVisible`.
    pub last_plot_visible: bool,
    /// C++ `m_lastPlotRevealed` (FFH "Alertness": target revealed when the mission started;
    /// gates amphibious landings of human groups). `setLastPathPlotRevealed` /
    /// `isLastPathPlotRevealed`.
    pub last_plot_revealed: bool,

    // ---- CvSelectionGroupAI members read/written by rules code ----
    /// C++ `CvSelectionGroupAI::m_iMissionAIX` (INVALID_PLOT_COORD when none).
    pub ai_mission_ai_x: i32,
    /// C++ `CvSelectionGroupAI::m_iMissionAIY` (INVALID_PLOT_COORD when none).
    pub ai_mission_ai_y: i32,
    /// C++ `CvSelectionGroupAI::m_bForceSeparate` (`AI_isForceSeparate` / `AI_makeForceSeparate`).
    pub ai_force_separate: bool,
    /// C++ `CvSelectionGroupAI::m_eMissionAIType` (`AI_getMissionAIType`). Write through
    /// `group_ai_set_mission_ai`.
    pub ai_mission_ai_type: Option<MissionAIType>,
    /// C++ `CvSelectionGroupAI::m_missionAIUnit`. Read through `group_ai_get_mission_ai_unit`
    /// (None if the unit no longer exists).
    pub ai_mission_ai_unit: Option<UnitId>,
    /// C++ `CvSelectionGroupAI::m_bGroupAttack` (`AI_isGroupAttack`; `AI_cancelGroupAttack` sets false).
    pub ai_group_attack: bool,
    /// C++ `CvSelectionGroupAI::m_iGroupAttackX`.
    pub ai_group_attack_x: i32,
    /// C++ `CvSelectionGroupAI::m_iGroupAttackY`.
    pub ai_group_attack_y: i32,
}

impl SelectionGroup {
    /// C++ `CvSelectionGroup::reset(iID, eOwner, false)` + `CvSelectionGroupAI::AI_reset()`.
    pub fn new(id: GroupId, owner: PlayerId) -> SelectionGroup {
        SelectionGroup {
            id,
            mission_timer: 0,
            force_update: false,
            owner,
            activity_type: ActivityType::ACTIVITY_AWAKE,
            automate_type: None,
            units: Vec::new(),
            mission_queue: Vec::new(),
            last_plot_visible: false,
            last_plot_revealed: false,

            ai_mission_ai_x: INVALID_PLOT_COORD,
            ai_mission_ai_y: INVALID_PLOT_COORD,
            ai_force_separate: false,
            ai_mission_ai_type: None,
            ai_mission_ai_unit: None,
            ai_group_attack: false,
            ai_group_attack_x: -1,
            ai_group_attack_y: -1,
        }
    }
}

impl Game {
    /// C++ `CvSelectionGroup::init(iID, eOwner)`: `reset(iID, eOwner)` + `AI_init()`.
    /// The group must already be in `self.groups` (C++ `CvPlayer::addSelectionGroup`); new
    /// groups are normally created directly with `SelectionGroup::new`.
    pub fn group_init(&mut self, g: GroupId, owner: PlayerId) {
        self.groups[g] = SelectionGroup::new(g, owner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_group_matches_cpp_reset() {
        let g = SelectionGroup::new(GroupId(7), PlayerId(1));
        assert_eq!((g.id, g.owner), (GroupId(7), PlayerId(1)));
        assert_eq!(g.activity_type, ActivityType::ACTIVITY_AWAKE);
        assert_eq!(g.automate_type, None);
        assert_eq!((g.mission_timer, g.force_update), (0, false));
        assert!(g.units.is_empty() && g.mission_queue.is_empty());
        // CvSelectionGroupAI::AI_reset
        assert_eq!((g.ai_mission_ai_x, g.ai_mission_ai_y), (INVALID_PLOT_COORD, INVALID_PLOT_COORD));
        assert_eq!((g.ai_mission_ai_type, g.ai_mission_ai_unit), (None, None));
        assert!(!g.ai_force_separate && !g.ai_group_attack);
        assert_eq!((g.ai_group_attack_x, g.ai_group_attack_y), (-1, -1));
    }
}
