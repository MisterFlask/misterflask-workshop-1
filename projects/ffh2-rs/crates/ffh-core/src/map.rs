//! Port of `CvMap` (CvMap.h / CvMap.cpp): the plot grid and the areas.
//!
//! `Map` owns the plots (`plots[y * grid_width + x]`, indexable by `PlotIdx`) and the
//! areas (`Slab<AreaId, Area>`). Pure geometry (`plot_xy`, `plot_idx`, `xy`, `is_plot`,
//! distances, ...) is `impl Map`; every other rules method of CvMap is an `impl Game`
//! method `map_*`. Trivial getters/setters are fields. See `docs/porting/map.md`.

use std::collections::VecDeque;

use crate::area::Area;
use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::path::{FinderCtx, PathNode, ASNL_ADDCLOSED};
use crate::plot::Plot;
use crate::slab::Slab;
use crate::structs::XYCoords;
use crate::utils::estimate_direction;
use ffh_data::*;

/// `iPlotsPerCellX` / `iPlotsPerCellY` of the active landscape (C++
/// `GC.getLandscapeInfo(GC.getActiveLandscapeID())`). BTS `CIV4TerrainSettings.xml` defines a
/// single landscape, `LANDSCAPE_DEFAULT`, with 4 x 4 plots per cell; FFH2 does not override
/// it and ffh-data does not load the (otherwise graphics-only) landscape infos.
const LANDSCAPE_PLOTS_PER_CELL_X: i32 = 4;
const LANDSCAPE_PLOTS_PER_CELL_Y: i32 = 4;

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
    /// id (= creation) order. C++ iterates by slot and would reuse a deleted area's slot, but
    /// its only `deleteArea` call (`setPlotType`) is dead code (see `plot_set_plot_type`) and
    /// `recalculateAreas` empties the array (`removeAll` restarts at slot 0), so the C++ order
    /// is creation order too.
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
        // gDLL->logMemState(..): debug only.
        let ms = self.map_script.clone();
        ms.before_init(self);

        // reset(pInitInfo)
        let init_data = match init {
            Some(init) => *init,
            None => self.map_init_data_from_script(),
        };
        // C++ `CvPlot::c_aePirateCoveTypes` is a static of CvPlot, not reset with the map.
        let pirate_cove_types = std::mem::take(&mut self.map.pirate_cove_types);
        self.map = Map::new(self.d, &init_data, self.num_players(), self.num_teams());
        self.map.pirate_cove_types = pirate_cove_types;

        // m_areas.init() (Map::new starts with no areas), then setup().
        self.map_setup();

        // m_pMapPlots = new CvPlot[numPlotsINLINE()] + CvPlot::init(iX, iY) (done by Map::new):
        // the first CvPlot constructor loads the static pirate cove set (once per process in
        // C++, `c_bPirateCoveTypesLoaded`).
        if self.map.num_plots() > 0 && self.map.pirate_cove_types.is_empty() {
            self.plot_init_pirate_cove_types();
        }
        self.map_calculate_areas();
    }

    /// The `pInitInfo == NULL` branch of C++ `CvMap::reset`: grid size of the world size (or the
    /// map script's `getGridSize`) in cells, converted to plots; latitudes and wrapping from the
    /// map script when it overrides both values.
    fn map_init_data_from_script(&self) -> MapInitData {
        let ms = self.map_script.clone();
        let world_size = self.setup.world_size; // never NO_WORLDSIZE here
        let world = &self.d.worlds[world_size];
        let mut grid_w = world.grid_width;
        let mut grid_h = world.grid_height;
        // check map script for grid size override
        if let Some((w, h)) = ms.get_grid_size(self, world_size) {
            debug_assert!(w > 0 && h > 0, "the width and height returned by python getGridSize() must be positive");
            grid_w = w;
            grid_h = h;
        }
        // convert to plot dimensions (GC.getNumLandscapeInfos() > 0)
        grid_w *= LANDSCAPE_PLOTS_PER_CELL_X;
        grid_h *= LANDSCAPE_PLOTS_PER_CELL_Y;

        // C++ quirk: without a map script override the latitudes are not assigned at all, so
        // the map keeps the previous ones (90 / -90 from the CvMap constructor).
        let mut top_latitude = self.map.top_latitude;
        let mut bottom_latitude = self.map.bottom_latitude;
        let top = ms.get_top_latitude(self);
        let bottom = ms.get_bottom_latitude(self);
        if let (Some(top), Some(bottom)) = (top, bottom) {
            if top != -1 && bottom != -1 {
                top_latitude = top;
                bottom_latitude = bottom;
            }
        }

        let mut wrap_x = true;
        let mut wrap_y = false;
        if let (Some(x), Some(y)) = (ms.get_wrap_x(self), ms.get_wrap_y(self)) {
            wrap_x = x;
            wrap_y = y;
        }

        MapInitData { grid_w, grid_h, top_latitude, bottom_latitude, wrap_x, wrap_y }
    }

    /// C++ `CvMap::setup`: (re)initializes the FAStar finders for the map size. (The
    /// interface path finder is UI only.)
    fn map_setup(&mut self) {
        let geom = self.map_geom();
        self.path_finder.get_mut().initialize(geom);
        self.step_finder.get_mut().initialize(geom);
        self.route_finder.get_mut().initialize(geom);
        self.border_finder.get_mut().initialize(geom);
        self.area_finder.get_mut().initialize(geom);
        self.plot_group_finder.get_mut().initialize(geom);
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
        let init_data = MapInitData { grid_w, grid_h, top_latitude, bottom_latitude, wrap_x, wrap_y };

        // Set init core data
        self.setup.world_size = world_size;
        self.setup.climate = climate;
        self.setup.sea_level = sea_level;
        // setCustomMapOptions: custom map options are not ported.

        // Init map
        self.map_init(Some(&init_data));
    }

    /// C++ `CvMap::calculateAreas` (protected): every plot without an area starts a new area
    /// and flood-fills it (C++ area finder: `areaValid` / `joinArea`).
    pub fn map_calculate_areas(&mut self) {
        for p in self.map.plot_ids() {
            // gDLL->callUpdater(): UI only.
            if self.map.plots[p].area.is_none() {
                let water = self.plot_is_water(p);
                let area = self.map_add_area(water);

                self.plot_set_area(p, Some(area));

                // GeneratePath(&GC.getAreaFinder(), x, y, -1, -1, pLoopPlot->isWater(), iArea):
                // water areas only connect through cardinal directions.
                let (x, y) = self.map.xy(p);
                let ctx = FinderCtx { start_x: x, start_y: y, dest_x: -1, dest_y: -1, info: area.i() };
                let water = self.plot_is_water(p);
                let closed = self.map_flood_fill(p, water, |g, parent, node| {
                    let parent = parent.map(|q| g.finder_node(q));
                    g.area_valid(parent.as_ref(), &g.finder_node(node), 0, &ctx)
                });
                for q in closed {
                    let node = self.finder_node(q);
                    self.join_area(&node, ASNL_ADDCLOSED, &ctx);
                }
            }
        }
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
        for p in self.map.plot_ids() {
            self.plot_set_revealed(p, team, new_value, terrain_only, None, false);
        }

        self.update_plot_groups();
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
        for p in self.map.plot_ids() {
            self.plot_update_sight(p, increment, false);
        }

        self.update_plot_groups();
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
        for p in self.map.plot_ids() {
            if self.plot_area(p) == Some(area) {
                self.map.plots[p].min_original_start_dist = -1;
            }
        }

        for i in self.civ_player_ids() {
            let Some(starting_plot) = self.player_get_starting_plot(i) else {
                continue;
            };
            if self.plot_area(starting_plot) != Some(area) {
                continue;
            }
            let (sx, sy) = self.map.xy(starting_plot);
            for p in self.map.plot_ids() {
                if self.plot_area(p) == Some(area) {
                    // C++: the path distance (`calculatePathDistance`) is commented out in
                    // favour of the step distance, which is never -1.
                    let (x, y) = self.map.xy(p);
                    let dist = self.step_distance(sx, sy, x, y);

                    if dist != -1 {
                        let current = self.map.plots[p].min_original_start_dist;
                        if current == -1 || dist < current {
                            self.map.plots[p].min_original_start_dist = dist;
                        }
                    }
                }
            }
        }
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
    pub fn map_combine_plot_groups(&mut self, _player: PlayerId, group1: PlotGroupId, group2: PlotGroupId) {
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
        let mut plot = None;

        let mut count = 0;

        while count < timeout {
            // C++ quirk: `plotSorenINLINE(getSorenRandNum(W, "Rand Plot Width"),
            // getSorenRandNum(H, "Rand Plot Height"))` -- MSVC evaluates call arguments right
            // to left, so the height is drawn before the width.
            let (grid_w, grid_h) = (self.map.grid_width, self.map.grid_height);
            let y = self.soren_rand_num(grid_h, "Rand Plot Height");
            let x = self.soren_rand_num(grid_w, "Rand Plot Width");
            let test_plot = self.map.plot_soren(x, y).expect("TestPlot is not assigned a valid value");

            if area.is_none() || self.map.plots[test_plot].area == area {
                let mut valid = true;

                if valid && min_unit_distance != -1 {
                    for dx in -min_unit_distance..=min_unit_distance {
                        for dy in -min_unit_distance..=min_unit_distance {
                            if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                                if self.plot_is_unit(loop_plot) {
                                    valid = false;
                                }
                            }
                        }
                    }
                }

                if valid && (flags & RANDPLOT_LAND) != 0 && self.plot_is_water(test_plot) {
                    valid = false;
                }

                if valid && (flags & RANDPLOT_UNOWNED) != 0 && self.plot_is_owned(test_plot) {
                    valid = false;
                }

                if valid && (flags & RANDPLOT_ADJACENT_UNOWNED) != 0 && self.plot_is_adjacent_owned(test_plot) {
                    valid = false;
                }

                if valid && (flags & RANDPLOT_ADJACENT_LAND) != 0 && !self.plot_is_adjacent_to_land(test_plot) {
                    valid = false;
                }

                if valid && (flags & RANDPLOT_PASSIBLE) != 0 && self.plot_is_impassable(test_plot) {
                    valid = false;
                }

                if valid && (flags & RANDPLOT_NOT_VISIBLE_TO_CIV) != 0 && self.plot_is_visible_to_civ_team(test_plot) {
                    valid = false;
                }

                if valid && (flags & RANDPLOT_NOT_CITY) != 0 && self.plot_is_city(test_plot, false, None) {
                    valid = false;
                }

                if valid {
                    plot = Some(test_plot);
                    break;
                }
            }

            count += 1;
        }

        plot
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
        // XXX look for barbarian cities???

        let mut best_value = MAX_INT;
        let mut best_city = None;

        // C++ `plotINLINE(iX, iY)->area()` (re-read per city; it cannot change in the loop).
        let plot_area = self.map.plot_xy(x, y).and_then(|p| self.plot_area(p));

        for i in self.player_ids() {
            if !self.players[i].alive {
                continue;
            }
            if owner.is_some() && owner != Some(i) {
                continue;
            }
            let player_team = self.players[i].team;
            if team.is_some() && team != Some(player_team) {
                continue;
            }
            for &c in &self.players[i].cities {
                if !same_area || Some(self.city_area(c)) == plot_area || (coastal_only && self.city_water_area(c) == plot_area) {
                    if !coastal_only || self.city_is_coastal(c, self.d.defines.min_water_size_for_ocean) {
                        if team_at_war_with.is_none_or(|t| self.at_war(player_team, t)) {
                            let (cx, cy) = (self.cities[c].x, self.cities[c].y);
                            if direction.is_none() || estimate_direction(self.dx_wrap(cx - x), self.dy_wrap(cy - y)) == direction {
                                if skip_city.is_none() || Some(c) != skip_city {
                                    let value = self.plot_distance(x, y, cx, cy);

                                    if value < best_value {
                                        best_value = value;
                                        best_city = Some(c);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        best_city
    }

    /// C++ `CvMap::findSelectionGroup` (C++ defaults: no owner, `bReadyToSelect = false`,
    /// `bWorkers = false`).
    pub fn map_find_selection_group(&self, x: i32, y: i32, owner: Option<PlayerId>, ready_to_select: bool, workers: bool) -> Option<GroupId> {
        // XXX look for barbarian cities???

        let mut best_value = MAX_INT;
        let mut best_group = None;

        for i in self.player_ids() {
            if !self.players[i].alive {
                continue;
            }
            if owner.is_some() && owner != Some(i) {
                continue;
            }
            for &g in &self.players[i].groups {
                if !ready_to_select || self.group_ready_to_select(g, false) {
                    if !workers || self.group_has_worker(g) {
                        let value = self.plot_distance(x, y, self.group_get_x(g), self.group_get_y(g));

                        if value < best_value {
                            best_value = value;
                            best_group = Some(g);
                        }
                    }
                }
            }
        }

        best_group
    }

    /// C++ `CvMap::findWater`: water (or fresh water) within `range` of `plot`.
    pub fn map_find_water(&self, plot: PlotIdx, range: i32, fresh_water: bool) -> bool {
        let (x, y) = self.map.xy(plot);

        for dx in -range..=range {
            for dy in -range..=range {
                if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                    if fresh_water {
                        if self.plot_is_fresh_water(loop_plot) {
                            return true;
                        }
                    } else if self.plot_is_water(loop_plot) {
                        return true;
                    }
                }
            }
        }

        false
    }

    // ------------------------------------------------------------------
    // Path distance (C++ step finder)
    // ------------------------------------------------------------------

    /// C++ `CvMap::resetPathDistance`: clears the step finder cache.
    pub fn map_reset_path_distance(&mut self) {
        self.step_finder.get_mut().force_reset();
    }

    /// C++ `CvMap::calculatePathDistance`: step-finder path length, -1 if unreachable (C++
    /// also returns -1 for NULL plots; all callers pass real plots).
    pub fn map_calculate_path_distance(&mut self, source: PlotIdx, dest: PlotIdx) -> i32 {
        let (sx, sy) = self.map.xy(source);
        let (dx, dy) = self.map.xy(dest);

        let g: &Game = self;
        let mut finder = g.step_finder.borrow_mut();
        if finder.generate_path(g, sx, sy, dx, dy, false, 0, true) {
            if let Some(node) = finder.last_node() {
                return node.data1;
            }
        }

        -1 // no passable path exists
    }

    // ------------------------------------------------------------------
    // FAStar flood fills (area, plot group and irrigation finders)
    // ------------------------------------------------------------------

    /// The FAStar node of plot `p` as the DLL's finder callbacks see it (`m_iX`, `m_iY`; the
    /// search bookkeeping members are not read by the flood-fill callbacks).
    pub(crate) fn finder_node(&self, p: PlotIdx) -> PathNode {
        let (x, y) = self.map.xy(p);
        PathNode { x, y, ..PathNode::default() }
    }

    /// Flood fill replacing C++ `FAStar::GeneratePath(finder, iX, iY, -1, -1, bCardinalOnly,
    /// iInfo)` on the finders that only have a validity callback (`GC.getAreaFinder()`,
    /// `GC.getPlotGroupFinder()`, `CvPlot::updateIrrigated`'s finder: no destination, cost or
    /// heuristic). Returns every plot reachable from `start` through steps that
    /// `valid(game, Some(parent), node)` accepts -- `valid(game, None, start)` is asked first, as
    /// FAStar does for the start node -- in the order they enter the closed list: breadth
    /// first, neighbours in `DirectionType` order (cardinal directions only if
    /// `cardinal_only`) on the wrapped map.
    ///
    /// The finders' NotifyList callbacks (`joinArea`, `countPlotGroup`, `checkFreshWater`,
    /// `changeIrrigated`) act on `ASNL_ADDCLOSED` only, and nothing they change is read by the
    /// validity callbacks, so callers apply them to the returned plots in order.
    pub(crate) fn map_flood_fill(&self, start: PlotIdx, cardinal_only: bool, valid: impl Fn(&Game, Option<PlotIdx>, PlotIdx) -> bool) -> Vec<PlotIdx> {
        let mut closed = Vec::new();
        if !valid(self, None, start) {
            return closed;
        }

        let mut seen = vec![false; self.map.num_plots()];
        let mut open = VecDeque::new();
        seen[start.idx()] = true;
        open.push_back(start);

        while let Some(p) = open.pop_front() {
            closed.push(p);
            let (x, y) = self.map.xy(p);
            for i in 0..DirectionType::NUM {
                let dir = DirectionType::from_usize(i);
                if cardinal_only && !crate::utils::is_cardinal_direction(dir) {
                    continue;
                }
                if let Some(q) = self.plot_direction(x, y, dir) {
                    if !seen[q.idx()] && valid(self, Some(p), q) {
                        seen[q.idx()] = true;
                        open.push_back(q);
                    }
                }
            }
        }

        closed
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::rng::Rng;
    use crate::setup::GameSetup;

    /// A two-civ duel game whose map is drawn as ASCII art: `rows[0]` is the northern (highest
    /// y) row; `.` water, `#` land, `h` hills, `p` peak. No areas yet (see
    /// [`assign_test_areas`]).
    pub(crate) fn game_from_ascii(rows: &[&str], wrap_x: bool, wrap_y: bool) -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 1));
        let h = rows.len() as i32;
        let w = rows[0].len() as i32;
        let init = MapInitData { grid_w: w, grid_h: h, top_latitude: 90, bottom_latitude: -90, wrap_x, wrap_y };
        g.map = Map::new(d, &init, g.num_players(), g.num_teams());
        for (r, row) in rows.iter().enumerate() {
            assert_eq!(row.len() as i32, w, "ragged map");
            let y = h - 1 - r as i32;
            for (x, c) in row.chars().enumerate() {
                let p = g.map.plot_idx(x as i32, y).unwrap();
                g.map.plots[p].plot_type = match c {
                    '.' => PlotType::PLOT_OCEAN,
                    '#' => PlotType::PLOT_LAND,
                    'h' => PlotType::PLOT_HILLS,
                    'p' => PlotType::PLOT_PEAK,
                    _ => panic!("bad map char {c:?}"),
                };
            }
        }
        g
    }

    /// The areas `CvMap::calculateAreas` builds (same flood fill, with the `areaValid` rule --
    /// same water status as the parent -- inlined), set directly on the plots and counted in
    /// `num_tiles`, without `setArea`'s side effects.
    pub(crate) fn assign_test_areas(g: &mut Game) {
        for p in g.map.plot_ids() {
            if g.map.plots[p].area.is_none() {
                let water = g.plot_is_water(p);
                let a = g.map_add_area(water);
                let closed = g.map_flood_fill(p, water, |g, parent, node| parent.is_none_or(|q| g.plot_is_water(q) == g.plot_is_water(node)));
                for q in closed {
                    g.map.plots[q].area = Some(a);
                    g.map.areas[a].num_tiles += 1;
                }
            }
        }
    }

    /// The plot at (x, y).
    pub(crate) fn at(g: &Game, x: i32, y: i32) -> PlotIdx {
        g.map.plot_idx(x, y).unwrap()
    }

    #[test]
    fn flood_fill_water_is_cardinal_land_is_diagonal() {
        // Diagonal water does not connect; diagonal land does.
        let mut g = game_from_ascii(&["#.", ".#"], false, false);
        assign_test_areas(&mut g);
        let area = |x, y| g.map.plots[at(&g, x, y)].area.unwrap();
        assert_ne!(area(0, 0), area(1, 1));
        assert_eq!(area(1, 0), area(0, 1));
        assert_eq!(g.map.areas.len(), 3);
        assert_eq!(g.map.get_num_land_areas(), 1);
        assert_eq!(g.map.areas[area(1, 0)].num_tiles, 2);
        assert!(g.map.areas[area(0, 0)].water);
    }

    #[test]
    fn flood_fill_order_and_wrap() {
        // 4x1 strip: water at both ends meets through the x wrap only when wrapping.
        let mut g = game_from_ascii(&[".##."], true, false);
        let closed = g.map_flood_fill(at(&g, 0, 0), true, |g, parent, node| parent.is_none_or(|q| g.plot_is_water(q) == g.plot_is_water(node)));
        assert_eq!(closed, vec![at(&g, 0, 0), at(&g, 3, 0)]);
        assign_test_areas(&mut g);
        assert_eq!(g.map.areas.len(), 2);

        let mut g = game_from_ascii(&[".##."], false, false);
        assign_test_areas(&mut g);
        assert_eq!(g.map.areas.len(), 3);

        // Breadth first, neighbours in direction order (N, NE, E, SE, S, SW, W, NW).
        let g = game_from_ascii(&["###", "###", "###"], false, false);
        let closed = g.map_flood_fill(at(&g, 1, 1), false, |_, _, _| true);
        let xy: Vec<(i32, i32)> = closed.iter().map(|&p| g.map.xy(p)).collect();
        assert_eq!(xy, vec![(1, 1), (1, 2), (2, 2), (2, 1), (2, 0), (1, 0), (0, 0), (0, 1), (0, 2)]);
        let closed = g.map_flood_fill(at(&g, 1, 1), true, |_, _, _| true);
        let xy: Vec<(i32, i32)> = closed.iter().map(|&p| g.map.xy(p)).collect();
        assert_eq!(xy, vec![(1, 1), (1, 2), (2, 1), (1, 0), (0, 1), (2, 2), (0, 2), (2, 0), (0, 0)]);

        // An invalid start (C++ validity callback with a NULL parent) floods nothing; steps are
        // validated per (parent, node) pair.
        assert!(g.map_flood_fill(at(&g, 1, 1), false, |_, parent, _| parent.is_some()).is_empty());
        let only_from_center = g.map_flood_fill(at(&g, 0, 0), false, |g, parent, _| parent.is_none_or(|q| q == at(g, 0, 0)));
        assert_eq!(only_from_center.len(), 4);
    }

    #[test]
    fn sync_rand_plot_draws_height_first() {
        let mut g = game_from_ascii(&["....", "..#.", "...."], false, false);
        assign_test_areas(&mut g);
        let (w, h) = (g.map.grid_width, g.map.grid_height);

        // No conditions: the first draw wins.
        let mut r: Rng = g.soren_rand.clone();
        let ey = r.get_i(h, "");
        let ex = r.get_i(w, "");
        let got = g.map_sync_rand_plot(0, None, -1, 100);
        assert_eq!(got, Some(at(&g, ex, ey)));
        assert_eq!(g.soren_rand, r);

        // RANDPLOT_LAND: retries until the single land plot comes up.
        let mut r: Rng = g.soren_rand.clone();
        let mut expect = None;
        for _ in 0..100 {
            let y = r.get_i(h, "");
            let x = r.get_i(w, "");
            if (x, y) == (2, 1) {
                expect = Some(at(&g, x, y));
                break;
            }
        }
        assert_eq!(g.map_sync_rand_plot(RANDPLOT_LAND, None, -1, 100), expect);
        assert_eq!(g.soren_rand, r);

        // Area filter + timeout: two draws per try.
        let land_area = g.map.plots[at(&g, 2, 1)].area;
        let mut r: Rng = g.soren_rand.clone();
        let got = g.map_sync_rand_plot(0, land_area, -1, 3);
        let mut expect = None;
        for _ in 0..3 {
            let y = r.get_i(h, "");
            let x = r.get_i(w, "");
            if (x, y) == (2, 1) {
                expect = Some(at(&g, x, y));
                break;
            }
        }
        assert_eq!(got, expect);
        assert_eq!(g.soren_rand, r);

        // Timeout 0: no draw.
        let before = g.soren_rand.clone();
        assert_eq!(g.map_sync_rand_plot(0, None, -1, 0), None);
        assert_eq!(g.soren_rand, before);
    }

    #[test]
    fn find_water_range() {
        let g = game_from_ascii(&["#####", "#####", "####.", "#####"], false, false);
        let p = at(&g, 1, 1);
        assert!(!g.map_find_water(p, 1, false));
        assert!(!g.map_find_water(p, 2, false));
        assert!(g.map_find_water(p, 3, false));
        assert!(g.map_find_water(at(&g, 3, 2), 1, false));
    }

    #[test]
    fn init_data_from_world_size_and_script() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 1));
        let duel = &d.worlds[g.setup.world_size];
        let data = g.map_init_data_from_script();
        assert_eq!((data.grid_w, data.grid_h), (duel.grid_width * 4, duel.grid_height * 4));
        assert_eq!((data.top_latitude, data.bottom_latitude), (90, -90));
        assert!(data.wrap_x && !data.wrap_y);

        struct Script;
        impl crate::mapgen::MapScript for Script {
            fn get_grid_size(&self, _g: &Game, _w: WorldSizeType) -> Option<(i32, i32)> {
                Some((5, 3))
            }
            fn get_top_latitude(&self, _g: &Game) -> Option<i32> {
                Some(60)
            }
            fn get_bottom_latitude(&self, _g: &Game) -> Option<i32> {
                Some(-60)
            }
            fn get_wrap_x(&self, _g: &Game) -> Option<bool> {
                Some(false)
            }
            fn get_wrap_y(&self, _g: &Game) -> Option<bool> {
                Some(true)
            }
        }
        g.map_script = std::sync::Arc::new(Script);
        let data = g.map_init_data_from_script();
        assert_eq!((data.grid_w, data.grid_h), (20, 12));
        assert_eq!((data.top_latitude, data.bottom_latitude), (60, -60));
        assert!(!data.wrap_x && data.wrap_y);

        // Only one latitude overridden: both keep the map's current values (C++ quirk).
        struct TopOnly;
        impl crate::mapgen::MapScript for TopOnly {
            fn get_top_latitude(&self, _g: &Game) -> Option<i32> {
                Some(60)
            }
        }
        g.map_script = std::sync::Arc::new(TopOnly);
        g.map.top_latitude = 70;
        g.map.bottom_latitude = -50;
        let data = g.map_init_data_from_script();
        assert_eq!((data.top_latitude, data.bottom_latitude), (70, -50));
    }

    #[test]
    fn setup_sizes_the_finders() {
        let mut g = game_from_ascii(&["###", "###"], true, false);
        g.map_setup();
        let geom = g.map_geom();
        assert_eq!(g.step_finder.borrow().geom, geom);
        assert_eq!(g.area_finder.borrow().nodes.len(), 6);
        assert_eq!(g.plot_group_finder.borrow().geom, geom);
        g.map_reset_path_distance();
        assert!(g.step_finder.borrow().last_node.is_none());
    }

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
