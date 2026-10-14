//! Port of `CvMap` (CvMap.h / CvMap.cpp): the plot grid and the areas.
//!
//! `Map` owns the plots (`plots[y * grid_width + x]`, indexable by `PlotIdx`) and the
//! areas (`Slab<AreaId, Area>`). Pure geometry (`plot_xy`, `plot_idx`, `xy`, `is_plot`,
//! distances, ...) is `impl Map`; every other rules method of CvMap is an `impl Game`
//! method `map_*`. Trivial getters/setters are fields. See `docs/porting/map.md`.

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::area::Area;
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::plot::Plot;
use crate::slab::Slab;
use crate::structs::XYCoords;
use ffh_data::*;

/// C++ `coordRange` (CvMap.h): wraps `coord` into `0..range` when `wrap`.
///
/// C++ quirk (kept): `range + (coord % range)` with truncating `%` maps a negative multiple
/// of `range` to `range` itself, i.e. off the map (`plot_xy` then returns None).
#[inline]
pub fn coord_range(coord: i32, range: i32, wrap: bool) -> i32 {
    if wrap && range != 0 {
        if coord < 0 {
            return range + (coord % range);
        } else if coord >= range {
            return coord % range;
        }
    }
    coord
}

/// C++ `coordDistance` (CvGameCoreUtils.h), used by the distance helpers below.
#[inline]
fn coord_distance(from: i32, to: i32, range: i32, wrap: bool) -> i32 {
    if wrap && (from - to).abs() > range / 2 {
        return range - (from - to).abs();
    }
    (from - to).abs()
}

/// C++ `CvFractal::FRAC_WRAP_X` / `FRAC_WRAP_Y` (CvFractal.h).
const FRAC_WRAP_X: i32 = 0x0001;
const FRAC_WRAP_Y: i32 = 0x0002;

/// C++ `CvMapInitData`: explicit map dimensions (world builder maps, tests, this port's
/// map scripts). Default = the C++ default constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapInitData {
    pub grid_w: i32,
    pub grid_h: i32,
    pub top_latitude: i32,
    pub bottom_latitude: i32,
    pub wrap_x: bool,
    pub wrap_y: bool,
}

impl Default for MapInitData {
    fn default() -> Self {
        MapInitData { grid_w: 0, grid_h: 0, top_latitude: 90, bottom_latitude: -90, wrap_x: false, wrap_y: false }
    }
}

/// C++ `CvMap`.
#[derive(Clone, Debug)]
pub struct Map {
    /// C++ `m_iGridWidth` (plots).
    pub grid_width: i32,
    /// C++ `m_iGridHeight` (plots).
    pub grid_height: i32,
    /// C++ `m_iLandPlots` (`changeLandPlots` is a plain `+=`).
    pub land_plots: i32,
    /// C++ `m_iOwnedPlots` (`changeOwnedPlots` is a plain `+=`).
    pub owned_plots: i32,
    /// C++ `m_iTopLatitude` (clamped to -90..=90).
    pub top_latitude: i32,
    /// C++ `m_iBottomLatitude` (clamped to -90..=90).
    pub bottom_latitude: i32,
    /// C++ `m_iNextRiverID` (`incrementNextRiverID` is a plain `+= 1`).
    pub next_river_id: i32,
    /// C++ `m_bWrapX`.
    pub wrap_x: bool,
    /// C++ `m_bWrapY`.
    pub wrap_y: bool,
    /// C++ `m_paiNumBonus` (`getNumBonuses` / `changeNumBonuses`).
    pub num_bonus: IdxVec<BonusType, i32>,
    /// C++ `m_paiNumBonusOnLand` (`getNumBonusesOnLand` / `changeNumBonusesOnLand`).
    pub num_bonus_on_land: IdxVec<BonusType, i32>,
    /// C++ `m_pMapPlots`: `plots[y * grid_width + x]`; index with a `PlotIdx` (`plots[p]`).
    pub plots: Vec<Plot>,
    /// C++ `m_areas` (`FFreeListTrashArray<CvArea>`). Ids are never reused; iteration is in
    /// id (= creation) order. C++ iterates by free-list slot and reuses the slot of a deleted
    /// area, so after `deleteArea` + `addArea` (land/water change in `setPlotType`) the C++
    /// visits the new area where the deleted one was, here it comes last.
    pub areas: Slab<AreaId, Area>,
    /// C++ static `CvPlot::c_aePirateCoveTypes` (a `std::set`, kept sorted here): the
    /// `PIRATE_COVE_IMPROVEMENT` upgrade chain, filled by `Game::plot_init_pirate_cove_types`.
    pub pirate_cove_types: Vec<ImprovementType>,
}

impl Default for Map {
    /// C++ `CvMap()`: `reset(&CvMapInitData())`, an empty 0x0 map.
    fn default() -> Self {
        let init = MapInitData::default();
        Map {
            grid_width: init.grid_w,
            grid_height: init.grid_h,
            land_plots: 0,
            owned_plots: 0,
            top_latitude: init.top_latitude,
            bottom_latitude: init.bottom_latitude,
            next_river_id: 0,
            wrap_x: init.wrap_x,
            wrap_y: init.wrap_y,
            num_bonus: IdxVec::new(),
            num_bonus_on_land: IdxVec::new(),
            plots: Vec::new(),
            areas: Slab::new(),
            pirate_cove_types: Vec::new(),
        }
    }
}

impl Map {
    /// C++ `CvMap::reset(pInitInfo)` + the plot creation of `CvMap::init` (`CvPlot::init(x, y)`
    /// for every plot). Areas are computed afterwards by `Game::map_calculate_areas`, once
    /// plot types are known. Arrays are sized from the game data and player/team counts.
    pub fn new(d: &GameData, init: &MapInitData, num_players: usize, num_teams: usize) -> Map {
        let w = init.grid_w.max(0);
        let h = init.grid_h.max(0);
        let mut plots = Vec::with_capacity((w * h) as usize);
        for i in 0..w * h {
            plots.push(Plot::new(d, i % w, i / w, num_players, num_teams));
        }
        Map {
            grid_width: init.grid_w,
            grid_height: init.grid_h,
            land_plots: 0,
            owned_plots: 0,
            // C++: std::min(.., 90) then std::max(.., -90)
            top_latitude: init.top_latitude.min(90).max(-90),
            bottom_latitude: init.bottom_latitude.min(90).max(-90),
            next_river_id: 0,
            wrap_x: init.wrap_x,
            wrap_y: init.wrap_y,
            num_bonus: IdxVec::from_elem(0, d.bonuses.len()),
            num_bonus_on_land: IdxVec::from_elem(0, d.bonuses.len()),
            plots,
            areas: Slab::new(),
            pirate_cove_types: Vec::new(),
        }
    }

    /// C++ `CvMap::numPlots`.
    #[inline]
    pub fn num_plots(&self) -> usize {
        self.plots.len()
    }

    /// All plot indices in C++ `plotByIndex` order (`for (iI = 0; iI < numPlotsINLINE(); iI++)`).
    /// The iterator does not borrow the map.
    #[inline]
    pub fn plot_ids(&self) -> impl Iterator<Item = PlotIdx> + 'static {
        (0..self.plots.len()).map(|i| PlotIdx(i as u32))
    }

    /// C++ `CvMap::isPlot`: in bounds (no wrapping).
    #[inline]
    pub fn is_plot(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.grid_width && y >= 0 && y < self.grid_height
    }

    /// C++ `CvMap::plotNum`: `y * width + x` (unchecked).
    #[inline]
    pub fn plot_num(&self, x: i32, y: i32) -> i32 {
        y * self.grid_width + x
    }

    /// C++ `CvMap::plotX`.
    #[inline]
    pub fn plot_x(&self, index: i32) -> i32 {
        index % self.grid_width
    }

    /// C++ `CvMap::plotY`.
    #[inline]
    pub fn plot_y(&self, index: i32) -> i32 {
        index / self.grid_width
    }

    /// The plot at in-bounds coordinates (C++ `isPlot(x, y) ? plotNum(x, y)`), no wrapping.
    #[inline]
    pub fn plot_idx(&self, x: i32, y: i32) -> Option<PlotIdx> {
        if self.is_plot(x, y) {
            Some(PlotIdx(self.plot_num(x, y) as u32))
        } else {
            None
        }
    }

    /// C++ `CvMap::plot` / `plotINLINE`: wraps the coordinates (`coordRange`), None if off the
    /// map or `INVALID_PLOT_COORD`. C++ `plotXY(x, y, dx, dy)` is `plot_xy(x + dx, y + dy)`.
    #[inline]
    pub fn plot_xy(&self, x: i32, y: i32) -> Option<PlotIdx> {
        if x == INVALID_PLOT_COORD || y == INVALID_PLOT_COORD {
            return None;
        }
        let map_x = coord_range(x, self.grid_width, self.wrap_x);
        let map_y = coord_range(y, self.grid_height, self.wrap_y);
        self.plot_idx(map_x, map_y)
    }

    /// C++ inline `plotSorenINLINE` (CvMap.h): no wrapping and no bounds check
    /// (debug-asserted); None only for `INVALID_PLOT_COORD`.
    #[inline]
    pub fn plot_soren(&self, x: i32, y: i32) -> Option<PlotIdx> {
        if x == INVALID_PLOT_COORD || y == INVALID_PLOT_COORD {
            return None;
        }
        debug_assert!(self.is_plot(x, y), "plot_soren({x}, {y}) off the map");
        Some(PlotIdx(self.plot_num(x, y) as u32))
    }

    /// C++ `CvMap::plotByIndex`: None if out of range.
    #[inline]
    pub fn plot_by_index(&self, index: i32) -> Option<PlotIdx> {
        if index >= 0 && (index as usize) < self.plots.len() {
            Some(PlotIdx(index as u32))
        } else {
            None
        }
    }

    /// Coordinates of a plot (C++ `getX_INLINE()`, `getY_INLINE()`; = `plotX/plotY(index)`).
    #[inline]
    pub fn xy(&self, p: PlotIdx) -> (i32, i32) {
        let i = p.0 as i32;
        (i % self.grid_width, i / self.grid_width)
    }

    /// Coordinates of a plot as C++ `XYCoords`.
    #[inline]
    pub fn xy_coords(&self, p: PlotIdx) -> XYCoords {
        let (x, y) = self.xy(p);
        XYCoords { x, y }
    }

    /// C++ `CvMap::isWrap`.
    #[inline]
    pub fn is_wrap(&self) -> bool {
        self.wrap_x || self.wrap_y
    }

    /// C++ `CvMap::maxPlotDistance`: `plotDistance` from (0, 0) to the farthest plot.
    pub fn max_plot_distance(&self) -> i32 {
        let x = if self.wrap_x { self.grid_width / 2 } else { self.grid_width - 1 };
        let y = if self.wrap_y { self.grid_height / 2 } else { self.grid_height - 1 };
        let dx = coord_distance(0, x, self.grid_width, self.wrap_x);
        let dy = coord_distance(0, y, self.grid_height, self.wrap_y);
        // C++ plotDistance: max + min / 2
        std::cmp::max(1, dx.max(dy) + dx.min(dy) / 2)
    }

    /// C++ `CvMap::maxStepDistance`: `stepDistance` from (0, 0) to the farthest plot.
    pub fn max_step_distance(&self) -> i32 {
        let x = if self.wrap_x { self.grid_width / 2 } else { self.grid_width - 1 };
        let y = if self.wrap_y { self.grid_height / 2 } else { self.grid_height - 1 };
        let dx = coord_distance(0, x, self.grid_width, self.wrap_x);
        let dy = coord_distance(0, y, self.grid_height, self.wrap_y);
        // C++ stepDistance: max
        std::cmp::max(1, dx.max(dy))
    }

    /// C++ `CvMap::getMapFractalFlags`: `CvFractal` wrap flags for this map.
    pub fn get_map_fractal_flags(&self) -> i32 {
        let wrap_x = if self.wrap_x { FRAC_WRAP_X } else { 0 };
        let wrap_y = if self.wrap_y { FRAC_WRAP_Y } else { 0 };
        wrap_x | wrap_y
    }

    /// C++ `CvMap::getNumLandAreas`.
    pub fn get_num_land_areas(&self) -> i32 {
        self.areas.iter().filter(|(_, a)| !a.water).count() as i32
    }

    /// C++ `CvMap::findBiggestArea`: the water/land area with the most tiles (first in
    /// iteration order on ties; None if no area has tiles).
    pub fn find_biggest_area(&self, water: bool) -> Option<AreaId> {
        let mut best_value = 0;
        let mut best_area = None;
        for (id, area) in self.areas.iter() {
            if area.water == water {
                let value = area.num_tiles;
                if value > best_value {
                    best_value = value;
                    best_area = Some(id);
                }
            }
        }
        best_area
    }
}

impl Game {
    // ------------------------------------------------------------------
    // Initialisation
    // ------------------------------------------------------------------

    /// C++ `CvMap::init` and the world-size branch of `CvMap::reset`: recreates the map and
    /// computes the areas. `None` = C++ `pInitInfo == NULL`: grid size from the world size
    /// (times the landscape's plots per cell), latitudes and wrapping from the map script;
    /// `Some` = the `Map::new` path.
    pub fn map_init(&mut self, init: Option<&MapInitData>) {
        todo!("CvMap::init")
    }

    /// C++ `CvMap::rebuild` (world builder / Python): sets world size, climate and sea level
    /// in the setup, then `init` with explicit dimensions. Custom map options are not ported.
    #[allow(clippy::too_many_arguments)]
    pub fn map_rebuild(
        &mut self,
        grid_w: i32,
        grid_h: i32,
        top_latitude: i32,
        bottom_latitude: i32,
        wrap_x: bool,
        wrap_y: bool,
        world_size: WorldSizeType,
        climate: ClimateType,
        sea_level: SeaLevelType,
    ) {
        todo!("CvMap::rebuild")
    }

    /// C++ `CvMap::calculateAreas` (protected): every plot without an area starts a new area
    /// and flood-fills it (C++ area finder: `areaValid` / `joinArea`).
    pub fn map_calculate_areas(&mut self) {
        todo!("CvMap::calculateAreas")
    }

    /// C++ `CvMap::recalculateAreas`.
    pub fn map_recalculate_areas(&mut self) {
        for p in self.map.plot_ids() {
            self.plot_set_area(p, None);
        }
        // m_areas.removeAll()
        for a in self.map.areas.ids() {
            self.map.areas.remove(a);
        }
        self.map_calculate_areas();
    }

    /// C++ `CvMap::addArea` + `CvArea::init(id, bWater)` (the C++ callers always `init` the new
    /// area immediately).
    pub fn map_add_area(&mut self, water: bool) -> AreaId {
        let d = self.d;
        let (num_players, num_teams) = (self.num_players(), self.num_teams());
        self.map.areas.insert_with(|id| Area::new(d, id, water, num_players, num_teams))
    }

    /// C++ `CvMap::deleteArea`.
    pub fn map_delete_area(&mut self, a: AreaId) {
        self.map.areas.remove(a);
    }

    // ------------------------------------------------------------------
    // Whole-map updates
    // ------------------------------------------------------------------

    /// C++ `CvMap::erasePlots`.
    pub fn map_erase_plots(&mut self) {
        for p in self.map.plot_ids() {
            self.plot_erase(p);
        }
    }

    /// C++ `CvMap::setRevealedPlots` (C++ default `bTerrainOnly = false`), then
    /// `CvGame::updatePlotGroups`.
    pub fn map_set_revealed_plots(&mut self, team: TeamId, new_value: bool, terrain_only: bool) {
        todo!("CvMap::setRevealedPlots")
    }

    /// C++ `CvMap::setAllPlotTypes` (graphics rebuild dropped).
    pub fn map_set_all_plot_types(&mut self, plot_type: PlotType) {
        for p in self.map.plot_ids() {
            self.plot_set_plot_type(p, plot_type, false);
        }
        self.map_recalculate_areas();
    }

    /// C++ `CvMap::doTurn`.
    pub fn map_do_turn(&mut self) {
        for p in self.map.plot_ids() {
            self.plot_do_turn(p);
        }
    }

    /// C++ `CvMap::updateSight`: every plot's sight, then `CvGame::updatePlotGroups`.
    pub fn map_update_sight(&mut self, increment: bool) {
        todo!("CvMap::updateSight")
    }

    /// C++ `CvMap::updateIrrigated`.
    pub fn map_update_irrigated(&mut self) {
        for p in self.map.plot_ids() {
            self.plot_update_irrigated(p);
        }
    }

    /// C++ `CvMap::updateWorkingCity`.
    pub fn map_update_working_city(&mut self) {
        for p in self.map.plot_ids() {
            self.plot_update_working_city(p);
        }
    }

    /// C++ `CvMap::updateMinOriginalStartDist`: step distance from the nearest starting plot
    /// in `area`.
    pub fn map_update_min_original_start_dist(&mut self, area: AreaId) {
        todo!("CvMap::updateMinOriginalStartDist")
    }

    /// C++ `CvMap::updateYield`.
    pub fn map_update_yield(&mut self) {
        for p in self.map.plot_ids() {
            self.plot_update_yield(p);
        }
    }

    /// C++ `CvMap::verifyUnitValidPlot`.
    pub fn map_verify_unit_valid_plot(&mut self) {
        for p in self.map.plot_ids() {
            self.plot_verify_unit_valid_plot(p);
        }
    }

    /// C++ `CvMap::combinePlotGroups`: moves the plots of the smaller group into the larger
    /// one; removing the last plot deletes the smaller group. (`player` is unused in C++ too.)
    pub fn map_combine_plot_groups(&mut self, player: PlayerId, group1: PlotGroupId, group2: PlotGroupId) {
        if group1 == group2 {
            return;
        }
        let (new_group, old_group) = if self.plot_groups[group1].plots.len() > self.plot_groups[group2].plots.len() {
            (group1, group2)
        } else {
            (group2, group1)
        };
        let mut node = if self.plot_groups[old_group].plots.is_empty() { None } else { Some(0) };
        while let Some(i) = node {
            let xy = self.plot_groups[old_group].plots[i];
            let p = self.map.plot_soren(xy.x, xy.y).expect("plot group plot");
            self.plot_group_add_plot(new_group, p);
            node = self.plot_group_delete_plots_node(old_group, i);
        }
    }

    // ------------------------------------------------------------------
    // Searches
    // ------------------------------------------------------------------

    /// C++ `CvMap::syncRandPlot` (C++ defaults: `iFlags = 0`, `iArea = -1`,
    /// `iMinUnitDistance = -1`, `iTimeout = 100`). `flags` are `RANDPLOT_*` (`crate::defs`).
    pub fn map_sync_rand_plot(&mut self, flags: i32, area: Option<AreaId>, min_unit_distance: i32, timeout: i32) -> Option<PlotIdx> {
        todo!("CvMap::syncRandPlot")
    }

    /// C++ `CvMap::findCity`: nearest alive city matching the filters (C++ defaults: no
    /// owner/team, `bSameArea = true`, `bCoastalOnly = false`, no war team, no direction,
    /// no skipped city).
    #[allow(clippy::too_many_arguments)]
    pub fn map_find_city(
        &self,
        x: i32,
        y: i32,
        owner: Option<PlayerId>,
        team: Option<TeamId>,
        same_area: bool,
        coastal_only: bool,
        team_at_war_with: Option<TeamId>,
        direction: Option<DirectionType>,
        skip_city: Option<CityId>,
    ) -> Option<CityId> {
        todo!("CvMap::findCity")
    }

    /// C++ `CvMap::findSelectionGroup` (C++ defaults: no owner, `bReadyToSelect = false`,
    /// `bWorkers = false`).
    pub fn map_find_selection_group(&self, x: i32, y: i32, owner: Option<PlayerId>, ready_to_select: bool, workers: bool) -> Option<GroupId> {
        todo!("CvMap::findSelectionGroup")
    }

    /// C++ `CvMap::findWater`: water (or fresh water) within `range` of `plot`.
    pub fn map_find_water(&self, plot: PlotIdx, range: i32, fresh_water: bool) -> bool {
        todo!("CvMap::findWater")
    }

    // ------------------------------------------------------------------
    // Path distance (C++ step finder)
    // ------------------------------------------------------------------

    /// C++ `CvMap::resetPathDistance`: clears the step finder cache.
    pub fn map_reset_path_distance(&mut self) {
        todo!("CvMap::resetPathDistance")
    }

    /// C++ `CvMap::calculatePathDistance`: step-finder path length, -1 if unreachable (C++
    /// also returns -1 for NULL plots; all callers pass real plots).
    pub fn map_calculate_path_distance(&mut self, source: PlotIdx, dest: PlotIdx) -> i32 {
        todo!("CvMap::calculatePathDistance")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(w: i32, h: i32, wrap_x: bool, wrap_y: bool) -> Map {
        let mut m = Map::default();
        m.grid_width = w;
        m.grid_height = h;
        m.wrap_x = wrap_x;
        m.wrap_y = wrap_y;
        m.plots = (0..w * h).map(|i| Plot::with_counts(i % w, i / w, 0, 0, 0, 0, 0)).collect();
        m
    }

    #[test]
    fn coord_range_matches_cpp() {
        assert_eq!(coord_range(-1, 10, true), 9);
        assert_eq!(coord_range(10, 10, true), 0);
        assert_eq!(coord_range(23, 10, true), 3);
        assert_eq!(coord_range(-1, 10, false), -1);
        assert_eq!(coord_range(5, 0, true), 5);
        // C++ quirk: a negative multiple of the range lands on `range` (off the map).
        assert_eq!(coord_range(-10, 10, true), 10);
    }

    #[test]
    fn plot_lookup_and_wrap() {
        let m = map(10, 6, true, false);
        assert_eq!(m.num_plots(), 60);
        assert_eq!(m.plot_idx(3, 2), Some(PlotIdx(23)));
        assert_eq!(m.plot_idx(-1, 2), None);
        assert_eq!(m.plot_xy(-1, 2), Some(PlotIdx(29)));
        assert_eq!(m.plot_xy(10, 0), Some(PlotIdx(0)));
        assert_eq!(m.plot_xy(0, -1), None);
        assert_eq!(m.plot_xy(0, 6), None);
        assert_eq!(m.plot_xy(-10, 0), None);
        assert_eq!(m.plot_xy(INVALID_PLOT_COORD, 0), None);
        assert_eq!(m.xy(PlotIdx(29)), (9, 2));
        assert_eq!((m.plot_x(29), m.plot_y(29)), (9, 2));
        assert_eq!(m.plot_by_index(60), None);
        assert_eq!(m.plot_by_index(59), Some(PlotIdx(59)));
        assert_eq!(m.plot_soren(4, 5), Some(PlotIdx(54)));
        for p in m.plot_ids() {
            let (x, y) = m.xy(p);
            assert_eq!((m.plots[p].x, m.plots[p].y), (x, y));
            assert_eq!(m.plot_xy(x, y), Some(p));
        }
    }

    #[test]
    fn max_distances() {
        // non-wrapping 10x6: plotDistance(0,0,9,5) = 9 + 5/2
        let m = map(10, 6, false, false);
        assert_eq!(m.max_plot_distance(), 11);
        assert_eq!(m.max_step_distance(), 9);
        assert_eq!(m.get_map_fractal_flags(), 0);
        // wrap X: x distance to width/2
        let m = map(10, 6, true, false);
        assert_eq!(m.max_plot_distance(), 5 + 5 / 2);
        assert_eq!(m.max_step_distance(), 5);
        assert_eq!(m.get_map_fractal_flags(), FRAC_WRAP_X);
        assert!(m.is_wrap());
    }

    #[test]
    fn default_is_cpp_constructor() {
        let m = Map::default();
        assert_eq!((m.grid_width, m.grid_height), (0, 0));
        assert_eq!((m.top_latitude, m.bottom_latitude), (90, -90));
        assert!(!m.wrap_x && !m.wrap_y);
        assert_eq!(m.max_plot_distance(), 1);
    }

    #[test]
    fn biggest_area() {
        let mut m = map(2, 2, false, false);
        let a = m.areas.insert_with(|id| Area::with_counts(id, false, 0, 0, 0, 0));
        let b = m.areas.insert_with(|id| Area::with_counts(id, false, 0, 0, 0, 0));
        let w = m.areas.insert_with(|id| Area::with_counts(id, true, 0, 0, 0, 0));
        m.areas[a].num_tiles = 3;
        m.areas[b].num_tiles = 3;
        m.areas[w].num_tiles = 1;
        assert_eq!(m.find_biggest_area(false), Some(a));
        assert_eq!(m.find_biggest_area(true), Some(w));
        assert_eq!(m.get_num_land_areas(), 2);
    }
}
