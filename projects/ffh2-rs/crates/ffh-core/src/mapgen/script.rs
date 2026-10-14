//! Map scripts: the replacement for the Python map script module that the DLL consults
//! through `gDLL->getPythonIFace()->getMapScriptModule()` (Civ4 map scripts are Python and not
//! part of FFH2's rules; this port ships its own, implementing [`MapScript`]).
//!
//! Every hook mirrors one `callFunction(getMapScriptModule(), "<name>", ...)` site. A hook
//! returning `None` / `false` means "use the DLL default" (C++ `pythonUsingDefaultImpl()`), so
//! the ported C++ falls through to its own implementation exactly as with a script that does
//! not override the function. [`DefaultMapScript`] overrides nothing.
//!
//! Call sites clone the `Arc` first (`let ms = self.map_script.clone(); ms.hook(self, ..)`).

#![allow(unused_variables)]

use std::sync::Arc;

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

pub trait MapScript: Send + Sync {
    /// Script name (C++ `gDLL->getMapScriptName()`).
    fn name(&self) -> &str {
        "default"
    }

    // ---- CvMap::init / CvMap::reset ----

    /// `beforeInit` (CvMap::init, before the map is reset).
    fn before_init(&self, g: &mut Game) {}

    /// `getGridSize(worldSize)`: map size in cells (C++ multiplies by the landscape's plots per
    /// cell). `None` = `CvWorldInfo` grid size.
    fn get_grid_size(&self, g: &Game, world_size: WorldSizeType) -> Option<(i32, i32)> {
        None
    }

    /// `getTopLatitude`.
    fn get_top_latitude(&self, g: &Game) -> Option<i32> {
        None
    }

    /// `getBottomLatitude`.
    fn get_bottom_latitude(&self, g: &Game) -> Option<i32> {
        None
    }

    /// `getWrapX`.
    fn get_wrap_x(&self, g: &Game) -> Option<bool> {
        None
    }

    /// `getWrapY`.
    fn get_wrap_y(&self, g: &Game) -> Option<bool> {
        None
    }

    // ---- CvMapGenerator ----

    /// `beforeGeneration` (no default behaviour).
    fn before_generation(&self, g: &mut Game) {}

    /// `generateRandomMap`: true = handled by the script (skips plot types and terrain).
    fn generate_random_map(&self, g: &mut Game) -> bool {
        false
    }

    /// `generatePlotTypes`: one plot type per plot index. `None` = all `PLOT_LAND` (C++ default).
    fn generate_plot_types(&self, g: &mut Game) -> Option<Vec<PlotType>> {
        None
    }

    /// `generateTerrainTypes`: one terrain per plot index. `None` = keep the terrain set by
    /// `setPlotTypes` (C++ default).
    fn generate_terrain_types(&self, g: &mut Game) -> Option<Vec<TerrainType>> {
        None
    }

    /// `addRivers`: true = handled by the script.
    fn add_rivers(&self, g: &mut Game) -> bool {
        false
    }

    /// `addLakes`: true = handled by the script.
    fn add_lakes(&self, g: &mut Game) -> bool {
        false
    }

    /// `addFeatures`: true = handled by the script.
    fn add_features(&self, g: &mut Game) -> bool {
        false
    }

    /// `addBonuses`: true = handled by the script.
    fn add_bonuses(&self, g: &mut Game) -> bool {
        false
    }

    /// `addBonusType(eBonus)`: true = handled by the script.
    fn add_bonus_type(&self, g: &mut Game, bonus: BonusType) -> bool {
        false
    }

    /// `addGoodies`: true = handled by the script. C++ quirk: FfH's `addImprovements` and
    /// `addUniqueImprovements` also consult `addGoodies` (so the hook runs up to three times).
    fn add_goodies(&self, g: &mut Game) -> bool {
        false
    }

    /// `afterGeneration` (no default behaviour).
    fn after_generation(&self, g: &mut Game) {}

    /// `canPlaceBonusAt(plot)`: `None` = DLL rules.
    fn can_place_bonus_at(&self, g: &Game, plot: PlotIdx) -> Option<bool> {
        None
    }

    /// `canPlaceGoodyAt(plot)`: `None` = DLL rules.
    fn can_place_goody_at(&self, g: &Game, plot: PlotIdx) -> Option<bool> {
        None
    }

    /// `getRiverStartCardinalDirection(plot)`: `None` = lowest river altitude.
    fn get_river_start_cardinal_direction(&self, g: &Game, plot: PlotIdx) -> Option<CardinalDirectionType> {
        None
    }

    /// `getRiverAltitude(plot)`: `None` = DLL formula.
    fn get_river_altitude(&self, g: &Game, plot: PlotIdx) -> Option<i32> {
        None
    }

    // ---- CvGame ----

    /// `assignStartingPlots`: true = handled by the script (after the DLL's preset-plot pass).
    fn assign_starting_plots(&self, g: &mut Game) -> bool {
        false
    }

    /// `normalizeStartingPlotLocations`: true = handled by the script.
    fn normalize_starting_plot_locations(&self, g: &mut Game) -> bool {
        false
    }

    /// `normalizeAddRiver`: true = handled by the script.
    fn normalize_add_river(&self, g: &mut Game) -> bool {
        false
    }

    /// `normalizeRemovePeaks`: true = handled by the script.
    fn normalize_remove_peaks(&self, g: &mut Game) -> bool {
        false
    }

    /// `normalizeAddLakes`: true = handled by the script.
    fn normalize_add_lakes(&self, g: &mut Game) -> bool {
        false
    }

    /// `normalizeRemoveBadFeatures`: true = handled by the script.
    fn normalize_remove_bad_features(&self, g: &mut Game) -> bool {
        false
    }

    /// `normalizeRemoveBadTerrain`: true = handled by the script.
    fn normalize_remove_bad_terrain(&self, g: &mut Game) -> bool {
        false
    }

    /// `normalizeAddFoodBonuses`: true = handled by the script.
    fn normalize_add_food_bonuses(&self, g: &mut Game) -> bool {
        false
    }

    /// `normalizeAddGoodTerrain`: true = handled by the script.
    fn normalize_add_good_terrain(&self, g: &mut Game) -> bool {
        false
    }

    /// `normalizeAddExtras`: true = handled by the script.
    fn normalize_add_extras(&self, g: &mut Game) -> bool {
        false
    }

    /// `isBonusIgnoreLatitude`: `None` = false.
    fn is_bonus_ignore_latitude(&self, g: &Game) -> Option<bool> {
        None
    }

    // ---- CvPlayer ----

    /// `findStartingArea(ePlayer)`: an area id or -1 (no preference). `None` = DLL search.
    fn find_starting_area(&self, g: &Game, p: PlayerId) -> Option<i32> {
        None
    }

    /// `findStartingPlot(ePlayer)`: `None` = DLL search.
    fn find_starting_plot(&self, g: &Game, p: PlayerId) -> Option<PlotIdx> {
        None
    }

    /// `minStartingDistanceModifier`: `None` = 0.
    fn min_starting_distance_modifier(&self, g: &Game) -> Option<i32> {
        None
    }

    /// `startHumansOnSameTile`: `None` = false.
    fn start_humans_on_same_tile(&self, g: &Game) -> Option<bool> {
        None
    }
}

/// A map script that overrides nothing (every hook uses the DLL default).
#[derive(Default)]
pub struct DefaultMapScript;

impl MapScript for DefaultMapScript {}

pub type MapScriptRef = Arc<dyn MapScript>;

pub fn default_map_script() -> MapScriptRef {
    Arc::new(DefaultMapScript)
}
