//! Map generation: port of CvMapGenerator.cpp (as `impl Game` methods with the `mapgen_`
//! prefix; the C++ class is a stateless singleton) and CvFractal.cpp ([`fractal::Fractal`]),
//! plus the map script hook trait ([`script::MapScript`]) that replaces the Python map script
//! module. Map scripts themselves (Civ4's are Python, not FFH2 rules) are this port's own
//! implementations of `MapScript`.

#![allow(unused_variables)]
#![allow(unused_imports)]

pub mod fractal;
pub mod script;

pub use fractal::Fractal;
pub use script::{default_map_script, DefaultMapScript, MapScript, MapScriptRef};

use crate::game::Game;
use crate::ids::*;
use ffh_data::*;

impl Game {
    /// C++ `CvMapGenerator::canPlaceBonusAt(eBonus, iX, iY, bIgnoreLatitude)`.
    pub fn mapgen_can_place_bonus_at(&self, bonus: BonusType, x: i32, y: i32, ignore_latitude: bool) -> bool {
        todo!("CvMapGenerator::canPlaceBonusAt")
    }

    /// C++ `CvMapGenerator::canPlaceGoodyAt(eImprovement, iX, iY)`.
    pub fn mapgen_can_place_goody_at(&self, improvement: ImprovementType, x: i32, y: i32) -> bool {
        todo!("CvMapGenerator::canPlaceGoodyAt")
    }

    /// C++ `CvMapGenerator::addGameElements`: rivers, lakes, features, bonuses, goodies, FfH
    /// unique improvements and lairs, then the script's `afterGeneration`.
    pub fn mapgen_add_game_elements(&mut self) {
        self.mapgen_add_rivers();
        self.mapgen_add_lakes();
        self.mapgen_add_features();
        self.mapgen_add_bonuses();
        self.mapgen_add_goodies();

        // FfH: Added by Kael 10/04/2008
        if !self.is_option(GameOptionType::GAMEOPTION_NO_UNIQUE_IMPROVEMENTS) {
            self.mapgen_add_unique_improvements();
        }
        if !self.is_option(GameOptionType::GAMEOPTION_NO_LAIRS) {
            self.mapgen_add_improvements();
        }

        // Call for Python to make map modifications after it's been generated
        self.mapgen_after_generation();
    }

    /// C++ `CvMapGenerator::addLakes` (map rand: `LAKE_PLOT_RAND` per inland, river-less land plot).
    pub fn mapgen_add_lakes(&mut self) {
        todo!("CvMapGenerator::addLakes")
    }

    /// C++ `CvMapGenerator::addRivers` (four passes over the plots).
    pub fn mapgen_add_rivers(&mut self) {
        todo!("CvMapGenerator::addRivers")
    }

    /// C++ `CvMapGenerator::doRiver(pStartPlot, eLastCardinalDirection, eOriginalCardinalDirection,
    /// iThisRiverID)`: grows a river from the SE corner of `start` (river id -1 = new river).
    pub fn mapgen_do_river(
        &mut self,
        start: PlotIdx,
        last_cardinal_direction: Option<CardinalDirectionType>,
        original_cardinal_direction: Option<CardinalDirectionType>,
        this_river_id: i32,
    ) {
        todo!("CvMapGenerator::doRiver")
    }

    /// C++ `CvMapGenerator::addRiver(pFreshWaterPlot)`: tries to give `plot` fresh water.
    pub fn mapgen_add_river(&mut self, fresh_water_plot: PlotIdx) -> bool {
        todo!("CvMapGenerator::addRiver")
    }

    /// C++ `CvMapGenerator::addFeatures`.
    pub fn mapgen_add_features(&mut self) {
        todo!("CvMapGenerator::addFeatures")
    }

    /// C++ `CvMapGenerator::addBonuses` (by `iPlacementOrder`, unique / non-unique types).
    pub fn mapgen_add_bonuses(&mut self) {
        todo!("CvMapGenerator::addBonuses")
    }

    /// C++ `CvMapGenerator::addUniqueBonusType(eBonusType)`.
    pub fn mapgen_add_unique_bonus_type(&mut self, bonus: BonusType) {
        todo!("CvMapGenerator::addUniqueBonusType")
    }

    /// C++ `CvMapGenerator::addNonUniqueBonusType(eBonusType)`.
    pub fn mapgen_add_non_unique_bonus_type(&mut self, bonus: BonusType) {
        todo!("CvMapGenerator::addNonUniqueBonusType")
    }

    /// C++ `CvMapGenerator::addGoodies`.
    pub fn mapgen_add_goodies(&mut self) {
        todo!("CvMapGenerator::addGoodies")
    }

    /// C++ `CvMapGenerator::addImprovements` (FfH lairs; uses the *soren* rand).
    pub fn mapgen_add_improvements(&mut self) {
        todo!("CvMapGenerator::addImprovements")
    }

    /// C++ `CvMapGenerator::addUniqueImprovements` (FfH unique features; uses the *soren* rand).
    pub fn mapgen_add_unique_improvements(&mut self) {
        todo!("CvMapGenerator::addUniqueImprovements")
    }

    /// C++ `CvMapGenerator::eraseRivers`.
    pub fn mapgen_erase_rivers(&mut self) {
        todo!("CvMapGenerator::eraseRivers")
    }

    /// C++ `CvMapGenerator::eraseFeatures`.
    pub fn mapgen_erase_features(&mut self) {
        todo!("CvMapGenerator::eraseFeatures")
    }

    /// C++ `CvMapGenerator::eraseBonuses`.
    pub fn mapgen_erase_bonuses(&mut self) {
        todo!("CvMapGenerator::eraseBonuses")
    }

    /// C++ `CvMapGenerator::eraseGoodies`.
    pub fn mapgen_erase_goodies(&mut self) {
        todo!("CvMapGenerator::eraseGoodies")
    }

    /// C++ `CvMapGenerator::generateRandomMap`.
    pub fn mapgen_generate_random_map(&mut self) {
        let ms = self.map_script.clone();
        ms.before_generation(self);

        if ms.generate_random_map(self) {
            return; // Python override
        }

        self.mapgen_generate_plot_types();
        self.mapgen_generate_terrain();
    }

    /// C++ `CvMapGenerator::generatePlotTypes`: the script's plot types, or all land.
    pub fn mapgen_generate_plot_types(&mut self) {
        let num_plots = self.map_geom().num_plots() as usize;
        let ms = self.map_script.clone();
        let plot_types = match ms.generate_plot_types(self) {
            Some(v) => {
                // Python override
                debug_assert_eq!(v.len(), num_plots, "python generatePlotTypes() should return list with length numPlotsINLINE");
                v
            }
            None => vec![PlotType::PLOT_LAND; num_plots],
        };
        self.mapgen_set_plot_types(&plot_types);
    }

    /// C++ `CvMapGenerator::generateTerrain`: applies the script's terrain types (no default).
    pub fn mapgen_generate_terrain(&mut self) {
        todo!("CvMapGenerator::generateTerrain")
    }

    /// C++ `CvMapGenerator::afterGeneration`.
    pub fn mapgen_after_generation(&mut self) {
        let ms = self.map_script.clone();
        ms.after_generation(self);
    }

    /// C++ `CvMapGenerator::setPlotTypes(paiPlotTypes)`: sets plot types, recalculates areas,
    /// then shallow/deep water terrain by land adjacency.
    pub fn mapgen_set_plot_types(&mut self, plot_types: &[PlotType]) {
        todo!("CvMapGenerator::setPlotTypes")
    }

    /// C++ `CvMapGenerator::getRiverValueAtPlot(pPlot)`. Note: C++ seeds a local `CvRandom`
    /// with `(x * 43251267) + (y * 8273903)` (int overflow wraps) for the final `get(10)`.
    pub fn mapgen_get_river_value_at_plot(&self, plot: PlotIdx) -> i32 {
        todo!("CvMapGenerator::getRiverValueAtPlot")
    }

    /// C++ `CvMapGenerator::calculateNumBonusesToAdd(eBonusType)` (map rand x4).
    pub fn mapgen_calculate_num_bonuses_to_add(&mut self, bonus: BonusType) -> i32 {
        todo!("CvMapGenerator::calculateNumBonusesToAdd")
    }
}
