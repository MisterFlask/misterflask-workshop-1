# Porting status

The rules port is complete: every rules function of the FFH2 0.41 DLL and rules Python is ported
(no `todo!()` is left in `ffh-core`), and an audit wave has compared every `ffh-core` file and the
`ffh-data` loaders line by line with the C++/Python. Per-class function maps live in
`docs/porting/*.md`; the AI (new code) is described in `docs/AI.md`.

Legend: **done** = fully ported and tested; **ported + audited** = every function ported, then
re-read against the original in the audit wave (see "Audit and stress testing" below).

## Data (ffh-data, CvInfos.cpp read functions)

| Module | Classes | Status |
|---|---|---|
| infos/game.rs | Commerce, GameSpeed, World, Climate, SeaLevel, Era, Handicap, GameOption, Goody, Hurry, Upkeep, CultureLevel, Victory, Process, Emphasize | done, audited |
| infos/civics.rs | Specialist, Religion, Corporation, CivicOption, Civic, Vote, VoteSource, EspionageMission, Project, Tech | done, audited |
| infos/unit.rs | Unit, UnitClass, Promotion, Spell, Build, SpecialUnit, Automate, Command, Mission | done, audited |
| infos/building.rs, civ.rs | Building, BuildingClass, SpecialBuilding, Trait, Civilization, LeaderHead | done, audited |
| infos/terrain.rs, basic.rs | Yield, Terrain, Feature, BonusClass, Bonus, Improvement, Route + 12 basic | done, audited |
| infos/event.rs | Event, EventTrigger | done, audited |
| defines (GlobalDefines) | generated `defines_gen.rs` | done, audited (unlisted text defines read as -1, see DATA.md) |

## Rules engine (ffh-core)

| C++ | Rust | Status |
|---|---|---|
| CvRandom | rng.rs | done (bit-exact, parity-tested) |
| CvStructs.h | structs.rs | done |
| CvDefines.h | defs.rs | done |
| CvMap / CvPlot / CvArea / CvPlotGroup | map.rs, plot/, area.rs, plot_group.rs | ported + audited ([map.md](porting/map.md)) |
| CvUnit | unit/ | ported + audited ([unit.md](porting/unit.md)) |
| CvCity | city/ | ported + audited ([city.md](porting/city.md)) |
| CvPlayer | player/ | ported + audited ([player.md](porting/player.md)) |
| CvTeam / CvSelectionGroup / CvDeal | team/, group/, deal.rs | ported + audited ([team_group_deal.md](porting/team_group_deal.md)) |
| CvGame / CvGameCoreUtils / CvMapGenerator | game/, utils.rs, path.rs, mapgen/ | ported + audited ([game.md](porting/game.md)); `FAStar` (exe) re-implemented in path.rs |
| CvPlayerAI::AI_foundValue (+ bonus values) | found_value.rs | ported + audited (rules need it: start plots, barbarian cities); `AI_baseBonusVal` cached and reset as in C++ |
| CvCityAI::AI_assignWorkingPlots | governor.rs | new code: C++ rules-visible steps kept, choice by the governor |
| CvFractal | mapgen/fractal.rs | done (bit-exact, parity-tested) |
| Map script (BTS Fractal.py) | mapscript.rs | done (re-implementation; Python map scripts are not FFH2 rules) |

## Python (ffh-core/src/ffh)

| Python | Status |
|---|---|
| XML call-expression parser | done (ffh/pycall.rs) |
| CvSpellInterface.py | done, audited (spells_a.rs, spells_b.rs: 242 callbacks) |
| CvEventManager.py (rules handlers) | done, audited (events.rs) |
| CustomFunctions.py | done, audited (custom.rs; AI war policy and UI functions excluded) |
| CvGameUtils.py (enabled callbacks: cannotDoCivic, cannotConstruct, cannotResearch, getBuildingCostMod, canBuild, cannotTrain, ...) | done, audited (game_utils.rs) |
| CvRandomEventInterface.py | done, audited (random_events_a.rs, random_events_b.rs) |
| ScenarioFunctions.py | out of scope (scenarios are not the base game) |

## Audit and stress testing

**Audit wave.** Every `ffh-core` file group and the `ffh-data` loaders were compared line by line
with the original (conditions, constants, defaults, rounding, RNG order, side effects, FfH
blocks); `git log --grep audit` lists the commits. Most groups had no findings. Fixes:

- `CvTeam::setHasTech` / `CvPlayer::chooseTech`: the free-tech popups only reach human players; AI
  players pick research in their own turn (no free tech for AI from Python `chooseTech`).
- Random events: the Iron Orb free tech is for humans only; the Pron events use the player-level
  attitude.
- `spellRoar` stops on a NULL unit, like the Python.
- No `AI_chooseResearch` while research is disabled (FfH Stasis).
- `AI_reset` also clears the `AI_baseBonusVal` cache.
- Text defines not listed in `SetPostGlobalsGlobalDefines` (e.g. `FLOOD_PLAINS_FEATURE`) read as -1
  like C++, so flood plains are removed when a city is founded, as in the original DLL.

**Stress runs.** `ffh-stress` (see README) played about 1,400 AI games over the settings matrix
(game speeds, handicaps, start eras, world sizes, 2 to 35 civs, climates, sea levels, options, teams,
victories), also on a build with debug assertions and overflow checks, with bookkeeping
cross-checks (cities, units, groups, areas, teams) every 5 turns. Debug assertions that the C++
itself violates became `// C++ quirk:` notes, the values kept as in C++: plot city-radius counts
that go negative when a radius-3 city dies, war symmetry for players added mid-game, shared
(`bTeamShare`) building counts for team members added mid-game, and blockade and
specialist-commerce counts that can go negative.

## Not ported (by design)

- C++ AI (CvUnitAI, CvCityAI, CvPlayerAI, CvTeamAI, CvSelectionGroupAI, CvGameAI) and
  the Python AI hooks (`AI_chooseTech`, `AI_chooseProduction`, `AI_MageTurn`,
  `CustomFunctions.warScript`, ...): replaced by the new AI in `ffh-ai`.
- UI, graphics, sound, networking, Python screens, worldbuilder, replays.

## Deliberate deviations

Fidelity bar (CLAUDE.md): rules, formulas, odds and outcomes match the original; RNG-stream parity
and artefacts of C++ container order need not.

**Container order and path finding**

- **Per-owner object lists iterate in creation order.** C++ keeps units, cities, selection groups
  and plot groups (and deals and areas) in `FFreeListTrashArray`s, which iterate in slot order and
  reuse freed slots LIFO; after deletions the two orders differ. This changes only the order of
  RNG draws across a player's cities/units (e.g. the `CvPlayer::doTurn` city loop, unit and group
  turn loops, Python `getUnitList` loops, disband draws) and first-match tie-breaks among equal
  values (`CvCity::updateTradeRoutes`, `killGoldenAgeUnits`). No player-visible rule changes, so
  slot order is deliberately not emulated. Ids are never reused.
- **Diplomatic vote containers** do reproduce the C++ ids and slot order (`doVoteResults` order);
  a new vote takes the highest free slot, which differs from the C++ free list only if a vote
  waits an extra turn and two votes are added in one later turn. `clearSecretaryGeneral` reads
  uninitialized data in C++; the port uses id 0.
- **Path finding** re-implements the exe's closed-source `FAStar` as the A* Explorer algorithm
  over the DLL's own callbacks. Its tie-breaking is deterministic but cannot match the exe: lowest
  total cost first, ties to the most recently opened node, children in `DirectionType` order;
  finders with NULL callbacks cost 1 per step with a `stepDistance`/0 heuristic (only
  reachability and the closed set are used); `bReuse` keeps the tree only when start, info,
  cardinal flag and group match. Details in the `path.rs` module comment.
- **`getCombatOdds`** uses IEEE single precision with the same operations as the source types; the
  original ran on the x87 FPU, so rounding at a 0.5/1000 edge could differ in rare cases (it only
  feeds AI pillage decisions and the combat log).

**Decisions that were popups or AI calls**

- **Controller decisions.** Every C++ AI decision and human popup that rules code waits on goes
  through `Controller` for every player: random-event choices (`choose_event`), diplomatic votes
  and elections (`diplo_vote`, `choose_election`), research and free techs, production,
  conquered cities, trade acceptance (`consider_offer`). Event texts are stored as English
  templates; messages go to the game log.
- **`getTradeDenial`**: the rules-based denials (war/peace feasibility of vassal and surrender
  deals, peace or war with a third team) are ported; the C++ AI's own judgement (attitude, worst
  enemy, valuations, power, victory threats) reads as no denial. Whether an AI accepts a deal is
  `Controller::consider_offer`.
- **Headless.** No active player: mission timers stay 0 and combat is always quick (no
  animation delay); "active player" UI branches never run.
- **Kuriotates settlements**: the human-only `BUTTONPOPUP_CONFIRMSETTLEMENT` (city or settlement)
  has no controller hook yet; a human's new city stays as it is (radius 2, not a settlement), as
  with an unanswered popup. AI players follow the C++ rule (radius 3).
- **Team attitude value** (`CvTeamAI::AI_getAttitudeVal`, used to pick the worst enemy) averages
  the controller's `AttitudeType` (0-4) instead of the C++ raw attitude score.
- **Barbarian city founding unit**: C++ asks `CvCityAI::AI_bestUnitAI(UNITAI_CITY_DEFENSE)`
  (then ATTACK) for the free units of a new barbarian city; the port uses the city's conscript
  unit, the same stand-in FfH itself uses in `CvPlot::setOwner`.
- **Barbarian slot**: the exe (not the DLL) gives it `BARBARIAN_CIVILIZATION` /
  `BARBARIAN_LEADER` and an AI slot status; `GameSetup::standard` does the same.

**AI policy and AI bookkeeping not run by the rules**

- **AI-only Python restrictions are AI policy, not rules.** FFH2's CvGameUtils
  `cannotTrain`, `cannotConstruct`, `cannotCreate`, `cannotResearch` and `canBuild`
  contain `if not pPlayer.isHuman(): ...` branches that stop *AI* players from
  building/researching certain things (e.g. more than one scout, airships, religion
  techs the leader dislikes; in `canBuild`: no forts, no farms under Fallow, workshops need
  Guilds, no cottages under Aristocracy, no windmills under Arete). These encode the old AI's
  build policy, so they are not rules (`py_can_build` always answers -1); `ffh-ai` applies the
  `canBuild` bans itself. All branches that apply to every player are ported.
- **AI-only branches in spell requirements are kept.** Unlike CvGameUtils, the
  `isHuman() == False` checks in CvSpellInterface `req*` functions are ported as written:
  they decide whether a spell is castable at all for AI players (their game, their rules).
- **`CustomFunctions.warScript`/`dogpile`** (AI war declarations called from
  `onBeginPlayerTurn` for AI players) are AI behaviour and are replaced by the new
  AI's diplomacy.
- **Omitted AI calls** (marked `// AI:` in the code): `AI_doFavoriteReligion` is ported
  (`player_ai_do_favorite_religion`) but not run from `doTurn`, so `favorite_religion` stays unset
  and `canConvert`'s AI-only favorite-religion restriction never applies; `AI_nowHasTech`
  (re-choose production after a tech) is omitted; the AI spell casting before `groupAttack`
  (`AI_DirectDamageCast`, `AI_SummonCast`, `AI_BuffCast`) is omitted; `CvUnit::found`'s
  `isUnitAllowedPermDefense` conversion (CvUnitAI bookkeeping) is omitted.

**Small differences**

- **`deleteCity` clears the plots' working-city references** to the dead city (C++ leaves them,
  but `getWorkingCity()` then returns NULL, which is equivalent).
- **Python `getUnitPlayerID`** (CustomFunctions) finds any unit by id; the original only found
  units whose per-player slot was below the unit count, so some lair events silently failed.
- **Unspecified C++ evaluation order** (two RNG draws in one expression) follows MSVC x86:
  call arguments right to left, binary operands left to right (see CLAUDE.md).

## Next steps

1. **Save/load** (in progress in `crates/ffh-core/src/save/`): a round-trip test (save at turn N,
   load, same checksums as an uninterrupted run).
2. **Controller hooks** for the remaining rules-side stand-ins: `confirm_settlement` (Kuriotates
   `BUTTONPOPUP_CONFIRMSETTLEMENT`), the barbarian city defender (`AI_bestUnitAI`), and the
   favorite religion (`AI_doFavoriteReligion`).
3. **AI quality**: see `docs/AI.md` for the current AI and its known gaps.
4. Keep the stress matrix green after rules changes (`ffh-stress batch --matrix all`, also on the
   checked build).
