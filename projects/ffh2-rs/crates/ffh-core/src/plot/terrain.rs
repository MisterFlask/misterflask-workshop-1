//! CvPlot, topic "terrain": terrain/feature/bonus/improvement/route changes and validity,
//! builds, goodies, turn processing (`doTurn`, `doFeature`, `doImprovement`), FFH plot
//! counter and temporary terrain, pirate coves.
//!
//! (`setPlotType` lives in `geography.rs` with the area code it drives.)
//!
//! Porting notes:
//! - C++ indexes info arrays with whatever `getTerrainType()` / `getImprovementType()` holds.
//!   Where that can be `NO_X` (undefined behaviour in C++, never reached with FFH2 data or
//!   after map generation) the missing type counts as "no match" or the step is skipped;
//!   each place says so.
//! - C++ `area()` is `Plot::area` filtered to live areas (`live_area`):
//!   `CvMap::getArea` returns NULL for an unknown id.
//! - C++ `getPlotCity()` / `getWorkingCity()` (`getCity(IDInfo)`) return NULL for a deleted
//!   city; the ids are read with the same filter.
//! - `addMessage` calls go to the game log. The `findCity` lookups that only name a city in
//!   those messages have no side effects, so they run only when the log is enabled.

use crate::defs::{MAX_INT, NUM_CITY_PLOTS};
use crate::game::Game;
use crate::ids::*;
use crate::utils::final_improvement_upgrade;
use ffh_data::*;

impl Game {
    // ------------------------------------------------------------------
    // Turn processing
    // ------------------------------------------------------------------

    /// C++ `CvPlot::doTurn`: timers, improvement duration/upgrade, FFH lair spawns and
    /// feature upgrades, `doFeature`, `doCulture`, `verifyUnitValidPlot`, FFH temp terrain expiry.
    pub fn plot_do_turn(&mut self, p: PlotIdx) {
        let d = self.d;

        if self.map.plots[p].force_unowned_timer > 0 {
            // changeForceUnownedTimer(-1)
            self.map.plots[p].force_unowned_timer -= 1;
        }

        if self.plot_is_owned(p) {
            self.plot_change_ownership_duration(p, 1);
        }

        if let Some(improvement) = self.map.plots[p].improvement_type {
            // changeImprovementDuration(1)
            self.map.plots[p].improvement_duration += 1;

            //FfH Improvements: Added by Kael 08/07/2007
            if let Some(upgrade) = d.improvements[improvement].improvement_upgrade {
                if !self.plot_is_being_worked(p) && d.improvements[upgrade].outside_borders {
                    self.plot_do_improvement_upgrade(p);
                }
            }

            // C++ re-reads getImprovementType() below (doImprovementUpgrade may have upgraded
            // it). It never becomes NO_IMPROVEMENT there with FFH2 data; C++ would then read
            // GC.getImprovementInfo(NO_IMPROVEMENT) (undefined), here the step is skipped.
            if let Some(improvement) = self.map.plots[p].improvement_type {
                if let Some(unit) = d.improvements[improvement].spawn_unit_type {
                    if !self.is_option(GameOptionType::GAMEOPTION_NO_BARBARIANS) {
                        // C++ dereferences GC.getMapINLINE().getArea(getArea()) unchecked.
                        if let Some(area) = live_area(self, p) {
                            let unowned_tiles = self.area_get_num_unowned_tiles(area);
                            if unowned_tiles > 0 {
                                let mut tiles = d.defines.tiles_per_spawn;
                                if d.units[unit].animal {
                                    tiles *= 2;
                                }
                                let barbarian = self.barbarian_player();
                                let barbarian_units = self.map.areas[area].units_per_player[barbarian];
                                if barbarian_units == 0 || (unowned_tiles / barbarian_units) > tiles {
                                    let mut chance = d.handicaps[self.handicap].lair_spawn_rate;
                                    chance *= 10000;
                                    chance /= d.game_speeds[self.game_speed_type()].train_percent;
                                    if self.soren_rand_num(10000, "Spawn Unit") < chance {
                                        if !self.plot_is_visible_other_unit(p, barbarian) {
                                            let (x, y) = self.map.xy(p);
                                            let new_unit = self.player_init_unit(
                                                barbarian,
                                                unit,
                                                x,
                                                y,
                                                Some(UnitAIType::UNITAI_ATTACK),
                                                None,
                                            );
                                            if self.units.contains(new_unit) && self.unit_is_animal(new_unit) {
                                                if let Some(promotion) = opt_from_i32(d.defines.hidden_nationality_promotion) {
                                                    self.unit_set_has_promotion(new_unit, promotion, true);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(improvement) = self.map.plots[p].improvement_type {
                if let Some(feature_upgrade) = d.improvements[improvement].feature_upgrade {
                    if self.soren_rand_num(100, "Feature Upgrade") < d.defines.feature_upgrade_chance {
                        self.plot_set_feature_type(p, Some(feature_upgrade));
                        self.plot_set_improvement_type(p, None);
                    }
                }
            }
            //FfH: End Add
        }

        self.plot_do_feature(p);

        self.plot_do_culture(p);

        self.plot_verify_unit_valid_plot(p);

        //FfH: Added by Kael 11/02/2007
        if self.map.plots[p].temp_terrain_timer > 0 {
            // changeTempTerrainTimer(-1)
            self.map.plots[p].temp_terrain_timer -= 1;
            if self.map.plots[p].temp_terrain_timer == 0 {
                // setTerrainType(getRealTerrainType(), true, true). The real terrain is always
                // set while the timer runs (setTempTerrainType); C++ setTerrainType(NO_TERRAIN)
                // would be undefined, so a missing real terrain leaves the terrain alone.
                if let Some(real) = self.map.plots[p].real_terrain_type {
                    self.plot_set_terrain_type(p, real, true);
                }
                self.map.plots[p].real_terrain_type = None;
            }
        }
        //FfH: End Add
    }

    /// C++ `CvPlot::doImprovement` (worked plots): bonus discovery, then `doImprovementUpgrade`.
    pub fn plot_do_improvement(&mut self, p: PlotIdx) {
        let d = self.d;

        debug_assert!(self.plot_is_being_worked(p) && self.plot_is_owned(p));

        if let Some(improvement) = self.map.plots[p].improvement_type {
            if self.map.plots[p].bonus_type.is_none() {
                // C++ GET_TEAM(getTeam()): the plot is owned (asserted above); an unowned plot
                // would index GET_TEAM(NO_TEAM).
                if let Some(team) = self.plot_get_team(p) {
                    for bonus in d.bonuses.keys() {
                        if self.team_is_has_tech(team, d.bonuses[bonus].tech_reveal) {
                            let discover_rand = d.improvements[improvement].improvement_bonus[bonus].discover_rand;
                            if discover_rand > 0 {
                                //FfH Mana Effects: Modified by Kael 08/21/2007 (the discover
                                // chance gets the owner's modifier; BtS rolled discover_rand)
                                let mut chance = discover_rand;
                                if let Some(owner) = self.map.plots[p].owner {
                                    let modifier = self.players[owner].discover_rand_modifier;
                                    if 100 + modifier != 0 {
                                        chance *= 100;
                                        chance /= 100 + modifier;
                                    }
                                }

                                // UNOFFICIAL_PATCH 10/22/09 jdog5000: game speed scaling
                                chance *= d.game_speeds[self.game_speed_type()].research_percent;
                                chance /= 100;

                                // C++ quirk (kept): a chance scaled down to 0 always succeeds
                                // (getSorenRandNum(0) returns 0).
                                if self.soren_rand_num(chance, "Bonus Discovery") == 0 {
                                    self.plot_set_bonus_type(p, Some(bonus));

                                    // TXT_KEY_MISC_DISCOVERED_NEW_RESOURCE (UI message)
                                    log_owner_city_message(self, p, |city| {
                                        format!("Discovered {} near {}", d.text.name(&d.bonuses[bonus].base.description), city)
                                    });

                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        self.plot_do_improvement_upgrade(p);
    }

    /// C++ `CvPlot::doImprovementUpgrade` (protected).
    pub fn plot_do_improvement_upgrade(&mut self, p: PlotIdx) {
        let d = self.d;

        let Some(improvement) = self.map.plots[p].improvement_type else {
            return;
        };
        let Some(upgrade) = d.improvements[improvement].improvement_upgrade else {
            return;
        };

        if self.plot_is_being_worked(p) || d.improvements[upgrade].outside_borders {
            //FfH: Modified by Kael 05/12/2008
            if let Some(owner) = self.map.plots[p].owner {
                let civilization = self.player_get_civilization_type(owner);
                let upgrade_civilization = d.improvements[upgrade].prereq_civilization;
                if upgrade_civilization.is_none() || upgrade_civilization == civilization {
                    // changeUpgradeProgress(getImprovementUpgradeRate())
                    let rate = self.player_get_improvement_upgrade_rate(owner);
                    self.map.plots[p].upgrade_progress += rate;
                }
                let current = self.map.plots[p].improvement_type.unwrap_or(improvement);
                let current_civilization = d.improvements[current].prereq_civilization;
                if current_civilization.is_some() && current_civilization != civilization {
                    self.plot_set_improvement_type(p, d.improvements[current].improvement_pillage);
                }
            } else if d.improvements[upgrade].prereq_civilization.is_none() {
                // changeUpgradeProgress(1)
                self.map.plots[p].upgrade_progress += 1;
            }
            //FfH: End Modify

            // BUGFIX (Sephi): the FFH code above can change the improvement to NO_IMPROVEMENT.
            // C++ quirk (kept): after a pillage the progress is 0 and compared against the NEW
            // improvement's upgrade time, but the plot then becomes the OLD improvement's upgrade.
            if let Some(current) = self.map.plots[p].improvement_type {
                if self.map.plots[p].upgrade_progress >= self.improvement_upgrade_time(current) {
                    self.plot_set_improvement_type(p, Some(upgrade));
                }
            }
        }
    }

    /// C++ `CvPlot::doFeature` (protected): FFH feature upgrades and flames, feature
    /// disappearance and growth.
    pub fn plot_do_feature(&mut self, p: PlotIdx) {
        let d = self.d;
        let (x, y) = self.map.xy(p);

        if let Some(feature) = self.map.plots[p].feature_type {
            //FfH: Added by Kael 03/20/2008
            if let Some(feature_upgrade) = d.features[feature].feature_upgrade {
                let prereq_religion = d.features[feature_upgrade].prereq_state_religion;
                let religion_ok = prereq_religion.is_none()
                    || match self.map.plots[p].owner {
                        Some(owner) => prereq_religion == self.player_get_state_religion(owner),
                        None => false,
                    };
                if religion_ok {
                    // UNOFFICIAL_PATCH 10/22/09 jdog5000: game speed scaling
                    let odds = (100 * d.game_speeds[self.game_speed_type()].feature_production_percent) / 100;
                    if self.soren_rand_num(odds, "Feature Upgrade") < d.defines.feature_upgrade_chance {
                        self.plot_set_feature_type(p, Some(feature_upgrade));
                    }
                }
            }
            let flames_feature = d.defines.flames_feature;
            let flames_spread_effect = d.defines.flames_spread_effect;
            if flames_feature != -1 && flames_spread_effect != -1 {
                if opt_to_i32(self.map.plots[p].feature_type) == flames_feature {
                    if self.soren_rand_num(100, "Flames Spread") < d.defines.flames_spread_chance {
                        for i in 0..DirectionType::NUM {
                            let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) else {
                                continue;
                            };
                            if let Some(adjacent_feature) = self.map.plots[adjacent].feature_type {
                                if d.features[adjacent_feature].flammable && self.map.plots[adjacent].improvement_type.is_none() {
                                    self.plot_set_improvement_type(adjacent, opt_from_i32(flames_spread_effect));
                                }
                            }
                        }
                    }
                    let flames = FeatureType::from_usize(flames_feature as usize);
                    // isTerrain(getTerrainType()): a plot without terrain matches nothing.
                    let flames_terrain = self.map.plots[p].terrain_type.map_or(false, |t| d.features[flames].terrain[t]);
                    if !flames_terrain {
                        // C++ quirk (kept): the expiry roll reuses the "Flames Spread" log string.
                        if self.soren_rand_num(100, "Flames Spread") < d.defines.flames_expire_chance {
                            self.plot_set_feature_type(p, None);
                            let flames_expire_effect = d.defines.flames_expire_effect;
                            if flames_expire_effect != -1 {
                                let expire_feature = opt_from_i32(flames_expire_effect);
                                if self.plot_can_have_feature(p, expire_feature) {
                                    self.plot_set_feature_type(p, expire_feature);
                                }
                            }
                            if self.map.plots[p].feature_type.is_none() {
                                return;
                            }
                        }
                    }
                }
            }
            //FfH: End Add

            // The feature is still set here (the flames branch returns when it removed it).
            let Some(feature) = self.map.plots[p].feature_type else {
                return;
            };
            let probability = d.features[feature].disappearance_probability;
            if probability > 0 {
                // UNOFFICIAL_PATCH 10/22/09 jdog5000: game speed scaling
                let odds = (10000 * d.game_speeds[self.game_speed_type()].feature_production_percent) / 100;
                if self.soren_rand_num(odds, "Feature Disappearance") < probability {
                    self.plot_set_feature_type(p, None);
                }
            }
        } else if !self.plot_is_unit(p) && self.map.plots[p].improvement_type.is_none() {
            for feature in d.features.keys() {
                if !self.plot_can_have_feature(p, Some(feature)) {
                    continue;
                }
                let bonus_ok = match self.map.plots[p].bonus_type {
                    None => true,
                    Some(bonus) => d.bonuses[bonus].feature[feature],
                };
                if !bonus_ok {
                    continue;
                }

                let mut probability = 0;
                for j in 0..CardinalDirectionType::NUM {
                    let Some(loop_plot) = self.plot_cardinal_direction(x, y, CardinalDirectionType::from_usize(j)) else {
                        continue;
                    };
                    if self.map.plots[loop_plot].feature_type == Some(feature) {
                        match self.map.plots[loop_plot].improvement_type {
                            None => probability += d.features[feature].growth_probability,
                            Some(loop_improvement) => {
                                probability += d.improvements[loop_improvement].feature_growth_probability;
                            }
                        }
                    }
                }

                probability *= std::cmp::max(0, d.defines.feature_growth_modifier + 100);
                probability /= 100;

                if self.plot_is_route(p) {
                    probability *= std::cmp::max(0, d.defines.route_feature_growth_modifier + 100);
                    probability /= 100;
                }

                if probability > 0 {
                    // UNOFFICIAL_PATCH 10/22/09 jdog5000: game speed scaling
                    let odds = (10000 * d.game_speeds[self.game_speed_type()].feature_production_percent) / 100;
                    if self.soren_rand_num(odds, "Feature Growth") < probability {
                        self.plot_set_feature_type(p, Some(feature));

                        // TXT_KEY_MISC_FEATURE_GROWN_NEAR_CITY (UI message)
                        log_owner_city_message(self, p, |city| {
                            format!("{} has grown near {}", d.text.name(&d.features[feature].base.description), city)
                        });

                        break;
                    }
                }
            }
        }
    }

    /// C++ `CvPlot::erase`: kills the plot's units and city, clears bonus, improvement, route,
    /// feature, FFH temporary terrain and rivers.
    pub fn plot_erase(&mut self, p: PlotIdx) {
        // kill units
        let old_units = self.map.plots[p].units.clone();
        for unit in old_units {
            if self.units.contains(unit) {
                self.unit_kill(unit, false, None);
            }
        }

        // kill cities
        if let Some(city) = live_plot_city(self, p) {
            self.city_kill(city, false);
        }

        self.plot_set_bonus_type(p, None);
        self.plot_set_improvement_type(p, None);
        self.plot_set_route_type(p, None, false);
        self.plot_set_feature_type(p, None);

        //FfH: Added by Kael 10/14/2009
        self.map.plots[p].real_terrain_type = None;
        self.map.plots[p].temp_terrain_timer = 0;
        //FfH: End Add

        // disable rivers
        self.plot_set_n_of_river(p, false, None);
        self.plot_set_w_of_river(p, false, None);
        self.map.plots[p].river_id = -1;
    }

    // ------------------------------------------------------------------
    // Plot type and terrain
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isWater`.
    #[inline]
    pub fn plot_is_water(&self, p: PlotIdx) -> bool {
        self.map.plots[p].plot_type == PlotType::PLOT_OCEAN
    }

    /// C++ `CvPlot::isFlatlands`.
    #[inline]
    pub fn plot_is_flatlands(&self, p: PlotIdx) -> bool {
        self.map.plots[p].plot_type == PlotType::PLOT_LAND
    }

    /// C++ `CvPlot::isHills`.
    #[inline]
    pub fn plot_is_hills(&self, p: PlotIdx) -> bool {
        self.map.plots[p].plot_type == PlotType::PLOT_HILLS
    }

    /// C++ `CvPlot::isPeak`.
    #[inline]
    pub fn plot_is_peak(&self, p: PlotIdx) -> bool {
        self.map.plots[p].plot_type == PlotType::PLOT_PEAK
    }

    /// C++ `CvPlot::isImpassable`: peaks, or impassable terrain/feature.
    pub fn plot_is_impassable(&self, p: PlotIdx) -> bool {
        if self.plot_is_peak(p) {
            return true;
        }

        let plot = &self.map.plots[p];
        let Some(terrain) = plot.terrain_type else {
            return false;
        };

        match plot.feature_type {
            None => self.d.terrains[terrain].impassable,
            Some(feature) => self.d.features[feature].impassable,
        }
    }

    /// C++ `CvPlot::setTerrainType` (`bRebuildGraphics` dropped: graphics only).
    /// Switches the plot type when the terrain's water flag differs.
    pub fn plot_set_terrain_type(&mut self, p: PlotIdx, terrain: TerrainType, recalculate: bool) {
        let d = self.d;
        let old_terrain = self.map.plots[p].terrain_type;

        if old_terrain == Some(terrain) {
            return;
        }

        let update_sight = match old_terrain {
            Some(old) => {
                d.terrains[old].see_from_level != d.terrains[terrain].see_from_level
                    || d.terrains[old].see_through_level != d.terrains[terrain].see_through_level
            }
            None => false,
        };

        if update_sight {
            self.plot_update_see_from_sight(p, false, true);
        }

        self.map.plots[p].terrain_type = Some(terrain);

        self.plot_update_yield(p);
        self.plot_update_plot_group(p);

        if update_sight {
            self.plot_update_see_from_sight(p, true, true);
        }

        // (RebuildPlot: graphics)

        // GC.getTerrainInfo(getTerrainType()): re-read after the updates above.
        let current = self.map.plots[p].terrain_type.unwrap_or(terrain);
        let water = d.terrains[current].water;
        if water != self.plot_is_water(p) {
            let plot_type = if water { PlotType::PLOT_OCEAN } else { PlotType::PLOT_LAND };
            self.plot_set_plot_type(p, plot_type, recalculate);
        }
    }

    /// C++ `CvPlot::setTempTerrainType` (FFH): remembers the real terrain and starts the timer.
    pub fn plot_set_temp_terrain_type(&mut self, p: PlotIdx, terrain: TerrainType, timer: i32) {
        if self.map.plots[p].terrain_type != Some(terrain) {
            // Dont overwrite the real terrain if we double temp
            if self.map.plots[p].real_terrain_type.is_none() {
                self.map.plots[p].real_terrain_type = self.map.plots[p].terrain_type;
            }
            // changeTempTerrainTimer(iTimer)
            self.map.plots[p].temp_terrain_timer += timer;
            self.plot_set_terrain_type(p, terrain, true);
        }
    }

    /// C++ `CvPlot::changePlotCounter` (FFH hell terrain): clamps to 0..=100 and moves the
    /// terrain up/down its terrain chain.
    pub fn plot_change_plot_counter(&mut self, p: PlotIdx, change: i32) {
        let d = self.d;
        let mut changed = false;

        let plot = &mut self.map.plots[p];
        plot.plot_counter += change;
        if plot.plot_counter < 0 {
            plot.plot_counter = 0;
        }
        if plot.plot_counter > 100 {
            plot.plot_counter = 100;
        }

        // C++ reads GC.getTerrainInfo(getTerrainType()) unchecked; a plot without terrain is
        // left alone. A NO_TERRAIN TerrainDown/TerrainUp (C++: setTerrainType(NO_TERRAIN),
        // undefined) does not occur in FFH2 data (only terrains with a target have thresholds
        // inside 0..=100); it leaves the terrain unchanged.
        if let Some(terrain) = self.map.plots[p].terrain_type {
            if self.map.plots[p].plot_counter < d.terrains[terrain].plot_counter_down {
                if let Some(down) = d.terrains[terrain].terrain_down {
                    self.plot_set_terrain_type(p, down, true);
                }
                changed = true;
            }
        }
        // getTerrainType() again: the step down above may have changed it.
        if let Some(terrain) = self.map.plots[p].terrain_type {
            if self.map.plots[p].plot_counter > d.terrains[terrain].plot_counter_up {
                if let Some(up) = d.terrains[terrain].terrain_up {
                    self.plot_set_terrain_type(p, up, true);
                }
                changed = true;
            }
        }

        if changed {
            if let Some(feature) = self.map.plots[p].feature_type {
                let terrain_ok = self.map.plots[p].terrain_type.map_or(false, |t| d.features[feature].terrain[t]);
                if !terrain_ok {
                    self.plot_set_feature_type(p, None);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Features
    // ------------------------------------------------------------------

    /// C++ `CvPlot::canHaveFeature` (None = NO_FEATURE: always true).
    pub fn plot_can_have_feature(&self, p: PlotIdx, feature: Option<FeatureType>) -> bool {
        let d = self.d;

        let Some(feature) = feature else {
            return true;
        };

        let plot = &self.map.plots[p];

        if plot.feature_type.is_some() {
            return false;
        }

        if self.plot_is_peak(p) {
            return false;
        }

        if live_plot_city(self, p).is_some() {
            return false;
        }

        let info = &d.features[feature];

        // isTerrain(getTerrainType()): C++ asserts a terrain; none matches nothing here.
        if !plot.terrain_type.map_or(false, |t| info.terrain[t]) {
            return false;
        }

        if info.no_coast && self.plot_is_coastal_land(p, -1) {
            return false;
        }

        if info.no_river && self.plot_is_river(p) {
            return false;
        }

        if info.requires_flatlands && self.plot_is_hills(p) {
            return false;
        }

        if info.no_adjacent {
            let (x, y) = self.map.xy(p);
            for i in 0..DirectionType::NUM {
                if let Some(adjacent) = self.plot_direction(x, y, DirectionType::from_usize(i)) {
                    if self.map.plots[adjacent].feature_type == Some(feature) {
                        return false;
                    }
                }
            }
        }

        if info.requires_river && !self.plot_is_river(p) {
            return false;
        }

        true
    }

    /// C++ `CvPlot::setFeatureType` (the art-only `iVariety` argument is dropped).
    ///
    /// C++ also re-runs the body when only the art variety changes (same feature); with the
    /// same feature that re-run recomputes the same yields and city feature health/happiness,
    /// so only a real feature change is processed here.
    pub fn plot_set_feature_type(&mut self, p: PlotIdx, feature: Option<FeatureType>) {
        let d = self.d;
        let old_feature = self.map.plots[p].feature_type;

        if old_feature == feature {
            return;
        }

        let update_sight = match (old_feature, feature) {
            (Some(old), Some(new)) => d.features[old].see_through_change != d.features[new].see_through_change,
            _ => true,
        };

        if update_sight {
            self.plot_update_see_from_sight(p, false, true);
        }

        self.map.plots[p].feature_type = feature;

        self.plot_update_yield(p);

        if update_sight {
            self.plot_update_see_from_sight(p, true, true);
        }

        // (updateFeatureSymbol, updateRiverSymbolArt: graphics)

        let (x, y) = self.map.xy(p);
        for i in 0..NUM_CITY_PLOTS {
            if let Some(loop_plot) = self.plot_city(x, y, i) {
                if let Some(loop_city) = live_plot_city(self, loop_plot) {
                    self.city_update_feature_health(loop_city);
                    self.city_update_feature_happiness(loop_city);
                }
            }
        }

        if self.map.plots[p].feature_type.is_none() {
            if let Some(improvement) = self.map.plots[p].improvement_type {
                if d.improvements[improvement].requires_feature {
                    self.plot_set_improvement_type(p, None);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Bonuses
    // ------------------------------------------------------------------

    /// C++ `CvPlot::canHaveBonus` (None = NO_BONUS: always true).
    pub fn plot_can_have_bonus(&self, p: PlotIdx, bonus: Option<BonusType>, ignore_latitude: bool) -> bool {
        let d = self.d;

        let Some(bonus) = bonus else {
            return true;
        };

        let plot = &self.map.plots[p];

        if plot.bonus_type.is_some() {
            return false;
        }

        if self.plot_is_peak(p) {
            return false;
        }

        let info = &d.bonuses[bonus];

        // isFeatureTerrain/isTerrain(getTerrainType()): C++ asserts a terrain; none matches
        // nothing here.
        if let Some(feature) = plot.feature_type {
            if !info.feature[feature] {
                return false;
            }

            if !plot.terrain_type.map_or(false, |t| info.feature_terrain[t]) {
                return false;
            }
        } else if !plot.terrain_type.map_or(false, |t| info.terrain[t]) {
            return false;
        }

        if self.plot_is_hills(p) {
            if !info.hills {
                return false;
            }
        } else if self.plot_is_flatlands(p) && !info.flatlands {
            return false;
        }

        if info.no_river_side && self.plot_is_river_side(p) {
            return false;
        }

        if info.min_area_size != -1 {
            // C++ dereferences area() unchecked; a plot without area has no tiles here.
            let num_tiles = live_area(self, p).map_or(0, |a| self.map.areas[a].num_tiles);
            if num_tiles < info.min_area_size {
                return false;
            }
        }

        if !ignore_latitude {
            let latitude = self.plot_get_latitude(p);

            if latitude > info.max_latitude {
                return false;
            }

            if latitude < info.min_latitude {
                return false;
            }
        }

        if !plot.potential_city_work {
            return false;
        }

        //FfH: Added by Kael 12/18/2008
        if live_plot_city(self, p).is_some() {
            let bonus_mana = d.defines.bonus_mana;
            if bonus_mana != -1 && bonus.i() == bonus_mana {
                return false;
            }
        }
        //FfH: End Add

        true
    }

    /// C++ `CvPlot::getBonusType(eTeam)`: the bonus if `team` has its reveal tech (or it is
    /// force-revealed). `team == None` is the plain `bonus_type` field.
    pub fn plot_get_bonus_type(&self, p: PlotIdx, team: Option<TeamId>) -> Option<BonusType> {
        let bonus = self.map.plots[p].bonus_type;

        if let (Some(team), Some(b)) = (team, bonus) {
            if !self.team_is_has_tech(team, self.d.bonuses[b].tech_reveal) && !self.team_is_force_revealed_bonus(team, b) {
                return None;
            }
        }

        bonus
    }

    /// C++ `CvPlot::getNonObsoleteBonusType`.
    pub fn plot_get_non_obsolete_bonus_type(&self, p: PlotIdx, team: TeamId) -> Option<BonusType> {
        let bonus = self.plot_get_bonus_type(p, Some(team));
        if let Some(b) = bonus {
            if self.team_is_bonus_obsolete(team, b) {
                return None;
            }
        }

        bonus
    }

    /// C++ `CvPlot::setBonusType`.
    pub fn plot_set_bonus_type(&mut self, p: PlotIdx, bonus: Option<BonusType>) {
        let old_bonus = self.map.plots[p].bonus_type;

        if old_bonus == bonus {
            return;
        }

        if let Some(old) = old_bonus {
            if let Some(area) = live_area(self, p) {
                self.map.areas[area].num_bonuses[old] -= 1;
            }
            self.map.num_bonus[old] -= 1;

            if !self.plot_is_water(p) {
                self.map.num_bonus_on_land[old] -= 1;
            }
        }

        self.plot_update_plot_group_bonus(p, false);
        self.map.plots[p].bonus_type = bonus;
        self.plot_update_plot_group_bonus(p, true);

        if let Some(new) = self.map.plots[p].bonus_type {
            if let Some(area) = live_area(self, p) {
                self.map.areas[area].num_bonuses[new] += 1;
            }
            self.map.num_bonus[new] += 1;

            if !self.plot_is_water(p) {
                self.map.num_bonus_on_land[new] += 1;
            }
        }

        self.plot_update_yield(p);

        // (setLayoutDirty, GlobeLayer_DIRTY_BIT: graphics)
    }

    // ------------------------------------------------------------------
    // Improvements and goodies
    // ------------------------------------------------------------------

    /// C++ `CvPlot::canHaveImprovement` (C++ defaults: `eTeam = NO_TEAM`, `bPotential = false`).
    pub fn plot_can_have_improvement(&self, p: PlotIdx, improvement: ImprovementType, team: Option<TeamId>, potential: bool) -> bool {
        let d = self.d;
        let info = &d.improvements[improvement];
        let plot = &self.map.plots[p];

        let mut valid = false;

        if live_plot_city(self, p).is_some() {
            return false;
        }

        //FfH: Modified by Kael 09/10/2008
        if self.plot_is_impassable(p) {
            if self.plot_is_peak(p) {
                if !info.requires_peak {
                    return false;
                }
            } else {
                return false;
            }
        }
        if info.requires_peak && !self.plot_is_peak(p) {
            return false;
        }
        //FfH: End Modify

        if info.water != self.plot_is_water(p) {
            return false;
        }

        if let Some(feature) = plot.feature_type {
            if d.features[feature].no_improvement {
                return false;
            }
        }

        if let Some(bonus) = self.plot_get_bonus_type(p, team) {
            if info.improvement_bonus[bonus].bonus_makes_valid {
                return true;
            }
        }

        if info.no_fresh_water && self.plot_is_fresh_water(p) {
            return false;
        }

        if info.requires_flatlands && !self.plot_is_flatlands(p) {
            return false;
        }

        if info.requires_feature && plot.feature_type.is_none() {
            return false;
        }

        if info.hills_makes_valid && self.plot_is_hills(p) {
            valid = true;
        }

        if info.fresh_water_makes_valid && self.plot_is_fresh_water(p) {
            valid = true;
        }

        if info.river_side_makes_valid && self.plot_is_river_side(p) {
            valid = true;
        }

        // getTerrainMakesValid(getTerrainType()): C++ asserts a terrain.
        if plot.terrain_type.is_some_and(|t| info.terrain_makes_valid[t]) {
            valid = true;
        }

        if let Some(feature) = plot.feature_type {
            if info.feature_makes_valid[feature] {
                valid = true;
            }
        }

        if !valid {
            return false;
        }

        if info.requires_river_side {
            valid = false;

            let (x, y) = self.map.xy(p);
            for i in 0..CardinalDirectionType::NUM {
                if let Some(loop_plot) = self.plot_cardinal_direction(x, y, CardinalDirectionType::from_usize(i)) {
                    if self.plot_is_river_crossing(p, self.direction_xy_plots(p, loop_plot))
                        && self.map.plots[loop_plot].improvement_type != Some(improvement)
                    {
                        valid = true;
                        break;
                    }
                }
            }

            if !valid {
                return false;
            }
        }

        for i in 0..YieldType::NUM {
            if self.plot_calculate_nature_yield(p, YieldType::from_usize(i), team, false) < info.prereq_nature_yield[i] {
                return false;
            }
        }

        let ignore_irrigation = self.plot_get_team(p).is_some_and(|t| self.team_is_ignore_irrigation(t));
        if !ignore_irrigation && !potential && info.requires_irrigation && !self.plot_is_irrigation_available(p, false) {
            return false;
        }

        true
    }

    /// C++ `CvPlot::setImprovementType`.
    pub fn plot_set_improvement_type(&mut self, p: PlotIdx, improvement: Option<ImprovementType>) {
        let d = self.d;
        let old_improvement = self.map.plots[p].improvement_type;

        if old_improvement == improvement {
            return;
        }

        if let Some(old) = old_improvement {
            if let Some(area) = live_area(self, p) {
                self.map.areas[area].num_improvements[old] -= 1;
            }
            if let Some(owner) = self.map.plots[p].owner {
                self.players[owner].improvement_count[old] -= 1;
            }
        }

        self.plot_update_plot_group_bonus(p, false);
        self.map.plots[p].improvement_type = improvement;
        self.plot_update_plot_group_bonus(p, true);

        if self.map.plots[p].improvement_type.is_none() {
            // setImprovementDuration(0)
            self.map.plots[p].improvement_duration = 0;
        }

        // setUpgradeProgress(0)
        self.map.plots[p].upgrade_progress = 0;

        for team in self.team_ids() {
            if self.team_is_alive(team) && self.plot_is_visible(p, team) {
                // setRevealedImprovementType(team, getImprovementType()) (side effects: UI)
                let current = self.map.plots[p].improvement_type;
                self.map.plots[p].revealed_improvement_type[team] = current;
            }
        }

        if let Some(new) = self.map.plots[p].improvement_type {
            if let Some(area) = live_area(self, p) {
                self.map.areas[area].num_improvements[new] += 1;
            }
            if let Some(owner) = self.map.plots[p].owner {
                self.players[owner].improvement_count[new] += 1;
            }
        }

        self.plot_update_irrigated(p);
        self.plot_update_yield(p);

        let (x, y) = self.map.xy(p);
        for i in 0..NUM_CITY_PLOTS {
            if let Some(loop_plot) = self.plot_city(x, y, i) {
                if let Some(loop_city) = live_plot_city(self, loop_plot) {
                    self.city_update_feature_happiness(loop_city);
                }
            }
        }

        // Building or removing a fort will now force a plotgroup update to verify resource connections.
        let acts_as_city = |i: Option<ImprovementType>| i.is_some_and(|i| d.improvements[i].acts_as_city);
        if acts_as_city(self.map.plots[p].improvement_type) != acts_as_city(old_improvement) {
            self.plot_update_plot_group(p);
        }

        if acts_as_city(old_improvement) {
            self.plot_verify_unit_valid_plot(p);
        }

        // (debug mode setLayoutDirty: graphics)

        if let Some(current) = self.map.plots[p].improvement_type {
            //FfH Improvements: Added by Kael 08/07/2007
            if let Some(bonus) = d.improvements[current].bonus_convert {
                self.plot_set_bonus_type(p, Some(bonus));
            }
            //FfH: End Add

            if let Some(current) = self.map.plots[p].improvement_type {
                self.ev_improvement_built(current, x, y);
            }
        }

        if self.map.plots[p].improvement_type.is_none() {
            // eOldImprovement is set: the improvement changed to NO_IMPROVEMENT.
            if let Some(old) = old_improvement {
                let owner = self.map.plots[p].owner;
                self.ev_improvement_destroyed(old, owner, x, y);
            }
        }

        if let Some(working_city) = self.map.plots[p].working_city.filter(|&c| self.cities.contains(c)) {
            let free_specialists = |i: Option<ImprovementType>| {
                i.is_some_and(|i| self.cities[working_city].improvement_free_specialists[i] > 0)
            };
            if free_specialists(improvement) || free_specialists(old_improvement) {
                // AI_setAssignWorkDirty(true): AI bookkeeping field
                self.cities[working_city].ai_assign_work_dirty = true;
            }
        }

        // (CitizenButtons_DIRTY_BIT: UI)
    }

    /// C++ `CvPlot::getUpgradeTimeLeft`.
    pub fn plot_get_upgrade_time_left(&self, p: PlotIdx, improvement: ImprovementType, player: Option<PlayerId>) -> i32 {
        let progress = if self.map.plots[p].improvement_type == Some(improvement) {
            self.map.plots[p].upgrade_progress
        } else {
            0
        };
        let upgrade_left = self.improvement_upgrade_time(improvement) - progress;

        let Some(player) = player else {
            return upgrade_left;
        };

        let upgrade_rate = self.player_get_improvement_upgrade_rate(player);

        if upgrade_rate == 0 {
            return upgrade_left;
        }

        let mut turns_left = upgrade_left / upgrade_rate;

        if turns_left * upgrade_rate < upgrade_left {
            turns_left += 1;
        }

        std::cmp::max(1, turns_left)
    }

    /// C++ `CvPlot::isGoody` (C++ default `eTeam = NO_TEAM`).
    pub fn plot_is_goody(&self, p: PlotIdx, team: Option<TeamId>) -> bool {
        if team.is_some_and(|t| self.team_is_barbarian(t)) {
            return false;
        }

        self.map.plots[p].improvement_type.is_some_and(|i| self.d.improvements[i].goody)
    }

    /// C++ `CvPlot::isRevealedGoody` (FFH: lairs and equipment count as goodies at war).
    pub fn plot_is_revealed_goody(&self, p: PlotIdx, team: Option<TeamId>) -> bool {
        let d = self.d;

        let Some(team) = team else {
            return self.plot_is_goody(p, None);
        };

        if self.team_is_barbarian(team) {
            return false;
        }

        //FfH: Added by Kael 08/27/2007
        if let Some(improvement) = self.map.plots[p].improvement_type {
            let info = &d.improvements[improvement];
            if info.spawn_unit_type.is_some() && !info.permanent {
                let barbarian_team = self.players[self.barbarian_player()].team;
                if self.at_war(team, barbarian_team) {
                    return true;
                }
            }
        }
        if self.plot_is_unit(p) {
            for &unit in &self.map.plots[p].units {
                if d.units[self.units[unit].unit_type].equipment_promotion.is_some() && self.at_war(team, self.unit_get_team(unit)) {
                    return true;
                }
            }
        }
        //FfH: End Add

        self.map.plots[p].revealed_improvement_type[team].is_some_and(|i| d.improvements[i].goody)
    }

    /// C++ `CvPlot::removeGoody`.
    pub fn plot_remove_goody(&mut self, p: PlotIdx) {
        self.plot_set_improvement_type(p, None);
    }

    /// C++ static `CvPlot::c_aePirateCoveTypes` set-up (first `CvPlot` constructor): the
    /// `PIRATE_COVE_IMPROVEMENT` define and its upgrade chain, stored sorted in
    /// `Map::pirate_cove_types`.
    ///
    /// C++ fills the static set once, when the first plot is constructed (`CvMap::init`).
    /// The set lives on `Map` here and `Map::new` / `Map::default` start it empty, so
    /// `map_init` must call this right after it installs the new `Map`. Rebuilding is
    /// idempotent.
    pub fn plot_init_pirate_cove_types(&mut self) {
        let d = self.d;
        let mut types: Vec<ImprovementType> = Vec::new();

        let mut pirate_cove: Option<ImprovementType> = opt_from_i32(d.defines.pirate_cove_improvement);
        while let Some(cove) = pirate_cove {
            match types.binary_search(&cove) {
                Ok(_) => {
                    // C++ would walk an upgrade cycle forever (re-inserting into the set does
                    // not end the loop); the chain is complete once a type repeats.
                    break;
                }
                Err(pos) => types.insert(pos, cove),
            }
            pirate_cove = d.improvements[cove].improvement_upgrade;
        }

        self.map.pirate_cove_types = types;
    }

    /// C++ `CvPlot::isPirateCove` (BETTER AI).
    pub fn plot_is_pirate_cove(&self, p: PlotIdx) -> bool {
        match self.map.plots[p].improvement_type {
            Some(i) => self.map.pirate_cove_types.binary_search(&i).is_ok(),
            None => false,
        }
    }

    /// C++ `CvPlot::isPirateCoveValid` (BETTER AI; AI-only callers).
    pub fn plot_is_pirate_cove_valid(&self, p: PlotIdx, player: PlayerId) -> bool {
        if !self.plot_is_water(p) {
            return false;
        }

        if !self.plot_is_adjacent_to_land(p) {
            return false;
        }

        let plot = &self.map.plots[p];

        if live_plot_city(self, p).is_some() {
            return false;
        }

        if plot.owner != Some(player) {
            return false;
        }

        if plot.improvement_type.is_some() {
            return false;
        }

        if plot.bonus_type.is_some() {
            return false;
        }

        let (x, y) = self.map.xy(p);
        for dx in -2..=2 {
            for dy in -2..=2 {
                if let Some(loop_plot) = self.plot_xy(x, y, dx, dy) {
                    if self.plot_is_pirate_cove(loop_plot) {
                        return false;
                    }
                }
            }
        }

        true
    }

    // ------------------------------------------------------------------
    // Routes
    // ------------------------------------------------------------------

    /// C++ `CvPlot::isRoute`.
    #[inline]
    pub fn plot_is_route(&self, p: PlotIdx) -> bool {
        self.map.plots[p].route_type.is_some()
    }

    /// C++ `CvPlot::setRouteType`.
    pub fn plot_set_route_type(&mut self, p: PlotIdx, route: Option<RouteType>, update_plot_groups: bool) {
        if self.map.plots[p].route_type == route {
            return;
        }

        let old_route = self.plot_is_route(p); // XXX is this right??? (C++)

        self.plot_update_plot_group_bonus(p, false);
        self.map.plots[p].route_type = route;
        self.plot_update_plot_group_bonus(p, true);

        for team in self.team_ids() {
            if self.team_is_alive(team) && self.plot_is_visible(p, team) {
                // setRevealedRouteType(team, getRouteType()) (side effects: UI)
                let current = self.map.plots[p].route_type;
                self.map.plots[p].revealed_route_type[team] = current;
            }
        }

        self.plot_update_yield(p);

        if update_plot_groups && old_route != self.plot_is_route(p) {
            self.plot_update_plot_group(p);
        }

        // (debug mode updateRouteSymbol: graphics)

        if let Some(current) = self.map.plots[p].route_type {
            let (x, y) = self.map.xy(p);
            self.ev_route_built(current, x, y);
        }
    }

    /// C++ `CvPlot::updateCityRoute`.
    pub fn plot_update_city_route(&mut self, p: PlotIdx, update_plot_group: bool) {
        if live_plot_city(self, p).is_none() {
            return;
        }

        // C++ FAssertMsg(isOwned()) then GET_PLAYER(getOwnerINLINE()): a city plot is owned.
        let Some(owner) = self.map.plots[p].owner else {
            debug_assert!(false, "CvPlot::updateCityRoute: city plot without owner");
            return;
        };

        let mut city_route = self.player_get_best_route(owner, None);

        if city_route.is_none() {
            city_route = opt_from_i32(self.d.defines.initial_city_route_type);
        }

        self.plot_set_route_type(p, city_route, update_plot_group);
    }

    // ------------------------------------------------------------------
    // Builds
    // ------------------------------------------------------------------

    /// C++ `CvPlot::canBuild` (calls the `canBuild` Python callback when enabled).
    pub fn plot_can_build(&self, p: PlotIdx, build: BuildType, player: PlayerId, test_visible: bool) -> bool {
        let d = self.d;
        let plot = &self.map.plots[p];

        //FfH: Added by Kael 11/10/2008
        if plot.build_disabled {
            return false;
        }
        //FfH: End Add

        if d.defines.use_can_build_callback != 0 {
            // Python canBuild(iX, iY, iBuild, iPlayer): -1 = use default, 0 = cannot, >= 1 = can.
            let result = self.py_can_build(p, build, test_visible, player);
            if result >= 1 {
                return true;
            } else if result == 0 {
                return false;
            }
        }

        let build_info = &d.builds[build];
        let player_team = self.players[player].team;
        let plot_team = self.plot_get_team(p);

        let mut valid = false;

        if let Some(improvement) = build_info.improvement {
            if !self.plot_can_have_improvement(p, improvement, Some(player_team), test_visible) {
                return false;
            }

            if let Some(current) = plot.improvement_type {
                if d.improvements[current].permanent {
                    return false;
                }

                if current == improvement {
                    return false;
                }

                if let Some(final_improvement) = final_improvement_upgrade(d, current, 0) {
                    if Some(final_improvement) == final_improvement_upgrade(d, improvement, 0) {
                        return false;
                    }
                }
            }

            if !test_visible && Some(player_team) != plot_team {
                if d.improvements[improvement].outside_borders {
                    //outside borders can't be built in other's culture
                    if plot_team.is_some() {
                        return false;
                    }
                } else {
                    //only buildable in own culture
                    return false;
                }
            }

            valid = true;
        }

        if let Some(route) = build_info.route {
            if let Some(current) = plot.route_type {
                if d.routes[current].value >= d.routes[route].value {
                    return false;
                }
            }

            if !test_visible {
                if let Some(bonus) = d.routes[route].prereq_bonus {
                    if !self.plot_is_adjacent_plot_group_connected_bonus(p, player, bonus) {
                        return false;
                    }
                }

                let mut found_valid = true;
                for i in 0..d.defines.num_route_prereq_or_bonuses.max(0) as usize {
                    if let Some(bonus) = d.routes[route].prereq_or_bonus.get(i).copied().flatten() {
                        found_valid = false;

                        if self.plot_is_adjacent_plot_group_connected_bonus(p, player, bonus) {
                            found_valid = true;
                            break;
                        }
                    }
                }

                if !found_valid {
                    return false;
                }
            }

            valid = true;
        }

        if let Some(feature) = plot.feature_type {
            if build_info.feature_remove[feature] {
                if let Some(owner) = plot.owner {
                    // C++ quirk (kept): the FfH maintain-features test is negated, so a foreign
                    // worker at peace MAY clear a feature the plot owner's civilization maintains.
                    if Some(player_team) != plot_team
                        && !self.at_war_opt(Some(player_team), plot_team)
                        //FfH: Added by Kael 04/24/2008
                        && !self
                            .player_get_civilization_type(owner)
                            .is_some_and(|civ| d.civilizations[civ].maintain_features[feature])
                    {
                        return false;
                    }
                }

                valid = true;
            }
        }

        valid
    }

    /// C++ `CvPlot::getBuildTime`.
    pub fn plot_get_build_time(&self, p: PlotIdx, build: BuildType) -> i32 {
        let d = self.d;
        let plot = &self.map.plots[p];
        let build_info = &d.builds[build];

        let mut time = build_info.time;

        if let Some(feature) = plot.feature_type {
            time += build_info.feature_time[feature];
        }

        // GC.getTerrainInfo(getTerrainType()): C++ asserts a terrain; none counts as modifier 0.
        let build_modifier = plot.terrain_type.map_or(0, |t| d.terrains[t].build_modifier);
        time *= std::cmp::max(0, build_modifier + 100);
        time /= 100;

        time *= d.game_speeds[self.game_speed_type()].build_percent;
        time /= 100;

        time *= d.eras[self.start_era()].build_percent;
        time /= 100;

        time
    }

    /// C++ `CvPlot::getBuildTurnsLeft`.
    pub fn plot_get_build_turns_left(&self, p: PlotIdx, build: BuildType, now_extra: i32, then_extra: i32) -> i32 {
        let mut now_build_rate = now_extra;
        let mut then_build_rate = then_extra;

        for &unit in &self.map.plots[p].units {
            if self.unit_get_build_type(unit) == Some(build) {
                if self.unit_can_move(unit) {
                    now_build_rate += self.unit_work_rate(unit, false);
                }
                then_build_rate += self.unit_work_rate(unit, true);
            }
        }

        if then_build_rate == 0 {
            //this means it will take forever under current circumstances
            return MAX_INT;
        }

        let mut build_left = self.plot_get_build_time(p, build);

        build_left -= self.map.plots[p].build_progress[build];
        build_left -= now_build_rate;

        build_left = std::cmp::max(0, build_left);

        let mut turns_left = build_left / then_build_rate;

        if turns_left * then_build_rate < build_left {
            turns_left += 1;
        }

        turns_left += 1;
        std::cmp::max(1, turns_left)
    }

    /// C++ `CvPlot::getFeatureProduction`: production from clearing the feature and the city
    /// that receives it (the C++ `CvCity** ppCity` out-parameter).
    pub fn plot_get_feature_production(&self, p: PlotIdx, build: BuildType, team: TeamId) -> (i32, Option<CityId>) {
        let d = self.d;

        // C++ leaves *ppCity untouched here (callers start from NULL).
        let Some(feature) = self.map.plots[p].feature_type else {
            return (0, None);
        };

        let (x, y) = self.map.xy(p);

        let mut city = self.map.plots[p].working_city.filter(|&c| self.cities.contains(c));

        if city.is_none() {
            city = self.map_find_city(x, y, None, Some(team), false, false, None, None, None);
        }

        let Some(city) = city else {
            return (0, None);
        };

        let (city_x, city_y, city_owner, city_population) = {
            let c = &self.cities[city];
            (c.x, c.y, c.owner, c.population)
        };

        let mut production = d.builds[build].feature_production[feature]
            - std::cmp::max(0, self.plot_distance(x, y, city_x, city_y) - 2) * 5;

        production *= std::cmp::max(0, self.players[city_owner].feature_production_modifier + 100);
        production /= 100;

        production *= d.game_speeds[self.game_speed_type()].feature_production_percent;
        production /= 100;

        production *= std::cmp::min(
            d.defines.base_feature_production_percent + d.defines.feature_production_percent_multiplier * city_population,
            100,
        );
        production /= 100;

        if self.plot_get_team(p) != Some(team) {
            production *= d.defines.different_team_feature_production_percent;
            production /= 100;
        }

        (std::cmp::max(0, production), Some(city))
    }

    /// C++ `CvPlot::changeBuildProgress`: true if the build finished.
    pub fn plot_change_build_progress(&mut self, p: PlotIdx, build: BuildType, change: i32, team: TeamId) -> bool {
        let d = self.d;
        let mut finished = false;

        if change != 0 {
            self.map.plots[p].build_progress[build] += change;
            debug_assert!(self.map.plots[p].build_progress[build] >= 0);

            if self.map.plots[p].build_progress[build] >= self.plot_get_build_time(p, build) {
                self.map.plots[p].build_progress[build] = 0;

                if let Some(improvement) = d.builds[build].improvement {
                    self.plot_set_improvement_type(p, Some(improvement));
                }

                if let Some(route) = d.builds[build].route {
                    self.plot_set_route_type(p, Some(route), true);
                }

                if let Some(feature) = self.map.plots[p].feature_type {
                    if d.builds[build].feature_remove[feature] {
                        //FfH: Added by Kael 11/11/2007
                        let mut valid = true;
                        if let Some(owner) = self.map.plots[p].owner {
                            let maintain_features = self
                                .player_get_civilization_type(owner)
                                .is_some_and(|civ| d.civilizations[civ].maintain_features[feature]);
                            if maintain_features && d.builds[build].improvement.is_some() {
                                valid = false;
                            }
                        }
                        if valid {
                            //FfH: End Add
                            let (production, city) = self.plot_get_feature_production(p, build, team);

                            if production > 0 {
                                if let Some(city) = city {
                                    self.cities[city].feature_production += production;

                                    // TXT_KEY_MISC_CLEARING_FEATURE_BONUS (UI message)
                                    let city_owner = self.cities[city].owner;
                                    let turn = self.game_turn();
                                    let city_name = &self.cities[city].name;
                                    self.log.msg(turn, Some(city_owner), || {
                                        format!(
                                            "Clearing {} has produced {} production for {}",
                                            d.text.name(&d.features[feature].base.description),
                                            production,
                                            d.text.name(city_name)
                                        )
                                    });
                                }
                            }

                            // Python Event
                            self.ev_plot_feature_removed(p, feature, city);

                            self.plot_set_feature_type(p, None);
                        }
                    }
                }

                finished = true;
            }
        }

        finished
    }

    /// C++ `CvPlot::isBuilding` (FFH; AI-only callers): a unit of `team` is building `build`
    /// within `range`.
    pub fn plot_is_building(&self, p: PlotIdx, build: BuildType, team: TeamId, range: i32, exclude_center: bool) -> bool {
        let (x, y) = self.map.xy(p);

        for dx in -range..=range {
            for dy in -range..=range {
                let Some(loop_plot) = self.plot_xy(x, y, dx, dy) else {
                    continue;
                };
                if self.plot_is_owned(loop_plot)
                    && self.plot_get_team(loop_plot) == Some(team)
                    && (dx != 0 || dy != 0 || !exclude_center)
                {
                    for &unit in &self.map.plots[loop_plot].units {
                        if self.unit_get_build_type(unit) == Some(build) {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}

// ----------------------------------------------------------------------
// Private helpers (free functions: no name clashes with other `impl Game` blocks)
// ----------------------------------------------------------------------

/// C++ `CvPlot::area()`: the plot's area if it exists (`CvMap::getArea` is NULL for an
/// unknown id).
fn live_area(g: &Game, p: PlotIdx) -> Option<AreaId> {
    g.map.plots[p].area.filter(|&a| g.map.areas.contains(a))
}

/// C++ `CvPlot::getPlotCity()` (`getCity(m_plotCity)`: NULL for a deleted city).
fn live_plot_city(g: &Game, p: PlotIdx) -> Option<CityId> {
    g.map.plots[p].plot_city.filter(|&c| g.cities.contains(c))
}

/// Game-log stand-in for the C++ messages to the plot owner that name the nearest city
/// (`findCity(x, y, getOwnerINLINE(), NO_TEAM, false)`; no message without a city).
fn log_owner_city_message(g: &mut Game, p: PlotIdx, text: impl FnOnce(&str) -> String) {
    if !g.log.enabled {
        return;
    }
    let (x, y) = g.map.xy(p);
    let owner = g.map.plots[p].owner;
    if let Some(city) = g.map_find_city(x, y, owner, None, false, false, None, None, None) {
        let message = text(g.d.text.name(&g.cities[city].name));
        let turn = g.game_turn();
        g.log.msg(turn, owner, || message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, MapInitData};
    use crate::setup::GameSetup;

    /// A 2-civ game with a `w` x `h` non-wrapping map of grassland flatlands (no areas).
    fn game_with_map(w: i32, h: i32) -> Game {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 1));
        let init = MapInitData { grid_w: w, grid_h: h, ..MapInitData::default() };
        g.map = Map::new(d, &init, g.num_players(), g.num_teams());
        let grass = d.t::<TerrainType>("TERRAIN_GRASS");
        for p in g.map.plot_ids() {
            g.map.plots[p].plot_type = PlotType::PLOT_LAND;
            g.map.plots[p].terrain_type = Some(grass);
        }
        g
    }

    #[test]
    fn pirate_cove_types_follow_the_upgrade_chain() {
        let mut g = game_with_map(3, 3);
        let d = g.d;
        assert!(g.map.pirate_cove_types.is_empty());
        g.plot_init_pirate_cove_types();
        let cove = d.t::<ImprovementType>("IMPROVEMENT_PIRATE_COVE");
        let harbor = d.t::<ImprovementType>("IMPROVEMENT_PIRATE_HARBOR");
        let port = d.t::<ImprovementType>("IMPROVEMENT_PIRATE_PORT");
        let mut expected = vec![cove, harbor, port];
        expected.sort();
        assert_eq!(g.map.pirate_cove_types, expected);
        // idempotent
        g.plot_init_pirate_cove_types();
        assert_eq!(g.map.pirate_cove_types, expected);

        let p = PlotIdx(4);
        assert!(!g.plot_is_pirate_cove(p));
        g.map.plots[p].improvement_type = Some(harbor);
        assert!(g.plot_is_pirate_cove(p));
        g.map.plots[p].improvement_type = Some(d.t::<ImprovementType>("IMPROVEMENT_FARM"));
        assert!(!g.plot_is_pirate_cove(p));
    }

    #[test]
    fn impassable_peaks_terrain_and_features() {
        let mut g = game_with_map(2, 2);
        let d = g.d;
        let p = PlotIdx(0);
        assert!(!g.plot_is_impassable(p));
        g.map.plots[p].plot_type = PlotType::PLOT_PEAK;
        assert!(g.plot_is_impassable(p));
        g.map.plots[p].plot_type = PlotType::PLOT_LAND;
        // an impassable feature overrides the terrain
        g.map.plots[p].feature_type = Some(d.t::<FeatureType>("FEATURE_VOLCANO"));
        assert!(g.plot_is_impassable(p));
        g.map.plots[p].feature_type = Some(d.t::<FeatureType>("FEATURE_FOREST"));
        assert!(!g.plot_is_impassable(p));
        // no terrain (before map generation): passable
        g.map.plots[p].feature_type = None;
        g.map.plots[p].terrain_type = None;
        assert!(!g.plot_is_impassable(p));
    }

    #[test]
    fn build_time_adds_feature_time_and_scales() {
        let mut g = game_with_map(2, 2);
        let d = g.d;
        let p = PlotIdx(0);
        let farm = d.t::<BuildType>("BUILD_FARM");
        let forest = d.t::<FeatureType>("FEATURE_FOREST");
        let speed = &d.game_speeds[g.game_speed_type()];
        let era = &d.eras[g.start_era()];
        let scale = |t: i32, terrain_modifier: i32| ((t * (terrain_modifier + 100) / 100) * speed.build_percent / 100) * era.build_percent / 100;

        assert_eq!(d.builds[farm].time, 500);
        assert_eq!(d.terrains[d.t::<TerrainType>("TERRAIN_GRASS")].build_modifier, 0);
        assert_eq!(g.plot_get_build_time(p, farm), scale(500, 0));

        g.map.plots[p].feature_type = Some(forest);
        assert_eq!(d.builds[farm].feature_time[forest], 250);
        assert_eq!(g.plot_get_build_time(p, farm), scale(750, 0));

        // a terrain with a build modifier
        let (terrain, modifier) = d
            .terrains
            .iter_enumerated()
            .find(|(_, t)| t.build_modifier > 0)
            .map(|(id, t)| (id, t.build_modifier))
            .expect("a terrain with a build modifier");
        g.map.plots[p].terrain_type = Some(terrain);
        assert_eq!(g.plot_get_build_time(p, farm), scale(750, modifier));
    }

    #[test]
    fn build_turns_left_without_workers() {
        let mut g = game_with_map(2, 2);
        let d = g.d;
        let p = PlotIdx(0);
        let farm = d.t::<BuildType>("BUILD_FARM");
        let time = g.plot_get_build_time(p, farm);
        assert!(time > 0);

        // no workers and no extra rate: forever
        assert_eq!(g.plot_get_build_turns_left(p, farm, 0, 0), MAX_INT);
        // rounds up, then one more turn
        assert_eq!(g.plot_get_build_turns_left(p, farm, 0, time), 2);
        assert_eq!(g.plot_get_build_turns_left(p, farm, 0, time - 1), 3);
        // progress and this turn's work count first
        g.map.plots[p].build_progress[farm] = time - 1;
        assert_eq!(g.plot_get_build_turns_left(p, farm, 1, 1), 1);
        assert_eq!(g.plot_get_build_turns_left(p, farm, 0, 1), 2);
    }

    #[test]
    fn bonus_type_needs_reveal_tech_or_force_reveal() {
        let mut g = game_with_map(2, 2);
        let d = g.d;
        let p = PlotIdx(0);
        let (bonus, tech) = d
            .bonuses
            .iter_enumerated()
            .find_map(|(b, info)| info.tech_reveal.map(|t| (b, t)))
            .expect("a bonus with a reveal tech");
        let team = TeamId(0);

        assert_eq!(g.plot_get_bonus_type(p, Some(team)), None);
        g.map.plots[p].bonus_type = Some(bonus);
        assert_eq!(g.plot_get_bonus_type(p, None), Some(bonus));
        assert_eq!(g.plot_get_bonus_type(p, Some(team)), None);
        g.teams[team].revealed_bonuses.push(bonus);
        assert_eq!(g.plot_get_bonus_type(p, Some(team)), Some(bonus));
        g.teams[team].revealed_bonuses.clear();
        g.teams[team].has_tech[tech] = true;
        assert_eq!(g.plot_get_bonus_type(p, Some(team)), Some(bonus));
    }

    #[test]
    fn goodies_are_hidden_from_barbarians() {
        let mut g = game_with_map(2, 2);
        let d = g.d;
        let p = PlotIdx(0);
        let goody = d.improvements.iter_enumerated().find(|(_, i)| i.goody).map(|(id, _)| id).expect("a goody improvement");
        let barbarian = g.barbarian_team();

        assert!(!g.plot_is_goody(p, None));
        g.map.plots[p].improvement_type = Some(goody);
        assert!(g.plot_is_goody(p, None));
        assert!(g.plot_is_goody(p, Some(TeamId(0))));
        assert!(!g.plot_is_goody(p, Some(barbarian)));
        assert!(g.plot_is_revealed_goody(p, None));
        assert!(!g.plot_is_revealed_goody(p, Some(barbarian)));
    }

    #[test]
    fn plot_counter_clamps_to_0_100() {
        let mut g = game_with_map(2, 2);
        let d = g.d;
        let p = PlotIdx(0);
        // tundra has no hell terrain step (down 0, up 100): only the clamp applies
        let tundra = d.t::<TerrainType>("TERRAIN_TUNDRA");
        assert_eq!((d.terrains[tundra].plot_counter_down, d.terrains[tundra].plot_counter_up), (0, 100));
        g.map.plots[p].terrain_type = Some(tundra);

        g.plot_change_plot_counter(p, 40);
        assert_eq!(g.map.plots[p].plot_counter, 40);
        g.plot_change_plot_counter(p, 150);
        assert_eq!(g.map.plots[p].plot_counter, 100);
        g.plot_change_plot_counter(p, -250);
        assert_eq!(g.map.plots[p].plot_counter, 0);
        assert_eq!(g.map.plots[p].terrain_type, Some(tundra));
    }

    #[test]
    fn can_have_feature_basic_rules() {
        let mut g = game_with_map(3, 3);
        let d = g.d;
        let p = PlotIdx(4);
        let forest = d.t::<FeatureType>("FEATURE_FOREST");
        let flood_plains = d.t::<FeatureType>("FEATURE_FLOOD_PLAINS");
        let jungle = d.t::<FeatureType>("FEATURE_JUNGLE");

        assert!(g.plot_can_have_feature(p, None));
        assert!(g.plot_can_have_feature(p, Some(forest)));
        // flood plains need desert and a river
        assert!(!g.plot_can_have_feature(p, Some(flood_plains)));
        g.map.plots[p].terrain_type = Some(d.t::<TerrainType>("TERRAIN_DESERT"));
        assert!(!g.plot_can_have_feature(p, Some(flood_plains)));
        g.map.plots[p].river_crossing_count = 1;
        assert!(g.plot_can_have_feature(p, Some(flood_plains)));
        g.map.plots[p].plot_type = PlotType::PLOT_HILLS;
        assert!(!g.plot_can_have_feature(p, Some(flood_plains)));

        // an existing feature, a peak or no terrain block everything
        g.map.plots[p].terrain_type = Some(d.t::<TerrainType>("TERRAIN_GRASS"));
        g.map.plots[p].plot_type = PlotType::PLOT_LAND;
        g.map.plots[p].feature_type = Some(jungle);
        assert!(!g.plot_can_have_feature(p, Some(forest)));
        g.map.plots[p].feature_type = None;
        g.map.plots[p].plot_type = PlotType::PLOT_PEAK;
        assert!(!g.plot_can_have_feature(p, Some(forest)));
        g.map.plots[p].plot_type = PlotType::PLOT_LAND;
        g.map.plots[p].terrain_type = None;
        assert!(!g.plot_can_have_feature(p, Some(forest)));
    }

    #[test]
    fn temp_terrain_on_same_terrain_is_a_no_op() {
        let mut g = game_with_map(2, 2);
        let d = g.d;
        let p = PlotIdx(0);
        g.plot_set_temp_terrain_type(p, d.t::<TerrainType>("TERRAIN_GRASS"), 10);
        assert_eq!(g.map.plots[p].temp_terrain_timer, 0);
        assert_eq!(g.map.plots[p].real_terrain_type, None);
    }

    #[test]
    fn feature_production_needs_a_feature() {
        let g = game_with_map(2, 2);
        let farm = g.d.t::<BuildType>("BUILD_FARM");
        assert_eq!(g.plot_get_feature_production(PlotIdx(0), farm, TeamId(0)), (0, None));
    }

    #[test]
    fn build_progress_accumulates_until_the_build_time() {
        let mut g = game_with_map(2, 2);
        let p = PlotIdx(0);
        let farm = g.d.t::<BuildType>("BUILD_FARM");
        let time = g.plot_get_build_time(p, farm);
        assert!(time > 2);

        assert!(!g.plot_change_build_progress(p, farm, 0, TeamId(0)));
        assert_eq!(g.map.plots[p].build_progress[farm], 0);
        assert!(!g.plot_change_build_progress(p, farm, 1, TeamId(0)));
        assert!(!g.plot_change_build_progress(p, farm, time - 2, TeamId(0)));
        assert_eq!(g.map.plots[p].build_progress[farm], time - 1);
        assert_eq!(g.map.plots[p].improvement_type, None);
    }

    #[test]
    fn can_have_bonus_basic_rules() {
        let mut g = game_with_map(3, 3);
        let d = g.d;
        let p = PlotIdx(4);
        let grass = d.t::<TerrainType>("TERRAIN_GRASS");
        // a bonus valid on grassland flatlands that does not look at rivers
        let bonus = d
            .bonuses
            .iter_enumerated()
            .find(|(_, b)| b.terrain[grass] && b.flatlands && !b.no_river_side && b.min_area_size > 1)
            .map(|(id, _)| id)
            .expect("a grassland bonus");
        let min_area_size = d.bonuses[bonus].min_area_size;

        assert!(g.plot_can_have_bonus(p, None, true));
        // without an area the plot counts as an area of 0 tiles
        g.map.plots[p].potential_city_work = true;
        assert!(!g.plot_can_have_bonus(p, Some(bonus), true));
        let area = g.map_add_area(false);
        g.map.plots[p].area = Some(area);
        g.map.areas[area].num_tiles = min_area_size - 1;
        assert!(!g.plot_can_have_bonus(p, Some(bonus), true));
        g.map.areas[area].num_tiles = min_area_size;
        assert!(g.plot_can_have_bonus(p, Some(bonus), true));
        // only plots a city could work
        g.map.plots[p].potential_city_work = false;
        assert!(!g.plot_can_have_bonus(p, Some(bonus), true));
        g.map.plots[p].potential_city_work = true;
        assert!(g.plot_can_have_bonus(p, Some(bonus), true));
        // peaks and occupied plots never get bonuses
        g.map.plots[p].plot_type = PlotType::PLOT_PEAK;
        assert!(!g.plot_can_have_bonus(p, Some(bonus), true));
        g.map.plots[p].plot_type = PlotType::PLOT_LAND;
        g.map.plots[p].bonus_type = Some(bonus);
        assert!(!g.plot_can_have_bonus(p, Some(bonus), true));
        g.map.plots[p].bonus_type = None;
        // a feature needs the bonus' feature list
        let jungle = d.t::<FeatureType>("FEATURE_JUNGLE");
        g.map.plots[p].feature_type = Some(jungle);
        let jungle_ok = d.bonuses[bonus].feature[jungle] && d.bonuses[bonus].feature_terrain[grass];
        assert_eq!(g.plot_can_have_bonus(p, Some(bonus), true), jungle_ok);
    }
}
