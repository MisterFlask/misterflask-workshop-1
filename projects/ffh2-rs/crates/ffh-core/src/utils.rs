//! Port of CvGameCoreUtils.cpp/.h (plus the geometry tables of `CvGlobals::init`).
//!
//! * Pure helpers (coordinates, directions, city plot indices, score/asset formulas,
//!   binomial coefficients and the combat-odds arithmetic, shuffles, calendar math) are
//!   free functions, fully ported.
//! * Helpers that need the map size take a [`MapGeom`] (C++ reads it from `GC.getMap()`);
//!   the `Game` wrappers (`Game::plot_distance`, ...) fetch it with [`Game::map_geom`].
//! * Helpers that only read static data take `d: &GameData`.
//! * Helpers that need game objects (units, teams, plots) are `impl Game` methods.
//! * The FAStar callbacks (`pathValid`, `stepCost`, ...) live in [`crate::path`].
//!
//! Not ported (UI/debug only): `directionAngle`, `setListHelp`, `PUF_isSelected`,
//! `PUF_makeInfoBarDirty`, `get*TypeString`. C++ `getCity(IDInfo)` / `getUnit(IDInfo)` are
//! `self.cities.get(id)` / `self.units.get(id)` (ids are never reused).

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::map::coord_range;
use crate::rng::Rng;
use crate::structs::TradeData;
use ffh_data::*;

// ------------------------------------------------------------------------------------------
// CvGlobals tables (CvGlobals::init, FfH 37-plot city radius)
// ------------------------------------------------------------------------------------------

/// C++ `GC.getPlotDirectionX()`, indexed by `DirectionType`.
pub const PLOT_DIRECTION_X: [i32; DirectionType::NUM] = [0, 1, 1, 1, 0, -1, -1, -1];
/// C++ `GC.getPlotDirectionY()`, indexed by `DirectionType`.
pub const PLOT_DIRECTION_Y: [i32; DirectionType::NUM] = [1, 1, 0, -1, -1, -1, 0, 1];
/// C++ `GC.getPlotCardinalDirectionX()`, indexed by `CardinalDirectionType`.
pub const PLOT_CARDINAL_DIRECTION_X: [i32; CardinalDirectionType::NUM] = [0, 1, 0, -1];
/// C++ `GC.getPlotCardinalDirectionY()`, indexed by `CardinalDirectionType`.
pub const PLOT_CARDINAL_DIRECTION_Y: [i32; CardinalDirectionType::NUM] = [1, 0, -1, 0];

/// C++ `GC.getCityPlotX()` (FfH: 37 plots, radius 3).
pub const CITY_PLOT_X: [i32; NUM_CITY_PLOTS] = [
    0, //
    0, 1, 1, 1, 0, -1, -1, -1, //
    0, 1, 2, 2, 2, 1, 0, -1, -2, -2, -2, -1, //
    0, 1, 2, 3, 3, 3, 2, 1, 0, -1, -2, -3, -3, -3, -2, -1,
];
/// C++ `GC.getCityPlotY()`.
pub const CITY_PLOT_Y: [i32; NUM_CITY_PLOTS] = [
    0, //
    1, 1, 0, -1, -1, -1, 0, 1, //
    2, 2, 1, 0, -1, -2, -2, -2, -1, 0, 1, 2, //
    3, 3, 2, 1, 0, -1, -2, -3, -3, -3, -2, -1, 0, 1, 2, 3,
];
/// C++ `GC.getCityPlotPriority()`.
pub const CITY_PLOT_PRIORITY: [i32; NUM_CITY_PLOTS] = [
    0, //
    1, 2, 1, 2, 1, 2, 1, 2, //
    3, 4, 4, 3, 4, 4, 3, 4, 4, 3, 4, 4, //
    5, 6, 7, 6, 5, 6, 7, 6, 5, 6, 7, 6, 5, 6, 7, 6,
];

/// C++ `GC.getXYCityPlot(i, j)` = `m_aaiXYCityPlot[dx + CITY_PLOTS_RADIUS][dy + CITY_PLOTS_RADIUS]`
/// (first index is x). -1 = not a city plot.
pub const XY_CITY_PLOT: [[i32; CITY_PLOTS_DIAMETER as usize]; CITY_PLOTS_DIAMETER as usize] = [
    [-1, -1, 32, 33, 34, -1, -1],
    [-1, 31, 17, 18, 19, 35, -1],
    [30, 16, 6, 7, 8, 20, 36],
    [29, 15, 5, 0, 1, 9, 21],
    [28, 14, 4, 3, 2, 10, 22],
    [-1, 27, 13, 12, 11, 23, -1],
    [-1, -1, 26, 25, 24, -1, -1],
];

/// FfH: number of city plots inside the radius-2 (BtS) fat cross. C++ hard-codes 21
/// (`plotCityXY(dx, dy, 2)` rejects indices > 21; `initScoreCalculation` uses 21).
pub const NUM_CITY_PLOTS_RADIUS_2: i32 = 21;

/// C++ `GC.getTurnRightDirection()`.
pub const TURN_RIGHT_DIRECTION: [DirectionType; DirectionType::NUM] = [
    DirectionType::DIRECTION_NORTHEAST,
    DirectionType::DIRECTION_EAST,
    DirectionType::DIRECTION_SOUTHEAST,
    DirectionType::DIRECTION_SOUTH,
    DirectionType::DIRECTION_SOUTHWEST,
    DirectionType::DIRECTION_WEST,
    DirectionType::DIRECTION_NORTHWEST,
    DirectionType::DIRECTION_NORTH,
];

/// C++ `GC.getTurnLeftDirection()`.
pub const TURN_LEFT_DIRECTION: [DirectionType; DirectionType::NUM] = [
    DirectionType::DIRECTION_NORTHWEST,
    DirectionType::DIRECTION_NORTH,
    DirectionType::DIRECTION_NORTHEAST,
    DirectionType::DIRECTION_EAST,
    DirectionType::DIRECTION_SOUTHEAST,
    DirectionType::DIRECTION_SOUTH,
    DirectionType::DIRECTION_SOUTHWEST,
    DirectionType::DIRECTION_WEST,
];

/// C++ `GC.getXYDirection(i, j)` = `m_aaeXYDirection[dx + 1][dy + 1]` (first index is x).
pub const XY_DIRECTION: [[Option<DirectionType>; DIRECTION_DIAMETER as usize]; DIRECTION_DIAMETER as usize] = [
    [Some(DirectionType::DIRECTION_SOUTHWEST), Some(DirectionType::DIRECTION_WEST), Some(DirectionType::DIRECTION_NORTHWEST)],
    [Some(DirectionType::DIRECTION_SOUTH), None, Some(DirectionType::DIRECTION_NORTH)],
    [Some(DirectionType::DIRECTION_SOUTHEAST), Some(DirectionType::DIRECTION_EAST), Some(DirectionType::DIRECTION_NORTHEAST)],
];

// ------------------------------------------------------------------------------------------
// Basic math (CvGameCoreUtils.h inlines)
// ------------------------------------------------------------------------------------------

/// C++ `range(int iNum, int iLow, int iHigh)` (clamp).
#[inline]
pub fn range(num: i32, low: i32, high: i32) -> i32 {
    debug_assert!(high >= low, "High should be higher than low");
    if num < low {
        low
    } else if num > high {
        high
    } else {
        num
    }
}

/// C++ `range(float fNum, float fLow, float fHigh)`.
#[inline]
pub fn range_f(num: f32, low: f32, high: f32) -> f32 {
    debug_assert!(high >= low, "High should be higher than low");
    if num < low {
        low
    } else if num > high {
        high
    } else {
        num
    }
}

/// C++ `getSign`.
#[inline]
pub fn get_sign(x: i32) -> i32 {
    if x < 0 {
        -1
    } else if x > 0 {
        1
    } else {
        0
    }
}

/// C++ `coordDistance`.
#[inline]
pub fn coord_distance(from: i32, to: i32, range: i32, wrap: bool) -> i32 {
    if wrap && (from - to).abs() > range / 2 {
        return range - (from - to).abs();
    }
    (from - to).abs()
}

/// C++ `wrapCoordDifference`.
#[inline]
pub fn wrap_coord_difference(diff: i32, range: i32, wrap: bool) -> i32 {
    if wrap {
        if diff > range / 2 {
            return diff - range;
        } else if diff < -(range / 2) {
            return diff + range;
        }
    }
    diff
}

// ------------------------------------------------------------------------------------------
// Map geometry
// ------------------------------------------------------------------------------------------

/// The map dimensions the C++ coordinate helpers read from `GC.getMap()`
/// (`getGridWidth/Height`, `isWrapX/Y`); `Game::map_geom()` takes them from `Game::map`.
/// Also mirrors the CvMap.h inline index helpers (`isPlot`, `plotNum`, `plotX/Y`,
/// `plotINLINE`, `plotSorenINLINE`; `Map` has the same functions) so that code without a
/// `Game` (path finders, tests) can turn coordinates into [`PlotIdx`] (`y * width + x`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct MapGeom {
    pub width: i32,
    pub height: i32,
    pub wrap_x: bool,
    pub wrap_y: bool,
}

impl MapGeom {
    pub fn new(width: i32, height: i32, wrap_x: bool, wrap_y: bool) -> MapGeom {
        MapGeom { width, height, wrap_x, wrap_y }
    }

    /// C++ `xDistance`.
    #[inline]
    pub fn x_distance(&self, from_x: i32, to_x: i32) -> i32 {
        coord_distance(from_x, to_x, self.width, self.wrap_x)
    }

    /// C++ `yDistance`.
    #[inline]
    pub fn y_distance(&self, from_y: i32, to_y: i32) -> i32 {
        coord_distance(from_y, to_y, self.height, self.wrap_y)
    }

    /// C++ `dxWrap`.
    #[inline]
    pub fn dx_wrap(&self, dx: i32) -> i32 {
        wrap_coord_difference(dx, self.width, self.wrap_x)
    }

    /// C++ `dyWrap`.
    #[inline]
    pub fn dy_wrap(&self, dy: i32) -> i32 {
        wrap_coord_difference(dy, self.height, self.wrap_y)
    }

    /// C++ `plotDistance`: `max(dx, dy) + min(dx, dy) / 2`.
    #[inline]
    pub fn plot_distance(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        let dx = self.x_distance(x1, x2);
        let dy = self.y_distance(y1, y2);
        dx.max(dy) + dx.min(dy) / 2
    }

    /// C++ `stepDistance`: `max(dx, dy)`.
    #[inline]
    pub fn step_distance(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        self.x_distance(x1, x2).max(self.y_distance(y1, y2))
    }

    /// C++ `CvMap::numPlotsINLINE`.
    #[inline]
    pub fn num_plots(&self) -> i32 {
        self.width * self.height
    }

    /// C++ `CvMap::isPlotINLINE`.
    #[inline]
    pub fn is_plot(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width && y >= 0 && y < self.height
    }

    /// C++ `CvMap::plotNumINLINE`.
    #[inline]
    pub fn plot_num(&self, x: i32, y: i32) -> i32 {
        y * self.width + x
    }

    /// C++ `CvMap::plotX(iIndex)`.
    #[inline]
    pub fn plot_x(&self, idx: PlotIdx) -> i32 {
        idx.0 as i32 % self.width
    }

    /// C++ `CvMap::plotY(iIndex)`.
    #[inline]
    pub fn plot_y(&self, idx: PlotIdx) -> i32 {
        idx.0 as i32 / self.width
    }

    /// `(x, y)` of a plot index.
    #[inline]
    pub fn plot_coords(&self, idx: PlotIdx) -> (i32, i32) {
        (self.plot_x(idx), self.plot_y(idx))
    }

    /// C++ `CvMap::plotINLINE(iX, iY)`: wraps the coordinates, `None` off the map or for
    /// `INVALID_PLOT_COORD`.
    #[inline]
    pub fn plot(&self, x: i32, y: i32) -> Option<PlotIdx> {
        if x == INVALID_PLOT_COORD || y == INVALID_PLOT_COORD {
            return None;
        }
        let mx = coord_range(x, self.width, self.wrap_x);
        let my = coord_range(y, self.height, self.wrap_y);
        if self.is_plot(mx, my) {
            Some(PlotIdx(self.plot_num(mx, my) as u32))
        } else {
            None
        }
    }

    /// C++ `CvMap::plotSorenINLINE(iX, iY)`: no wrapping or bounds check (coordinates must be
    /// on the map; `None` only for `INVALID_PLOT_COORD`).
    #[inline]
    pub fn plot_soren(&self, x: i32, y: i32) -> Option<PlotIdx> {
        if x == INVALID_PLOT_COORD || y == INVALID_PLOT_COORD {
            return None;
        }
        debug_assert!(self.is_plot(x, y));
        Some(PlotIdx(self.plot_num(x, y) as u32))
    }

    /// C++ `plotDirection(iX, iY, eDirection)` (`NO_DIRECTION` = the plot itself).
    #[inline]
    pub fn plot_direction(&self, x: i32, y: i32, dir: Option<DirectionType>) -> Option<PlotIdx> {
        match dir {
            None => self.plot(x, y),
            Some(d) => self.plot(x + PLOT_DIRECTION_X[d.idx()], y + PLOT_DIRECTION_Y[d.idx()]),
        }
    }

    /// C++ `plotCardinalDirection`.
    #[inline]
    pub fn plot_cardinal_direction(&self, x: i32, y: i32, dir: CardinalDirectionType) -> Option<PlotIdx> {
        self.plot(x + PLOT_CARDINAL_DIRECTION_X[dir.idx()], y + PLOT_CARDINAL_DIRECTION_Y[dir.idx()])
    }

    /// C++ `plotXY(iX, iY, iDX, iDY)`.
    #[inline]
    pub fn plot_xy(&self, x: i32, y: i32, dx: i32, dy: i32) -> Option<PlotIdx> {
        self.plot(x + dx, y + dy)
    }

    /// The plot at city plot index `index` around `(x, y)`, without the FfH city-radius check
    /// of C++ `plotCity` (see [`Game::plot_city`]).
    #[inline]
    pub fn plot_city_offset(&self, x: i32, y: i32, index: usize) -> Option<PlotIdx> {
        self.plot(x + CITY_PLOT_X[index], y + CITY_PLOT_Y[index])
    }

    /// C++ `directionXY(const CvPlot* pFromPlot, const CvPlot* pToPlot)`.
    #[inline]
    pub fn direction_xy_plots(&self, from: PlotIdx, to: PlotIdx) -> Option<DirectionType> {
        let (fx, fy) = self.plot_coords(from);
        let (tx, ty) = self.plot_coords(to);
        direction_xy(self.dx_wrap(tx - fx), self.dy_wrap(ty - fy))
    }

    /// C++ `estimateDirection(const CvPlot* pFromPlot, const CvPlot* pToPlot)`.
    #[inline]
    pub fn estimate_direction_plots(&self, from: PlotIdx, to: PlotIdx) -> Option<DirectionType> {
        let (fx, fy) = self.plot_coords(from);
        let (tx, ty) = self.plot_coords(to);
        estimate_direction(self.dx_wrap(tx - fx), self.dy_wrap(ty - fy))
    }
}

// ------------------------------------------------------------------------------------------
// Directions and city plots
// ------------------------------------------------------------------------------------------

/// C++ `directionXY(iDX, iDY)`: `None` (NO_DIRECTION) unless both deltas are in -1..=1, and for (0, 0).
#[inline]
pub fn direction_xy(dx: i32, dy: i32) -> Option<DirectionType> {
    if dx.abs() > DIRECTION_RADIUS || dy.abs() > DIRECTION_RADIUS {
        None
    } else {
        XY_DIRECTION[(dx + DIRECTION_RADIUS) as usize][(dy + DIRECTION_RADIUS) as usize]
    }
}

/// C++ `plotCityXY(iDX, iDY)`: city plot index of an offset, `None` (-1) outside the fat cross.
#[inline]
pub fn plot_city_xy(dx: i32, dy: i32) -> Option<usize> {
    if dx.abs() > CITY_PLOTS_RADIUS || dy.abs() > CITY_PLOTS_RADIUS {
        None
    } else {
        let i = XY_CITY_PLOT[(dx + CITY_PLOTS_RADIUS) as usize][(dy + CITY_PLOTS_RADIUS) as usize];
        (i >= 0).then_some(i as usize)
    }
}

/// C++ `plotCityXY(iDX, iDY, iRadius)` (FfH): like [`plot_city_xy`] limited to a city radius
/// (radius 2 additionally drops the four (+-2, +-2) corners: their indices are > 21).
#[inline]
pub fn plot_city_xy_radius(dx: i32, dy: i32, radius: i32) -> Option<usize> {
    if dx.abs() > radius || dy.abs() > radius {
        return None;
    }
    // C++ indexes the 7x7 table without checking |d| <= CITY_PLOTS_RADIUS (radius <= 3 in FfH).
    let i = XY_CITY_PLOT[(dx + CITY_PLOTS_RADIUS) as usize][(dy + CITY_PLOTS_RADIUS) as usize];
    if radius == 2 && i > NUM_CITY_PLOTS_RADIUS_2 {
        return None;
    }
    (i >= 0).then_some(i as usize)
}

/// C++ `getOppositeCardinalDirection`.
#[inline]
pub fn get_opposite_cardinal_direction(dir: CardinalDirectionType) -> CardinalDirectionType {
    CardinalDirectionType(((dir.0 as usize + 2) % CardinalDirectionType::NUM) as u16)
}

/// C++ `cardinalDirectionToDirection` (`None` = NO_DIRECTION for an invalid input).
#[inline]
pub fn cardinal_direction_to_direction(card: CardinalDirectionType) -> Option<DirectionType> {
    match card {
        CardinalDirectionType::CARDINALDIRECTION_NORTH => Some(DirectionType::DIRECTION_NORTH),
        CardinalDirectionType::CARDINALDIRECTION_EAST => Some(DirectionType::DIRECTION_EAST),
        CardinalDirectionType::CARDINALDIRECTION_SOUTH => Some(DirectionType::DIRECTION_SOUTH),
        CardinalDirectionType::CARDINALDIRECTION_WEST => Some(DirectionType::DIRECTION_WEST),
        _ => None,
    }
}

/// C++ `isCardinalDirection`.
#[inline]
pub fn is_cardinal_direction(dir: DirectionType) -> bool {
    matches!(
        dir,
        DirectionType::DIRECTION_EAST | DirectionType::DIRECTION_NORTH | DirectionType::DIRECTION_SOUTH | DirectionType::DIRECTION_WEST
    )
}

/// C++ `estimateDirection(iDX, iDY)`: the direction whose unit vector has the largest positive
/// dot product with (dx, dy); `None` for (0, 0). Same single-precision arithmetic as the C++.
pub fn estimate_direction(dx: i32, dy: i32) -> Option<DirectionType> {
    let sqrt2: f32 = 1.0f32 / 2.0f32.sqrt();
    // N, NE, E, SE, S, SW, W, NW
    let displacements: [[f32; 2]; 8] = [
        [0.0, 1.0],
        [sqrt2, sqrt2],
        [1.0, 0.0],
        [sqrt2, -sqrt2],
        [0.0, -1.0],
        [-sqrt2, -sqrt2],
        [-1.0, 0.0],
        [-sqrt2, sqrt2],
    ];
    let mut maximum: f32 = 0.0;
    let mut maximum_index: i32 = -1;
    for (i, d) in displacements.iter().enumerate() {
        let dot_product = dx as f32 * d[0] + dy as f32 * d[1];
        if dot_product > maximum {
            maximum = dot_product;
            maximum_index = i as i32;
        }
    }
    opt_from_i32(maximum_index)
}

// ------------------------------------------------------------------------------------------
// Score / asset formulas
// ------------------------------------------------------------------------------------------

/// C++ `getPopulationAsset`.
#[inline]
pub fn get_population_asset(population: i32) -> i32 {
    population * 2
}

/// C++ `getLandPlotsAsset`.
#[inline]
pub fn get_land_plots_asset(land_plots: i32) -> i32 {
    land_plots
}

/// C++ `getPopulationPower`.
#[inline]
pub fn get_population_power(population: i32) -> i32 {
    population / 2
}

/// C++ `getPopulationScore`.
#[inline]
pub fn get_population_score(population: i32) -> i32 {
    population
}

/// C++ `getLandPlotsScore`.
#[inline]
pub fn get_land_plots_score(land_plots: i32) -> i32 {
    land_plots
}

/// C++ `getTechScore`: `era + 1` (C++ `getEra()` is -1 = `NO_ERA` for a tech without one).
pub fn get_tech_score(d: &GameData, tech: TechType) -> i32 {
    opt_to_i32(d.techs[tech].era) + 1
}

/// C++ `getWonderScore`.
pub fn get_wonder_score(d: &GameData, wonder_class: BuildingClassType) -> i32 {
    if is_limited_wonder_class(d, wonder_class) {
        5
    } else {
        0
    }
}

// ------------------------------------------------------------------------------------------
// Data-only predicates
// ------------------------------------------------------------------------------------------

/// C++ `isPromotionValid(ePromotion, eUnit, bLeader)` (FfH version: no BtS "Blitz needs more
/// than one move" rule and no recursive prerequisite-promotion checks).
pub fn is_promotion_valid(d: &GameData, promotion: PromotionType, unit: UnitType, leader: bool) -> bool {
    let k_unit = &d.units[unit];
    let k_promotion = &d.promotions[promotion];

    if k_unit.free_promotions[promotion] {
        return true;
    }

    let Some(unit_combat) = k_unit.unit_combat_type else {
        return false;
    };

    if !leader && k_promotion.leader {
        return false;
    }

    if !k_promotion.unit_combat[unit_combat] {
        return false;
    }

    if k_unit.only_defensive {
        if k_promotion.city_attack_percent != 0
            || k_promotion.withdrawal_change != 0
            || k_promotion.collateral_damage_change != 0
            || k_promotion.blitz
            || k_promotion.amphib
            || k_promotion.river
            || k_promotion.hills_attack_percent != 0
        {
            return false;
        }
    }

    if k_unit.ignore_terrain_cost && k_promotion.move_discount_change != 0 {
        return false;
    }

    // FfH: Modified by Kael 06/28/2008 -- BtS rejected Blitz for units with 1 move; removed.

    if k_unit.collateral_damage == 0 || k_unit.collateral_damage_limit == 0 || k_unit.collateral_damage_max_units == 0 {
        if k_promotion.collateral_damage_change != 0 {
            return false;
        }
    }

    if k_unit.interception_probability == 0 && k_promotion.intercept_change != 0 {
        return false;
    }

    // FfH: Modified by Kael 09/03/2007 -- BtS also required the promotion's prerequisite
    // promotion (and one of the two OR prerequisites) to be valid for the unit; removed.

    true
}

/// C++ `finalImprovementUpgrade(eImprovement, iCount = 0)`: the end of the improvement's
/// upgrade chain; `None` (NO_IMPROVEMENT) once more than `GC.getNumImprovementInfos()` steps
/// were taken (a cycle).
pub fn final_improvement_upgrade(d: &GameData, improvement: ImprovementType, count: i32) -> Option<ImprovementType> {
    if count > d.improvements.len() as i32 {
        return None;
    }

    match d.improvements[improvement].improvement_upgrade {
        Some(upgrade) => final_improvement_upgrade(d, upgrade, count + 1),
        None => Some(improvement),
    }
}

/// C++ `isReligionTech`.
pub fn is_religion_tech(d: &GameData, tech: TechType) -> bool {
    d.religions.iter().any(|r| r.tech_prereq == Some(tech))
}

/// C++ `isCorporationTech`.
pub fn is_corporation_tech(d: &GameData, tech: TechType) -> bool {
    d.corporations.iter().any(|c| c.tech_prereq == Some(tech))
}

/// C++ `isTechRequiredForUnit`. (`UnitInfo::prereq_and_techs` holds only the listed techs;
/// the C++ scans `NUM_UNIT_AND_TECH_PREREQS` slots whose unused entries are NO_TECH.)
pub fn is_tech_required_for_unit(d: &GameData, tech: TechType, unit: UnitType) -> bool {
    let info = &d.units[unit];

    if info.prereq_and_tech == Some(tech) {
        return true;
    }

    info.prereq_and_techs.iter().any(|&t| t == tech)
}

/// C++ `isTechRequiredForBuilding`.
pub fn is_tech_required_for_building(d: &GameData, tech: TechType, building: BuildingType) -> bool {
    let info = &d.buildings[building];

    if info.prereq_and_tech == Some(tech) {
        return true;
    }

    // C++ loops over `GC.getNUM_BUILDING_AND_TECH_PREREQS()` slots (the length of this list).
    if info.prereq_and_techs.iter().any(|&t| t == Some(tech)) {
        return true;
    }

    if let Some(special) = info.special_building_type {
        if d.special_buildings[special].tech_prereq == Some(tech) {
            return true;
        }
    }

    false
}

/// C++ `isTechRequiredForProject`.
pub fn is_tech_required_for_project(d: &GameData, tech: TechType, project: ProjectType) -> bool {
    d.projects[project].tech_prereq == Some(tech)
}

/// C++ `isWorldUnitClass`: `getMaxGlobalInstances() != -1`.
pub fn is_world_unit_class(d: &GameData, unit_class: UnitClassType) -> bool {
    d.unit_classes[unit_class].max_global_instances != -1
}

/// C++ `isTeamUnitClass`: `getMaxTeamInstances() != -1`.
pub fn is_team_unit_class(d: &GameData, unit_class: UnitClassType) -> bool {
    d.unit_classes[unit_class].max_team_instances != -1
}

/// C++ `isNationalUnitClass`: `getMaxPlayerInstances() != -1`.
pub fn is_national_unit_class(d: &GameData, unit_class: UnitClassType) -> bool {
    d.unit_classes[unit_class].max_player_instances != -1
}

/// C++ `isLimitedUnitClass`.
pub fn is_limited_unit_class(d: &GameData, unit_class: UnitClassType) -> bool {
    is_world_unit_class(d, unit_class) || is_team_unit_class(d, unit_class) || is_national_unit_class(d, unit_class)
}

/// C++ `isWorldWonderClass`: `getMaxGlobalInstances() != -1`.
pub fn is_world_wonder_class(d: &GameData, building_class: BuildingClassType) -> bool {
    d.building_classes[building_class].max_global_instances != -1
}

/// C++ `isTeamWonderClass`: `getMaxTeamInstances() != -1`.
pub fn is_team_wonder_class(d: &GameData, building_class: BuildingClassType) -> bool {
    d.building_classes[building_class].max_team_instances != -1
}

/// C++ `isNationalWonderClass`: `getMaxPlayerInstances() != -1`.
pub fn is_national_wonder_class(d: &GameData, building_class: BuildingClassType) -> bool {
    d.building_classes[building_class].max_player_instances != -1
}

/// C++ `isLimitedWonderClass`.
pub fn is_limited_wonder_class(d: &GameData, building_class: BuildingClassType) -> bool {
    is_world_wonder_class(d, building_class) || is_team_wonder_class(d, building_class) || is_national_wonder_class(d, building_class)
}

/// C++ `limitedWonderClassLimit`: sum of the global/team/player limits, -1 if unlimited.
pub fn limited_wonder_class_limit(d: &GameData, building_class: BuildingClassType) -> i32 {
    let info = &d.building_classes[building_class];
    let mut count = 0;
    let mut is_limited = false;

    let mut max = info.max_global_instances;
    if max != -1 {
        count += max;
        is_limited = true;
    }

    max = info.max_team_instances;
    if max != -1 {
        count += max;
        is_limited = true;
    }

    max = info.max_player_instances;
    if max != -1 {
        count += max;
        is_limited = true;
    }

    if is_limited {
        count
    } else {
        -1
    }
}

/// C++ `isWorldProject`: `getMaxGlobalInstances() != -1`.
pub fn is_world_project(d: &GameData, project: ProjectType) -> bool {
    d.projects[project].max_global_instances != -1
}

/// C++ `isTeamProject`: `getMaxTeamInstances() != -1`.
pub fn is_team_project(d: &GameData, project: ProjectType) -> bool {
    d.projects[project].max_team_instances != -1
}

/// C++ `isLimitedProject`.
pub fn is_limited_project(d: &GameData, project: ProjectType) -> bool {
    is_world_project(d, project) || is_team_project(d, project)
}

/// C++ `isPlotEventTrigger`: the trigger needs plots with some property (picked per plot).
pub fn is_plot_event_trigger(d: &GameData, trigger: EventTriggerType) -> bool {
    let k_trigger = &d.event_triggers[trigger];

    if k_trigger.num_plots_required > 0 {
        if k_trigger.plot_type.is_some() {
            return true;
        }

        if !k_trigger.features_required.is_empty() {
            return true;
        }

        if !k_trigger.terrains_required.is_empty() {
            return true;
        }

        if !k_trigger.improvements_required.is_empty() {
            return true;
        }

        if !k_trigger.bonuses_required.is_empty() {
            return true;
        }

        if !k_trigger.routes_required.is_empty() {
            return true;
        }

        if k_trigger.units_on_plot && !k_trigger.units_required.is_empty() {
            return true;
        }

        if k_trigger.prereq_event_city && !k_trigger.pick_city {
            return true;
        }
    }

    false
}

/// C++ `baseYieldToSymbol`.
pub fn base_yield_to_symbol(d: &GameData, num_yield_types: i32, yield_stack: i32) -> i32 {
    num_yield_types * d.defines.max_yield_stack + yield_stack
}

/// C++ `isPickableName`: false for names ending in "NOPICK" (case-insensitive).
pub fn is_pickable_name(name: &str) -> bool {
    let b = name.as_bytes();
    // C++ reads &szName[iLen - 6] unchecked; shorter names cannot end in "NOPICK".
    if b.len() >= 6 && b[b.len() - 6..].eq_ignore_ascii_case(b"NOPICK") {
        return false;
    }
    true
}

// ------------------------------------------------------------------------------------------
// Combat odds
// ------------------------------------------------------------------------------------------

/// C++ `getBinomialCoefficient(iN, iK)` (64-bit, intermediate wrap-around as in the C++).
pub fn get_binomial_coefficient(n: i32, k: i32) -> i64 {
    let mut temp: i64 = 1;
    // take advantage of symmetry in combination, eg. 15C12 = 15C3
    let k = k.min(n - k);
    let mut i = 1;
    while i <= k {
        temp = temp.wrapping_mul((n - i + 1) as i64) / i as i64;
        i += 1;
    }
    // C++: FAssert(iTemp < MAX_INT)
    temp
}

/// MSVC's `pow(float, int)` overload (`_Pow_int`): repeated squaring in single precision.
/// C++ `getCombatOdds` calls `pow(float, int)`, which resolves to this, not to `powf`.
pub fn pow_int_f32(mut x: f32, y: i32) -> f32 {
    let mut n: u32 = if y >= 0 { y as u32 } else { (y as i64).unsigned_abs() as u32 };
    let mut z: f32 = 1.0;
    loop {
        if n & 1 != 0 {
            z *= x;
        }
        n >>= 1;
        if n == 0 {
            return if y < 0 { 1.0 / z } else { z };
        }
        x *= x;
    }
}

/// The unit values C++ `getCombatOdds(pAttacker, pDefender)` reads; gathered by
/// [`Game::get_combat_odds`] and evaluated by [`combat_odds`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CombatOddsInput {
    /// `pAttacker->currCombatStr(NULL, pDefender)` (FfH passes the defender).
    pub attacker_strength: i32,
    /// `pAttacker->currFirepower(NULL, pDefender)`.
    pub attacker_firepower: i32,
    /// `pDefender->currCombatStr(pDefender->plot(), pAttacker)`.
    pub defender_strength: i32,
    /// `pDefender->currFirepower(pDefender->plot(), pAttacker)`.
    pub defender_firepower: i32,
    /// `pAttacker->currHitPoints()`.
    pub attacker_hit_points: i32,
    /// `pDefender->currHitPoints()`.
    pub defender_hit_points: i32,
    /// `pDefender->maxHitPoints()`.
    pub defender_max_hit_points: i32,
    /// `pAttacker->combatLimit()`.
    pub attacker_combat_limit: i32,
    pub attacker_first_strikes: i32,
    pub attacker_chance_first_strikes: i32,
    pub attacker_immune_to_first_strikes: bool,
    pub defender_first_strikes: i32,
    pub defender_chance_first_strikes: i32,
    pub defender_immune_to_first_strikes: bool,
}

/// The arithmetic of C++ `getCombatOdds`: attacker victory odds in 0..=1000, with the same
/// mixed int/float evaluation order (`COMBAT_DIE_SIDES`, `COMBAT_DAMAGE` defines as arguments).
pub fn combat_odds(k: &CombatOddsInput, combat_die_sides: i32, combat_damage: i32) -> i32 {
    let mut odds: i32 = 0;

    let defender_odds = (combat_die_sides * k.defender_strength) / (k.attacker_strength + k.defender_strength);
    if defender_odds == 0 {
        return 1000;
    }
    let attacker_odds = combat_die_sides - defender_odds;
    if attacker_odds == 0 {
        return 0;
    }

    let strength_factor = (k.attacker_firepower + k.defender_firepower + 1) / 2;

    // calculate damage done in one round
    let damage_to_attacker =
        1.max((combat_damage * (k.defender_firepower + strength_factor)) / (k.attacker_firepower + strength_factor));
    let damage_to_defender =
        1.max((combat_damage * (k.attacker_firepower + strength_factor)) / (k.defender_firepower + strength_factor));

    // needed rounds = round_up(health / damage)
    let defender_hit_limit = k.defender_max_hit_points - k.attacker_combat_limit;
    let needed_rounds_attacker =
        (0.max(k.defender_hit_points - defender_hit_limit) + damage_to_defender - 1) / damage_to_defender;
    let needed_rounds_defender = (k.attacker_hit_points + damage_to_attacker - 1) / damage_to_attacker;
    let max_rounds = needed_rounds_attacker + needed_rounds_defender - 1;

    // possible first strikes distribution
    let attacker_low_fs = if k.defender_immune_to_first_strikes { 0 } else { k.attacker_first_strikes };
    let attacker_high_fs =
        if k.defender_immune_to_first_strikes { 0 } else { k.attacker_first_strikes + k.attacker_chance_first_strikes };
    let defender_low_fs = if k.attacker_immune_to_first_strikes { 0 } else { k.defender_first_strikes };
    let defender_high_fs =
        if k.attacker_immune_to_first_strikes { 0 } else { k.defender_first_strikes + k.defender_chance_first_strikes };

    let die = combat_die_sides as f32;
    let p_att = attacker_odds as f32 / die;
    let p_def = defender_odds as f32 / die;

    for i in attacker_low_fs..attacker_high_fs + 1 {
        for j in defender_low_fs..defender_high_fs + 1 {
            if i >= j {
                // attacker gets more or equal first strikes than defender
                let first_strikes = i - j;
                for i3 in 0..first_strikes + 1 {
                    // event: i3 first strikes hit the defender
                    let odds_event = (get_binomial_coefficient(first_strikes, i3) as f32)
                        * pow_int_f32(p_att, i3)
                        * pow_int_f32(1.0f32 - p_att, first_strikes - i3);
                    let odds_after_event: f32 = if i3 >= needed_rounds_attacker {
                        1.0
                    } else {
                        let mut f: f32 = 0.0;
                        for i4 in (needed_rounds_attacker - i3)..(max_rounds - i3 + 1) {
                            f += (get_binomial_coefficient(max_rounds - i3, i4) as f32)
                                * pow_int_f32(p_att, i4)
                                * pow_int_f32(1.0f32 - p_att, (max_rounds - i3) - i4);
                        }
                        f
                    };
                    odds += (1000.0f64 * ((odds_event * odds_after_event) as f64 + 0.0005)) as i32;
                }
            } else {
                // attacker gets less first strikes than defender
                let first_strikes = j - i;
                for i3 in 0..first_strikes + 1 {
                    // check if the attacker is still alive
                    if i3 < needed_rounds_defender {
                        let odds_event = (get_binomial_coefficient(first_strikes, i3) as f32)
                            * pow_int_f32(p_def, i3)
                            * pow_int_f32(1.0f32 - p_def, first_strikes - i3);
                        let mut odds_after_event: f32 = 0.0;
                        for i4 in needed_rounds_attacker..(max_rounds - i3 + 1) {
                            odds_after_event += (get_binomial_coefficient(max_rounds - i3, i4) as f32)
                                * pow_int_f32(p_att, i4)
                                * pow_int_f32(1.0f32 - p_att, (max_rounds - i3) - i4);
                        }
                        odds += (1000.0f64 * ((odds_event * odds_after_event) as f64 + 0.0005)) as i32;
                    }
                }
            }
        }
    }

    // weigh the total to the number of possible combinations of first strikes events
    odds /= ((if k.defender_immune_to_first_strikes { 0 } else { k.attacker_chance_first_strikes }) + 1)
        * ((if k.attacker_immune_to_first_strikes { 0 } else { k.defender_chance_first_strikes }) + 1);
    odds
}

/// The arithmetic of C++ `getEspionageModifier` (team espionage points ever as arguments).
pub fn espionage_modifier(espionage_spending_multiplier: i32, target_points: i32, our_points: i32) -> i32 {
    let mut modifier = espionage_spending_multiplier * (2 * target_points + our_points);
    modifier /= 1.max(target_points + 2 * our_points);
    modifier
}

// ------------------------------------------------------------------------------------------
// Misc
// ------------------------------------------------------------------------------------------

/// C++ `setTradeItem(pItem, eItemType, iData)`: a fresh, non-offering, visible trade item.
pub fn set_trade_item(item_type: TradeableItem, data: i32) -> TradeData {
    TradeData { item_type, data, offering: false, hidden: false }
}

/// C++ `shuffleArray(piShuffle, iNum, rand)`: fills `shuffle` with a permutation of
/// `0..shuffle.len()` (one draw per entry, as in the C++).
pub fn shuffle_array(shuffle: &mut [i32], rng: &mut Rng) {
    let num = shuffle.len() as i32;
    for i in 0..num {
        shuffle[i as usize] = i;
    }
    for i in 0..num {
        let j = rng.get_i(num - i, "") + i;
        if i != j {
            shuffle.swap(i as usize, j as usize);
        }
    }
}

/// C++ `shuffle(iNum, rand)`.
pub fn shuffle(num: i32, rng: &mut Rng) -> Vec<i32> {
    let mut v = vec![0; num.max(0) as usize];
    shuffle_array(&mut v, rng);
    v
}

/// The calendar arithmetic of C++ `getTurnMonthForGame`. `increments` are the game speed's
/// `GameTurnInfo`s as `(iMonthIncrement, iNumGameTurnsPerIncrement)`; `num_months` /
/// `num_seasons` are `GC.getNumMonthInfos()` / `GC.getNumSeasonInfos()`.
#[allow(clippy::too_many_arguments)]
pub fn turn_month_for_game(
    game_turn: i32,
    start_year: i32,
    calendar: CalendarType,
    increments: &[(i32, i32)],
    num_months: i32,
    num_seasons: i32,
    weeks_per_months: i32,
) -> i32 {
    let mut turn_month = start_year * num_months;
    match calendar {
        CalendarType::CALENDAR_DEFAULT => {
            let mut turn_count = 0;
            for &(month_increment, turns_per_increment) in increments {
                if game_turn > turn_count + turns_per_increment {
                    turn_month += month_increment * turns_per_increment;
                    turn_count += turns_per_increment;
                } else {
                    turn_month += month_increment * (game_turn - turn_count);
                    turn_count += game_turn - turn_count;
                    break;
                }
            }
            if game_turn > turn_count {
                let (last_month_increment, _) = *increments.last().expect("game speed without turn increments");
                turn_month += last_month_increment * (game_turn - turn_count);
            }
        }
        CalendarType::CALENDAR_BI_YEARLY => turn_month += 2 * game_turn * num_months,
        CalendarType::CALENDAR_YEARS | CalendarType::CALENDAR_TURNS => turn_month += game_turn * num_months,
        CalendarType::CALENDAR_SEASONS => turn_month += (game_turn * num_months) / num_seasons,
        CalendarType::CALENDAR_MONTHS => turn_month += game_turn,
        CalendarType::CALENDAR_WEEKS => turn_month += game_turn / weeks_per_months,
        _ => debug_assert!(false, "unknown calendar"),
    }
    turn_month
}

/// C++ `getTurnMonthForGame(iGameTurn, iStartYear, eCalendar, eSpeed)`.
pub fn get_turn_month_for_game(d: &GameData, game_turn: i32, start_year: i32, calendar: CalendarType, speed: GameSpeedType) -> i32 {
    // `GC.getGameSpeedInfo(eSpeed).getGameTurnInfo(i)` for `i < getNumTurnIncrements()`.
    let increments: Vec<(i32, i32)> = d.game_speeds[speed]
        .game_turn_info
        .iter()
        .map(|t| (t.month_increment, t.num_game_turns_per_increment))
        .collect();
    turn_month_for_game(
        game_turn,
        start_year,
        calendar,
        &increments,
        d.months.len() as i32,
        d.seasons.len() as i32,
        d.defines.weeks_per_months,
    )
}

/// C++ `getTurnYearForGame(iGameTurn, iStartYear, eCalendar, eSpeed)`.
pub fn get_turn_year_for_game(d: &GameData, game_turn: i32, start_year: i32, calendar: CalendarType, speed: GameSpeedType) -> i32 {
    get_turn_month_for_game(d, game_turn, start_year, calendar, speed) / d.months.len() as i32
}

/// C++ `PlotUnitFunc` (`bool (*)(const CvUnit*, int iData1, int iData2)`): unit filters
/// passed to plot unit queries (`CvPlot::plotCount`, `plotCheck`, ...). Unused data is -1.
pub type PlotUnitFunc = fn(&Game, UnitId, i32, i32) -> bool;

// ------------------------------------------------------------------------------------------
// Helpers that need game objects
// ------------------------------------------------------------------------------------------

impl Game {
    /// The map dimensions used by the coordinate helpers (C++ `GC.getMapINLINE()` grid size
    /// and wrap flags).
    #[inline]
    pub fn map_geom(&self) -> MapGeom {
        MapGeom { width: self.map.grid_width, height: self.map.grid_height, wrap_x: self.map.wrap_x, wrap_y: self.map.wrap_y }
    }

    /// C++ `xDistance`.
    #[inline]
    pub fn x_distance(&self, from_x: i32, to_x: i32) -> i32 {
        self.map_geom().x_distance(from_x, to_x)
    }

    /// C++ `yDistance`.
    #[inline]
    pub fn y_distance(&self, from_y: i32, to_y: i32) -> i32 {
        self.map_geom().y_distance(from_y, to_y)
    }

    /// C++ `dxWrap`.
    #[inline]
    pub fn dx_wrap(&self, dx: i32) -> i32 {
        self.map_geom().dx_wrap(dx)
    }

    /// C++ `dyWrap`.
    #[inline]
    pub fn dy_wrap(&self, dy: i32) -> i32 {
        self.map_geom().dy_wrap(dy)
    }

    /// C++ `plotDistance`.
    #[inline]
    pub fn plot_distance(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        self.map_geom().plot_distance(x1, y1, x2, y2)
    }

    /// C++ `stepDistance`.
    #[inline]
    pub fn step_distance(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        self.map_geom().step_distance(x1, y1, x2, y2)
    }

    /// C++ `plotDirection(iX, iY, eDirection)`. For `NO_DIRECTION` use `plot_direction_opt`.
    #[inline]
    pub fn plot_direction(&self, x: i32, y: i32, dir: DirectionType) -> Option<PlotIdx> {
        self.map.plot_xy(x + PLOT_DIRECTION_X[dir.idx()], y + PLOT_DIRECTION_Y[dir.idx()])
    }

    /// C++ `plotDirection` with a possibly `NO_DIRECTION` (`None` = the plot itself).
    #[inline]
    pub fn plot_direction_opt(&self, x: i32, y: i32, dir: Option<DirectionType>) -> Option<PlotIdx> {
        match dir {
            None => self.map.plot_xy(x, y),
            Some(d) => self.plot_direction(x, y, d),
        }
    }

    /// C++ `plotCardinalDirection`.
    #[inline]
    pub fn plot_cardinal_direction(&self, x: i32, y: i32, dir: CardinalDirectionType) -> Option<PlotIdx> {
        self.map.plot_xy(x + PLOT_CARDINAL_DIRECTION_X[dir.idx()], y + PLOT_CARDINAL_DIRECTION_Y[dir.idx()])
    }

    /// C++ `plotXY(iX, iY, iDX, iDY)` (offset lookup; `self.map.plot_xy(x, y)` is CvMap::plot).
    #[inline]
    pub fn plot_xy(&self, x: i32, y: i32, dx: i32, dy: i32) -> Option<PlotIdx> {
        self.map.plot_xy(x + dx, y + dy)
    }

    /// C++ `directionXY(const CvPlot*, const CvPlot*)`.
    #[inline]
    pub fn direction_xy_plots(&self, from: PlotIdx, to: PlotIdx) -> Option<DirectionType> {
        let (fx, fy) = self.map.xy(from);
        let (tx, ty) = self.map.xy(to);
        direction_xy(self.dx_wrap(tx - fx), self.dy_wrap(ty - fy))
    }

    /// C++ `estimateDirection(const CvPlot*, const CvPlot*)`.
    #[inline]
    pub fn estimate_direction_plots(&self, from: PlotIdx, to: PlotIdx) -> Option<DirectionType> {
        let (fx, fy) = self.map.xy(from);
        let (tx, ty) = self.map.xy(to);
        estimate_direction(self.dx_wrap(tx - fx), self.dy_wrap(ty - fy))
    }

    /// C++ `plotCity(iX, iY, iIndex)`. FfH: if a city stands on (x, y) and
    /// `index > pCity->getNumCityPlots()` (C++ quirk: `>`, not `>=`) the result is `None`.
    pub fn plot_city(&self, x: i32, y: i32, index: usize) -> Option<PlotIdx> {
        // FfH: Added by Kael 11/18/2007
        // C++ dereferences `GC.getMapINLINE().plot(iX, iY)` (crashes off the map);
        // `getPlotCity()` is `getCity(m_plotCity)` (NULL for a destroyed city).
        let center = self.map.plot_xy(x, y).expect("plotCity: (x, y) is not on the map");
        if let Some(city) = self.map.plots[center].plot_city.filter(|&c| self.cities.contains(c)) {
            // C++ quirk: `>` lets index 21 through for a radius-2 city (21 plots, indices 0..=20).
            if index as i32 > self.city_num_city_plots(city) {
                return None;
            }
        }
        // FfH: End Add

        self.map.plot_xy(x + CITY_PLOT_X[index], y + CITY_PLOT_Y[index])
    }

    /// C++ `plotCityXY(const CvCity* pCity, const CvPlot* pPlot)` (FfH: uses the city's plot radius).
    pub fn plot_city_xy_for(&self, city: CityId, plot: PlotIdx) -> Option<usize> {
        let c = &self.cities[city];
        let (x, y) = self.map.xy(plot);
        // FfH: Modified by Kael 12/03/2007 (`getPlotRadius()` is the `plot_radius` field)
        plot_city_xy_radius(self.dx_wrap(x - c.x), self.dy_wrap(y - c.y), c.plot_radius)
    }

    /// C++ `atWar(eTeamA, eTeamB)` (`CvTeam::isAtWar` is the `Team::at_war` field).
    #[inline]
    pub fn at_war(&self, a: TeamId, b: TeamId) -> bool {
        // C++ quirk: the C++ FAssert of symmetry fails for teams FfH creates mid-game
        // (`addPlayerAdvanced`: Infernals, `formEmpire`), which are at war with the barbarians
        // one-sidedly; checked here for every other pair.
        debug_assert!(
            self.teams[a].at_war[b] == self.teams[b].at_war[a] || self.team_is_barbarian(a) || self.team_is_barbarian(b),
            "at_war({a:?}, {b:?}) is not symmetric"
        );
        debug_assert!(a != b || !self.teams[a].at_war[b]);
        self.teams[a].at_war[b]
    }

    /// C++ `atWar` with possibly `NO_TEAM` arguments (false if either is `None`).
    #[inline]
    pub fn at_war_opt(&self, a: Option<TeamId>, b: Option<TeamId>) -> bool {
        match (a, b) {
            (Some(a), Some(b)) => self.at_war(a, b),
            _ => false,
        }
    }

    /// C++ `isPotentialEnemy(eOurTeam, eTheirTeam)` (UNOFFICIAL_PATCH: sneak attack only if
    /// war can be declared). `AI_isSneakAttackReady` is AI war-plan bookkeeping (`Team` fields).
    pub fn is_potential_enemy(&self, our_team: TeamId, their_team: Option<TeamId>) -> bool {
        let Some(their_team) = their_team else {
            return false;
        };

        // UNOFFICIAL_PATCH 05/05/09 jdog5000 (original BtS: `atWar || AI_isSneakAttackReady`):
        // fixes the AI launching invasions while unable to declare war.
        self.at_war(our_team, their_team)
            || (self.team_ai_is_sneak_attack_ready(our_team, their_team) && self.team_can_declare_war(our_team, their_team))
    }

    /// C++ `isBeforeUnitCycle(pFirstUnit, pSecondUnit)`.
    pub fn is_before_unit_cycle(&self, first: UnitId, second: UnitId) -> bool {
        debug_assert!(first != second, "isBeforeUnitCycle: same unit");
        let (a, b) = (&self.units[first], &self.units[second]);

        if a.owner != b.owner {
            return a.owner < b.owner;
        }

        let (domain_a, domain_b) = (self.unit_get_domain_type(first), self.unit_get_domain_type(second));
        if domain_a != domain_b {
            return domain_a < domain_b;
        }

        let (combat_a, combat_b) = (self.unit_base_combat_str(first), self.unit_base_combat_str(second));
        if combat_a != combat_b {
            return combat_a > combat_b;
        }

        if a.unit_type != b.unit_type {
            return a.unit_type > b.unit_type;
        }

        if a.level != b.level {
            return a.level > b.level;
        }

        if a.experience != b.experience {
            return a.experience > b.experience;
        }

        // C++ `getID()`: the per-player FFreeListTrashArray id, which grows with every unit the
        // owner creates; both units have the same owner here, so the global id (creation order)
        // orders them the same way.
        first < second
    }

    /// C++ `getCombatOdds(pAttacker, pDefender)`: fill a [`CombatOddsInput`] from the units and
    /// call [`combat_odds`] with `COMBAT_DIE_SIDES` / `COMBAT_DAMAGE`.
    pub fn get_combat_odds(&self, attacker: UnitId, defender: UnitId) -> i32 {
        // C++ `pDefender->plot()` (a pointer: NULL off the map).
        let defender_plot = self.unit_plot_opt(defender);
        let k = CombatOddsInput {
            // FfH: Modified by Kael 09/02/2007 (BtS: `currCombatStr(NULL, NULL)` and
            // `currFirepower(NULL, NULL)`): the attacker's strength against this defender.
            attacker_strength: self.unit_curr_combat_str(attacker, None, Some(defender), None),
            attacker_firepower: self.unit_curr_firepower(attacker, None, Some(defender)),
            defender_strength: self.unit_curr_combat_str(defender, defender_plot, Some(attacker), None),
            defender_firepower: self.unit_curr_firepower(defender, defender_plot, Some(attacker)),
            attacker_hit_points: self.unit_curr_hit_points(attacker),
            defender_hit_points: self.unit_curr_hit_points(defender),
            defender_max_hit_points: self.unit_max_hit_points(defender),
            // `combatLimit()` returns the FfH `m_iCombatLimit` member.
            attacker_combat_limit: self.units[attacker].combat_limit,
            attacker_first_strikes: self.unit_first_strikes(attacker),
            attacker_chance_first_strikes: self.unit_chance_first_strikes(attacker),
            attacker_immune_to_first_strikes: self.unit_immune_to_first_strikes(attacker),
            defender_first_strikes: self.unit_first_strikes(defender),
            defender_chance_first_strikes: self.unit_chance_first_strikes(defender),
            defender_immune_to_first_strikes: self.unit_immune_to_first_strikes(defender),
        };

        debug_assert!(k.attacker_strength + k.defender_strength > 0);
        debug_assert!(k.attacker_firepower + k.defender_firepower > 0);

        combat_odds(&k, self.d.defines.combat_die_sides, self.d.defines.combat_damage)
    }

    /// C++ `getEspionageModifier(eOurTeam, eTargetTeam)` (see [`espionage_modifier`]).
    pub fn get_espionage_modifier(&self, our_team: TeamId, target_team: TeamId) -> i32 {
        debug_assert!(our_team != target_team);
        debug_assert!(our_team != self.barbarian_team());
        debug_assert!(target_team != self.barbarian_team());

        let target_points = self.teams[target_team].espionage_points_ever;
        let our_points = self.teams[our_team].espionage_points_ever;

        espionage_modifier(self.d.defines.espionage_spending_multiplier, target_points, our_points)
    }

    /// C++ `getDiscoveryTech(eUnit, ePlayer)`: the researchable tech whose flavors best match
    /// the unit type's (first one on ties; `None` if no tech scores above 0).
    pub fn get_discovery_tech(&self, unit: UnitType, player: PlayerId) -> Option<TechType> {
        let mut best_tech = None;
        let mut best_value = 0;
        let unit_info = &self.d.units[unit];
        let num_flavor_types = self.d.reg.count::<FlavorType>();

        for tech in self.d.techs.keys() {
            if self.player_can_research(player, tech, false) {
                let tech_info = &self.d.techs[tech];
                let mut value = 0;

                for j in 0..num_flavor_types {
                    let flavor = FlavorType::from_usize(j);
                    value += tech_info.flavor_value[flavor] * unit_info.flavor_value[flavor];
                }

                if value > best_value {
                    best_value = value;
                    best_tech = Some(tech);
                }
            }
        }

        best_tech
    }

    /// C++ `getWorldSizeMaxConscript(eCivic)` (`GC.getMapINLINE().getWorldSize()` is the
    /// init-core world size).
    pub fn get_world_size_max_conscript(&self, civic: CivicType) -> i32 {
        let mut max_conscript = self.d.civics[civic].max_conscript;

        max_conscript *= 0.max(self.d.worlds[self.setup.world_size].max_conscript_modifier + 100);
        max_conscript /= 100;

        max_conscript
    }

    // ---- PlotUnitFunc's (C++ PUF_*); signature `PlotUnitFunc` ----

    /// C++ `PUF_isGroupHead`.
    pub fn puf_is_group_head(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_is_group_head(u)
    }

    /// C++ `PUF_isPlayer` (data1 = player).
    pub fn puf_is_player(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        debug_assert!(data1 != -1, "Invalid data argument, should be >= 0");
        self.units[u].owner.i() == data1
    }

    /// C++ `PUF_isTeam` (data1 = team).
    pub fn puf_is_team(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        debug_assert!(data1 != -1, "Invalid data argument, should be >= 0");
        self.unit_get_team(u).i() == data1
    }

    /// C++ `PUF_isCombatTeam` (data1 = team, data2 = viewing team).
    pub fn puf_is_combat_team(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        debug_assert!(data1 != -1, "Invalid data argument, should be >= 0");
        let for_team = puf_data_team(data2);
        let combat_owner = self.unit_get_combat_owner(u, Some(for_team), self.unit_plot_opt(u));

        // C++ `isInvisible(eTeam, bDebug = false, bCheckCargo = false)`.
        self.players[combat_owner].team.i() == data1 && !self.unit_is_invisible(u, for_team, false)
    }

    /// C++ `PUF_isOtherPlayer` (data1 = player).
    pub fn puf_is_other_player(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        debug_assert!(data1 != -1, "Invalid data argument, should be >= 0");
        self.units[u].owner.i() != data1
    }

    /// C++ `PUF_isOtherTeam` (data1 = player).
    pub fn puf_is_other_team(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        let team = self.players[puf_data_player(data1)].team;
        if self.unit_can_coexist_with_enemy_unit(u, Some(team)) {
            return false;
        }

        self.unit_get_team(u) != team
    }

    /// C++ `PUF_isEnemy` (data1 = player, data2 = alwaysHostile flag). FfH: `isAlwaysHostile`.
    pub fn puf_is_enemy(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        debug_assert!(data2 != -1, "Invalid data argument, should be >= 0");
        let other_team = self.players[puf_data_player(data1)].team;
        let our_team = self.players[self.unit_get_combat_owner(u, Some(other_team), self.unit_plot_opt(u))].team;

        if self.unit_can_coexist_with_enemy_unit(u, Some(other_team)) {
            return false;
        }

        // FfH: Added by Kael 10/01/2007
        if other_team != our_team && self.unit_is_always_hostile(u, None) {
            return true;
        }
        // FfH: End Add

        if data2 != 0 {
            other_team != our_team
        } else {
            self.at_war(other_team, our_team)
        }
    }

    /// C++ `PUF_isVisible` (data1 = player).
    pub fn puf_is_visible(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        // C++ `isInvisible(eTeam, bDebug = false)` (default `bCheckCargo = true`).
        !self.unit_is_invisible(u, self.players[puf_data_player(data1)].team, true)
    }

    /// C++ `PUF_isVisibleDebug` (data1 = player).
    pub fn puf_is_visible_debug(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        // C++ `isInvisible(eTeam, bDebug = true)`: `bDebug` only matters in debug mode (never on
        // here; `unit_is_invisible` drops it), so this is `PUF_isVisible`.
        !self.unit_is_invisible(u, self.players[puf_data_player(data1)].team, true)
    }

    /// C++ `PUF_canSiege` (data1 = player).
    pub fn puf_can_siege(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_can_siege(u, self.players[puf_data_player(data1)].team)
    }

    /// C++ `PUF_isPotentialEnemy` (data1 = player, data2 = alwaysHostile flag).
    pub fn puf_is_potential_enemy(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        debug_assert!(data2 != -1, "Invalid data argument, should be >= 0");
        let other_team = self.players[puf_data_player(data1)].team;
        let our_team = self.players[self.unit_get_combat_owner(u, Some(other_team), self.unit_plot_opt(u))].team;

        if self.unit_can_coexist_with_enemy_unit(u, Some(other_team)) {
            return false;
        }

        if data2 != 0 {
            other_team != our_team
        } else {
            self.is_potential_enemy(other_team, Some(our_team))
        }
    }

    /// C++ `PUF_canDeclareWar` (data1 = player, data2 = alwaysHostile flag).
    pub fn puf_can_declare_war(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        debug_assert!(data2 != -1, "Invalid data argument, should be >= 0");
        let other_team = self.players[puf_data_player(data1)].team;
        let our_team = self.players[self.unit_get_combat_owner(u, Some(other_team), self.unit_plot_opt(u))].team;

        if self.unit_can_coexist_with_enemy_unit(u, Some(other_team)) {
            return false;
        }

        if data2 != 0 {
            false
        } else {
            self.team_can_declare_war(other_team, our_team)
        }
    }

    /// C++ `PUF_canDefend`.
    pub fn puf_can_defend(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_can_defend(u, None)
    }

    /// C++ `PUF_cannotDefend`.
    pub fn puf_cannot_defend(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        !self.puf_can_defend(u, data1, data2)
    }

    /// C++ `PUF_canDefendGroupHead`.
    pub fn puf_can_defend_group_head(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.puf_can_defend(u, data1, data2) && self.puf_is_group_head(u, data1, data2)
    }

    /// C++ `PUF_canDefendEnemy`.
    pub fn puf_can_defend_enemy(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.puf_can_defend(u, data1, data2) && self.puf_is_enemy(u, data1, data2)
    }

    /// C++ `PUF_canDefendPotentialEnemy`.
    pub fn puf_can_defend_potential_enemy(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.puf_can_defend(u, data1, data2) && self.puf_is_potential_enemy(u, data1, data2)
    }

    /// C++ `PUF_canAirAttack`.
    pub fn puf_can_air_attack(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_can_air_attack(u)
    }

    /// C++ `PUF_canAirDefend`.
    pub fn puf_can_air_defend(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_can_air_defend(u, None)
    }

    /// C++ `PUF_isFighting`.
    pub fn puf_is_fighting(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_is_fighting(u)
    }

    /// C++ `PUF_isAnimal`.
    pub fn puf_is_animal(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_is_animal(u)
    }

    /// C++ `PUF_isMilitaryHappiness`.
    pub fn puf_is_military_happiness(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_is_military_happiness(u)
    }

    /// C++ `PUF_isInvestigate`.
    pub fn puf_is_investigate(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_is_investigate(u)
    }

    /// C++ `PUF_isCounterSpy`.
    pub fn puf_is_counter_spy(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_is_counter_spy(u)
    }

    /// C++ `PUF_isSpy`.
    pub fn puf_is_spy(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_is_spy(u)
    }

    /// C++ `PUF_isDomainType` (data1 = domain).
    pub fn puf_is_domain_type(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        debug_assert!(data1 != -1, "Invalid data argument, should be >= 0");
        self.unit_get_domain_type(u).i() == data1
    }

    /// C++ `PUF_isUnitType` (data1 = unit type).
    pub fn puf_is_unit_type(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        debug_assert!(data1 != -1, "Invalid data argument, should be >= 0");
        self.units[u].unit_type.i() == data1
    }

    /// C++ `PUF_isUnitAIType` (data1 = unit AI type; reads the `Unit::ai_type` bookkeeping field).
    pub fn puf_is_unit_ai_type(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        debug_assert!(data1 != -1, "Invalid data argument, should be >= 0");
        opt_to_i32(self.units[u].ai_type) == data1
    }

    /// C++ `PUF_isCityAIType` (`CvUnitAI::AI_isCityAIType`).
    pub fn puf_is_city_ai_type(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_ai_is_city_ai_type(u)
    }

    /// C++ `PUF_isNotCityAIType`.
    pub fn puf_is_not_city_ai_type(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        !self.puf_is_city_ai_type(u, data1, data2)
    }

    /// C++ `PUF_isNoMission`.
    pub fn puf_is_no_mission(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        // C++ dereferences `getGroup()` (crashes for a unit without a group).
        let group = self.units[u].group.expect("PUF_isNoMission: unit without a selection group");
        self.groups[group].activity_type != ActivityType::ACTIVITY_MISSION
    }

    /// C++ `PUF_isFiniteRange`.
    pub fn puf_is_finite_range(&self, u: UnitId, data1: i32, data2: i32) -> bool {
        self.unit_get_domain_type(u) != DomainType::DOMAIN_AIR || self.d.units[self.units[u].unit_type].air_range > 0
    }
}

/// A `PlotUnitFunc` data argument naming a player (C++ `(PlayerTypes)iData`).
#[inline]
fn puf_data_player(data: i32) -> PlayerId {
    debug_assert!(data != -1, "Invalid data argument, should be >= 0");
    PlayerId::from_usize(data as usize)
}

/// A `PlotUnitFunc` data argument naming a team (C++ `(TeamTypes)iData`).
#[inline]
fn puf_data_team(data: i32) -> TeamId {
    debug_assert!(data != -1, "Invalid data argument, should be >= 0");
    TeamId::from_usize(data as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom(w: i32, h: i32, wx: bool, wy: bool) -> MapGeom {
        MapGeom::new(w, h, wx, wy)
    }

    #[test]
    fn range_and_sign() {
        assert_eq!(range(-5, 0, 10), 0);
        assert_eq!(range(15, 0, 10), 10);
        assert_eq!(range(7, 0, 10), 7);
        assert_eq!(range_f(1.5, 0.0, 1.0), 1.0);
        assert_eq!(get_sign(-3), -1);
        assert_eq!(get_sign(0), 0);
        assert_eq!(get_sign(9), 1);
    }

    #[test]
    fn coord_helpers() {
        // no wrap
        assert_eq!(coord_distance(1, 9, 10, false), 8);
        // wrap: |1-9| = 8 > 5 -> 10 - 8 = 2
        assert_eq!(coord_distance(1, 9, 10, true), 2);
        assert_eq!(coord_distance(2, 7, 10, true), 5);
        assert_eq!(wrap_coord_difference(8, 10, true), -2);
        assert_eq!(wrap_coord_difference(-8, 10, true), 2);
        assert_eq!(wrap_coord_difference(5, 10, true), 5);
        assert_eq!(wrap_coord_difference(-5, 10, true), -5);
        assert_eq!(wrap_coord_difference(8, 10, false), 8);
        assert_eq!(coord_range(-1, 10, true), 9);
        assert_eq!(coord_range(10, 10, true), 0);
        assert_eq!(coord_range(23, 10, true), 3);
        assert_eq!(coord_range(-1, 10, false), -1);
        // C++ quirk: -range maps to range (off the map)
        assert_eq!(coord_range(-10, 10, true), 10);
    }

    #[test]
    fn plot_and_step_distance() {
        let g = geom(20, 10, true, false);
        // matches the pattern in CvGameCoreUtils.h
        assert_eq!(g.plot_distance(5, 5, 5, 5), 0);
        assert_eq!(g.plot_distance(5, 5, 6, 6), 1);
        assert_eq!(g.plot_distance(5, 5, 7, 6), 2);
        assert_eq!(g.plot_distance(5, 5, 7, 7), 3);
        assert_eq!(g.plot_distance(5, 5, 8, 7), 4);
        assert_eq!(g.plot_distance(5, 5, 8, 8), 4);
        assert_eq!(g.step_distance(5, 5, 8, 7), 3);
        assert_eq!(g.step_distance(5, 5, 5, 8), 3);
        // x wraps, y does not
        assert_eq!(g.plot_distance(0, 0, 19, 0), 1);
        assert_eq!(g.plot_distance(0, 0, 0, 9), 9);
        assert_eq!(g.dx_wrap(19), -1);
        assert_eq!(g.dy_wrap(9), 9);
    }

    #[test]
    fn plot_lookup() {
        let g = geom(8, 4, true, false);
        assert_eq!(g.num_plots(), 32);
        assert_eq!(g.plot(0, 0), Some(PlotIdx(0)));
        assert_eq!(g.plot(-1, 0), Some(PlotIdx(7)));
        assert_eq!(g.plot(8, 1), Some(PlotIdx(8)));
        assert_eq!(g.plot(0, -1), None);
        assert_eq!(g.plot(0, 4), None);
        assert_eq!(g.plot(-8, 0), None); // coordRange quirk
        assert_eq!(g.plot(INVALID_PLOT_COORD, 0), None);
        assert_eq!(g.plot_coords(PlotIdx(13)), (5, 1));
        assert_eq!(g.plot_direction(0, 0, Some(DirectionType::DIRECTION_NORTH)), Some(PlotIdx(8)));
        assert_eq!(g.plot_direction(0, 0, Some(DirectionType::DIRECTION_WEST)), Some(PlotIdx(7)));
        assert_eq!(g.plot_direction(0, 0, Some(DirectionType::DIRECTION_SOUTH)), None);
        assert_eq!(g.plot_direction(3, 2, None), Some(PlotIdx(19)));
        assert_eq!(g.plot_cardinal_direction(7, 0, CardinalDirectionType::CARDINALDIRECTION_EAST), Some(PlotIdx(0)));
        assert_eq!(g.plot_xy(1, 1, 2, 1), Some(PlotIdx(19)));
        assert_eq!(g.direction_xy_plots(PlotIdx(0), PlotIdx(7)), Some(DirectionType::DIRECTION_WEST));
        assert_eq!(g.direction_xy_plots(PlotIdx(0), PlotIdx(9)), Some(DirectionType::DIRECTION_NORTHEAST));
        assert_eq!(g.direction_xy_plots(PlotIdx(0), PlotIdx(2)), None);
    }

    #[test]
    fn directions() {
        // directionXY agrees with the direction offset tables
        for d in 0..DirectionType::NUM {
            let dir = DirectionType(d as u16);
            assert_eq!(direction_xy(PLOT_DIRECTION_X[d], PLOT_DIRECTION_Y[d]), Some(dir));
            // turning right then left is the identity
            assert_eq!(TURN_LEFT_DIRECTION[TURN_RIGHT_DIRECTION[d].idx()], dir);
            assert_eq!(estimate_direction(PLOT_DIRECTION_X[d] * 5, PLOT_DIRECTION_Y[d] * 5), Some(dir));
        }
        assert_eq!(direction_xy(0, 0), None);
        assert_eq!(direction_xy(2, 0), None);
        assert_eq!(estimate_direction(0, 0), None);
        assert_eq!(estimate_direction(3, 1), Some(DirectionType::DIRECTION_EAST));
        assert_eq!(estimate_direction(2, 1), Some(DirectionType::DIRECTION_NORTHEAST));
        assert_eq!(estimate_direction(-1, -7), Some(DirectionType::DIRECTION_SOUTH));
        for c in 0..CardinalDirectionType::NUM {
            let card = CardinalDirectionType(c as u16);
            let dir = cardinal_direction_to_direction(card).unwrap();
            assert!(is_cardinal_direction(dir));
            assert_eq!(PLOT_CARDINAL_DIRECTION_X[c], PLOT_DIRECTION_X[dir.idx()]);
            assert_eq!(PLOT_CARDINAL_DIRECTION_Y[c], PLOT_DIRECTION_Y[dir.idx()]);
            let opp = get_opposite_cardinal_direction(card);
            assert_eq!(get_opposite_cardinal_direction(opp), card);
            assert_eq!(PLOT_CARDINAL_DIRECTION_X[opp.idx()], -PLOT_CARDINAL_DIRECTION_X[c]);
        }
        assert!(!is_cardinal_direction(DirectionType::DIRECTION_NORTHEAST));
        assert_eq!(cardinal_direction_to_direction(CardinalDirectionType(9)), None);
    }

    #[test]
    fn city_plots() {
        // plotCityXY inverts the city plot offset tables
        for i in 0..NUM_CITY_PLOTS {
            assert_eq!(plot_city_xy(CITY_PLOT_X[i], CITY_PLOT_Y[i]), Some(i));
            assert_eq!(plot_city_xy_radius(CITY_PLOT_X[i], CITY_PLOT_Y[i], 3), Some(i));
        }
        assert_eq!(plot_city_xy(0, 0), Some(CITY_HOME_PLOT));
        assert_eq!(plot_city_xy(3, 3), None);
        assert_eq!(plot_city_xy(4, 0), None);
        // radius-2 fat cross: the 21 BtS plots
        let mut n = 0;
        for dx in -3..=3 {
            for dy in -3..=3 {
                if plot_city_xy_radius(dx, dy, 2).is_some() {
                    n += 1;
                }
            }
        }
        assert_eq!(n, 21);
        assert_eq!(plot_city_xy_radius(2, 2, 2), None);
        assert_eq!(plot_city_xy_radius(-2, -1, 2), Some(17));
        assert_eq!(plot_city_xy_radius(0, 3, 3), Some(21));
        assert_eq!(plot_city_xy_radius(1, 0, 1), Some(3));
        assert_eq!(plot_city_xy_radius(2, 0, 1), None);
        // priorities grow with ring distance
        assert_eq!(CITY_PLOT_PRIORITY[0], 0);
        assert!(CITY_PLOT_PRIORITY[21..].iter().all(|&p| p >= 5));
    }

    #[test]
    fn score_helpers() {
        assert_eq!(get_population_asset(7), 14);
        assert_eq!(get_land_plots_asset(9), 9);
        assert_eq!(get_population_power(7), 3);
        assert_eq!(get_population_score(5), 5);
        assert_eq!(get_land_plots_score(12), 12);
        assert!(is_pickable_name("LEADER_FOO"));
        assert!(!is_pickable_name("UNIT_BAR_NOPICK"));
        assert!(!is_pickable_name("unit_bar_nopick"));
        assert!(is_pickable_name("NOPIC"));
        assert_eq!(espionage_modifier(100, 0, 0), 0);
        assert_eq!(espionage_modifier(100, 10, 10), 100);
        assert_eq!(espionage_modifier(100, 30, 0), 200);
        let t = set_trade_item(TradeableItem::TRADE_PEACE_TREATY, 3);
        assert_eq!((t.item_type, t.data, t.offering, t.hidden), (TradeableItem::TRADE_PEACE_TREATY, 3, false, false));
    }

    #[test]
    fn binomial() {
        assert_eq!(get_binomial_coefficient(15, 3), 455);
        assert_eq!(get_binomial_coefficient(15, 12), 455);
        assert_eq!(get_binomial_coefficient(10, 0), 1);
        assert_eq!(get_binomial_coefficient(10, 10), 1);
        assert_eq!(get_binomial_coefficient(5, 2), 10);
        assert_eq!(get_binomial_coefficient(30, 15), 155_117_520);
    }

    #[test]
    fn pow_int_matches_repeated_multiplication() {
        assert_eq!(pow_int_f32(0.5, 0), 1.0);
        assert_eq!(pow_int_f32(0.5, 3), 0.125);
        assert_eq!(pow_int_f32(2.0, -2), 0.25);
        assert_eq!(pow_int_f32(3.0, 5), 243.0);
    }

    #[test]
    fn combat_odds_math() {
        // Expected values from a verbatim C++ transcription of getCombatOdds (float math,
        // MSVC `_Pow_int`), COMBAT_DIE_SIDES = 1000, COMBAT_DAMAGE = 20.
        // (aS aF dS dF aHP dHP dMaxHP aLimit aFS aCFS aImmune dFS dCFS dImmune) -> odds
        #[rustfmt::skip]
        let cases: [([i32; 14], i32); 11] = [
            ([500, 500, 500, 500, 100, 100, 100, 100, 0, 0, 0, 0, 0, 0], 500),
            ([600, 600, 500, 500, 100, 100, 100, 100, 0, 0, 0, 0, 0, 0], 730),
            ([800, 800, 500, 500, 100, 100, 100, 100, 0, 0, 0, 0, 0, 0], 956),
            ([500, 500, 600, 600, 100, 100, 100, 100, 0, 0, 0, 0, 0, 0], 272),
            ([500, 500, 500, 500, 100, 100, 100, 100, 2, 0, 0, 0, 0, 0], 636),
            ([500, 500, 500, 500, 100, 100, 100, 100, 0, 1, 0, 0, 0, 0], 534),
            ([500, 500, 500, 500, 100, 100, 100, 100, 1, 0, 0, 0, 0, 0], 568),
            ([700, 650, 900, 820, 64, 100, 100, 100, 1, 2, 0, 2, 1, 0], 65),
            ([300, 300, 450, 400, 100, 35, 100, 75, 0, 0, 0, 3, 2, 0], 690),
            ([1200, 1100, 400, 450, 90, 100, 100, 100, 0, 0, 0, 0, 0, 0], 996),
            ([450, 450, 500, 500, 100, 100, 100, 100, 3, 0, 1, 2, 2, 1], 315),
        ];
        for (k, expect) in cases {
            let inp = CombatOddsInput {
                attacker_strength: k[0],
                attacker_firepower: k[1],
                defender_strength: k[2],
                defender_firepower: k[3],
                attacker_hit_points: k[4],
                defender_hit_points: k[5],
                defender_max_hit_points: k[6],
                attacker_combat_limit: k[7],
                attacker_first_strikes: k[8],
                attacker_chance_first_strikes: k[9],
                attacker_immune_to_first_strikes: k[10] != 0,
                defender_first_strikes: k[11],
                defender_chance_first_strikes: k[12],
                defender_immune_to_first_strikes: k[13] != 0,
            };
            assert_eq!(combat_odds(&inp, 1000, 20), expect, "{k:?}");
        }
        // early returns
        let one_sided = CombatOddsInput { attacker_strength: 1000, defender_strength: 0, ..Default::default() };
        assert_eq!(combat_odds(&one_sided, 1000, 20), 1000);
        let hopeless = CombatOddsInput { attacker_strength: 0, defender_strength: 1000, ..Default::default() };
        assert_eq!(combat_odds(&hopeless, 1000, 20), 0);
    }

    #[test]
    fn shuffles_are_permutations_and_deterministic() {
        let mut r1 = Rng::new(12345);
        let mut r2 = Rng::new(12345);
        let a = shuffle(37, &mut r1);
        let b = shuffle(37, &mut r2);
        assert_eq!(a, b);
        let mut sorted = a.clone();
        sorted.sort();
        assert_eq!(sorted, (0..37).collect::<Vec<_>>());
        assert_ne!(a, (0..37).collect::<Vec<_>>());
        // exactly one draw per entry, j = get(num - i) + i
        let mut r3 = Rng::new(12345);
        let mut expect: Vec<i32> = (0..37).collect();
        for i in 0..37 {
            let j = r3.get_i(37 - i, "") + i;
            expect.swap(i as usize, j as usize);
        }
        assert_eq!(a, expect);
        assert_eq!(r1, r3);
    }

    #[test]
    fn calendar_math() {
        // two increments: 10 turns of 12 months, then 20 turns of 6 months
        let inc = [(12, 10), (6, 20)];
        let ty = |t| turn_month_for_game(t, -4000, CalendarType::CALENDAR_DEFAULT, &inc, 12, 4, 4) / 12;
        assert_eq!(ty(0), -4000);
        assert_eq!(ty(5), -3995);
        assert_eq!(ty(10), -3990);
        assert_eq!(ty(12), -3989);
        assert_eq!(ty(30), -3980);
        // beyond the last increment continue with the last month increment
        assert_eq!(ty(34), -3978);
        let m = |c, t| turn_month_for_game(t, 0, c, &inc, 12, 4, 4);
        assert_eq!(m(CalendarType::CALENDAR_BI_YEARLY, 3), 72);
        assert_eq!(m(CalendarType::CALENDAR_YEARS, 3), 36);
        assert_eq!(m(CalendarType::CALENDAR_TURNS, 3), 36);
        assert_eq!(m(CalendarType::CALENDAR_SEASONS, 3), 9);
        assert_eq!(m(CalendarType::CALENDAR_MONTHS, 3), 3);
        assert_eq!(m(CalendarType::CALENDAR_WEEKS, 9), 2);
    }

    #[test]
    fn game_geometry_and_war() {
        let d = ffh_data::global();
        let mut g = Game::new(d, crate::setup::GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 1));
        g.map.grid_width = 8;
        g.map.grid_height = 4;
        g.map.wrap_x = true;
        g.map.wrap_y = false;
        assert_eq!(g.map_geom(), geom(8, 4, true, false));
        assert_eq!(g.plot_distance(0, 0, 7, 0), 1);
        assert_eq!(g.step_distance(0, 0, 0, 3), 3);

        let (a, b) = (TeamId(0), TeamId(1));
        assert!(!g.at_war(a, b));
        g.teams[a].at_war[b] = true;
        g.teams[b].at_war[a] = true;
        assert!(g.at_war(a, b) && g.at_war(b, a));
        assert!(g.at_war_opt(Some(a), Some(b)));
        assert!(!g.at_war_opt(None, Some(b)));
        assert!(!g.at_war(a, TeamId(2)));
    }

    // ---- data-only helpers (expected values read off the vendored FFH2 XML) ----

    #[test]
    fn tech_score_and_tech_requirements() {
        let d = ffh_data::global();
        let tech = |s: &str| d.t::<TechType>(s);
        let unit = |s: &str| d.t::<UnitType>(s);

        // era + 1: ancient, classical, medieval
        assert_eq!(get_tech_score(d, tech("TECH_AGRICULTURE")), 1);
        assert_eq!(get_tech_score(d, tech("TECH_WRITING")), 2);
        assert_eq!(get_tech_score(d, tech("TECH_GUILDS")), 3);

        // the seven religions' founding techs
        assert!(is_religion_tech(d, tech("TECH_ORDERS_FROM_HEAVEN")));
        assert!(is_religion_tech(d, tech("TECH_HONOR")));
        assert!(!is_religion_tech(d, tech("TECH_AGRICULTURE")));
        assert_eq!(d.techs.keys().filter(|&t| is_religion_tech(d, t)).count(), 7);
        // FFH2 has no corporations
        assert!(d.techs.keys().all(|t| !is_corporation_tech(d, t)));

        // UNIT_CHARIOT: PrereqTech TECH_TRADE + TechTypes [TECH_CONSTRUCTION]
        assert!(is_tech_required_for_unit(d, tech("TECH_TRADE"), unit("UNIT_CHARIOT")));
        assert!(is_tech_required_for_unit(d, tech("TECH_CONSTRUCTION"), unit("UNIT_CHARIOT")));
        assert!(!is_tech_required_for_unit(d, tech("TECH_BRONZE_WORKING"), unit("UNIT_CHARIOT")));
        assert!(is_tech_required_for_unit(d, tech("TECH_BRONZE_WORKING"), unit("UNIT_AXEMAN")));
        assert!(d.techs.keys().all(|t| !is_tech_required_for_unit(d, t, unit("UNIT_WARRIOR"))));

        let granary = d.t::<BuildingType>("BUILDING_GRANARY");
        assert!(is_tech_required_for_building(d, tech("TECH_AGRICULTURE"), granary));
        assert!(!is_tech_required_for_building(d, tech("TECH_MINING"), granary));
        // a temple (SPECIALBUILDING_TEMPLE has no tech prerequisite of its own)
        let temple = d.t::<BuildingType>("BUILDING_TEMPLE_OF_THE_ORDER");
        assert!(is_tech_required_for_building(d, tech("TECH_ORDERS_FROM_HEAVEN"), temple));
        assert_eq!(d.techs.keys().filter(|&t| is_tech_required_for_building(d, t, temple)).count(), 1);

        let bane = d.t::<ProjectType>("PROJECT_BANE_DIVINE");
        assert!(is_tech_required_for_project(d, tech("TECH_MALEVOLENT_DESIGNS"), bane));
        assert!(!is_tech_required_for_project(d, tech("TECH_AGRICULTURE"), bane));
        let samhain = d.t::<ProjectType>("PROJECT_SAMHAIN");
        assert!(d.techs.keys().all(|t| !is_tech_required_for_project(d, t, samhain)));
    }

    #[test]
    fn limited_classes_and_projects() {
        let d = ffh_data::global();
        let unit_class = |s: &str| d.t::<UnitClassType>(s);
        let building_class = |s: &str| d.t::<BuildingClassType>(s);

        // world unit (hero), national unit (4 per player), unlimited
        let herne = unit_class("UNITCLASS_HERNE");
        assert!(is_world_unit_class(d, herne) && !is_team_unit_class(d, herne) && !is_national_unit_class(d, herne));
        let ira = unit_class("UNITCLASS_IRA");
        assert!(!is_world_unit_class(d, ira) && is_national_unit_class(d, ira) && is_limited_unit_class(d, ira));
        assert!(!is_limited_unit_class(d, unit_class("UNITCLASS_WARRIOR")));
        assert!(d.unit_classes.keys().all(|c| !is_team_unit_class(d, c)));

        let heron_throne = building_class("BUILDINGCLASS_HERON_THRONE");
        assert!(is_world_wonder_class(d, heron_throne) && !is_national_wonder_class(d, heron_throne));
        assert_eq!(limited_wonder_class_limit(d, heron_throne), 1);
        assert_eq!(get_wonder_score(d, heron_throne), 5);
        let palace = building_class("BUILDINGCLASS_PALACE");
        assert!(!is_world_wonder_class(d, palace) && is_national_wonder_class(d, palace));
        assert_eq!(limited_wonder_class_limit(d, palace), 1);
        let granary = building_class("BUILDINGCLASS_GRANARY");
        assert!(!is_limited_wonder_class(d, granary));
        assert_eq!(limited_wonder_class_limit(d, granary), -1);
        assert_eq!(get_wonder_score(d, granary), 0);
        let count = |f: fn(&GameData, BuildingClassType) -> bool| d.building_classes.keys().filter(|&c| f(d, c)).count();
        assert_eq!((count(is_world_wonder_class), count(is_team_wonder_class), count(is_national_wonder_class)), (38, 0, 20));

        let bane = d.t::<ProjectType>("PROJECT_BANE_DIVINE");
        assert!(is_world_project(d, bane) && !is_team_project(d, bane) && is_limited_project(d, bane));
        assert!(!is_limited_project(d, d.t::<ProjectType>("PROJECT_ELEGY_OF_THE_SHEAIM")));
    }

    #[test]
    fn promotion_validity() {
        let d = ffh_data::global();
        let valid = |p: &str, u: &str| is_promotion_valid(d, d.t::<PromotionType>(p), d.t::<UnitType>(u), false);

        assert!(valid("PROMOTION_COMBAT1", "UNIT_WARRIOR"));
        assert!(valid("PROMOTION_AMPHIBIOUS", "UNIT_WARRIOR"));
        assert!(!valid("PROMOTION_BARRAGE1", "UNIT_WARRIOR")); // not for melee

        // no unit combat: only free promotions
        assert!(!valid("PROMOTION_COMBAT1", "UNIT_WORKER"));
        assert!(!valid("PROMOTION_COMBAT1", "UNIT_AIR_ELEMENTAL"));
        assert!(valid("PROMOTION_FLYING", "UNIT_AIR_ELEMENTAL"));
        // only defensive (an animal): no city attack, no blitz
        assert!(valid("PROMOTION_COMBAT1", "UNIT_ELEPHANT"));
        assert!(!valid("PROMOTION_CITY_RAIDER1", "UNIT_ELEPHANT"));
        assert!(!valid("PROMOTION_BURNING_BLOOD", "UNIT_ELEPHANT"));
        // collateral damage promotions need a collateral damage unit
        assert!(valid("PROMOTION_BARRAGE1", "UNIT_CATAPULT"));
        assert!(!valid("PROMOTION_BARRAGE1", "UNIT_BATTERING_RAM"));

        // Over all (promotion, unit) pairs; the count comes from an independent evaluation of
        // the C++ rules over the XML. FFH2 has no leader promotions, so bLeader changes nothing.
        let mut n = 0;
        for p in d.promotions.keys() {
            for u in d.units.keys() {
                let v = is_promotion_valid(d, p, u, false);
                assert_eq!(v, is_promotion_valid(d, p, u, true));
                n += v as usize;
            }
        }
        assert_eq!((n, d.promotions.len() * d.units.len()), (26312, 84825));
    }

    #[test]
    fn final_improvement_upgrades() {
        let d = ffh_data::global();
        let imp = |s: &str| d.t::<ImprovementType>(s);
        let fin = |s: &str, count| final_improvement_upgrade(d, imp(s), count);

        // cottage -> hamlet -> village -> town -> enclave
        assert_eq!(fin("IMPROVEMENT_COTTAGE", 0), Some(imp("IMPROVEMENT_ENCLAVE")));
        assert_eq!(fin("IMPROVEMENT_TOWN", 0), Some(imp("IMPROVEMENT_ENCLAVE")));
        assert_eq!(fin("IMPROVEMENT_FORT", 0), Some(imp("IMPROVEMENT_CITADEL")));
        assert_eq!(fin("IMPROVEMENT_PIRATE_COVE", 0), Some(imp("IMPROVEMENT_PIRATE_PORT")));
        assert_eq!(fin("IMPROVEMENT_FARM", 0), Some(imp("IMPROVEMENT_FARM")));
        // more than GC.getNumImprovementInfos() steps give NO_IMPROVEMENT
        let n = d.improvements.len() as i32;
        assert_eq!(n, 81);
        assert_eq!(fin("IMPROVEMENT_FARM", n), Some(imp("IMPROVEMENT_FARM")));
        assert_eq!(fin("IMPROVEMENT_FARM", n + 1), None);
        assert_eq!(fin("IMPROVEMENT_COTTAGE", n), None);
        assert_eq!(fin("IMPROVEMENT_COTTAGE", n - 4), Some(imp("IMPROVEMENT_ENCLAVE")));
        assert_eq!(fin("IMPROVEMENT_COTTAGE", n - 3), None);
    }

    #[test]
    fn plot_event_triggers() {
        let d = ffh_data::global();
        let plot_trigger = |s: &str| is_plot_event_trigger(d, d.t::<EventTriggerType>(s));

        assert!(plot_trigger("EVENTTRIGGER_VOLCANO")); // iPlotType 0 (PLOT_PEAK)
        assert!(plot_trigger("EVENTTRIGGER_ANTELOPE")); // features
        assert!(plot_trigger("EVENTTRIGGER_DUSTBOWL")); // terrains, 4 plots
        assert!(plot_trigger("EVENTTRIGGER_FARM_BANDITS")); // improvements
        assert!(plot_trigger("EVENTTRIGGER_TWO_HEADED_COW")); // bonuses
        assert!(plot_trigger("EVENTTRIGGER_WASHED_OUT")); // routes
        assert!(plot_trigger("EVENTTRIGGER_DUSTBOWL_CONT")); // prereq event plot, no picked city
        assert!(!plot_trigger("EVENTTRIGGER_ALCHEMIST")); // a plot without any plot property
        assert!(!plot_trigger("EVENTTRIGGER_AMURITE_TRIAL")); // no plot

        // count from an independent evaluation of the C++ rules over the XML
        assert_eq!(d.event_triggers.keys().filter(|&t| is_plot_event_trigger(d, t)).count(), 56);
    }

    #[test]
    fn turn_month_from_game_speed() {
        let d = ffh_data::global();
        let normal = d.t::<GameSpeedType>("GAMESPEED_NORMAL");
        let quick = d.t::<GameSpeedType>("GAMESPEED_QUICK");
        assert_eq!(d.game_speeds[normal].game_turn_info, vec![GameTurnInfo { month_increment: 12, num_game_turns_per_increment: 690 }]);

        assert_eq!(get_turn_month_for_game(d, 7, 0, CalendarType::CALENDAR_DEFAULT, normal), 84);
        assert_eq!(get_turn_year_for_game(d, 7, 0, CalendarType::CALENDAR_DEFAULT, normal), 7);
        assert_eq!(get_turn_year_for_game(d, 7, -4000, CalendarType::CALENDAR_DEFAULT, normal), -3993);
        // past the last increment (480 turns on quick) the last month increment continues
        assert_eq!(get_turn_year_for_game(d, 500, 0, CalendarType::CALENDAR_DEFAULT, quick), 500);
        assert_eq!(get_turn_month_for_game(d, 500, 0, CalendarType::CALENDAR_DEFAULT, quick), 6000);
        // 12 months, 4 seasons, WEEKS_PER_MONTHS 4
        assert_eq!(get_turn_month_for_game(d, 5, 1, CalendarType::CALENDAR_SEASONS, normal), 12 + 15);
        assert_eq!(get_turn_month_for_game(d, 9, 0, CalendarType::CALENDAR_WEEKS, normal), 2);
        assert_eq!(get_turn_month_for_game(d, 9, 0, CalendarType::CALENDAR_MONTHS, normal), 9);
        assert_eq!(get_turn_year_for_game(d, 9, 0, CalendarType::CALENDAR_BI_YEARLY, normal), 18);
    }

    // ---- helpers that read game state ----

    /// A game with real `Player` objects (not `player_init`ed) on a `w` x `h` map.
    fn test_game(w: i32, h: i32, wrap_x: bool) -> Game {
        use crate::map::{Map, MapInitData};
        use crate::player::Player;
        let d = ffh_data::global();
        let mut g = Game::new(d, crate::setup::GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 1));
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        g.map = Map::new(d, &MapInitData { grid_w: w, grid_h: h, wrap_x, ..Default::default() }, num_players, num_teams);
        for p in g.player_ids() {
            g.players[p] = Player::new(d, p, &g.setup.players[p.idx()], num_players, num_teams);
        }
        g
    }

    /// C++ `CvPlayer::addCity` + `reset`, registered on its plot (no `CvCity::init`).
    fn add_city(g: &mut Game, owner: PlayerId, x: i32, y: i32) -> CityId {
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        let id = g.cities.next_id();
        let c = g.player_add_city(owner, crate::city::City::new(g.d, num_players, num_teams, id, owner, x, y, 0));
        let p = g.map.plot_idx(x, y).unwrap();
        g.map.plots[p].plot_city = Some(c);
        c
    }

    /// C++ `CvPlayer::addUnit` + `reset`, with the coordinates set directly (no `init`/`setXY`).
    fn add_unit(g: &mut Game, owner: PlayerId, unit_type: &str, x: i32, y: i32) -> UnitId {
        use crate::unit::{Unit, UnitResetInfo};
        let d = g.d;
        let t = d.t::<UnitType>(unit_type);
        let id = g.units.next_id();
        let u = g.player_add_unit(owner, Unit::new(id, t, owner, d, &UnitResetInfo::from_info(&d.units[t])));
        g.units[u].x = x;
        g.units[u].y = y;
        u
    }

    #[test]
    fn plot_city_follows_the_city_radius() {
        let mut g = test_game(10, 8, false);
        let at = |g: &Game, i: usize| g.map.plot_idx(4 + CITY_PLOT_X[i], 4 + CITY_PLOT_Y[i]);

        // no city on (4, 4): any of the 37 indices
        assert_eq!(g.plot_city(4, 4, CITY_HOME_PLOT), g.map.plot_idx(4, 4));
        assert_eq!(g.plot_city(4, 4, 36), at(&g, 36));
        // off the (non-wrapping) map
        assert_eq!(g.plot_city(0, 0, 5), None);

        let c = add_city(&mut g, PlayerId(0), 4, 4);
        assert_eq!(g.cities[c].plot_radius, 2);
        assert_eq!(g.plot_city(4, 4, 20), at(&g, 20));
        // C++ quirk: `iIndex > getNumCityPlots()` lets index 21 (radius 3) through
        assert_eq!(g.plot_city(4, 4, 21), at(&g, 21));
        assert_eq!(g.plot_city(4, 4, 22), None);
        g.cities[c].plot_radius = 3;
        assert_eq!(g.plot_city(4, 4, 36), at(&g, 36));

        // a destroyed city (stale id on the plot) no longer limits the index
        g.cities[c].plot_radius = 2;
        g.player_delete_city(PlayerId(0), c);
        assert_eq!(g.plot_city(4, 4, 30), at(&g, 30));
    }

    #[test]
    fn plot_city_xy_for_uses_the_city_radius() {
        let mut g = test_game(10, 8, true);
        let c = add_city(&mut g, PlayerId(0), 0, 4);
        let p = |g: &Game, x, y| g.map.plot_idx(x, y).unwrap();

        assert_eq!(g.plot_city_xy_for(c, p(&g, 0, 4)), Some(CITY_HOME_PLOT));
        assert_eq!(g.plot_city_xy_for(c, p(&g, 2, 5)), plot_city_xy(2, 1));
        // x wraps: (9, 4) is one plot west of the city
        assert_eq!(g.plot_city_xy_for(c, p(&g, 9, 4)), plot_city_xy(-1, 0));
        // radius 2: no (2, 2) corners, no third ring
        assert_eq!(g.plot_city_xy_for(c, p(&g, 2, 6)), None);
        assert_eq!(g.plot_city_xy_for(c, p(&g, 3, 4)), None);
        g.cities[c].plot_radius = 3;
        assert_eq!(g.plot_city_xy_for(c, p(&g, 2, 6)), plot_city_xy(2, 2));
        assert_eq!(g.plot_city_xy_for(c, p(&g, 7, 4)), plot_city_xy(-3, 0));
        assert_eq!(g.plot_city_xy_for(c, p(&g, 4, 4)), None);
    }

    #[test]
    fn team_level_helpers() {
        let mut g = test_game(4, 4, false);
        let d = g.d;
        let (a, b) = (TeamId(0), TeamId(1));

        // espionage: ESPIONAGE_SPENDING_MULTIPLIER 100
        assert_eq!(g.get_espionage_modifier(a, b), 0);
        g.teams[b].espionage_points_ever = 30;
        assert_eq!(g.get_espionage_modifier(a, b), 200);
        assert_eq!(g.get_espionage_modifier(b, a), 50);

        // potential enemies (canDeclareWar is only asked once the sneak attack is ready)
        assert!(!g.is_potential_enemy(a, None));
        assert!(!g.is_potential_enemy(a, Some(b)));
        g.teams[a].ai_war_plan[b] = Some(WarPlanType::WARPLAN_PREPARING_TOTAL);
        assert!(!g.is_potential_enemy(a, Some(b)));
        g.teams[a].at_war[b] = true;
        g.teams[b].at_war[a] = true;
        assert!(g.is_potential_enemy(a, Some(b)) && g.is_potential_enemy(b, Some(a)));

        // CIVIC_MILITARY_STATE: iMaxConscript 3, scaled by the world size modifier
        let military_state = d.t::<CivicType>("CIVIC_MILITARY_STATE");
        for (world, expect) in [
            ("WORLDSIZE_DUEL", 1),
            ("WORLDSIZE_TINY", 2),
            ("WORLDSIZE_SMALL", 3),
            ("WORLDSIZE_STANDARD", 3),
            ("WORLDSIZE_LARGE", 4),
            ("WORLDSIZE_HUGE", 5),
        ] {
            g.setup.world_size = d.t::<WorldSizeType>(world);
            assert_eq!(g.get_world_size_max_conscript(military_state), expect, "{world}");
        }
        assert_eq!(g.get_world_size_max_conscript(d.t::<CivicType>("CIVIC_DESPOTISM")), 0);
    }

    #[test]
    fn unit_filters_and_cycle_order() {
        use crate::group::SelectionGroup;
        let mut g = test_game(6, 4, false);
        let d = g.d;
        let (p0, p1) = (PlayerId(0), PlayerId(1));
        let warrior = add_unit(&mut g, p0, "UNIT_WARRIOR", 1, 1);
        let worker = add_unit(&mut g, p0, "UNIT_WORKER", 1, 1);
        let other = add_unit(&mut g, p1, "UNIT_WARRIOR", 1, 1);

        assert!(g.puf_is_player(warrior, p0.i(), -1) && !g.puf_is_player(warrior, p1.i(), -1));
        assert!(g.puf_is_other_player(other, p0.i(), -1) && !g.puf_is_other_player(other, p1.i(), -1));
        assert!(g.puf_is_team(other, 1, -1) && !g.puf_is_team(other, 0, -1));
        assert!(g.puf_is_unit_type(warrior, d.t::<UnitType>("UNIT_WARRIOR").i(), -1));
        assert!(!g.puf_is_unit_type(worker, d.t::<UnitType>("UNIT_WARRIOR").i(), -1));

        // unit AI type (NO_UNITAI after reset)
        let city_defense = UnitAIType::UNITAI_CITY_DEFENSE;
        assert!(!g.puf_is_unit_ai_type(warrior, city_defense.i(), -1));
        assert!(!g.puf_is_city_ai_type(warrior, -1, -1) && g.puf_is_not_city_ai_type(warrior, -1, -1));
        g.units[warrior].ai_type = Some(city_defense);
        assert!(g.puf_is_unit_ai_type(warrior, city_defense.i(), -1));
        assert!(g.puf_is_city_ai_type(warrior, -1, -1) && !g.puf_is_not_city_ai_type(warrior, -1, -1));

        assert!(!g.puf_is_fighting(warrior, -1, -1));
        g.units[warrior].combat_unit = Some(other);
        assert!(g.puf_is_fighting(warrior, -1, -1));

        // group head and mission activity
        let group = g.groups.next_id();
        g.player_add_selection_group(p0, SelectionGroup::new(group, p0));
        g.groups[group].units = vec![worker, warrior];
        g.units[warrior].group = Some(group);
        g.units[worker].group = Some(group);
        assert!(g.puf_is_group_head(worker, -1, -1) && !g.puf_is_group_head(warrior, -1, -1));
        assert!(g.puf_is_no_mission(warrior, -1, -1));
        g.groups[group].activity_type = ActivityType::ACTIVITY_MISSION;
        assert!(!g.puf_is_no_mission(warrior, -1, -1));

        // different owners: the lower player id cycles first
        assert!(g.is_before_unit_cycle(warrior, other));
        assert!(!g.is_before_unit_cycle(other, worker));
    }
}
