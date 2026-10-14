# CvPlayer port map (`crates/ffh-core/src/player/`)

Source: FFH2 0.41 `CvGameCoreDLL/CvPlayer.cpp` (23,639 lines) and the player-level `CvPlayerAI` functions
rules code calls. Line numbers refer to those files.

**Status: ported and audited.** Each Rust file below is one `impl Game` block; every method has a complete body
(no `todo!` left), and each file was compared line by line with the C++ in the audit wave. "C++ lines" is the
length of the C++ function bodies the file holds.

## Summary

| Rust file | Topic | C++ lines | methods | ported |
|---|---|---:|---:|---:|
| `player/mod.rs` | struct `Player`, `Player::new` (reset), `player_init`, score-history helpers | 763 | 2 | 2 |
| `player/turn.rs` | turn flow and turn-loop unit checks, alive/turn-active state, identity and names, score/population, space launch, empire splitting (new colony player), CvPlayerAI hooks | 1526 | 49 | 49 |
| `player/start.rs` | initial state, free starting units, starting area/plot, advanced start | 2029 | 23 | 23 |
| `player/cities.rs` | city/plot-group lists, found/acquire/raze/disband, city names, capital, counts over cities, goody huts | 1843 | 34 | 34 |
| `player/units.rs` | unit/group lists and lifecycle, train/construct/create checks and production costs, class counts, building effects, hurry, worker builds/routes, free promotions, golden-age/great-people units, combat experience, group cycle | 2039 | 63 | 63 |
| `player/economy.rs` | city-update helpers, totals, unit cost/supply, inflation, gold, commerce, research (rate, queue, progress), golden age, yields, trade routes, corporations flags, happiness/health, specialists, growth | 2084 | 115 | 115 |
| `player/civics_religion.rs` | civics/revolution/upkeep, anarchy, state religion and conversion, holy cities, corporations, vote sources, FFH traits and hidden civics/religions | 2046 | 63 | 63 |
| `player/diplomacy_trade.rs` | contact, diplomacy events, trade items/denials, deals, bonuses, foreign trade, espionage, CvPlayerAI peacetime values | 2586 | 43 | 43 |
| `player/events.rs` | random events: triggered data, weights, options, costs, quests/expiry, event bookkeeping | 2808 | 29 | 29 |
| **total** | | **17724** | **421** | **421** |

Methods = 420 `impl Game` fns `player_*` plus `Player::new`.

Other CvPlayer.cpp functions: 305 trivial getters/setters became `Player` field accesses (1740 C++ lines),
32 list-plumbing functions became Vec/Slab operations (128 lines), and 72 UI/graphics/network/serialization
functions were skipped (2689 lines, of which `read`/`write` are 983). `Player` has 221 fields.

## Conventions used

- Methods are `impl Game` fns `player_<snake_case C++ name>(&self|&mut self, p: PlayerId, ..)`. Overloads get a
  suffix: `getProductionNeeded(UnitTypes|BuildingTypes|ProjectTypes)` -> `player_get_production_needed_{unit,building,project}`,
  `updateCommerce(eCommerce)` -> `player_update_commerce_type`, `isFreePromotion(UnitCombatTypes|UnitClassTypes, ..)` ->
  `player_is_free_promotion_{unit_combat,unit_class}`, `trigger(const EventTriggeredData&)` -> `player_trigger_data`,
  out-parameter overloads `calculateUnitCost(int&..)`/`calculateUnitSupply(int&..)` -> `*_breakdown` returning a struct.
- C++ `const` functions that draw from the game RNG take `&mut self`: `getNewCityName`, `getCivilizationCityName`,
  `pickTriggerCity`, `pickTriggerUnit`; `hasBusyUnit` (kills an empty busy group). Non-const C++ functions that only
  read state take `&self` (`calculateScore`, `upgradeAllPrice`, `canPopRush`, `hasShrine`).
- Pointers: `CvPlot*` -> `PlotIdx`/`Option<PlotIdx>` (Option where C++ accepts NULL), `CvCity*` -> `CityId`, `CvUnit*`
  -> `UnitId`, `CvArea*`/area ids -> `AreaId`; `-1`/`NO_X` arguments and results -> `Option`. Cost functions that use
  -1 as "not possible" keep the i32 sentinel. Coordinates stay `x, y: i32` as in C++.
- Civic arrays (`CivicTypes*`) -> `&IdxVec<CivicOptionType, Option<CivicType>>` (`Option<&..>` where C++ accepts NULL).
- `AdvancedStartActionTypes` and `DiploEventTypes` are not generated; they are Rust enums `AdvancedStartAction`
  (`start.rs`) and `DiploEvent` (`diplomacy_trade.rs`), re-exported from `player`.
- FFH `CvGame::isOption` hard-locks options (NO_ESPIONAGE always on, RANDOM_PERSONALITIES and PICK_RELIGION always
  off): never read `setup.option(..)` directly in rules code; use `is_option(..)` (the CvGame port).
- `Player::new` reproduces `reset()`; the cross-player part of `reset` (other players' `gold_per_turn_by_player[p]`,
  espionage weights against `p`'s team, CvPlayerAI per-player arrays) belongs to `player_init`.
- Per-owner lists (`units`, `cities`, `groups`, `plot_groups`) are in creation order; objects live in the `Game`
  slabs. Use `player_add_*`/`player_delete_*` so both stay consistent. `capital_city` may name a deleted city: read it
  through `player_get_capital_city`.
- The 7 score histories (`m_mapScoreHistory` ..) are `Vec<i32>` indexed by turn: `turn_history` / `set_turn_history`.

### CvPlayerAI state used by rules (plain `Player` fields)

| C++ call (from CvUnit/CvCity/CvPlayer/CvGame/CvTeam/CvDeal or FFH Python) | Rust |
|---|---|
| `AI_getMemoryCount(ePlayer, eMemory)` / `AI_changeMemoryCount(..)` | `players[p].ai_memory_count[other][memory]` (asserts >= 0) |
| `AI_setFirstContact(ePlayer, b)` | `players[p].ai_first_contact[other]` |
| `AI_getCivicTimer` / `AI_setCivicTimer` / `AI_changeCivicTimer` | `players[p].ai_civic_timer` |
| `AI_getReligionTimer` / `AI_setReligionTimer` / `AI_changeReligionTimer` | `players[p].ai_religion_timer` |
| `AI_changeNumTrainAIUnits(eUnitAI, i)` | `players[p].ai_num_train_ai_units[unit_ai]` |
| `AI_getNumAIUnits` / `AI_changeNumAIUnits(eUnitAI, i)` | `players[p].ai_num_ai_units[unit_ai]` |
| `AI_changeGoldTradedTo(ePlayer, i)` | `players[p].ai_gold_traded_to[other]` |
| `AI_get/set/changeAttitudeExtra(ePlayer, ..)` (FFH Python events) | `players[p].ai_attitude_extra[other]` |
| `AI_isSummonSuicideMode` / `AI_setSummonSuicideMode` | `players[p].sum_sui_mode` |
| `AI_changePeacetimeTradeValue(ePlayer, i)` | `player_ai_change_peacetime_trade_value` (also updates team AI state) |
| `AI_changePeacetimeGrantValue(ePlayer, i)` | `player_ai_change_peacetime_grant_value` (also updates team AI state) |
| `AI_makeAssignWorkDirty` / `AI_assignWorkingPlots` / `AI_updateAssignWork` | `player_ai_*` in `turn.rs` (loop over cities into the citizen governor) |
| `AI_makeProductionDirty` | `player_ai_make_production_dirty` (turn.rs) |

AI decisions called by CvPlayer (`AI_conquerCity`, `AI_chooseResearch`, `AI_chooseFreeTech`, `AI_chooseEvent`,
`AI_diploVote`, `AI_doAdvancedStart`, `AI_*Trade` denials, `AI_unitValue`, `AI_foundValue`, ...) go through
`self.controller(p)` or are omitted with a `// AI:` comment, per CLAUDE.md.

## `player/mod.rs` -- 763 C++ lines

Struct `Player`, `Player::new` (reset), `player_init`, score-history helpers.

| Rust | C++ | C++ lines | count | status |
|---|---|---|---:|---|
| `Player::new` | `CvPlayer::CvPlayer` (constructor (`reset(NO_PLAYER, true)`)) | 37-92 | 56 | **ported** |
| `Player::new` | `CvPlayer::uninit` (part of reset (frees arrays)) | 314-395 | 82 | **ported** |
| `Player::new` | `CvPlayer::reset` (reset defaults) | 400-831 | 432 | **ported** |
| `player_init` | `CvPlayer::init` | 119-311 | 193 | **ported** |

## `player/turn.rs` -- 1526 C++ lines

Turn flow and turn-loop unit checks, alive/turn-active state, identity and names, score/population, space launch, empire splitting (new colony player), CvPlayerAI hooks.

| Rust | C++ | C++ lines | count | status |
|---|---|---|---:|---|
| `player_set_disable_human` | `CvPlayer::setDisableHuman` | 2547-2551 | 5 | **ported** |
| `player_update_human` | `CvPlayer::updateHuman` | 2564-2586 | 23 | **ported** |
| `player_is_barbarian` | `CvPlayer::isBarbarian` | 2588-2591 | 4 | **ported** |
| `player_get_name` | `CvPlayer::getName` | 2594-2605 | 12 | **ported** |
| `player_get_civilization_description` | `CvPlayer::getCivilizationDescription` | 2621-2631 | 11 | **ported** |
| `player_get_civilization_short_description` | `CvPlayer::getCivilizationShortDescription` | 2647-2657 | 11 | **ported** |
| `player_get_civilization_adjective` | `CvPlayer::getCivilizationAdjective` | 2673-2683 | 11 | **ported** |
| `player_do_turn` | `CvPlayer::doTurn` | 2818-2971 | 154 | **ported** |
| `player_do_turn_units` | `CvPlayer::doTurnUnits` | 2974-3059 | 86 | **ported** |
| `player_update_timers` | `CvPlayer::updateTimers` | 3358-3378 | 21 | **ported** |
| `player_has_ready_unit` | `CvPlayer::hasReadyUnit` | 3381-3397 | 17 | **ported** |
| `player_has_auto_unit` | `CvPlayer::hasAutoUnit` | 3400-3416 | 17 | **ported** |
| `player_has_busy_unit` | `CvPlayer::hasBusyUnit` | 3419-3441 | 23 | **ported** |
| `player_calculate_score` | `CvPlayer::calculateScore` | 3456-3479 | 24 | **replaced** by score v2 (`world_scores`, docs/SCORE.md); FFH2's formula is `player_legacy_score` |
| `player_get_average_population` | `CvPlayer::getAveragePopulation` | 8482-8490 | 9 | **ported** |
| `player_change_total_population` | `CvPlayer::changeTotalPopulation` | 8493-8505 | 13 | **ported** |
| `player_get_real_population` | `CvPlayer::getRealPopulation` | 8508-8525 | 18 | **ported** |
| `player_change_total_land_scored` | `CvPlayer::changeTotalLandScored` | 8547-8560 | 14 | **ported** |
| `player_get_pop_score` | `CvPlayer::getPopScore` | 10169-10194 | 26 | **ported** |
| `player_change_pop_score` | `CvPlayer::changePopScore` | 10196-10205 | 10 | **ported** |
| `player_get_land_score` | `CvPlayer::getLandScore` | 10208-10233 | 26 | **ported** |
| `player_change_land_score` | `CvPlayer::changeLandScore` | 10236-10245 | 10 | **ported** |
| `player_change_wonders_score` | `CvPlayer::changeWondersScore` | 10254-10263 | 10 | **ported** |
| `player_change_tech_score` | `CvPlayer::changeTechScore` | 10272-10281 | 10 | **ported** |
| `player_is_minor_civ` | `CvPlayer::isMinorCiv` | 10413-10416 | 4 | **ported** |
| `player_set_alive` | `CvPlayer::setAlive` | 10431-10507 | 77 | **ported** |
| `player_verify_alive` | `CvPlayer::verifyAlive` | 10510-10558 | 49 | **ported** |
| `player_set_turn_active` | `CvPlayer::setTurnActive` | 10581-10754 | 174 | **ported** |
| `player_set_auto_moves` | `CvPlayer::setAutoMoves` | 10763-10784 | 22 | **ported** |
| `player_set_end_turn` | `CvPlayer::setEndTurn` | 10793-10806 | 14 | **ported** |
| `player_is_turn_done` | `CvPlayer::isTurnDone` | 10808-10824 | 17 | **ported** |
| `player_get_handicap_type` | `CvPlayer::getHandicapType` | 10889-10892 | 4 | **ported** |
| `player_get_civilization_type` | `CvPlayer::getCivilizationType` | 10895-10898 | 4 | **ported** |
| `player_get_leader_type` | `CvPlayer::getLeaderType` | 10901-10904 | 4 | **ported** |
| `player_update_team_type` | `CvPlayer::updateTeamType` | 11092-11102 | 11 | **ported** |
| `player_set_team` | `CvPlayer::setTeam` | 11104-11136 | 33 | **ported** |
| `player_get_split_empire_player` | `CvPlayer::getSplitEmpirePlayer` | 20406-20451 | 46 | **ported** |
| `player_can_split_empire` | `CvPlayer::canSplitEmpire` | 20453-20490 | 38 | **ported** |
| `player_can_split_area` | `CvPlayer::canSplitArea` | 20492-20527 | 36 | **ported** |
| `player_get_split_empire_leaders` | `CvPlayer::getSplitEmpireLeaders` | 20529-20611 | 83 | **ported** |
| `player_split_empire` | `CvPlayer::splitEmpire` | 20613-20843 | 231 | **ported** |
| `player_launch` | `CvPlayer::launch` | 20957-21001 | 45 | **ported** |
| `player_do_update_cache_on_turn` | `CvPlayer::doUpdateCacheOnTurn` | 21191-21195 | 5 | **ported** |
| `player_is_pirate` | `CvPlayer::isPirate` | 23413-23420 | 8 | **ported** |
| `player_ai_do_tower_mastery` | `CvPlayer::AI_doTowerMastery` | 23587-23600 | 14 | **ported** |
| `player_ai_make_assign_work_dirty` | `CvPlayerAI::AI_makeAssignWorkDirty` (CvPlayerAI.cpp) | 1375-1384 | 10 | **ported** |
| `player_ai_assign_working_plots` | `CvPlayerAI::AI_assignWorkingPlots` (CvPlayerAI.cpp) | 1387-1396 | 10 | **ported** |
| `player_ai_update_assign_work` | `CvPlayerAI::AI_updateAssignWork` (CvPlayerAI.cpp) | 1399-1408 | 10 | **ported** |
| `player_ai_make_production_dirty` | `CvPlayerAI::AI_makeProductionDirty` (CvPlayerAI.cpp) | 1411-1422 | 12 | **ported** |

## `player/start.rs` -- 2029 C++ lines

Initial state, free starting units, starting area/plot, advanced start.

| Rust | C++ | C++ lines | count | status |
|---|---|---|---:|---|
| `player_init_free_state` | `CvPlayer::initFreeState` | 860-871 | 12 | **ported** |
| `player_init_free_units` | `CvPlayer::initFreeUnits` | 874-968 | 95 | **ported** |
| `player_add_free_unit_ai` | `CvPlayer::addFreeUnitAI` | 971-1033 | 63 | **ported** |
| `player_add_free_unit` | `CvPlayer::addFreeUnit` | 1036-1125 | 90 | **ported** |
| `player_starting_plot_range` | `CvPlayer::startingPlotRange` | 1128-1154 | 27 | **ported** |
| `player_starting_plot_within_range` | `CvPlayer::startingPlotWithinRange` | 1157-1164 | 8 | **ported** |
| `player_starting_plot_distance_factor` | `CvPlayer::startingPlotDistanceFactor` | 1166-1208 | 43 | **ported** |
| `player_find_starting_area` | `CvPlayer::findStartingArea` | 1212-1269 | 58 | **ported** |
| `player_find_starting_plot` | `CvPlayer::findStartingPlot` | 1272-1360 | 89 | **ported** |
| `player_find_best_found_value` | `CvPlayer::findBestFoundValue` | 3482-3502 | 21 | **ported** |
| `player_get_starting_plot` | `CvPlayer::getStartingPlot` | 8431-8434 | 4 | **ported** |
| `player_set_starting_plot` | `CvPlayer::setStartingPlot` | 8437-8473 | 37 | **ported** |
| `player_do_advanced_start_action` | `CvPlayer::doAdvancedStartAction` | 14890-15431 | 542 | **ported** |
| `player_get_advanced_start_unit_cost` | `CvPlayer::getAdvancedStartUnitCost` | 15437-15604 | 168 | **ported** |
| `player_get_advanced_start_city_cost` | `CvPlayer::getAdvancedStartCityCost` | 15610-15706 | 97 | **ported** |
| `player_get_advanced_start_pop_cost` | `CvPlayer::getAdvancedStartPopCost` | 15712-15757 | 46 | **ported** |
| `player_get_advanced_start_culture_cost` | `CvPlayer::getAdvancedStartCultureCost` | 15763-15807 | 45 | **ported** |
| `player_get_advanced_start_building_cost` | `CvPlayer::getAdvancedStartBuildingCost` | 15813-15923 | 111 | **ported** |
| `player_get_advanced_start_route_cost` | `CvPlayer::getAdvancedStartRouteCost` | 15929-16031 | 103 | **ported** |
| `player_get_advanced_start_improvement_cost` | `CvPlayer::getAdvancedStartImprovementCost` | 16037-16167 | 131 | **ported** |
| `player_get_advanced_start_tech_cost` | `CvPlayer::getAdvancedStartTechCost` | 16173-16310 | 138 | **ported** |
| `player_get_advanced_start_visibility_cost` | `CvPlayer::getAdvancedStartVisibilityCost` | 16316-16384 | 69 | **ported** |
| `player_get_new_city_production_value` | `CvPlayer::getNewCityProductionValue` | 21809-21840 | 32 | **ported** |

## `player/cities.rs` -- 1843 C++ lines

City/plot-group lists, found/acquire/raze/disband, city names, capital, counts over cities, goody huts.

| Rust | C++ | C++ lines | count | status |
|---|---|---|---:|---|
| `player_init_plot_group` | `CvPlayer::initPlotGroup` | 1363-1374 | 12 | **ported** |
| `player_init_city` | `CvPlayer::initCity` | 1377-1391 | 15 | **ported** |
| `player_acquire_city` | `CvPlayer::acquireCity` | 1394-1939 | 546 | **ported** |
| `player_kill_cities` | `CvPlayer::killCities` | 1942-1953 | 12 | **ported** |
| `player_get_new_city_name` | `CvPlayer::getNewCityName` | 1956-2000 | 45 | **ported** |
| `player_get_civilization_city_name` | `CvPlayer::getCivilizationCityName` | 2003-2030 | 28 | **ported** |
| `player_is_city_name_valid` | `CvPlayer::isCityNameValid` | 2033-2069 | 37 | **ported** |
| `player_update_plot_groups` | `CvPlayer::updatePlotGroups` | 3089-3113 | 25 | **ported** |
| `player_count_num_coastal_cities` | `CvPlayer::countNumCoastalCities` | 3574-3591 | 18 | **ported** |
| `player_count_num_coastal_cities_by_area` | `CvPlayer::countNumCoastalCitiesByArea` | 3594-3615 | 22 | **ported** |
| `player_count_total_culture` | `CvPlayer::countTotalCulture` | 3618-3632 | 15 | **ported** |
| `player_count_city_features` | `CvPlayer::countCityFeatures` | 3743-3772 | 30 | **ported** |
| `player_count_num_buildings` | `CvPlayer::countNumBuildings` | 3775-3794 | 20 | **ported** |
| `player_count_num_cities_connected_to_capital` | `CvPlayer::countNumCitiesConnectedToCapital` | 3797-3814 | 18 | **ported** |
| `player_find_new_capital` | `CvPlayer::findNewCapital` | 4849-4908 | 60 | **ported** |
| `player_get_num_government_centers` | `CvPlayer::getNumGovernmentCenters` | 4911-4928 | 18 | **ported** |
| `player_can_raze` | `CvPlayer::canRaze` | 4931-4964 | 34 | **ported** |
| `player_raze` | `CvPlayer::raze` | 4967-5074 | 108 | **ported** |
| `player_disband` | `CvPlayer::disband` | 5077-5087 | 11 | **ported** |
| `player_can_receive_goody` | `CvPlayer::canReceiveGoody` | 5090-5196 | 107 | **ported** |
| `player_receive_goody` | `CvPlayer::receiveGoody` | 5199-5415 | 217 | **ported** |
| `player_do_goody` | `CvPlayer::doGoody` | 5418-5459 | 42 | **ported** |
| `player_can_found` | `CvPlayer::canFound` | 5462-5635 | 174 | **ported** |
| `player_found` | `CvPlayer::found` | 5638-5731 | 94 | **ported** |
| `player_get_capital_city` | `CvPlayer::getCapitalCity` | 10049-10052 | 4 | **ported** |
| `player_set_capital_city` | `CvPlayer::setCapitalCity` | 10055-10115 | 61 | **ported** |
| `player_get_city_name` | `CvPlayer::getCityName` | 13063-13077 | 15 | **ported** |
| `player_add_plot_group` | `CvPlayer::addPlotGroup` | 13116-13119 | 4 | **ported** |
| `player_delete_plot_group` | `CvPlayer::deletePlotGroup` | 13122-13125 | 4 | **ported** |
| `player_add_city` | `CvPlayer::addCity` | 13152-13155 | 4 | **ported** |
| `player_delete_city` | `CvPlayer::deleteCity` | 13158-13161 | 4 | **ported** |
| `player_pick_conquered_city_owner` | `CvPlayer::pickConqueredCityOwner` | 21575-21588 | 14 | **ported** |
| `player_get_num_building` | `CvPlayer::getNumBuilding` | 23170-23180 | 11 | **ported** |
| `player_get_num_settlements` | `CvPlayer::getNumSettlements` | 23192-23205 | 14 | **ported** |

## `player/units.rs` -- 2039 C++ lines

Unit/group lists and lifecycle, train/construct/create checks and production costs, class counts, building effects, hurry, worker builds/routes, free promotions, golden-age/great-people units, combat experience, group cycle.

| Rust | C++ | C++ lines | count | status |
|---|---|---|---:|---|
| `player_init_unit` | `CvPlayer::initUnit` | 2072-2086 | 15 | **ported** |
| `player_disband_unit` | `CvPlayer::disbandUnit` | 2089-2262 | 174 | **ported** |
| `player_kill_units` | `CvPlayer::killUnits` | 2265-2301 | 37 | **ported** |
| `player_update_plunder` | `CvPlayer::updatePlunder` | 3346-3356 | 11 | **ported** |
| `player_upgrade_all_price` | `CvPlayer::upgradeAllPrice` | 3505-3526 | 22 | **ported** |
| `player_count_religion_spread_units` | `CvPlayer::countReligionSpreadUnits` | 3529-3551 | 23 | **ported** |
| `player_count_corporation_spread_units` | `CvPlayer::countCorporationSpreadUnits` | 3553-3571 | 19 | **ported** |
| `player_can_train` | `CvPlayer::canTrain` | 5734-5882 | 149 | **ported** |
| `player_can_construct` | `CvPlayer::canConstruct` | 5885-6077 | 193 | **ported** |
| `player_can_create` | `CvPlayer::canCreate` | 6080-6171 | 92 | **ported** |
| `player_can_maintain` | `CvPlayer::canMaintain` | 6174-6182 | 9 | **ported** |
| `player_is_production_maxed_unit_class` | `CvPlayer::isProductionMaxedUnitClass` | 6185-6208 | 24 | **ported** |
| `player_is_production_maxed_building_class` | `CvPlayer::isProductionMaxedBuildingClass` | 6211-6237 | 27 | **ported** |
| `player_is_production_maxed_project` | `CvPlayer::isProductionMaxedProject` | 6240-6258 | 19 | **ported** |
| `player_get_production_needed_unit` | `CvPlayer::getProductionNeeded(UnitTypes)` | 6261-6316 | 56 | **ported** |
| `player_get_production_needed_building` | `CvPlayer::getProductionNeeded(BuildingTypes)` | 6319-6352 | 34 | **ported** |
| `player_get_production_needed_project` | `CvPlayer::getProductionNeeded(ProjectTypes)` | 6355-6388 | 34 | **ported** |
| `player_get_production_modifier_unit` | `CvPlayer::getProductionModifier(UnitTypes)` | 6390-6413 | 24 | **ported** |
| `player_get_production_modifier_building` | `CvPlayer::getProductionModifier(BuildingTypes)` | 6415-6447 | 33 | **ported** |
| `player_get_production_modifier_project` | `CvPlayer::getProductionModifier(ProjectTypes)` | 6449-6459 | 11 | **ported** |
| `player_get_building_class_prereq_building` | `CvPlayer::getBuildingClassPrereqBuilding` | 6461-6489 | 29 | **ported** |
| `player_remove_building_class` | `CvPlayer::removeBuildingClass` | 6492-6526 | 35 | **ported** |
| `player_process_building` | `CvPlayer::processBuilding` | 6530-6653 | 124 | **ported** |
| `player_can_build` | `CvPlayer::canBuild` | 6656-6697 | 42 | **ported** |
| `player_get_build_cost` | `CvPlayer::getBuildCost` | 6700-6710 | 11 | **ported** |
| `player_get_best_route` | `CvPlayer::getBestRoute` | 6713-6746 | 34 | **ported** |
| `player_get_improvement_upgrade_rate` | `CvPlayer::getImprovementUpgradeRate` | 6749-6759 | 11 | **ported** |
| `player_units_required_for_golden_age` | `CvPlayer::unitsRequiredForGoldenAge` | 8250-8253 | 4 | **ported** |
| `player_units_golden_age_capable` | `CvPlayer::unitsGoldenAgeCapable` | 8256-8273 | 18 | **ported** |
| `player_units_golden_age_ready` | `CvPlayer::unitsGoldenAgeReady` | 8276-8310 | 35 | **ported** |
| `player_kill_golden_age_units` | `CvPlayer::killGoldenAgeUnits` | 8313-8380 | 68 | **ported** |
| `player_great_people_threshold` | `CvPlayer::greatPeopleThreshold` | 8383-8411 | 29 | **ported** |
| `player_get_domestic_great_general_rate_modifier` | `CvPlayer::getDomesticGreatGeneralRateModifier` | 8930-8933 | 4 | **ported** |
| `player_set_combat_experience` | `CvPlayer::setCombatExperience` | 10288-10339 | 52 | **ported** |
| `player_change_combat_experience` | `CvPlayer::changeCombatExperience` | 10341-10344 | 4 | **ported** |
| `player_is_building_free` | `CvPlayer::isBuildingFree` | 11793-11796 | 4 | **ported** |
| `player_change_free_building_count` | `CvPlayer::changeFreeBuildingCount` | 11799-11871 | 73 | **ported** |
| `player_change_extra_building_happiness` | `CvPlayer::changeExtraBuildingHappiness` | 11882-11893 | 12 | **ported** |
| `player_change_extra_building_health` | `CvPlayer::changeExtraBuildingHealth` | 11903-11914 | 12 | **ported** |
| `player_is_unit_class_maxed_out` | `CvPlayer::isUnitClassMaxedOut` | 11947-11960 | 14 | **ported** |
| `player_get_unit_class_count_plus_making` | `CvPlayer::getUnitClassCountPlusMaking` | 11998-12001 | 4 | **ported** |
| `player_is_building_class_maxed_out` | `CvPlayer::isBuildingClassMaxedOut` | 12012-12025 | 14 | **ported** |
| `player_get_building_class_count_plus_making` | `CvPlayer::getBuildingClassCountPlusMaking` | 12063-12066 | 4 | **ported** |
| `player_can_hurry` | `CvPlayer::canHurry` | 12077-12080 | 4 | **ported** |
| `player_can_pop_rush` | `CvPlayer::canPopRush` | 12083-12086 | 4 | **ported** |
| `player_change_hurry_count` | `CvPlayer::changeHurryCount` | 12088-12107 | 20 | **ported** |
| `player_is_special_building_not_required` | `CvPlayer::isSpecialBuildingNotRequired` | 12117-12120 | 4 | **ported** |
| `player_update_group_cycle` | `CvPlayer::updateGroupCycle` | 12630-12735 | 106 | **ported** |
| `player_remove_group_cycle` | `CvPlayer::removeGroupCycle` | 12738-12756 | 19 | **ported** |
| `player_add_unit` | `CvPlayer::addUnit` | 13188-13191 | 4 | **ported** |
| `player_delete_unit` | `CvPlayer::deleteUnit` | 13194-13197 | 4 | **ported** |
| `player_add_selection_group` | `CvPlayer::addSelectionGroup` | 13224-13227 | 4 | **ported** |
| `player_delete_selection_group` | `CvPlayer::deleteSelectionGroup` | 13230-13235 | 6 | **ported** |
| `player_create_great_people` | `CvPlayer::createGreatPeople` | 17603-17684 | 82 | **ported** |
| `player_is_free_promotion_unit_combat` | `CvPlayer::isFreePromotion(UnitCombatTypes, PromotionTypes)` | 21003-21014 | 12 | **ported** |
| `player_set_free_promotion_unit_combat` | `CvPlayer::setFreePromotion(UnitCombatTypes, PromotionTypes, bFree)` | 21016-21034 | 19 | **ported** |
| `player_is_free_promotion_unit_class` | `CvPlayer::isFreePromotion(UnitClassTypes, PromotionTypes)` | 21036-21047 | 12 | **ported** |
| `player_set_free_promotion_unit_class` | `CvPlayer::setFreePromotion(UnitClassTypes, PromotionTypes, bFree)` | 21049-21067 | 19 | **ported** |
| `player_get_unit_extra_cost` | `CvPlayer::getUnitExtraCost` | 21106-21117 | 12 | **ported** |
| `player_set_unit_extra_cost` | `CvPlayer::setUnitExtraCost` | 21119-21141 | 23 | **ported** |
| `player_verify_unit_stacks_valid` | `CvPlayer::verifyUnitStacksValid` | 21866-21873 | 8 | **ported** |
| `player_count_group_flag_units` | `CvPlayer::countGroupFlagUnits` | 23470-23486 | 17 | **ported** |
| `player_get_conquest_unit_class_count` | `CvPlayer::getConquestUnitClassCount` | 23602-23618 | 17 | **ported** |

## `player/economy.rs` -- 2084 C++ lines

City-update helpers, totals, unit cost/supply, inflation, gold, commerce, research (rate, queue, progress), golden age, yields, trade routes, corporations flags, happiness/health, specialists, growth.

| Rust | C++ | C++ lines | count | status |
|---|---|---|---:|---|
| `player_update_yield` | `CvPlayer::updateYield` | 3116-3125 | 10 | **ported** |
| `player_update_maintenance` | `CvPlayer::updateMaintenance` | 3128-3137 | 10 | **ported** |
| `player_update_power_health` | `CvPlayer::updatePowerHealth` | 3140-3149 | 10 | **ported** |
| `player_update_extra_building_happiness` | `CvPlayer::updateExtraBuildingHappiness` | 3152-3161 | 10 | **ported** |
| `player_update_extra_building_health` | `CvPlayer::updateExtraBuildingHealth` | 3164-3173 | 10 | **ported** |
| `player_update_feature_happiness` | `CvPlayer::updateFeatureHappiness` | 3176-3185 | 10 | **ported** |
| `player_update_religion_happiness` | `CvPlayer::updateReligionHappiness` | 3188-3197 | 10 | **ported** |
| `player_update_extra_specialist_yield` | `CvPlayer::updateExtraSpecialistYield` | 3200-3209 | 10 | **ported** |
| `player_update_commerce_type` | `CvPlayer::updateCommerce(eCommerce)` | 3212-3221 | 10 | **ported** |
| `player_update_commerce` | `CvPlayer::updateCommerce()` | 3224-3233 | 10 | **ported** |
| `player_update_building_commerce` | `CvPlayer::updateBuildingCommerce` | 3236-3245 | 10 | **ported** |
| `player_update_religion_commerce` | `CvPlayer::updateReligionCommerce` | 3248-3257 | 10 | **ported** |
| `player_update_corporation` | `CvPlayer::updateCorporation` | 3260-3267 | 8 | **ported** |
| `player_update_city_plot_yield` | `CvPlayer::updateCityPlotYield` | 3270-3279 | 10 | **ported** |
| `player_update_city_sight` | `CvPlayer::updateCitySight` | 3282-3291 | 10 | **ported** |
| `player_update_trade_routes` | `CvPlayer::updateTradeRoutes` | 3294-3344 | 51 | **ported** |
| `player_choose_tech` | `CvPlayer::chooseTech` | 3444-3453 | 10 | **ported** |
| `player_calculate_total_yield` | `CvPlayer::calculateTotalYield` | 6762-6774 | 13 | **ported** |
| `player_calculate_total_city_happiness` | `CvPlayer::calculateTotalCityHappiness` | 6777-6789 | 13 | **ported** |
| `player_calculate_total_exports` | `CvPlayer::calculateTotalExports` | 6792-6815 | 24 | **ported** |
| `player_calculate_total_imports` | `CvPlayer::calculateTotalImports` | 6818-6847 | 30 | **ported** |
| `player_calculate_total_city_unhappiness` | `CvPlayer::calculateTotalCityUnhappiness` | 6850-6862 | 13 | **ported** |
| `player_calculate_total_city_healthiness` | `CvPlayer::calculateTotalCityHealthiness` | 6865-6877 | 13 | **ported** |
| `player_calculate_total_city_unhealthiness` | `CvPlayer::calculateTotalCityUnhealthiness` | 6879-6891 | 13 | **ported** |
| `player_calculate_unit_cost_breakdown` | `CvPlayer::calculateUnitCost(int& iFreeUnits, ...)` | 6894-6953 | 60 | **ported** |
| `player_calculate_unit_cost` | `CvPlayer::calculateUnitCost()` | 6956-6979 | 24 | **ported** |
| `player_calculate_unit_supply` | `CvPlayer::calculateUnitSupply()` | 6981-6999 | 19 | **ported** |
| `player_calculate_unit_supply_breakdown` | `CvPlayer::calculateUnitSupply(int& iPaidUnits, int& iBaseSupplyCost)` | 7001-7024 | 24 | **ported** |
| `player_calculate_pre_inflated_costs` | `CvPlayer::calculatePreInflatedCosts` | 7027-7035 | 9 | **ported** |
| `player_calculate_inflation_rate` | `CvPlayer::calculateInflationRate` | 7038-7086 | 49 | **ported** |
| `player_calculate_inflated_costs` | `CvPlayer::calculateInflatedCosts` | 7089-7099 | 11 | **ported** |
| `player_calculate_base_net_gold` | `CvPlayer::calculateBaseNetGold` | 7102-7111 | 10 | **ported** |
| `player_calculate_research_modifier` | `CvPlayer::calculateResearchModifier` | 7113-7167 | 55 | **ported** |
| `player_calculate_base_net_research` | `CvPlayer::calculateBaseNetResearch` | 7169-7183 | 15 | **ported** |
| `player_calculate_gold_rate` | `CvPlayer::calculateGoldRate` | 7186-7200 | 15 | **ported** |
| `player_calculate_research_rate` | `CvPlayer::calculateResearchRate` | 7203-7217 | 15 | **ported** |
| `player_calculate_total_commerce` | `CvPlayer::calculateTotalCommerce` | 7219-7232 | 14 | **ported** |
| `player_is_research` | `CvPlayer::isResearch` | 7234-7255 | 22 | **ported** |
| `player_can_ever_research` | `CvPlayer::canEverResearch` | 7258-7295 | 38 | **ported** |
| `player_can_research` | `CvPlayer::canResearch` | 7298-7377 | 80 | **ported** |
| `player_get_current_research` | `CvPlayer::getCurrentResearch` | 7380-7394 | 15 | **ported** |
| `player_is_current_research_repeat` | `CvPlayer::isCurrentResearchRepeat` | 7397-7409 | 13 | **ported** |
| `player_is_no_research_available` | `CvPlayer::isNoResearchAvailable` | 7412-7430 | 19 | **ported** |
| `player_get_research_turns_left` | `CvPlayer::getResearchTurnsLeft` | 7433-7440 | 8 | **ported** |
| `player_get_research_turns_left_times100` | `CvPlayer::getResearchTurnsLeftTimes100` | 7442-7492 | 51 | **ported** |
| `player_specialist_yield` | `CvPlayer::specialistYield` | 8414-8417 | 4 | **ported** |
| `player_specialist_commerce` | `CvPlayer::specialistCommerce` | 8420-8428 | 9 | **ported** |
| `player_is_golden_age` | `CvPlayer::isGoldenAge` | 8627-8630 | 4 | **ported** |
| `player_change_golden_age_turns` | `CvPlayer::changeGoldenAgeTurns` | 8633-8693 | 61 | **ported** |
| `player_get_golden_age_length` | `CvPlayer::getGoldenAgeLength` | 8695-8698 | 4 | **ported** |
| `player_change_happy_per_military_unit` | `CvPlayer::changeHappyPerMilitaryUnit` | 9275-9283 | 9 | **ported** |
| `player_is_military_food_production` | `CvPlayer::isMilitaryFoodProduction` | 9292-9295 | 4 | **ported** |
| `player_is_no_unhealthy_population` | `CvPlayer::isNoUnhealthyPopulation` | 9383-9386 | 4 | **ported** |
| `player_change_no_unhealthy_population_count` | `CvPlayer::changeNoUnhealthyPopulationCount` | 9389-9398 | 10 | **ported** |
| `player_is_building_only_healthy` | `CvPlayer::isBuildingOnlyHealthy` | 9423-9426 | 4 | **ported** |
| `player_change_building_only_healthy_count` | `CvPlayer::changeBuildingOnlyHealthyCount` | 9429-9438 | 10 | **ported** |
| `player_change_distance_maintenance_modifier` | `CvPlayer::changeDistanceMaintenanceModifier` | 9447-9455 | 9 | **ported** |
| `player_change_num_cities_maintenance_modifier` | `CvPlayer::changeNumCitiesMaintenanceModifier` | 9464-9472 | 9 | **ported** |
| `player_change_corporation_maintenance_modifier` | `CvPlayer::changeCorporationMaintenanceModifier` | 9481-9489 | 9 | **ported** |
| `player_get_total_maintenance` | `CvPlayer::getTotalMaintenance` | 9492-9495 | 4 | **ported** |
| `player_change_extra_health` | `CvPlayer::changeExtraHealth` | 9535-9543 | 9 | **ported** |
| `player_change_building_good_health` | `CvPlayer::changeBuildingGoodHealth` | 9552-9561 | 10 | **ported** |
| `player_change_building_bad_health` | `CvPlayer::changeBuildingBadHealth` | 9570-9579 | 10 | **ported** |
| `player_change_extra_happiness` | `CvPlayer::changeExtraHappiness` | 9588-9596 | 9 | **ported** |
| `player_change_building_happiness` | `CvPlayer::changeBuildingHappiness` | 9605-9613 | 9 | **ported** |
| `player_change_largest_city_happiness` | `CvPlayer::changeLargestCityHappiness` | 9622-9630 | 9 | **ported** |
| `player_get_war_weariness_percent_anger` | `CvPlayer::getWarWearinessPercentAnger` | 9633-9644 | 12 | **ported** |
| `player_update_war_weariness_percent_anger` | `CvPlayer::updateWarWearinessPercentAnger` | 9647-9677 | 31 | **ported** |
| `player_get_modified_war_weariness_percent_anger` | `CvPlayer::getModifiedWarWearinessPercentAnger` | 9679-9708 | 30 | **ported** |
| `player_change_war_weariness_modifier` | `CvPlayer::changeWarWearinessModifier` | 9717-9725 | 9 | **ported** |
| `player_change_free_specialist` | `CvPlayer::changeFreeSpecialist` | 9734-9743 | 10 | **ported** |
| `player_is_no_foreign_trade` | `CvPlayer::isNoForeignTrade` | 9752-9755 | 4 | **ported** |
| `player_change_no_foreign_trade_count` | `CvPlayer::changeNoForeignTradeCount` | 9758-9767 | 10 | **ported** |
| `player_is_no_corporations` | `CvPlayer::isNoCorporations` | 9776-9779 | 4 | **ported** |
| `player_change_no_corporations_count` | `CvPlayer::changeNoCorporationsCount` | 9782-9791 | 10 | **ported** |
| `player_is_no_foreign_corporations` | `CvPlayer::isNoForeignCorporations` | 9800-9803 | 4 | **ported** |
| `player_change_no_foreign_corporations_count` | `CvPlayer::changeNoForeignCorporationsCount` | 9806-9815 | 10 | **ported** |
| `player_change_coastal_trade_routes` | `CvPlayer::changeCoastalTradeRoutes` | 9824-9833 | 10 | **ported** |
| `player_change_trade_routes` | `CvPlayer::changeTradeRoutes` | 9842-9851 | 10 | **ported** |
| `player_change_sea_plot_yield` | `CvPlayer::changeSeaPlotYield` | 11181-11192 | 12 | **ported** |
| `player_change_yield_rate_modifier` | `CvPlayer::changeYieldRateModifier` | 11203-11226 | 24 | **ported** |
| `player_change_capital_yield_rate_modifier` | `CvPlayer::changeCapitalYieldRateModifier` | 11237-11267 | 31 | **ported** |
| `player_update_extra_yield_threshold` | `CvPlayer::updateExtraYieldThreshold` | 11278-11310 | 33 | **ported** |
| `player_change_trade_yield_modifier` | `CvPlayer::changeTradeYieldModifier` | 11321-11332 | 12 | **ported** |
| `player_change_free_city_commerce` | `CvPlayer::changeFreeCityCommerce` | 11343-11355 | 13 | **ported** |
| `player_set_commerce_percent` | `CvPlayer::setCommercePercent` | 11366-11419 | 54 | **ported** |
| `player_change_commerce_percent` | `CvPlayer::changeCommercePercent` | 11422-11425 | 4 | **ported** |
| `player_get_commerce_rate` | `CvPlayer::getCommerceRate` | 11428-11454 | 27 | **ported** |
| `player_change_commerce_rate_modifier` | `CvPlayer::changeCommerceRateModifier` | 11482-11495 | 14 | **ported** |
| `player_change_capital_commerce_rate_modifier` | `CvPlayer::changeCapitalCommerceRateModifier` | 11506-11526 | 21 | **ported** |
| `player_change_state_religion_building_commerce` | `CvPlayer::changeStateReligionBuildingCommerce` | 11537-11549 | 13 | **ported** |
| `player_change_specialist_extra_commerce` | `CvPlayer::changeSpecialistExtraCommerce` | 11560-11574 | 15 | **ported** |
| `player_is_commerce_flexible` | `CvPlayer::isCommerceFlexible` | 11585-11604 | 20 | **ported** |
| `player_change_commerce_flexible_count` | `CvPlayer::changeCommerceFlexibleCount` | 11607-11628 | 22 | **ported** |
| `player_change_gold_per_turn_by_player` | `CvPlayer::changeGoldPerTurnByPlayer` | 11639-11659 | 21 | **ported** |
| `player_change_feature_happiness` | `CvPlayer::changeFeatureHappiness` | 11925-11936 | 12 | **ported** |
| `player_is_specialist_valid` | `CvPlayer::isSpecialistValid` | 12342-12345 | 4 | **ported** |
| `player_change_specialist_valid_count` | `CvPlayer::changeSpecialistValidCount` | 12348-12361 | 14 | **ported** |
| `player_change_specialist_extra_yield` | `CvPlayer::changeSpecialistExtraYield` | 12584-12600 | 17 | **ported** |
| `player_change_improvement_yield_change` | `CvPlayer::changeImprovementYieldChange` | 12613-12626 | 14 | **ported** |
| `player_find_path_length` | `CvPlayer::findPathLength` | 12790-12847 | 58 | **ported** |
| `player_get_queue_position` | `CvPlayer::getQueuePosition` | 12851-12866 | 16 | **ported** |
| `player_clear_research_queue` | `CvPlayer::clearResearchQueue` | 12869-12886 | 18 | **ported** |
| `player_push_research` | `CvPlayer::pushResearch` | 12892-12999 | 108 | **ported** |
| `player_pop_research` | `CvPlayer::popResearch` | 13003-13024 | 22 | **ported** |
| `player_do_gold` | `CvPlayer::doGold` | 13576-13634 | 59 | **ported** |
| `player_do_research` | `CvPlayer::doResearch` | 13637-13688 | 52 | **ported** |
| `player_verify_gold_commerce_percent` | `CvPlayer::verifyGoldCommercePercent` | 16454-16465 | 12 | **ported** |
| `player_invalidate_population_rank_cache` | `CvPlayer::invalidatePopulationRankCache` | 21160-21168 | 9 | **ported** |
| `player_invalidate_yield_rank_cache` | `CvPlayer::invalidateYieldRankCache` | 21170-21178 | 9 | **ported** |
| `player_invalidate_commerce_rank_cache` | `CvPlayer::invalidateCommerceRankCache` | 21180-21188 | 9 | **ported** |
| `player_get_growth_threshold` | `CvPlayer::getGrowthThreshold` | 21842-21864 | 23 | **ported** |
| `player_get_tech_free_unit` | `CvPlayer::getTechFreeUnit` | 21875-21895 | 21 | **ported** |
| `player_is_has_tech` | `CvPlayer::isHasTech` | 23059-23062 | 4 | **ported** |
| `player_change_specialist_type_extra_commerce` | `CvPlayer::changeSpecialistTypeExtraCommerce` | 23381-23389 | 9 | **ported** |

## `player/civics_religion.rs` -- 2046 C++ lines

Civics/revolution/upkeep, anarchy, state religion and conversion, holy cities, corporations, vote sources, FFH traits and hidden civics/religions.

| Rust | C++ | C++ lines | count | status |
|---|---|---|---:|---|
| `player_set_has_trait` | `CvPlayer::setHasTrait` | 2435-2545 | 111 | **ported** |
| `player_verify_civics` | `CvPlayer::verifyCivics` | 3062-3086 | 25 | **ported** |
| `player_is_civic` | `CvPlayer::isCivic` | 7495-7508 | 14 | **ported** |
| `player_can_do_civics` | `CvPlayer::canDoCivics` | 7511-7614 | 104 | **ported** |
| `player_can_revolution` | `CvPlayer::canRevolution` | 7617-7675 | 59 | **ported** |
| `player_revolution` | `CvPlayer::revolution` | 7678-7713 | 36 | **ported** |
| `player_get_civic_percent_anger` | `CvPlayer::getCivicPercentAnger` | 7716-7764 | 49 | **ported** |
| `player_can_do_religion` | `CvPlayer::canDoReligion` | 7767-7801 | 35 | **ported** |
| `player_can_change_religion` | `CvPlayer::canChangeReligion` | 7804-7817 | 14 | **ported** |
| `player_can_convert` | `CvPlayer::canConvert` | 7820-7879 | 60 | **ported** |
| `player_convert` | `CvPlayer::convert` | 7882-7913 | 32 | **ported** |
| `player_has_holy_city` | `CvPlayer::hasHolyCity` | 7916-7930 | 15 | **ported** |
| `player_count_holy_cities` | `CvPlayer::countHolyCities` | 7933-7949 | 17 | **ported** |
| `player_found_religion` | `CvPlayer::foundReligion` | 7952-8054 | 103 | **ported** |
| `player_has_headquarters` | `CvPlayer::hasHeadquarters` | 8057-8069 | 13 | **ported** |
| `player_count_headquarters` | `CvPlayer::countHeadquarters` | 8072-8085 | 14 | **ported** |
| `player_count_corporations` | `CvPlayer::countCorporations` | 8088-8102 | 15 | **ported** |
| `player_found_corporation` | `CvPlayer::foundCorporation` | 8105-8157 | 53 | **ported** |
| `player_get_civic_anarchy_length` | `CvPlayer::getCivicAnarchyLength` | 8160-8211 | 52 | **ported** |
| `player_get_religion_anarchy_length` | `CvPlayer::getReligionAnarchyLength` | 8214-8246 | 33 | **ported** |
| `player_is_anarchy` | `CvPlayer::isAnarchy` | 8732-8735 | 4 | **ported** |
| `player_change_anarchy_turns` | `CvPlayer::changeAnarchyTurns` | 8738-8783 | 46 | **ported** |
| `player_update_max_anarchy_turns` | `CvPlayer::updateMaxAnarchyTurns` | 8792-8816 | 25 | **ported** |
| `player_change_anarchy_modifier` | `CvPlayer::changeAnarchyModifier` | 8825-8834 | 10 | **ported** |
| `player_is_state_religion` | `CvPlayer::isStateReligion` | 9914-9917 | 4 | **ported** |
| `player_change_state_religion_count` | `CvPlayer::changeStateReligionCount` | 9920-9941 | 22 | **ported** |
| `player_is_no_non_state_religion_spread` | `CvPlayer::isNoNonStateReligionSpread` | 9950-9953 | 4 | **ported** |
| `player_change_state_religion_happiness` | `CvPlayer::changeStateReligionHappiness` | 9969-9977 | 9 | **ported** |
| `player_change_non_state_religion_happiness` | `CvPlayer::changeNonStateReligionHappiness` | 9986-9994 | 9 | **ported** |
| `player_get_state_religion` | `CvPlayer::getStateReligion` | 11002-11005 | 4 | **ported** |
| `player_set_last_state_religion` | `CvPlayer::setLastStateReligion` | 11008-11075 | 68 | **ported** |
| `player_is_has_civic_option` | `CvPlayer::isHasCivicOption` | 12140-12143 | 4 | **ported** |
| `player_is_no_civic_upkeep` | `CvPlayer::isNoCivicUpkeep` | 12163-12166 | 4 | **ported** |
| `player_count_total_has_religion` | `CvPlayer::countTotalHasReligion` | 12195-12208 | 14 | **ported** |
| `player_get_has_corporation_count` | `CvPlayer::getHasCorporationCount` | 12210-12218 | 9 | **ported** |
| `player_count_total_has_corporation` | `CvPlayer::countTotalHasCorporation` | 12221-12231 | 11 | **ported** |
| `player_is_active_corporation` | `CvPlayer::isActiveCorporation` | 12233-12246 | 14 | **ported** |
| `player_find_highest_has_religion_count` | `CvPlayer::findHighestHasReligionCount` | 12249-12268 | 20 | **ported** |
| `player_change_has_religion_count` | `CvPlayer::changeHasReligionCount` | 12271-12285 | 15 | **ported** |
| `player_change_has_corporation_count` | `CvPlayer::changeHasCorporationCount` | 12288-12302 | 15 | **ported** |
| `player_set_loyal_member` | `CvPlayer::setLoyalMember` | 12395-12408 | 14 | **ported** |
| `player_get_single_civic_upkeep` | `CvPlayer::getSingleCivicUpkeep` | 12418-12478 | 61 | **ported** |
| `player_get_civic_upkeep` | `CvPlayer::getCivicUpkeep` | 12481-12499 | 19 | **ported** |
| `player_set_civics` | `CvPlayer::setCivics` | 12502-12571 | 70 | **ported** |
| `player_process_civics` | `CvPlayer::processCivics` | 16468-16581 | 114 | **ported** |
| `player_get_vote` | `CvPlayer::getVote` | 21069-21080 | 12 | **ported** |
| `player_set_vote` | `CvPlayer::setVote` | 21082-21104 | 23 | **ported** |
| `player_has_shrine` | `CvPlayer::hasShrine` | 21145-21158 | 14 | **ported** |
| `player_process_vote_source_bonus` | `CvPlayer::processVoteSourceBonus` | 21197-21204 | 8 | **ported** |
| `player_get_votes` | `CvPlayer::getVotes` | 21206-21290 | 85 | **ported** |
| `player_can_do_resolution` | `CvPlayer::canDoResolution` | 21292-21420 | 129 | **ported** |
| `player_can_defy_resolution` | `CvPlayer::canDefyResolution` | 21422-21499 | 78 | **ported** |
| `player_set_defied_resolution` | `CvPlayer::setDefiedResolution` | 21502-21525 | 24 | **ported** |
| `player_set_endorsed_resolution` | `CvPlayer::setEndorsedResolution` | 21528-21531 | 4 | **ported** |
| `player_is_full_member` | `CvPlayer::isFullMember` | 21533-21557 | 25 | **ported** |
| `player_is_voting_member` | `CvPlayer::isVotingMember` | 21559-21573 | 15 | **ported** |
| `player_get_religion_population` | `CvPlayer::getReligionPopulation` | 21794-21807 | 14 | **ported** |
| `player_can_see_civic` | `CvPlayer::canSeeCivic` | 22904-22915 | 12 | **ported** |
| `player_can_see_religion` | `CvPlayer::canSeeReligion` | 22917-22935 | 19 | **ported** |
| `player_is_gambling_ring` | `CvPlayer::isGamblingRing` | 23044-23057 | 14 | **ported** |
| `player_is_slave_trade` | `CvPlayer::isSlaveTrade` | 23064-23077 | 14 | **ported** |
| `player_is_smuggling_ring` | `CvPlayer::isSmugglingRing` | 23079-23092 | 14 | **ported** |
| `player_ai_do_favorite_religion` | `CvPlayer::AI_doFavoriteReligion` | 23488-23586 | 99 | **ported** |

## `player/diplomacy_trade.rs` -- 2586 C++ lines

Contact, diplomacy events, trade items/denials, deals, bonuses, foreign trade, espionage, CvPlayerAI peacetime values.

| Rust | C++ | C++ lines | count | status |
|---|---|---|---:|---|
| `player_count_owned_bonuses` | `CvPlayer::countOwnedBonuses` | 3635-3678 | 44 | **ported** |
| `player_count_unimproved_bonuses` | `CvPlayer::countUnimprovedBonuses` | 3681-3740 | 60 | **ported** |
| `player_count_potential_foreign_trade_cities` | `CvPlayer::countPotentialForeignTradeCities` | 3817-3847 | 31 | **ported** |
| `player_count_potential_foreign_trade_cities_connected` | `CvPlayer::countPotentialForeignTradeCitiesConnected` | 3850-3889 | 40 | **ported** |
| `player_can_contact` | `CvPlayer::canContact` | 3893-3955 | 63 | **ported** |
| `player_handle_diplo_event` | `CvPlayer::handleDiploEvent` | 4005-4162 | 158 | **ported** |
| `player_can_trade_with` | `CvPlayer::canTradeWith` | 4165-4213 | 49 | **ported** |
| `player_can_receive_trade_city` | `CvPlayer::canReceiveTradeCity` | 4215-4223 | 9 | **ported** |
| `player_can_trade_item` | `CvPlayer::canTradeItem` | 4225-4553 | 329 | **ported** |
| `player_get_trade_denial` | `CvPlayer::getTradeDenial` | 4556-4631 | 76 | **ported** |
| `player_can_trade_network_with` | `CvPlayer::canTradeNetworkWith` | 4634-4649 | 16 | **ported** |
| `player_get_num_available_bonuses` | `CvPlayer::getNumAvailableBonuses` | 4652-4674 | 23 | **ported** |
| `player_get_num_tradeable_bonuses` | `CvPlayer::getNumTradeableBonuses` | 4677-4680 | 4 | **ported** |
| `player_has_bonus` | `CvPlayer::hasBonus` | 4682-4694 | 13 | **ported** |
| `player_get_num_trade_bonus_imports` | `CvPlayer::getNumTradeBonusImports` | 4696-4733 | 38 | **ported** |
| `player_is_trading_with_team` | `CvPlayer::isTradingWithTeam` | 4736-4771 | 36 | **ported** |
| `player_can_stop_trading_with_team` | `CvPlayer::canStopTradingWithTeam` | 4774-4797 | 24 | **ported** |
| `player_stop_trading_with_team` | `CvPlayer::stopTradingWithTeam` | 4800-4831 | 32 | **ported** |
| `player_kill_all_deals` | `CvPlayer::killAllDeals` | 4834-4846 | 13 | **ported** |
| `player_change_bonus_export` | `CvPlayer::changeBonusExport` | 11712-11733 | 22 | **ported** |
| `player_change_bonus_import` | `CvPlayer::changeBonusImport` | 11744-11765 | 22 | **ported** |
| `player_do_espionage_points` | `CvPlayer::doEspionagePoints` | 13690-13718 | 29 | **ported** |
| `player_get_espionage_spending` | `CvPlayer::getEspionageSpending` | 13720-13822 | 103 | **ported** |
| `player_can_do_espionage_mission` | `CvPlayer::canDoEspionageMission` | 13824-13869 | 46 | **ported** |
| `player_get_espionage_mission_cost` | `CvPlayer::getEspionageMissionCost` | 13871-13887 | 17 | **ported** |
| `player_get_espionage_mission_base_cost` | `CvPlayer::getEspionageMissionBaseCost` | 13889-14285 | 397 | **ported** |
| `player_get_espionage_mission_cost_modifier` | `CvPlayer::getEspionageMissionCostModifier` | 14288-14397 | 110 | **ported** |
| `player_do_espionage_mission` | `CvPlayer::doEspionageMission` | 14400-14857 | 458 | **ported** |
| `player_set_espionage_spending_weight_against_team` | `CvPlayer::setEspionageSpendingWeightAgainstTeam` | 14866-14880 | 15 | **ported** |
| `player_change_espionage_spending_weight_against_team` | `CvPlayer::changeEspionageSpendingWeightAgainstTeam` | 14882-14888 | 7 | **ported** |
| `player_can_have_trade_routes_with` | `CvPlayer::canHaveTradeRoutesWith` | 21590-21623 | 34 | **ported** |
| `player_can_steal_tech` | `CvPlayer::canStealTech` | 21625-21636 | 12 | **ported** |
| `player_can_force_civics` | `CvPlayer::canForceCivics` | 21638-21641 | 4 | **ported** |
| `player_can_force_religion` | `CvPlayer::canForceReligion` | 21643-21646 | 4 | **ported** |
| `player_can_spy_destroy_unit` | `CvPlayer::canSpyDestroyUnit` | 21648-21666 | 19 | **ported** |
| `player_can_spy_bribe_unit` | `CvPlayer::canSpyBribeUnit` | 21668-21705 | 38 | **ported** |
| `player_can_spy_destroy_building` | `CvPlayer::canSpyDestroyBuilding` | 21707-21721 | 15 | **ported** |
| `player_can_spy_destroy_project` | `CvPlayer::canSpyDestroyProject` | 21723-21759 | 37 | **ported** |
| `player_force_peace` | `CvPlayer::forcePeace` | 21761-21776 | 16 | **ported** |
| `player_can_spies_enter_borders` | `CvPlayer::canSpiesEnterBorders` | 21778-21792 | 15 | **ported** |
| `player_is_no_diplomacy_with_enemies` | `CvPlayer::isNoDiplomacyWithEnemies` | 23007-23014 | 8 | **ported** |
| `player_ai_change_peacetime_trade_value` | `CvPlayerAI::AI_changePeacetimeTradeValue` (CvPlayerAI.cpp) | 11671-11720 | 50 | **ported** |
| `player_ai_change_peacetime_grant_value` | `CvPlayerAI::AI_changePeacetimeGrantValue` (CvPlayerAI.cpp) | 11731-11780 | 50 | **ported** |

## `player/events.rs` -- 2808 C++ lines

Random events: triggered data, weights, options, costs, quests/expiry, event bookkeeping.

| Rust | C++ | C++ lines | count | status |
|---|---|---|---:|---|
| `player_get_event_triggered` | `CvPlayer::getEventTriggered` | 13252-13255 | 4 | **ported** |
| `player_add_event_triggered` | `CvPlayer::addEventTriggered` | 13257-13260 | 4 | **ported** |
| `player_delete_event_triggered` | `CvPlayer::deleteEventTriggered` | 13262-13265 | 4 | **ported** |
| `player_get_event_occured` | `CvPlayer::getEventOccured` | 17687-17699 | 13 | **ported** |
| `player_is_trigger_fired` | `CvPlayer::isTriggerFired` | 17701-17704 | 4 | **ported** |
| `player_reset_event_occured` | `CvPlayer::resetEventOccured` | 17706-17717 | 12 | **ported** |
| `player_set_event_occured` | `CvPlayer::setEventOccured` | 17719-17756 | 38 | **ported** |
| `player_get_event_countdown` | `CvPlayer::getEventCountdown` | 17759-17771 | 13 | **ported** |
| `player_set_event_countdown` | `CvPlayer::setEventCountdown` | 17773-17778 | 6 | **ported** |
| `player_reset_event_countdown` | `CvPlayer::resetEventCountdown` | 17780-17790 | 11 | **ported** |
| `player_reset_trigger_fired` | `CvPlayer::resetTriggerFired` | 17793-17801 | 9 | **ported** |
| `player_set_trigger_fired` | `CvPlayer::setTriggerFired` | 17803-17920 | 118 | **ported** |
| `player_init_triggered_data` | `CvPlayer::initTriggeredData` | 17922-18433 | 512 | **ported** |
| `player_can_do_event` | `CvPlayer::canDoEvent` | 18436-18813 | 378 | **ported** |
| `player_apply_event` | `CvPlayer::applyEvent` | 18816-19574 | 759 | **ported** |
| `player_is_valid_event_tech` | `CvPlayer::isValidEventTech` | 19576-19606 | 31 | **ported** |
| `player_get_best_event_tech` | `CvPlayer::getBestEventTech` | 19609-19671 | 63 | **ported** |
| `player_get_event_cost` | `CvPlayer::getEventCost` | 19673-19694 | 22 | **ported** |
| `player_do_events` | `CvPlayer::doEvents` | 19697-19826 | 130 | **ported** |
| `player_expire_event` | `CvPlayer::expireEvent` | 19829-19855 | 27 | **ported** |
| `player_check_expire_event` | `CvPlayer::checkExpireEvent` | 19857-19953 | 97 | **ported** |
| `player_trigger` | `CvPlayer::trigger(EventTriggerTypes)` | 19956-19959 | 4 | **ported** |
| `player_trigger_data` | `CvPlayer::trigger(const EventTriggeredData&)` | 19961-19976 | 16 | **ported** |
| `player_can_trigger` | `CvPlayer::canTrigger` | 19978-20077 | 100 | **ported** |
| `player_pick_trigger_city` | `CvPlayer::pickTriggerCity` | 20079-20107 | 29 | **ported** |
| `player_pick_trigger_unit` | `CvPlayer::pickTriggerUnit` | 20109-20137 | 29 | **ported** |
| `player_get_event_trigger_weight` | `CvPlayer::getEventTriggerWeight` | 20139-20403 | 265 | **ported** |
| `player_is_valid_trigger_religion` | `CvPlayer::isValidTriggerReligion` | 20845-20899 | 55 | **ported** |
| `player_is_valid_trigger_corporation` | `CvPlayer::isValidTriggerCorporation` | 20901-20955 | 55 | **ported** |

## Trivial accessors -> `Player` fields

These CvPlayer functions are plain member reads/writes (or have only UI side effects) and are not methods;
use the field. `change*` = `+=` (C++ asserts on the sign are debug-only).

| C++ | Rust field |
|---|---|
| `hasTrait` | `traits[t]` |
| `getDisableHuman` | `disable_human` |
| `isHuman` | `human` |
| `getTotalPopulation` | `total_population` |
| `getTotalLand`, `changeTotalLand` | `total_land` |
| `getTotalLandScored` | `total_land_scored` |
| `getGold`, `setGold`, `changeGold` | `gold` |
| `getGoldPerTurn` | `gold_per_turn` |
| `getAdvancedStartPoints`, `setAdvancedStartPoints`, `changeAdvancedStartPoints` | `advanced_start_points` |
| `getGoldenAgeTurns` | `golden_age_turns` |
| `getNumUnitGoldenAges`, `changeNumUnitGoldenAges` | `num_unit_golden_ages` |
| `getStrikeTurns`, `changeStrikeTurns` | `strike_turns` |
| `getAnarchyTurns` | `anarchy_turns` |
| `getMaxAnarchyTurns` | `max_anarchy_turns` |
| `getAnarchyModifier` | `anarchy_modifier` |
| `getGoldenAgeModifier`, `changeGoldenAgeModifier` | `golden_age_modifier` |
| `getHurryModifier`, `changeHurryModifier` | `global_hurry_modifier` |
| `getGreatPeopleCreated`, `setGreatPeopleCreated` | `great_people_created` |
| `incrementGreatPeopleCreated` | `great_people_created += 1` |
| `getGreatGeneralsCreated` | `great_generals_created` |
| `incrementGreatGeneralsCreated` | `great_generals_created += 1` |
| `getGreatPeopleThresholdModifier`, `changeGreatPeopleThresholdModifier`, `setGreatPeopleThresholdModifier` | `great_people_threshold_modifier` |
| `getGreatGeneralsThresholdModifier`, `changeGreatGeneralsThresholdModifier` | `great_generals_threshold_modifier` |
| `getGreatPeopleRateModifier`, `changeGreatPeopleRateModifier` | `great_people_rate_modifier` |
| `getGreatGeneralRateModifier`, `changeGreatGeneralRateModifier` | `great_general_rate_modifier` |
| `changeDomesticGreatGeneralRateModifier` | `domestic_great_general_rate_modifier` |
| `getStateReligionGreatPeopleRateModifier`, `changeStateReligionGreatPeopleRateModifier` | `state_religion_great_people_rate_modifier` |
| `getMaxGlobalBuildingProductionModifier`, `changeMaxGlobalBuildingProductionModifier` | `max_global_building_production_modifier` |
| `getMaxTeamBuildingProductionModifier`, `changeMaxTeamBuildingProductionModifier` | `max_team_building_production_modifier` |
| `getMaxPlayerBuildingProductionModifier`, `changeMaxPlayerBuildingProductionModifier` | `max_player_building_production_modifier` |
| `getFreeExperience`, `changeFreeExperience` | `free_experience` |
| `getFeatureProductionModifier`, `changeFeatureProductionModifier` | `feature_production_modifier` |
| `getWorkerSpeedModifier`, `changeWorkerSpeedModifier` | `worker_speed_modifier` |
| `getImprovementUpgradeRateModifier`, `changeImprovementUpgradeRateModifier` | `improvement_upgrade_rate_modifier` |
| `getMilitaryProductionModifier`, `changeMilitaryProductionModifier` | `military_production_modifier` |
| `getSpaceProductionModifier`, `changeSpaceProductionModifier` | `space_production_modifier` |
| `getCityDefenseModifier`, `changeCityDefenseModifier` | `city_defense_modifier` |
| `getNumNukeUnits`, `changeNumNukeUnits` | `num_nuke_units` |
| `getNumOutsideUnits`, `changeNumOutsideUnits` | `num_outside_units` |
| `getBaseFreeUnits`, `changeBaseFreeUnits` | `base_free_units` |
| `getBaseFreeMilitaryUnits`, `changeBaseFreeMilitaryUnits` | `base_free_military_units` |
| `getFreeUnitsPopulationPercent`, `changeFreeUnitsPopulationPercent` | `free_units_population_percent` |
| `getFreeMilitaryUnitsPopulationPercent`, `changeFreeMilitaryUnitsPopulationPercent` | `free_military_units_population_percent` |
| `getGoldPerUnit`, `changeGoldPerUnit` | `gold_per_unit` |
| `getGoldPerMilitaryUnit`, `changeGoldPerMilitaryUnit` | `gold_per_military_unit` |
| `getExtraUnitCost`, `changeExtraUnitCost` | `extra_unit_cost` |
| `getNumMilitaryUnits`, `changeNumMilitaryUnits` | `num_military_units` |
| `getHappyPerMilitaryUnit` | `happy_per_military_unit` |
| `getMilitaryFoodProductionCount`, `changeMilitaryFoodProductionCount` | `military_food_production_count` |
| `getHighestUnitLevel`, `setHighestUnitLevel` | `highest_unit_level` |
| `getMaxConscript`, `changeMaxConscript` | `max_conscript` |
| `getConscriptCount`, `setConscriptCount`, `changeConscriptCount` | `conscript_count` |
| `getOverflowResearch`, `setOverflowResearch`, `changeOverflowResearch` | `overflow_research` |
| `getNoUnhealthyPopulationCount` | `no_unhealthy_population_count` |
| `getExpInBorderModifier`, `changeExpInBorderModifier` | `exp_in_border_modifier` |
| `getBuildingOnlyHealthyCount` | `building_only_healthy_count` |
| `getDistanceMaintenanceModifier` | `distance_maintenance_modifier` |
| `getNumCitiesMaintenanceModifier` | `num_cities_maintenance_modifier` |
| `getCorporationMaintenanceModifier` | `corporation_maintenance_modifier` |
| `changeTotalMaintenance` | `total_maintenance (x100)` |
| `getUpkeepModifier`, `changeUpkeepModifier` | `upkeep_modifier` |
| `getLevelExperienceModifier`, `changeLevelExperienceModifier` | `level_experience_modifier` |
| `getExtraHealth` | `extra_health` |
| `getBuildingGoodHealth` | `building_good_health` |
| `getBuildingBadHealth` | `building_bad_health` |
| `getExtraHappiness` | `extra_happiness` |
| `getBuildingHappiness` | `building_happiness` |
| `getLargestCityHappiness` | `largest_city_happiness` |
| `getWarWearinessModifier` | `war_weariness_modifier` |
| `getFreeSpecialist` | `free_specialist` |
| `getNoForeignTradeCount` | `no_foreign_trade_count` |
| `getNoCorporationsCount` | `no_corporations_count` |
| `getNoForeignCorporationsCount` | `no_foreign_corporations_count` |
| `getCoastalTradeRoutes` | `coastal_trade_routes` |
| `getTradeRoutes` | `trade_routes` |
| `getRevolutionTimer`, `setRevolutionTimer`, `changeRevolutionTimer` | `revolution_timer` |
| `getConversionTimer`, `setConversionTimer`, `changeConversionTimer` | `conversion_timer` |
| `getStateReligionCount` | `state_religion_count` |
| `getNoNonStateReligionSpreadCount`, `changeNoNonStateReligionSpreadCount` | `no_non_state_religion_spread_count` |
| `getStateReligionHappiness` | `state_religion_happiness` |
| `getNonStateReligionHappiness` | `non_state_religion_happiness` |
| `getStateReligionUnitProductionModifier`, `changeStateReligionUnitProductionModifier` | `state_religion_unit_production_modifier` |
| `getStateReligionBuildingProductionModifier`, `changeStateReligionBuildingProductionModifier` | `state_religion_building_production_modifier` |
| `getStateReligionFreeExperience`, `changeStateReligionFreeExperience` | `state_religion_free_experience` |
| `getCitiesLost`, `changeCitiesLost` | `cities_lost` |
| `getWinsVsBarbs`, `changeWinsVsBarbs` | `wins_vs_barbs` |
| `getAssets`, `changeAssets` | `assets` |
| `getPower`, `changePower` | `power` |
| `getWondersScore` | `wonders_score` |
| `getTechScore` | `tech_score` |
| `getCombatExperience` | `combat_experience` |
| `isAlive` | `alive` |
| `isEverAlive` | `ever_alive` |
| `isTurnActive` | `turn_active` |
| `isAutoMoves` | `auto_moves` |
| `isEndTurn` | `end_turn` |
| `isExtendedGame` | `extended_game` |
| `makeExtendedGame` | `extended_game = true` |
| `isFoundedFirstCity`, `setFoundedFirstCity` | `founded_first_city` |
| `isStrike` | `strike` |
| `setStrike` | `strike (C++ also shows a message)` |
| `getID` | `id` |
| `getPersonalityType`, `setPersonalityType` | `personality_type` |
| `getCurrentEra` | `current_era` |
| `setCurrentEra` | `current_era (C++ side effects are graphics only)` |
| `getLastStateReligion` | `last_state_religion` |
| `getParent`, `setParent` | `parent` |
| `getTeam` | `team` |
| `getSeaPlotYield` | `sea_plot_yield[y]` |
| `getYieldRateModifier` | `yield_rate_modifier[y]` |
| `getCapitalYieldRateModifier` | `capital_yield_rate_modifier[y]` |
| `getExtraYieldThreshold` | `extra_yield_threshold[y]` |
| `getTradeYieldModifier` | `trade_yield_modifier[y]` |
| `getFreeCityCommerce` | `free_city_commerce[c]` |
| `getCommercePercent` | `commerce_percent[c]` |
| `changeCommerceRate` | `commerce_rate[c] (x100)` |
| `getCommerceRateModifier` | `commerce_rate_modifier[c]` |
| `getCapitalCommerceRateModifier` | `capital_commerce_rate_modifier[c]` |
| `getStateReligionBuildingCommerce` | `state_religion_building_commerce[c]` |
| `getSpecialistExtraCommerce` | `specialist_extra_commerce[c]` |
| `getCommerceFlexibleCount` | `commerce_flexible_count[c]` |
| `getGoldPerTurnByPlayer` | `gold_per_turn_by_player[p]` |
| `isFeatAccomplished`, `setFeatAccomplished` | `feat_accomplished[f]` |
| `isOption`, `setOption` | `options[o]` |
| `getBonusExport` | `bonus_export[b]` |
| `getBonusImport` | `bonus_import[b]` |
| `getImprovementCount`, `changeImprovementCount` | `improvement_count[i]` |
| `getFreeBuildingCount` | `free_building_count[b]` |
| `getExtraBuildingHappiness` | `extra_building_happiness[b]` |
| `getExtraBuildingHealth` | `extra_building_health[b]` |
| `getFeatureHappiness` | `feature_happiness[f]` |
| `getUnitClassCount`, `changeUnitClassCount` | `unit_class_count[uc]` |
| `getUnitClassMaking`, `changeUnitClassMaking` | `unit_class_making[uc]` |
| `getBuildingClassCount`, `changeBuildingClassCount` | `building_class_count[bc]` |
| `getBuildingClassMaking`, `changeBuildingClassMaking` | `building_class_making[bc]` |
| `getHurryCount` | `hurry_count[h]` |
| `getSpecialBuildingNotRequiredCount`, `changeSpecialBuildingNotRequiredCount` | `special_building_not_required_count[sb]` |
| `getHasCivicOptionCount`, `changeHasCivicOptionCount` | `has_civic_option_count[o]` |
| `getNoCivicUpkeepCount`, `changeNoCivicUpkeepCount` | `no_civic_upkeep_count[o]` |
| `getHasReligionCount` | `has_religion_count[r]` |
| `getUpkeepCount`, `changeUpkeepCount` | `upkeep_count[u]` |
| `getSpecialistValidCount` | `specialist_valid_count[s]` |
| `isResearchingTech`, `setResearchingTech` | `researching_tech[t]` |
| `isLoyalMember` | `loyal_member[vs]` |
| `getCivics` | `civics[o]` |
| `getSpecialistExtraYield` | `specialist_extra_yield[s][y]` |
| `getImprovementYieldChange` | `improvement_yield_change[i][y]` |
| `getScoreHistory` | `turn_history(&score_history, turn)` |
| `updateScoreHistory` | `set_turn_history(&mut score_history, turn, v)` |
| `getEconomyHistory` | `turn_history(&economy_history, turn)` |
| `updateEconomyHistory` | `set_turn_history(&mut economy_history, turn, v)` |
| `getIndustryHistory` | `turn_history(&industry_history, turn)` |
| `updateIndustryHistory` | `set_turn_history(&mut industry_history, turn, v)` |
| `getAgricultureHistory` | `turn_history(&agriculture_history, turn)` |
| `updateAgricultureHistory` | `set_turn_history(&mut agriculture_history, turn, v)` |
| `getPowerHistory` | `turn_history(&power_history, turn)` |
| `updatePowerHistory` | `set_turn_history(&mut power_history, turn, v)` |
| `getCultureHistory` | `turn_history(&culture_history, turn)` |
| `updateCultureHistory` | `set_turn_history(&mut culture_history, turn, v)` |
| `getEspionageHistory` | `turn_history(&espionage_history, turn)` |
| `updateEspionageHistory` | `set_turn_history(&mut espionage_history, turn, v)` |
| `getScriptData`, `setScriptData` | `script_data` |
| `getEspionageSpendingWeightAgainstTeam` | `espionage_spending_weight_against_team[t]` |
| `isAdaptive`, `setAdaptive` | `adaptive` |
| `isAgnostic`, `setAgnostic` | `agnostic` |
| `isAssimilation`, `setAssimilation` | `assimilation` |
| `isDeclaringWar`, `setDeclaringWar` | `declaring_war` |
| `isHideUnits`, `setHideUnits` | `hide_units` |
| `isIgnoreFood`, `setIgnoreFood` | `ignore_food` |
| `isInsane`, `setInsane` | `insane` |
| `changeNoDiplomacyWithEnemies` | `no_diplomacy_with_enemies` |
| `isSeeInvisible`, `setSeeInvisible` | `see_invisible` |
| `isSprawling`, `setSprawling` | `sprawling` |
| `getAlignment`, `setAlignment` | `alignment` |
| `getDisableProduction`, `changeDisableProduction` | `disable_production` |
| `getDisableResearch`, `changeDisableResearch` | `disable_research` |
| `getDisableSpellcasting`, `changeDisableSpellcasting` | `disable_spellcasting` |
| `getEnslavementChance`, `changeEnslavementChance` | `enslavement_chance` |
| `getFreeXPFromCombat`, `changeFreeXPFromCombat` | `free_xp_from_combat` |
| `getMaxCities`, `setMaxCities` | `max_cities` |
| `getPillagingGold`, `changePillagingGold` | `pillaging_gold` |
| `getPlayersKilled`, `changePlayersKilled` | `players_killed` |
| `getRealPlayer`, `setRealPlayer` | `real_player` |
| `getResistEnemyModify`, `changeResistEnemyModify` | `resist_enemy_modify` |
| `getResistModify`, `changeResistModify` | `resist_modify` |
| `getSanctuaryTimer` | `sanctuary_timer` |
| `changeSanctuaryTimer` | `sanctuary_timer (C++ side effects are graphics only)` |
| `getTempPlayerTimer`, `changeTempPlayerTimer` | `temp_player_timer` |
| `getStartingGold`, `changeStartingGold` | `starting_gold` |
| `getSummonDuration`, `changeSummonDuration` | `summon_duration` |
| `getUpgradeCostModifier`, `changeUpgradeCostModifier` | `upgrade_cost_modifier` |
| `getDiscoverRandModifier`, `changeDiscoverRandModifier` | `discover_rand_modifier` |
| `getHealChange`, `changeHealChange` | `heal_change` |
| `getHealChangeEnemy`, `changeHealChangeEnemy` | `heal_change_enemy` |
| `getSpecialistTypeExtraCommerce` | `specialist_type_extra_commerce[s][c]` |
| `isConquestMode` | `conquest_mode` |
| `startConquestMode` | `conquest_mode = true` |
| `getFavoriteReligion`, `setFavoriteReligion` | `favorite_religion` |
| `getArcaneTowerVictoryFlag`, `setArcaneTowerVictoryFlag` | `tower_vic_flag` |
| `AI_isSummonSuicideMode`, `AI_setSummonSuicideMode` | `sum_sui_mode` |

## List plumbing -> Vec / Slab operations

| C++ | Rust |
|---|---|
| `deleteGroupCycleNode`, `nextGroupCycleNode`, `previousGroupCycleNode`, `headGroupCycleNode`, `tailGroupCycleNode` | `players[p].group_cycle` Vec ops |
| `getLengthResearchQueue` | `players[p].research_queue.len()` |
| `nextResearchQueueNode` | iterate `players[p].research_queue` |
| `headResearchQueueNode` | `players[p].research_queue.first()` |
| `tailResearchQueueNode` | `players[p].research_queue.last()` |
| `addCityName` | `players[p].city_names.push(..)` |
| `getNumCityNames` | `players[p].city_names.len()` |
| `nextCityNameNode`, `headCityNameNode` | iterate `players[p].city_names` |
| `firstPlotGroup`, `nextPlotGroup` | iterate `players[p].plot_groups` |
| `getNumPlotGroups` | `players[p].plot_groups.len()` |
| `getPlotGroup` | `self.plot_groups.get(id)` |
| `firstCity`, `nextCity` | iterate `players[p].cities` |
| `getNumCities` | `players[p].num_cities()` |
| `getCity` | `self.cities.get(id)` |
| `firstUnit`, `nextUnit` | iterate `players[p].units` |
| `getNumUnits` | `players[p].num_units()` |
| `getUnit` | `self.units.get(id)` |
| `firstSelectionGroup`, `nextSelectionGroup` | iterate `players[p].groups` |
| `getNumSelectionGroups` | `players[p].groups.len()` |
| `getSelectionGroup` | `self.groups.get(id)` |
| `firstEventTriggered`, `nextEventTriggered` | iterate `players[p].events_triggered` |
| `getNumEventsTriggered` | `players[p].events_triggered.len()` |

## Skipped (not rules)

| C++ | lines | reason |
|---|---|---|
| `~CvPlayer` | 95-116 | destructor |
| `setupGraphical` | 837-857 | graphics |
| `cycleSelectionGroups` | 2306-2419 | UI (human unit cycling; only caller is CvGame::cycleSelectionGroups) |
| `getNameKey` | 2608-2618 | UI text keys |
| `getCivilizationDescriptionKey` | 2634-2644 | UI text keys |
| `getCivilizationShortDescriptionKey` | 2660-2670 | UI text keys |
| `getCivilizationAdjectiveKey` | 2685-2695 | UI text keys |
| `getFlagDecal` | 2698-2708 | graphics (flag/art/colors) |
| `isWhiteFlag` | 2710-2720 | graphics (flag/art/colors) |
| `getStateReligionName` | 2723-2726 | UI text keys |
| `getStateReligionKey` | 2728-2736 | UI text keys |
| `getBestAttackUnitName` | 2739-2742 | UI text keys |
| `getWorstEnemyName` | 2745-2757 | UI text keys |
| `getBestAttackUnitKey` | 2759-2794 | UI text keys |
| `getArtStyleType` | 2797-2807 | graphics (flag/art/colors) |
| `getUnitButton` | 2809-2816 | graphics (flag/art/colors) |
| `contact` | 3958-4002 | UI (opens the diplomacy screen) |
| `isConnected` | 10347-10350 | network/PBEM |
| `getNetID` | 10352-10355 | network/PBEM |
| `setNetID` | 10358-10361 | network/PBEM |
| `sendReminder` | 10363-10393 | network/PBEM |
| `getStartTime` | 10395-10398 | UI (time played) |
| `setStartTime` | 10401-10404 | UI (time played) |
| `getTotalTimePlayed` | 10407-10410 | UI (time played) |
| `setTurnActiveForPbem` | 10566-10578 | network/PBEM |
| `getPlayerColor` | 11139-11142 | graphics (flag/art/colors) |
| `getPlayerTextColorR` | 11145-11149 | graphics (flag/art/colors) |
| `getPlayerTextColorG` | 11152-11156 | graphics (flag/art/colors) |
| `getPlayerTextColorB` | 11159-11163 | graphics (flag/art/colors) |
| `getPlayerTextColorA` | 11166-11170 | graphics (flag/art/colors) |
| `isPlayable` | 11693-11696 | setup (playable flag for scenario setup) |
| `setPlayable` | 11698-11701 | setup (playable flag for scenario setup) |
| `addMessage` | 13268-13271 | UI (message/popup/diplomacy queues) |
| `clearMessages` | 13274-13277 | UI (message/popup/diplomacy queues) |
| `getGameMessages` | 13280-13283 | UI (message/popup/diplomacy queues) |
| `expireMessages` | 13286-13307 | UI (message/popup/diplomacy queues) |
| `addPopup` | 13310-13327 | UI (message/popup/diplomacy queues) |
| `clearPopups` | 13330-13342 | UI (message/popup/diplomacy queues) |
| `popFrontPopup` | 13345-13354 | UI (message/popup/diplomacy queues) |
| `getPopups` | 13357-13360 | UI (message/popup/diplomacy queues) |
| `addDiplomacy` | 13363-13369 | UI (message/popup/diplomacy queues) |
| `clearDiplomacy` | 13372-13384 | UI (message/popup/diplomacy queues) |
| `getDiplomacy` | 13387-13390 | UI (message/popup/diplomacy queues) |
| `popFrontDiplomacy` | 13393-13402 | UI (message/popup/diplomacy queues) |
| `showSpaceShip` | 13405-13411 | UI (message/popup/diplomacy queues) |
| `clearSpaceShipPopups` | 13413-13437 | UI (message/popup/diplomacy queues) |
| `getPbemEmailAddress` | 13554-13557 | network/PBEM |
| `setPbemEmailAddress` | 13559-13562 | network/PBEM |
| `getSmtpHost` | 13564-13567 | network/PBEM |
| `setSmtpHost` | 13569-13572 | network/PBEM |
| `doWarnings` | 16386-16451 | UI (enemy glow / "enemy troops spotted" messages) |
| `showMissedMessages` | 16583-16596 | UI (messages) |
| `isPbemNewTurn` | 16598-16601 | network/PBEM |
| `setPbemNewTurn` | 16603-16606 | network/PBEM |
| `read` | 16614-17129 | serialization |
| `write` | 17135-17601 | serialization |
| `buildTradeTable` | 21898-22099 | diplomacy screen |
| `getHeadingTradeString` | 22101-22145 | diplomacy screen |
| `getItemTradeString` | 22148-22303 | diplomacy screen |
| `updateTradeList` | 22305-22398 | diplomacy screen |
| `getIntroMusicScriptId` | 22400-22413 | sound |
| `getMusicScriptId` | 22415-22428 | sound |
| `getGlobeLayerColors` | 22431-22454 | UI (globe view layers) |
| `getTradeLayerColors` | 22456-22484 | UI (globe view layers) |
| `getUnitLayerColors` | 22486-22671 | UI (globe view layers) |
| `getResourceLayerColors` | 22673-22746 | UI (globe view layers) |
| `getReligionLayerColors` | 22748-22797 | UI (globe view layers) |
| `getCultureLayerColors` | 22799-22862 | UI (globe view layers) |
| `cheat` | 22864-22870 | cheat keys |
| `getUnitArtInfo` | 22872-22886 | graphics |
| `hasSpaceshipArrived` | 22888-22901 | UI (spaceship screen) |
| `getUnitArtStyleType` | 23623-23638 | graphics (unit art style) |

Also omitted: members `m_listGameMessages`, `m_listPopups`, `m_listDiplomacy` (UI queues), `m_uiStartTime`,
`m_bPbemNewTurn`, and `m_paiExtraBuildingYield` / `m_paiExtraBuildingCommerce` (declared in CvPlayer.h but never
allocated or used). CvPlayerAI members not touched by rules code (peace/espionage weights, counters, contact
timers, caches, city sites) belong to the AI crate.

## Porting notes (C++ quirks seen while reading)

- `reset()` declares a local `bool m_bSumSuiMode = false;`, so the member is never reset (`Player::new` uses false).
- `processBuilding`: `bChange` is uninitialized when `iChange == 0`, and the FFH `changeResistEnemyModify` / `changeResistModify` calls are not multiplied by `iChange` (removing a building adds the modifier again).
- `changeFreeBuildingCount`: after the Sephi "tolerant cities" fix the original `setNumFreeBuilding(eIndex, 1)` call is still there, so the fix has no effect when the count goes 0 -> 1.
- `killGoldenAgeUnits(pUnitAlive)` dereferences `pUnitAlive` in the distance loop even though NULL is accepted earlier.
- `getResearchTurnsLeft`: `getResearchTurnsLeftTimes100` returns MAX_INT when nothing is researched; `MAX_INT + 99` overflows (ported with `wrapping_add`).
- `isPirate` compares `getCivilizationType()` with the CIVILIZATION_LANUN define as ints (NO_CIVILIZATION == -1 matches a missing define).
- `acquireCity`: the `m_mapEventsOccured` loop mutates entries only, so its (hash_map) order does not matter.
- `findStartingPlot`/`addFreeUnitAI` depend on AI valuations: `AI_foundValue` is ported in `found_value.rs`; `AI_unitValue` is ported (`start.rs` `ai_unit_value`) for the UnitAIs `initFreeUnits` asks for (CITY_DEFENSE, WORKER, EXPLORE), which read only static data, techs and bonuses.
