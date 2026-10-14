# Porting map: CvMap, CvPlot, CvArea, CvPlotGroup

**Status: ported and audited.** The structs carry every gameplay data member, constructors
reproduce C++ `reset()`/`init()`, and every rules method is an `impl Game` (or `impl Map`) method
with a complete body (no `todo!` left); each file was compared line by line with the C++ in the
audit wave. C++ line ranges below refer to
`/home/user/ref/ffh2/CvGameCoreDLL/{CvMap,CvPlot,CvArea,CvPlotGroup}.cpp` (FFH2 0.41).

## Files

Each plot topic file was one porting unit; the three small classes formed a fifth. "C++ lines"
counts the bodies of the C++ functions the file holds (comments included).

| Unit | Rust file(s) | Topic | C++ lines | status |
|---|---|---|---:|---|
| 1 | `plot/terrain.rs` | terrain/feature/bonus/improvement/route changes and validity, builds, goodies, `doTurn`/`doFeature`/`doImprovement`, FFH plot counter, temporary terrain, pirate coves | 1818 | ported |
| 2 | `plot/geography.rs` | `setPlotType` and areas, adjacency, latitude, rivers, irrigation, fresh water, potential city work, yields | 1814 | ported |
| 3 | `plot/ownership.rs` | owner/team, culture and culture ranges, city radius, plot/working city, found values, plot groups and the trade network, visibility counts and revealed state | 2154 | ported |
| 4 | `plot/units.rs` | unit list and `plotCount`/`plotCheck` queries, defenders, movement cost, defense, nukes, sight (line of sight, sight sources), events, `canTrain` | 1844 | ported |
| 5 | `map.rs` + `area.rs` + `plot_group.rs` | CvMap (init, areas, whole-map updates, searches, path distance), CvArea, CvPlotGroup | 1410 | ported |

Cross-file dependencies: the plot files call each other's methods freely (e.g. `setOwner` in
unit 3 calls `changeAdjacentSight` in unit 4). CvGameCoreUtils helpers live in `utils.rs`, not in
these files: `plotDirection`, `plotCardinalDirection`, `plotCity`, `directionXY`, `plotDistance`,
`stepDistance`, `dxWrap`/`dyWrap`, `isBeforeUnitCycle`, `finalImprovementUpgrade`,
`getPopulationPower`, `shuffleArray`, the `PUF_*` predicates; the FAStar callbacks
(`areaValid`/`joinArea`, `plotGroupValid`/`countPlotGroup`, `potentialIrrigation`/
`checkFreshWater`/`changeIrrigated`) live in `path.rs`. `plotXY(x, y, dx, dy)` needs no helper:
it is `self.map.plot_xy(x + dx, y + dy)`.

## Structs

| Rust | C++ | fields | not ported |
|---|---|---:|---|
| `map::Map` | `CvMap` | 14 | none (all members; plus `pirate_cove_types`, the static `CvPlot::c_aePirateCoveTypes`) |
| `plot::Plot` | `CvPlot` | 59 | `m_pPlotArea` (pointer cache), `m_iFeatureVariety` (art), `m_bShowCitySymbols`, `m_bFlagDirty`, `m_bPlotLayoutDirty`, `m_bLayoutStateWorked`, symbol/flag/route/river/feature entities, `m_pCenterUnit`, `m_pPlotBuilder`, `m_symbols` |
| `area::Area` | `CvArea` | 29 | none |
| `plot_group::PlotGroup` | `CvPlotGroup` | 4 | none |

FFH2 members on `Plot`: `move_disabled_ai`, `move_disabled_human`, `build_disabled`,
`found_disabled`, `python_active`, `min_level`, `plot_counter`, `portal_exit_x`,
`portal_exit_y`, `temp_terrain_timer`, `real_terrain_type`. 0.41 has a temporary *terrain*
timer only; there are no temporary feature/bonus timers in this version.

AI bookkeeping: CvPlot/CvArea have no AI subclasses and non-AI code calls no `AI_*` method on
them. The AI state that rules code maintains is already in the C++ members, ported as plain
fields: `Area::area_ai_type`, `target_cities`, `num_ai_units`, `num_train_ai_units`,
`best_found_value`, `Plot::found_value`.

Constructors: `Map::new(d, &MapInitData, num_players, num_teams)` (C++ `reset(pInitInfo)` +
plot creation of `init`; areas come later from `map_calculate_areas`), `Plot::new(d, x, y,
num_players, num_teams)`, `Area::new(d, id, water, num_players, num_teams)` (used by
`Game::map_add_area`), `PlotGroup::new(d, id, owner)` (then `plot_group_add_plot`). Each also
has a `with_counts(..)` form taking every array size explicitly, and `Default` mirrors the C++
default constructor. C++ allocates most CvPlot arrays lazily; here they are allocated up
front with the values the C++ getters return for a `NULL` array (0, false, `None`).

## Conventions specific to these classes

- **Names.** `CvPlot::getBestDefender` -> `plot_get_best_defender`, `CvArea::isLake` ->
  `area_is_lake`, `CvPlotGroup::addPlot` -> `plot_group_add_plot`, `CvMap::findCity` ->
  `map_find_city` (C++ name in snake_case, `get`/`is`/`set` kept). CvMap's pure geometry is
  `impl Map`: `self.map.plot_xy(x, y)`, `self.map.is_plot(..)`, `self.map.find_biggest_area(..)`.
- **Map geometry.** `Map::plot_xy(x, y)` = C++ `plot`/`plotINLINE` (wraps with `coordRange`,
  None off-map or for `INVALID_PLOT_COORD`); `Map::plot_idx(x, y)` = in-bounds lookup without
  wrapping; `Map::plot_soren(x, y)` = `plotSorenINLINE`; `Map::xy(p)` = (`getX`, `getY`);
  `Map::plot_ids()` = `for (iI = 0; iI < numPlotsINLINE(); iI++) plotByIndexINLINE(iI)`;
  `map::coord_range` = `coordRange`. `self.map.plots[p]` indexes with a `PlotIdx`.
- **Overloads.** `isAdjacentToArea(int)` / `isAdjacentToArea(const CvArea*)` ->
  `plot_is_adjacent_to_area`; `updatePlotGroup()` -> `plot_update_plot_group`,
  `updatePlotGroup(PlayerTypes, bool)` -> `plot_update_plot_group_player`;
  `updateRiverCrossing()` -> `plot_update_river_crossing`, `updateRiverCrossing(DirectionTypes)`
  -> `plot_update_river_crossing_dir`; `isVisibleEnemyUnit(PlayerTypes)` ->
  `plot_is_visible_enemy_unit`, `isVisibleEnemyUnit(const CvUnit*)` ->
  `plot_is_visible_enemy_unit_for_unit`; `getYield()` / `getYield(e)` -> the `yields` field.
- **Dropped parameters** (graphics/UI only): `bRebuildGraphics` (`setPlotType`,
  `setTerrainType`), `iVariety` (`setFeatureType`, art variety), `bUpdate` (`addUnit`,
  `removeUnit`), `bDisplay` (`calculateYield`; only UI passes true), `bHelp`
  (`defenseModifier`), `bDebug` (`isVisible`, `isAdjacentVisible`, `getRevealedTeam`,
  `isRevealed`, `getRevealedOwner`, `getRevealedImprovementType`, `getRevealedRouteType`):
  debug mode and the active team are UI concepts and never affect rules. Custom map options
  (`getNumCustomMapOptions`, `getCustomMapOption`, `rebuild`'s options) are not ported.
- **Defaults and nullability.** C++ default arguments become explicit parameters (the doc
  comment states the default). A parameter is `Option` only where the C++ gives NO_X/NULL a
  meaning (`getBonusType(NO_TEAM)`, `canHaveBonus(NO_BONUS)`, `isRiverCrossing(NO_DIRECTION)`,
  owner/team filters, ...); NULL-checked pointers that every caller fills (`calculatePathDistance`,
  `canSeePlot`) are required. Out-parameters are returned (`plot_get_feature_production`
  returns `(production, city)`).
- **Constness.** `&self` vs `&mut self` follows what the body does; CvMap/CvArea const-ness is
  unreliable (e.g. `findCity`, `getNumLandAreas` are non-const). `&mut self` despite C++ const:
  `plot_get_inland_corner` (map RNG). `&self` despite C++ non-const: `plot_is_best_adjacent_found`,
  `map_find_city`, `map_find_selection_group`, `map_find_water`, all `Map` geometry.
  `plot_get_found_value` stays `&mut self` (fills the cache).
- **Linked lists.** `CvPlot::m_units` -> `Plot::units: Vec<UnitId>` (`getNumUnits` = `len()`,
  `getUnitByIndex(i)` = `units.get(i)`, `head/next/prev/tailUnitNode` = index walks; copy the
  list before loops that move or kill units, as the C++ does with `oldUnits`).
  `CvPlotGroup::m_plots` -> `PlotGroup::plots: Vec<XYCoords>`; node handles are indices and
  `plot_group_delete_plots_node(pg, i)` returns the index of the next entry (the same `i`)
  or None, deleting the group when it empties. Repeatedly removing index 0 is O(n^2) for big
  groups; draining instead is fine as long as the observable order is unchanged.
- **Dead ids.** C++ `getCity(IDInfo)` / `CvPlayer::getPlotGroup(id)` return NULL for deleted
  objects. `area_get_target_city` and `plot_get_plot_group` filter dead ids the same way.
  `plot_city`, `working_city`, `working_city_override` are kept in sync by
  `setPlotCity`/`updateWorkingCity`/`setWorkingCityOverride` (and city destruction), so they
  are read directly.
- **Area ids and order.** `Map::areas` is a `Slab`: ids are never reused and iteration is in
  creation order. C++ `FFreeListTrashArray` iterates by slot and reuses a deleted area's slot,
  so after `deleteArea` + `addArea` (a single land/water flip in `setPlotType`) C++ visits the
  new area at the old slot while Rust visits it last. This only matters for ties in loops
  over areas (e.g. `findBiggestArea`). After `recalculateAreas` both orders agree.
- **Storage widths.** C++ `short`/`char` members are `i32`. Only `m_aiFoundValue` (AI found
  value cache, `short`) can overflow in C++; `plot_get_found_value` truncates it the same way.
- **Pirate coves.** The C++ static set `CvPlot::c_aePirateCoveTypes` is
  `Map::pirate_cove_types` (sorted), filled by `plot_init_pirate_cove_types` (reads
  `ImprovementInfo::improvement_upgrade`), run by `map_init` when the set is empty (C++ loads
  it once per process).

## Trivial accessors -> fields

Getters/setters without logic (or whose only side effects are UI) are fields; `change*` is
`+=` (C++ `FAssert`s may become `debug_assert!`s).

| C++ | Rust |
|---|---|
| `CvMap::getGridWidth/Height`, `getLandPlots`/`changeLandPlots`, `getOwnedPlots`/`changeOwnedPlots`, `getTopLatitude`, `getBottomLatitude`, `getNextRiverID`/`incrementNextRiverID`, `isWrapX`/`isWrapY` | `map.grid_width`, `grid_height`, `land_plots`, `owned_plots`, `top_latitude`, `bottom_latitude`, `next_river_id`, `wrap_x`, `wrap_y` |
| `CvMap::getNumBonuses`/`changeNumBonuses`, `getNumBonusesOnLand`/`changeNumBonusesOnLand` | `map.num_bonus[b]`, `map.num_bonus_on_land[b]` |
| `CvMap::getWorldSize`, `getClimate`, `getSeaLevel` | `self.setup.world_size`, `climate`, `sea_level` |
| `CvMap::getArea(id)`, `getNumAreas`, `getIndexAfterLastArea`, `firstArea`/`nextArea` | `map.areas.get(a)` / `map.areas[a]`, `map.areas.len()`, `map.areas.next_id()`, `map.areas.iter()` / `ids()` |
| `CvPlot::getX`/`getY`, `area()`/`getArea()` | `plots[p].x`, `.y` (or `map.xy(p)`), `plots[p].area` |
| `CvPlot::getOwner`, `getPlotType`, `getTerrainType`, `getFeatureType`, `getImprovementType`, `getRouteType`, `getRiverNSDirection`/`getRiverWEDirection` | `owner`, `plot_type`, `terrain_type`, `feature_type`, `improvement_type`, `route_type`, `river_ns_direction`, `river_we_direction` |
| `CvPlot::getBonusType(NO_TEAM)`, `isCity()` (no arguments) | `bonus_type`; `plot_city.is_some()` |
| `CvPlot::getPlotCity`, `getWorkingCity`, `getWorkingCityOverride` | `plot_city`, `working_city`, `working_city_override` |
| `CvPlot::get/set/changeImprovementDuration`, `get/set/changeUpgradeProgress`, `get/set/changeForceUnownedTimer`, `getOwnershipDuration`, `getCityRadiusCount`/`changeCityRadiusCount`, `is/setStartingPlot`, `get/setRiverID`, `get/setMinOriginalStartDist`, `get/changeReconCount`, `get/changeRiverCrossingCount` | same-named fields |
| `CvPlot::isNOfRiver`, `isWOfRiver`, `isIrrigated`, `isPotentialCityWork` | `n_of_river`, `w_of_river`, `irrigated`, `potential_city_work` |
| `CvPlot::getYield(e)`, `getCulture(p)`, `setFoundValue(p, v)`, `getPlayerCityRadiusCount(p)`/`changePlayerCityRadiusCount`, `getBuildProgress(b)`, `getCultureRangeCities(p, l)` | `yields[e.idx()]`, `culture[p]`, `found_value[p]`, `player_city_radius_count[p]`, `build_progress[b]`, `culture_range_cities[p][l]` |
| `CvPlot::getVisibilityCount(t)`, `getStolenVisibilityCount(t)`, `getBlockadedCount(t)`, `getInvisibleVisibilityCount(t, i)`/`changeInvisibleVisibilityCount` | `visibility_count[t]`, `stolen_visibility_count[t]`, `blockaded_count[t]`, `invisible_visibility_count[t][i]` |
| `CvPlot::isRevealed(t, _)`, `get/setRevealedOwner`, `get/setRevealedImprovementType`, `get/setRevealedRouteType` (setter side effects are UI only) | `revealed[t]`, `revealed_owner[t]`, `revealed_improvement_type[t]`, `revealed_route_type[t]` |
| `CvPlot::getNumUnits`, `getUnitByIndex`, `head/next/prev/tailUnitNode` | `units` (`Vec<UnitId>`) |
| `CvPlot::get/setScriptData` | `script_data` |
| FFH `CvPlot::is/setMoveDisabledAI`, `is/setMoveDisabledHuman`, `is/setBuildDisabled`, `is/setFoundDisabled`, `is/setPythonActive`, `get/setMinLevel`, `getPlotCounter`, `get/setPortalExitX/Y`, `get/setRealTerrainType`, `getTempTerrainTimer`/`changeTempTerrainTimer` | `move_disabled_ai`, `move_disabled_human`, `build_disabled`, `found_disabled`, `python_active`, `min_level`, `plot_counter`, `portal_exit_x/y`, `real_terrain_type`, `temp_terrain_timer` |
| `CvArea::getID`/`setID`, `getNumTiles`, `get/changeNumOwnedTiles`, `get/changeNumRiverEdges`, `getNumUnits`, `getNumCities`, `getTotalPopulation`, `get/changeNumStartingPlots`, `isWater` | `id`, `num_tiles`, `num_owned_tiles`, `num_river_edges`, `num_units`, `num_cities`, `total_population`, `num_starting_plots`, `water` |
| `CvArea::getUnitsPerPlayer`, `get/changeAnimalsPerPlayer`, `getCitiesPerPlayer`, `getPopulationPerPlayer`, `getBuildingGoodHealth`, `getBuildingBadHealth`, `getBuildingHappiness`, `getFreeSpecialist`, `get/changePower`, `get/setBestFoundValue` | `units_per_player[p]`, `animals_per_player[p]`, `cities_per_player[p]`, `population_per_player[p]`, `building_good_health[p]`, `building_bad_health[p]`, `building_happiness[p]`, `free_specialist[p]`, `power[p]`, `best_found_value[p]` |
| `CvArea::get/changeNumRevealedTiles`, `getCleanPowerCount`, `getBorderObstacleCount`, `get/setAreaAIType`, `setTargetCity` | `num_revealed_tiles[t]`, `clean_power_count[t]`, `border_obstacle_count[t]`, `area_ai_type[t]`, `target_cities[p]` |
| `CvArea::getYieldRateModifier`, `get/changeNumTrainAIUnits`, `get/changeNumAIUnits`, `get/changeNumBonuses`, `get/changeNumImprovements` | `yield_rate_modifier[p][y.idx()]`, `num_train_ai_units[p][ai]`, `num_ai_units[p][ai]`, `num_bonuses[b]`, `num_improvements[i]` |
| `CvPlotGroup::getID`/`setID`, `getOwner`, `getNumBonuses`, `insertAtEndPlots`, `getLengthPlots`, `headPlotsNode`/`nextPlotsNode` | `id`, `owner`, `num_bonuses[b]`, `plots.push(xy)`, `plots.len()`, indices `0`, `i + 1` |

## Functions per Rust file

Status: `ported` = body done (all of them). Rust names without a type prefix
are `impl Game` methods.

### `map.rs` (37 C++ functions, 801 C++ lines, all ported)

| C++ | lines | # | Rust | status |
|---|---|---:|---|---|
| `CvMap::init` | 63-102 | 40 | `map_init` | ported |
| `CvMap::reset` | 117-238 | 122 | `Map::new` | ported (init-data branch; the `NULL` branch (world size, map script) is in `map_init`) |
| `CvMap::erasePlots` | 277-285 | 9 | `map_erase_plots` | ported |
| `CvMap::setRevealedPlots` | 288-300 | 13 | `map_set_revealed_plots` | ported |
| `CvMap::setAllPlotTypes` | 303-323 | 21 | `map_set_all_plot_types` | ported |
| `CvMap::doTurn` | 327-337 | 11 | `map_do_turn` | ported |
| `CvMap::updateSight` | 415-423 | 9 | `map_update_sight` | ported |
| `CvMap::updateIrrigated` | 426-434 | 9 | `map_update_irrigated` | ported |
| `CvMap::updateWorkingCity` | 448-456 | 9 | `map_update_working_city` | ported |
| `CvMap::updateMinOriginalStartDist` | 459-510 | 52 | `map_update_min_original_start_dist` | ported |
| `CvMap::updateYield` | 513-521 | 9 | `map_update_yield` | ported |
| `CvMap::verifyUnitValidPlot` | 524-530 | 7 | `map_verify_unit_valid_plot` | ported |
| `CvMap::combinePlotGroups` | 533-566 | 34 | `map_combine_plot_groups` | ported |
| `CvMap::syncRandPlot` | 568-701 | 134 | `map_sync_rand_plot` | ported |
| `CvMap::findCity` | 704-759 | 56 | `map_find_city` | ported |
| `CvMap::findSelectionGroup` | 762-803 | 42 | `map_find_selection_group` | ported |
| `CvMap::findBiggestArea` | 806-832 | 27 | `Map::find_biggest_area` | ported |
| `CvMap::getMapFractalFlags` | 835-850 | 16 | `Map::get_map_fractal_flags` | ported |
| `CvMap::findWater` | 854-888 | 35 | `map_find_water` | ported |
| `CvMap::isPlot` | 891-894 | 4 | `Map::is_plot` | ported |
| `CvMap::numPlots` | 897-900 | 4 | `Map::num_plots` | ported |
| `CvMap::plotNum` | 903-906 | 4 | `Map::plot_num` | ported |
| `CvMap::plotX` | 909-912 | 4 | `Map::plot_x` | ported |
| `CvMap::plotY` | 915-918 | 4 | `Map::plot_y` | ported |
| `CvMap::maxPlotDistance` | 965-968 | 4 | `Map::max_plot_distance` | ported |
| `CvMap::maxStepDistance` | 971-974 | 4 | `Map::max_step_distance` | ported |
| `CvMap::isWrap` | 1050-1053 | 4 | `Map::is_wrap` | ported |
| `CvMap::plotByIndex` | 1120-1123 | 4 | `Map::plot_by_index` | ported |
| `CvMap::plot` | 1126-1129 | 4 | `Map::plot_xy` | ported |
| `CvMap::getNumLandAreas` | 1150-1167 | 18 | `Map::get_num_land_areas` | ported |
| `CvMap::addArea` | 1176-1179 | 4 | `map_add_area` | ported |
| `CvMap::deleteArea` | 1182-1185 | 4 | `map_delete_area` | ported |
| `CvMap::recalculateAreas` | 1200-1214 | 15 | `map_recalculate_areas` | ported |
| `CvMap::resetPathDistance` | 1217-1220 | 4 | `map_reset_path_distance` | ported |
| `CvMap::calculatePathDistance` | 1223-1243 | 21 | `map_calculate_path_distance` | ported |
| `CvMap::rebuild` | 1327-1339 | 13 | `map_rebuild` | ported |
| `CvMap::calculateAreas` | 1346-1372 | 27 | `map_calculate_areas` | ported |

### `area.rs` (25 C++ functions, 415 C++ lines, all ported)

| C++ | lines | # | Rust | status |
|---|---|---:|---|---|
| `CvArea::init` | 99-110 | 12 | `Area::new` | ported |
| `CvArea::reset` | 122-202 | 81 | `Area::with_counts` | ported |
| `CvArea::calculateTotalBestNatureYield` | 217-236 | 20 | `area_calculate_total_best_nature_yield` | ported |
| `CvArea::countCoastalLand` | 239-266 | 28 | `area_count_coastal_land` | ported |
| `CvArea::countNumUniqueBonusTypes` | 269-288 | 20 | `area_count_num_unique_bonus_types` | ported |
| `CvArea::countHasReligion` | 291-321 | 31 | `area_count_has_religion` | ported |
| `CvArea::countHasCorporation` | 323-349 | 27 | `area_count_has_corporation` | ported |
| `CvArea::isLake` | 358-361 | 4 | `area_is_lake` | ported |
| `CvArea::changeNumTiles` | 364-381 | 18 | `area_change_num_tiles` | ported |
| `CvArea::getNumUnownedTiles` | 390-393 | 4 | `area_get_num_unowned_tiles` | ported |
| `CvArea::changeUnitsPerPlayer` | 462-470 | 9 | `area_change_units_per_player` | ported |
| `CvArea::changeCitiesPerPlayer` | 498-506 | 9 | `area_change_cities_per_player` | ported |
| `CvArea::changePopulationPerPlayer` | 517-525 | 9 | `area_change_population_per_player` | ported |
| `CvArea::changeBuildingGoodHealth` | 536-548 | 13 | `area_change_building_good_health` | ported |
| `CvArea::changeBuildingBadHealth` | 559-571 | 13 | `area_change_building_bad_health` | ported |
| `CvArea::changeBuildingHappiness` | 582-593 | 12 | `area_change_building_happiness` | ported |
| `CvArea::changeFreeSpecialist` | 604-616 | 13 | `area_change_free_specialist` | ported |
| `CvArea::getNumUnrevealedTiles` | 661-664 | 4 | `area_get_num_unrevealed_tiles` | ported |
| `CvArea::isCleanPower` | 684-687 | 4 | `area_is_clean_power` | ported |
| `CvArea::changeCleanPowerCount` | 690-714 | 25 | `area_change_clean_power_count` | ported |
| `CvArea::isBorderObstacle` | 724-727 | 4 | `area_is_border_obstacle` | ported |
| `CvArea::changeBorderObstacleCount` | 730-741 | 12 | `area_change_border_obstacle_count` | ported |
| `CvArea::getTargetCity` | 761-766 | 6 | `area_get_target_city` | ported |
| `CvArea::changeYieldRateModifier` | 795-820 | 26 | `area_change_yield_rate_modifier` | ported |
| `CvArea::getNumTotalBonuses` | 873-883 | 11 | `area_get_num_total_bonuses` | ported |

### `plot_group.rs` (8 C++ functions, 194 C++ lines, all ported)

| C++ | lines | # | Rust | status |
|---|---|---:|---|---|
| `CvPlotGroup::init` | 29-41 | 13 | `PlotGroup::new` | ported |
| `CvPlotGroup::reset` | 53-73 | 21 | `PlotGroup::with_counts` | ported |
| `CvPlotGroup::addPlot` | 76-86 | 11 | `plot_group_add_plot` | ported |
| `CvPlotGroup::removePlot` | 89-109 | 21 | `plot_group_remove_plot` | ported |
| `CvPlotGroup::recalculatePlots` | 112-174 | 63 | `plot_group_recalculate_plots` | ported |
| `CvPlotGroup::hasBonus` | 203-206 | 4 | `plot_group_has_bonus` | ported |
| `CvPlotGroup::changeNumBonuses` | 209-256 | 48 | `plot_group_change_num_bonuses` | ported |
| `CvPlotGroup::deletePlotsNode` | 265-277 | 13 | `plot_group_delete_plots_node` | ported |

### `plot/mod.rs` (2 C++ functions, 79 C++ lines, all ported)

| C++ | lines | # | Rust | status |
|---|---|---:|---|---|
| `CvPlot::init` | 106-117 | 12 | `Plot::new` | ported |
| `CvPlot::reset` | 177-243 | 67 | `Plot::with_counts` | ported |

### `plot/terrain.rs` (36 C++ functions, 1818 C++ lines, all ported)

| C++ | lines | # | Rust | status |
|---|---|---:|---|---|
| `CvPlot::erase` | 275-327 | 53 | `plot_erase` | ported |
| `CvPlot::doTurn` | 408-527 | 120 | `plot_do_turn` | ported |
| `CvPlot::doImprovement` | 530-600 | 71 | `plot_do_improvement` | ported |
| `CvPlot::doImprovementUpgrade` | 602-654 | 53 | `plot_do_improvement_upgrade` | ported |
| `CvPlot::canHaveBonus` | 2168-2270 | 103 | `plot_can_have_bonus` | ported |
| `CvPlot::canHaveImprovement` | 2273-2424 | 152 | `plot_can_have_improvement` | ported |
| `CvPlot::canBuild` | 2427-2588 | 162 | `plot_can_build` | ported |
| `CvPlot::getBuildTime` | 2591-2614 | 24 | `plot_get_build_time` | ported |
| `CvPlot::getBuildTurnsLeft` | 2617-2668 | 52 | `plot_get_build_turns_left` | ported |
| `CvPlot::getFeatureProduction` | 2671-2710 | 40 | `plot_get_feature_production` | ported |
| `CvPlot::isGoody` | 3481-3489 | 9 | `plot_is_goody` | ported |
| `CvPlot::isRevealedGoody` | 3492-3535 | 44 | `plot_is_revealed_goody` | ported |
| `CvPlot::removeGoody` | 3538-3541 | 4 | `plot_remove_goody` | ported |
| `CvPlot::canHaveFeature` | 3717-3786 | 70 | `plot_can_have_feature` | ported |
| `CvPlot::isRoute` | 3789-3792 | 4 | `plot_is_route` | ported |
| `CvPlot::isImpassable` | 4027-4040 | 14 | `plot_is_impassable` | ported |
| `CvPlot::getUpgradeTimeLeft` | 4309-4337 | 29 | `plot_get_upgrade_time_left` | ported |
| `CvPlot::isWater` | 5070-5073 | 4 | `plot_is_water` | ported |
| `CvPlot::isFlatlands` | 5076-5079 | 4 | `plot_is_flatlands` | ported |
| `CvPlot::isHills` | 5082-5085 | 4 | `plot_is_hills` | ported |
| `CvPlot::isPeak` | 5088-5091 | 4 | `plot_is_peak` | ported |
| `CvPlot::setTerrainType` | 5344-5390 | 47 | `plot_set_terrain_type` | ported |
| `CvPlot::setFeatureType` | 5399-5486 | 88 | `plot_set_feature_type` | ported |
| `CvPlot::getBonusType` | 5535-5549 | 15 | `plot_get_bonus_type` | ported |
| `CvPlot::getNonObsoleteBonusType` | 5552-5566 | 15 | `plot_get_non_obsolete_bonus_type` | ported |
| `CvPlot::setBonusType` | 5569-5611 | 43 | `plot_set_bonus_type` | ported |
| `CvPlot::setImprovementType` | 5620-5740 | 121 | `plot_set_improvement_type` | ported |
| `CvPlot::setRouteType` | 5749-5792 | 44 | `plot_set_route_type` | ported |
| `CvPlot::updateCityRoute` | 5795-5812 | 18 | `plot_update_city_route` | ported |
| `CvPlot::changeBuildProgress` | 7875-7962 | 88 | `plot_change_build_progress` | ported |
| `CvPlot::doFeature` | 8648-8834 | 187 | `plot_do_feature` | ported |
| `CvPlot::changePlotCounter` | 10673-10705 | 33 | `plot_change_plot_counter` | ported |
| `CvPlot::isBuilding` | 10740-10775 | 36 | `plot_is_building` | ported |
| `CvPlot::setTempTerrainType` | 10845-10856 | 12 | `plot_set_temp_terrain_type` | ported |
| `CvPlot::isPirateCove` | 10898-10901 | 4 | `plot_is_pirate_cove` | ported |
| `CvPlot::isPirateCoveValid` | 10903-10949 | 47 | `plot_is_pirate_cove_valid` | ported |

### `plot/geography.rs` (44 C++ functions, 1814 C++ lines, all ported)

| C++ | lines | # | Rust | status |
|---|---|---:|---|---|
| `CvPlot::isAdjacentToArea` | 1256-1277 | 22 | `plot_is_adjacent_to_area` | ported |
| `CvPlot::isAdjacentToArea` | 1279-1282 | 4 | `plot_is_adjacent_to_area` | ported |
| `CvPlot::shareAdjacentArea` | 1285-1317 | 33 | `plot_share_adjacent_area` | ported |
| `CvPlot::isAdjacentToLand` | 1320-1341 | 22 | `plot_is_adjacent_to_land` | ported |
| `CvPlot::isCoastalLand` | 1344-1373 | 30 | `plot_is_coastal_land` | ported |
| `CvPlot::isLake` | 1417-1426 | 10 | `plot_is_lake` | ported |
| `CvPlot::isFreshWater` | 1431-1476 | 46 | `plot_is_fresh_water` | ported |
| `CvPlot::isPotentialIrrigation` | 1479-1490 | 12 | `plot_is_potential_irrigation` | ported |
| `CvPlot::canHavePotentialIrrigation` | 1493-1514 | 22 | `plot_can_have_potential_irrigation` | ported |
| `CvPlot::isIrrigationAvailable` | 1517-1546 | 30 | `plot_is_irrigation_available` | ported |
| `CvPlot::isRiverSide` | 1613-1632 | 20 | `plot_is_river_side` | ported |
| `CvPlot::isRiver` | 1635-1638 | 4 | `plot_is_river` | ported |
| `CvPlot::isRiverConnection` | 1641-1688 | 48 | `plot_is_river_connection` | ported |
| `CvPlot::getNearestLandPlotInternal` | 1691-1716 | 26 | `plot_get_nearest_land_plot_internal` | ported |
| `CvPlot::getNearestLandArea` | 1719-1723 | 5 | `plot_get_nearest_land_area` | ported |
| `CvPlot::getNearestLandPlot` | 1726-1729 | 4 | `plot_get_nearest_land_plot` | ported |
| `CvPlot::at` | 4055-4058 | 4 | `plot_at` | ported |
| `CvPlot::getLatitude` | 4061-4103 | 43 | `plot_get_latitude` | ported |
| `CvPlot::waterArea` | 4123-4159 | 37 | `plot_water_area` | ported |
| `CvPlot::secondWaterArea` | 4161-4197 | 37 | `plot_second_water_area` | ported |
| `CvPlot::setArea` | 4206-4230 | 25 | `plot_set_area` | ported |
| `CvPlot::setNOfRiver` | 4415-4453 | 39 | `plot_set_n_of_river` | ported |
| `CvPlot::setWOfRiver` | 4462-4500 | 39 | `plot_set_w_of_river` | ported |
| `CvPlot::getInlandCorner` | 4518-4549 | 32 | `plot_get_inland_corner` | ported |
| `CvPlot::hasCoastAtSECorner` | 4552-4580 | 29 | `plot_has_coast_at_se_corner` | ported |
| `CvPlot::setIrrigated` | 4589-4612 | 24 | `plot_set_irrigated` | ported |
| `CvPlot::updateIrrigated` | 4615-4674 | 60 | `plot_update_irrigated` | ported |
| `CvPlot::isPotentialCityWorkForArea` | 4683-4707 | 25 | `plot_is_potential_city_work_for_area` | ported |
| `CvPlot::updatePotentialCityWork` | 4710-4744 | 35 | `plot_update_potential_city_work` | ported |
| `CvPlot::setPlotType` | 5094-5335 | 242 | `plot_set_plot_type` | ported |
| `CvPlot::calculateNatureYield` | 6116-6184 | 69 | `plot_calculate_nature_yield` | ported |
| `CvPlot::calculateBestNatureYield` | 6187-6190 | 4 | `plot_calculate_best_nature_yield` | ported |
| `CvPlot::calculateTotalBestNatureYield` | 6193-6196 | 4 | `plot_calculate_total_best_nature_yield` | ported |
| `CvPlot::calculateImprovementYieldChange` | 6204-6285 | 82 | `plot_calculate_improvement_yield_change` | ported |
| `CvPlot::calculateYield` | 6369-6518 | 150 | `plot_calculate_yield` | ported |
| `CvPlot::hasYield` | 6521-6534 | 14 | `plot_has_yield` | ported |
| `CvPlot::updateYield` | 6537-6583 | 47 | `plot_update_yield` | ported |
| `CvPlot::isRiverCrossing` | 7409-7424 | 16 | `plot_is_river_crossing` | ported |
| `CvPlot::updateRiverCrossing` | 7427-7568 | 142 | `plot_update_river_crossing_dir` | ported |
| `CvPlot::updateRiverCrossing` | 7571-7579 | 9 | `plot_update_river_crossing` | ported |
| `CvPlot::processArea` | 8943-9053 | 111 | `plot_process_area` | ported |
| `CvPlot::calculateMaxYield` | 9726-9783 | 58 | `plot_calculate_max_yield` | ported |
| `CvPlot::getYieldWithBuild` | 9785-9861 | 77 | `plot_get_yield_with_build` | ported |
| `CvPlot::isAdjacentToWater` | 10717-10738 | 22 | `plot_is_adjacent_to_water` | ported |

### `plot/ownership.rs` (68 C++ functions, 2154 C++ lines, all ported)

| C++ | lines | # | Rust | status |
|---|---|---:|---|---|
| `CvPlot::getTeam` | 395-405 | 11 | `plot_get_team` | ported |
| `CvPlot::updateCulture` | 656-662 | 7 | `plot_update_culture` | ported |
| `CvPlot::isConnectedTo` | 1118-1122 | 5 | `plot_is_connected_to` | ported |
| `CvPlot::isConnectedToCapital` | 1125-1145 | 21 | `plot_is_connected_to_capital` | ported |
| `CvPlot::getPlotGroupConnectedBonus` | 1148-1165 | 18 | `plot_get_plot_group_connected_bonus` | ported |
| `CvPlot::isPlotGroupConnectedBonus` | 1168-1171 | 4 | `plot_is_plot_group_connected_bonus` | ported |
| `CvPlot::isAdjacentPlotGroupConnectedBonus` | 1174-1193 | 20 | `plot_is_adjacent_plot_group_connected_bonus` | ported |
| `CvPlot::updatePlotGroupBonus` | 1196-1253 | 58 | `plot_update_plot_group_bonus` | ported |
| `CvPlot::isWithinTeamCityRadius` | 1390-1414 | 25 | `plot_is_within_team_city_radius` | ported |
| `CvPlot::isAdjacentOwned` | 3032-3051 | 20 | `plot_is_adjacent_owned` | ported |
| `CvPlot::isAdjacentPlayer` | 3054-3076 | 23 | `plot_is_adjacent_player` | ported |
| `CvPlot::isAdjacentTeam` | 3079-3101 | 23 | `plot_is_adjacent_team` | ported |
| `CvPlot::isWithinCultureRange` | 3104-3117 | 14 | `plot_is_within_culture_range` | ported |
| `CvPlot::getNumCultureRangeCities` | 3120-3133 | 14 | `plot_get_num_culture_range_cities` | ported |
| `CvPlot::calculateCulturalOwner` | 3136-3265 | 130 | `plot_calculate_cultural_owner` | ported |
| `CvPlot::isOwned` | 3355-3358 | 4 | `plot_is_owned` | ported |
| `CvPlot::isBarbarian` | 3361-3364 | 4 | `plot_is_barbarian` | ported |
| `CvPlot::isVisible` | 3373-3388 | 16 | `plot_is_visible` | ported |
| `CvPlot::isVisibleToCivTeam` | 3397-3413 | 17 | `plot_is_visible_to_civ_team` | ported |
| `CvPlot::isVisibleToWatchingHuman` | 3416-3435 | 20 | `plot_is_visible_to_watching_human` | ported |
| `CvPlot::isAdjacentVisible` | 3438-3457 | 20 | `plot_is_adjacent_visible` | ported |
| `CvPlot::isAdjacentNonvisible` | 3459-3478 | 20 | `plot_is_adjacent_nonvisible` | ported |
| `CvPlot::isCity` | 3544-3558 | 15 | `plot_is_city` | ported |
| `CvPlot::isOccupation` | 3617-3629 | 13 | `plot_is_occupation` | ported |
| `CvPlot::isBeingWorked` | 3632-3644 | 13 | `plot_is_being_worked` | ported |
| `CvPlot::isTradeNetworkImpassable` | 3809-3812 | 4 | `plot_is_trade_network_impassable` | ported |
| `CvPlot::isRiverNetwork` | 3814-3832 | 19 | `plot_is_river_network` | ported |
| `CvPlot::isNetworkTerrain` | 3834-3853 | 20 | `plot_is_network_terrain` | ported |
| `CvPlot::isBonusNetwork` | 3856-3874 | 19 | `plot_is_bonus_network` | ported |
| `CvPlot::isTradeNetwork` | 3877-3905 | 29 | `plot_is_trade_network` | ported |
| `CvPlot::isTradeNetworkConnected` | 3908-3987 | 80 | `plot_is_trade_network_connected` | ported |
| `CvPlot::isOwnershipScore` | 4247-4250 | 4 | `plot_is_ownership_score` | ported |
| `CvPlot::setOwnershipDuration` | 4253-4275 | 23 | `plot_set_ownership_duration` | ported |
| `CvPlot::changeOwnershipDuration` | 4278-4281 | 4 | `plot_change_ownership_duration` | ported |
| `CvPlot::isForceUnowned` | 4359-4362 | 4 | `plot_is_force_unowned` | ported |
| `CvPlot::isCityRadius` | 4384-4387 | 4 | `plot_is_city_radius` | ported |
| `CvPlot::setOwner` | 4812-5061 | 250 | `plot_set_owner` | ported |
| `CvPlot::setPlotCity` | 5821-5899 | 79 | `plot_set_plot_city` | ported |
| `CvPlot::updateWorkingCity` | 5908-6024 | 117 | `plot_update_working_city` | ported |
| `CvPlot::setWorkingCityOverride` | 6033-6049 | 17 | `plot_set_working_city_override` | ported |
| `CvPlot::countTotalCulture` | 6600-6616 | 17 | `plot_count_total_culture` | ported |
| `CvPlot::findHighestCultureTeam` | 6619-6629 | 11 | `plot_find_highest_culture_team` | ported |
| `CvPlot::findHighestCulturePlayer` | 6632-6652 | 21 | `plot_find_highest_culture_player` | ported |
| `CvPlot::calculateCulturePercent` | 6655-6667 | 13 | `plot_calculate_culture_percent` | ported |
| `CvPlot::calculateTeamCulturePercent` | 6670-6689 | 20 | `plot_calculate_team_culture_percent` | ported |
| `CvPlot::setCulture` | 6692-6728 | 37 | `plot_set_culture` | ported |
| `CvPlot::changeCulture` | 6731-6737 | 7 | `plot_change_culture` | ported |
| `CvPlot::getFoundValue` | 6740-6774 | 35 | `plot_get_found_value` | ported |
| `CvPlot::isBestAdjacentFound` | 6777-6804 | 28 | `plot_is_best_adjacent_found` | ported |
| `CvPlot::isPlayerCityRadius` | 6843-6846 | 4 | `plot_is_player_city_radius` | ported |
| `CvPlot::getPlotGroup` | 6871-6882 | 12 | `plot_get_plot_group` | ported |
| `CvPlot::getOwnerPlotGroup` | 6885-6893 | 9 | `plot_get_owner_plot_group` | ported |
| `CvPlot::setPlotGroup` | 6896-6963 | 68 | `plot_set_plot_group` | ported |
| `CvPlot::updatePlotGroup` | 6966-6979 | 14 | `plot_update_plot_group` | ported |
| `CvPlot::updatePlotGroup` | 6982-7082 | 101 | `plot_update_plot_group_player` | ported |
| `CvPlot::changeVisibilityCount` | 7099-7185 | 87 | `plot_change_visibility_count` | ported |
| `CvPlot::changeStolenVisibilityCount` | 7202-7248 | 47 | `plot_change_stolen_visibility_count` | ported |
| `CvPlot::changeBlockadedCount` | 7264-7290 | 27 | `plot_change_blockaded_count` | ported |
| `CvPlot::getRevealedTeam` | 7313-7328 | 16 | `plot_get_revealed_team` | ported |
| `CvPlot::updateRevealedOwner` | 7366-7406 | 41 | `plot_update_revealed_owner` | ported |
| `CvPlot::setRevealed` | 7601-7721 | 121 | `plot_set_revealed` | ported |
| `CvPlot::isAdjacentRevealed` | 7723-7742 | 20 | `plot_is_adjacent_revealed` | ported |
| `CvPlot::isAdjacentNonrevealed` | 7744-7763 | 20 | `plot_is_adjacent_nonrevealed` | ported |
| `CvPlot::isCultureRangeCity` | 8367-8370 | 4 | `plot_is_culture_range_city` | ported |
| `CvPlot::changeCultureRangeCities` | 8373-8411 | 39 | `plot_change_culture_range_cities` | ported |
| `CvPlot::isInvisibleVisible` | 8434-8437 | 4 | `plot_is_invisible_visible` | ported |
| `CvPlot::doCulture` | 8837-8940 | 104 | `plot_do_culture` | ported |
| `CvPlot::countFriendlyCulture` | 10416-10434 | 19 | `plot_count_friendly_culture` | ported |

### `plot/units.rs` (47 C++ functions, 1844 C++ lines, all ported)

| C++ | lines | # | Rust | status |
|---|---|---:|---|---|
| `CvPlot::verifyUnitValidPlot` | 905-998 | 94 | `plot_verify_unit_valid_plot` | ported |
| `CvPlot::nukeExplosion` | 1001-1115 | 115 | `plot_nuke_explosion` | ported |
| `CvPlot::seeFromLevel` | 1732-1761 | 30 | `plot_see_from_level` | ported |
| `CvPlot::seeThroughLevel` | 1764-1793 | 30 | `plot_see_through_level` | ported |
| `CvPlot::changeAdjacentSight` | 1797-1869 | 73 | `plot_change_adjacent_sight` | ported |
| `CvPlot::canSeePlot` | 1871-1903 | 33 | `plot_can_see_plot` | ported |
| `CvPlot::canSeeDisplacementPlot` | 1905-1980 | 76 | `plot_can_see_displacement_plot` | ported |
| `CvPlot::shouldProcessDisplacementPlot` | 1982-2033 | 52 | `plot_should_process_displacement_plot` | ported |
| `CvPlot::updateSight` | 2035-2137 | 103 | `plot_update_sight` | ported |
| `CvPlot::updateSeeFromSight` | 2140-2165 | 26 | `plot_update_see_from_sight` | ported |
| `CvPlot::getBestDefender` | 2713-2789 | 77 | `plot_get_best_defender` | ported |
| `CvPlot::getUnitPower` | 2856-2878 | 23 | `plot_get_unit_power` | ported |
| `CvPlot::defenseModifier` | 2881-2935 | 55 | `plot_defense_modifier` | ported |
| `CvPlot::movementCost` | 2938-3019 | 82 | `plot_movement_cost` | ported |
| `CvPlot::getExtraMovePathCost` | 3021-3024 | 4 | `plot_get_extra_move_path_cost` | ported |
| `CvPlot::changeExtraMovePathCost` | 3027-3030 | 4 | `plot_change_extra_move_path_cost` | ported |
| `CvPlot::plotAction` | 3268-3288 | 21 | `plot_plot_action` | ported |
| `CvPlot::plotCount` | 3291-3322 | 32 | `plot_plot_count` | ported |
| `CvPlot::plotCheck` | 3324-3352 | 29 | `plot_plot_check` | ported |
| `CvPlot::isFriendlyCity` | 3561-3601 | 41 | `plot_is_friendly_city` | ported |
| `CvPlot::isEnemyCity` | 3604-3614 | 11 | `plot_is_enemy_city` | ported |
| `CvPlot::isUnit` | 3647-3650 | 4 | `plot_is_unit` | ported |
| `CvPlot::isInvestigate` | 3653-3656 | 4 | `plot_is_investigate` | ported |
| `CvPlot::isVisibleEnemyDefender` | 3659-3662 | 4 | `plot_is_visible_enemy_defender` | ported |
| `CvPlot::getVisibleEnemyDefender` | 3665-3668 | 4 | `plot_get_visible_enemy_defender` | ported |
| `CvPlot::getNumDefenders` | 3671-3674 | 4 | `plot_get_num_defenders` | ported |
| `CvPlot::getNumVisibleEnemyDefenders` | 3677-3680 | 4 | `plot_get_num_visible_enemy_defenders` | ported |
| `CvPlot::getNumVisiblePotentialEnemyDefenders` | 3683-3686 | 4 | `plot_get_num_visible_potential_enemy_defenders` | ported |
| `CvPlot::isVisibleEnemyUnit` | 3689-3692 | 4 | `plot_is_visible_enemy_unit` | ported |
| `CvPlot::getNumVisibleUnits` | 3694-3697 | 4 | `plot_get_num_visible_units` | ported |
| `CvPlot::isVisibleEnemyUnit` | 3700-3703 | 4 | `plot_is_visible_enemy_unit_for_unit` | ported |
| `CvPlot::isVisibleOtherUnit` | 3705-3708 | 4 | `plot_is_visible_other_unit` | ported |
| `CvPlot::isFighting` | 3711-3714 | 4 | `plot_is_fighting` | ported |
| `CvPlot::isValidRoute` | 3795-3806 | 12 | `plot_is_valid_route` | ported |
| `CvPlot::isValidDomainForLocation` | 3990-3998 | 9 | `plot_is_valid_domain_for_location` | ported |
| `CvPlot::isValidDomainForAction` | 4001-4024 | 24 | `plot_is_valid_domain_for_action` | ported |
| `CvPlot::addUnit` | 8507-8543 | 37 | `plot_add_unit` | ported |
| `CvPlot::removeUnit` | 8546-8572 | 27 | `plot_remove_unit` | ported |
| `CvPlot::canTrigger` | 9863-10031 | 169 | `plot_can_trigger` | ported |
| `CvPlot::canApplyEvent` | 10033-10107 | 75 | `plot_can_apply_event` | ported |
| `CvPlot::applyEvent` | 10109-10169 | 61 | `plot_apply_event` | ported |
| `CvPlot::canTrain` | 10171-10414 | 244 | `plot_can_train` | ported |
| `CvPlot::countNumAirUnits` | 10436-10453 | 18 | `plot_count_num_air_units` | ported |
| `CvPlot::airUnitSpaceAvailable` | 10455-10470 | 16 | `plot_air_unit_space_available` | ported |
| `CvPlot::isEspionageCounterSpy` | 10473-10491 | 19 | `plot_is_espionage_counter_spy` | ported |
| `CvPlot::getNumAnimalUnits` | 10650-10666 | 17 | `plot_get_num_animal_units` | ported |
| `CvPlot::getRangeDefense` | 10777-10833 | 57 | `plot_get_range_defense` | ported |

## Not ported

Graphics, UI, sound and serialization functions, and the AI-only `AI_*` helpers of CvPlot
(their only callers are CvUnitAI/CvSelectionGroupAI/CvGameTextMgr; the new AI may
re-implement them). Calls to these from rules code are simply dropped.

- **CvMap** (21 functions, 221 lines): `uninit` 105-113, `setup` 243-254, `setupGraphical` 260-274, `updateFlagSymbols` 340-357, `updateFog` 360-368, `updateVisibility` 371-379, `updateSymbolVisibility` 382-390, `updateSymbols` 393-401, `updateMinimapColor` 404-412, `updateCenterUnit` 437-445, `pointXToPlotX` 921-926, `plotXToPointX` 929-934, `pointYToPlotY` 937-942, `plotYToPointY` 945-950, `getWidthCoords` 953-956, `getHeightCoords` 959-962, `getNumCustomMapOptions` 1074-1077, `getCustomMapOption` 1080-1083, `pointToPlot` 1132-1135, `read` 1249-1288, `write` 1293-1321
- **CvPlot** (72 functions, 1867 lines): `uninit` 120-173, `setupGraphical` 249-264, `updateGraphicEra` 266-273, `getPointX` 330-333, `getPointY` 336-339, `getPoint` 342-353, `getSymbolSize` 356-380, `getSymbolOffsetX` 383-386, `getSymbolOffsetY` 389-392, `updateFog` 665-703, `updateVisibility` 706-726, `updateSymbolDisplay` 729-759, `updateSymbolVisibility` 762-792, `updateSymbols` 795-847, `updateMinimapColor` 850-860, `updateCenterUnit` 863-902, `isVisibleWorked` 1376-1387, `isRiverMask` 1549-1576, `isRiverCrossingFlowClockwise` 1579-1610, `AI_sumStrength` 2792-2831, `getSelectedUnit` 2834-2853, `isRevealedBarbarian` 3367-3370, `isActiveVisible` 3391-3394, `getFOWIndex` 4106-4109, `getFeatureVariety` 4233-4238, `isShowCitySymbols` 4747-4750, `updateShowCitySymbols` 4753-4791, `isFlagDirty` 4794-4797, `setFlagDirty` 4800-4803, `setFeatureDummyVisibility` 5488-5495, `addFeatureDummyModel` 5497-5504, `setFeatureDummyTexture` 5506-5513, `pickFeatureDummyTag` 5515-5524, `resetFeatureModel` 5526-5533, `updateFeatureSymbolVisibility` 7965-7990, `updateFeatureSymbol` 7993-8031, `getRouteSymbol` 8034-8037, `updateRouteSymbol` 8041-8089, `getRiverSymbol` 8092-8095, `getFeatureSymbol` 8098-8101, `updateRiverSymbol` 8104-8172, `updateRiverSymbolArt` 8175-8190, `getFlagSymbol` 8193-8196, `getFlagSymbolOffset` 8198-8201, `updateFlagSymbol` 8203-8295, `getCenterUnit` 8298-8301, `getDebugCenterUnit` 8304-8323, `setCenterUnit` 8326-8344, `getNumSymbols` 8599-8602, `getSymbol` 8605-8608, `addSymbol` 8611-8616, `deleteSymbol` 8619-8622, `deleteAllSymbols` 8625-8633, `plotMinimapColor` 9056-9088, `read` 9094-9333, `write` 9339-9580, `setLayoutDirty` 9582-9601, `updatePlotBuilder` 9603-9617, `isLayoutDirty` 9619-9622, `isLayoutStateDifferent` 9624-9632, `setLayoutStateToCurrent` 9634-9637, `getVisibleImprovementState` 9641-9676, `getVisibleBonusState` 9678-9709, `shouldUsePlotBuilder` 9711-9723, `getAreaIdForGreatWall` 10493-10496, `getSoundScriptId` 10498-10517, `get3DAudioScriptFootstepIndex` 10519-10532, `getAqueductSourceWeight` 10534-10547, `shouldDisplayBridge` 10549-10575, `checkLateEra` 10577-10597, `AI_neededPermDefenseReserve` 10960-10990, `AI_neededBetterDefender` 10992-11097
- **CvArea** (3 functions, 119 lines): `uninit` 113-117, `read` 912-970, `write` 973-1027
- **CvPlotGroup** (3 functions, 37 lines): `uninit` 44-49, `read` 298-314, `write` 317-330

Declared although their C++ callers are AI or UI only (cheap, and reachable from FFH2 Python):
`plot_is_pirate_cove_valid`, `plot_get_yield_with_build`, `plot_is_building` (AI),
`plot_calculate_max_yield` (no C++ callers), `plot_is_best_adjacent_found` (UI action
recommendation).

## Porting notes (C++ quirks kept)

- `coordRange` quirk (kept, tested): with wrapping, a coordinate equal to a negative multiple
  of the range maps to `range`, i.e. off the map, so `plot_xy(-width, y)` is None.
- `getLatitude` divides by `height - 1` (or `width - 1`): a 1-plot-high map divides by zero
  (C++ undefined cast; Rust saturates).
- `shouldProcessDisplacementPlot` uses single-precision `atan2`; for adjacent plots the 45
  degree edge of the 90 degree cone compares equal and is inside (checked in a unit test).
- `doCulture` walks its `oldUnits` snapshot with `nextUnitNode` of the plot's list; that only
  follows the snapshot node's `m_pNext`, so iterate the snapshot.
- `changeVisibilityCount` (FFH) clamps a negative count to 0 before applying the change.
- `updatePotentialCityWork` (FFH) only checks the 21 inner city plots, not `NUM_CITY_PLOTS`.
- `plotCheck` calls `funcA` without a NULL check (Rust: required), `plotCount` accepts NULL.
- `CvMap::combinePlotGroups` ignores its `ePlayer` argument; `CvPlot::canTrain` ignores
  `bContinue`; `shouldProcessDisplacementPlot` ignores `range`.
- `CvArea::getNumRevealedTiles`/`changeNumRevealedTiles` assert `< MAX_PLAYERS` on a team
  index (array is per team); harmless.
- `getFoundValue` stores `AI_foundValue` in a `short` (see Storage widths).
