//! Map generation: port of CvMapGenerator.cpp (as `impl Game` methods with the `mapgen_`
//! prefix; the C++ class is a stateless singleton) and CvFractal.cpp ([`fractal::Fractal`]),
//! plus the map script hook trait ([`script::MapScript`]) that replaces the Python map script
//! module. Map scripts themselves (Civ4's are Python, not FFH2 rules) are this port's own
//! implementations of `MapScript`.

pub mod fractal;
pub mod script;

pub use fractal::Fractal;
pub use script::{default_map_script, DefaultMapScript, MapScript, MapScriptRef};

use std::cmp::max;

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::rng::Rng;
use crate::utils::*;
use ffh_data::*;

impl Game {
    /// C++ `CvMapGenerator::canPlaceBonusAt(eBonus, iX, iY, bIgnoreLatitude)`.
    pub fn mapgen_can_place_bonus_at(&self, bonus: BonusType, x: i32, y: i32, ignore_latitude: bool) -> bool {
        let d = self.d;
        let plot = self.map.plot_xy(x, y).expect("canPlaceBonusAt: plot off the map");
        let area = self.map.plots[plot].area;

        if !self.plot_can_have_bonus(plot, Some(bonus), ignore_latitude) {
            return false;
        }

        let ms = self.map_script.clone();
        if let Some(result) = ms.can_place_bonus_at(self, plot) {
            // Python override
            return result;
        }

        for i in 0..DirectionType::NUM {
            if let Some(loop_plot) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                let other = self.map.plots[loop_plot].bonus_type;
                if other.is_some() && other != Some(bonus) {
                    return false;
                }
            }
        }

        let info = &d.bonuses[bonus];

        if self.plot_is_water(plot) {
            if (self.map.num_bonus_on_land[bonus] * 100) / (self.map.num_bonus[bonus] + 1) < info.min_land_percent {
                return false;
            }
        }

        // Make sure there are no bonuses of the same class (but a different type) nearby:

        // (C++ indexes the bonus class info; every FFH2 bonus has a class)
        let range = info.bonus_class_type.map_or(0, |c| d.bonus_classes[c].unique_range);

        for dx in -range..=range {
            for dy in -range..=range {
                if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                    if self.map.plots[loop_plot].area == area {
                        let (lx, ly) = self.map.xy(loop_plot);
                        if self.plot_distance(x, y, lx, ly) <= range {
                            if let Some(other_bonus) = self.map.plots[loop_plot].bonus_type {
                                if d.bonuses[other_bonus].bonus_class_type == info.bonus_class_type {
                                    return false;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Make sure there are none of the same bonus nearby:

        let range = info.unique_range;

        for dx in -range..=range {
            for dy in -range..=range {
                if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                    if self.map.plots[loop_plot].area == area {
                        let (lx, ly) = self.map.xy(loop_plot);
                        if self.plot_distance(x, y, lx, ly) <= range && self.map.plots[loop_plot].bonus_type == Some(bonus) {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    /// C++ `CvMapGenerator::canPlaceGoodyAt(eImprovement, iX, iY)`.
    pub fn mapgen_can_place_goody_at(&self, improvement: ImprovementType, x: i32, y: i32) -> bool {
        let d = self.d;
        debug_assert!(d.improvements[improvement].goody, "ImprovementType eImprovement is expected to be a goody");

        if self.is_option(GameOptionType::GAMEOPTION_NO_GOODY_HUTS) {
            return false;
        }

        let plot = self.map.plot_xy(x, y).expect("canPlaceGoodyAt: plot off the map");

        if !self.plot_can_have_improvement(plot, improvement, None, false) {
            return false;
        }

        let ms = self.map_script.clone();
        if let Some(result) = ms.can_place_goody_at(self, plot) {
            // Python override
            return result;
        }

        if self.map.plots[plot].improvement_type.is_some() {
            return false;
        }

        if self.map.plots[plot].bonus_type.is_some() {
            return false;
        }

        if self.plot_is_impassable(plot) {
            return false;
        }

        let unique_range = d.improvements[improvement].goody_unique_range;
        for dx in -unique_range..=unique_range {
            for dy in -unique_range..=unique_range {
                if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                    if self.map.plots[loop_plot].improvement_type == Some(improvement) {
                        return false;
                    }
                }
            }
        }

        true
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
        let ms = self.map_script.clone();
        if ms.add_lakes(self) {
            return;
        }

        let lake_plot_rand = self.d.defines.lake_plot_rand;

        for plot in self.map.plot_ids() {
            if !self.plot_is_water(plot)
                && !self.plot_is_coastal_land(plot, -1)
                && !self.plot_is_river(plot)
                && self.map_rand_num(lake_plot_rand, "addLakes") == 0
            {
                self.plot_set_plot_type(plot, PlotType::PLOT_OCEAN, true);
            }
        }
    }

    /// C++ `CvMapGenerator::addRivers` (four passes over the plots).
    pub fn mapgen_add_rivers(&mut self) {
        let ms = self.map_script.clone();
        if ms.add_rivers(self) {
            return;
        }

        let defines = &self.d.defines;
        let plots_per_river_edge = defines.plots_per_river_edge;

        for pass in 0..4 {
            let river_source_range = if pass <= 1 {
                defines.river_source_min_river_range
            } else {
                defines.river_source_min_river_range / 2
            };

            let sea_water_range = if pass <= 1 {
                defines.river_source_min_seawater_range
            } else {
                defines.river_source_min_seawater_range / 2
            };

            for plot in self.map.plot_ids() {
                if self.plot_is_water(plot) {
                    continue;
                }
                let few_river_edges = |g: &Game| {
                    let area = &g.map.areas[g.map.plots[plot].area.expect("land plot without area")];
                    area.num_river_edges < (area.num_tiles / plots_per_river_edge) + 1
                };
                let hills_or_peak = self.plot_is_hills(plot) || self.plot_is_peak(plot);

                if (pass == 0 && hills_or_peak)
                    || (pass == 1 && !self.plot_is_coastal_land(plot, -1) && self.map_rand_num(8, "addRivers") == 0)
                    || (pass == 2 && hills_or_peak && few_river_edges(self))
                    || (pass == 3 && few_river_edges(self))
                {
                    if !self.map_find_water(plot, river_source_range, true) && !self.map_find_water(plot, sea_water_range, false) {
                        if let Some(start_plot) = self.plot_get_inland_corner(plot) {
                            self.mapgen_do_river(start_plot, None, None, -1);
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvMapGenerator::doRiver(pStartPlot, eLastCardinalDirection, eOriginalCardinalDirection,
    /// iThisRiverID)`: grows a river from the SE corner of `start` (river id -1 = new river).
    ///
    /// The C++ recursion is a tail call (its last statement); it is iterated here with the
    /// same arguments, which also keeps long rivers off the stack.
    pub fn mapgen_do_river(
        &mut self,
        start: PlotIdx,
        last_cardinal_direction: Option<CardinalDirectionType>,
        original_cardinal_direction: Option<CardinalDirectionType>,
        this_river_id: i32,
    ) {
        const NORTH: CardinalDirectionType = CardinalDirectionType::CARDINALDIRECTION_NORTH;
        const EAST: CardinalDirectionType = CardinalDirectionType::CARDINALDIRECTION_EAST;
        const SOUTH: CardinalDirectionType = CardinalDirectionType::CARDINALDIRECTION_SOUTH;
        const WEST: CardinalDirectionType = CardinalDirectionType::CARDINALDIRECTION_WEST;

        let mut start_plot = start;
        let mut last_cardinal_direction = last_cardinal_direction;
        let mut original_cardinal_direction = original_cardinal_direction;
        let mut this_river_id = this_river_id;

        loop {
            if this_river_id == -1 {
                this_river_id = self.map.next_river_id;
                self.map.next_river_id += 1;
            }

            let other_river_id = self.map.plots[start_plot].river_id;
            if other_river_id != -1 && other_river_id != this_river_id {
                return; // Another river already exists here; can't branch off of an existing river!
            }

            let river_plot: Option<PlotIdx>;
            let mut best_cardinal_direction: Option<CardinalDirectionType> = None;
            let (sx, sy) = self.map.xy(start_plot);

            match last_cardinal_direction {
                Some(NORTH) => {
                    let rp = start_plot;
                    let adjacent = self.plot_cardinal_direction(sx, sy, EAST);
                    match adjacent {
                        Some(adj) if !self.map.plots[rp].w_of_river && !self.plot_is_water(rp) && !self.plot_is_water(adj) => {}
                        _ => return,
                    }

                    self.map.plots[start_plot].river_id = this_river_id;
                    self.plot_set_w_of_river(rp, true, last_cardinal_direction);
                    river_plot = self.plot_cardinal_direction(sx, sy, NORTH);
                }
                Some(EAST) => {
                    let Some(rp) = self.plot_cardinal_direction(sx, sy, EAST) else {
                        return;
                    };
                    let (rx, ry) = self.map.xy(rp);
                    let adjacent = self.plot_cardinal_direction(rx, ry, SOUTH);
                    match adjacent {
                        Some(adj) if !self.map.plots[rp].n_of_river && !self.plot_is_water(rp) && !self.plot_is_water(adj) => {}
                        _ => return,
                    }

                    self.map.plots[start_plot].river_id = this_river_id;
                    self.plot_set_n_of_river(rp, true, last_cardinal_direction);
                    river_plot = Some(rp);
                }
                Some(SOUTH) => {
                    let Some(rp) = self.plot_cardinal_direction(sx, sy, SOUTH) else {
                        return;
                    };
                    let (rx, ry) = self.map.xy(rp);
                    let adjacent = self.plot_cardinal_direction(rx, ry, EAST);
                    match adjacent {
                        Some(adj) if !self.map.plots[rp].w_of_river && !self.plot_is_water(rp) && !self.plot_is_water(adj) => {}
                        _ => return,
                    }

                    self.map.plots[start_plot].river_id = this_river_id;
                    self.plot_set_w_of_river(rp, true, last_cardinal_direction);
                    river_plot = Some(rp);
                }
                Some(WEST) => {
                    let rp = start_plot;
                    let adjacent = self.plot_cardinal_direction(sx, sy, SOUTH);
                    match adjacent {
                        Some(adj) if !self.map.plots[rp].n_of_river && !self.plot_is_water(rp) && !self.plot_is_water(adj) => {}
                        _ => return,
                    }

                    self.map.plots[start_plot].river_id = this_river_id;
                    self.plot_set_n_of_river(rp, true, last_cardinal_direction);
                    river_plot = self.plot_cardinal_direction(sx, sy, WEST);
                }
                _ => {
                    // River is starting here, set the direction in the next step
                    river_plot = Some(start_plot);

                    let ms = self.map_script.clone();
                    if let Some(dir) = ms.get_river_start_cardinal_direction(self, start_plot) {
                        // Python override
                        best_cardinal_direction = Some(dir);
                    }
                }
            }

            let Some(river_plot) = river_plot else {
                return; // The river has flowed off the edge of the map. All is well.
            };
            if self.plot_has_coast_at_se_corner(river_plot) {
                return; // The river has flowed into the ocean. All is well.
            }

            if best_cardinal_direction.is_none() {
                let mut best_value = MAX_INT;
                let (rx, ry) = self.map.xy(river_plot);

                for i in 0..CardinalDirectionType::NUM {
                    let dir = CardinalDirectionType::from_usize(i);
                    let opposite = Some(get_opposite_cardinal_direction(dir));
                    if opposite != original_cardinal_direction && opposite != last_cardinal_direction {
                        if let Some(adjacent) = self.plot_cardinal_direction(rx, ry, dir) {
                            let value = self.mapgen_get_river_value_at_plot(adjacent);
                            if value < best_value {
                                best_value = value;
                                best_cardinal_direction = Some(dir);
                            }
                        }
                    }
                }
            }

            let Some(best) = best_cardinal_direction else {
                return;
            };
            if original_cardinal_direction.is_none() {
                original_cardinal_direction = Some(best);
            }
            // doRiver(pRiverPlot, eBestCardinalDirection, eOriginalCardinalDirection, iThisRiverID)
            start_plot = river_plot;
            last_cardinal_direction = Some(best);
        }
    }

    /// C++ `CvMapGenerator::addRiver(pFreshWaterPlot)`: tries to give `plot` fresh water.
    pub fn mapgen_add_river(&mut self, fresh_water_plot: PlotIdx) -> bool {
        // cannot have a river flow next to water
        if self.plot_is_water(fresh_water_plot) {
            return false;
        }

        // if it already has a fresh water river, then success! we done
        if self.plot_is_river(fresh_water_plot) {
            return true;
        }

        let mut success = false;

        // randomize the order of directions
        let mut shuffle_order = [0i32; CardinalDirectionType::NUM];
        shuffle_array(&mut shuffle_order, &mut self.map_rand);

        // make two passes, once for each flow direction of the river
        let nw_flow_pass = self.map_rand_num(2, "addRiver");
        let (fx, fy) = self.map.xy(fresh_water_plot);
        let mut pass = 0;
        while !success && pass <= 1 {
            // try placing a river edge in each direction, in random order
            let mut i = 0;
            while !success && i < CardinalDirectionType::NUM {
                let nw_pass = pass == nw_flow_pass;
                let (river_plot, river_direction) = match CardinalDirectionType::from_usize(shuffle_order[i] as usize) {
                    CardinalDirectionType::CARDINALDIRECTION_NORTH => {
                        if nw_pass {
                            (self.plot_direction(fx, fy, DirectionType::DIRECTION_NORTH), CardinalDirectionType::CARDINALDIRECTION_WEST)
                        } else {
                            (self.plot_direction(fx, fy, DirectionType::DIRECTION_NORTHWEST), CardinalDirectionType::CARDINALDIRECTION_EAST)
                        }
                    }
                    CardinalDirectionType::CARDINALDIRECTION_EAST => {
                        if nw_pass {
                            (Some(fresh_water_plot), CardinalDirectionType::CARDINALDIRECTION_NORTH)
                        } else {
                            (self.plot_direction(fx, fy, DirectionType::DIRECTION_NORTH), CardinalDirectionType::CARDINALDIRECTION_SOUTH)
                        }
                    }
                    CardinalDirectionType::CARDINALDIRECTION_SOUTH => {
                        if nw_pass {
                            (Some(fresh_water_plot), CardinalDirectionType::CARDINALDIRECTION_WEST)
                        } else {
                            (self.plot_direction(fx, fy, DirectionType::DIRECTION_WEST), CardinalDirectionType::CARDINALDIRECTION_EAST)
                        }
                    }
                    CardinalDirectionType::CARDINALDIRECTION_WEST => {
                        if nw_pass {
                            (self.plot_direction(fx, fy, DirectionType::DIRECTION_WEST), CardinalDirectionType::CARDINALDIRECTION_NORTH)
                        } else {
                            (self.plot_direction(fx, fy, DirectionType::DIRECTION_NORTHWEST), CardinalDirectionType::CARDINALDIRECTION_SOUTH)
                        }
                    }
                    _ => unreachable!("invalid cardinal direction"),
                };

                if let Some(rp) = river_plot {
                    if !self.plot_has_coast_at_se_corner(rp) {
                        // try to make the river
                        self.mapgen_do_river(rp, Some(river_direction), Some(river_direction), -1);

                        // if it succeeded, then we will be a river now!
                        if self.plot_is_river(fresh_water_plot) {
                            success = true;
                        }
                    }
                }
                i += 1;
            }
            pass += 1;
        }

        success
    }

    /// C++ `CvMapGenerator::addFeatures`.
    pub fn mapgen_add_features(&mut self) {
        let ms = self.map_script.clone();
        if ms.add_features(self) {
            return;
        }

        let d = self.d;
        for plot in self.map.plot_ids() {
            for f in d.features.keys() {
                if self.plot_can_have_feature(plot, Some(f))
                    && self.map_rand_num(10000, "addFeaturesAtPlot") < d.features[f].appearance_probability
                {
                    self.plot_set_feature_type(plot, Some(f));
                }
            }
        }
    }

    /// C++ `CvMapGenerator::addBonuses` (by `iPlacementOrder`, unique / non-unique types).
    pub fn mapgen_add_bonuses(&mut self) {
        let ms = self.map_script.clone();
        if ms.add_bonuses(self) {
            return; // Python override
        }

        let d = self.d;
        for order in 0..d.bonuses.len() as i32 {
            for b in d.bonuses.keys() {
                if d.bonuses[b].placement_order == order && !ms.add_bonus_type(self, b) {
                    if d.bonuses[b].one_area {
                        self.mapgen_add_unique_bonus_type(b);
                    } else {
                        self.mapgen_add_non_unique_bonus_type(b);
                    }
                }
            }
        }
    }

    /// C++ `CvMapGenerator::addUniqueBonusType(eBonusType)`.
    pub fn mapgen_add_unique_bonus_type(&mut self, bonus: BonusType) {
        let d = self.d;
        // C++ `piAreaTried` (FFreeList::INVALID_INDEX = None), sized by the area count.
        let mut area_tried: Vec<Option<AreaId>> = vec![None; self.map.areas.len()];

        let info = &d.bonuses[bonus];

        let bonus_count = self.mapgen_calculate_num_bonuses_to_add(bonus);

        let ignore_latitude = self.python_is_bonus_ignore_latitudes();

        debug_assert!(info.one_area, "addUniqueBonusType called with non-unique bonus type");

        loop {
            let mut best_value = 0;
            let mut best_area: Option<AreaId> = None;

            for (area_id, area) in self.map.areas.iter() {
                let tried = area_tried.contains(&Some(area_id));

                if !tried {
                    // number of unique bonuses starting on the area, plus this one
                    let num_unique_bonuses_on_area = self.area_count_num_unique_bonus_types(area_id) + 1;
                    let num_tiles = area.num_tiles;
                    let value = num_tiles / num_unique_bonuses_on_area;

                    if value > best_value {
                        best_value = value;
                        best_area = Some(area_id);
                    }
                }
            }

            let Some(best_area) = best_area else {
                break; // can't place bonus on any area
            };

            if let Some(slot) = area_tried.iter_mut().find(|slot| slot.is_none()) {
                *slot = Some(best_area);
            }

            // Place the bonuses:

            let shuffled = shuffle(self.map.num_plots() as i32, &mut self.map_rand);

            for &index in &shuffled {
                let plot = PlotIdx(index as u32);

                if self.map.num_bonus[bonus] >= bonus_count {
                    break; // We already have enough
                }

                if self.map.plots[plot].area != Some(best_area) {
                    continue;
                }
                let (px, py) = self.map.xy(plot);
                if !self.mapgen_can_place_bonus_at(bonus, px, py, ignore_latitude) {
                    continue;
                }
                self.plot_set_bonus_type(plot, Some(bonus));

                let group_range = info.group_range;
                for dx in -group_range..=group_range {
                    for dy in -group_range..=group_range {
                        if self.map.num_bonus[bonus] < bonus_count {
                            if let Some(loop_plot) = self.plot_xy(px, py, dx, dy) {
                                if self.map.plots[loop_plot].area == Some(best_area) {
                                    let (lx, ly) = self.map.xy(loop_plot);
                                    if self.mapgen_can_place_bonus_at(bonus, lx, ly, ignore_latitude)
                                        && self.map_rand_num(100, "addUniqueBonusType") < info.group_rand
                                    {
                                        self.plot_set_bonus_type(loop_plot, Some(bonus));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// C++ `CvMapGenerator::addNonUniqueBonusType(eBonusType)`.
    pub fn mapgen_add_non_unique_bonus_type(&mut self, bonus: BonusType) {
        let d = self.d;
        let mut bonus_count = self.mapgen_calculate_num_bonuses_to_add(bonus);

        if bonus_count == 0 {
            return;
        }

        let shuffled = shuffle(self.map.num_plots() as i32, &mut self.map_rand);

        let info = &d.bonuses[bonus];

        let ignore_latitude = self.python_is_bonus_ignore_latitudes();

        for &index in &shuffled {
            let plot = PlotIdx(index as u32);
            let (px, py) = self.map.xy(plot);
            if !self.mapgen_can_place_bonus_at(bonus, px, py, ignore_latitude) {
                continue;
            }
            self.plot_set_bonus_type(plot, Some(bonus));
            bonus_count -= 1;

            let group_range = info.group_range;
            for dx in -group_range..=group_range {
                for dy in -group_range..=group_range {
                    if bonus_count > 0 {
                        if let Some(loop_plot) = self.plot_xy(px, py, dx, dy) {
                            let (lx, ly) = self.map.xy(loop_plot);
                            if self.mapgen_can_place_bonus_at(bonus, lx, ly, ignore_latitude)
                                && self.map_rand_num(100, "addNonUniqueBonusType") < info.group_rand
                            {
                                self.plot_set_bonus_type(loop_plot, Some(bonus));
                                bonus_count -= 1;
                            }
                        }
                    }
                }
            }

            debug_assert!(bonus_count >= 0, "iBonusCount must be >= 0");

            if bonus_count == 0 {
                break;
            }
        }
    }

    /// C++ `CvMapGenerator::addGoodies`.
    pub fn mapgen_add_goodies(&mut self) {
        let ms = self.map_script.clone();
        if ms.add_goodies(self) {
            return; // Python override
        }

        let d = self.d;
        if d.eras[self.start_era()].no_goodies {
            return;
        }

        let num_plots = self.map.num_plots() as i32;
        let shuffled = shuffle(num_plots, &mut self.map_rand);

        for imp in d.improvements.keys() {
            let info = &d.improvements[imp];
            if !(info.goody && info.tiles_per_goody > 0) {
                continue;
            }
            for &index in &shuffled {
                let plot = PlotIdx(index as u32);
                if self.plot_is_water(plot) {
                    continue;
                }
                let area_id = self.map.plots[plot].area.expect("pArea is expected not to be NULL");
                let area = &self.map.areas[area_id];
                if area.num_improvements[imp] < (area.num_tiles + (info.tiles_per_goody / 2)) / info.tiles_per_goody {
                    let (px, py) = self.map.xy(plot);
                    if self.mapgen_can_place_goody_at(imp, px, py) {
                        self.plot_set_improvement_type(plot, Some(imp));
                    }
                }
            }
        }
    }

    /// C++ `CvMapGenerator::addImprovements` (FfH lairs; uses the *soren* rand).
    pub fn mapgen_add_improvements(&mut self) {
        let ms = self.map_script.clone();
        // C++ quirk: FfH consults the map script's `addGoodies` here (see `MapScript::add_goodies`).
        if ms.add_goodies(self) {
            return;
        }

        let d = self.d;
        for plot in self.map.plot_ids() {
            if self.map.plots[plot].bonus_type.is_some() || self.map.plots[plot].improvement_type.is_some() {
                continue;
            }
            // (C++ declares bValid outside the loop, but always resets it before reading it)
            let mut valid = true;
            let (x, y) = self.map.xy(plot);
            for j in 0..DirectionType::NUM {
                if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(j)) {
                    if self.map.plots[adjacent].improvement_type.is_some() {
                        valid = false;
                    }
                    if self.map.plots[adjacent].plot_city.is_some() {
                        valid = false;
                    }
                }
            }
            if self.map.plots[plot].plot_city.is_some() {
                valid = false;
            }
            if !self.map.plots[plot].units.is_empty() {
                valid = false;
            }
            if valid {
                for imp in d.improvements.keys() {
                    if self.plot_can_have_improvement(plot, imp, None, false)
                        && self.soren_rand_num(10000, "Spawn Improvement") < d.improvements[imp].appearance_probability
                    {
                        self.plot_set_improvement_type(plot, Some(imp));
                        self.plot_set_feature_type(plot, None);
                    }
                }
            }
        }
    }

    /// C++ `CvMapGenerator::addUniqueImprovements` (FfH unique features; uses the *soren* rand).
    pub fn mapgen_add_unique_improvements(&mut self) {
        let ms = self.map_script.clone();
        // C++ quirk: FfH consults the map script's `addGoodies` here (see `MapScript::add_goodies`).
        if ms.add_goodies(self) {
            return;
        }

        let d = self.d;
        // C++ quirk: bValid lives across plots (and improvements) and is only reset on plots
        // without bonus and improvement; on other plots the stale value is read, but those
        // plots fail the improvement/bonus checks guarded by it anyway.
        let mut valid = true;
        let chance = d.defines.improvement_unique_chance + d.worlds[self.setup.world_size].unique_feature_chance;
        for imp in d.improvements.keys() {
            if !d.improvements[imp].unique {
                continue;
            }
            let mut best_value = 0;
            let mut best_plot: Option<PlotIdx> = None;
            if self.is_option(GameOptionType::GAMEOPTION_ALL_UNIQUE_IMPROVEMENTS)
                || self.soren_rand_num(100, "Unique Improvements") < chance
            {
                for plot in self.map.plot_ids() {
                    if self.map.plots[plot].bonus_type.is_none() && self.map.plots[plot].improvement_type.is_none() {
                        valid = true;
                        let (x, y) = self.map.xy(plot);
                        for j in 0..DirectionType::NUM {
                            if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(j)) {
                                if self.map.plots[adjacent].improvement_type.is_some() {
                                    valid = false;
                                }
                            }
                        }
                    }
                    if valid
                        && self.map.plots[plot].improvement_type.is_none()
                        && self.plot_can_have_improvement(plot, imp, None, false)
                        && self.map.plots[plot].bonus_type.is_none()
                    {
                        let value = self.soren_rand_num(10000, "Unique Improvements");
                        if value > best_value {
                            best_value = value;
                            best_plot = Some(plot);
                        }
                    }
                }
                if let Some(best_plot) = best_plot {
                    self.plot_set_improvement_type(best_plot, Some(imp));
                    self.plot_set_feature_type(best_plot, None);
                }
            }
        }
    }

    /// C++ `CvMapGenerator::eraseRivers`.
    pub fn mapgen_erase_rivers(&mut self) {
        for plot in self.map.plot_ids() {
            if self.map.plots[plot].n_of_river {
                self.plot_set_n_of_river(plot, false, None);
            }
            if self.map.plots[plot].w_of_river {
                self.plot_set_w_of_river(plot, false, None);
            }
        }
    }

    /// C++ `CvMapGenerator::eraseFeatures`.
    pub fn mapgen_erase_features(&mut self) {
        for plot in self.map.plot_ids() {
            self.plot_set_feature_type(plot, None);
        }
    }

    /// C++ `CvMapGenerator::eraseBonuses`.
    pub fn mapgen_erase_bonuses(&mut self) {
        for plot in self.map.plot_ids() {
            self.plot_set_bonus_type(plot, None);
        }
    }

    /// C++ `CvMapGenerator::eraseGoodies`.
    pub fn mapgen_erase_goodies(&mut self) {
        for plot in self.map.plot_ids() {
            if self.plot_is_goody(plot, None) {
                self.plot_remove_goody(plot);
            }
        }
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
        let ms = self.map_script.clone();
        if let Some(terrain_map) = ms.generate_terrain_types(self) {
            // Python override
            debug_assert_eq!(
                terrain_map.len(),
                self.map.num_plots(),
                "python generateTerrain() should return a list with length CyMap().getNumPoints()"
            );
            // Generate terrain for each plot
            for plot in self.map.plot_ids() {
                self.plot_set_terrain_type(plot, terrain_map[plot.idx()], false);
            }
        }
    }

    /// C++ `CvMapGenerator::afterGeneration`.
    pub fn mapgen_after_generation(&mut self) {
        let ms = self.map_script.clone();
        ms.after_generation(self);
    }

    /// C++ `CvMapGenerator::setPlotTypes(paiPlotTypes)`: sets plot types, recalculates areas,
    /// then shallow/deep water terrain by land adjacency.
    pub fn mapgen_set_plot_types(&mut self, plot_types: &[PlotType]) {
        for plot in self.map.plot_ids() {
            self.plot_set_plot_type(plot, plot_types[plot.idx()], false);
        }

        self.map_recalculate_areas();

        let defines = &self.d.defines;
        let shallow_water: TerrainType = opt_from_i32(defines.shallow_water_terrain).expect("SHALLOW_WATER_TERRAIN");
        let deep_water: TerrainType = opt_from_i32(defines.deep_water_terrain).expect("DEEP_WATER_TERRAIN");

        for plot in self.map.plot_ids() {
            if self.plot_is_water(plot) {
                if self.plot_is_adjacent_to_land(plot) {
                    self.plot_set_terrain_type(plot, shallow_water, false);
                } else {
                    self.plot_set_terrain_type(plot, deep_water, false);
                }
            }
        }
    }

    /// C++ `CvMapGenerator::getRiverValueAtPlot(pPlot)`. Note: C++ seeds a local `CvRandom`
    /// with `(x * 43251267) + (y * 8273903)` (int overflow wraps) for the final `get(10)`.
    pub fn mapgen_get_river_value_at_plot(&self, plot: PlotIdx) -> i32 {
        let ms = self.map_script.clone();
        let result = ms.get_river_altitude(self, plot);
        if let Some(altitude) = result {
            // Python override
            if altitude >= 0 {
                return altitude;
            }
            debug_assert!(false, "python getRiverAltitude() must return >= 0");
        }

        // C++ `iSum = result`: 0 unless the script returned a negative altitude.
        let mut sum = result.unwrap_or(0);

        let num_plot_types = PlotType::NUM as i32;
        sum += (num_plot_types - self.map.plots[plot].plot_type.i()) * 20;

        let (x, y) = self.map.xy(plot);
        for i in 0..DirectionType::NUM {
            match self.plot_direction(x, y, DirectionType::from_usize(i)) {
                Some(adjacent) => sum += num_plot_types - self.map.plots[adjacent].plot_type.i(),
                None => sum += num_plot_types * 10,
            }
        }

        let mut river_rand = Rng::new(0);
        river_rand.reset(x.wrapping_mul(43251267).wrapping_add(y.wrapping_mul(8273903)) as u32);

        sum += river_rand.get(10, "River Rand") as i32;

        sum
    }

    /// C++ `CvMapGenerator::calculateNumBonusesToAdd(eBonusType)` (map rand x4).
    pub fn mapgen_calculate_num_bonuses_to_add(&mut self, bonus: BonusType) -> i32 {
        let d = self.d;
        let info = &d.bonuses[bonus];

        // Calculate iBonusCount, the amount of this bonus to be placed:

        let rand1 = self.map_rand_num(info.rand_appearance1, "calculateNumBonusesToAdd-1");
        let rand2 = self.map_rand_num(info.rand_appearance2, "calculateNumBonusesToAdd-2");
        let rand3 = self.map_rand_num(info.rand_appearance3, "calculateNumBonusesToAdd-3");
        let rand4 = self.map_rand_num(info.rand_appearance4, "calculateNumBonusesToAdd-4");
        let base_count = info.const_appearance + rand1 + rand2 + rand3 + rand4;

        let ignore_latitude = self.python_is_bonus_ignore_latitudes();

        // Calculate iNumPossible, the number of plots that are eligible to have this bonus:

        let mut land_tiles = 0;
        if info.tiles_per > 0 {
            let mut num_possible = 0;
            for plot in self.map.plot_ids() {
                if self.plot_can_have_bonus(plot, Some(bonus), ignore_latitude) {
                    num_possible += 1;
                }
            }
            land_tiles += num_possible / info.tiles_per;
        }

        let players = (self.count_civ_players_alive() * info.percent_per_player) / 100;
        let mut bonus_count = (base_count * (land_tiles + players)) / 100;
        bonus_count = max(1, bonus_count);

        // FfH: Added by Kael 09/29/2070
        if self.is_option(GameOptionType::GAMEOPTION_DOUBLE_BONUSES) {
            bonus_count *= 2;
        }
        // FfH: End Add

        bonus_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, MapInitData};
    use crate::setup::GameSetup;

    fn game_with_map(w: i32, h: i32, wrap_x: bool) -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 7));
        let init = MapInitData { grid_w: w, grid_h: h, wrap_x, ..MapInitData::default() };
        let (np, nt) = (g.num_players(), g.num_teams());
        g.map = Map::new(d, &init, np, nt);
        g
    }

    /// The C++ formula, written out independently.
    fn river_value_reference(g: &Game, plot: PlotIdx) -> i32 {
        let pt = |p: PlotIdx| g.map.plots[p].plot_type.0 as i32;
        let (x, y) = g.map.xy(plot);
        let mut sum = (4 - pt(plot)) * 20;
        for (dx, dy) in PLOT_DIRECTION_X.iter().zip(PLOT_DIRECTION_Y.iter()) {
            sum += match g.map.plot_xy(x + dx, y + dy) {
                Some(adj) => 4 - pt(adj),
                None => 40,
            };
        }
        // CvRandom::init((x * 43251267) + (y * 8273903)) then get(10): one LCG step.
        let seed = (x as i64 * 43251267 + y as i64 * 8273903) as i32 as u32;
        let s = seed.wrapping_mul(1103515245).wrapping_add(12345);
        sum + ((((s >> 16) & 0xFFFF) * 10) / 65536) as i32
    }

    #[test]
    fn river_value_matches_cpp_formula() {
        let mut g = game_with_map(70, 50, true);
        // A few land types so the plot-type terms differ.
        for (i, p) in g.map.plot_ids().enumerate() {
            g.map.plots[p].plot_type = PlotType::from_usize(i % PlotType::NUM);
        }
        for p in g.map.plot_ids() {
            assert_eq!(g.mapgen_get_river_value_at_plot(p), river_value_reference(&g, p), "plot {:?}", g.map.xy(p));
        }
        // The seed products overflow i32 on this map (wrapping multiplication).
        assert!(69i64 * 43251267 > i32::MAX as i64);
    }

    #[test]
    fn no_goody_huts_option_blocks_goodies() {
        let d = ffh_data::global();
        let mut g = game_with_map(4, 4, false);
        g.setup.options[GameOptionType::GAMEOPTION_NO_GOODY_HUTS] = true;
        let goody = d.improvements.keys().find(|&i| d.improvements[i].goody).expect("a goody improvement");
        assert!(!g.mapgen_can_place_goody_at(goody, 1, 1));
    }

    #[test]
    fn every_bonus_has_a_class() {
        // canPlaceBonusAt indexes the bonus class unconditionally in C++.
        let d = ffh_data::global();
        assert!(d.bonuses.iter().all(|b| b.bonus_class_type.is_some()));
    }
}
