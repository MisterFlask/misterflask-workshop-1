# Porting map: CvTeam, CvSelectionGroup, CvDeal

Map of the Rust port of `CvTeam` (+ the rules-visible parts of `CvTeamAI`),
`CvSelectionGroup` (+ `CvSelectionGroupAI`) and `CvDeal`. Each Rust file below has a table
listing every C++ function it holds, with the C++ line range (signature to closing brace,
in `/home/user/ref/ffh2/CvGameCoreDLL/`) and line count.

**Status: ported and audited.** Every function is ported (no `todo!` left), and each file was
compared line by line with the C++ in the audit wave.

Read `projects/ffh2-rs/CLAUDE.md` ("Rules engine conventions") first; this page only adds
what is specific to these three classes.

## Summary

| Rust file | C++ lines | functions | ported |
|---|---:|---:|---:|
| `team/mod.rs` | 311 | 6 | 6 |
| `team/diplomacy.rs` | 1895 | 43 | 43 |
| `team/tech.rs` | 1463 | 40 | 40 |
| `team/misc.rs` | 1519 | 63 | 63 |
| `group/mod.rs` | 77 | 6 | 6 |
| `group/missions.rs` | 1549 | 18 | 18 |
| `group/movement.rs` | 1348 | 26 | 26 |
| `group/misc.rs` | 1464 | 60 | 60 |
| `deal.rs` | 969 | 28 | 28 |
| **total** | **10595** | **290** | **290** |

"functions" counts C++ functions (a few map to the same Rust method, e.g. `area` and
`getArea`). Rust side: 266 `impl Game` methods (team 147, group 104, deal 15), all ported,
plus 14 associated functions (`Team::new`, `SelectionGroup::new`, `Deal::new` and the static
`CvDeal` helpers). 172 trivial C++ accessors map to fields (see
[Accessors that are fields](#accessors-that-are-fields)).

Structs: `Team` 72 fields (60 `CvTeam` + 12 `CvTeamAI`, prefixed `ai_`),
`SelectionGroup` 18 fields (10 `CvSelectionGroup` + 8 `CvSelectionGroupAI`, prefixed `ai_`),
`Deal` 6 fields.

## Conventions specific to these classes

**Names.** `CvTeam::foo` -> `team_foo`, `CvSelectionGroup::foo` -> `group_foo`,
`CvDeal::foo` -> `deal_foo`, snake_case, keeping `get`/`is`/`can`/`has`
(`getLeaderID` -> `team_get_leader_id`). `CvTeamAI::AI_foo` / `CvSelectionGroupAI::AI_foo`
-> `team_ai_foo` / `group_ai_foo`. Exceptions:
- `CvSelectionGroup::groupMove/groupAttack/groupPathTo/groupRoadTo/groupBuild/
  groupDeclareWar/groupAmphibMove` drop the redundant prefix: `group_move`, `group_attack`, ...
- Overloads: `changeWarWeariness(TeamTypes, int)` -> `team_change_war_weariness`,
  `changeWarWeariness(TeamTypes, const CvPlot&, int)` -> `team_change_war_weariness_at_plot`;
  `updateTechShare()` -> `team_update_tech_share`, `updateTechShare(TechTypes)` ->
  `team_update_tech_share_tech`.
- `CvSelectionGroup::area()` and `getArea()` are one method, `group_area` (the area id).
- Static `CvDeal` helpers are associated functions: `CvDeal::isAnnual(e)` -> `Deal::is_annual(e)`,
  `getPeaceItem()` -> `Deal::get_peace_item()`, `isVassalTrade(list)` -> `Deal::is_vassal_trade(&[TradeData])`.

**Signatures.**
- C++ default arguments are explicit parameters; each method's doc comment gives the C++
  defaults (`pushMission(MISSION_SKIP)` is
  `group_push_mission(g, MissionType::MISSION_SKIP, -1, -1, 0, false, false, None, None, None)`).
- `NO_X` arguments/results are `Option`: `team_get_defensive_pact_count(t, None)` (C++
  `NO_TEAM`), `team_declare_war(.., war_plan: None)` (`NO_WARPLAN`), `team_get_leader_id -> Option<PlayerId>`,
  `group_plot -> Option<PlotIdx>`, `deal_is_cancelable(d, None)` (`NO_PLAYER`).
- NO_TECH: `team_is_has_tech(t, None)` is true and `team_get_research_progress(t, None)` is
  0, as in C++; `team_set_has_tech` takes a `TechType` (C++ returns at once for NO_TECH, so
  callers with an optional tech skip the call).
- Out-parameters become tuple results: `groupAttack(.., bool& bFailedAlreadyFighting)` ->
  `group_attack(..) -> (attacked, failed_already_fighting)`;
  `AI_getBestGroupAttacker(.., int& iUnitOdds, ..)` -> `(Option<UnitId>, odds)`;
  `getBestBuildRoute(plot, BuildTypes*)` -> `(Option<RouteType>, Option<BuildType>)`;
  `splitGroup(.., CvSelectionGroup** ppOtherGroup)` -> `(split, other)`;
  `generatePath(.., int* piPathTurns)` -> `Option<path_turns>` (None = no path).
  UI-text out-parameters (`CvDeal::isCancelable(.., CvWString* pszReason)`) are dropped.
- Constness follows what the C++ body does, not its declaration: read-only non-const C++
  functions take `&self` (`isBusy`, `canStartMission`, `canMoveInto`, `isCancelable`, ...);
  `const` C++ functions that mutate game state take `&mut self` (`CvTeam::assignVassal`,
  `freeVassal`). The path methods (`group_generate_path`, `group_get_path_first_plot`,
  `group_get_path_end_turn_plot`, `group_reset_path`) take `&self` because C++ calls
  `generatePath` from const code (`CvUnit::getUpgradeCity`); the C++ global FAStar state
  lives behind interior mutability in the path module (`RefCell<PathFinder>`).
- Linked-list node APIs are index-based: `deleteMissionQueueNode(pNode)` ->
  `group_delete_mission_queue_node(g, index)`, `deleteUnitNode(pNode)` ->
  `group_delete_unit_node(g, index)`; head/next/prev/tail node accessors are plain `Vec` access.
- Group owner: `SelectionGroup::owner` is a `PlayerId`; C++ ownerless groups exist only as
  the UI selection list (not ported), so `NO_PLAYER`-owner branches (`isHuman` returning
  true, `getTeam` returning `NO_TEAM`, the `addUnit` entity notifications) are dead here.

**Accessors with logic are methods.** Use them instead of the raw fields:
- `team_is_open_borders` -- FFH returns true for every pair under
  GAMEOPTION_ALWAYS_OPEN_BORDERS; `Team::open_borders` is only the raw flag.
- `team_is_alive`, `team_is_ever_alive`, `team_is_a_vassal`, `team_is_x_trading`
  (`team_is_permanent_alliance_trading` / `team_is_vassal_state_trading` check game options).
- `team_set_war_weariness` clamps at 0; `team_change_alive_count` frees vassals at 0;
  `team_ai_set_war_plan` only changes the plan when `war || !at_war` and resets the
  war-plan state counter.
- `team_get_total_population` / `team_get_total_land` add half of each vassal's (and halve
  a vassal's own) unless `check_vassals` is false (C++ default true).
- `group_is_automated` includes FfH AI-controlled units of human players (Xienwolf tweak);
  `group_ai_get_mission_ai_unit` drops dead units.

**Game state owned elsewhere.**
- Game options are read through `team::is_option` (private helper = `self.setup.option(o)`)
  or `Game::is_option`. FFH's `CvGame::isOption` forces GAMEOPTION_NO_ESPIONAGE on and
  GAMEOPTION_RANDOM_PERSONALITIES / GAMEOPTION_PICK_RELIGION off; the helper debug-asserts it
  is not used for those (`setHasTech` reads PICK_RELIGION through `Game::is_option`).
- `CvGame::getGameTurn()` is `GC.getInitCore().getGameTurn()` = `self.setup.game_turn`
  (used by `deal_init`, `deal_turns_to_cancel`).

**AI bookkeeping.** Fields: war plans (`Team::ai_war_plan`), war-plan state, at-war/at-peace/
has-met/open-borders/defensive-pact/share-war counters, war success, enemy peacetime
trade/grant values, worst enemy; group mission AI (type, x/y, unit), force separate, group
attack (flag, x/y). Methods (rules code calls them): `team_ai_set_war_plan`,
`team_ai_is_chosen_war`, `team_ai_is_sneak_attack_preparing/ready`, `team_ai_share_war`,
`team_ai_do_counter`, `team_ai_update_worst_enemy`, `team_ai_get_attitude`,
`team_ai_make_assign_work_dirty` (governor), `group_ai_is_controlled`,
`group_ai_is_declare_war`, `group_ai_get_best_group_attacker/sacrifice` (pick the attacker in
`groupAttack`, also for humans), `group_ai_queue_group_attack`, `group_ai_set_mission_ai`,
`group_ai_get_mission_ai_plot/unit`, `group_ai_get/set_groupflag` (FFH stores the flag per
unit, `CvUnitAI::m_iGroupflag` -> `Unit::ai_groupflag`, default GROUPFLAG_NONE; the group
reads the head unit's and sets all units'), `group_is_head_unit`, `group_get_bombard_turns`.
`CvTeam::doTurn` calls `AI_doTurnPre()` = `AI_doCounter()` + early returns, so port it as
`team_ai_do_counter`; `AI_doTurnPost()` = `team_ai_update_worst_enemy` + `// AI:`
`AI_updateAreaStragies(false)` + `// AI: AI_doWar()` (war decisions -> controller).
Other `CvTeamAI`/`CvSelectionGroupAI` functions (trade denials, `AI_doWar`, `AI_update`,
`AI_separate`, stack strength, ...) are AI: omit with a `// AI:` comment or route decisions
through `controller.rs`.

**Python and events** (call the `crate::ffh::*` ports, per CLAUDE.md): `CvTeam::canDeclareWar`
-> CvGameUtils `canDeclareWar` (only when `USE_CAN_DECLARE_WAR_CALLBACK`, 0 in FFH2 data);
`setHasTech` -> CvGameUtils `doHolyCityTech` (FFH returns False) and event `techAcquired`;
`declareWar`/`makePeace` -> `changeWar`; `makeHasMet` -> `firstContact`; `setVassal` ->
`vassalState`; `CvSelectionGroup::groupAttack` -> CvGameUtils `doCombat` (FFH returns
False); `pushMission` -> `selectionGroupPushMission`; `CvDeal::startTrade` (gold) ->
`playerGoldTrade`.

## Rust files

### `crates/ffh-core/src/team/mod.rs`

C++ lines: **311** total; 6 functions, all ported.

| C++ function | C++ lines | # | Rust | Status |
|---|---|---:|---|---|
| `CvTeam::CvTeam` | CvTeam.cpp:27-68 | 42 | `Team::new` | ported |
| `CvTeam::init` | CvTeam.cpp:93-105 | 13 | `team_init` | ported |
| `CvTeam::reset` | CvTeam.cpp:144-339 | 196 | `Team::new + team_init` | ported |
| `CvTeamAI::CvTeamAI` | CvTeamAI.cpp:45-59 | 15 | `Team::new` | ported |
| `CvTeamAI::AI_init` | CvTeamAI.cpp:80-86 | 7 | `team_init` | ported |
| `CvTeamAI::AI_reset` | CvTeamAI.cpp:94-131 | 38 | `Team::new + team_init` | ported |

### `crates/ffh-core/src/team/diplomacy.rs`

C++ lines: **1895** total; 43 functions, all ported.

| C++ function | C++ lines | # | Rust | Status |
|---|---|---:|---|---|
| `CvTeam::canChangeWarPeace` | CvTeam.cpp:1001-1056 | 56 | `team_can_change_war_peace` | ported |
| `CvTeam::canDeclareWar` | CvTeam.cpp:1059-1122 | 64 | `team_can_declare_war` | ported |
| `CvTeam::declareWar` | CvTeam.cpp:1125-1471 | 347 | `team_declare_war` | ported |
| `CvTeam::makePeace` | CvTeam.cpp:1473-1603 | 131 | `team_make_peace` | ported |
| `CvTeam::canContact` | CvTeam.cpp:1606-1634 | 29 | `team_can_contact` | ported |
| `CvTeam::meet` | CvTeam.cpp:1637-1644 | 8 | `team_meet` | ported |
| `CvTeam::signOpenBorders` | CvTeam.cpp:1647-1671 | 25 | `team_sign_open_borders` | ported |
| `CvTeam::signDefensivePact` | CvTeam.cpp:1674-1698 | 25 | `team_sign_defensive_pact` | ported |
| `CvTeam::canSignDefensivePact` | CvTeam.cpp:1700-1720 | 21 | `team_can_sign_defensive_pact` | ported |
| `CvTeam::getAtWarCount` | CvTeam.cpp:1896-1920 | 25 | `team_get_at_war_count` | ported |
| `CvTeam::getWarPlanCount` | CvTeam.cpp:1923-1946 | 24 | `team_get_war_plan_count` | ported |
| `CvTeam::getAnyWarPlanCount` | CvTeam.cpp:1949-1974 | 26 | `team_get_any_war_plan_count` | ported |
| `CvTeam::getChosenWarCount` | CvTeam.cpp:1977-2000 | 24 | `team_get_chosen_war_count` | ported |
| `CvTeam::getHasMetCivCount` | CvTeam.cpp:2003-2029 | 27 | `team_get_has_met_civ_count` | ported |
| `CvTeam::hasMetHuman` | CvTeam.cpp:2032-2055 | 24 | `team_has_met_human` | ported |
| `CvTeam::getDefensivePactCount` | CvTeam.cpp:2058-2083 | 26 | `team_get_defensive_pact_count` | ported |
| `CvTeam::getVassalCount` | CvTeam.cpp:2085-2108 | 24 | `team_get_vassal_count` | ported |
| `CvTeam::isAVassal` | CvTeam.cpp:2110-2130 | 21 | `team_is_a_vassal` | ported |
| `CvTeam::canVassalRevolt` | CvTeam.cpp:2132-2164 | 33 | `team_can_vassal_revolt` | ported |
| `CvTeam::setWarWeariness` | CvTeam.cpp:3280-3285 | 6 | `team_set_war_weariness` | ported |
| `CvTeam::changeWarWeariness` | CvTeam.cpp:3288-3292 | 5 | `team_change_war_weariness` | ported |
| `CvTeam::changeWarWeariness` | CvTeam.cpp:3294-3306 | 13 | `team_change_war_weariness_at_plot` | ported |
| `CvTeam::makeHasMet` | CvTeam.cpp:3399-3493 | 95 | `team_make_has_met` | ported |
| `CvTeam::isFreeTrade` | CvTeam.cpp:3528-3541 | 14 | `team_is_free_trade` | ported |
| `CvTeam::isOpenBorders` | CvTeam.cpp:3544-3557 | 14 | `team_is_open_borders` | ported |
| `CvTeam::setOpenBorders` | CvTeam.cpp:3560-3597 | 38 | `team_set_open_borders` | ported |
| `CvTeam::setDefensivePact` | CvTeam.cpp:3608-3642 | 35 | `team_set_defensive_pact` | ported |
| `CvTeam::setForcePeace` | CvTeam.cpp:3653-3677 | 25 | `team_set_force_peace` | ported |
| `CvTeam::setVassal` | CvTeam.cpp:3686-3980 | 295 | `team_set_vassal` | ported |
| `CvTeam::assignVassal` | CvTeam.cpp:3982-4012 | 31 | `team_assign_vassal` | ported |
| `CvTeam::freeVassal` | CvTeam.cpp:4015-4065 | 51 | `team_free_vassal` | ported |
| `CvTeam::isParent` | CvTeam.cpp:4716-4739 | 24 | `team_is_parent` | ported |
| `CvTeam::doWarWeariness` | CvTeam.cpp:5296-5312 | 17 | `team_do_war_weariness` | ported |
| `CvTeam::cancelDefensivePacts` | CvTeam.cpp:5670-5711 | 42 | `team_cancel_defensive_pacts` | ported |
| `CvTeam::isFriendlyTerritory` | CvTeam.cpp:5714-5737 | 24 | `team_is_friendly_territory` | ported |
| `CvTeamAI::AI_shareWar` | CvTeamAI.cpp:700-719 | 20 | `team_ai_share_war` | ported |
| `CvTeamAI::AI_getAttitude` | CvTeamAI.cpp:722-761 | 40 | `team_ai_get_attitude` | ported |
| `CvTeamAI::AI_updateWorstEnemy` | CvTeamAI.cpp:2329-2361 | 33 | `team_ai_update_worst_enemy` | ported |
| `CvTeamAI::AI_isChosenWar` | CvTeamAI.cpp:2603-2621 | 19 | `team_ai_is_chosen_war` | ported |
| `CvTeamAI::AI_isSneakAttackPreparing` | CvTeamAI.cpp:2624-2627 | 4 | `team_ai_is_sneak_attack_preparing` | ported |
| `CvTeamAI::AI_isSneakAttackReady` | CvTeamAI.cpp:2630-2633 | 4 | `team_ai_is_sneak_attack_ready` | ported |
| `CvTeamAI::AI_setWarPlan` | CvTeamAI.cpp:2636-2667 | 32 | `team_ai_set_war_plan` | ported |
| `CvTeamAI::AI_doCounter` | CvTeamAI.cpp:3074-3127 | 54 | `team_ai_do_counter` | ported |

### `crates/ffh-core/src/team/tech.rs`

C++ lines: **1463** total; 40 functions, all ported.

| C++ function | C++ lines | # | Rust | Status |
|---|---|---:|---|---|
| `CvTeam::shareItems` | CvTeam.cpp:632-706 | 75 | `team_share_items` | ported |
| `CvTeam::processBuilding` | CvTeam.cpp:831-865 | 35 | `team_process_building` | ported |
| `CvTeam::doTurn` | CvTeam.cpp:868-947 | 80 | `team_do_turn` | ported |
| `CvTeam::updateYield` | CvTeam.cpp:950-964 | 15 | `team_update_yield` | ported |
| `CvTeam::updatePowerHealth` | CvTeam.cpp:967-981 | 15 | `team_update_power_health` | ported |
| `CvTeam::updateCommerce` | CvTeam.cpp:984-998 | 15 | `team_update_commerce` | ported |
| `CvTeam::getResearchCost` | CvTeam.cpp:2469-2493 | 25 | `team_get_research_cost` | ported |
| `CvTeam::getResearchLeft` | CvTeam.cpp:2496-2499 | 4 | `team_get_research_left` | ported |
| `CvTeam::hasBonus` | CvTeam.cpp:2533-2547 | 15 | `team_has_bonus` | ported |
| `CvTeam::isBonusObsolete` | CvTeam.cpp:2549-2557 | 9 | `team_is_bonus_obsolete` | ported |
| `CvTeam::isExtraWaterSeeFrom` | CvTeam.cpp:2904-2907 | 4 | `team_is_extra_water_see_from` | ported |
| `CvTeam::changeExtraWaterSeeFromCount` | CvTeam.cpp:2910-2921 | 12 | `team_change_extra_water_see_from_count` | ported |
| `CvTeam::isBridgeBuilding` | CvTeam.cpp:3073-3076 | 4 | `team_is_bridge_building` | ported |
| `CvTeam::isIrrigation` | CvTeam.cpp:3100-3103 | 4 | `team_is_irrigation` | ported |
| `CvTeam::changeIrrigationCount` | CvTeam.cpp:3106-3115 | 10 | `team_change_irrigation_count` | ported |
| `CvTeam::isIgnoreIrrigation` | CvTeam.cpp:3124-3127 | 4 | `team_is_ignore_irrigation` | ported |
| `CvTeam::isWaterWork` | CvTeam.cpp:3143-3146 | 4 | `team_is_water_work` | ported |
| `CvTeam::changeWaterWorkCount` | CvTeam.cpp:3149-3158 | 10 | `team_change_water_work_count` | ported |
| `CvTeam::isTechShare` | CvTeam.cpp:3317-3320 | 4 | `team_is_tech_share` | ported |
| `CvTeam::changeTechShareCount` | CvTeam.cpp:3323-3338 | 16 | `team_change_tech_share_count` | ported |
| `CvTeam::isObsoleteBuilding` | CvTeam.cpp:4393-4396 | 4 | `team_is_obsolete_building` | ported |
| `CvTeam::changeObsoleteBuildingCount` | CvTeam.cpp:4399-4436 | 38 | `team_change_obsolete_building_count` | ported |
| `CvTeam::getResearchProgress` | CvTeam.cpp:4439-4449 | 11 | `team_get_research_progress` | ported |
| `CvTeam::setResearchProgress` | CvTeam.cpp:4452-4481 | 30 | `team_set_research_progress` | ported |
| `CvTeam::changeResearchProgress` | CvTeam.cpp:4484-4487 | 4 | `team_change_research_progress` | ported |
| `CvTeam::changeResearchProgressPercent` | CvTeam.cpp:4489-4508 | 20 | `team_change_research_progress_percent` | ported |
| `CvTeam::isTerrainTrade` | CvTeam.cpp:4527-4530 | 4 | `team_is_terrain_trade` | ported |
| `CvTeam::changeTerrainTradeCount` | CvTeam.cpp:4533-4556 | 24 | `team_change_terrain_trade_count` | ported |
| `CvTeam::isRiverTrade` | CvTeam.cpp:4565-4568 | 4 | `team_is_river_trade` | ported |
| `CvTeam::changeRiverTradeCount` | CvTeam.cpp:4571-4589 | 19 | `team_change_river_trade_count` | ported |
| `CvTeam::isHasTech` | CvTeam.cpp:4741-4752 | 12 | `team_is_has_tech` | ported |
| `CvTeam::setHasTech` | CvTeam.cpp:4772-5249 | 478 | `team_set_has_tech` | ported |
| `CvTeam::changeImprovementYieldChange` | CvTeam.cpp:5278-5292 | 15 | `team_change_improvement_yield_change` | ported |
| `CvTeam::updateTechShare` | CvTeam.cpp:5314-5359 | 46 | `team_update_tech_share_tech` | ported |
| `CvTeam::updateTechShare` | CvTeam.cpp:5362-5370 | 9 | `team_update_tech_share` | ported |
| `CvTeam::testCircumnavigated` | CvTeam.cpp:5373-5470 | 98 | `team_test_circumnavigated` | ported |
| `CvTeam::processTech` | CvTeam.cpp:5472-5667 | 196 | `team_process_tech` | ported |
| `CvTeam::setForceRevealedBonus` | CvTeam.cpp:5877-5938 | 62 | `team_set_force_revealed_bonus` | ported |
| `CvTeam::isForceRevealedBonus` | CvTeam.cpp:5940-5953 | 14 | `team_is_force_revealed_bonus` | ported |
| `CvTeamAI::AI_makeAssignWorkDirty` | CvTeamAI.cpp:180-194 | 15 | `team_ai_make_assign_work_dirty` | ported |

### `crates/ffh-core/src/team/misc.rs`

C++ lines: **1519** total; 63 functions, all ported.

| C++ function | C++ lines | # | Rust | Status |
|---|---|---:|---|---|
| `CvTeam::addTeam` | CvTeam.cpp:342-629 | 288 | `team_add_team` | ported |
| `CvTeam::shareCounters` | CvTeam.cpp:709-828 | 120 | `team_share_counters` | ported |
| `CvTeam::getAssets` | CvTeam.cpp:1723-1742 | 20 | `team_get_assets` | ported |
| `CvTeam::getPower` | CvTeam.cpp:1745-1765 | 21 | `team_get_power` | ported |
| `CvTeam::getDefensivePower` | CvTeam.cpp:1768-1788 | 21 | `team_get_defensive_power` | ported |
| `CvTeam::getEnemyPower` | CvTeam.cpp:1791-1808 | 18 | `team_get_enemy_power` | ported |
| `CvTeam::getNumNukeUnits` | CvTeam.cpp:1811-1831 | 21 | `team_get_num_nuke_units` | ported |
| `CvTeam::getVotes` | CvTeam.cpp:1833-1867 | 35 | `team_get_votes` | ported |
| `CvTeam::isVotingMember` | CvTeam.cpp:1869-1872 | 4 | `team_is_voting_member` | ported |
| `CvTeam::isFullMember` | CvTeam.cpp:1874-1894 | 21 | `team_is_full_member` | ported |
| `CvTeam::getUnitClassMaking` | CvTeam.cpp:2167-2186 | 20 | `team_get_unit_class_making` | ported |
| `CvTeam::getUnitClassCountPlusMaking` | CvTeam.cpp:2189-2192 | 4 | `team_get_unit_class_count_plus_making` | ported |
| `CvTeam::getBuildingClassMaking` | CvTeam.cpp:2195-2214 | 20 | `team_get_building_class_making` | ported |
| `CvTeam::getBuildingClassCountPlusMaking` | CvTeam.cpp:2217-2220 | 4 | `team_get_building_class_count_plus_making` | ported |
| `CvTeam::getHasReligionCount` | CvTeam.cpp:2223-2242 | 20 | `team_get_has_religion_count` | ported |
| `CvTeam::getHasCorporationCount` | CvTeam.cpp:2245-2261 | 17 | `team_get_has_corporation_count` | ported |
| `CvTeam::countTotalCulture` | CvTeam.cpp:2264-2283 | 20 | `team_count_total_culture` | ported |
| `CvTeam::countNumUnitsByArea` | CvTeam.cpp:2286-2307 | 22 | `team_count_num_units_by_area` | ported |
| `CvTeam::countNumCitiesByArea` | CvTeam.cpp:2310-2331 | 22 | `team_count_num_cities_by_area` | ported |
| `CvTeam::countTotalPopulationByArea` | CvTeam.cpp:2334-2353 | 20 | `team_count_total_population_by_area` | ported |
| `CvTeam::countPowerByArea` | CvTeam.cpp:2356-2375 | 20 | `team_count_power_by_area` | ported |
| `CvTeam::countEnemyPowerByArea` | CvTeam.cpp:2378-2412 | 35 | `team_count_enemy_power_by_area` | ported |
| `CvTeam::countNumAIUnitsByArea` | CvTeam.cpp:2415-2436 | 22 | `team_count_num_ai_units_by_area` | ported |
| `CvTeam::countEnemyDangerByArea` | CvTeam.cpp:2439-2466 | 28 | `team_count_enemy_danger_by_area` | ported |
| `CvTeam::hasHolyCity` | CvTeam.cpp:2502-2516 | 15 | `team_has_holy_city` | ported |
| `CvTeam::hasHeadquarters` | CvTeam.cpp:2519-2531 | 13 | `team_has_headquarters` | ported |
| `CvTeam::isHuman` | CvTeam.cpp:2560-2578 | 19 | `team_is_human` | ported |
| `CvTeam::isBarbarian` | CvTeam.cpp:2581-2584 | 4 | `team_is_barbarian` | ported |
| `CvTeam::isMinorCiv` | CvTeam.cpp:2587-2610 | 24 | `team_is_minor_civ` | ported |
| `CvTeam::getLeaderID` | CvTeam.cpp:2613-2637 | 25 | `team_get_leader_id` | ported |
| `CvTeam::getSecretaryID` | CvTeam.cpp:2640-2659 | 20 | `team_get_secretary_id` | ported |
| `CvTeam::getHandicapType` | CvTeam.cpp:2662-2692 | 31 | `team_get_handicap_type` | ported |
| `CvTeam::getName` | CvTeam.cpp:2695-2713 | 19 | `team_get_name` | ported |
| `CvTeam::isAlive` | CvTeam.cpp:2735-2738 | 4 | `team_is_alive` | ported |
| `CvTeam::changeAliveCount` | CvTeam.cpp:2741-2761 | 21 | `team_change_alive_count` | ported |
| `CvTeam::isEverAlive` | CvTeam.cpp:2770-2773 | 4 | `team_is_ever_alive` | ported |
| `CvTeam::getTotalPopulation` | CvTeam.cpp:2796-2821 | 26 | `team_get_total_population` | ported |
| `CvTeam::getTotalLand` | CvTeam.cpp:2831-2856 | 26 | `team_get_total_land` | ported |
| `CvTeam::isForceTeamVoteEligible` | CvTeam.cpp:2885-2888 | 4 | `team_is_force_team_vote_eligible` | ported |
| `CvTeam::isMapTrading` | CvTeam.cpp:2930-2933 | 4 | `team_is_map_trading` | ported |
| `CvTeam::isTechTrading` | CvTeam.cpp:2949-2952 | 4 | `team_is_tech_trading` | ported |
| `CvTeam::isGoldTrading` | CvTeam.cpp:2968-2971 | 4 | `team_is_gold_trading` | ported |
| `CvTeam::isOpenBordersTrading` | CvTeam.cpp:2987-2990 | 4 | `team_is_open_borders_trading` | ported |
| `CvTeam::isDefensivePactTrading` | CvTeam.cpp:3006-3009 | 4 | `team_is_defensive_pact_trading` | ported |
| `CvTeam::isPermanentAllianceTrading` | CvTeam.cpp:3025-3033 | 9 | `team_is_permanent_alliance_trading` | ported |
| `CvTeam::isVassalStateTrading` | CvTeam.cpp:3049-3057 | 9 | `team_is_vassal_state_trading` | ported |
| `CvTeam::isStolenVisibility` | CvTeam.cpp:3225-3231 | 7 | `team_is_stolen_visibility` | ported |
| `CvTeam::setStolenVisibilityTimer` | CvTeam.cpp:3234-3263 | 30 | `team_set_stolen_visibility_timer` | ported |
| `CvTeam::changeStolenVisibilityTimer` | CvTeam.cpp:3266-3269 | 4 | `team_change_stolen_visibility_timer` | ported |
| `CvTeam::isCommerceFlexible` | CvTeam.cpp:3349-3352 | 4 | `team_is_commerce_flexible` | ported |
| `CvTeam::isProjectMaxedOut` | CvTeam.cpp:4130-4143 | 14 | `team_is_project_maxed_out` | ported |
| `CvTeam::changeProjectCount` | CvTeam.cpp:4187-4299 | 113 | `team_change_project_count` | ported |
| `CvTeam::isUnitClassMaxedOut` | CvTeam.cpp:4327-4340 | 14 | `team_is_unit_class_maxed_out` | ported |
| `CvTeam::isBuildingClassMaxedOut` | CvTeam.cpp:4360-4373 | 14 | `team_is_building_class_maxed_out` | ported |
| `CvTeam::getVictoryDelay` | CvTeam.cpp:4619-4640 | 22 | `team_get_victory_delay` | ported |
| `CvTeam::getLaunchSuccessRate` | CvTeam.cpp:4652-4675 | 24 | `team_get_launch_success_rate` | ported |
| `CvTeam::resetVictoryProgress` | CvTeam.cpp:4677-4714 | 38 | `team_reset_victory_progress` | ported |
| `CvTeam::setEspionagePointsAgainstTeam` | CvTeam.cpp:5746-5758 | 13 | `team_set_espionage_points_against_team` | ported |
| `CvTeam::changeEspionagePointsAgainstTeam` | CvTeam.cpp:5760-5766 | 7 | `team_change_espionage_points_against_team` | ported |
| `CvTeam::verifySpyUnitsValidPlot` | CvTeam.cpp:5842-5874 | 33 | `team_verify_spy_units_valid_plot` | ported |
| `CvTeam::setTurnActive` | CvTeam.cpp:5975-5987 | 13 | `team_set_turn_active` | ported |
| `CvTeam::isTurnActive` | CvTeam.cpp:5989-6006 | 18 | `team_is_turn_active` | ported |
| `CvTeam::hasShrine` | CvTeam.cpp:6213-6227 | 15 | `team_has_shrine` | ported |

### `crates/ffh-core/src/group/mod.rs`

C++ lines: **77** total; 6 functions, all ported.

| C++ function | C++ lines | # | Rust | Status |
|---|---|---:|---|---|
| `CvSelectionGroup::CvSelectionGroup` | CvSelectionGroup.cpp:29-32 | 4 | `SelectionGroup::new` | ported |
| `CvSelectionGroup::init` | CvSelectionGroup.cpp:41-53 | 13 | `group_init` | ported |
| `CvSelectionGroup::reset` | CvSelectionGroup.cpp:66-97 | 32 | `SelectionGroup::new` | ported |
| `CvSelectionGroupAI::CvSelectionGroupAI` | CvSelectionGroupAI.cpp:16-19 | 4 | `SelectionGroup::new` | ported |
| `CvSelectionGroupAI::AI_init` | CvSelectionGroupAI.cpp:28-34 | 7 | `group_init` | ported |
| `CvSelectionGroupAI::AI_reset` | CvSelectionGroupAI.cpp:42-58 | 17 | `SelectionGroup::new` | ported |

### `crates/ffh-core/src/group/missions.rs`

C++ lines: **1549** total; 18 functions, all ported.

| C++ function | C++ lines | # | Rust | Status |
|---|---|---:|---|---|
| `CvSelectionGroup::updateTimers` | CvSelectionGroup.cpp:342-384 | 43 | `group_update_timers` | ported |
| `CvSelectionGroup::doDelayedDeath` | CvSelectionGroup.cpp:388-417 | 30 | `group_do_delayed_death` | ported |
| `CvSelectionGroup::pushMission` | CvSelectionGroup.cpp:457-506 | 50 | `group_push_mission` | ported |
| `CvSelectionGroup::popMission` | CvSelectionGroup.cpp:509-521 | 13 | `group_pop_mission` | ported |
| `CvSelectionGroup::autoMission` | CvSelectionGroup.cpp:524-568 | 45 | `group_auto_mission` | ported |
| `CvSelectionGroup::updateMission` | CvSelectionGroup.cpp:571-597 | 27 | `group_update_mission` | ported |
| `CvSelectionGroup::canStartMission` | CvSelectionGroup.cpp:672-994 | 323 | `group_can_start_mission` | ported |
| `CvSelectionGroup::startMission` | CvSelectionGroup.cpp:997-1436 | 440 | `group_start_mission` | ported |
| `CvSelectionGroup::continueMission` | CvSelectionGroup.cpp:1439-1801 | 363 | `group_continue_mission` | ported |
| `CvSelectionGroup::updateMissionTimer` | CvSelectionGroup.cpp:3732-3788 | 57 | `group_update_mission_timer` | ported |
| `CvSelectionGroup::clearMissionQueue` | CvSelectionGroup.cpp:4500-4514 | 15 | `group_clear_mission_queue` | ported |
| `CvSelectionGroup::insertAtEndMissionQueue` | CvSelectionGroup.cpp:4540-4557 | 18 | `group_insert_at_end_mission_queue` | ported |
| `CvSelectionGroup::deleteMissionQueueNode` | CvSelectionGroup.cpp:4560-4587 | 28 | `group_delete_mission_queue_node` | ported |
| `CvSelectionGroup::getMissionType` | CvSelectionGroup.cpp:4614-4634 | 21 | `group_get_mission_type` | ported |
| `CvSelectionGroup::getMissionData1` | CvSelectionGroup.cpp:4637-4657 | 21 | `group_get_mission_data1` | ported |
| `CvSelectionGroup::getMissionData2` | CvSelectionGroup.cpp:4660-4680 | 21 | `group_get_mission_data2` | ported |
| `CvSelectionGroup::activateHeadMission` | CvSelectionGroup.cpp:4725-4736 | 12 | `group_activate_head_mission` | ported |
| `CvSelectionGroup::deactivateHeadMission` | CvSelectionGroup.cpp:4739-4760 | 22 | `group_deactivate_head_mission` | ported |

### `crates/ffh-core/src/group/movement.rs`

C++ lines: **1348** total; 26 functions, all ported.

| C++ function | C++ lines | # | Rust | Status |
|---|---|---:|---|---|
| `CvSelectionGroup::lastMissionPlot` | CvSelectionGroup.cpp:600-669 | 70 | `group_last_mission_plot` | ported |
| `CvSelectionGroup::canEnterTerritory` | CvSelectionGroup.cpp:2471-2495 | 25 | `group_can_enter_territory` | ported |
| `CvSelectionGroup::canEnterArea` | CvSelectionGroup.cpp:2497-2521 | 25 | `group_can_enter_area` | ported |
| `CvSelectionGroup::canMoveInto` | CvSelectionGroup.cpp:2524-2546 | 23 | `group_can_move_into` | ported |
| `CvSelectionGroup::canMoveOrAttackInto` | CvSelectionGroup.cpp:2549-2571 | 23 | `group_can_move_or_attack_into` | ported |
| `CvSelectionGroup::canMoveThrough` | CvSelectionGroup.cpp:2574-2598 | 25 | `group_can_move_through` | ported |
| `CvSelectionGroup::canBombard` | CvSelectionGroup.cpp:2644-2659 | 16 | `group_can_bombard` | ported |
| `CvSelectionGroup::getBestBuildRoute` | CvSelectionGroup.cpp:2956-3008 | 53 | `group_get_best_build_route` | ported |
| `CvSelectionGroup::groupDeclareWar` | CvSelectionGroup.cpp:3012-3036 | 25 | `group_declare_war` | ported |
| `CvSelectionGroup::groupAttack` | CvSelectionGroup.cpp:3040-3264 | 225 | `group_attack` | ported |
| `CvSelectionGroup::groupMove` | CvSelectionGroup.cpp:3266-3333 | 68 | `group_move` | ported |
| `CvSelectionGroup::groupPathTo` | CvSelectionGroup.cpp:3337-3399 | 63 | `group_path_to` | ported |
| `CvSelectionGroup::groupRoadTo` | CvSelectionGroup.cpp:3403-3423 | 21 | `group_road_to` | ported |
| `CvSelectionGroup::groupBuild` | CvSelectionGroup.cpp:3427-3497 | 71 | `group_build` | ported |
| `CvSelectionGroup::setTransportUnit` | CvSelectionGroup.cpp:3499-3575 | 77 | `group_set_transport_unit` | ported |
| `CvSelectionGroup::isAmphibPlot` | CvSelectionGroup.cpp:3578-3588 | 11 | `group_is_amphib_plot` | ported |
| `CvSelectionGroup::groupAmphibMove` | CvSelectionGroup.cpp:3592-3661 | 70 | `group_amphib_move` | ported |
| `CvSelectionGroup::getPathFirstPlot` | CvSelectionGroup.cpp:3936-3960 | 25 | `group_get_path_first_plot` | ported |
| `CvSelectionGroup::getPathEndTurnPlot` | CvSelectionGroup.cpp:3963-3990 | 28 | `group_get_path_end_turn_plot` | ported |
| `CvSelectionGroup::generatePath` | CvSelectionGroup.cpp:3993-4020 | 28 | `group_generate_path` | ported |
| `CvSelectionGroup::resetPath` | CvSelectionGroup.cpp:4023-4026 | 4 | `group_reset_path` | ported |
| `CvSelectionGroupAI::AI_getBestGroupAttacker` | CvSelectionGroupAI.cpp:308-417 | 110 | `group_ai_get_best_group_attacker` | ported |
| `CvSelectionGroupAI::AI_getBestGroupSacrifice` | CvSelectionGroupAI.cpp:419-505 | 87 | `group_ai_get_best_group_sacrifice` | ported |
| `CvSelectionGroupAI::AI_queueGroupAttack` | CvSelectionGroupAI.cpp:576-582 | 7 | `group_ai_queue_group_attack` | ported |
| `CvSelectionGroupAI::AI_isDeclareWar` | CvSelectionGroupAI.cpp:600-730 | 131 | `group_ai_is_declare_war` | ported |
| `CvSelectionGroup::getBombardTurns` | CvSelectionGroupAI.cpp:1017-1053 | 37 | `group_get_bombard_turns` | ported |

### `crates/ffh-core/src/group/misc.rs`

C++ lines: **1464** total; 60 functions, all ported.

| C++ function | C++ lines | # | Rust | Status |
|---|---|---:|---|---|
| `CvSelectionGroup::kill` | CvSelectionGroup.cpp:100-109 | 10 | `group_kill` | ported |
| `CvSelectionGroup::sentryAlert` | CvSelectionGroup.cpp:111-180 | 70 | `group_sentry_alert` | ported |
| `CvSelectionGroup::doTurn` | CvSelectionGroup.cpp:182-299 | 118 | `group_do_turn` | ported |
| `CvSelectionGroup::canDoCommand` | CvSelectionGroup.cpp:1804-1844 | 41 | `group_can_do_command` | ported |
| `CvSelectionGroup::canEverDoCommand` | CvSelectionGroup.cpp:1846-1901 | 56 | `group_can_ever_do_command` | ported |
| `CvSelectionGroup::isHuman` | CvSelectionGroup.cpp:2170-2178 | 9 | `group_is_human` | ported |
| `CvSelectionGroup::isBusy` | CvSelectionGroup.cpp:2181-2216 | 36 | `group_is_busy` | ported |
| `CvSelectionGroup::isCargoBusy` | CvSelectionGroup.cpp:2219-2265 | 47 | `group_is_cargo_busy` | ported |
| `CvSelectionGroup::baseMoves` | CvSelectionGroup.cpp:2268-2293 | 26 | `group_base_moves` | ported |
| `CvSelectionGroup::isWaiting` | CvSelectionGroup.cpp:2296-2305 | 10 | `group_is_waiting` | ported |
| `CvSelectionGroup::isFull` | CvSelectionGroup.cpp:2308-2362 | 55 | `group_is_full` | ported |
| `CvSelectionGroup::hasCargo` | CvSelectionGroup.cpp:2365-2381 | 17 | `group_has_cargo` | ported |
| `CvSelectionGroup::getCargo` | CvSelectionGroup.cpp:2383-2397 | 15 | `group_get_cargo` | ported |
| `CvSelectionGroup::canAllMove` | CvSelectionGroup.cpp:2399-2425 | 27 | `group_can_all_move` | ported |
| `CvSelectionGroup::canAnyMove` | CvSelectionGroup.cpp:2428-2447 | 20 | `group_can_any_move` | ported |
| `CvSelectionGroup::hasMoved` | CvSelectionGroup.cpp:2449-2468 | 20 | `group_has_moved` | ported |
| `CvSelectionGroup::canFight` | CvSelectionGroup.cpp:2601-2620 | 20 | `group_can_fight` | ported |
| `CvSelectionGroup::canDefend` | CvSelectionGroup.cpp:2623-2642 | 20 | `group_can_defend` | ported |
| `CvSelectionGroup::visibilityRange` | CvSelectionGroup.cpp:2661-2679 | 19 | `group_visibility_range` | ported |
| `CvSelectionGroup::unloadAll` | CvSelectionGroup.cpp:2681-2694 | 14 | `group_unload_all` | ported |
| `CvSelectionGroup::alwaysInvisible` | CvSelectionGroup.cpp:2696-2720 | 25 | `group_always_invisible` | ported |
| `CvSelectionGroup::isInvisible` | CvSelectionGroup.cpp:2723-2744 | 22 | `group_is_invisible` | ported |
| `CvSelectionGroup::countNumUnitAIType` | CvSelectionGroup.cpp:2747-2772 | 26 | `group_count_num_unit_ai_type` | ported |
| `CvSelectionGroup::hasWorker` | CvSelectionGroup.cpp:2775-2778 | 4 | `group_has_worker` | ported |
| `CvSelectionGroup::setBlockading` | CvSelectionGroup.cpp:2831-2840 | 10 | `group_set_blockading` | ported |
| `CvSelectionGroup::getX` | CvSelectionGroup.cpp:2843-2857 | 15 | `group_get_x` | ported |
| `CvSelectionGroup::getY` | CvSelectionGroup.cpp:2860-2874 | 15 | `group_get_y` | ported |
| `CvSelectionGroup::at` | CvSelectionGroup.cpp:2877-2880 | 4 | `group_at` | ported |
| `CvSelectionGroup::atPlot` | CvSelectionGroup.cpp:2883-2886 | 4 | `group_at_plot` | ported |
| `CvSelectionGroup::plot` | CvSelectionGroup.cpp:2889-2903 | 15 | `group_plot` | ported |
| `CvSelectionGroup::getArea` | CvSelectionGroup.cpp:2906-2920 | 15 | `group_area` | ported |
| `CvSelectionGroup::area` | CvSelectionGroup.cpp:2922-2936 | 15 | `group_area` | ported |
| `CvSelectionGroup::getDomainType` | CvSelectionGroup.cpp:2939-2953 | 15 | `group_get_domain_type` | ported |
| `CvSelectionGroup::readyToSelect` | CvSelectionGroup.cpp:3664-3667 | 4 | `group_ready_to_select` | ported |
| `CvSelectionGroup::readyToMove` | CvSelectionGroup.cpp:3670-3673 | 4 | `group_ready_to_move` | ported |
| `CvSelectionGroup::readyToAuto` | CvSelectionGroup.cpp:3676-3679 | 4 | `group_ready_to_auto` | ported |
| `CvSelectionGroup::getTeam` | CvSelectionGroup.cpp:3700-3708 | 9 | `group_get_team` | ported |
| `CvSelectionGroup::setActivityType` | CvSelectionGroup.cpp:3809-3868 | 60 | `group_set_activity_type` | ported |
| `CvSelectionGroup::isAutomated` | CvSelectionGroup.cpp:3877-3891 | 15 | `group_is_automated` | ported |
| `CvSelectionGroup::setAutomateType` | CvSelectionGroup.cpp:3894-3927 | 34 | `group_set_automate_type` | ported |
| `CvSelectionGroup::clearUnits` | CvSelectionGroup.cpp:4029-4039 | 11 | `group_clear_units` | ported |
| `CvSelectionGroup::addUnit` | CvSelectionGroup.cpp:4043-4099 | 57 | `group_add_unit` | ported |
| `CvSelectionGroup::removeUnit` | CvSelectionGroup.cpp:4102-4120 | 19 | `group_remove_unit` | ported |
| `CvSelectionGroup::deleteUnitNode` | CvSelectionGroup.cpp:4123-4148 | 26 | `group_delete_unit_node` | ported |
| `CvSelectionGroup::mergeIntoGroup` | CvSelectionGroup.cpp:4162-4208 | 47 | `group_merge_into_group` | ported |
| `CvSelectionGroup::splitGroup` | CvSelectionGroup.cpp:4212-4376 | 165 | `group_split_group` | ported |
| `CvSelectionGroup::getUnitIndex` | CvSelectionGroup.cpp:4385-4413 | 29 | `group_get_unit_index` | ported |
| `CvSelectionGroup::getHeadUnit` | CvSelectionGroup.cpp:4421-4433 | 13 | `group_get_head_unit` | ported |
| `CvSelectionGroup::getUnitAt` | CvSelectionGroup.cpp:4435-4452 | 18 | `group_get_unit_at` | ported |
| `CvSelectionGroup::getHeadUnitAI` | CvSelectionGroup.cpp:4455-4467 | 13 | `group_get_head_unit_ai` | ported |
| `CvSelectionGroup::getHeadOwner` | CvSelectionGroup.cpp:4470-4482 | 13 | `group_get_head_owner` | ported |
| `CvSelectionGroup::getHeadTeam` | CvSelectionGroup.cpp:4485-4497 | 13 | `group_get_head_team` | ported |
| `CvSelectionGroup::isAIControl` | CvSelectionGroup.cpp:4763-4779 | 17 | `group_is_ai_control` | ported |
| `CvSelectionGroupAI::AI_isControlled` | CvSelectionGroupAI.cpp:594-597 | 4 | `group_ai_is_controlled` | ported |
| `CvSelectionGroupAI::AI_getMissionAIPlot` | CvSelectionGroupAI.cpp:733-736 | 4 | `group_ai_get_mission_ai_plot` | ported |
| `CvSelectionGroupAI::AI_setMissionAI` | CvSelectionGroupAI.cpp:757-780 | 24 | `group_ai_set_mission_ai` | ported |
| `CvSelectionGroupAI::AI_getMissionAIUnit` | CvSelectionGroupAI.cpp:783-786 | 4 | `group_ai_get_mission_ai_unit` | ported |
| `CvSelectionGroupAI::AI_getGroupflag` | CvSelectionGroupAI.cpp:952-955 | 4 | `group_ai_get_groupflag` | ported |
| `CvSelectionGroupAI::AI_setGroupflag` | CvSelectionGroupAI.cpp:957-966 | 10 | `group_ai_set_groupflag` | ported |
| `CvSelectionGroupAI::isHeadUnit` | CvSelectionGroupAI.cpp:968-975 | 8 | `group_is_head_unit` | ported |

### `crates/ffh-core/src/deal.rs`

C++ lines: **969** total; 28 functions, all ported.

| C++ function | C++ lines | # | Rust | Status |
|---|---|---:|---|---|
| `CvDeal::CvDeal` | CvDeal.cpp:17-20 | 4 | `Deal::new` | ported |
| `CvDeal::init` | CvDeal.cpp:29-41 | 13 | `deal_init` | ported |
| `CvDeal::reset` | CvDeal.cpp:53-64 | 12 | `Deal::new` | ported |
| `CvDeal::kill` | CvDeal.cpp:67-105 | 39 | `deal_kill` | ported |
| `CvDeal::addTrades` | CvDeal.cpp:108-283 | 176 | `deal_add_trades` | ported |
| `CvDeal::doTurn` | CvDeal.cpp:286-320 | 35 | `deal_do_turn` | ported |
| `CvDeal::verify` | CvDeal.cpp:324-373 | 50 | `deal_verify` | ported |
| `CvDeal::isPeaceDeal` | CvDeal.cpp:376-397 | 22 | `deal_is_peace_deal` | ported |
| `CvDeal::isVassalDeal` | CvDeal.cpp:399-402 | 4 | `deal_is_vassal_deal` | ported |
| `CvDeal::isVassalTrade` | CvDeal.cpp:404-418 | 15 | `Deal::is_vassal_trade` | ported |
| `CvDeal::isUncancelableVassalDeal` | CvDeal.cpp:421-498 | 78 | `deal_is_uncancelable_vassal_deal` | ported |
| `CvDeal::isVassalTributeDeal` | CvDeal.cpp:500-511 | 12 | `Deal::is_vassal_tribute_deal` | ported |
| `CvDeal::isPeaceDealBetweenOthers` | CvDeal.cpp:513-540 | 28 | `deal_is_peace_deal_between_others` | ported |
| `CvDeal::startTrade` | CvDeal.cpp:684-888 | 205 | `deal_start_trade` | ported |
| `CvDeal::endTrade` | CvDeal.cpp:891-989 | 99 | `deal_end_trade` | ported |
| `CvDeal::startTeamTrade` | CvDeal.cpp:991-1023 | 33 | `deal_start_team_trade` | ported |
| `CvDeal::endTeamTrade` | CvDeal.cpp:1025-1071 | 47 | `deal_end_team_trade` | ported |
| `CvDeal::isCancelable` | CvDeal.cpp:1073-1090 | 18 | `deal_is_cancelable` | ported |
| `CvDeal::turnsToCancel` | CvDeal.cpp:1092-1095 | 4 | `deal_turns_to_cancel` | ported |
| `CvDeal::isAnnual` | CvDeal.cpp:1098-1114 | 17 | `Deal::is_annual` | ported |
| `CvDeal::isDual` | CvDeal.cpp:1117-1130 | 14 | `Deal::is_dual` | ported |
| `CvDeal::hasData` | CvDeal.cpp:1133-1142 | 10 | `Deal::has_data` | ported |
| `CvDeal::isGold` | CvDeal.cpp:1144-1147 | 4 | `Deal::is_gold` | ported |
| `CvDeal::isVassal` | CvDeal.cpp:1149-1152 | 4 | `Deal::is_vassal` | ported |
| `CvDeal::isEndWar` | CvDeal.cpp:1154-1167 | 14 | `Deal::is_end_war` | ported |
| `CvDeal::getPeaceItem` | CvDeal.cpp:1169-1172 | 4 | `Deal::get_peace_item` | ported |
| `CvDeal::getGoldItem` | CvDeal.cpp:1174-1177 | 4 | `Deal::get_gold_item` | ported |
| `CvDeal::getGoldPerTurnItem` | CvDeal.cpp:1179-1182 | 4 | `Deal::get_gold_per_turn_item` | ported |

## Accessors that are fields

Trivial C++ getters/setters/changers (a plain read, `=` or `+=`, possibly with an `FAssert`)
have no Rust method. Where the C++ setter also refreshes the UI or graphics the table says so.

**CvTeam** (CvTeam.cpp):

| C++ | Rust (`self.teams[t].`...) |
|---|---|
| `getNumMembers` (CvTeam.cpp:2716) | `num_members` |
| `changeNumMembers` (CvTeam.cpp:2722) | `num_members += n` |
| `getAliveCount` (CvTeam.cpp:2729) | `alive_count` |
| `getEverAliveCount` (CvTeam.cpp:2764) | `ever_alive_count` |
| `changeEverAliveCount` (CvTeam.cpp:2776) | `ever_alive_count += n` |
| `getNumCities` (CvTeam.cpp:2783) | `num_cities` |
| `changeNumCities` (CvTeam.cpp:2789) | `num_cities += n` |
| `changeTotalPopulation` (CvTeam.cpp:2824) | `total_population += n` |
| `changeTotalLand` (CvTeam.cpp:2859) | `total_land += n` |
| `getNukeInterception` (CvTeam.cpp:2866) | `nuke_interception` |
| `changeNukeInterception` (CvTeam.cpp:2872) | `nuke_interception += n` |
| `getForceTeamVoteEligibilityCount` (CvTeam.cpp:2879) | `force_team_vote_eligibility_count[vs]` |
| `changeForceTeamVoteEligibilityCount` (CvTeam.cpp:2891) | `force_team_vote_eligibility_count[vs] += n` |
| `getExtraWaterSeeFromCount` (CvTeam.cpp:2898) | `extra_water_see_from_count` |
| `getMapTradingCount` (CvTeam.cpp:2924) | `map_trading_count` |
| `changeMapTradingCount` (CvTeam.cpp:2936) | `map_trading_count += n` |
| `getTechTradingCount` (CvTeam.cpp:2943) | `tech_trading_count` |
| `changeTechTradingCount` (CvTeam.cpp:2955) | `tech_trading_count += n` |
| `getGoldTradingCount` (CvTeam.cpp:2962) | `gold_trading_count` |
| `changeGoldTradingCount` (CvTeam.cpp:2974) | `gold_trading_count += n` |
| `getOpenBordersTradingCount` (CvTeam.cpp:2981) | `open_borders_trading_count` |
| `changeOpenBordersTradingCount` (CvTeam.cpp:2993) | `open_borders_trading_count += n` |
| `getDefensivePactTradingCount` (CvTeam.cpp:3000) | `defensive_pact_trading_count` |
| `changeDefensivePactTradingCount` (CvTeam.cpp:3012) | `defensive_pact_trading_count += n` |
| `getPermanentAllianceTradingCount` (CvTeam.cpp:3019) | `permanent_alliance_trading_count` |
| `changePermanentAllianceTradingCount` (CvTeam.cpp:3036) | `permanent_alliance_trading_count += n` |
| `getVassalTradingCount` (CvTeam.cpp:3043) | `vassal_trading_count` |
| `changeVassalTradingCount` (CvTeam.cpp:3060) | `vassal_trading_count += n` |
| `getBridgeBuildingCount` (CvTeam.cpp:3067) | `bridge_building_count` |
| `changeBridgeBuildingCount` (CvTeam.cpp:3079) | `bridge_building_count += n (C++ side effect is graphics only)` |
| `getIrrigationCount` (CvTeam.cpp:3094) | `irrigation_count` |
| `getIgnoreIrrigationCount` (CvTeam.cpp:3118) | `ignore_irrigation_count` |
| `changeIgnoreIrrigationCount` (CvTeam.cpp:3130) | `ignore_irrigation_count += n` |
| `getWaterWorkCount` (CvTeam.cpp:3137) | `water_work_count` |
| `getVassalPower` (CvTeam.cpp:3160) | `vassal_power` |
| `setVassalPower` (CvTeam.cpp:3165) | `vassal_power = v` |
| `getMasterPower` (CvTeam.cpp:3170) | `master_power` |
| `setMasterPower` (CvTeam.cpp:3175) | `master_power = v` |
| `getEnemyWarWearinessModifier` (CvTeam.cpp:3180) | `enemy_war_weariness_modifier` |
| `changeEnemyWarWearinessModifier` (CvTeam.cpp:3185) | `enemy_war_weariness_modifier += n` |
| `isMapCentering` (CvTeam.cpp:3190) | `map_centering` |
| `setMapCentering` (CvTeam.cpp:3196) | `map_centering = v (C++ side effect is UI only)` |
| `getID` (CvTeam.cpp:3210) | `id` |
| `getStolenVisibilityTimer` (CvTeam.cpp:3216) | `stolen_visibility_timer[o]` |
| `getWarWeariness` (CvTeam.cpp:3272) | `war_weariness[o]` |
| `getTechShareCount` (CvTeam.cpp:3309) | `tech_share_count[i]` |
| `getCommerceFlexibleCount` (CvTeam.cpp:3341) | `commerce_flexible_count[c.idx()]` |
| `changeCommerceFlexibleCount` (CvTeam.cpp:3355) | `commerce_flexible_count[c.idx()] += n (C++ side effect is UI only)` |
| `getExtraMoves` (CvTeam.cpp:3374) | `extra_moves[d.idx()]` |
| `changeExtraMoves` (CvTeam.cpp:3382) | `extra_moves[d.idx()] += n` |
| `isHasMet` (CvTeam.cpp:3391) | `has_met[o]` |
| `isAtWar` (CvTeam.cpp:3496) | `at_war[o]` |
| `setAtWar` (CvTeam.cpp:3504) | `at_war[o] = v` |
| `isPermanentWarPeace` (CvTeam.cpp:3512) | `permanent_war_peace[o]` |
| `setPermanentWarPeace` (CvTeam.cpp:3520) | `permanent_war_peace[o] = v` |
| `isDefensivePact` (CvTeam.cpp:3600) | `defensive_pact[o]` |
| `isForcePeace` (CvTeam.cpp:3645) | `force_peace[o]` |
| `isVassal` (CvTeam.cpp:3679) | `vassal[o]` |
| `isCapitulated` (CvTeam.cpp:4067) | `capitulated` |
| `getRouteChange` (CvTeam.cpp:4075) | `route_change[r]` |
| `changeRouteChange` (CvTeam.cpp:4083) | `route_change[r] += n` |
| `getProjectCount` (CvTeam.cpp:4091) | `project_count[p]` |
| `getProjectMaking` (CvTeam.cpp:4302) | `project_making[p]` |
| `changeProjectMaking` (CvTeam.cpp:4310) | `project_making[p] += n` |
| `getUnitClassCount` (CvTeam.cpp:4319) | `unit_class_count[c]` |
| `changeUnitClassCount` (CvTeam.cpp:4343) | `unit_class_count[c] += n` |
| `getBuildingClassCount` (CvTeam.cpp:4352) | `building_class_count[c]` |
| `changeBuildingClassCount` (CvTeam.cpp:4376) | `building_class_count[c] += n` |
| `getObsoleteBuildingCount` (CvTeam.cpp:4385) | `obsolete_building_count[b]` |
| `getTechCount` (CvTeam.cpp:4511) | `tech_count[t]` |
| `getTerrainTradeCount` (CvTeam.cpp:4519) | `terrain_trade_count[t]` |
| `getRiverTradeCount` (CvTeam.cpp:4559) | `river_trade_count` |
| `getVictoryCountdown` (CvTeam.cpp:4592) | `victory_countdown[v]` |
| `setVictoryCountdown` (CvTeam.cpp:4599) | `victory_countdown[v] = n` |
| `changeVictoryCountdown` (CvTeam.cpp:4607) | `victory_countdown[v] += n` |
| `setCanLaunch` (CvTeam.cpp:4642) | `can_launch[v] = b` |
| `canLaunch` (CvTeam.cpp:4647) | `can_launch[v]` |
| `isNoTradeTech` (CvTeam.cpp:5252) | `no_trade_tech[t]` |
| `setNoTradeTech` (CvTeam.cpp:5260) | `no_trade_tech[t] = v` |
| `getImprovementYieldChange` (CvTeam.cpp:5268) | `improvement_yield_change[i][y.idx()]` |
| `getEspionagePointsAgainstTeam` (CvTeam.cpp:5739) | `espionage_points_against_team[o]` |
| `getEspionagePointsEver` (CvTeam.cpp:5768) | `espionage_points_ever` |
| `setEspionagePointsEver` (CvTeam.cpp:5773) | `espionage_points_ever = v` |
| `changeEspionagePointsEver` (CvTeam.cpp:5781) | `espionage_points_ever += n` |
| `getCounterespionageTurnsLeftAgainstTeam` (CvTeam.cpp:5786) | `counterespionage_turns_left_against_team[o]` |
| `setCounterespionageTurnsLeftAgainstTeam` (CvTeam.cpp:5793) | `counterespionage_turns_left_against_team[o] = v (C++ side effect is UI only)` |
| `changeCounterespionageTurnsLeftAgainstTeam` (CvTeam.cpp:5806) | `counterespionage_turns_left_against_team[o] += n` |
| `getCounterespionageModAgainstTeam` (CvTeam.cpp:5814) | `counterespionage_mod_against_team[o]` |
| `setCounterespionageModAgainstTeam` (CvTeam.cpp:5821) | `counterespionage_mod_against_team[o] = v (C++ side effect is UI only)` |
| `changeCounterespionageModAgainstTeam` (CvTeam.cpp:5834) | `counterespionage_mod_against_team[o] += n` |
| `isBarbarianAlly` (CvTeam.cpp:6274) | `barbarian_ally` |
| `setBarbarianAlly` (CvTeam.cpp:6279) | `barbarian_ally = v` |
| `isNoCivicAnger` (CvTeam.cpp:6284) | `no_civic_anger` |
| `setNoCivicAnger` (CvTeam.cpp:6289) | `no_civic_anger = v` |

**CvTeamAI** (CvTeamAI.cpp):

| C++ | Rust (`self.teams[t].`...) |
|---|---|
| `AI_getWorstEnemy` (CvTeamAI.cpp:2323) | `ai_worst_enemy` |
| `AI_getWarPlanStateCounter` (CvTeamAI.cpp:2364) | `ai_war_plan_state_counter[o]` |
| `AI_setWarPlanStateCounter` (CvTeamAI.cpp:2372) | `ai_war_plan_state_counter[o] = v` |
| `AI_changeWarPlanStateCounter` (CvTeamAI.cpp:2381) | `ai_war_plan_state_counter[o] += n` |
| `AI_getAtWarCounter` (CvTeamAI.cpp:2387) | `ai_at_war_counter[o]` |
| `AI_setAtWarCounter` (CvTeamAI.cpp:2395) | `ai_at_war_counter[o] = v` |
| `AI_changeAtWarCounter` (CvTeamAI.cpp:2404) | `ai_at_war_counter[o] += n` |
| `AI_getAtPeaceCounter` (CvTeamAI.cpp:2410) | `ai_at_peace_counter[o]` |
| `AI_setAtPeaceCounter` (CvTeamAI.cpp:2418) | `ai_at_peace_counter[o] = v` |
| `AI_changeAtPeaceCounter` (CvTeamAI.cpp:2427) | `ai_at_peace_counter[o] += n` |
| `AI_getHasMetCounter` (CvTeamAI.cpp:2433) | `ai_has_met_counter[o]` |
| `AI_setHasMetCounter` (CvTeamAI.cpp:2441) | `ai_has_met_counter[o] = v` |
| `AI_changeHasMetCounter` (CvTeamAI.cpp:2450) | `ai_has_met_counter[o] += n` |
| `AI_getOpenBordersCounter` (CvTeamAI.cpp:2456) | `ai_open_borders_counter[o]` |
| `AI_setOpenBordersCounter` (CvTeamAI.cpp:2464) | `ai_open_borders_counter[o] = v` |
| `AI_changeOpenBordersCounter` (CvTeamAI.cpp:2473) | `ai_open_borders_counter[o] += n` |
| `AI_getDefensivePactCounter` (CvTeamAI.cpp:2479) | `ai_defensive_pact_counter[o]` |
| `AI_setDefensivePactCounter` (CvTeamAI.cpp:2487) | `ai_defensive_pact_counter[o] = v` |
| `AI_changeDefensivePactCounter` (CvTeamAI.cpp:2496) | `ai_defensive_pact_counter[o] += n` |
| `AI_getShareWarCounter` (CvTeamAI.cpp:2502) | `ai_share_war_counter[o]` |
| `AI_setShareWarCounter` (CvTeamAI.cpp:2510) | `ai_share_war_counter[o] = v` |
| `AI_changeShareWarCounter` (CvTeamAI.cpp:2519) | `ai_share_war_counter[o] += n` |
| `AI_getWarSuccess` (CvTeamAI.cpp:2525) | `ai_war_success[o]` |
| `AI_setWarSuccess` (CvTeamAI.cpp:2533) | `ai_war_success[o] = v` |
| `AI_changeWarSuccess` (CvTeamAI.cpp:2542) | `ai_war_success[o] += n` |
| `AI_getEnemyPeacetimeTradeValue` (CvTeamAI.cpp:2548) | `ai_enemy_peacetime_trade_value[o]` |
| `AI_setEnemyPeacetimeTradeValue` (CvTeamAI.cpp:2556) | `ai_enemy_peacetime_trade_value[o] = v` |
| `AI_changeEnemyPeacetimeTradeValue` (CvTeamAI.cpp:2565) | `ai_enemy_peacetime_trade_value[o] += n` |
| `AI_getEnemyPeacetimeGrantValue` (CvTeamAI.cpp:2571) | `ai_enemy_peacetime_grant_value[o]` |
| `AI_setEnemyPeacetimeGrantValue` (CvTeamAI.cpp:2579) | `ai_enemy_peacetime_grant_value[o] = v` |
| `AI_changeEnemyPeacetimeGrantValue` (CvTeamAI.cpp:2588) | `ai_enemy_peacetime_grant_value[o] += n` |
| `AI_getWarPlan` (CvTeamAI.cpp:2594) | `ai_war_plan[o] (write via team_ai_set_war_plan)` |

**CvSelectionGroup** (CvSelectionGroup.cpp):

| C++ | Rust (`self.groups[g].`...) |
|---|---|
| `getID` (CvSelectionGroup.cpp:3682) | `id` |
| `setID` (CvSelectionGroup.cpp:3688) | `id = v` |
| `getOwner` (CvSelectionGroup.cpp:3694) | `owner` |
| `getMissionTimer` (CvSelectionGroup.cpp:3711) | `mission_timer` |
| `setMissionTimer` (CvSelectionGroup.cpp:3717) | `mission_timer = v` |
| `changeMissionTimer` (CvSelectionGroup.cpp:3726) | `mission_timer += n` |
| `isForceUpdate` (CvSelectionGroup.cpp:3791) | `force_update` |
| `setForceUpdate` (CvSelectionGroup.cpp:3797) | `force_update = v` |
| `getActivityType` (CvSelectionGroup.cpp:3803) | `activity_type` |
| `getAutomateType` (CvSelectionGroup.cpp:3871) | `automate_type` |
| `nextUnitNode` (CvSelectionGroup.cpp:4151) | `iterate units` |
| `getNumUnits` (CvSelectionGroup.cpp:4157) | `units.len()` |
| `headUnitNode` (CvSelectionGroup.cpp:4415) | `units.first() / iterate units` |
| `getLengthMissionQueue` (CvSelectionGroup.cpp:4517) | `mission_queue.len()` |
| `getMissionFromQueue` (CvSelectionGroup.cpp:4523) | `mission_queue.get(i)` |
| `nextMissionQueueNode` (CvSelectionGroup.cpp:4590) | `iterate mission_queue` |
| `prevMissionQueueNode` (CvSelectionGroup.cpp:4596) | `iterate mission_queue backwards` |
| `headMissionQueueNode` (CvSelectionGroup.cpp:4602) | `mission_queue.first()` |
| `tailMissionQueueNode` (CvSelectionGroup.cpp:4608) | `mission_queue.last()` |
| `setLastPathPlotVisibility` (CvSelectionGroup.cpp:4786) | `last_plot_visible = v` |
| `isLastPathPlotVisible` (CvSelectionGroup.cpp:4791) | `last_plot_visible` |
| `setLastPathPlotRevealed` (CvSelectionGroup.cpp:4796) | `last_plot_revealed = v` |
| `isLastPathPlotRevealed` (CvSelectionGroup.cpp:4801) | `last_plot_revealed` |

**CvSelectionGroupAI** (CvSelectionGroupAI.cpp):

| C++ | Rust (`self.groups[g].`...) |
|---|---|
| `AI_cancelGroupAttack` (CvSelectionGroupAI.cpp:584) | `ai_group_attack = false` |
| `AI_isGroupAttack` (CvSelectionGroupAI.cpp:589) | `ai_group_attack` |
| `AI_isForceSeparate` (CvSelectionGroupAI.cpp:739) | `ai_force_separate` |
| `AI_makeForceSeparate` (CvSelectionGroupAI.cpp:745) | `ai_force_separate = true` |
| `AI_getMissionAIType` (CvSelectionGroupAI.cpp:751) | `ai_mission_ai_type` |

**CvDeal** (CvDeal.cpp):

| C++ | Rust (`self.deals[d].`...) |
|---|---|
| `getID` (CvDeal.cpp:543) | `id` |
| `setID` (CvDeal.cpp:549) | `id = v` |
| `getInitialGameTurn` (CvDeal.cpp:555) | `initial_game_turn` |
| `setInitialGameTurn` (CvDeal.cpp:561) | `initial_game_turn = v` |
| `getFirstPlayer` (CvDeal.cpp:567) | `first_player` |
| `getSecondPlayer` (CvDeal.cpp:573) | `second_player` |
| `clearFirstTrades` (CvDeal.cpp:579) | `first_trades.clear()` |
| `insertAtEndFirstTrades` (CvDeal.cpp:585) | `first_trades.push(t)` |
| `nextFirstTradesNode` (CvDeal.cpp:591) | `iterate first_trades` |
| `getLengthFirstTrades` (CvDeal.cpp:597) | `first_trades.len()` |
| `headFirstTradesNode` (CvDeal.cpp:603) | `first_trades.first()` |
| `getFirstTrades` (CvDeal.cpp:609) | `&first_trades` |
| `clearSecondTrades` (CvDeal.cpp:615) | `second_trades.clear()` |
| `insertAtEndSecondTrades` (CvDeal.cpp:621) | `second_trades.push(t)` |
| `nextSecondTradesNode` (CvDeal.cpp:627) | `iterate second_trades` |
| `getLengthSecondTrades` (CvDeal.cpp:633) | `second_trades.len()` |
| `headSecondTradesNode` (CvDeal.cpp:639) | `second_trades.first()` |
| `getSecondTrades` (CvDeal.cpp:645) | `&second_trades` |

## Not ported

| C++ function | C++ lines | Reason |
|---|---|---|
| `CvDeal::~CvDeal` | CvDeal.cpp:23-26 | memory management |
| `CvDeal::uninit` | CvDeal.cpp:44-48 | memory management |
| `CvDeal::write` | CvDeal.cpp:651-664 | serialization |
| `CvDeal::read` | CvDeal.cpp:666-679 | serialization |
| `CvSelectionGroup::~CvSelectionGroup` | CvSelectionGroup.cpp:35-38 | memory management |
| `CvSelectionGroup::uninit` | CvSelectionGroup.cpp:56-61 | memory management |
| `CvSelectionGroup::showMoves` | CvSelectionGroup.cpp:301-339 | UI (whether to animate moves for watching humans) |
| `CvSelectionGroup::playActionSound` | CvSelectionGroup.cpp:420-454 | sound |
| `CvSelectionGroup::setupActionCache` | CvSelectionGroup.cpp:1903-1933 | UI action cache |
| `CvSelectionGroup::canDoInterfaceMode` | CvSelectionGroup.cpp:1936-2059 | UI (interface modes) |
| `CvSelectionGroup::canDoInterfaceModeAt` | CvSelectionGroup.cpp:2063-2167 | UI (interface modes) |
| `CvSelectionGroup::IsSelected` | CvSelectionGroup.cpp:2781-2800 | UI selection |
| `CvSelectionGroup::NotifyEntity` | CvSelectionGroup.cpp:2803-2814 | graphics (unit entities) |
| `CvSelectionGroup::airCircle` | CvSelectionGroup.cpp:2817-2828 | graphics (air patrol animation) |
| `CvSelectionGroup::getPathLastNode` | CvSelectionGroup.cpp:3930-3933 | raw FAStar node: use the path module's API |
| `CvSelectionGroup::read` | CvSelectionGroup.cpp:4683-4702 | serialization |
| `CvSelectionGroup::write` | CvSelectionGroup.cpp:4705-4721 | serialization |
| `CvSelectionGroupAI::~CvSelectionGroupAI` | CvSelectionGroupAI.cpp:22-25 | memory management |
| `CvSelectionGroupAI::AI_uninit` | CvSelectionGroupAI.cpp:37-39 | memory management |
| `CvSelectionGroupAI::AI_separate` | CvSelectionGroupAI.cpp:61-79 | AI only (not called from rules code) |
| `CvSelectionGroupAI::AI_seperateNonAI` | CvSelectionGroupAI.cpp:81-101 | AI only (not called from rules code) |
| `CvSelectionGroupAI::AI_seperateAI` | CvSelectionGroupAI.cpp:103-123 | AI only (not called from rules code) |
| `CvSelectionGroupAI::AI_update` | CvSelectionGroupAI.cpp:125-281 | AI only (not called from rules code) |
| `CvSelectionGroupAI::AI_attackOdds` | CvSelectionGroupAI.cpp:285-305 | AI only (not called from rules code) |
| `CvSelectionGroupAI::AI_compareStacks` | CvSelectionGroupAI.cpp:509-540 | AI only (not called from rules code) |
| `CvSelectionGroupAI::AI_sumStrength` | CvSelectionGroupAI.cpp:542-574 | AI only (not called from rules code) |
| `CvSelectionGroupAI::AI_isFull` | CvSelectionGroupAI.cpp:788-848 | AI only (not called from rules code) |
| `CvSelectionGroupAI::AI_ejectBestDefender` | CvSelectionGroupAI.cpp:851-895 | AI only (not called from rules code) |
| `CvSelectionGroupAI::read` | CvSelectionGroupAI.cpp:900-920 | serialization |
| `CvSelectionGroupAI::write` | CvSelectionGroupAI.cpp:923-943 | serialization |
| `CvSelectionGroupAI::AI_GroupPower` | CvSelectionGroupAI.cpp:977-1003 | AI only (not called from rules code) |
| `CvTeam::~CvTeam` | CvTeam.cpp:71-90 | memory management |
| `CvTeam::uninit` | CvTeam.cpp:108-139 | memory management |
| `CvTeam::getProjectDefaultArtType` | CvTeam.cpp:4098-4103 | project art types (graphics) |
| `CvTeam::setProjectDefaultArtType` | CvTeam.cpp:4105-4110 | project art types (graphics) |
| `CvTeam::getProjectArtType` | CvTeam.cpp:4112-4119 | project art types (graphics) |
| `CvTeam::setProjectArtType` | CvTeam.cpp:4121-4128 | project art types (graphics) |
| `CvTeam::isProjectAndArtMaxedOut` | CvTeam.cpp:4145-4165 | project art types (graphics) |
| `CvTeam::finalizeProjectArtTypes` | CvTeam.cpp:4167-4184 | project art types (graphics) |
| `CvTeam::announceTechToPlayers` | CvTeam.cpp:4754-4770 | UI messages only |
| `CvTeam::countNumHumanGameTurnActive` | CvTeam.cpp:5955-5973 | UI only (CvGameInterface) |
| `CvTeam::read` | CvTeam.cpp:6009-6112 | serialization |
| `CvTeam::write` | CvTeam.cpp:6115-6209 | serialization |
| `CvTeam::getCompletedSpaceshipProjects` | CvTeam.cpp:6229-6239 | spaceship screen (exe only) |
| `CvTeam::getProjectPartNumber` | CvTeam.cpp:6241-6261 | spaceship screen / project art (exe only) |
| `CvTeam::hasLaunched` | CvTeam.cpp:6263-6271 | spaceship screen (exe only) |

Data members not ported: `CvTeam::m_paiProjectDefaultArtTypes`, `m_pavProjectArtTypes`
(project art, graphics), `m_aePlayerMembers` (declared with `setHasPlayerMember`,
`isHasPlayerMember`, `getPlayerMemberAt`, `getPlayerMemberListSize`, none of which is
defined or used), `CvSelectionGroup::m_bIsBusyCache` and `m_aDifferentUnitCache` (UI action
cache; `bUseCache` parameters dropped). `CvSelectionGroup::m_lastPlotVisible` /
`m_lastPlotRevealed` (FFH "Alertness") are kept: `groupAttack` and `groupAmphibMove` read
them for human groups, so they are rules state, not camera state.
