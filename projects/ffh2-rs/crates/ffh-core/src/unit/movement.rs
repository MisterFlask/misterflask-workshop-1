//! CvUnit movement: territory/area entry, `canMoveInto`, `move`, paths, loading and cargo, moves
//! left, visibility and invisibility, enemy tests, jump/withdraw to a valid plot.
//!
//! `docs/porting/unit.md` lists the C++ functions in this file with their CvUnit.cpp line
//! ranges; all of them are ported. `CvUnit::setXY` itself (called here by `move`,
//! `jumpToNearestValidPlot` and `withdrawlToNearestValidPlot`) lives in `unit/lifecycle.rs`.
//!
//! Helpers are module-level functions (not `impl Game` methods) so that they cannot clash with
//! helpers of the other `unit/*.rs` files.

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

/// C++ `getGroup()` at call sites that dereference it (every unit on the map has a group).
fn group_of(g: &Game, u: UnitId) -> GroupId {
    g.units[u].group.expect("CvUnit::getGroup: unit without a selection group")
}

/// C++ `getTransportUnit()` (`::getUnit(m_transportUnit)`: NULL once the transport is gone).
fn transport_of(g: &Game, u: UnitId) -> Option<UnitId> {
    g.units[u].transport_unit.filter(|&t| g.units.contains(t))
}

/// C++ `::getCity(IDInfo)` on a plot's city field: NULL for a city that no longer exists.
fn live_city(g: &Game, city: Option<CityId>) -> Option<CityId> {
    city.filter(|&c| g.cities.contains(c))
}

/// C++ `(InvisibleTypes)GC.getDefineINT("INVISIBLE_TYPE")` (INVISIBLE_LAND in FFH2).
fn invisible_type_define(g: &Game) -> Option<InvisibleType> {
    opt_from_i32(g.d.defines.invisible_type)
}

/// C++ `NO_TECH == eTech || !GET_TEAM(eTeam).isHasTech(eTech)`: `team` lacks the tech that makes
/// an impassable terrain or feature passable.
fn lacks_passable_tech(g: &Game, team: TeamId, tech: Option<TechType>) -> bool {
    match tech {
        None => true,
        Some(tech) => !g.team_is_has_tech(team, Some(tech)),
    }
}

/// C++ `CvPlot::isActiveVisible(false)`: visible to the active (interface) team, which does not
/// exist when headless (`isVisible(NO_TEAM)` is false).
fn plot_is_active_visible(g: &Game, p: PlotIdx) -> bool {
    g.active_player.is_some_and(|active| g.plot_is_visible(p, g.players[active].team))
}

/// Python `CvGameUtils.unitCannotMoveInto(argsList)`, called by `CvUnit::canMoveInto` only when
/// `USE_UNIT_CANNOT_MOVE_INTO_CALLBACK` is set (FFH2 leaves it off). FFH2 returns False.
fn py_unit_cannot_move_into(_g: &Game, _player: PlayerId, _u: UnitId, _x: i32, _y: i32) -> bool {
    false
}

impl Game {
    /// C++ `CvUnit::getPathEndTurnPlot` (CvUnit.cpp:2914-2917).
    /// C++ `getGroup()->getPathEndTurnPlot()` (last path generated for the group).
    pub fn unit_get_path_end_turn_plot(&self, u: UnitId) -> Option<PlotIdx> {
        self.group_get_path_end_turn_plot(group_of(self, u))
    }

    /// C++ `CvUnit::generatePath` (CvUnit.cpp:2920-2923).
    /// C++ `getGroup()->generatePath(plot(), pToPlot, iFlags, bReuse, piPathTurns)`; `flags` are `MOVE_*`.
    pub fn unit_generate_path(
        &self,
        u: UnitId,
        to: PlotIdx,
        flags: i32,
        reuse: bool,
        path_turns: Option<&mut i32>,
    ) -> bool {
        let from = self.unit_plot(u);
        let result = self.group_generate_path(group_of(self, u), from, to, flags, reuse);

        // C++ `CvSelectionGroup::generatePath`: `*piPathTurns = MAX_INT` unless a path was found.
        if let Some(path_turns) = path_turns {
            *path_turns = result.unwrap_or(MAX_INT);
        }

        result.is_some()
    }

    /// C++ `CvUnit::canEnterTerritory` (CvUnit.cpp:2926-2987).
    pub fn unit_can_enter_territory(&self, u: UnitId, team: Option<TeamId>, ignore_right_of_passage: bool) -> bool {
        let our_team = self.unit_get_team(u);

        if self.team_is_friendly_territory(our_team, team) {
            return true;
        }

        let Some(team) = team else {
            return true;
        };

        if self.unit_is_enemy(u, Some(team), None) {
            return true;
        }

        if self.unit_is_rival_territory(u) {
            return true;
        }

        if self.unit_always_invisible(u) {
            return true;
        }

        if !ignore_right_of_passage {
            if self.team_is_open_borders(our_team, team) {
                return true;
            }
        }

        //FfH: Added by Kael 09/02/2007 (so hidden nationality units can enter all territories)
        if self.unit_is_hidden_nationality(u) {
            return true;
        }
        // (so barbarians can enter player areas they are at peace with an vice versa)
        if self.team_is_barbarian(team) {
            return true;
        }
        if self.team_is_barbarian(our_team) {
            return true;
        }
        let owner = self.units[u].owner;
        if self.players[owner].declaring_war {
            if let Some(state_religion) = self.player_get_state_religion(owner) {
                if self.d.religions[state_religion].sneak_attack {
                    return true;
                }
            }
        }
        //FfH: End Add

        false
    }

    /// C++ `CvUnit::canEnterArea` (CvUnit.cpp:2990-3009).
    pub fn unit_can_enter_area(
        &self,
        u: UnitId,
        team: Option<TeamId>,
        area: Option<AreaId>,
        ignore_right_of_passage: bool,
    ) -> bool {
        if !self.unit_can_enter_territory(u, team, ignore_right_of_passage) {
            return false;
        }

        if self.unit_is_barbarian(u) && DomainType::DOMAIN_LAND == self.unit_get_domain_type(u) {
            if let Some(team) = team {
                if team != self.unit_get_team(u) {
                    if let Some(area) = area {
                        if self.area_is_border_obstacle(area, team) {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    /// C++ `CvUnit::getDeclareWarMove` (CvUnit.cpp:3013-3057).
    /// Returns the team to declare war on to move into `plot` (human players).
    pub fn unit_get_declare_war_move(&self, u: UnitId, plot: PlotIdx) -> Option<TeamId> {
        // (C++ FAssert(isHuman()))
        let domain = self.unit_get_domain_type(u);

        if domain != DomainType::DOMAIN_AIR {
            let team = self.unit_get_team(u);
            let plot_area = self.map.plots[plot].area;

            if let Some(revealed_team) = self.plot_get_revealed_team(plot, team) {
                if !self.unit_can_enter_area(u, Some(revealed_team), plot_area, false)
                    || (domain == DomainType::DOMAIN_SEA
                        && !self.unit_can_cargo_enter_area(u, Some(revealed_team), plot_area, false)
                        && self.group_is_amphib_plot(group_of(self, u), plot))
                {
                    //FfH: Modified by Kael 03/29/2009
                    // C++ `canDeclareWar(pPlot->getTeam())` takes the plot's actual team.
                    // C++ quirk: that is NO_TEAM when the revealed owner is out of date (an
                    // out-of-range `GET_TEAM` read); here such a plot gives no war declaration.
                    if self.unit_is_always_hostile(u, Some(plot))
                        || self.plot_get_team(plot).is_some_and(|plot_team| self.team_can_declare_war(team, plot_team))
                    {
                        return Some(revealed_team);
                    }
                }
            } else if plot_is_active_visible(self, plot) {
                if self.unit_can_move_into(u, plot, true, true, true) {
                    let owner = self.units[u].owner.i();
                    let always_hostile = self.unit_is_always_hostile(u, Some(plot)) as i32;
                    let unit = self.plot_plot_check(
                        plot,
                        Game::puf_can_declare_war,
                        owner,
                        always_hostile,
                        None,
                        None,
                        Some(Game::puf_is_visible),
                        owner,
                        -1,
                    );

                    if let Some(unit) = unit {
                        return Some(self.unit_get_team(unit));
                    }
                }
            }
        }

        None
    }

    /// C++ `CvUnit::willRevealByMove` (CvUnit.cpp:3059-3078).
    pub fn unit_will_reveal_by_move(&self, u: UnitId, plot: PlotIdx) -> bool {
        // (`visibilityRange()` is that of the unit's current plot, as in C++.)
        let visibility_range = self.unit_visibility_range(u);
        let range = visibility_range + 1;
        let team = self.unit_get_team(u);
        let (x, y) = self.map.xy(plot);

        for i in -range..=range {
            for j in -range..=range {
                if let Some(loop_plot) = self.plot_xy(x, y, i, j) {
                    if !self.map.plots[loop_plot].revealed[team]
                        && self.plot_can_see_plot(plot, loop_plot, team, visibility_range, None)
                    {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// C++ `CvUnit::canMoveInto` (CvUnit.cpp:3080-3444).
    /// Ends with the `unitCannotMoveInto` Python callback when `USE_UNIT_CANNOT_MOVE_INTO_CALLBACK` is set.
    pub fn unit_can_move_into(
        &self,
        u: UnitId,
        plot: PlotIdx,
        attack: bool,
        declare_war: bool,
        ignore_load: bool,
    ) -> bool {
        let d = self.d;
        let info = &d.units[self.units[u].unit_type];
        let owner = self.units[u].owner;

        if self.unit_at_plot(u, plot) {
            return false;
        }

        if self.plot_is_impassable(plot) {
            if !self.unit_can_move_impassable(u) {
                return false;
            }
        }

        // Cannot move around in unrevealed land freely
        if info.no_reveal_map && self.unit_will_reveal_by_move(u, plot) {
            return false;
        }

        if d.defines.use_spies_no_enter_borders != 0 {
            if self.unit_is_spy(u) {
                if let Some(plot_owner) = self.map.plots[plot].owner {
                    if !self.player_can_spies_enter_borders(owner, plot_owner) {
                        return false;
                    }
                }
            }
        }

        let team = self.unit_get_team(u);
        let domain = self.unit_get_domain_type(u);

        let plot_area = self.map.plots[plot].area;
        let mut plot_team = self.plot_get_team(plot);
        let mut can_enter_area = self.unit_can_enter_area(u, plot_team, plot_area, false);
        if can_enter_area {
            if let Some(feature) = self.map.plots[plot].feature_type {
                if info.feature_impassable[feature] {
                    if lacks_passable_tech(self, team, info.feature_passable_tech[feature]) {
                        // sea units can enter impassable in own cultural borders
                        if DomainType::DOMAIN_SEA != domain || self.plot_get_team(plot) != Some(team) {
                            return false;
                        }
                    }
                }
            }

            // UNOFFICIAL_PATCH 09/17/09 (TC01 & jdog5000): always check terrain also (BtS only
            // did in an `else` of the feature test).
            // (C++ indexes with the plot's terrain unchecked; every map plot has one.)
            if let Some(terrain) = self.map.plots[plot].terrain_type {
                if info.terrain_impassable[terrain] {
                    if lacks_passable_tech(self, team, info.terrain_passable_tech[terrain]) {
                        // sea units can enter impassable in own cultural borders
                        if DomainType::DOMAIN_SEA != domain || self.plot_get_team(plot) != Some(team) {
                            if ignore_load || !self.unit_can_load(u, plot) {
                                return false;
                            }
                        }
                    }
                }
            }
        }

        match domain {
            DomainType::DOMAIN_SEA => {
                if !self.plot_is_water(plot) && !self.unit_can_move_all_terrain(u) {
                    if !self.plot_is_friendly_city(plot, u, true) || !self.plot_is_coastal_land(plot, -1) {
                        return false;
                    }
                }
            }

            DomainType::DOMAIN_AIR => {
                if !attack {
                    let mut valid = false;

                    if self.plot_is_friendly_city(plot, u, true) {
                        valid = true;

                        if info.air_unit_cap > 0 {
                            if self.plot_air_unit_space_available(plot, team) <= 0 {
                                valid = false;
                            }
                        }
                    }

                    if !valid {
                        if ignore_load || !self.unit_can_load(u, plot) {
                            return false;
                        }
                    }
                }
            }

            DomainType::DOMAIN_LAND => {
                if self.plot_is_water(plot) && !self.unit_can_move_all_terrain(u)
                    //FfH: Added by Kael 08/27/2007 (for boarding)
                    && !(attack && self.unit_is_boarding(u))
                {
                    if !self.plot_is_city(plot, false, None) || 0 == d.defines.land_units_can_attack_water_cities {
                        if ignore_load
                            || !self.unit_is_human(u)
                            || self.plot_is_water(self.unit_plot(u))
                            || !self.unit_can_load(u, plot)
                        {
                            return false;
                        }
                    }
                }
            }

            DomainType::DOMAIN_IMMOBILE => {
                return false;
            }

            // (C++ FAssert(false))
            _ => {}
        }

        //FfH: Modified by Kael 08/04/2007 (So owned animals dont have limited movement)
        if self.unit_is_animal(u) && self.unit_is_barbarian(u) {
            if self.plot_is_owned(plot) {
                return false;
            }

            if !attack {
                if self.plot_get_bonus_type(plot, None).is_some() {
                    return false;
                }

                if self.map.plots[plot].improvement_type.is_some() {
                    return false;
                }

                if !self.map.plots[plot].units.is_empty() {
                    return false;
                }
            }
        }

        if self.unit_is_no_capture(u) {
            if !attack {
                if self.plot_is_enemy_city(plot, u) {
                    return false;
                }
            }
        }

        if attack {
            if self.units[u].made_attack && !self.unit_is_blitz(u) {
                return false;
            }
        }

        if domain == DomainType::DOMAIN_AIR {
            if attack {
                if !self.unit_can_air_strike(u, plot) {
                    return false;
                }
            }
        } else {
            if self.unit_can_attack(u) {
                if attack || !self.unit_can_coexist_with_enemy_unit(u, None) {
                    if !self.unit_is_human(u) || self.plot_is_visible(plot, team) {
                        if self.plot_is_visible_enemy_unit_for_unit(plot, u) != attack {
                            if !declare_war
                                || (self.plot_is_visible_other_unit(plot, owner) != attack
                                    && !(attack
                                        && live_city(self, self.map.plots[plot].plot_city).is_some()
                                        && !self.unit_is_no_capture(u)))
                            {
                                return false;
                            }
                        }
                    }
                }

                if attack {
                    let defender = self.plot_get_best_defender(plot, None, Some(owner), Some(u), true, false, false);
                    if let Some(defender) = defender {
                        if !self.unit_can_attack_unit(u, defender) {
                            return false;
                        }
                    }
                }
            } else {
                if attack {
                    return false;
                }

                if !self.unit_can_coexist_with_enemy_unit(u, None) {
                    if !self.unit_is_human(u) || self.plot_is_visible(plot, team) {
                        if self.plot_is_enemy_city(plot, u) {
                            return false;
                        }

                        if self.plot_is_visible_enemy_unit_for_unit(plot, u) {
                            return false;
                        }
                    }
                }
            }

            if self.unit_is_human(u) {
                plot_team = self.plot_get_revealed_team(plot, team);
                can_enter_area = self.unit_can_enter_area(u, plot_team, plot_area, false);
            }

            if !can_enter_area {
                // C++ FAssert(ePlotTeam != NO_TEAM): `canEnterTerritory(NO_TEAM)` is always true and
                // the border-obstacle test needs a team, so a plot that cannot be entered has one.
                let plot_team = plot_team.expect("CvUnit::canMoveInto: area not enterable without a team");

                if !self.team_can_declare_war(team, plot_team) {
                    return false;
                }

                if self.unit_is_human(u) {
                    if !declare_war {
                        return false;
                    }
                } else if self.team_ai_is_sneak_attack_ready(team, plot_team) {
                    if !self.group_ai_is_declare_war(group_of(self, u), Some(plot)) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }

        //FfH: Added by Kael 09/02/2007
        if let Some(feature) = self.map.plots[plot].feature_type {
            if let Some(require_resist) = d.features[feature].require_resist {
                if self.unit_get_damage_type_resist(u, require_resist) < d.defines.feature_require_resist_amount {
                    return false;
                }
            }
        }
        if let Some(plot_owner) = self.map.plots[plot].owner {
            if self.plot_get_team(plot) != Some(team) {
                if self.players[plot_owner].sanctuary_timer != 0 {
                    return false;
                }
            }
        }
        if self.units[u].level < self.map.plots[plot].min_level {
            return false;
        }
        if self.map.plots[plot].move_disabled_human {
            if self.unit_is_human(u) {
                return false;
            }
        }
        if self.map.plots[plot].move_disabled_ai {
            if !self.unit_is_human(u) {
                return false;
            }
        }
        //FfH: End Add

        if d.defines.use_unit_cannot_move_into_callback != 0 {
            // Python Override
            let (x, y) = self.map.xy(plot);
            if py_unit_cannot_move_into(self, owner, u, x, y) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::canMoveOrAttackInto` (CvUnit.cpp:3447-3450).
    pub fn unit_can_move_or_attack_into(&self, u: UnitId, plot: PlotIdx, declare_war: bool) -> bool {
        self.unit_can_move_into(u, plot, false, declare_war, false) || self.unit_can_move_into(u, plot, true, declare_war, false)
    }

    /// C++ `CvUnit::canMoveThrough` (CvUnit.cpp:3453-3456).
    pub fn unit_can_move_through(&self, u: UnitId, plot: PlotIdx) -> bool {
        self.unit_can_move_into(u, plot, false, false, true)
    }

    /// C++ `CvUnit::move` (CvUnit.cpp:3598-3645).
    /// C++ `bShow` (animate the move) is dropped. The `OnUnitChangeTo` feature change is rules;
    /// the bird effect (async rand) is graphics.
    pub fn unit_move(&mut self, u: UnitId, plot: PlotIdx) {
        // (C++ FAssert(canMoveOrAttackInto(pPlot) || isMadeAttack()))
        let old_plot = self.unit_plot_opt(u);

        let cost = self.plot_movement_cost(plot, u, self.unit_plot(u));
        self.unit_change_moves(u, cost);

        // C++ `setXY(x, y, true, true, bShow && pPlot->isVisibleToWatchingHuman(), bShow)`.
        let (x, y) = self.map.xy(plot);
        self.unit_set_xy(u, x, y, true);

        //change feature
        if let Some(feature) = self.map.plots[plot].feature_type {
            // C++ resolves the `getOnUnitChangeTo()` type string here (an empty string changes
            // nothing); ffh-data resolves it at load.
            if let Some(new_feature) = self.d.features[feature].on_unit_change_to {
                self.plot_set_feature_type(plot, Some(new_feature));
            }
        }

        // (Active player, unowned plot: the "birds scatter" effect -- async rand and graphics.)

        //FfH: Modified by Kael 08/02/2008
        if self.d.defines.use_on_unit_move_callback != 0 {
            // (C++ assumes the unit survived the move.)
            if self.units.contains(u) {
                self.ev_unit_move(plot, u, old_plot);
            }
        }
        //FfH: End Modify
    }

    /// C++ `CvUnit::jumpToNearestValidPlot` (CvUnit.cpp:3648-3727).
    /// Returns false if the unit was killed (no valid plot).
    pub fn unit_jump_to_nearest_valid_plot(&mut self, u: UnitId) -> bool {
        // (C++ FAssertMsg: neither attacking nor fighting)
        let (x, y) = (self.units[u].x, self.units[u].y);
        let owner = self.units[u].owner;
        let team = self.unit_get_team(u);
        let domain = self.unit_get_domain_type(u);

        let nearest_city = self.map_find_city(x, y, Some(owner), None, true, false, None, None, None);

        let mut best_value = MAX_INT;
        let mut best_plot: Option<PlotIdx> = None;

        for loop_plot in self.map.plot_ids() {
            if self.plot_is_valid_domain_for_location(loop_plot, u) {
                if self.unit_can_move_into(u, loop_plot, false, false, false) {
                    let loop_team = self.plot_get_team(loop_plot);
                    if self.unit_can_enter_area(u, loop_team, self.map.plots[loop_plot].area, false)
                        && !self.unit_is_enemy(u, loop_team, Some(loop_plot))
                    {
                        // (C++ FAssertMsg(!atPlot(pLoopPlot)))
                        if domain != DomainType::DOMAIN_AIR || self.plot_is_friendly_city(loop_plot, u, true) {
                            if self.map.plots[loop_plot].revealed[team] {
                                let (loop_x, loop_y) = self.map.xy(loop_plot);
                                let mut value = self.plot_distance(x, y, loop_x, loop_y) * 2;

                                if let Some(city) = nearest_city {
                                    let (city_x, city_y) = (self.cities[city].x, self.cities[city].y);
                                    value += self.plot_distance(loop_x, loop_y, city_x, city_y);
                                }

                                if domain == DomainType::DOMAIN_SEA && !self.plot_is_water(self.unit_plot(u)) {
                                    if !self.plot_is_water(loop_plot)
                                        || !self.plot_is_adjacent_to_area(loop_plot, self.unit_area(u))
                                    {
                                        value *= 3;
                                    }
                                } else if self.map.plots[loop_plot].area != Some(self.unit_area(u)) {
                                    value *= 3;
                                }

                                if value < best_value {
                                    best_value = value;
                                    best_plot = Some(loop_plot);
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut valid = true;
        if let Some(best_plot) = best_plot {
            let (best_x, best_y) = self.map.xy(best_plot);
            self.unit_set_xy(u, best_x, best_y, false);
        } else {
            self.unit_kill(u, false, None);
            valid = false;
        }

        valid
    }

    /// C++ `CvUnit::canLoadUnit` (CvUnit.cpp:4094-4164).
    pub fn unit_can_load_unit(&self, u: UnitId, transport: UnitId, plot: PlotIdx) -> bool {
        if transport == u {
            return false;
        }

        if self.unit_get_team(transport) != self.unit_get_team(u) {
            return false;
        }

        // UNOFFICIAL_PATCH 10/30/09 (Mongoose & jdog5000): from Mongoose SDK
        if self.unit_is_cargo(u) {
            return false;
        }

        if self.units[u].cargo > 0 {
            return false;
        }

        if self.unit_is_cargo(transport) {
            return false;
        }

        let special_unit = self.unit_get_special_unit_type(u);
        if self.unit_cargo_space_available(transport, special_unit, Some(self.unit_get_domain_type(u))) == 0 {
            return false;
        }

        if !self.unit_at_plot(transport, plot) {
            return false;
        }

        //FfH Hidden Nationality: Modified by Kael 08/27/2007
        if self.unit_is_hidden_nationality(u) != self.unit_is_hidden_nationality(transport) {
            return false;
        }

        if let Some(special_unit) = special_unit {
            if self.d.special_units[special_unit].city_load {
                if !self.plot_is_city(plot, true, Some(self.unit_get_team(u))) {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvUnit::loadUnit` (CvUnit.cpp:4167-4175).
    pub fn unit_load_unit(&mut self, u: UnitId, transport: UnitId) {
        if !self.unit_can_load_unit(u, transport, self.unit_plot(u)) {
            return;
        }

        self.unit_set_transport_unit(u, Some(transport));
    }

    /// C++ `CvUnit::shouldLoadOnMove` (CvUnit.cpp:4177-4233).
    pub fn unit_should_load_on_move(&self, u: UnitId, plot: PlotIdx) -> bool {
        if self.unit_is_cargo(u) {
            return false;
        }

        match self.unit_get_domain_type(u) {
            DomainType::DOMAIN_LAND => {
                // UNOFFICIAL_PATCH 10/30/09 (Mongoose SDK): BtS tested only `isWater()`.
                if self.plot_is_water(plot) && !self.unit_can_move_all_terrain(u) {
                    return true;
                }
            }
            DomainType::DOMAIN_AIR => {
                if !self.plot_is_friendly_city(plot, u, true) {
                    return true;
                }

                if self.d.units[self.units[u].unit_type].air_unit_cap > 0 {
                    if self.plot_air_unit_space_available(plot, self.unit_get_team(u)) <= 0 {
                        return true;
                    }
                }
            }
            _ => {}
        }

        // (C++ indexes with the plot's terrain unchecked; every map plot has one.)
        if let Some(terrain) = self.map.plots[plot].terrain_type {
            let info = &self.d.units[self.units[u].unit_type];
            if info.terrain_impassable[terrain] {
                if lacks_passable_tech(self, self.unit_get_team(u), info.terrain_passable_tech[terrain]) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvUnit::canLoad` (CvUnit.cpp:4236-4255).
    pub fn unit_can_load(&self, u: UnitId, plot: PlotIdx) -> bool {
        for &loop_unit in &self.map.plots[plot].units {
            if self.unit_can_load_unit(u, loop_unit, plot) {
                return true;
            }
        }

        false
    }

    /// C++ `CvUnit::load` (CvUnit.cpp:4258-4296).
    pub fn unit_load(&mut self, u: UnitId) {
        let plot = self.unit_plot(u);

        if !self.unit_can_load(u, plot) {
            return;
        }

        for pass in 0..2 {
            for loop_unit in self.map.plots[plot].units.clone() {
                if self.unit_can_load_unit(u, loop_unit, plot) {
                    let candidate = if pass == 0 {
                        self.units[loop_unit].owner == self.units[u].owner
                    } else {
                        self.unit_get_team(loop_unit) == self.unit_get_team(u)
                    };

                    if candidate {
                        self.unit_set_transport_unit(u, Some(loop_unit));
                        break;
                    }
                }
            }

            if self.unit_is_cargo(u) {
                break;
            }
        }
    }

    /// C++ `CvUnit::canUnload` (CvUnit.cpp:4299-4337).
    pub fn unit_can_unload(&self, u: UnitId) -> bool {
        if transport_of(self, u).is_none() {
            return false;
        }

        // C++ `CvPlot& kPlot = *(plot())` (only used from here on).
        let plot = self.unit_plot(u);

        if !self.plot_is_valid_domain_for_location(plot, u) {
            return false;
        }

        if self.unit_get_domain_type(u) == DomainType::DOMAIN_AIR {
            if self.plot_is_friendly_city(plot, u, true) {
                let team = self.unit_get_team(u);
                let num_air_units = self.plot_count_num_air_units(plot, team);
                if let Some(city) = live_city(self, self.map.plots[plot].plot_city) {
                    if num_air_units >= self.city_air_unit_capacity(city, team) {
                        return false;
                    }
                } else if num_air_units >= self.d.defines.city_air_unit_capacity {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvUnit::unload` (CvUnit.cpp:4340-4348).
    pub fn unit_unload(&mut self, u: UnitId) {
        if !self.unit_can_unload(u) {
            return;
        }

        self.unit_set_transport_unit(u, None);
    }

    /// C++ `CvUnit::canUnloadAll` (CvUnit.cpp:4351-4359).
    pub fn unit_can_unload_all(&self, u: UnitId) -> bool {
        self.units[u].cargo != 0
    }

    /// C++ `CvUnit::unloadAll` (CvUnit.cpp:4362-4384).
    pub fn unit_unload_all(&mut self, u: UnitId) {
        if !self.unit_can_unload_all(u) {
            return;
        }

        for cargo in self.unit_get_cargo_units(u) {
            if self.unit_can_unload(cargo) {
                self.unit_set_transport_unit(cargo, None);
            } else {
                // (C++ FAssert(isHuman() || pCargo->getDomainType() == DOMAIN_AIR))
                let cargo_group = group_of(self, cargo);
                self.group_set_activity_type(cargo_group, ActivityType::ACTIVITY_AWAKE);
            }
        }
    }

    /// C++ `CvUnit::getInvisibleType` (CvUnit.cpp:8519-8557).
    /// FfH: hidden by the plot owner (`isHideUnits`), invisible-from-promotion, else the
    /// `invisible_type` field.
    pub fn unit_get_invisible_type(&self, u: UnitId) -> Option<InvisibleType> {
        let unit = &self.units[u];

        //FfH: Modified by Kael 06/20/2010
        if unit.invisible_type.is_none() {
            if let Some(plot) = self.unit_plot_opt(u) {
                if let Some(plot_owner) = self.map.plots[plot].owner {
                    if self.players[plot_owner].hide_units && !unit.ignore_hide {
                        if self.plot_get_team(plot) == Some(self.unit_get_team(u)) {
                            if !self.plot_is_city(plot, false, None) {
                                return invisible_type_define(self);
                            }
                        }
                    }
                }
            }
        }

        if self.unit_is_invisible_from_promotion(u) {
            if self.d.units[unit.unit_type].equipment_promotion.is_some() {
                // C++ `(InvisibleTypes)2`: INVISIBLE_ALL.
                return Some(InvisibleType(2));
            } else {
                return invisible_type_define(self);
            }
        }

        unit.invisible_type
        //FfH: End Modify
    }

    /// C++ `CvUnit::getNumSeeInvisibleTypes` (CvUnit.cpp:8559-8570).
    pub fn unit_get_num_see_invisible_types(&self, u: UnitId) -> i32 {
        //FfH: Added by Kael 12/07/2008
        if self.unit_is_see_invisible(u) {
            return 1;
        }
        //FfH: End Add

        self.d.units[self.units[u].unit_type].see_invisible_types.len() as i32
    }

    /// C++ `CvUnit::getSeeInvisibleType` (CvUnit.cpp:8572-8583).
    pub fn unit_get_see_invisible_type(&self, u: UnitId, i: i32) -> InvisibleType {
        //FfH: Added by Kael 12/07/2008
        if self.unit_is_see_invisible(u) {
            return invisible_type_define(self).expect("INVISIBLE_TYPE define is not an invisible type");
        }
        //FfH: End Add

        self.d.units[self.units[u].unit_type].see_invisible_types[i as usize]
    }

    /// C++ `CvUnit::visibilityRange` (CvUnit.cpp:8604-8618).
    pub fn unit_visibility_range(&self, u: UnitId) -> i32 {
        //FfH: Modified by Kael 08/10/2007
        let mut range = self.d.defines.unit_visibility_range;
        range += self.units[u].extra_visibility_range;
        if let Some(improvement) = self.map.plots[self.unit_plot(u)].improvement_type {
            range += self.d.improvements[improvement].visibility_change;
        }
        range
        //FfH: End Modify
    }

    /// C++ `CvUnit::baseMoves` (CvUnit.cpp:8621-8624).
    pub fn unit_base_moves(&self, u: UnitId) -> i32 {
        let unit = &self.units[u];
        let team = self.unit_get_team(u);
        self.d.units[unit.unit_type].moves + unit.extra_moves + self.teams[team].extra_moves[self.unit_get_domain_type(u).idx()]
    }

    /// C++ `CvUnit::maxMoves` (CvUnit.cpp:8627-8630).
    pub fn unit_max_moves(&self, u: UnitId) -> i32 {
        self.unit_base_moves(u) * self.d.defines.move_denominator
    }

    /// C++ `CvUnit::movesLeft` (CvUnit.cpp:8633-8636).
    pub fn unit_moves_left(&self, u: UnitId) -> i32 {
        std::cmp::max(0, self.unit_max_moves(u) - self.units[u].moves)
    }

    /// C++ `CvUnit::canMove` (CvUnit.cpp:8639-8657).
    pub fn unit_can_move(&self, u: UnitId) -> bool {
        if self.unit_is_dead(u) {
            return false;
        }

        if self.units[u].moves >= self.unit_max_moves(u) {
            return false;
        }

        if self.unit_get_immobile_timer(u) > 0 {
            return false;
        }

        true
    }

    /// C++ `CvUnit::hasMoved` (CvUnit.cpp:8660-8663).
    pub fn unit_has_moved(&self, u: UnitId) -> bool {
        self.units[u].moves > 0
    }

    /// C++ `CvUnit::canCoexistWithEnemyUnit` (CvUnit.cpp:8885-8903).
    pub fn unit_can_coexist_with_enemy_unit(&self, u: UnitId, team: Option<TeamId>) -> bool {
        let Some(team) = team else {
            return self.unit_always_invisible(u);
        };

        self.unit_is_invisible(u, team, true)
    }

    /// C++ `CvUnit::alwaysInvisible` (CvUnit.cpp:10165-10168).
    pub fn unit_always_invisible(&self, u: UnitId) -> bool {
        self.d.units[self.units[u].unit_type].invisible
    }

    /// C++ `CvUnit::canMoveImpassable` (CvUnit.cpp:10198-10209).
    pub fn unit_can_move_impassable(&self, u: UnitId) -> bool {
        //FfH Flying: Added by Kael 07/30/2007
        if self.unit_is_flying(u) {
            return true;
        }
        //FfH: End Add

        self.d.units[self.units[u].unit_type].can_move_impassable
    }

    /// C++ `CvUnit::canMoveAllTerrain` (CvUnit.cpp:10211-10222).
    pub fn unit_can_move_all_terrain(&self, u: UnitId) -> bool {
        //FfH Flying: Added by Kael 07/30/2007
        if self.unit_is_flying(u) || self.unit_is_water_walking(u) {
            return true;
        }
        //FfH: End Add

        self.d.units[self.units[u].unit_type].can_move_all_terrain
    }

    /// C++ `CvUnit::flatMovementCost` (CvUnit.cpp:10224-10235).
    pub fn unit_flat_movement_cost(&self, u: UnitId) -> bool {
        //FfH Flying: Added by Kael 07/30/2007
        if self.unit_is_flying(u) {
            return true;
        }
        //FfH: End Add

        self.d.units[self.units[u].unit_type].flat_movement_cost
    }

    /// C++ `CvUnit::ignoreTerrainCost` (CvUnit.cpp:10238-10249).
    pub fn unit_ignore_terrain_cost(&self, u: UnitId) -> bool {
        //FfH Flying: Added by Kael 07/30/2007
        if self.unit_is_flying(u) {
            return true;
        }
        //FfH: End Add

        self.d.units[self.units[u].unit_type].ignore_terrain_cost
    }

    /// C++ `CvUnit::isNeverInvisible` (CvUnit.cpp:10252-10255).
    pub fn unit_is_never_invisible(&self, u: UnitId) -> bool {
        !self.unit_always_invisible(u) && self.unit_get_invisible_type(u).is_none()
    }

    /// C++ `CvUnit::isInvisible` (CvUnit.cpp:10258-10323).
    /// C++ `isInvisible(eTeam, bDebug, bCheckCargo)`: `bDebug` (debug-mode view) is dropped;
    /// C++ default `bCheckCargo` is true.
    pub fn unit_is_invisible(&self, u: UnitId, team: TeamId, check_cargo: bool) -> bool {
        // (`bDebug && isDebugMode()`: debug view only)
        let our_team = self.unit_get_team(u);

        if our_team == team {
            return false;
        }

        //FfH: Added by Kael 04/11/2008
        let plot = self.unit_plot_opt(u);
        if let Some(plot) = plot {
            if self.plot_is_city(plot, false, None) {
                if Some(our_team) == self.plot_get_team(plot) {
                    return false;
                }
            }
            if let Some(plot_owner) = self.map.plots[plot].owner {
                let plot_team = self.players[plot_owner].team;
                if plot_team != our_team {
                    if self.players[plot_owner].see_invisible {
                        return false;
                    }
                }
                if plot_team == our_team {
                    if self.players[plot_owner].hide_units && !self.units[u].ignore_hide {
                        return true;
                    }
                }
            }
        }
        //FfH: End Add

        if self.unit_always_invisible(u) {
            return true;
        }

        if check_cargo && self.unit_is_cargo(u) {
            return true;
        }

        let Some(invisible_type) = self.unit_get_invisible_type(u) else {
            return false;
        };

        //FfH: Added by Kael 01/16/2009
        let Some(plot) = plot else {
            return false;
        };
        //FfH: End Add

        !self.plot_is_invisible_visible(plot, team, invisible_type)
    }

    /// C++ `CvUnit::isInvisibleInPlot` (CvUnit.cpp:10326-10380).
    /// FfH variant of `isInvisible` for a given plot; `bDebug` dropped.
    pub fn unit_is_invisible_in_plot(&self, u: UnitId, team: TeamId, plot: Option<PlotIdx>, check_cargo: bool) -> bool {
        // (`bDebug && isDebugMode()`: debug view only)
        let our_team = self.unit_get_team(u);

        if our_team == team {
            return false;
        }

        if let Some(plot) = plot {
            if self.plot_is_city(plot, false, None) {
                if Some(our_team) == self.plot_get_team(plot) {
                    return false;
                }
            }
            if let Some(plot_owner) = self.map.plots[plot].owner {
                let plot_team = self.players[plot_owner].team;
                if plot_team != our_team {
                    if self.players[plot_owner].see_invisible {
                        return false;
                    }
                }
                if plot_team == our_team {
                    if self.players[plot_owner].hide_units && !self.units[u].ignore_hide {
                        return true;
                    }
                }
            }
        }

        if self.unit_always_invisible(u) {
            return true;
        }

        if check_cargo && self.unit_is_cargo(u) {
            return true;
        }

        // (The invisible type is that of the unit's own plot, as in C++.)
        let Some(invisible_type) = self.unit_get_invisible_type(u) else {
            return false;
        };

        let Some(plot) = plot else {
            return false;
        };

        !self.plot_is_invisible_visible(plot, team, invisible_type)
    }

    /// C++ `CvUnit::specialCargo` (CvUnit.cpp:10561-10564).
    pub fn unit_special_cargo(&self, u: UnitId) -> Option<SpecialUnitType> {
        self.d.units[self.units[u].unit_type].special_cargo
    }

    /// C++ `CvUnit::domainCargo` (CvUnit.cpp:10567-10570).
    pub fn unit_domain_cargo(&self, u: UnitId) -> Option<DomainType> {
        self.d.units[self.units[u].unit_type].domain_cargo
    }

    /// C++ `CvUnit::changeCargoSpace` (CvUnit.cpp:10578-10586).
    pub fn unit_change_cargo_space(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            self.units[u].cargo_capacity += change;
        }
    }

    /// C++ `CvUnit::isFull` (CvUnit.cpp:10588-10591).
    pub fn unit_is_full(&self, u: UnitId) -> bool {
        let unit = &self.units[u];
        unit.cargo >= unit.cargo_capacity
    }

    /// C++ `CvUnit::cargoSpaceAvailable` (CvUnit.cpp:10594-10613).
    /// C++ defaults: `NO_SPECIALUNIT`, `NO_DOMAIN`.
    pub fn unit_cargo_space_available(
        &self,
        u: UnitId,
        special_cargo: Option<SpecialUnitType>,
        domain_cargo: Option<DomainType>,
    ) -> i32 {
        if let Some(our_special_cargo) = self.unit_special_cargo(u) {
            if Some(our_special_cargo) != special_cargo {
                return 0;
            }
        }

        if let Some(our_domain_cargo) = self.unit_domain_cargo(u) {
            if Some(our_domain_cargo) != domain_cargo {
                return 0;
            }
        }

        let unit = &self.units[u];
        std::cmp::max(0, unit.cargo_capacity - unit.cargo)
    }

    /// C++ `CvUnit::hasCargo` (CvUnit.cpp:10616-10619).
    pub fn unit_has_cargo(&self, u: UnitId) -> bool {
        self.units[u].cargo > 0
    }

    /// C++ `CvUnit::canCargoAllMove` (CvUnit.cpp:10622-10650).
    pub fn unit_can_cargo_all_move(&self, u: UnitId) -> bool {
        let plot = self.unit_plot(u);

        for &loop_unit in &self.map.plots[plot].units {
            // C++ `pLoopUnit->getTransportUnit() == this`.
            if self.units[loop_unit].transport_unit == Some(u) {
                if self.unit_get_domain_type(loop_unit) == DomainType::DOMAIN_LAND {
                    if !self.unit_can_move(loop_unit) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// C++ `CvUnit::canCargoEnterArea` (CvUnit.cpp:10652-10673).
    pub fn unit_can_cargo_enter_area(
        &self,
        u: UnitId,
        team: Option<TeamId>,
        area: Option<AreaId>,
        ignore_right_of_passage: bool,
    ) -> bool {
        let plot = self.unit_plot(u);

        for &loop_unit in &self.map.plots[plot].units {
            if self.units[loop_unit].transport_unit == Some(u) {
                if !self.unit_can_enter_area(loop_unit, team, area, ignore_right_of_passage) {
                    return false;
                }
            }
        }

        true
    }

    /// C++ `CvUnit::getUnitAICargo` (CvUnit.cpp:10675-10690).
    pub fn unit_get_unit_ai_cargo(&self, u: UnitId, unit_ai: UnitAIType) -> i32 {
        let mut count = 0;

        for cargo in self.unit_get_cargo_units(u) {
            if self.units[cargo].ai_type == Some(unit_ai) {
                count += 1;
            }
        }

        count
    }

    /// C++ `CvUnit::changeMoves` (CvUnit.cpp:11771-11774).
    pub fn unit_change_moves(&mut self, u: UnitId, change: i32) {
        // C++ `setMoves(getMoves() + iChange)`; the rest of setMoves is interface updates.
        self.units[u].moves += change;
    }

    /// C++ `CvUnit::finishMoves` (CvUnit.cpp:11777-11780).
    pub fn unit_finish_moves(&mut self, u: UnitId) {
        // C++ `setMoves(maxMoves())`; the rest of setMoves is interface updates.
        let max_moves = self.unit_max_moves(u);
        self.units[u].moves = max_moves;
    }

    /// C++ `CvUnit::changeCargo` (CvUnit.cpp:11876-11880).
    pub fn unit_change_cargo(&mut self, u: UnitId, change: i32) {
        self.units[u].cargo += change;
    }

    /// C++ `CvUnit::getCargoUnits` (CvUnit.cpp:11882-11902).
    /// Units on this unit's plot whose transport is this unit, in plot order.
    pub fn unit_get_cargo_units(&self, u: UnitId) -> Vec<UnitId> {
        let mut units = Vec::new();

        if self.unit_has_cargo(u) {
            let plot = self.unit_plot(u);
            for &loop_unit in &self.map.plots[plot].units {
                if self.units[loop_unit].transport_unit == Some(u) {
                    units.push(loop_unit);
                }
            }
        }

        // (C++ FAssert(getCargo() == aUnits.size()))
        units
    }

    /// C++ `CvUnit::changeExtraVisibilityRange` (CvUnit.cpp:12100-12111).
    pub fn unit_change_extra_visibility_range(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            let plot = self.unit_plot(u);
            let team = self.unit_get_team(u);

            let range = self.unit_visibility_range(u);
            self.plot_change_adjacent_sight(plot, team, range, false, Some(u), true);

            self.units[u].extra_visibility_range += change;
            // (FAssert(getExtraVisibilityRange() >= 0) removed by Sephi 0.41k: some promotions
            // can reduce visibility.)

            let range = self.unit_visibility_range(u);
            self.plot_change_adjacent_sight(plot, team, range, true, Some(u), true);
        }
    }

    /// C++ `CvUnit::getImmobileTimer` (CvUnit.cpp:12516-12527).
    pub fn unit_get_immobile_timer(&self, u: UnitId) -> i32 {
        // FfH: held units are immobile.
        if self.unit_is_held(u) {
            return 999;
        }
        self.units[u].immobile_timer
    }

    /// C++ `CvUnit::setImmobileTimer` (CvUnit.cpp:12529-12551).
    /// FfH: casts the delayed spell when the timer reaches 0.
    pub fn unit_set_immobile_timer(&mut self, u: UnitId, value: i32) {
        //FfH: Modified by Kael 09/15/2008
        if value != self.unit_get_immobile_timer(u) && !self.unit_is_held(u) {
            self.units[u].immobile_timer = value;
            // (setInfoBarDirty: interface)
            if self.unit_get_immobile_timer(u) == 0 {
                if let Some(spell) = self.units[u].delayed_spell {
                    self.unit_cast(u, spell);
                }
            }
        }
        //FfH: End Modify
    }

    /// C++ `CvUnit::changeImmobileTimer` (CvUnit.cpp:12553-12559).
    pub fn unit_change_immobile_timer(&mut self, u: UnitId, change: i32) {
        if change != 0 {
            let value = std::cmp::max(0, self.unit_get_immobile_timer(u) + change);
            self.unit_set_immobile_timer(u, value);
        }
    }

    /// C++ `CvUnit::isCargo` (CvUnit.cpp:12918-12921).
    pub fn unit_is_cargo(&self, u: UnitId) -> bool {
        // C++ `getTransportUnit() != NULL` (`::getUnit` yields NULL for a dead unit).
        self.units[u].transport_unit.map_or(false, |t| self.units.contains(t))
    }

    /// C++ `CvUnit::setTransportUnit` (CvUnit.cpp:12924-12978).
    pub fn unit_set_transport_unit(&mut self, u: UnitId, transport: Option<UnitId>) {
        let old_transport = transport_of(self, u);

        if old_transport != transport {
            if let Some(old_transport) = old_transport {
                self.unit_change_cargo(old_transport, -1);
            }

            if let Some(transport) = transport {
                // (C++ FAssertMsg: the transport has cargo space for this unit)

                // Because what if a group of 3 tries to get in a transport which can hold 2...
                // (C++ `joinGroup(NULL, true)`: `bRemoveSelected` is interface only.)
                self.unit_join_group(u, None, true);

                self.units[u].transport_unit = Some(transport);

                if self.unit_get_domain_type(u) != DomainType::DOMAIN_AIR {
                    let group = group_of(self, u);
                    self.group_set_activity_type(group, ActivityType::ACTIVITY_SLEEP);
                }

                if self.final_initialized {
                    self.unit_finish_moves(u);
                }

                self.unit_change_cargo(transport, 1);
                let transport_group = group_of(self, transport);
                self.group_set_activity_type(transport_group, ActivityType::ACTIVITY_AWAKE);
            } else {
                self.units[u].transport_unit = None;

                let group = group_of(self, u);
                self.group_set_activity_type(group, ActivityType::ACTIVITY_AWAKE);
            }

            // (_DEBUG builds only: getCargoUnits of both transports)
        }
    }

    /// C++ `CvUnit::isTerrainDoubleMove` (CvUnit.cpp:13064-13069).
    pub fn unit_is_terrain_double_move(&self, u: UnitId, terrain: TerrainType) -> bool {
        self.units[u].terrain_double_move_count[terrain] > 0
    }

    /// C++ `CvUnit::changeTerrainDoubleMoveCount` (CvUnit.cpp:13072-13078).
    pub fn unit_change_terrain_double_move_count(&mut self, u: UnitId, terrain: TerrainType, change: i32) {
        self.units[u].terrain_double_move_count[terrain] += change;
    }

    /// C++ `CvUnit::isFeatureDoubleMove` (CvUnit.cpp:13089-13094).
    pub fn unit_is_feature_double_move(&self, u: UnitId, feature: FeatureType) -> bool {
        self.units[u].feature_double_move_count[feature] > 0
    }

    /// C++ `CvUnit::changeFeatureDoubleMoveCount` (CvUnit.cpp:13097-13103).
    pub fn unit_change_feature_double_move_count(&mut self, u: UnitId, feature: FeatureType, change: i32) {
        self.units[u].feature_double_move_count[feature] += change;
    }

    /// C++ `CvUnit::potentialWarAction` (CvUnit.cpp:13651-13672).
    /// Reads `getGroup()->AI_isDeclareWar(pPlot)` and the team war plan (AI bookkeeping).
    pub fn unit_potential_war_action(&self, u: UnitId, plot: PlotIdx) -> bool {
        let Some(plot_team) = self.plot_get_team(plot) else {
            return false;
        };
        let unit_team = self.unit_get_team(u);

        if self.unit_is_enemy(u, Some(plot_team), Some(plot)) {
            return true;
        }

        if self.group_ai_is_declare_war(group_of(self, u), Some(plot)) && self.teams[unit_team].ai_war_plan[plot_team].is_some() {
            return true;
        }

        false
    }

    /// C++ `CvUnit::getOpenPlot` (CvUnit.cpp:16728-16763).
    /// FfH: nearest empty plot valid for this unit (used by `betray`).
    pub fn unit_get_open_plot(&self, u: UnitId) -> Option<PlotIdx> {
        let (x, y) = (self.units[u].x, self.units[u].y);
        let equipment = self.d.units[self.units[u].unit_type].equipment_promotion.is_some();

        let mut best_plot: Option<PlotIdx> = None;
        let mut best_value = MAX_INT;

        for loop_plot in self.map.plot_ids() {
            if self.plot_is_valid_domain_for_location(loop_plot, u) {
                if self.unit_can_move_into(u, loop_plot, false, false, false) || equipment {
                    if self.map.plots[loop_plot].units.is_empty() {
                        let (loop_x, loop_y) = self.map.xy(loop_plot);
                        let mut value = self.plot_distance(x, y, loop_x, loop_y) * 2;
                        if self.map.plots[loop_plot].area != Some(self.unit_area(u)) {
                            value *= 3;
                        }
                        if value < best_value {
                            best_value = value;
                            best_plot = Some(loop_plot);
                        }
                    }
                }
            }
        }

        best_plot
    }

    /// C++ `CvUnit::withdrawlToNearestValidPlot` (CvUnit.cpp:16805-16864).
    /// FfH: move to the best adjacent valid plot; if none, kill the unit when `kill`.
    pub fn unit_withdrawl_to_nearest_valid_plot(&mut self, u: UnitId, kill: bool) -> bool {
        let (x, y) = (self.units[u].x, self.units[u].y);
        let domain = self.unit_get_domain_type(u);

        let mut best_plot: Option<PlotIdx> = None;
        let mut best_value = MAX_INT;

        for i in -1..=1 {
            for j in -1..=1 {
                if let Some(loop_plot) = self.plot_xy(x, y, i, j) {
                    if self.plot_is_valid_domain_for_location(loop_plot, u) {
                        if self.unit_can_move_into(u, loop_plot, false, false, false) {
                            let (loop_x, loop_y) = self.map.xy(loop_plot);
                            let mut value = self.plot_distance(x, y, loop_x, loop_y) * 2;
                            if domain == DomainType::DOMAIN_SEA && !self.plot_is_water(self.unit_plot(u)) {
                                if !self.plot_is_water(loop_plot)
                                    || !self.plot_is_adjacent_to_area(loop_plot, self.unit_area(u))
                                {
                                    value *= 3;
                                }
                            } else if self.map.plots[loop_plot].area != Some(self.unit_area(u)) {
                                value *= 3;
                            }
                            if value < best_value {
                                best_value = value;
                                best_plot = Some(loop_plot);
                            }
                        }
                    }
                }
            }
        }

        let valid = best_plot.is_some();
        if let Some(best_plot) = best_plot {
            let (best_x, best_y) = self.map.xy(best_plot);
            self.unit_set_xy(u, best_x, best_y, false);
        } else if kill {
            self.unit_kill(u, false, None);
        }

        valid
    }

    /// C++ `CvUnit::isEnemy` (CvUnit.cpp:18011-18029).
    /// `plot` `None` = the unit's own plot (C++ default NULL).
    pub fn unit_is_enemy(&self, u: UnitId, team: Option<TeamId>, plot: Option<PlotIdx>) -> bool {
        let plot = plot.or_else(|| self.unit_plot_opt(u));

        //FfH: Added by Kael 10/26/2007 (to prevent spinlocks when always hostile units attack barbarian allied teams)
        if self.unit_is_always_hostile(u, plot) {
            if Some(self.unit_get_team(u)) != team {
                return true;
            }
        }
        //FfH: End Add

        let combat_owner = self.unit_get_combat_owner(u, team, plot);
        self.at_war_opt(Some(self.players[combat_owner].team), team)
    }

    /// C++ `CvUnit::isPotentialEnemy` (CvUnit.cpp:18031-18039).
    pub fn unit_is_potential_enemy(&self, u: UnitId, team: Option<TeamId>, plot: Option<PlotIdx>) -> bool {
        let plot = plot.or_else(|| self.unit_plot_opt(u));

        let combat_owner = self.unit_get_combat_owner(u, team, plot);
        self.is_potential_enemy(self.players[combat_owner].team, team)
    }

    /// C++ `CvUnit::isAlwaysHostile` (CvUnit.cpp:18339-18360).
    pub fn unit_is_always_hostile(&self, u: UnitId, plot: Option<PlotIdx>) -> bool {
        //FfH: Added by Kael 09/15/2007
        if self.unit_is_hidden_nationality(u) {
            return true;
        }
        //FfH: End Add

        if !self.d.units[self.units[u].unit_type].always_hostile {
            return false;
        }

        if let Some(plot) = plot {
            if self.plot_is_city(plot, true, Some(self.unit_get_team(u))) {
                return false;
            }
        }

        true
    }

    /// C++ `CvUnit::verifyStackValid` (CvUnit.cpp:18362-18373).
    pub fn unit_verify_stack_valid(&mut self, u: UnitId) -> bool {
        if !self.unit_always_invisible(u) {
            if self.plot_is_visible_enemy_unit_for_unit(self.unit_plot(u), u) {
                return self.unit_jump_to_nearest_valid_plot(u);
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, MapInitData};
    use crate::setup::GameSetup;
    use crate::unit::{Unit, UnitResetInfo};

    /// A game on a small flat grassland map, plus a constructor for bare units (only the
    /// `Unit` struct: no plot, group or owner bookkeeping).
    fn game() -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 3));
        let init = MapInitData { grid_w: 4, grid_h: 4, ..Default::default() };
        g.map = Map::new(d, &init, g.num_players(), g.num_teams());
        g
    }

    fn add(g: &mut Game, unit_type: &str) -> UnitId {
        let d = g.d;
        let unit_type = d.t::<UnitType>(unit_type);
        g.units.insert_with(|id| Unit::new(id, unit_type, PlayerId(0), d, &UnitResetInfo::default()))
    }

    #[test]
    fn cargo_space_by_special_and_domain_cargo() {
        let mut g = game();
        let d = g.d;
        let bird = Some(d.t::<SpecialUnitType>("SPECIALUNIT_BIRD"));
        let (land, air, sea) = (Some(DomainType::DOMAIN_LAND), Some(DomainType::DOMAIN_AIR), Some(DomainType::DOMAIN_SEA));

        // Hunter: carries one bird (SpecialCargo SPECIALUNIT_BIRD, DomainCargo DOMAIN_AIR).
        let hunter = add(&mut g, "UNIT_HUNTER");
        assert_eq!(g.unit_special_cargo(hunter), bird);
        assert_eq!(g.unit_domain_cargo(hunter), air);
        g.units[hunter].cargo_capacity = 1;
        assert_eq!(g.unit_cargo_space_available(hunter, bird, air), 1);
        assert_eq!(g.unit_cargo_space_available(hunter, None, air), 0);
        assert_eq!(g.unit_cargo_space_available(hunter, bird, land), 0);
        assert_eq!(g.unit_cargo_space_available(hunter, bird, None), 0);
        g.unit_change_cargo(hunter, 1);
        assert_eq!(g.unit_cargo_space_available(hunter, bird, air), 0);
        assert!(g.unit_is_full(hunter));

        // Trireme: any land unit, special or not.
        let trireme = add(&mut g, "UNIT_TRIREME");
        assert_eq!(g.unit_special_cargo(trireme), None);
        assert_eq!(g.unit_domain_cargo(trireme), land);
        g.units[trireme].cargo_capacity = 2;
        assert_eq!(g.unit_cargo_space_available(trireme, None, land), 2);
        assert_eq!(g.unit_cargo_space_available(trireme, bird, land), 2);
        assert_eq!(g.unit_cargo_space_available(trireme, None, sea), 0);
        assert_eq!(g.unit_cargo_space_available(trireme, None, None), 0);
        // never negative (a promotion can shrink the capacity below the cargo)
        g.unit_change_cargo(trireme, 2);
        g.unit_change_cargo_space(trireme, -1);
        assert_eq!(g.unit_cargo_space_available(trireme, None, land), 0);
        assert!(g.unit_has_cargo(trireme));
        assert!(g.unit_can_unload_all(trireme));

        // Warrior: carries nothing.
        let warrior = add(&mut g, "UNIT_WARRIOR");
        assert_eq!(g.unit_cargo_space_available(warrior, None, land), 0);
        assert!(!g.unit_has_cargo(warrior));
        assert!(!g.unit_can_unload_all(warrior));
    }

    #[test]
    fn terrain_movement_flags() {
        let mut g = game();
        let warrior = add(&mut g, "UNIT_WARRIOR");
        assert!(!g.unit_can_move_impassable(warrior));
        assert!(!g.unit_can_move_all_terrain(warrior));
        assert!(!g.unit_flat_movement_cost(warrior));
        assert!(!g.unit_ignore_terrain_cost(warrior));
        assert!(!g.unit_always_invisible(warrior));
        assert!(!g.unit_can_coexist_with_enemy_unit(warrior, None));

        // FfH: water walking only gives all-terrain movement ...
        g.units[warrior].water_walking = 1;
        assert!(g.unit_can_move_all_terrain(warrior));
        assert!(!g.unit_can_move_impassable(warrior));
        // ... flying gives everything
        g.units[warrior].water_walking = 0;
        g.units[warrior].flying = 1;
        assert!(g.unit_can_move_impassable(warrior));
        assert!(g.unit_can_move_all_terrain(warrior));
        assert!(g.unit_flat_movement_cost(warrior));
        assert!(g.unit_ignore_terrain_cost(warrior));

        // unit info flags
        let abashi = add(&mut g, "UNIT_ABASHI");
        assert!(g.unit_can_move_impassable(abashi));
        assert!(g.unit_ignore_terrain_cost(abashi));
        assert!(!g.unit_flat_movement_cost(abashi));
        let fireball = add(&mut g, "UNIT_FIREBALL");
        assert!(g.unit_flat_movement_cost(fireball));
        assert!(!g.unit_can_move_impassable(fireball));
        let drown = add(&mut g, "UNIT_DROWN");
        assert!(g.unit_can_move_all_terrain(drown));
        assert!(!g.unit_can_move_impassable(drown));
    }

    #[test]
    fn see_invisible_and_invisible_types() {
        let mut g = game();
        let d = g.d;
        let land = d.t::<InvisibleType>("INVISIBLE_LAND");
        let animal = d.t::<InvisibleType>("INVISIBLE_ANIMAL");
        let all = d.t::<InvisibleType>("INVISIBLE_ALL");
        assert_eq!(invisible_type_define(&g), Some(land));
        assert_eq!(all, InvisibleType(2));

        let hunter = add(&mut g, "UNIT_HUNTER");
        assert_eq!(g.unit_get_num_see_invisible_types(hunter), 1);
        assert_eq!(g.unit_get_see_invisible_type(hunter, 0), animal);
        let warrior = add(&mut g, "UNIT_WARRIOR");
        assert_eq!(g.unit_get_num_see_invisible_types(warrior), 0);
        // FfH: the see-invisible promotion replaces the unit's own types by INVISIBLE_TYPE
        g.units[hunter].see_invisible = 1;
        g.units[warrior].see_invisible = 1;
        assert_eq!(g.unit_get_num_see_invisible_types(hunter), 1);
        assert_eq!(g.unit_get_see_invisible_type(hunter, 0), land);
        assert_eq!(g.unit_get_num_see_invisible_types(warrior), 1);
        assert_eq!(g.unit_get_see_invisible_type(warrior, 0), land);

        // A unit with its own invisible type skips the hide-units plot test.
        g.units[warrior].invisible_type = Some(animal);
        assert_eq!(g.unit_get_invisible_type(warrior), Some(animal));
        assert!(!g.unit_is_never_invisible(warrior));
        // FfH: invisibility from a promotion: INVISIBLE_TYPE, or INVISIBLE_ALL for equipment
        g.units[warrior].invisible = 1;
        assert_eq!(g.unit_get_invisible_type(warrior), Some(land));
        let godslayer = add(&mut g, "EQUIPMENT_GODSLAYER");
        assert_eq!(g.units[godslayer].invisible_type, None);
        g.units[godslayer].invisible_type = Some(land);
        g.units[godslayer].invisible = 1;
        assert_eq!(g.unit_get_invisible_type(godslayer), Some(all));
    }

    #[test]
    fn immobile_timer() {
        let mut g = game();
        let u = add(&mut g, "UNIT_WARRIOR");
        assert_eq!(g.unit_get_immobile_timer(u), 0);
        g.unit_change_immobile_timer(u, 3);
        assert_eq!(g.unit_get_immobile_timer(u), 3);
        g.unit_change_immobile_timer(u, -5);
        assert_eq!(g.unit_get_immobile_timer(u), 0);
        g.unit_set_immobile_timer(u, 2);
        assert_eq!(g.units[u].immobile_timer, 2);

        // FfH: held units report 999 and ignore changes
        g.units[u].held = 1;
        assert_eq!(g.unit_get_immobile_timer(u), 999);
        g.unit_set_immobile_timer(u, 0);
        g.unit_change_immobile_timer(u, -1);
        assert_eq!(g.units[u].immobile_timer, 2);
        g.units[u].held = 0;
        g.unit_set_immobile_timer(u, 0);
        assert_eq!(g.unit_get_immobile_timer(u), 0);
    }

    #[test]
    fn always_hostile_off_the_map() {
        let mut g = game();
        let u = add(&mut g, "UNIT_WARRIOR");
        assert!(!g.unit_is_always_hostile(u, None));
        // FfH: hidden nationality makes any unit always hostile (no plot test)
        g.units[u].hidden_nationality = 1;
        assert!(g.unit_is_always_hostile(u, None));
    }

    #[test]
    fn moves_counter() {
        let mut g = game();
        let u = add(&mut g, "UNIT_WARRIOR");
        assert!(!g.unit_has_moved(u));
        g.unit_change_moves(u, g.d.defines.move_denominator);
        assert!(g.unit_has_moved(u));
        assert_eq!(g.units[u].moves, g.d.defines.move_denominator);
        g.unit_change_moves(u, -g.d.defines.move_denominator);
        assert!(!g.unit_has_moved(u));
    }

    #[test]
    fn active_visibility_needs_an_active_player() {
        let mut g = game();
        // (`Game::new` players are placeholders until `game_init` assigns their teams.)
        g.players[PlayerId(0)].team = TeamId(0);
        g.players[PlayerId(1)].team = TeamId(1);
        let p = g.map.plot_xy(1, 1).unwrap();
        let team = g.players[PlayerId(0)].team;
        g.map.plots[p].visibility_count[team] = 1;
        assert!(!plot_is_active_visible(&g, p));
        g.active_player = Some(PlayerId(0));
        assert!(plot_is_active_visible(&g, p));
        g.active_player = Some(PlayerId(1));
        assert!(!plot_is_active_visible(&g, p));
    }
}
