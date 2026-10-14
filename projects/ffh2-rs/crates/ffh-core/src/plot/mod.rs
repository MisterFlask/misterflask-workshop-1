//! Port of `CvPlot` (CvPlot.h / CvPlot.cpp): one map tile.
//!
//! Plots live in `Game::map.plots` (`Vec<Plot>`, indexable by [`PlotIdx`]); every
//! rules method of CvPlot is an `impl Game` method `plot_*` taking the `PlotIdx`
//! first, split by topic:
//!
//! - [`terrain`]: plot type/terrain/feature/bonus/improvement/route changes, builds,
//!   goodies, turn processing (`doTurn`, `doFeature`, `doImprovement`), FFH plot
//!   counter, temporary terrain and pirate coves.
//! - [`geography`]: plot type changes and areas, adjacency, latitude, rivers,
//!   irrigation, fresh water, potential city work, yields.
//! - [`ownership`]: owner, culture, city radius, working city, plot groups and the
//!   trade network, visibility counts and revealed state.
//! - [`units`]: units on the plot, defenders, movement cost, defense, nukes, sight
//!   (line of sight and sight sources), events and training.
//!
//! Trivial C++ getters/setters are struct fields (see `docs/porting/map.md` for the
//! full C++ -> Rust table). Graphics/UI members (symbols, flag entities, plot
//! builder, center unit, layout/flag dirty bits, feature art variety) are not ported.

#![allow(unused_imports)]

pub mod geography;
pub mod ownership;
pub mod terrain;
pub mod units;

use std::ops::{Index, IndexMut};

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

/// C++ `ConstPlotUnitFunc`: a `PUF_*` predicate (CvGameCoreUtils) called as
/// `func(unit, iData1, iData2)` by `CvPlot::plotCount` / `CvPlot::plotCheck`.
pub type ConstPlotUnitFunc = fn(&Game, UnitId, i32, i32) -> bool;

/// C++ `PlotUnitFunc`: a mutating unit callback for `CvPlot::plotAction`.
pub type PlotUnitFunc = fn(&mut Game, UnitId, i32, i32) -> bool;

/// C++ `CvPlot`.
///
/// C++ allocates most per-player/per-team arrays lazily (`NULL` reads as 0 / NO_X);
/// here they are allocated up front with the same neutral values, which the C++
/// getters return for a `NULL` array.
#[derive(Clone, Debug)]
pub struct Plot {
    /// C++ `m_iX` (short).
    pub x: i32,
    /// C++ `m_iY` (short).
    pub y: i32,
    /// C++ `m_iArea` (`FFreeList::INVALID_INDEX` = None). C++ `area()` / `getArea()` read this
    /// (the `m_pPlotArea` pointer cache is not ported).
    pub area: Option<AreaId>,
    /// C++ `m_iOwnershipDuration` (short). Setter with side effects: `plot_set_ownership_duration`.
    pub ownership_duration: i32,
    /// C++ `m_iImprovementDuration` (short).
    pub improvement_duration: i32,
    /// C++ `m_iUpgradeProgress` (short).
    pub upgrade_progress: i32,
    /// C++ `m_iForceUnownedTimer` (short).
    pub force_unowned_timer: i32,
    /// C++ `m_iCityRadiusCount` (short).
    pub city_radius_count: i32,
    /// C++ `m_iRiverID` (-1 = none).
    pub river_id: i32,
    /// C++ `m_iMinOriginalStartDist` (short, -1 = none).
    pub min_original_start_dist: i32,
    /// C++ `m_iReconCount` (short).
    pub recon_count: i32,
    /// C++ `m_iRiverCrossingCount` (short).
    pub river_crossing_count: i32,

    /// C++ `m_bStartingPlot`.
    pub starting_plot: bool,
    /// C++ `m_bHills`: saved but never read in 0.41 (`isHills()` tests `plot_type`).
    pub hills: bool,
    /// C++ `m_bNOfRiver`. Setter with side effects: `plot_set_n_of_river`.
    pub n_of_river: bool,
    /// C++ `m_bWOfRiver`. Setter with side effects: `plot_set_w_of_river`.
    pub w_of_river: bool,
    /// C++ `m_bIrrigated`. Setter with side effects: `plot_set_irrigated`.
    pub irrigated: bool,
    /// C++ `m_bPotentialCityWork`.
    pub potential_city_work: bool,

    /// C++ `m_eOwner` (char). Setter: `plot_set_owner`.
    pub owner: Option<PlayerId>,
    /// C++ `m_ePlotType` (short). Setter: `plot_set_plot_type`.
    pub plot_type: PlotType,
    /// C++ `m_eTerrainType` (short). Setter: `plot_set_terrain_type`.
    pub terrain_type: Option<TerrainType>,
    /// C++ `m_eFeatureType` (short). Setter: `plot_set_feature_type`.
    pub feature_type: Option<FeatureType>,
    /// C++ `m_eBonusType` (short): `getBonusType(NO_TEAM)`. Setter: `plot_set_bonus_type`.
    pub bonus_type: Option<BonusType>,
    /// C++ `m_eImprovementType` (short). Setter: `plot_set_improvement_type`.
    pub improvement_type: Option<ImprovementType>,
    /// C++ `m_eRouteType` (short). Setter: `plot_set_route_type`.
    pub route_type: Option<RouteType>,
    /// C++ `m_eRiverNSDirection` (char).
    pub river_ns_direction: Option<CardinalDirectionType>,
    /// C++ `m_eRiverWEDirection` (char).
    pub river_we_direction: Option<CardinalDirectionType>,

    /// C++ `m_plotCity` (`getPlotCity()`). Setter: `plot_set_plot_city`.
    pub plot_city: Option<CityId>,
    /// C++ `m_workingCity` (`getWorkingCity()`). Updated by `plot_update_working_city`.
    pub working_city: Option<CityId>,
    /// C++ `m_workingCityOverride`. Setter: `plot_set_working_city_override`.
    pub working_city_override: Option<CityId>,

    /// C++ `m_aiYield` (short[NUM_YIELD_TYPES]); `yield` is a Rust keyword.
    pub yields: [i32; YieldType::NUM],
    /// C++ `m_aiCulture` (lazy, int[MAX_PLAYERS]). Setter: `plot_set_culture`.
    pub culture: IdxVec<PlayerId, i32>,
    /// C++ `m_aiFoundValue` (lazy, short[MAX_PLAYERS]; -1 = needs recomputation). AI cache:
    /// read through `plot_get_found_value`. C++ stores it in a `short` (values truncate).
    pub found_value: IdxVec<PlayerId, i32>,
    /// C++ `m_aiPlayerCityRadiusCount` (lazy, char[MAX_PLAYERS]).
    pub player_city_radius_count: IdxVec<PlayerId, i32>,
    /// C++ `m_aiPlotGroup` (lazy, plot group id per player). Read with `plot_get_plot_group`
    /// (drops ids of deleted groups, like C++); set with `plot_set_plot_group`.
    pub plot_group: IdxVec<PlayerId, Option<PlotGroupId>>,
    /// C++ `m_aiVisibilityCount` (lazy, short[MAX_TEAMS]). Change: `plot_change_visibility_count`.
    pub visibility_count: IdxVec<TeamId, i32>,
    /// C++ `m_aiStolenVisibilityCount` (lazy, short[MAX_TEAMS]).
    pub stolen_visibility_count: IdxVec<TeamId, i32>,
    /// C++ `m_aiBlockadedCount` (lazy, short[MAX_TEAMS]).
    pub blockaded_count: IdxVec<TeamId, i32>,
    /// C++ `m_aiRevealedOwner` (lazy, char[MAX_TEAMS], -1 = NO_PLAYER).
    pub revealed_owner: IdxVec<TeamId, Option<PlayerId>>,
    /// C++ `m_abRiverCrossing` (lazy, bool[NUM_DIRECTION_TYPES]).
    pub river_crossing: [bool; DirectionType::NUM],
    /// C++ `m_abRevealed` (lazy, bool[MAX_TEAMS]). Setter: `plot_set_revealed`.
    pub revealed: IdxVec<TeamId, bool>,
    /// C++ `m_aeRevealedImprovementType` (lazy, short[MAX_TEAMS]).
    pub revealed_improvement_type: IdxVec<TeamId, Option<ImprovementType>>,
    /// C++ `m_aeRevealedRouteType` (lazy, short[MAX_TEAMS]).
    pub revealed_route_type: IdxVec<TeamId, Option<RouteType>>,
    /// C++ `m_szScriptData` (Python script data).
    pub script_data: String,
    /// C++ `m_paiBuildProgress` (lazy, short[GC.getNumBuildInfos()]).
    pub build_progress: IdxVec<BuildType, i32>,
    /// C++ `m_apaiCultureRangeCities` (lazy, char[MAX_PLAYERS][GC.getNumCultureLevelInfos()]).
    pub culture_range_cities: IdxVec<PlayerId, IdxVec<CultureLevelType, i32>>,
    /// C++ `m_apaiInvisibleVisibilityCount` (lazy, short[MAX_TEAMS][GC.getNumInvisibleInfos()]).
    pub invisible_visibility_count: IdxVec<TeamId, IdxVec<InvisibleType, i32>>,
    /// C++ `m_units` (`CLinkList<IDInfo>`), in C++ list order. Add with `plot_add_unit`
    /// (keeps the unit-cycle order), remove with `plot_remove_unit`.
    pub units: Vec<UnitId>,

    // ---- FFH2 additions (Kael) ----
    /// C++ `m_bMoveDisabledAI`.
    pub move_disabled_ai: bool,
    /// C++ `m_bMoveDisabledHuman`.
    pub move_disabled_human: bool,
    /// C++ `m_bBuildDisabled`.
    pub build_disabled: bool,
    /// C++ `m_bFoundDisabled`.
    pub found_disabled: bool,
    /// C++ `m_bPythonActive` (defaults to true).
    pub python_active: bool,
    /// C++ `m_iMinLevel`.
    pub min_level: i32,
    /// C++ `m_iPlotCounter` (0..=100, hell terrain spread). Change: `plot_change_plot_counter`.
    pub plot_counter: i32,
    /// C++ `m_iPortalExitX`.
    pub portal_exit_x: i32,
    /// C++ `m_iPortalExitY`.
    pub portal_exit_y: i32,
    /// C++ `m_iTempTerrainTimer` (`changeTempTerrainTimer` is a plain `+=`).
    pub temp_terrain_timer: i32,
    /// C++ `m_eRealTerrainType` (short): terrain to restore when the temp terrain expires.
    pub real_terrain_type: Option<TerrainType>,
}

impl Plot {
    /// C++ `CvPlot::init(iX, iY)` (= `reset(iX, iY)`), arrays sized from the game data and the
    /// game's player/team counts.
    pub fn new(d: &GameData, x: i32, y: i32, num_players: usize, num_teams: usize) -> Plot {
        Plot::with_counts(x, y, num_players, num_teams, d.builds.len(), d.culture_levels.len(), d.invisibles.len())
    }

    /// C++ `CvPlot::reset` (as `reset(iX, iY)`) with explicit array sizes.
    pub fn with_counts(
        x: i32,
        y: i32,
        num_players: usize,
        num_teams: usize,
        num_builds: usize,
        num_culture_levels: usize,
        num_invisibles: usize,
    ) -> Plot {
        Plot {
            x,
            y,
            area: None,
            ownership_duration: 0,
            improvement_duration: 0,
            upgrade_progress: 0,
            force_unowned_timer: 0,
            city_radius_count: 0,
            river_id: -1,
            min_original_start_dist: -1,
            recon_count: 0,
            river_crossing_count: 0,
            starting_plot: false,
            hills: false,
            n_of_river: false,
            w_of_river: false,
            irrigated: false,
            potential_city_work: false,
            owner: None,
            plot_type: PlotType::PLOT_OCEAN,
            terrain_type: None,
            feature_type: None,
            bonus_type: None,
            improvement_type: None,
            route_type: None,
            river_ns_direction: None,
            river_we_direction: None,
            plot_city: None,
            working_city: None,
            working_city_override: None,
            yields: [0; YieldType::NUM],
            culture: IdxVec::from_elem(0, num_players),
            found_value: IdxVec::from_elem(0, num_players),
            player_city_radius_count: IdxVec::from_elem(0, num_players),
            plot_group: IdxVec::from_elem(None, num_players),
            visibility_count: IdxVec::from_elem(0, num_teams),
            stolen_visibility_count: IdxVec::from_elem(0, num_teams),
            blockaded_count: IdxVec::from_elem(0, num_teams),
            revealed_owner: IdxVec::from_elem(None, num_teams),
            river_crossing: [false; DirectionType::NUM],
            revealed: IdxVec::from_elem(false, num_teams),
            revealed_improvement_type: IdxVec::from_elem(None, num_teams),
            revealed_route_type: IdxVec::from_elem(None, num_teams),
            script_data: String::new(),
            build_progress: IdxVec::from_elem(0, num_builds),
            culture_range_cities: IdxVec::from_fn(num_players, |_| IdxVec::from_elem(0, num_culture_levels)),
            invisible_visibility_count: IdxVec::from_fn(num_teams, |_| IdxVec::from_elem(0, num_invisibles)),
            units: Vec::new(),
            move_disabled_ai: false,
            move_disabled_human: false,
            build_disabled: false,
            found_disabled: false,
            python_active: true,
            min_level: 0,
            plot_counter: 0,
            portal_exit_x: 0,
            portal_exit_y: 0,
            temp_terrain_timer: 0,
            real_terrain_type: None,
        }
    }
}

impl Default for Plot {
    /// C++ `CvPlot()` (`reset(0, 0, true)`: no per-player/per-team arrays).
    fn default() -> Self {
        Plot::with_counts(0, 0, 0, 0, 0, 0, 0)
    }
}

/// `self.map.plots[p]` with a [`PlotIdx`].
impl Index<PlotIdx> for Vec<Plot> {
    type Output = Plot;
    #[inline]
    #[track_caller]
    fn index(&self, p: PlotIdx) -> &Plot {
        &self[p.idx()]
    }
}

impl IndexMut<PlotIdx> for Vec<Plot> {
    #[inline]
    #[track_caller]
    fn index_mut(&mut self, p: PlotIdx) -> &mut Plot {
        &mut self[p.idx()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_defaults() {
        let p = Plot::with_counts(3, 4, 2, 2, 5, 7, 3);
        assert_eq!((p.x, p.y), (3, 4));
        assert_eq!(p.plot_type, PlotType::PLOT_OCEAN);
        assert_eq!(p.river_id, -1);
        assert_eq!(p.min_original_start_dist, -1);
        assert!(p.python_active);
        assert_eq!(p.culture_range_cities.len(), 2);
        assert_eq!(p.culture_range_cities[PlayerId(1)].len(), 7);
        assert_eq!(p.invisible_visibility_count[TeamId(0)].len(), 3);
        assert_eq!(p.build_progress.len(), 5);
        assert_eq!(p.revealed_owner[TeamId(1)], None);
    }

    #[test]
    fn index_by_plot_idx() {
        let mut v: Vec<Plot> = (0..3).map(|i| Plot::with_counts(i, 0, 0, 0, 0, 0, 0)).collect();
        v[PlotIdx(2)].recon_count += 1;
        assert_eq!(v[PlotIdx(2)].recon_count, 1);
        assert_eq!(v[1].x, 1);
    }
}
