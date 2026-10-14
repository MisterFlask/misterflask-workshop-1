# Porting map: CvGame, CvGameCoreUtils, CvMapGenerator, CvFractal

Rust layout of `CvGame` (CvGame.h/.cpp, 10,114 lines), `CvGameCoreUtils` (2,236 lines),
`CvMapGenerator` (1,358 lines) and `CvFractal` (417 lines) in `crates/ffh-core/src`. Each Rust
file below is a unit of work: it lists every C++ function it holds with its line range in
`/home/user/ref/ffh2/CvGameCoreDLL/<file>` and line count (brace to brace), the Rust name, and the
status after the skeleton phase:

- **ported** — body complete (it may call methods that are still `todo!`);
- **todo** — declared with its final signature, body `todo!("CvGame::<name>")`;
- **field** — trivial getter/setter: use the field (see "Field access" below);
- **math** — the arithmetic is ported as a pure helper, gathering inputs from game objects is `todo`.

Skeleton-phase constraint: no code reads info-struct fields beyond `.base` (the `ffh-data`
info structs were being filled in parallel), so every function that needs info fields is
`todo` even when it is otherwise trivial (e.g. `isTeamVote`, `getTechScore`).

| Rust file | C++ lines held | ported | todo |
|---|---:|---:|---:|
| `game.rs` (struct, `new`, InitCore-backed accessors) | ~110 + members | 18 | 0 |
| `game/setup_init.rs` | 2,338 | 4 (+`game_init` sequence) | 23 |
| `game/turn.rs` | 1,758 | 10 | 42 |
| `game/votes.rs` | 1,419 | 8 | 25 |
| `game/world.rs` | 1,777 | 28 | 29 |
| `utils.rs` | ~1,310 (+ CvGlobals tables) | 73 fns: 21 `MapGeom`, 32 of 51 free fns, 20 of 55 `Game` methods | 54 |
| `path.rs` | 615 (+ FAStar, exe-side) | 5 of 19 callbacks, node walks | 15 (incl. `generate_path`) |
| `mapgen/mod.rs` | 1,266 | 4 | 21 |
| `mapgen/fractal.rs` | 373 | all (parity-tested) | 0 |
| `mapgen/script.rs` | (Python map-script hooks) | trait with defaults | 0 |

CvGame totals: 187 rules methods declared (including the 18 accessors/queries in `game.rs`),
68 with complete bodies, 119 `todo!`.

## Conventions used in these files

- CvGame methods are `impl Game` methods without prefix. Names drop a leading `get`
  (`getSorenRandNum` -> `soren_rand_num`, `getGameTurn` -> `game_turn`, `getSecretaryGeneral` ->
  `secretary_general`) unless that clashes with a field: `getGlobalCounter` ->
  `get_global_counter` (field `global_counter` is `m_iGlobalCounter` = `getTrueGlobalCounter`),
  `getHolyCity` -> `get_holy_city`, `getHeadquarters`, `getShrineBuildingCount`,
  `getShrineBuilding`, `getMaxGlobalCounter`.
- C++ overloads get distinct names: `testVictory(eVictory, eTeam, pbEndScore)` ->
  `test_victory_team(v, t) -> (valid, end_score)`, `testVictory()` -> `test_victory()`;
  `addVoteTriggered(kData, iChoice)` -> `add_vote_triggered_from_selection`;
  `plotCityXY(dx, dy)` / `(dx, dy, radius)` / `(pCity, pPlot)` -> `plot_city_xy` /
  `plot_city_xy_radius` / `Game::plot_city_xy_for`; `estimateDirection(dx, dy)` / `(plot, plot)`
  -> `estimate_direction` / `estimate_direction_plots`; `directionXY` likewise.
- Values stored in CvInitCore are read through `game.rs` accessors (`game_turn()`,
  `max_turns()`, `start_era()`, `game_speed_type()`, `calendar()`, `is_victory_valid(v)`,
  `is_option(o)` (FfH locked options!), `target_score()`, ...) and written directly
  (`self.setup.max_turns = v`), except `set_game_turn` which has side effects.
- Python event reporter calls (`CvEventReporter::*`) call the `ev_*` hooks of `ffh/events.rs`
  at the C++ call site (`ev_victory`, `ev_game_end`, `ev_religion_founded`,
  `ev_corporation_founded` in ported bodies). `gameStart` has no hook yet (see Notes).
- Messages that C++ sends to every alive player are logged once (`self.log.msg(turn, None, ..)`).

## `game.rs` — struct, construction, InitCore accessors

`pub struct Game`: every CvGame data member (see doc comments for the C++ name), plus
replacements for state C++ keeps outside CvGame: `trophies` (Python `CvDataStorage`),
`map_script` (Python map script module), the FAStar instances of `GC` (`path_finder`,
`step_finder`, `route_finder`, `border_finder`, `area_finder`, `plot_group_finder`, `RefCell`).
Not ported members: `m_uiInitialTime`, `m_bDebugMode(Cache)`, `m_bPbemTurnSent`,
`m_bHotPbemBetweenTurns`, `m_bPlayerOptionsSent`, `m_ePausePlayer`, `m_listReplayMessages`,
`m_pReplayInfo`, `m_iNumSessions`; `m_eEventPlayer` and `m_apaiPlayerVote` are declared but never
used in the C++. `m_bScoreDirty` is kept (rank update timing feeds the FfH challenge options).

| C++ | file:lines | n | Rust | status |
|---|---|---:|---|---|
| `CvGame::CvGame` + `reset(NO_HANDICAP, true)` | CvGame.cpp:35-78 | 44 | `Game::new` (sizes arrays from data, handicap from setup; empty `Map::default()`, teams `Team::new`, placeholder players) | ported |
| `getMapRandNum` | CvGame.cpp:7793-7796 | 4 | `map_rand_num` | ported |
| `getSorenRandNum` | CvGame.cpp:7805-7808 | 4 | `soren_rand_num` | ported |
| `getGameTurn` | CvGame.cpp:3486-3489 | 4 | `game_turn` | ported |
| `getMaxTurns` | CvGame.cpp:3540-3543 | 4 | `max_turns` | ported |
| `getMaxCityElimination` | CvGame.cpp:3559-3562 | 4 | `max_city_elimination` | ported |
| `getNumAdvancedStartPoints` | CvGame.cpp:3571-3574 | 4 | `num_advanced_start_points` | ported |
| `getTargetScore` | CvGame.cpp:3742-3745 | 4 | `target_score` | ported |
| `getGameSpeedType` | CvGame.cpp:4937-4940 | 4 | `game_speed_type` | ported |
| `getStartEra` | CvGame.cpp:4943-4946 | 4 | `start_era` | ported |
| `getCalendar` | CvGame.cpp:4949-4952 | 4 | `calendar` | ported |
| `isVictoryValid` | CvGame.cpp:5417-5422 | 6 | `is_victory_valid` | ported |
| `isOption` | CvGame.cpp:5073-5092 | 20 | `is_option` | ported |
| `getNumHumanPlayers` | CvGame.cpp:3481-3484 | 4 | `num_human_players` | ported |
| `isNetworkMultiPlayer` | CvGame.cpp:3419-3422 | 4 | `is_network_multi_player` (false) | ported |
| `isGameMultiPlayer` | CvGame.cpp:3425-3428 | 4 | `is_game_multi_player` (false) | ported |
| `isHotSeat` | CvGame.cpp:4589-4592 | 4 | `is_hot_seat` (false) | ported |
| `isPbem` | CvGame.cpp:4594-4597 | 4 | `is_pbem` (false) | ported |
| `isSimultaneousTeamTurns` | CvGame.cpp:4606-4619 | 14 | `is_simultaneous_team_turns` (false) | ported |

## `game/setup_init.rs` — new game creation (2,338 lines)

| C++ | lines | n | Rust | status |
|---|---|---:|---|---|
| (exe new-game sequence) | — | — | `game_init`: `map_init(None)`, `init`, `team_init` per team, stub `game_init_players` (`CvPlayer::init`), `init_diplomacy`, map generation, ... | sequence |
| `init` | 93-344 | 252 | `init(handicap)` | todo |
| `setInitialItems` | 349-380 | 32 | `set_initial_items` | todo |
| `regenerateMap` | 383-466 | 84 | `regenerate_map` | todo |
| `uninit` | 469-519 | 51 | part of `reset` | ported |
| `reset` | 524-763 | 240 | `reset(handicap)` | ported |
| `initDiplomacy` | 766-827 | 62 | `init_diplomacy` | todo |
| `initFreeState` | 830-893 | 64 | `init_free_state` | todo |
| `initFreeUnits` | 896-910 | 15 | `init_free_units` | todo |
| `assignStartingPlots` | 913-1131 | 219 | `assign_starting_plots` | todo |
| `normalizeStartingPlotLocations` | 1136-1253 | 118 | `normalize_starting_plot_locations` | todo |
| `normalizeAddRiver` | 1256-1308 | 53 | `normalize_add_river` | todo |
| `normalizeRemovePeaks` | 1311-1347 | 37 | `normalize_remove_peaks` | todo |
| `normalizeAddLakes` | 1349-1362 | 14 | `normalize_add_lakes` | todo |
| `normalizeFindLakePlot` | 1364-1418 | 55 | `normalize_find_lake_plot` | todo |
| `normalizeRemoveBadFeatures` | 1421-1495 | 75 | `normalize_remove_bad_features` | todo |
| `normalizeRemoveBadTerrain` | 1498-1589 | 92 | `normalize_remove_bad_terrain` | todo |
| `normalizeAddFoodBonuses` | 1592-1699 | 108 | `normalize_add_food_bonuses` | todo |
| `normalizeAddGoodTerrain` | 1702-1804 | 103 | `normalize_add_good_terrain` | todo |
| `normalizeAddExtras` | 1807-2104 | 298 | `normalize_add_extras` | todo |
| `normalizeStartingPlots` | 2107-2163 | 57 | `normalize_starting_plots` | ported |
| `getTeamClosenessScore` | 2171-2227 | 57 | `team_closeness_score` | todo |
| `initScoreCalculation` | 3996-4066 | 71 | `init_score_calculation` | todo |
| `setFinalInitialized` | 4627-4652 | 26 | `set_final_initialized` | todo |
| `addPlayer` | 8623-8664 | 42 | `add_player` | todo |
| `addPlayerAdvanced` | 8667-8744 | 78 | `add_player_advanced` | todo |
| `isEventActive` | 9623-9634 | 12 | `is_event_active` | ported |
| `initEvents` | 9636-9645 | 10 | `init_events` | todo |
| `pythonIsBonusIgnoreLatitudes` | 9724-9736 | 13 | `python_is_bonus_ignore_latitudes` | ported |

## `game/turn.rs` — update loop, turn processing, queries, score and victory (1,758 lines)

| C++ | lines | n | Rust | status |
|---|---|---:|---|---|
| `update` | 2230-2294 | 65 | `update` | todo |
| `updateScore` | 2297-2384 | 88 | `update_score(force)` | todo |
| `updatePlotGroups` | 2386-2399 | 14 | `update_plot_groups` | todo |
| `updateBuildingCommerce` | 2402-2413 | 12 | `update_building_commerce` | todo |
| `updateCitySight` | 2416-2429 | 14 | `update_city_sight` | todo |
| `updateTradeRoutes` | 2432-2443 | 12 | `update_trade_routes` | todo |
| `testExtendedGame` | 2446-2469 | 24 | `test_extended_game` | todo |
| `implementDeal` | 2683-2698 | 16 | `implement_deal` | todo |
| `verifyDeals` | 2701-2710 | 10 | `verify_deals` | todo |
| `getAdjustedPopulationPercent` | 2737-2776 | 40 | `adjusted_population_percent` | todo |
| `getProductionPerPopulation` | 2779-2786 | 8 | `production_per_population` | todo |
| `getAdjustedLandPercent` | 2789-2803 | 15 | `adjusted_land_percent` | todo |
| `countCivPlayersAlive` | 3046-3059 | 14 | `count_civ_players_alive` | todo |
| `countCivPlayersEverAlive` | 3062-3079 | 18 | `count_civ_players_ever_alive` | todo |
| `countCivTeamsAlive` | 3082-3095 | 14 | `count_civ_teams_alive` | todo |
| `countCivTeamsEverAlive` | 3098-3115 | 18 | `count_civ_teams_ever_alive` | todo |
| `countHumanPlayersAlive` | 3118-3132 | 15 | `count_human_players_alive` | todo |
| `countTotalCivPower` | 3135-3149 | 15 | `count_total_civ_power` | todo |
| `countTotalNukeUnits` | 3152-3165 | 14 | `count_total_nuke_units` | todo |
| `countKnownTechNumTeams` | 3168-3184 | 17 | `count_known_tech_num_teams` | todo |
| `goldenAgeLength` | 3295-3305 | 11 | `golden_age_length` | todo |
| `victoryDelay` | 3307-3317 | 11 | `victory_delay` | todo |
| `getCurrentEra` | 3366-3390 | 25 | `current_era` | todo |
| `isTeamGame` | 3431-3435 | 5 | `is_team_game` | todo |
| `setGameTurn` | 3492-3506 | 15 | `set_game_turn` | ported |
| `incrementGameTurn` | 3509-3512 | 4 | `increment_game_turn` | ported |
| `getTurnYear` | 3515-3519 | 5 | `turn_year` | ported |
| `getGameTurnYear` | 3522-3525 | 4 | `game_turn_year` | ported |
| `countNumHumanGameTurnActive` | 3761-3780 | 20 | `count_num_human_game_turn_active` | todo |
| `getNumCivCities` | 3796-3799 | 4 | `num_civ_cities` | todo |
| `changeTradeRoutes` | 3828-3837 | 10 | `change_trade_routes` | ported |
| `isFreeTrade` | 3846-3849 | 4 | `is_free_trade` | ported |
| `changeFreeTradeCount` | 3852-3868 | 17 | `change_free_trade_count` | ported |
| `isNoNukes` | 3877-3880 | 4 | `is_no_nukes` | ported |
| `setAIAutoPlay` | 4075-4095 | 21 | `set_ai_auto_play` | todo |
| `changeAIAutoPlay` | 4098-4106 | 9 | `change_ai_auto_play` | todo |
| `circumnavigationAvailable` | 4144-4169 | 26 | `circumnavigation_available` | todo |
| `getBestLandUnitCombat` | 4829-4837 | 9 | `best_land_unit_combat` | todo |
| `setWinner` | 4863-4894 | 32 | `set_winner` | ported |
| `setGameState` | 4903-4934 | 32 | `set_game_state` | ported |
| `isInAdvancedStart` | 5493-5504 | 12 | `is_in_advanced_start` | todo |
| `doTurn` | 5830-6100 | 271 | `do_turn` | todo |
| `doDeals` | 6103-6114 | 12 | `do_deals` | todo |
| `doGlobalWarming` | 6117-6201 | 85 | `do_global_warming` | todo |
| `updateWar` | 6947-6977 | 31 | `update_war` | todo |
| `updateMoves` | 6980-7072 | 93 | `update_moves` | todo |
| `verifyCivics` | 7075-7086 | 12 | `verify_civics` | todo |
| `updateTimers` | 7089-7100 | 12 | `update_timers` | todo |
| `testAlive` | 7133-7141 | 9 | `test_alive` | todo |
| `testVictory(eVictory, eTeam, pbEndScore)` | 7143-7451 | 309 | `test_victory_team` | todo |
| `testVictory()` | 7453-7550 | 98 | `test_victory` | todo |
| `calculateSyncChecksum` | 7811-7913 | 103 | `calculate_sync_checksum` | todo |

## `game/votes.rs` — diplomatic votes (1,419 lines)

| C++ | lines | n | Rust | status |
|---|---|---:|---|---|
| `isTeamVote` | 2806-2809 | 4 | `is_team_vote` | todo |
| `isChooseElection` | 2812-2815 | 4 | `is_choose_election` | todo |
| `isTeamVoteEligible` | 2818-2875 | 58 | `is_team_vote_eligible` | todo |
| `countVote` | 2878-2894 | 17 | `count_vote` | todo |
| `countPossibleVote` | 2897-2910 | 14 | `count_possible_vote` | todo |
| `findHighestVoteTeam` | 2914-2937 | 24 | `find_highest_vote_team` | todo |
| `getVoteRequired` | 2940-2943 | 4 | `vote_required` | todo |
| `getSecretaryGeneral` | 2946-2993 | 48 | `secretary_general` | todo |
| `canHaveSecretaryGeneral` | 2995-3009 | 15 | `can_have_secretary_general` | todo |
| `clearSecretaryGeneral` | 3011-3032 | 22 | `clear_secretary_general` | todo |
| `updateSecretaryGeneral` | 3034-3044 | 11 | `update_secretary_general` | todo |
| `isDiploVote` | 4171-4190 | 20 | `is_diplo_vote` | todo |
| `changeDiploVote` | 4200-4219 | 20 | `change_diplo_vote` | todo |
| `canDoResolution` | 4221-4262 | 42 | `can_do_resolution` | todo |
| `isValidVoteSelection` | 4264-4526 | 263 | `is_valid_vote_selection` | todo |
| `isVotePassed` | 5300-5312 | 13 | `is_vote_passed` | todo |
| `setVoteOutcome` | 5315-5343 | 29 | `set_vote_outcome` | todo |
| `setVoteChosen` | 5506-5515 | 10 | `set_vote_chosen` | ported |
| `getPlayerVote` | 5695-5702 | 8 | `player_vote` | todo |
| `setPlayerVote` | 5705-5712 | 8 | `set_player_vote` | todo |
| `castVote` | 5715-5761 | 47 | `cast_vote` | todo |
| `doDiploVote` | 6455-6460 | 6 | `do_diplo_vote` | ported |
| `processVote` | 7553-7742 | 190 | `process_vote` | todo |
| `setVoteSourceReligion` | 8904-8923 | 20 | `set_vote_source_religion` | ported |
| `getVoteSelection` | 9092-9095 | 4 | `vote_selection` | ported |
| `addVoteSelection` | 9097-9237 | 141 | `add_vote_selection` | todo |
| `deleteVoteSelection` | 9239-9242 | 4 | `delete_vote_selection` | ported |
| `getVoteTriggered` | 9244-9247 | 4 | `vote_triggered` | ported |
| `addVoteTriggered(kData, iChoice)` | 9249-9257 | 9 | `add_vote_triggered_from_selection` | ported |
| `addVoteTriggered(eVoteSource, kOptionData)` | 9259-9291 | 33 | `add_vote_triggered` | todo |
| `deleteVoteTriggered` | 9293-9296 | 4 | `delete_vote_triggered` | ported |
| `doVoteResults` | 9298-9518 | 221 | `do_vote_results` | todo |
| `doVoteSelection` | 9520-9621 | 102 | `do_vote_selection` | todo |

## `game/world.rs` — religions, corporations, limits, caches, barbarians, FfH counters (1,777 lines)

| C++ | lines | n | Rust | status |
|---|---|---:|---|---|
| `getNumFreeBonuses` | 3187-3197 | 11 | `num_free_bonuses` | todo |
| `countReligionLevels` | 3200-3216 | 17 | `count_religion_levels` | todo |
| `countCorporationLevels` | 3218-3232 | 15 | `count_corporation_levels` | todo |
| `replaceCorporation` | 3234-3260 | 27 | `replace_corporation` | todo |
| `calculateReligionPercent` | 3263-3292 | 30 | `calculate_religion_percent` | todo |
| `getImprovementUpgradeTime` | 3321-3334 | 14 | `improvement_upgrade_time` | todo |
| `canTrainNukes` | 3337-3363 | 27 | `can_train_nukes` | todo |
| `isUnitClassMaxedOut` | 5149-5162 | 14 | `is_unit_class_maxed_out` | todo |
| `isBuildingClassMaxedOut` | 5181-5194 | 14 | `is_building_class_maxed_out` | todo |
| `isProjectMaxedOut` | 5213-5226 | 14 | `is_project_maxed_out` | todo |
| `isForceCivic` | 5245-5248 | 4 | `is_force_civic` | ported |
| `isForceCivicOption` | 5251-5267 | 17 | `is_force_civic_option` | todo |
| `changeForceCivicCount` | 5270-5289 | 20 | `change_force_civic_count` | ported |
| `isReligionFounded` | 5354-5357 | 4 | `is_religion_founded` | ported |
| `makeReligionFounded` | 5360-5372 | 13 | `make_religion_founded` | ported |
| `isCorporationFounded` | 5397-5400 | 4 | `is_corporation_founded` | ported |
| `makeCorporationFounded` | 5403-5415 | 13 | `make_corporation_founded` | ported |
| `makeSpecialBuildingValid` | 5456-5479 | 24 | `make_special_building_valid` | ported |
| `getHolyCity` | 5518-5523 | 6 | `get_holy_city` | ported |
| `setHolyCity` | 5526-5620 | 95 | `set_holy_city` | todo |
| `getHeadquarters` | 5623-5628 | 6 | `get_headquarters` | ported |
| `setHeadquarters` | 5631-5692 | 62 | `set_headquarters` | todo |
| `isDestroyedCityName` | 5787-5800 | 14 | `is_destroyed_city_name` | ported |
| `isGreatPersonBorn` | 5807-5820 | 14 | `is_great_person_born` | ported |
| `doHolyCity` | 6204-6322 | 119 | `do_holy_city` | todo |
| `doHeadquarters` | 6325-6452 | 128 | `do_headquarters` | todo |
| `createBarbarianCities` | 6463-6584 | 122 | `create_barbarian_cities` | todo |
| `createBarbarianUnits` | 6587-6807 | 221 | `create_barbarian_units` | todo |
| `createAnimals` | 6810-6945 | 136 | `create_animals` | todo |
| `isCompetingCorporation` | 8747-8769 | 23 | `is_competing_corporation` | todo |
| `getPlotExtraYield` | 8771-8782 | 12 | `plot_extra_yield` | ported |
| `setPlotExtraYield` | 8784-8822 | 39 | `set_plot_extra_yield` | todo |
| `removePlotExtraYield` | 8824-8840 | 17 | `remove_plot_extra_yield` | todo |
| `getPlotExtraCost` | 8842-8853 | 12 | `plot_extra_cost` | ported |
| `changePlotExtraCost` | 8855-8877 | 23 | `change_plot_extra_cost` | ported |
| `removePlotExtraCost` | 8879-8889 | 11 | `remove_plot_extra_cost` | ported |
| `getShrineBuildingCount` | 8930-8941 | 12 | `get_shrine_building_count` | ported |
| `getShrineBuilding` | 8943-8968 | 26 | `get_shrine_building` | ported |
| `changeShrineBuilding` | 8970-9016 | 47 | `change_shrine_building` (removal quirk kept) | ported |
| `culturalVictoryValid` | 9018-9026 | 9 | `cultural_victory_valid` | ported |
| `culturalVictoryCultureLevel` | 9033-9041 | 9 | `cultural_victory_culture_level` | ported |
| `getCultureThreshold` | 9043-9052 | 10 | `culture_threshold` | todo |
| `doUpdateCacheOnTurn` | 9054-9090 | 37 | `do_update_cache_on_turn` | todo |
| `isCivEverActive` | 9647-9662 | 16 | `is_civ_ever_active` | todo |
| `isLeaderEverActive` | 9664-9679 | 16 | `is_leader_ever_active` | todo |
| `isUnitEverActive` | 9681-9695 | 15 | `is_unit_ever_active` | todo |
| `isBuildingEverActive` | 9697-9711 | 15 | `is_building_ever_active` | todo |
| `processBuilding` | 9713-9722 | 10 | `process_building` | todo |
| `getGlobalCounter` | 9803-9806 | 4 | `get_global_counter` | ported |
| `getMaxGlobalCounter` | 9813-9816 | 4 | `get_max_global_counter` | ported |
| `changeGlobalCounter` | 9823-9857 | 35 | `change_global_counter` | ported |
| `foundBarbarianCity` | 9879-9962 | 84 | `found_barbarian_city` | todo |
| `getTrophyValue` | 10024-10037 | 14 | `trophy_value` | ported |
| `isHasTrophy` | 10039-10042 | 4 | `is_has_trophy` | ported |
| `setTrophyValue` | 10044-10054 | 11 | `set_trophy_value` | ported |
| `changeTrophyValue` | 10056-10059 | 4 | `change_trophy_value` | ported |
| `isReligionDisabled` | 10061-10113 | 53 | `is_religion_disabled` | ported |

## Field access (trivial CvGame getters/setters)

| C++ | Rust |
|---|---|
| `getElapsedGameTurns` / `incrementElapsedGameTurns` | `elapsed_game_turns` / `+= 1` |
| `setMaxTurns` / `changeMaxTurns` | `setup.max_turns` |
| `setMaxCityElimination`, `setNumAdvancedStartPoints`, `setTargetScore` | `setup.max_city_elimination`, `setup.num_advanced_start_points`, `setup.target_score` |
| `get/setStartTurn`, `get/setStartYear`, `get/setEstimateEndTurn` | `start_turn`, `start_year`, `estimate_end_turn` |
| `getTurnSlice` / `setTurnSlice` / `changeTurnSlice` | `turn_slice` |
| `get/set/changeCutoffSlice` | `cutoff_slice` |
| `getNumGameTurnActive` / `changeNumGameTurnActive` | `num_game_turn_active` |
| `getNumCities` / `changeNumCities` | `num_cities` |
| `getTotalPopulation` / `changeTotalPopulation` | `total_population` |
| `getTradeRoutes`, `getFreeTradeCount`, `getNoNukesCount` / `changeNoNukesCount` | `trade_routes`, `free_trade_count`, `no_nukes_count` |
| `get/set/changeSecretaryGeneralTimer`, `get/set/changeVoteTimer` | `secretary_general_timer[vs]`, `vote_timer[vs]` |
| `getNukesExploded` / `changeNukesExploded` | `nukes_exploded` |
| `getMax/InitPopulation/Land/Tech/Wonders` | `max_population`, ..., `init_wonders` |
| `getAIAutoPlay` | `ai_auto_play` |
| `isScoreDirty` / `setScoreDirty` | `score_dirty` |
| `isCircumnavigated` / `makeCircumnavigated` | `circumnavigated` (`= true`) |
| `getDiploVoteCount` | `diplo_vote[vs]` |
| `isFinalInitialized` | `final_initialized` |
| `getHandicapType` / `setHandicapType` | `handicap` |
| `getBestLandUnit` / `setBestLandUnit` | `best_land_unit` |
| `getWinner`, `getVictory`, `getGameState` | `winner`, `victory`, `game_state` |
| `get/setRankPlayer`, `get/setPlayerRank`, `get/setPlayerScore` | `rank_player[rank]`, `player_rank[p]`, `player_score[p]` |
| `get/setRankTeam`, `get/setTeamRank`, `get/setTeamScore` | `rank_team[rank]`, `team_rank[t]`, `team_score[t]` |
| `setOption`, `setVictoryValid` | `setup.options[o]`, `setup.victories[v]` |
| `get/increment/decrementUnitCreatedCount` | `unit_created_count[u]` |
| `get/increment/decrementUnitClassCreatedCount` | `unit_class_created_count[uc]` |
| `get/incrementBuildingClassCreatedCount` | `building_class_created_count[bc]` |
| `getProjectCreatedCount` / `incrementProjectCreatedCount(e, iExtra)` | `project_created_count[p]` (`+= extra`) |
| `getForceCivicCount` | `force_civic_count[c]` |
| `getVoteOutcome` | `vote_outcome[v]` |
| `getReligionGameTurnFounded`, `get/setReligionSlotTaken` | `religion_game_turn_founded[r]`, `religion_slot_taken[r]` |
| `getCorporationGameTurnFounded` | `corporation_game_turn_founded[c]` |
| `isSpecialUnitValid` / `makeSpecialUnitValid` | `special_unit_valid[su]` |
| `isSpecialBuildingValid` | `special_building_valid[sb]` |
| `isNukesValid` / `makeNukesValid` | `nukes_valid` |
| `get/setScriptData` | `script_data` |
| `addDestroyedCityName`, `addGreatPersonBornName` | `destroyed_cities.push`, `great_people_born.push` |
| `getNumDeals`, `getDeal`, `addDeal`, `deleteDeal`, `first/nextDeal`, `getIndexAfterLastDeal` | `deals` slab (`len`, `get`, `insert`, `remove`, `iter`) |
| `getMapRand`, `getSorenRand` | `map_rand`, `soren_rand` |
| `culturalVictoryNumCultureCities` | `num_culture_victory_cities` |
| `getVoteSourceReligion` | `vote_source_religions[vs]` |
| FfH `get/changeCrime`, `get/changeCutLosersCounter`, `get/changeFlexibleDifficultyCounter`, `get/changeHighToLowCounter`, `get/changeIncreasingDifficultyCounter`, `get/changeGlobalCounterLimit`, `get/changeScenarioCounter` | `crime`, `cut_losers_counter`, `flexible_difficulty_counter`, `high_to_low_counter`, `increasing_difficulty_counter`, `global_counter_limit`, `scenario_counter` |
| FfH `getTrueGlobalCounter`, `getTrueMaxGlobalCounter` | `global_counter`, `max_global_counter` |
| FfH `is/setEventTriggered`, `is/setGamblingRing`, `is/setNoBonus`, `is/setNoOutsideTechTrades`, `is/setSlaveTrade`, `is/setSmugglingRing` | `event_triggered[t]`, `gambling_ring[vs]`, `no_bonus[b]`, `no_outside_tech_trades[vs]`, `slave_trade[vs]`, `smuggling_ring[vs]` |

## Not ported (UI / network / replay / serialization)

CvGame.cpp: `cityPushOrder` 2472-2486, `selectUnit` 2489-2551, `selectGroup` 2554-2611, `selectAll`
2614-2635, `selectionListIgnoreBuildingDefense` 2638-2680, `getGlobeviewConfigurationParameters`
2716-2728, `getSymbolID` 2731-2734, `getActiveTeam` 3393-3403, `getActiveCivilizationType`
3406-3416, `isModem`/`setModem` 3438-3454, `reviveActivePlayer` 3457-3478 (end of AI autoplay),
`setName`/`getName`, `getMinutesPlayed` 3625-3628, `getTurnSlicesRemaining`, `resetTurnTimer`
3667-3687, `incrementTurnTimer` 3689-3699, `getMaxTurnLen` 3702-3739, `updateTurnTimer` 7103-7130
(MP turn timer, never on), `get/setInitialTime`, debug mode 4529-4576, pitboss/PBEM/player options
4578-4703, `get/setActivePlayer` 4706-4778, `updateUnitEnemyGlow`, pause player 4805-4820,
`isMPOption`/`isForcedControl` and setters, `calculateOptionsChecksum` 7916-7938, replay
messages 7941-8073, `read`/`write`/`writeReplay`/`saveReplay` 8077-8511, `showEndGameSequence`
8514-8605, replay info and `hasSkippedSaveChecksum` 8607-8621. CvGameInterface.cpp (all UI:
colored/blockaded plots, selection, actions, controls, flyout menus, soundtracks, end turn
buttons). CvGameCoreUtils.cpp: `directionAngle` 147-161, `setListHelp` x3 976-1016,
`PUF_isSelected` 1232-1235, `PUF_makeInfoBarDirty` 1237-1241, debug strings 2037-2236.
`getCity`/`getUnit` 202-220 are `self.cities.get(id)` / `self.units.get(id)`.

## `utils.rs` — CvGameCoreUtils (geometry, formulas, predicates, PUF_*)

Also holds the CvGlobals tables (CvGlobals.cpp:240-365: `PLOT_DIRECTION_X/Y`,
`PLOT_CARDINAL_DIRECTION_X/Y`, `CITY_PLOT_X/Y`, `CITY_PLOT_PRIORITY`, `XY_CITY_PLOT`,
`TURN_LEFT/RIGHT_DIRECTION`, `XY_DIRECTION`) and [`MapGeom`], a copyable snapshot of the
map's grid size and wrap flags that mirrors the CvMap.h inline index helpers (`isPlotINLINE`,
`plotNumINLINE`, `plotX/Y`, `plotINLINE`, `plotSorenINLINE`; `coordRange` CvMap.h:24-40 is
`crate::map::coord_range`) for code without a `Game` (path finders, tests).
`Game::map_geom()` builds it from `Map::{grid_width, grid_height, wrap_x, wrap_y}`; the `Game`
distance wrappers go through it, the plot lookups through `Map::plot_xy` / `Map::xy`.

| C++ | file:lines | n | Rust | status |
|---|---|---:|---|---|
| `getSign` | .h:44 | 1 | `get_sign` | ported |
| `range` (int, float) | .h:46-80 | 35 | `range`, `range_f` | ported |
| `coordDistance`, `wrapCoordDifference` | .h:82-107 | 26 | `coord_distance`, `wrap_coord_difference` | ported |
| `xDistance`, `yDistance`, `dxWrap`, `dyWrap` | .h:109-127 | 19 | `MapGeom::*` + `Game::*` | ported |
| `plotDistance`, `stepDistance` | .h:144-173 | 14 | `MapGeom::*` + `Game::*` | ported |
| `plotDirection`, `plotCardinalDirection`, `plotXY` | .h:175-195 | 23 | `MapGeom::*` + `Game::plot_direction(_opt)`, `plot_cardinal_direction`, `plot_xy` | ported |
| `directionXY` (deltas, plots) | .h:197-212 | 16 | `direction_xy`, `MapGeom/Game::direction_xy_plots` | ported |
| `plotCity` | 31-47 | 17 | `Game::plot_city` (geometry: `MapGeom::plot_city_offset`) | todo |
| `plotCityXY(dx, dy)` | 49-59 | 11 | `plot_city_xy` | ported |
| `plotCityXY(dx, dy, iRadius)` | 62-73 | 12 | `plot_city_xy_radius` | ported |
| `plotCityXY(pCity, pPlot)` | 76-84 | 9 | `Game::plot_city_xy_for` | todo |
| `getOppositeCardinalDirection` | 86-89 | 4 | `get_opposite_cardinal_direction` | ported |
| `cardinalDirectionToDirection` | 91-105 | 15 | `cardinal_direction_to_direction` | ported |
| `isCardinalDirection` | 107-118 | 12 | `is_cardinal_direction` | ported |
| `estimateDirection` (deltas, plots) | 120-144 | 24 | `estimate_direction`, `*_plots` | ported |
| `atWar` | 163-174 | 12 | `Game::at_war` (`Team::at_war`, C++ asserts as `debug_assert!`), `at_war_opt` (NO_TEAM) | ported |
| `isPotentialEnemy` | 176-200 | 25 | `Game::is_potential_enemy` | todo |
| `isBeforeUnitCycle` | 222-259 | 38 | `Game::is_before_unit_cycle` | todo |
| `isPromotionValid` | 261-373 | 113 | `is_promotion_valid(d, ..)` | todo |
| `getPopulationAsset` .. `getLandPlotsScore` | 375-398 | 24 | `get_population_asset`, ... | ported |
| `getTechScore` | 400-403 | 4 | `get_tech_score(d, ..)` | todo |
| `getWonderScore` | 405-415 | 11 | `get_wonder_score(d, ..)` | ported |
| `finalImprovementUpgrade` | 417-434 | 18 | `final_improvement_upgrade(d, ..)` | todo |
| `getWorldSizeMaxConscript` | 436-446 | 11 | `Game::get_world_size_max_conscript` | todo |
| `isReligionTech`, `isCorporationTech` | 448-476 | 28 | `is_religion_tech`, `is_corporation_tech` | todo |
| `isTechRequiredForUnit/Building/Project` | 478-534 | 55 | `is_tech_required_for_*` | todo |
| `isWorld/Team/NationalUnitClass` | 536-549 | 12 | `is_*_unit_class` | todo |
| `isLimitedUnitClass` | 551-554 | 4 | `is_limited_unit_class` | ported |
| `isWorld/Team/NationalWonderClass` | 556-569 | 12 | `is_*_wonder_class` | todo |
| `isLimitedWonderClass` | 571-574 | 4 | `is_limited_wonder_class` | ported |
| `limitedWonderClassLimit` | 576-604 | 29 | `limited_wonder_class_limit` | todo |
| `isWorldProject`, `isTeamProject` | 606-614 | 8 | `is_world_project`, `is_team_project` | todo |
| `isLimitedProject` | 616-619 | 4 | `is_limited_project` | ported |
| `getBinomialCoefficient` | 627-641 | 15 | `get_binomial_coefficient` | ported |
| `getCombatOdds` | 647-874 | 228 | `combat_odds(&CombatOddsInput, ..)` + `Game::get_combat_odds` | math (tested vs C++ transcription) |
| `getEspionageModifier` | 876-888 | 13 | `espionage_modifier` + `Game::get_espionage_modifier` | math |
| `setTradeItem` | 890-896 | 7 | `set_trade_item` | ported |
| `isPlotEventTrigger` | 898-946 | 49 | `is_plot_event_trigger(d, ..)` | todo |
| `getDiscoveryTech` | 948-973 | 26 | `Game::get_discovery_tech` | todo |
| `PUF_*` (37 filters) | 1018-1251 | ~225 | `Game::puf_*` (`PlotUnitFunc`); the 5 composed ones ported | todo |
| `baseYieldToSymbol` | 1906-1917 | 12 | `base_yield_to_symbol` | ported |
| `isPickableName` | 1920-1933 | 14 | `is_pickable_name` | ported |
| `shuffle`, `shuffleArray` | 1937-1965 | 27 | `shuffle`, `shuffle_array` | ported |
| `getTurnYearForGame` | 1967-1970 | 4 | `get_turn_year_for_game` | ported |
| `getTurnMonthForGame` | 1973-2033 | 61 | `turn_month_for_game` + `get_turn_month_for_game(d, ..)` | math |

## `path.rs` — FAStar replacement and finder callbacks (CvGameCoreUtils.cpp)

`PathFinder` replaces the exe's `FAStar` (no source; `FAStarNode.h` and
`CvDLLFAStarIFaceBase.h` give the interface): `generate_path(g, start, dest, cardinal_only,
info, reuse)` is `todo` (A* with the callbacks of its `FinderKind`; dest (-1, -1) = flood fill;
the `reuse` semantics and the exe's tie-breaking among equal-cost nodes are unknown, so paths with
equal costs may differ from C++). `path_first_xy`, `path_end_turn_xy`, `path_turns` are the node
walks of `CvSelectionGroup::getPathFirstPlot`/`getPathEndTurnPlot`/`generatePath`
(CvSelectionGroup.cpp:3936-4020), ported. The notify callbacks that mutate state (`joinArea`,
`changeIrrigated`, `countPlotGroup`) only act on `ASNL_ADDCLOSED` and do not affect the search.

| C++ | lines | n | Rust | status |
|---|---|---:|---|---|
| `potentialIrrigation` | 1253-1261 | 9 | `Game::potential_irrigation` | todo |
| `checkFreshWater` | 1264-1275 | 12 | `Game::check_fresh_water` | todo |
| `changeIrrigated` | 1278-1286 | 9 | `Game::change_irrigated` | todo |
| `pathDestValid` | 1289-1399 | 111 | `Game::path_dest_valid` | todo |
| `pathHeuristic` | 1402-1405 | 4 | `Game::path_heuristic` | ported |
| `pathCost` | 1408-1544 | 137 | `Game::path_cost` | todo |
| `pathValid` | 1547-1648 | 102 | `Game::path_valid` | todo |
| `pathAdd` | 1651-1714 | 64 | `Game::path_add` | todo |
| `stepDestValid` | 1717-1735 | 19 | `Game::step_dest_valid` | todo |
| `stepHeuristic` | 1738-1741 | 4 | `Game::step_heuristic` | ported |
| `stepCost` | 1744-1747 | 4 | `step_cost` | ported |
| `stepValid` | 1750-1772 | 23 | `Game::step_valid` | todo |
| `stepAdd` | 1775-1789 | 15 | `step_add` | ported |
| `routeValid` | 1792-1815 | 24 | `Game::route_valid` | todo |
| `borderValid` | 1818-1838 | 21 | `Game::border_valid` | todo |
| `areaValid` | 1841-1849 | 9 | `Game::area_valid` | todo |
| `joinArea` | 1852-1860 | 9 | `Game::join_area` | todo |
| `plotGroupValid` | 1863-1892 | 30 | `Game::plot_group_valid` | todo |
| `countPlotGroup` | 1895-1903 | 9 | `count_plot_group` | ported |

## `mapgen/mod.rs` — CvMapGenerator (1,266 lines)

`impl Game` methods with the `mapgen_` prefix (the C++ class is a stateless singleton). Python
map-script calls go through `self.map_script` ([`MapScript`], `mapgen/script.rs`: one hook per
`getMapScriptModule()` call site in CvMap/CvMapGenerator/CvGame/CvPlayer; `None`/`false` = use
the DLL default). Note the C++ default `generatePlotTypes` is all land: a real map needs a
`MapScript`. The map port's `mapscript.rs` (Fractal.py / FractalWorld on top of `Fractal`; not
yet in `lib.rs`) compiles against this `Fractal` API and can be wrapped in a `MapScript` impl
(`generate_plot_types`, `generate_terrain_types`, `add_features`).

| C++ | lines | n | Rust | status |
|---|---|---:|---|---|
| `canPlaceBonusAt` | 45-164 | 120 | `mapgen_can_place_bonus_at` | todo |
| `canPlaceGoodyAt` | 167-236 | 70 | `mapgen_can_place_goody_at` | todo |
| `addGameElements` | 239-269 | 31 | `mapgen_add_game_elements` | ported |
| `addLakes` | 272-308 | 37 | `mapgen_add_lakes` | todo |
| `addRivers` | 310-379 | 70 | `mapgen_add_rivers` | todo |
| `doRiver` | 383-538 | 156 | `mapgen_do_river` | todo |
| `addRiver` | 546-651 | 106 | `mapgen_add_river` | todo |
| `addFeatures` | 654-682 | 29 | `mapgen_add_features` | todo |
| `addBonuses` | 684-720 | 37 | `mapgen_add_bonuses` | todo |
| `addUniqueBonusType` | 722-836 | 115 | `mapgen_add_unique_bonus_type` | todo |
| `addNonUniqueBonusType` | 838-895 | 58 | `mapgen_add_non_unique_bonus_type` | todo |
| `addGoodies` | 898-946 | 49 | `mapgen_add_goodies` | todo |
| `eraseRivers` | 949-965 | 17 | `mapgen_erase_rivers` | todo |
| `eraseFeatures` | 967-976 | 10 | `mapgen_erase_features` | todo |
| `eraseBonuses` | 978-987 | 10 | `mapgen_erase_bonuses` | todo |
| `eraseGoodies` | 989-1001 | 13 | `mapgen_erase_goodies` | todo |
| `generateRandomMap` | 1009-1030 | 22 | `mapgen_generate_random_map` | ported |
| `generatePlotTypes` | 1032-1059 | 28 | `mapgen_generate_plot_types` | ported |
| `generateTerrain` | 1061-1078 | 18 | `mapgen_generate_terrain` | todo |
| `afterGeneration` | 1081-1086 | 6 | `mapgen_after_generation` | ported |
| `setPlotTypes` | 1088-1120 | 33 | `mapgen_set_plot_types` | todo |
| `getRiverValueAtPlot` | 1124-1175 | 52 | `mapgen_get_river_value_at_plot` | todo |
| `calculateNumBonusesToAdd` | 1177-1220 | 44 | `mapgen_calculate_num_bonuses_to_add` | todo |
| `addImprovements` (FfH) | 1223-1284 | 62 | `mapgen_add_improvements` | todo |
| `addUniqueImprovements` (FfH) | 1286-1358 | 73 | `mapgen_add_unique_improvements` | todo |

## `mapgen/fractal.rs` — CvFractal (373 lines, fully ported)

`fracInit` 50-53, `fracInitHinted` 61-65, `fracInitInternal` 67-273 (207), `getHeight` 276-321,
`getHeightFromPercent` 324-366, `tectonicAction` 370-401, `yieldX` 404-417, `reset`/`uninit`
26-48 -> `Fractal::{frac_init, frac_init_hinted, height, height_from_percent, ...}`;
`Fractal::init(..) -> Fractal` = Python `CyFractal()` + `fracInit`. `FracVals` are module consts
(`fractal::FRAC_WRAP_X`, ..., also `Fractal::FRAC_*`).
Verified bit-exact (heights, percent thresholds and RNG state) against the original C++ compiled
by `tools/parity` (`tests/parity_fractal.rs`, fixture `tests/fixtures/cvfractal.txt`, 8 configs
incl. hints and rifts). Shift counts use `wrapping_shl` (x86 `shl` masking) for the
out-of-range exponents the C++ leaves undefined.

## Notes for the implementers

- **Sized player/team arrays.** C++ loops run over `MAX_CIV_PLAYERS` (35) slots, most of them
  empty. Where empty slots matter they must be accounted for: `updateScore` ranks all 35 civ
  slots with `>=` ties (empty slots score 0 and win ties against zero-score players at lower
  indices); `assignStartingPlots`/`doHolyCity` use `% MAX_CIV_TEAMS` / `% NumReligionInfos`
  offsets (rotation order over alive entries is unchanged with sized arrays).
- **Vote containers.** C++ `m_voteSelections`/`m_votesTriggered` (and `m_deals`) are
  `FFreeListTrashArray`s: iteration is in slot order and freed slots are reused LIFO. With two
  FfH vote sources, `doVoteResults` can process triggered votes in a different order than
  insertion order; emulate the slot allocation if exact parity is needed. Ids come from
  `next_vote_selection_id`/`next_vote_triggered_id`.
- **AI autoplay.** `update` ends the game (`GAMESTATE_OVER`) when no human is alive unless
  `ai_auto_play > 0` (or the exe's autorun flag): AI-only games must set `ai_auto_play` (or the
  port should add an explicit autorun flag). `reviveActivePlayer` (end of autoplay) is not ported.
- **Event hooks.** `setWinner` calls `ev_victory` only when both winner and victory are set (C++
  also reports NO_TEAM / NO_VICTORY, which FFH2's `onVictory` ignores), `setGameState(OVER)` calls
  `ev_game_end`, `makeReligionFounded`/`makeCorporationFounded` call theirs. **Missing hook:**
  `gameStart` (end of `game_init`, also `regenerateMap`): `ffh/events.rs` has no `ev_game_start`,
  and FFH2's `onGameStart` has rules effects (GAMEOPTION_THAW temp terrain with soren draws,
  scorpion-clan archers on goblin forts, a trophy). `doTurn` (begin/endGameTurn) and others are in
  todo bodies.
- **New-game order.** `game_init` reconstructs the exe sequence; the exe-side placement of
  `initEvents` (soren draws) relative to map generation (`addUniqueImprovements` and
  `addImprovements` also draw from the soren rand) is not visible in the DLL source.
- **Game handicap.** `Game::new` takes the first human slot's handicap, else `STANDARD_HANDICAP`.
- **Combat odds.** `combat_odds` uses IEEE single precision with MSVC's `pow(float, int)`
  (`_Pow_int`); the original ran on x87 with D3D's FPU control word, so tiny differences are
  possible. It only feeds AI pillage decisions and the combat log.
- **Trophies** are per game here (C++: persistent Python data storage; used by WB scenarios only).
- **Path finders** live in `Game` as `RefCell<PathFinder>` because C++ `generatePath` is `const`.
