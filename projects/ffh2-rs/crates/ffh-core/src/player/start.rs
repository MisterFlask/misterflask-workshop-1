//! CvPlayer: game start -- initial gold/research state, free starting units, starting
//! area/plot selection, and the advanced-start point economy.
//!
//! Also holds the `CvPlayerAI` valuations these rules depend on (marked `// AI:`; they live
//! in the AI class in C++, but their results decide rules outcomes):
//! - [`ai_found_value`] (`AI_foundValue`, with `AI_bonusVal`, `AI_baseBonusVal`,
//!   `AI_corporationBonusVal` and `AI_countDeadlockedBonuses`): the city-site value behind
//!   `CvPlot::getFoundValue`, which `findStartingPlot` maximizes;
//! - the starting-location reset of `AI_updateFoundValues(true)`;
//! - the part of `AI_unitValue` (with `AI_combatValue`, `AI_trueCombatValue`,
//!   `AI_unitImpassableCount`, `AI_unitAIDomainType`) that picks the free starting units.
//!
//! They are free functions (not `Game` methods) so they cannot clash with helpers of the
//! same C++ functions elsewhere.

#![allow(unused_imports)]
#![allow(unused_variables)]

use std::cell::Cell;
use std::cmp::{max, min};

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::structs::*;
use crate::utils::{is_limited_unit_class, is_limited_wonder_class, is_world_unit_class};
use ffh_data::*;

/// C++ `AdvancedStartActionTypes` (CvEnums.h; not generated since no info table uses it).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AdvancedStartAction {
    /// C++ `ADVANCEDSTARTACTION_EXIT`
    Exit,
    /// C++ `ADVANCEDSTARTACTION_UNIT` (data = UnitTypes)
    Unit,
    /// C++ `ADVANCEDSTARTACTION_CITY`
    City,
    /// C++ `ADVANCEDSTARTACTION_POP`
    Pop,
    /// C++ `ADVANCEDSTARTACTION_CULTURE`
    Culture,
    /// C++ `ADVANCEDSTARTACTION_BUILDING` (data = BuildingTypes)
    Building,
    /// C++ `ADVANCEDSTARTACTION_IMPROVEMENT` (data = ImprovementTypes)
    Improvement,
    /// C++ `ADVANCEDSTARTACTION_ROUTE` (data = RouteTypes)
    Route,
    /// C++ `ADVANCEDSTARTACTION_TECH` (data = TechTypes)
    Tech,
    /// C++ `ADVANCEDSTARTACTION_VISIBILITY`
    Visibility,
    /// C++ `ADVANCEDSTARTACTION_AUTOMATE`
    Automate,
}



/// C++ `GC.getCivilizationInfo(getCivilizationType())`: C++ indexes the info unchecked, so a
/// player without a civilization is an error.
fn civ_of(g: &Game, p: PlayerId) -> CivilizationType {
    g.player_get_civilization_type(p).expect("player has no civilization")
}

/// C++ `GC.getHandicapInfo(getHandicapType())` (indexed unchecked in C++).
fn handicap_of(g: &Game, p: PlayerId) -> HandicapType {
    g.player_get_handicap_type(p).expect("player has no handicap")
}

/// C++ `pPlot->area()` (never NULL once the map's areas are calculated).
fn area_of(g: &Game, plot: PlotIdx) -> AreaId {
    g.map.plots[plot].area.expect("plot has no area")
}

impl Game {
    // ------------------------------------------------------------------ initial state and units

    /// C++ `CvPlayer::initFreeState`: starting gold (handicap + start era + FFH trait
    /// starting gold) and an empty research queue.
    pub fn player_init_free_state(&mut self, p: PlayerId) {
        let d = self.d;
        let handicap = handicap_of(self, p);
        let start_era = self.start_era();
        // setGold/changeGold only have UI side effects.
        self.players[p].gold = 0;
        self.players[p].gold += d.handicaps[handicap].starting_gold;
        self.players[p].gold += d.eras[start_era].starting_gold;

        // FfH Traits: Added by Kael 08/02/2007
        let starting_gold = self.players[p].starting_gold;
        self.players[p].gold += starting_gold;
        // FfH: End Add

        self.player_clear_research_queue(p);
    }

    /// C++ `CvPlayer::initFreeUnits`: advanced-start points and starting visibility, or the
    /// civilization's free units plus era/handicap defense, worker and explore units.
    pub fn player_init_free_units(&mut self, p: PlayerId) {
        let d = self.d;

        if self.is_option(GameOptionType::GAMEOPTION_ADVANCED_START) && !self.player_is_barbarian(p) {
            let handicap = handicap_of(self, p);
            let mut points = self.num_advanced_start_points();

            points *= d.handicaps[handicap].advanced_start_points_mod;
            points /= 100;

            if !self.players[p].human {
                points *= d.handicaps[handicap].ai_advanced_start_percent;
                points /= 100;
            }

            // setAdvancedStartPoints: UI side effects only.
            self.players[p].advanced_start_points = points;

            // Starting visibility
            if let Some(starting_plot) = self.player_get_starting_plot(p) {
                let (sx, sy) = self.map.xy(starting_plot);
                let team = self.players[p].team;
                for plot in self.map.plot_ids() {
                    let (x, y) = self.map.xy(plot);
                    if self.plot_distance(x, y, sx, sy) <= d.defines.advanced_start_sight_range {
                        self.plot_set_revealed(plot, team, true, false, None, false);
                    }
                }
            }
        } else {
            let civ = civ_of(self, p);
            let start_era = self.start_era();
            let game_handicap = self.handicap;
            let handicap = handicap_of(self, p);

            for class in d.unit_classes.keys() {
                if let Some(unit) = d.civ_unit(civ, class) {
                    let mut free_count = d.civilizations[civ].civilization_free_units_class[class];

                    let ai_multiplier = if !self.players[p].human { d.handicaps[game_handicap].ai_starting_unit_multiplier } else { 0 };
                    free_count *= d.eras[start_era].starting_unit_multiplier + ai_multiplier;

                    for _ in 0..free_count {
                        self.player_add_free_unit(p, unit, None);
                    }
                }
            }

            let mut free_count = d.eras[start_era].starting_defense_units;
            free_count += d.handicaps[handicap].starting_defense_units;

            if !self.players[p].human {
                free_count += d.handicaps[game_handicap].ai_starting_defense_units;
            }

            if free_count > 0 {
                self.player_add_free_unit_ai(p, UnitAIType::UNITAI_CITY_DEFENSE, free_count);
            }

            let mut free_count = d.eras[start_era].starting_worker_units;
            free_count += d.handicaps[handicap].starting_worker_units;

            if !self.players[p].human {
                free_count += d.handicaps[game_handicap].ai_starting_worker_units;
            }

            if free_count > 0 {
                self.player_add_free_unit_ai(p, UnitAIType::UNITAI_WORKER, free_count);
            }

            let mut free_count = d.eras[start_era].starting_explore_units;
            free_count += d.handicaps[handicap].starting_explore_units;

            if !self.players[p].human {
                free_count += d.handicaps[game_handicap].ai_starting_explore_units;
            }

            if free_count > 0 {
                self.player_add_free_unit_ai(p, UnitAIType::UNITAI_EXPLORE, free_count);
            }
        }
    }

    /// C++ `CvPlayer::addFreeUnitAI`: adds `count` units of the best trainable unit for
    /// `unit_ai` (best by `AI_unitValue`; see [`ai_unit_value`]).
    pub fn player_add_free_unit_ai(&mut self, p: PlayerId, unit_ai: UnitAIType, count: i32) {
        let d = self.d;
        let civ = civ_of(self, p);

        let mut best_unit = None;
        let mut best_value = 0;

        for class in d.unit_classes.keys() {
            let Some(unit) = d.civ_unit(civ, class) else {
                continue;
            };

            // FfH: Added by Kael 07/30/2009 (no world units as free units)
            if self.player_can_train(p, unit, false, false, false) && !is_world_unit_class(d, class) {
                let info = &d.units[unit];
                let mut valid = true;

                if info.prereq_and_bonus.is_some() {
                    valid = false;
                }

                // C++ scans all NUM_UNIT_PREREQ_OR_BONUSES slots for one != NO_BONUS; the data
                // keeps only the listed bonuses.
                if !info.prereq_or_bonuses.is_empty() {
                    valid = false;
                }

                if valid {
                    // AI: AI_unitValue(eLoopUnit, eUnitAI, NULL)
                    let value = ai_unit_value(self, p, unit, unit_ai);

                    if value > best_value {
                        best_unit = Some(unit);
                        best_value = value;
                    }
                }
            }
        }

        if let Some(unit) = best_unit {
            for _ in 0..count {
                self.player_add_free_unit(p, unit, Some(unit_ai));
            }
        }
    }

    /// C++ `CvPlayer::addFreeUnit(eUnit, eUnitAI = NO_UNITAI)`: places a starting unit near
    /// the starting plot (FFH: settlers get STARTING_SETTLER_PROMOTION).
    pub fn player_add_free_unit(&mut self, p: PlayerId, unit: UnitType, unit_ai: Option<UnitAIType>) {
        let d = self.d;
        let info = &d.units[unit];

        if self.is_option(GameOptionType::GAMEOPTION_ONE_CITY_CHALLENGE) && self.players[p].human {
            if unit_ai == Some(UnitAIType::UNITAI_SETTLE) || info.default_unit_ai_type == Some(UnitAIType::UNITAI_SETTLE) {
                if self.players[p].ai_num_ai_units[UnitAIType::UNITAI_SETTLE] >= 1 {
                    return;
                }
            }
        }

        let Some(starting_plot) = self.player_get_starting_plot(p) else {
            return;
        };

        let mut best_plot = None;

        if self.players[p].human {
            // Map script `startHumansOnSameTile` (lResult stays 0 unless it returns true).
            let ms = self.map_script.clone();
            if !ms.start_humans_on_same_tile(self).unwrap_or(false) {
                if !info.found {
                    // FfH: Modified by Kael 09/16/2008 (21 tries instead of NUM_CITY_PLOTS)
                    let rand_offset = self.soren_rand_num(21, "Place Units (Player)");
                    let (sx, sy) = self.map.xy(starting_plot);
                    let starting_area = self.map.plots[starting_plot].area;
                    for i in 0..21 {
                        // C++ quirk: the index still wraps at NUM_CITY_PLOTS (37), so offsets past
                        // the 21 inner plots look at the outer ring (and only 0..3 wrap around).
                        let index = ((i + rand_offset) as usize) % NUM_CITY_PLOTS;
                        let Some(loop_plot) = self.plot_city(sx, sy, index) else {
                            continue;
                        };
                        if self.map.plots[loop_plot].area == starting_area
                            && !self.plot_is_impassable(loop_plot)
                            && !self.plot_is_unit(loop_plot)
                            // FfH: Modified by Kael 09/16/2008 (was `!isGoody()`)
                            && self.map.plots[loop_plot].improvement_type.is_none()
                        {
                            best_plot = Some(loop_plot);
                            break;
                        }
                    }
                }
            }
        }

        let best_plot = best_plot.unwrap_or(starting_plot);
        let (x, y) = self.map.xy(best_plot);

        // FfH: Modified by Kael 08/13/2007
        let u = self.player_init_unit(p, unit, x, y, unit_ai, None);
        if info.default_unit_ai_type == Some(UnitAIType::UNITAI_SETTLE) {
            if let Some(promotion) = opt_from_i32::<PromotionType>(d.defines.starting_settler_promotion) {
                // C++ uses the new unit unconditionally; it only dies here if a creation hook
                // killed it.
                if self.units.contains(u) {
                    self.unit_set_has_promotion(u, promotion, true);
                }
            }
        }
        // FfH: End Add
    }

    // ------------------------------------------------------------------ starting plot

    /// C++ `CvPlayer::startingPlotRange`.
    pub fn player_starting_plot_range(&self, p: PlayerId) -> i32 {
        let d = self.d;
        let civs_alive = self.count_civ_players_alive();

        let mut range = self.map.max_step_distance() + 10;

        range *= d.defines.starting_distance_percent;
        range /= 100;

        range *= self.map.land_plots / (d.worlds[self.setup.world_size].target_num_cities * civs_alive);

        // FfH: Modified by Kael 11/18/2007 (21 instead of NUM_CITY_PLOTS)
        range /= 21;

        range += min((self.map.areas.len() as i32 + 1) / 2, civs_alive);

        // Map script `minStartingDistanceModifier` (the default implementation returns 0,
        // which leaves the range unchanged).
        let ms = self.map_script.clone();
        let modifier = ms.min_starting_distance_modifier(self).unwrap_or(0);
        range *= max(0, modifier + 100);
        range /= 100;

        max(range, d.defines.min_civ_starting_distance)
    }

    /// C++ `CvPlayer::startingPlotWithinRange`: always false since BtS ("XXX changes to
    /// AI_foundValue ... make this function redundant but it is still called from Python").
    pub fn player_starting_plot_within_range(&self, p: PlayerId, plot: PlotIdx, other: PlayerId, range: i32, pass: i32) -> bool {
        false
    }

    /// C++ `CvPlayer::startingPlotDistanceFactor` (called on the player whose starting plot is
    /// measured, `p`, for the evaluating player `other`).
    pub fn player_starting_plot_distance_factor(&self, p: PlayerId, plot: PlotIdx, other: PlayerId, range: i32) -> i32 {
        debug_assert!(other != p);
        let d = self.d;
        let mut range = range;

        let mut value = 1000;

        if let Some(starting_plot) = self.player_get_starting_plot(p) {
            if self.is_team_game() {
                if self.players[other].team == self.players[p].team {
                    range *= d.defines.own_team_starting_modifier;
                    range /= 100;
                } else {
                    range *= d.defines.rival_team_starting_modifier;
                    range /= 100;
                }
            }

            let (x, y) = self.map.xy(plot);
            let (sx, sy) = self.map.xy(starting_plot);
            let mut distance = self.step_distance(x, y, sx, sy);
            if self.map.plots[starting_plot].area != self.map.plots[plot].area {
                distance *= 4;
                distance /= 3;
            }

            value *= distance;
            value /= range;
        }

        max(1, value)
    }

    /// C++ `CvPlayer::findStartingArea`: best land area id, None if it does not matter
    /// (C++ -1; the map script's `findStartingArea` override is consulted first).
    pub fn player_find_starting_area(&self, p: PlayerId) -> Option<AreaId> {
        let ms = self.map_script.clone();
        if let Some(result) = ms.find_starting_area(self, p) {
            // Python override
            if result == -1 {
                return None;
            }
            if result >= 0 && self.map.areas.contains(AreaId::from_usize(result as usize)) {
                return Some(AreaId::from_usize(result as usize));
            }
            // C++ FAssertMsg: "python findStartingArea() must return -1 or the ID of a valid
            // area"; falls through to the DLL search.
        }

        let mut best_value = 0;
        let mut best_area = None;

        // find best land area
        for (id, area) in self.map.areas.iter() {
            if !area.water {
                // iNumPlayersOnArea is the number of players starting on the area, plus this player
                let num_players_on_area = area.num_starting_plots + 1;
                let tile_value = (self.area_calculate_total_best_nature_yield(id)
                    + (self.area_count_coastal_land(id) * 2)
                    + area.num_river_edges
                    + area.num_tiles)
                    + 1;
                let mut value = tile_value / num_players_on_area;

                value *= min(NUM_CITY_PLOTS as i32 + 1, area.num_tiles + 1);
                value /= NUM_CITY_PLOTS as i32 + 1;

                if num_players_on_area <= 2 {
                    value *= 4;
                    value /= 3;
                }

                if value > best_value {
                    best_value = value;
                    best_area = Some(id);
                }
            }
        }

        best_area
    }

    /// C++ `CvPlayer::findStartingPlot(bRandomize = false)`: best plot by found value
    /// (C++ `AI_updateFoundValues(true)` + `CvPlot::getFoundValue`).
    pub fn player_find_starting_plot(&mut self, p: PlayerId, randomize: bool) -> Option<PlotIdx> {
        let ms = self.map_script.clone();
        if let Some(plot) = ms.find_starting_plot(self, p) {
            // Python override
            if plot.idx() < self.map.num_plots() {
                return Some(plot);
            }
            // C++ FAssertMsg: "python findStartingPlot() returned an invalid plot index!";
            // falls through to the DLL search.
        }

        // C++ iBestArea: -1 (None) = any area.
        let mut best_area: Option<AreaId> = None;

        let mut new = false;
        if let Some(starting_plot) = self.player_get_starting_plot(p) {
            best_area = self.map.plots[starting_plot].area;
            self.player_set_starting_plot(p, None, true);
            new = true;
        }

        // AI: AI_updateFoundValues(true) -- this sets all plots found values to -1
        ai_update_found_values_starting_loc(self, p);

        if !new {
            best_area = self.player_find_starting_area(p);
        }

        // C++ quirk: iRange is computed but no longer used (the distance factor is now done
        // inside the found value).
        let _range = self.player_starting_plot_range(p);

        // C++ quirk: every pass repeats the same search (found values are cached), so passes
        // after the first only run when no plot has a positive value.
        for _pass in 0..self.map.max_plot_distance() {
            let mut best_plot = None;
            let mut best_value = 0;

            for plot in self.map.plot_ids() {
                if best_area.is_none() || self.map.plots[plot].area == best_area {
                    // the distance factor is now done inside foundValue
                    let mut value = self.plot_get_found_value(plot, p);

                    if randomize && value > 0 {
                        value += self.soren_rand_num(10000, "Randomize Starting Location");
                    }

                    if value > best_value {
                        best_value = value;
                        best_plot = Some(plot);
                    }
                }
            }

            if best_plot.is_some() {
                return best_plot;
            }
            // C++ FAssertMsg(iPass != 0, "could not find starting plot in first pass.")
        }

        // C++ FAssertMsg(false, "Could not find starting plot.")
        None
    }

    /// C++ `CvPlayer::findBestFoundValue`: best `CvArea::getBestFoundValue` over all areas.
    pub fn player_find_best_found_value(&self, p: PlayerId) -> i32 {
        let mut best_value = 0;

        for (_, area) in self.map.areas.iter() {
            let value = area.best_found_value[p];

            if value > best_value {
                best_value = value;
            }
        }

        best_value
    }

    /// C++ `CvPlayer::getStartingPlot` (`plotSorenINLINE(m_iStartingX, m_iStartingY)`).
    pub fn player_get_starting_plot(&self, p: PlayerId) -> Option<PlotIdx> {
        self.map.plot_soren(self.players[p].starting_x, self.players[p].starting_y)
    }

    /// C++ `CvPlayer::setStartingPlot(pNewValue, bUpdateStartDist)`: updates the areas'
    /// starting plot counts (and min original start distances).
    pub fn player_set_starting_plot(&mut self, p: PlayerId, plot: Option<PlotIdx>, update_start_dist: bool) {
        let old_starting_plot = self.player_get_starting_plot(p);

        if old_starting_plot != plot {
            if let Some(old) = old_starting_plot {
                let old_area = area_of(self, old);
                self.map.areas[old_area].num_starting_plots -= 1;

                if update_start_dist {
                    // C++ quirk: m_iStartingX/Y still point at the old plot here, so the old
                    // area's distances are recomputed with this player's old start counted.
                    self.map_update_min_original_start_dist(old_area);
                }
            }

            match plot {
                None => {
                    self.players[p].starting_x = INVALID_PLOT_COORD;
                    self.players[p].starting_y = INVALID_PLOT_COORD;
                }
                Some(new_plot) => {
                    let (x, y) = self.map.xy(new_plot);
                    self.players[p].starting_x = x;
                    self.players[p].starting_y = y;

                    let new_area = area_of(self, new_plot);
                    self.map.areas[new_area].num_starting_plots += 1;

                    if update_start_dist {
                        self.map_update_min_original_start_dist(new_area);
                    }
                }
            }
        }
    }

    /// C++ `CvPlayer::getNewCityProductionValue`: production value of a new city's free
    /// start-era buildings and population (advanced start, settler extra cost).
    pub fn player_get_new_city_production_value(&self, p: PlayerId) -> i32 {
        let d = self.d;
        let civ = civ_of(self, p);
        let start_era = self.start_era();

        let mut value = 0;
        for class in d.building_classes.keys() {
            if let Some(building) = d.civ_building(civ, class) {
                if let Some(free_start_era) = d.buildings[building].free_start_era {
                    if start_era >= free_start_era {
                        value += (100 * self.player_get_production_needed_building(p, building))
                            / max(1, 100 + self.player_get_production_modifier_building(p, building));
                    }
                }
            }
        }

        value *= 100 + d.defines.new_city_building_value_modifier;
        value /= 100;

        value += (d.defines.advanced_start_city_cost * d.game_speeds[self.game_speed_type()].growth_percent) / 100;

        let population = d.defines.initial_city_population + d.eras[start_era].free_population;
        for i in 1..=population {
            value += (self.player_get_growth_threshold(p, i) * d.defines.advanced_start_population_cost) / 100;
        }

        value
    }

    // ------------------------------------------------------------------ advanced start

    /// C++ `CvPlayer::doAdvancedStartAction(eAction, iX, iY, iData, bAdd)`: buys (or sells)
    /// a unit/city/pop/culture/building/improvement/route/tech/visibility with
    /// advanced-start points; `data` is the type index the action needs.
    pub fn player_do_advanced_start_action(&mut self, p: PlayerId, action: AdvancedStartAction, x: i32, y: i32, data: i32, add: bool) {
        let d = self.d;

        if self.players[p].advanced_start_points < 0 {
            return;
        }

        let plot = self.map.plot_xy(x, y);

        if self.players[p].num_cities() == 0 {
            match action {
                AdvancedStartAction::Exit => {
                    // Try to build this player's empire
                    // AI: AI_doAdvancedStart(true) -- the AI spends the remaining points (the
                    // interface busy flags around it are UI).
                }
                AdvancedStartAction::Automate | AdvancedStartAction::City => {}
                _ => {
                    // The first action must be to place a city
                    // so players can lose by spending everything
                    return;
                }
            }
        }

        match action {
            AdvancedStartAction::Exit => {
                let points = self.players[p].advanced_start_points;
                self.players[p].gold += points;
                self.players[p].advanced_start_points = -1;
                // UI: setInAdvancedStart(false) for the active player.

                if self.players[p].human {
                    for c in self.players[p].cities.clone() {
                        self.city_choose_production(c, None, None, None, false, false);
                    }

                    self.player_choose_tech(p, 0);

                    // UI: if canRevolution(NULL), the BUTTONPOPUP_CHANGECIVIC popup.
                }
            }
            AdvancedStartAction::Automate => {
                // AI: AI_doAdvancedStart(true) (the interface busy flags around it are UI).
            }
            AdvancedStartAction::Unit => {
                let Some(plot) = plot else {
                    return;
                };

                // C++ casts iData unchecked (NO_UNIT would index -1).
                let Some(unit) = opt_from_i32::<UnitType>(data) else {
                    return;
                };
                let mut cost = self.player_get_advanced_start_unit_cost(p, unit, add, Some(plot));

                if add && cost < 0 {
                    return;
                }

                if add {
                    // Add unit to the map
                    if self.players[p].advanced_start_points >= cost {
                        // C++ `if (NULL != pUnit)`: initUnit always returns the new unit.
                        let u = self.player_init_unit(p, unit, x, y, None, None);
                        if self.units.contains(u) {
                            self.unit_finish_moves(u);
                        }
                        self.players[p].advanced_start_points -= cost;
                    }
                } else {
                    // Remove unit from the map

                    // If cost is -1 we already know this unit isn't present
                    if cost != -1 {
                        for u in self.map.plots[plot].units.clone() {
                            if self.units[u].unit_type == unit {
                                self.unit_kill(u, false, None);
                                self.players[p].advanced_start_points += cost;
                                return;
                            }
                        }
                    }

                    // Proper unit not found above, delete first found
                    if let Some(&u) = self.map.plots[plot].units.first() {
                        let unit_type = self.units[u].unit_type;
                        cost = self.player_get_advanced_start_unit_cost(p, unit_type, false, None);
                        // C++ FAssertMsg(iCost != -1, "If this is -1 then that means it's going to
                        // try to delete a unit which shouldn't exist"); the -1 is still applied.
                        self.unit_kill(u, false, None);
                        self.players[p].advanced_start_points += cost;
                    }
                }
                // UI: Advanced_Start_DIRTY_BIT
            }
            AdvancedStartAction::City => {
                let Some(plot) = plot else {
                    return;
                };

                let cost = self.player_get_advanced_start_city_cost(p, add, Some(plot));

                if cost < 0 {
                    return;
                }

                if add {
                    // Add City to the map
                    if self.players[p].num_cities() == 0 {
                        let team = self.players[p].team;
                        let mut closest_player = None;
                        let mut min_distance = MAX_INT;
                        for other in self.civ_player_ids() {
                            let k = &self.players[other];
                            if k.alive && k.team == team && k.num_cities() == 0 {
                                // C++ FAsserts a starting plot and dereferences it.
                                let Some(other_start) = self.player_get_starting_plot(other) else {
                                    continue;
                                };
                                let (sx, sy) = self.map.xy(other_start);
                                let distance = self.plot_distance(x, y, sx, sy);
                                if distance < min_distance {
                                    closest_player = Some(other);
                                    min_distance = distance;
                                }
                            }
                        }
                        // "Self at a minimum should always be valid"
                        if let Some(closest) = closest_player {
                            if closest != p {
                                let temp_plot = self.player_get_starting_plot(closest);
                                let own_start = self.player_get_starting_plot(p);
                                self.player_set_starting_plot(closest, own_start, false);
                                self.player_set_starting_plot(p, temp_plot, false);
                            }
                        }
                    }
                    if self.players[p].advanced_start_points >= cost || self.players[p].num_cities() == 0 {
                        self.player_found(p, x, y);
                        let points = self.players[p].advanced_start_points;
                        self.players[p].advanced_start_points -= min(cost, points);
                        // UI: updateColoredPlots
                        if let Some(c) = self.map.plots[plot].plot_city {
                            if self.cities[c].population > 1 {
                                let threshold = self.city_growth_threshold(c);
                                self.cities[c].food = threshold / 2;
                            }
                        }
                    }
                } else {
                    // Remove City from the map
                    self.plot_set_route_type(plot, None, true);
                    if let Some(c) = self.map.plots[plot].plot_city {
                        self.city_kill(c, true);
                    }
                    self.plot_set_improvement_type(plot, None);
                    self.players[p].advanced_start_points += cost;
                }
                // UI: Advanced_Start_DIRTY_BIT
            }
            AdvancedStartAction::Pop => {
                let Some(plot) = plot else {
                    return;
                };

                if let Some(c) = self.map.plots[plot].plot_city {
                    let cost = self.player_get_advanced_start_pop_cost(p, add, Some(c));

                    if cost < 0 {
                        return;
                    }

                    let mut pop_changed = false;
                    if add {
                        if self.players[p].advanced_start_points >= cost {
                            self.city_change_population(c, 1);
                            self.players[p].advanced_start_points -= cost;
                            pop_changed = true;
                        }
                    } else {
                        self.city_change_population(c, -1);
                        self.players[p].advanced_start_points += cost;
                        pop_changed = true;
                    }

                    if pop_changed {
                        let population = self.cities[c].population;
                        self.cities[c].highest_population = population;
                        if population == 1 {
                            self.cities[c].food = 0;
                            self.cities[c].food_kept = 0;
                        } else if population > 1 {
                            let threshold = self.city_growth_threshold(c);
                            let city = &mut self.cities[c];
                            city.food = threshold / 2;
                            city.food_kept = (city.food * city.max_food_kept_percent) / 100;
                        }
                    }
                }
            }
            AdvancedStartAction::Culture => {
                let Some(plot) = plot else {
                    return;
                };

                if let Some(c) = self.map.plots[plot].plot_city {
                    let cost = self.player_get_advanced_start_culture_cost(p, add, Some(c));

                    if cost < 0 {
                        return;
                    }

                    if add {
                        // Add Culture to the City
                        if self.players[p].advanced_start_points >= cost {
                            let threshold = self.city_culture_threshold(c);
                            self.city_set_culture(c, p, threshold, true, true);
                            self.players[p].advanced_start_points -= cost;
                        }
                    } else {
                        // Remove Culture from the city
                        let level = max(0, opt_to_i32(self.cities[c].culture_level) - 1);
                        let threshold = self.city_culture_threshold_for_level(opt_from_i32(level));
                        self.city_set_culture(c, p, threshold, true, true);
                        self.players[p].advanced_start_points += cost;
                    }
                }
            }
            AdvancedStartAction::Building => {
                let Some(plot) = plot else {
                    return;
                };

                if let Some(c) = self.map.plots[plot].plot_city {
                    // C++ casts iData unchecked (NO_BUILDING would index -1).
                    let Some(building) = opt_from_i32::<BuildingType>(data) else {
                        return;
                    };
                    let cost = self.player_get_advanced_start_building_cost(p, building, add, Some(c));

                    if cost < 0 {
                        return;
                    }

                    if add {
                        // Add Building to the City
                        if self.players[p].advanced_start_points >= cost {
                            let num = self.cities[c].num_real_building[building];
                            self.city_set_num_real_building(c, building, num + 1);
                            self.players[p].advanced_start_points -= cost;
                            if d.buildings[building].food_kept != 0 {
                                let city = &mut self.cities[c];
                                city.food_kept = (city.food * city.max_food_kept_percent) / 100;
                            }
                        }
                    } else {
                        // Remove Building from the map
                        let num = self.cities[c].num_real_building[building];
                        self.city_set_num_real_building(c, building, num - 1);
                        self.players[p].advanced_start_points += cost;
                        if d.buildings[building].food_kept != 0 {
                            let city = &mut self.cities[c];
                            city.food_kept = (city.food * city.max_food_kept_percent) / 100;
                        }
                    }
                }
                // UI: Advanced_Start_DIRTY_BIT
            }
            AdvancedStartAction::Route => {
                let Some(plot) = plot else {
                    return;
                };

                let mut route = opt_from_i32::<RouteType>(data);
                let mut cost = self.player_get_advanced_start_route_cost(p, route, add, Some(plot));

                if add && cost < 0 {
                    return;
                }

                if add {
                    // Add Route to the plot
                    if self.players[p].advanced_start_points >= cost {
                        self.plot_set_route_type(plot, route, true);
                        self.players[p].advanced_start_points -= cost;
                    }
                } else {
                    // Remove Route from the Plot
                    if self.map.plots[plot].route_type != route {
                        route = self.map.plots[plot].route_type;
                        cost = self.player_get_advanced_start_route_cost(p, route, add, None);
                    }

                    if cost < 0 {
                        return;
                    }

                    self.plot_set_route_type(plot, None, true);
                    self.players[p].advanced_start_points += cost;
                }
                // UI: Advanced_Start_DIRTY_BIT
            }
            AdvancedStartAction::Improvement => {
                let Some(plot) = plot else {
                    return;
                };

                let mut improvement = opt_from_i32::<ImprovementType>(data);
                let mut cost = self.player_get_advanced_start_improvement_cost(p, improvement, add, Some(plot));

                if add && cost < 0 {
                    return;
                }

                if add {
                    // Add Improvement to the plot
                    if self.players[p].advanced_start_points >= cost {
                        if let Some(feature) = self.map.plots[plot].feature_type {
                            let civ = civ_of(self, p);
                            for build in d.builds.keys() {
                                let build_info = &d.builds[build];
                                let loop_improvement = build_info.improvement;

                                if improvement == loop_improvement {
                                    if build_info.feature_remove[feature]
                                        && self.player_can_build(p, plot, build, false, false)
                                        // FfH: Added by Kael 11/11/2007
                                        && (!d.civilizations[civ].maintain_features[feature] || build_info.improvement.is_none())
                                    {
                                        self.plot_set_feature_type(plot, None);
                                        break;
                                    }
                                }
                            }
                        }

                        self.plot_set_improvement_type(plot, improvement);

                        self.players[p].advanced_start_points -= cost;
                    }
                } else {
                    // Remove Improvement from the Plot
                    if self.map.plots[plot].improvement_type != improvement {
                        improvement = self.map.plots[plot].improvement_type;
                        cost = self.player_get_advanced_start_improvement_cost(p, improvement, add, Some(plot));
                    }

                    if cost < 0 {
                        return;
                    }

                    self.plot_set_improvement_type(plot, None);
                    self.players[p].advanced_start_points += cost;
                }
                // UI: Advanced_Start_DIRTY_BIT
            }
            AdvancedStartAction::Tech => {
                let tech = opt_from_i32::<TechType>(data);
                let cost = self.player_get_advanced_start_tech_cost(p, tech, add);

                if cost < 0 {
                    return;
                }
                // A cost >= 0 implies a tech (NO_TECH costs -1).
                let Some(tech) = tech else {
                    return;
                };

                let team = self.players[p].team;
                if add {
                    // Add Tech to team
                    if self.players[p].advanced_start_points >= cost {
                        self.team_set_has_tech(team, tech, true, Some(p), false, false);
                        self.players[p].advanced_start_points -= cost;
                    }
                } else {
                    // Remove Tech from the Team
                    self.team_set_has_tech(team, tech, false, Some(p), false, false);
                    self.players[p].advanced_start_points += cost;
                }
                // UI: Advanced_Start_DIRTY_BIT
            }
            AdvancedStartAction::Visibility => {
                let Some(plot) = plot else {
                    return;
                };

                let cost = self.player_get_advanced_start_visibility_cost(p, add, Some(plot));

                if cost < 0 {
                    return;
                }

                let team = self.players[p].team;
                if add {
                    // Add Visibility to the plot
                    if self.players[p].advanced_start_points >= cost {
                        self.plot_set_revealed(plot, team, true, true, None, true);
                        self.players[p].advanced_start_points -= cost;
                    }
                } else {
                    // Remove Visibility from the Plot
                    self.plot_set_revealed(plot, team, false, true, None, true);
                    self.players[p].advanced_start_points += cost;
                }
            }
        }
    }

    /// C++ `CvPlayer::getAdvancedStartUnitCost(eUnit, bAdd, pPlot = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_unit_cost(&self, p: PlayerId, unit: UnitType, add: bool, plot: Option<PlotIdx>) -> i32 {
        let d = self.d;
        let pl = &self.players[p];
        let info = &d.units[unit];
        let mut num_unit_type = 0;

        if pl.num_cities() == 0 {
            return -1;
        }

        let mut cost = (self.player_get_production_needed_unit(p, unit) * info.advanced_start_cost) / 100;
        if cost < 0 {
            return -1;
        }

        match plot {
            None => {
                if add {
                    let mut valid = false;
                    for &c in &pl.cities {
                        if self.city_can_train(c, unit, false, false, false, false) {
                            valid = true;
                            break;
                        }
                    }

                    if !valid {
                        return -1;
                    }
                }
            }
            Some(plot) => {
                let mut city = None;

                if d.defines.advanced_start_allow_units_outside_cities == 0 {
                    city = self.map.plots[plot].plot_city;

                    let Some(c) = city.filter(|&c| self.cities[c].owner == p) else {
                        return -1;
                    };

                    cost *= 100;
                    cost /= max(1, 100 + self.city_production_modifier_unit(c, unit));
                } else {
                    if self.map.plots[plot].owner != Some(p) {
                        return -1;
                    }

                    cost *= 100;
                    cost /= max(1, 100 + self.player_get_production_modifier_unit(p, unit));
                }

                if add {
                    let max_units_per_city = d.defines.advanced_start_max_units_per_city;
                    if max_units_per_city >= 0 {
                        if info.military_support && pl.num_military_units >= max_units_per_city * pl.num_cities() {
                            return -1;
                        }
                    }

                    if let Some(c) = city {
                        if !self.city_can_train(c, unit, false, false, false, false) {
                            return -1;
                        }
                    } else {
                        if !self.plot_can_train(plot, unit, false, false) {
                            return -1;
                        }

                        if self.plot_is_impassable(plot) && !info.can_move_impassable {
                            return -1;
                        }

                        if let Some(feature) = self.map.plots[plot].feature_type {
                            if info.feature_impassable[feature] {
                                let tech = info.feature_passable_tech[feature];
                                if tech.is_none() || !self.team_is_has_tech(pl.team, tech) {
                                    return -1;
                                }
                            }
                        } else if let Some(terrain) = self.map.plots[plot].terrain_type {
                            if info.terrain_impassable[terrain] {
                                let tech = info.terrain_passable_tech[terrain];
                                if tech.is_none() || !self.team_is_has_tech(pl.team, tech) {
                                    return -1;
                                }
                            }
                        }
                    }
                } else {
                    // Must be this unit at plot in order to remove
                    let mut unit_found = false;

                    for &u in &self.map.plots[plot].units {
                        if self.units[u].unit_type == unit {
                            unit_found = true;
                        }
                    }

                    if !unit_found {
                        return -1;
                    }
                }
            }
        }

        // Increase cost if the XML defines that additional units will cost more
        if info.advanced_start_cost_increase != 0 {
            for &u in &pl.units {
                if self.units[u].unit_type == unit {
                    num_unit_type += 1;
                }
            }

            if !add {
                num_unit_type -= 1;
            }

            if num_unit_type > 0 {
                cost *= 100 + info.advanced_start_cost_increase * num_unit_type;
                cost /= 100;
            }
        }

        cost
    }

    /// C++ `CvPlayer::getAdvancedStartCityCost(bAdd, pPlot = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_city_cost(&self, p: PlayerId, add: bool, plot: Option<PlotIdx>) -> i32 {
        let d = self.d;
        let mut num_cities = self.players[p].num_cities();

        let mut cost = self.player_get_new_city_production_value(p);

        if cost < 0 {
            return -1;
        }

        // Valid plot?
        if let Some(plot) = plot {
            let (px, py) = self.map.xy(plot);

            if add {
                // Need valid plot to found on if adding
                if !self.player_can_found(p, px, py, false) {
                    return -1;
                }
            } else {
                // Need your own city present to remove
                match self.map.plots[plot].plot_city {
                    Some(c) => {
                        if self.cities[c].owner != p {
                            return -1;
                        }
                    }
                    None => return -1,
                }
            }

            // Is there a distance limit on how far a city can be placed from a player's start/another city?
            let max_range = d.defines.advanced_start_city_placement_max_range;
            if max_range > 0 {
                let team = self.players[p].team;
                let mut closest_player = None;
                let mut closest_distance = MAX_INT;

                for other in self.civ_player_ids() {
                    if self.players[other].alive {
                        if let Some(starting_plot) = self.player_get_starting_plot(other) {
                            let (sx, sy) = self.map.xy(starting_plot);
                            let distance = self.plot_distance(px, py, sx, sy);
                            if distance <= max_range {
                                if distance < closest_distance || (distance == closest_distance && team != self.players[other].team) {
                                    closest_distance = distance;
                                    closest_player = Some(other);
                                }
                            }
                        }
                    }
                }

                let Some(closest) = closest_player.filter(|&cp| self.players[cp].team == team) else {
                    return -1;
                };
                //Only allow founding a city at someone elses start point if
                //We have no cities and they have no cities.
                if p != closest && (self.players[p].num_cities() > 0 || self.players[closest].num_cities() > 0) {
                    return -1;
                }
            }
        }

        // Increase cost if the XML defines that additional units will cost more
        if d.defines.advanced_start_city_cost_increase != 0 {
            if !add {
                num_cities -= 1;
            }

            if num_cities > 0 {
                cost *= 100 + d.defines.advanced_start_city_cost_increase * num_cities;
                cost /= 100;
            }
        }

        cost
    }

    /// C++ `CvPlayer::getAdvancedStartPopCost(bAdd, pCity = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_pop_cost(&self, p: PlayerId, add: bool, city: Option<CityId>) -> i32 {
        let d = self.d;

        if self.players[p].num_cities() == 0 {
            return -1;
        }

        let mut cost = (self.player_get_growth_threshold(p, 1) * d.defines.advanced_start_population_cost) / 100;

        if let Some(c) = city {
            if self.cities[c].owner != p {
                return -1;
            }

            let mut population = self.cities[c].population;

            // Need to have Population to remove it
            if !add {
                population -= 1;

                if population < d.defines.initial_city_population + d.eras[self.start_era()].free_population {
                    return -1;
                }
            }

            cost = (self.player_get_growth_threshold(p, population) * d.defines.advanced_start_population_cost) / 100;

            // Increase cost if the XML defines that additional Pop will cost more
            if d.defines.advanced_start_population_cost_increase != 0 {
                population -= 1;

                if population > 0 {
                    cost *= 100 + d.defines.advanced_start_population_cost_increase * population;
                    cost /= 100;
                }
            }
        }

        cost
    }

    /// C++ `CvPlayer::getAdvancedStartCultureCost(bAdd, pCity = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_culture_cost(&self, p: PlayerId, add: bool, city: Option<CityId>) -> i32 {
        let d = self.d;

        if self.players[p].num_cities() == 0 {
            return -1;
        }

        let mut cost = d.defines.advanced_start_culture_cost;
        if cost < 0 {
            return -1;
        }

        if let Some(c) = city {
            if self.cities[c].owner != p {
                return -1;
            }

            // C++ getCultureLevel(): NO_CULTURELEVEL is -1.
            let level = opt_to_i32(self.cities[c].culture_level);

            // Need to have enough culture to remove it
            if !add {
                if level <= 0 {
                    return -1;
                }
            }

            let culture = if add {
                self.city_culture_threshold_for_level(opt_from_i32(level + 1)) - self.city_culture(c, p)
            } else {
                self.city_culture(c, p) - self.city_culture_threshold_for_level(opt_from_i32(level - 1))
            };

            cost *= culture;
            cost /= max(1, d.game_speeds[self.game_speed_type()].hurry_percent);
        }

        cost
    }

    /// C++ `CvPlayer::getAdvancedStartBuildingCost(eBuilding, bAdd, pCity = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_building_cost(&self, p: PlayerId, building: BuildingType, add: bool, city: Option<CityId>) -> i32 {
        let d = self.d;
        let info = &d.buildings[building];

        if self.players[p].num_cities() == 0 {
            return -1;
        }

        let mut num_building_type;

        let mut cost = (self.player_get_production_needed_building(p, building) * info.advanced_start_cost) / 100;

        if cost < 0 {
            return -1;
        }

        if let Some(free_start_era) = info.free_start_era {
            if self.start_era() >= free_start_era {
                // you get this building for free
                return -1;
            }
        }

        if city.is_none() {
            if add {
                let mut valid = false;
                for &c in &self.players[p].cities {
                    if self.city_can_construct(c, building, false, false, false) {
                        valid = true;
                        break;
                    }
                }

                if !valid {
                    return -1;
                }
            }
        }
        if let Some(c) = city {
            if self.cities[c].owner != p {
                return -1;
            }

            cost *= 100;
            cost /= max(1, 100 + self.city_production_modifier_building(c, building));

            if add {
                if !self.city_can_construct(c, building, true, false, false) {
                    return -1;
                }
            } else {
                if self.cities[c].num_real_building[building] <= 0 {
                    return -1;
                }

                // Check other buildings in this city and make sure none of them require this one
                let civ = civ_of(self, p);

                // Loop through Buildings to see which are present
                for loop_building in d.buildings.keys() {
                    if self.city_num_building(c, loop_building) > 0 {
                        // Loop through present Building's requirements
                        for class in d.building_classes.keys() {
                            if d.buildings[loop_building].building_class_needed_in_city[class] {
                                if d.civ_building(civ, class) == Some(building) {
                                    return -1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Increase cost if the XML defines that additional Buildings will cost more
        if info.advanced_start_cost_increase != 0 {
            num_building_type = self.player_count_num_buildings(p, building);

            if !add {
                num_building_type -= 1;
            }

            if num_building_type > 0 {
                cost *= 100 + info.advanced_start_cost_increase * max(0, num_building_type - self.players[p].num_cities());
                cost /= 100;
            }
        }

        cost
    }

    /// C++ `CvPlayer::getAdvancedStartImprovementCost(eImprovement, bAdd, pPlot = NULL)`;
    /// -1 = not possible (also for NO_IMPROVEMENT).
    pub fn player_get_advanced_start_improvement_cost(&self, p: PlayerId, improvement: Option<ImprovementType>, add: bool, plot: Option<PlotIdx>) -> i32 {
        let d = self.d;

        let Some(improvement) = improvement else {
            return -1;
        };

        if self.players[p].num_cities() == 0 {
            return -1;
        }

        let team = self.players[p].team;
        let info = &d.improvements[improvement];
        let mut num_improvements = 0;
        let mut cost = info.advanced_start_cost;

        // This denotes cities may not be purchased through Advanced Start
        if cost < 0 {
            return -1;
        }

        cost *= d.game_speeds[self.game_speed_type()].build_percent;
        cost /= 100;

        // Can this Improvement be on our plot?
        if let Some(plot) = plot {
            if add {
                // Valid Plot
                if !self.plot_can_have_improvement(plot, improvement, Some(team), false) {
                    return -1;
                }

                let mut valid = false;

                for build in d.builds.keys() {
                    let build_info = &d.builds[build];
                    let loop_improvement = build_info.improvement;

                    if Some(improvement) == loop_improvement && self.player_can_build(p, plot, build, false, false) {
                        valid = true;

                        if let Some(feature) = self.map.plots[plot].feature_type {
                            if build_info.feature_remove[feature]
                                // FfH: Added by Kael 11/11/2007
                                && !d.civilizations[civ_of(self, p)].maintain_features[feature]
                            {
                                cost += d.features[feature].advanced_start_remove_cost;
                            }
                        }

                        break;
                    }
                }

                if !valid {
                    return -1;
                }

                // Can't place twice
                if self.map.plots[plot].improvement_type == Some(improvement) {
                    return -1;
                }
            } else {
                // Need this improvement in order to remove it
                if self.map.plots[plot].improvement_type != Some(improvement) {
                    return -1;
                }
            }

            // Must be owned by me
            if self.map.plots[plot].owner != Some(p) {
                return -1;
            }
        }

        // Tech requirement
        for build in d.builds.keys() {
            if d.builds[build].improvement == Some(improvement) {
                if !self.team_is_has_tech(team, d.builds[build].tech_prereq) {
                    return -1;
                }
            }
        }

        // Increase cost if the XML defines that additional units will cost more
        if info.advanced_start_cost_increase != 0 {
            for loop_plot in self.map.plot_ids() {
                if self.map.plots[loop_plot].improvement_type == Some(improvement) {
                    num_improvements += 1;
                }
            }

            if !add {
                num_improvements -= 1;
            }

            if num_improvements > 0 {
                cost *= 100 + info.advanced_start_cost_increase * num_improvements;
                cost /= 100;
            }
        }

        cost
    }

    /// C++ `CvPlayer::getAdvancedStartRouteCost(eRoute, bAdd, pPlot = NULL)`; -1 = not
    /// possible (also for NO_ROUTE).
    pub fn player_get_advanced_start_route_cost(&self, p: PlayerId, route: Option<RouteType>, add: bool, plot: Option<PlotIdx>) -> i32 {
        let d = self.d;

        if self.players[p].num_cities() == 0 {
            return -1;
        }

        let Some(route) = route else {
            return -1;
        };

        let team = self.players[p].team;
        let info = &d.routes[route];
        let mut num_routes = 0;

        let mut cost = info.advanced_start_cost;

        // This denotes cities may not be purchased through Advanced Start
        if cost < 0 {
            return -1;
        }

        cost *= d.game_speeds[self.game_speed_type()].build_percent;
        cost /= 100;

        // No invalid plots!
        if let Some(plot) = plot {
            if self.map.plots[plot].plot_city.is_some() {
                return -1;
            }

            if add {
                if self.plot_is_impassable(plot) || self.plot_is_water(plot) {
                    return -1;
                }
                // Can't place twice
                if self.map.plots[plot].route_type == Some(route) {
                    return -1;
                }
            } else {
                // Need Route to remove it
                if self.map.plots[plot].route_type != Some(route) {
                    return -1;
                }
            }

            // Must be owned by me
            if self.map.plots[plot].owner != Some(p) {
                return -1;
            }
        }

        // Tech requirement
        for build in d.builds.keys() {
            if d.builds[build].route == Some(route) {
                if !self.team_is_has_tech(team, d.builds[build].tech_prereq) {
                    return -1;
                }
            }
        }

        // Increase cost if the XML defines that additional units will cost more
        if info.advanced_start_cost_increase != 0 {
            for loop_plot in self.map.plot_ids() {
                if self.map.plots[loop_plot].route_type == Some(route) {
                    num_routes += 1;
                }
            }

            if !add {
                num_routes -= 1;
            }

            if num_routes > 0 {
                cost *= 100 + info.advanced_start_cost_increase * num_routes;
                cost /= 100;
            }
        }

        cost
    }

    /// C++ `CvPlayer::getAdvancedStartTechCost(eTech, bAdd)`; -1 = not possible (also for NO_TECH).
    pub fn player_get_advanced_start_tech_cost(&self, p: PlayerId, tech: Option<TechType>, add: bool) -> i32 {
        let d = self.d;

        let Some(tech) = tech else {
            return -1;
        };

        if self.players[p].num_cities() == 0 {
            return -1;
        }

        let team = self.players[p].team;
        let mut num_techs = 0;

        let mut cost = (self.team_get_research_cost(team, tech) * d.techs[tech].advanced_start_cost) / 100;
        if cost < 0 {
            return -1;
        }

        if add {
            if !self.player_can_research(p, tech, false) {
                return -1;
            }
        } else {
            if !self.team_is_has_tech(team, Some(tech)) {
                return -1;
            }

            // Search through all techs to see if any of the currently owned ones requires this tech
            // (C++ scans the NUM_OR/AND_TECH_PREREQS slots; the data keeps the listed techs).
            for loop_tech in d.techs.keys() {
                if self.team_is_has_tech(team, Some(loop_tech)) {
                    // Or Prereqs
                    if d.techs[loop_tech].prereq_or_techs.contains(&tech) {
                        return -1;
                    }

                    // And Prereqs
                    if d.techs[loop_tech].prereq_and_techs.contains(&tech) {
                        return -1;
                    }
                }
            }

            // If player has placed anything on the map which uses this tech then you cannot remove it

            // Units
            for &u in &self.players[p].units {
                let unit_info = &d.units[self.units[u].unit_type];
                if unit_info.prereq_and_tech == Some(tech) {
                    return -1;
                }

                if unit_info.prereq_and_techs.contains(&tech) {
                    return -1;
                }
            }

            // Cities
            for &c in &self.players[p].cities {
                // All Buildings
                for building in d.buildings.keys() {
                    if self.cities[c].num_real_building[building] > 0 {
                        if d.buildings[building].prereq_and_tech == Some(tech) {
                            return -1;
                        }

                        if d.buildings[building].prereq_and_techs.contains(&Some(tech)) {
                            return -1;
                        }
                    }
                }
            }
        }

        // Increase cost if the XML defines that additional units will cost more
        if d.techs[tech].advanced_start_cost_increase != 0 {
            for loop_tech in d.techs.keys() {
                if self.team_is_has_tech(team, Some(loop_tech)) {
                    num_techs += 1;
                }
            }

            if !add {
                num_techs -= 1;
            }

            if num_techs > 0 {
                cost *= 100 + d.techs[tech].advanced_start_cost_increase * num_techs;
                cost /= 100;
            }
        }

        cost
    }

    /// C++ `CvPlayer::getAdvancedStartVisibilityCost(bAdd, pPlot = NULL)`; -1 = not possible.
    pub fn player_get_advanced_start_visibility_cost(&self, p: PlayerId, add: bool, plot: Option<PlotIdx>) -> i32 {
        let d = self.d;

        if self.players[p].num_cities() == 0 {
            return -1;
        }

        let team = self.players[p].team;
        let mut num_visible_plots = 0;
        let mut cost = d.defines.advanced_start_visibility_cost;

        // This denotes Visibility may not be purchased through Advanced Start
        if cost == -1 {
            return -1;
        }

        // Valid Plot?
        if let Some(plot) = plot {
            if add {
                if self.map.plots[plot].revealed[team] {
                    return -1;
                }
                if !self.plot_is_adjacent_revealed(plot, team) {
                    return -1;
                }
            } else {
                if !self.map.plots[plot].revealed[team] {
                    return -1;
                }
            }
        }

        // Increase cost if the XML defines that additional units will cost more
        if d.defines.advanced_start_visibility_cost_increase != 0 {
            for loop_plot in self.map.plot_ids() {
                if self.map.plots[loop_plot].revealed[team] {
                    num_visible_plots += 1;
                }
            }

            if !add {
                num_visible_plots -= 1;
            }

            if num_visible_plots > 0 {
                cost *= 100 + d.defines.advanced_start_visibility_cost_increase * num_visible_plots;
                cost /= 100;
            }
        }

        cost
    }
}

// ============================================================================================
// CvPlayerAI valuations used by the rules above (AI: they live in the AI class in C++).
// ============================================================================================

/// AI: C++ `CvPlayerAI::AI_updateFoundValues(bStartingLoc = true)`: every area's best found
/// value for `p` back to 0 and every plot's found value to -1 (recomputed lazily by
/// `CvPlot::getFoundValue`). The `bStartingLoc = false` branch (city sites, the
/// `getCityFoundValue` callback) is AI-only.
fn ai_update_found_values_starting_loc(g: &mut Game, p: PlayerId) {
    for (_, area) in g.map.areas.iter_mut() {
        area.best_found_value[p] = 0;
    }

    for plot in g.map.plots.iter_mut() {
        plot.found_value[p] = -1;
    }
}


/// AI: C++ `CvPlayerAI::AI_unitValue(eUnit, eUnitAI, pArea = NULL)` for the UnitAIs rules
/// code asks for (`addFreeUnitAI` from `initFreeUnits`: CITY_DEFENSE, WORKER and EXPLORE).
/// Those cases read only static data, techs and bonuses. The other UnitAIs depend on AI
/// strategy state (`AI_isDoStrategy`, unit class/combat weights, missionary needs, area AI
/// types) and belong to the AI crate; here they are never valid (value 0).
fn ai_unit_value(g: &Game, p: PlayerId, unit: UnitType, unit_ai: UnitAIType) -> i32 {
    debug_assert!(
        matches!(unit_ai, UnitAIType::UNITAI_CITY_DEFENSE | UnitAIType::UNITAI_WORKER | UnitAIType::UNITAI_EXPLORE),
        "AI_unitValue is only ported for the free starting units"
    );
    let d = g.d;
    let info = &d.units[unit];

    if info.domain_type != ai_unit_ai_domain_type(unit_ai) {
        if unit_ai != UnitAIType::UNITAI_ICBM {
            //XXX
            return 0;
        }
    }

    if info.not_unit_ai_type[unit_ai] {
        return 0;
    }

    let mut valid = info.unit_ai_type[unit_ai];

    if !valid {
        match unit_ai {
            UnitAIType::UNITAI_WORKER => {
                for build in d.builds.keys() {
                    if info.builds[build] {
                        valid = true;
                        break;
                    }
                }
            }
            UnitAIType::UNITAI_CITY_DEFENSE => {
                if info.combat > 0 && !info.no_defensive_bonus && info.city_defense_modifier > 0 {
                    valid = true;
                }
            }
            UnitAIType::UNITAI_EXPLORE => {
                if info.combat > 0 && ai_unit_impassable_count(g, p, unit) == 0 {
                    valid = true;
                }
            }
            // AI: the other UnitAIs (not requested by rules code).
            _ => return 0,
        }
    }

    if !valid {
        return 0;
    }

    // FfH: Modified by Kael 05/07/2008 (CvPlayerAI::AI_combatValue, times 3)
    let combat_value = ai_combat_value(g, p, unit) * 3;

    let mut value = 1;

    value += info.ai_weight;

    match unit_ai {
        UnitAIType::UNITAI_WORKER => {
            for build in d.builds.keys() {
                if info.builds[build] {
                    value += 50;
                }
            }
            value += info.moves * 100;
        }
        UnitAIType::UNITAI_CITY_DEFENSE => {
            value += (combat_value * 2) / 3;
            value += (combat_value * info.city_defense_modifier) / 75;
        }
        UnitAIType::UNITAI_EXPLORE => {
            value += combat_value / 2;
            value += info.moves * 200;
            if info.no_bad_goodies {
                value += 100;
            }
        }
        // AI: the other UnitAIs (not requested by rules code).
        _ => return 0,
    }

    // (The ATTACK/ATTACK_CITY amphibious bonus needs an area; addFreeUnitAI passes NULL.)

    max(0, value)
}

/// AI: C++ `CvPlayerAI::AI_unitAIDomainType` (None = NO_DOMAIN).
fn ai_unit_ai_domain_type(unit_ai: UnitAIType) -> Option<DomainType> {
    match unit_ai {
        UnitAIType::UNITAI_UNKNOWN => None,

        UnitAIType::UNITAI_ANIMAL
        | UnitAIType::UNITAI_SETTLE
        | UnitAIType::UNITAI_WORKER
        | UnitAIType::UNITAI_ATTACK
        | UnitAIType::UNITAI_ATTACK_CITY
        | UnitAIType::UNITAI_COLLATERAL
        | UnitAIType::UNITAI_PILLAGE
        | UnitAIType::UNITAI_RESERVE
        | UnitAIType::UNITAI_COUNTER
        | UnitAIType::UNITAI_PARADROP
        | UnitAIType::UNITAI_CITY_DEFENSE
        | UnitAIType::UNITAI_CITY_COUNTER
        | UnitAIType::UNITAI_CITY_SPECIAL
        | UnitAIType::UNITAI_EXPLORE
        | UnitAIType::UNITAI_MISSIONARY
        | UnitAIType::UNITAI_PROPHET
        | UnitAIType::UNITAI_ARTIST
        | UnitAIType::UNITAI_SCIENTIST
        | UnitAIType::UNITAI_GENERAL
        | UnitAIType::UNITAI_MERCHANT
        | UnitAIType::UNITAI_ENGINEER
        | UnitAIType::UNITAI_SPY
        | UnitAIType::UNITAI_ATTACK_CITY_LEMMING
        // BETTER AI (New UNITAI) Sephi
        | UnitAIType::UNITAI_MAGE
        | UnitAIType::UNITAI_TERRAFORMER
        | UnitAIType::UNITAI_MANA_UPGRADE
        | UnitAIType::UNITAI_WARWIZARD
        | UnitAIType::UNITAI_BARBSMASHER
        | UnitAIType::UNITAI_HERO
        | UnitAIType::UNITAI_FEASTING
        | UnitAIType::UNITAI_MEDIC => Some(DomainType::DOMAIN_LAND),

        UnitAIType::UNITAI_ICBM => Some(DomainType::DOMAIN_IMMOBILE),

        UnitAIType::UNITAI_WORKER_SEA
        | UnitAIType::UNITAI_ATTACK_SEA
        | UnitAIType::UNITAI_RESERVE_SEA
        | UnitAIType::UNITAI_ESCORT_SEA
        | UnitAIType::UNITAI_EXPLORE_SEA
        | UnitAIType::UNITAI_ASSAULT_SEA
        | UnitAIType::UNITAI_SETTLER_SEA
        | UnitAIType::UNITAI_MISSIONARY_SEA
        | UnitAIType::UNITAI_SPY_SEA
        | UnitAIType::UNITAI_CARRIER_SEA
        | UnitAIType::UNITAI_MISSILE_CARRIER_SEA
        | UnitAIType::UNITAI_PIRATE_SEA => Some(DomainType::DOMAIN_SEA),

        UnitAIType::UNITAI_ATTACK_AIR | UnitAIType::UNITAI_DEFENSE_AIR | UnitAIType::UNITAI_CARRIER_AIR | UnitAIType::UNITAI_MISSILE_AIR => {
            Some(DomainType::DOMAIN_AIR)
        }

        // C++ FAssert(false)
        _ => None,
    }
}

/// AI: C++ `CvPlayerAI::AI_unitImpassableCount`: impassable terrains/features the team cannot
/// pass yet.
fn ai_unit_impassable_count(g: &Game, p: PlayerId, unit: UnitType) -> i32 {
    let d = g.d;
    let info = &d.units[unit];
    let team = g.players[p].team;
    let mut count = 0;

    for terrain in d.terrains.keys() {
        if info.terrain_impassable[terrain] {
            let tech = info.terrain_passable_tech[terrain];
            if tech.is_none() || !g.team_is_has_tech(team, tech) {
                count += 1;
            }
        }
    }

    for feature in d.features.keys() {
        if info.feature_impassable[feature] {
            let tech = info.feature_passable_tech[feature];
            if tech.is_none() || !g.team_is_has_tech(team, tech) {
                count += 1;
            }
        }
    }

    count
}

/// AI: C++ `CvPlayerAI::AI_combatValue` (FfH; replaces `CvGameAI::AI_combatValue`).
fn ai_combat_value(g: &Game, p: PlayerId, unit: UnitType) -> i32 {
    let d = g.d;
    let info = &d.units[unit];
    let mut value = 100 * ai_true_combat_value(g, p, unit);
    value *= (((info.first_strikes * 2) + info.chance_first_strikes) * (d.defines.combat_damage / 5)) + 100;
    value /= 100;
    value /= g.best_land_unit_combat();
    value
}

/// AI: C++ `CvPlayerAI::AI_trueCombatValue` (FfH): base strength plus damage-type strength,
/// bonus affinities and the weapon promotion the player's resources would give.
fn ai_true_combat_value(g: &Game, p: PlayerId, unit: UnitType) -> i32 {
    let d = g.d;
    let info = &d.units[unit];

    let mut combat = if info.domain_type == Some(DomainType::DOMAIN_AIR) { info.air_combat } else { info.combat };

    for damage_type in d.damages.keys() {
        combat += info.damage_type_combat[damage_type];
    }

    for bonus in d.bonuses.keys() {
        combat += info.bonus_affinity[bonus] * g.player_get_num_available_bonuses(p, bonus);
    }

    if info.weapon_tier > 0 {
        // (C++ indexes the WEAPON_REQ_BONUS_TIER* defines unchecked; they name bonuses.)
        let available = |bonus_define: i32| opt_from_i32::<BonusType>(bonus_define).map_or(0, |b| g.player_get_num_available_bonuses(p, b));

        let mut combat_bonus = 0;
        if let Some(promotion) = opt_from_i32::<PromotionType>(d.defines.weapon_promotion_tier1) {
            if available(d.defines.weapon_req_bonus_tier1) > 0 {
                combat_bonus = d.promotions[promotion].extra_combat_str;
            }
        }
        if let Some(promotion) = opt_from_i32::<PromotionType>(d.defines.weapon_promotion_tier2) {
            if info.weapon_tier > 1 && available(d.defines.weapon_req_bonus_tier2) > 0 {
                combat_bonus = d.promotions[promotion].extra_combat_str;
            }
        }
        if let Some(promotion) = opt_from_i32::<PromotionType>(d.defines.weapon_promotion_tier3) {
            if info.weapon_tier > 2 && available(d.defines.weapon_req_bonus_tier3) > 0 {
                combat_bonus = d.promotions[promotion].extra_combat_str;
            }
        }
        combat += combat_bonus;
    }

    combat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, MapInitData};
    use crate::player::Player;
    use crate::setup::GameSetup;

    /// Two AI civs on a `w` x `h` (non-wrapping, all ocean) map without areas; players are
    /// reset (`Player::new`) but not initialized.
    fn game(w: i32, h: i32) -> Game {
        let d = ffh_data::global();
        let setup = GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 7);
        let mut g = Game::new(d, setup);
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        g.map = Map::new(d, &MapInitData { grid_w: w, grid_h: h, ..MapInitData::default() }, num_players, num_teams);
        g
    }

    #[test]
    fn starting_plot_tracks_area_counts() {
        let mut g = game(6, 4);
        let west = g.map_add_area(false);
        let east = g.map_add_area(false);
        for plot in g.map.plot_ids() {
            let (x, _) = g.map.xy(plot);
            g.map.plots[plot].area = Some(if x < 3 { west } else { east });
        }
        let p = PlayerId(0);
        assert_eq!(g.player_get_starting_plot(p), None);

        let first = g.map.plot_xy(1, 1).unwrap();
        g.player_set_starting_plot(p, Some(first), false);
        assert_eq!(g.player_get_starting_plot(p), Some(first));
        assert_eq!((g.players[p].starting_x, g.players[p].starting_y), (1, 1));
        assert_eq!(g.map.areas[west].num_starting_plots, 1);

        // Setting the same plot again changes nothing.
        g.player_set_starting_plot(p, Some(first), false);
        assert_eq!(g.map.areas[west].num_starting_plots, 1);

        let second = g.map.plot_xy(4, 2).unwrap();
        g.player_set_starting_plot(p, Some(second), false);
        assert_eq!(g.map.areas[west].num_starting_plots, 0);
        assert_eq!(g.map.areas[east].num_starting_plots, 1);

        g.player_set_starting_plot(p, None, false);
        assert_eq!(g.map.areas[east].num_starting_plots, 0);
        assert_eq!((g.players[p].starting_x, g.players[p].starting_y), (INVALID_PLOT_COORD, INVALID_PLOT_COORD));
        assert_eq!(g.player_get_starting_plot(p), None);
    }

    #[test]
    fn starting_loc_found_value_reset_and_best_found_value() {
        let mut g = game(4, 3);
        let land = g.map_add_area(false);
        let water = g.map_add_area(true);
        let (p, other) = (PlayerId(1), PlayerId(0));
        assert_eq!(g.player_find_best_found_value(p), 0);

        g.map.areas[land].best_found_value[p] = 120;
        g.map.areas[water].best_found_value[p] = 340;
        g.map.areas[water].best_found_value[other] = 999;
        g.map.plots[3].found_value[other] = 55;
        assert_eq!(g.player_find_best_found_value(p), 340);

        ai_update_found_values_starting_loc(&mut g, p);
        assert_eq!(g.player_find_best_found_value(p), 0);
        assert!(g.map.plots.iter().all(|plot| plot.found_value[p] == -1));
        // Other players' caches are untouched.
        assert_eq!(g.map.plots[3].found_value[other], 55);
        assert_eq!(g.player_find_best_found_value(other), 999);
    }

    #[test]
    fn init_free_state_sets_starting_gold_and_clears_research() {
        let mut g = game(2, 2);
        let d = g.d;
        let p = PlayerId(0);
        let tech = TechType(0);
        g.players[p].gold = 77;
        g.players[p].starting_gold = 25;
        g.players[p].research_queue.push(tech);
        g.players[p].researching_tech[tech] = true;

        g.player_init_free_state(p);

        let handicap = g.player_get_handicap_type(p).unwrap();
        let expected = d.handicaps[handicap].starting_gold + d.eras[g.start_era()].starting_gold + 25;
        assert_eq!(g.players[p].gold, expected);
        assert!(g.players[p].research_queue.is_empty());
        assert!(!g.players[p].researching_tech[tech]);
    }

    #[test]
    fn advanced_start_costs_need_a_city() {
        let g = game(3, 3);
        let p = PlayerId(0);
        let plot = g.map.plot_xy(1, 1);
        assert_eq!(g.player_get_advanced_start_unit_cost(p, UnitType(0), true, plot), -1);
        assert_eq!(g.player_get_advanced_start_unit_cost(p, UnitType(0), false, None), -1);
        assert_eq!(g.player_get_advanced_start_pop_cost(p, true, None), -1);
        assert_eq!(g.player_get_advanced_start_culture_cost(p, true, None), -1);
        assert_eq!(g.player_get_advanced_start_building_cost(p, BuildingType(0), true, None), -1);
        assert_eq!(g.player_get_advanced_start_route_cost(p, Some(RouteType(0)), true, plot), -1);
        assert_eq!(g.player_get_advanced_start_improvement_cost(p, Some(ImprovementType(0)), true, plot), -1);
        assert_eq!(g.player_get_advanced_start_tech_cost(p, Some(TechType(0)), true), -1);
        assert_eq!(g.player_get_advanced_start_visibility_cost(p, true, plot), -1);
        // NO_X arguments.
        assert_eq!(g.player_get_advanced_start_route_cost(p, None, false, None), -1);
        assert_eq!(g.player_get_advanced_start_improvement_cost(p, None, false, None), -1);
        assert_eq!(g.player_get_advanced_start_tech_cost(p, None, false), -1);
    }

    #[test]
    fn advanced_start_action_guards_and_exit() {
        let mut g = game(3, 3);
        let p = PlayerId(0);

        // Not in advanced start (points -1): nothing happens.
        g.player_do_advanced_start_action(p, AdvancedStartAction::Exit, 0, 0, -1, true);
        assert_eq!((g.players[p].gold, g.players[p].advanced_start_points), (0, -1));

        // Without a city only EXIT, AUTOMATE and CITY are accepted.
        g.players[p].advanced_start_points = 50;
        g.player_do_advanced_start_action(p, AdvancedStartAction::Tech, 0, 0, 0, true);
        g.player_do_advanced_start_action(p, AdvancedStartAction::Visibility, 1, 1, -1, true);
        g.player_do_advanced_start_action(p, AdvancedStartAction::Automate, 1, 1, -1, true);
        assert_eq!((g.players[p].gold, g.players[p].advanced_start_points), (0, 50));

        // EXIT turns the remaining points into gold and leaves advanced start (an AI player:
        // no production/tech choices).
        g.player_do_advanced_start_action(p, AdvancedStartAction::Exit, 0, 0, -1, true);
        assert_eq!((g.players[p].gold, g.players[p].advanced_start_points), (50, -1));
    }

    #[test]
    fn distance_factor_is_neutral_without_a_start() {
        let g = game(4, 4);
        let plot = g.map.plot_xy(2, 2).unwrap();
        assert_eq!(g.player_starting_plot_distance_factor(PlayerId(1), plot, PlayerId(0), 7), 1000);
    }

    #[test]
    fn starting_area_ignores_water() {
        let mut g = game(3, 3);
        let water = g.map_add_area(true);
        g.map.areas[water].num_tiles = 9;
        assert_eq!(g.player_find_starting_area(PlayerId(0)), None);
    }

    #[test]
    fn unit_ai_domains() {
        assert_eq!(ai_unit_ai_domain_type(UnitAIType::UNITAI_UNKNOWN), None);
        let land_ais = [
            UnitAIType::UNITAI_SETTLE,
            UnitAIType::UNITAI_WORKER,
            UnitAIType::UNITAI_CITY_DEFENSE,
            UnitAIType::UNITAI_EXPLORE,
            UnitAIType::UNITAI_MEDIC,
        ];
        for land in land_ais {
            assert_eq!(ai_unit_ai_domain_type(land), Some(DomainType::DOMAIN_LAND));
        }
        assert_eq!(ai_unit_ai_domain_type(UnitAIType::UNITAI_ICBM), Some(DomainType::DOMAIN_IMMOBILE));
        assert_eq!(ai_unit_ai_domain_type(UnitAIType::UNITAI_PIRATE_SEA), Some(DomainType::DOMAIN_SEA));
        assert_eq!(ai_unit_ai_domain_type(UnitAIType::UNITAI_MISSILE_AIR), Some(DomainType::DOMAIN_AIR));
    }

}
