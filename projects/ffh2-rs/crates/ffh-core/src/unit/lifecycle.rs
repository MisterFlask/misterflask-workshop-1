//! CvUnit lifecycle: `init`, `convert`, `kill`, `doTurn`, selection-group membership, position
//! (`setXY`, `plot`), damage, ownership, names and random events; plus the few `CvUnitAI`
//! bookkeeping functions that rules code depends on (`AI_init`, `AI_setUnitAIType`, ...).
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges. Every `todo!()` names the C++ function still to port.

#![allow(unused_variables, unused_imports)]

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::unit::{CombatDetails, DefenderCombatValues, Unit, UnitResetInfo};
use ffh_data::*;

impl Game {
    /// C++ `CvUnit::init` (CvUnit.cpp:106-308).
    /// C++ `init(iID, eUnit, eUnitAI, eOwner, iX, iY, eFacingDirection)`: the slot `u` is allocated by the
    /// `CvPlayer::initUnit` port; this first re-runs `reset` (`self.units[u] = Unit::new(..)`), then
    /// `setXY`, names, counters, free promotions, `AI_init` (-> `unit_ai_init`, which draws the
    /// "AI Unit Birthmark" sync rand) and the FfH additions. `eFacingDirection` (graphics) is dropped.
    pub fn unit_init(&mut self, u: UnitId, unit_type: UnitType, unit_ai: UnitAIType, owner: PlayerId, x: i32, y: i32) {
        todo!("CvUnit::init")
    }

    /// C++ `CvUnit::convert` (CvUnit.cpp:570-719).
    /// `from` is C++ `pUnit` (the unit being replaced; it is killed with delay at the end).
    pub fn unit_convert(&mut self, u: UnitId, from: UnitId) {
        todo!("CvUnit::convert")
    }

    /// C++ `CvUnit::kill` (CvUnit.cpp:722-1002).
    pub fn unit_kill(&mut self, u: UnitId, delay: bool, player: Option<PlayerId>) {
        todo!("CvUnit::kill")
    }

    /// C++ `CvUnit::doTurn` (CvUnit.cpp:1011-1269).
    pub fn unit_do_turn(&mut self, u: UnitId) {
        todo!("CvUnit::doTurn")
    }

    /// C++ `CvUnit::isInGroup` (CvUnit.cpp:10724-10727).
    pub fn unit_is_in_group(&self, u: UnitId) -> bool {
        self.units[u].group.is_some()
    }

    /// C++ `CvUnit::isGroupHead` (CvUnit.cpp:10730-10733).
    pub fn unit_is_group_head(&self, u: UnitId) -> bool {
        todo!("CvUnit::isGroupHead")
    }

    /// C++ `CvUnit::canJoinGroup` (CvUnit.cpp:10742-10811).
    pub fn unit_can_join_group(&self, u: UnitId, plot: PlotIdx, group: GroupId) -> bool {
        todo!("CvUnit::canJoinGroup")
    }

    /// C++ `CvUnit::joinGroup` (CvUnit.cpp:10814-10944).
    /// C++ `bRemoveSelected` (interface selection list) is dropped; C++ default `bRejoin` is true.
    pub fn unit_join_group(&mut self, u: UnitId, group: Option<GroupId>, rejoin: bool) {
        todo!("CvUnit::joinGroup")
    }

    /// C++ `CvUnit::setXY` (CvUnit.cpp:10995-11579).
    /// C++ `bUpdate`, `bShow`, `bCheckPlotVisible` only drive graphics (plot center unit, flags,
    /// entity movement) and are dropped. `x`/`y` may be `INVALID_PLOT_COORD` (unit leaves the map).
    pub fn unit_set_xy(&mut self, u: UnitId, x: i32, y: i32, group: bool) {
        todo!("CvUnit::setXY")
    }

    /// C++ `CvUnit::at` (CvUnit.cpp:11582-11585).
    pub fn unit_at(&self, u: UnitId, x: i32, y: i32) -> bool {
        let unit = &self.units[u];
        unit.x == x && unit.y == y
    }

    /// C++ `CvUnit::atPlot` (CvUnit.cpp:11588-11591).
    pub fn unit_at_plot(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::atPlot")
    }

    /// C++ `CvUnit::plot` (CvUnit.cpp:11594-11597).
    /// C++ `plot()` (`plotSorenINLINE(m_iX, m_iY)`): `None` while the unit is off the map.
    /// See also [`Game::unit_plot`] for call sites that dereference the result.
    pub fn unit_plot_opt(&self, u: UnitId) -> Option<PlotIdx> {
        todo!("CvUnit::plot")
    }

    /// Convenience for C++ `plot()` call sites that dereference the pointer (C++ would crash on
    /// NULL; this panics). Not a separate C++ function.
    pub fn unit_plot(&self, u: UnitId) -> PlotIdx {
        self.unit_plot_opt(u).expect("CvUnit::plot: unit is not on the map")
    }

    /// C++ `CvUnit::getArea` (CvUnit.cpp:11600-11603) and `CvUnit::area` (CvUnit.cpp:11606-11609).
    /// C++ `getArea()` (area id) and `area()` (`CvArea*`) are the same thing here.
    pub fn unit_area(&self, u: UnitId) -> AreaId {
        todo!("CvUnit::area")
    }

    /// C++ `CvUnit::onMap` (CvUnit.cpp:11612-11615).
    pub fn unit_on_map(&self, u: UnitId) -> bool {
        todo!("CvUnit::onMap")
    }

    /// C++ `CvUnit::getReconPlot` (CvUnit.cpp:11631-11634).
    pub fn unit_get_recon_plot(&self, u: UnitId) -> Option<PlotIdx> {
        todo!("CvUnit::getReconPlot")
    }

    /// C++ `CvUnit::setReconPlot` (CvUnit.cpp:11637-11665).
    pub fn unit_set_recon_plot(&mut self, u: UnitId, plot: Option<PlotIdx>) {
        todo!("CvUnit::setReconPlot")
    }

    /// C++ `CvUnit::setDamage` (CvUnit.cpp:11687-11721).
    /// C++ `bNotifyEntity` (damage animation) is dropped. Kills the unit (with delay) when dead.
    pub fn unit_set_damage(&mut self, u: UnitId, value: i32, player: Option<PlayerId>) {
        todo!("CvUnit::setDamage")
    }

    /// C++ `CvUnit::changeDamage` (CvUnit.cpp:11724-11727).
    pub fn unit_change_damage(&mut self, u: UnitId, change: i32, player: Option<PlayerId>) {
        todo!("CvUnit::changeDamage")
    }

    /// C++ `CvUnit::doDelayedDeath` (CvUnit.cpp:12643-12652).
    /// Returns true if the unit was killed.
    pub fn unit_do_delayed_death(&mut self, u: UnitId) -> bool {
        todo!("CvUnit::doDelayedDeath")
    }

    /// C++ `CvUnit::getVisualOwner` (CvUnit.cpp:12736-12760).
    /// C++ `NO_TEAM` means "the active (interface) team"; rules call sites pass a team.
    pub fn unit_get_visual_owner(&self, u: UnitId, for_team: Option<TeamId>) -> PlayerId {
        todo!("CvUnit::getVisualOwner")
    }

    /// C++ `CvUnit::getCombatOwner` (CvUnit.cpp:12763-12774).
    pub fn unit_get_combat_owner(&self, u: UnitId, for_team: Option<TeamId>, plot: Option<PlotIdx>) -> PlayerId {
        todo!("CvUnit::getCombatOwner")
    }

    /// C++ `CvUnit::getTeam` (CvUnit.cpp:12776-12779).
    /// C++ `GET_PLAYER(getOwnerINLINE()).getTeam()`.
    pub fn unit_get_team(&self, u: UnitId) -> TeamId {
        todo!("CvUnit::getTeam")
    }

    /// C++ `CvUnit::setCapturingPlayer` (CvUnit.cpp:12788-12799).
    pub fn unit_set_capturing_player(&mut self, u: UnitId, player: Option<PlayerId>) {
        // FfH: units immune to capture never get a capturing player.
        if !self.unit_is_immune_to_capture(u) {
            self.units[u].capturing_player = player;
        }
    }

    /// C++ `CvUnit::getName` (CvUnit.cpp:12997-13009).
    /// C++ `getName(uiForm)`: `m_szName` + unit description, or just the description.
    /// The grammatical form argument (text formatting) is dropped.
    pub fn unit_get_name(&self, u: UnitId) -> String {
        todo!("CvUnit::getName")
    }

    /// C++ `CvUnit::getNameKey` (CvUnit.cpp:13012-13022).
    pub fn unit_get_name_key(&self, u: UnitId) -> String {
        todo!("CvUnit::getNameKey")
    }

    /// C++ `CvUnit::setName` (CvUnit.cpp:13031-13041).
    pub fn unit_set_name(&mut self, u: UnitId, name: String) {
        // C++ also runs `gDLL->stripSpecialCharacters` (exe function, cosmetic) and marks the
        // info pane dirty; names never affect rules.
        self.units[u].name = name;
    }

    /// C++ `CvUnit::getTriggerValue` (CvUnit.cpp:18082-18158).
    /// Calls the trigger's `PythonCanDoUnit` callback (port in `crate::ffh`).
    pub fn unit_get_trigger_value(
        &self,
        u: UnitId,
        trigger: EventTriggerType,
        plot: Option<PlotIdx>,
        check_plot: bool,
    ) -> i32 {
        todo!("CvUnit::getTriggerValue")
    }

    /// C++ `CvUnit::canApplyEvent` (CvUnit.cpp:18160-18197).
    pub fn unit_can_apply_event(&self, u: UnitId, event: EventType) -> bool {
        todo!("CvUnit::canApplyEvent")
    }

    /// C++ `CvUnit::applyEvent` (CvUnit.cpp:18199-18245).
    pub fn unit_apply_event(&mut self, u: UnitId, event: EventType) {
        todo!("CvUnit::applyEvent")
    }

    /// C++ `CvUnitAI::AI_init` (CvUnitAI.cpp:41-52).
    /// Called by `CvUnit::init`: `AI_reset(eUnitAI)` (sets `ai_type` and the other AI fields),
    /// `AI_setBirthmark(getSorenRandNum(10000, "AI Unit Birthmark"))` (the sync rand draw must be
    /// kept for RNG parity; `AI_setBirthmark` adjusts it for UNITAI_EXPLORE_SEA), then
    /// `area()->changeNumAIUnits` / `GET_PLAYER(owner).AI_changeNumAIUnits` (+1).
    pub fn unit_ai_init(&mut self, u: UnitId, unit_ai: UnitAIType) {
        todo!("CvUnitAI::AI_init")
    }

    /// C++ `CvUnitAI::AI_isCityAIType` (CvUnitAI.cpp:1376-1382).
    /// Used by rules code through `PUF_isCityAIType` (CvGameCoreUtils).
    pub fn unit_ai_is_city_ai_type(&self, u: UnitId) -> bool {
        let t = self.units[u].ai_type;
        t == Some(UnitAIType::UNITAI_CITY_DEFENSE)
            || t == Some(UnitAIType::UNITAI_CITY_COUNTER)
            || t == Some(UnitAIType::UNITAI_CITY_SPECIAL)
            || t == Some(UnitAIType::UNITAI_RESERVE)
    }

    /// C++ `CvUnitAI::AI_setUnitAIType` (CvUnitAI.cpp:1432-1448).
    /// Called by rules code (`CvUnit::convert`, `CvUnit::found`, CvSelectionGroup). On change: area and
    /// player AI-unit counts -1/+1 and `joinGroup(NULL)`.
    pub fn unit_ai_set_unit_ai_type(&mut self, u: UnitId, unit_ai: UnitAIType) {
        todo!("CvUnitAI::AI_setUnitAIType")
    }

    /// C++ `CvUnitAI::getOriginPlot` (CvUnitAI.cpp:19100-19107).
    /// `None` when `ai_origin_x == -1 && ai_origin_y == -1`.
    pub fn unit_ai_get_origin_plot(&self, u: UnitId) -> Option<PlotIdx> {
        todo!("CvUnitAI::getOriginPlot")
    }

    /// C++ `CvUnitAI::setOriginPlot` (CvUnitAI.cpp:19109-19121).
    /// Called by `CvUnit::found`. `None` stores -1/-1.
    pub fn unit_ai_set_origin_plot(&mut self, u: UnitId, plot: Option<PlotIdx>) {
        todo!("CvUnitAI::setOriginPlot")
    }
}
