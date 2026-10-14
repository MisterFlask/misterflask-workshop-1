# Porting status: CvUnit

C++ source: `CvGameCoreDLL/CvUnit.h` and `CvUnit.cpp` (18545 lines, 671 functions totalling
17356 lines), plus the `CvUnitAI` bookkeeping that rules code calls. Rust:
`crates/ffh-core/src/unit/`.

Line numbers refer to the FFH2 0.41 reference (`/home/user/ref/ffh2/CvGameCoreDLL`); counts
are function lines including the signature and braces. Each `unit/<topic>.rs` file below holds
the methods listed for it, each with a `/// C++ CvUnit::name (CvUnit.cpp:a-b)` doc line.

**Status: ported and audited.** Every method has a complete body (no `todo!` left), and each file
was compared line by line with the C++ in the audit wave.

## Files

| Rust file | C++ functions | C++ lines | Rust methods | ported |
|---|---:|---:|---:|---:|
| `unit/mod.rs` | 1 (`reset`) | 214 | `Unit::new` | yes |
| `unit/lifecycle.rs` | 35 | 2100 | 35 | 35 |
| `unit/movement.rs` | 68 | 1706 | 68 | 68 |
| `unit/combat.rs` | 51 | 3180 | 51 | 51 |
| `unit/promotions.rs` | 141 | 1974 | 141 | 141 |
| `unit/actions.rs` | 46 | 1940 | 46 | 46 |
| `unit/missions.rs` | 52 | 1883 | 52 | 52 |
| `unit/ffh.rs` | 98 | 2406 | 98 | 98 |
| **total** | 492 | 15403 | 491 | 491 |

Plus 124 trivial C++ accessors that are plain field access (556 lines) and 60 excluded
functions (1454 lines), listed at the end.

## Conventions specific to CvUnit

- Methods are `impl Game` methods `unit_<snake_case C++ name>(u: UnitId, ..)`. C++ overloads get
  distinct names: `canAttack(defender)` -> `unit_can_attack_unit`, `hasUpgrade(eUnit, ..)` ->
  `unit_has_upgrade_to`, `getUpgradeCity(eUnit, ..)` -> `unit_get_upgrade_city_for`.
  `getArea()`/`area()` are both `unit_area`. `plot()` is `unit_plot_opt` (faithful, `None` off the
  map) plus the convenience `unit_plot` for call sites that dereference it.
- Constness: C++ `const` -> `&self`. A few non-const C++ queries that change nothing are `&self`
  (`canDoCommand`, `canGift`, `canCast` and the other `can*` spell tests, `canSpellAutomate`,
  `countUnitsWithinRange`); `isResisted` is `const` in C++ but draws a sync rand -> `&mut self`.
- Pointers: `CvPlot*` -> `PlotIdx` (`Option<PlotIdx>` where C++ accepts/returns NULL),
  `CvUnit*`/`IDInfo` -> `UnitId`, `CvCity*` -> `CityId`, `CvSelectionGroup*` -> `GroupId`,
  `CvArea*` -> `AreaId`, `CombatDetails*` -> `Option<&mut CombatDetails>`, `int*` out-params ->
  `Option<&mut i32>`. `getDefenderCombatValues` returns its `int&` out-params as
  `DefenderCombatValues`; `resolveAirCombat` returns the damages it stored in the air mission.
- Type arguments with C++ `NO_X` handling are `Option<XType>` (`canSpread(eReligion)`,
  `canPromote(ePromotion, iLeaderUnitId)`, `canUpgrade(eUnit)`, `canAutomate`, ...); teams are
  `Option<TeamId>` where C++ handles NO_TEAM (`isEnemy`, `canEnterTerritory`, `getCombatOwner`, ...).
  Per-player unit ids (`iLeaderUnitId`, `iUnitId`, `m_iSummoner`) are global `UnitId`s here.
  Target coordinates of `*At(pPlot, iX, iY)` / `airlift(iX, iY)` / `rangeStrike(iX, iY)` stay `x, y`
  (they may be off the map). `setExperience`/`changeExperience` keep `max == -1` = no maximum.
- Dropped parameters (interface only): `bQuick` (combat animations; the port always takes the
  C++ "not visible" path, see `isCombatVisible`), `bShow`/`bUpdate`/`bCheckPlotVisible` of
  `setXY`/`move`, `bNotifyEntity` of `setDamage`, `bRemoveSelected` of `joinGroup`, `bDebug` of
  `isInvisible*`, `eFacingDirection` of `init`, the `CvBattleDefinition&`/`CvAirMissionDefinition&`
  of `resolveCombat`/`resolveAirCombat`, `uiForm` of `getName`.
- Python callbacks reached from CvUnit (spell `PyRequirement`/`PyResult`, promotion `PyPerTurn`,
  improvement/feature `PythonOnMove`/`PythonAtRange`, `getExperienceNeeded`, `doPillageGold`,
  `getUpgradePriceOverride`, `unitCannotMoveInto`, `cannotSpreadReligion`, `postCombatWon/Lost`,
  event trigger `PythonCanDoUnit`, CvEventReporter events) go to their ports in `crate::ffh`.
- C++ `FAssert`s are debug-build checks (no-ops in the shipped DLL) and are not ported as
  runtime checks: the C++ itself violates some of them (`pillage()` drives `extra_withdrawal`
  negative while fighting an interceptor; a promotion has a negative `iCargoChange`).
- AI calls: decisions (`AI_acceptUnit`, `AI_captureUnit`) go through the controller; AI bookkeeping
  (`ai_type`, `ai_groupflag`, origin plot, summon flags, allowed-group flags) are `Unit` fields;
  `AI_init`/`AI_setUnitAIType`/`AI_isCityAIType`/`getOriginPlot`/`setOriginPlot` are ported as
  `unit_ai_*` (lifecycle.rs) because rules code depends on their side effects; AI valuations
  (`AI_promotionValue`, `AI_unitValue`, `AI_update`, `isUnitAllowedPermDefense`, ...) are omitted
  with an `// AI:` note.

## unit/lifecycle.rs -- 35 C++ functions, 2100 C++ lines

CvUnit lifecycle: `init`, `convert`, `kill`, `doTurn`, selection-group membership, position (`setXY`, `plot`), damage, ownership, names and random events; plus the few `CvUnitAI` bookkeeping functions that rules code depends on (`AI_init`, `AI_setUnitAIType`, ...).

| C++ function | C++ lines | count | Rust method | status |
|---|---|---:|---|---|
| `CvUnit::init` | CvUnit.cpp:106-308 | 203 | `unit_init` | ported |
| `CvUnit::convert` | CvUnit.cpp:570-719 | 150 | `unit_convert` | ported |
| `CvUnit::kill` | CvUnit.cpp:722-1002 | 281 | `unit_kill` | ported |
| `CvUnit::doTurn` | CvUnit.cpp:1011-1269 | 259 | `unit_do_turn` | ported |
| `CvUnit::isInGroup` | CvUnit.cpp:10724-10727 | 4 | `unit_is_in_group` | ported |
| `CvUnit::isGroupHead` | CvUnit.cpp:10730-10733 | 4 | `unit_is_group_head` | ported |
| `CvUnit::canJoinGroup` | CvUnit.cpp:10742-10811 | 70 | `unit_can_join_group` | ported |
| `CvUnit::joinGroup` | CvUnit.cpp:10814-10944 | 131 | `unit_join_group` | ported |
| `CvUnit::setXY` | CvUnit.cpp:10995-11579 | 585 | `unit_set_xy` | ported |
| `CvUnit::at` | CvUnit.cpp:11582-11585 | 4 | `unit_at` | ported |
| `CvUnit::atPlot` | CvUnit.cpp:11588-11591 | 4 | `unit_at_plot` | ported |
| `CvUnit::plot` | CvUnit.cpp:11594-11597 | 4 | `unit_plot_opt` | ported |
| (none) | | | `unit_plot` | ported (convenience) |
| `CvUnit::getArea` | CvUnit.cpp:11600-11603 | 4 | `unit_area` | ported |
| `CvUnit::area` | CvUnit.cpp:11606-11609 | 4 | `unit_area` | ported |
| `CvUnit::onMap` | CvUnit.cpp:11612-11615 | 4 | `unit_on_map` | ported |
| `CvUnit::getReconPlot` | CvUnit.cpp:11631-11634 | 4 | `unit_get_recon_plot` | ported |
| `CvUnit::setReconPlot` | CvUnit.cpp:11637-11665 | 29 | `unit_set_recon_plot` | ported |
| `CvUnit::setDamage` | CvUnit.cpp:11687-11721 | 35 | `unit_set_damage` | ported |
| `CvUnit::changeDamage` | CvUnit.cpp:11724-11727 | 4 | `unit_change_damage` | ported |
| `CvUnit::doDelayedDeath` | CvUnit.cpp:12643-12652 | 10 | `unit_do_delayed_death` | ported |
| `CvUnit::getVisualOwner` | CvUnit.cpp:12736-12760 | 25 | `unit_get_visual_owner` | ported |
| `CvUnit::getCombatOwner` | CvUnit.cpp:12763-12774 | 12 | `unit_get_combat_owner` | ported |
| `CvUnit::getTeam` | CvUnit.cpp:12776-12779 | 4 | `unit_get_team` | ported |
| `CvUnit::setCapturingPlayer` | CvUnit.cpp:12788-12799 | 12 | `unit_set_capturing_player` | ported |
| `CvUnit::getName` | CvUnit.cpp:12997-13009 | 13 | `unit_get_name` | ported |
| `CvUnit::getNameKey` | CvUnit.cpp:13012-13022 | 11 | `unit_get_name_key` | ported |
| `CvUnit::setName` | CvUnit.cpp:13031-13041 | 11 | `unit_set_name` | ported |
| `CvUnit::getTriggerValue` | CvUnit.cpp:18082-18158 | 77 | `unit_get_trigger_value` | ported |
| `CvUnit::canApplyEvent` | CvUnit.cpp:18160-18197 | 38 | `unit_can_apply_event` | ported |
| `CvUnit::applyEvent` | CvUnit.cpp:18199-18245 | 47 | `unit_apply_event` | ported |
| `CvUnitAI::AI_init` | CvUnitAI.cpp:41-52 | 12 | `unit_ai_init` | ported |
| `CvUnitAI::AI_isCityAIType` | CvUnitAI.cpp:1376-1382 | 7 | `unit_ai_is_city_ai_type` | ported |
| `CvUnitAI::AI_setUnitAIType` | CvUnitAI.cpp:1432-1448 | 17 | `unit_ai_set_unit_ai_type` | ported |
| `CvUnitAI::getOriginPlot` | CvUnitAI.cpp:19100-19107 | 8 | `unit_ai_get_origin_plot` | ported |
| `CvUnitAI::setOriginPlot` | CvUnitAI.cpp:19109-19121 | 13 | `unit_ai_set_origin_plot` | ported |

## unit/movement.rs -- 68 C++ functions, 1706 C++ lines

CvUnit movement: territory/area entry, `canMoveInto`, `move`, paths, loading and cargo, moves left, visibility and invisibility, enemy tests, jump/withdraw to a valid plot.

| C++ function | C++ lines | count | Rust method | status |
|---|---|---:|---|---|
| `CvUnit::getPathEndTurnPlot` | CvUnit.cpp:2914-2917 | 4 | `unit_get_path_end_turn_plot` | ported |
| `CvUnit::generatePath` | CvUnit.cpp:2920-2923 | 4 | `unit_generate_path` | ported |
| `CvUnit::canEnterTerritory` | CvUnit.cpp:2926-2987 | 62 | `unit_can_enter_territory` | ported |
| `CvUnit::canEnterArea` | CvUnit.cpp:2990-3009 | 20 | `unit_can_enter_area` | ported |
| `CvUnit::getDeclareWarMove` | CvUnit.cpp:3013-3057 | 45 | `unit_get_declare_war_move` | ported |
| `CvUnit::willRevealByMove` | CvUnit.cpp:3059-3078 | 20 | `unit_will_reveal_by_move` | ported |
| `CvUnit::canMoveInto` | CvUnit.cpp:3080-3444 | 365 | `unit_can_move_into` | ported |
| `CvUnit::canMoveOrAttackInto` | CvUnit.cpp:3447-3450 | 4 | `unit_can_move_or_attack_into` | ported |
| `CvUnit::canMoveThrough` | CvUnit.cpp:3453-3456 | 4 | `unit_can_move_through` | ported |
| `CvUnit::move` | CvUnit.cpp:3598-3645 | 48 | `unit_move` | ported |
| `CvUnit::jumpToNearestValidPlot` | CvUnit.cpp:3648-3727 | 80 | `unit_jump_to_nearest_valid_plot` | ported |
| `CvUnit::canLoadUnit` | CvUnit.cpp:4094-4164 | 71 | `unit_can_load_unit` | ported |
| `CvUnit::loadUnit` | CvUnit.cpp:4167-4175 | 9 | `unit_load_unit` | ported |
| `CvUnit::shouldLoadOnMove` | CvUnit.cpp:4177-4233 | 57 | `unit_should_load_on_move` | ported |
| `CvUnit::canLoad` | CvUnit.cpp:4236-4255 | 20 | `unit_can_load` | ported |
| `CvUnit::load` | CvUnit.cpp:4258-4296 | 39 | `unit_load` | ported |
| `CvUnit::canUnload` | CvUnit.cpp:4299-4337 | 39 | `unit_can_unload` | ported |
| `CvUnit::unload` | CvUnit.cpp:4340-4348 | 9 | `unit_unload` | ported |
| `CvUnit::canUnloadAll` | CvUnit.cpp:4351-4359 | 9 | `unit_can_unload_all` | ported |
| `CvUnit::unloadAll` | CvUnit.cpp:4362-4384 | 23 | `unit_unload_all` | ported |
| `CvUnit::getInvisibleType` | CvUnit.cpp:8519-8557 | 39 | `unit_get_invisible_type` | ported |
| `CvUnit::getNumSeeInvisibleTypes` | CvUnit.cpp:8559-8570 | 12 | `unit_get_num_see_invisible_types` | ported |
| `CvUnit::getSeeInvisibleType` | CvUnit.cpp:8572-8583 | 12 | `unit_get_see_invisible_type` | ported |
| `CvUnit::visibilityRange` | CvUnit.cpp:8604-8618 | 15 | `unit_visibility_range` | ported |
| `CvUnit::baseMoves` | CvUnit.cpp:8621-8624 | 4 | `unit_base_moves` | ported |
| `CvUnit::maxMoves` | CvUnit.cpp:8627-8630 | 4 | `unit_max_moves` | ported |
| `CvUnit::movesLeft` | CvUnit.cpp:8633-8636 | 4 | `unit_moves_left` | ported |
| `CvUnit::canMove` | CvUnit.cpp:8639-8657 | 19 | `unit_can_move` | ported |
| `CvUnit::hasMoved` | CvUnit.cpp:8660-8663 | 4 | `unit_has_moved` | ported |
| `CvUnit::canCoexistWithEnemyUnit` | CvUnit.cpp:8885-8903 | 19 | `unit_can_coexist_with_enemy_unit` | ported |
| `CvUnit::alwaysInvisible` | CvUnit.cpp:10165-10168 | 4 | `unit_always_invisible` | ported |
| `CvUnit::canMoveImpassable` | CvUnit.cpp:10198-10209 | 12 | `unit_can_move_impassable` | ported |
| `CvUnit::canMoveAllTerrain` | CvUnit.cpp:10211-10222 | 12 | `unit_can_move_all_terrain` | ported |
| `CvUnit::flatMovementCost` | CvUnit.cpp:10224-10235 | 12 | `unit_flat_movement_cost` | ported |
| `CvUnit::ignoreTerrainCost` | CvUnit.cpp:10238-10249 | 12 | `unit_ignore_terrain_cost` | ported |
| `CvUnit::isNeverInvisible` | CvUnit.cpp:10252-10255 | 4 | `unit_is_never_invisible` | ported |
| `CvUnit::isInvisible` | CvUnit.cpp:10258-10323 | 66 | `unit_is_invisible` | ported |
| `CvUnit::isInvisibleInPlot` | CvUnit.cpp:10326-10380 | 55 | `unit_is_invisible_in_plot` | ported |
| `CvUnit::specialCargo` | CvUnit.cpp:10561-10564 | 4 | `unit_special_cargo` | ported |
| `CvUnit::domainCargo` | CvUnit.cpp:10567-10570 | 4 | `unit_domain_cargo` | ported |
| `CvUnit::changeCargoSpace` | CvUnit.cpp:10578-10586 | 9 | `unit_change_cargo_space` | ported |
| `CvUnit::isFull` | CvUnit.cpp:10588-10591 | 4 | `unit_is_full` | ported |
| `CvUnit::cargoSpaceAvailable` | CvUnit.cpp:10594-10613 | 20 | `unit_cargo_space_available` | ported |
| `CvUnit::hasCargo` | CvUnit.cpp:10616-10619 | 4 | `unit_has_cargo` | ported |
| `CvUnit::canCargoAllMove` | CvUnit.cpp:10622-10650 | 29 | `unit_can_cargo_all_move` | ported |
| `CvUnit::canCargoEnterArea` | CvUnit.cpp:10652-10673 | 22 | `unit_can_cargo_enter_area` | ported |
| `CvUnit::getUnitAICargo` | CvUnit.cpp:10675-10690 | 16 | `unit_get_unit_ai_cargo` | ported |
| `CvUnit::changeMoves` | CvUnit.cpp:11771-11774 | 4 | `unit_change_moves` | ported |
| `CvUnit::finishMoves` | CvUnit.cpp:11777-11780 | 4 | `unit_finish_moves` | ported |
| `CvUnit::changeCargo` | CvUnit.cpp:11876-11880 | 5 | `unit_change_cargo` | ported |
| `CvUnit::getCargoUnits` | CvUnit.cpp:11882-11902 | 21 | `unit_get_cargo_units` | ported |
| `CvUnit::changeExtraVisibilityRange` | CvUnit.cpp:12100-12111 | 12 | `unit_change_extra_visibility_range` | ported |
| `CvUnit::getImmobileTimer` | CvUnit.cpp:12516-12527 | 12 | `unit_get_immobile_timer` | ported |
| `CvUnit::setImmobileTimer` | CvUnit.cpp:12529-12551 | 23 | `unit_set_immobile_timer` | ported |
| `CvUnit::changeImmobileTimer` | CvUnit.cpp:12553-12559 | 7 | `unit_change_immobile_timer` | ported |
| `CvUnit::isCargo` | CvUnit.cpp:12918-12921 | 4 | `unit_is_cargo` | ported |
| `CvUnit::setTransportUnit` | CvUnit.cpp:12924-12978 | 55 | `unit_set_transport_unit` | ported |
| `CvUnit::isTerrainDoubleMove` | CvUnit.cpp:13064-13069 | 6 | `unit_is_terrain_double_move` | ported |
| `CvUnit::changeTerrainDoubleMoveCount` | CvUnit.cpp:13072-13078 | 7 | `unit_change_terrain_double_move_count` | ported |
| `CvUnit::isFeatureDoubleMove` | CvUnit.cpp:13089-13094 | 6 | `unit_is_feature_double_move` | ported |
| `CvUnit::changeFeatureDoubleMoveCount` | CvUnit.cpp:13097-13103 | 7 | `unit_change_feature_double_move_count` | ported |
| `CvUnit::potentialWarAction` | CvUnit.cpp:13651-13672 | 22 | `unit_potential_war_action` | ported |
| `CvUnit::getOpenPlot` | CvUnit.cpp:16728-16763 | 36 | `unit_get_open_plot` | ported |
| `CvUnit::withdrawlToNearestValidPlot` | CvUnit.cpp:16805-16864 | 60 | `unit_withdrawl_to_nearest_valid_plot` | ported |
| `CvUnit::isEnemy` | CvUnit.cpp:18011-18029 | 19 | `unit_is_enemy` | ported |
| `CvUnit::isPotentialEnemy` | CvUnit.cpp:18031-18039 | 9 | `unit_is_potential_enemy` | ported |
| `CvUnit::isAlwaysHostile` | CvUnit.cpp:18339-18360 | 22 | `unit_is_always_hostile` | ported |
| `CvUnit::verifyStackValid` | CvUnit.cpp:18362-18373 | 12 | `unit_verify_stack_valid` | ported |

## unit/combat.rs -- 51 C++ functions, 3180 C++ lines

CvUnit combat: strength (`maxCombatStr` & co., `isBetterDefenderThan`, attack/defend tests, air strength and damage) and resolution (land and air combat, collateral and flanking damage, interception, air and range strikes, combat state), plus the FfH combat additions (`combatWon`, defensive strikes).

| C++ function | C++ lines | count | Rust method | status |
|---|---|---:|---|---|
| `CvUnit::updateAirStrike` | CvUnit.cpp:1272-1329 | 58 | `unit_update_air_strike` | ported |
| `CvUnit::resolveAirCombat` | CvUnit.cpp:1331-1412 | 82 | `unit_resolve_air_combat` | ported |
| `CvUnit::updateAirCombat` | CvUnit.cpp:1415-1582 | 168 | `unit_update_air_combat` | ported |
| `CvUnit::resolveCombat` | CvUnit.cpp:1584-1783 | 200 | `unit_resolve_combat` | ported |
| `CvUnit::updateCombat` | CvUnit.cpp:1786-2251 | 466 | `unit_update_combat` | ported |
| `CvUnit::isBetterDefenderThan` | CvUnit.cpp:2479-2646 | 168 | `unit_is_better_defender_than` | ported |
| `CvUnit::attack` | CvUnit.cpp:3459-3467 | 9 | `unit_attack` | ported |
| `CvUnit::fightInterceptor` | CvUnit.cpp:3469-3476 | 8 | `unit_fight_interceptor` | ported |
| `CvUnit::attackForDamage` | CvUnit.cpp:3478-3595 | 118 | `unit_attack_for_damage` | ported |
| `CvUnit::isFighting` | CvUnit.cpp:8905-8908 | 4 | `unit_is_fighting` | ported |
| `CvUnit::isAttacking` | CvUnit.cpp:8911-8914 | 4 | `unit_is_attacking` | ported |
| `CvUnit::isDefending` | CvUnit.cpp:8917-8920 | 4 | `unit_is_defending` | ported |
| `CvUnit::isCombat` | CvUnit.cpp:8923-8926 | 4 | `unit_is_combat` | ported |
| `CvUnit::maxCombatStr` | CvUnit.cpp:8997-9557 | 561 | `unit_max_combat_str` | ported |
| `CvUnit::currCombatStr` | CvUnit.cpp:9560-9563 | 4 | `unit_curr_combat_str` | ported |
| `CvUnit::currFirepower` | CvUnit.cpp:9566-9569 | 4 | `unit_curr_firepower` | ported |
| `CvUnit::currEffectiveStr` | CvUnit.cpp:9574-9582 | 9 | `unit_curr_effective_str` | ported |
| `CvUnit::canFight` | CvUnit.cpp:9596-9608 | 13 | `unit_can_fight` | ported |
| `CvUnit::canAttack` | CvUnit.cpp:9611-9636 | 26 | `unit_can_attack` | ported |
| `CvUnit::canAttack` | CvUnit.cpp:9637-9659 | 23 | `unit_can_attack_unit` | ported |
| `CvUnit::canDefend` | CvUnit.cpp:9661-9689 | 29 | `unit_can_defend` | ported |
| `CvUnit::canSiege` | CvUnit.cpp:9692-9710 | 19 | `unit_can_siege` | ported |
| `CvUnit::airBaseCombatStr` | CvUnit.cpp:9713-9716 | 4 | `unit_air_base_combat_str` | ported |
| `CvUnit::airMaxCombatStr` | CvUnit.cpp:9719-9766 | 48 | `unit_air_max_combat_str` | ported |
| `CvUnit::airCurrCombatStr` | CvUnit.cpp:9769-9772 | 4 | `unit_air_curr_combat_str` | ported |
| `CvUnit::airCombatLimit` | CvUnit.cpp:9798-9801 | 4 | `unit_air_combat_limit` | ported |
| `CvUnit::canAirAttack` | CvUnit.cpp:9804-9807 | 4 | `unit_can_air_attack` | ported |
| `CvUnit::canAirDefend` | CvUnit.cpp:9810-9844 | 35 | `unit_can_air_defend` | ported |
| `CvUnit::airCombatDamage` | CvUnit.cpp:9847-9875 | 29 | `unit_air_combat_damage` | ported |
| `CvUnit::rangeCombatDamage` | CvUnit.cpp:9878-9897 | 20 | `unit_range_combat_damage` | ported |
| `CvUnit::bestInterceptor` | CvUnit.cpp:9900-9948 | 49 | `unit_best_interceptor` | ported |
| `CvUnit::bestSeaPillageInterceptor` | CvUnit.cpp:9951-10001 | 51 | `unit_best_sea_pillage_interceptor` | ported |
| `CvUnit::getAttackPlot` | CvUnit.cpp:11904-11907 | 4 | `unit_get_attack_plot` | ported |
| `CvUnit::setAttackPlot` | CvUnit.cpp:11910-11927 | 18 | `unit_set_attack_plot` | ported |
| `CvUnit::changeCombatTimer` | CvUnit.cpp:11945-11948 | 4 | `unit_change_combat_timer` | ported |
| `CvUnit::changeCombatFirstStrikes` | CvUnit.cpp:11961-11964 | 4 | `unit_change_combat_first_strikes` | ported |
| `CvUnit::setCombatUnit` | CvUnit.cpp:12838-12909 | 72 | `unit_set_combat_unit` | ported |
| `CvUnit::doDefensiveStrike` | CvUnit.cpp:16206-16259 | 54 | `unit_do_defensive_strike` | ported |
| `CvUnit::combatWon` | CvUnit.cpp:16306-16560 | 255 | `unit_combat_won` | ported |
| `CvUnit::canAdvance` | CvUnit.cpp:17230-17251 | 22 | `unit_can_advance` | ported |
| `CvUnit::collateralCombat` | CvUnit.cpp:17254-17382 | 129 | `unit_collateral_combat` | ported |
| `CvUnit::flankingStrikeCombat` | CvUnit.cpp:17385-17468 | 84 | `unit_flanking_strike_combat` | ported |
| `CvUnit::interceptTest` | CvUnit.cpp:17472-17490 | 19 | `unit_intercept_test` | ported |
| `CvUnit::airStrikeTarget` | CvUnit.cpp:17493-17511 | 19 | `unit_air_strike_target` | ported |
| `CvUnit::canAirStrike` | CvUnit.cpp:17514-17547 | 34 | `unit_can_air_strike` | ported |
| `CvUnit::airStrike` | CvUnit.cpp:17550-17587 | 38 | `unit_air_strike` | ported |
| `CvUnit::canRangeStrike` | CvUnit.cpp:17589-17622 | 34 | `unit_can_range_strike` | ported |
| `CvUnit::canRangeStrikeAt` | CvUnit.cpp:17624-17660 | 37 | `unit_can_range_strike_at` | ported |
| `CvUnit::rangeStrike` | CvUnit.cpp:17663-17726 | 64 | `unit_range_strike` | ported |
| `CvUnit::isTargetOf` | CvUnit.cpp:17980-18009 | 30 | `unit_is_target_of` | ported |
| `CvUnit::getDefenderCombatValues` | CvUnit.cpp:18051-18080 | 30 | `unit_get_defender_combat_values` | ported |

## unit/promotions.rs -- 141 C++ functions, 1974 C++ lines

CvUnit promotions, experience and levels, leaders (great generals), upgrades, and the unit characteristics they modify: promotion-driven counters (`changeBlitzCount`, `changeExtraMoves`, ...), hit points and base strength, combat modifiers, first strikes, XP values, interception, fortification and the queries derived from the unit info (`getDomainType`, `isAnimal`, ...).

| C++ function | C++ lines | count | Rust method | status |
|---|---|---:|---|---|
| `CvUnit::canPromote` | CvUnit.cpp:7717-7776 | 60 | `unit_can_promote` | ported |
| `CvUnit::promote` | CvUnit.cpp:7778-7835 | 58 | `unit_promote` | ported |
| `CvUnit::lead` | CvUnit.cpp:7837-7877 | 41 | `unit_lead` | ported |
| `CvUnit::canLead` | CvUnit.cpp:7880-7920 | 41 | `unit_can_lead` | ported |
| `CvUnit::canGiveExperience` | CvUnit.cpp:7923-7943 | 21 | `unit_can_give_experience` | ported |
| `CvUnit::giveExperience` | CvUnit.cpp:7945-7980 | 36 | `unit_give_experience` | ported |
| `CvUnit::getStackExperienceToGive` | CvUnit.cpp:7982-7985 | 4 | `unit_get_stack_experience_to_give` | ported |
| `CvUnit::upgradePrice` | CvUnit.cpp:7987-8027 | 41 | `unit_upgrade_price` | ported |
| `CvUnit::upgradeAvailable` | CvUnit.cpp:8030-8064 | 35 | `unit_upgrade_available` | ported |
| `CvUnit::canUpgrade` | CvUnit.cpp:8067-8138 | 72 | `unit_can_upgrade` | ported |
| `CvUnit::isReadyForUpgrade` | CvUnit.cpp:8140-8153 | 14 | `unit_is_ready_for_upgrade` | ported |
| `CvUnit::hasUpgrade` | CvUnit.cpp:8159-8162 | 4 | `unit_has_upgrade` | ported |
| `CvUnit::hasUpgrade` | CvUnit.cpp:8168-8171 | 4 | `unit_has_upgrade_to` | ported |
| `CvUnit::getUpgradeCity` | CvUnit.cpp:8178-8214 | 37 | `unit_get_upgrade_city` | ported |
| `CvUnit::getUpgradeCity` | CvUnit.cpp:8222-8428 | 207 | `unit_get_upgrade_city_for` | ported |
| `CvUnit::upgrade` | CvUnit.cpp:8430-8470 | 41 | `unit_upgrade` | ported |
| `CvUnit::getHandicapType` | CvUnit.cpp:8473-8476 | 4 | `unit_get_handicap_type` | ported |
| `CvUnit::getCivilizationType` | CvUnit.cpp:8479-8482 | 4 | `unit_get_civilization_type` | ported |
| `CvUnit::getSpecialUnitType` | CvUnit.cpp:8494-8497 | 4 | `unit_get_special_unit_type` | ported |
| `CvUnit::getCaptureUnitType` | CvUnit.cpp:8500-8504 | 5 | `unit_get_capture_unit_type` | ported |
| `CvUnit::getUnitCombatType` | CvUnit.cpp:8507-8510 | 4 | `unit_get_unit_combat_type` | ported |
| `CvUnit::getDomainType` | CvUnit.cpp:8513-8516 | 4 | `unit_get_domain_type` | ported |
| `CvUnit::flavorValue` | CvUnit.cpp:8586-8589 | 4 | `unit_flavor_value` | ported |
| `CvUnit::isBarbarian` | CvUnit.cpp:8592-8595 | 4 | `unit_is_barbarian` | ported |
| `CvUnit::isHuman` | CvUnit.cpp:8598-8601 | 4 | `unit_is_human` | ported |
| `CvUnit::airRange` | CvUnit.cpp:8666-8669 | 4 | `unit_air_range` | ported |
| `CvUnit::nukeRange` | CvUnit.cpp:8672-8675 | 4 | `unit_nuke_range` | ported |
| `CvUnit::isAnimal` | CvUnit.cpp:8799-8802 | 4 | `unit_is_animal` | ported |
| `CvUnit::isNoBadGoodies` | CvUnit.cpp:8805-8808 | 4 | `unit_is_no_bad_goodies` | ported |
| `CvUnit::isOnlyDefensive` | CvUnit.cpp:8811-8822 | 12 | `unit_is_only_defensive` | ported |
| `CvUnit::isNoCapture` | CvUnit.cpp:8825-8836 | 12 | `unit_is_no_capture` | ported |
| `CvUnit::isRivalTerritory` | CvUnit.cpp:8839-8842 | 4 | `unit_is_rival_territory` | ported |
| `CvUnit::isMilitaryHappiness` | CvUnit.cpp:8845-8848 | 4 | `unit_is_military_happiness` | ported |
| `CvUnit::isInvestigate` | CvUnit.cpp:8851-8854 | 4 | `unit_is_investigate` | ported |
| `CvUnit::isCounterSpy` | CvUnit.cpp:8857-8860 | 4 | `unit_is_counter_spy` | ported |
| `CvUnit::isSpy` | CvUnit.cpp:8863-8866 | 4 | `unit_is_spy` | ported |
| `CvUnit::isFound` | CvUnit.cpp:8869-8872 | 4 | `unit_is_found` | ported |
| `CvUnit::isGoldenAge` | CvUnit.cpp:8875-8883 | 9 | `unit_is_golden_age` | ported |
| `CvUnit::maxHitPoints` | CvUnit.cpp:8929-8932 | 4 | `unit_max_hit_points` | ported |
| `CvUnit::currHitPoints` | CvUnit.cpp:8935-8938 | 4 | `unit_curr_hit_points` | ported |
| `CvUnit::isHurt` | CvUnit.cpp:8941-8944 | 4 | `unit_is_hurt` | ported |
| `CvUnit::isDead` | CvUnit.cpp:8947-8950 | 4 | `unit_is_dead` | ported |
| `CvUnit::baseCombatStr` | CvUnit.cpp:8958-8971 | 14 | `unit_base_combat_str` | ported |
| `CvUnit::baseCombatStrDefense` | CvUnit.cpp:8979-8987 | 9 | `unit_base_combat_str_defense` | ported |
| `CvUnit::isFortifyable` | CvUnit.cpp:10016-10024 | 9 | `unit_is_fortifyable` | ported |
| `CvUnit::fortifyModifier` | CvUnit.cpp:10027-10044 | 18 | `unit_fortify_modifier` | ported |
| `CvUnit::experienceNeeded` | CvUnit.cpp:10047-10065 | 19 | `unit_experience_needed` | ported |
| `CvUnit::attackXPValue` | CvUnit.cpp:10068-10081 | 14 | `unit_attack_xp_value` | ported |
| `CvUnit::defenseXPValue` | CvUnit.cpp:10084-10097 | 14 | `unit_defense_xp_value` | ported |
| `CvUnit::maxXPValue` | CvUnit.cpp:10100-10129 | 30 | `unit_max_xp_value` | ported |
| `CvUnit::firstStrikes` | CvUnit.cpp:10132-10135 | 4 | `unit_first_strikes` | ported |
| `CvUnit::chanceFirstStrikes` | CvUnit.cpp:10138-10141 | 4 | `unit_chance_first_strikes` | ported |
| `CvUnit::maxFirstStrikes` | CvUnit.cpp:10144-10147 | 4 | `unit_max_first_strikes` | ported |
| `CvUnit::immuneToFirstStrikes` | CvUnit.cpp:10171-10174 | 4 | `unit_immune_to_first_strikes` | ported |
| `CvUnit::noDefensiveBonus` | CvUnit.cpp:10177-10180 | 4 | `unit_no_defensive_bonus` | ported |
| `CvUnit::ignoreBuildingDefense` | CvUnit.cpp:10183-10195 | 13 | `unit_ignore_building_defense` | ported |
| `CvUnit::isNukeImmune` | CvUnit.cpp:10384-10387 | 4 | `unit_is_nuke_immune` | ported |
| `CvUnit::maxInterceptionProbability` | CvUnit.cpp:10390-10393 | 4 | `unit_max_interception_probability` | ported |
| `CvUnit::currInterceptionProbability` | CvUnit.cpp:10396-10406 | 11 | `unit_curr_interception_probability` | ported |
| `CvUnit::evasionProbability` | CvUnit.cpp:10409-10412 | 4 | `unit_evasion_probability` | ported |
| `CvUnit::withdrawalProbability` | CvUnit.cpp:10415-10430 | 16 | `unit_withdrawal_probability` | ported |
| `CvUnit::collateralDamage` | CvUnit.cpp:10433-10436 | 4 | `unit_collateral_damage` | ported |
| `CvUnit::collateralDamageLimit` | CvUnit.cpp:10439-10442 | 4 | `unit_collateral_damage_limit` | ported |
| `CvUnit::collateralDamageMaxUnits` | CvUnit.cpp:10445-10448 | 4 | `unit_collateral_damage_max_units` | ported |
| `CvUnit::cityAttackModifier` | CvUnit.cpp:10451-10454 | 4 | `unit_city_attack_modifier` | ported |
| `CvUnit::cityDefenseModifier` | CvUnit.cpp:10457-10460 | 4 | `unit_city_defense_modifier` | ported |
| `CvUnit::animalCombatModifier` | CvUnit.cpp:10463-10466 | 4 | `unit_animal_combat_modifier` | ported |
| `CvUnit::hillsAttackModifier` | CvUnit.cpp:10469-10472 | 4 | `unit_hills_attack_modifier` | ported |
| `CvUnit::hillsDefenseModifier` | CvUnit.cpp:10475-10478 | 4 | `unit_hills_defense_modifier` | ported |
| `CvUnit::terrainAttackModifier` | CvUnit.cpp:10481-10486 | 6 | `unit_terrain_attack_modifier` | ported |
| `CvUnit::terrainDefenseModifier` | CvUnit.cpp:10489-10494 | 6 | `unit_terrain_defense_modifier` | ported |
| `CvUnit::featureAttackModifier` | CvUnit.cpp:10497-10502 | 6 | `unit_feature_attack_modifier` | ported |
| `CvUnit::featureDefenseModifier` | CvUnit.cpp:10504-10509 | 6 | `unit_feature_defense_modifier` | ported |
| `CvUnit::unitClassAttackModifier` | CvUnit.cpp:10511-10516 | 6 | `unit_unit_class_attack_modifier` | ported |
| `CvUnit::unitClassDefenseModifier` | CvUnit.cpp:10519-10524 | 6 | `unit_unit_class_defense_modifier` | ported |
| `CvUnit::unitCombatModifier` | CvUnit.cpp:10527-10532 | 6 | `unit_unit_combat_modifier` | ported |
| `CvUnit::domainModifier` | CvUnit.cpp:10535-10540 | 6 | `unit_domain_modifier` | ported |
| `CvUnit::bombardRate` | CvUnit.cpp:10543-10546 | 4 | `unit_bombard_rate` | ported |
| `CvUnit::airBombBaseRate` | CvUnit.cpp:10549-10552 | 4 | `unit_air_bomb_base_rate` | ported |
| `CvUnit::airBombCurrRate` | CvUnit.cpp:10555-10558 | 4 | `unit_air_bomb_curr_rate` | ported |
| `CvUnit::setExperience` | CvUnit.cpp:11788-11800 | 13 | `unit_set_experience` | ported |
| `CvUnit::changeExperience` | CvUnit.cpp:11802-11840 | 39 | `unit_change_experience` | ported |
| `CvUnit::setLevel` | CvUnit.cpp:11847-11864 | 18 | `unit_set_level` | ported |
| `CvUnit::changeLevel` | CvUnit.cpp:11866-11869 | 4 | `unit_change_level` | ported |
| `CvUnit::setFortifyTurns` | CvUnit.cpp:11971-11980 | 10 | `unit_set_fortify_turns` | ported |
| `CvUnit::changeFortifyTurns` | CvUnit.cpp:11982-11985 | 4 | `unit_change_fortify_turns` | ported |
| `CvUnit::isBlitz` | CvUnit.cpp:11992-11995 | 4 | `unit_is_blitz` | ported |
| `CvUnit::changeBlitzCount` | CvUnit.cpp:11997-12001 | 5 | `unit_change_blitz_count` | ported |
| `CvUnit::isAmphib` | CvUnit.cpp:12008-12011 | 4 | `unit_is_amphib` | ported |
| `CvUnit::changeAmphibCount` | CvUnit.cpp:12013-12017 | 5 | `unit_change_amphib_count` | ported |
| `CvUnit::isRiver` | CvUnit.cpp:12024-12027 | 4 | `unit_is_river` | ported |
| `CvUnit::changeRiverCount` | CvUnit.cpp:12029-12033 | 5 | `unit_change_river_count` | ported |
| `CvUnit::isEnemyRoute` | CvUnit.cpp:12040-12043 | 4 | `unit_is_enemy_route` | ported |
| `CvUnit::changeEnemyRouteCount` | CvUnit.cpp:12045-12049 | 5 | `unit_change_enemy_route_count` | ported |
| `CvUnit::isAlwaysHeal` | CvUnit.cpp:12056-12059 | 4 | `unit_is_always_heal` | ported |
| `CvUnit::changeAlwaysHealCount` | CvUnit.cpp:12061-12065 | 5 | `unit_change_always_heal_count` | ported |
| `CvUnit::isHillsDoubleMove` | CvUnit.cpp:12072-12075 | 4 | `unit_is_hills_double_move` | ported |
| `CvUnit::changeHillsDoubleMoveCount` | CvUnit.cpp:12077-12081 | 5 | `unit_change_hills_double_move_count` | ported |
| `CvUnit::changeImmuneToFirstStrikesCount` | CvUnit.cpp:12088-12092 | 5 | `unit_change_immune_to_first_strikes_count` | ported |
| `CvUnit::changeExtraMoves` | CvUnit.cpp:12119-12123 | 5 | `unit_change_extra_moves` | ported |
| `CvUnit::changeExtraMoveDiscount` | CvUnit.cpp:12132-12136 | 5 | `unit_change_extra_move_discount` | ported |
| `CvUnit::changeExtraAirRange` | CvUnit.cpp:12143-12146 | 4 | `unit_change_extra_air_range` | ported |
| `CvUnit::changeExtraIntercept` | CvUnit.cpp:12153-12156 | 4 | `unit_change_extra_intercept` | ported |
| `CvUnit::changeExtraEvasion` | CvUnit.cpp:12163-12166 | 4 | `unit_change_extra_evasion` | ported |
| `CvUnit::changeExtraFirstStrikes` | CvUnit.cpp:12173-12177 | 5 | `unit_change_extra_first_strikes` | ported |
| `CvUnit::changeExtraChanceFirstStrikes` | CvUnit.cpp:12184-12188 | 5 | `unit_change_extra_chance_first_strikes` | ported |
| `CvUnit::changeExtraWithdrawal` | CvUnit.cpp:12197-12201 | 5 | `unit_change_extra_withdrawal` | ported |
| `CvUnit::changeExtraCollateralDamage` | CvUnit.cpp:12208-12212 | 5 | `unit_change_extra_collateral_damage` | ported |
| `CvUnit::changeExtraBombardRate` | CvUnit.cpp:12219-12223 | 5 | `unit_change_extra_bombard_rate` | ported |
| `CvUnit::changeExtraEnemyHeal` | CvUnit.cpp:12230-12234 | 5 | `unit_change_extra_enemy_heal` | ported |
| `CvUnit::changeExtraNeutralHeal` | CvUnit.cpp:12241-12245 | 5 | `unit_change_extra_neutral_heal` | ported |
| `CvUnit::changeExtraFriendlyHeal` | CvUnit.cpp:12253-12257 | 5 | `unit_change_extra_friendly_heal` | ported |
| `CvUnit::changeSameTileHeal` | CvUnit.cpp:12264-12268 | 5 | `unit_change_same_tile_heal` | ported |
| `CvUnit::changeAdjacentTileHeal` | CvUnit.cpp:12275-12279 | 5 | `unit_change_adjacent_tile_heal` | ported |
| `CvUnit::getExtraCombatPercent` | CvUnit.cpp:12281-12294 | 14 | `unit_get_extra_combat_percent` | ported |
| `CvUnit::changeExtraCombatPercent` | CvUnit.cpp:12296-12304 | 9 | `unit_change_extra_combat_percent` | ported |
| `CvUnit::changeExtraCityAttackPercent` | CvUnit.cpp:12311-12319 | 9 | `unit_change_extra_city_attack_percent` | ported |
| `CvUnit::changeExtraCityDefensePercent` | CvUnit.cpp:12326-12334 | 9 | `unit_change_extra_city_defense_percent` | ported |
| `CvUnit::changeExtraHillsAttackPercent` | CvUnit.cpp:12341-12349 | 9 | `unit_change_extra_hills_attack_percent` | ported |
| `CvUnit::changeExtraHillsDefensePercent` | CvUnit.cpp:12356-12364 | 9 | `unit_change_extra_hills_defense_percent` | ported |
| `CvUnit::changeRevoltProtection` | CvUnit.cpp:12371-12379 | 9 | `unit_change_revolt_protection` | ported |
| `CvUnit::changeCollateralDamageProtection` | CvUnit.cpp:12386-12394 | 9 | `unit_change_collateral_damage_protection` | ported |
| `CvUnit::changePillageChange` | CvUnit.cpp:12401-12409 | 9 | `unit_change_pillage_change` | ported |
| `CvUnit::changeUpgradeDiscount` | CvUnit.cpp:12416-12424 | 9 | `unit_change_upgrade_discount` | ported |
| `CvUnit::changeExperiencePercent` | CvUnit.cpp:12431-12439 | 9 | `unit_change_experience_percent` | ported |
| `CvUnit::changeKamikazePercent` | CvUnit.cpp:12446-12454 | 9 | `unit_change_kamikaze_percent` | ported |
| `CvUnit::setPromotionReady` | CvUnit.cpp:12591-12621 | 31 | `unit_set_promotion_ready` | ported |
| `CvUnit::testPromotionReady` | CvUnit.cpp:12624-12627 | 4 | `unit_test_promotion_ready` | ported |
| `CvUnit::getUnitClassType` | CvUnit.cpp:12813-12816 | 4 | `unit_get_unit_class_type` | ported |
| `CvUnit::changeExtraDomainModifier` | CvUnit.cpp:12989-12994 | 6 | `unit_change_extra_domain_modifier` | ported |
| `CvUnit::changeExtraTerrainAttackPercent` | CvUnit.cpp:13114-13125 | 12 | `unit_change_extra_terrain_attack_percent` | ported |
| `CvUnit::changeExtraTerrainDefensePercent` | CvUnit.cpp:13135-13146 | 12 | `unit_change_extra_terrain_defense_percent` | ported |
| `CvUnit::changeExtraFeatureAttackPercent` | CvUnit.cpp:13156-13167 | 12 | `unit_change_extra_feature_attack_percent` | ported |
| `CvUnit::changeExtraFeatureDefensePercent` | CvUnit.cpp:13177-13188 | 12 | `unit_change_extra_feature_defense_percent` | ported |
| `CvUnit::changeExtraUnitCombatModifier` | CvUnit.cpp:13198-13203 | 6 | `unit_change_extra_unit_combat_modifier` | ported |
| `CvUnit::canAcquirePromotion` | CvUnit.cpp:13206-13347 | 142 | `unit_can_acquire_promotion` | ported |
| `CvUnit::isPromotionValid` | CvUnit.cpp:13349-13383 | 35 | `unit_is_promotion_valid` | ported |
| `CvUnit::canAcquirePromotionAny` | CvUnit.cpp:13386-13399 | 14 | `unit_can_acquire_promotion_any` | ported |
| `CvUnit::setHasPromotion` | CvUnit.cpp:13410-13620 | 211 | `unit_set_has_promotion` | ported |
| `CvUnit::isSuicide` | CvUnit.cpp:18041-18044 | 4 | `unit_is_suicide` | ported |
| `CvUnit::getDropRange` | CvUnit.cpp:18046-18049 | 4 | `unit_get_drop_range` | ported |

## unit/actions.rs -- 46 C++ functions, 1940 C++ lines

CvUnit commands and actions: `canDoCommand`/`doCommand`, automation, scrap/gift, hold/sleep/fortify/patrol/heal (and heal rates), airlift, nuke, recon, paradrop, air bomb, bombard, pillage, plunder and blockades.

| C++ function | C++ lines | count | Rust method | status |
|---|---|---:|---|---|
| `CvUnit::canDoCommand` | CvUnit.cpp:2649-2785 | 137 | `unit_can_do_command` | ported |
| `CvUnit::doCommand` | CvUnit.cpp:2788-2905 | 118 | `unit_do_command` | ported |
| `CvUnit::canAutomate` | CvUnit.cpp:3730-3820 | 91 | `unit_can_automate` | ported |
| `CvUnit::automate` | CvUnit.cpp:3920-3928 | 9 | `unit_automate` | ported |
| `CvUnit::canScrap` | CvUnit.cpp:3931-3950 | 20 | `unit_can_scrap` | ported |
| `CvUnit::scrap` | CvUnit.cpp:3953-3974 | 22 | `unit_scrap` | ported |
| `CvUnit::canGift` | CvUnit.cpp:3977-4054 | 78 | `unit_can_gift` | ported |
| `CvUnit::gift` | CvUnit.cpp:4057-4091 | 35 | `unit_gift` | ported |
| `CvUnit::canHold` | CvUnit.cpp:4387-4390 | 4 | `unit_can_hold` | ported |
| `CvUnit::canSleep` | CvUnit.cpp:4393-4406 | 14 | `unit_can_sleep` | ported |
| `CvUnit::canFortify` | CvUnit.cpp:4409-4422 | 14 | `unit_can_fortify` | ported |
| `CvUnit::canAirPatrol` | CvUnit.cpp:4425-4443 | 19 | `unit_can_air_patrol` | ported |
| `CvUnit::canSeaPatrol` | CvUnit.cpp:4446-4469 | 24 | `unit_can_sea_patrol` | ported |
| `CvUnit::canHeal` | CvUnit.cpp:4501-4519 | 19 | `unit_can_heal` | ported |
| `CvUnit::canSentry` | CvUnit.cpp:4522-4535 | 14 | `unit_can_sentry` | ported |
| `CvUnit::healRate` | CvUnit.cpp:4538-4673 | 136 | `unit_heal_rate` | ported |
| `CvUnit::healTurns` | CvUnit.cpp:4676-4703 | 28 | `unit_heal_turns` | ported |
| `CvUnit::doHeal` | CvUnit.cpp:4706-4709 | 4 | `unit_do_heal` | ported |
| `CvUnit::canAirlift` | CvUnit.cpp:4712-4744 | 33 | `unit_can_airlift` | ported |
| `CvUnit::canAirliftAt` | CvUnit.cpp:4747-4782 | 36 | `unit_can_airlift_at` | ported |
| `CvUnit::airlift` | CvUnit.cpp:4785-4815 | 31 | `unit_airlift` | ported |
| `CvUnit::isNukeVictim` | CvUnit.cpp:4818-4855 | 38 | `unit_is_nuke_victim` | ported |
| `CvUnit::canNuke` | CvUnit.cpp:4858-4866 | 9 | `unit_can_nuke` | ported |
| `CvUnit::canNukeAt` | CvUnit.cpp:4869-4904 | 36 | `unit_can_nuke_at` | ported |
| `CvUnit::nuke` | CvUnit.cpp:4907-5084 | 178 | `unit_nuke` | ported |
| `CvUnit::canRecon` | CvUnit.cpp:5087-5105 | 19 | `unit_can_recon` | ported |
| `CvUnit::canReconAt` | CvUnit.cpp:5109-5123 | 15 | `unit_can_recon_at` | ported |
| `CvUnit::recon` | CvUnit.cpp:5126-5155 | 30 | `unit_recon` | ported |
| `CvUnit::canParadrop` | CvUnit.cpp:5158-5176 | 19 | `unit_can_paradrop` | ported |
| `CvUnit::canParadropAt` | CvUnit.cpp:5180-5222 | 43 | `unit_can_paradrop_at` | ported |
| `CvUnit::paradrop` | CvUnit.cpp:5225-5260 | 36 | `unit_paradrop` | ported |
| `CvUnit::canAirBomb` | CvUnit.cpp:5263-5281 | 19 | `unit_can_air_bomb` | ported |
| `CvUnit::canAirBombAt` | CvUnit.cpp:5284-5337 | 54 | `unit_can_air_bomb_at` | ported |
| `CvUnit::airBomb` | CvUnit.cpp:5340-5431 | 92 | `unit_air_bomb` | ported |
| `CvUnit::bombardTarget` | CvUnit.cpp:5434-5470 | 37 | `unit_bombard_target` | ported |
| `CvUnit::canBombard` | CvUnit.cpp:5473-5496 | 24 | `unit_can_bombard` | ported |
| `CvUnit::bombard` | CvUnit.cpp:5499-5560 | 62 | `unit_bombard` | ported |
| `CvUnit::canPillage` | CvUnit.cpp:5563-5620 | 58 | `unit_can_pillage` | ported |
| `CvUnit::pillage` | CvUnit.cpp:5623-5750 | 128 | `unit_pillage` | ported |
| `CvUnit::canPlunder` | CvUnit.cpp:5753-5789 | 37 | `unit_can_plunder` | ported |
| `CvUnit::plunder` | CvUnit.cpp:5792-5806 | 15 | `unit_plunder` | ported |
| `CvUnit::updatePlunder` | CvUnit.cpp:5809-5854 | 46 | `unit_update_plunder` | ported |
| `CvUnit::isAutomated` | CvUnit.cpp:10004-10007 | 4 | `unit_is_automated` | ported |
| `CvUnit::isWaiting` | CvUnit.cpp:10010-10013 | 4 | `unit_is_waiting` | ported |
| `CvUnit::setBlockading` | CvUnit.cpp:12677-12685 | 9 | `unit_set_blockading` | ported |
| `CvUnit::collectBlockadeGold` | CvUnit.cpp:12687-12728 | 42 | `unit_collect_blockade_gold` | ported |

## unit/missions.rs -- 52 C++ functions, 1883 C++ lines

CvUnit missions: spy actions (sabotage, destroy, steal plans, infiltrate, espionage), found a city, spread religion/corporation, great people (join, construct, discover, hurry, trade, great work, golden age) and worker builds.

| C++ function | C++ lines | count | Rust method | status |
|---|---|---:|---|---|
| `CvUnit::sabotageCost` | CvUnit.cpp:5857-5860 | 4 | `unit_sabotage_cost` | ported |
| `CvUnit::sabotageProb` | CvUnit.cpp:5864-5908 | 45 | `unit_sabotage_prob` | ported |
| `CvUnit::canSabotage` | CvUnit.cpp:5911-5942 | 32 | `unit_can_sabotage` | ported |
| `CvUnit::sabotage` | CvUnit.cpp:5945-6016 | 72 | `unit_sabotage` | ported |
| `CvUnit::destroyCost` | CvUnit.cpp:6019-6047 | 29 | `unit_destroy_cost` | ported |
| `CvUnit::destroyProb` | CvUnit.cpp:6050-6097 | 48 | `unit_destroy_prob` | ported |
| `CvUnit::canDestroy` | CvUnit.cpp:6100-6135 | 36 | `unit_can_destroy` | ported |
| `CvUnit::destroy` | CvUnit.cpp:6138-6195 | 58 | `unit_destroy` | ported |
| `CvUnit::stealPlansCost` | CvUnit.cpp:6198-6210 | 13 | `unit_steal_plans_cost` | ported |
| `CvUnit::stealPlansProb` | CvUnit.cpp:6214-6259 | 46 | `unit_steal_plans_prob` | ported |
| `CvUnit::canStealPlans` | CvUnit.cpp:6262-6292 | 31 | `unit_can_steal_plans` | ported |
| `CvUnit::stealPlans` | CvUnit.cpp:6295-6352 | 58 | `unit_steal_plans` | ported |
| `CvUnit::canFound` | CvUnit.cpp:6355-6379 | 25 | `unit_can_found` | ported |
| `CvUnit::found` | CvUnit.cpp:6382-6441 | 60 | `unit_found` | ported |
| `CvUnit::canSpread` | CvUnit.cpp:6444-6542 | 99 | `unit_can_spread` | ported |
| `CvUnit::spread` | CvUnit.cpp:6545-6609 | 65 | `unit_spread` | ported |
| `CvUnit::canSpreadCorporation` | CvUnit.cpp:6612-6698 | 87 | `unit_can_spread_corporation` | ported |
| `CvUnit::spreadCorporationCost` | CvUnit.cpp:6700-6730 | 31 | `unit_spread_corporation_cost` | ported |
| `CvUnit::spreadCorporation` | CvUnit.cpp:6732-6775 | 44 | `unit_spread_corporation` | ported |
| `CvUnit::canJoin` | CvUnit.cpp:6778-6822 | 45 | `unit_can_join` | ported |
| `CvUnit::join` | CvUnit.cpp:6825-6849 | 25 | `unit_join` | ported |
| `CvUnit::canConstruct` | CvUnit.cpp:6852-6904 | 53 | `unit_can_construct` | ported |
| `CvUnit::construct` | CvUnit.cpp:6907-6933 | 27 | `unit_construct` | ported |
| `CvUnit::getDiscoveryTech` | CvUnit.cpp:6936-6939 | 4 | `unit_get_discovery_tech` | ported |
| `CvUnit::getDiscoverResearch` | CvUnit.cpp:6942-6964 | 23 | `unit_get_discover_research` | ported |
| `CvUnit::canDiscover` | CvUnit.cpp:6967-6989 | 23 | `unit_can_discover` | ported |
| `CvUnit::discover` | CvUnit.cpp:6992-7014 | 23 | `unit_discover` | ported |
| `CvUnit::getMaxHurryProduction` | CvUnit.cpp:7017-7034 | 18 | `unit_get_max_hurry_production` | ported |
| `CvUnit::getHurryProduction` | CvUnit.cpp:7037-7054 | 18 | `unit_get_hurry_production` | ported |
| `CvUnit::canHurry` | CvUnit.cpp:7057-7092 | 36 | `unit_can_hurry` | ported |
| `CvUnit::hurry` | CvUnit.cpp:7095-7119 | 25 | `unit_hurry` | ported |
| `CvUnit::getTradeGold` | CvUnit.cpp:7122-7149 | 28 | `unit_get_trade_gold` | ported |
| `CvUnit::canTrade` | CvUnit.cpp:7152-7185 | 34 | `unit_can_trade` | ported |
| `CvUnit::trade` | CvUnit.cpp:7188-7205 | 18 | `unit_trade` | ported |
| `CvUnit::getGreatWorkCulture` | CvUnit.cpp:7208-7225 | 18 | `unit_get_great_work_culture` | ported |
| `CvUnit::canGreatWork` | CvUnit.cpp:7228-7253 | 26 | `unit_can_great_work` | ported |
| `CvUnit::greatWork` | CvUnit.cpp:7256-7292 | 37 | `unit_great_work` | ported |
| `CvUnit::getEspionagePoints` | CvUnit.cpp:7295-7305 | 11 | `unit_get_espionage_points` | ported |
| `CvUnit::canInfiltrate` | CvUnit.cpp:7307-7339 | 33 | `unit_can_infiltrate` | ported |
| `CvUnit::infiltrate` | CvUnit.cpp:7342-7361 | 20 | `unit_infiltrate` | ported |
| `CvUnit::canEspionage` | CvUnit.cpp:7364-7423 | 60 | `unit_can_espionage` | ported |
| `CvUnit::espionage` | CvUnit.cpp:7425-7488 | 64 | `unit_espionage` | ported |
| `CvUnit::testSpyIntercepted` | CvUnit.cpp:7490-7556 | 67 | `unit_test_spy_intercepted` | ported |
| `CvUnit::getSpyInterceptPercent` | CvUnit.cpp:7558-7585 | 28 | `unit_get_spy_intercept_percent` | ported |
| `CvUnit::isIntruding` | CvUnit.cpp:7587-7602 | 16 | `unit_is_intruding` | ported |
| `CvUnit::canGoldenAge` | CvUnit.cpp:7604-7620 | 17 | `unit_can_golden_age` | ported |
| `CvUnit::goldenAge` | CvUnit.cpp:7623-7643 | 21 | `unit_golden_age` | ported |
| `CvUnit::canBuild` | CvUnit.cpp:7646-7679 | 34 | `unit_can_build` | ported |
| `CvUnit::build` | CvUnit.cpp:7682-7714 | 33 | `unit_build` | ported |
| `CvUnit::canBuildRoute` | CvUnit.cpp:8679-8698 | 20 | `unit_can_build_route` | ported |
| `CvUnit::getBuildType` | CvUnit.cpp:8700-8765 | 66 | `unit_get_build_type` | ported |
| `CvUnit::workRate` | CvUnit.cpp:8768-8796 | 29 | `unit_work_rate` | ported |

## unit/ffh.rs -- 98 C++ functions, 2406 C++ lines

FfH2 additions to CvUnit: spells (`canCast`, `cast`, ...), summons, damage types and resistances, immortality, weapons, bonus affinity, betrayal, terraformers and the FfH status counters (`isFlying`, `changeFear`, ...).

| C++ function | C++ lines | count | Rust method | status |
|---|---|---:|---|---|
| `CvUnit::canSpellAutomate` | CvUnit.cpp:3827-3915 | 89 | `unit_can_spell_automate` | ported |
| `CvUnit::canCast` | CvUnit.cpp:13675-14101 | 427 | `unit_can_cast` | ported |
| `CvUnit::canCreateUnit` | CvUnit.cpp:14103-14140 | 38 | `unit_can_create_unit` | ported |
| `CvUnit::canAddPromotion` | CvUnit.cpp:14142-14234 | 93 | `unit_can_add_promotion` | ported |
| `CvUnit::canCreateBuilding` | CvUnit.cpp:14236-14247 | 12 | `unit_can_create_building` | ported |
| `CvUnit::canCreateFeature` | CvUnit.cpp:14249-14271 | 23 | `unit_can_create_feature` | ported |
| `CvUnit::canCreateImprovement` | CvUnit.cpp:14273-14288 | 16 | `unit_can_create_improvement` | ported |
| `CvUnit::canDispel` | CvUnit.cpp:14290-14330 | 41 | `unit_can_dispel` | ported |
| `CvUnit::canImmobile` | CvUnit.cpp:14332-14359 | 28 | `unit_can_immobile` | ported |
| `CvUnit::canPush` | CvUnit.cpp:14361-14391 | 31 | `unit_can_push` | ported |
| `CvUnit::canRemovePromotion` | CvUnit.cpp:14393-14482 | 90 | `unit_can_remove_promotion` | ported |
| `CvUnit::canSpreadReligion` | CvUnit.cpp:14484-14501 | 18 | `unit_can_spread_religion` | ported |
| `CvUnit::cast` | CvUnit.cpp:14503-14675 | 173 | `unit_cast` | ported |
| `CvUnit::castAddPromotion` | CvUnit.cpp:14677-14793 | 117 | `unit_cast_add_promotion` | ported |
| `CvUnit::castDamage` | CvUnit.cpp:14795-14838 | 44 | `unit_cast_damage` | ported |
| `CvUnit::castDispel` | CvUnit.cpp:14840-14893 | 54 | `unit_cast_dispel` | ported |
| `CvUnit::castImmobile` | CvUnit.cpp:14895-14940 | 46 | `unit_cast_immobile` | ported |
| `CvUnit::castPush` | CvUnit.cpp:14942-14998 | 57 | `unit_cast_push` | ported |
| `CvUnit::castRemovePromotion` | CvUnit.cpp:15000-15080 | 81 | `unit_cast_remove_promotion` | ported |
| `CvUnit::castConvertUnit` | CvUnit.cpp:15082-15088 | 7 | `unit_cast_convert_unit` | ported |
| `CvUnit::castCreateUnit` | CvUnit.cpp:15090-15175 | 86 | `unit_cast_create_unit` | ported |
| `CvUnit::isImmuneToSpell` | CvUnit.cpp:15197-15245 | 49 | `unit_is_immune_to_spell` | ported |
| `CvUnit::changeDuration` | CvUnit.cpp:15267-15270 | 4 | `unit_change_duration` | ported |
| `CvUnit::isAlive` | CvUnit.cpp:15282-15285 | 4 | `unit_is_alive` | ported |
| `CvUnit::changeAlive` | CvUnit.cpp:15287-15293 | 7 | `unit_change_alive` | ported |
| `CvUnit::isAIControl` | CvUnit.cpp:15295-15298 | 4 | `unit_is_ai_control` | ported |
| `CvUnit::changeAIControl` | CvUnit.cpp:15300-15306 | 7 | `unit_change_ai_control` | ported |
| `CvUnit::isBoarding` | CvUnit.cpp:15308-15311 | 4 | `unit_is_boarding` | ported |
| `CvUnit::changeBoarding` | CvUnit.cpp:15313-15319 | 7 | `unit_change_boarding` | ported |
| `CvUnit::changeDefensiveStrikeChance` | CvUnit.cpp:15326-15332 | 7 | `unit_change_defensive_strike_chance` | ported |
| `CvUnit::changeDefensiveStrikeDamage` | CvUnit.cpp:15339-15345 | 7 | `unit_change_defensive_strike_damage` | ported |
| `CvUnit::isDoubleFortifyBonus` | CvUnit.cpp:15347-15350 | 4 | `unit_is_double_fortify_bonus` | ported |
| `CvUnit::changeDoubleFortifyBonus` | CvUnit.cpp:15352-15358 | 7 | `unit_change_double_fortify_bonus` | ported |
| `CvUnit::isFear` | CvUnit.cpp:15360-15363 | 4 | `unit_is_fear` | ported |
| `CvUnit::changeFear` | CvUnit.cpp:15365-15371 | 7 | `unit_change_fear` | ported |
| `CvUnit::isFlying` | CvUnit.cpp:15373-15376 | 4 | `unit_is_flying` | ported |
| `CvUnit::changeFlying` | CvUnit.cpp:15378-15384 | 7 | `unit_change_flying` | ported |
| `CvUnit::isHeld` | CvUnit.cpp:15386-15389 | 4 | `unit_is_held` | ported |
| `CvUnit::changeHeld` | CvUnit.cpp:15391-15397 | 7 | `unit_change_held` | ported |
| `CvUnit::isHiddenNationality` | CvUnit.cpp:15399-15409 | 11 | `unit_is_hidden_nationality` | ported |
| `CvUnit::changeHiddenNationality` | CvUnit.cpp:15411-15427 | 17 | `unit_change_hidden_nationality` | ported |
| `CvUnit::changeIgnoreBuildingDefense` | CvUnit.cpp:15429-15435 | 7 | `unit_change_ignore_building_defense` | ported |
| `CvUnit::isImmortal` | CvUnit.cpp:15437-15440 | 4 | `unit_is_immortal` | ported |
| `CvUnit::changeImmortal` | CvUnit.cpp:15442-15452 | 11 | `unit_change_immortal` | ported |
| `CvUnit::isImmuneToCapture` | CvUnit.cpp:15454-15457 | 4 | `unit_is_immune_to_capture` | ported |
| `CvUnit::changeImmuneToCapture` | CvUnit.cpp:15459-15465 | 7 | `unit_change_immune_to_capture` | ported |
| `CvUnit::isImmuneToDefensiveStrike` | CvUnit.cpp:15467-15470 | 4 | `unit_is_immune_to_defensive_strike` | ported |
| `CvUnit::changeImmuneToDefensiveStrike` | CvUnit.cpp:15472-15478 | 7 | `unit_change_immune_to_defensive_strike` | ported |
| `CvUnit::isImmuneToFear` | CvUnit.cpp:15480-15487 | 8 | `unit_is_immune_to_fear` | ported |
| `CvUnit::changeImmuneToFear` | CvUnit.cpp:15489-15495 | 7 | `unit_change_immune_to_fear` | ported |
| `CvUnit::isImmuneToMagic` | CvUnit.cpp:15497-15500 | 4 | `unit_is_immune_to_magic` | ported |
| `CvUnit::changeImmuneToMagic` | CvUnit.cpp:15502-15508 | 7 | `unit_change_immune_to_magic` | ported |
| `CvUnit::isInvisibleFromPromotion` | CvUnit.cpp:15510-15513 | 4 | `unit_is_invisible_from_promotion` | ported |
| `CvUnit::changeInvisibleFromPromotion` | CvUnit.cpp:15515-15528 | 14 | `unit_change_invisible_from_promotion` | ported |
| `CvUnit::isSeeInvisible` | CvUnit.cpp:15530-15533 | 4 | `unit_is_see_invisible` | ported |
| `CvUnit::changeSeeInvisible` | CvUnit.cpp:15535-15541 | 7 | `unit_change_see_invisible` | ported |
| `CvUnit::changeOnlyDefensive` | CvUnit.cpp:15543-15549 | 7 | `unit_change_only_defensive` | ported |
| `CvUnit::isTargetWeakestUnit` | CvUnit.cpp:15551-15554 | 4 | `unit_is_target_weakest_unit` | ported |
| `CvUnit::changeTargetWeakestUnit` | CvUnit.cpp:15556-15562 | 7 | `unit_change_target_weakest_unit` | ported |
| `CvUnit::isTargetWeakestUnitCounter` | CvUnit.cpp:15564-15567 | 4 | `unit_is_target_weakest_unit_counter` | ported |
| `CvUnit::changeTargetWeakestUnitCounter` | CvUnit.cpp:15569-15575 | 7 | `unit_change_target_weakest_unit_counter` | ported |
| `CvUnit::isTwincast` | CvUnit.cpp:15577-15580 | 4 | `unit_is_twincast` | ported |
| `CvUnit::changeTwincast` | CvUnit.cpp:15582-15588 | 7 | `unit_change_twincast` | ported |
| `CvUnit::isWaterWalking` | CvUnit.cpp:15590-15593 | 4 | `unit_is_water_walking` | ported |
| `CvUnit::changeWaterWalking` | CvUnit.cpp:15595-15601 | 7 | `unit_change_water_walking` | ported |
| `CvUnit::changeBetterDefenderThanPercent` | CvUnit.cpp:15608-15614 | 7 | `unit_change_better_defender_than_percent` | ported |
| `CvUnit::changeCombatHealPercent` | CvUnit.cpp:15621-15627 | 7 | `unit_change_combat_heal_percent` | ported |
| `CvUnit::calcCombatLimit` | CvUnit.cpp:15629-15648 | 20 | `unit_calc_combat_limit` | ported |
| `CvUnit::changeCombatPercentInBorders` | CvUnit.cpp:15655-15662 | 8 | `unit_change_combat_percent_in_borders` | ported |
| `CvUnit::changeCombatPercentGlobalCounter` | CvUnit.cpp:15669-15676 | 8 | `unit_change_combat_percent_global_counter` | ported |
| `CvUnit::changeFreePromotionPick` | CvUnit.cpp:15683-15689 | 7 | `unit_change_free_promotion_pick` | ported |
| `CvUnit::changeGoldFromCombat` | CvUnit.cpp:15696-15702 | 7 | `unit_change_gold_from_combat` | ported |
| `CvUnit::mutate` | CvUnit.cpp:15709-15722 | 14 | `unit_mutate` | ported |
| `CvUnit::getWithdrawlProbDefensive` | CvUnit.cpp:15724-15732 | 9 | `unit_get_withdrawl_prob_defensive` | ported |
| `CvUnit::setRace` | CvUnit.cpp:15744-15751 | 8 | `unit_set_race` | ported |
| `CvUnit::changeResist` | CvUnit.cpp:15773-15776 | 4 | `unit_change_resist` | ported |
| `CvUnit::changeResistModify` | CvUnit.cpp:15788-15791 | 4 | `unit_change_resist_modify` | ported |
| `CvUnit::getSpellCasterXP` | CvUnit.cpp:15803-15810 | 8 | `unit_get_spell_caster_xp` | ported |
| `CvUnit::changeSpellCasterXP` | CvUnit.cpp:15812-15818 | 7 | `unit_change_spell_caster_xp` | ported |
| `CvUnit::changeSpellDamageModify` | CvUnit.cpp:15825-15831 | 7 | `unit_change_spell_damage_modify` | ported |
| `CvUnit::changeWorkRateModify` | CvUnit.cpp:15848-15854 | 7 | `unit_change_work_rate_modify` | ported |
| `CvUnit::isResisted` | CvUnit.cpp:15856-15866 | 11 | `unit_is_resisted` | ported |
| `CvUnit::getResistChance` | CvUnit.cpp:15868-15895 | 28 | `unit_get_resist_chance` | ported |
| `CvUnit::changeBaseCombatStr` | CvUnit.cpp:15897-15900 | 4 | `unit_change_base_combat_str` | ported |
| `CvUnit::changeBaseCombatStrDefense` | CvUnit.cpp:15902-15905 | 4 | `unit_change_base_combat_str_defense` | ported |
| `CvUnit::getExtraSpellMove` | CvUnit.cpp:16090-16104 | 15 | `unit_get_extra_spell_move` | ported |
| `CvUnit::doDamage` | CvUnit.cpp:16106-16204 | 99 | `unit_do_damage` | ported |
| `CvUnit::doEscape` | CvUnit.cpp:16261-16267 | 7 | `unit_do_escape` | ported |
| `CvUnit::doImmortalRebirth` | CvUnit.cpp:16269-16304 | 36 | `unit_do_immortal_rebirth` | ported |
| `CvUnit::setWeapons` | CvUnit.cpp:16562-16617 | 56 | `unit_set_weapons` | ported |
| `CvUnit::changeBonusAffinity` | CvUnit.cpp:16619-16626 | 8 | `unit_change_bonus_affinity` | ported |
| `CvUnit::updateBonusAffinity` | CvUnit.cpp:16633-16648 | 16 | `unit_update_bonus_affinity` | ported |
| `CvUnit::changeDamageTypeCombat` | CvUnit.cpp:16650-16657 | 8 | `unit_change_damage_type_combat` | ported |
| `CvUnit::getDamageTypeResist` | CvUnit.cpp:16669-16681 | 13 | `unit_get_damage_type_resist` | ported |
| `CvUnit::changeDamageTypeResist` | CvUnit.cpp:16683-16689 | 7 | `unit_change_damage_type_resist` | ported |
| `CvUnit::countUnitsWithinRange` | CvUnit.cpp:16691-16726 | 36 | `unit_count_units_within_range` | ported |
| `CvUnit::betray` | CvUnit.cpp:16765-16781 | 17 | `unit_betray` | ported |
| `CvUnit::updateTerraformer` | CvUnit.cpp:16783-16798 | 16 | `unit_update_terraformer` | ported |

## Trivial accessors -> `Unit` fields

| C++ function | C++ lines | Rust |
|---|---|---|
| `CvUnit::getID` | CvUnit.cpp:10693-10696 | `unit.id` |
| `CvUnit::getGroupID` | CvUnit.cpp:10718-10721 | `unit.group` |
| `CvUnit::getGroup` | CvUnit.cpp:10736-10739 | `unit.group` (`self.groups[g]`) |
| `CvUnit::getX` | CvUnit.cpp:10983-10986 | `unit.x` |
| `CvUnit::getY` | CvUnit.cpp:10989-10992 | `unit.y` |
| `CvUnit::getLastMoveTurn` | CvUnit.cpp:11618-11621 | `unit.last_move_turn` |
| `CvUnit::setLastMoveTurn` | CvUnit.cpp:11624-11628 | `unit.last_move_turn` |
| `CvUnit::getGameTurnCreated` | CvUnit.cpp:11668-11671 | `unit.game_turn_created` |
| `CvUnit::setGameTurnCreated` | CvUnit.cpp:11674-11678 | `unit.game_turn_created` |
| `CvUnit::getDamage` | CvUnit.cpp:11681-11684 | `unit.damage` |
| `CvUnit::getMoves` | CvUnit.cpp:11730-11733 | `unit.moves` |
| `CvUnit::setMoves` | CvUnit.cpp:11736-11768 | `unit.moves` (rest of setMoves is interface updates) |
| `CvUnit::getExperience` | CvUnit.cpp:11783-11786 | `unit.experience` |
| `CvUnit::getLevel` | CvUnit.cpp:11842-11845 | `unit.level` |
| `CvUnit::getCargo` | CvUnit.cpp:11871-11874 | `unit.cargo` |
| `CvUnit::isAirCombat` | CvUnit.cpp:11929-11932 | `unit.air_combat` |
| `CvUnit::getCombatTimer` | CvUnit.cpp:11934-11937 | `unit.combat_timer` |
| `CvUnit::setCombatTimer` | CvUnit.cpp:11939-11943 | `unit.combat_timer` |
| `CvUnit::getCombatFirstStrikes` | CvUnit.cpp:11950-11953 | `unit.combat_first_strikes` |
| `CvUnit::setCombatFirstStrikes` | CvUnit.cpp:11955-11959 | `unit.combat_first_strikes` |
| `CvUnit::getFortifyTurns` | CvUnit.cpp:11966-11969 | `unit.fortify_turns` |
| `CvUnit::getBlitzCount` | CvUnit.cpp:11987-11990 | `unit.blitz_count` |
| `CvUnit::getAmphibCount` | CvUnit.cpp:12003-12006 | `unit.amphib_count` |
| `CvUnit::getRiverCount` | CvUnit.cpp:12019-12022 | `unit.river_count` |
| `CvUnit::getEnemyRouteCount` | CvUnit.cpp:12035-12038 | `unit.enemy_route_count` |
| `CvUnit::getAlwaysHealCount` | CvUnit.cpp:12051-12054 | `unit.always_heal_count` |
| `CvUnit::getHillsDoubleMoveCount` | CvUnit.cpp:12067-12070 | `unit.hills_double_move_count` |
| `CvUnit::getImmuneToFirstStrikesCount` | CvUnit.cpp:12083-12086 | `unit.immune_to_first_strikes_count` |
| `CvUnit::getExtraVisibilityRange` | CvUnit.cpp:12095-12098 | `unit.extra_visibility_range` |
| `CvUnit::getExtraMoves` | CvUnit.cpp:12113-12116 | `unit.extra_moves` |
| `CvUnit::getExtraMoveDiscount` | CvUnit.cpp:12126-12129 | `unit.extra_move_discount` |
| `CvUnit::getExtraAirRange` | CvUnit.cpp:12138-12141 | `unit.extra_air_range` |
| `CvUnit::getExtraIntercept` | CvUnit.cpp:12148-12151 | `unit.extra_intercept` |
| `CvUnit::getExtraEvasion` | CvUnit.cpp:12158-12161 | `unit.extra_evasion` |
| `CvUnit::getExtraFirstStrikes` | CvUnit.cpp:12168-12171 | `unit.extra_first_strikes` |
| `CvUnit::getExtraChanceFirstStrikes` | CvUnit.cpp:12179-12182 | `unit.extra_chance_first_strikes` |
| `CvUnit::getExtraWithdrawal` | CvUnit.cpp:12191-12194 | `unit.extra_withdrawal` |
| `CvUnit::getExtraCollateralDamage` | CvUnit.cpp:12203-12206 | `unit.extra_collateral_damage` |
| `CvUnit::getExtraBombardRate` | CvUnit.cpp:12214-12217 | `unit.extra_bombard_rate` |
| `CvUnit::getExtraEnemyHeal` | CvUnit.cpp:12225-12228 | `unit.extra_enemy_heal` |
| `CvUnit::getExtraNeutralHeal` | CvUnit.cpp:12236-12239 | `unit.extra_neutral_heal` |
| `CvUnit::getExtraFriendlyHeal` | CvUnit.cpp:12247-12250 | `unit.extra_friendly_heal` |
| `CvUnit::getSameTileHeal` | CvUnit.cpp:12259-12262 | `unit.same_tile_heal` |
| `CvUnit::getAdjacentTileHeal` | CvUnit.cpp:12270-12273 | `unit.adjacent_tile_heal` |
| `CvUnit::getExtraCityAttackPercent` | CvUnit.cpp:12306-12309 | `unit.extra_city_attack_percent` |
| `CvUnit::getExtraCityDefensePercent` | CvUnit.cpp:12321-12324 | `unit.extra_city_defense_percent` |
| `CvUnit::getExtraHillsAttackPercent` | CvUnit.cpp:12336-12339 | `unit.extra_hills_attack_percent` |
| `CvUnit::getExtraHillsDefensePercent` | CvUnit.cpp:12351-12354 | `unit.extra_hills_defense_percent` |
| `CvUnit::getRevoltProtection` | CvUnit.cpp:12366-12369 | `unit.revolt_protection` |
| `CvUnit::getCollateralDamageProtection` | CvUnit.cpp:12381-12384 | `unit.collateral_damage_protection` |
| `CvUnit::getPillageChange` | CvUnit.cpp:12396-12399 | `unit.pillage_change` |
| `CvUnit::getUpgradeDiscount` | CvUnit.cpp:12411-12414 | `unit.upgrade_discount` |
| `CvUnit::getExperiencePercent` | CvUnit.cpp:12426-12429 | `unit.experience_percent` |
| `CvUnit::getKamikazePercent` | CvUnit.cpp:12441-12444 | `unit.kamikaze_percent` |
| `CvUnit::isMadeAttack` | CvUnit.cpp:12561-12564 | `unit.made_attack` |
| `CvUnit::setMadeAttack` | CvUnit.cpp:12567-12570 | `unit.made_attack` |
| `CvUnit::isMadeInterception` | CvUnit.cpp:12573-12576 | `unit.made_interception` |
| `CvUnit::setMadeInterception` | CvUnit.cpp:12579-12582 | `unit.made_interception` |
| `CvUnit::isPromotionReady` | CvUnit.cpp:12585-12588 | `unit.promotion_ready` |
| `CvUnit::isDelayedDeath` | CvUnit.cpp:12630-12633 | `unit.death_delay` |
| `CvUnit::startDelayedDeath` | CvUnit.cpp:12636-12639 | `unit.death_delay` (`= true`) |
| `CvUnit::isBlockading` | CvUnit.cpp:12672-12675 | `unit.blockading` |
| `CvUnit::getOwner` | CvUnit.cpp:12731-12734 | `unit.owner` |
| `CvUnit::getCapturingPlayer` | CvUnit.cpp:12782-12785 | `unit.capturing_player` |
| `CvUnit::getUnitType` | CvUnit.cpp:12802-12805 | `unit.unit_type` |
| `CvUnit::getUnitInfo` | CvUnit.cpp:12807-12810 | `self.d.units[unit.unit_type]` |
| `CvUnit::getLeaderUnitType` | CvUnit.cpp:12818-12821 | `unit.leader_unit_type` |
| `CvUnit::setLeaderUnitType` | CvUnit.cpp:12823-12830 | `unit.leader_unit_type` (C++ also reloads the entity: graphics) |
| `CvUnit::getCombatUnit` | CvUnit.cpp:12832-12835 | `unit.combat_unit` |
| `CvUnit::getTransportUnit` | CvUnit.cpp:12912-12915 | `unit.transport_unit` |
| `CvUnit::getExtraDomainModifier` | CvUnit.cpp:12981-12986 | `unit.extra_domain_modifier[domain.idx()]` |
| `CvUnit::getNameNoDesc` | CvUnit.cpp:13025-13028 | `unit.name` |
| `CvUnit::getScriptData` | CvUnit.cpp:13044-13047 | `unit.script_data` |
| `CvUnit::setScriptData` | CvUnit.cpp:13050-13053 | `unit.script_data` |
| `CvUnit::getTerrainDoubleMoveCount` | CvUnit.cpp:13056-13061 | `unit.terrain_double_move_count[terrain]` |
| `CvUnit::getFeatureDoubleMoveCount` | CvUnit.cpp:13081-13086 | `unit.feature_double_move_count[feature]` |
| `CvUnit::getExtraTerrainAttackPercent` | CvUnit.cpp:13106-13111 | `unit.extra_terrain_attack_percent[terrain]` |
| `CvUnit::getExtraTerrainDefensePercent` | CvUnit.cpp:13127-13132 | `unit.extra_terrain_defense_percent[terrain]` |
| `CvUnit::getExtraFeatureAttackPercent` | CvUnit.cpp:13148-13153 | `unit.extra_feature_attack_percent[feature]` |
| `CvUnit::getExtraFeatureDefensePercent` | CvUnit.cpp:13169-13174 | `unit.extra_feature_defense_percent[feature]` |
| `CvUnit::getExtraUnitCombatModifier` | CvUnit.cpp:13190-13195 | `unit.extra_unit_combat_modifier[unit_combat]` |
| `CvUnit::isHasPromotion` | CvUnit.cpp:13402-13407 | `unit.has_promotion[promotion]` |
| `CvUnit::setBaseCombatStr` | CvUnit.cpp:8953-8956 | `unit.base_combat` |
| `CvUnit::setBaseCombatStrDefense` | CvUnit.cpp:8974-8977 | `unit.base_combat_defense` |
| `CvUnit::combatLimit` | CvUnit.cpp:9787-9795 | `unit.combat_limit` |
| `CvUnit::cargoSpace` | CvUnit.cpp:10573-10576 | `unit.cargo_capacity` |
| `CvUnit::getAutoSpellCast` | CvUnit.cpp:16872-16875 | `unit.auto_spell_cast` |
| `CvUnit::setAutoSpellCast` | CvUnit.cpp:16877-16880 | `unit.auto_spell_cast` |
| `CvUnit::isHasCasted` | CvUnit.cpp:15177-15180 | `unit.has_casted` |
| `CvUnit::setHasCasted` | CvUnit.cpp:15182-15185 | `unit.has_casted` |
| `CvUnit::isIgnoreHide` | CvUnit.cpp:15187-15190 | `unit.ignore_hide` |
| `CvUnit::setIgnoreHide` | CvUnit.cpp:15192-15195 | `unit.ignore_hide` |
| `CvUnit::getDelayedSpell` | CvUnit.cpp:15247-15250 | `unit.delayed_spell` |
| `CvUnit::setDelayedSpell` | CvUnit.cpp:15252-15255 | `unit.delayed_spell` |
| `CvUnit::getDuration` | CvUnit.cpp:15257-15260 | `unit.duration` |
| `CvUnit::setDuration` | CvUnit.cpp:15262-15265 | `unit.duration` |
| `CvUnit::isFleeWithdrawl` | CvUnit.cpp:15272-15275 | `unit.flee_withdrawl` |
| `CvUnit::setFleeWithdrawl` | CvUnit.cpp:15277-15280 | `unit.flee_withdrawl` |
| `CvUnit::getDefensiveStrikeChance` | CvUnit.cpp:15321-15324 | `unit.defensive_strike_chance` |
| `CvUnit::getDefensiveStrikeDamage` | CvUnit.cpp:15334-15337 | `unit.defensive_strike_damage` |
| `CvUnit::getBetterDefenderThanPercent` | CvUnit.cpp:15603-15606 | `unit.better_defender_than_percent` |
| `CvUnit::getCombatHealPercent` | CvUnit.cpp:15616-15619 | `unit.combat_heal_percent` |
| `CvUnit::getCombatPercentInBorders` | CvUnit.cpp:15650-15653 | `unit.combat_percent_in_borders` |
| `CvUnit::getCombatPercentGlobalCounter` | CvUnit.cpp:15664-15667 | `unit.combat_percent_global_counter` |
| `CvUnit::getFreePromotionPick` | CvUnit.cpp:15678-15681 | `unit.free_promotion_pick` |
| `CvUnit::getGoldFromCombat` | CvUnit.cpp:15691-15694 | `unit.gold_from_combat` |
| `CvUnit::setInvisibleType` | CvUnit.cpp:15734-15737 | `unit.invisible_type` |
| `CvUnit::getRace` | CvUnit.cpp:15739-15742 | `unit.race` |
| `CvUnit::getReligion` | CvUnit.cpp:15753-15756 | `unit.religion` |
| `CvUnit::setReligion` | CvUnit.cpp:15758-15761 | `unit.religion` |
| `CvUnit::getResist` | CvUnit.cpp:15763-15766 | `unit.resist` |
| `CvUnit::setResist` | CvUnit.cpp:15768-15771 | `unit.resist` |
| `CvUnit::getResistModify` | CvUnit.cpp:15778-15781 | `unit.resist_modify` |
| `CvUnit::setResistModify` | CvUnit.cpp:15783-15786 | `unit.resist_modify` |
| `CvUnit::getScenarioCounter` | CvUnit.cpp:15793-15796 | `unit.scenario_counter` |
| `CvUnit::setScenarioCounter` | CvUnit.cpp:15798-15801 | `unit.scenario_counter` |
| `CvUnit::getSpellDamageModify` | CvUnit.cpp:15820-15823 | `unit.spell_damage_modify` |
| `CvUnit::getSummoner` | CvUnit.cpp:15833-15836 | `unit.summoner` |
| `CvUnit::setSummoner` | CvUnit.cpp:15838-15841 | `unit.summoner` |
| `CvUnit::getWorkRateModify` | CvUnit.cpp:15843-15846 | `unit.work_rate_modify` |
| `CvUnit::getBonusAffinity` | CvUnit.cpp:16628-16631 | `unit.bonus_affinity[bonus]` |
| `CvUnit::getDamageTypeCombat` | CvUnit.cpp:16659-16662 | `unit.damage_type_combat[damage_type]` |
| `CvUnit::getTotalDamageTypeCombat` | CvUnit.cpp:16664-16667 | `unit.total_damage_type_combat` |
| `CvUnit::isTerraformer` | CvUnit.cpp:16800-16803 | `unit.terraformer` |
| `CvUnitAI::AI_getUnitAIType` | CvUnitAI.cpp:1425-1428 | `unit.ai_type` |
| `CvUnitAI::AI_getBirthmark` | CvUnitAI.cpp:1385-1388 | `unit.ai_birthmark` |
| `CvUnitAI::AI_getGroupflag` | CvUnitAI.cpp:19123-19126 | `unit.ai_groupflag` |
| `CvUnitAI::AI_setGroupflag` | CvUnitAI.cpp:19128-19131 | `unit.ai_groupflag` |
| `CvUnitAI::isSuicideSummon` | CvUnitAI.cpp:22094-22097 | `unit.ai_suicide_summon` |
| `CvUnitAI::setSuicideSummon` | CvUnitAI.cpp:22099-22102 | `unit.ai_suicide_summon` |
| `CvUnitAI::changeAllowedPermDefense` | CvUnitAI.cpp:22114-22117 | `unit.ai_allowed_perm_defense` |
| `CvUnitAI::changeAllowedPatrol` | CvUnitAI.cpp:22119-22122 | `unit.ai_allowed_patrol` |
| `CvUnitAI::changeAllowedExplore` | CvUnitAI.cpp:22124-22127 | `unit.ai_allowed_explore` |

## Excluded (not rules)

| C++ function | C++ lines | count | reason |
|---|---|---:|---|
| `CvUnit::reloadEntity` | CvUnit.cpp:75-103 | 29 | graphics/interface |
| `CvUnit::setupGraphical` | CvUnit.cpp:554-567 | 14 | graphics/interface |
| `CvUnit::NotifyEntity` | CvUnit.cpp:1005-1008 | 4 | graphics/interface |
| `CvUnit::isActionRecommended` | CvUnit.cpp:2268-2476 | 209 | graphics/interface (action recommendations for the active player) |
| `CvUnit::checkRemoveSelectionAfterAttack` | CvUnit.cpp:2253-2265 | 13 | graphics/interface (selection list) |
| `CvUnit::airCircle` | CvUnit.cpp:4472-4498 | 27 | graphics/interface (air patrol animation) |
| `CvUnit::getVisualCivAdjective` | CvUnit.cpp:8484-8492 | 9 | graphics/interface (message text) |
| `CvUnit::getHotKeyNumber` | CvUnit.cpp:10947-10950 | 4 | graphics/interface (hot keys) |
| `CvUnit::setHotKeyNumber` | CvUnit.cpp:10953-10980 | 28 | graphics/interface (hot keys) |
| `CvUnit::getFacingDirection` | CvUnit.cpp:12456-12473 | 18 | graphics/interface (no FfH unit has bLineOfSight, so `getFacingDirection(true)` is always NO_DIRECTION) |
| `CvUnit::setFacingDirection` | CvUnit.cpp:12475-12500 | 26 | graphics/interface |
| `CvUnit::rotateFacingDirectionClockwise` | CvUnit.cpp:12502-12507 | 6 | graphics/interface |
| `CvUnit::rotateFacingDirectionCounterClockwise` | CvUnit.cpp:12509-12514 | 6 | graphics/interface |
| `CvUnit::isCombatFocus` | CvUnit.cpp:12655-12658 | 4 | graphics/interface |
| `CvUnit::isInfoBarDirty` | CvUnit.cpp:12661-12664 | 4 | graphics/interface |
| `CvUnit::setInfoBarDirty` | CvUnit.cpp:12667-12670 | 4 | graphics/interface |
| `CvUnit::getSubUnitCount` | CvUnit.cpp:13623-13626 | 4 | graphics/interface (formation figures) |
| `CvUnit::getSubUnitsAlive` | CvUnit.cpp:13629-13632 | 4 | graphics/interface (formation figures) |
| `CvUnit::getSubUnitsAlive` | CvUnit.cpp:13635-13649 | 15 | graphics/interface (formation figures) |
| `CvUnit::isRanged` | CvUnit.cpp:10150-10162 | 13 | graphics/interface (only feeds the battle animation definition) |
| `CvUnit::maxCombatStrFloat` | CvUnit.cpp:9584-9587 | 4 | graphics/interface (odds display) |
| `CvUnit::currCombatStrFloat` | CvUnit.cpp:9590-9593 | 4 | graphics/interface (odds display) |
| `CvUnit::airMaxCombatStrFloat` | CvUnit.cpp:9775-9778 | 4 | graphics/interface (odds display) |
| `CvUnit::airCurrCombatStrFloat` | CvUnit.cpp:9781-9784 | 4 | graphics/interface (odds display) |
| `CvUnit::setGroupSize` | CvUnit.cpp:15704-15707 | 4 | graphics/interface (FfH formation size) |
| `CvUnit::getUnitArtStyleType` | CvUnit.cpp:15907-15910 | 4 | graphics/interface (FfH art style) |
| `CvUnit::setUnitArtStyleType` | CvUnit.cpp:15912-15915 | 4 | graphics/interface (FfH art style) |
| `CvUnit::getArtInfo` | CvUnit.cpp:18247-18258 | 12 | graphics/interface |
| `CvUnit::getButton` | CvUnit.cpp:18260-18270 | 11 | graphics/interface |
| `CvUnit::getGroupSize` | CvUnit.cpp:18277-18284 | 8 | graphics/interface |
| `CvUnit::getGroupDefinitions` | CvUnit.cpp:18287-18290 | 4 | graphics/interface |
| `CvUnit::getUnitGroupRequired` | CvUnit.cpp:18292-18295 | 4 | graphics/interface |
| `CvUnit::isRenderAlways` | CvUnit.cpp:18297-18300 | 4 | graphics/interface |
| `CvUnit::getAnimationMaxSpeed` | CvUnit.cpp:18302-18305 | 4 | graphics/interface |
| `CvUnit::getAnimationPadTime` | CvUnit.cpp:18307-18310 | 4 | graphics/interface |
| `CvUnit::getFormationType` | CvUnit.cpp:18312-18315 | 4 | graphics/interface |
| `CvUnit::isMechUnit` | CvUnit.cpp:18317-18320 | 4 | graphics/interface (rules read `CvUnitInfo::isMechUnit` directly) |
| `CvUnit::isRenderBelowWater` | CvUnit.cpp:18322-18325 | 4 | graphics/interface |
| `CvUnit::getRenderPriority` | CvUnit.cpp:18327-18337 | 11 | graphics/interface |
| `CvUnit::isCombatVisible` | CvUnit.cpp:18379-18405 | 27 | graphics/interface (the port always resolves combat as not visible / quick) |
| `CvUnit::shouldShowEnemyGlow` | CvUnit.cpp:18408-18443 | 36 | graphics/interface |
| `CvUnit::shouldShowFoundBorders` | CvUnit.cpp:18445-18448 | 4 | graphics/interface |
| `CvUnit::cheat` | CvUnit.cpp:18451-18460 | 10 | graphics/interface (cheat key) |
| `CvUnit::getHealthBarModifier` | CvUnit.cpp:18462-18465 | 4 | graphics/interface |
| `CvUnit::getLayerAnimationPaths` | CvUnit.cpp:18467-18481 | 15 | graphics/interface |
| `CvUnit::getSelectionSoundScript` | CvUnit.cpp:18483-18491 | 9 | graphics/interface |
| `CvUnit::planBattle` | CvUnit.cpp:17737-17897 | 161 | graphics/interface (battle animation) |
| `CvUnit::computeUnitsToDie` | CvUnit.cpp:17908-17916 | 9 | graphics/interface (battle animation) |
| `CvUnit::verifyRoundsValid` | CvUnit.cpp:17924-17932 | 9 | graphics/interface (battle animation) |
| `CvUnit::increaseBattleRounds` | CvUnit.cpp:17939-17949 | 11 | graphics/interface (battle animation) |
| `CvUnit::computeWaveSize` | CvUnit.cpp:17959-17978 | 20 | graphics/interface (battle animation) |
| `CvUnit::uninit` | CvUnit.cpp:311-330 | 20 | memory management (Rust drops the arrays) |
| `CvUnit::read` | CvUnit.cpp:16885-17061 | 177 | serialization |
| `CvUnit::write` | CvUnit.cpp:17064-17226 | 163 | serialization |
| `CvUnit::getIndex` | CvUnit.cpp:10699-10702 | 4 | identity: free-list index; the slab key `UnitId` replaces it |
| `CvUnit::getIDInfo` | CvUnit.cpp:10705-10709 | 5 | identity: `IDInfo` is a `UnitId` here |
| `CvUnit::setID` | CvUnit.cpp:10712-10715 | 4 | identity (set at creation) |
| `CvUnit::getPathLastNode` | CvUnit.cpp:2908-2911 | 4 | pathfinder internals (`FAStarNode`); `crate::path` exposes its own results |
| `CvUnit::chooseSpell` | CvUnit.cpp:15917-16088 | 172 | AI: spell choice by AI valuation (`AI_promotionValue`, `AI_unitValue`, ...); belongs to ffh-ai |
| `CvUnit::AI_canJoinGroup` | CvUnit.cpp:18499-18545 | 47 | AI: only CvUnitAI calls it; belongs to ffh-ai |

## Unit struct notes

- 140 fields: one per gameplay data member of `CvUnit` (131 of the 139 in CvUnit.h:984-1140,
  in declaration order), plus 9 `ai_*` fields for the `CvUnitAI` members that rules code reads
  or writes (`ai_control` is FfH's `m_iAIControl`, a `CvUnit` member): `m_eUnitAIType` (`ai_type`,
  `Option` because it is NO_UNITAI between `reset` and `AI_init`, and `setXY` checks that),
  `m_iBirthmark` (set from the "AI Unit Birthmark" sync rand in `AI_init`), `m_iGroupflag`,
  `m_iOriginX/Y`, `m_bSuicideSummon`, `m_bAllowedPermDefense/Patrol/Explore`.
- Not ported as fields: `m_iHotKeyNumber`, `m_eFacingDirection`, `m_bCombatFocus`,
  `m_bInfoBarDirty`, `m_iGroupSize`, `m_iUnitArtStyleType` (graphics/interface; facing only
  matters for bLineOfSight units and FfH has none), `m_iCombatDamage` (declared but never used),
  `m_pUnitInfo` (use `d.units[unit_type]`), `CvUnitAI::m_iAutomatedAbortTurn` and
  `m_bPermanentSummon` (AI-internal only).
- `Unit::new` is `reset(iID, eUnit, eOwner, false)`; the `CvUnitInfo` values reset copies are
  passed in a `UnitResetInfo` (`UnitResetInfo::from_info`). `Default` is the C++ constructor's
  `reset(0, NO_UNIT, NO_PLAYER, true)` (no per-type arrays), with type/owner index 0 standing in
  for NO_UNIT/NO_PLAYER.
