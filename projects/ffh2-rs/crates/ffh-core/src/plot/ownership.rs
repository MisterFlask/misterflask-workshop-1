//! CvPlot, topic "ownership": owner and team, culture and culture ranges, city radius,
//! plot city and working city, AI found values, plot groups and the trade network,
//! visibility counts and revealed state.
//!
//! C++ `getPlotCity()`, `getWorkingCity()` and `getWorkingCityOverride()` look their city up by
//! `IDInfo`, so an id of a destroyed city reads as NULL (e.g. while `CvCity::kill` updates the
//! culture of the plots around a city it has already deleted); the code below reads those
//! fields through [`live_city`] for the same semantics.

use crate::defs::{MAX_INT, NUM_CITY_PLOTS};
use crate::game::Game;
use crate::ids::*;
use crate::utils::{CITY_PLOT_PRIORITY, NUM_CITY_PLOTS_RADIUS_2};
use ffh_data::*;

/// C++ `getCity(IDInfo)`: `None` for a destroyed city.
#[inline]
fn live_city(g: &Game, city: Option<CityId>) -> Option<CityId> {
    city.filter(|&c| g.cities.contains(c))
}

/// C++ `plotDirection(getX_INLINE(), getY_INLINE(), (DirectionTypes)iI)` for `iI` in
/// `0..NUM_DIRECTION_TYPES`, in that order (`None` off the map).
fn adjacent_plots(g: &Game, p: PlotIdx) -> [Option<PlotIdx>; DirectionType::NUM] {
    let (x, y) = g.map.xy(p);
    std::array::from_fn(|i| g.plot_direction(x, y, DirectionType::from_usize(i)))
}

/// Python `CvGameUtils.getCityFoundValue(argsList)`, called by `CvPlot::getFoundValue` only when
/// `USE_GET_CITY_FOUND_VALUE_CALLBACK` is set (FFH2 leaves it off). FFH2 returns -1 ("any value
/// besides -1 will be used"), i.e. "use the default".
fn py_get_city_found_value(_g: &Game, _player: PlayerId, _x: i32, _y: i32) -> i32 {
    -1
}

impl Game {
    // ------------------------------------------------------------------
    // Owner
    // ------------------------------------------------------------------

    /// C++ `CvPlot::getTeam`: the owner's team (None if unowned).
    pub fn plot_get_team(&self, p: PlotIdx) -> Option<TeamId> {
        self.map.plots[p].owner.map(|owner| self.players[owner].team)
    }

    /// C++ `CvPlot::isOwned`.
    #[inline]
    pub fn plot_is_owned(&self, p: PlotIdx) -> bool {
        self.map.plots[p].owner.is_some()
    }

    /// C++ `CvPlot::isBarbarian`.
    #[inline]
    pub fn plot_is_barbarian(&self, p: PlotIdx) -> bool {
        self.map.plots[p].owner == Some(self.barbarian_player())
    }

    /// C++ `CvPlot::setOwner`: city revolts (acquireCity), land/area/plot-group bookkeeping,
    /// sight, revealed owners, goodies and first contact.
    pub fn plot_set_owner(&mut self, p: PlotIdx, new_value: Option<PlayerId>, check_units: bool, update_plot_group: bool) {
        if self.map.plots[p].owner == new_value {
            return;
        }

        // C++: addReplayMessage(REPLAY_MESSAGE_PLOT_OWNER_CHANGE, ..) (replay, not ported).
        let (x, y) = self.map.xy(p);

        if let Some(old_city) = live_city(self, self.map.plots[p].plot_city) {
            // C++ calls GET_PLAYER(eNewValue).acquireCity: a city plot always gets a new owner.
            let new_owner = new_value.expect("CvPlot::setOwner: a city plot needs a new owner");
            debug_assert!(self.cities[old_city].owner != new_owner, "pOldCity->getOwnerINLINE() is not expected to be equal with eNewValue");

            // C++: TXT_KEY_MISC_CITY_REVOLTED_JOINED to the old and the new owner and a
            // TXT_KEY_MISC_CITY_REVOLTS_JOINS replay message.
            let turn = self.game_turn();
            let old_owner = self.map.plots[p].owner;
            self.log.msg(turn, old_owner, || format!("{} has revolted and joined player {}", self.cities[old_city].name, new_owner));

            self.player_acquire_city(new_owner, old_city, false, false, update_plot_group); // will delete old_city

            if let Some(new_city) = live_city(self, self.map.plots[p].plot_city) {
                let new_team = self.players[new_owner].team;
                let old_units = self.map.plots[p].units.clone();
                for u in old_units {
                    if !self.units.contains(u) {
                        continue;
                    }
                    if self.unit_is_enemy(u, Some(new_team), Some(p)) {
                        self.unit_kill(u, false, Some(new_owner));
                    }
                }

                //FfH: Modified by Kael 01/23/2009 (was AI_bestUnitAI(UNITAI_CITY_DEFENSE / UNITAI_ATTACK))
                let best_unit = self.city_conscript_unit(new_city);
                //FfH: End Modify

                if let Some(best_unit) = best_unit {
                    let free_units = self.d.defines.base_revolt_free_units
                        + (self.cities[new_city].highest_population * self.d.defines.revolt_free_units_percent) / 100;
                    for _ in 0..free_units {
                        self.player_init_unit(new_owner, best_unit, x, y, Some(UnitAIType::UNITAI_CITY_DEFENSE), None);
                    }
                }
            }
        } else {
            self.plot_set_ownership_duration(p, 0);

            if let Some(old_owner) = self.map.plots[p].owner {
                let old_team = self.players[old_owner].team;
                let range = self.d.defines.plot_visibility_range;
                self.plot_change_adjacent_sight(p, old_team, range, false, None, update_plot_group);

                if let Some(a) = self.map.plots[p].area {
                    self.map.areas[a].num_owned_tiles -= 1;
                }
                self.map.owned_plots -= 1;

                if !self.plot_is_water(p) {
                    self.players[old_owner].total_land -= 1;
                    self.teams[old_team].total_land -= 1;
                    // Score v2 reads total_land (docs/SCORE.md); C++ only dirties via land_score.
                    self.score_dirty = true;

                    if self.plot_is_ownership_score(p) {
                        self.player_change_total_land_scored(old_owner, -1);
                    }
                }

                if let Some(improvement) = self.map.plots[p].improvement_type {
                    self.players[old_owner].improvement_count[improvement] -= 1;
                }

                self.plot_update_plot_group_bonus(p, false);
            }

            let old_team = self.plot_get_team(p);
            for u in self.map.plots[p].units.clone() {
                let unit_team = self.unit_get_team(u);
                if old_team != Some(unit_team) && old_team.map_or(true, |t| !self.teams[t].vassal[unit_team]) {
                    //FfH: Modified by Kael 04/19/2009 (summons are not outside units)
                    if self.units[u].duration == 0 {
                        let unit_owner = self.units[u].owner;
                        self.players[unit_owner].num_outside_units -= 1;
                    }
                    //FfH: End Modify
                }

                if self.units[u].blockading {
                    self.unit_set_blockading(u, false);
                    let g = self.units[u].group.expect("CvPlot::setOwner: unit without a selection group");
                    self.group_clear_mission_queue(g);
                    self.group_set_activity_type(g, ActivityType::ACTIVITY_AWAKE);
                }
            }

            self.map.plots[p].owner = new_value;

            self.plot_set_working_city_override(p, None);
            self.plot_update_working_city(p);

            if let Some(owner) = self.map.plots[p].owner {
                let team = self.players[owner].team;
                let range = self.d.defines.plot_visibility_range;
                self.plot_change_adjacent_sight(p, team, range, true, None, update_plot_group);

                if let Some(a) = self.map.plots[p].area {
                    self.map.areas[a].num_owned_tiles += 1;
                }
                self.map.owned_plots += 1;

                if !self.plot_is_water(p) {
                    self.players[owner].total_land += 1;
                    self.teams[team].total_land += 1;
                    // Score v2 reads total_land (docs/SCORE.md).
                    self.score_dirty = true;

                    if self.plot_is_ownership_score(p) {
                        self.player_change_total_land_scored(owner, 1);
                    }
                }

                if let Some(improvement) = self.map.plots[p].improvement_type {
                    self.players[owner].improvement_count[improvement] += 1;
                }

                self.plot_update_plot_group_bonus(p, true);
            }

            let new_team = self.plot_get_team(p);
            for u in self.map.plots[p].units.clone() {
                let unit_team = self.unit_get_team(u);
                if new_team != Some(unit_team) && new_team.map_or(true, |t| !self.teams[t].vassal[unit_team]) {
                    //FfH: Modified by Kael 04/19/2009 (summons are not outside units)
                    if self.units[u].duration == 0 {
                        let unit_owner = self.units[u].owner;
                        self.players[unit_owner].num_outside_units += 1;
                    }
                    //FfH: End Modify
                }
            }

            for t in self.team_ids() {
                if self.team_is_alive(t) {
                    self.plot_update_revealed_owner(p, t);
                }
            }

            self.plot_update_irrigated(p);
            self.plot_update_yield(p);

            if update_plot_group {
                self.plot_update_plot_group(p);
            }

            if check_units {
                self.plot_verify_unit_valid_plot(p);
            }

            if let Some(owner) = self.map.plots[p].owner {
                if self.plot_is_goody(p, None) {
                    self.player_do_goody(owner, p, None);
                }

                for t in self.civ_team_ids() {
                    if self.team_is_alive(t) && self.plot_is_visible(p, t) {
                        if let Some(plot_team) = self.plot_get_team(p) {
                            self.team_meet(t, plot_team, true);
                        }
                    }
                }
            }

            // C++ debug mode: minimap / globe layer / culture border dirty bits (UI).
        }
        // C++: updateSymbols() (graphics).
    }

    /// C++ `CvPlot::isOwnershipScore`.
    #[inline]
    pub fn plot_is_ownership_score(&self, p: PlotIdx) -> bool {
        self.map.plots[p].ownership_duration >= self.d.defines.ownership_score_duration_threshold
    }

    /// C++ `CvPlot::setOwnershipDuration` (updates the owner's scored land).
    pub fn plot_set_ownership_duration(&mut self, p: PlotIdx, new_value: i32) {
        if self.map.plots[p].ownership_duration != new_value {
            let old_ownership_score = self.plot_is_ownership_score(p);

            self.map.plots[p].ownership_duration = new_value;
            debug_assert!(self.map.plots[p].ownership_duration >= 0);

            if old_ownership_score != self.plot_is_ownership_score(p) {
                if let Some(owner) = self.map.plots[p].owner {
                    if !self.plot_is_water(p) {
                        let change = if self.plot_is_ownership_score(p) { 1 } else { -1 };
                        self.player_change_total_land_scored(owner, change);
                    }
                }
            }
        }
    }

    /// C++ `CvPlot::changeOwnershipDuration`.
    pub fn plot_change_ownership_duration(&mut self, p: PlotIdx, change: i32) {
        let v = self.map.plots[p].ownership_duration + change;
        self.plot_set_ownership_duration(p, v);
    }

    /// C++ `CvPlot::isForceUnowned`.
    #[inline]
    pub fn plot_is_force_unowned(&self, p: PlotIdx) -> bool {
        self.map.plots[p].force_unowned_timer > 0
    }

    /// C++ `CvPlot::isAdjacentOwned`.
    pub fn plot_is_adjacent_owned(&self, p: PlotIdx) -> bool {
        adjacent_plots(self, p).into_iter().flatten().any(|adj| self.plot_is_owned(adj))
    }

    /// C++ `CvPlot::isAdjacentPlayer` (C++ default `bLandOnly = false`).
    pub fn plot_is_adjacent_player(&self, p: PlotIdx, player: PlayerId, land_only: bool) -> bool {
        for adj in adjacent_plots(self, p).into_iter().flatten() {
            if self.map.plots[adj].owner == Some(player) {
                if !land_only || !self.plot_is_water(adj) {
                    return true;
                }
            }
        }
        false
    }

    /// C++ `CvPlot::isAdjacentTeam` (C++ default `bLandOnly = false`).
    pub fn plot_is_adjacent_team(&self, p: PlotIdx, team: TeamId, land_only: bool) -> bool {
        for adj in adjacent_plots(self, p).into_iter().flatten() {
            if self.plot_get_team(adj) == Some(team) {
                if !land_only || !self.plot_is_water(adj) {
                    return true;
                }
            }
        }
        false
    }

    // ------------------------------------------------------------------
    // Culture
    // ------------------------------------------------------------------

    /// C++ `CvPlot::calculateCulturalOwner`.
    pub fn plot_calculate_cultural_owner(&self, p: PlotIdx) -> Option<PlayerId> {
        if self.plot_is_force_unowned(p) {
            return None;
        }

        let plot = &self.map.plots[p];
        let mut best_culture = 0;
        let mut best_player: Option<PlayerId> = None;

        for pl in self.player_ids() {
            if self.players[pl].alive {
                let culture = plot.culture[pl];

                if culture > 0 && self.plot_is_within_culture_range(p, pl) {
                    if culture > best_culture || (culture == best_culture && plot.owner == Some(pl)) {
                        best_culture = culture;
                        best_player = Some(pl);
                    }
                }
            }
        }

        let (x, y) = self.map.xy(p);

        if live_city(self, plot.plot_city).is_none() {
            if let Some(best) = best_player {
                let best_team = self.players[best].team;
                let mut best_priority = MAX_INT;
                let mut best_city: Option<CityId> = None;

                for i in 0..NUM_CITY_PLOTS {
                    let Some(loop_plot) = self.plot_city(x, y, i) else {
                        continue;
                    };
                    let Some(loop_city) = live_city(self, self.map.plots[loop_plot].plot_city) else {
                        continue;
                    };
                    let city_team = self.city_team(loop_city);
                    if city_team == best_team || self.teams[best_team].vassal[city_team] {
                        let city_owner = self.cities[loop_city].owner;
                        if plot.culture[city_owner] > 0 && self.plot_is_within_culture_range(p, city_owner) {
                            let mut priority = CITY_PLOT_PRIORITY[i];

                            if city_team == best_team {
                                // "priority ranges from 0 to 4 -> give priority to Masters of a Vassal"
                                // C++ quirk: FfH's 37-plot radius has priorities up to 7, so a close
                                // vassal city can still beat a distant master city.
                                priority += 5;
                            }

                            if priority < best_priority || (priority == best_priority && city_owner == best) {
                                best_priority = priority;
                                best_city = Some(loop_city);
                            }
                        }
                    }
                }

                if let Some(city) = best_city {
                    best_player = Some(self.cities[city].owner);
                }
            }
        }

        if best_player.is_none() {
            let mut valid = true;

            for i in 0..CardinalDirectionType::NUM {
                let Some(loop_plot) = self.plot_cardinal_direction(x, y, CardinalDirectionType::from_usize(i)) else {
                    continue;
                };
                match self.map.plots[loop_plot].owner {
                    Some(owner) => {
                        if best_player.is_none() {
                            best_player = Some(owner);
                        } else if best_player != Some(owner) {
                            valid = false;
                            break;
                        }
                    }
                    None => {
                        valid = false;
                        break;
                    }
                }
            }

            if !valid {
                best_player = None;
            }
        }

        best_player
    }

    /// C++ `CvPlot::updateCulture`: non-city plots go to their cultural owner.
    pub fn plot_update_culture(&mut self, p: PlotIdx, bump_units: bool, update_plot_groups: bool) {
        if live_city(self, self.map.plots[p].plot_city).is_none() {
            let owner = self.plot_calculate_cultural_owner(p);
            self.plot_set_owner(p, owner, bump_units, update_plot_groups);
        }
    }

    /// C++ `CvPlot::doCulture` (protected): city revolts and culture flips, then `updateCulture`.
    pub fn plot_do_culture(&mut self, p: PlotIdx) {
        if let Some(city) = live_city(self, self.map.plots[p].plot_city) {
            if let Some(cultural_owner) = self.plot_calculate_cultural_owner(p) {
                if Some(self.players[cultural_owner].team) != self.plot_get_team(p) && !self.city_is_occupation(city) {
                    let roll = self.soren_rand_num(100, "Revolt #1");
                    if roll < self.city_revolt_test_probability(city) {
                        let city_strength = self.city_culture_strength(city, cultural_owner);
                        let garrison = self.city_culture_garrison(city, cultural_owner);

                        if self.soren_rand_num(city_strength, "Revolt #2") > garrison || self.city_is_barbarian(city) {
                            // C++ walks a snapshot of the plot's units (`oldUnits`; its
                            // `nextUnitNode` calls only follow the snapshot nodes).
                            let old_units = self.map.plots[p].units.clone();
                            for u in old_units {
                                if !self.units.contains(u) {
                                    continue;
                                }
                                if self.unit_is_barbarian(u) {
                                    // Xienwolf Tweak 12/22/08: barbarian heroes and equipment are
                                    // moved instead of killed (was `kill(false, eCulturalOwner)`).
                                    self.unit_jump_to_nearest_valid_plot(u);
                                } else if self.unit_can_defend(u, None) {
                                    let damage = self.unit_curr_hit_points(u) / 2;
                                    self.unit_change_damage(u, damage, Some(cultural_owner));
                                }
                            }

                            let flips = self.city_is_barbarian(city)
                                || (!self.is_option(GameOptionType::GAMEOPTION_NO_CITY_FLIPPING)
                                    && (self.is_option(GameOptionType::GAMEOPTION_FLIPPING_AFTER_CONQUEST)
                                        || !self.cities[city].ever_owned[cultural_owner])
                                    && self.cities[city].num_revolts[cultural_owner] >= self.d.defines.num_warning_revolts);

                            if flips {
                                if self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && self.players[cultural_owner].human {
                                    self.city_kill(city, true);
                                } else {
                                    self.plot_set_owner(p, Some(cultural_owner), true, true); // will delete city
                                }
                            } else {
                                self.cities[city].num_revolts[cultural_owner] += 1;
                                let turns = self.d.defines.base_revolt_occupation_turns
                                    + (city_strength * self.d.defines.revolt_occupation_turns_percent) / 100;
                                self.city_change_occupation_timer(city, turns);

                                // XXX announce for all seen cities?
                                // C++: TXT_KEY_MISC_REVOLT_IN_CITY to the owner and the cultural owner.
                                let turn = self.game_turn();
                                let owner = self.map.plots[p].owner;
                                self.log.msg(turn, owner, || {
                                    format!("Revolt in {} (culture of player {})", self.cities[city].name, cultural_owner)
                                });
                            }
                        }
                    }
                }
            }
        }

        self.plot_update_culture(p, true, true);
    }

    /// C++ `CvPlot::countTotalCulture` (alive players).
    pub fn plot_count_total_culture(&self, p: PlotIdx) -> i32 {
        let plot = &self.map.plots[p];
        let mut total_culture = 0;
        for pl in self.player_ids() {
            if self.players[pl].alive {
                total_culture += plot.culture[pl];
            }
        }
        total_culture
    }

    /// C++ `CvPlot::countFriendlyCulture`: culture of `team`, its vassals and open-border partners.
    pub fn plot_count_friendly_culture(&self, p: PlotIdx, team: TeamId) -> i32 {
        let plot = &self.map.plots[p];
        let mut total_culture = 0;
        for pl in self.player_ids() {
            if self.players[pl].alive {
                let loop_team = self.players[pl].team;
                if loop_team == team || self.teams[loop_team].vassal[team] || self.team_is_open_borders(loop_team, team) {
                    total_culture += plot.culture[pl];
                }
            }
        }
        total_culture
    }

    /// C++ `CvPlot::findHighestCultureTeam`.
    pub fn plot_find_highest_culture_team(&self, p: PlotIdx) -> Option<TeamId> {
        self.plot_find_highest_culture_player(p).map(|best| self.players[best].team)
    }

    /// C++ `CvPlot::findHighestCulturePlayer`.
    pub fn plot_find_highest_culture_player(&self, p: PlotIdx) -> Option<PlayerId> {
        let plot = &self.map.plots[p];
        let mut best_player = None;
        let mut best_value = 0;
        for pl in self.player_ids() {
            if self.players[pl].alive {
                let value = plot.culture[pl];
                if value > best_value {
                    best_value = value;
                    best_player = Some(pl);
                }
            }
        }
        best_player
    }

    /// C++ `CvPlot::calculateCulturePercent`.
    pub fn plot_calculate_culture_percent(&self, p: PlotIdx, player: PlayerId) -> i32 {
        let total_culture = self.plot_count_total_culture(p);
        if total_culture > 0 {
            // C++ `int` arithmetic (wraps on MSVC).
            return self.map.plots[p].culture[player].wrapping_mul(100) / total_culture;
        }
        0
    }

    /// C++ `CvPlot::calculateTeamCulturePercent`.
    pub fn plot_calculate_team_culture_percent(&self, p: PlotIdx, team: TeamId) -> i32 {
        let mut team_culture_percent = 0;
        for pl in self.player_ids() {
            if self.players[pl].alive && self.players[pl].team == team {
                team_culture_percent += self.plot_calculate_culture_percent(p, pl);
            }
        }
        team_culture_percent
    }

    /// C++ `CvPlot::setCulture` (`getCulture(e)` is the `culture` field).
    pub fn plot_set_culture(&mut self, p: PlotIdx, player: PlayerId, new_value: i32, update: bool, update_plot_groups: bool) {
        if self.map.plots[p].culture[player] != new_value {
            self.map.plots[p].culture[player] = new_value;
            debug_assert!(self.map.plots[p].culture[player] >= 0);

            if update {
                self.plot_update_culture(p, true, update_plot_groups);
            }

            if let Some(city) = live_city(self, self.map.plots[p].plot_city) {
                self.cities[city].ai_assign_work_dirty = true;
            }
        }
    }

    /// C++ `CvPlot::changeCulture`.
    pub fn plot_change_culture(&mut self, p: PlotIdx, player: PlayerId, change: i32, update: bool) {
        if change != 0 {
            let v = self.map.plots[p].culture[player] + change;
            self.plot_set_culture(p, player, v, update, true);
        }
    }

    /// C++ `CvPlot::isWithinCultureRange`.
    pub fn plot_is_within_culture_range(&self, p: PlotIdx, player: PlayerId) -> bool {
        self.map.plots[p].culture_range_cities[player].iter().any(|&n| n > 0)
    }

    /// C++ `CvPlot::getNumCultureRangeCities`.
    pub fn plot_get_num_culture_range_cities(&self, p: PlotIdx, player: PlayerId) -> i32 {
        self.map.plots[p].culture_range_cities[player].iter().sum()
    }

    /// C++ `CvPlot::isCultureRangeCity`.
    #[inline]
    pub fn plot_is_culture_range_city(&self, p: PlotIdx, player: PlayerId, range: CultureLevelType) -> bool {
        self.map.plots[p].culture_range_cities[player][range] > 0
    }

    /// C++ `CvPlot::changeCultureRangeCities`.
    pub fn plot_change_culture_range_cities(&mut self, p: PlotIdx, player: PlayerId, range: CultureLevelType, change: i32, update_plot_groups: bool) {
        if change != 0 {
            let old_culture_range_cities = self.plot_is_culture_range_city(p, player, range);

            self.map.plots[p].culture_range_cities[player][range] += change;

            if old_culture_range_cities != self.plot_is_culture_range_city(p, player, range) {
                self.plot_update_culture(p, true, update_plot_groups);
            }
        }
    }

    // ------------------------------------------------------------------
    // Cities
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isCityRadius` (C++ returns int).
    #[inline]
    pub fn plot_is_city_radius(&self, p: PlotIdx) -> bool {
        self.map.plots[p].city_radius_count > 0
    }

    /// C++ `CvPlot::isPlayerCityRadius`.
    #[inline]
    pub fn plot_is_player_city_radius(&self, p: PlotIdx, player: PlayerId) -> bool {
        self.map.plots[p].player_city_radius_count[player] > 0
    }

    /// C++ `CvPlot::isWithinTeamCityRadius` (C++ default `eIgnorePlayer = NO_PLAYER`).
    pub fn plot_is_within_team_city_radius(&self, p: PlotIdx, team: TeamId, ignore_player: Option<PlayerId>) -> bool {
        for pl in self.player_ids() {
            if self.players[pl].alive && self.players[pl].team == team {
                if ignore_player.map_or(true, |ignore| pl != ignore) {
                    if self.plot_is_player_city_radius(p, pl) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// C++ `CvPlot::isCity` (C++ defaults `bCheckImprovement = false`, `eForTeam = NO_TEAM`):
    /// a city, or (`check_improvement`) an improvement that acts as a city for `for_team`.
    pub fn plot_is_city(&self, p: PlotIdx, check_improvement: bool, for_team: Option<TeamId>) -> bool {
        if check_improvement {
            if let Some(improvement) = self.map.plots[p].improvement_type {
                let info = &self.d.improvements[improvement];
                if info.acts_as_city {
                    let plot_team = self.plot_get_team(p);
                    match for_team {
                        None => return true,
                        Some(for_team) => {
                            if (plot_team.is_none() && info.outside_borders) || self.team_is_friendly_territory(for_team, plot_team) {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        live_city(self, self.map.plots[p].plot_city).is_some()
    }

    /// C++ `CvPlot::setPlotCity` (`getPlotCity()` is the `plot_city` field).
    pub fn plot_set_plot_city(&mut self, p: PlotIdx, city: Option<CityId>) {
        if live_city(self, self.map.plots[p].plot_city) == city {
            return;
        }

        let d = self.d;
        let (x, y) = self.map.xy(p);

        if let Some(old_city) = live_city(self, self.map.plots[p].plot_city) {
            let city_owner = self.cities[old_city].owner;
            // C++ `plotCity` limits the indices by the city standing on this plot (the old one).
            for i in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(x, y, i) {
                    self.map.plots[loop_plot].city_radius_count -= 1;
                    // C++ quirk: the count can go negative (C++ `FAssert` in
                    // `changePlayerCityRadiusCount`): a city added its radius over the 22 plots
                    // `plotCity` allowed when `CvCity::init` called `setPlotCity` (radius 2), but
                    // `setPlotRadius(3)` (Kuriotates cities, Wonder radius buildings) does not
                    // update the counts, so removing it now subtracts over all 37 plots.
                    self.map.plots[loop_plot].player_city_radius_count[city_owner] -= 1;
                }
            }
        }

        self.plot_update_plot_group_bonus(p, false);
        if let Some(old_city) = live_city(self, self.map.plots[p].plot_city) {
            if let Some(group) = self.map.plots[p].owner.and_then(|owner| self.plot_get_plot_group(p, owner)) {
                for b in d.bonuses.keys() {
                    let num = self.plot_groups[group].num_bonuses[b];
                    self.city_change_num_bonuses(old_city, b, -num);
                }
            }
        }

        self.map.plots[p].plot_city = city;

        if let Some(new_city) = live_city(self, self.map.plots[p].plot_city) {
            if let Some(group) = self.map.plots[p].owner.and_then(|owner| self.plot_get_plot_group(p, owner)) {
                for b in d.bonuses.keys() {
                    let num = self.plot_groups[group].num_bonuses[b];
                    self.city_change_num_bonuses(new_city, b, num);
                }
            }
        }
        self.plot_update_plot_group_bonus(p, true);

        if let Some(new_city) = live_city(self, self.map.plots[p].plot_city) {
            let city_owner = self.cities[new_city].owner;
            // `plotCity` now limits the indices by the new city.
            for i in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(x, y, i) {
                    self.map.plots[loop_plot].city_radius_count += 1;
                    self.map.plots[loop_plot].player_city_radius_count[city_owner] += 1;
                }
            }
        }

        self.plot_update_irrigated(p);
        self.plot_update_yield(p);

        // C++: updateMinimapColor() (UI).
    }

    /// C++ `CvPlot::isOccupation`.
    pub fn plot_is_occupation(&self, p: PlotIdx) -> bool {
        match live_city(self, self.map.plots[p].plot_city) {
            Some(city) => self.city_is_occupation(city),
            None => false,
        }
    }

    /// C++ `CvPlot::isBeingWorked`.
    pub fn plot_is_being_worked(&self, p: PlotIdx) -> bool {
        match live_city(self, self.map.plots[p].working_city) {
            Some(city) => self.city_is_working_plot(city, p),
            None => false,
        }
    }

    /// C++ `CvPlot::updateWorkingCity` (`getWorkingCity()` is the `working_city` field).
    pub fn plot_update_working_city(&mut self, p: PlotIdx) {
        let mut best_city = live_city(self, self.map.plots[p].plot_city);

        if best_city.is_none() {
            best_city = live_city(self, self.map.plots[p].working_city_override);
            debug_assert!(
                best_city.map_or(true, |c| Some(self.cities[c].owner) == self.map.plots[p].owner),
                "pBest city is expected to either be NULL or the current plot instance's"
            );
        }

        if best_city.is_none() && self.plot_is_owned(p) {
            let owner = self.map.plots[p].owner;
            let (x, y) = self.map.xy(p);
            let mut best_plot = 0;

            for i in 0..NUM_CITY_PLOTS {
                let Some(loop_plot) = self.plot_city(x, y, i) else {
                    continue;
                };
                let Some(loop_city) = live_city(self, self.map.plots[loop_plot].plot_city) else {
                    continue;
                };

                // BUGFIX (modified CityRadius) Sephi 0.41k: an AI city only tries to work plots
                // it can actually use.
                if self.cities[loop_city].plot_radius == 3 || (i as i32) < NUM_CITY_PLOTS_RADIUS_2 {
                    if Some(self.cities[loop_city].owner) == owner {
                        // XXX use getGameTurnAcquired() instead???
                        let better = match best_city {
                            None => true,
                            Some(best) => {
                                let (lc, bc) = (&self.cities[loop_city], &self.cities[best]);
                                CITY_PLOT_PRIORITY[i] < CITY_PLOT_PRIORITY[best_plot]
                                    || (CITY_PLOT_PRIORITY[i] == CITY_PLOT_PRIORITY[best_plot]
                                        && (lc.game_turn_founded < bc.game_turn_founded
                                            || (lc.game_turn_founded == bc.game_turn_founded && lc.id < bc.id)))
                            }
                        };
                        if better {
                            best_plot = i;
                            best_city = Some(loop_city);
                        }
                    }
                }
            }
        }

        let old_working_city = live_city(self, self.map.plots[p].working_city);

        if old_working_city != best_city {
            if let Some(old) = old_working_city {
                // C++ `setWorkingPlot(this, false)` = `setWorkingPlot(getCityPlotIndex(this), false)`.
                // FfH: a plot left outside a shrunk city radius gets index -1 there (undefined
                // behaviour in C++); skipped.
                if let Some(index) = self.city_plot_index(old, p) {
                    self.city_set_working_plot(old, index, false);
                }
            }

            // C++ also asserts `!isBeingWorked()` for a new working city.
            debug_assert!(best_city.is_none() || self.plot_is_owned(p), "isOwned is expected to be true");
            self.map.plots[p].working_city = best_city;

            if let Some(old) = old_working_city {
                self.cities[old].ai_assign_work_dirty = true;
            }
            if let Some(new) = live_city(self, self.map.plots[p].working_city) {
                self.cities[new].ai_assign_work_dirty = true;
            }

            self.plot_update_yield(p);

            // C++: updateFog(), updateShowCitySymbols(), colored plots of the active player (UI).
        }
    }

    /// C++ `CvPlot::setWorkingCityOverride`.
    pub fn plot_set_working_city_override(&mut self, p: PlotIdx, city: Option<CityId>) {
        if live_city(self, self.map.plots[p].working_city_override) != city {
            if let Some(c) = city {
                debug_assert!(
                    Some(self.cities[c].owner) == self.map.plots[p].owner,
                    "Argument city pNewValue's owner is expected to be the same as the current instance"
                );
            }
            self.map.plots[p].working_city_override = city;

            self.plot_update_working_city(p);
        }
    }

    /// C++ `CvPlot::getFoundValue`: cached `AI_foundValue` (or the `getCityFoundValue` Python
    /// callback) when the cache holds -1; also raises the area's best found value.
    /// Used by starting-plot selection. Non-const in C++ (fills the cache).
    pub fn plot_get_found_value(&mut self, p: PlotIdx, player: PlayerId) -> i32 {
        // C++: a NULL `m_aiFoundValue` reads 0; here the array always exists (0 by default).
        if self.map.plots[p].found_value[player] == -1 {
            let (x, y) = self.map.xy(p);
            let mut result = -1; // C++ `long lResult`
            if self.d.defines.use_get_city_found_value_callback != 0 {
                result = py_get_city_found_value(self, player, x, y);
            }

            if result == -1 {
                // C++ `m_aiFoundValue` is a `short` array: the stored value truncates.
                let value = self.player_ai_found_value(player, x, y, -1, true, &[]);
                self.map.plots[p].found_value[player] = value as i16 as i32;
            }
            // C++ quirk: a Python result other than -1 is never stored either (the cache stays -1).

            let value = self.map.plots[p].found_value[player];
            if let Some(a) = self.map.plots[p].area {
                if value > self.map.areas[a].best_found_value[player] {
                    self.map.areas[a].best_found_value[player] = value;
                }
            }
        }

        self.map.plots[p].found_value[player]
    }

    /// C++ `CvPlot::isBestAdjacentFound` (non-const in C++ but read-only; UI action recommendation).
    pub fn plot_is_best_adjacent_found(&self, p: PlotIdx, player: PlayerId) -> bool {
        let (x, y) = self.map.xy(p);
        let plot_value = self.player_ai_found_value(player, x, y, -1, false, &[]);

        if plot_value == 0 {
            return false;
        }

        let team = self.players[player].team;
        for adjacent in adjacent_plots(self, p).into_iter().flatten() {
            if self.map.plots[adjacent].revealed[team] {
                let (ax, ay) = self.map.xy(adjacent);
                if self.player_ai_found_value(player, ax, ay, -1, false, &[]) > plot_value {
                    return false;
                }
            }
        }

        true
    }

    // ------------------------------------------------------------------
    // Plot groups and the trade network
    // ------------------------------------------------------------------

    /// C++ `CvPlot::getPlotGroup`: `player`'s plot group containing this plot (ids of deleted
    /// groups read as None, like C++ `CvPlayer::getPlotGroup`).
    #[inline]
    pub fn plot_get_plot_group(&self, p: PlotIdx, player: PlayerId) -> Option<PlotGroupId> {
        self.map.plots[p].plot_group[player].filter(|&g| self.plot_groups.contains(g))
    }

    /// C++ `CvPlot::getOwnerPlotGroup`.
    pub fn plot_get_owner_plot_group(&self, p: PlotIdx) -> Option<PlotGroupId> {
        let owner = self.map.plots[p].owner?;
        self.plot_get_plot_group(p, owner)
    }

    /// C++ `CvPlot::setPlotGroup` (moves the plot city's bonuses between groups).
    pub fn plot_set_plot_group(&mut self, p: PlotIdx, player: PlayerId, group: Option<PlotGroupId>) {
        let old_plot_group = self.plot_get_plot_group(p, player);

        if old_plot_group == group {
            return;
        }

        let d = self.d;
        let city = live_city(self, self.map.plots[p].plot_city);

        if self.map.plots[p].owner == Some(player) {
            self.plot_update_plot_group_bonus(p, false);
        }

        if let (Some(old_group), Some(city)) = (old_plot_group, city) {
            if self.cities[city].owner == player {
                for b in d.bonuses.keys() {
                    let num = self.plot_groups[old_group].num_bonuses[b];
                    self.city_change_num_bonuses(city, b, -num);
                }
            }
        }

        self.map.plots[p].plot_group[player] = group;

        if let (Some(new_group), Some(city)) = (self.plot_get_plot_group(p, player), city) {
            if self.cities[city].owner == player {
                for b in d.bonuses.keys() {
                    let num = self.plot_groups[new_group].num_bonuses[b];
                    self.city_change_num_bonuses(city, b, num);
                }
            }
        }

        if self.map.plots[p].owner == Some(player) {
            self.plot_update_plot_group_bonus(p, true);
        }
    }

    /// C++ `CvPlot::updatePlotGroup()`: all alive players.
    pub fn plot_update_plot_group(&mut self, p: PlotIdx) {
        for pl in self.player_ids() {
            if self.players[pl].alive {
                self.plot_update_plot_group_player(p, pl, true);
            }
        }
    }

    /// C++ `CvPlot::updatePlotGroup(PlayerTypes, bool)` (C++ default `bRecalculate = true`).
    pub fn plot_update_plot_group_player(&mut self, p: PlotIdx, player: PlayerId, recalculate: bool) {
        if !self.final_initialized {
            return;
        }

        let team = self.players[player].team;
        let mut plot_group = self.plot_get_plot_group(p, player);

        if let Some(group) = plot_group {
            if recalculate {
                let mut connected = false;

                if self.plot_is_trade_network(p, team) {
                    connected = true;

                    for adj in adjacent_plots(self, p).into_iter().flatten() {
                        if self.plot_get_plot_group(adj, player) == Some(group) {
                            if !self.plot_is_trade_network_connected(p, adj, team) {
                                connected = false;
                                break;
                            }
                        }
                    }
                }

                if !connected {
                    let empty = self.plot_groups[group].plots.len() == 1;
                    debug_assert!(!self.plot_groups[group].plots.is_empty(), "pPlotGroup should have more than 0 plots");

                    self.plot_group_remove_plot(group, p);

                    if !empty {
                        self.plot_group_recalculate_plots(group);
                    }
                }
            }

            plot_group = self.plot_get_plot_group(p, player);
        }

        if self.plot_is_trade_network(p, team) {
            for adj in adjacent_plots(self, p).into_iter().flatten() {
                // Re-read every time: combining groups changes the neighbours' groups.
                let Some(adjacent_group) = self.plot_get_plot_group(adj, player) else {
                    continue;
                };
                if Some(adjacent_group) != plot_group && self.plot_is_trade_network_connected(p, adj, team) {
                    match plot_group {
                        None => {
                            self.plot_group_add_plot(adjacent_group, p);
                            plot_group = Some(adjacent_group);
                            debug_assert_eq!(self.plot_get_plot_group(p, player), plot_group);
                        }
                        Some(group) => {
                            debug_assert_eq!(self.plot_get_plot_group(p, player), plot_group);
                            self.map_combine_plot_groups(player, group, adjacent_group);
                            plot_group = self.plot_get_plot_group(p, player);
                            debug_assert!(plot_group.is_some(), "PlotGroup is not assigned a valid value");
                        }
                    }
                }
            }

            if plot_group.is_none() {
                self.player_init_plot_group(player, p);
            }
        }
    }

    /// C++ `CvPlot::updatePlotGroupBonus`: adds/removes this plot's bonuses (city free bonuses,
    /// capital imports/exports, the plot bonus) to/from the owner's plot group.
    pub fn plot_update_plot_group_bonus(&mut self, p: PlotIdx, add: bool) {
        let Some(owner) = self.map.plots[p].owner else {
            return;
        };
        let Some(plot_group) = self.plot_get_plot_group(p, owner) else {
            return;
        };

        let d = self.d;
        let team = self.players[owner].team;
        let sign = if add { 1 } else { -1 };

        if let Some(city) = live_city(self, self.map.plots[p].plot_city) {
            for b in d.bonuses.keys() {
                if !self.team_is_bonus_obsolete(team, b) {
                    let change = self.cities[city].free_bonus[b] * sign;
                    self.plot_group_change_num_bonuses(plot_group, b, change);
                }
            }

            if self.city_is_capital(city) {
                for b in d.bonuses.keys() {
                    let export_change = self.players[owner].bonus_export[b] * -sign;
                    self.plot_group_change_num_bonuses(plot_group, b, export_change);
                    let import_change = self.players[owner].bonus_import[b] * sign;
                    self.plot_group_change_num_bonuses(plot_group, b, import_change);
                }
            }
        }

        if let Some(bonus) = self.plot_get_non_obsolete_bonus_type(p, team) {
            if self.team_is_has_tech(team, d.bonuses[bonus].tech_city_trade) {
                if self.plot_is_city(p, true, Some(team))
                    || self.map.plots[p].improvement_type.map_or(false, |i| d.improvements[i].improvement_bonus[bonus].bonus_trade)
                {
                    // C++ also re-tests `pPlotGroup != NULL` here.
                    if self.plot_is_bonus_network(p, team) {
                        self.plot_group_change_num_bonuses(plot_group, bonus, sign);
                    }
                }
            }
        }
    }

    /// C++ `CvPlot::isConnectedTo`.
    pub fn plot_is_connected_to(&self, p: PlotIdx, city: CityId) -> bool {
        debug_assert!(self.plot_is_owned(p));
        // C++ quirk: the `CvPlotGroup` pointers are compared, so a plot and a city that are both
        // outside any plot group of that player count as connected.
        // An unowned plot makes C++ index NO_PLAYER (undefined behaviour); that test is skipped.
        let by_plot_owner = match self.map.plots[p].owner {
            Some(owner) => self.plot_get_plot_group(p, owner) == self.city_plot_group(city, owner),
            None => false,
        };
        let city_owner = self.cities[city].owner;
        by_plot_owner || self.plot_get_plot_group(p, city_owner) == self.city_plot_group(city, city_owner)
    }

    /// C++ `CvPlot::isConnectedToCapital` (C++ default `ePlayer = NO_PLAYER` = owner).
    pub fn plot_is_connected_to_capital(&self, p: PlotIdx, player: Option<PlayerId>) -> bool {
        let player = player.or(self.map.plots[p].owner);

        if let Some(player) = player {
            if let Some(capital_city) = self.player_get_capital_city(player) {
                return self.plot_is_connected_to(p, capital_city);
            }
        }

        false
    }

    /// C++ `CvPlot::getPlotGroupConnectedBonus`.
    pub fn plot_get_plot_group_connected_bonus(&self, p: PlotIdx, player: PlayerId, bonus: BonusType) -> i32 {
        match self.plot_get_plot_group(p, player) {
            Some(g) => self.plot_groups[g].num_bonuses[bonus],
            None => 0,
        }
    }

    /// C++ `CvPlot::isPlotGroupConnectedBonus`.
    #[inline]
    pub fn plot_is_plot_group_connected_bonus(&self, p: PlotIdx, player: PlayerId, bonus: BonusType) -> bool {
        self.plot_get_plot_group_connected_bonus(p, player, bonus) > 0
    }

    /// C++ `CvPlot::isAdjacentPlotGroupConnectedBonus`.
    pub fn plot_is_adjacent_plot_group_connected_bonus(&self, p: PlotIdx, player: PlayerId, bonus: BonusType) -> bool {
        adjacent_plots(self, p).into_iter().flatten().any(|adj| self.plot_is_plot_group_connected_bonus(adj, player, bonus))
    }

    /// C++ `CvPlot::isTradeNetworkImpassable`.
    pub fn plot_is_trade_network_impassable(&self, p: PlotIdx, team: TeamId) -> bool {
        self.plot_is_impassable(p) && !self.plot_is_river_network(p, team)
    }

    /// C++ `CvPlot::isRiverNetwork`.
    pub fn plot_is_river_network(&self, p: PlotIdx, team: TeamId) -> bool {
        if !self.plot_is_river(p) {
            return false;
        }

        if self.team_is_river_trade(team) {
            return true;
        }

        if self.plot_get_team(p) == Some(team) {
            return true;
        }

        false
    }

    /// C++ `CvPlot::isNetworkTerrain`.
    pub fn plot_is_network_terrain(&self, p: PlotIdx, team: TeamId) -> bool {
        // C++ FAssert(getTerrainType() != NO_TERRAIN): a plot without terrain trades nothing.
        if let Some(terrain) = self.map.plots[p].terrain_type {
            if self.team_is_terrain_trade(team, terrain) {
                return true;
            }
        }

        if self.plot_is_water(p) && self.plot_get_team(p) == Some(team) {
            return true;
        }

        false
    }

    /// C++ `CvPlot::isBonusNetwork`.
    pub fn plot_is_bonus_network(&self, p: PlotIdx, team: TeamId) -> bool {
        self.plot_is_route(p) || self.plot_is_river_network(p, team) || self.plot_is_network_terrain(p, team)
    }

    /// C++ `CvPlot::isTradeNetwork`.
    pub fn plot_is_trade_network(&self, p: PlotIdx, team: TeamId) -> bool {
        if self.at_war_opt(Some(team), self.plot_get_team(p)) {
            return false;
        }

        if self.map.plots[p].blockaded_count[team] > 0 {
            return false;
        }

        if self.plot_is_trade_network_impassable(p, team) {
            return false;
        }

        if !self.plot_is_owned(p) && !self.map.plots[p].revealed[team] {
            return false;
        }

        self.plot_is_bonus_network(p, team)
    }

    /// C++ `CvPlot::isTradeNetworkConnected`.
    pub fn plot_is_trade_network_connected(&self, p: PlotIdx, other: PlotIdx, team: TeamId) -> bool {
        if self.at_war_opt(Some(team), self.plot_get_team(p)) || self.at_war_opt(Some(team), self.plot_get_team(other)) {
            return false;
        }

        if self.plot_is_trade_network_impassable(p, team) || self.plot_is_trade_network_impassable(other, team) {
            return false;
        }

        if !self.plot_is_owned(p) && (!self.map.plots[p].revealed[team] || !self.map.plots[other].revealed[team]) {
            return false;
        }

        if self.plot_is_route(p) && self.plot_is_route(other) {
            return true;
        }

        if self.plot_is_city(p, true, Some(team)) && self.plot_is_network_terrain(other, team) {
            return true;
        }

        if self.plot_is_network_terrain(p, team) {
            if self.plot_is_city(other, true, Some(team)) {
                return true;
            }

            if self.plot_is_network_terrain(other, team) {
                return true;
            }

            if self.plot_is_river_network(other, team) && self.plot_is_river_connection(other, self.direction_xy_plots(other, p)) {
                return true;
            }
        }

        if self.plot_is_river_network(p, team) {
            if self.plot_is_network_terrain(other, team) && self.plot_is_river_connection(p, self.direction_xy_plots(p, other)) {
                return true;
            }

            if self.plot_is_river_connection(p, self.direction_xy_plots(p, other))
                || self.plot_is_river_connection(other, self.direction_xy_plots(other, p))
            {
                if self.plot_is_river_network(other, team) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvPlot::changeBlockadedCount` (`getBlockadedCount` is the `blockaded_count` field).
    pub fn plot_change_blockaded_count(&mut self, p: PlotIdx, team: TeamId, change: i32) {
        if change != 0 {
            self.map.plots[p].blockaded_count[team] += change;
            // C++ quirk: can go negative (the C++ FAssert fails): `CvUnit::changeHiddenNationality`
            // calls `updatePlunder(-1)` for any ship losing its hidden nationality (e.g. the
            // Revelation spell), blockading or not, and with the hidden unit's enemies.
            debug_assert!(self.map.plots[p].blockaded_count[team] == 0 || self.plot_is_water(p));

            if let Some(working_city) = live_city(self, self.map.plots[p].working_city) {
                self.cities[working_city].ai_assign_work_dirty = true;
            }
        }
    }

    // ------------------------------------------------------------------
    // Visibility and revealed state
    // (`bDebug` arguments are dropped: debug mode is a UI feature and never on in rules;
    // `getVisibilityCount`, `isRevealed`, `getRevealedOwner`, `getRevealedImprovementType`,
    // `getRevealedRouteType` and their UI-only setters are fields.)
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isVisible` (`bDebug` dropped).
    #[inline]
    pub fn plot_is_visible(&self, p: PlotIdx, team: TeamId) -> bool {
        let pl = &self.map.plots[p];
        pl.visibility_count[team] > 0 || pl.stolen_visibility_count[team] > 0
    }

    /// C++ `CvPlot::isVisibleToCivTeam`: visible to any alive non-barbarian team.
    pub fn plot_is_visible_to_civ_team(&self, p: PlotIdx) -> bool {
        self.civ_team_ids().any(|t| self.team_is_alive(t) && self.plot_is_visible(p, t))
    }

    /// C++ `CvPlot::isVisibleToWatchingHuman`.
    pub fn plot_is_visible_to_watching_human(&self, p: PlotIdx) -> bool {
        self.civ_player_ids().any(|pl| {
            let player = &self.players[pl];
            player.alive && player.human && self.plot_is_visible(p, player.team)
        })
    }

    /// C++ `CvPlot::isAdjacentVisible` (`bDebug` dropped).
    pub fn plot_is_adjacent_visible(&self, p: PlotIdx, team: TeamId) -> bool {
        adjacent_plots(self, p).into_iter().flatten().any(|adj| self.plot_is_visible(adj, team))
    }

    /// C++ `CvPlot::isAdjacentNonvisible`.
    pub fn plot_is_adjacent_nonvisible(&self, p: PlotIdx, team: TeamId) -> bool {
        adjacent_plots(self, p).into_iter().flatten().any(|adj| !self.plot_is_visible(adj, team))
    }

    /// C++ `CvPlot::changeVisibilityCount`: reveals on becoming visible, meets the owner,
    /// propagates stolen visibility.
    pub fn plot_change_visibility_count(&mut self, p: PlotIdx, team: TeamId, change: i32, see_invisible: Option<InvisibleType>, update_plot_groups: bool) {
        if change == 0 {
            return;
        }

        //FfH: Added by Kael 08/23/2008 (to fix an issue where visibility can get into the negatives)
        if self.map.plots[p].visibility_count[team] < 0 {
            self.map.plots[p].visibility_count[team] = 0;
        }
        //FfH: End Add

        let old_visible = self.plot_is_visible(p, team);

        // C++ no longer asserts a non-negative count ("Some things can reduce Visibility", Sephi).
        self.map.plots[p].visibility_count[team] += change;

        if let Some(invisible) = see_invisible {
            // C++ `changeInvisibleVisibilityCount` (its other side effects are UI only).
            self.map.plots[p].invisible_visibility_count[team][invisible] += change;
        }

        if old_visible != self.plot_is_visible(p, team) {
            if self.plot_is_visible(p, team) {
                self.plot_set_revealed(p, team, true, false, None, update_plot_groups);

                for adj in adjacent_plots(self, p).into_iter().flatten() {
                    self.plot_update_revealed_owner(adj, team);
                }

                if let Some(plot_team) = self.plot_get_team(p) {
                    self.team_meet(plot_team, team, true);
                }
            }

            // C++: pCity->setInfoDirty(true) (UI).

            for t in self.team_ids() {
                if self.team_is_alive(t) && self.team_is_stolen_visibility(t, team) {
                    let stolen_change = if self.plot_is_visible(p, team) { 1 } else { -1 };
                    self.plot_change_stolen_visibility_count(p, t, stolen_change);
                }
            }

            // C++ active team: updateFog(), updateMinimapColor(), updateCenterUnit() (UI).
        }
    }

    /// C++ `CvPlot::changeStolenVisibilityCount`.
    pub fn plot_change_stolen_visibility_count(&mut self, p: PlotIdx, team: TeamId, change: i32) {
        if change == 0 {
            return;
        }

        let old_visible = self.plot_is_visible(p, team);

        self.map.plots[p].stolen_visibility_count[team] += change;
        debug_assert!(self.map.plots[p].stolen_visibility_count[team] >= 0);

        if old_visible != self.plot_is_visible(p, team) {
            if self.plot_is_visible(p, team) {
                self.plot_set_revealed(p, team, true, false, None, true);
            }

            // C++: pCity->setInfoDirty(true); active team: updateFog(), updateMinimapColor(),
            // updateCenterUnit() (UI).
        }
    }

    /// C++ `CvPlot::isInvisibleVisible`.
    #[inline]
    pub fn plot_is_invisible_visible(&self, p: PlotIdx, team: TeamId, invisible: InvisibleType) -> bool {
        self.map.plots[p].invisible_visibility_count[team][invisible] > 0
    }

    /// C++ `CvPlot::getRevealedTeam` (`bDebug` dropped).
    pub fn plot_get_revealed_team(&self, p: PlotIdx, team: TeamId) -> Option<TeamId> {
        self.map.plots[p].revealed_owner[team].map(|owner| self.players[owner].team)
    }

    /// C++ `CvPlot::updateRevealedOwner`.
    pub fn plot_update_revealed_owner(&mut self, p: PlotIdx, team: TeamId) {
        let revealed = self.plot_is_visible(p, team) || self.plot_is_adjacent_visible(p, team);

        if revealed {
            // C++ `setRevealedOwner` (its other side effects are UI only).
            self.map.plots[p].revealed_owner[team] = self.map.plots[p].owner;
        }
    }

    /// C++ `CvPlot::setRevealed`.
    pub fn plot_set_revealed(&mut self, p: PlotIdx, team: TeamId, new_value: bool, terrain_only: bool, from_team: Option<TeamId>, update_plot_group: bool) {
        let city = live_city(self, self.map.plots[p].plot_city);

        if self.map.plots[p].revealed[team] != new_value {
            self.map.plots[p].revealed[team] = new_value;

            if let Some(a) = self.map.plots[p].area {
                let change = if self.map.plots[p].revealed[team] { 1 } else { -1 };
                self.map.areas[a].num_revealed_tiles[team] += change;
            }

            if update_plot_group {
                for pl in self.player_ids() {
                    if self.players[pl].alive && self.players[pl].team == team {
                        self.plot_update_plot_group_player(p, pl, true);
                    }
                }
            }

            // C++ active team: updateSymbols(), updateFog(), updateVisibility(), minimap and
            // globe layer dirty bits (UI).

            if self.map.plots[p].revealed[team] {
                // ONEVENT - PlotRevealed
                //FfH: Modified by Kael 08/02/2008 (only with USE_PLOT_REVEALED_CALLBACK)
                if self.d.defines.use_plot_revealed_callback != 0 {
                    self.ev_plot_revealed(p, team);
                }
                //FfH: End Modify
            }
        }

        if !terrain_only {
            let plot = &mut self.map.plots[p];
            if plot.revealed[team] {
                match from_team {
                    None => {
                        plot.revealed_owner[team] = plot.owner;
                        plot.revealed_improvement_type[team] = plot.improvement_type;
                        plot.revealed_route_type[team] = plot.route_type;

                        if let Some(city) = city {
                            self.cities[city].revealed[team] = true;
                        }
                    }
                    Some(from_team) => {
                        if plot.revealed_owner[from_team] == plot.owner {
                            plot.revealed_owner[team] = plot.revealed_owner[from_team];
                        }

                        if plot.revealed_improvement_type[from_team] == plot.improvement_type {
                            plot.revealed_improvement_type[team] = plot.revealed_improvement_type[from_team];
                        }

                        if plot.revealed_route_type[from_team] == plot.route_type {
                            plot.revealed_route_type[team] = plot.revealed_route_type[from_team];
                        }

                        if let Some(city) = city {
                            if self.city_is_revealed(city, from_team, false) {
                                self.cities[city].revealed[team] = true;
                            }
                        }
                    }
                }
            } else {
                plot.revealed_owner[team] = None;
                plot.revealed_improvement_type[team] = None;
                plot.revealed_route_type[team] = None;

                if let Some(city) = city {
                    self.cities[city].revealed[team] = false;
                }
            }
        }
    }

    /// C++ `CvPlot::isAdjacentRevealed`.
    pub fn plot_is_adjacent_revealed(&self, p: PlotIdx, team: TeamId) -> bool {
        adjacent_plots(self, p).into_iter().flatten().any(|adj| self.map.plots[adj].revealed[team])
    }

    /// C++ `CvPlot::isAdjacentNonrevealed`.
    pub fn plot_is_adjacent_nonrevealed(&self, p: PlotIdx, team: TeamId) -> bool {
        adjacent_plots(self, p).into_iter().flatten().any(|adj| !self.map.plots[adj].revealed[team])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, MapInitData};
    use crate::plot_group::PlotGroup;
    use crate::setup::GameSetup;

    const P0: PlayerId = PlayerId(0);
    const P1: PlayerId = PlayerId(1);
    const T0: TeamId = TeamId(0);
    const T1: TeamId = TeamId(1);

    /// A two-civ game on a `w` x `h` non-wrapping ocean map: players 0 and 1 (teams 0 and 1)
    /// and the barbarians are alive, the open slots are not.
    fn game(w: i32, h: i32) -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 1));
        let init = MapInitData { grid_w: w, grid_h: h, ..Default::default() };
        g.map = Map::new(d, &init, g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p].id = p;
            g.players[p].team = TeamId(p.0);
        }
        for p in [P0, P1, g.barbarian_player()] {
            g.players[p].alive = true;
            let t = g.players[p].team;
            g.teams[t].alive_count = 1;
        }
        g
    }

    fn plot(g: &Game, x: i32, y: i32) -> PlotIdx {
        g.map.plot_xy(x, y).expect("test plot on the map")
    }

    #[test]
    fn culture_counts_and_percentages() {
        let mut g = game(5, 5);
        let p = plot(&g, 2, 2);
        let dead = PlayerId(2); // an open slot
        let barbarian = g.barbarian_player();
        g.map.plots[p].culture[P0] = 30;
        g.map.plots[p].culture[P1] = 30;
        g.map.plots[p].culture[dead] = 1000;
        g.map.plots[p].culture[barbarian] = 10;

        assert_eq!(g.plot_count_total_culture(p), 70);
        // ties keep the first player
        assert_eq!(g.plot_find_highest_culture_player(p), Some(P0));
        assert_eq!(g.plot_find_highest_culture_team(p), Some(T0));
        // integer division: 30 * 100 / 70
        assert_eq!(g.plot_calculate_culture_percent(p, P0), 42);
        assert_eq!(g.plot_calculate_team_culture_percent(p, T1), 42);
        assert_eq!(g.plot_calculate_team_culture_percent(p, TeamId(2)), 0);

        // friendly culture: own team, vassals and open-borders partners
        assert_eq!(g.plot_count_friendly_culture(p, T0), 30);
        g.teams[T1].vassal[T0] = true;
        assert_eq!(g.plot_count_friendly_culture(p, T0), 60);
        g.teams[T1].vassal[T0] = false;
        let barbarian_team = g.barbarian_team();
        g.teams[barbarian_team].open_borders[T0] = true;
        assert_eq!(g.plot_count_friendly_culture(p, T0), 40);

        let empty = plot(&g, 0, 0);
        assert_eq!(g.plot_find_highest_culture_player(empty), None);
        assert_eq!(g.plot_find_highest_culture_team(empty), None);
        assert_eq!(g.plot_calculate_culture_percent(empty, P0), 0);
    }

    #[test]
    fn owner_team_and_adjacency() {
        let mut g = game(5, 5);
        let center = plot(&g, 2, 2);
        let east = plot(&g, 3, 2);
        assert_eq!(g.plot_get_team(center), None);
        assert!(!g.plot_is_adjacent_owned(center));

        g.map.plots[east].owner = Some(P1);
        assert_eq!(g.plot_get_team(east), Some(T1));
        assert!(!g.plot_is_barbarian(east));
        assert!(g.plot_is_adjacent_owned(center));
        assert!(g.plot_is_adjacent_player(center, P1, false));
        assert!(!g.plot_is_adjacent_player(center, P0, false));
        assert!(g.plot_is_adjacent_team(center, T1, false));
        // the map is ocean: land-only queries need a land neighbour
        assert!(!g.plot_is_adjacent_player(center, P1, true));
        assert!(!g.plot_is_adjacent_team(center, T1, true));
        g.map.plots[east].plot_type = PlotType::PLOT_LAND;
        assert!(g.plot_is_adjacent_player(center, P1, true));
        assert!(g.plot_is_adjacent_team(center, T1, true));

        // ownership duration on an unowned plot has no scoring side effects
        let threshold = g.d.defines.ownership_score_duration_threshold;
        g.plot_change_ownership_duration(center, threshold);
        assert!(g.plot_is_ownership_score(center));
        g.plot_set_ownership_duration(center, 0);
        assert!(!g.plot_is_ownership_score(center));
    }

    #[test]
    fn cultural_owner_without_culture_follows_cardinal_neighbours() {
        let mut g = game(5, 5);
        let center = plot(&g, 2, 2);
        assert_eq!(g.plot_calculate_cultural_owner(center), None);

        for (x, y) in [(2, 3), (3, 2), (2, 1), (1, 2)] {
            let q = plot(&g, x, y);
            g.map.plots[q].owner = Some(P0);
        }
        assert_eq!(g.plot_calculate_cultural_owner(center), Some(P0));
        // diagonal neighbours do not matter
        let north_east = plot(&g, 3, 3);
        g.map.plots[north_east].owner = Some(P1);
        assert_eq!(g.plot_calculate_cultural_owner(center), Some(P0));
        // two different cardinal owners, or an unowned cardinal neighbour: no owner
        let west = plot(&g, 1, 2);
        g.map.plots[west].owner = Some(P1);
        assert_eq!(g.plot_calculate_cultural_owner(center), None);
        g.map.plots[west].owner = None;
        assert_eq!(g.plot_calculate_cultural_owner(center), None);

        // off-map neighbours are skipped (corner of a non-wrapping map)
        let corner = plot(&g, 0, 0);
        for (x, y) in [(0, 1), (1, 0)] {
            let q = plot(&g, x, y);
            g.map.plots[q].owner = Some(P1);
        }
        assert_eq!(g.plot_calculate_cultural_owner(corner), Some(P1));
        // culture outside the player's culture range does not count
        g.map.plots[corner].culture[P0] = 50;
        assert_eq!(g.plot_calculate_cultural_owner(corner), Some(P1));
        // force-unowned plots have no owner
        g.map.plots[corner].force_unowned_timer = 2;
        assert_eq!(g.plot_calculate_cultural_owner(corner), None);
    }

    #[test]
    fn culture_range_without_culture_keeps_plot_unowned() {
        let mut g = game(5, 5);
        let p = plot(&g, 2, 2);
        let level = CultureLevelType(1);

        g.plot_change_culture_range_cities(p, P0, level, 1, false);
        assert!(g.plot_is_culture_range_city(p, P0, level));
        assert!(g.plot_is_within_culture_range(p, P0));
        assert_eq!(g.plot_get_num_culture_range_cities(p, P0), 1);
        assert_eq!(g.map.plots[p].owner, None);

        g.plot_change_culture(p, P1, 25, false);
        assert_eq!(g.map.plots[p].culture[P1], 25);
        assert_eq!(g.map.plots[p].owner, None);

        g.plot_change_culture_range_cities(p, P0, level, -1, false);
        assert!(!g.plot_is_within_culture_range(p, P0));
        assert_eq!(g.map.plots[p].owner, None);
    }

    #[test]
    fn visibility_count_reveals_and_clamps() {
        let mut g = game(5, 5);
        let center = plot(&g, 2, 2);
        let east = plot(&g, 3, 2);
        let far = plot(&g, 4, 4);
        let route = RouteType(0);
        g.map.plots[east].owner = Some(P1);
        g.map.plots[center].route_type = Some(route);
        // team 1 steals team 0's visibility
        g.teams[T1].stolen_visibility_timer[T0] = 3;

        g.plot_change_visibility_count(center, T0, 1, None, false);
        assert!(g.plot_is_visible(center, T0));
        assert!(g.map.plots[center].revealed[T0]);
        assert_eq!(g.map.plots[center].revealed_route_type[T0], Some(route));
        assert_eq!(g.map.plots[center].stolen_visibility_count[T1], 1);
        assert!(g.plot_is_visible(center, T1));
        assert!(g.map.plots[center].revealed[T1]);
        // the neighbours' revealed owners are refreshed (they border a visible plot)
        assert_eq!(g.map.plots[east].revealed_owner[T0], Some(P1));
        assert_eq!(g.plot_get_revealed_team(east, T0), Some(T1));
        assert!(!g.map.plots[east].revealed[T0]);
        assert!(g.plot_is_adjacent_visible(east, T0));
        assert!(!g.plot_is_adjacent_visible(far, T0));
        assert!(g.plot_is_adjacent_nonvisible(center, T0));
        assert!(g.plot_is_adjacent_revealed(east, T0));
        assert!(!g.plot_is_adjacent_revealed(far, T0));
        assert!(g.plot_is_adjacent_nonrevealed(center, T0));
        assert!(g.plot_is_visible_to_civ_team(center));
        assert!(!g.plot_is_visible_to_civ_team(far));
        assert!(!g.plot_is_visible_to_watching_human(center));
        g.players[P0].human = true;
        assert!(g.plot_is_visible_to_watching_human(center));

        // hiding again keeps the plot revealed and returns the stolen visibility
        g.plot_change_visibility_count(center, T0, -1, None, false);
        assert!(!g.plot_is_visible(center, T0));
        assert!(g.map.plots[center].revealed[T0]);
        assert_eq!(g.map.plots[center].stolen_visibility_count[T1], 0);

        // FfH: a negative count is reset to 0 before the change
        g.map.plots[far].visibility_count[T1] = -3;
        let invisible = InvisibleType(0);
        g.plot_change_visibility_count(far, T1, 1, Some(invisible), false);
        assert_eq!(g.map.plots[far].visibility_count[T1], 1);
        assert!(g.plot_is_invisible_visible(far, T1, invisible));
        assert!(g.map.plots[far].revealed[T1]);

        // stolen visibility alone also reveals
        let corner = plot(&g, 0, 0);
        g.plot_change_stolen_visibility_count(corner, T0, 1);
        assert!(g.plot_is_visible(corner, T0));
        assert!(g.map.plots[corner].revealed[T0]);
        g.plot_change_stolen_visibility_count(corner, T0, -1);
        assert!(!g.plot_is_visible(corner, T0));
    }

    #[test]
    fn set_revealed_copies_matching_knowledge_from_another_team() {
        let mut g = game(5, 5);
        let p = plot(&g, 2, 2);
        let area = g.map_add_area(true);
        let route = RouteType(0);
        g.map.plots[p].area = Some(area);
        g.map.plots[p].owner = Some(P0);
        g.map.plots[p].route_type = Some(route);
        g.map.plots[p].improvement_type = Some(ImprovementType(0));
        // team 0 knows the owner and the route, but not the improvement
        g.map.plots[p].revealed_owner[T0] = Some(P0);
        g.map.plots[p].revealed_route_type[T0] = Some(route);

        g.plot_set_revealed(p, T1, true, false, Some(T0), false);
        assert!(g.map.plots[p].revealed[T1]);
        assert_eq!(g.map.areas[area].num_revealed_tiles[T1], 1);
        assert_eq!(g.map.plots[p].revealed_owner[T1], Some(P0));
        assert_eq!(g.map.plots[p].revealed_route_type[T1], Some(route));
        assert_eq!(g.map.plots[p].revealed_improvement_type[T1], None);

        // revealing again from nobody refreshes everything
        g.plot_set_revealed(p, T1, true, false, None, false);
        assert_eq!(g.map.areas[area].num_revealed_tiles[T1], 1);
        assert_eq!(g.map.plots[p].revealed_improvement_type[T1], Some(ImprovementType(0)));

        // terrain only: the details are kept
        g.plot_set_revealed(p, T1, false, true, None, false);
        assert!(!g.map.plots[p].revealed[T1]);
        assert_eq!(g.map.areas[area].num_revealed_tiles[T1], 0);
        assert_eq!(g.map.plots[p].revealed_owner[T1], Some(P0));

        // otherwise an unrevealed plot forgets them
        g.plot_set_revealed(p, T1, false, false, None, false);
        assert_eq!(g.map.plots[p].revealed_owner[T1], None);
        assert_eq!(g.map.plots[p].revealed_route_type[T1], None);
        assert_eq!(g.map.plots[p].revealed_improvement_type[T1], None);
    }

    #[test]
    fn network_terrain_rivers_and_routes() {
        let mut g = game(5, 5);
        let d = g.d;
        let water = plot(&g, 2, 2);
        let coast = d.t::<TerrainType>("TERRAIN_COAST");
        g.map.plots[water].terrain_type = Some(coast);
        // owned water is network terrain for its owner only
        assert!(!g.plot_is_network_terrain(water, T0));
        g.map.plots[water].owner = Some(P0);
        assert!(g.plot_is_network_terrain(water, T0));
        assert!(!g.plot_is_network_terrain(water, T1));
        // terrain trade (techs)
        g.teams[T1].terrain_trade_count[coast] = 1;
        assert!(g.plot_is_network_terrain(water, T1));
        assert!(g.plot_is_bonus_network(water, T1));

        let land = plot(&g, 0, 0);
        g.map.plots[land].plot_type = PlotType::PLOT_LAND;
        g.map.plots[land].terrain_type = Some(d.t::<TerrainType>("TERRAIN_GRASS"));
        assert!(!g.plot_is_bonus_network(land, T0));
        // a river carries trade for its owner, or for teams with river trade
        g.map.plots[land].river_crossing_count = 1;
        assert!(!g.plot_is_river_network(land, T0));
        g.teams[T0].river_trade_count = 1;
        assert!(g.plot_is_river_network(land, T0));
        assert!(g.plot_is_bonus_network(land, T0));
        g.map.plots[land].owner = Some(P1);
        assert!(g.plot_is_river_network(land, T1));
        // routes always do
        g.map.plots[land].river_crossing_count = 0;
        assert!(!g.plot_is_bonus_network(land, T1));
        g.map.plots[land].route_type = Some(RouteType(0));
        assert!(g.plot_is_bonus_network(land, T1));

        g.plot_change_blockaded_count(water, T1, 1);
        assert_eq!(g.map.plots[water].blockaded_count[T1], 1);
        g.plot_change_blockaded_count(water, T1, -1);
        assert_eq!(g.map.plots[water].blockaded_count[T1], 0);
    }

    #[test]
    fn acts_as_city_improvements() {
        let mut g = game(5, 5);
        let p = plot(&g, 2, 2);
        let cove = g.d.t::<ImprovementType>("IMPROVEMENT_PIRATE_COVE");
        assert!(!g.plot_is_city(p, true, None));
        g.map.plots[p].improvement_type = Some(cove);
        assert!(!g.plot_is_city(p, false, None));
        assert!(g.plot_is_city(p, true, None));
        // unowned: an outside-borders improvement acts as a city for everybody
        assert!(g.plot_is_city(p, true, Some(T0)));
        // owned: only in friendly territory
        g.map.plots[p].owner = Some(P1);
        assert!(g.plot_is_city(p, true, Some(T1)));
        assert!(!g.plot_is_city(p, true, Some(T0)));
        g.teams[T1].vassal[T0] = true; // team 1 is team 0's vassal
        assert!(g.plot_is_city(p, true, Some(T0)));
        assert!(!g.plot_is_occupation(p));
        assert!(!g.plot_is_being_worked(p));
    }

    #[test]
    fn team_city_radius() {
        let mut g = game(5, 5);
        let p = plot(&g, 2, 2);
        assert!(!g.plot_is_within_team_city_radius(p, T0, None));
        g.map.plots[p].player_city_radius_count[P0] = 1;
        assert!(g.plot_is_player_city_radius(p, P0));
        assert!(g.plot_is_within_team_city_radius(p, T0, None));
        assert!(!g.plot_is_within_team_city_radius(p, T0, Some(P0)));
        assert!(g.plot_is_within_team_city_radius(p, T0, Some(P1)));
        assert!(!g.plot_is_within_team_city_radius(p, T1, None));
        // dead players do not count
        g.players[P0].alive = false;
        assert!(!g.plot_is_within_team_city_radius(p, T0, None));
    }

    #[test]
    fn found_value_cache() {
        let mut g = game(5, 5);
        let p = plot(&g, 2, 2);
        let area = g.map_add_area(true);
        g.map.plots[p].area = Some(area);
        assert_eq!(g.plot_get_found_value(p, P0), 0);
        g.map.plots[p].found_value[P0] = 7;
        assert_eq!(g.plot_get_found_value(p, P0), 7);
        // A cached value is returned as is and does not raise the area's best value.
        assert_eq!(g.map.areas[area].best_found_value[P0], 0);
        // -1 recomputes through `player_ai_found_value` (found_value.rs, tested there).
    }

    #[test]
    fn plot_group_queries() {
        let mut g = game(5, 5);
        let d = g.d;
        let center = plot(&g, 2, 2);
        let east = plot(&g, 3, 2);
        let bonus = BonusType(0);
        let pg = g.plot_groups.insert_with(|id| PlotGroup::new(d, id, P0));
        g.plot_groups[pg].num_bonuses[bonus] = 2;
        g.map.plots[east].plot_group[P0] = Some(pg);

        assert_eq!(g.plot_get_plot_group(east, P0), Some(pg));
        assert_eq!(g.plot_get_plot_group_connected_bonus(east, P0, bonus), 2);
        assert!(g.plot_is_adjacent_plot_group_connected_bonus(center, P0, bonus));
        assert!(!g.plot_is_adjacent_plot_group_connected_bonus(center, P1, bonus));
        assert_eq!(g.plot_get_owner_plot_group(east), None);
        g.map.plots[east].owner = Some(P0);
        assert_eq!(g.plot_get_owner_plot_group(east), Some(pg));
        // no capital: not connected
        assert!(!g.plot_is_connected_to_capital(east, None));

        // another player's group on an unowned city-less plot moves no bonuses
        g.plot_set_plot_group(center, P1, Some(pg));
        assert_eq!(g.plot_get_plot_group(center, P1), Some(pg));
        g.plot_set_plot_group(center, P1, None);
        assert_eq!(g.plot_get_plot_group(center, P1), None);

        // plot groups are only maintained once the game is final-initialized
        g.plot_update_plot_group_player(center, P1, true);
        g.plot_update_plot_group(center);
        assert_eq!(g.plot_get_plot_group(center, P1), None);

        // a deleted group reads as none
        g.plot_groups.remove(pg);
        assert_eq!(g.plot_get_plot_group(east, P0), None);
        assert!(!g.plot_is_adjacent_plot_group_connected_bonus(center, P0, bonus));
    }
}
