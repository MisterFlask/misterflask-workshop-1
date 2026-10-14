# CvCity port map

Rust: `crates/ffh-core/src/city/` (`mod.rs` + five topic files, each an `impl Game` block).
C++: `CvGameCoreDLL/CvCity.cpp` (14770 lines) and `CvCity.h`, plus the city AI bookkeeping of
`CvCityAI.h` / `CvCityAI.cpp` that rules code reaches through `AI_*` calls.

**Status: ported and audited.** `City` has every rules data member of `CvCity`; every
rules-relevant `CvCity` method is a `Game::city_*` method with a complete body (no `todo!` left).
Each file was compared line by line with the C++ in the audit wave. Citizen-assignment AI
(`AI_assignWorkingPlots` and friends) is the new governor in `crate::governor`, not this module.

## Summary

| file | C++ lines | methods | ported |
|---|---:|---:|---:|
| `production.rs` | 2149 | 89 | 89 |
| `turn.rs` | 2446 | 29 | 29 |
| `citizens.rs` | 2355 | 124 | 124 |
| `buildings.rs` | 1896 | 70 | 70 |
| `yields_culture.rs` | 1902 | 103 | 103 |
| **total** | 10748 | 415 | 415 |

`city_area` covers two C++ functions (`area()` and `getArea()`), so the C++ function count is one
higher than the Rust method count.

## Conventions used in this module

- **Names.** `getFoo` -> `city_foo`, `isFoo` -> `city_is_foo`; `setFoo` / `changeFoo` / `canFoo`
  / `doFoo` keep the verb. Overloads get a suffix naming the distinguishing argument
  (`getProductionNeeded(UnitTypes)` -> `city_production_needed_unit`, `getHurryGold(eHurry,
  iCost)` -> `city_hurry_gold_for_cost`, `(int, int, ...)` raw variants -> `_raw`); the
  all-types variant of a per-type update is plural (`updateCommerce()` -> `city_update_commerces`,
  `updateCommerce(e)` -> `city_update_commerce`); argument-less "any" variants say so
  (`isHolyCity()` -> `city_is_any_holy_city`). The tables below give every mapping.
- **Signatures.** The city id comes first. `CvPlot*` -> `PlotIdx` (or `Option` where C++ allows
  NULL), `CvCity*` -> `CityId`, `CvUnit*` -> `UnitId`, `CvArea*` -> `AreaId`, `CvPlotGroup*` ->
  `Option<PlotGroupId>`. Enum arguments/results that may be `NO_X` are `Option<XType>`
  (`getProductionUnit`, `findHighestCulture`, `getLiberationPlayer`, `getConscriptUnit`,
  `isConnectedToCapital(ePlayer = NO_PLAYER)`, `countNumImprovedPlots(eImprovement =
  NO_IMPROVEMENT)`, ...). C++ `-1` index results are `Option<usize>` (`getFirstUnitOrder`,
  `getCityPlotIndex`). City plot indices are `usize`. `popOrder`'s `iNum` stays `i32` (`-1` =
  last). Order data keeps the C++ `OrderData { order_type, data1, data2, save }`. Rust has no
  default arguments: the doc comment names the C++ defaults where they matter.
- **`canTrain` / `canConstruct` / `canUpgrade`** take a non-optional type; C++ returns false for
  `NO_UNIT` / `NO_BUILDING`, so callers holding an `Option` check it first.
- **`&self` vs `&mut self`** follows what the C++ body does, not only its `const`:
  `hasShrine`, `canContinueProduction` and `getProductionExperience` are non-const in C++ but
  pure, so they take `&self`. Anything that writes state or draws from the RNG is `&mut self`.
  `invalidate*RankCache` keep `&mut self` (non-const in C++).
- **Rank caches.** C++ caches `findPopulationRank` / `find*RateRank` in `mutable` members written
  from `const` methods. In `City` they are `CacheI32` / `CacheBool` (`Cell`-like `get`/`set`,
  backed by relaxed atomics so `Game` stays `Sync`), so these methods keep `&self`. The cache is
  real state (a stale value would be observable), so port the caching, not a recomputation.
- **Ported bodies** follow the C++ line by line but omit `FAssert`s, UI dirty bits
  (`setInfoDirty`, `setLayoutDirty`, interface dirty bits, `plotAction(PUF_makeInfoBarDirty)`)
  and other pure UI. `AI_setAssignWorkDirty(true)` is `city.ai_assign_work_dirty = true`.
- **Python callbacks** (`canTrain`/`cannotTrain`, `canConstruct`/`cannotConstruct`,
  `canCreate`/`cannotCreate`, `canMaintain`/`cannotMaintain`, `getBuildingCostMod`,
  `getConscriptUnitType`, `doGrowth`, `doCulture`, `doPlotCulture`, `doProduction`,
  `doReligion`, `doGreatPeople`, `doMeltdown`, `citiesDestroyFeatures`, event triggers'
  `PythonCanDoCity`) and `CvEventReporter` events (`cityDoTurn`, `cityGrowth`, `cityHurry`,
  `cityLost`, `cultureExpansion`, `religionSpread`, `unitBuilt`, `buildingBuilt`, ...) go to
  their ports in `crate::ffh`, per the project conventions.
- **AI decisions** inside rules code go to the owner's controller: `AI_chooseProduction` and the
  human `chooseProduction` popup -> `Controller::choose_production`; `AI_chooseFreeTech` ->
  `Controller::choose_free_tech`. Citizen assignment (`AI_assignWorkingPlots`,
  `AI_updateAssignWork`, `AI_addBestCitizen`, `AI_removeWorstCitizen`) is the governor
  (`crate::governor`). Other AI follows the project rules (decision -> controller method,
  bookkeeping -> plain field, else omit with a `// AI:` comment): `AI_doTurn` is omitted;
  `AI_unitValue` picks the UnitAI of conscripts (`initConscriptedUnit`) and of upgraded queued
  units (`doCheckProduction`), and the latter also keeps the area/player train-UnitAI counts.

## The `City` struct (`mod.rs`)

183 fields: 174 of `CvCity`'s 179 data members (C++ order and names, `m_iFooBar` -> `foo_bar`)
and 9 `CvCityAI` bookkeeping fields. Omitted members are pure graphics/UI: `m_bWallOverride`,
`m_bInfoDirty`, `m_bLayoutDirty`, `m_kWallOverridePoints`, `m_iCitySizeBoost` (only used by
`getVisibleBuildings`).

Notable representations: `maintenance`, `commerce_rate` and `culture` hold the C++ times-100
values; `trade_cities: Vec<Option<CityId>>` (`MAX_TRADE_ROUTES` slots; read through
`city_trade_city`, which drops ids of cities that no longer exist, like C++ `getCity(IDInfo)`);
`order_queue: Vec<OrderData>` head first (`City::head_order()`); `building_original_owner:
IdxVec<BuildingType, Option<PlayerId>>`, `building_original_time` uses `MIN_INT` for none;
`civ: Option<CivilizationType>` is FFH's `m_iCiv` (`getTrueCivilizationType` /
`setCivilizationType`); `crime` is the city's own `m_iCrime` (`city_crime` adds the game's).

`City::new(d, num_players, num_teams, id, owner, x, y, crime)` reproduces `CvCity::reset()`
(with `uninit`) and the bookkeeping part of `CvCityAI::AI_reset()`. Per-type arrays are sized
from `d`, per-player/per-team arrays from the parameters. `reset()` draws
`getSorenRandNum(20, "Crime")`, and C++ runs it twice for every new city (constructor via
`FFreeListTrashArray::add`, value discarded; then `init`). So `City::new` takes `crime` as a
parameter and `city_init` must make both draws, in that order, before anything else.

### CvCityAI bookkeeping

| C++ (called from rules code) | Rust |
|---|---|
| `AI_setAssignWorkDirty(b)`, `AI_isAssignWorkDirty()` | field `ai_assign_work_dirty` |
| `AI_setChooseProductionDirty(b)`, `AI_isChooseProductionDirty()` | field `ai_choose_production_dirty` |
| `AI_isEmphasize(e)` | field `ai_emphasize[e]` (player-settable state) |
| `AI_setEmphasize(e, b)` (via `doTask(TASK_SET_EMPHASIZE)`) | `city_ai_set_emphasize` (`citizens.rs`; keeps the counts below) |
| `m_iEmphasizeAvoidGrowthCount`, `m_iEmphasizeGreatPeopleCount`, `m_aiEmphasizeYieldCount`, `m_aiEmphasizeCommerceCount`, `m_iEmphasizeAvoidAngryCitizensCount`, `m_iEmphasizeAvoidUnhealthyCitizensCount` | fields `ai_emphasize_*_count`; read through `city_ai_is_emphasize_*` |
| `AI_stopGrowth()` (in `doGrowth`, FFH2 GrowthControl) | `city_ai_stop_growth` |
| `AI_init()` / `AI_reset()` | `City::new` + governor (see `city_init`) |
| `AI_assignWorkingPlots`, `AI_updateAssignWork`, `AI_addBestCitizen`, `AI_removeWorstCitizen` | governor (`crate::governor`) |
| `AI_chooseProduction` | `Controller::choose_production` |

AI state that rules code does not touch is not in `City`: `m_routeToCity`, best builds
(`AI_getBestBuild` is only read by AI and by the UI's `CvUnit::isActionRecommended`), worker
counts, Sephi's defense/patrol caches (`AI_calculateNeeded*` / `AI_neededPermDefense`, called
from `CvPlayer::doTurn` and `CvPlot::AI_*` for AI players only), player closeness,
special-yield multipliers, and `m_bForceEmphasizeCulture` (set only by `CvPlayerAI`; it also
bumps the culture emphasize count, which only the AI reads).

## Methods by file

### `production.rs`

89 methods, all ported; 2149 lines of C++.

| C++ function | C++ lines (CvCity.cpp) | count | Rust | status |
|---|---|---:|---|---|
| `chooseProduction(UnitTypes eTrainUnit, BuildingTypes eConstructBuilding, ProjectTypes eCreateProject, bool bFinish, bool bFront)` | 1330-1362 | 33 | `city_choose_production` | ported |
| `allUpgradesAvailable(UnitTypes eUnit, int iUpgradeCount) const` | 1677-1747 | 71 | `city_all_upgrades_available` | ported |
| `isWorldWondersMaxed() const` | 1750-1768 | 19 | `city_is_world_wonders_maxed` | ported |
| `isTeamWondersMaxed() const` | 1771-1789 | 19 | `city_is_team_wonders_maxed` | ported |
| `isNationalWondersMaxed() const` | 1792-1807 | 16 | `city_is_national_wonders_maxed` | ported |
| `isBuildingsMaxed() const` | 1810-1828 | 19 | `city_is_buildings_maxed` | ported |
| `canTrain(UnitTypes eUnit, bool bContinue, bool bTestVisible, bool bIgnoreCost, bool bIgnoreUpgrades) const` | 1832-1850 | 19 | `city_can_train` | ported |
| `canUpgrade(UnitTypes eUnit, bool bContinue, bool bTestVisible, bool bIgnoreCost, bool bIgnoreUpgrades) const` | 1852-1968 | 117 | `city_can_upgrade` | ported |
| `canTrain(UnitCombatTypes eUnitCombat) const` | 1970-1989 | 20 | `city_can_train_combat` | ported |
| `canConstruct(BuildingTypes eBuilding, bool bContinue, bool bTestVisible, bool bIgnoreCost) const` | 1992-2309 | 318 | `city_can_construct` | ported |
| `canCreate(ProjectTypes eProject, bool bContinue, bool bTestVisible) const` | 2312-2374 | 63 | `city_can_create` | ported |
| `canMaintain(ProcessTypes eProcess, bool bContinue) const` | 2377-2419 | 43 | `city_can_maintain` | ported |
| `isProduction() const` | 2451-2454 | 4 | `city_is_production` | ported |
| `isProductionLimited() const` | 2457-2487 | 31 | `city_is_production_limited` | ported |
| `isProductionUnit() const` | 2490-2500 | 11 | `city_is_production_unit` | ported |
| `isProductionBuilding() const` | 2503-2513 | 11 | `city_is_production_building` | ported |
| `isProductionProject() const` | 2516-2526 | 11 | `city_is_production_project` | ported |
| `isProductionProcess() const` | 2529-2539 | 11 | `city_is_production_process` | ported |
| `canContinueProduction(OrderData order)` | 2542-2568 | 27 | `city_can_continue_production` | ported |
| `getProductionExperience(UnitTypes eUnit)` | 2571-2598 | 28 | `city_production_experience` | ported |
| `addProductionExperience(CvUnit* pUnit, bool bConscript)` | 2601-2621 | 21 | `city_add_production_experience` | ported |
| `getProductionUnit() const` | 2624-2648 | 25 | `city_production_unit` | ported |
| `getProductionUnitAI() const` | 2651-2675 | 25 | `city_production_unit_ai` | ported |
| `getProductionBuilding() const` | 2678-2704 | 27 | `city_production_building` | ported |
| `getProductionProject() const` | 2707-2733 | 27 | `city_production_project` | ported |
| `getProductionProcess() const` | 2736-2760 | 25 | `city_production_process` | ported |
| `getGeneralProductionTurnsLeft() const` | 2797-2828 | 32 | `city_general_production_turns_left` | ported |
| `isFoodProduction() const` | 2865-2889 | 25 | `city_is_food_production` | ported |
| `isFoodProduction(UnitTypes eUnit) const` | 2892-2908 | 17 | `city_is_food_production_unit` | ported |
| `getFirstUnitOrder(UnitTypes eUnit) const` | 2911-2933 | 23 | `city_first_unit_order` | ported |
| `getFirstBuildingOrder(BuildingTypes eBuilding) const` | 2936-2958 | 23 | `city_first_building_order` | ported |
| `getFirstProjectOrder(ProjectTypes eProject) const` | 2961-2983 | 23 | `city_first_project_order` | ported |
| `getNumTrainUnitAI(UnitAITypes eUnitAI) const` | 2986-3006 | 21 | `city_num_train_unit_ai` | ported |
| `getProduction() const` | 3009-3039 | 31 | `city_production` | ported |
| `getProductionNeeded() const` | 3042-3072 | 31 | `city_production_needed` | ported |
| `getProductionNeeded(UnitTypes eUnit) const` | 3074-3077 | 4 | `city_production_needed_unit` | ported |
| `getProductionNeeded(BuildingTypes eBuilding) const` | 3079-3101 | 23 | `city_production_needed_building` | ported |
| `getProductionNeeded(ProjectTypes eProject) const` | 3103-3106 | 4 | `city_production_needed_project` | ported |
| `getProductionTurnsLeft() const` | 3108-3138 | 31 | `city_production_turns_left` | ported |
| `getProductionTurnsLeft(UnitTypes eUnit, int iNum) const` | 3141-3161 | 21 | `city_production_turns_left_unit` | ported |
| `getProductionTurnsLeft(BuildingTypes eBuilding, int iNum) const` | 3164-3185 | 22 | `city_production_turns_left_building` | ported |
| `getProductionTurnsLeft(ProjectTypes eProject, int iNum) const` | 3188-3208 | 21 | `city_production_turns_left_project` | ported |
| `getProductionTurnsLeft(int iProductionNeeded, int iProduction, int iFirstProductionDifference, int iProductionDifference) const` | 3211-3233 | 23 | `city_production_turns_left_raw` | ported |
| `setProduction(int iNewValue)` | 3236-3250 | 15 | `city_set_production` | ported |
| `changeProduction(int iChange)` | 3253-3267 | 15 | `city_change_production` | ported |
| `getProductionModifier() const` | 3270-3300 | 31 | `city_production_modifier` | ported |
| `getProductionModifier(UnitTypes eUnit) const` | 3303-3337 | 35 | `city_production_modifier_unit` | ported |
| `getProductionModifier(BuildingTypes eBuilding) const` | 3340-3361 | 22 | `city_production_modifier_building` | ported |
| `getProductionModifier(ProjectTypes eProject) const` | 3364-3385 | 22 | `city_production_modifier_project` | ported |
| `getProductionDifference(int iProductionNeeded, int iProduction, int iProductionModifier, bool bFoodProduction, bool bOverflow) const` | 3388-3413 | 26 | `city_production_difference` | ported |
| `getCurrentProductionDifference(bool bIgnoreFood, bool bOverflow) const` | 3416-3419 | 4 | `city_current_production_difference` | ported |
| `getExtraProductionDifference(int iExtra) const` | 3422-3425 | 4 | `city_extra_production_difference` | ported |
| `getExtraProductionDifference(int iExtra, UnitTypes eUnit) const` | 3427-3430 | 4 | `city_extra_production_difference_unit` | ported |
| `getExtraProductionDifference(int iExtra, BuildingTypes eBuilding) const` | 3432-3435 | 4 | `city_extra_production_difference_building` | ported |
| `getExtraProductionDifference(int iExtra, ProjectTypes eProject) const` | 3437-3440 | 4 | `city_extra_production_difference_project` | ported |
| `getExtraProductionDifference(int iExtra, int iModifier) const` | 3442-3445 | 4 | `city_extra_production_difference_modifier` | ported |
| `canHurry(HurryTypes eHurry, bool bTestVisible) const` | 3448-3491 | 44 | `city_can_hurry` | ported |
| `canHurryUnit(HurryTypes eHurry, UnitTypes eUnit, bool bIgnoreNew) const` | 3493-3521 | 29 | `city_can_hurry_unit` | ported |
| `canHurryBuilding(HurryTypes eHurry, BuildingTypes eBuilding, bool bIgnoreNew) const` | 3523-3551 | 29 | `city_can_hurry_building` | ported |
| `hurry(HurryTypes eHurry)` | 3554-3583 | 30 | `city_hurry` | ported |
| `getConscriptUnit() const` | 3586-3630 | 45 | `city_conscript_unit` | ported |
| `getConscriptPopulation() const` | 3633-3650 | 18 | `city_conscript_population` | ported |
| `conscriptMinCityPopulation() const` | 3653-3662 | 10 | `city_conscript_min_city_population` | ported |
| `flatConscriptAngerLength() const` | 3665-3675 | 11 | `city_flat_conscript_anger_length` | ported |
| `canConscript() const` | 3678-3723 | 46 | `city_can_conscript` | ported |
| `initConscriptedUnit()` | 3725-3767 | 43 | `city_init_conscripted_unit` | ported |
| `conscript()` | 3770-3794 | 25 | `city_conscript` | ported |
| `processProcess(ProcessTypes eProcess, int iChange)` | 4209-4217 | 9 | `city_process_process` | ported |
| `productionLeft() const` | 4971-4974 | 4 | `city_production_left` | ported |
| `getHurryCostModifier(bool bIgnoreNew) const` | 4976-5004 | 29 | `city_hurry_cost_modifier` | ported |
| `getHurryCostModifier(UnitTypes eUnit, bool bIgnoreNew) const` | 5006-5009 | 4 | `city_hurry_cost_modifier_unit` | ported |
| `getHurryCostModifier(BuildingTypes eBuilding, bool bIgnoreNew) const` | 5011-5014 | 4 | `city_hurry_cost_modifier_building` | ported |
| `getHurryCostModifier(int iBaseModifier, int iProduction, bool bIgnoreNew) const` | 5016-5032 | 17 | `city_hurry_cost_modifier_raw` | ported |
| `hurryCost(bool bExtra) const` | 5035-5038 | 4 | `city_hurry_cost` | ported |
| `getHurryCost(bool bExtra, UnitTypes eUnit, bool bIgnoreNew) const` | 5040-5045 | 6 | `city_hurry_cost_unit` | ported |
| `getHurryCost(bool bExtra, BuildingTypes eBuilding, bool bIgnoreNew) const` | 5047-5052 | 6 | `city_hurry_cost_building` | ported |
| `getHurryCost(bool bExtra, int iProductionLeft, int iHurryModifier, int iModifier) const` | 5054-5071 | 18 | `city_hurry_cost_raw` | ported |
| `hurryGold(HurryTypes eHurry) const` | 5073-5076 | 4 | `city_hurry_gold` | ported |
| `getHurryGold(HurryTypes eHurry, int iHurryCost) const` | 5078-5090 | 13 | `city_hurry_gold_for_cost` | ported |
| `hurryPopulation(HurryTypes eHurry) const` | 5093-5096 | 4 | `city_hurry_population` | ported |
| `getHurryPopulation(HurryTypes eHurry, int iHurryCost) const` | 5098-5108 | 11 | `city_hurry_population_for_cost` | ported |
| `hurryProduction(HurryTypes eHurry) const` | 5110-5125 | 16 | `city_hurry_production` | ported |
| `flatHurryAngerLength() const` | 5128-5139 | 12 | `city_flat_hurry_anger_length` | ported |
| `hurryAngerLength(HurryTypes eHurry) const` | 5142-5152 | 11 | `city_hurry_anger_length` | ported |
| `maxHurryPopulation() const` | 5155-5158 | 4 | `city_max_hurry_population` | ported |
| `changeOverflowProduction(int iChange, int iProductionModifier)` | 7093-7098 | 6 | `city_change_overflow_production` | ported |
| `setProductionAutomated(bool bNewValue, bool bClear)` | 7717-7739 | 23 | `city_set_production_automated` | ported |
| `isFreePromotion(PromotionTypes eIndex) const` | 10241-10244 | 4 | `city_is_free_promotion` | ported |
| `isUnhappyProduction() const` | 14657-14660 | 4 | `city_is_unhappy_production` | ported |

### `turn.rs`

29 methods, all ported; 2446 lines of C++.

| C++ function | C++ lines (CvCity.cpp) | count | Rust | status |
|---|---|---:|---|---|
| `init(int iID, PlayerTypes eOwner, int iX, int iY, bool bBumpUnits, bool bUpdatePlotGroups)` | 146-348 | 203 | `city_init` | ported |
| `kill(bool bUpdatePlotGroups)` | 732-912 | 181 | `city_kill` | ported |
| `doTurn()` | 915-1150 | 236 | `city_do_turn` | ported |
| `doTask(TaskTypes eTask, int iData1, int iData2, bool bOption, bool bAlt, bool bShift, bool bCtrl)` | 1252-1327 | 76 | `city_do_task` | ported |
| `setGameTurnFounded(int iNewValue)` | 5444-5453 | 10 | `city_set_game_turn_founded` | ported |
| `isOccupation() const` | 7496-7499 | 4 | `city_is_occupation` | ported |
| `setOccupationTimer(int iNewValue)` | 7502-7526 | 25 | `city_set_occupation_timer` | ported |
| `changeOccupationTimer(int iChange)` | 7529-7532 | 4 | `city_change_occupation_timer` | ported |
| `setName(const wchar* szNewValue, bool bFound)` | 9418-9441 | 24 | `city_set_name` | ported |
| `doFoundMessage()` | 9444-9453 | 10 | `city_do_found_message` | ported |
| `clearOrderQueue()` | 11126-11137 | 12 | `city_clear_order_queue` | ported |
| `pushOrder(OrderTypes eOrder, int iData1, int iData2, bool bSave, bool bPop, bool bAppend, bool bForce)` | 11140-11255 | 116 | `city_push_order` | ported |
| `popOrder(int iNum, bool bFinish, bool bChoose)` | 11258-11639 | 382 | `city_pop_order` | ported |
| `startHeadOrder()` | 11642-11655 | 14 | `city_start_head_order` | ported |
| `stopHeadOrder()` | 11658-11669 | 12 | `city_stop_head_order` | ported |
| `doCheckProduction()` | 11888-12030 | 143 | `city_do_check_production` | ported |
| `doProduction(bool bAllowNoProduction)` | 12033-12084 | 52 | `city_do_production` | ported |
| `doDecay()` | 12087-12142 | 56 | `city_do_decay` | ported |
| `doMeltdown()` | 12282-12321 | 40 | `city_do_meltdown` | ported |
| `getTriggerValue(EventTriggerTypes eTrigger) const` | 13144-13356 | 213 | `city_trigger_value` | ported |
| `canApplyEvent(EventTypes eEvent, const EventTriggeredData& kTriggeredData) const` | 13358-13456 | 99 | `city_can_apply_event` | ported |
| `applyEvent(EventTypes eEvent, const EventTriggeredData& kTriggeredData, bool bClear)` | 13458-13656 | 199 | `city_apply_event` | ported |
| `isEventOccured(EventTypes eEvent) const` | 13658-13669 | 12 | `city_is_event_occured` | ported |
| `setEventOccured(EventTypes eEvent, bool bOccured)` | 13671-13689 | 19 | `city_set_event_occured` | ported |
| `liberate(bool bConquest)` | 14169-14228 | 60 | `city_liberate` | ported |
| `getLiberationPlayer(bool bConquest) const` | 14230-14342 | 113 | `city_liberation_player` | ported |
| `isAutoRaze() const` | 14382-14410 | 29 | `city_is_auto_raze` | ported |
| `applyBuildEffects(CvUnit* pUnit)` | 14499-14591 | 93 | `city_apply_build_effects` | ported |
| `getCrime() const` | 14675-14683 | 9 | `city_crime` | ported |

### `citizens.rs`

124 methods, all ported; 2355 lines of C++.

| C++ function | C++ lines (CvCity.cpp) | count | Rust | status |
|---|---|---:|---|---|
| `createGreatPeople(UnitTypes eGreatPersonUnit, bool bIncrementThreshold, bool bIncrementExperience)` | 1246-1249 | 4 | `city_create_great_people` | ported |
| `getCityPlotIndex(const CvPlot* pPlot) const` | 1365-1368 | 4 | `city_plot_index` | ported |
| `getCityIndexPlot(int iIndex) const` | 1371-1374 | 4 | `city_index_plot` | ported |
| `canWork(CvPlot* pPlot) const` | 1377-1431 | 55 | `city_can_work` | ported |
| `verifyWorkingPlot(int iIndex)` | 1434-1455 | 22 | `city_verify_working_plot` | ported |
| `verifyWorkingPlots()` | 1458-1466 | 9 | `city_verify_working_plots` | ported |
| `clearWorkingOverride(int iIndex)` | 1469-1479 | 11 | `city_clear_working_override` | ported |
| `countNumImprovedPlots(ImprovementTypes eImprovement, bool bPotential) const` | 1482-1515 | 34 | `city_count_num_improved_plots` | ported |
| `countNumWaterPlots() const` | 1518-1543 | 26 | `city_count_num_water_plots` | ported |
| `countNumRiverPlots() const` | 1545-1566 | 22 | `city_count_num_river_plots` | ported |
| `canJoin() const` | 2422-2425 | 4 | `city_can_join` | ported |
| `getFoodTurnsLeft() const` | 2428-2448 | 21 | `city_food_turns_left` | ported |
| `processSpecialist(SpecialistTypes eSpecialist, int iChange)` | 4220-4250 | 31 | `city_process_specialist` | ported |
| `getOvercrowdingPercentAnger(int iExtra) const` | 4414-4429 | 16 | `city_overcrowding_percent_anger` | ported |
| `getNoMilitaryPercentAnger() const` | 4432-4444 | 13 | `city_no_military_percent_anger` | ported |
| `getCulturePercentAnger() const` | 4447-4486 | 40 | `city_culture_percent_anger` | ported |
| `getReligionPercentAnger() const` | 4489-4534 | 46 | `city_religion_percent_anger` | ported |
| `getHurryPercentAnger(int iExtra) const` | 4537-4545 | 9 | `city_hurry_percent_anger` | ported |
| `getConscriptPercentAnger(int iExtra) const` | 4548-4556 | 9 | `city_conscript_percent_anger` | ported |
| `getDefyResolutionPercentAnger(int iExtra) const` | 4558-4566 | 9 | `city_defy_resolution_percent_anger` | ported |
| `getWarWearinessPercentAnger() const` | 4569-4579 | 11 | `city_war_weariness_percent_anger` | ported |
| `getLargestCityHappiness() const` | 4582-4590 | 9 | `city_largest_city_happiness` | ported |
| `getVassalHappiness() const` | 4592-4608 | 17 | `city_vassal_happiness` | ported |
| `getVassalUnhappiness() const` | 4610-4626 | 17 | `city_vassal_unhappiness` | ported |
| `unhappyLevel(int iExtra) const` | 4629-4675 | 47 | `city_unhappy_level` | ported |
| `happyLevel() const` | 4678-4706 | 29 | `city_happy_level` | ported |
| `angryPopulation(int iExtra) const` | 4709-4714 | 6 | `city_angry_population` | ported |
| `visiblePopulation() const` | 4717-4720 | 4 | `city_visible_population` | ported |
| `totalFreeSpecialists() const` | 4723-4743 | 21 | `city_total_free_specialists` | ported |
| `extraPopulation() const` | 4746-4749 | 4 | `city_extra_population` | ported |
| `extraSpecialists() const` | 4752-4755 | 4 | `city_extra_specialists` | ported |
| `extraFreeSpecialists() const` | 4758-4761 | 4 | `city_extra_free_specialists` | ported |
| `unhealthyPopulation(bool bNoAngry, int iExtra) const` | 4764-4772 | 9 | `city_unhealthy_population` | ported |
| `totalGoodBuildingHealth() const` | 4775-4778 | 4 | `city_total_good_building_health` | ported |
| `totalBadBuildingHealth() const` | 4781-4789 | 9 | `city_total_bad_building_health` | ported |
| `goodHealth() const` | 4792-4842 | 51 | `city_good_health` | ported |
| `badHealth(bool bNoAngry, int iExtra) const` | 4845-4907 | 63 | `city_bad_health` | ported |
| `healthRate(bool bNoAngry, int iExtra) const` | 4910-4913 | 4 | `city_health_rate` | ported |
| `foodConsumption(bool bNoAngry, int iExtra) const` | 4916-4932 | 17 | `city_food_consumption` | ported |
| `foodDifference(bool bBottom) const` | 4935-4962 | 28 | `city_food_difference` | ported |
| `growthThreshold() const` | 4965-4968 | 4 | `city_growth_threshold` | ported |
| `setPopulation(int iNewValue)` | 5475-5537 | 63 | `city_set_population` | ported |
| `changePopulation(int iChange)` | 5540-5543 | 4 | `city_change_population` | ported |
| `getRealPopulation() const` | 5546-5549 | 4 | `city_real_population` | ported |
| `changeSpecialistPopulation(int iChange)` | 5583-5594 | 12 | `city_change_specialist_population` | ported |
| `changeNumGreatPeople(int iChange)` | 5603-5612 | 10 | `city_change_num_great_people` | ported |
| `getGreatPeopleRate() const` | 5621-5629 | 9 | `city_great_people_rate` | ported |
| `getTotalGreatPeopleRateModifier() const` | 5632-5654 | 23 | `city_total_great_people_rate_modifier` | ported |
| `changeWarWearinessModifier(int iChange)` | 6059-6067 | 9 | `city_change_war_weariness_modifier` | ported |
| `changeHurryAngerModifier(int iChange)` | 6076-6094 | 19 | `city_change_hurry_anger_modifier` | ported |
| `updateFreshWaterHealth()` | 6150-6184 | 35 | `city_update_fresh_water_health` | ported |
| `updateFeatureHealth()` | 6199-6249 | 51 | `city_update_feature_health` | ported |
| `getBuildingHealth(BuildingTypes eBuilding) const` | 6264-6274 | 11 | `city_building_health` | ported |
| `getBuildingGoodHealth(BuildingTypes eBuilding) const` | 6276-6283 | 8 | `city_building_good_health` | ported |
| `getBuildingBadHealth(BuildingTypes eBuilding) const` | 6285-6297 | 13 | `city_building_bad_health` | ported |
| `changeBuildingGoodHealth(int iChange)` | 6299-6313 | 15 | `city_change_building_good_health` | ported |
| `changeBuildingBadHealth(int iChange)` | 6316-6330 | 15 | `city_change_building_bad_health` | ported |
| `updatePowerHealth()` | 6345-6393 | 49 | `city_update_power_health` | ported |
| `changeBonusGoodHealth(int iChange)` | 6408-6424 | 17 | `city_change_bonus_good_health` | ported |
| `changeBonusBadHealth(int iChange)` | 6427-6443 | 17 | `city_change_bonus_bad_health` | ported |
| `getMilitaryHappiness() const` | 6452-6455 | 4 | `city_military_happiness` | ported |
| `changeMilitaryHappinessUnits(int iChange)` | 6458-6467 | 10 | `city_change_military_happiness_units` | ported |
| `getBuildingHappiness(BuildingTypes eBuilding) const` | 6482-6507 | 26 | `city_building_happiness` | ported |
| `changeBuildingGoodHappiness(int iChange)` | 6510-6519 | 10 | `city_change_building_good_happiness` | ported |
| `changeBuildingBadHappiness(int iChange)` | 6522-6531 | 10 | `city_change_building_bad_happiness` | ported |
| `updateExtraBuildingHappiness()` | 6546-6585 | 40 | `city_update_extra_building_happiness` | ported |
| `updateExtraBuildingHealth()` | 6600-6636 | 37 | `city_update_extra_building_health` | ported |
| `updateFeatureHappiness()` | 6651-6709 | 59 | `city_update_feature_happiness` | ported |
| `changeBonusGoodHappiness(int iChange)` | 6724-6733 | 10 | `city_change_bonus_good_happiness` | ported |
| `changeBonusBadHappiness(int iChange)` | 6736-6745 | 10 | `city_change_bonus_bad_happiness` | ported |
| `getReligionHappiness(ReligionTypes eReligion) const` | 6760-6779 | 20 | `city_religion_happiness` | ported |
| `updateReligionHappiness()` | 6782-6821 | 40 | `city_update_religion_happiness` | ported |
| `changeExtraHappiness(int iChange)` | 6830-6838 | 9 | `city_change_extra_happiness` | ported |
| `changeExtraHealth(int iChange)` | 6846-6854 | 9 | `city_change_extra_health` | ported |
| `changeHurryAngerTimer(int iChange)` | 6864-6873 | 10 | `city_change_hurry_anger_timer` | ported |
| `changeConscriptAngerTimer(int iChange)` | 6882-6891 | 10 | `city_change_conscript_anger_timer` | ported |
| `changeDefyResolutionAngerTimer(int iChange)` | 6899-6908 | 10 | `city_change_defy_resolution_anger_timer` | ported |
| `flatDefyResolutionAngerLength() const` | 6910-6920 | 11 | `city_flat_defy_resolution_anger_length` | ported |
| `changeHappinessTimer(int iChange)` | 6929-6938 | 10 | `city_change_happiness_timer` | ported |
| `isNoUnhappiness() const` | 6947-6950 | 4 | `city_is_no_unhappiness` | ported |
| `changeNoUnhappinessCount(int iChange)` | 6953-6962 | 10 | `city_change_no_unhappiness_count` | ported |
| `isNoUnhealthyPopulation() const` | 6971-6979 | 9 | `city_is_no_unhealthy_population` | ported |
| `changeNoUnhealthyPopulationCount(int iChange)` | 6982-6991 | 10 | `city_change_no_unhealthy_population_count` | ported |
| `isBuildingOnlyHealthy() const` | 7000-7008 | 9 | `city_is_building_only_healthy` | ported |
| `changeBuildingOnlyHealthyCount(int iChange)` | 7011-7020 | 10 | `city_change_building_only_healthy_count` | ported |
| `changeFreeSpecialist(int iChange)` | 7312-7321 | 10 | `city_change_free_specialist` | ported |
| `setWeLoveTheKingDay(bool bNewValue)` | 7642-7674 | 33 | `city_set_we_love_the_king_day` | ported |
| `setCitizensAutomated(bool bNewValue)` | 7683-7708 | 26 | `city_set_citizens_automated` | ported |
| `getCommerceHappinessByType(CommerceTypes eCommerce) const` | 9014-9017 | 4 | `city_commerce_happiness_by_type` | ported |
| `getCommerceHappiness() const` | 9020-9033 | 14 | `city_commerce_happiness` | ported |
| `changeCommerceHappinessPer(CommerceTypes eIndex, int iChange)` | 9036-9048 | 13 | `city_change_commerce_happiness_per` | ported |
| `setGreatPeopleUnitRate(UnitTypes eIndex, int iNewValue)` | 9848-9859 | 12 | `city_set_great_people_unit_rate` | ported |
| `changeGreatPeopleUnitRate(UnitTypes eIndex, int iChange)` | 9862-9865 | 4 | `city_change_great_people_unit_rate` | ported |
| `setSpecialistCount(SpecialistTypes eIndex, int iNewValue)` | 9899-9921 | 23 | `city_set_specialist_count` | ported |
| `changeSpecialistCount(SpecialistTypes eIndex, int iChange)` | 9924-9927 | 4 | `city_change_specialist_count` | ported |
| `alterSpecialistCount(SpecialistTypes eIndex, int iChange)` | 9930-10015 | 86 | `city_alter_specialist_count` | ported |
| `isSpecialistValid(SpecialistTypes eIndex, int iExtra) const` | 10026-10029 | 4 | `city_is_specialist_valid` | ported |
| `changeMaxSpecialistCount(SpecialistTypes eIndex, int iChange)` | 10032-10043 | 12 | `city_change_max_specialist_count` | ported |
| `isSpecialistForced() const` | 10054-10067 | 14 | `city_is_specialist_forced` | ported |
| `setForceSpecialistCount(SpecialistTypes eIndex, int iNewValue)` | 10070-10086 | 17 | `city_set_force_specialist_count` | ported |
| `changeForceSpecialistCount(SpecialistTypes eIndex, int iChange)` | 10089-10092 | 4 | `city_change_force_specialist_count` | ported |
| `getAddedFreeSpecialistCount(SpecialistTypes eIndex) const` | 10102-10117 | 16 | `city_added_free_specialist_count` | ported |
| `setFreeSpecialistCount(SpecialistTypes eIndex, int iNewValue)` | 10119-10141 | 23 | `city_set_free_specialist_count` | ported |
| `changeFreeSpecialistCount(SpecialistTypes eIndex, int iChange)` | 10143-10146 | 4 | `city_change_free_specialist_count` | ported |
| `changeImprovementFreeSpecialists(ImprovementTypes eIndex, int iChange)` | 10155-10164 | 10 | `city_change_improvement_free_specialists` | ported |
| `getCurrentStateReligionHappiness() const` | 10183-10191 | 9 | `city_current_state_religion_happiness` | ported |
| `changeStateReligionHappiness(ReligionTypes eIndex, int iChange)` | 10202-10213 | 12 | `city_change_state_religion_happiness` | ported |
| `isWorkingPlot(const CvPlot* pPlot) const` | 10292-10304 | 13 | `city_is_working_plot` | ported |
| `setWorkingPlot(int iIndex, bool bNewValue)` | 10307-10366 | 60 | `city_set_working_plot` | ported |
| `setWorkingPlot(CvPlot* pPlot, bool bNewValue)` | 10369-10372 | 4 | `city_set_working_plot_at` | ported |
| `alterWorkingPlot(int iIndex)` | 10375-10423 | 49 | `city_alter_working_plot` | ported |
| `doGrowth()` | 11743-11800 | 58 | `city_do_growth` | ported |
| `doGreatPeople()` | 12218-12279 | 62 | `city_do_great_people` | ported |
| `getBestYieldAvailable(YieldTypes eYield) const` | 14344-14380 | 37 | `city_best_yield_available` | ported |
| `getNumCityPlots() const` | 14613-14620 | 8 | `city_num_city_plots` | ported |
| `setPlotRadius(int iNewValue)` | 14627-14655 | 29 | `city_set_plot_radius` | ported |
| `CvCityAI::AI_stopGrowth` | 432-488 (CvCityAI.cpp) | 57 | `city_ai_stop_growth` | ported |
| `CvCityAI::AI_isEmphasizeAvoidGrowth` | 4861-4864 (CvCityAI.cpp) | 4 | `city_ai_is_emphasize_avoid_growth` | ported |
| `CvCityAI::AI_isEmphasizeAvoidAngryCitizens` | 4876-4879 (CvCityAI.cpp) | 4 | `city_ai_is_emphasize_avoid_angry_citizens` | ported |
| `CvCityAI::AI_isEmphasizeAvoidUnhealthyCitizens` | 4886-4889 (CvCityAI.cpp) | 4 | `city_ai_is_emphasize_avoid_unhealthy_citizens` | ported |
| `CvCityAI::AI_isEmphasizeGreatPeople` | 4900-4903 (CvCityAI.cpp) | 4 | `city_ai_is_emphasize_great_people` | ported |
| `CvCityAI::AI_isEmphasizeYield` | 4995-4998 (CvCityAI.cpp) | 4 | `city_ai_is_emphasize_yield` | ported |
| `CvCityAI::AI_isEmphasizeCommerce` | 5009-5012 (CvCityAI.cpp) | 4 | `city_ai_is_emphasize_commerce` | ported |
| `CvCityAI::AI_setEmphasize` | 5024-5091 (CvCityAI.cpp) | 68 | `city_ai_set_emphasize` | ported |

### `buildings.rs`

70 methods, all ported; 1896 lines of C++.

| C++ function | C++ lines (CvCity.cpp) | count | Rust | status |
|---|---|---:|---|---|
| `getBonusHealth(BonusTypes eBonus) const` | 3797-3810 | 14 | `city_bonus_health` | ported |
| `getBonusHappiness(BonusTypes eBonus) const` | 3813-3826 | 14 | `city_bonus_happiness` | ported |
| `getBonusPower(BonusTypes eBonus, bool bDirty) const` | 3829-3851 | 23 | `city_bonus_power` | ported |
| `getBonusYieldRateModifier(YieldTypes eIndex, BonusTypes eBonus) const` | 3854-3867 | 14 | `city_bonus_yield_rate_modifier` | ported |
| `processBonus(BonusTypes eBonus, int iChange)` | 3870-3972 | 103 | `city_process_bonus` | ported |
| `processBuilding(BuildingTypes eBuilding, int iChange, bool bObsolete)` | 3975-4206 | 232 | `city_process_building` | ported |
| `isHeadquarters(CorporationTypes eIndex) const` | 4378-4381 | 4 | `city_is_headquarters` | ported |
| `setHeadquarters(CorporationTypes eIndex)` | 4383-4396 | 14 | `city_set_headquarters` | ported |
| `isHeadquarters() const` | 4398-4411 | 14 | `city_is_any_headquarters` | ported |
| `getNumBuilding(BuildingTypes eIndex) const` | 5242-5247 | 6 | `city_num_building` | ported |
| `getNumActiveBuilding(BuildingTypes eIndex) const` | 5250-5260 | 11 | `city_num_active_building` | ported |
| `hasActiveWorldWonder() const` | 5263-5279 | 17 | `city_has_active_world_wonder` | ported |
| `getCorporationCount() const` | 5300-5316 | 17 | `city_corporation_count` | ported |
| `isGovernmentCenter() const` | 5747-5750 | 4 | `city_is_government_center` | ported |
| `changeGovernmentCenterCount(int iChange)` | 5753-5762 | 10 | `city_change_government_center_count` | ported |
| `getMaintenance() const` | 5765-5768 | 4 | `city_maintenance` | ported |
| `updateMaintenance()` | 5776-5800 | 25 | `city_update_maintenance` | ported |
| `calculateDistanceMaintenance() const` | 5802-5805 | 4 | `city_calculate_distance_maintenance` | ported |
| `calculateDistanceMaintenanceTimes100() const` | 5807-5855 | 49 | `city_calculate_distance_maintenance_times100` | ported |
| `calculateNumCitiesMaintenance() const` | 5857-5860 | 4 | `city_calculate_num_cities_maintenance` | ported |
| `calculateNumCitiesMaintenanceTimes100() const` | 5862-5905 | 44 | `city_calculate_num_cities_maintenance_times100` | ported |
| `calculateColonyMaintenance() const` | 5908-5911 | 4 | `city_calculate_colony_maintenance` | ported |
| `calculateColonyMaintenanceTimes100() const` | 5913-5959 | 47 | `city_calculate_colony_maintenance_times100` | ported |
| `calculateCorporationMaintenance() const` | 5962-5965 | 4 | `city_calculate_corporation_maintenance` | ported |
| `calculateCorporationMaintenanceTimes100() const` | 5967-5982 | 16 | `city_calculate_corporation_maintenance_times100` | ported |
| `calculateCorporationMaintenanceTimes100(CorporationTypes eCorporation) const` | 5984-6027 | 44 | `city_calculate_corporation_maintenance_times100_for` | ported |
| `calculateBaseMaintenanceTimes100() const` | 6030-6033 | 4 | `city_calculate_base_maintenance_times100` | ported |
| `changeMaintenanceModifier(int iChange)` | 6042-6050 | 9 | `city_change_maintenance_modifier` | ported |
| `getAirUnitCapacity(TeamTypes eTeam) const` | 7283-7286 | 4 | `city_air_unit_capacity` | ported |
| `isPower() const` | 7330-7333 | 4 | `city_is_power` | ported |
| `isAreaCleanPower() const` | 7336-7344 | 9 | `city_is_area_clean_power` | ported |
| `isDirtyPower() const` | 7353-7356 | 4 | `city_is_dirty_power` | ported |
| `changePowerCount(int iChange, bool bDirty)` | 7359-7394 | 36 | `city_change_power_count` | ported |
| `changeDefenseDamage(int iChange)` | 7403-7418 | 16 | `city_change_defense_damage` | ported |
| `changeDefenseModifier(int iChange)` | 7420-7431 | 12 | `city_change_defense_modifier` | ported |
| `isBombardable(const CvUnit* pUnit) const` | 7446-7454 | 9 | `city_is_bombardable` | ported |
| `getNaturalDefense() const` | 7457-7465 | 9 | `city_natural_defense` | ported |
| `getTotalDefense(bool bIgnoreBuilding) const` | 7468-7476 | 9 | `city_total_defense` | ported |
| `getDefenseModifier(bool bIgnoreBuilding) const` | 7479-7487 | 9 | `city_defense_modifier` | ported |
| `setCorporationYield(YieldTypes eIndex, int iNewValue)` | 8768-8784 | 17 | `city_set_corporation_yield` | ported |
| `getCorporationYieldByCorporation(YieldTypes eIndex, CorporationTypes eCorporation) const` | 8794-8816 | 23 | `city_corporation_yield_by_corporation` | ported |
| `getCorporationCommerceByCorporation(CommerceTypes eIndex, CorporationTypes eCorporation) const` | 8818-8840 | 23 | `city_corporation_commerce_by_corporation` | ported |
| `updateCorporationCommerce(CommerceTypes eIndex)` | 8842-8858 | 17 | `city_update_corporation_commerce` | ported |
| `updateCorporationYield(YieldTypes eIndex)` | 8860-8877 | 18 | `city_update_corporation_yield` | ported |
| `updateCorporation()` | 8880-8897 | 18 | `city_update_corporation` | ported |
| `updateCorporationBonus()` | 8900-8979 | 80 | `city_update_corporation_bonus` | ported |
| `isNoBonus(BonusTypes eIndex) const` | 9475-9478 | 4 | `city_is_no_bonus` | ported |
| `changeNoBonusCount(BonusTypes eIndex, int iChange)` | 9480-9506 | 27 | `city_change_no_bonus_count` | ported |
| `changeFreeBonus(BonusTypes eIndex, int iChange)` | 9517-9529 | 13 | `city_change_free_bonus` | ported |
| `getNumBonuses(BonusTypes eIndex) const` | 9531-9552 | 22 | `city_num_bonuses` | ported |
| `hasBonus(BonusTypes eIndex) const` | 9555-9558 | 4 | `city_has_bonus` | ported |
| `changeNumBonuses(BonusTypes eIndex, int iChange)` | 9561-9600 | 40 | `city_change_num_bonuses` | ported |
| `isCorporationBonus(BonusTypes eBonus) const` | 9610-9633 | 24 | `city_is_corporation_bonus` | ported |
| `isActiveCorporation(CorporationTypes eCorporation) const` | 9635-9663 | 29 | `city_is_active_corporation` | ported |
| `setNumRealBuilding(BuildingTypes eIndex, int iNewValue)` | 10434-10449 | 16 | `city_set_num_real_building` | ported |
| `setNumRealBuildingTimed(BuildingTypes eIndex, int iNewValue, bool bFirst, PlayerTypes eOriginalOwner, int iOriginalTime)` | 10452-10648 | 197 | `city_set_num_real_building_timed` | ported |
| `setNumFreeBuilding(BuildingTypes eIndex, int iNewValue)` | 10659-10677 | 19 | `city_set_num_free_building` | ported |
| `setHasCorporation(CorporationTypes eIndex, bool bNewValue, bool bAnnounce, bool bArrows)` | 10887-10993 | 107 | `city_set_has_corporation` | ported |
| `isValidBuildingLocation(BuildingTypes eBuilding) const` | 13112-13142 | 31 | `city_is_valid_building_location` | ported |
| `getBuildingYieldChange(BuildingClassTypes eBuildingClass, YieldTypes eYield) const` | 13754-13765 | 12 | `city_building_yield_change` | ported |
| `setBuildingYieldChange(BuildingClassTypes eBuildingClass, YieldTypes eYield, int iChange)` | 13767-13817 | 51 | `city_set_building_yield_change` | ported |
| `changeBuildingYieldChange(BuildingClassTypes eBuildingClass, YieldTypes eYield, int iChange)` | 13819-13822 | 4 | `city_change_building_yield_change` | ported |
| `getBuildingCommerceChange(BuildingClassTypes eBuildingClass, CommerceTypes eCommerce) const` | 13824-13835 | 12 | `city_building_commerce_change` | ported |
| `setBuildingCommerceChange(BuildingClassTypes eBuildingClass, CommerceTypes eCommerce, int iChange)` | 13837-13871 | 35 | `city_set_building_commerce_change` | ported |
| `changeBuildingCommerceChange(BuildingClassTypes eBuildingClass, CommerceTypes eCommerce, int iChange)` | 13873-13876 | 4 | `city_change_building_commerce_change` | ported |
| `setBuildingHappyChange(BuildingClassTypes eBuildingClass, int iChange)` | 13939-14006 | 68 | `city_set_building_happy_change` | ported |
| `getBuildingHappyChange(BuildingClassTypes eBuildingClass) const` | 14011-14022 | 12 | `city_building_happy_change` | ported |
| `setBuildingHealthChange(BuildingClassTypes eBuildingClass, int iChange)` | 14085-14151 | 67 | `city_set_building_health_change` | ported |
| `getBuildingHealthChange(BuildingClassTypes eBuildingClass) const` | 14156-14167 | 12 | `city_building_health_change` | ported |
| `isHasBuildingClass(int iBuildingClass) const` | 14705-14718 | 14 | `city_is_has_building_class` | ported |

### `yields_culture.rs`

103 methods, all ported; 1902 lines of C++.

| C++ function | C++ lines (CvCity.cpp) | count | Rust | status |
|---|---|---:|---|---|
| `updateYield()` | 1213-1227 | 15 | `city_update_yield` | ported |
| `findPopulationRank() const` | 1569-1592 | 24 | `city_find_population_rank` | ported |
| `findBaseYieldRateRank(YieldTypes eYield) const` | 1595-1619 | 25 | `city_find_base_yield_rate_rank` | ported |
| `findYieldRateRank(YieldTypes eYield) const` | 1622-1646 | 25 | `city_find_yield_rate_rank` | ported |
| `findCommerceRateRank(CommerceTypes eCommerce) const` | 1649-1673 | 25 | `city_find_commerce_rate_rank` | ported |
| `getHandicapType() const` | 4253-4256 | 4 | `city_handicap_type` | ported |
| `getCivilizationType() const` | 4259-4274 | 16 | `city_civilization_type` | ported |
| `getPersonalityType() const` | 4277-4280 | 4 | `city_personality_type` | ported |
| `hasTrait(TraitTypes eTrait) const` | 4309-4312 | 4 | `city_has_trait` | ported |
| `isBarbarian() const` | 4315-4318 | 4 | `city_is_barbarian` | ported |
| `isHuman() const` | 4321-4324 | 4 | `city_is_human` | ported |
| `isVisible(TeamTypes eTeam, bool bDebug) const` | 4327-4330 | 4 | `city_is_visible` | ported |
| `isCapital() const` | 4333-4336 | 4 | `city_is_capital` | ported |
| `isCoastal(int iMinWaterSize) const` | 4339-4342 | 4 | `city_is_coastal` | ported |
| `isDisorder() const` | 4345-4353 | 9 | `city_is_disorder` | ported |
| `isHolyCity(ReligionTypes eIndex) const` | 4356-4359 | 4 | `city_is_holy_city` | ported |
| `isHolyCity() const` | 4362-4375 | 14 | `city_is_any_holy_city` | ported |
| `cultureDistance(int iDX, int iDY) const` | 5161-5164 | 4 | `city_culture_distance` | ported |
| `cultureStrength(PlayerTypes ePlayer) const` | 5167-5212 | 46 | `city_culture_strength` | ported |
| `cultureGarrison(PlayerTypes ePlayer) const` | 5215-5239 | 25 | `city_culture_garrison` | ported |
| `getReligionCount() const` | 5282-5298 | 17 | `city_religion_count` | ported |
| `at(int iX, int iY) const` | 5356-5359 | 4 | `city_at` | ported |
| `at(CvPlot* pPlot) const` | 5362-5365 | 4 | `city_at_plot` | ported |
| `plot() const` | 5368-5371 | 4 | `city_plot` | ported |
| `plotGroup(PlayerTypes ePlayer) const` | 5374-5377 | 4 | `city_plot_group` | ported |
| `isConnectedTo(CvCity* pCity) const` | 5380-5383 | 4 | `city_is_connected_to` | ported |
| `isConnectedToCapital(PlayerTypes ePlayer) const` | 5386-5389 | 4 | `city_is_connected_to_capital` | ported |
| `getArea() const` | 5392-5395 | 4 | `city_area` | ported |
| `area() const` | 5397-5400 | 4 | `city_area` | ported |
| `waterArea() const` | 5403-5406 | 4 | `city_water_area` | ported |
| `getRallyPlot() const` | 5409-5412 | 4 | `city_rally_plot` | ported |
| `setRallyPlot(CvPlot* pPlot)` | 5415-5435 | 21 | `city_set_rally_plot` | ported |
| `changeExtraTradeRoutes(int iChange)` | 7150-7159 | 10 | `city_change_extra_trade_routes` | ported |
| `changeTradeRouteModifier(int iChange)` | 7167-7175 | 9 | `city_change_trade_route_modifier` | ported |
| `changeForeignTradeRouteModifier(int iChange)` | 7182-7190 | 9 | `city_change_foreign_trade_route_modifier` | ported |
| `setPlundered(bool bNewValue)` | 7625-7633 | 9 | `city_set_plundered` | ported |
| `getTeam() const` | 7813-7816 | 4 | `city_team` | ported |
| `getCultureThreshold() const` | 7825-7828 | 4 | `city_culture_threshold` | ported |
| `getCultureThreshold(CultureLevelTypes eLevel)` | 7830-7838 | 9 | `city_culture_threshold_for_level` | ported |
| `setCultureLevel(CultureLevelTypes eNewValue, bool bUpdatePlotGroups)` | 7841-7971 | 131 | `city_set_culture_level` | ported |
| `updateCultureLevel(bool bUpdatePlotGroups)` | 7974-7996 | 23 | `city_update_culture_level` | ported |
| `changeSeaPlotYield(YieldTypes eIndex, int iChange)` | 8007-8019 | 13 | `city_change_sea_plot_yield` | ported |
| `changeRiverPlotYield(YieldTypes eIndex, int iChange)` | 8030-8042 | 13 | `city_change_river_plot_yield` | ported |
| `getBaseYieldRateModifier(YieldTypes eIndex, int iExtra) const` | 8053-8091 | 39 | `city_base_yield_rate_modifier` | ported |
| `getYieldRate(YieldTypes eIndex) const` | 8094-8097 | 4 | `city_yield_rate` | ported |
| `setBaseYieldRate(YieldTypes eIndex, int iNewValue)` | 8100-8126 | 27 | `city_set_base_yield_rate` | ported |
| `changeBaseYieldRate(YieldTypes eIndex, int iChange)` | 8129-8132 | 4 | `city_change_base_yield_rate` | ported |
| `changeYieldRateModifier(YieldTypes eIndex, int iChange)` | 8143-8167 | 25 | `city_change_yield_rate_modifier` | ported |
| `changePowerYieldRateModifier(YieldTypes eIndex, int iChange)` | 8178-8202 | 25 | `city_change_power_yield_rate_modifier` | ported |
| `changeBonusYieldRateModifier(YieldTypes eIndex, int iChange)` | 8213-8237 | 25 | `city_change_bonus_yield_rate_modifier` | ported |
| `totalTradeModifier(CvCity* pOtherCity) const` | 8248-8277 | 30 | `city_total_trade_modifier` | ported |
| `getPopulationTradeModifier() const` | 8279-8282 | 4 | `city_population_trade_modifier` | ported |
| `getPeaceTradeModifier(TeamTypes eTeam) const` | 8284-8302 | 19 | `city_peace_trade_modifier` | ported |
| `getBaseTradeProfit(CvCity* pCity) const` | 8304-8314 | 11 | `city_base_trade_profit` | ported |
| `calculateTradeProfit(CvCity* pCity) const` | 8316-8324 | 9 | `city_calculate_trade_profit` | ported |
| `calculateTradeYield(YieldTypes eIndex, int iTradeProfit) const` | 8327-8337 | 11 | `city_calculate_trade_yield` | ported |
| `setTradeYield(YieldTypes eIndex, int iNewValue)` | 8340-8356 | 17 | `city_set_trade_yield` | ported |
| `getExtraSpecialistYield(YieldTypes eIndex, SpecialistTypes eSpecialist) const` | 8367-8374 | 8 | `city_extra_specialist_yield` | ported |
| `updateExtraSpecialistYield(YieldTypes eYield)` | 8377-8402 | 26 | `city_update_extra_specialist_yield` | ported |
| `updateExtraSpecialistYield()` | 8405-8413 | 9 | `city_update_extra_specialist_yields` | ported |
| `getCommerceRate(CommerceTypes eIndex) const` | 8416-8419 | 4 | `city_commerce_rate` | ported |
| `getCommerceRateTimes100(CommerceTypes eIndex) const` | 8421-8440 | 20 | `city_commerce_rate_times100` | ported |
| `getCommerceFromPercent(CommerceTypes eIndex, int iYieldRate) const` | 8443-8455 | 13 | `city_commerce_from_percent` | ported |
| `getBaseCommerceRate(CommerceTypes eIndex) const` | 8458-8461 | 4 | `city_base_commerce_rate` | ported |
| `getBaseCommerceRateTimes100(CommerceTypes eIndex) const` | 8463-8480 | 18 | `city_base_commerce_rate_times100` | ported |
| `getTotalCommerceRateModifier(CommerceTypes eIndex) const` | 8483-8486 | 4 | `city_total_commerce_rate_modifier` | ported |
| `updateCommerce(CommerceTypes eIndex)` | 8489-8524 | 36 | `city_update_commerce` | ported |
| `updateCommerce()` | 8527-8535 | 9 | `city_update_commerces` | ported |
| `changeProductionToCommerceModifier(CommerceTypes eIndex, int iChange)` | 8546-8557 | 12 | `city_change_production_to_commerce_modifier` | ported |
| `getBuildingCommerceByBuilding(CommerceTypes eIndex, BuildingTypes eBuilding) const` | 8568-8640 | 73 | `city_building_commerce_by_building` | ported |
| `updateBuildingCommerce()` | 8643-8665 | 23 | `city_update_building_commerce` | ported |
| `changeSpecialistCommerce(CommerceTypes eIndex, int iChange)` | 8676-8688 | 13 | `city_change_specialist_commerce` | ported |
| `getReligionCommerceByReligion(CommerceTypes eIndex, ReligionTypes eReligion) const` | 8699-8724 | 26 | `city_religion_commerce_by_religion` | ported |
| `updateReligionCommerce(CommerceTypes eIndex)` | 8728-8747 | 20 | `city_update_religion_commerce` | ported |
| `updateReligionCommerce()` | 8750-8758 | 9 | `city_update_religion_commerces` | ported |
| `changeCommerceRateModifier(CommerceTypes eIndex, int iChange)` | 8990-9003 | 14 | `city_change_commerce_rate_modifier` | ported |
| `getCulture(PlayerTypes eIndex) const` | 9084-9089 | 6 | `city_culture` | ported |
| `countTotalCultureTimes100() const` | 9099-9115 | 17 | `city_count_total_culture_times100` | ported |
| `findHighestCulture() const` | 9118-9143 | 26 | `city_find_highest_culture` | ported |
| `calculateCulturePercent(PlayerTypes eIndex) const` | 9146-9158 | 13 | `city_calculate_culture_percent` | ported |
| `calculateTeamCulturePercent(TeamTypes eIndex) const` | 9161-9180 | 20 | `city_calculate_team_culture_percent` | ported |
| `setCulture(PlayerTypes eIndex, int iNewValue, bool bPlots, bool bUpdatePlotGroups)` | 9183-9186 | 4 | `city_set_culture` | ported |
| `setCultureTimes100(PlayerTypes eIndex, int iNewValue, bool bPlots, bool bUpdatePlotGroups)` | 9188-9205 | 18 | `city_set_culture_times100` | ported |
| `changeCulture(PlayerTypes eIndex, int iChange, bool bPlots, bool bUpdatePlotGroups)` | 9208-9211 | 4 | `city_change_culture` | ported |
| `changeCultureTimes100(PlayerTypes eIndex, int iChange, bool bPlots, bool bUpdatePlotGroups)` | 9213-9216 | 4 | `city_change_culture_times100` | ported |
| `getRevoltTestProbability() const` | 9235-9253 | 19 | `city_revolt_test_probability` | ported |
| `isRevealed(TeamTypes eIndex, bool bDebug) const` | 9290-9303 | 14 | `city_is_revealed` | ported |
| `setEspionageVisibility(TeamTypes eIndex, bool bNewValue, bool bUpdatePlotGroups)` | 9345-9358 | 14 | `city_set_espionage_visibility` | ported |
| `updateEspionageVisibility(bool bUpdatePlotGroups)` | 9360-9404 | 45 | `city_update_espionage_visibility` | ported |
| `setHasReligion(ReligionTypes eIndex, bool bNewValue, bool bAnnounce, bool bArrows)` | 10688-10810 | 123 | `city_set_has_religion` | ported |
| `processVoteSourceBonus(VoteSourceTypes eVoteSource, bool bActive)` | 10813-10876 | 64 | `city_process_vote_source_bonus` | ported |
| `getTradeCity(int iIndex) const` | 10996-11001 | 6 | `city_trade_city` | ported |
| `getTradeRoutes() const` | 11004-11017 | 14 | `city_trade_routes` | ported |
| `clearTradeRoutes()` | 11020-11036 | 17 | `city_clear_trade_routes` | ported |
| `updateTradeRoutes()` | 11040-11123 | 84 | `city_update_trade_routes` | ported |
| `doCulture()` | 11803-11817 | 15 | `city_do_culture` | ported |
| `doPlotCulture(bool bUpdate, PlayerTypes ePlayer, int iCultureRate)` | 11820-11886 | 67 | `city_do_plot_culture` | ported |
| `doReligion()` | 12145-12215 | 71 | `city_do_religion` | ported |
| `hasShrine(ReligionTypes eReligion)` | 13693-13711 | 19 | `city_has_shrine` | ported |
| `invalidatePopulationRankCache()` | 13713-13716 | 4 | `city_invalidate_population_rank_cache` | ported |
| `invalidateYieldRankCache(YieldTypes eYield)` | 13718-13735 | 18 | `city_invalidate_yield_rank_cache` | ported |
| `invalidateCommerceRankCache(CommerceTypes eCommerce)` | 13737-13752 | 16 | `city_invalidate_commerce_rank_cache` | ported |
| `getExtraSpecialistCommerce(CommerceTypes eIndex) const` | 14720-14728 | 9 | `city_extra_specialist_commerce` | ported |
| `getExtraSpecialistCommerce(CommerceTypes eIndex, SpecialistTypes eSpecialist) const` | 14730-14733 | 4 | `city_extra_specialist_commerce_by_specialist` | ported |

## Trivial accessors (no method: use the field)

C++ getters/setters/changers that only read or write a member (bounds `FAssert`s aside, UI dirty
bits aside) are field accesses on `self.cities[c]`. A `changeFoo(iChange)` of this kind is
`foo += change`.

| field | C++ accessors |
|---|---|
| `id` | `getID`, `getIDInfo` |
| `x` | `getX` |
| `y` | `getY` |
| `game_turn_founded` | `getGameTurnFounded` |
| `game_turn_acquired` | `getGameTurnAcquired`, `setGameTurnAcquired` |
| `population` | `getPopulation` |
| `highest_population` | `getHighestPopulation`, `setHighestPopulation` |
| `working_population` | `getWorkingPopulation`, `changeWorkingPopulation` |
| `specialist_population` | `getSpecialistPopulation` |
| `num_great_people` | `getNumGreatPeople` |
| `base_great_people_rate` | `getBaseGreatPeopleRate`, `changeBaseGreatPeopleRate` |
| `great_people_rate_modifier` | `getGreatPeopleRateModifier`, `changeGreatPeopleRateModifier` |
| `great_people_progress` | `getGreatPeopleProgress`, `changeGreatPeopleProgress` |
| `num_world_wonders` | `getNumWorldWonders`, `changeNumWorldWonders` |
| `num_team_wonders` | `getNumTeamWonders`, `changeNumTeamWonders` |
| `num_national_wonders` | `getNumNationalWonders`, `changeNumNationalWonders` |
| `num_buildings` | `getNumBuildings`, `changeNumBuildings` |
| `government_center_count` | `getGovernmentCenterCount` |
| `maintenance` | `getMaintenanceTimes100` |
| `maintenance_modifier` | `getMaintenanceModifier` |
| `war_weariness_modifier` | `getWarWearinessModifier` |
| `hurry_anger_modifier` | `getHurryAngerModifier` |
| `heal_rate` | `getHealRate`, `changeHealRate` |
| `espionage_health_counter` | `getEspionageHealthCounter`, `changeEspionageHealthCounter` |
| `espionage_happiness_counter` | `getEspionageHappinessCounter`, `changeEspionageHappinessCounter` |
| `fresh_water_good_health` | `getFreshWaterGoodHealth` |
| `fresh_water_bad_health` | `getFreshWaterBadHealth` |
| `feature_good_health` | `getFeatureGoodHealth` |
| `feature_bad_health` | `getFeatureBadHealth` |
| `building_good_health` | `getBuildingGoodHealth` |
| `building_bad_health` | `getBuildingBadHealth` |
| `power_good_health` | `getPowerGoodHealth` |
| `power_bad_health` | `getPowerBadHealth` |
| `bonus_good_health` | `getBonusGoodHealth` |
| `bonus_bad_health` | `getBonusBadHealth` |
| `military_happiness_units` | `getMilitaryHappinessUnits` |
| `building_good_happiness` | `getBuildingGoodHappiness` |
| `building_bad_happiness` | `getBuildingBadHappiness` |
| `extra_building_good_happiness` | `getExtraBuildingGoodHappiness` |
| `extra_building_bad_happiness` | `getExtraBuildingBadHappiness` |
| `extra_building_good_health` | `getExtraBuildingGoodHealth` |
| `extra_building_bad_health` | `getExtraBuildingBadHealth` |
| `feature_good_happiness` | `getFeatureGoodHappiness` |
| `feature_bad_happiness` | `getFeatureBadHappiness` |
| `bonus_good_happiness` | `getBonusGoodHappiness` |
| `bonus_bad_happiness` | `getBonusBadHappiness` |
| `religion_good_happiness` | `getReligionGoodHappiness` |
| `religion_bad_happiness` | `getReligionBadHappiness` |
| `extra_happiness` | `getExtraHappiness` |
| `extra_health` | `getExtraHealth` |
| `hurry_anger_timer` | `getHurryAngerTimer` |
| `conscript_anger_timer` | `getConscriptAngerTimer` |
| `defy_resolution_anger_timer` | `getDefyResolutionAngerTimer` |
| `happiness_timer` | `getHappinessTimer` |
| `no_unhappiness_count` | `getNoUnhappinessCount` |
| `no_unhealthy_population_count` | `getNoUnhealthyPopulationCount` |
| `building_only_healthy_count` | `getBuildingOnlyHealthyCount` |
| `food` | `getFood`, `setFood`, `changeFood` |
| `food_kept` | `getFoodKept`, `setFoodKept`, `changeFoodKept` |
| `max_food_kept_percent` | `getMaxFoodKeptPercent`, `changeMaxFoodKeptPercent` |
| `overflow_production` | `getOverflowProduction`, `setOverflowProduction` |
| `feature_production` | `getFeatureProduction`, `setFeatureProduction`, `changeFeatureProduction` |
| `military_production_modifier` | `getMilitaryProductionModifier`, `changeMilitaryProductionModifier` |
| `space_production_modifier` | `getSpaceProductionModifier`, `changeSpaceProductionModifier` |
| `extra_trade_routes` | `getExtraTradeRoutes` |
| `trade_route_modifier` | `getTradeRouteModifier` |
| `foreign_trade_route_modifier` | `getForeignTradeRouteModifier` |
| `building_defense` | `getBuildingDefense`, `changeBuildingDefense` |
| `building_bombard_defense` | `getBuildingBombardDefense`, `changeBuildingBombardDefense` |
| `free_experience` | `getFreeExperience`, `changeFreeExperience` |
| `curr_airlift` | `getCurrAirlift`, `setCurrAirlift`, `changeCurrAirlift` |
| `max_airlift` | `getMaxAirlift`, `changeMaxAirlift` |
| `air_modifier` | `getAirModifier`, `changeAirModifier` |
| `air_unit_capacity` | `changeAirUnitCapacity` |
| `nuke_modifier` | `getNukeModifier`, `changeNukeModifier` |
| `free_specialist` | `getFreeSpecialist` |
| `power_count` | `getPowerCount` |
| `dirty_power_count` | `getDirtyPowerCount` |
| `defense_damage` | `getDefenseDamage` |
| `last_defense_damage` | `getLastDefenseDamage`, `setLastDefenseDamage` |
| `occupation_timer` | `getOccupationTimer` |
| `culture_update_timer` | `getCultureUpdateTimer`, `setCultureUpdateTimer`, `changeCultureUpdateTimer` |
| `never_lost` | `isNeverLost`, `setNeverLost` |
| `bombarded` | `isBombarded`, `setBombarded` |
| `drafted` | `isDrafted`, `setDrafted` |
| `airlift_targeted` | `isAirliftTargeted`, `setAirliftTargeted` |
| `plundered` | `isPlundered` |
| `we_love_the_king_day` | `isWeLoveTheKingDay` |
| `citizens_automated` | `isCitizensAutomated` |
| `production_automated` | `isProductionAutomated` |
| `owner` | `getOwner` |
| `previous_owner` | `getPreviousOwner`, `setPreviousOwner` |
| `original_owner` | `getOriginalOwner`, `setOriginalOwner` |
| `culture_level` | `getCultureLevel` |
| `sea_plot_yield` | `getSeaPlotYield` |
| `river_plot_yield` | `getRiverPlotYield` |
| `base_yield_rate` | `getBaseYieldRate` |
| `yield_rate_modifier` | `getYieldRateModifier` |
| `power_yield_rate_modifier` | `getPowerYieldRateModifier` |
| `bonus_yield_rate_modifier` | `getBonusYieldRateModifier` |
| `trade_yield` | `getTradeYield` |
| `extra_specialist_yield` | `getExtraSpecialistYield` |
| `production_to_commerce_modifier` | `getProductionToCommerceModifier` |
| `building_commerce` | `getBuildingCommerce` |
| `specialist_commerce` | `getSpecialistCommerce` |
| `religion_commerce` | `getReligionCommerce` |
| `corporation_yield` | `getCorporationYield` |
| `corporation_commerce` | `getCorporationCommerce` |
| `commerce_rate_modifier` | `getCommerceRateModifier` |
| `commerce_happiness_per` | `getCommerceHappinessPer` |
| `domain_free_experience` | `getDomainFreeExperience`, `changeDomainFreeExperience` |
| `domain_production_modifier` | `getDomainProductionModifier`, `changeDomainProductionModifier` |
| `culture` | `getCultureTimes100` |
| `num_revolts` | `getNumRevolts`, `changeNumRevolts` |
| `ever_owned` | `isEverOwned`, `setEverOwned` |
| `trade_route` | `isTradeRoute`, `setTradeRoute` |
| `revealed` | `setRevealed` |
| `espionage_visibility` | `getEspionageVisibility` |
| `name` | `getNameKey` |
| `script_data` | `getScriptData`, `setScriptData` |
| `no_bonus` | `getNoBonusCount` |
| `free_bonus` | `getFreeBonus` |
| `num_corp_produced_bonuses` | `getNumCorpProducedBonuses` |
| `building_production` | `getBuildingProduction`, `setBuildingProduction`, `changeBuildingProduction` |
| `building_production_time` | `getBuildingProductionTime`, `setBuildingProductionTime`, `changeBuildingProductionTime` |
| `project_production` | `getProjectProduction`, `setProjectProduction`, `changeProjectProduction` |
| `building_original_owner` | `getBuildingOriginalOwner` |
| `building_original_time` | `getBuildingOriginalTime` |
| `unit_production` | `getUnitProduction`, `setUnitProduction`, `changeUnitProduction` |
| `unit_production_time` | `getUnitProductionTime`, `setUnitProductionTime`, `changeUnitProductionTime` |
| `great_people_unit_rate` | `getGreatPeopleUnitRate` |
| `great_people_unit_progress` | `getGreatPeopleUnitProgress`, `setGreatPeopleUnitProgress`, `changeGreatPeopleUnitProgress` |
| `specialist_count` | `getSpecialistCount` |
| `max_specialist_count` | `getMaxSpecialistCount` |
| `force_specialist_count` | `getForceSpecialistCount` |
| `free_specialist_count` | `getFreeSpecialistCount` |
| `improvement_free_specialists` | `getImprovementFreeSpecialists` |
| `religion_influence` | `getReligionInfluence`, `changeReligionInfluence` |
| `state_religion_happiness` | `getStateReligionHappiness` |
| `unit_combat_free_experience` | `getUnitCombatFreeExperience`, `changeUnitCombatFreeExperience` |
| `free_promotion_count` | `getFreePromotionCount`, `changeFreePromotionCount` |
| `specialist_free_experience` | `getSpecialistFreeExperience`, `changeSpecialistFreeExperience` |
| `espionage_defense_modifier` | `getEspionageDefenseModifier`, `changeEspionageDefenseModifier` |
| `working_plot` | `isWorkingPlot` |
| `num_real_building` | `getNumRealBuilding` |
| `num_free_building` | `getNumFreeBuilding` |
| `has_religion` | `isHasReligion` |
| `has_corporation` | `isHasCorporation` |
| `order_queue` | `getOrderQueueLength`, `getOrderFromQueue`, `nextOrderQueueNode`, `headOrderQueueNode`, `getNumOrdersQueued`, `getOrderData` |
| `civ` | `getTrueCivilizationType`, `setCivilizationType` |
| `settlement` | `isSettlement`, `setSettlement` |
| `plot_radius` | `getPlotRadius` |
| `unhappy_production` | `changeUnhappyProduction` |
| `crime` | `changeCrime` |
| `mutate_chance` | `changeMutateChance`, `getMutateChance` |
| `resist_magic` | `changeResistMagic`, `getResistMagic` |

## Not ported

- **constructor / `reset` (ported as `City::new`)**: `CvCity` (32-102), `uninit` (351-385), `reset` (389-713)
- **destructor (Rust drop)**: `~CvCity` (104-143)
- **graphics / entity / art**: `setupGraphical` (719-730), `updateVisibility` (1231-1243), `getArtStyleType` (4283-4291), `getCitySizeType` (4294-4297), `getBuildingArtInfo` (4299-4302), `getBuildingVisibilityPriority` (4304-4307), `getCitySizeBoost` (7554-7557), `setCitySizeBoost` (7560-7568), `isWallOverride` (7742-7745), `setWallOverride` (7748-7756), `isLayoutDirty` (7771-7774), `setLayoutDirty` (7777-7780), `setWallOverridePoints` (11730-11734), `getWallOverridePoints` (11736-11739), `getVisibleBuildings` (12828-12885), `getVisibleEffects` (12893-12949), `getUnitArtStyleType` (14740-14762)
- **UI (selection, billboards, info bars, exe queue strings)**: `isCitySelected` (1153-1156), `canBeSelected` (1159-1189), `updateSelectedCity` (1192-1210), `isInfoDirty` (7759-7762), `setInfoDirty` (7765-7768), `getCityBillboardSizeIconColors` (12951-12994), `getCityBillboardProductionIcon` (12996-13054), `getFoodBarPercentages` (13056-13076), `getProductionBarPercentages` (13078-13096), `getBarBackgroundColor` (13098-13105), `isStarCity` (13107-13110), `getBuildQueue` (14466-14496), `getUnitArtStyleButton` (14764-14767)
- **UI text (localized names; log messages can use the info's `base.description`)**: `getProductionName` (2763-2794), `getProductionNameKey` (2831-2862), `getName` (9412-9415)
- **free-list index (`CityId` replaces it)**: `getIndex` (5325-5328)
- **id assignment (ids are `Game::cities` slab keys)**: `setID` (5338-5341)
- **serialization**: `read` (12325-12575), `write` (12577-12806)
- **sound / music**: `getMusicScriptId` (14412-14440), `getSoundscapeScriptId` (14442-14445)
- **debug cheat**: `cheat` (14447-14464)
- **declared in CvCity.h but never defined or called**: `getOverflowProductionDifference`,
  `isAreaBorderObstacle`.
- **`AI_*` virtuals** of `CvCity.h` other than those in the CvCityAI table above: AI.

## Porting notes (C++ quirks spotted while reading)

- `init`: besides the two "Crime" draws, C++ finishes with `AI_init()` -> `AI_reset()`, which
  clears the AI bookkeeping fields again after `init` has run (e.g. an
  `ai_choose_production_dirty` set by `setPopulation` for an AI owner).
- `setPlotRadius`: the loop that frees worked outer plots tests `iNewValue < getPlotRadius()`
  after `m_iPlotRadius = iNewValue`, so it never runs.
- `setNumFreeBuilding` calls `processBuilding(eIndex, iNewValue - iOldNumBuilding)` (new free
  count minus old total count), not the change in `getNumBuilding` (kept as is).
- `AI_getEmphasizeCommerceCount` returns `count > 0` (a bool as int); `AI_isEmphasizeCommerce`
  is unaffected.
- `getRealPopulation` returns a 32-bit `long` on MSVC (ported with a wrapping multiply).
- `setBuildingHappyChange` / `setBuildingHealthChange`: the UNOFFICIAL_PATCH versions are live;
  the BTS originals right above them are commented out (one of them changed happiness from the
  health setter).
- `processBuilding` adds the building's `ModifyGlobalCounter` to the FFH global counter without
  multiplying by `iChange`, so removing such a building raises the counter again; `kill`
  subtracts it explicitly before removing each building. `kill` also clears trade routes
  (UNOFFICIAL_PATCH).
- `doGrowth` halts growth through `AI_stopGrowth` for every owner (humans set the emphasize
  flags with `TASK_SET_EMPHASIZE`).
- `getProductionModifier(UnitTypes)` is not clamped at 0 in FFH (the building / project
  versions are).
- `isDisorder` is also true while the owner's production is disabled (FFH).
- `changeOverflowProduction` stores the overflow divided by the production modifier.
