//! CvUnit missions: spy actions (sabotage, destroy, steal plans, infiltrate, espionage), found
//! a city, spread religion/corporation, great people (join, construct, discover, hurry, trade,
//! great work, golden age) and worker builds.
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges; all of them are ported.
//!
//! Interface-only C++ (camera `lookAt`, `NotifyEntity` mission animations behind
//! `isActiveVisible`, popups and `addMessage`) is not ported; the outcome messages go to the
//! game log.

#![allow(unused_variables, unused_imports)]

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::unit::{CombatDetails, DefenderCombatValues, Unit, UnitResetInfo};
use crate::utils::{is_limited_project, is_limited_unit_class, is_limited_wonder_class};
use ffh_data::*;

/// C++ `CvPlot::getPlotCity()` (`getCity(m_plotCity)`: NULL for a deleted city).
fn live_plot_city(g: &Game, plot: PlotIdx) -> Option<CityId> {
    g.map.plots[plot].plot_city.filter(|&c| g.cities.contains(c))
}

/// C++ `GC.getGameSpeedInfo(GC.getGameINLINE().getGameSpeedType())`.
fn game_speed_info(g: &Game) -> &'static GameSpeedInfo {
    &g.d.game_speeds[g.game_speed_type()]
}

/// The unit counts shared by the spy mission probabilities (`sabotageProb`, `destroyProb`,
/// `stealPlansProb`): `(iDefenseCount, iCounterSpyCount)`, i.e. units of the plot's team
/// (`pPlot->getTeam()`, every unit for NO_TEAM) on `plot` that pass `PUF_canDefend`, and those
/// passing `PUF_isCounterSpy` on `plot` and its adjacent plots (in direction order).
fn spy_unit_counts(g: &Game, plot: PlotIdx) -> (i32, i32) {
    let team = g.plot_get_team(plot);

    let defense_count = g.plot_plot_count(plot, Some(Game::puf_can_defend), -1, -1, None, team, None, -1, -1);

    let mut counter_spy_count = g.plot_plot_count(plot, Some(Game::puf_is_counter_spy), -1, -1, None, team, None, -1, -1);

    let (x, y) = g.map.xy(plot);
    for i in 0..DirectionType::NUM {
        if let Some(loop_plot) = g.plot_direction(x, y, DirectionType::from_usize(i)) {
            counter_spy_count += g.plot_plot_count(loop_plot, Some(Game::puf_is_counter_spy), -1, -1, None, team, None, -1, -1);
        }
    }

    (defense_count, counter_spy_count)
}

/// Game-log stand-in for a C++ `addMessage` to `player`; the text is only built when the log
/// is enabled.
fn log_message(g: &mut Game, player: Option<PlayerId>, text: impl FnOnce(&Game) -> String) {
    if g.log.enabled {
        let turn = g.game_turn();
        let text = text(g);
        g.log.msg(turn, player, || text);
    }
}

/// Display name of a city for log messages.
fn city_display_name(g: &Game, city: CityId) -> String {
    g.d.text.name(&g.cities[city].name).to_string()
}

/// Python `CvGameUtils.cannotSpreadReligion(argsList)`: FFH2 0.41 returns False. `canSpread`
/// only calls it when `USE_USE_CANNOT_SPREAD_RELIGION_CALLBACK` is set (the define is not in the
/// FFH2 XML, so C++ reads 0).
fn py_cannot_spread_religion(g: &Game, owner: PlayerId, u: UnitId, religion: ReligionType, x: i32, y: i32) -> bool {
    false
}

impl Game {
    /// C++ `CvUnit::sabotageCost` (CvUnit.cpp:5857-5860).
    pub fn unit_sabotage_cost(&self, u: UnitId, plot: PlotIdx) -> i32 {
        self.d.defines.base_spy_sabotage_cost
    }

    /// C++ `CvUnit::sabotageProb` (CvUnit.cpp:5864-5908).
    /// C++ default `eProbStyle = PROBABILITY_REAL`.
    pub fn unit_sabotage_prob(&self, u: UnitId, plot: PlotIdx, prob_style: ProbabilityType) -> i32 {
        let mut prob = 0; // XXX

        let (defense_count, mut counter_spy_count) = if self.plot_is_owned(plot) { spy_unit_counts(self, plot) } else { (0, 0) };

        if prob_style == ProbabilityType::PROBABILITY_HIGH {
            counter_spy_count = 0;
        }

        prob += 40 / (defense_count + 1); // XXX

        if prob_style != ProbabilityType::PROBABILITY_LOW {
            prob += 50 / (counter_spy_count + 1); // XXX
        }

        prob
    }

    /// C++ `CvUnit::canSabotage` (CvUnit.cpp:5911-5942).
    pub fn unit_can_sabotage(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        if !self.d.units[self.units[u].unit_type].sabotage {
            return false;
        }

        if self.plot_get_team(plot) == Some(self.unit_get_team(u)) {
            return false;
        }

        if self.plot_is_city(plot, false, None) {
            return false;
        }

        if self.map.plots[plot].improvement_type.is_none() {
            return false;
        }

        if !test_visible {
            if self.players[self.units[u].owner].gold < self.unit_sabotage_cost(u, plot) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::sabotage` (CvUnit.cpp:5945-6016).
    pub fn unit_sabotage(&mut self, u: UnitId) -> bool {
        let d = self.d;
        let plot = self.unit_plot(u);

        if !self.unit_can_sabotage(u, plot, false) {
            return false;
        }

        let caught = self.soren_rand_num(100, "Spy: Sabotage") > self.unit_sabotage_prob(u, plot, ProbabilityType::PROBABILITY_REAL);

        let owner = self.units[u].owner;
        let cost = self.unit_sabotage_cost(u, plot);
        self.players[owner].gold -= cost;

        if !caught {
            // `canSabotage` requires an improvement.
            let improvement = self.map.plots[plot].improvement_type.expect("CvUnit::sabotage: plot without an improvement");
            self.plot_set_improvement_type(plot, d.improvements[improvement].improvement_pillage);

            self.unit_finish_moves(u);

            // TXT_KEY_MISC_SPY_SABOTAGED to the owner and TXT_KEY_MISC_SABOTAGE_NEAR to the plot
            // owner, naming the plot owner's nearest city (no message without one).
            let plot_owner = self.map.plots[plot].owner;
            log_message(self, Some(owner), |g| {
                let (x, y) = g.map.xy(plot);
                let near = g.map_find_city(x, y, plot_owner, None, false, false, None, None, None);
                let near = near.map(|c| city_display_name(g, c)).unwrap_or_default();
                format!("{} has sabotaged an improvement near {}", g.unit_get_name(u), near)
            });
        } else {
            // TXT_KEY_MISC_SPY_CAUGHT_AND_KILLED (to the plot owner) and TXT_KEY_MISC_YOUR_SPY_CAUGHT.
            log_message(self, Some(owner), |g| format!("Your {} has been caught!", g.unit_get_name(u)));

            if self.plot_is_owned(plot) {
                let plot_team = self.plot_get_team(plot);
                if !self.unit_is_enemy(u, plot_team, Some(plot)) {
                    let plot_owner = self.map.plots[plot].owner.expect("owned plot");
                    self.players[plot_owner].ai_memory_count[owner][MemoryType::MEMORY_SPY_CAUGHT] += 1;
                }
            }

            let plot_owner = self.map.plots[plot].owner;
            self.unit_kill(u, true, plot_owner);
        }

        true
    }

    /// C++ `CvUnit::destroyCost` (CvUnit.cpp:6019-6047).
    pub fn unit_destroy_cost(&self, u: UnitId, plot: PlotIdx) -> i32 {
        let d = self.d;

        let Some(city) = live_plot_city(self, plot) else {
            return 0;
        };

        let mut limited = false;

        if self.city_is_production_unit(city) {
            let unit = self.city_production_unit(city).expect("CvUnit::destroyCost: unit production without a unit");
            let unit_class = d.units[unit].unit_class_type.expect("CvUnitInfo without a unit class");
            limited = is_limited_unit_class(d, unit_class);
        } else if self.city_is_production_building(city) {
            let building = self.city_production_building(city).expect("CvUnit::destroyCost: building production without a building");
            let building_class = d.buildings[building].building_class_type.expect("CvBuildingInfo without a building class");
            limited = is_limited_wonder_class(d, building_class);
        } else if self.city_is_production_project(city) {
            let project = self.city_production_project(city).expect("CvUnit::destroyCost: project production without a project");
            limited = is_limited_project(d, project);
        }

        let multiplier = if limited { d.defines.spy_destroy_cost_multiplier_limited } else { d.defines.spy_destroy_cost_multiplier };
        d.defines.base_spy_destroy_cost + self.city_production(city) * multiplier
    }

    /// C++ `CvUnit::destroyProb` (CvUnit.cpp:6050-6097).
    pub fn unit_destroy_prob(&self, u: UnitId, plot: PlotIdx, prob_style: ProbabilityType) -> i32 {
        let Some(city) = live_plot_city(self, plot) else {
            return 0;
        };

        let mut prob = 0; // XXX

        let (defense_count, mut counter_spy_count) = spy_unit_counts(self, plot);

        if prob_style == ProbabilityType::PROBABILITY_HIGH {
            counter_spy_count = 0;
        }

        prob += 25 / (defense_count + 1); // XXX

        if prob_style != ProbabilityType::PROBABILITY_LOW {
            prob += 50 / (counter_spy_count + 1); // XXX
        }

        prob += std::cmp::min(25, self.city_production_turns_left(city)); // XXX

        prob
    }

    /// C++ `CvUnit::canDestroy` (CvUnit.cpp:6100-6135).
    pub fn unit_can_destroy(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        if !self.d.units[self.units[u].unit_type].destroy {
            return false;
        }

        if self.plot_get_team(plot) == Some(self.unit_get_team(u)) {
            return false;
        }

        let Some(city) = live_plot_city(self, plot) else {
            return false;
        };

        if self.city_production(city) == 0 {
            return false;
        }

        if !test_visible {
            if self.players[self.units[u].owner].gold < self.unit_destroy_cost(u, plot) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::destroy` (CvUnit.cpp:6138-6195).
    pub fn unit_destroy(&mut self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_destroy(u, plot, false) {
            return false;
        }

        let caught = self.soren_rand_num(100, "Spy: Destroy") > self.unit_destroy_prob(u, plot, ProbabilityType::PROBABILITY_REAL);

        let city = live_plot_city(self, plot).expect("City is not assigned a valid value");

        let owner = self.units[u].owner;
        let cost = self.unit_destroy_cost(u, plot);
        self.players[owner].gold -= cost;

        if !caught {
            let production = self.city_production(city);
            self.city_set_production(city, production / 2);

            self.unit_finish_moves(u);

            // TXT_KEY_MISC_SPY_DESTROYED_PRODUCTION / TXT_KEY_MISC_CITY_PRODUCTION_DESTROYED.
            log_message(self, Some(owner), |g| {
                format!("{} has destroyed the production in {}", g.unit_get_name(u), city_display_name(g, city))
            });
        } else {
            // TXT_KEY_MISC_SPY_CAUGHT_AND_KILLED (to the city owner) and TXT_KEY_MISC_YOUR_SPY_CAUGHT.
            log_message(self, Some(owner), |g| format!("Your {} has been caught!", g.unit_get_name(u)));

            let city_team = self.city_team(city);
            let city_owner = self.cities[city].owner;
            if !self.unit_is_enemy(u, Some(city_team), None) {
                self.players[city_owner].ai_memory_count[owner][MemoryType::MEMORY_SPY_CAUGHT] += 1;
            }

            self.unit_kill(u, true, Some(city_owner));
        }

        true
    }

    /// C++ `CvUnit::stealPlansCost` (CvUnit.cpp:6198-6210).
    pub fn unit_steal_plans_cost(&self, u: UnitId, plot: PlotIdx) -> i32 {
        let Some(city) = live_plot_city(self, plot) else {
            return 0;
        };

        let city_team = self.city_team(city);
        self.d.defines.base_spy_steal_plans_cost
            + (self.team_get_total_land(city_team, true) + self.team_get_total_population(city_team, true))
                * self.d.defines.spy_steal_plans_cost_multiplier
    }

    /// C++ `CvUnit::stealPlansProb` (CvUnit.cpp:6214-6259).
    pub fn unit_steal_plans_prob(&self, u: UnitId, plot: PlotIdx, prob_style: ProbabilityType) -> i32 {
        let Some(city) = live_plot_city(self, plot) else {
            return 0;
        };

        let mut prob = if self.city_is_government_center(city) { 20 } else { 0 }; // XXX

        let (defense_count, mut counter_spy_count) = spy_unit_counts(self, plot);

        if prob_style == ProbabilityType::PROBABILITY_HIGH {
            counter_spy_count = 0;
        }

        prob += 20 / (defense_count + 1); // XXX

        if prob_style != ProbabilityType::PROBABILITY_LOW {
            prob += 50 / (counter_spy_count + 1); // XXX
        }

        prob
    }

    /// C++ `CvUnit::canStealPlans` (CvUnit.cpp:6262-6292).
    pub fn unit_can_steal_plans(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        if !self.d.units[self.units[u].unit_type].steal_plans {
            return false;
        }

        if self.plot_get_team(plot) == Some(self.unit_get_team(u)) {
            return false;
        }

        if live_plot_city(self, plot).is_none() {
            return false;
        }

        if !test_visible {
            if self.players[self.units[u].owner].gold < self.unit_steal_plans_cost(u, plot) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::stealPlans` (CvUnit.cpp:6295-6352).
    pub fn unit_steal_plans(&mut self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_steal_plans(u, plot, false) {
            return false;
        }

        let caught = self.soren_rand_num(100, "Spy: Steal Plans") > self.unit_steal_plans_prob(u, plot, ProbabilityType::PROBABILITY_REAL);

        let city = live_plot_city(self, plot).expect("City is not assigned a valid value");

        let owner = self.units[u].owner;
        let cost = self.unit_steal_plans_cost(u, plot);
        self.players[owner].gold -= cost;

        let city_team = self.city_team(city);

        if !caught {
            let team = self.unit_get_team(u);
            self.team_change_stolen_visibility_timer(team, city_team, 2);

            self.unit_finish_moves(u);

            // TXT_KEY_MISC_SPY_STOLE_PLANS / TXT_KEY_MISC_PLANS_STOLEN.
            log_message(self, Some(owner), |g| format!("{} has stolen the plans of {}", g.unit_get_name(u), city_display_name(g, city)));
        } else {
            // TXT_KEY_MISC_SPY_CAUGHT_AND_KILLED (to the city owner) and TXT_KEY_MISC_YOUR_SPY_CAUGHT.
            log_message(self, Some(owner), |g| format!("Your {} has been caught!", g.unit_get_name(u)));

            let city_owner = self.cities[city].owner;
            if !self.unit_is_enemy(u, Some(city_team), None) {
                self.players[city_owner].ai_memory_count[owner][MemoryType::MEMORY_SPY_CAUGHT] += 1;
            }

            self.unit_kill(u, true, Some(city_owner));
        }

        true
    }

    /// C++ `CvUnit::canFound` (CvUnit.cpp:6355-6379).
    pub fn unit_can_found(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        if !self.unit_is_found(u) {
            return false;
        }

        let (x, y) = self.map.xy(plot);
        if !self.player_can_found(self.units[u].owner, x, y, test_visible) {
            return false;
        }

        // FfH: Added by Kael 05/06/2010
        if self.map.plots[plot].found_disabled {
            return false;
        }
        if self.units[u].has_casted {
            return false;
        }
        // FfH: End Add

        true
    }

    /// C++ `CvUnit::found` (CvUnit.cpp:6382-6441).
    /// For AI players, the founder's UNITAI_COUNTER group mates first leave its group (Better AI
    /// bookkeeping); the CvUnitAI `isUnitAllowedPermDefense` conversion of some of them into
    /// city defenders is omitted (`// AI:` note).
    pub fn unit_found(&mut self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_found(u, plot, false) {
            return false;
        }

        // UI: the active player's camera looks at the plot.

        // BETTER AI (Use Patrol for CityDefense) Sephi: the founder's UNITAI_COUNTER escorts
        // leave its group.
        if !self.unit_is_human(u) {
            // C++ walks `getGroup()->headUnitNode()` advancing with `plot()->nextUnitNode(..)`
            // (CLinkList::next only follows the node link, so this is the group's list) and
            // fetches the next node before the body runs: iterate a copy. C++ dereferences
            // `getGroup()`; every unit on the map has one.
            if let Some(group) = self.units[u].group.filter(|&g| self.groups.contains(g)) {
                for loop_unit in self.groups[group].units.clone() {
                    if !self.units.contains(loop_unit) {
                        continue;
                    }
                    if loop_unit != u && self.units[loop_unit].ai_type == Some(UnitAIType::UNITAI_COUNTER) {
                        // AI: `pLoopUnit->isUnitAllowedPermDefense()` (CvUnitAI valuation, omitted)
                        // additionally turns allowed units into permanent city defenders
                        // (`AI_setGroupflag(GROUPFLAG_PERMDEFENSE_NEW)` after `joinGroup(NULL)`,
                        // `AI_setUnitAIType(UNITAI_CITY_DEFENSE)` after `setOriginPlot(NULL)`).
                        // Both C++ branches leave the group and clear the origin plot.
                        self.unit_join_group(loop_unit, None, true);
                        self.unit_ai_set_origin_plot(loop_unit, None);
                    }
                }
            }
        }

        let owner = self.units[u].owner;
        let (x, y) = (self.units[u].x, self.units[u].y);
        self.player_found(owner, x, y);

        // Graphics: NotifyEntity(MISSION_FOUND).

        // C++ keeps using `this` (founding a city never destroys the founder).
        if self.units.contains(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::canSpread` (CvUnit.cpp:6444-6542).
    /// Ends with the `cannotSpreadReligion` Python callback (USE_USE_CANNOT_SPREAD_RELIGION_CALLBACK).
    pub fn unit_can_spread(
        &self,
        u: UnitId,
        plot: PlotIdx,
        religion: Option<ReligionType>,
        test_visible: bool,
    ) -> bool {
        // UNOFFICIAL_PATCH (efficiency): the Python callback moved below the faster checks.

        let Some(religion) = religion else {
            return false;
        };

        if self.d.units[self.units[u].unit_type].religion_spreads[religion] <= 0 {
            return false;
        }

        let Some(city) = live_plot_city(self, plot) else {
            return false;
        };

        if self.cities[city].has_religion[religion] {
            return false;
        }

        if !self.unit_can_enter_area(u, self.plot_get_team(plot), self.plot_area(plot), false) {
            return false;
        }

        if !test_visible {
            if self.city_team(city) != self.unit_get_team(u) {
                let city_owner = self.cities[city].owner;
                if self.player_is_no_non_state_religion_spread(city_owner) {
                    if Some(religion) != self.player_get_state_religion(city_owner) {
                        return false;
                    }
                }
            }
        }

        if self.d.defines.use_use_cannot_spread_religion_callback != 0 {
            let (x, y) = self.map.xy(plot);
            if py_cannot_spread_religion(self, self.units[u].owner, u, religion, x, y) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::spread` (CvUnit.cpp:6545-6609).
    pub fn unit_spread(&mut self, u: UnitId, religion: Option<ReligionType>) -> bool {
        let d = self.d;
        let plot = self.unit_plot(u);

        if !self.unit_can_spread(u, plot, religion, false) {
            return false;
        }

        // `canSpread` rejects NO_RELIGION.
        let religion = religion.expect("CvUnit::spread: no religion");

        if let Some(city) = live_plot_city(self, plot) {
            let mut spread_prob = d.units[self.units[u].unit_type].religion_spreads[religion];

            if self.city_team(city) != self.unit_get_team(u) {
                spread_prob /= 2;
            }

            let success;

            let num_religions = d.religions.len() as i32;
            spread_prob += ((num_religions - self.city_religion_count(city)) * (100 - spread_prob)) / num_religions;

            if self.soren_rand_num(100, "Unit Spread Religion") < spread_prob {
                // FfH: Modified by Kael 10/04/2008 (spreading an unfounded religion founds it here)
                if self.is_religion_founded(religion) {
                    self.city_set_has_religion(city, religion, true, true, false);
                } else {
                    self.city_set_has_religion(city, religion, true, true, false);
                    self.set_holy_city(religion, Some(city), true);
                    self.religion_slot_taken[religion] = true;
                }
                // FfH: End Modify

                success = true;
            } else {
                // TXT_KEY_MISC_RELIGION_FAILED_TO_SPREAD
                let owner = self.units[u].owner;
                log_message(self, Some(owner), |g| {
                    format!(
                        "{} failed to spread {} to {}",
                        g.unit_get_name(u),
                        g.d.text.name(&g.d.religions[religion].base.description),
                        city_display_name(g, city)
                    )
                });
                success = false;
            }

            // Python Event
            self.ev_unit_spread_religion_attempt(u, religion, success);
        }

        // Graphics: NotifyEntity(MISSION_SPREAD).

        if self.units.contains(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::canSpreadCorporation` (CvUnit.cpp:6612-6698).
    pub fn unit_can_spread_corporation(
        &self,
        u: UnitId,
        plot: PlotIdx,
        corporation: Option<CorporationType>,
        test_visible: bool,
    ) -> bool {
        let d = self.d;

        let Some(corporation) = corporation else {
            return false;
        };

        let owner = self.units[u].owner;

        if !self.player_is_active_corporation(owner, corporation) {
            return false;
        }

        if d.units[self.units[u].unit_type].corporation_spreads[corporation] <= 0 {
            return false;
        }

        let Some(city) = live_plot_city(self, plot) else {
            return false;
        };

        if self.cities[city].has_corporation[corporation] {
            return false;
        }

        if !self.unit_can_enter_area(u, self.plot_get_team(plot), self.plot_area(plot), false) {
            return false;
        }

        if !test_visible {
            if !self.player_is_active_corporation(self.cities[city].owner, corporation) {
                return false;
            }

            for loop_corporation in d.corporations.keys() {
                if self.city_is_headquarters(city, loop_corporation) {
                    if self.is_competing_corporation(loop_corporation, corporation) {
                        return false;
                    }
                }
            }

            let mut valid = false;

            // C++ `getPrereqBonus(i)` for i < NUM_CORPORATION_PREREQ_BONUSES: the listed bonuses,
            // then NO_BONUS (`prereq_bonus` holds only the listed ones).
            let prereq_bonuses = &d.corporations[corporation].prereq_bonus;

            // FfH: Added by Kael 10/21/2007 (So that corporations can be made to not require bonuses)
            if prereq_bonuses.is_empty() {
                // `getPrereqBonus(0) == NO_BONUS`
                valid = true;
            }
            // FfH: End Add

            let num_prereqs = d.defines.num_corporation_prereq_bonuses.max(0) as usize;
            for &bonus in prereq_bonuses.iter().take(num_prereqs) {
                if self.city_has_bonus(city, bonus) {
                    valid = true;
                    break;
                }
            }

            if !valid {
                return false;
            }

            if self.players[owner].gold < self.unit_spread_corporation_cost(u, corporation, Some(city)) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::spreadCorporationCost` (CvUnit.cpp:6700-6730).
    pub fn unit_spread_corporation_cost(&self, u: UnitId, corporation: CorporationType, city: Option<CityId>) -> i32 {
        let d = self.d;
        let owner = self.units[u].owner;

        let mut cost = std::cmp::max(0, d.corporations[corporation].spread_cost * (100 + self.player_calculate_inflation_rate(owner)));
        cost /= 100;

        if let Some(city) = city {
            let team = self.unit_get_team(u);
            let city_team = self.city_team(city);
            if team != city_team && !self.teams[city_team].vassal[team] {
                cost *= d.defines.corporation_foreign_spread_cost_percent;
                cost /= 100;
            }

            for loop_corporation in d.corporations.keys() {
                if loop_corporation != corporation {
                    if self.city_is_active_corporation(city, loop_corporation) {
                        if self.is_competing_corporation(corporation, loop_corporation) {
                            cost *= 100 + d.corporations[loop_corporation].spread_factor;
                            cost /= 100;
                        }
                    }
                }
            }
        }

        cost
    }

    /// C++ `CvUnit::spreadCorporation` (CvUnit.cpp:6732-6775).
    pub fn unit_spread_corporation(&mut self, u: UnitId, corporation: Option<CorporationType>) -> bool {
        let d = self.d;
        let plot = self.unit_plot(u);

        if !self.unit_can_spread_corporation(u, plot, corporation, false) {
            return false;
        }

        // `canSpreadCorporation` rejects NO_CORPORATION.
        let corporation = corporation.expect("CvUnit::spreadCorporation: no corporation");

        if let Some(city) = live_plot_city(self, plot) {
            let owner = self.units[u].owner;
            let cost = self.unit_spread_corporation_cost(u, corporation, Some(city));
            self.players[owner].gold -= cost;

            let mut spread_prob = d.units[self.units[u].unit_type].corporation_spreads[corporation];

            if self.city_team(city) != self.unit_get_team(u) {
                spread_prob /= 2;
            }

            let num_corporations = d.corporations.len() as i32;
            spread_prob += ((num_corporations - self.city_corporation_count(city)) * (100 - spread_prob)) / num_corporations;

            if self.soren_rand_num(100, "Unit Spread Corporation") < spread_prob {
                self.city_set_has_corporation(city, corporation, true, true, false);
            } else {
                // TXT_KEY_MISC_CORPORATION_FAILED_TO_SPREAD
                log_message(self, Some(owner), |g| {
                    format!(
                        "{} failed to spread {} to {}",
                        g.unit_get_name(u),
                        g.d.text.name(&g.d.corporations[corporation].base.description),
                        city_display_name(g, city)
                    )
                });
            }
        }

        // Graphics: NotifyEntity(MISSION_SPREAD_CORPORATION).

        if self.units.contains(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::canJoin` (CvUnit.cpp:6778-6822).
    pub fn unit_can_join(&self, u: UnitId, plot: PlotIdx, specialist: Option<SpecialistType>) -> bool {
        let Some(specialist) = specialist else {
            return false;
        };

        if !self.d.units[self.units[u].unit_type].great_peoples[specialist] {
            return false;
        }

        let Some(city) = live_plot_city(self, plot) else {
            return false;
        };

        if !self.city_can_join(city) {
            return false;
        }

        if self.city_team(city) != self.unit_get_team(u) {
            return false;
        }

        if self.units[u].death_delay {
            return false;
        }

        // FfH: Added by Kael 08/18/2008
        if self.units[u].has_casted {
            return false;
        }
        // FfH: End Add

        true
    }

    /// C++ `CvUnit::join` (CvUnit.cpp:6825-6849).
    pub fn unit_join(&mut self, u: UnitId, specialist: Option<SpecialistType>) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_join(u, plot, specialist) {
            return false;
        }

        // `canJoin` rejects NO_SPECIALIST.
        let specialist = specialist.expect("CvUnit::join: no specialist");

        if let Some(city) = live_plot_city(self, plot) {
            self.city_change_free_specialist_count(city, specialist, 1);
        }

        // Graphics: NotifyEntity(MISSION_JOIN).

        if self.units.contains(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::canConstruct` (CvUnit.cpp:6852-6904).
    pub fn unit_can_construct(
        &self,
        u: UnitId,
        plot: PlotIdx,
        building: Option<BuildingType>,
        test_visible: bool,
    ) -> bool {
        let Some(building) = building else {
            return false;
        };

        // FfH: Added by Kael 08/18/2008
        if self.units[u].has_casted {
            return false;
        }
        // FfH: End Add

        let Some(city) = live_plot_city(self, plot) else {
            return false;
        };

        if self.unit_get_team(u) != self.city_team(city) {
            return false;
        }

        if self.cities[city].num_real_building[building] > 0 {
            return false;
        }

        let info = &self.d.units[self.units[u].unit_type];
        if !info.force_buildings[building] {
            if !info.buildings[building] {
                return false;
            }

            if !self.city_can_construct(city, building, false, test_visible, true) {
                return false;
            }
        }

        if self.units[u].death_delay {
            return false;
        }

        true
    }

    /// C++ `CvUnit::construct` (CvUnit.cpp:6907-6933).
    pub fn unit_construct(&mut self, u: UnitId, building: Option<BuildingType>) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_construct(u, plot, building, false) {
            return false;
        }

        // `canConstruct` rejects NO_BUILDING.
        let building = building.expect("CvUnit::construct: no building");

        if let Some(city) = live_plot_city(self, plot) {
            let num = self.cities[city].num_real_building[building];
            self.city_set_num_real_building(city, building, num + 1);

            self.ev_building_built(city, building);
        }

        // Graphics: NotifyEntity(MISSION_CONSTRUCT).

        if self.units.contains(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::getDiscoveryTech` (CvUnit.cpp:6936-6939).
    /// C++ `::getDiscoveryTech(getUnitType(), getOwnerINLINE())` (CvGameCoreUtils).
    pub fn unit_get_discovery_tech(&self, u: UnitId) -> Option<TechType> {
        self.get_discovery_tech(self.units[u].unit_type, self.units[u].owner)
    }

    /// C++ `CvUnit::getDiscoverResearch` (CvUnit.cpp:6942-6964).
    pub fn unit_get_discover_research(&self, u: UnitId, tech: Option<TechType>) -> i32 {
        let info = &self.d.units[self.units[u].unit_type];
        let team = self.unit_get_team(u);

        let mut research = info.base_discover + info.discover_multiplier * self.team_get_total_population(team, true);

        research *= game_speed_info(self).unit_discover_percent;
        research /= 100;

        if let Some(tech) = tech {
            research = std::cmp::min(self.team_get_research_left(team, tech), research);
        }

        // FfH: Added by Kael 08/18/2008
        if self.units[u].has_casted {
            return 0;
        }
        // FfH: End Add

        std::cmp::max(0, research)
    }

    /// C++ `CvUnit::canDiscover` (CvUnit.cpp:6967-6989).
    pub fn unit_can_discover(&self, u: UnitId, plot: PlotIdx) -> bool {
        let Some(tech) = self.unit_get_discovery_tech(u) else {
            return false;
        };

        if self.unit_get_discover_research(u, Some(tech)) == 0 {
            return false;
        }

        if self.units[u].death_delay {
            return false;
        }

        true
    }

    /// C++ `CvUnit::discover` (CvUnit.cpp:6992-7014).
    pub fn unit_discover(&mut self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_discover(u, plot) {
            return false;
        }

        let tech = self.unit_get_discovery_tech(u).expect("DiscoveryTech is not assigned a valid value");

        let team = self.unit_get_team(u);
        let owner = self.units[u].owner;
        let research = self.unit_get_discover_research(u, Some(tech));
        self.team_change_research_progress(team, tech, research, owner);

        // Graphics: NotifyEntity(MISSION_DISCOVER).

        if self.units.contains(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::getMaxHurryProduction` (CvUnit.cpp:7017-7034).
    pub fn unit_get_max_hurry_production(&self, u: UnitId, city: CityId) -> i32 {
        let info = &self.d.units[self.units[u].unit_type];

        let mut production = info.base_hurry + info.hurry_multiplier * self.cities[city].population;

        production *= game_speed_info(self).unit_hurry_percent;
        production /= 100;

        // FfH: Added by Kael 08/18/2008
        if self.units[u].has_casted {
            return 0;
        }
        // FfH: End Add

        std::cmp::max(0, production)
    }

    /// C++ `CvUnit::getHurryProduction` (CvUnit.cpp:7037-7054).
    pub fn unit_get_hurry_production(&self, u: UnitId, plot: PlotIdx) -> i32 {
        let Some(city) = live_plot_city(self, plot) else {
            return 0;
        };

        let mut production = self.unit_get_max_hurry_production(u, city);

        production = std::cmp::min(self.city_production_left(city), production);

        std::cmp::max(0, production)
    }

    /// C++ `CvUnit::canHurry` (CvUnit.cpp:7057-7092).
    pub fn unit_can_hurry(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        if self.units[u].death_delay {
            return false;
        }

        if self.unit_get_hurry_production(u, plot) == 0 {
            return false;
        }

        let Some(city) = live_plot_city(self, plot) else {
            return false;
        };

        if self.city_production_turns_left(city) == 1 {
            return false;
        }

        if !test_visible {
            if !self.city_is_production_building(city) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::hurry` (CvUnit.cpp:7095-7119).
    pub fn unit_hurry(&mut self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_hurry(u, plot, false) {
            return false;
        }

        if let Some(city) = live_plot_city(self, plot) {
            let production = self.unit_get_hurry_production(u, plot);
            self.city_change_production(city, production);
        }

        // Graphics: NotifyEntity(MISSION_HURRY).

        if self.units.contains(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::getTradeGold` (CvUnit.cpp:7122-7149).
    pub fn unit_get_trade_gold(&self, u: UnitId, plot: PlotIdx) -> i32 {
        let city = live_plot_city(self, plot);
        let capital_city = self.player_get_capital_city(self.units[u].owner);

        let Some(city) = city else {
            return 0;
        };

        let info = &self.d.units[self.units[u].unit_type];
        let trade_profit = match capital_city {
            Some(capital_city) => self.city_calculate_trade_profit(city, capital_city),
            None => 0,
        };
        let mut gold = info.base_trade + info.trade_multiplier * trade_profit;

        gold *= game_speed_info(self).unit_trade_percent;
        gold /= 100;

        // FfH: Added by Kael 08/18/2008
        if self.units[u].has_casted {
            return 0;
        }
        // FfH: End Add

        std::cmp::max(0, gold)
    }

    /// C++ `CvUnit::canTrade` (CvUnit.cpp:7152-7185).
    pub fn unit_can_trade(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        if self.units[u].death_delay {
            return false;
        }

        let Some(city) = live_plot_city(self, plot) else {
            return false;
        };

        if self.unit_get_trade_gold(u, plot) == 0 {
            return false;
        }

        if !self.unit_can_enter_area(u, self.plot_get_team(plot), self.plot_area(plot), false) {
            return false;
        }

        if !test_visible {
            if self.city_team(city) == self.unit_get_team(u) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::trade` (CvUnit.cpp:7188-7205).
    pub fn unit_trade(&mut self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_trade(u, plot, false) {
            return false;
        }

        let owner = self.units[u].owner;
        let gold = self.unit_get_trade_gold(u, plot);
        self.players[owner].gold += gold;

        // Graphics: NotifyEntity(MISSION_TRADE).

        self.unit_kill(u, true, None);

        true
    }

    /// C++ `CvUnit::getGreatWorkCulture` (CvUnit.cpp:7208-7225).
    pub fn unit_get_great_work_culture(&self, u: UnitId, plot: PlotIdx) -> i32 {
        let mut culture = self.d.units[self.units[u].unit_type].great_work_culture;

        culture *= game_speed_info(self).unit_great_work_percent;
        culture /= 100;

        // FfH: Added by Kael 08/18/2008
        if self.units[u].has_casted {
            return 0;
        }
        // FfH: End Add

        std::cmp::max(0, culture)
    }

    /// C++ `CvUnit::canGreatWork` (CvUnit.cpp:7228-7253).
    pub fn unit_can_great_work(&self, u: UnitId, plot: PlotIdx) -> bool {
        if self.units[u].death_delay {
            return false;
        }

        let Some(city) = live_plot_city(self, plot) else {
            return false;
        };

        if self.cities[city].owner != self.units[u].owner {
            return false;
        }

        if self.unit_get_great_work_culture(u, plot) == 0 {
            return false;
        }

        true
    }

    /// C++ `CvUnit::greatWork` (CvUnit.cpp:7256-7292).
    pub fn unit_great_work(&mut self, u: UnitId) -> bool {
        let d = self.d;
        let plot = self.unit_plot(u);

        if !self.unit_can_great_work(u, plot) {
            return false;
        }

        if let Some(city) = live_plot_city(self, plot) {
            self.cities[city].culture_update_timer = 0;
            self.city_set_occupation_timer(city, 0);

            let owner = self.units[u].owner;
            let culture_to_add = 100 * self.unit_get_great_work_culture(u, plot);
            let num_turns_applied = (d.defines.great_works_culture_turns * game_speed_info(self).unit_great_work_percent) / 100;

            for _ in 0..num_turns_applied {
                self.city_change_culture_times100(city, owner, culture_to_add / num_turns_applied, true, true);
            }

            if num_turns_applied > 0 {
                self.city_change_culture_times100(city, owner, culture_to_add % num_turns_applied, false, true);
            }
        }

        // Graphics: NotifyEntity(MISSION_GREAT_WORK).

        if self.units.contains(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::getEspionagePoints` (CvUnit.cpp:7295-7305).
    /// `plot` is unused by the C++ (callers pass NULL).
    pub fn unit_get_espionage_points(&self, u: UnitId, plot: Option<PlotIdx>) -> i32 {
        let mut espionage_points = self.d.units[self.units[u].unit_type].espionage_points;

        // (C++ scales by the game speed's great work percent.)
        espionage_points *= game_speed_info(self).unit_great_work_percent;
        espionage_points /= 100;

        std::cmp::max(0, espionage_points)
    }

    /// C++ `CvUnit::canInfiltrate` (CvUnit.cpp:7307-7339).
    pub fn unit_can_infiltrate(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        if self.units[u].death_delay {
            return false;
        }

        if self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
            return false;
        }

        if self.unit_get_espionage_points(u, None) == 0 {
            return false;
        }

        let city = match live_plot_city(self, plot) {
            Some(city) if !self.city_is_barbarian(city) => city,
            _ => return false,
        };

        if !test_visible {
            if self.city_team(city) == self.unit_get_team(u) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::infiltrate` (CvUnit.cpp:7342-7361).
    pub fn unit_infiltrate(&mut self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_infiltrate(u, plot, false) {
            return false;
        }

        let points = self.unit_get_espionage_points(u, None);
        let team = self.unit_get_team(u);
        // C++ `GET_PLAYER(plot()->getOwnerINLINE())`: `canInfiltrate` found a city here.
        let plot_owner = self.map.plots[plot].owner.expect("CvUnit::infiltrate: city plot without an owner");
        let target_team = self.players[plot_owner].team;
        self.team_change_espionage_points_against_team(team, target_team, points);
        self.teams[team].espionage_points_ever += points;

        // Graphics: NotifyEntity(MISSION_INFILTRATE).

        if self.units.contains(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::canEspionage` (CvUnit.cpp:7364-7423).
    pub fn unit_can_espionage(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        if self.units[u].death_delay {
            return false;
        }

        if !self.unit_is_spy(u) {
            return false;
        }

        if self.is_option(GameOptionType::GAMEOPTION_NO_ESPIONAGE) {
            return false;
        }

        let Some(plot_owner) = self.map.plots[plot].owner else {
            return false;
        };

        if self.player_is_barbarian(plot_owner) {
            return false;
        }

        let target_team = self.players[plot_owner].team;
        let team = self.unit_get_team(u);

        if target_team == team {
            return false;
        }

        if self.teams[team].vassal[target_team] {
            return false;
        }

        if !test_visible {
            if self.units[u].made_attack {
                return false;
            }

            if self.unit_has_moved(u) {
                return false;
            }

            if target_team != team && !self.unit_is_invisible(u, target_team, true) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::espionage` (CvUnit.cpp:7425-7488).
    /// `mission` `None` (or a two-phase mission with `data == -1`) opens a popup for humans in C++
    /// and returns false; the chosen mission arrives as a new MISSION_ESPIONAGE.
    pub fn unit_espionage(&mut self, u: UnitId, mission: Option<EspionageMissionType>, data: i32) -> bool {
        let d = self.d;
        let plot = self.unit_plot(u);

        if !self.unit_can_espionage(u, plot, false) {
            return false;
        }

        // `canEspionage` requires an owned plot.
        let target_player = self.map.plots[plot].owner.expect("CvUnit::espionage: unowned plot");

        let Some(mission) = mission else {
            // UI: BUTTONPOPUP_DOESPIONAGE for the (human) owner; the chosen mission comes back as a
            // new MISSION_ESPIONAGE.
            return false;
        };

        if d.espionage_missions[mission].two_phases && data == -1 {
            // UI: BUTTONPOPUP_DOESPIONAGE_TARGET for the (human) owner.
            return false;
        }

        if self.unit_test_spy_intercepted(u, target_player, d.espionage_missions[mission].difficulty_mod) {
            return false;
        }

        let owner = self.units[u].owner;
        if self.player_do_espionage_mission(owner, mission, Some(target_player), Some(plot), data, Some(u)) {
            // Graphics: NotifyEntity(MISSION_ESPIONAGE).

            // C++ keeps using `this` (no espionage mission destroys the spy).
            if !self.units.contains(u) {
                return true;
            }

            if !self.unit_test_spy_intercepted(u, target_player, d.defines.espionage_spy_mission_escape_mod) {
                self.unit_set_fortify_turns(u, 0);
                self.units[u].made_attack = true;
                self.unit_finish_moves(u);

                if let Some(capital) = self.player_get_capital_city(owner) {
                    let (x, y) = (self.cities[capital].x, self.cities[capital].y);
                    self.unit_set_xy(u, x, y, false);

                    // TXT_KEY_ESPIONAGE_SPY_SUCCESS
                    log_message(self, Some(owner), |g| {
                        format!("{} has returned to {}", g.unit_get_name(u), city_display_name(g, capital))
                    });
                }
            }

            return true;
        }

        false
    }

    /// C++ `CvUnit::testSpyIntercepted` (CvUnit.cpp:7490-7556).
    /// Returns true if the spy was caught (and killed).
    pub fn unit_test_spy_intercepted(&mut self, u: UnitId, target_player: PlayerId, modifier: i32) -> bool {
        if self.player_is_barbarian(target_player) {
            return false;
        }

        let target_team = self.players[target_player].team;

        if self.soren_rand_num(10000, "Spy Interception") >= self.unit_get_spy_intercept_percent(u, target_team) * (100 + modifier) {
            return false;
        }

        // UI: TXT_KEY_SPY_INTERCEPTED[_MISSION|_SPY][_REVEAL] messages naming the target's
        // closest city (`findCity(.., eTargetPlayer, team, true, false)`) or the target civ.
        let owner = self.units[u].owner;
        log_message(self, Some(owner), |g| format!("Your {} has been intercepted!", g.unit_get_name(u)));

        if self.soren_rand_num(100, "Spy Reveal identity") < self.d.defines.espionage_spy_reveal_identity_percent {
            if !self.unit_is_enemy(u, Some(target_team), None) {
                self.players[target_player].ai_memory_count[owner][MemoryType::MEMORY_SPY_CAUGHT] += 1;
            }

            log_message(self, Some(target_player), |g| {
                format!("A {} {} has been intercepted", g.player_get_civilization_adjective(owner), g.unit_get_name(u))
            });
        } else {
            log_message(self, Some(target_player), |g| format!("A {} has been intercepted", g.unit_get_name(u)));
        }

        // Graphics: NotifyEntity(MISSION_SURRENDER).

        self.unit_kill(u, true, None);

        true
    }

    /// C++ `CvUnit::getSpyInterceptPercent` (CvUnit.cpp:7558-7585).
    pub fn unit_get_spy_intercept_percent(&self, u: UnitId, target_team: TeamId) -> i32 {
        let defines = &self.d.defines;
        let team = self.unit_get_team(u);
        let plot = self.unit_plot(u);

        let mut success = 0;

        let target_points = self.teams[target_team].espionage_points_ever;
        let our_points = self.teams[team].espionage_points_ever;
        success += (defines.espionage_intercept_spending_max * target_points) / std::cmp::max(1, target_points + our_points);

        if self.plot_is_espionage_counter_spy(plot, target_team) {
            success += defines.espionage_intercept_counterspy;
        }

        if self.teams[target_team].counterespionage_mod_against_team[team] > 0 {
            success += defines.espionage_intercept_counterespionage_mission;
        }

        if 0 == self.units[u].fortify_turns
            || self.plot_plot_count(plot, Some(Game::puf_is_spy), -1, -1, None, Some(team), None, -1, -1) > 1
        {
            success += defines.espionage_intercept_recent_mission;
        }

        std::cmp::min(100, std::cmp::max(0, success))
    }

    /// C++ `CvUnit::isIntruding` (CvUnit.cpp:7587-7602).
    pub fn unit_is_intruding(&self, u: UnitId) -> bool {
        let team = self.unit_get_team(u);

        let local_team = match self.plot_get_team(self.unit_plot(u)) {
            Some(local_team) if local_team != team => local_team,
            _ => return false,
        };

        if self.teams[local_team].vassal[team] {
            return false;
        }

        true
    }

    /// C++ `CvUnit::canGoldenAge` (CvUnit.cpp:7604-7620).
    pub fn unit_can_golden_age(&self, u: UnitId, plot: PlotIdx, test_visible: bool) -> bool {
        if !self.unit_is_golden_age(u) {
            return false;
        }

        if !test_visible {
            let owner = self.units[u].owner;
            if self.player_units_required_for_golden_age(owner) > self.player_units_golden_age_ready(owner) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::goldenAge` (CvUnit.cpp:7623-7643).
    pub fn unit_golden_age(&mut self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_golden_age(u, plot, false) {
            return false;
        }

        let owner = self.units[u].owner;

        self.player_kill_golden_age_units(owner, Some(u));

        let length = self.player_get_golden_age_length(owner);
        self.player_change_golden_age_turns(owner, length);
        self.players[owner].num_unit_golden_ages += 1;

        // Graphics: NotifyEntity(MISSION_GOLDEN_AGE).

        // `killGoldenAgeUnits` spares `pUnitAlive` (this unit).
        if self.units.contains(u) {
            self.unit_kill(u, true, None);
        }

        true
    }

    /// C++ `CvUnit::canBuild` (CvUnit.cpp:7646-7679).
    pub fn unit_can_build(&self, u: UnitId, plot: PlotIdx, build: BuildType, test_visible: bool) -> bool {
        if !self.d.units[self.units[u].unit_type].builds[build] {
            return false;
        }

        if !self.player_can_build(self.units[u].owner, plot, build, false, test_visible) {
            return false;
        }

        if !self.plot_is_valid_domain_for_action(plot, u) {
            return false;
        }

        // ADDON (stop automated workers from building forts) Sephi
        if self.unit_is_automated(u) {
            if build.i() == self.d.defines.build_fort {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::build` (CvUnit.cpp:7682-7714).
    /// Returns true if the build finished.
    pub fn unit_build(&mut self, u: UnitId, build: BuildType) -> bool {
        let plot = self.unit_plot(u);

        if !self.unit_can_build(u, plot, build, false) {
            return false;
        }

        // Graphics: NotifyEntity(build mission type), before changeBuildProgress.

        let owner = self.units[u].owner;
        let cost = self.player_get_build_cost(owner, plot, build);
        self.players[owner].gold -= cost;

        let team = self.unit_get_team(u);
        let work_rate = self.unit_work_rate(u, false);
        let finished = self.plot_change_build_progress(plot, build, work_rate, team);

        // C++ keeps using `this` (finishing a build never destroys the worker).
        if !self.units.contains(u) {
            return finished;
        }

        self.unit_finish_moves(u); // needs to be at bottom because movesLeft() can affect workRate()...

        if finished {
            if self.d.builds[build].kill {
                self.unit_kill(u, true, None);
            }
        }

        // Python Event
        self.ev_unit_build_improvement(u, build, finished);

        finished
    }

    /// C++ `CvUnit::canBuildRoute` (CvUnit.cpp:8679-8698).
    pub fn unit_can_build_route(&self, u: UnitId) -> bool {
        let d = self.d;
        let info = &d.units[self.units[u].unit_type];
        let team = self.unit_get_team(u);

        for build in d.builds.keys() {
            if d.builds[build].route.is_some() {
                if info.builds[build] {
                    if self.team_is_has_tech(team, d.builds[build].tech_prereq) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// C++ `CvUnit::getBuildType` (CvUnit.cpp:8700-8765).
    /// The build of the group's head mission (MISSION_BUILD, or MISSION_ROUTE_TO's best route build).
    pub fn unit_get_build_type(&self, u: UnitId) -> Option<BuildType> {
        // C++ dereferences `getGroup()` (every unit on the map has a group).
        let group = self.units[u].group.filter(|&g| self.groups.contains(g))?;

        if let Some(mission) = self.groups[group].mission_queue.first() {
            match mission.mission_type {
                MissionType::MISSION_ROUTE_TO => {
                    let (route, build) = self.group_get_best_build_route(group, self.unit_plot(u));
                    if route.is_some() {
                        return build;
                    }
                }

                MissionType::MISSION_BUILD => return opt_from_i32(mission.data1),

                // MISSION_MOVE_TO, MISSION_MOVE_TO_UNIT, MISSION_SKIP, ... MISSION_DIE_ANIMATION:
                // no build (C++ asserts on the remaining mission types).
                _ => {}
            }
        }

        None
    }

    /// C++ `CvUnit::workRate` (CvUnit.cpp:8768-8796).
    pub fn unit_work_rate(&self, u: UnitId, max: bool) -> i32 {
        if !max {
            if !self.unit_can_move(u) {
                return 0;
            }
        }

        let unit = &self.units[u];
        let mut rate = self.d.units[unit.unit_type].work_rate;

        // FfH: Added by Kael 08/13/2008
        rate += unit.work_rate_modify;
        // FfH: End Add

        rate *= std::cmp::max(0, self.players[unit.owner].worker_speed_modifier + 100);
        rate /= 100;

        if !self.unit_is_human(u) && !self.unit_is_barbarian(u) {
            rate *= std::cmp::max(0, self.d.handicaps[self.handicap].ai_work_rate_modifier + 100);
            rate /= 100;
        }

        rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::SelectionGroup;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;
    use crate::structs::MissionData;

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

    /// C++ `CvPlayer::addUnit` + `reset`, with the coordinates set directly (no `init`/`setXY`,
    /// so the unit is not in its plot's unit list).
    fn add_unit(g: &mut Game, owner: PlayerId, unit_type: &str, x: i32, y: i32) -> UnitId {
        let d = g.d;
        let t = d.t::<UnitType>(unit_type);
        let id = g.units.next_id();
        let u = g.player_add_unit(owner, Unit::new(id, t, owner, d, &UnitResetInfo::from_info(&d.units[t])));
        g.units[u].x = x;
        g.units[u].y = y;
        u
    }

    #[test]
    fn spy_probabilities_on_empty_plots() {
        let mut g = test_game();
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);
        let plot = g.map.plot_idx(2, 1).unwrap();
        let (low, real, high) = (ProbabilityType::PROBABILITY_LOW, ProbabilityType::PROBABILITY_REAL, ProbabilityType::PROBABILITY_HIGH);

        // Unowned plot: no defenders or counter spies are counted.
        assert_eq!(g.unit_sabotage_prob(u, plot, low), 40);
        assert_eq!(g.unit_sabotage_prob(u, plot, real), 40 + 50);
        assert_eq!(g.unit_sabotage_prob(u, plot, high), 40 + 50);

        // Owned plot without units: same counts.
        g.map.plots[plot].owner = Some(PlayerId(1));
        assert_eq!(g.unit_sabotage_prob(u, plot, real), 90);

        // Destroy / steal plans need a city on the plot.
        assert_eq!(g.unit_destroy_prob(u, plot, real), 0);
        assert_eq!(g.unit_steal_plans_prob(u, plot, real), 0);
        assert_eq!(g.unit_destroy_cost(u, plot), 0);
        assert_eq!(g.unit_steal_plans_cost(u, plot), 0);
        assert_eq!(g.unit_sabotage_cost(u, plot), g.d.defines.base_spy_sabotage_cost);

        // A warrior is no spy.
        assert!(!g.unit_can_sabotage(u, plot, true));
        assert!(!g.unit_can_destroy(u, plot, true));
        assert!(!g.unit_can_steal_plans(u, plot, true));
    }

    #[test]
    fn great_work_and_espionage_scale_with_game_speed() {
        let mut g = test_game();
        let d = g.d;
        let u = add_unit(&mut g, PlayerId(0), "UNIT_ARTIST", 1, 1);
        let plot = g.unit_plot(u);
        let info = &d.units[g.units[u].unit_type];
        assert!(info.great_work_culture > 0);

        g.setup.game_speed = d.t::<GameSpeedType>("GAMESPEED_QUICK");
        let percent = d.game_speeds[g.setup.game_speed].unit_great_work_percent;
        assert_eq!(percent, 67);
        assert_eq!(g.unit_get_great_work_culture(u, plot), info.great_work_culture * percent / 100);
        assert_eq!(g.unit_get_espionage_points(u, None), std::cmp::max(0, info.espionage_points * percent / 100));

        // FfH: a unit that has cast this turn has no great work.
        g.units[u].has_casted = true;
        assert_eq!(g.unit_get_great_work_culture(u, plot), 0);

        // Without a city there is nothing to hurry, trade with or work on.
        g.units[u].has_casted = false;
        assert_eq!(g.unit_get_hurry_production(u, plot), 0);
        assert_eq!(g.unit_get_trade_gold(u, plot), 0);
        assert!(!g.unit_can_great_work(u, plot));
        assert!(!g.unit_can_hurry(u, plot, false));
        assert!(!g.unit_can_trade(u, plot, false));
        assert!(!g.unit_can_join(u, plot, None));
        assert!(!g.unit_can_construct(u, plot, None, false));

        // FfH always plays without espionage.
        assert!(!g.unit_can_infiltrate(u, plot, true));
    }

    #[test]
    fn discover_research_without_tech_target() {
        let mut g = test_game();
        let d = g.d;
        let u = add_unit(&mut g, PlayerId(0), "UNIT_ARTIST", 1, 1);
        let info = &d.units[g.units[u].unit_type];
        let team = g.unit_get_team(u);
        g.teams[team].total_population = 7;

        g.setup.game_speed = d.t::<GameSpeedType>("GAMESPEED_EPIC");
        let percent = d.game_speeds[g.setup.game_speed].unit_discover_percent;
        let expected = (info.base_discover + info.discover_multiplier * 7) * percent / 100;
        assert_eq!(g.unit_get_discover_research(u, None), std::cmp::max(0, expected));

        g.units[u].has_casted = true;
        assert_eq!(g.unit_get_discover_research(u, None), 0);
    }

    #[test]
    fn intruding_needs_foreign_non_vassal_territory() {
        let mut g = test_game();
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let u = add_unit(&mut g, p0, "UNIT_WARRIOR", 2, 2);
        let plot = g.unit_plot(u);
        let (t0, t1) = (g.players[p0].team, g.players[p1].team);

        assert!(!g.unit_is_intruding(u));
        g.map.plots[plot].owner = Some(p0);
        assert!(!g.unit_is_intruding(u));
        g.map.plots[plot].owner = Some(p1);
        assert!(g.unit_is_intruding(u));
        g.teams[t1].vassal[t0] = true;
        assert!(!g.unit_is_intruding(u));
    }

    #[test]
    fn spy_intercept_percent() {
        let mut g = test_game();
        let d = g.d;
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let u = add_unit(&mut g, p0, "UNIT_WARRIOR", 2, 2);
        let (t0, t1) = (g.players[p0].team, g.players[p1].team);
        let defines = &d.defines;

        // No espionage spent, recent mission (not fortified).
        assert_eq!(g.unit_get_spy_intercept_percent(u, t1), defines.espionage_intercept_recent_mission);

        g.teams[t1].espionage_points_ever = 300;
        g.teams[t0].espionage_points_ever = 100;
        let spending = defines.espionage_intercept_spending_max * 300 / 400;
        assert_eq!(g.unit_get_spy_intercept_percent(u, t1), spending + defines.espionage_intercept_recent_mission);

        // Fortified and alone: no recent mission bonus; counterespionage adds its own.
        g.units[u].fortify_turns = 1;
        g.teams[t1].counterespionage_mod_against_team[t0] = 10;
        assert_eq!(g.unit_get_spy_intercept_percent(u, t1), spending + defines.espionage_intercept_counterespionage_mission);
    }

    #[test]
    fn build_type_follows_head_mission() {
        let mut g = test_game();
        let d = g.d;
        let owner = PlayerId(0);
        let u = add_unit(&mut g, owner, "UNIT_WORKER", 1, 1);

        // No group, or no mission: no build.
        assert_eq!(g.unit_get_build_type(u), None);
        let id = g.groups.next_id();
        let group = g.player_add_selection_group(owner, SelectionGroup::new(id, owner));
        g.units[u].group = Some(group);
        g.groups[group].units.push(u);
        assert_eq!(g.unit_get_build_type(u), None);

        let road = d.t::<BuildType>("BUILD_ROAD");
        let mission = |mission_type: MissionType, data1: i32| MissionData { mission_type, data1, data2: -1, flags: 0, push_turn: 0 };
        g.groups[group].mission_queue.push(mission(MissionType::MISSION_BUILD, road.i()));
        assert_eq!(g.unit_get_build_type(u), Some(road));

        g.groups[group].mission_queue.insert(0, mission(MissionType::MISSION_MOVE_TO, 3));
        assert_eq!(g.unit_get_build_type(u), None);
    }

    #[test]
    fn can_build_route_needs_the_build_tech() {
        let mut g = test_game();
        let d = g.d;
        let u = add_unit(&mut g, PlayerId(0), "UNIT_WORKER", 1, 1);
        let team = g.unit_get_team(u);
        let info = &d.units[g.units[u].unit_type];

        let route_builds: Vec<BuildType> = d.builds.keys().filter(|&b| d.builds[b].route.is_some() && info.builds[b]).collect();
        assert!(!route_builds.is_empty());
        let free = route_builds.iter().any(|&b| d.builds[b].tech_prereq.is_none());
        assert_eq!(g.unit_can_build_route(u), free);

        for &b in &route_builds {
            if let Some(tech) = d.builds[b].tech_prereq {
                g.teams[team].has_tech[tech] = true;
            }
        }
        assert!(g.unit_can_build_route(u));

        // A warrior builds nothing.
        let warrior = add_unit(&mut g, PlayerId(0), "UNIT_WARRIOR", 1, 1);
        assert!(!g.unit_can_build_route(warrior));
    }
}
