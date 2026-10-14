//! CvPlot, topic "geography": plot type changes and areas, adjacency, latitude, rivers,
//! irrigation, fresh water, potential city work, and yields.

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Plot type, areas
    // ------------------------------------------------------------------

    /// C++ `CvPlot::setPlotType` (`bRebuildGraphics` dropped: graphics only; C++ default
    /// `bRecalculate = true`). Water <-> land changes re-terrain the neighbours and either
    /// recalculate all areas or move this plot to a new/adjacent area.
    pub fn plot_set_plot_type(&mut self, p: PlotIdx, plot_type: PlotType, recalculate: bool) {
        todo!("CvPlot::setPlotType")
    }

    /// C++ `CvPlot::setArea` (`getArea()` / `area()` are the `area` field).
    pub fn plot_set_area(&mut self, p: PlotIdx, area: Option<AreaId>) {
        todo!("CvPlot::setArea")
    }

    /// C++ `CvPlot::processArea` (protected): adds (`change = 1`) or removes (`-1`) this
    /// plot's contribution to an area's counters.
    pub fn plot_process_area(&mut self, p: PlotIdx, area: AreaId, change: i32) {
        todo!("CvPlot::processArea")
    }

    /// C++ `CvPlot::waterArea`: own area if water, else the biggest adjacent water area.
    pub fn plot_water_area(&self, p: PlotIdx) -> Option<AreaId> {
        todo!("CvPlot::waterArea")
    }

    /// C++ `CvPlot::secondWaterArea`: the biggest adjacent water area other than `waterArea`.
    pub fn plot_second_water_area(&self, p: PlotIdx) -> Option<AreaId> {
        todo!("CvPlot::secondWaterArea")
    }

    /// C++ `CvPlot::isAdjacentToArea` (both overloads: `int iAreaID` and `const CvArea*`).
    pub fn plot_is_adjacent_to_area(&self, p: PlotIdx, area: AreaId) -> bool {
        todo!("CvPlot::isAdjacentToArea")
    }

    /// C++ `CvPlot::shareAdjacentArea`.
    pub fn plot_share_adjacent_area(&self, p: PlotIdx, other: PlotIdx) -> bool {
        todo!("CvPlot::shareAdjacentArea")
    }

    /// C++ `CvPlot::isAdjacentToLand`.
    pub fn plot_is_adjacent_to_land(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isAdjacentToLand")
    }

    /// C++ `CvPlot::isAdjacentToWater` (FFH).
    pub fn plot_is_adjacent_to_water(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isAdjacentToWater")
    }

    /// C++ `CvPlot::isCoastalLand` (C++ default `iMinWaterSize = -1`).
    pub fn plot_is_coastal_land(&self, p: PlotIdx, min_water_size: i32) -> bool {
        todo!("CvPlot::isCoastalLand")
    }

    /// C++ `CvPlot::getNearestLandPlotInternal` (recursive ring search).
    pub fn plot_get_nearest_land_plot_internal(&self, p: PlotIdx, distance: i32) -> Option<PlotIdx> {
        todo!("CvPlot::getNearestLandPlotInternal")
    }

    /// C++ `CvPlot::getNearestLandArea` (C++ -1 = None).
    pub fn plot_get_nearest_land_area(&self, p: PlotIdx) -> Option<AreaId> {
        todo!("CvPlot::getNearestLandArea")
    }

    /// C++ `CvPlot::getNearestLandPlot`.
    pub fn plot_get_nearest_land_plot(&self, p: PlotIdx) -> Option<PlotIdx> {
        todo!("CvPlot::getNearestLandPlot")
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
        match self.map.plots[p].area {
            Some(a) if self.map.areas.contains(a) => self.area_is_lake(a),
            _ => false,
        }
    }

    /// C++ `CvPlot::isFreshWater`: river, adjacent lake or fresh-water feature.
    pub fn plot_is_fresh_water(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isFreshWater")
    }

    /// C++ `CvPlot::isRiver`.
    #[inline]
    pub fn plot_is_river(&self, p: PlotIdx) -> bool {
        self.map.plots[p].river_crossing_count > 0
    }

    /// C++ `CvPlot::isRiverSide`.
    pub fn plot_is_river_side(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isRiverSide")
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
        todo!("CvPlot::updateRiverCrossing(DirectionTypes)")
    }

    /// C++ `CvPlot::updateRiverCrossing()`: all directions.
    pub fn plot_update_river_crossing(&mut self, p: PlotIdx) {
        for i in 0..DirectionType::NUM {
            self.plot_update_river_crossing_dir(p, DirectionType::from_usize(i));
        }
    }

    /// C++ `CvPlot::setNOfRiver`.
    pub fn plot_set_n_of_river(&mut self, p: PlotIdx, new_value: bool, river_dir: Option<CardinalDirectionType>) {
        todo!("CvPlot::setNOfRiver")
    }

    /// C++ `CvPlot::setWOfRiver`.
    pub fn plot_set_w_of_river(&mut self, p: PlotIdx, new_value: bool, river_dir: Option<CardinalDirectionType>) {
        todo!("CvPlot::setWOfRiver")
    }

    /// C++ `CvPlot::getInlandCorner` (map generation). Const in C++ but shuffles with the map
    /// RNG (`GC.getGameINLINE().getMapRand()`), hence `&mut self`.
    pub fn plot_get_inland_corner(&mut self, p: PlotIdx) -> Option<PlotIdx> {
        todo!("CvPlot::getInlandCorner")
    }

    /// C++ `CvPlot::hasCoastAtSECorner`.
    pub fn plot_has_coast_at_se_corner(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::hasCoastAtSECorner")
    }

    /// C++ `CvPlot::isPotentialIrrigation`.
    pub fn plot_is_potential_irrigation(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::isPotentialIrrigation")
    }

    /// C++ `CvPlot::canHavePotentialIrrigation`.
    pub fn plot_can_have_potential_irrigation(&self, p: PlotIdx) -> bool {
        todo!("CvPlot::canHavePotentialIrrigation")
    }

    /// C++ `CvPlot::isIrrigationAvailable` (C++ default `bIgnoreSelf = false`).
    pub fn plot_is_irrigation_available(&self, p: PlotIdx, ignore_self: bool) -> bool {
        todo!("CvPlot::isIrrigationAvailable")
    }

    /// C++ `CvPlot::setIrrigated`.
    pub fn plot_set_irrigated(&mut self, p: PlotIdx, new_value: bool) {
        todo!("CvPlot::setIrrigated")
    }

    /// C++ `CvPlot::updateIrrigated`: spreads/removes irrigation (C++ uses a private FAStar
    /// flood fill with `potentialIrrigation` / `checkFreshWater` / `changeIrrigated`).
    pub fn plot_update_irrigated(&mut self, p: PlotIdx) {
        todo!("CvPlot::updateIrrigated")
    }

    // ------------------------------------------------------------------
    // Potential city work
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isPotentialCityWorkForArea`.
    pub fn plot_is_potential_city_work_for_area(&self, p: PlotIdx, area: AreaId) -> bool {
        todo!("CvPlot::isPotentialCityWorkForArea")
    }

    /// C++ `CvPlot::updatePotentialCityWork` (FFH: only the 21 inner city plots count).
    pub fn plot_update_potential_city_work(&mut self, p: PlotIdx) {
        todo!("CvPlot::updatePotentialCityWork")
    }

    // ------------------------------------------------------------------
    // Yields (`getYield(e)` is the `yields` field)
    // ------------------------------------------------------------------

    /// C++ `CvPlot::calculateNatureYield` (C++ default `bIgnoreFeature = false`).
    pub fn plot_calculate_nature_yield(&self, p: PlotIdx, yield_type: YieldType, team: Option<TeamId>, ignore_feature: bool) -> i32 {
        todo!("CvPlot::calculateNatureYield")
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
        todo!("CvPlot::calculateImprovementYieldChange")
    }

    /// C++ `CvPlot::calculateYield` (`bDisplay` dropped: it selects the active team's revealed
    /// state for UI display; rules always use the real state).
    pub fn plot_calculate_yield(&self, p: PlotIdx, yield_type: YieldType) -> i32 {
        todo!("CvPlot::calculateYield")
    }

    /// C++ `CvPlot::hasYield`.
    pub fn plot_has_yield(&self, p: PlotIdx) -> bool {
        self.map.plots[p].yields.iter().any(|&v| v > 0)
    }

    /// C++ `CvPlot::updateYield`: recomputes `yields` and updates the working city.
    pub fn plot_update_yield(&mut self, p: PlotIdx) {
        todo!("CvPlot::updateYield")
    }

    /// C++ `CvPlot::calculateMaxYield` (no C++ callers; exposed to Python).
    pub fn plot_calculate_max_yield(&self, p: PlotIdx, yield_type: YieldType) -> i32 {
        todo!("CvPlot::calculateMaxYield")
    }

    /// C++ `CvPlot::getYieldWithBuild` (AI-only callers).
    pub fn plot_get_yield_with_build(&self, p: PlotIdx, build: BuildType, yield_type: YieldType, with_upgrade: bool) -> i32 {
        todo!("CvPlot::getYieldWithBuild")
    }
}
