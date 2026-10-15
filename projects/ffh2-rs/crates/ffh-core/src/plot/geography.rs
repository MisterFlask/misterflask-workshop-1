//! CvPlot, topic "geography": plot type changes and areas, adjacency, latitude, rivers,
//! irrigation, fresh water, potential city work, and yields.

use crate::defs::NUM_CITY_PLOTS;
use crate::game::Game;
use crate::ids::*;
use crate::path::{FinderCtx, ASNL_ADDCLOSED};
use crate::utils::{get_population_power, shuffle_array, NUM_CITY_PLOTS_RADIUS_2};
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Plot type, areas
    // ------------------------------------------------------------------

    /// C++ `CvPlot::area()`: the plot's area, None if the plot has none (C++
    /// `getArea(FFreeList::INVALID_INDEX)` is NULL) or the area no longer exists.
    #[inline]
    pub(crate) fn plot_area(&self, p: PlotIdx) -> Option<AreaId> {
        self.map.plots[p].area.filter(|&a| self.map.areas.contains(a))
    }

    /// C++ `pPlot->area()->X` where the C++ dereferences `area()` without a NULL check (every
    /// plot has an area once `CvMap::calculateAreas` has run).
    fn plot_area_expect(&self, p: PlotIdx) -> AreaId {
        self.plot_area(p).expect("CvPlot::area() is NULL (areas not calculated)")
    }

    /// C++ `CvPlot::setPlotType` (`bRebuildGraphics` dropped: graphics only; C++ default
    /// `bRecalculate = true`). Water <-> land changes re-terrain the neighbours and either
    /// recalculate all areas or move this plot to a new/adjacent area.
    pub fn plot_set_plot_type(&mut self, p: PlotIdx, plot_type: PlotType, recalculate: bool) {
        if self.map.plots[p].plot_type == plot_type {
            return;
        }

        if self.map.plots[p].plot_type == PlotType::PLOT_OCEAN || plot_type == PlotType::PLOT_OCEAN {
            self.plot_erase(p);
        }

        let was_water = self.plot_is_water(p);

        self.plot_update_see_from_sight(p, false, true);

        self.map.plots[p].plot_type = plot_type;

        self.plot_update_yield(p);
        self.plot_update_plot_group(p);

        self.plot_update_see_from_sight(p, true, true);

        let shallow_water = opt_from_i32::<TerrainType>(self.d.defines.shallow_water_terrain).expect("SHALLOW_WATER_TERRAIN");
        let deep_water = opt_from_i32::<TerrainType>(self.d.defines.deep_water_terrain).expect("DEEP_WATER_TERRAIN");
        let land = opt_from_i32::<TerrainType>(self.d.defines.land_terrain).expect("LAND_TERRAIN");

        let terrain_matches = match self.map.plots[p].terrain_type {
            Some(t) => self.d.terrains[t].water == self.plot_is_water(p),
            None => false,
        };
        if !terrain_matches {
            if self.plot_is_water(p) {
                if self.plot_is_adjacent_to_land(p) {
                    self.plot_set_terrain_type(p, shallow_water, recalculate);
                } else {
                    self.plot_set_terrain_type(p, deep_water, recalculate);
                }
            } else {
                self.plot_set_terrain_type(p, land, recalculate);
            }
        }

        self.map_reset_path_distance();

        if was_water != self.plot_is_water(p) {
            let (x, y) = self.map.xy(p);

            if recalculate {
                for i in 0..DirectionType::NUM {
                    if let Some(loop_plot) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                        if self.plot_is_water(loop_plot) {
                            if self.plot_is_adjacent_to_land(loop_plot) {
                                self.plot_set_terrain_type(loop_plot, shallow_water, recalculate);
                            } else {
                                self.plot_set_terrain_type(loop_plot, deep_water, recalculate);
                            }
                        }
                    }
                }
            }

            for i in 0..DirectionType::NUM {
                if let Some(loop_plot) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                    self.plot_update_yield(loop_plot);
                    self.plot_update_plot_group(loop_plot);
                }
            }

            for i in 0..NUM_CITY_PLOTS {
                if let Some(loop_plot) = self.plot_city(x, y, i) {
                    self.plot_update_potential_city_work(loop_plot);
                }
            }

            let land_change = if self.plot_is_water(p) { -1 } else { 1 };

            self.map.land_plots += land_change;

            if let Some(bonus) = self.map.plots[p].bonus_type {
                self.map.num_bonus_on_land[bonus] += land_change;
            }

            if let Some(owner) = self.map.plots[p].owner {
                self.players[owner].total_land += land_change;
                let team = self.plot_get_team(p).expect("owned plot has a team");
                self.teams[team].total_land += land_change;
                // Score v2 reads total_land (docs/SCORE.md).
                self.score_dirty = true;
            }

            if recalculate {
                let mut new_area: Option<AreaId> = None;
                let mut recalculate_areas = false;

                // XXX might want to change this if we allow diagonal water movement...
                if self.plot_is_water(p) {
                    for i in 0..CardinalDirectionType::NUM {
                        if let Some(loop_plot) = self.plot_cardinal_direction(x, y, CardinalDirectionType::from_usize(i)) {
                            let loop_area = self.plot_area_expect(loop_plot);
                            if self.map.areas[loop_area].water {
                                if new_area.is_none() {
                                    new_area = Some(loop_area);
                                } else if new_area != Some(loop_area) {
                                    recalculate_areas = true;
                                    break;
                                }
                            }
                        }
                    }
                } else {
                    for i in 0..DirectionType::NUM {
                        if let Some(loop_plot) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                            let loop_area = self.plot_area_expect(loop_plot);
                            if !self.map.areas[loop_area].water {
                                if new_area.is_none() {
                                    new_area = Some(loop_area);
                                } else if new_area != Some(loop_area) {
                                    recalculate_areas = true;
                                    break;
                                }
                            }
                        }
                    }
                }

                if !recalculate_areas {
                    let last_dir = DirectionType::from_usize(DirectionType::NUM - 1);
                    let mut last_area = self.plot_direction(x, y, last_dir).and_then(|q| self.plot_area(q));

                    let mut area_count = 0;

                    for i in 0..DirectionType::NUM {
                        let curr_area = self.plot_direction(x, y, DirectionType::from_usize(i)).and_then(|q| self.plot_area(q));

                        if curr_area != last_area {
                            area_count += 1;
                        }

                        last_area = curr_area;
                    }

                    if area_count > 2 {
                        recalculate_areas = true;
                    }
                }

                if recalculate_areas {
                    self.map_recalculate_areas();
                } else {
                    self.plot_set_area(p, None);

                    // C++ quirk: `area()` is already NULL after `setArea(INVALID_INDEX)`, so this
                    // never deletes anything: the plot's old one-tile area stays in the area list
                    // with no tiles.
                    if let Some(old_area) = self.plot_area(p) {
                        if self.map.areas[old_area].num_tiles == 1 {
                            self.map_delete_area(old_area);
                        }
                    }

                    let new_area = match new_area {
                        Some(a) => a,
                        None => {
                            let water = self.plot_is_water(p);
                            self.map_add_area(water)
                        }
                    };

                    self.plot_set_area(p, Some(new_area));
                }
            }
        }

        // bRebuildGraphics: RebuildPlot, feature/route/river symbols, layout -- graphics only.
    }

    /// C++ `CvPlot::setArea` (`getArea()` / `area()` are the `area` field).
    pub fn plot_set_area(&mut self, p: PlotIdx, area: Option<AreaId>) {
        if self.map.plots[p].area != area {
            // C++ also computes `bOldLake = isLake()` here and never uses it.
            if let Some(old_area) = self.plot_area(p) {
                self.plot_process_area(p, old_area, -1);
            }

            self.map.plots[p].area = area;

            if let Some(new_area) = self.plot_area(p) {
                self.plot_process_area(p, new_area, 1);

                self.plot_update_irrigated(p);
                self.plot_update_yield(p);
            }
        }
    }

    /// C++ `CvPlot::processArea` (protected): adds (`change = 1`) or removes (`-1`) this
    /// plot's contribution to an area's counters.
    pub fn plot_process_area(&mut self, p: PlotIdx, area: AreaId, change: i32) {
        // XXX am not updating getBestFoundValue() or getAreaAIType()...

        self.area_change_num_tiles(area, change);

        if self.plot_is_owned(p) {
            self.map.areas[area].num_owned_tiles += change;
        }

        if self.map.plots[p].n_of_river {
            self.map.areas[area].num_river_edges += change;
        }
        if self.map.plots[p].w_of_river {
            self.map.areas[area].num_river_edges += change;
        }

        if let Some(bonus) = self.map.plots[p].bonus_type {
            self.map.areas[area].num_bonuses[bonus] += change;
        }

        if let Some(improvement) = self.map.plots[p].improvement_type {
            self.map.areas[area].num_improvements[improvement] += change;
        }

        for i in self.player_ids() {
            if self.player_get_starting_plot(i) == Some(p) {
                self.map.areas[area].num_starting_plots += change;
            }

            let unit_power = self.plot_get_unit_power(p, Some(i));
            self.map.areas[area].power[i] += unit_power * change;

            let units = self.plot_plot_count(p, Some(Game::puf_is_player), i.i(), -1, None, None, None, -1, -1);
            self.area_change_units_per_player(area, i, units * change);
            let animals = self.plot_plot_count(p, Some(Game::puf_is_animal), -1, -1, Some(i), None, None, -1, -1);
            self.map.areas[area].animals_per_player[i] += animals * change;

            for j in 0..UnitAIType::NUM {
                let unit_ai = UnitAIType::from_usize(j);
                let count = self.plot_plot_count(p, Some(Game::puf_is_unit_ai_type), unit_ai.i(), -1, Some(i), None, None, -1, -1);
                self.map.areas[area].num_ai_units[i][unit_ai] += count * change;
            }
        }

        for t in self.team_ids() {
            if self.map.plots[p].revealed[t] {
                self.map.areas[area].num_revealed_tiles[t] += change;
            }
        }

        if let Some(city) = self.map.plots[p].plot_city {
            let owner = self.cities[city].owner;
            let population = self.cities[city].population;

            // XXX make sure all of this (esp. the changePower()) syncs up...
            self.map.areas[area].power[owner] += get_population_power(population) * change;

            self.area_change_cities_per_player(area, owner, change);
            self.area_change_population_per_player(area, owner, population * change);

            let d = self.d;
            for (b, building) in d.buildings.iter_enumerated() {
                let num_active = self.city_num_active_building(city, b);
                if num_active > 0 {
                    self.map.areas[area].power[owner] += building.power_value * change * num_active;

                    if building.area_health > 0 {
                        self.area_change_building_good_health(area, owner, building.area_health * change * num_active);
                    } else {
                        self.area_change_building_bad_health(area, owner, building.area_health * change * num_active);
                    }
                    self.area_change_building_happiness(area, owner, building.area_happiness * change * num_active);
                    self.area_change_free_specialist(area, owner, building.area_free_specialist * change * num_active);

                    let team = self.city_team(city);
                    let clean_power = if building.area_clean_power { change * num_active } else { 0 };
                    self.area_change_clean_power_count(area, team, clean_power);

                    let border_obstacle = if building.area_border_obstacle { change * num_active } else { 0 };
                    self.area_change_border_obstacle_count(area, team, border_obstacle);

                    for j in 0..YieldType::NUM {
                        let modifier = building.area_yield_modifier[j] * change * num_active;
                        self.area_change_yield_rate_modifier(area, owner, YieldType::from_usize(j), modifier);
                    }
                }
            }

            for j in 0..UnitAIType::NUM {
                let unit_ai = UnitAIType::from_usize(j);
                let count = self.city_num_train_unit_ai(city, unit_ai);
                self.map.areas[area].num_train_ai_units[owner][unit_ai] += count * change;
            }

            for i in self.player_ids() {
                if self.area_get_target_city(area, i) == Some(city) {
                    self.map.areas[area].target_cities[i] = None;
                }
            }
        }
    }

    /// C++ `CvPlot::waterArea`: own area if water, else the biggest adjacent water area.
    pub fn plot_water_area(&self, p: PlotIdx) -> Option<AreaId> {
        if self.plot_is_water(p) {
            return self.plot_area(p);
        }

        let mut best_value = 0;
        let mut best_area = None;

        let (x, y) = self.map.xy(p);
        for i in 0..DirectionType::NUM {
            if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                if self.plot_is_water(adjacent) {
                    let adjacent_area = self.plot_area_expect(adjacent);
                    let value = self.map.areas[adjacent_area].num_tiles;

                    if value > best_value {
                        best_value = value;
                        best_area = Some(adjacent_area);
                    }
                }
            }
        }

        best_area
    }

    /// C++ `CvPlot::secondWaterArea`: the biggest adjacent water area other than `waterArea`.
    pub fn plot_second_water_area(&self, p: PlotIdx) -> Option<AreaId> {
        let water_area = self.plot_water_area(p);

        // C++: FAssert(!isWater()).

        let mut best_value = 0;
        let mut best_area = None;

        let (x, y) = self.map.xy(p);
        for i in 0..DirectionType::NUM {
            if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                // C++ compares `getArea()` with `pWaterArea->getID()` (never NULL here: an
                // adjacent water plot makes `waterArea()` non-NULL).
                if self.plot_is_water(adjacent) && self.map.plots[adjacent].area != water_area {
                    let adjacent_area = self.plot_area_expect(adjacent);
                    let value = self.map.areas[adjacent_area].num_tiles;

                    if value > best_value {
                        best_value = value;
                        best_area = Some(adjacent_area);
                    }
                }
            }
        }

        best_area
    }

    /// C++ `CvPlot::isAdjacentToArea` (both overloads: `int iAreaID` and `const CvArea*`).
    pub fn plot_is_adjacent_to_area(&self, p: PlotIdx, area: AreaId) -> bool {
        self.plot_is_adjacent_to_area_id(p, Some(area))
    }

    /// C++ `CvPlot::isAdjacentToArea(int iAreaID)` with any id, including
    /// `FFreeList::INVALID_INDEX` (= None, matching plots without an area).
    fn plot_is_adjacent_to_area_id(&self, p: PlotIdx, area: Option<AreaId>) -> bool {
        let (x, y) = self.map.xy(p);
        for i in 0..DirectionType::NUM {
            if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                if self.map.plots[adjacent].area == area {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvPlot::shareAdjacentArea`.
    pub fn plot_share_adjacent_area(&self, p: PlotIdx, other: PlotIdx) -> bool {
        let mut last_area: Option<AreaId> = None; // FFreeList::INVALID_INDEX

        let (x, y) = self.map.xy(p);
        for i in 0..DirectionType::NUM {
            if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                let curr_area = self.map.plots[adjacent].area;

                if curr_area != last_area {
                    if self.plot_is_adjacent_to_area_id(other, curr_area) {
                        return true;
                    }

                    last_area = curr_area;
                }
            }
        }

        false
    }

    /// C++ `CvPlot::isAdjacentToLand`.
    pub fn plot_is_adjacent_to_land(&self, p: PlotIdx) -> bool {
        let (x, y) = self.map.xy(p);
        for i in 0..DirectionType::NUM {
            if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                if !self.plot_is_water(adjacent) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvPlot::isAdjacentToWater` (FFH).
    pub fn plot_is_adjacent_to_water(&self, p: PlotIdx) -> bool {
        let (x, y) = self.map.xy(p);
        for i in 0..DirectionType::NUM {
            if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                if self.plot_is_water(adjacent) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvPlot::isCoastalLand` (C++ default `iMinWaterSize = -1`).
    pub fn plot_is_coastal_land(&self, p: PlotIdx, min_water_size: i32) -> bool {
        if self.plot_is_water(p) {
            return false;
        }

        let (x, y) = self.map.xy(p);
        for i in 0..DirectionType::NUM {
            if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                if self.plot_is_water(adjacent) && self.map.areas[self.plot_area_expect(adjacent)].num_tiles >= min_water_size {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvPlot::getNearestLandPlotInternal` (recursive ring search; the tail recursion is
    /// a loop here).
    pub fn plot_get_nearest_land_plot_internal(&self, p: PlotIdx, distance: i32) -> Option<PlotIdx> {
        let (x, y) = self.map.xy(p);
        let mut distance = distance;
        loop {
            if distance > self.map.grid_height && distance > self.map.grid_width {
                return None;
            }

            for dx in -distance..=distance {
                for dy in -distance..=distance {
                    if dx.abs() + dy.abs() == distance {
                        if let Some(plot) = self.plot_xy(x, y, dx, dy) {
                            if !self.plot_is_water(plot) {
                                return Some(plot);
                            }
                        }
                    }
                }
            }

            distance += 1;
        }
    }

    /// C++ `CvPlot::getNearestLandArea` (C++ -1 = None).
    pub fn plot_get_nearest_land_area(&self, p: PlotIdx) -> Option<AreaId> {
        // C++ `pPlot->getArea()`: the raw id.
        self.plot_get_nearest_land_plot(p).and_then(|plot| self.map.plots[plot].area)
    }

    /// C++ `CvPlot::getNearestLandPlot`.
    pub fn plot_get_nearest_land_plot(&self, p: PlotIdx) -> Option<PlotIdx> {
        self.plot_get_nearest_land_plot_internal(p, 0)
    }

    /// C++ `CvPlot::at`.
    #[inline]
    pub fn plot_at(&self, p: PlotIdx, x: i32, y: i32) -> bool {
        let pl = &self.map.plots[p];
        pl.x == x && pl.y == y
    }

    /// C++ `CvPlot::getLatitude` (the UNOFFICIAL_PATCH double-precision version used by 0.41).
    pub fn plot_get_latitude(&self, p: PlotIdx) -> i32 {
        let m = &self.map;
        let pl = &m.plots[p];
        // C++ quirk: a 1-plot-high (or wide) map divides by zero here; `as i32` saturates
        // where the C++ cast is undefined.
        let mut f_latitude: f64 = if m.wrap_x || !m.wrap_y {
            (pl.y as f64 * 1.0) / ((m.grid_height - 1) as f64)
        } else {
            (pl.x as f64 * 1.0) / ((m.grid_width - 1) as f64)
        };
        f_latitude *= (m.top_latitude - m.bottom_latitude) as f64;
        let i_latitude = (f_latitude + 0.5) as i32;
        (i_latitude + m.bottom_latitude).abs()
    }

    // ------------------------------------------------------------------
    // Water, rivers, irrigation
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isLake`: the plot's area is a lake.
    pub fn plot_is_lake(&self, p: PlotIdx) -> bool {
        match self.plot_area(p) {
            Some(a) => self.area_is_lake(a),
            None => false,
        }
    }

    /// C++ `CvPlot::isFreshWater`: river, adjacent lake or fresh-water feature.
    pub fn plot_is_fresh_water(&self, p: PlotIdx) -> bool {
        if self.plot_is_water(p) {
            return false;
        }

        if self.plot_is_impassable(p) {
            return false;
        }

        if self.plot_is_river(p) {
            return true;
        }

        let (x, y) = self.map.xy(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                    if self.plot_is_lake(loop_plot) {
                        return true;
                    }

                    if let Some(feature) = self.map.plots[loop_plot].feature_type {
                        if self.d.features[feature].adds_fresh_water {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// C++ `CvPlot::isRiver`.
    #[inline]
    pub fn plot_is_river(&self, p: PlotIdx) -> bool {
        self.map.plots[p].river_crossing_count > 0
    }

    /// C++ `CvPlot::isRiverSide`.
    pub fn plot_is_river_side(&self, p: PlotIdx) -> bool {
        let (x, y) = self.map.xy(p);
        for i in 0..CardinalDirectionType::NUM {
            if let Some(loop_plot) = self.plot_cardinal_direction(x, y, CardinalDirectionType::from_usize(i)) {
                if self.plot_is_river_crossing(p, self.direction_xy_plots(p, loop_plot)) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvPlot::isRiverCrossing` (None = NO_DIRECTION: false).
    #[inline]
    pub fn plot_is_river_crossing(&self, p: PlotIdx, dir: Option<DirectionType>) -> bool {
        match dir {
            Some(d) => self.map.plots[p].river_crossing[d.idx()],
            None => false,
        }
    }

    /// C++ `CvPlot::isRiverConnection` (None = NO_DIRECTION: false).
    pub fn plot_is_river_connection(&self, p: PlotIdx, dir: Option<DirectionType>) -> bool {
        let Some(dir) = dir else {
            return false;
        };
        let crossing = |d: DirectionType| self.plot_is_river_crossing(p, Some(d));
        match dir {
            DirectionType::DIRECTION_NORTH => crossing(DirectionType::DIRECTION_EAST) || crossing(DirectionType::DIRECTION_WEST),
            DirectionType::DIRECTION_NORTHEAST => crossing(DirectionType::DIRECTION_NORTH) || crossing(DirectionType::DIRECTION_EAST),
            DirectionType::DIRECTION_EAST => crossing(DirectionType::DIRECTION_NORTH) || crossing(DirectionType::DIRECTION_SOUTH),
            DirectionType::DIRECTION_SOUTHEAST => crossing(DirectionType::DIRECTION_SOUTH) || crossing(DirectionType::DIRECTION_EAST),
            DirectionType::DIRECTION_SOUTH => crossing(DirectionType::DIRECTION_EAST) || crossing(DirectionType::DIRECTION_WEST),
            DirectionType::DIRECTION_SOUTHWEST => crossing(DirectionType::DIRECTION_SOUTH) || crossing(DirectionType::DIRECTION_WEST),
            DirectionType::DIRECTION_WEST => crossing(DirectionType::DIRECTION_NORTH) || crossing(DirectionType::DIRECTION_SOUTH),
            DirectionType::DIRECTION_NORTHWEST => crossing(DirectionType::DIRECTION_NORTH) || crossing(DirectionType::DIRECTION_WEST),
            // C++: FAssert(false)
            _ => false,
        }
    }

    /// C++ `CvPlot::updateRiverCrossing(DirectionTypes)`.
    pub fn plot_update_river_crossing_dir(&mut self, p: PlotIdx, dir: DirectionType) {
        let (x, y) = self.map.xy(p);

        let mut corner_plot: Option<PlotIdx> = None;
        let mut valid = false;
        let plot = self.plot_direction(x, y, dir);

        if plot.is_none_or(|q| !self.plot_is_water(q)) && !self.plot_is_water(p) {
            match dir {
                DirectionType::DIRECTION_NORTH => {
                    if let Some(q) = plot {
                        valid = self.map.plots[q].n_of_river;
                    }
                }
                DirectionType::DIRECTION_NORTHEAST => {
                    corner_plot = self.plot_direction(x, y, DirectionType::DIRECTION_NORTH);
                }
                DirectionType::DIRECTION_EAST => {
                    valid = self.map.plots[p].w_of_river;
                }
                DirectionType::DIRECTION_SOUTHEAST => {
                    corner_plot = Some(p);
                }
                DirectionType::DIRECTION_SOUTH => {
                    valid = self.map.plots[p].n_of_river;
                }
                DirectionType::DIRECTION_SOUTHWEST => {
                    corner_plot = self.plot_direction(x, y, DirectionType::DIRECTION_WEST);
                }
                DirectionType::DIRECTION_WEST => {
                    if let Some(q) = plot {
                        valid = self.map.plots[q].w_of_river;
                    }
                }
                DirectionType::DIRECTION_NORTHWEST => {
                    corner_plot = self.plot_direction(x, y, DirectionType::DIRECTION_NORTHWEST);
                }
                // C++: FAssert(false)
                _ => {}
            }

            if let Some(corner) = corner_plot {
                let (cx, cy) = self.map.xy(corner);
                let north_east = self.plot_direction(cx, cy, DirectionType::DIRECTION_EAST);
                let south_east = self.plot_direction(cx, cy, DirectionType::DIRECTION_SOUTHEAST);
                let south_west = self.plot_direction(cx, cy, DirectionType::DIRECTION_SOUTH);
                let north_west = Some(corner);

                if let (Some(sw), Some(nw), Some(se), Some(ne)) = (south_west, north_west, south_east, north_east) {
                    let n_of_river = |q: PlotIdx| self.map.plots[q].n_of_river;
                    let w_of_river = |q: PlotIdx| self.map.plots[q].w_of_river;
                    let water = |q: PlotIdx| self.plot_is_water(q);

                    if w_of_river(sw) && w_of_river(nw) {
                        valid = true;
                    } else if n_of_river(ne) && n_of_river(nw) {
                        valid = true;
                    } else if dir == DirectionType::DIRECTION_NORTHEAST || dir == DirectionType::DIRECTION_SOUTHWEST {
                        if n_of_river(ne) && (w_of_river(nw) || water(nw)) {
                            valid = true;
                        } else if (n_of_river(ne) || water(se)) && w_of_river(nw) {
                            valid = true;
                        } else if w_of_river(sw) && (n_of_river(nw) || water(nw)) {
                            valid = true;
                        } else if (w_of_river(sw) || water(se)) && n_of_river(nw) {
                            valid = true;
                        }
                    } else {
                        debug_assert!(dir == DirectionType::DIRECTION_SOUTHEAST || dir == DirectionType::DIRECTION_NORTHWEST);

                        if n_of_river(nw) && (w_of_river(nw) || water(ne)) {
                            valid = true;
                        } else if (n_of_river(nw) || water(sw)) && w_of_river(nw) {
                            valid = true;
                        } else if n_of_river(ne) && (w_of_river(sw) || water(sw)) {
                            valid = true;
                        } else if (n_of_river(ne) || water(ne)) && w_of_river(sw) {
                            valid = true;
                        }
                    }
                }
            }
        }

        if self.plot_is_river_crossing(p, Some(dir)) != valid {
            let pl = &mut self.map.plots[p];
            pl.river_crossing[dir.idx()] = valid;

            // changeRiverCrossingCount
            pl.river_crossing_count += if valid { 1 } else { -1 };
            debug_assert!(pl.river_crossing_count >= 0);
        }
    }

    /// C++ `CvPlot::updateRiverCrossing()`: all directions.
    pub fn plot_update_river_crossing(&mut self, p: PlotIdx) {
        for i in 0..DirectionType::NUM {
            self.plot_update_river_crossing_dir(p, DirectionType::from_usize(i));
        }
    }

    /// C++ `CvPlot::setNOfRiver`.
    pub fn plot_set_n_of_river(&mut self, p: PlotIdx, new_value: bool, river_dir: Option<CardinalDirectionType>) {
        if self.map.plots[p].n_of_river != new_value || river_dir != self.map.plots[p].river_we_direction {
            if self.map.plots[p].n_of_river != new_value {
                self.plot_update_plot_group_bonus(p, false);
                self.map.plots[p].n_of_river = new_value;
                self.plot_update_plot_group_bonus(p, true);

                self.plot_update_river_crossing(p);
                self.plot_update_yield(p);

                let (x, y) = self.map.xy(p);
                for i in 0..DirectionType::NUM {
                    if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                        self.plot_update_river_crossing(adjacent);
                        self.plot_update_yield(adjacent);
                    }
                }

                if let Some(area) = self.plot_area(p) {
                    self.map.areas[area].num_river_edges += if self.map.plots[p].n_of_river { 1 } else { -1 };
                }
            }

            debug_assert!(
                matches!(river_dir, None | Some(CardinalDirectionType::CARDINALDIRECTION_WEST) | Some(CardinalDirectionType::CARDINALDIRECTION_EAST)),
                "invalid parameter"
            );
            self.map.plots[p].river_we_direction = river_dir;

            // updateRiverSymbol: graphics only.
        }
    }

    /// C++ `CvPlot::setWOfRiver`.
    pub fn plot_set_w_of_river(&mut self, p: PlotIdx, new_value: bool, river_dir: Option<CardinalDirectionType>) {
        if self.map.plots[p].w_of_river != new_value || river_dir != self.map.plots[p].river_ns_direction {
            if self.map.plots[p].w_of_river != new_value {
                self.plot_update_plot_group_bonus(p, false);
                self.map.plots[p].w_of_river = new_value;
                self.plot_update_plot_group_bonus(p, true);

                self.plot_update_river_crossing(p);
                self.plot_update_yield(p);

                let (x, y) = self.map.xy(p);
                for i in 0..DirectionType::NUM {
                    if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                        self.plot_update_river_crossing(adjacent);
                        self.plot_update_yield(adjacent);
                    }
                }

                if let Some(area) = self.plot_area(p) {
                    self.map.areas[area].num_river_edges += if self.map.plots[p].w_of_river { 1 } else { -1 };
                }
            }

            debug_assert!(
                matches!(river_dir, None | Some(CardinalDirectionType::CARDINALDIRECTION_NORTH) | Some(CardinalDirectionType::CARDINALDIRECTION_SOUTH)),
                "invalid parameter"
            );
            self.map.plots[p].river_ns_direction = river_dir;

            // updateRiverSymbol: graphics only.
        }
    }

    /// C++ `CvPlot::getInlandCorner` (map generation). Const in C++ but shuffles with the map
    /// RNG (`GC.getGameINLINE().getMapRand()`), hence `&mut self`.
    pub fn plot_get_inland_corner(&mut self, p: PlotIdx) -> Option<PlotIdx> {
        let mut river_plot: Option<PlotIdx> = None; // will be a plot through whose SE corner we want the river to run
        let mut shuffle = [0i32; 4];

        shuffle_array(&mut shuffle, &mut self.map_rand);

        let (x, y) = self.map.xy(p);
        for &choice in &shuffle {
            match choice {
                0 => river_plot = self.map.plot_soren(x, y),
                1 => river_plot = self.plot_direction(x, y, DirectionType::DIRECTION_NORTH),
                2 => river_plot = self.plot_direction(x, y, DirectionType::DIRECTION_NORTHWEST),
                3 => river_plot = self.plot_direction(x, y, DirectionType::DIRECTION_WEST),
                _ => {}
            }
            match river_plot {
                Some(q) if !self.plot_has_coast_at_se_corner(q) => break,
                _ => river_plot = None,
            }
        }

        river_plot
    }

    /// C++ `CvPlot::hasCoastAtSECorner`.
    pub fn plot_has_coast_at_se_corner(&self, p: PlotIdx) -> bool {
        if self.plot_is_water(p) {
            return true;
        }

        let (x, y) = self.map.xy(p);
        for dir in [DirectionType::DIRECTION_EAST, DirectionType::DIRECTION_SOUTHEAST, DirectionType::DIRECTION_SOUTH] {
            if let Some(adjacent) = self.plot_direction(x, y, dir) {
                if self.plot_is_water(adjacent) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvPlot::isPotentialIrrigation`.
    pub fn plot_is_potential_irrigation(&self, p: PlotIdx) -> bool {
        let pl = &self.map.plots[p];
        // `isCity()` with its defaults is `getPlotCity() != NULL`.
        let city_or_carrier = (pl.plot_city.is_some() && !self.plot_is_hills(p))
            || pl.improvement_type.is_some_and(|i| self.d.improvements[i].carries_irrigation);
        if city_or_carrier {
            if let Some(team) = self.plot_get_team(p) {
                if self.team_is_irrigation(team) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvPlot::canHavePotentialIrrigation`.
    pub fn plot_can_have_potential_irrigation(&self, p: PlotIdx) -> bool {
        if self.map.plots[p].plot_city.is_some() && !self.plot_is_hills(p) {
            return true;
        }

        for (i, improvement) in self.d.improvements.iter_enumerated() {
            if improvement.carries_irrigation && self.plot_can_have_improvement(p, i, None, true) {
                return true;
            }
        }

        false
    }

    /// C++ `CvPlot::isIrrigationAvailable` (C++ default `bIgnoreSelf = false`).
    pub fn plot_is_irrigation_available(&self, p: PlotIdx, ignore_self: bool) -> bool {
        if !ignore_self && self.map.plots[p].irrigated {
            return true;
        }

        if self.plot_is_fresh_water(p) {
            return true;
        }

        let (x, y) = self.map.xy(p);
        for i in 0..DirectionType::NUM {
            if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                if self.map.plots[adjacent].irrigated {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `CvPlot::setIrrigated`.
    pub fn plot_set_irrigated(&mut self, p: PlotIdx, new_value: bool) {
        if self.map.plots[p].irrigated != new_value {
            self.map.plots[p].irrigated = new_value;

            let (x, y) = self.map.xy(p);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                        self.plot_update_yield(loop_plot);
                        // setLayoutDirty(true): graphics only.
                    }
                }
            }
        }
    }

    /// C++ `CvPlot::updateIrrigated`: spreads/removes irrigation (C++ uses a private FAStar
    /// flood fill with `potentialIrrigation` / `checkFreshWater` / `changeIrrigated`).
    pub fn plot_update_irrigated(&mut self, p: PlotIdx) {
        if self.plot_area(p).is_none() {
            return;
        }

        if !self.final_initialized {
            return;
        }

        // pIrrigatedFinder = gDLL->getFAStarIFace()->create(): a local finder (8 directions).
        if self.map.plots[p].irrigated {
            if !self.plot_is_potential_irrigation(p) {
                self.plot_set_irrigated(p, false);

                let (x, y) = self.map.xy(p);
                for i in 0..DirectionType::NUM {
                    if let Some(loop_plot) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                        let mut found_fresh_water = false;
                        for q in self.plot_irrigation_flood_fill(loop_plot) {
                            let node = self.finder_node(q);
                            self.check_fresh_water(&node, ASNL_ADDCLOSED, &mut found_fresh_water);
                        }

                        if !found_fresh_water {
                            let irrigated = false;
                            for q in self.plot_irrigation_flood_fill(loop_plot) {
                                let node = self.finder_node(q);
                                self.change_irrigated(&node, ASNL_ADDCLOSED, irrigated);
                            }
                        }
                    }
                }
            }
        } else if self.plot_is_potential_irrigation(p) && self.plot_is_irrigation_available(p, true) {
            let irrigated = true;
            for q in self.plot_irrigation_flood_fill(p) {
                let node = self.finder_node(q);
                self.change_irrigated(&node, ASNL_ADDCLOSED, irrigated);
            }
        }

        // gDLL->getFAStarIFace()->destroy(pIrrigatedFinder)
    }

    /// `GeneratePath(pIrrigatedFinder, iX, iY, -1, -1)` of C++ `CvPlot::updateIrrigated`: the
    /// plots reachable from `start` through potential irrigation plots (`potentialIrrigation`),
    /// in closed-list order.
    fn plot_irrigation_flood_fill(&self, start: PlotIdx) -> Vec<PlotIdx> {
        let (x, y) = self.map.xy(start);
        let ctx = FinderCtx { start_x: x, start_y: y, dest_x: -1, dest_y: -1, info: 0 };
        self.map_flood_fill(start, false, |g, parent, node| {
            let parent = parent.map(|q| g.finder_node(q));
            g.potential_irrigation(parent.as_ref(), &g.finder_node(node), 0, &ctx)
        })
    }

    // ------------------------------------------------------------------
    // Potential city work
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isPotentialCityWorkForArea`.
    pub fn plot_is_potential_city_work_for_area(&self, p: PlotIdx, area: AreaId) -> bool {
        let (x, y) = self.map.xy(p);
        for i in 0..NUM_CITY_PLOTS {
            if let Some(loop_plot) = self.plot_city(x, y, i) {
                if !self.plot_is_water(loop_plot) || self.d.defines.water_potential_city_work_for_area != 0 {
                    if self.plot_area(loop_plot) == Some(area) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// C++ `CvPlot::updatePotentialCityWork` (FFH: only the 21 inner city plots count).
    pub fn plot_update_potential_city_work(&mut self, p: PlotIdx) {
        let mut valid = false;

        // FfH: Modified by Kael 07/12/208 (`NUM_CITY_PLOTS` -> 21)
        let (x, y) = self.map.xy(p);
        for i in 0..NUM_CITY_PLOTS_RADIUS_2 as usize {
            if let Some(loop_plot) = self.plot_city(x, y, i) {
                if !self.plot_is_water(loop_plot) {
                    valid = true;
                    break;
                }
            }
        }

        if self.map.plots[p].potential_city_work != valid {
            self.map.plots[p].potential_city_work = valid;

            self.plot_update_yield(p);
        }
    }

    // ------------------------------------------------------------------
    // Yields (`getYield(e)` is the `yields` field)
    // ------------------------------------------------------------------

    /// C++ `CvPlot::calculateNatureYield` (C++ default `bIgnoreFeature = false`).
    pub fn plot_calculate_nature_yield(&self, p: PlotIdx, yield_type: YieldType, team: Option<TeamId>, ignore_feature: bool) -> i32 {
        if self.plot_is_impassable(p) {
            return 0;
        }

        // C++: FAssertMsg(getTerrainType() != NO_TERRAIN), then indexes the terrain infos with it
        // (undefined for NO_TERRAIN); no terrain yields nothing here.
        let pl = &self.map.plots[p];
        let Some(terrain_type) = pl.terrain_type else {
            return 0;
        };
        let terrain = &self.d.terrains[terrain_type];
        let yield_info = &self.d.yields[yield_type];
        let y = yield_type.idx();

        let mut yield_ = terrain.yield_[y];

        if self.plot_is_hills(p) {
            yield_ += yield_info.hills_change;
        }

        if self.plot_is_peak(p) {
            yield_ += yield_info.peak_change;
        }

        if self.plot_is_lake(p) {
            yield_ += yield_info.lake_change;
        }

        if let Some(team) = team {
            if let Some(bonus) = self.plot_get_bonus_type(p, Some(team)) {
                yield_ += self.d.bonuses[bonus].yield_change[y];
            }
        }

        let feature = if ignore_feature { None } else { pl.feature_type.map(|f| &self.d.features[f]) };

        if self.plot_is_river(p) {
            yield_ += match feature {
                None => terrain.river_yield_change[y],
                Some(feature) => feature.river_yield_change[y],
            };
        }

        if self.plot_is_hills(p) {
            yield_ += match feature {
                None => terrain.hills_yield_change[y],
                Some(feature) => feature.hills_yield_change[y],
            };
        }

        if let Some(feature) = feature {
            yield_ += feature.yield_change[y];
        }

        // FfH: Added by Kael 09/25/2008
        if let Some(owner) = pl.owner {
            if self.player_get_civilization_type(owner) == terrain.civilization_yield_type {
                yield_ += terrain.civilization_yield_change[y];
            }
        }
        // FfH: End Add

        std::cmp::max(0, yield_)
    }

    /// C++ `CvPlot::calculateBestNatureYield`.
    pub fn plot_calculate_best_nature_yield(&self, p: PlotIdx, yield_type: YieldType, team: Option<TeamId>) -> i32 {
        std::cmp::max(
            self.plot_calculate_nature_yield(p, yield_type, team, false),
            self.plot_calculate_nature_yield(p, yield_type, team, true),
        )
    }

    /// C++ `CvPlot::calculateTotalBestNatureYield`.
    pub fn plot_calculate_total_best_nature_yield(&self, p: PlotIdx, team: Option<TeamId>) -> i32 {
        self.plot_calculate_best_nature_yield(p, YieldType::YIELD_FOOD, team)
            + self.plot_calculate_best_nature_yield(p, YieldType::YIELD_PRODUCTION, team)
            + self.plot_calculate_best_nature_yield(p, YieldType::YIELD_COMMERCE, team)
    }

    /// C++ `CvPlot::calculateImprovementYieldChange` (BETTER_BTS_AI version with `bBestRoute`;
    /// C++ defaults `bOptimal = false`, `bBestRoute = false`).
    pub fn plot_calculate_improvement_yield_change(
        &self,
        p: PlotIdx,
        improvement: ImprovementType,
        yield_type: YieldType,
        player: Option<PlayerId>,
        optimal: bool,
        best_route: bool,
    ) -> i32 {
        let info = &self.d.improvements[improvement];
        let y = yield_type.idx();

        let mut yield_ = info.yield_change[y];

        if self.plot_is_river_side(p) {
            yield_ += info.river_side_yield_change[y];
        }

        if self.plot_is_hills(p) {
            yield_ += info.hills_yield_change[y];
        }

        if optimal || self.plot_is_irrigation_available(p, false) {
            yield_ += info.irrigated_yield_change[y];
        }

        if optimal {
            let mut best_yield = 0;

            for route_yield in &info.route_yield_changes {
                best_yield = std::cmp::max(best_yield, route_yield[y]);
            }

            yield_ += best_yield;
        } else {
            let mut route = self.map.plots[p].route_type;

            if best_route {
                if let Some(player) = player {
                    route = self.player_get_best_route(player, Some(p));
                }
            }

            if let Some(route) = route {
                yield_ += info.route_yield_changes[route][y];
            }
        }

        match player {
            Some(player) if !optimal => {
                yield_ += self.players[player].improvement_yield_change[improvement][y];
                let team = self.players[player].team;
                yield_ += self.teams[team].improvement_yield_change[improvement][y];
            }
            _ => {
                for tech_yield in &info.tech_yield_changes {
                    yield_ += tech_yield[y];
                }

                for civic in &self.d.civics {
                    yield_ += civic.improvement_yield_changes[improvement][y];
                }
            }
        }

        if let Some(player) = player {
            if let Some(bonus) = self.plot_get_bonus_type(p, Some(self.players[player].team)) {
                yield_ += info.improvement_bonus[bonus].yield_change[y];
            }
        }

        yield_
    }

    /// C++ `CvPlot::calculateYield` (`bDisplay` dropped: it selects the active team's revealed
    /// state for UI display; rules always use the real state).
    pub fn plot_calculate_yield(&self, p: PlotIdx, yield_type: YieldType) -> i32 {
        let pl = &self.map.plots[p];
        let y = yield_type.idx();

        // FfH: Added by Kael 10/04/2008
        if let Some(owner) = pl.owner {
            if yield_type == YieldType::YIELD_FOOD && self.players[owner].ignore_food {
                return 0;
            }
        }
        // FfH: End Add

        // bDisplay && isDebugMode(): UI only.

        if pl.terrain_type.is_none() {
            return 0;
        }

        if !pl.potential_city_work {
            return 0;
        }

        let yield_info = &self.d.yields[yield_type];
        let mut city = false;

        let player = pl.owner;
        let improvement = pl.improvement_type;
        let route = pl.route_type;

        let mut yield_ = self.plot_calculate_nature_yield(p, yield_type, player.map(|pp| self.players[pp].team), false);

        if let Some(improvement) = improvement {
            yield_ += self.plot_calculate_improvement_yield_change(p, improvement, yield_type, player, false, false);
        }

        if let Some(route) = route {
            yield_ += self.d.routes[route].yield_change[y];
        }

        if let Some(player) = player {
            if let Some(plot_city) = pl.plot_city {
                yield_ += yield_info.city_change;
                if yield_info.population_change_divisor != 0 {
                    yield_ += (self.cities[plot_city].population + yield_info.population_change_offset) / yield_info.population_change_divisor;
                }
                city = true;
            }

            if self.plot_is_water(p) && !self.plot_is_impassable(p) {
                yield_ += self.players[player].sea_plot_yield[y];

                if let Some(working_city) = pl.working_city {
                    yield_ += self.cities[working_city].sea_plot_yield[y];
                }
            }

            if self.plot_is_river(p) && !self.plot_is_impassable(p) {
                if let Some(working_city) = pl.working_city {
                    yield_ += self.cities[working_city].river_plot_yield[y];
                }
            }
        }

        if city {
            yield_ = std::cmp::max(yield_, yield_info.min_city);
        }

        yield_ += self.plot_extra_yield(pl.x, pl.y, yield_type);

        if let Some(player) = player {
            let threshold = self.players[player].extra_yield_threshold[y];
            if threshold > 0 && yield_ >= threshold {
                yield_ += self.d.defines.extra_yield;
            }

            if self.player_is_golden_age(player) && yield_ >= yield_info.golden_age_yield_threshold {
                yield_ += yield_info.golden_age_yield;
            }
        }

        std::cmp::max(0, yield_)
    }

    /// C++ `CvPlot::hasYield`.
    pub fn plot_has_yield(&self, p: PlotIdx) -> bool {
        self.map.plots[p].yields.iter().any(|&v| v > 0)
    }

    /// C++ `CvPlot::updateYield`: recomputes `yields` and updates the working city.
    pub fn plot_update_yield(&mut self, p: PlotIdx) {
        if self.plot_area(p).is_none() {
            return;
        }

        for i in 0..YieldType::NUM {
            let yield_type = YieldType::from_usize(i);
            let new_yield = self.plot_calculate_yield(p, yield_type);

            let old_yield = self.map.plots[p].yields[i];
            if old_yield != new_yield {
                self.map.plots[p].yields[i] = new_yield;
                debug_assert!(new_yield >= 0);

                if let Some(working_city) = self.map.plots[p].working_city {
                    if self.plot_is_being_worked(p) {
                        self.city_change_base_yield_rate(working_city, yield_type, new_yield - old_yield);
                    }

                    // AI_setAssignWorkDirty(true)
                    self.cities[working_city].ai_assign_work_dirty = true;
                }
            }
        }

        // if (bChange) updateSymbols(): graphics only.
    }

    /// C++ `CvPlot::calculateMaxYield` (no C++ callers; exposed to Python).
    pub fn plot_calculate_max_yield(&self, p: PlotIdx, yield_type: YieldType) -> i32 {
        if self.map.plots[p].terrain_type.is_none() {
            return 0;
        }
        let y = yield_type.idx();

        let mut max_yield = self.plot_calculate_nature_yield(p, yield_type, None, false);

        let mut improvement_yield = 0;
        for improvement in self.d.improvements.keys() {
            improvement_yield = std::cmp::max(self.plot_calculate_improvement_yield_change(p, improvement, yield_type, None, true, false), improvement_yield);
        }
        max_yield += improvement_yield;

        let mut route_yield = 0;
        for route in &self.d.routes {
            route_yield = std::cmp::max(route.yield_change[y], route_yield);
        }
        max_yield += route_yield;

        if self.plot_is_water(p) && !self.plot_is_impassable(p) {
            let mut building_yield = 0;
            for building in &self.d.buildings {
                building_yield = std::cmp::max(building.sea_plot_yield_change[y] + building.global_sea_plot_yield_change[y], building_yield);
            }
            max_yield += building_yield;
        }

        if self.plot_is_river(p) {
            let mut building_yield = 0;
            for building in &self.d.buildings {
                building_yield = std::cmp::max(building.river_plot_yield_change[y], building_yield);
            }
            max_yield += building_yield;
        }

        let mut extra_yield_threshold = 0;
        for trait_info in &self.d.traits {
            extra_yield_threshold = std::cmp::max(trait_info.extra_yield_threshold[y], extra_yield_threshold);
        }
        if extra_yield_threshold > 0 && max_yield > extra_yield_threshold {
            max_yield += self.d.defines.extra_yield;
        }

        max_yield
    }

    /// C++ `CvPlot::getYieldWithBuild` (AI-only callers).
    pub fn plot_get_yield_with_build(&self, p: PlotIdx, build: BuildType, yield_type: YieldType, with_upgrade: bool) -> i32 {
        let pl = &self.map.plots[p];
        let build_info = &self.d.builds[build];
        let mut yield_ = 0;

        let mut ignore_feature = false;
        if let Some(feature) = pl.feature_type {
            if build_info.feature_remove[feature] {
                ignore_feature = true;
            }

            // FfH: Added by Kael 04/24/2008
            if let Some(owner) = pl.owner {
                // C++ indexes the civilization infos with the owner's civilization (never
                // NO_CIVILIZATION for a plot owner).
                if let Some(civ) = self.player_get_civilization_type(owner) {
                    if self.d.civilizations[civ].maintain_features[feature] {
                        ignore_feature = false;
                    }
                }
            }
            // FfH: End Add
        }

        yield_ += self.plot_calculate_nature_yield(p, yield_type, self.plot_get_team(p), ignore_feature);

        if let Some(mut improvement) = build_info.improvement {
            if with_upgrade {
                // in the case that improvements upgrade, use 2 upgrade levels higher for the
                // yield calculations.
                let mut upgrade_improvement = self.d.improvements[improvement].improvement_upgrade;
                if let Some(upgrade) = upgrade_improvement {
                    // unless it's commerce on a low food tile, in which case only use 1 level higher
                    if yield_type != YieldType::YIELD_COMMERCE
                        || pl.yields[YieldType::YIELD_FOOD.idx()] >= self.d.defines.food_consumption_per_population
                    {
                        if let Some(upgrade2) = self.d.improvements[upgrade].improvement_upgrade {
                            upgrade_improvement = Some(upgrade2);
                        }
                    }
                }

                if let Some(upgrade) = upgrade_improvement {
                    if upgrade != improvement {
                        improvement = upgrade;
                    }
                }
            }

            yield_ += self.plot_calculate_improvement_yield_change(p, improvement, yield_type, pl.owner, false, false);
        }

        if let Some(route) = build_info.route {
            if let Some(improvement) = pl.improvement_type {
                let route_yield_changes = &self.d.improvements[improvement].route_yield_changes;
                // C++ quirk: sums the route yield changes of every yield type, not just `eYield`.
                for i in 0..YieldType::NUM {
                    yield_ += route_yield_changes[route][i];
                    if let Some(current_route) = pl.route_type {
                        yield_ -= route_yield_changes[current_route][i];
                    }
                }
            }
        }

        yield_
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tests::{assign_test_areas, at, game_from_ascii};
    use crate::rng::Rng;

    #[test]
    fn adjacency_and_water_areas() {
        // y=3  # # . . #
        // y=2  # # # . .
        // y=1  . # # # #
        // y=0  . . # # .
        let mut g = game_from_ascii(&["##..#", "###..", ".####", "..##."], false, false);
        assign_test_areas(&mut g);
        let area = |g: &Game, x, y| g.map.plots[at(g, x, y)].area.unwrap();
        // Water: the NE lake (4 tiles, cardinally connected), the SW sea (3) and the SE pond (1).
        let lake = area(&g, 3, 3);
        let sea = area(&g, 0, 0);
        let pond = area(&g, 4, 0);
        assert_eq!(g.map.areas[lake].num_tiles, 4);
        assert_eq!(g.map.areas[sea].num_tiles, 3);
        assert_eq!(g.map.areas[pond].num_tiles, 1);
        // Land: the main body (11 tiles) and the NE island (4, 3), cut off by the lake.
        let land = area(&g, 1, 1);
        assert_eq!(g.map.areas[land].num_tiles, 11);
        assert_ne!(area(&g, 4, 3), land);

        let p = at(&g, 3, 1);
        assert!(g.plot_is_adjacent_to_land(p));
        assert!(g.plot_is_adjacent_to_water(p));
        assert!(!g.plot_is_adjacent_to_water(at(&g, 0, 3)));
        assert!(g.plot_is_adjacent_to_water(at(&g, 1, 3)));
        assert!(g.plot_is_adjacent_to_land(at(&g, 3, 3)));
        assert!(g.plot_is_adjacent_to_land(at(&g, 0, 0)));
        assert!(g.plot_is_coastal_land(p, -1));
        assert!(g.plot_is_coastal_land(p, 4));
        assert!(!g.plot_is_coastal_land(p, 5));
        assert!(!g.plot_is_coastal_land(at(&g, 3, 3), -1), "water is never coastal land");
        assert!(!g.plot_is_coastal_land(at(&g, 0, 3), -1));
        assert!(g.plot_is_coastal_land(at(&g, 1, 2), -1));

        // waterArea: the biggest adjacent water area; secondWaterArea: the biggest other one.
        assert_eq!(g.plot_water_area(p), Some(lake));
        assert_eq!(g.plot_second_water_area(p), Some(pond));
        assert_eq!(g.plot_water_area(at(&g, 0, 0)), Some(sea));
        assert_eq!(g.plot_water_area(at(&g, 0, 3)), None);
        assert_eq!(g.plot_water_area(at(&g, 1, 1)), Some(sea));
        assert_eq!(g.plot_second_water_area(at(&g, 1, 1)), None);

        assert!(g.plot_is_adjacent_to_area(p, pond));
        assert!(!g.plot_is_adjacent_to_area(at(&g, 0, 3), lake));
        assert!(g.plot_share_adjacent_area(at(&g, 1, 1), at(&g, 4, 1)));
        assert!(!g.plot_share_adjacent_area(at(&g, 0, 0), at(&g, 4, 3)));

        // nearest land of (0, 0): (0, 2) at ring distance 2
        assert_eq!(g.plot_get_nearest_land_plot(at(&g, 0, 0)), Some(at(&g, 0, 2)));
        assert_eq!(g.plot_get_nearest_land_area(at(&g, 0, 0)), Some(land));
    }

    #[test]
    fn nearest_land_ring_order() {
        let mut rows = vec![".......".to_string(); 7];
        // land at (5, 3) and (3, 5), both at distance 2 from (3, 3)
        rows[7 - 1 - 3].replace_range(5..6, "#");
        rows[7 - 1 - 5].replace_range(3..4, "#");
        let rows: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
        let g = game_from_ascii(&rows, false, false);
        let center = at(&g, 3, 3);
        // the ring is scanned dx-major from -d, so (0, +2) comes before (+2, 0)
        assert_eq!(g.plot_get_nearest_land_plot(center), Some(at(&g, 3, 5)));
        assert_eq!(g.plot_get_nearest_land_plot(at(&g, 5, 3)), Some(at(&g, 5, 3)));
        // starting beyond the land ring: nothing until the distance exceeds both map sizes
        assert_eq!(g.plot_get_nearest_land_plot_internal(center, 3), None);

        let g = game_from_ascii(&["...", "..."], false, false);
        assert_eq!(g.plot_get_nearest_land_plot(at(&g, 0, 0)), None);
        assert_eq!(g.plot_get_nearest_land_area(at(&g, 0, 0)), None);
    }

    #[test]
    fn river_crossings() {
        let mut g = game_from_ascii(&["#####", "#####", "#####", "#####", "#####"], false, false);
        let p = at(&g, 2, 2);
        let crossing = |g: &Game, d: DirectionType| g.plot_is_river_crossing(p, Some(d));
        // West of river on p: crossing to the east; north of river on the plot to the north:
        // crossing to the north; both meet at p's NE corner.
        g.map.plots[p].w_of_river = true;
        let q = at(&g, 2, 3);
        g.map.plots[q].n_of_river = true;
        g.plot_update_river_crossing(p);
        assert!(crossing(&g, DirectionType::DIRECTION_NORTH));
        assert!(crossing(&g, DirectionType::DIRECTION_NORTHEAST));
        assert!(crossing(&g, DirectionType::DIRECTION_EAST));
        for d in [
            DirectionType::DIRECTION_SOUTHEAST,
            DirectionType::DIRECTION_SOUTH,
            DirectionType::DIRECTION_SOUTHWEST,
            DirectionType::DIRECTION_WEST,
            DirectionType::DIRECTION_NORTHWEST,
        ] {
            assert!(!crossing(&g, d), "{d:?}");
        }
        assert_eq!(g.map.plots[p].river_crossing_count, 3);
        assert!(g.plot_is_river(p));
        assert!(g.plot_is_river_side(p));
        assert!(g.plot_is_river_connection(p, Some(DirectionType::DIRECTION_SOUTH)));
        assert!(!g.plot_is_river_connection(p, None));
        assert!(!g.plot_is_river_crossing(p, None));

        // SE corner: the river runs along the east edges of p and of the plot south of it.
        let q = at(&g, 2, 1);
        g.map.plots[q].w_of_river = true;
        g.plot_update_river_crossing_dir(p, DirectionType::DIRECTION_SOUTHEAST);
        assert!(crossing(&g, DirectionType::DIRECTION_SOUTHEAST));
        assert_eq!(g.map.plots[p].river_crossing_count, 4);

        // Removing the rivers clears the crossings and the count.
        g.map.plots[p].w_of_river = false;
        let q = at(&g, 2, 1);
        g.map.plots[q].w_of_river = false;
        let q = at(&g, 2, 3);
        g.map.plots[q].n_of_river = false;
        g.plot_update_river_crossing(p);
        assert_eq!(g.map.plots[p].river_crossing_count, 0);
        assert!(!g.plot_is_river(p));
        assert!(!g.plot_is_river_side(p));

        // No crossing towards water, and none at all on water.
        g.map.plots[p].w_of_river = true;
        let q = at(&g, 3, 2);
        g.map.plots[q].plot_type = PlotType::PLOT_OCEAN;
        g.plot_update_river_crossing(p);
        assert!(!crossing(&g, DirectionType::DIRECTION_EAST));
        let q = at(&g, 3, 2);
        g.map.plots[q].plot_type = PlotType::PLOT_LAND;
        g.plot_update_river_crossing(p);
        assert!(crossing(&g, DirectionType::DIRECTION_EAST));
        g.map.plots[p].plot_type = PlotType::PLOT_OCEAN;
        g.plot_update_river_crossing(p);
        assert_eq!(g.map.plots[p].river_crossing_count, 0);
    }

    #[test]
    fn inland_corner() {
        // Only the NW candidate (1, 3) has no water at its SE corner.
        let mut g = game_from_ascii(&["#####", "###.#", "#####", "#.#.#", "#####"], false, false);
        let p = at(&g, 2, 2);
        assert!(g.plot_has_coast_at_se_corner(p));
        assert!(g.plot_has_coast_at_se_corner(at(&g, 2, 3)));
        assert!(g.plot_has_coast_at_se_corner(at(&g, 1, 2)));
        assert!(!g.plot_has_coast_at_se_corner(at(&g, 1, 3)));
        assert!(g.plot_has_coast_at_se_corner(at(&g, 3, 3)), "water plots have coast");
        let mut r: Rng = g.map_rand.clone();
        assert_eq!(g.plot_get_inland_corner(p), Some(at(&g, 1, 3)));
        // one shuffle of four entries: four map rand draws
        for i in 0..4 {
            r.get_i(4 - i, "");
        }
        assert_eq!(g.map_rand, r);

        let mut g = game_from_ascii(&["...", "...", "..."], false, false);
        assert_eq!(g.plot_get_inland_corner(at(&g, 1, 1)), None);
    }

    #[test]
    fn nature_improvement_and_plot_yields() {
        let d = ffh_data::global();
        // y=2  # # #
        // y=1  # h #
        // y=0  . . .   (a three-tile lake)
        let mut g = game_from_ascii(&["###", "#h#", "..."], false, false);
        assign_test_areas(&mut g);
        let grass = d.t::<TerrainType>("TERRAIN_GRASS");
        let coast = d.t::<TerrainType>("TERRAIN_COAST");
        for q in g.map.plot_ids() {
            g.map.plots[q].terrain_type = Some(if g.plot_is_water(q) { coast } else { grass });
            g.map.plots[q].potential_city_work = true;
        }
        let (food, prod) = (YieldType::YIELD_FOOD, YieldType::YIELD_PRODUCTION);
        let (f, pr) = (food.idx(), prod.idx());
        let (gi, ci) = (&d.terrains[grass], &d.terrains[coast]);
        let (hills, flat, lake) = (at(&g, 1, 1), at(&g, 1, 2), at(&g, 1, 0));
        assert!(g.plot_is_lake(lake));

        // flat grass: the terrain yield; hills: + the yield's hills change + the terrain's
        // hills yield change; lake: + the yield's lake change
        assert_eq!(g.plot_calculate_nature_yield(flat, food, None, false), gi.yield_[f].max(0));
        let hills_food = (gi.yield_[f] + d.yields[food].hills_change + gi.hills_yield_change[f]).max(0);
        assert_eq!(g.plot_calculate_nature_yield(hills, food, None, false), hills_food);
        let lake_food = (ci.yield_[f] + d.yields[food].lake_change).max(0);
        assert_eq!(g.plot_calculate_nature_yield(lake, food, None, false), lake_food);

        // A forest replaces the terrain's hills yield change by its own and adds its yield.
        let forest = d.t::<FeatureType>("FEATURE_FOREST");
        let fi = &d.features[forest];
        g.map.plots[hills].feature_type = Some(forest);
        let with_forest = (gi.yield_[pr] + d.yields[prod].hills_change + fi.hills_yield_change[pr] + fi.yield_change[pr]).max(0);
        let ignoring_forest = (gi.yield_[pr] + d.yields[prod].hills_change + gi.hills_yield_change[pr]).max(0);
        assert_eq!(g.plot_calculate_nature_yield(hills, prod, None, false), with_forest);
        assert_eq!(g.plot_calculate_nature_yield(hills, prod, None, true), ignoring_forest);
        assert_eq!(g.plot_calculate_best_nature_yield(hills, prod, None), with_forest.max(ignoring_forest));

        // Unowned, no improvement/route/city: the plot yield is the nature yield; updateYield
        // stores it.
        let nature: Vec<i32> = (0..YieldType::NUM).map(|i| g.plot_calculate_nature_yield(flat, YieldType::from_usize(i), None, false)).collect();
        for (i, &n) in nature.iter().enumerate() {
            assert_eq!(g.plot_calculate_yield(flat, YieldType::from_usize(i)), n);
        }
        g.plot_update_yield(flat);
        assert_eq!(g.map.plots[flat].yields.to_vec(), nature);
        assert!(g.plot_calculate_max_yield(flat, food) >= nature[f]);

        // A farm on the hills: the adjacent lake gives fresh water, hence irrigation; without a
        // player every tech and civic change counts.
        let farm = d.t::<ImprovementType>("IMPROVEMENT_FARM");
        let fa = &d.improvements[farm];
        assert!(g.plot_is_fresh_water(hills));
        assert!(!g.plot_is_fresh_water(flat));
        assert!(g.plot_is_irrigation_available(hills, false));
        let mut expect = fa.yield_change[f] + fa.hills_yield_change[f] + fa.irrigated_yield_change[f];
        expect += fa.tech_yield_changes.iter().map(|y| y[f]).sum::<i32>();
        expect += d.civics.iter().map(|c| c.improvement_yield_changes[farm][f]).sum::<i32>();
        assert_eq!(g.plot_calculate_improvement_yield_change(hills, farm, food, None, false, false), expect);
        let best_route = fa.route_yield_changes.iter().map(|y| y[f]).max().unwrap_or(0).max(0);
        assert_eq!(g.plot_calculate_improvement_yield_change(hills, farm, food, None, true, false), expect + best_route);

        // getYieldWithBuild: nature yield (forest ignored if the farm build removes it) plus the
        // farm's change.
        let build_farm = d.t::<BuildType>("BUILD_FARM");
        let ignore = d.builds[build_farm].feature_remove[forest];
        let with_build = g.plot_calculate_nature_yield(hills, food, None, ignore) + expect;
        assert_eq!(g.plot_get_yield_with_build(hills, build_farm, food, false), with_build);
    }

    /// Score v2 reads `total_land`: every change of it marks the score dirty (C++ only marked it
    /// through the land score, which counts plots owned long enough), and so does a vassal change.
    #[test]
    fn land_and_vassal_changes_mark_the_score_dirty() {
        let d = ffh_data::global();
        let mut g = game_from_ascii(&["....", ".##.", ".#..", "...."], false, false);
        assign_test_areas(&mut g);
        let grass = d.t::<TerrainType>("TERRAIN_GRASS");
        let coast = d.t::<TerrainType>("TERRAIN_COAST");
        for q in g.map.plot_ids() {
            g.map.plots[q].terrain_type = Some(if g.plot_is_water(q) { coast } else { grass });
        }
        for p in g.player_ids() {
            g.players[p].id = p;
            g.players[p].team = TeamId(p.0);
        }
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        for p in [p0, p1, g.barbarian_player()] {
            g.players[p].alive = true;
            let t = g.players[p].team;
            g.teams[t].alive_count = 1;
        }

        let land = at(&g, 1, 2);
        g.score_dirty = false;
        g.plot_set_owner(land, Some(p0), false, false);
        assert_eq!(g.players[p0].total_land, 1);
        assert!(g.score_dirty, "a land plot gained");

        g.score_dirty = false;
        g.plot_set_owner(land, Some(p1), false, false);
        assert_eq!((g.players[p0].total_land, g.players[p1].total_land), (0, 1));
        assert!(g.score_dirty, "a land plot changed hands");

        g.score_dirty = false;
        g.plot_set_owner(land, None, false, false);
        assert_eq!(g.players[p1].total_land, 0);
        assert!(g.score_dirty, "a land plot lost");

        // an owned water plot turning into land changes the owner's land
        let water = at(&g, 2, 1);
        g.plot_set_owner(water, Some(p0), false, false);
        assert_eq!(g.players[p0].total_land, 0, "water is not land");
        g.score_dirty = false;
        g.plot_set_plot_type(water, PlotType::PLOT_LAND, false);
        assert_eq!(g.players[p0].total_land, 1);
        assert!(g.score_dirty, "an owned plot became land");

        g.score_dirty = false;
        g.team_set_vassal(TeamId(1), TeamId(0), true, false);
        assert!(g.score_dirty, "a new vassal shares its people and land");
    }

    #[test]
    fn river_and_irrigation_setters_update_yields() {
        let d = ffh_data::global();
        let mut g = game_from_ascii(&["####", "####", "####"], false, false);
        assign_test_areas(&mut g);
        let grass = d.t::<TerrainType>("TERRAIN_GRASS");
        for q in g.map.plot_ids() {
            g.map.plots[q].terrain_type = Some(grass);
            g.map.plots[q].potential_city_work = true;
        }
        let p = at(&g, 1, 1);
        let area = g.map.plots[p].area.unwrap();
        let commerce = YieldType::YIELD_COMMERCE;

        // A river along p's east edge: crossings on p and its eastern neighbour, river yields,
        // one more river edge in the area.
        g.plot_set_w_of_river(p, true, Some(CardinalDirectionType::CARDINALDIRECTION_SOUTH));
        assert!(g.map.plots[p].w_of_river);
        assert_eq!(g.map.plots[p].river_ns_direction, Some(CardinalDirectionType::CARDINALDIRECTION_SOUTH));
        assert!(g.plot_is_river(p) && g.plot_is_river(at(&g, 2, 1)));
        assert!(!g.plot_is_river(at(&g, 0, 1)));
        assert_eq!(g.map.areas[area].num_river_edges, 1);
        let river_commerce = g.plot_calculate_nature_yield(p, commerce, None, false);
        assert_eq!(river_commerce, (d.terrains[grass].yield_[commerce.idx()] + d.terrains[grass].river_yield_change[commerce.idx()]).max(0));
        assert_eq!(g.map.plots[p].yields[commerce.idx()], river_commerce);
        assert_eq!(g.map.plots[at(&g, 2, 1)].yields[commerce.idx()], river_commerce);

        // Only the direction changes: no yield/edge update, just the stored direction.
        g.plot_set_w_of_river(p, true, Some(CardinalDirectionType::CARDINALDIRECTION_NORTH));
        assert_eq!(g.map.plots[p].river_ns_direction, Some(CardinalDirectionType::CARDINALDIRECTION_NORTH));
        assert_eq!(g.map.areas[area].num_river_edges, 1);

        g.plot_set_n_of_river(p, true, Some(CardinalDirectionType::CARDINALDIRECTION_WEST));
        assert_eq!(g.map.areas[area].num_river_edges, 2);
        assert!(g.plot_is_river_crossing(p, Some(DirectionType::DIRECTION_SOUTH)));
        g.plot_set_n_of_river(p, false, None);
        g.plot_set_w_of_river(p, false, None);
        assert_eq!(g.map.areas[area].num_river_edges, 0);
        assert!(!g.plot_is_river(p) && !g.plot_is_river(at(&g, 2, 1)));
        assert_eq!(g.map.plots[p].yields[commerce.idx()], d.terrains[grass].yield_[commerce.idx()].max(0));

        // setIrrigated toggles the flag (yield updates around it run without effect here).
        g.plot_set_irrigated(p, true);
        assert!(g.map.plots[p].irrigated);
        assert!(g.plot_is_irrigation_available(at(&g, 0, 0), true));
        g.plot_set_irrigated(p, false);
        assert!(!g.map.plots[p].irrigated);

        // A plot type change that keeps land land: new yields, same area, no area recount.
        let food = YieldType::YIELD_FOOD;
        g.plot_set_plot_type(p, PlotType::PLOT_HILLS, true);
        assert!(g.plot_is_hills(p));
        assert_eq!(g.map.plots[p].area, Some(area));
        assert_eq!(g.map.plots[p].yields[food.idx()], g.plot_calculate_nature_yield(p, food, None, false));
        assert_eq!(g.map.areas.len(), 1);
    }

    #[test]
    fn irrigation_and_yield_shortcuts() {
        let mut g = game_from_ascii(&["...", "...", "..."], false, false);
        let p = at(&g, 1, 1);
        // water is never fresh water, so only (adjacent) irrigation counts
        assert!(!g.plot_is_irrigation_available(p, false));
        g.map.plots[p].irrigated = true;
        assert!(g.plot_is_irrigation_available(p, false));
        assert!(!g.plot_is_irrigation_available(p, true));
        let q = at(&g, 0, 0);
        g.map.plots[q].irrigated = true;
        assert!(g.plot_is_irrigation_available(p, true));
        assert!(!g.plot_is_fresh_water(p));

        // calculateYield early outs: no terrain, not potential city work, FfH ignore-food owner.
        assert_eq!(g.plot_calculate_yield(p, YieldType::YIELD_FOOD), 0);
        assert_eq!(g.plot_calculate_max_yield(p, YieldType::YIELD_FOOD), 0);
        g.map.plots[p].terrain_type = Some(TerrainType(0));
        assert_eq!(g.plot_calculate_yield(p, YieldType::YIELD_FOOD), 0);
        g.map.plots[p].potential_city_work = true;
        g.map.plots[p].owner = Some(PlayerId(0));
        g.players[PlayerId(0)].ignore_food = true;
        assert_eq!(g.plot_calculate_yield(p, YieldType::YIELD_FOOD), 0);

        // updateYield and updateIrrigated do nothing without an area.
        g.plot_update_yield(p);
        assert_eq!(g.map.plots[p].yields, [0; YieldType::NUM]);
        g.plot_update_irrigated(p);
        assert!(g.map.plots[p].irrigated);
    }
}
