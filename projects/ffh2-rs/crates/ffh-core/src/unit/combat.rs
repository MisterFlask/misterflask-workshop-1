//! CvUnit combat: strength (`maxCombatStr` & co., `isBetterDefenderThan`, attack/defend tests,
//! air strength and damage) and resolution (land and air combat, collateral and flanking damage,
//! interception, air and range strikes, combat state), plus the FfH combat additions
//! (`combatWon`, defensive strikes).
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges; all of them are ported.
//!
//! Headless combat is never animated: C++ `isCombatVisible` is never true without an
//! interface, so every `bVisible` branch (battle/air-mission animations, `planBattle`, the
//! animation timers they set) is omitted and a combat resolves and finishes in the same call
//! (the C++ `!bVisible` path). The combat-timer countdown at the top of `updateCombat` /
//! `updateAirCombat` is kept: `attackForDamage` still finishes its combat one update later.
//! Interface-only code (messages, facing directions, combat focus, entity notifications,
//! `checkRemoveSelectionAfterAttack`, siege towers, explosion effects) is omitted; the main
//! combat outcomes go to the game log.

use crate::defs::INVALID_PLOT_COORD;
use crate::game::Game;
use crate::ids::*;
use crate::unit::{CombatDetails, DefenderCombatValues};
use crate::utils::{is_world_unit_class, range};
use ffh_data::*;

/// C++ `getGroup()` at call sites that dereference it (a unit on the map always has a group;
/// C++ would crash otherwise).
fn group_of(game: &Game, u: UnitId) -> GroupId {
    game.units[u].group.filter(|&g| game.groups.contains(g)).expect("CvUnit::getGroup: unit has no selection group")
}

/// C++ `getCombatUnit()`: `::getUnit(m_combatUnit)`, NULL once that unit no longer exists.
fn combat_unit_of(game: &Game, u: UnitId) -> Option<UnitId> {
    game.units[u].combat_unit.filter(|&c| game.units.contains(c))
}

/// C++ `pPlot->getOwnerINLINE() == getOwnerINLINE()`: the `bInBorders` argument of the
/// `changeExperience` calls after a combat on `plot`.
fn owns_plot(game: &Game, u: UnitId, plot: PlotIdx) -> bool {
    game.map.plots[plot].owner == Some(game.units[u].owner)
}

/// C++ `std::max(GC.getMOVE_DENOMINATOR(), pPlot->movementCost(this, plot()))`: the moves an
/// attack into `plot` costs when the attacker stays where it is.
fn attack_move_cost(game: &Game, u: UnitId, plot: PlotIdx) -> i32 {
    let from = game.unit_plot(u);
    game.d.defines.move_denominator.max(game.plot_movement_cost(plot, u, from))
}

/// The final step of C++ `maxCombatStr` (FfH Defense Str, Kael 08/18/2007): a strength in
/// hundredths scaled by the total modifier; negative modifiers divide instead of multiply.
fn apply_combat_modifier(str_x100: i32, modifier: i32) -> i32 {
    if modifier > 0 {
        (str_x100 * (modifier + 100)) / 100
    } else {
        (str_x100 * 100) / (100 - modifier)
    }
}

/// The damage part of C++ `getDefenderCombatValues`: `(iOurDamage, iTheirDamage)`, the damage
/// each side takes per lost combat round.
fn combat_round_damages(combat_damage: i32, our_firepower: i32, their_firepower: i32) -> (i32, i32) {
    let strength_factor = (our_firepower + their_firepower + 1) / 2;

    let our_damage = 1.max((combat_damage * (their_firepower + strength_factor)) / (our_firepower + strength_factor));
    let their_damage = 1.max((combat_damage * (our_firepower + strength_factor)) / (their_firepower + strength_factor));
    (our_damage, their_damage)
}

/// C++ `GC.getHandicapInfo(GET_PLAYER(ePlayer).getHandicapType()).getFreeWinsVsBarbs()`.
fn free_wins_vs_barbs(game: &Game, p: PlayerId) -> i32 {
    let handicap = game.player_get_handicap_type(p).expect("player without a handicap");
    game.d.handicaps[handicap].free_wins_vs_barbs
}

/// The text name of a unit's type (for the game log).
fn unit_type_name(d: &GameData, unit_type: UnitType) -> String {
    d.text.name(&d.units[unit_type].base.description).to_string()
}

impl Game {
    /// C++ `CvUnit::updateAirStrike` (CvUnit.cpp:1272-1329).
    /// C++ `bQuick` dropped: the port never shows combat animations (`isCombatVisible` is false),
    /// so the air-mission branch is never taken.
    pub fn unit_update_air_strike(&mut self, u: UnitId, plot: PlotIdx, finish: bool) {
        if !finish {
            if self.unit_is_fighting(u) {
                return;
            }

            if !self.unit_air_strike(u, plot) {
                return;
            }

            // C++ `if (bVisible) { ..air mission animation, combat timer..; return; }`: never
            // visible headless.
        }

        if let Some(defender) = combat_unit_of(self, u) {
            self.unit_set_combat_unit(defender, None, false);
        }
        self.unit_set_combat_unit(u, None, false);
        self.unit_set_attack_plot(u, None, false);

        let group = group_of(self, u);
        self.group_clear_mission_queue(group);

        if self.unit_is_suicide(u) && !self.unit_is_dead(u) {
            self.unit_kill(u, true, None);
        }
    }

    /// C++ `CvUnit::resolveAirCombat` (CvUnit.cpp:1331-1412).
    /// Returns (our damage, interceptor damage), which C++ stores in the `CvAirMissionDefinition`
    /// out-param (read by `updateAirCombat` for its messages only).
    pub fn unit_resolve_air_combat(&mut self, u: UnitId, interceptor: UnitId, plot: PlotIdx) -> (i32, i32) {
        let d = self.d;

        let their_strength = if self.unit_get_domain_type(interceptor) == DomainType::DOMAIN_AIR {
            self.unit_air_curr_combat_str(interceptor, Some(u))
        } else {
            self.unit_curr_combat_str(interceptor, None, None, None)
        };
        let our_strength = if self.unit_get_domain_type(u) == DomainType::DOMAIN_AIR {
            self.unit_air_curr_combat_str(u, Some(interceptor))
        } else {
            self.unit_curr_combat_str(u, None, None, None)
        };
        let total_strength = our_strength + their_strength;
        if total_strength == 0 {
            // (C++ FAssert(false); the mission damages stay 0)
            return (0, 0);
        }

        let our_odds = (100 * our_strength) / 1.max(total_strength);

        let our_round_damage = (self.unit_curr_interception_probability(interceptor) * d.defines.max_interception_damage) / 100;
        let mut their_round_damage = (self.unit_curr_interception_probability(u) * d.defines.max_interception_damage) / 100;
        if self.unit_get_domain_type(u) == DomainType::DOMAIN_AIR {
            their_round_damage = d.defines.min_interception_damage.max(their_round_damage);
        }

        let mut their_damage = 0;
        let mut our_damage = 0;

        let owner = self.units[u].owner;
        let interceptor_owner = self.units[interceptor].owner;
        for _round in 0..d.defines.interception_max_rounds {
            if self.soren_rand_num(100, "Air combat") < our_odds {
                if self.unit_get_domain_type(interceptor) == DomainType::DOMAIN_AIR {
                    their_damage += their_round_damage;
                    self.unit_change_damage(interceptor, their_round_damage, Some(owner));
                    if self.unit_is_dead(interceptor) {
                        break;
                    }
                }
            } else {
                our_damage += our_round_damage;
                self.unit_change_damage(u, our_round_damage, Some(interceptor_owner));
                if self.unit_is_dead(u) {
                    break;
                }
            }
        }

        let min_xp = d.defines.min_experience_per_combat;
        let max_xp = d.defines.max_experience_per_combat;
        if self.unit_is_dead(u) {
            if their_round_damage > 0 {
                // C++ quirk: the interceptor earns the attacker's attack XP value.
                let mut experience = self.unit_attack_xp_value(u);
                experience = (experience * our_strength) / 1.max(their_strength);
                experience = range(experience, min_xp, max_xp);
                let max = self.unit_max_xp_value(u);
                let in_borders = owns_plot(self, interceptor, plot);
                let update_global = !self.unit_is_barbarian(u);
                self.unit_change_experience(interceptor, experience, max, true, in_borders, update_global);
            }
        } else if self.unit_is_dead(interceptor) {
            let mut experience = self.unit_defense_xp_value(interceptor);
            experience = (experience * their_strength) / 1.max(our_strength);
            experience = range(experience, min_xp, max_xp);
            let max = self.unit_max_xp_value(interceptor);
            let in_borders = owns_plot(self, u, plot);
            let update_global = !self.unit_is_barbarian(interceptor);
            self.unit_change_experience(u, experience, max, true, in_borders, update_global);
        } else if our_damage > 0 {
            if their_round_damage > 0 {
                let max = self.unit_max_xp_value(u);
                let in_borders = owns_plot(self, interceptor, plot);
                let update_global = !self.unit_is_barbarian(u);
                self.unit_change_experience(interceptor, d.defines.experience_from_withdrawl, max, true, in_borders, update_global);
            }
        } else if their_damage > 0 {
            let max = self.unit_max_xp_value(interceptor);
            let in_borders = owns_plot(self, u, plot);
            let update_global = !self.unit_is_barbarian(interceptor);
            self.unit_change_experience(u, d.defines.experience_from_withdrawl, max, true, in_borders, update_global);
        }

        (our_damage, their_damage)
    }

    /// C++ `CvUnit::updateAirCombat` (CvUnit.cpp:1415-1582).
    /// C++ `bQuick` dropped (see `unit_update_air_strike`).
    pub fn unit_update_air_combat(&mut self, u: UnitId) {
        let d = self.d;
        let mut finish = false;

        if self.units[u].combat_timer > 0 {
            self.unit_change_combat_timer(u, -1);

            if self.units[u].combat_timer > 0 {
                return;
            } else {
                finish = true;
            }
        }

        let Some(plot) = self.unit_get_attack_plot(u) else {
            return;
        };

        let interceptor = if finish { combat_unit_of(self, u) } else { self.unit_best_interceptor(u, plot) };

        let Some(interceptor) = interceptor else {
            self.unit_set_attack_plot(u, None, false);
            self.unit_set_combat_unit(u, None, false);

            let group = group_of(self, u);
            self.group_clear_mission_queue(group);

            return;
        };

        // C++ `bVisible = isCombatVisible(pInterceptor)` (when !bQuick): never true headless.

        // if not finished and not fighting yet, set up combat damage and mission
        if !finish {
            if !self.unit_is_fighting(u) {
                let our_plot = self.unit_plot(u);
                if self.plot_is_fighting(our_plot) || self.plot_is_fighting(plot) {
                    return;
                }

                self.units[u].made_attack = true;

                self.unit_set_combat_unit(u, Some(interceptor), true);
                self.unit_set_combat_unit(interceptor, Some(u), false);
            }

            // C++ fills a CvAirMissionDefinition (MISSION_PARADROP for non-air units, else
            // MISSION_AIRSTRIKE) for the animation; only the damages are read back (messages).
            let (our_damage, their_damage) = self.unit_resolve_air_combat(u, interceptor, plot);

            // C++ `if (!bVisible) bFinish = true; else { ..mission animation and timer.. }`
            finish = true;

            self.unit_change_moves(u, d.defines.move_denominator);
            if self.unit_get_domain_type(interceptor) != DomainType::DOMAIN_AIR {
                self.units[interceptor].made_interception = true;
            }

            // UI: shot down / hurt / aborted messages to both owners.
            let turn = self.game_turn();
            let (owner, unit_type) = (self.units[u].owner, self.units[u].unit_type);
            let interceptor_type = self.units[interceptor].unit_type;
            let (dead, interceptor_dead) = (self.unit_is_dead(u), self.unit_is_dead(interceptor));
            self.log.msg(turn, Some(owner), || {
                format!(
                    "{} intercepted by {} (damage {} / {}){}{}",
                    unit_type_name(d, unit_type),
                    unit_type_name(d, interceptor_type),
                    our_damage,
                    their_damage,
                    if dead { ", shot down" } else { "" },
                    if interceptor_dead { ", interceptor destroyed" } else { "" }
                )
            });
        }

        if finish {
            self.unit_set_attack_plot(u, None, false);
            self.unit_set_combat_unit(u, None, false);
            self.unit_set_combat_unit(interceptor, None, false);

            if !self.unit_is_dead(u) && self.unit_is_suicide(u) {
                self.unit_kill(u, true, None);
            }
        }
    }

    /// C++ `CvUnit::resolveCombat` (CvUnit.cpp:1584-1783).
    /// The `CvBattleDefinition&` out-param only feeds the battle animation and is dropped.
    /// The `combatLogCalc`/`combatLogHit` Python events (USE_COMBAT_RESULT_CALLBACK) are reporting.
    pub fn unit_resolve_combat(&mut self, u: UnitId, defender: UnitId, plot: PlotIdx) {
        let d = self.d;

        // C++ fills `CombatDetails` for both units here; they only feed the `combatLogCalc` /
        // `combatLogHit` reporting events (human players with USE_COMBAT_RESULT_CALLBACK), which
        // are not ported, so no details are collected.

        // FfH: Modified by Kael 01/14/2009 (the attacker's strength knows its defender)
        let attacker_strength = self.unit_curr_combat_str(u, None, Some(defender), None);
        let attacker_firepower = self.unit_curr_firepower(u, None, Some(defender));

        let values = self.unit_get_defender_combat_values(u, defender, plot, attacker_strength, attacker_firepower, None);
        let DefenderCombatValues {
            their_odds: defender_odds,
            their_strength: defender_strength,
            our_damage: attacker_damage,
            their_damage: defender_damage,
        } = values;
        let attacker_kill_odds = defender_odds * (100 - self.unit_withdrawal_probability(u)) / 100;

        // FfH: Modified by Kael 08/02/2008: `combatLogCalc` event (reporting only, see above).

        self.unit_collateral_combat(u, plot, Some(defender));

        let owner = self.units[u].owner;
        let defender_owner = self.units[defender].owner;
        let min_xp = d.defines.min_experience_per_combat;
        let max_xp = d.defines.max_experience_per_combat;

        loop {
            if self.soren_rand_num(d.defines.combat_die_sides, "Combat") < defender_odds {
                if self.units[u].combat_first_strikes == 0 {
                    if self.units[u].damage + attacker_damage >= self.unit_max_hit_points(u)
                        && self.soren_rand_num(100, "Withdrawal") < self.unit_withdrawal_probability(u)
                    {
                        self.unit_flanking_strike_combat(
                            u,
                            plot,
                            attacker_strength,
                            attacker_firepower,
                            attacker_kill_odds,
                            defender_damage,
                            defender,
                        );

                        let max = self.unit_max_xp_value(defender);
                        let in_borders = owns_plot(self, u, plot);
                        let update_global = !self.unit_is_barbarian(defender);
                        self.unit_change_experience(u, d.defines.experience_from_withdrawl, max, true, in_borders, update_global);

                        // FfH Promotions: Added by Kael 08/12/2007
                        self.units[u].flee_withdrawl = true;

                        break;
                    }

                    self.unit_change_damage(u, attacker_damage, Some(defender_owner));

                    // UI: battle definition first strikes / ranged damage; `combatLogHit` event.
                }
            } else if self.units[defender].combat_first_strikes == 0 {
                // FfH: defensive withdrawal (outside cities)
                if self.units[defender].damage + defender_damage >= self.unit_max_hit_points(defender) {
                    if !self.plot_is_city(plot, false, None) {
                        if self.soren_rand_num(100, "Withdrawal") < self.unit_get_withdrawl_prob_defensive(defender) {
                            self.units[defender].flee_withdrawl = true;
                            break;
                        }
                    }
                }
                // FfH: End Add

                let combat_limit = self.units[u].combat_limit;
                if d.defines.max_hit_points.min(self.units[defender].damage + defender_damage) > combat_limit {
                    let max = self.unit_max_xp_value(defender);
                    let in_borders = owns_plot(self, u, plot);
                    let update_global = !self.unit_is_barbarian(defender);
                    self.unit_change_experience(u, d.defines.experience_from_withdrawl, max, true, in_borders, update_global);
                    self.unit_set_damage(defender, combat_limit, Some(owner));

                    // FfH: Added by Kael 05/27/2008
                    self.units[u].made_attack = true;
                    let cost = attack_move_cost(self, u, plot);
                    self.unit_change_moves(u, cost);
                    // FfH: End Add

                    break;
                }

                self.unit_change_damage(defender, defender_damage, Some(owner));

                // UI: battle definition first strikes / ranged damage; `combatLogHit` event.
            }

            if self.units[u].combat_first_strikes > 0 {
                self.unit_change_combat_first_strikes(u, -1);
            }

            if self.units[defender].combat_first_strikes > 0 {
                self.unit_change_combat_first_strikes(defender, -1);
            }

            if self.unit_is_dead(u) || self.unit_is_dead(defender) {
                if self.unit_is_dead(u) {
                    let mut experience = self.unit_defense_xp_value(u);
                    experience = (experience * attacker_strength) / defender_strength;
                    experience = range(experience, min_xp, max_xp);
                    let max = self.unit_max_xp_value(u);
                    let in_borders = owns_plot(self, defender, plot);
                    let update_global = !self.unit_is_barbarian(u);
                    self.unit_change_experience(defender, experience, max, true, in_borders, update_global);
                } else {
                    self.unit_flanking_strike_combat(
                        u,
                        plot,
                        attacker_strength,
                        attacker_firepower,
                        attacker_kill_odds,
                        defender_damage,
                        defender,
                    );

                    let mut experience = self.unit_attack_xp_value(defender);
                    experience = (experience * defender_strength) / attacker_strength;
                    experience = range(experience, min_xp, max_xp);
                    let max = self.unit_max_xp_value(defender);
                    let in_borders = owns_plot(self, u, plot);
                    let update_global = !self.unit_is_barbarian(defender);
                    self.unit_change_experience(u, experience, max, true, in_borders, update_global);
                }

                break;
            }
        }
    }

    /// C++ `CvUnit::updateCombat` (CvUnit.cpp:1786-2251).
    /// C++ `bQuick` dropped: combat always resolves and finishes in the same call (the C++
    /// `bVisible == false` path); combat timers still count down if set.
    pub fn unit_update_combat(&mut self, u: UnitId) {
        let d = self.d;
        let mut finish = false;

        if self.units[u].combat_timer > 0 {
            self.unit_change_combat_timer(u, -1);

            if self.units[u].combat_timer > 0 {
                return;
            } else {
                finish = true;
            }
        }

        let Some(plot) = self.unit_get_attack_plot(u) else {
            return;
        };

        if self.unit_get_domain_type(u) == DomainType::DOMAIN_AIR {
            self.unit_update_air_strike(u, plot, finish);
            return;
        }

        let owner = self.units[u].owner;
        let defender = if finish {
            combat_unit_of(self, u)
        } else {
            self.plot_get_best_defender(plot, None, Some(owner), Some(u), true, false, false)
        };

        let Some(defender) = defender else {
            self.unit_set_attack_plot(u, None, false);
            self.unit_set_combat_unit(u, None, false);

            let advance_unit = if self.unit_can_advance(u, plot, 0) { Some(u) } else { None };
            let group = group_of(self, u);
            self.group_move(group, plot, true, advance_unit, false);

            let group = group_of(self, u);
            self.group_clear_mission_queue(group);

            return;
        };

        // C++ `bVisible = isCombatVisible(pDefender)` (when !bQuick): never true headless.

        let turn = self.game_turn();

        // FfH: Added by Kael 07/30/2007
        if !self.unit_is_immune_to_defensive_strike(u) {
            self.unit_do_defensive_strike(defender, u);
        }
        if self.unit_is_fear(defender) {
            if !self.unit_is_immune_to_fear(u) {
                let mut chance = self.unit_base_combat_str(u) + 20 + self.units[u].level
                    - self.unit_base_combat_str(defender)
                    - self.units[defender].level;
                if chance < 4 {
                    chance = 4;
                }
                if self.soren_rand_num(40, "Im afeared!") > chance {
                    self.units[u].made_attack = true;
                    let cost = attack_move_cost(self, u, plot);
                    self.unit_change_moves(u, cost);
                    let unit_type = self.units[u].unit_type;
                    self.log.msg(turn, Some(owner), || format!("{} is afraid to attack", unit_type_name(d, unit_type)));
                    finish = true;
                }
            }
        }
        // FfH: End Add

        // if not finished and not fighting yet, set up combat damage and mission
        if !finish {
            if !self.unit_is_fighting(u) {
                let our_plot = self.unit_plot(u);
                if self.plot_is_fighting(our_plot) || self.plot_is_fighting(plot) {
                    return;
                }

                self.units[u].made_attack = true;

                // UI: both units rotate to face each other (`setFacingDirection`; no FfH unit has
                // bLineOfSight, so the facing direction never matters for rules).

                self.unit_set_combat_unit(u, Some(defender), true);
                self.unit_set_combat_unit(defender, Some(u), false);

                let defender_group = group_of(self, defender);
                self.group_clear_mission_queue(defender_group);

                // UI: combat focus camera or "units under attack" message to the defender.
            }

            if !self.unit_can_defend(defender, None) {
                // C++ `if (!bVisible) bFinish = true; else { ..surrender mission and timer.. }`
                finish = true;

                // Kill them!
                self.unit_set_damage(defender, d.defines.max_hit_points, None);
            } else {
                // UI: the CvBattleDefinition (start damages) for the animation.
                self.unit_resolve_combat(u, defender, plot);

                // C++ `if (!bVisible) bFinish = true; else { ..planBattle, combat timer.. }`
                finish = true;
            }
        }

        if finish {
            // UI: release the locked camera, `RemoveUnitFromBattle`, `NotifyEntity`.
            self.unit_set_attack_plot(u, None, false);
            self.unit_set_combat_unit(u, None, false);
            self.unit_set_combat_unit(defender, None, false);

            let team = self.unit_get_team(u);
            let defender_team = self.unit_get_team(defender);
            let defender_owner = self.units[defender].owner;
            let unit_type = self.units[u].unit_type;
            let defender_type = self.units[defender].unit_type;

            if self.unit_is_dead(u) {
                if self.unit_is_barbarian(u) {
                    self.players[defender_owner].wins_vs_barbs += 1;
                }

                // FfH Hidden Nationality: Modified by Kael 08/27/2007
                if !self.unit_is_hidden_nationality(u)
                    && !self.unit_is_hidden_nationality(defender)
                    && self.units[u].duration == 0
                    && !d.units[unit_type].no_war_weariness
                {
                    self.team_change_war_weariness_at_plot(team, defender_team, plot, d.defines.ww_unit_killed_attacking);
                    self.team_change_war_weariness_at_plot(defender_team, team, plot, d.defines.ww_killed_unit_defending);
                    // C++ `AI_changeWarSuccess` (AI bookkeeping, plain add).
                    self.teams[defender_team].ai_war_success[team] += d.defines.war_success_defending;
                }

                self.log.msg(turn, Some(owner), || {
                    format!("{} died attacking {}", unit_type_name(d, unit_type), unit_type_name(d, defender_type))
                });

                // FfH: Added by Kael 07/30/2007
                self.unit_combat_won(defender, u, false);

                // report event to Python, along with some other key state
                // FfH: Modified by Kael 08/02/2008
                if d.defines.use_combat_result_callback != 0 {
                    self.ev_combat_result(defender, u);
                }
            } else if self.unit_is_dead(defender) {
                if self.unit_is_barbarian(defender) {
                    self.players[owner].wins_vs_barbs += 1;
                }

                // FfH Hidden Nationality: Modified by Kael 08/27/2007
                if !self.unit_is_hidden_nationality(u)
                    && !self.unit_is_hidden_nationality(defender)
                    && self.units[defender].duration == 0
                    && !d.units[defender_type].no_war_weariness
                {
                    self.team_change_war_weariness_at_plot(defender_team, team, plot, d.defines.ww_unit_killed_defending);
                    self.team_change_war_weariness_at_plot(team, defender_team, plot, d.defines.ww_killed_unit_attacking);
                    // C++ `AI_changeWarSuccess` (AI bookkeeping, plain add).
                    self.teams[team].ai_war_success[defender_team] += d.defines.war_success_attacking;
                }

                self.log.msg(turn, Some(owner), || {
                    format!("{} destroyed {}", unit_type_name(d, unit_type), unit_type_name(d, defender_type))
                });

                // FfH: Added by Kael 05/15/2007
                self.unit_combat_won(u, defender, true);

                // C++ quirk: `combatWon` can kill the attacker (it loses Hidden next to an enemy
                // with no free plot around, see there), and C++ goes on with the deleted unit, up
                // to `getGroup()->clearMissionQueue()` on a NULL group. Here the defender still
                // dies (`kill(false)`, as in both branches below), uncaptured as nobody advances;
                // the attacker's own follow-up is skipped (its group's missions were cleared when
                // it left the group).
                if !self.units.contains(u) {
                    self.unit_kill(defender, false, None);
                    return;
                }

                // report event to Python, along with some other key state
                // FfH: Modified by Kael 08/02/2008
                if d.defines.use_combat_result_callback != 0 {
                    self.ev_combat_result(u, defender);
                }

                let mut advance = false;

                if self.unit_is_suicide(u) {
                    self.unit_kill(u, true, None);

                    self.unit_kill(defender, false, None);
                } else {
                    let threshold = if self.unit_can_defend(defender, None) { 1 } else { 0 };
                    advance = self.unit_can_advance(u, plot, threshold);

                    if advance {
                        // FfH: Added by Kael 11/14/2007 (equipment is always captured)
                        if !self.unit_is_no_capture(u) || d.units[defender_type].equipment_promotion.is_some() {
                            self.unit_set_capturing_player(defender, Some(owner));
                        }
                    }

                    self.unit_kill(defender, false, None);

                    // FfH Fear: Added by Kael 07/30/2007
                    if self.unit_is_fear(u) && !self.plot_is_city(plot, false, None) {
                        for loop_unit in self.map.plots[plot].units.clone() {
                            // C++ walks the live plot list (a unit that flees leaves it); units
                            // removed meanwhile are skipped.
                            if !self.units.contains(loop_unit) {
                                continue;
                            }
                            if self.unit_is_enemy(loop_unit, Some(team), None) {
                                if !self.unit_is_immune_to_fear(loop_unit) {
                                    let threshold = self.unit_base_combat_str(u) + 10 - self.unit_base_combat_str(loop_unit);
                                    if self.soren_rand_num(20, "Im afeared!") <= threshold {
                                        self.unit_join_group(loop_unit, None, true);
                                        self.unit_withdrawl_to_nearest_valid_plot(loop_unit, false);
                                    }
                                }
                            }
                        }
                    }
                    advance = self.unit_can_advance(u, plot, 0);
                    // FfH: End Add

                    if !advance {
                        let cost = attack_move_cost(self, u, plot);
                        self.unit_change_moves(u, cost);
                        // UI: checkRemoveSelectionAfterAttack.
                    }
                }

                if self.plot_get_num_visible_enemy_defenders(plot, u) == 0 {
                    let group = group_of(self, u);
                    self.group_move(group, plot, true, if advance { Some(u) } else { None }, false);
                }

                // This is is put before the plot advancement, the unit will always try to walk back
                // to the square that they came from, before advancing.
                let group = group_of(self, u);
                self.group_clear_mission_queue(group);
            } else {
                // FfH Promotions: Modified by Kael 08/12/2007 (withdrawals flee)
                if self.units[defender].flee_withdrawl {
                    self.unit_join_group(defender, None, true);
                    self.units[defender].flee_withdrawl = false;
                    self.unit_withdrawl_to_nearest_valid_plot(defender, true);

                    // BUGFfH: Modified by Denev 10/14/2009 (0.41k): when the defender fled, the
                    // attacker loses moves as if it had won.
                    if self.unit_can_advance(u, plot, 0) {
                        let group = group_of(self, u);
                        self.group_move(group, plot, true, Some(u), false);
                    } else {
                        let cost = attack_move_cost(self, u, plot);
                        self.unit_change_moves(u, cost);
                        // UI: checkRemoveSelectionAfterAttack.
                    }

                    let group = group_of(self, u);
                    self.group_clear_mission_queue(group);
                    self.log.msg(turn, Some(owner), || {
                        format!("{} fled from {}", unit_type_name(d, defender_type), unit_type_name(d, unit_type))
                    });
                }
                if self.units[u].flee_withdrawl {
                    self.unit_join_group(u, None, true);
                    self.units[u].flee_withdrawl = false;
                    self.log.msg(turn, Some(owner), || {
                        format!("{} withdrew from {}", unit_type_name(d, unit_type), unit_type_name(d, defender_type))
                    });
                    let cost = attack_move_cost(self, u, plot);
                    self.unit_change_moves(u, cost);
                    // UI: checkRemoveSelectionAfterAttack.
                    let group = group_of(self, u);
                    self.group_clear_mission_queue(group);
                }
                // FfH: End Modify
            }
        }
    }

    /// C++ `CvUnit::isBetterDefenderThan` (CvUnit.cpp:2479-2646).
    /// `defender` `None` -> true. Ties fall back to `isBeforeUnitCycle` (CvGameCoreUtils).
    pub fn unit_is_better_defender_than(&self, u: UnitId, defender: Option<UnitId>, attacker: Option<UnitId>) -> bool {
        let d = self.d;

        let Some(defender) = defender else {
            return true;
        };

        let attacker_team = attacker.map(|a| self.unit_get_team(a));

        if self.unit_can_coexist_with_enemy_unit(u, attacker_team) {
            return false;
        }

        if !self.unit_can_defend(u, None) {
            return false;
        }

        if self.unit_can_defend(u, None) && !self.unit_can_defend(defender, None) {
            return true;
        }

        if let Some(a) = attacker {
            if self.unit_is_target_of(u, a) && !self.unit_is_target_of(defender, a) {
                return true;
            }

            if !self.unit_is_target_of(u, a) && self.unit_is_target_of(defender, a) {
                return false;
            }

            if self.unit_can_attack_unit(a, defender) && !self.unit_can_attack_unit(a, u) {
                return false;
            }

            if self.unit_can_attack_unit(a, u) && !self.unit_can_attack_unit(a, defender) {
                return true;
            }
        }

        // C++ evaluates both strengths on this unit's plot.
        let our_plot = self.unit_plot_opt(u);
        let first_strike_damage = (d.defines.combat_damage * 2) / 5;

        let mut our_defense = self.unit_curr_combat_str(u, our_plot, attacker, None);
        if is_world_unit_class(d, self.unit_get_unit_class_type(u)) {
            our_defense /= 2;
        }

        match attacker {
            None => {
                if self.unit_collateral_damage(defender) > 0 {
                    our_defense *= 100 + self.unit_collateral_damage(defender);
                    our_defense /= 100;
                }

                if self.unit_curr_interception_probability(defender) > 0 {
                    our_defense *= 100 + self.unit_curr_interception_probability(defender);
                    our_defense /= 100;
                }
            }
            Some(a) => {
                if !self.unit_immune_to_first_strikes(a) {
                    our_defense *=
                        ((self.unit_first_strikes(u) * 2) + self.unit_chance_first_strikes(u)) * first_strike_damage + 100;
                    our_defense /= 100;
                }

                if self.unit_immune_to_first_strikes(u) {
                    our_defense *=
                        ((self.unit_first_strikes(a) * 2) + self.unit_chance_first_strikes(a)) * first_strike_damage + 100;
                    our_defense /= 100;
                }
            }
        }

        let mut asset_value = 1.max(d.units[self.units[u].unit_type].asset_value);
        let mut cargo_asset_value = 0;
        for cargo in self.unit_get_cargo_units(u) {
            cargo_asset_value += d.units[self.units[cargo].unit_type].asset_value;
        }
        our_defense = our_defense * asset_value / 1.max(asset_value + cargo_asset_value);

        let mut their_defense = self.unit_curr_combat_str(defender, our_plot, attacker, None);
        if is_world_unit_class(d, self.unit_get_unit_class_type(defender)) {
            their_defense /= 2;
        }

        match attacker {
            None => {
                if self.unit_collateral_damage(u) > 0 {
                    their_defense *= 100 + self.unit_collateral_damage(u);
                    their_defense /= 100;
                }

                if self.unit_curr_interception_probability(u) > 0 {
                    their_defense *= 100 + self.unit_curr_interception_probability(u);
                    their_defense /= 100;
                }
            }
            Some(a) => {
                if !self.unit_immune_to_first_strikes(a) {
                    their_defense *= ((self.unit_first_strikes(defender) * 2) + self.unit_chance_first_strikes(defender))
                        * first_strike_damage
                        + 100;
                    their_defense /= 100;
                }

                if self.unit_immune_to_first_strikes(defender) {
                    their_defense *=
                        ((self.unit_first_strikes(a) * 2) + self.unit_chance_first_strikes(a)) * first_strike_damage + 100;
                    their_defense /= 100;
                }
            }
        }

        asset_value = 1.max(d.units[self.units[defender].unit_type].asset_value);
        cargo_asset_value = 0;
        for cargo in self.unit_get_cargo_units(defender) {
            cargo_asset_value += d.units[self.units[cargo].unit_type].asset_value;
        }
        their_defense = their_defense * asset_value / 1.max(asset_value + cargo_asset_value);

        // FfH Promotions: Added by Kael 07/30/2007
        our_defense *= self.units[u].better_defender_than_percent;
        our_defense /= 100;
        their_defense *= self.units[defender].better_defender_than_percent;
        their_defense /= 100;
        // FfH Promotions: End Add

        if our_defense == their_defense {
            let our_leader = self.units[u].leader_unit_type;
            let their_leader = self.units[defender].leader_unit_type;
            if our_leader.is_none() && their_leader.is_some() {
                our_defense += 1;
            } else if our_leader.is_some() && their_leader.is_none() {
                their_defense += 1;
            } else if self.is_before_unit_cycle(u, defender) {
                our_defense += 1;
            }
        }

        our_defense > their_defense
    }

    /// C++ `CvUnit::attack` (CvUnit.cpp:3459-3467).
    /// C++ `bQuick` dropped.
    pub fn unit_attack(&mut self, u: UnitId, plot: PlotIdx) {
        // (C++ FAssert(canMoveInto(pPlot, true)), FAssert(getCombatTimer() == 0))
        self.unit_set_attack_plot(u, Some(plot), false);

        self.unit_update_combat(u);
    }

    /// C++ `CvUnit::fightInterceptor` (CvUnit.cpp:3469-3476).
    /// C++ `bQuick` dropped.
    pub fn unit_fight_interceptor(&mut self, u: UnitId, plot: PlotIdx) {
        // (C++ FAssert(getCombatTimer() == 0))
        self.unit_set_attack_plot(u, Some(plot), true);

        self.unit_update_air_combat(u);
    }

    /// C++ `CvUnit::attackForDamage` (CvUnit.cpp:3478-3595).
    pub fn unit_attack_for_damage(
        &mut self,
        u: UnitId,
        defender: UnitId,
        attacker_damage_change: i32,
        defender_damage_change: i32,
    ) {
        // (C++ returns for a NULL defender; FAssert(getCombatTimer() == 0), FAssert(!isFighting()))
        let defender_plot = self.unit_plot_opt(defender);
        self.unit_set_attack_plot(u, defender_plot, false);

        let Some(plot) = self.unit_get_attack_plot(u) else {
            return;
        };

        // UI: both units rotate to face each other.

        // C++ `bVisible = isCombatVisible(pDefender)`: never true headless.

        // if not finished and not fighting yet, set up combat damage and mission
        if !self.unit_is_fighting(u) {
            let our_plot = self.unit_plot(u);
            if self.plot_is_fighting(our_plot) || self.plot_is_fighting(plot) {
                return;
            }

            self.unit_set_combat_unit(u, Some(defender), true);
            self.unit_set_combat_unit(defender, Some(u), false);

            let defender_group = group_of(self, defender);
            self.group_clear_mission_queue(defender_group);

            // UI: combat focus camera or "units under attack" message to the defender.
        }

        // UI: the CvBattleDefinition (start damages) for the animation.
        let owner = self.units[u].owner;
        let defender_owner = self.units[defender].owner;
        self.unit_change_damage(u, attacker_damage_change, Some(defender_owner));
        self.unit_change_damage(defender, defender_damage_change, Some(owner));

        // C++ `if (bVisible) { ..planBattle, combat timer.. } else { setCombatTimer(1); }`: the
        // combat finishes in the next `updateCombat`.
        self.units[u].combat_timer = 1;
    }

    /// C++ `CvUnit::isFighting` (CvUnit.cpp:8905-8908).
    pub fn unit_is_fighting(&self, u: UnitId) -> bool {
        // C++ `getCombatUnit() != NULL` (`::getUnit` yields NULL for a dead unit).
        self.units[u].combat_unit.map_or(false, |c| self.units.contains(c))
    }

    /// C++ `CvUnit::isAttacking` (CvUnit.cpp:8911-8914).
    pub fn unit_is_attacking(&self, u: UnitId) -> bool {
        self.unit_get_attack_plot(u).is_some() && !self.units[u].death_delay
    }

    /// C++ `CvUnit::isDefending` (CvUnit.cpp:8917-8920).
    pub fn unit_is_defending(&self, u: UnitId) -> bool {
        self.unit_is_fighting(u) && !self.unit_is_attacking(u)
    }

    /// C++ `CvUnit::isCombat` (CvUnit.cpp:8923-8926).
    pub fn unit_is_combat(&self, u: UnitId) -> bool {
        self.unit_is_fighting(u) || self.unit_is_attacking(u)
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
        let d = self.d;
        let mut details = details;

        // C++ `if (pCombatDetails != NULL) pCombatDetails->iX = value;`
        macro_rules! set_detail {
            ($field:ident, $value:expr) => {
                if let Some(det) = details.as_deref_mut() {
                    det.$field = $value;
                }
            };
        }

        let mut plot = plot;
        let mut attacker = attacker;

        // FfH Damage Types: Added by Kael 09/02/2007 (without a plot, the "attacker" argument
        // is the defender this unit attacks)
        let mut defender: Option<UnitId> = None;
        if plot.is_none() && attacker.is_some() {
            defender = attacker;
            attacker = None;
        }
        // FfH: End Add

        // handle our new special case
        let mut attacked_plot: Option<PlotIdx> = None;
        let mut attacking_unknown_defender = false;
        if attacker == Some(u) {
            attacking_unknown_defender = true;
            attacked_plot = plot;

            // reset these values, we will fiddle with them below
            plot = None;
            attacker = None;
        } else if attacker.is_some() {
            // otherwise, attack plot is the plot of us (the defender)
            attacked_plot = self.unit_plot_opt(u);
        }

        if let Some(det) = details.as_deref_mut() {
            *det = CombatDetails {
                owner: Some(self.units[u].owner),
                visual_owner: Some(self.unit_get_visual_owner(u, None)),
                unit_name: self.unit_get_name(u),
                ..CombatDetails::default()
            };
        }

        // FfH Defense Str: Modified by Kael 08/18/2007 (attacks use the attack strength,
        // everything else the defense strength)
        // C++ quirk: `pAttacker == this` cannot hold any more here (it was reset above).
        let mut str_ = if (attacker.is_none() && plot.is_none()) || attacker == Some(u) {
            self.unit_base_combat_str(u)
        } else {
            self.unit_base_combat_str_defense(u)
        };

        if str_ == 0 {
            return 0;
        }
        // FfH: End Modify

        let team = self.unit_get_team(u);
        let mut modifier = 0;

        let mut extra_modifier = self.unit_get_extra_combat_percent(u);
        modifier += extra_modifier;
        set_detail!(extra_combat_percent, extra_modifier);

        // do modifiers for animals and barbarians (leaving these out for bAttackingUnknownDefender case)
        if let Some(a) = attacker {
            let handicap = &d.handicaps[self.handicap];

            if self.unit_is_animal(u) {
                if self.unit_is_human(a) {
                    extra_modifier = handicap.animal_combat_modifier;
                    modifier += extra_modifier;
                    set_detail!(animal_combat_modifier_ta, extra_modifier);
                } else {
                    extra_modifier = handicap.ai_animal_combat_modifier;
                    modifier += extra_modifier;
                    set_detail!(ai_animal_combat_modifier_ta, extra_modifier);
                }
            }

            if self.unit_is_animal(a) {
                if self.unit_is_human(u) {
                    extra_modifier = -handicap.animal_combat_modifier;
                    modifier += extra_modifier;
                    set_detail!(animal_combat_modifier_aa, extra_modifier);
                } else {
                    extra_modifier = -handicap.ai_animal_combat_modifier;
                    modifier += extra_modifier;
                    set_detail!(ai_animal_combat_modifier_aa, extra_modifier);
                }
            }

            if self.unit_is_barbarian(u) {
                if self.unit_is_human(a) {
                    extra_modifier = handicap.barbarian_combat_modifier;
                    modifier += extra_modifier;
                    set_detail!(barbarian_combat_modifier_tb, extra_modifier);
                } else {
                    extra_modifier = handicap.ai_barbarian_combat_modifier;
                    modifier += extra_modifier;
                    set_detail!(ai_barbarian_combat_modifier_tb, extra_modifier);
                }
            }

            if self.unit_is_barbarian(a) {
                if self.unit_is_human(u) {
                    extra_modifier = -handicap.barbarian_combat_modifier;
                    modifier += extra_modifier;
                    set_detail!(barbarian_combat_modifier_ab, extra_modifier);
                } else {
                    extra_modifier = -handicap.ai_barbarian_combat_modifier;
                    modifier += extra_modifier;
                    // FfH: Modified by Kael 07/31/2008 (was stored in the TB field)
                    set_detail!(ai_barbarian_combat_modifier_ab, extra_modifier);
                }
            }
        }

        // add defensive bonuses (leaving these out for bAttackingUnknownDefender case)
        if let Some(p) = plot {
            if !self.unit_no_defensive_bonus(u) {
                let ignore_building = match attacker {
                    Some(a) => self.unit_ignore_building_defense(a),
                    None => true,
                };
                extra_modifier = self.plot_defense_modifier(p, Some(team), ignore_building);
                modifier += extra_modifier;
                set_detail!(plot_defense_modifier, extra_modifier);
            }

            extra_modifier = self.unit_fortify_modifier(u);
            modifier += extra_modifier;
            set_detail!(fortify_modifier, extra_modifier);

            if self.plot_is_city(p, true, Some(team)) {
                extra_modifier = self.unit_city_defense_modifier(u);
                modifier += extra_modifier;
                set_detail!(city_defense_modifier, extra_modifier);
            }

            if self.plot_is_hills(p) {
                extra_modifier = self.unit_hills_defense_modifier(u);
                modifier += extra_modifier;
                set_detail!(hills_defense_modifier, extra_modifier);
            }

            if let Some(feature) = self.map.plots[p].feature_type {
                extra_modifier = self.unit_feature_defense_modifier(u, feature);
                modifier += extra_modifier;
                set_detail!(feature_defense_modifier, extra_modifier);
            } else {
                let terrain = self.map.plots[p].terrain_type.expect("CvUnit::maxCombatStr: plot without terrain");
                extra_modifier = self.unit_terrain_defense_modifier(u, terrain);
                modifier += extra_modifier;
                set_detail!(terrain_defense_modifier, extra_modifier);
            }
        }

        // if we are attacking to an plot with an unknown defender, the calc the modifier in reverse
        if attacking_unknown_defender {
            attacker = Some(u);
        }

        // calc attacker bonueses
        // UNOFFICIAL_PATCH 09/20/09 jdog5000: also requires the attacked plot.
        if let (Some(a), Some(ap)) = (attacker, attacked_plot) {
            let mut temp_modifier = 0;

            if self.plot_is_city(ap, true, Some(team)) {
                extra_modifier = -self.unit_city_attack_modifier(a);
                temp_modifier += extra_modifier;
                set_detail!(city_attack_modifier, extra_modifier);

                if self.unit_is_barbarian(a) {
                    extra_modifier = d.defines.city_barbarian_defense_modifier;
                    temp_modifier += extra_modifier;
                    set_detail!(city_barbarian_defense_modifier, extra_modifier);
                }
            }

            if self.plot_is_hills(ap) {
                extra_modifier = -self.unit_hills_attack_modifier(a);
                temp_modifier += extra_modifier;
                set_detail!(hills_attack_modifier, extra_modifier);
            }

            if let Some(feature) = self.map.plots[ap].feature_type {
                extra_modifier = -self.unit_feature_attack_modifier(a, feature);
                temp_modifier += extra_modifier;
                set_detail!(feature_attack_modifier, extra_modifier);
            } else {
                let terrain = self.map.plots[ap].terrain_type.expect("CvUnit::maxCombatStr: plot without terrain");
                extra_modifier = -self.unit_terrain_attack_modifier(a, terrain);
                // C++ quirk: added to iModifier, not iTempModifier (never reversed for an unknown
                // defender).
                modifier += extra_modifier;
                set_detail!(terrain_attack_modifier, extra_modifier);
            }

            // only compute comparisions if we are the defender with a known attacker
            if !attacking_unknown_defender {
                // FfH Promotions: Added by Kael 08/13/2007
                for promotion in d.promotions.keys() {
                    let info = &d.promotions[promotion];
                    // (C++ reads `isHasPromotion(NO_PROMOTION)` out of bounds for a missing
                    // combat type; every promotion with a combat mod names one)
                    if self.units[u].has_promotion[promotion] && info.promotion_combat_mod > 0 {
                        if info.promotion_combat_type.is_some_and(|t| self.units[a].has_promotion[t]) {
                            modifier += info.promotion_combat_mod;
                        }
                    }
                    if self.units[a].has_promotion[promotion] && info.promotion_combat_mod > 0 {
                        if info.promotion_combat_type.is_some_and(|t| self.units[u].has_promotion[t]) {
                            modifier -= info.promotion_combat_mod;
                        }
                    }
                }
                let global_counter = self.get_global_counter();
                if global_counter * self.units[u].combat_percent_global_counter / 100 != 0 {
                    modifier += global_counter * self.units[u].combat_percent_global_counter / 100;
                }
                if global_counter * self.units[a].combat_percent_global_counter / 100 != 0 {
                    modifier -= global_counter * self.units[a].combat_percent_global_counter / 100;
                }
                // FfH: End Add

                extra_modifier = self.unit_unit_class_defense_modifier(u, self.unit_get_unit_class_type(a));
                temp_modifier += extra_modifier;
                set_detail!(class_defense_modifier, extra_modifier);

                extra_modifier = -self.unit_unit_class_attack_modifier(a, self.unit_get_unit_class_type(u));
                temp_modifier += extra_modifier;
                set_detail!(class_attack_modifier, extra_modifier);

                if let Some(unit_combat) = self.unit_get_unit_combat_type(a) {
                    extra_modifier = self.unit_unit_combat_modifier(u, unit_combat);
                    temp_modifier += extra_modifier;
                    set_detail!(combat_modifier_a, extra_modifier);
                }
                if let Some(unit_combat) = self.unit_get_unit_combat_type(u) {
                    extra_modifier = -self.unit_unit_combat_modifier(a, unit_combat);
                    temp_modifier += extra_modifier;
                    set_detail!(combat_modifier_t, extra_modifier);
                }

                extra_modifier = self.unit_domain_modifier(u, self.unit_get_domain_type(a));
                temp_modifier += extra_modifier;
                set_detail!(domain_modifier_a, extra_modifier);

                extra_modifier = -self.unit_domain_modifier(a, self.unit_get_domain_type(u));
                temp_modifier += extra_modifier;
                set_detail!(domain_modifier_t, extra_modifier);

                if self.unit_is_animal(a) {
                    extra_modifier = self.unit_animal_combat_modifier(u);
                    temp_modifier += extra_modifier;
                    set_detail!(animal_combat_modifier_a, extra_modifier);
                }

                if self.unit_is_animal(u) {
                    extra_modifier = -self.unit_animal_combat_modifier(a);
                    temp_modifier += extra_modifier;
                    set_detail!(animal_combat_modifier_t, extra_modifier);
                }
            }

            let attacker_plot = self.unit_plot(a);
            if !self.unit_is_river(a) {
                if self.plot_is_river_crossing(attacker_plot, self.direction_xy_plots(attacker_plot, ap)) {
                    extra_modifier = -d.defines.river_attack_modifier;
                    temp_modifier += extra_modifier;
                    set_detail!(river_attack_modifier, extra_modifier);
                }
            }

            if !self.unit_is_amphib(a) {
                if !self.plot_is_water(ap) && self.plot_is_water(attacker_plot) {
                    extra_modifier = -d.defines.amphib_attack_modifier;
                    temp_modifier += extra_modifier;
                    set_detail!(amphib_attack_modifier, extra_modifier);
                }
            }

            if self.units[a].kamikaze_percent != 0 {
                extra_modifier = self.units[a].kamikaze_percent;
                temp_modifier += extra_modifier;
                set_detail!(kamikaze_modifier, extra_modifier);
            }

            // if we are attacking an unknown defender, then use the reverse of the modifier
            if attacking_unknown_defender {
                modifier -= temp_modifier;
            } else {
                modifier += temp_modifier;
            }
        }

        // FfH Defense Str: Modified by Kael 08/18/2007 (strength in hundredths, damage types
        // reduced by the opponent's resistances)
        set_detail!(modifier_total, modifier);
        set_detail!(base_combat_str, str_);

        str_ *= 100;
        // C++ quirk: for an unknown defender `pAttacker` is this unit again, so its own
        // resistances reduce its damage-type strength.
        for opponent in [attacker, defender].into_iter().flatten() {
            for damage_type in d.damages.keys() {
                let damage_type_combat = self.units[u].damage_type_combat[damage_type];
                if damage_type_combat != 0 {
                    let resist = self.unit_get_damage_type_resist(opponent, damage_type);
                    if resist != 0 {
                        str_ -= damage_type_combat * 100;
                        str_ += damage_type_combat * (100 - resist);
                    }
                }
            }
        }

        let combat = apply_combat_modifier(str_, modifier);
        // FfH: End Modify

        if let Some(det) = details.as_deref_mut() {
            det.combat = combat;
            det.max_combat_str = 1.max(combat);
            det.curr_hit_points = self.unit_curr_hit_points(u);
            det.max_hit_points = self.unit_max_hit_points(u);
            det.curr_combat_str = (det.max_combat_str * det.curr_hit_points) / det.max_hit_points;
        }

        1.max(combat)
    }

    /// C++ `CvUnit::currCombatStr` (CvUnit.cpp:9560-9563).
    pub fn unit_curr_combat_str(
        &self,
        u: UnitId,
        plot: Option<PlotIdx>,
        attacker: Option<UnitId>,
        details: Option<&mut CombatDetails>,
    ) -> i32 {
        (self.unit_max_combat_str(u, plot, attacker, details) * self.unit_curr_hit_points(u)) / self.unit_max_hit_points(u)
    }

    /// C++ `CvUnit::currFirepower` (CvUnit.cpp:9566-9569).
    pub fn unit_curr_firepower(&self, u: UnitId, plot: Option<PlotIdx>, attacker: Option<UnitId>) -> i32 {
        (self.unit_max_combat_str(u, plot, attacker, None) + self.unit_curr_combat_str(u, plot, attacker, None) + 1) / 2
    }

    /// C++ `CvUnit::currEffectiveStr` (CvUnit.cpp:9574-9582).
    pub fn unit_curr_effective_str(
        &self,
        u: UnitId,
        plot: Option<PlotIdx>,
        attacker: Option<UnitId>,
        details: Option<&mut CombatDetails>,
    ) -> i32 {
        let mut curr_str = self.unit_curr_combat_str(u, plot, attacker, details);

        curr_str *= self.unit_max_hit_points(u) + self.unit_curr_hit_points(u);
        curr_str /= 2 * self.unit_max_hit_points(u);

        curr_str
    }

    /// C++ `CvUnit::canFight` (CvUnit.cpp:9596-9608).
    pub fn unit_can_fight(&self, u: UnitId) -> bool {
        // FfH: a unit with only defensive strength can still fight.
        !(self.unit_base_combat_str(u) == 0 && self.unit_base_combat_str_defense(u) == 0)
    }

    /// C++ `CvUnit::canAttack` (CvUnit.cpp:9611-9636).
    pub fn unit_can_attack(&self, u: UnitId) -> bool {
        if !self.unit_can_fight(u) {
            return false;
        }

        if self.unit_is_only_defensive(u) {
            return false;
        }

        // FfH: Added by Kael 04/25/2010
        if self.unit_base_combat_str(u) == 0 {
            return false;
        }

        if self.unit_get_immobile_timer(u) != 0 {
            return false;
        }
        // FfH: End Add

        true
    }

    /// C++ `CvUnit::canAttack` (CvUnit.cpp:9637-9659).
    /// C++ overload `canAttack(const CvUnit& defender)`.
    pub fn unit_can_attack_unit(&self, u: UnitId, defender: UnitId) -> bool {
        if !self.unit_can_attack(u) {
            return false;
        }

        if self.units[defender].damage >= self.units[u].combat_limit {
            return false;
        }

        // Artillery can't amphibious attack
        if self.plot_is_water(self.unit_plot(u)) && !self.plot_is_water(self.unit_plot(defender)) {
            if self.units[u].combat_limit < 100 {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::canDefend` (CvUnit.cpp:9661-9689).
    /// `plot` `None` = the unit's own plot (C++ default NULL).
    pub fn unit_can_defend(&self, u: UnitId, plot: Option<PlotIdx>) -> bool {
        if !self.unit_can_fight(u) {
            return false;
        }

        // (C++ dereferences `plot()` for a NULL `pPlot`)
        let plot = plot.unwrap_or_else(|| self.unit_plot(u));
        if !self.plot_is_valid_domain_for_action(plot, u) {
            if self.d.defines.land_units_can_attack_water_cities == 0 {
                return false;
            }
        }

        // FfH: Added by Kael 10/31/2007
        if self.unit_base_combat_str_defense(u) == 0 {
            return false;
        }
        // FfH: End Add

        true
    }

    /// C++ `CvUnit::canSiege` (CvUnit.cpp:9692-9710).
    pub fn unit_can_siege(&self, u: UnitId, team: TeamId) -> bool {
        if !self.unit_can_defend(u, None) {
            return false;
        }

        if !self.unit_is_enemy(u, Some(team), None) {
            return false;
        }

        if !self.unit_is_never_invisible(u) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::airBaseCombatStr` (CvUnit.cpp:9713-9716).
    pub fn unit_air_base_combat_str(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].air_combat
    }

    /// C++ `CvUnit::airMaxCombatStr` (CvUnit.cpp:9719-9766).
    pub fn unit_air_max_combat_str(&self, u: UnitId, other: Option<UnitId>) -> i32 {
        let air_base_combat_str = self.unit_air_base_combat_str(u);
        if air_base_combat_str == 0 {
            return 0;
        }

        let mut modifier = self.unit_get_extra_combat_percent(u);

        if self.units[u].kamikaze_percent != 0 {
            modifier += self.units[u].kamikaze_percent;
        }

        // C++ quirk (BtS): the extra combat percent counts twice.
        if self.unit_get_extra_combat_percent(u) != 0 {
            modifier += self.unit_get_extra_combat_percent(u);
        }

        if let Some(o) = other {
            if let Some(unit_combat) = self.unit_get_unit_combat_type(o) {
                modifier += self.unit_unit_combat_modifier(u, unit_combat);
            }

            modifier += self.unit_domain_modifier(u, self.unit_get_domain_type(o));

            if self.unit_is_animal(o) {
                modifier += self.unit_animal_combat_modifier(u);
            }
        }

        let combat = if modifier > 0 {
            air_base_combat_str * (modifier + 100)
        } else {
            (air_base_combat_str * 10000) / (100 - modifier)
        };

        1.max(combat)
    }

    /// C++ `CvUnit::airCurrCombatStr` (CvUnit.cpp:9769-9772).
    pub fn unit_air_curr_combat_str(&self, u: UnitId, other: Option<UnitId>) -> i32 {
        (self.unit_air_max_combat_str(u, other) * self.unit_curr_hit_points(u)) / self.unit_max_hit_points(u)
    }

    /// C++ `CvUnit::airCombatLimit` (CvUnit.cpp:9798-9801).
    pub fn unit_air_combat_limit(&self, u: UnitId) -> i32 {
        self.d.units[self.units[u].unit_type].air_combat_limit
    }

    /// C++ `CvUnit::canAirAttack` (CvUnit.cpp:9804-9807).
    pub fn unit_can_air_attack(&self, u: UnitId) -> bool {
        self.unit_air_base_combat_str(u) > 0
    }

    /// C++ `CvUnit::canAirDefend` (CvUnit.cpp:9810-9844).
    /// `plot` `None` = the unit's own plot (C++ default NULL).
    pub fn unit_can_air_defend(&self, u: UnitId, plot: Option<PlotIdx>) -> bool {
        // (C++ dereferences `plot()` for a NULL `pPlot` when it gets that far)
        if self.unit_max_interception_probability(u) == 0 {
            return false;
        }

        if self.unit_get_domain_type(u) != DomainType::DOMAIN_AIR {
            let plot = plot.unwrap_or_else(|| self.unit_plot(u));
            // UNOFFICIAL_PATCH 10/30/09 Mongoose & jdog5000: land units which are cargo cannot intercept
            if !self.plot_is_valid_domain_for_location(plot, u) || self.unit_is_cargo(u) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::airCombatDamage` (CvUnit.cpp:9847-9875).
    pub fn unit_air_combat_damage(&self, u: UnitId, defender: UnitId) -> i32 {
        let d = self.d;
        let plot = self.unit_plot(defender);

        let our_strength = self.unit_air_curr_combat_str(u, Some(defender));
        // (C++ FAssert(iOurStrength > 0))
        let their_strength = self.unit_max_combat_str(defender, Some(plot), Some(u), None);

        let strength_factor = (our_strength + their_strength + 1) / 2;

        let mut damage =
            1.max((d.defines.air_combat_damage * (our_strength + strength_factor)) / (their_strength + strength_factor));

        if let Some(city) = self.map.plots[plot].plot_city {
            damage *= 0.max(self.cities[city].air_modifier + 100);
            damage /= 100;
        }

        damage
    }

    /// C++ `CvUnit::rangeCombatDamage` (CvUnit.cpp:9878-9897).
    pub fn unit_range_combat_damage(&self, u: UnitId, defender: UnitId) -> i32 {
        let d = self.d;
        let plot = self.unit_plot(defender);

        let our_strength = self.unit_air_curr_combat_str(u, Some(defender));
        // (C++ FAssert(iOurStrength > 0))
        let their_strength = self.unit_max_combat_str(defender, Some(plot), Some(u), None);

        let strength_factor = (our_strength + their_strength + 1) / 2;

        1.max((d.defines.range_combat_damage * (our_strength + strength_factor)) / (their_strength + strength_factor))
    }

    /// C++ `CvUnit::bestInterceptor` (CvUnit.cpp:9900-9948).
    pub fn unit_best_interceptor(&self, u: UnitId, plot: PlotIdx) -> Option<UnitId> {
        let mut best_value = 0;
        let mut best_unit = None;
        let (plot_x, plot_y) = self.map.xy(plot);

        for p in self.player_ids() {
            if !self.players[p].alive {
                continue;
            }
            let team = self.players[p].team;
            if self.unit_is_enemy(u, Some(team), None) && !self.unit_is_invisible(u, team, false) {
                for &loop_unit in &self.players[p].units {
                    if !self.unit_can_air_defend(loop_unit, None) {
                        continue;
                    }
                    if self.units[loop_unit].made_interception {
                        continue;
                    }
                    let air = self.unit_get_domain_type(loop_unit) == DomainType::DOMAIN_AIR;
                    if air && self.unit_has_moved(loop_unit) {
                        continue;
                    }
                    if air && self.groups[group_of(self, loop_unit)].activity_type != ActivityType::ACTIVITY_INTERCEPT {
                        continue;
                    }
                    let (x, y) = (self.units[loop_unit].x, self.units[loop_unit].y);
                    if self.plot_distance(x, y, plot_x, plot_y) <= self.unit_air_range(loop_unit) {
                        let value = self.unit_curr_interception_probability(loop_unit);

                        if value > best_value {
                            best_value = value;
                            best_unit = Some(loop_unit);
                        }
                    }
                }
            }
        }

        best_unit
    }

    /// C++ `CvUnit::bestSeaPillageInterceptor` (CvUnit.cpp:9951-10001).
    pub fn unit_best_sea_pillage_interceptor(&self, u: UnitId, pillager: UnitId, min_odds: i32) -> Option<UnitId> {
        let mut best_unit: Option<UnitId> = None;
        let team = self.unit_get_team(u);
        let (pillager_x, pillager_y) = (self.units[pillager].x, self.units[pillager].y);
        let pillager_area = self.unit_area(pillager);

        for dx in -1..=1 {
            for dy in -1..=1 {
                let Some(loop_plot) = self.plot_xy(pillager_x, pillager_y, dx, dy) else {
                    continue;
                };

                for &loop_unit in &self.map.plots[loop_plot].units {
                    if self.unit_area(loop_unit) != pillager_area {
                        continue;
                    }
                    if self.unit_is_invisible(loop_unit, team, true) {
                        continue;
                    }
                    if !self.unit_is_enemy(u, Some(self.unit_get_team(loop_unit)), None) {
                        continue;
                    }
                    if self.unit_get_domain_type(loop_unit) != DomainType::DOMAIN_SEA {
                        continue;
                    }
                    if self.groups[group_of(self, loop_unit)].activity_type != ActivityType::ACTIVITY_PATROL {
                        continue;
                    }
                    if best_unit.is_none() || self.unit_is_better_defender_than(loop_unit, best_unit, Some(u)) {
                        if self.get_combat_odds(pillager, loop_unit) < min_odds {
                            best_unit = Some(loop_unit);
                        }
                    }
                }
            }
        }

        best_unit
    }

    /// C++ `CvUnit::getAttackPlot` (CvUnit.cpp:11904-11907).
    pub fn unit_get_attack_plot(&self, u: UnitId) -> Option<PlotIdx> {
        let unit = &self.units[u];
        self.map.plot_soren(unit.attack_plot_x, unit.attack_plot_y)
    }

    /// C++ `CvUnit::setAttackPlot` (CvUnit.cpp:11910-11927).
    pub fn unit_set_attack_plot(&mut self, u: UnitId, plot: Option<PlotIdx>, air_combat: bool) {
        if self.unit_get_attack_plot(u) != plot {
            let (x, y) = match plot {
                Some(p) => self.map.xy(p),
                None => (INVALID_PLOT_COORD, INVALID_PLOT_COORD),
            };
            self.units[u].attack_plot_x = x;
            self.units[u].attack_plot_y = y;
        }

        self.units[u].air_combat = air_combat;
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
        // UI: combat focus; for an attacker the cheat-level combat log and the siege tower flag.
        let _ = attacking;

        match combat_unit {
            Some(c) => {
                // (C++ FAssert(getCombatUnit() == NULL), FAssert(!plot()->isFighting()))
                self.units[u].combat_unit = Some(c);
                let first_strikes = if self.unit_immune_to_first_strikes(c) {
                    0
                } else {
                    let chance = self.unit_chance_first_strikes(u);
                    self.unit_first_strikes(u) + self.soren_rand_num(chance + 1, "First Strike")
                };
                self.units[u].combat_first_strikes = first_strikes;
            }
            None => {
                // C++ quirk: a combat unit that no longer exists is not cleared (`getCombatUnit()`
                // is already NULL then).
                if combat_unit_of(self, u).is_some() {
                    self.units[u].combat_unit = None;
                    self.units[u].combat_first_strikes = 0;
                    // UI: info pane / plot list dirty bits, siege tower.
                }
            }
        }

        self.units[u].combat_timer = 0;
        // UI: info bar dirty, combat focus.
    }

    /// C++ `CvUnit::doDefensiveStrike` (CvUnit.cpp:16206-16259).
    /// FfH: `u` is the defender; its stack may strike the attacker before combat.
    pub fn unit_do_defensive_strike(&mut self, u: UnitId, attacker: UnitId) {
        let d = self.d;
        let mut best_unit: Option<UnitId> = None;
        let mut best_damage = 0;
        let plot = self.unit_plot(u);
        let attacker_team = self.unit_get_team(attacker);

        for loop_unit in self.map.plots[plot].units.clone() {
            if self.units[loop_unit].defensive_strike_chance > 0 {
                if self.at_war(self.unit_get_team(loop_unit), attacker_team) {
                    if self.unit_is_blitz(loop_unit) || !self.units[loop_unit].made_attack {
                        let chance = self.units[loop_unit].defensive_strike_chance;
                        if self.soren_rand_num(100, "Defensive Strike") < chance {
                            if self.units[loop_unit].defensive_strike_damage > best_damage {
                                best_damage = self.units[loop_unit].defensive_strike_damage;
                                best_unit = Some(loop_unit);
                            }
                        }
                    }
                }
            }
        }

        if best_damage > 0 {
            let best = best_unit.expect("defensive strike damage without a striker");
            if !self.unit_is_blitz(best) {
                self.units[best].made_attack = true;
            }
            let strike_damage = self.units[best].defensive_strike_damage;
            let mut damage = 0;
            damage += self.soren_rand_num(strike_damage, "Defensive Strike 1");
            damage += self.soren_rand_num(strike_damage, "Defensive Strike 2");
            damage -= self.unit_base_combat_str_defense(attacker) * 2;
            damage -= self.units[attacker].level;
            if damage + self.units[attacker].damage > 95 {
                damage = 95 - self.units[attacker].damage;
            }
            if damage > 0 {
                let best_owner = self.units[best].owner;
                self.unit_change_damage(attacker, damage, Some(best_owner));

                let turn = self.game_turn();
                let attacker_owner = self.units[attacker].owner;
                let (attacker_type, best_type) = (self.units[attacker].unit_type, self.units[best].unit_type);
                self.log.msg(turn, Some(attacker_owner), || {
                    format!(
                        "{} hit by a defensive strike from {} ({} damage)",
                        unit_type_name(d, attacker_type),
                        unit_type_name(d, best_type),
                        damage
                    )
                });

                self.unit_change_experience(best, 1, 100, true, false, false);
            }
        }
    }

    /// C++ `CvUnit::combatWon` (CvUnit.cpp:16306-16560).
    /// FfH post-combat effects (XP, combat-apply promotions, capture/enslave, gold, poison,
    /// create/convert units, `PyPostCombatWon`/`PyPostCombatLost` callbacks in `crate::ffh`).
    pub fn unit_combat_won(&mut self, u: UnitId, loser: UnitId, attacking: bool) {
        let d = self.d;
        // C++ `bAttacking` only picks the plot of the explosion effect (graphics).
        let _ = attacking;

        let mut convert = false;
        // C++ `iUnit`: the unit type to create from the loser (capture or slave).
        let mut new_unit_type: Option<UnitType> = None;
        let combat_apply_chance = d.defines.combat_apply_chance;

        for promotion in d.promotions.keys() {
            let promotion_info = &d.promotions[promotion];
            if self.units[u].has_promotion[promotion] {
                if promotion_info.free_xp_from_combat != 0 {
                    self.unit_change_experience(u, promotion_info.free_xp_from_combat, -1, false, false, false);
                }
                if promotion_info.modify_global_counter_on_combat != 0 {
                    if self.unit_is_alive(loser) {
                        self.change_global_counter(promotion_info.modify_global_counter_on_combat);
                    }
                }
                if promotion_info.removed_by_combat {
                    self.unit_set_has_promotion(u, promotion, false);
                    // C++ quirk: losing an invisible promotion (Hidden is removed by combat) on a
                    // plot with a visible enemy withdraws the winner, and kills it at once when no
                    // plot around is free (`changeInvisibleFromPromotion` ->
                    // `withdrawlToNearestValidPlot(true)` -> `kill(false)`); C++ then goes on using
                    // the deleted unit. Everything left here is about the winner, so it is skipped.
                    if !self.units.contains(u) {
                        return;
                    }
                }
                if let Some(apply) = promotion_info.promotion_combat_apply {
                    let loser_plot = self.unit_plot(loser);
                    for loop_unit in self.map.plots[loser_plot].units.clone() {
                        if !self.units.contains(loop_unit) {
                            continue;
                        }
                        if !self.units[loop_unit].has_promotion[apply] {
                            if self.unit_is_alive(loop_unit) || !d.promotions[apply].prereq_alive {
                                let loop_team = self.unit_get_team(loop_unit);
                                if self.unit_is_enemy(u, Some(loop_team), None) {
                                    if self.unit_can_acquire_promotion(loop_unit, apply) {
                                        if self.soren_rand_num(100, "Combat Apply") <= combat_apply_chance {
                                            self.unit_set_has_promotion(loop_unit, apply, true);
                                            // UI: promotion messages to both owners.
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if promotion_info.combat_capture_percent != 0 {
                    if new_unit_type.is_none() && self.unit_is_alive(loser) {
                        if self.soren_rand_num(100, "Combat Capture") <= promotion_info.combat_capture_percent {
                            new_unit_type = Some(self.units[loser].unit_type);
                            convert = true;
                        }
                    }
                }
                if let Some(capture_unit_combat) = promotion_info.capture_unit_combat {
                    if new_unit_type.is_none() && self.unit_get_unit_combat_type(loser) == Some(capture_unit_combat) {
                        new_unit_type = Some(self.units[loser].unit_type);
                        convert = true;
                    }
                }
            }
            if self.units[loser].has_promotion[promotion] {
                if let Some(apply) = promotion_info.promotion_combat_apply {
                    if !self.units[u].has_promotion[apply] {
                        if self.unit_is_alive(u) || !d.promotions[apply].prereq_alive {
                            let team = self.unit_get_team(u);
                            if self.unit_is_enemy(loser, Some(team), None) {
                                if self.soren_rand_num(100, "Combat Apply") <= combat_apply_chance {
                                    self.unit_set_has_promotion(u, apply, true);
                                    // UI: promotion messages to both owners.
                                }
                            }
                        }
                    }
                }
            }
        }

        let owner = self.units[u].owner;
        let info = &d.units[self.units[u].unit_type];

        if self.players[owner].free_xp_from_combat != 0 {
            let free_xp = self.players[owner].free_xp_from_combat;
            self.unit_change_experience(u, free_xp, -1, false, false, false);
        }
        if self.units[u].combat_heal_percent != 0 {
            if self.unit_is_alive(loser) {
                let mut heal = self.units[u].combat_heal_percent;
                if heal > self.units[u].damage {
                    heal = self.units[u].damage;
                }
                if heal != 0 {
                    self.unit_change_damage(u, -heal, None);
                }
            }
        }
        // UI: explode-in-combat effects and sounds (suicide exploders, exploding losers).

        let enslavement_chance = info.enslavement_chance + self.players[owner].enslavement_chance;
        if enslavement_chance > 0 {
            if self.units[u].duration == 0
                && self.unit_is_alive(loser)
                && !self.unit_is_animal(loser)
                && new_unit_type.is_none()
            {
                if self.soren_rand_num(100, "Enslavement") <= enslavement_chance {
                    new_unit_type = opt_from_i32::<UnitType>(d.defines.slave_unit);
                }
            }
        }
        if let Some(promotion) = info.promotion_from_combat {
            if self.unit_is_alive(loser) {
                self.unit_set_has_promotion(u, promotion, true);
            }
        }
        if self.units[u].gold_from_combat != 0 {
            if !self.unit_is_animal(loser) {
                let gold = self.units[u].gold_from_combat;
                self.players[owner].gold += gold;
                let turn = self.game_turn();
                self.log.msg(turn, Some(owner), || format!("{gold} gold from combat"));
            }
        }
        if self.units[u].duration > 0 {
            self.unit_change_duration(u, info.duration_from_combat);
        }
        if self.units[loser].damage_type_combat[DamageType::DAMAGE_POISON] > 0 && d.defines.poisoned_promotion != -1 {
            if self.unit_is_alive(u) && self.units[u].damage > 0 {
                let resist = self.unit_get_damage_type_resist(u, DamageType::DAMAGE_POISON);
                if self.soren_rand_num(100, "Poisoned") >= resist {
                    if let Some(poisoned) = opt_from_i32::<PromotionType>(d.defines.poisoned_promotion) {
                        self.unit_set_has_promotion(u, poisoned, true);
                    }
                }
            }
        }
        if let Some(create_unit_type) = info.unit_create_from_combat {
            let loser_info = &d.units[self.units[loser].unit_type];
            if !self.unit_is_immune_to_capture(loser) && self.unit_is_alive(loser) && loser_info.equipment_promotion.is_none() {
                if self.soren_rand_num(100, "Create Unit from Combat") <= info.unit_create_from_combat_chance {
                    let (x, y) = self.map.xy(self.unit_plot(u));
                    let new_unit = self.player_init_unit(owner, create_unit_type, x, y, None, None);
                    self.units[new_unit].duration = self.units[u].duration;
                    if self.unit_is_hidden_nationality(u) {
                        // (C++ casts the define unchecked; it always names a promotion)
                        if let Some(hidden) = opt_from_i32::<PromotionType>(d.defines.hidden_nationality_promotion) {
                            self.unit_set_has_promotion(new_unit, hidden, true);
                        }
                    }
                    new_unit_type = None;
                }
            }
        }
        if let Some(new_type) = new_unit_type {
            let loser_info = &d.units[self.units[loser].unit_type];
            if (!self.unit_is_immune_to_capture(loser) && !self.unit_is_no_capture(u) && !self.unit_is_immortal(loser))
                || loser_info.equipment_promotion.is_some()
            {
                let (x, y) = self.map.xy(self.unit_plot(u));
                let new_unit = self.player_init_unit(owner, new_type, x, y, None, None);
                if self.units[u].duration != 0 {
                    self.units[new_unit].duration = self.units[u].duration;
                }
                if Some(new_type) == opt_from_i32::<UnitType>(d.defines.slave_unit) {
                    if let Some(race) = self.units[loser].race {
                        self.unit_set_has_promotion(new_unit, race, true);
                    }
                }
                if convert {
                    self.unit_set_damage(loser, 75, None);
                    self.unit_convert(new_unit, loser);
                }
            }
        }
        // C++ calls `postCombatWon` / `postCombatLost` only for a non-empty callback name; the
        // dispatch tables skip types without one. Both units still exist here (combat deaths are
        // delayed and the FfH callbacks only kill with delay); the guards only keep a dangling
        // unit, which C++ would not survive either, from being touched.
        if self.units.contains(u) && self.units.contains(loser) {
            self.py_post_combat_won(u, loser);
        }
        if self.units.contains(u) && self.units.contains(loser) {
            self.py_post_combat_lost(loser, u);
        }
        if !self.units.contains(u) {
            return;
        }
        if let Some(convert_unit_type) = info.unit_convert_from_combat {
            if self.soren_rand_num(100, "Convert Unit from Combat") <= info.unit_convert_from_combat_chance {
                let (x, y) = (self.units[u].x, self.units[u].y);
                let unit_ai = self.units[u].ai_type;
                let new_unit = self.player_init_unit(owner, convert_unit_type, x, y, unit_ai, None);
                self.unit_convert(new_unit, u);
            }
        }
    }

    /// C++ `CvUnit::canAdvance` (CvUnit.cpp:17230-17251).
    pub fn unit_can_advance(&self, u: UnitId, plot: PlotIdx, threshold: i32) -> bool {
        // (C++ FAssert(canFight()), FAssert(getDomainType() != DOMAIN_AIR/DOMAIN_IMMOBILE))
        if self.plot_get_num_visible_enemy_defenders(plot, u) > threshold {
            return false;
        }

        if self.unit_is_no_capture(u) {
            if self.plot_is_enemy_city(plot, u) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::collateralCombat` (CvUnit.cpp:17254-17382).
    /// C++ quirk to keep in mind: candidates are kept in a `std::map<CvUnit*, int>` (pointer
    /// order) and the max-value scan keeps the first maximum in that order; values are
    /// `(1 + rand(10000)) * currHitPoints` so ties are rare. The rand draws happen in plot order.
    pub fn unit_collateral_combat(&mut self, u: UnitId, plot: PlotIdx, skip_unit: Option<UnitId>) {
        let d = self.d;
        let air = self.unit_get_domain_type(u) == DomainType::DOMAIN_AIR;

        let base_strength = if air { self.unit_air_base_combat_str(u) } else { self.unit_base_combat_str(u) };
        let collateral_strength = base_strength * self.unit_collateral_damage(u) / 100;
        if collateral_strength == 0 {
            return;
        }

        let possible_targets =
            (self.plot_get_num_visible_enemy_defenders(plot, u) - 1).min(self.unit_collateral_damage_max_units(u));

        // C++ `std::map<CvUnit*, int> mapUnitDamage`: ordered by heap address in C++, which is
        // not reproducible; plot order here (it only decides exact ties of the rolled values).
        let team = self.unit_get_team(u);
        let mut unit_damage: Vec<(UnitId, i32)> = Vec::new();
        for loop_unit in self.map.plots[plot].units.clone() {
            if Some(loop_unit) != skip_unit {
                let loop_team = self.unit_get_team(loop_unit);
                if self.unit_is_enemy(u, Some(loop_team), Some(plot)) {
                    if !self.unit_is_invisible(loop_unit, team, true) {
                        if self.unit_can_defend(loop_unit, None) {
                            let mut value = 1 + self.soren_rand_num(10000, "Collateral Damage");

                            value *= self.unit_curr_hit_points(loop_unit);

                            unit_damage.push((loop_unit, value));
                        }
                    }
                }
            }
        }

        let city = if air { self.map.plots[plot].plot_city } else { None };

        let owner = self.units[u].owner;
        let unit_combat = self.unit_get_unit_combat_type(u);
        let mut damage_count = 0;
        let mut count = 0;

        while count < possible_targets {
            let mut best_value = 0;
            let mut best_index: Option<usize> = None;

            for (index, &(_, value)) in unit_damage.iter().enumerate() {
                if value > best_value {
                    best_value = value;
                    best_index = Some(index);
                }
            }

            let Some(best_index) = best_index else {
                break;
            };
            let (best_unit, _) = unit_damage.remove(best_index);

            let immune = unit_combat
                .is_some_and(|uc| d.units[self.units[best_unit].unit_type].unit_combat_collateral_immune[uc] != 0);
            if !immune {
                let their_strength = self.unit_base_combat_str(best_unit);

                let strength_factor = (collateral_strength + their_strength + 1) / 2;

                let mut collateral_damage = (d.defines.collateral_combat_damage * (collateral_strength + strength_factor))
                    / (their_strength + strength_factor);

                collateral_damage *= 100 + self.units[u].extra_collateral_damage;

                collateral_damage *= 0.max(100 - self.units[best_unit].collateral_damage_protection);
                collateral_damage /= 100;

                if let Some(c) = city {
                    collateral_damage *= 100 + self.cities[c].air_modifier;
                    collateral_damage /= 100;
                }

                collateral_damage /= 100;

                collateral_damage = 0.max(collateral_damage);

                let limit = self.unit_collateral_damage_limit(u);
                let max_damage =
                    limit.min((limit * (collateral_strength + strength_factor)) / (their_strength + strength_factor));
                let damage = self.units[best_unit].damage;
                let new_damage = damage.max((damage + collateral_damage).min(max_damage));

                if damage != new_damage {
                    self.unit_set_damage(best_unit, new_damage, Some(owner));
                    damage_count += 1;
                }
            }

            count += 1;
        }

        if damage_count > 0 {
            let turn = self.game_turn();
            let unit_type = self.units[u].unit_type;
            self.log.msg(turn, Some(owner), || {
                format!("{} inflicted collateral damage on {} units", unit_type_name(d, unit_type), damage_count)
            });
        }
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
        let d = self.d;

        let skip_team = self.unit_get_team(skip_unit);
        if self.plot_is_city(plot, true, Some(skip_team)) {
            return;
        }

        let info = &d.units[self.units[u].unit_type];
        let team = self.unit_get_team(u);

        let mut flanked_units: Vec<(UnitId, i32)> = Vec::new();
        for loop_unit in self.map.plots[plot].units.clone() {
            if loop_unit == skip_unit {
                continue;
            }
            let loop_team = self.unit_get_team(loop_unit);
            if !self.unit_is_dead(loop_unit) && self.unit_is_enemy(u, Some(loop_team), Some(plot)) {
                if !self.unit_is_invisible(loop_unit, team, true) {
                    if self.unit_can_defend(loop_unit, None) {
                        let flanking_strength = info.flanking_strike_unit_class[self.unit_get_unit_class_type(loop_unit)];

                        if flanking_strength > 0 {
                            // C++ quirk: the flanked unit's combat values are computed but unused; the
                            // roll below uses the odds passed in for the main defender.
                            let _ = self.unit_get_defender_combat_values(
                                u,
                                loop_unit,
                                plot,
                                attacker_strength,
                                attacker_firepower,
                                None,
                            );

                            if self.soren_rand_num(d.defines.combat_die_sides, "Flanking Combat") >= defender_odds {
                                let collateral_damage = (flanking_strength * defender_damage) / 100;
                                let damage = self.units[loop_unit].damage;
                                let unit_damage =
                                    damage.max((damage + collateral_damage).min(self.unit_collateral_damage_limit(u)));

                                if damage != unit_damage {
                                    flanked_units.push((loop_unit, unit_damage));
                                }
                            }
                        }
                    }
                }
            }
        }

        let owner = self.units[u].owner;
        let num_units_hit = (flanked_units.len() as i32).min(self.unit_collateral_damage_max_units(u));

        for _ in 0..num_units_hit {
            let index_hit = self.soren_rand_num(flanked_units.len() as i32, "Pick Flanked Unit") as usize;
            let (unit, damage) = flanked_units[index_hit];
            // (a unit killed meanwhile, e.g. as cargo of a killed unit, would be a dangling
            // pointer in C++)
            if self.units.contains(unit) {
                self.unit_set_damage(unit, damage, Some(owner));
                if self.unit_is_dead(unit) {
                    let turn = self.game_turn();
                    let (unit_type, flanked_type) = (self.units[u].unit_type, self.units[unit].unit_type);
                    self.log.msg(turn, Some(owner), || {
                        format!("{} killed {} by flanking", unit_type_name(d, unit_type), unit_type_name(d, flanked_type))
                    });

                    self.unit_kill(unit, false, None);
                }
            }

            // C++ `erase(std::remove(.., listFlankedUnits[iIndexHit]))`: the entries are distinct
            // (one per unit), so this removes exactly that entry and keeps the order.
            flanked_units.remove(index_hit);
        }

        if num_units_hit > 0 {
            let turn = self.game_turn();
            let unit_type = self.units[u].unit_type;
            self.log.msg(turn, Some(owner), || {
                format!("{} damaged {} units by flanking", unit_type_name(d, unit_type), num_units_hit)
            });
        }
    }

    /// C++ `CvUnit::interceptTest` (CvUnit.cpp:17472-17490).
    /// Returns true if the unit was intercepted.
    pub fn unit_intercept_test(&mut self, u: UnitId, plot: PlotIdx) -> bool {
        if self.soren_rand_num(100, "Evasion Rand") >= self.unit_evasion_probability(u) {
            if let Some(interceptor) = self.unit_best_interceptor(u, plot) {
                if self.soren_rand_num(100, "Intercept Rand (Air)") < self.unit_curr_interception_probability(interceptor) {
                    self.unit_fight_interceptor(u, plot);

                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvUnit::airStrikeTarget` (CvUnit.cpp:17493-17511).
    pub fn unit_air_strike_target(&self, u: UnitId, plot: PlotIdx) -> Option<UnitId> {
        let owner = self.units[u].owner;
        let defender = self.plot_get_best_defender(plot, None, Some(owner), Some(u), true, false, false)?;

        if !self.unit_is_dead(defender) && self.unit_can_defend(defender, None) {
            return Some(defender);
        }

        None
    }

    /// C++ `CvUnit::canAirStrike` (CvUnit.cpp:17514-17547).
    pub fn unit_can_air_strike(&self, u: UnitId, plot: PlotIdx) -> bool {
        if self.unit_get_domain_type(u) != DomainType::DOMAIN_AIR {
            return false;
        }

        if !self.unit_can_air_attack(u) {
            return false;
        }

        if self.unit_plot_opt(u) == Some(plot) {
            return false;
        }

        if !self.plot_is_visible(plot, self.unit_get_team(u)) {
            return false;
        }

        let (x, y) = self.map.xy(plot);
        if self.plot_distance(self.units[u].x, self.units[u].y, x, y) > self.unit_air_range(u) {
            return false;
        }

        if self.unit_air_strike_target(u, plot).is_none() {
            return false;
        }

        true
    }

    /// C++ `CvUnit::airStrike` (CvUnit.cpp:17550-17587).
    pub fn unit_air_strike(&mut self, u: UnitId, plot: PlotIdx) -> bool {
        let d = self.d;

        if !self.unit_can_air_strike(u, plot) {
            return false;
        }

        if self.unit_intercept_test(u, plot) {
            return false;
        }

        // (C++ FAssert(pDefender != NULL && pDefender->canDefend()): nothing changed since
        // `canAirStrike`)
        let defender = self.unit_air_strike_target(u, plot).expect("CvUnit::airStrike: no target");

        self.unit_set_recon_plot(u, Some(plot));

        self.units[u].made_attack = true;
        self.unit_change_moves(u, d.defines.move_denominator);

        let damage = self.unit_air_combat_damage(u, defender);

        let defender_damage = self.units[defender].damage;
        let unit_damage = defender_damage.max((defender_damage + damage).min(self.unit_air_combat_limit(u)));

        let turn = self.game_turn();
        let owner = self.units[u].owner;
        let (unit_type, defender_type) = (self.units[u].unit_type, self.units[defender].unit_type);
        self.log.msg(turn, Some(owner), || {
            format!(
                "{} air strike on {} ({} damage)",
                unit_type_name(d, unit_type),
                unit_type_name(d, defender_type),
                unit_damage - defender_damage
            )
        });

        self.unit_collateral_combat(u, plot, Some(defender));

        self.unit_set_damage(defender, unit_damage, Some(owner));

        true
    }

    /// C++ `CvUnit::canRangeStrike` (CvUnit.cpp:17589-17622).
    pub fn unit_can_range_strike(&self, u: UnitId) -> bool {
        if self.unit_get_domain_type(u) == DomainType::DOMAIN_AIR {
            return false;
        }

        if self.unit_air_range(u) <= 0 {
            return false;
        }

        if self.unit_air_base_combat_str(u) <= 0 {
            return false;
        }

        if !self.unit_can_fight(u) {
            return false;
        }

        if self.units[u].made_attack && !self.unit_is_blitz(u) {
            return false;
        }

        if !self.unit_can_move(u) && self.units[u].moves > 0 {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canRangeStrikeAt` (CvUnit.cpp:17624-17660).
    /// `plot` is the plot the strike is made from; `x`/`y` the target (may be off the map).
    /// C++ passes `getFacingDirection(true)` to `canSeePlot`: always NO_DIRECTION with FfH data
    /// (no unit has bLineOfSight).
    pub fn unit_can_range_strike_at(&self, u: UnitId, plot: PlotIdx, x: i32, y: i32) -> bool {
        if !self.unit_can_range_strike(u) {
            return false;
        }

        let Some(target_plot) = self.map.plot_xy(x, y) else {
            return false;
        };

        let team = self.unit_get_team(u);
        if !self.plot_is_visible(plot, team) {
            return false;
        }

        let (from_x, from_y) = self.map.xy(plot);
        let (target_x, target_y) = self.map.xy(target_plot);
        let air_range = self.unit_air_range(u);
        if self.plot_distance(from_x, from_y, target_x, target_y) > air_range {
            return false;
        }

        if self.unit_air_strike_target(u, target_plot).is_none() {
            return false;
        }

        if !self.plot_can_see_plot(plot, target_plot, team, air_range, None) {
            return false;
        }

        true
    }

    /// C++ `CvUnit::rangeStrike` (CvUnit.cpp:17663-17726).
    /// C++ sets the defender group's mission timer only inside `isActiveVisible` (interface);
    /// that is animation pacing, not rules.
    pub fn unit_range_strike(&mut self, u: UnitId, x: i32, y: i32) -> bool {
        let d = self.d;

        let Some(plot) = self.map.plot_xy(x, y) else {
            return false;
        };

        // C++ quirk (BtS): the target plot is passed as the plot the strike is made from, so the
        // range and line-of-sight tests are trivially met here (callers test from `plot()`).
        if !self.unit_can_range_strike_at(u, plot, x, y) {
            return false;
        }

        // (C++ FAssert(pDefender != NULL && pDefender->canDefend()), checked just above)
        let defender = self.unit_air_strike_target(u, plot).expect("CvUnit::rangeStrike: no target");

        if d.defines.ranged_attacks_use_moves == 0 {
            self.units[u].made_attack = true;
        }
        self.unit_change_moves(u, d.defines.move_denominator);

        let damage = self.unit_range_combat_damage(u, defender);

        let defender_damage = self.units[defender].damage;
        let unit_damage = defender_damage.max((defender_damage + damage).min(self.unit_air_combat_limit(u)));

        let turn = self.game_turn();
        let owner = self.units[u].owner;
        let (unit_type, defender_type) = (self.units[u].unit_type, self.units[defender].unit_type);
        self.log.msg(turn, Some(owner), || {
            format!(
                "{} ranged attack on {} ({} damage)",
                unit_type_name(d, unit_type),
                unit_type_name(d, defender_type),
                unit_damage - defender_damage
            )
        });

        self.unit_collateral_combat(u, plot, Some(defender));

        // set damage but don't update entity damage visibility
        self.unit_set_damage(defender, unit_damage, Some(owner));

        // UI: the range attack animation (and its mission timer on the defender's group).

        true
    }

    /// C++ `CvUnit::isTargetOf` (CvUnit.cpp:17980-18009).
    pub fn unit_is_target_of(&self, u: UnitId, attacker: UnitId) -> bool {
        let d = self.d;
        let attacker_info = &d.units[self.units[attacker].unit_type];
        let our_info = &d.units[self.units[u].unit_type];

        if !self.plot_is_city(self.unit_plot(u), true, Some(self.unit_get_team(u))) {
            // (C++ also tests `NO_UNITCLASS != getUnitClassType()`: every unit has a class)
            if attacker_info.target_unit_class[self.unit_get_unit_class_type(u)] {
                return true;
            }

            if let Some(unit_combat) = self.unit_get_unit_combat_type(u) {
                if attacker_info.target_unit_combat[unit_combat] {
                    return true;
                }
            }
        }

        if let Some(attacker_class) = attacker_info.unit_class_type {
            if our_info.defender_unit_class[attacker_class] {
                return true;
            }
        }

        if let Some(attacker_combat) = attacker_info.unit_combat_type {
            if our_info.defender_unit_combat[attacker_combat] {
                return true;
            }
        }

        false
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
        let d = self.d;
        let combat_die_sides = d.defines.combat_die_sides;

        let their_strength = self.unit_curr_combat_str(defender, Some(plot), Some(u), their_details);
        let their_firepower = self.unit_curr_firepower(defender, Some(plot), Some(u));

        // (C++ FAssert((iOurStrength + iTheirStrength) > 0), FAssert((iOurFirepower + iTheirFirepower) > 0))
        let mut their_odds = (combat_die_sides * their_strength) / (our_strength + their_strength);

        if self.unit_is_barbarian(defender) {
            let owner = self.units[u].owner;
            if self.players[owner].wins_vs_barbs < free_wins_vs_barbs(self, owner) {
                their_odds = ((10 * combat_die_sides) / 100).min(their_odds);
            }
        }
        if self.unit_is_barbarian(u) {
            let defender_owner = self.units[defender].owner;
            if self.players[defender_owner].wins_vs_barbs < free_wins_vs_barbs(self, defender_owner) {
                their_odds = ((90 * combat_die_sides) / 100).max(their_odds);
            }
        }

        let (our_damage, their_damage) = combat_round_damages(d.defines.combat_damage, our_firepower, their_firepower);

        DefenderCombatValues { their_odds, their_strength, our_damage, their_damage }
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

    const W: i32 = 6;
    const H: i32 = 4;

    /// A game on a 6x4 land map (one area) with real `Player` objects (not `player_init`ed).
    fn test_game() -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 11));
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

    /// A unit (C++ `addUnit` + `reset`) placed directly on (x, y): in the plot's unit list and
    /// in a group of its own, without `init`/`setXY`.
    fn add_unit(g: &mut Game, owner: PlayerId, unit_type: &str, x: i32, y: i32) -> UnitId {
        let d = g.d;
        let t = d.t::<UnitType>(unit_type);
        let id = g.units.next_id();
        let u = g.player_add_unit(owner, Unit::new(id, t, owner, d, &UnitResetInfo::from_info(&d.units[t])));
        g.units[u].x = x;
        g.units[u].y = y;
        let plot = g.map.plot_idx(x, y).unwrap();
        g.map.plots[plot].units.push(u);
        let gid = g.groups.next_id();
        let group = g.player_add_selection_group(owner, SelectionGroup::new(gid, owner));
        g.groups[group].units.push(u);
        g.units[u].group = Some(group);
        u
    }

    fn declare_war(g: &mut Game, a: PlayerId, b: PlayerId) {
        let (ta, tb) = (g.players[a].team, g.players[b].team);
        g.teams[ta].at_war[tb] = true;
        g.teams[tb].at_war[ta] = true;
    }

    #[test]
    fn modifier_and_round_damage_arithmetic() {
        // Positive modifiers multiply, others divide (integer division as in C++).
        assert_eq!(apply_combat_modifier(500, 0), 500);
        assert_eq!(apply_combat_modifier(500, 50), 750);
        assert_eq!(apply_combat_modifier(500, -50), 333);
        assert_eq!(apply_combat_modifier(450, 25), 562);
        assert_eq!(apply_combat_modifier(700, -100), 350);

        // Equal firepower: COMBAT_DAMAGE each way.
        assert_eq!(combat_round_damages(20, 1000, 1000), (20, 20));
        // strength factor (1500 + 500 + 1) / 2 = 1000: 20 * 1500 / 2500 and 20 * 2500 / 1500.
        assert_eq!(combat_round_damages(20, 1500, 500), (12, 33));
        assert_eq!(combat_round_damages(20, 500, 1500), (33, 12));
        // The strength factor bounds the ratio at 3x: (20 * 50002) / 150001 and (20 * 150001) / 50002.
        assert_eq!(combat_round_damages(20, 100_000, 1), (6, 59));
        // Never below 1.
        assert_eq!(combat_round_damages(1, 100_000, 1), (1, 2));
    }

    #[test]
    fn attack_plot_and_combat_state() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);
        let enemy = add_unit(&mut g, PlayerId(1), "UNIT_WARRIOR", 2, 1);
        let target = g.map.plot_idx(2, 1).unwrap();

        assert_eq!(g.unit_get_attack_plot(u), None);
        assert!(!g.unit_is_attacking(u) && !g.unit_is_defending(u) && !g.unit_is_combat(u));

        g.unit_set_attack_plot(u, Some(target), true);
        assert_eq!((g.units[u].attack_plot_x, g.units[u].attack_plot_y), (2, 1));
        assert_eq!(g.unit_get_attack_plot(u), Some(target));
        assert!(g.units[u].air_combat);
        assert!(g.unit_is_attacking(u) && g.unit_is_combat(u) && !g.unit_is_defending(u));

        // A unit about to die is no longer attacking.
        g.units[u].death_delay = true;
        assert!(!g.unit_is_attacking(u));
        g.units[u].death_delay = false;

        // The defender side: fighting without an attack plot.
        g.units[enemy].combat_unit = Some(u);
        assert!(g.unit_is_fighting(enemy) && g.unit_is_defending(enemy) && g.unit_is_combat(enemy));

        // Same plot again: only the air combat flag changes.
        g.unit_set_attack_plot(u, Some(target), false);
        assert_eq!(g.unit_get_attack_plot(u), Some(target));
        assert!(!g.units[u].air_combat);

        g.unit_set_attack_plot(u, None, false);
        assert_eq!((g.units[u].attack_plot_x, g.units[u].attack_plot_y), (INVALID_PLOT_COORD, INVALID_PLOT_COORD));
        assert!(!g.unit_is_attacking(u));
    }

    #[test]
    fn clearing_the_combat_unit() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);
        let enemy = add_unit(&mut g, PlayerId(1), "UNIT_WARRIOR", 2, 1);
        let rng = g.soren_rand.clone();

        g.units[u].combat_unit = Some(enemy);
        g.units[u].combat_first_strikes = 2;
        g.units[u].combat_timer = 3;
        g.unit_set_combat_unit(u, None, false);
        assert_eq!(g.units[u].combat_unit, None);
        assert_eq!((g.units[u].combat_first_strikes, g.units[u].combat_timer), (0, 0));

        // C++ quirk: a vanished opponent is not cleared (only the timer is reset).
        let gone = g.units.next_id();
        g.units[u].combat_unit = Some(gone);
        g.units[u].combat_first_strikes = 1;
        g.units[u].combat_timer = 2;
        g.unit_set_combat_unit(u, None, false);
        assert_eq!(g.units[u].combat_unit, Some(gone));
        assert_eq!((g.units[u].combat_first_strikes, g.units[u].combat_timer), (1, 0));
        assert!(!g.unit_is_fighting(u));

        assert_eq!(g.soren_rand, rng, "clearing draws no rand");
    }

    #[test]
    fn combat_timers_count_down() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);

        // A running timer only counts down.
        g.units[u].combat_timer = 3;
        g.unit_update_combat(u);
        assert_eq!(g.units[u].combat_timer, 2);
        g.unit_update_air_combat(u);
        assert_eq!(g.units[u].combat_timer, 1);

        // The last tick finishes the combat; without an attack plot there is nothing to finish.
        g.unit_update_combat(u);
        assert_eq!(g.units[u].combat_timer, 0);
        assert_eq!(g.unit_get_attack_plot(u), None);
    }

    #[test]
    fn combat_won_heals_without_rolls() {
        let mut g = test_game();
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        declare_war(&mut g, p0, p1);
        let winner = add_unit(&mut g, p0, "UNIT_WARRIOR", 1, 1);
        let loser = add_unit(&mut g, p1, "UNIT_WARRIOR", 2, 1);
        let rng = g.soren_rand.clone();

        // FfH combat healing: up to iCombatHealPercent, never below 0 damage.
        g.units[winner].damage = 25;
        g.units[winner].combat_heal_percent = 10;
        g.unit_combat_won(winner, loser, true);
        assert_eq!(g.units[winner].damage, 15);
        g.units[winner].combat_heal_percent = 40;
        g.unit_combat_won(winner, loser, true);
        assert_eq!(g.units[winner].damage, 0);

        // Plain warriors have no combat promotions, captures, slaves or conversions to roll for.
        assert_eq!(g.soren_rand, rng);
        assert_eq!(g.players[p0].units.len(), 1);
    }

    #[test]
    fn air_strength_info_queries() {
        let mut g = test_game();
        let d = g.d;
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 0, 0);
        let info = &d.units[g.units[u].unit_type];
        assert_eq!(g.unit_air_base_combat_str(u), info.air_combat);
        assert_eq!(g.unit_air_combat_limit(u), info.air_combat_limit);
        assert_eq!(g.unit_can_air_attack(u), info.air_combat > 0);
        // A unit without air strength has no air strength at all.
        if info.air_combat == 0 {
            assert_eq!(g.unit_air_max_combat_str(u, None), 0);
        }
    }

    #[test]
    fn defensive_strike_rolls() {
        let mut g = test_game();
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        declare_war(&mut g, p0, p1);

        let attacker = add_unit(&mut g, p0, "UNIT_WARRIOR", 1, 1);
        let defender = add_unit(&mut g, p1, "UNIT_WARRIOR", 2, 1);
        // Stack: a unit without defensive strikes (no roll), a striker that already attacked (no
        // roll), and a certain striker.
        let idle = add_unit(&mut g, p1, "UNIT_WARRIOR", 2, 1);
        let tired = add_unit(&mut g, p1, "UNIT_WARRIOR", 2, 1);
        let striker = add_unit(&mut g, p1, "UNIT_WARRIOR", 2, 1);
        g.units[idle].defensive_strike_chance = 0;
        g.units[defender].defensive_strike_chance = 0;
        g.units[tired].defensive_strike_chance = 100;
        g.units[tired].defensive_strike_damage = 30;
        g.units[tired].made_attack = true;
        g.units[striker].defensive_strike_chance = 100;
        g.units[striker].defensive_strike_damage = 10;

        // The attacker's defense absorbs any strike (2 * 20 > 2 * (10 - 1)), so no damage and no
        // experience change.
        g.units[attacker].base_combat_defense = 20;

        let mut rng = g.soren_rand.clone();
        assert!(rng.get_i(100, "Defensive Strike") < 100);
        rng.get_i(10, "Defensive Strike 1");
        rng.get_i(10, "Defensive Strike 2");

        g.unit_do_defensive_strike(defender, attacker);
        assert_eq!(g.soren_rand, rng, "one chance roll for the striker, then two damage rolls");
        assert!(g.units[striker].made_attack, "a striker without blitz has made its attack");
        assert_eq!(g.units[attacker].damage, 0);

        // Not at war: no rolls at all.
        let mut g2 = test_game();
        let attacker = add_unit(&mut g2, p0, "UNIT_WARRIOR", 1, 1);
        let defender = add_unit(&mut g2, p1, "UNIT_WARRIOR", 2, 1);
        g2.units[defender].defensive_strike_chance = 100;
        g2.units[defender].defensive_strike_damage = 50;
        let rng = g2.soren_rand.clone();
        g2.unit_do_defensive_strike(defender, attacker);
        assert_eq!(g2.soren_rand, rng);
        assert!(!g2.units[defender].made_attack);
    }

    /// A Hidden attacker that wins loses Hidden (removed by combat) in `combatWon`, turns visible
    /// on a plot it shares with an enemy that could not see it, and with no free plot around is
    /// killed on the spot (`withdrawlToNearestValidPlot(true)`). The combat must still finish
    /// without it: the defender dies, uncaptured (seed 36 with the Elohim forced, turn 607: a
    /// Champion that panicked `unit_combat_won` with a dangling id).
    #[test]
    fn attacker_killed_by_losing_hidden_still_finishes_the_combat() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 5));
        g.autorun = true;
        g.game_init();
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        declare_war(&mut g, p0, p1);
        let warrior = d.t::<UnitType>("UNIT_WARRIOR");
        let worker = d.t::<UnitType>("UNIT_WORKER");
        let hidden = d.t::<PromotionType>("PROMOTION_HIDDEN");
        assert!(d.promotions[hidden].removed_by_combat && d.promotions[hidden].invisible);

        // The plot and its eight neighbours (those on the map).
        let block = |g: &Game, p: PlotIdx| -> Vec<PlotIdx> {
            let (x, y) = g.map.xy(p);
            (-1..=1).flat_map(|i| (-1..=1).map(move |j| (i, j))).filter_map(|(i, j)| g.plot_xy(x, y, i, j)).collect()
        };
        let empty = |g: &Game, p: PlotIdx| g.map.plots[p].units.is_empty() && g.map.plots[p].plot_city.is_none();
        let open_land = |g: &Game, p: PlotIdx| !g.plot_is_water(p) && !g.plot_is_impassable(p);
        // An open land plot `a` in an empty block, next to open land `b`.
        let (a, b) = g
            .map
            .plot_ids()
            .filter(|&a| open_land(&g, a) && block(&g, a).iter().all(|&p| empty(&g, p)))
            .find_map(|a| block(&g, a).into_iter().find(|&b| b != a && open_land(&g, b)).map(|b| (a, b)))
            .expect("an open land plot next to open land");
        let ((ax, ay), (bx, by)) = (g.map.xy(a), g.map.xy(b));

        let attacker = g.player_init_unit(p0, warrior, ax, ay, None, None);
        g.unit_set_has_promotion(attacker, hidden, true);
        assert!(g.unit_is_invisible(attacker, g.players[p1].team, true));
        // An enemy that does not see it moves in; every other way out is closed: enemies on the land
        // around, and the defender on `b` (a worker, which cannot defend: no combat rolls).
        let lurker = g.player_init_unit(p1, warrior, ax, ay, None, None);
        assert_eq!(g.map.plots[a].units, vec![attacker, lurker]);
        for p in block(&g, a) {
            if p != a && p != b && g.plot_is_valid_domain_for_location(p, attacker) {
                let (x, y) = g.map.xy(p);
                g.player_init_unit(p1, warrior, x, y, None, None);
            }
        }
        let defender = g.player_init_unit(p1, worker, bx, by, None, None);
        for p in block(&g, a) {
            assert!(!g.unit_can_move_into(attacker, p, false, false, false), "no way out");
        }
        let (ours, theirs) = (g.players[p0].units.len(), g.players[p1].units.len());

        g.unit_attack(attacker, b);

        assert!(!g.units.contains(attacker), "visible next to the lurker with nowhere to go: killed");
        assert!(!g.units.contains(defender), "the defender still dies");
        assert_eq!(g.players[p0].units.len(), ours - 1, "nobody captures the worker");
        assert_eq!(g.players[p1].units.len(), theirs - 1);
        assert!(g.map.plots[b].units.is_empty());
        assert_eq!(g.map.plots[a].units, vec![lurker]);
    }
}
