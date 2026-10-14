//! CvUnit combat: strength (`maxCombatStr` & co., `isBetterDefenderThan`, attack/defend tests,
//! air strength and damage) and resolution (land and air combat, collateral and flanking damage,
//! interception, air and range strikes, combat state), plus the FfH combat additions
//! (`combatWon`, defensive strikes).
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
    /// C++ `CvUnit::updateAirStrike` (CvUnit.cpp:1272-1329).
    /// C++ `bQuick` dropped: the port never shows combat animations (`isCombatVisible` is false),
    /// so the air-mission branch is never taken.
    pub fn unit_update_air_strike(&mut self, u: UnitId, plot: PlotIdx, finish: bool) {
        todo!("CvUnit::updateAirStrike")
    }

    /// C++ `CvUnit::resolveAirCombat` (CvUnit.cpp:1331-1412).
    /// Returns (our damage, interceptor damage), which C++ stores in the `CvAirMissionDefinition`
    /// out-param (read by `updateAirCombat` for its messages only).
    pub fn unit_resolve_air_combat(&mut self, u: UnitId, interceptor: UnitId, plot: PlotIdx) -> (i32, i32) {
        todo!("CvUnit::resolveAirCombat")
    }

    /// C++ `CvUnit::updateAirCombat` (CvUnit.cpp:1415-1582).
    /// C++ `bQuick` dropped (see `unit_update_air_strike`).
    pub fn unit_update_air_combat(&mut self, u: UnitId) {
        todo!("CvUnit::updateAirCombat")
    }

    /// C++ `CvUnit::resolveCombat` (CvUnit.cpp:1584-1783).
    /// The `CvBattleDefinition&` out-param only feeds the battle animation and is dropped.
    /// The `combatLogCalc`/`combatLogHit` Python events (USE_COMBAT_RESULT_CALLBACK) are reporting.
    pub fn unit_resolve_combat(&mut self, u: UnitId, defender: UnitId, plot: PlotIdx) {
        todo!("CvUnit::resolveCombat")
    }

    /// C++ `CvUnit::updateCombat` (CvUnit.cpp:1786-2251).
    /// C++ `bQuick` dropped: combat always resolves and finishes in the same call (the C++
    /// `bVisible == false` path); combat timers still count down if set.
    pub fn unit_update_combat(&mut self, u: UnitId) {
        todo!("CvUnit::updateCombat")
    }

    /// C++ `CvUnit::isBetterDefenderThan` (CvUnit.cpp:2479-2646).
    /// `defender` `None` -> true. Ties fall back to `isBeforeUnitCycle` (CvGameCoreUtils).
    pub fn unit_is_better_defender_than(&self, u: UnitId, defender: Option<UnitId>, attacker: Option<UnitId>) -> bool {
        todo!("CvUnit::isBetterDefenderThan")
    }

    /// C++ `CvUnit::attack` (CvUnit.cpp:3459-3467).
    /// C++ `bQuick` dropped.
    pub fn unit_attack(&mut self, u: UnitId, plot: PlotIdx) {
        todo!("CvUnit::attack")
    }

    /// C++ `CvUnit::fightInterceptor` (CvUnit.cpp:3469-3476).
    /// C++ `bQuick` dropped.
    pub fn unit_fight_interceptor(&mut self, u: UnitId, plot: PlotIdx) {
        todo!("CvUnit::fightInterceptor")
    }

    /// C++ `CvUnit::attackForDamage` (CvUnit.cpp:3478-3595).
    pub fn unit_attack_for_damage(
        &mut self,
        u: UnitId,
        defender: UnitId,
        attacker_damage_change: i32,
        defender_damage_change: i32,
    ) {
        todo!("CvUnit::attackForDamage")
    }

    /// C++ `CvUnit::isFighting` (CvUnit.cpp:8905-8908).
    pub fn unit_is_fighting(&self, u: UnitId) -> bool {
        // C++ `getCombatUnit() != NULL` (`::getUnit` yields NULL for a dead unit).
        self.units[u].combat_unit.map_or(false, |c| self.units.contains(c))
    }

    /// C++ `CvUnit::isAttacking` (CvUnit.cpp:8911-8914).
    pub fn unit_is_attacking(&self, u: UnitId) -> bool {
        todo!("CvUnit::isAttacking")
    }

    /// C++ `CvUnit::isDefending` (CvUnit.cpp:8917-8920).
    pub fn unit_is_defending(&self, u: UnitId) -> bool {
        todo!("CvUnit::isDefending")
    }

    /// C++ `CvUnit::isCombat` (CvUnit.cpp:8923-8926).
    pub fn unit_is_combat(&self, u: UnitId) -> bool {
        todo!("CvUnit::isCombat")
    }

    /// C++ `CvUnit::maxCombatStr` (CvUnit.cpp:8997-9557).
    /// Call configurations (`plot`, `attacker`), see the comment above the C++ function:
    /// (None, None) = this unit attacks; (Some(p), Some(a)) = this unit defends `p` against `a`;
    /// (Some(p), None) = defends `p` against an unknown attacker; (Some(p), Some(u)) with the
    /// unit itself as attacker = approximate attack strength against an unknown defender on `p`.
    /// FfH: (None, Some(d)) = attack strength against defender `d`.
    /// `details` is filled when given (C++ `CombatDetails*`); its `visual_owner` uses
    /// `getVisualOwner()` with the interface team in C++.
    pub fn unit_max_combat_str(
        &self,
        u: UnitId,
        plot: Option<PlotIdx>,
        attacker: Option<UnitId>,
        details: Option<&mut CombatDetails>,
    ) -> i32 {
        todo!("CvUnit::maxCombatStr")
    }

    /// C++ `CvUnit::currCombatStr` (CvUnit.cpp:9560-9563).
    pub fn unit_curr_combat_str(
        &self,
        u: UnitId,
        plot: Option<PlotIdx>,
        attacker: Option<UnitId>,
        details: Option<&mut CombatDetails>,
    ) -> i32 {
        todo!("CvUnit::currCombatStr")
    }

    /// C++ `CvUnit::currFirepower` (CvUnit.cpp:9566-9569).
    pub fn unit_curr_firepower(&self, u: UnitId, plot: Option<PlotIdx>, attacker: Option<UnitId>) -> i32 {
        todo!("CvUnit::currFirepower")
    }

    /// C++ `CvUnit::currEffectiveStr` (CvUnit.cpp:9574-9582).
    pub fn unit_curr_effective_str(
        &self,
        u: UnitId,
        plot: Option<PlotIdx>,
        attacker: Option<UnitId>,
        details: Option<&mut CombatDetails>,
    ) -> i32 {
        todo!("CvUnit::currEffectiveStr")
    }

    /// C++ `CvUnit::canFight` (CvUnit.cpp:9596-9608).
    pub fn unit_can_fight(&self, u: UnitId) -> bool {
        // FfH: a unit with only defensive strength can still fight.
        !(self.unit_base_combat_str(u) == 0 && self.unit_base_combat_str_defense(u) == 0)
    }

    /// C++ `CvUnit::canAttack` (CvUnit.cpp:9611-9636).
    pub fn unit_can_attack(&self, u: UnitId) -> bool {
        todo!("CvUnit::canAttack")
    }

    /// C++ `CvUnit::canAttack` (CvUnit.cpp:9637-9659).
    /// C++ overload `canAttack(const CvUnit& defender)`.
    pub fn unit_can_attack_unit(&self, u: UnitId, defender: UnitId) -> bool {
        todo!("CvUnit::canAttack")
    }

    /// C++ `CvUnit::canDefend` (CvUnit.cpp:9661-9689).
    /// `plot` `None` = the unit's own plot (C++ default NULL).
    pub fn unit_can_defend(&self, u: UnitId, plot: Option<PlotIdx>) -> bool {
        todo!("CvUnit::canDefend")
    }

    /// C++ `CvUnit::canSiege` (CvUnit.cpp:9692-9710).
    pub fn unit_can_siege(&self, u: UnitId, team: TeamId) -> bool {
        todo!("CvUnit::canSiege")
    }

    /// C++ `CvUnit::airBaseCombatStr` (CvUnit.cpp:9713-9716).
    pub fn unit_air_base_combat_str(&self, u: UnitId) -> i32 {
        todo!("CvUnit::airBaseCombatStr")
    }

    /// C++ `CvUnit::airMaxCombatStr` (CvUnit.cpp:9719-9766).
    pub fn unit_air_max_combat_str(&self, u: UnitId, other: Option<UnitId>) -> i32 {
        todo!("CvUnit::airMaxCombatStr")
    }

    /// C++ `CvUnit::airCurrCombatStr` (CvUnit.cpp:9769-9772).
    pub fn unit_air_curr_combat_str(&self, u: UnitId, other: Option<UnitId>) -> i32 {
        todo!("CvUnit::airCurrCombatStr")
    }

    /// C++ `CvUnit::airCombatLimit` (CvUnit.cpp:9798-9801).
    pub fn unit_air_combat_limit(&self, u: UnitId) -> i32 {
        todo!("CvUnit::airCombatLimit")
    }

    /// C++ `CvUnit::canAirAttack` (CvUnit.cpp:9804-9807).
    pub fn unit_can_air_attack(&self, u: UnitId) -> bool {
        todo!("CvUnit::canAirAttack")
    }

    /// C++ `CvUnit::canAirDefend` (CvUnit.cpp:9810-9844).
    /// `plot` `None` = the unit's own plot (C++ default NULL).
    pub fn unit_can_air_defend(&self, u: UnitId, plot: Option<PlotIdx>) -> bool {
        todo!("CvUnit::canAirDefend")
    }

    /// C++ `CvUnit::airCombatDamage` (CvUnit.cpp:9847-9875).
    pub fn unit_air_combat_damage(&self, u: UnitId, defender: UnitId) -> i32 {
        todo!("CvUnit::airCombatDamage")
    }

    /// C++ `CvUnit::rangeCombatDamage` (CvUnit.cpp:9878-9897).
    pub fn unit_range_combat_damage(&self, u: UnitId, defender: UnitId) -> i32 {
        todo!("CvUnit::rangeCombatDamage")
    }

    /// C++ `CvUnit::bestInterceptor` (CvUnit.cpp:9900-9948).
    pub fn unit_best_interceptor(&self, u: UnitId, plot: PlotIdx) -> Option<UnitId> {
        todo!("CvUnit::bestInterceptor")
    }

    /// C++ `CvUnit::bestSeaPillageInterceptor` (CvUnit.cpp:9951-10001).
    pub fn unit_best_sea_pillage_interceptor(&self, u: UnitId, pillager: UnitId, min_odds: i32) -> Option<UnitId> {
        todo!("CvUnit::bestSeaPillageInterceptor")
    }

    /// C++ `CvUnit::getAttackPlot` (CvUnit.cpp:11904-11907).
    pub fn unit_get_attack_plot(&self, u: UnitId) -> Option<PlotIdx> {
        todo!("CvUnit::getAttackPlot")
    }

    /// C++ `CvUnit::setAttackPlot` (CvUnit.cpp:11910-11927).
    pub fn unit_set_attack_plot(&mut self, u: UnitId, plot: Option<PlotIdx>, air_combat: bool) {
        todo!("CvUnit::setAttackPlot")
    }

    /// C++ `CvUnit::changeCombatTimer` (CvUnit.cpp:11945-11948).
    pub fn unit_change_combat_timer(&mut self, u: UnitId, change: i32) {
        // C++ `setCombatTimer(getCombatTimer() + iChange)`.
        self.units[u].combat_timer += change;
    }

    /// C++ `CvUnit::changeCombatFirstStrikes` (CvUnit.cpp:11961-11964).
    pub fn unit_change_combat_first_strikes(&mut self, u: UnitId, change: i32) {
        // C++ `setCombatFirstStrikes(getCombatFirstStrikes() + iChange)`.
        self.units[u].combat_first_strikes += change;
    }

    /// C++ `CvUnit::setCombatUnit` (CvUnit.cpp:12838-12909).
    /// Draws the "First Strike" sync rand when a combat starts. Combat focus / siege tower /
    /// combat logging are interface-only.
    pub fn unit_set_combat_unit(&mut self, u: UnitId, combat_unit: Option<UnitId>, attacking: bool) {
        todo!("CvUnit::setCombatUnit")
    }

    /// C++ `CvUnit::doDefensiveStrike` (CvUnit.cpp:16206-16259).
    /// FfH: `u` is the defender; its stack may strike the attacker before combat.
    pub fn unit_do_defensive_strike(&mut self, u: UnitId, attacker: UnitId) {
        todo!("CvUnit::doDefensiveStrike")
    }

    /// C++ `CvUnit::combatWon` (CvUnit.cpp:16306-16560).
    /// FfH post-combat effects (XP, combat-apply promotions, capture/enslave, gold, poison,
    /// create/convert units, `PyPostCombatWon`/`PyPostCombatLost` callbacks in `crate::ffh`).
    pub fn unit_combat_won(&mut self, u: UnitId, loser: UnitId, attacking: bool) {
        todo!("CvUnit::combatWon")
    }

    /// C++ `CvUnit::canAdvance` (CvUnit.cpp:17230-17251).
    pub fn unit_can_advance(&self, u: UnitId, plot: PlotIdx, threshold: i32) -> bool {
        todo!("CvUnit::canAdvance")
    }

    /// C++ `CvUnit::collateralCombat` (CvUnit.cpp:17254-17382).
    /// C++ quirk to keep in mind: candidates are kept in a `std::map<CvUnit*, int>` (pointer
    /// order) and the max-value scan keeps the first maximum in that order; values are
    /// `(1 + rand(10000)) * currHitPoints` so ties are rare. The rand draws happen in plot order.
    pub fn unit_collateral_combat(&mut self, u: UnitId, plot: PlotIdx, skip_unit: Option<UnitId>) {
        todo!("CvUnit::collateralCombat")
    }

    /// C++ `CvUnit::flankingStrikeCombat` (CvUnit.cpp:17385-17468).
    /// C++ declares `pSkipUnit = NULL` but dereferences it unconditionally, so it is required here.
    pub fn unit_flanking_strike_combat(
        &mut self,
        u: UnitId,
        plot: PlotIdx,
        attacker_strength: i32,
        attacker_firepower: i32,
        defender_odds: i32,
        defender_damage: i32,
        skip_unit: UnitId,
    ) {
        todo!("CvUnit::flankingStrikeCombat")
    }

    /// C++ `CvUnit::interceptTest` (CvUnit.cpp:17472-17490).
    /// Returns true if the unit was intercepted.
    pub fn unit_intercept_test(&mut self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::interceptTest")
    }

    /// C++ `CvUnit::airStrikeTarget` (CvUnit.cpp:17493-17511).
    pub fn unit_air_strike_target(&self, u: UnitId, plot: PlotIdx) -> Option<UnitId> {
        todo!("CvUnit::airStrikeTarget")
    }

    /// C++ `CvUnit::canAirStrike` (CvUnit.cpp:17514-17547).
    pub fn unit_can_air_strike(&self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::canAirStrike")
    }

    /// C++ `CvUnit::airStrike` (CvUnit.cpp:17550-17587).
    pub fn unit_air_strike(&mut self, u: UnitId, plot: PlotIdx) -> bool {
        todo!("CvUnit::airStrike")
    }

    /// C++ `CvUnit::canRangeStrike` (CvUnit.cpp:17589-17622).
    pub fn unit_can_range_strike(&self, u: UnitId) -> bool {
        todo!("CvUnit::canRangeStrike")
    }

    /// C++ `CvUnit::canRangeStrikeAt` (CvUnit.cpp:17624-17660).
    /// `plot` is the plot the strike is made from; `x`/`y` the target (may be off the map).
    /// C++ passes `getFacingDirection(true)` to `canSeePlot`: always NO_DIRECTION with FfH data
    /// (no unit has bLineOfSight).
    pub fn unit_can_range_strike_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        todo!("CvUnit::canRangeStrikeAt")
    }

    /// C++ `CvUnit::rangeStrike` (CvUnit.cpp:17663-17726).
    /// C++ sets the defender group's mission timer only inside `isActiveVisible` (interface);
    /// that is animation pacing, not rules.
    pub fn unit_range_strike(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        todo!("CvUnit::rangeStrike")
    }

    /// C++ `CvUnit::isTargetOf` (CvUnit.cpp:17980-18009).
    pub fn unit_is_target_of(&self, u: UnitId, attacker: UnitId) -> bool {
        todo!("CvUnit::isTargetOf")
    }

    /// C++ `CvUnit::getDefenderCombatValues` (CvUnit.cpp:18051-18080).
    /// The C++ `int&` out-params are returned as a [`DefenderCombatValues`].
    pub fn unit_get_defender_combat_values(
        &self,
        u: UnitId,
        defender: UnitId,
        plot: PlotIdx,
        our_strength: i32,
        our_firepower: i32,
        their_details: Option<&mut CombatDetails>,
    ) -> DefenderCombatValues {
        todo!("CvUnit::getDefenderCombatValues")
    }
}
