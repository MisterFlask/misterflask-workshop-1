//! CvSelectionGroup: pathing (`generatePath`, path plots, `lastMissionPlot`), movement
//! (`groupMove`, `groupPathTo`, `groupRoadTo`, amphibious moves, transport), combat
//! (`groupAttack`, `groupDeclareWar`, best attacker / sacrifice from `CvSelectionGroupAI`),
//! build missions, and the can-move / can-enter checks.
//!
//! Pathfinder state: C++ keeps the last path in the global `GC.getPathFinder()` (FAStar),
//! and `generatePath` is `const` (called from const code such as `CvUnit::getUpgradeCity`).
//! The path methods here therefore take `&self`; the path module keeps its last-path state
//! behind interior mutability. `getPathLastNode` (raw FAStar node) maps to that module's API.

#![allow(unused_variables)]
#![allow(unused_imports)]
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Pathing
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::generatePath` (C++ defaults `iFlags = 0, bReuse = false`).
    /// Returns `Some(path turns)` on success (C++ `true`, `*piPathTurns`; `MAX_INT` if the
    /// last node is missing), `None` on failure.
    pub fn group_generate_path(&self, g: GroupId, from: PlotIdx, to: PlotIdx, flags: i32, reuse: bool) -> Option<i32> {
        todo!("CvSelectionGroup::generatePath")
    }

    /// C++ `CvSelectionGroup::getPathFirstPlot`: first step of the last generated path.
    pub fn group_get_path_first_plot(&self, g: GroupId) -> Option<PlotIdx> {
        todo!("CvSelectionGroup::getPathFirstPlot")
    }

    /// C++ `CvSelectionGroup::getPathEndTurnPlot`: where the last generated path ends this turn.
    pub fn group_get_path_end_turn_plot(&self, g: GroupId) -> Option<PlotIdx> {
        todo!("CvSelectionGroup::getPathEndTurnPlot")
    }

    /// C++ `CvSelectionGroup::resetPath` (FAStar `ForceReset`).
    pub fn group_reset_path(&self, g: GroupId) {
        todo!("CvSelectionGroup::resetPath")
    }

    /// C++ `CvSelectionGroup::lastMissionPlot`: target of the last queued move mission, else
    /// the group's plot (non-const in C++, read-only).
    pub fn group_last_mission_plot(&self, g: GroupId) -> Option<PlotIdx> {
        todo!("CvSelectionGroup::lastMissionPlot")
    }

    // ------------------------------------------------------------------
    // Can move / enter
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::canEnterTerritory` (`team = None` is NO_TEAM; C++ default
    /// `bIgnoreRightOfPassage = false`).
    pub fn group_can_enter_territory(&self, g: GroupId, team: Option<TeamId>, ignore_right_of_passage: bool) -> bool {
        todo!("CvSelectionGroup::canEnterTerritory")
    }

    /// C++ `CvSelectionGroup::canEnterArea` (C++ default `bIgnoreRightOfPassage = false`).
    pub fn group_can_enter_area(&self, g: GroupId, team: Option<TeamId>, area: AreaId, ignore_right_of_passage: bool) -> bool {
        todo!("CvSelectionGroup::canEnterArea")
    }

    /// C++ `CvSelectionGroup::canMoveInto` (any unit can; C++ default `bAttack = false`;
    /// non-const in C++, read-only).
    pub fn group_can_move_into(&self, g: GroupId, plot: PlotIdx, attack: bool) -> bool {
        todo!("CvSelectionGroup::canMoveInto")
    }

    /// C++ `CvSelectionGroup::canMoveOrAttackInto` (C++ default `bDeclareWar = false`).
    pub fn group_can_move_or_attack_into(&self, g: GroupId, plot: PlotIdx, declare_war: bool) -> bool {
        todo!("CvSelectionGroup::canMoveOrAttackInto")
    }

    /// C++ `CvSelectionGroup::canMoveThrough` (all units can).
    pub fn group_can_move_through(&self, g: GroupId, plot: PlotIdx) -> bool {
        todo!("CvSelectionGroup::canMoveThrough")
    }

    /// C++ `CvSelectionGroup::canBombard`.
    pub fn group_can_bombard(&self, g: GroupId, plot: PlotIdx) -> bool {
        todo!("CvSelectionGroup::canBombard")
    }

    /// C++ `CvSelectionGroup::isAmphibPlot`.
    pub fn group_is_amphib_plot(&self, g: GroupId, plot: PlotIdx) -> bool {
        todo!("CvSelectionGroup::isAmphibPlot")
    }

    // ------------------------------------------------------------------
    // Moving and fighting
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::groupDeclareWar` (C++ default `bForce = false`): returns true if
    /// the group was bumped by the war declaration.
    pub fn group_declare_war(&mut self, g: GroupId, plot: PlotIdx, force: bool) -> bool {
        todo!("CvSelectionGroup::groupDeclareWar")
    }

    /// C++ `CvSelectionGroup::groupAttack`: returns `(attack made, bFailedAlreadyFighting)`.
    pub fn group_attack(&mut self, g: GroupId, x: i32, y: i32, flags: i32) -> (bool, bool) {
        todo!("CvSelectionGroup::groupAttack")
    }

    /// C++ `CvSelectionGroup::groupMove` (C++ defaults `pCombatUnit = NULL, bEndMove = false`).
    pub fn group_move(&mut self, g: GroupId, plot: PlotIdx, combat: bool, combat_unit: Option<UnitId>, end_move: bool) {
        todo!("CvSelectionGroup::groupMove")
    }

    /// C++ `CvSelectionGroup::groupPathTo`: returns true if a move was made.
    pub fn group_path_to(&mut self, g: GroupId, x: i32, y: i32, flags: i32) -> bool {
        todo!("CvSelectionGroup::groupPathTo")
    }

    /// C++ `CvSelectionGroup::groupRoadTo`: returns true if a move or build was made.
    pub fn group_road_to(&mut self, g: GroupId, x: i32, y: i32, flags: i32) -> bool {
        todo!("CvSelectionGroup::groupRoadTo")
    }

    /// C++ `CvSelectionGroup::groupAmphibMove`: returns true if a landing was attempted.
    pub fn group_amphib_move(&mut self, g: GroupId, plot: PlotIdx, flags: i32) -> bool {
        todo!("CvSelectionGroup::groupAmphibMove")
    }

    /// C++ `CvSelectionGroup::setTransportUnit` (`None` unloads; splits the group if the
    /// transport cannot carry all of it).
    pub fn group_set_transport_unit(&mut self, g: GroupId, transport: Option<UnitId>) {
        todo!("CvSelectionGroup::setTransportUnit")
    }

    /// C++ `CvSelectionGroup::getBombardTurns` (BBAI, defined in CvSelectionGroupAI.cpp;
    /// used by `groupAttack` for AI groups): -1 if the group cannot bombard.
    pub fn group_get_bombard_turns(&self, g: GroupId, city: CityId) -> i32 {
        todo!("CvSelectionGroup::getBombardTurns")
    }

    /// C++ `CvSelectionGroupAI::AI_isDeclareWar` (`plot = None` allowed): whether this
    /// (non-human) group may declare war to move into `plot`. Read by pathfinding and
    /// `CvUnit`. Non-const in C++, read-only.
    pub fn group_ai_is_declare_war(&self, g: GroupId, plot: Option<PlotIdx>) -> bool {
        todo!("CvSelectionGroupAI::AI_isDeclareWar")
    }

    /// C++ `CvSelectionGroupAI::AI_getBestGroupAttacker` (C++ defaults `bForce = false,
    /// bNoBlitz = false`): picks the attacking unit in `groupAttack` (for humans too).
    /// Returns `(unit, iUnitOdds)`.
    pub fn group_ai_get_best_group_attacker(&self, g: GroupId, plot: PlotIdx, potential_enemy: bool, force: bool, no_blitz: bool) -> (Option<UnitId>, i32) {
        todo!("CvSelectionGroupAI::AI_getBestGroupAttacker")
    }

    /// C++ `CvSelectionGroupAI::AI_getBestGroupSacrifice` (C++ defaults `bForce = false,
    /// bNoBlitz = false`).
    pub fn group_ai_get_best_group_sacrifice(&self, g: GroupId, plot: PlotIdx, potential_enemy: bool, force: bool, no_blitz: bool) -> Option<UnitId> {
        todo!("CvSelectionGroupAI::AI_getBestGroupSacrifice")
    }

    /// C++ `CvSelectionGroupAI::AI_queueGroupAttack` (called by `groupAttack` for AI stacks).
    pub fn group_ai_queue_group_attack(&mut self, g: GroupId, x: i32, y: i32) {
        let grp = &mut self.groups[g];
        grp.ai_group_attack = true;

        grp.ai_group_attack_x = x;
        grp.ai_group_attack_y = y;
    }

    // ------------------------------------------------------------------
    // Building
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::groupBuild`: returns true if the build should continue.
    pub fn group_build(&mut self, g: GroupId, build: BuildType) -> bool {
        todo!("CvSelectionGroup::groupBuild")
    }

    /// C++ `CvSelectionGroup::getBestBuildRoute`: best route any unit can build on `plot`,
    /// with the build that makes it (C++ `*peBestBuild`).
    pub fn group_get_best_build_route(&self, g: GroupId, plot: PlotIdx) -> (Option<RouteType>, Option<BuildType>) {
        todo!("CvSelectionGroup::getBestBuildRoute")
    }
}
