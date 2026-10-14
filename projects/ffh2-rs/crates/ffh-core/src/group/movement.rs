//! CvSelectionGroup: pathing (`generatePath`, path plots, `lastMissionPlot`), movement
//! (`groupMove`, `groupPathTo`, `groupRoadTo`, amphibious moves, transport), combat
//! (`groupAttack`, `groupDeclareWar`, best attacker / sacrifice from `CvSelectionGroupAI`),
//! build missions, and the can-move / can-enter checks.
//!
//! Pathfinder state: C++ keeps the last path in the global `GC.getPathFinder()` (FAStar),
//! and `generatePath` is `const` (called from const code such as `CvUnit::getUpgradeCity`).
//! The path methods here therefore take `&self`; the path module keeps its last-path state
//! behind interior mutability (`Game::path_finder`). `getPathLastNode` (raw FAStar node) is
//! `self.path_finder.borrow().last_node()`.
//!
//! Unit loops whose body can change the group (moves, attacks, builds, loading) iterate over a
//! snapshot of the group's units and skip units that died or left the group meanwhile: C++
//! fetches the next `CLLNode` before the body runs, so units removed by the body are skipped
//! there as well.

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::MissionData;
use ffh_data::*;

// -------------------------------------------------------------------------------------------
// Helpers shared with `missions.rs`
// -------------------------------------------------------------------------------------------

/// The group's units, for a C++ `headUnitNode()`/`nextUnitNode()` loop whose body can change
/// the group (see the module docs). Empty if the group no longer exists.
pub(super) fn units_snapshot(g: &Game, grp: GroupId) -> Vec<UnitId> {
    g.groups.get(grp).map_or_else(Vec::new, |group| group.units.clone())
}

/// Whether `u` is still alive and in `grp` (C++: still in the linked list being walked).
pub(super) fn is_group_member(g: &Game, u: UnitId, grp: GroupId) -> bool {
    g.units.get(u).is_some_and(|unit| unit.group == Some(grp))
}

/// C++ `getNumUnits()`, 0 if the group no longer exists.
pub(super) fn num_units(g: &Game, grp: GroupId) -> usize {
    g.groups.get(grp).map_or(0, |group| group.units.len())
}

/// The head mission (C++ `headMissionQueueNode()->m_data`), if any.
pub(super) fn head_mission(g: &Game, grp: GroupId) -> Option<MissionData> {
    g.groups.get(grp).and_then(|group| group.mission_queue.first().cloned())
}

/// C++ `GET_PLAYER((PlayerTypes)iData1).getUnit(iData2)` for `MISSION_MOVE_TO_UNIT` (unit ids
/// are global here; the unit must still exist and belong to that player).
pub(super) fn mission_target_unit(g: &Game, player: i32, unit: i32) -> Option<UnitId> {
    if player < 0 || player as usize >= g.num_players() || unit < 0 {
        return None;
    }
    let u = UnitId(unit as u32);
    g.units.get(u).filter(|target| target.owner == PlayerId(player as u8)).map(|_| u)
}

/// C++ `CvSelectionGroup::canEnterArea` with a possibly NULL `pArea` (`CvUnit::canEnterArea`
/// accepts NULL; `groupDeclareWar` passes `pPlot->area()`).
fn can_enter_area(g: &Game, grp: GroupId, team: Option<TeamId>, area: Option<AreaId>, ignore_right_of_passage: bool) -> bool {
    let units = &g.groups[grp].units;
    if units.is_empty() {
        return false;
    }

    units.iter().all(|&u| g.unit_can_enter_area(u, team, area, ignore_right_of_passage))
}

/// CvGameUtils.py `doCombat` (via `CvGameInterface.doCombat`), called once per attack in
/// `groupAttack`: FFH2 returns False (a result of 1 would end the attack loop).
fn py_do_combat(_g: &Game, _grp: GroupId, _plot: PlotIdx) -> bool {
    false
}

/// C++ `CvUnitAI::AI_attackOdds(pPlot, bPotentialEnemy)` (needed by `AI_getBestGroupAttacker`):
/// attack odds out of 100, withdrawal included.
fn ai_attack_odds(g: &Game, u: UnitId, plot: PlotIdx, potential_enemy: bool) -> i32 {
    let owner = g.units[u].owner;
    let Some(defender) = g.plot_get_best_defender(plot, None, Some(owner), Some(u), !potential_enemy, potential_enemy, false) else {
        return 100;
    };

    let air = g.unit_get_domain_type(u) == DomainType::DOMAIN_AIR;
    let mut our_strength = if air { g.unit_air_curr_combat_str(u, None) } else { g.unit_curr_combat_str(u, None, None, None) };
    let our_firepower = if air { our_strength } else { g.unit_curr_firepower(u, None, None) };

    if our_strength == 0 {
        return 1;
    }

    let mut their_strength = g.unit_curr_combat_str(defender, Some(plot), Some(u), None);
    let their_firepower = g.unit_curr_firepower(defender, Some(plot), Some(u));

    let base_odds = (100 * our_strength) / (our_strength + their_strength);
    if base_odds == 0 {
        return 1;
    }

    let strength_factor = (our_firepower + their_firepower + 1) / 2;

    let combat_damage = g.d.defines.combat_damage;
    let damage_to_us = 1.max((combat_damage * (their_firepower + strength_factor)) / (our_firepower + strength_factor));
    let damage_to_them = 1.max((combat_damage * (our_firepower + strength_factor)) / (their_firepower + strength_factor));

    let hit_limit_them = g.unit_max_hit_points(defender) - g.units[u].combat_limit;

    let mut needed_rounds_us = (0.max(g.unit_curr_hit_points(defender) - hit_limit_them) + damage_to_them - 1) / damage_to_them;
    let mut needed_rounds_them = (0.max(g.unit_curr_hit_points(u)) + damage_to_us - 1) / damage_to_us;

    if !air {
        // BETTER_BTS_AI_MOD 10/30/09 Mongoose & jdog5000 (from Mongoose SDK)
        if !g.unit_immune_to_first_strikes(defender) {
            needed_rounds_us -= ((base_odds * g.unit_first_strikes(u)) + ((base_odds * g.unit_chance_first_strikes(u)) / 2)) / 100;
        }
        if !g.unit_immune_to_first_strikes(u) {
            needed_rounds_them -= (((100 - base_odds) * g.unit_first_strikes(defender))
                + (((100 - base_odds) * g.unit_chance_first_strikes(defender)) / 2))
                / 100;
        }
        needed_rounds_us = 1.max(needed_rounds_us);
        needed_rounds_them = 1.max(needed_rounds_them);
    }

    let rounds_diff = needed_rounds_us - needed_rounds_them;
    if rounds_diff > 0 {
        their_strength *= 1 + rounds_diff;
    } else {
        our_strength *= 1 - rounds_diff;
    }

    let mut odds = (our_strength * 100) / (our_strength + their_strength);
    odds += ((100 - odds) * g.unit_withdrawal_probability(u)) / 100;
    // BETTER AI (attack odds) Sephi: `iOdds += AI_getAttackOddsChange()` is commented out.

    1.max(odds.min(99))
}

/// C++ `CvUnitAI::AI_sacrificeValue(pPlot)` (needed by `AI_getBestGroupSacrifice`).
fn ai_sacrifice_value(g: &Game, u: UnitId, plot: Option<PlotIdx>) -> i32 {
    let mut collateral_damage_value = 0;
    if let Some(p) = plot {
        let possible_targets = (g.plot_get_num_visible_enemy_defenders(p, u) - 1).min(g.unit_collateral_damage_max_units(u));

        if possible_targets > 0 {
            collateral_damage_value = g.unit_collateral_damage(u);
            collateral_damage_value += 0.max(collateral_damage_value - 100);
            collateral_damage_value *= possible_targets;
            collateral_damage_value /= 5;
        }
    }

    let info = &g.d.units[g.units[u].unit_type];
    let mut value;
    if g.unit_get_domain_type(u) == DomainType::DOMAIN_AIR {
        value = 128 * (100 + g.unit_curr_interception_probability(u));
        if info.nuke_range != -1 {
            value += 25000;
        }
        value /= 1.max(1 + info.production_cost);
        value *= g.unit_max_hit_points(u) - g.units[u].damage;
        value /= 100;
    } else {
        value = 128 * g.unit_curr_effective_str(u, plot, plot.map(|_| u), None);
        value *= 100 + collateral_damage_value;
        value /= 100 + g.unit_city_defense_modifier(u);
        value *= 100 + g.unit_withdrawal_probability(u);
        value /= 1.max(1 + info.production_cost);
        value /= 10 + g.units[u].experience;

        if info.combat_limit < 100 {
            value *= 150;
            value /= 100;
        }
    }

    value
}

impl Game {
    // ------------------------------------------------------------------
    // Pathing
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::generatePath` (C++ defaults `iFlags = 0, bReuse = false`).
    /// Returns `Some(path turns)` on success (C++ `true`, `*piPathTurns`; `MAX_INT` if the
    /// last node is missing), `None` on failure.
    pub fn group_generate_path(&self, g: GroupId, from: PlotIdx, to: PlotIdx, flags: i32, reuse: bool) -> Option<i32> {
        let mut finder = self.path_finder.borrow_mut();

        // SetData(&GC.getPathFinder(), this)
        finder.set_group(Some(g));

        let (from_x, from_y) = self.map.xy(from);
        let (to_x, to_y) = self.map.xy(to);
        if finder.generate_path(self, from_x, from_y, to_x, to_y, false, flags, reuse) {
            Some(finder.path_turns().unwrap_or(MAX_INT))
        } else {
            None
        }
    }

    /// C++ `CvSelectionGroup::getPathFirstPlot`: first step of the last generated path.
    pub fn group_get_path_first_plot(&self, _g: GroupId) -> Option<PlotIdx> {
        // C++ reads the global path finder: the last path generated by any group.
        let (x, y) = self.path_finder.borrow().path_first_xy()?;
        self.map.plot_soren(x, y)
    }

    /// C++ `CvSelectionGroup::getPathEndTurnPlot`: where the last generated path ends this turn.
    pub fn group_get_path_end_turn_plot(&self, _g: GroupId) -> Option<PlotIdx> {
        let (x, y) = self.path_finder.borrow().path_end_turn_xy()?;
        self.map.plot_soren(x, y)
    }

    /// C++ `CvSelectionGroup::resetPath` (FAStar `ForceReset`).
    pub fn group_reset_path(&self, _g: GroupId) {
        self.path_finder.borrow_mut().force_reset();
    }

    /// C++ `CvSelectionGroup::lastMissionPlot`: target of the last queued move mission, else
    /// the group's plot (non-const in C++, read-only).
    pub fn group_last_mission_plot(&self, g: GroupId) -> Option<PlotIdx> {
        for mission in self.groups[g].mission_queue.iter().rev() {
            match mission.mission_type {
                MissionType::MISSION_MOVE_TO | MissionType::MISSION_ROUTE_TO => {
                    return self.map.plot_xy(mission.data1, mission.data2);
                }

                MissionType::MISSION_MOVE_TO_UNIT => {
                    if let Some(target) = mission_target_unit(self, mission.data1, mission.data2) {
                        return self.unit_plot_opt(target);
                    }
                }

                MissionType::MISSION_SKIP
                | MissionType::MISSION_SLEEP
                | MissionType::MISSION_FORTIFY
                | MissionType::MISSION_PLUNDER
                | MissionType::MISSION_AIRPATROL
                | MissionType::MISSION_SEAPATROL
                | MissionType::MISSION_HEAL
                | MissionType::MISSION_SENTRY
                | MissionType::MISSION_AIRLIFT
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

                _ => debug_assert!(false, "lastMissionPlot: unexpected mission {:?}", mission.mission_type),
            }
        }

        self.group_plot(g)
    }

    // ------------------------------------------------------------------
    // Can move / enter
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::canEnterTerritory` (`team = None` is NO_TEAM; C++ default
    /// `bIgnoreRightOfPassage = false`).
    pub fn group_can_enter_territory(&self, g: GroupId, team: Option<TeamId>, ignore_right_of_passage: bool) -> bool {
        let units = &self.groups[g].units;
        if units.is_empty() {
            return false;
        }

        units.iter().all(|&u| self.unit_can_enter_territory(u, team, ignore_right_of_passage))
    }

    /// C++ `CvSelectionGroup::canEnterArea` (C++ default `bIgnoreRightOfPassage = false`).
    pub fn group_can_enter_area(&self, g: GroupId, team: Option<TeamId>, area: AreaId, ignore_right_of_passage: bool) -> bool {
        can_enter_area(self, g, team, Some(area), ignore_right_of_passage)
    }

    /// C++ `CvSelectionGroup::canMoveInto` (any unit can; C++ default `bAttack = false`;
    /// non-const in C++, read-only).
    pub fn group_can_move_into(&self, g: GroupId, plot: PlotIdx, attack: bool) -> bool {
        self.groups[g].units.iter().any(|&u| self.unit_can_move_into(u, plot, attack, false, false))
    }

    /// C++ `CvSelectionGroup::canMoveOrAttackInto` (C++ default `bDeclareWar = false`).
    pub fn group_can_move_or_attack_into(&self, g: GroupId, plot: PlotIdx, declare_war: bool) -> bool {
        self.groups[g].units.iter().any(|&u| self.unit_can_move_or_attack_into(u, plot, declare_war))
    }

    /// C++ `CvSelectionGroup::canMoveThrough` (all units can).
    pub fn group_can_move_through(&self, g: GroupId, plot: PlotIdx) -> bool {
        let units = &self.groups[g].units;
        if units.is_empty() {
            return false;
        }

        units.iter().all(|&u| self.unit_can_move_through(u, plot))
    }

    /// C++ `CvSelectionGroup::canBombard`.
    pub fn group_can_bombard(&self, g: GroupId, plot: PlotIdx) -> bool {
        self.groups[g].units.iter().any(|&u| self.unit_can_bombard(u, plot))
    }

    /// C++ `CvSelectionGroup::isAmphibPlot`.
    pub fn group_is_amphib_plot(&self, g: GroupId, plot: PlotIdx) -> bool {
        let mut friendly = true;
        if let Some(head) = self.group_get_head_unit(g) {
            friendly = self.plot_is_friendly_city(plot, head, true);
        }

        self.group_get_domain_type(g) == Some(DomainType::DOMAIN_SEA) && self.plot_is_coastal_land(plot, -1) && !friendly
    }

    // ------------------------------------------------------------------
    // Moving and fighting
    // ------------------------------------------------------------------

    /// C++ `CvSelectionGroup::groupDeclareWar` (C++ default `bForce = false`): returns true if
    /// the group was bumped by the war declaration.
    pub fn group_declare_war(&mut self, g: GroupId, plot: PlotIdx, force: bool) -> bool {
        let team = self.group_get_team(g);
        let plot_team = self.plot_get_team(plot);

        if !self.group_ai_is_declare_war(g, Some(plot)) {
            return false;
        }

        let num_units_before = num_units(self, g);

        if force || !can_enter_area(self, g, plot_team, self.map.plots[plot].area, true) {
            if let Some(plot_team) = plot_team {
                if self.team_ai_is_sneak_attack_ready(team, plot_team) {
                    if self.team_can_declare_war(team, plot_team) {
                        self.team_declare_war(team, plot_team, true, None);
                    }
                }
            }
        }

        num_units_before != num_units(self, g)
    }

    /// C++ `CvSelectionGroup::groupAttack`: returns `(attack made, bFailedAlreadyFighting)`.
    pub fn group_attack(&mut self, g: GroupId, x: i32, y: i32, flags: i32) -> (bool, bool) {
        let mut dest_plot = self.map.plot_xy(x, y);

        if flags & MOVE_THROUGH_ENEMY != 0 {
            if let (Some(from), Some(dest)) = (self.group_plot(g), dest_plot) {
                if self.group_generate_path(g, from, dest, flags, false).is_some() {
                    dest_plot = self.group_get_path_first_plot(g);
                }
            }
        }

        // BETTER AI (Groupattack Spellcasting) Sephi: "Do some nasty Spells before we attack".
        let owner = self.groups[g].owner;
        if !self.players[owner].human {
            // AI: when the target is adjacent (`(getX() - iX)^2 + (getY() - iY)^2 < 4`), every unit
            // that has not cast yet tries `AI_DirectDamageCast(5)` (direct damage casters),
            // `AI_SummonCast()` (summoners), `AI_DirectDamageCast(1)`, `AI_BuffCast()` (buffers):
            // AI auto-casting, omitted.

            if let Some(dest) = dest_plot {
                if let Some(city) = self.map.plots[dest].plot_city {
                    let team = self.group_get_team(g);
                    if self.plot_get_team(dest).is_some_and(|dest_team| self.teams[team].at_war[dest_team]) {
                        let bombard_turns = self.group_get_bombard_turns(g, city);
                        if bombard_turns > 0 && bombard_turns < 8 {
                            for u in units_snapshot(self, g) {
                                if !is_group_member(self, u, g) {
                                    continue;
                                }
                                // Good Time to use Siege Units
                                if let Some(group_plot) = self.group_plot(g) {
                                    if self.unit_can_bombard(u, group_plot) {
                                        self.unit_bombard(u);
                                    }
                                }
                            }

                            // Bombard some more turns
                            if let Some(head) = self.group_get_head_unit(g) {
                                self.unit_finish_moves(head);
                            }
                            // C++ quirk: returns true without assigning bFailedAlreadyFighting.
                            return (true, false);
                        }
                    }
                }
            }
        }

        let Some(dest_plot) = dest_plot else {
            // C++ FAssertMsg(pDestPlot != NULL) and dereferences it below.
            return (false, false);
        };

        let human = self.group_is_human(g);
        let domain = self.group_get_domain_type(g);
        let stack = human
            && (domain == Some(DomainType::DOMAIN_AIR) || self.players[owner].options[PlayerOptionType::PLAYEROPTION_STACK_ATTACK]);

        let mut attack = false;
        let mut failed_already_fighting = false;
        if num_units(self, g) > 0 {
            let (dest_x, dest_y) = self.map.xy(dest_plot);
            if domain == Some(DomainType::DOMAIN_AIR) || self.step_distance(self.group_get_x(g), self.group_get_y(g), dest_x, dest_y) == 1 {
                let can_attack_dest = flags & MOVE_DIRECT_ATTACK != 0
                    || domain == Some(DomainType::DOMAIN_AIR)
                    || flags & MOVE_THROUGH_ENEMY != 0
                    || (self.group_plot(g).is_some_and(|from| self.group_generate_path(g, from, dest_plot, flags, false).is_some())
                        && self.group_get_path_first_plot(g) == Some(dest_plot));
                if can_attack_dest {
                    let (best_attack_unit, _) = self.group_ai_get_best_group_attacker(g, dest_plot, true, false, false);

                    if let Some(best_attack_unit) = best_attack_unit {
                        // if there are no defenders, do not attack
                        let best_defender =
                            self.plot_get_best_defender(dest_plot, None, Some(owner), Some(best_attack_unit), true, false, false);
                        if best_defender.is_none() {
                            return (false, false);
                        }

                        // Alertness (Pep, imported by Xienwolf): no accidental attacks into plots that
                        // were not visible when the move mission started.
                        if human && !self.groups[g].last_plot_visible {
                            return (false, false);
                        }

                        let no_blitz = !self.unit_is_blitz(best_attack_unit) || !self.units[best_attack_unit].made_attack;

                        if self.group_declare_war(g, dest_plot, false) {
                            return (true, false);
                        }

                        loop {
                            let (best, attack_odds) = self.group_ai_get_best_group_attacker(g, dest_plot, false, false, no_blitz);
                            let Some(mut best_attack_unit) = best else {
                                break;
                            };

                            if attack_odds < 68 {
                                if let Some(best_sacrifice) = self.group_ai_get_best_group_sacrifice(g, dest_plot, false, false, no_blitz) {
                                    best_attack_unit = best_sacrifice;
                                }
                            }

                            attack = true;

                            if py_do_combat(self, g, dest_plot) {
                                break;
                            }

                            if num_units(self, g) > 1 {
                                let attacker_plot = self.unit_plot(best_attack_unit);
                                if self.plot_is_fighting(attacker_plot) || self.plot_is_fighting(dest_plot) {
                                    // BETTER AI (Groupattack) Sephi: don't care about summons.
                                    if self.units[best_attack_unit].duration == 0 {
                                        failed_already_fighting = true;
                                    }
                                } else {
                                    // C++ attack(pDestPlot, bStack): bQuick is dropped (always quick).
                                    self.unit_attack(best_attack_unit, dest_plot);
                                }
                            } else {
                                self.unit_attack(best_attack_unit, dest_plot);
                                break;
                            }

                            if failed_already_fighting || !stack {
                                // if this is AI stack, follow through with the attack to the end
                                if !human && num_units(self, g) > 1 {
                                    self.group_ai_queue_group_attack(g, x, y);
                                }

                                break;
                            }
                        }
                    }
                }
            }
        }

        (attack, failed_already_fighting)
    }

    /// C++ `CvSelectionGroup::groupMove` (C++ defaults `pCombatUnit = NULL, bEndMove = false`).
    pub fn group_move(&mut self, g: GroupId, plot: PlotIdx, combat: bool, combat_unit: Option<UnitId>, _end_move: bool) {
        // Alertness (Pep / Xienwolf): C++ quirk: `bSentryAlert = isHuman() && sentryAlert() &&
        // head mission == MISSION_MOVE_TO` is computed but never used (sentryAlert is const), so
        // it is not evaluated here.

        for u in units_snapshot(self, g) {
            if !is_group_member(self, u, g) {
                continue;
            }

            let can_move = self.unit_can_move(u)
                && if combat && (!self.unit_is_no_capture(u) || !self.plot_is_enemy_city(plot, u)) {
                    self.unit_can_move_or_attack_into(u, plot, false)
                } else {
                    self.unit_can_move_into(u, plot, false, false, false)
                };

            if can_move || Some(u) == combat_unit {
                self.unit_move(u, plot);
            } else {
                // joinGroup(NULL, true): bRemoveSelected is UI; bRejoin defaults to true.
                self.unit_join_group(u, None, true);
                // UI: ExecuteMove.
            }
        }

        // UI: execute move (`ExecuteMove` of every unit when `bEndMove || !canAllMove()`).
    }

    /// C++ `CvSelectionGroup::groupPathTo`: returns true if a move was made.
    pub fn group_path_to(&mut self, g: GroupId, x: i32, y: i32, flags: i32) -> bool {
        if self.group_at(g, x, y) {
            return false; // XXX is this necessary?
        }

        debug_assert!(!self.group_is_busy(g));
        debug_assert!(!self.groups[g].mission_queue.is_empty());

        let Some(dest_plot) = self.map.plot_xy(x, y) else {
            // C++ FAssertMsg(pDestPlot != NULL) and dereferences it.
            return false;
        };

        let path_plot;
        if self.group_get_domain_type(g) == Some(DomainType::DOMAIN_AIR) {
            if !self.group_can_move_into(g, dest_plot, false) {
                return false;
            }

            path_plot = dest_plot;
        } else {
            let Some(from) = self.group_plot(g) else {
                return false;
            };
            if self.group_generate_path(g, from, dest_plot, flags, false).is_none() {
                return false;
            }

            path_plot = self.group_get_path_first_plot(g).expect("groupPathTo: path without a first plot");

            if self.group_amphib_move(g, path_plot, flags) {
                return false;
            }
        }

        let mut force = false;
        let mission_ai = self.groups[g].ai_mission_ai_type;
        if mission_ai == Some(MissionAIType::MISSIONAI_BLOCKADE) || mission_ai == Some(MissionAIType::MISSIONAI_PILLAGE) {
            force = true;
        }

        if self.group_declare_war(g, path_plot, force) {
            return false;
        }

        let end_move = path_plot == dest_plot;

        self.group_move(g, path_plot, flags & MOVE_THROUGH_ENEMY != 0, None, end_move);

        true
    }

    /// C++ `CvSelectionGroup::groupRoadTo`: returns true if a move or build was made.
    pub fn group_road_to(&mut self, g: GroupId, x: i32, y: i32, flags: i32) -> bool {
        if !self.group_ai_is_controlled(g) || !self.group_at(g, x, y) || self.groups[g].mission_queue.len() == 1 {
            if let Some(plot) = self.group_plot(g) {
                let (_best_route, best_build) = self.group_get_best_build_route(g, plot);

                if let Some(best_build) = best_build {
                    self.group_build(g, best_build);
                    return true;
                }
            }
        }

        self.group_path_to(g, x, y, flags)
    }

    /// C++ `CvSelectionGroup::groupAmphibMove`: returns true if a landing was attempted.
    pub fn group_amphib_move(&mut self, g: GroupId, plot: PlotIdx, flags: i32) -> bool {
        let mut landing = false;

        if self.group_declare_war(g, plot, false) {
            return true;
        }

        if self.group_is_amphib_plot(g, plot) {
            // Alertness (Pep / Xienwolf): no landings onto plots that were not revealed when the
            // move mission started.
            if self.group_is_human(g) {
                if !self.groups[g].last_plot_revealed {
                    return false;
                }
            }

            let (plot_x, plot_y) = self.map.xy(plot);
            if self.step_distance(self.group_get_x(g), self.group_get_y(g), plot_x, plot_y) == 1 {
                for transport in units_snapshot(self, g) {
                    if !is_group_member(self, transport, g) {
                        continue;
                    }

                    if self.units[transport].cargo > 0 && self.unit_domain_cargo(transport) == Some(DomainType::DOMAIN_LAND) {
                        let mut cargo_groups: Vec<GroupId> = Vec::new();
                        for cargo in self.unit_get_cargo_units(transport) {
                            // C++ pushes `getGroup()` (never NULL for a unit on the map).
                            if let Some(cargo_group) = self.units[cargo].group {
                                if !cargo_groups.contains(&cargo_group) {
                                    cargo_groups.push(cargo_group);
                                }
                            }
                        }

                        for cargo_group in cargo_groups {
                            if !self.groups.contains(cargo_group) {
                                continue;
                            }
                            if self.group_can_all_move(cargo_group) {
                                debug_assert!(!self.group_at(cargo_group, plot_x, plot_y));
                                self.group_push_mission(
                                    cargo_group,
                                    MissionType::MISSION_MOVE_TO,
                                    plot_x,
                                    plot_y,
                                    MOVE_IGNORE_DANGER | flags,
                                    false,
                                    false,
                                    None,
                                    None,
                                    None,
                                );
                                landing = true;
                            }
                        }
                    }
                }
            }
        }

        landing
    }

    /// C++ `CvSelectionGroup::setTransportUnit` (`None` unloads; splits the group if the
    /// transport cannot carry all of it).
    pub fn group_set_transport_unit(&mut self, g: GroupId, transport: Option<UnitId>) {
        // if we are loading
        if let Some(transport) = transport {
            let head = self.group_get_head_unit(g).expect("non-zero group without head unit");

            let cargo_space_available = self.unit_cargo_space_available(
                transport,
                self.unit_get_special_unit_type(head),
                Some(self.unit_get_domain_type(head)),
            );

            // if no space at all, give up
            if cargo_space_available < 1 {
                return;
            }

            // if there is space, but not enough to fit whole group, then split us, and set on the new group
            if cargo_space_available < num_units(self, g) as i32 {
                let (split_group, _) = self.group_split_group(g, cargo_space_available, None);
                if let Some(split_group) = split_group {
                    self.group_set_transport_unit(split_group, Some(transport));
                }
                return;
            }

            debug_assert!(cargo_space_available >= num_units(self, g) as i32, "cargo size too small");

            // setTransportUnit removes the unit from the current group (at least currently), so we
            // have to be careful in the loop here: loop until we do not load one.
            loop {
                let mut loaded_one = false;

                // loop over all the units, finding one to load
                for u in units_snapshot(self, g) {
                    if !is_group_member(self, u, g) {
                        continue;
                    }

                    // just in case implementation of setTransportUnit changes, check to make sure
                    // this unit is not already loaded
                    if self.units[u].transport_unit != Some(transport) {
                        // if there is room, load the unit and stop the loop (since
                        // setTransportUnit ungroups this unit currently)
                        // C++ quirk: the int result is used as a bool (any non-zero value).
                        let space_available = self.unit_cargo_space_available(
                            transport,
                            self.unit_get_special_unit_type(u),
                            Some(self.unit_get_domain_type(u)),
                        ) != 0;
                        if space_available {
                            self.unit_set_transport_unit(u, Some(transport));
                            loaded_one = true;
                            break;
                        }
                    }
                }

                if !loaded_one {
                    break;
                }
            }
        }
        // otherwise we are unloading
        else {
            // loop over all the units, unloading them
            for u in units_snapshot(self, g) {
                if !is_group_member(self, u, g) {
                    continue;
                }

                // unload unit
                self.unit_set_transport_unit(u, None);
            }
        }
    }

    /// C++ `CvSelectionGroup::getBombardTurns` (BBAI, defined in CvSelectionGroupAI.cpp;
    /// used by `groupAttack` for AI groups): -1 if the group cannot bombard.
    pub fn group_get_bombard_turns(&self, g: GroupId, city: CityId) -> i32 {
        let owner = self.groups[g].owner;
        let area = self.group_area(g).expect("getBombardTurns: group without an area");

        let has_bomber = self.map.areas[area].num_ai_units[owner][UnitAIType::UNITAI_ATTACK_AIR] > 0;
        let mut ignore_building_defense = has_bomber;
        let mut total_bombard_rate = if has_bomber { 16 } else { 0 };

        for &u in &self.groups[g].units {
            if self.unit_bombard_rate(u) > 0 {
                total_bombard_rate += self.unit_bombard_rate(u);
                ignore_building_defense = ignore_building_defense || self.unit_ignore_building_defense(u);
            }
        }

        let mut bombard_turns = self.city_defense_modifier(city, ignore_building_defense);
        if !ignore_building_defense {
            bombard_turns *= 100;
            bombard_turns /= 25.max(100 - self.cities[city].building_bombard_defense);
        }
        bombard_turns /= 8.max(total_bombard_rate);

        // added Sephi
        if total_bombard_rate == 0 {
            return -1;
        }

        bombard_turns
    }

    /// C++ `CvSelectionGroupAI::AI_isDeclareWar` (`plot = None` allowed): whether this
    /// (non-human) group may declare war to move into `plot`. Read by pathfinding and
    /// `CvUnit`. Non-const in C++, read-only.
    pub fn group_ai_is_declare_war(&self, g: GroupId, plot: Option<PlotIdx>) -> bool {
        debug_assert!(self.group_get_head_unit(g).is_some());

        if self.group_is_human(g) {
            return false;
        }

        let mut limited_war = false;
        if let Some(plot) = plot {
            if let Some(plot_team) = self.plot_get_team(plot) {
                let war_plan = self.teams[self.group_get_team(g)].ai_war_plan[plot_team];
                if war_plan == Some(WarPlanType::WARPLAN_LIMITED) {
                    limited_war = true;
                }
            }
        }

        if let Some(head) = self.group_get_head_unit(g) {
            match self.units[head].ai_type {
                Some(UnitAIType::UNITAI_UNKNOWN)
                | Some(UnitAIType::UNITAI_ANIMAL)
                | Some(UnitAIType::UNITAI_SETTLE)
                | Some(UnitAIType::UNITAI_WORKER) => {}

                Some(UnitAIType::UNITAI_ATTACK_CITY)
                | Some(UnitAIType::UNITAI_ATTACK_CITY_LEMMING)
                // BETTER AI (New UNITAI) Sephi
                | Some(UnitAIType::UNITAI_WARWIZARD) => return true,

                Some(UnitAIType::UNITAI_ATTACK) | Some(UnitAIType::UNITAI_COLLATERAL) | Some(UnitAIType::UNITAI_PILLAGE) => {
                    if limited_war {
                        return true;
                    }
                }

                Some(UnitAIType::UNITAI_PARADROP)
                | Some(UnitAIType::UNITAI_RESERVE)
                | Some(UnitAIType::UNITAI_COUNTER)
                | Some(UnitAIType::UNITAI_CITY_DEFENSE)
                | Some(UnitAIType::UNITAI_CITY_COUNTER)
                | Some(UnitAIType::UNITAI_CITY_SPECIAL)
                | Some(UnitAIType::UNITAI_EXPLORE)
                | Some(UnitAIType::UNITAI_MISSIONARY)
                | Some(UnitAIType::UNITAI_PROPHET)
                | Some(UnitAIType::UNITAI_ARTIST)
                | Some(UnitAIType::UNITAI_SCIENTIST)
                | Some(UnitAIType::UNITAI_GENERAL)
                | Some(UnitAIType::UNITAI_MERCHANT)
                | Some(UnitAIType::UNITAI_ENGINEER)
                | Some(UnitAIType::UNITAI_SPY)
                | Some(UnitAIType::UNITAI_ICBM)
                | Some(UnitAIType::UNITAI_WORKER_SEA)
                // BETTER AI (New UNITAI) Sephi
                | Some(UnitAIType::UNITAI_MAGE)
                | Some(UnitAIType::UNITAI_TERRAFORMER)
                | Some(UnitAIType::UNITAI_MANA_UPGRADE)
                | Some(UnitAIType::UNITAI_BARBSMASHER)
                | Some(UnitAIType::UNITAI_HERO)
                | Some(UnitAIType::UNITAI_FEASTING)
                | Some(UnitAIType::UNITAI_MEDIC) => {}

                Some(UnitAIType::UNITAI_ATTACK_SEA) | Some(UnitAIType::UNITAI_RESERVE_SEA) | Some(UnitAIType::UNITAI_ESCORT_SEA) => {
                    if limited_war {
                        return true;
                    }
                }

                Some(UnitAIType::UNITAI_EXPLORE_SEA) => {}

                Some(UnitAIType::UNITAI_ASSAULT_SEA) => {
                    if self.group_has_cargo(g) {
                        return true;
                    }
                }

                Some(UnitAIType::UNITAI_SETTLER_SEA)
                | Some(UnitAIType::UNITAI_MISSIONARY_SEA)
                | Some(UnitAIType::UNITAI_SPY_SEA)
                | Some(UnitAIType::UNITAI_CARRIER_SEA)
                | Some(UnitAIType::UNITAI_MISSILE_CARRIER_SEA)
                | Some(UnitAIType::UNITAI_PIRATE_SEA)
                | Some(UnitAIType::UNITAI_ATTACK_AIR)
                | Some(UnitAIType::UNITAI_DEFENSE_AIR)
                | Some(UnitAIType::UNITAI_CARRIER_AIR)
                | Some(UnitAIType::UNITAI_MISSILE_AIR) => {}

                other => debug_assert!(false, "AI_isDeclareWar: unexpected unit AI {other:?}"),
            }
        }

        false
    }

    /// C++ `CvSelectionGroupAI::AI_getBestGroupAttacker` (C++ defaults `bForce = false,
    /// bNoBlitz = false`): picks the attacking unit in `groupAttack` (for humans too).
    /// Returns `(unit, iUnitOdds)`.
    pub fn group_ai_get_best_group_attacker(&self, g: GroupId, plot: PlotIdx, potential_enemy: bool, force: bool, no_blitz: bool) -> (Option<UnitId>, i32) {
        let mut best_value = 0;
        let mut best_odds = 0;
        let mut best_unit = None;

        let units = &self.groups[g].units;

        let is_human = units.first().map_or(true, |&head| self.players[self.units[head].owner].human);

        for &loop_unit in units {
            if !self.unit_is_dead(loop_unit) {
                let can_attack = if self.unit_get_domain_type(loop_unit) == DomainType::DOMAIN_AIR {
                    self.unit_can_air_attack(loop_unit)
                } else {
                    let mut can_attack = self.unit_can_attack(loop_unit);

                    if can_attack && no_blitz && self.unit_is_blitz(loop_unit) && self.units[loop_unit].made_attack {
                        can_attack = false;
                    }
                    can_attack
                };

                if can_attack {
                    if force || self.unit_can_move(loop_unit) {
                        if force || self.unit_can_move_into(loop_unit, plot, true, potential_enemy, false) {
                            let odds = ai_attack_odds(self, loop_unit, plot, potential_enemy);

                            let mut value = odds;
                            // BETTER AI (Block some Units from attacking at low odds) Sephi
                            if !self.players[self.units[loop_unit].owner].human {
                                let unit_ai = self.units[loop_unit].ai_type;
                                if unit_ai == Some(UnitAIType::UNITAI_WARWIZARD) {
                                    if odds < 95 {
                                        value = 0; //Need to put the min to 1 to prevent WoC
                                    }
                                }

                                if unit_ai == Some(UnitAIType::UNITAI_HERO) {
                                    if odds < 85 {
                                        value = 0;
                                    }
                                }
                                if self.units[loop_unit].level > 4 && odds < 95 {
                                    value /= 2;
                                }
                            }
                            value = value.max(1);
                            debug_assert!(value > 0, "iValue is expected to be greater than 0");

                            if self.unit_collateral_damage(loop_unit) > 0 {
                                let possible_targets = (self.plot_get_num_visible_enemy_defenders(plot, loop_unit) - 1)
                                    .min(self.unit_collateral_damage_max_units(loop_unit));

                                if possible_targets > 0 {
                                    value *= 100 + ((self.unit_collateral_damage(loop_unit) * possible_targets) / 5);
                                    value /= 100;
                                }
                            }

                            // if non-human, prefer the last unit that has the best value (so as to avoid splitting the group)
                            if value > best_value || (!is_human && value > 0 && value == best_value) {
                                best_value = value;
                                best_odds = odds;
                                best_unit = Some(loop_unit);
                            }
                        }
                    }
                }
            }
        }

        (best_unit, best_odds)
    }

    /// C++ `CvSelectionGroupAI::AI_getBestGroupSacrifice` (C++ defaults `bForce = false,
    /// bNoBlitz = false`).
    pub fn group_ai_get_best_group_sacrifice(&self, g: GroupId, plot: PlotIdx, _potential_enemy: bool, force: bool, no_blitz: bool) -> Option<UnitId> {
        let mut best_value = 0;
        let mut best_unit = None;

        for &loop_unit in &self.groups[g].units {
            if !self.unit_is_dead(loop_unit) {
                let can_attack = if self.unit_get_domain_type(loop_unit) == DomainType::DOMAIN_AIR {
                    self.unit_can_air_attack(loop_unit)
                } else {
                    let mut can_attack = self.unit_can_attack(loop_unit);

                    if can_attack && no_blitz && self.unit_is_blitz(loop_unit) && self.units[loop_unit].made_attack {
                        can_attack = false;
                    }
                    can_attack
                };

                if can_attack {
                    if force || self.unit_can_move(loop_unit) {
                        // C++ quirk: bPotentialEnemy is not passed on (canMoveInto(pPlot, true)).
                        if force || self.unit_can_move_into(loop_unit, plot, true, false, false) {
                            let mut value = ai_sacrifice_value(self, loop_unit, Some(plot));
                            debug_assert!(value > 0, "iValue is expected to be greater than 0");

                            // BETTER AI (Summons make good groupattack sacrifice units) Sephi
                            if self.units[loop_unit].duration > 0 {
                                value += 10000;
                            }

                            // BETTER AI (Block some Units from attacking at low odds) Sephi
                            if !self.players[self.units[loop_unit].owner].human {
                                let unit_ai = self.units[loop_unit].ai_type;
                                if unit_ai == Some(UnitAIType::UNITAI_WARWIZARD) {
                                    value = 1;
                                }

                                if unit_ai == Some(UnitAIType::UNITAI_HERO) {
                                    value = 1;
                                }
                                if self.units[loop_unit].level > 4 {
                                    value = 1;
                                }
                            }

                            // we want to pick the last unit of highest value, so pick the last unit with a good value
                            if value >= best_value {
                                best_value = value;
                                best_unit = Some(loop_unit);
                            }
                        }
                    }
                }
            }
        }

        best_unit
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
        let d = self.d;
        let mut cont = false;

        let Some(plot) = self.group_plot(g) else {
            // C++ dereferences plot() only inside the unit loop (an empty group builds nothing).
            return false;
        };

        if let Some(improvement) = d.builds[build].improvement {
            if self.group_ai_is_controlled(g) {
                let owner = self.groups[g].owner;
                if self.players[owner].options[PlayerOptionType::PLAYEROPTION_SAFE_AUTOMATION] {
                    let plot_improvement = self.map.plots[plot].improvement_type;
                    if plot_improvement.is_some() && plot_improvement != opt_from_i32::<ImprovementType>(d.defines.ruins_improvement) {
                        let bonus = self.plot_get_non_obsolete_bonus_type(plot, self.players[owner].team);
                        if bonus.map_or(true, |bonus| !d.improvements[improvement].improvement_bonus[bonus].bonus_trade) {
                            if d.improvements[improvement].improvement_pillage.is_some() {
                                return false;
                            }
                        }
                    }
                }
                // C++: an `AI_getMissionAIType() == MISSION_BUILD` best-build check is commented out.
            }
        }

        for u in units_snapshot(self, g) {
            if !is_group_member(self, u, g) {
                continue;
            }

            debug_assert!(self.unit_at_plot(u, plot), "pLoopUnit is expected to be at pPlot");

            if self.unit_can_build(u, plot, build, false) {
                cont = true;

                if self.unit_build(u, build) {
                    cont = false;
                    break;
                }
            }
        }

        cont
    }

    /// C++ `CvSelectionGroup::getBestBuildRoute`: best route any unit can build on `plot`,
    /// with the build that makes it (C++ `*peBestBuild`).
    pub fn group_get_best_build_route(&self, g: GroupId, plot: PlotIdx) -> (Option<RouteType>, Option<BuildType>) {
        let d = self.d;

        let mut best_build = None;
        let mut best_value = 0;
        let mut best_route = None;

        for &loop_unit in &self.groups[g].units {
            for build in d.builds.keys() {
                if let Some(route) = d.builds[build].route {
                    if self.unit_can_build(loop_unit, plot, build, false) {
                        let value = d.routes[route].value;

                        if value > best_value {
                            best_value = value;
                            best_route = Some(route);
                            best_build = Some(build);
                        }
                    }
                }
            }
        }

        (best_route, best_build)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::SelectionGroup;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;
    use crate::unit::{Unit, UnitResetInfo};

    /// An AI game on a map drawn as `rows` (row 0 = y 0): `.` grassland, `^` peak, `~` coast.
    /// Land and water plots get one area each.
    fn test_game(rows: &[&str]) -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 7));
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        let (w, h) = (rows[0].len() as i32, rows.len() as i32);
        g.map = Map::new(d, &MapInitData { grid_w: w, grid_h: h, ..Default::default() }, num_players, num_teams);
        let land = g.map_add_area(false);
        let water = g.map_add_area(true);
        let grass = d.t::<TerrainType>("TERRAIN_GRASS");
        let coast = d.t::<TerrainType>("TERRAIN_COAST");
        for p in g.map.plot_ids() {
            let (x, y) = g.map.xy(p);
            let c = rows[y as usize].as_bytes()[x as usize];
            let plot = &mut g.map.plots[p];
            plot.plot_type = match c {
                b'~' => PlotType::PLOT_OCEAN,
                b'^' => PlotType::PLOT_PEAK,
                _ => PlotType::PLOT_LAND,
            };
            plot.terrain_type = Some(if c == b'~' { coast } else { grass });
            plot.area = Some(if c == b'~' { water } else { land });
        }
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        g
    }

    /// A unit (`addUnit` + `reset`, off the map) alone in a new group (attached directly,
    /// bypassing `joinGroup`).
    fn new_unit_group(g: &mut Game, owner: PlayerId, unit_type: &str) -> (UnitId, GroupId) {
        let d = g.d;
        let t = d.t::<UnitType>(unit_type);
        let id = g.units.next_id();
        let u = g.player_add_unit(owner, Unit::new(id, t, owner, d, &UnitResetInfo::from_info(&d.units[t])));
        g.units[u].ai_type = Some(UnitAIType::UNITAI_ATTACK);
        g.units[u].x = INVALID_PLOT_COORD;
        g.units[u].y = INVALID_PLOT_COORD;

        let id = g.groups.next_id();
        let grp = g.player_add_selection_group(owner, SelectionGroup::new(id, owner));
        g.groups[grp].units.push(u);
        g.units[u].group = Some(grp);
        (u, grp)
    }

    /// `new_unit_group` with the unit put on (x, y) directly (plot unit list only, no `setXY`
    /// bookkeeping): enough for path searches, not for moving the unit.
    fn add_unit_group(g: &mut Game, owner: PlayerId, unit_type: &str, x: i32, y: i32) -> (UnitId, GroupId) {
        let (u, grp) = new_unit_group(g, owner, unit_type);
        g.units[u].x = x;
        g.units[u].y = y;
        let plot = g.map.plot_idx(x, y).unwrap();
        g.map.plots[plot].units.push(u);
        (u, grp)
    }

    #[test]
    fn generate_path_counts_turns_and_costs() {
        let mut g = test_game(&["......", "......", "......"]);
        let (u, grp) = add_unit_group(&mut g, PlayerId(0), "UNIT_WARRIOR", 0, 1);
        assert_eq!(g.unit_max_moves(u), 60, "one move");
        let from = g.map.plot_idx(0, 1).unwrap();
        let to = g.map.plot_idx(4, 1).unwrap();

        // One move per turn: four steps in four turns, straight east (diagonals cost more).
        assert_eq!(g.group_generate_path(grp, from, to, 0, false), Some(4));
        assert_eq!(g.group_get_path_first_plot(grp), g.map.plot_idx(1, 1));
        assert_eq!(g.group_get_path_end_turn_plot(grp), g.map.plot_idx(1, 1));
        {
            let finder = g.path_finder.borrow();
            let last = finder.last_node().unwrap();
            assert_eq!((last.data1, last.data2), (0, 4));
            // Every step ends the turn: PATH_MOVEMENT_WEIGHT * 60 + PATH_TERRITORY_WEIGHT
            // (unowned) + PATH_DEFENSE_WEIGHT * 200 (no defense) + PATH_STEP_WEIGHT.
            assert_eq!(last.known_cost, 4 * (60_000 + 3 + 2_000 + 2));
        }

        // Moves used up: the first step already starts the second turn (pathAdd), unless
        // MOVE_MAX_MOVES counts from full moves.
        g.units[u].moves = 60;
        assert_eq!(g.group_generate_path(grp, from, to, 0, false), Some(5));
        assert_eq!(g.group_generate_path(grp, from, to, MOVE_MAX_MOVES, false), Some(4));

        // The group's own plot: success without a step.
        assert_eq!(g.group_generate_path(grp, from, from, 0, false), Some(1));
        assert_eq!(g.group_get_path_first_plot(grp), Some(from));

        g.group_reset_path(grp);
        assert_eq!(g.group_get_path_first_plot(grp), None);
    }

    #[test]
    fn generate_path_avoids_peaks_and_water() {
        let mut g = test_game(&["......", ".^^^..", "......", "~~~..."]);
        let (u, grp) = add_unit_group(&mut g, PlayerId(0), "UNIT_HORSEMAN", 0, 1);
        let moves = g.unit_max_moves(u) / g.d.defines.move_denominator;
        let from = g.map.plot_idx(0, 1).unwrap();
        let to = g.map.plot_idx(4, 1).unwrap();

        let turns = g.group_generate_path(grp, from, to, 0, false).expect("a path around the peaks");
        assert_eq!(turns, (4 + moves - 1) / moves, "four steps");
        let mut node = g.path_finder.borrow().last_node().cloned();
        while let Some(n) = node {
            let p = g.map.plot_idx(n.x, n.y).unwrap();
            assert!(!g.plot_is_peak(p) && !g.plot_is_water(p), "path through ({}, {})", n.x, n.y);
            node = n.parent.map(|i| g.path_finder.borrow().node(i).clone());
        }

        // Land units cannot enter water (pathDestValid) nor peaks.
        let coast = g.map.plot_idx(1, 3).unwrap();
        assert_eq!(g.group_generate_path(grp, from, coast, 0, false), None);
        let peak = g.map.plot_idx(2, 1).unwrap();
        assert_eq!(g.group_generate_path(grp, from, peak, 0, false), None);
    }

    /// pushMission -> startMission -> continueMission (groupAttack, groupPathTo, groupMove) and
    /// autoMission on the following turns.
    #[test]
    fn move_to_mission_moves_one_step_per_turn() {
        let mut g = test_game(&["......", "......", "......"]);
        let p = PlayerId(0);
        let (u, grp) = new_unit_group(&mut g, p, "UNIT_WARRIOR");
        g.unit_set_xy(u, 0, 1, true);
        g.players[p].turn_active = true;

        // pushMission(MISSION_MOVE_TO) starts the mission: one step east, then out of moves.
        g.group_push_mission(grp, MissionType::MISSION_MOVE_TO, 4, 1, 0, false, false, None, None, None);
        assert_eq!((g.units[u].x, g.units[u].y), (1, 1));
        assert_eq!(g.groups[grp].activity_type, ActivityType::ACTIVITY_MISSION);
        assert_eq!(g.groups[grp].mission_queue.len(), 1, "the mission continues next turn");
        assert_eq!(g.groups[grp].mission_timer, 0, "no human watches: no animation delay");

        // Next turn: autoMission continues it.
        g.units[u].moves = 0;
        g.group_auto_mission(grp);
        assert_eq!((g.units[u].x, g.units[u].y), (2, 1));
        g.units[u].moves = 0;
        g.group_auto_mission(grp);
        g.units[u].moves = 0;
        g.group_auto_mission(grp);
        assert_eq!((g.units[u].x, g.units[u].y), (4, 1));
        assert!(g.groups[grp].mission_queue.is_empty(), "arrived: the mission is deleted");
        assert_eq!(g.groups[grp].activity_type, ActivityType::ACTIVITY_AWAKE);
    }

    #[test]
    fn declare_war_depends_on_head_unit_ai() {
        let mut g = test_game(&["...."]);
        let (u, grp) = add_unit_group(&mut g, PlayerId(0), "UNIT_WARRIOR", 0, 0);
        let plot = g.map.plot_idx(1, 0).unwrap();
        g.units[u].ai_type = Some(UnitAIType::UNITAI_ATTACK_CITY);
        assert!(g.group_ai_is_declare_war(grp, Some(plot)));
        g.units[u].ai_type = Some(UnitAIType::UNITAI_WORKER);
        assert!(!g.group_ai_is_declare_war(grp, None));
        // Attack AIs only on a limited war plan against the plot's team (unowned: none).
        g.units[u].ai_type = Some(UnitAIType::UNITAI_ATTACK);
        assert!(!g.group_ai_is_declare_war(grp, Some(plot)));
        // Human groups never declare war this way.
        g.players[PlayerId(0)].human = true;
        g.units[u].ai_type = Some(UnitAIType::UNITAI_ATTACK_CITY);
        assert!(!g.group_ai_is_declare_war(grp, Some(plot)));
    }
}
