//! CvPlot, topic "units": the plot's unit list, unit queries (`plotCount`/`plotCheck` with
//! `PUF_*` predicates), defenders, movement cost, defense modifiers, nukes, sight (line of
//! sight and sight sources), plot events and unit training requirements.

use super::{ConstPlotUnitFunc, PlotUnitFunc};
use crate::game::Game;
use crate::ids::*;
use crate::utils::{final_improvement_upgrade, get_sign};
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Unit list (`getNumUnits`, `getUnitByIndex`, `headUnitNode`, ... are `units` field ops)
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isUnit`.
    #[inline]
    pub fn plot_is_unit(&self, p: PlotIdx) -> bool {
        !self.map.plots[p].units.is_empty()
    }

    /// C++ `CvPlot::addUnit` (`bUpdate` dropped: center unit / flag refresh only). Inserts the
    /// unit before the first unit it sorts before (`isBeforeUnitCycle`).
    pub fn plot_add_unit(&mut self, p: PlotIdx, unit: UnitId) {
        // (C++ FAssertMsg: the unit is expected to be at this plot.)
        let mut insert_at = None;
        for (i, &loop_unit) in self.map.plots[p].units.iter().enumerate() {
            if !self.is_before_unit_cycle(loop_unit, unit) {
                insert_at = Some(i);
                break;
            }
        }

        let units = &mut self.map.plots[p].units;
        match insert_at {
            Some(i) => units.insert(i, unit),
            None => units.push(unit),
        }

        // (bUpdate: updateCenterUnit, setFlagDirty -- graphics)
    }

    /// C++ `CvPlot::removeUnit` (`bUpdate` dropped: center unit / flag refresh only).
    pub fn plot_remove_unit(&mut self, p: PlotIdx, unit: UnitId) {
        let units = &mut self.map.plots[p].units;
        if let Some(i) = units.iter().position(|&u| u == unit) {
            units.remove(i);
        }
    }

    /// C++ `CvPlot::verifyUnitValidPlot`: bumps units that can no longer stay here.
    pub fn plot_verify_unit_valid_plot(&mut self, p: PlotIdx) {
        // C++ copies the unit pointers first (`aUnits`); the copy only drops units that
        // `jumpToNearestValidPlot` killed. A unit destroyed some other way would leave a
        // dangling pointer in C++; here it is skipped (`self.units.contains`).
        let mut units: Vec<UnitId> = self.map.plots[p].units.iter().copied().filter(|&u| self.units.contains(u)).collect();

        let mut i = 0;
        while i < units.len() {
            let loop_unit = units[i];
            let mut erased = false;

            if self.units.contains(loop_unit) {
                if self.unit_at_plot(loop_unit, p) {
                    if !self.unit_is_cargo(loop_unit) {
                        if !self.unit_is_combat(loop_unit) {
                            if !self.plot_is_valid_domain_for_location(p, loop_unit)
                                || !self.unit_can_enter_area(loop_unit, self.plot_get_team(p), self.map.plots[p].area, false)
                            {
                                if !self.unit_jump_to_nearest_valid_plot(loop_unit) {
                                    erased = true;
                                }
                            }
                        }
                    }
                }
            }

            if erased {
                units.remove(i);
            } else {
                i += 1;
            }
        }

        if self.plot_is_owned(p) {
            let mut i = 0;
            while i < units.len() {
                let loop_unit = units[i];
                let mut erased = false;

                if self.units.contains(loop_unit) {
                    if self.unit_at_plot(loop_unit, p) {
                        if !self.unit_is_combat(loop_unit) {
                            let plot_team = self.plot_get_team(p);
                            let unit_team = self.unit_get_team(loop_unit);
                            if Some(unit_team) != plot_team
                                && plot_team.map_or(true, |t| !self.teams[t].vassal[unit_team])
                            {
                                if self.plot_is_visible_enemy_unit_for_unit(p, loop_unit) {
                                    // `isOwned()` holds, so the plot has a team.
                                    let plot_team = plot_team.expect("CvPlot::verifyUnitValidPlot: owned plot without a team");
                                    if !self.unit_is_invisible(loop_unit, plot_team, true) {
                                        if !self.unit_jump_to_nearest_valid_plot(loop_unit) {
                                            erased = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if erased {
                    units.remove(i);
                } else {
                    i += 1;
                }
            }
        }
    }

    /// C++ `CvPlot::plotAction` (C++ defaults: data -1, owner/team none).
    pub fn plot_plot_action(&mut self, p: PlotIdx, func: PlotUnitFunc, data1: i32, data2: i32, owner: Option<PlayerId>, team: Option<TeamId>) {
        // C++ walks the live list, fetching the next node before calling `func`; walk a copy
        // and skip units that no longer exist.
        for loop_unit in self.map.plots[p].units.clone() {
            if !self.units.contains(loop_unit) {
                continue;
            }

            if owner.map_or(true, |o| self.units[loop_unit].owner == o) {
                if team.map_or(true, |t| self.unit_get_team(loop_unit) == t) {
                    func(self, loop_unit, data1, data2);
                }
            }
        }
    }

    /// C++ `CvPlot::plotCount`: units of `owner`/`team` passing `func_a` and `func_b`
    /// (a None predicate always passes).
    #[allow(clippy::too_many_arguments)]
    pub fn plot_plot_count(
        &self,
        p: PlotIdx,
        func_a: Option<ConstPlotUnitFunc>,
        data1_a: i32,
        data2_a: i32,
        owner: Option<PlayerId>,
        team: Option<TeamId>,
        func_b: Option<ConstPlotUnitFunc>,
        data1_b: i32,
        data2_b: i32,
    ) -> i32 {
        let mut count = 0;

        for &loop_unit in &self.map.plots[p].units {
            if owner.map_or(true, |o| self.units[loop_unit].owner == o) {
                if team.map_or(true, |t| self.unit_get_team(loop_unit) == t) {
                    if func_a.map_or(true, |f| f(self, loop_unit, data1_a, data2_a)) {
                        if func_b.map_or(true, |f| f(self, loop_unit, data1_b, data2_b)) {
                            count += 1;
                        }
                    }
                }
            }
        }

        count
    }

    /// C++ `CvPlot::plotCheck`: first unit of `owner`/`team` passing `func_a` and `func_b`
    /// (`func_a` is required, a None `func_b` always passes).
    #[allow(clippy::too_many_arguments)]
    pub fn plot_plot_check(
        &self,
        p: PlotIdx,
        func_a: ConstPlotUnitFunc,
        data1_a: i32,
        data2_a: i32,
        owner: Option<PlayerId>,
        team: Option<TeamId>,
        func_b: Option<ConstPlotUnitFunc>,
        data1_b: i32,
        data2_b: i32,
    ) -> Option<UnitId> {
        for &loop_unit in &self.map.plots[p].units {
            if owner.map_or(true, |o| self.units[loop_unit].owner == o) {
                if team.map_or(true, |t| self.unit_get_team(loop_unit) == t) {
                    if func_a(self, loop_unit, data1_a, data2_a) {
                        if func_b.map_or(true, |f| f(self, loop_unit, data1_b, data2_b)) {
                            return Some(loop_unit);
                        }
                    }
                }
            }
        }

        None
    }

    /// C++ `CvPlot::getNumAnimalUnits` (FFH).
    pub fn plot_get_num_animal_units(&self, p: PlotIdx) -> i32 {
        let mut count = 0;
        for &loop_unit in &self.map.plots[p].units {
            if self.unit_is_animal(loop_unit) {
                count += 1;
            }
        }
        count
    }

    /// C++ `CvPlot::getUnitPower` (C++ default `eOwner = NO_PLAYER`).
    pub fn plot_get_unit_power(&self, p: PlotIdx, owner: Option<PlayerId>) -> i32 {
        let mut count = 0;
        for &loop_unit in &self.map.plots[p].units {
            let unit = &self.units[loop_unit];
            if owner.map_or(true, |o| unit.owner == o) {
                count += self.d.units[unit.unit_type].power_value;
            }
        }
        count
    }

    // ------------------------------------------------------------------
    // Defenders and enemies
    // ------------------------------------------------------------------

    /// C++ `CvPlot::getBestDefender` (C++ defaults: no attacking player/attacker, all tests off).
    #[allow(clippy::too_many_arguments)]
    pub fn plot_get_best_defender(
        &self,
        p: PlotIdx,
        owner: Option<PlayerId>,
        attacking_player: Option<PlayerId>,
        attacker: Option<UnitId>,
        test_at_war: bool,
        test_potential_enemy: bool,
        test_can_move: bool,
    ) -> Option<UnitId> {
        let units = &self.map.plots[p].units;
        let mut best_unit: Option<UnitId> = None;

        //FfH: Added by Kael 10/28/2008
        let mut no_guard = true;
        for &loop_unit in units {
            if self.unit_is_target_weakest_unit_counter(loop_unit) {
                no_guard = false;
            }
        }
        //FfH: End Add

        let attacking_team = attacking_player.map(|ap| self.players[ap].team);

        for &loop_unit in units {
            let loop_owner = self.units[loop_unit].owner;

            if owner.map_or(true, |o| loop_owner == o) {
                if attacking_team.map_or(true, |at| !self.unit_is_invisible(loop_unit, at, true)) {
                    if !test_at_war
                        || attacking_team.is_none()
                        || self.unit_is_enemy(loop_unit, attacking_team, Some(p))
                        || attacker.is_some_and(|a| self.unit_is_enemy(a, Some(self.players[loop_owner].team), Some(p)))
                    {
                        if !test_potential_enemy
                            || attacking_team.is_none()
                            || self.unit_is_potential_enemy(loop_unit, attacking_team, Some(p))
                            || attacker.is_some_and(|a| self.unit_is_potential_enemy(a, Some(self.players[loop_owner].team), Some(p)))
                        {
                            if !test_can_move || (self.unit_can_move(loop_unit) && !self.unit_is_cargo(loop_unit)) {
                                if attacker.map_or(true, |a| {
                                    self.unit_get_domain_type(a) != DomainType::DOMAIN_AIR
                                        || self.units[loop_unit].damage < self.unit_air_combat_limit(a)
                                }) {
                                    //FfH: Modified by Kael 10/28/2008
                                    let targets_weakest = match attacker {
                                        Some(a) => {
                                            self.unit_is_target_weakest_unit(a)
                                                && self.unit_get_domain_type(a) == self.unit_get_domain_type(loop_unit)
                                                && self.unit_base_combat_str_defense(loop_unit) != 0
                                                && no_guard
                                        }
                                        None => false,
                                    };
                                    if let (true, Some(a)) = (targets_weakest, attacker) {
                                        if let Some(best) = best_unit {
                                            if self.unit_get_domain_type(a) != self.unit_get_domain_type(best)
                                                || self.unit_base_combat_str_defense(best) == 0
                                            {
                                                best_unit = None;
                                            }
                                        }
                                        if best_unit.map_or(true, |best| self.unit_is_better_defender_than(best, Some(loop_unit), attacker)) {
                                            best_unit = Some(loop_unit);
                                        }
                                    } else if self.unit_is_better_defender_than(loop_unit, best_unit, attacker)
                                        && attacker.map_or(true, |a| self.units[a].owner != loop_owner)
                                    {
                                        best_unit = Some(loop_unit);
                                    }
                                    //FfH: End Modify
                                }
                            }
                        }
                    }
                }
            }
        }

        best_unit
    }

    /// C++ `CvPlot::isInvestigate`.
    pub fn plot_is_investigate(&self, p: PlotIdx, team: TeamId) -> bool {
        self.plot_plot_check(p, Game::puf_is_investigate, -1, -1, None, Some(team), None, -1, -1).is_some()
    }

    /// C++ `CvPlot::isVisibleEnemyDefender`.
    pub fn plot_is_visible_enemy_defender(&self, p: PlotIdx, unit: UnitId) -> bool {
        let owner = self.units[unit].owner.i();
        let always_hostile = self.unit_is_always_hostile(unit, Some(p)) as i32;
        self.plot_plot_check(p, Game::puf_can_defend_enemy, owner, always_hostile, None, None, Some(Game::puf_is_visible), owner, -1)
            .is_some()
    }

    /// C++ `CvPlot::getVisibleEnemyDefender`.
    pub fn plot_get_visible_enemy_defender(&self, p: PlotIdx, player: PlayerId) -> Option<UnitId> {
        self.plot_plot_check(p, Game::puf_can_defend_enemy, player.i(), false as i32, None, None, Some(Game::puf_is_visible), player.i(), -1)
    }

    /// C++ `CvPlot::getNumDefenders`.
    pub fn plot_get_num_defenders(&self, p: PlotIdx, player: PlayerId) -> i32 {
        self.plot_plot_count(p, Some(Game::puf_can_defend), -1, -1, Some(player), None, None, -1, -1)
    }

    /// C++ `CvPlot::getNumVisibleEnemyDefenders`.
    pub fn plot_get_num_visible_enemy_defenders(&self, p: PlotIdx, unit: UnitId) -> i32 {
        let owner = self.units[unit].owner.i();
        let always_hostile = self.unit_is_always_hostile(unit, Some(p)) as i32;
        self.plot_plot_count(p, Some(Game::puf_can_defend_enemy), owner, always_hostile, None, None, Some(Game::puf_is_visible), owner, -1)
    }

    /// C++ `CvPlot::getNumVisiblePotentialEnemyDefenders`.
    pub fn plot_get_num_visible_potential_enemy_defenders(&self, p: PlotIdx, unit: UnitId) -> i32 {
        let owner = self.units[unit].owner.i();
        let always_hostile = self.unit_is_always_hostile(unit, Some(p)) as i32;
        self.plot_plot_count(
            p,
            Some(Game::puf_can_defend_potential_enemy),
            owner,
            always_hostile,
            None,
            None,
            Some(Game::puf_is_visible),
            owner,
            -1,
        )
    }

    /// C++ `CvPlot::isVisibleEnemyUnit(PlayerTypes)`.
    pub fn plot_is_visible_enemy_unit(&self, p: PlotIdx, player: PlayerId) -> bool {
        self.plot_plot_check(p, Game::puf_is_enemy, player.i(), false as i32, None, None, Some(Game::puf_is_visible), player.i(), -1)
            .is_some()
    }

    /// C++ `CvPlot::isVisibleEnemyUnit(const CvUnit*)`.
    pub fn plot_is_visible_enemy_unit_for_unit(&self, p: PlotIdx, unit: UnitId) -> bool {
        let owner = self.units[unit].owner.i();
        let always_hostile = self.unit_is_always_hostile(unit, Some(p)) as i32;
        self.plot_plot_check(p, Game::puf_is_enemy, owner, always_hostile, None, None, Some(Game::puf_is_visible), owner, -1).is_some()
    }

    /// C++ `CvPlot::getNumVisibleUnits`.
    pub fn plot_get_num_visible_units(&self, p: PlotIdx, player: PlayerId) -> i32 {
        self.plot_plot_count(p, Some(Game::puf_is_visible_debug), player.i(), -1, None, None, None, -1, -1)
    }

    /// C++ `CvPlot::isVisibleOtherUnit`.
    pub fn plot_is_visible_other_unit(&self, p: PlotIdx, player: PlayerId) -> bool {
        self.plot_plot_check(p, Game::puf_is_other_team, player.i(), -1, None, None, Some(Game::puf_is_visible), player.i(), -1)
            .is_some()
    }

    /// C++ `CvPlot::isFighting`.
    pub fn plot_is_fighting(&self, p: PlotIdx) -> bool {
        self.plot_plot_check(p, Game::puf_is_fighting, -1, -1, None, None, None, -1, -1).is_some()
    }

    /// C++ `CvPlot::isFriendlyCity`.
    pub fn plot_is_friendly_city(&self, p: PlotIdx, unit: UnitId, check_improvement: bool) -> bool {
        if !self.plot_is_city(p, check_improvement, Some(self.unit_get_team(unit))) {
            return false;
        }

        if self.plot_is_visible_enemy_unit_for_unit(p, unit) {
            return false;
        }

        if let Some(plot_team) = self.plot_get_team(p) {
            if self.unit_is_enemy(unit, Some(plot_team), None) {
                return false;
            }

            let team = self.players[self.unit_get_combat_owner(unit, Some(plot_team), Some(p))].team;

            if team == plot_team {
                return true;
            }

            if self.team_is_open_borders(team, plot_team) {
                return true;
            }

            if self.teams[plot_team].vassal[team] {
                return true;
            }
        }

        false
    }

    /// C++ `CvPlot::isEnemyCity`.
    pub fn plot_is_enemy_city(&self, p: PlotIdx, unit: UnitId) -> bool {
        if let Some(city) = self.map.plots[p].plot_city {
            return self.unit_is_enemy(unit, Some(self.city_team(city)), Some(p));
        }

        false
    }

    // ------------------------------------------------------------------
    // Domains, movement, defense
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isValidDomainForLocation`.
    pub fn plot_is_valid_domain_for_location(&self, p: PlotIdx, unit: UnitId) -> bool {
        if self.plot_is_valid_domain_for_action(p, unit) {
            return true;
        }

        self.plot_is_city(p, true, Some(self.unit_get_team(unit)))
    }

    /// C++ `CvPlot::isValidDomainForAction`.
    pub fn plot_is_valid_domain_for_action(&self, p: PlotIdx, unit: UnitId) -> bool {
        match self.unit_get_domain_type(unit) {
            DomainType::DOMAIN_SEA => self.plot_is_water(p) || self.unit_can_move_all_terrain(unit),
            DomainType::DOMAIN_AIR => false,
            DomainType::DOMAIN_LAND | DomainType::DOMAIN_IMMOBILE => !self.plot_is_water(p) || self.unit_can_move_all_terrain(unit),
            // (C++ FAssert(false))
            _ => false,
        }
    }

    /// C++ `CvPlot::isValidRoute`.
    pub fn plot_is_valid_route(&self, p: PlotIdx, unit: UnitId) -> bool {
        if self.plot_is_route(p) {
            if !self.unit_is_enemy(unit, self.plot_get_team(p), Some(p)) || self.unit_is_enemy_route(unit) {
                return true;
            }
        }

        false
    }

    /// C++ `CvPlot::movementCost`: cost for `unit` to enter this plot from `from_plot`.
    pub fn plot_movement_cost(&self, p: PlotIdx, unit: UnitId, from_plot: PlotIdx) -> i32 {
        let d = self.d;
        let move_denominator = d.defines.move_denominator;

        if self.unit_flat_movement_cost(unit) || self.unit_get_domain_type(unit) == DomainType::DOMAIN_AIR {
            return move_denominator;
        }

        if self.unit_is_human(unit) {
            if !self.map.plots[p].revealed[self.unit_get_team(unit)] {
                return self.unit_max_moves(unit);
            }
        }

        if !self.plot_is_valid_domain_for_location(from_plot, unit) {
            return self.unit_max_moves(unit);
        }

        if !self.plot_is_valid_domain_for_action(p, unit) {
            return move_denominator;
        }

        // (C++ FAssert: the unit is not DOMAIN_IMMOBILE.)

        let terrain = assigned_terrain(self, p);
        let feature = self.map.plots[p].feature_type;
        let hills = self.plot_is_hills(p);

        let mut regular_cost;
        if self.unit_ignore_terrain_cost(unit) {
            regular_cost = 1;
        } else {
            regular_cost = match feature {
                None => d.terrains[terrain].movement_cost,
                Some(f) => d.features[f].movement_cost,
            };

            if hills {
                regular_cost += d.defines.hills_extra_movement;
            }

            if regular_cost > 0 {
                regular_cost = (regular_cost - self.units[unit].extra_move_discount).max(1);
            }
        }

        let has_terrain_cost = regular_cost > 1;

        regular_cost = regular_cost.min(self.unit_base_moves(unit));

        regular_cost *= move_denominator;

        if has_terrain_cost {
            let double_move = match feature {
                None => self.unit_is_terrain_double_move(unit, terrain),
                Some(f) => self.unit_is_feature_double_move(unit, f),
            };
            if double_move || (hills && self.unit_is_hills_double_move(unit)) {
                regular_cost /= 2;
            }
        }

        let route_cost;
        let route_flat_cost;
        let unit_team = self.unit_get_team(unit);
        let valid_routes = self.plot_is_valid_route(from_plot, unit)
            && self.plot_is_valid_route(p, unit)
            && (self.team_is_bridge_building(unit_team)
                || !self.plot_is_river_crossing(from_plot, self.direction_xy_plots(from_plot, p)));
        match (valid_routes, self.map.plots[from_plot].route_type, self.map.plots[p].route_type) {
            // (`isValidRoute` implies `isRoute`: both plots have a route.)
            (true, Some(from_route), Some(route)) => {
                let team = &self.teams[unit_team];
                route_cost = (d.routes[from_route].movement_cost + team.route_change[from_route])
                    .max(d.routes[route].movement_cost + team.route_change[route]);
                let base_moves = self.unit_base_moves(unit);
                route_flat_cost =
                    (d.routes[from_route].flat_movement_cost * base_moves).max(d.routes[route].flat_movement_cost * base_moves);
            }
            _ => {
                route_cost = i32::MAX;
                route_flat_cost = i32::MAX;
            }
        }

        regular_cost.min(route_cost.min(route_flat_cost)).max(1)
    }

    /// C++ `CvPlot::getExtraMovePathCost` (`CvGame::getPlotExtraCost` at this plot).
    pub fn plot_get_extra_move_path_cost(&self, p: PlotIdx) -> i32 {
        let (x, y) = self.map.xy(p);
        self.plot_extra_cost(x, y)
    }

    /// C++ `CvPlot::changeExtraMovePathCost` (`CvGame::changePlotExtraCost` at this plot).
    pub fn plot_change_extra_move_path_cost(&mut self, p: PlotIdx, change: i32) {
        let (x, y) = self.map.xy(p);
        self.change_plot_extra_cost(x, y, change);
    }

    /// C++ `CvPlot::defenseModifier` (`bHelp` dropped: it reads the active team's revealed
    /// improvement for help text).
    pub fn plot_defense_modifier(&self, p: PlotIdx, defender: Option<TeamId>, ignore_building: bool) -> i32 {
        let d = self.d;
        let plot = &self.map.plots[p];

        let mut modifier = match plot.feature_type {
            None => d.terrains[assigned_terrain(self, p)].defense_modifier,
            Some(f) => d.features[f].defense_modifier,
        };

        if self.plot_is_hills(p) {
            modifier += d.defines.hills_extra_defense;
        }

        // (bHelp: the active team's revealed improvement instead)
        let improvement = plot.improvement_type;

        if let Some(improvement) = improvement {
            if let Some(defender) = defender {
                let plot_team = self.plot_get_team(p);
                if plot_team.is_none() || self.team_is_friendly_territory(defender, plot_team) {
                    modifier += d.improvements[improvement].defense_modifier;
                }
            }
        }

        // (C++ `if (!bHelp)`)
        if let Some(city) = plot.plot_city {
            modifier += self.city_defense_modifier(city, ignore_building);
        }
        //FfH: Added by Kael
        else if let Some(defender) = defender {
            let plot_team = self.plot_get_team(p);
            if plot_team.is_none() || self.team_is_friendly_territory(defender, plot_team) {
                modifier += self.plot_get_range_defense(p, defender, 3, false, true);
            }
        }
        //FfH: End Add

        modifier
    }

    /// C++ `CvPlot::getRangeDefense` (FFH): best defense/range-defense modifier of friendly
    /// improvements within `range` (`final_` = C++ `bFinal`: use final upgrades).
    pub fn plot_get_range_defense(&self, p: PlotIdx, defender: TeamId, range: i32, final_: bool, exclude_center: bool) -> i32 {
        let d = self.d;
        let mut best_modifier = 0;
        let (x, y) = self.map.xy(p);

        for dx in -range..=range {
            for dy in -range..=range {
                let Some(loop_plot) = self.plot_xy(x, y, dx, dy) else {
                    continue;
                };
                let Some(loop_owner) = self.map.plots[loop_plot].owner else {
                    continue;
                };
                let Some(mut improvement) = self.map.plots[loop_plot].improvement_type else {
                    continue;
                };

                if final_ {
                    if let Some(final_improvement) = final_improvement_upgrade(d, improvement, 0) {
                        improvement = final_improvement;
                    }
                }

                if self.plot_get_team(loop_plot) == Some(defender) {
                    if self
                        .plot_plot_check(loop_plot, Game::puf_is_enemy, loop_owner.i(), false as i32, None, None, None, -1, -1)
                        .is_none()
                    {
                        let mut modifier = 0;
                        let info = &d.improvements[improvement];
                        if dx == 0 && dy == 0 {
                            if !exclude_center {
                                modifier = info.defense_modifier;
                            }
                        } else if dx.abs() <= info.range && dy.abs() <= info.range {
                            modifier = info.range_defense_modifier;
                        }
                        if modifier > best_modifier {
                            best_modifier = modifier;
                        }
                    }
                }
            }
        }

        best_modifier
    }

    /// C++ `CvPlot::countNumAirUnits`.
    pub fn plot_count_num_air_units(&self, p: PlotIdx, team: TeamId) -> i32 {
        let mut count = 0;

        for &loop_unit in &self.map.plots[p].units {
            if self.unit_get_domain_type(loop_unit) == DomainType::DOMAIN_AIR
                && !self.unit_is_cargo(loop_unit)
                && self.unit_get_team(loop_unit) == team
            {
                count += self.d.units[self.units[loop_unit].unit_type].air_unit_cap;
            }
        }

        count
    }

    /// C++ `CvPlot::airUnitSpaceAvailable`.
    pub fn plot_air_unit_space_available(&self, p: PlotIdx, team: TeamId) -> i32 {
        let max_units = match self.map.plots[p].plot_city {
            Some(city) => {
                // C++ `pCity->getAirUnitCapacity(getTeam())`: a city plot is always owned.
                let plot_team = self.plot_get_team(p).expect("CvPlot::airUnitSpaceAvailable: city plot without a team");
                self.city_air_unit_capacity(city, plot_team)
            }
            None => self.d.defines.city_air_unit_capacity,
        };

        max_units - self.plot_count_num_air_units(p, team)
    }

    /// C++ `CvPlot::isEspionageCounterSpy`.
    pub fn plot_is_espionage_counter_spy(&self, p: PlotIdx, team: TeamId) -> bool {
        if let Some(city) = self.map.plots[p].plot_city {
            if self.city_team(city) == team {
                if self.cities[city].espionage_defense_modifier > 0 {
                    return true;
                }
            }
        }

        if self.plot_plot_count(p, Some(Game::puf_is_counter_spy), -1, -1, None, Some(team), None, -1, -1) > 0 {
            return true;
        }

        false
    }

    /// C++ `CvPlot::nukeExplosion` (C++ default `pNukeUnit = NULL`).
    pub fn plot_nuke_explosion(&mut self, p: PlotIdx, range: i32, nuke_unit: Option<UnitId>) {
        let d = self.d;
        let (x, y) = self.map.xy(p);
        // C++ reads `pNukeUnit->getOwnerINLINE()` at each use; the nuke unit is skipped below
        // and outlives the explosion (`CvUnit::nuke` kills it afterwards).
        let nuke_owner = nuke_unit.map(|n| self.units[n].owner);

        self.nukes_exploded += 1;

        for dx in -range..=range {
            for dy in -range..=range {
                let Some(loop_plot) = self.plot_xy(x, y, dx, dy) else {
                    continue;
                };

                // if we remove roads, don't remove them on the city... XXX

                let loop_city = self.map.plots[loop_plot].plot_city;

                if loop_city.is_none() {
                    if !self.plot_is_water(loop_plot) && !self.plot_is_impassable(loop_plot) {
                        if self.map.plots[loop_plot].feature_type.map_or(true, |f| !d.features[f].nuke_immune) {
                            if self.soren_rand_num(100, "Nuke Fallout") < d.defines.nuke_fallout_prob {
                                self.plot_set_improvement_type(loop_plot, None);
                                self.plot_set_feature_type(loop_plot, opt_from_i32(d.defines.nuke_feature));
                            }
                        }
                    }
                }

                // C++ copies the unit list (`oldUnits`) and looks each unit up by id, so units
                // killed meanwhile are skipped (`::getUnit` returns NULL).
                for loop_unit in self.map.plots[loop_plot].units.clone() {
                    if !self.units.contains(loop_unit) {
                        continue;
                    }

                    if Some(loop_unit) != nuke_unit {
                        if !self.unit_is_nuke_immune(loop_unit) && !self.units[loop_unit].death_delay {
                            // The two draws in source order (C++ leaves the operand order of
                            // `+` unspecified; MSVC evaluates left to right).
                            let damage_rand_1 = self.soren_rand_num(d.defines.nuke_unit_damage_rand_1, "Nuke Damage 1");
                            let damage_rand_2 = self.soren_rand_num(d.defines.nuke_unit_damage_rand_2, "Nuke Damage 2");
                            let mut nuke_damage = d.defines.nuke_unit_damage_base + damage_rand_1 + damage_rand_2;

                            if let Some(city) = loop_city {
                                nuke_damage *= (self.cities[city].nuke_modifier + 100).max(0);
                                nuke_damage /= 100;
                            }

                            if self.unit_can_fight(loop_unit) || self.unit_air_base_combat_str(loop_unit) > 0 {
                                self.unit_change_damage(loop_unit, nuke_damage, nuke_owner);
                            } else if nuke_damage >= d.defines.nuke_non_combat_death_threshold {
                                self.unit_kill(loop_unit, false, nuke_owner);
                            }
                        }
                    }
                }

                if let Some(city) = loop_city {
                    for building in d.buildings.keys() {
                        if self.cities[city].num_real_building[building] > 0 {
                            if !d.buildings[building].nuke_immune {
                                if self.soren_rand_num(100, "Building Nuked") < d.defines.nuke_building_destruction_prob {
                                    let num = self.cities[city].num_real_building[building];
                                    self.city_set_num_real_building(city, building, num - 1);
                                }
                            }
                        }
                    }

                    let population = self.cities[city].population;
                    let population_rand_1 = self.soren_rand_num(d.defines.nuke_population_death_rand_1, "Population Nuked 1");
                    let population_rand_2 = self.soren_rand_num(d.defines.nuke_population_death_rand_2, "Population Nuked 2");
                    let mut nuked_population =
                        (population * (d.defines.nuke_population_death_base + population_rand_1 + population_rand_2)) / 100;

                    nuked_population *= (self.cities[city].nuke_modifier + 100).max(0);
                    nuked_population /= 100;

                    let population = self.cities[city].population;
                    self.city_change_population(city, -((population - 1).min(nuked_population)));
                }
            }
        }

        // CvEventReporter::getInstance().nukeExplosion(this, pNukeUnit): FFH2's onNukeExplosion
        // only prints a debug line (CvUtil.pyPrint), so there is no events.rs hook to call.
    }

    // ------------------------------------------------------------------
    // Sight
    // ------------------------------------------------------------------

    /// C++ `CvPlot::seeFromLevel`.
    pub fn plot_see_from_level(&self, p: PlotIdx, team: TeamId) -> i32 {
        let d = self.d;

        let mut level = d.terrains[assigned_terrain(self, p)].see_from_level;

        if self.plot_is_peak(p) {
            level += d.defines.peak_see_from_change;
        }

        if self.plot_is_hills(p) {
            level += d.defines.hills_see_from_change;
        }

        if self.plot_is_water(p) {
            level += d.defines.seawater_see_from_change;

            if self.team_is_extra_water_see_from(team) {
                level += 1;
            }
        }

        level
    }

    /// C++ `CvPlot::seeThroughLevel`.
    pub fn plot_see_through_level(&self, p: PlotIdx) -> i32 {
        let d = self.d;

        let mut level = d.terrains[assigned_terrain(self, p)].see_through_level;

        if let Some(feature) = self.map.plots[p].feature_type {
            level += d.features[feature].see_through_change;
        }

        if self.plot_is_peak(p) {
            level += d.defines.peak_see_through_change;
        }

        if self.plot_is_hills(p) {
            level += d.defines.hills_see_through_change;
        }

        if self.plot_is_water(p) {
            // C++ quirk: adds SEAWATER_SEE_FROM_CHANGE, not SEAWATER_SEE_THROUGH_CHANGE (both
            // defines are 0).
            level += d.defines.seawater_see_from_change;
        }

        level
    }

    /// C++ `CvPlot::changeAdjacentSight`: adds/removes `team`'s sight of radius `range`
    /// around this plot (line of sight; `unit` supplies facing and see-invisible types).
    pub fn plot_change_adjacent_sight(&mut self, p: PlotIdx, team: TeamId, range: i32, increment: bool, unit: Option<UnitId>, update_plot_groups: bool) {
        let aerial = unit.is_some_and(|u| self.unit_get_domain_type(u) == DomainType::DOMAIN_AIR);

        // C++ `pUnit->getFacingDirection(true)` for non-aerial units: the unit's facing only
        // for bLineOfSight unit types, else NO_DIRECTION. No FFH unit has bLineOfSight (the
        // facing, `m_eFacingDirection`, is graphics state and not ported), so it is always
        // NO_DIRECTION.
        let facing_direction: Option<DirectionType> = None;
        debug_assert!(
            aerial || unit.map_or(true, |u| !self.d.units[self.units[u].unit_type].line_of_sight),
            "bLineOfSight units need CvUnit::m_eFacingDirection"
        );

        // fill invisible types
        let mut see_invisible_types: Vec<Option<InvisibleType>> = Vec::new();
        if let Some(u) = unit {
            for i in 0..self.unit_get_num_see_invisible_types(u) {
                see_invisible_types.push(Some(self.unit_get_see_invisible_type(u, i)));
            }
        }

        if see_invisible_types.is_empty() {
            see_invisible_types.push(None);
        }

        // check one extra outer ring
        let range = if aerial { range } else { range + 1 };

        let (x, y) = self.map.xy(p);
        let change = if increment { 1 } else { -1 };

        for &see_invisible in &see_invisible_types {
            for dx in -range..=range {
                for dy in -range..=range {
                    // check if in facing direction
                    if aerial || self.plot_should_process_displacement_plot(p, dx, dy, range - 1, facing_direction) {
                        let outer_ring = dx.abs() == range || dy.abs() == range;

                        // check if anything blocking the plot
                        if aerial || self.plot_can_see_displacement_plot(p, team, dx, dy, dx, dy, true, outer_ring) {
                            if let Some(plot) = self.plot_xy(x, y, dx, dy) {
                                self.plot_change_visibility_count(plot, team, change, see_invisible, update_plot_groups);
                            }
                        }
                    }

                    if facing_direction.is_some() {
                        // always reveal adjacent plots when using line of sight
                        if dx.abs() <= 1 && dy.abs() <= 1 {
                            if let Some(plot) = self.plot_xy(x, y, dx, dy) {
                                self.plot_change_visibility_count(plot, team, 1, see_invisible, update_plot_groups);
                                self.plot_change_visibility_count(plot, team, -1, see_invisible, update_plot_groups);
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvPlot::canSeePlot` (C++ returns false for a NULL plot).
    pub fn plot_can_see_plot(&self, p: PlotIdx, other: PlotIdx, team: TeamId, range: i32, facing: Option<DirectionType>) -> bool {
        let range = range + 1;

        // find displacement
        let (x, y) = self.map.xy(p);
        let (other_x, other_y) = self.map.xy(other);
        let dx = self.dx_wrap(other_x - x); // world wrap
        let dy = self.dy_wrap(other_y - y);

        // check if in facing direction
        if self.plot_should_process_displacement_plot(p, dx, dy, range - 1, facing) {
            let outer_ring = dx.abs() == range || dy.abs() == range;

            // check if anything blocking the plot
            if self.plot_can_see_displacement_plot(p, team, dx, dy, dx, dy, true, outer_ring) {
                return true;
            }
        }

        false
    }

    /// C++ `CvPlot::canSeeDisplacementPlot` (recursive line-of-sight test). `original_dx`/`dy`
    /// are only passed down the recursion, as in the C++ (their one use there is commented out).
    #[allow(clippy::too_many_arguments, clippy::only_used_in_recursion)]
    pub fn plot_can_see_displacement_plot(
        &self,
        p: PlotIdx,
        team: TeamId,
        dx: i32,
        dy: i32,
        original_dx: i32,
        original_dy: i32,
        first_plot: bool,
        outer_ring: bool,
    ) -> bool {
        let (x, y) = self.map.xy(p);
        let Some(plot) = self.plot_xy(x, y, dx, dy) else {
            return false;
        };

        // base case is current plot
        if dx == 0 && dy == 0 {
            return true;
        }

        // find closest of three points (1, 2, 3) to original line from Start (S) to End (E)
        // The diagonal is computed first as that guarantees a change in position
        // -------------
        // |   | 2 | S |
        // -------------
        // | E | 1 | 3 |
        // -------------

        let displacements = [[dx - get_sign(dx), dy - get_sign(dy)], [dx - get_sign(dx), dy], [dx, dy - get_sign(dy)]];
        let mut all_closest = [0; 3];
        let mut closest = -1;
        for i in 0..3 {
            // (C++ comment: the cross product with `original_dx/dy` would be more accurate,
            // but less structured on a grid; the parameters are unused.)
            all_closest[i] = (displacements[i][0] * dy - displacements[i][1] * dx).abs(); // cross product
            if closest == -1 || all_closest[i] < closest {
                closest = all_closest[i];
            }
        }

        // iterate through all minimum plots to see if any of them are passable
        for i in 0..3 {
            let next_dx = displacements[i][0];
            let next_dy = displacements[i][1];
            // make sure we change plots
            if (next_dx != dx || next_dy != dy) && all_closest[i] == closest {
                if self.plot_can_see_displacement_plot(p, team, next_dx, next_dy, original_dx, original_dy, false, false) {
                    let from_level = self.plot_see_from_level(p, team);
                    let through_level = self.plot_see_through_level(plot);
                    if outer_ring {
                        // check strictly higher level
                        // (the recursive call above found this plot, so it exists)
                        let pass_through_plot = self
                            .plot_xy(x, y, next_dx, next_dy)
                            .expect("CvPlot::canSeeDisplacementPlot: pass-through plot");
                        let pass_through_level = self.plot_see_through_level(pass_through_plot);
                        if from_level >= pass_through_level {
                            // either we can see through to it or it is high enough to see from far
                            if from_level > pass_through_level || self.plot_see_from_level(plot, team) > from_level {
                                return true;
                            }
                        }
                    } else if from_level >= through_level {
                        // we can clearly see this level
                        return true;
                    } else if first_plot {
                        // we can also see it if it is the first plot that is too tall
                        return true;
                    }
                }
            }
        }

        false
    }

    /// C++ `CvPlot::shouldProcessDisplacementPlot`: is (dx, dy) inside the sight cone of
    /// `facing` (None = no facing: always)? Reads only its arguments (`range` is unused in
    /// the C++ too).
    #[inline]
    #[allow(unused_variables)]
    pub fn plot_should_process_displacement_plot(&self, p: PlotIdx, dx: i32, dy: i32, range: i32, facing: Option<DirectionType>) -> bool {
        should_process_displacement(dx, dy, facing)
    }

    /// C++ `CvPlot::updateSight`: applies/removes all sight sources on this plot (FFH holy
    /// city religions, vassals, espionage, ownership, units, recon).
    pub fn plot_update_sight(&mut self, p: PlotIdx, increment: bool, update_plot_groups: bool) {
        let d = self.d;
        let plot_visibility_range = d.defines.plot_visibility_range;

        if let Some(city) = self.map.plots[p].plot_city {
            // Religion - Disabled with new Espionage System (BTS; FfH re-enables it below for
            // religions with bUpdateSight)

            //FfH: Added by Kael 11/03/2007
            for religion in d.religions.keys() {
                if self.cities[city].has_religion[religion] {
                    if d.religions[religion].update_sight {
                        if let Some(holy_city) = self.get_holy_city(religion) {
                            if self.player_get_state_religion(self.cities[holy_city].owner) == Some(religion) {
                                let holy_team = self.city_team(holy_city);
                                self.plot_change_adjacent_sight(p, holy_team, plot_visibility_range, increment, None, update_plot_groups);
                            }
                        }
                    }
                }
            }
            //FfH: End Add

            // Vassal (a city plot is always owned)
            if let Some(plot_team) = self.plot_get_team(p) {
                for t in self.team_ids() {
                    if self.teams[plot_team].vassal[t] {
                        self.plot_change_adjacent_sight(p, t, plot_visibility_range, increment, None, update_plot_groups);
                    }
                }
            }

            // EspionageEffect
            for t in self.civ_team_ids() {
                if self.cities[city].espionage_visibility[t] {
                    // Passive Effect: enough EPs gives you visibility into someone's cities
                    self.plot_change_adjacent_sight(p, t, plot_visibility_range, increment, None, update_plot_groups);
                }
            }
        }

        // Owned
        if let Some(plot_team) = self.plot_get_team(p) {
            self.plot_change_adjacent_sight(p, plot_team, plot_visibility_range, increment, None, update_plot_groups);
        }

        // Unit
        // (C++ walks the live list; walk a copy and skip units that no longer exist.)
        for loop_unit in self.map.plots[p].units.clone() {
            if !self.units.contains(loop_unit) {
                continue;
            }

            let team = self.unit_get_team(loop_unit);
            let range = self.unit_visibility_range(loop_unit);
            self.plot_change_adjacent_sight(p, team, range, increment, Some(loop_unit), update_plot_groups);
        }

        if self.map.plots[p].recon_count > 0 {
            let range = d.defines.recon_visibility_range;
            for player in self.player_ids() {
                for loop_unit in self.players[player].units.clone() {
                    if !self.units.contains(loop_unit) {
                        continue;
                    }

                    if self.unit_get_recon_plot(loop_unit) == Some(p) {
                        let team = self.unit_get_team(loop_unit);
                        self.plot_change_adjacent_sight(p, team, range, increment, Some(loop_unit), update_plot_groups);
                    }
                }
            }
        }
    }

    /// C++ `CvPlot::updateSeeFromSight`: `updateSight` on every plot that could see this one.
    pub fn plot_update_see_from_sight(&mut self, p: PlotIdx, increment: bool, update_plot_groups: bool) {
        let d = self.d;

        let mut range = d.defines.unit_visibility_range + 1;
        for promotion in d.promotions.iter() {
            range += promotion.visibility_change;
        }

        range = (d.defines.recon_visibility_range + 1).max(range);

        let (x, y) = self.map.xy(p);
        for dx in -range..=range {
            for dy in -range..=range {
                if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                    self.plot_update_sight(loop_plot, increment, update_plot_groups);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Events and training
    // ------------------------------------------------------------------

    /// C++ `CvPlot::canTrigger`.
    pub fn plot_can_trigger(&self, p: PlotIdx, trigger: EventTriggerType, player: PlayerId) -> bool {
        // (C++ FAssert: `isPlotEventTrigger(eTrigger)`.)
        let k_trigger = &self.d.event_triggers[trigger];
        let plot = &self.map.plots[p];

        if k_trigger.own_plot && plot.owner != Some(player) {
            return false;
        }

        if let Some(plot_type) = k_trigger.plot_type {
            if plot.plot_type != plot_type {
                return false;
            }
        }

        if !k_trigger.features_required.is_empty() {
            let found_valid = k_trigger.features_required.contains(&plot.feature_type);

            if !found_valid {
                return false;
            }
        }

        if !k_trigger.terrains_required.is_empty() {
            let found_valid = k_trigger.terrains_required.iter().any(|&t| Some(t) == plot.terrain_type);

            if !found_valid {
                return false;
            }
        }

        if !k_trigger.improvements_required.is_empty() {
            let found_valid = k_trigger.improvements_required.contains(&plot.improvement_type);

            if !found_valid {
                return false;
            }
        }

        if !k_trigger.bonuses_required.is_empty() {
            let team = if k_trigger.own_plot { Some(self.players[player].team) } else { None };
            let bonus = self.plot_get_bonus_type(p, team);
            let found_valid = k_trigger.bonuses_required.contains(&bonus);

            if !found_valid {
                return false;
            }
        }

        if !k_trigger.routes_required.is_empty() {
            let mut found_valid = false;

            if plot.plot_city.is_none() {
                found_valid = k_trigger.routes_required.contains(&plot.route_type);
            }

            if !found_valid {
                return false;
            }
        }

        if k_trigger.units_on_plot {
            let mut found_valid = false;

            for &loop_unit in &plot.units {
                if self.units[loop_unit].owner == player {
                    if self.unit_get_trigger_value(loop_unit, trigger, Some(p), false) != -1 {
                        found_valid = true;
                        break;
                    }
                }
            }

            if !found_valid {
                return false;
            }
        }

        if k_trigger.prereq_event_city && !k_trigger.prereq_events.is_empty() {
            let mut found_valid = true;

            for &event in &k_trigger.prereq_events {
                let triggered = self.player_get_event_occured(player, event);
                if triggered.map_or(true, |t| t.plot_x != plot.x || t.plot_y != plot.y) {
                    found_valid = false;
                    break;
                }
            }

            if !found_valid {
                return false;
            }
        }

        true
    }

    /// C++ `CvPlot::canApplyEvent`.
    pub fn plot_can_apply_event(&self, p: PlotIdx, event: EventType) -> bool {
        let k_event = &self.d.events[event];
        let plot = &self.map.plots[p];

        if k_event.feature_change > 0 {
            if let Some(feature) = k_event.feature {
                if plot.improvement_type.is_some() || !self.plot_can_have_feature(p, Some(feature)) {
                    return false;
                }
            }
        } else if k_event.feature_change < 0 {
            if plot.feature_type.is_none() {
                return false;
            }
        }

        if k_event.improvement_change > 0 {
            if let Some(improvement) = k_event.improvement {
                if !self.plot_can_have_improvement(p, improvement, self.plot_get_team(p), false) {
                    return false;
                }
            }
        } else if k_event.improvement_change < 0 {
            if plot.improvement_type.is_none() {
                return false;
            }
        }

        //FfH: Modified by Kael 03/19/2008 (the BTS bonus checks are commented out)

        if k_event.route_change < 0 {
            if plot.route_type.is_none() {
                return false;
            }

            if plot.plot_city.is_some() {
                return false;
            }
        }

        true
    }

    /// C++ `CvPlot::applyEvent`.
    pub fn plot_apply_event(&mut self, p: PlotIdx, event: EventType) {
        let k_event = &self.d.events[event];

        if k_event.feature_change > 0 {
            if let Some(feature) = k_event.feature {
                self.plot_set_feature_type(p, Some(feature));
            }
        } else if k_event.feature_change < 0 {
            self.plot_set_feature_type(p, None);
        }

        if k_event.improvement_change > 0 {
            if let Some(improvement) = k_event.improvement {
                self.plot_set_improvement_type(p, Some(improvement));
            }
        } else if k_event.improvement_change < 0 {
            self.plot_set_improvement_type(p, None);
        }

        if k_event.bonus_change > 0 {
            if let Some(bonus) = k_event.bonus {
                self.plot_set_bonus_type(p, Some(bonus));
            }
        } else if k_event.bonus_change < 0 {
            self.plot_set_bonus_type(p, None);
        }

        if k_event.route_change > 0 {
            if let Some(route) = k_event.route {
                self.plot_set_route_type(p, Some(route), true);
            }
        } else if k_event.route_change < 0 {
            self.plot_set_route_type(p, None, true);
        }

        let (x, y) = self.map.xy(p);
        for i in 0..YieldType::NUM {
            let change = k_event.plot_extra_yield[i];
            if change != 0 {
                self.set_plot_extra_yield(x, y, YieldType::from_usize(i), change);
            }
        }
    }

    /// C++ `CvPlot::canTrain`: plot/city requirements for training `unit` here
    /// (`continue_` = C++ `bContinue`, unused by the C++ body).
    #[allow(unused_variables)]
    pub fn plot_can_train(&self, p: PlotIdx, unit: UnitType, continue_: bool, test_visible: bool) -> bool {
        let d = self.d;
        let info = &d.units[unit];
        let plot = &self.map.plots[p];
        let city = plot.plot_city;
        let (x, y) = (plot.x, plot.y);

        if info.is_prereq_religion {
            if city.map_or(true, |c| self.city_religion_count(c) > 0) {
                return false;
            }
        }

        if let Some(religion) = info.prereq_religion {
            if city.map_or(true, |c| !self.cities[c].has_religion[religion]) {
                return false;
            }
        }

        if let Some(corporation) = info.prereq_corporation {
            if city.map_or(true, |c| !self.city_is_active_corporation(c, corporation)) {
                return false;
            }
        }

        if info.prereq_bonuses {
            if info.domain_type == Some(DomainType::DOMAIN_SEA) {
                let mut valid = false;

                for i in 0..DirectionType::NUM {
                    if let Some(loop_plot) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                        if self.plot_is_water(loop_plot) {
                            if self.area_get_num_total_bonuses(assigned_area(self, loop_plot)) > 0 {
                                valid = true;
                                break;
                            }
                        }
                    }
                }

                if !valid {
                    return false;
                }
            } else {
                // C++ quirk: a non-sea bPrereqBonuses unit needs an area WITHOUT bonuses (BTS
                // code; only UNIT_WORKBOAT, a sea unit, sets the flag).
                if self.area_get_num_total_bonuses(assigned_area(self, p)) > 0 {
                    return false;
                }
            }
        }

        // (C++ `isCity()`)
        if city.is_some() {
            if info.domain_type == Some(DomainType::DOMAIN_SEA) {
                if !self.plot_is_water(p) && !self.plot_is_coastal_land(p, info.min_area_size) {
                    return false;
                }
            } else if self.map.areas[assigned_area(self, p)].num_tiles < info.min_area_size {
                return false;
            }
        } else {
            if self.map.areas[assigned_area(self, p)].num_tiles < info.min_area_size {
                return false;
            }

            if info.domain_type == Some(DomainType::DOMAIN_SEA) {
                if !self.plot_is_water(p) {
                    return false;
                }
            } else if info.domain_type == Some(DomainType::DOMAIN_LAND) {
                if self.plot_is_water(p) {
                    return false;
                }
            } else {
                return false;
            }
        }

        if !test_visible {
            if let Some(religion) = info.holy_city {
                if city.map_or(true, |c| !self.city_is_holy_city(c, religion)) {
                    return false;
                }
            }

            //FfH Units: Modified by Kael 12/08/2007 (BTS: the PrereqBuilding check alone)
            let mut valid = true;
            if let Some(owner) = plot.owner {
                if !self.players[owner].human {
                    // Xienwolf Tweak 02/01/09: barbarian cities do not devote themselves to
                    // building Empires.
                    if self.is_option(GameOptionType::GAMEOPTION_AI_NO_BUILDING_PREREQS) || self.player_is_barbarian(owner) {
                        valid = false;
                    }
                }
            }
            if valid {
                if let Some(building) = info.prereq_building {
                    let Some(city) = city else {
                        return false;
                    };
                    if self.city_num_building(city, building) == 0 {
                        let special_building = d.buildings[building].special_building_type;
                        // (`GET_PLAYER(getOwnerINLINE())`: a city plot is always owned.)
                        let owner = plot.owner.expect("CvPlot::canTrain: city plot without an owner");
                        if special_building.map_or(true, |s| !self.player_is_special_building_not_required(owner, s)) {
                            return false;
                        }
                    }
                }
                if let Some(building_class) = info.prereq_building_class {
                    let Some(city) = city else {
                        return false;
                    };
                    if !self.city_is_has_building_class(city, building_class) {
                        return false;
                    }
                }
            }
            //FfH: End Modify

            if let Some(bonus) = info.prereq_and_bonus {
                match city {
                    None => {
                        if !plot_group_connected_bonus(self, p, bonus) {
                            return false;
                        }
                    }
                    Some(city) => {
                        if !self.city_has_bonus(city, bonus) {
                            return false;
                        }
                    }
                }
            }

            let mut requires_bonus = false;
            let mut needs_bonus = true;

            // (`prereq_or_bonuses` holds the C++ NUM_UNIT_PREREQ_OR_BONUSES slots minus NO_BONUS)
            for &bonus in &info.prereq_or_bonuses {
                requires_bonus = true;

                let has_bonus = match city {
                    None => plot_group_connected_bonus(self, p, bonus),
                    Some(city) => self.city_has_bonus(city, bonus),
                };
                if has_bonus {
                    needs_bonus = false;
                    break;
                }
            }

            if requires_bonus && needs_bonus {
                return false;
            }
        }

        true
    }
}

/// The plot's terrain. C++ `FAssertMsg(getTerrainType() != NO_TERRAIN, ...)` before reading
/// the terrain info: every plot has a terrain once the map is generated.
fn assigned_terrain(g: &Game, p: PlotIdx) -> TerrainType {
    g.map.plots[p].terrain_type.expect("CvPlot: TerrainType is not assigned a valid value")
}

/// C++ `area()` at call sites that dereference it (every plot has an area once the map is
/// generated).
fn assigned_area(g: &Game, p: PlotIdx) -> AreaId {
    g.map.plots[p].area.expect("CvPlot::area: plot without an area")
}

/// C++ `isPlotGroupConnectedBonus(getOwnerINLINE(), eBonus)` from `canTrain` on a plot without
/// a city. C++ passes NO_PLAYER for an unowned plot (an out-of-range read); an unowned plot is in
/// no plot group, so the bonus counts as not connected.
fn plot_group_connected_bonus(g: &Game, p: PlotIdx, bonus: BonusType) -> bool {
    match g.map.plots[p].owner {
        Some(owner) => g.plot_is_plot_group_connected_bonus(p, owner, bonus),
        None => false,
    }
}

/// Body of C++ `CvPlot::shouldProcessDisplacementPlot`, with the same single-precision float
/// ops as the C++ (`atan2` on floats, `60 * (float)M_PI / 180`).
fn should_process_displacement(dx: i32, dy: i32, facing: Option<DirectionType>) -> bool {
    let Some(facing) = facing else {
        return true;
    };
    if dx == 0 && dy == 0 {
        // always process this plot
        return true;
    }
    //                                    N       NE      E       SE       S        SW        W        NW
    const DISPLACEMENTS: [[i32; 2]; 8] = [[0, 1], [1, 1], [1, 0], [1, -1], [0, -1], [-1, -1], [-1, 0], [-1, 1]];
    let direction_x = DISPLACEMENTS[facing.idx()][0];
    let direction_y = DISPLACEMENTS[facing.idx()][1];

    // compute angle off of direction
    let cross_product = direction_x * dy - direction_y * dx;
    let dot_product = direction_x * dx + direction_y * dy;

    let theta = (cross_product as f32).atan2(dot_product as f32);
    let mut spread = 60.0_f32 * std::f32::consts::PI / 180.0;
    if dx.abs() <= 1 && dy.abs() <= 1 {
        // close plots use wider spread
        spread = 90.0_f32 * std::f32::consts::PI / 180.0;
    }
    theta >= -spread / 2.0 && theta <= spread / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, MapInitData};
    use crate::setup::GameSetup;
    use crate::unit::{Unit, UnitResetInfo};

    #[test]
    fn facing_cone_boundaries() {
        let n = Some(DirectionType::DIRECTION_NORTH);
        assert!(should_process_displacement(0, 1, n));
        // 45 degrees off for a close plot is on the 90-degree cone boundary (inclusive).
        assert!(should_process_displacement(1, 1, n));
        assert!(should_process_displacement(-1, 1, n));
        assert!(!should_process_displacement(1, 0, n));
        assert!(!should_process_displacement(0, -2, n));
        // far plots use the 60-degree cone
        assert!(should_process_displacement(1, 3, n));
        assert!(!should_process_displacement(2, 2, n));
        assert!(should_process_displacement(0, 0, n));
        assert!(should_process_displacement(5, -5, None));
    }

    /// A game on a `w` x `h` flat grassland map (no wrap).
    fn game_with_map(w: i32, h: i32) -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 3));
        let init = MapInitData { grid_w: w, grid_h: h, ..Default::default() };
        g.map = Map::new(d, &init, g.num_players(), g.num_teams());
        let grass = d.t::<TerrainType>("TERRAIN_GRASS");
        for plot in g.map.plots.iter_mut() {
            plot.plot_type = PlotType::PLOT_LAND;
            plot.terrain_type = Some(grass);
        }
        g
    }

    fn at(g: &Game, x: i32, y: i32) -> PlotIdx {
        g.map.plot_xy(x, y).unwrap()
    }

    #[test]
    fn see_levels() {
        let mut g = game_with_map(5, 5);
        let d = g.d;
        let team = TeamId(0);
        let (flat, hill, peak, forest, ocean) = (at(&g, 0, 0), at(&g, 1, 0), at(&g, 2, 0), at(&g, 3, 0), at(&g, 4, 0));
        g.map.plots[hill].plot_type = PlotType::PLOT_HILLS;
        g.map.plots[peak].plot_type = PlotType::PLOT_PEAK;
        g.map.plots[forest].feature_type = Some(d.t::<FeatureType>("FEATURE_FOREST"));
        g.map.plots[ocean].plot_type = PlotType::PLOT_OCEAN;
        g.map.plots[ocean].terrain_type = Some(d.t::<TerrainType>("TERRAIN_OCEAN"));

        let grass = &d.terrains[d.t::<TerrainType>("TERRAIN_GRASS")];
        let df = &d.defines;
        assert_eq!(g.plot_see_from_level(flat, team), grass.see_from_level);
        assert_eq!(g.plot_see_through_level(flat), grass.see_through_level);
        assert_eq!(g.plot_see_from_level(hill, team), grass.see_from_level + df.hills_see_from_change);
        assert_eq!(g.plot_see_through_level(hill), grass.see_through_level + df.hills_see_through_change);
        assert_eq!(g.plot_see_from_level(peak, team), grass.see_from_level + df.peak_see_from_change);
        assert_eq!(g.plot_see_through_level(peak), grass.see_through_level + df.peak_see_through_change);
        // a feature changes only the see-through level
        assert_eq!(g.plot_see_from_level(forest, team), grass.see_from_level);
        assert_eq!(
            g.plot_see_through_level(forest),
            grass.see_through_level + d.features[d.t::<FeatureType>("FEATURE_FOREST")].see_through_change
        );
        assert!(g.plot_see_through_level(forest) > g.plot_see_from_level(flat, team));

        let ocean_from = g.plot_see_from_level(ocean, team);
        g.teams[team].extra_water_see_from_count = 1;
        assert_eq!(g.plot_see_from_level(ocean, team), ocean_from + 1);
        // only water plots get the extra level
        assert_eq!(g.plot_see_from_level(flat, team), grass.see_from_level);
    }

    #[test]
    fn line_of_sight() {
        let mut g = game_with_map(9, 9);
        let d = g.d;
        let team = TeamId(0);
        let center = at(&g, 4, 4);

        // range 1 (+1 outer ring): adjacent plots are always visible, flat plots of the outer
        // ring are not seen from flat ground
        assert!(g.plot_can_see_plot(center, at(&g, 5, 4), team, 1, None));
        assert!(g.plot_can_see_plot(center, at(&g, 3, 5), team, 1, None));
        assert!(!g.plot_can_see_plot(center, at(&g, 6, 4), team, 1, None));
        assert!(!g.plot_can_see_plot(center, at(&g, 6, 6), team, 1, None));

        // ... but outer-ring hills are (they see from higher)
        let hill = at(&g, 6, 4);
        g.map.plots[hill].plot_type = PlotType::PLOT_HILLS;
        assert!(g.plot_can_see_plot(center, at(&g, 6, 4), team, 1, None));

        // ... unless something tall stands in between
        let forest = at(&g, 5, 4);
        g.map.plots[forest].feature_type = Some(d.t::<FeatureType>("FEATURE_FOREST"));
        assert!(!g.plot_can_see_plot(center, at(&g, 6, 4), team, 1, None));
        // the first blocking plot itself is visible
        assert!(g.plot_can_see_plot(center, at(&g, 5, 4), team, 1, None));

        // from a hill (see-from 2), flat outer-ring plots are visible over flat ground
        g.map.plots[center].plot_type = PlotType::PLOT_HILLS;
        assert!(g.plot_can_see_plot(center, at(&g, 4, 6), team, 1, None));
        assert!(g.plot_can_see_plot(center, at(&g, 2, 2), team, 1, None));
        assert!(g.plot_can_see_plot(center, at(&g, 2, 4), team, 1, None));
        // the forest (see-through 2) is visible, but the outer ring needs a strictly lower
        // pass-through plot (or a target higher than the viewer): the hill behind it is hidden
        assert!(g.plot_can_see_plot(center, at(&g, 5, 4), team, 1, None));
        assert!(!g.plot_can_see_plot(center, at(&g, 6, 4), team, 1, None));

        // range 2 (+1): a peak blocks the plots behind it, but is itself visible
        let mut g = game_with_map(9, 9);
        let center = at(&g, 4, 4);
        let peak = at(&g, 5, 4);
        g.map.plots[peak].plot_type = PlotType::PLOT_PEAK;
        assert!(g.plot_can_see_plot(center, at(&g, 5, 4), team, 2, None));
        assert!(!g.plot_can_see_plot(center, at(&g, 6, 4), team, 2, None));
        assert!(!g.plot_can_see_plot(center, at(&g, 7, 4), team, 2, None));
        // equally close lines of sight pass beside the peak
        assert!(g.plot_can_see_plot(center, at(&g, 6, 5), team, 2, None));
        assert!(g.plot_can_see_plot(center, at(&g, 6, 3), team, 2, None));
        assert!(g.plot_can_see_plot(center, at(&g, 4, 6), team, 2, None));
        assert!(g.plot_can_see_plot(center, at(&g, 2, 4), team, 2, None));

        // off-map displacements are never visible
        let corner = at(&g, 0, 0);
        assert!(!g.plot_can_see_displacement_plot(corner, team, -1, 0, -1, 0, true, false));
        assert!(g.plot_can_see_displacement_plot(corner, team, 0, 0, 0, 0, true, false));
    }

    #[test]
    fn defense_modifier_without_defender() {
        let mut g = game_with_map(3, 3);
        let d = g.d;
        let p = at(&g, 1, 1);
        let grass = &d.terrains[d.t::<TerrainType>("TERRAIN_GRASS")];
        let forest = d.t::<FeatureType>("FEATURE_FOREST");
        assert_eq!(g.plot_defense_modifier(p, None, false), grass.defense_modifier);
        g.map.plots[p].plot_type = PlotType::PLOT_HILLS;
        assert_eq!(g.plot_defense_modifier(p, None, false), grass.defense_modifier + d.defines.hills_extra_defense);
        // a feature replaces the terrain modifier
        g.map.plots[p].feature_type = Some(forest);
        assert_eq!(g.plot_defense_modifier(p, None, false), d.features[forest].defense_modifier + d.defines.hills_extra_defense);
        // unowned plots without improvements give no range defense
        assert_eq!(g.plot_get_range_defense(p, TeamId(0), 3, false, true), 0);
    }

    #[test]
    fn extra_move_path_cost() {
        let mut g = game_with_map(3, 3);
        let p = at(&g, 2, 1);
        assert_eq!(g.plot_get_extra_move_path_cost(p), 0);
        g.plot_change_extra_move_path_cost(p, 7);
        g.plot_change_extra_move_path_cost(p, 3);
        assert_eq!(g.plot_get_extra_move_path_cost(p), 10);
        assert_eq!(g.plot_extra_cost(2, 1), 10);
        assert_eq!(g.plot_get_extra_move_path_cost(at(&g, 1, 2)), 0);
    }

    #[test]
    fn trigger_and_event_requirements() {
        let mut g = game_with_map(3, 3);
        let d = g.d;
        let p = at(&g, 1, 1);
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let forest = d.t::<FeatureType>("FEATURE_FOREST");
        let road = d.t::<RouteType>("ROUTE_ROAD");
        let farm = d.t::<ImprovementType>("IMPROVEMENT_FARM");
        g.map.plots[p].owner = Some(p0);

        // own plot, FEATURE_FOREST, improvements [NONE], routes [NONE]
        let antelope = d.t::<EventTriggerType>("EVENTTRIGGER_ANTELOPE");
        assert!(!g.plot_can_trigger(p, antelope, p0));
        g.map.plots[p].feature_type = Some(forest);
        assert!(g.plot_can_trigger(p, antelope, p0));
        assert!(!g.plot_can_trigger(p, antelope, p1));
        g.map.plots[p].route_type = Some(road);
        assert!(!g.plot_can_trigger(p, antelope, p0));
        g.map.plots[p].route_type = None;
        g.map.plots[p].improvement_type = Some(farm);
        assert!(!g.plot_can_trigger(p, antelope, p0));
        g.map.plots[p].improvement_type = None;
        // a city plot never satisfies a route requirement (not even NONE)
        g.map.plots[p].plot_city = Some(CityId(0));
        assert!(!g.plot_can_trigger(p, antelope, p0));
        g.map.plots[p].plot_city = None;

        // own plot, TERRAIN_GRASS / PLAINS / TUNDRA
        let procession = d.t::<EventTriggerType>("EVENTTRIGGER_ANGELIC_PROCESSION");
        assert!(g.plot_can_trigger(p, procession, p0));
        g.map.plots[p].terrain_type = Some(d.t::<TerrainType>("TERRAIN_DESERT"));
        assert!(!g.plot_can_trigger(p, procession, p0));

        // route removal: needs a route, never on a city
        let washed_out = d.t::<EventType>("EVENT_WASHED_OUT_1");
        assert!(!g.plot_can_apply_event(p, washed_out));
        g.map.plots[p].route_type = Some(road);
        assert!(g.plot_can_apply_event(p, washed_out));
        g.map.plots[p].plot_city = Some(CityId(0));
        assert!(!g.plot_can_apply_event(p, washed_out));
        g.map.plots[p].plot_city = None;
        // improvement removal: needs an improvement
        let dustbowl = d.t::<EventType>("EVENT_DUSTBOWL_1");
        assert!(!g.plot_can_apply_event(p, dustbowl));
        g.map.plots[p].improvement_type = Some(farm);
        assert!(g.plot_can_apply_event(p, dustbowl));
        // FfH dropped the bonus checks: a bonus removal applies to a plot without a bonus
        assert_eq!(g.map.plots[p].bonus_type, None);
        assert!(g.plot_can_apply_event(p, d.t::<EventType>("EVENT_PIG_GIANT_1")));
    }

    #[test]
    fn unit_counts_and_power() {
        let mut g = game_with_map(3, 3);
        let d = g.d;
        let p = at(&g, 1, 1);
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let warrior = d.t::<UnitType>("UNIT_WARRIOR");
        let axeman = d.t::<UnitType>("UNIT_AXEMAN");
        let add = |g: &mut Game, unit_type: UnitType, owner: PlayerId, damage: i32| {
            let u = g.units.insert_with(|id| Unit::new(id, unit_type, owner, d, &UnitResetInfo::default()));
            g.units[u].damage = damage;
            g.map.plots[p].units.push(u);
            u
        };
        let a = add(&mut g, warrior, p0, 0);
        let b = add(&mut g, axeman, p1, 30);
        let c = add(&mut g, warrior, p1, 60);
        assert!(g.plot_is_unit(p));
        assert!(!g.plot_is_unit(at(&g, 0, 0)));

        let hurt: ConstPlotUnitFunc = |g, u, min_damage, _| g.units[u].damage >= min_damage;
        let is_type: ConstPlotUnitFunc = |g, u, t, _| g.units[u].unit_type.i() == t;
        assert_eq!(g.plot_plot_count(p, None, -1, -1, None, None, None, -1, -1), 3);
        assert_eq!(g.plot_plot_count(p, None, -1, -1, Some(p1), None, None, -1, -1), 2);
        assert_eq!(g.plot_plot_count(p, Some(hurt), 30, -1, None, None, None, -1, -1), 2);
        assert_eq!(g.plot_plot_count(p, Some(hurt), 30, -1, None, None, Some(is_type), warrior.i(), -1), 1);
        assert_eq!(g.plot_plot_count(p, Some(hurt), 30, -1, Some(p0), None, None, -1, -1), 0);
        // plotCheck returns the first match in list order
        assert_eq!(g.plot_plot_check(p, hurt, 0, -1, None, None, None, -1, -1), Some(a));
        assert_eq!(g.plot_plot_check(p, hurt, 1, -1, None, None, None, -1, -1), Some(b));
        assert_eq!(g.plot_plot_check(p, hurt, 1, -1, None, None, Some(is_type), warrior.i(), -1), Some(c));
        assert_eq!(g.plot_plot_check(p, hurt, 100, -1, None, None, None, -1, -1), None);

        let power = |t: UnitType| d.units[t].power_value;
        assert_eq!(g.plot_get_unit_power(p, None), 2 * power(warrior) + power(axeman));
        assert_eq!(g.plot_get_unit_power(p, Some(p0)), power(warrior));
        assert_eq!(g.plot_get_unit_power(p, Some(p1)), power(warrior) + power(axeman));

        g.plot_remove_unit(p, b);
        assert_eq!(g.map.plots[p].units, vec![a, c]);
        g.plot_remove_unit(p, b);
        assert_eq!(g.map.plots[p].units, vec![a, c]);
    }
}
