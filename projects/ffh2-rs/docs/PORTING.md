# Porting status

Legend: **done** = fully ported and tested; **skeleton** = structs + signatures,
bodies `todo!()`; **todo** = not started. Per-class function maps live in
`docs/porting/*.md`.

## Data (ffh-data, CvInfos.cpp read functions)

| Module | Classes | Status |
|---|---|---|
| infos/game.rs | Commerce, GameSpeed, World, Climate, SeaLevel, Era, Handicap, GameOption, Goody, Hurry, Upkeep, CultureLevel, Victory, Process, Emphasize | done |
| infos/civics.rs | Specialist, Religion, Corporation, CivicOption, Civic, Vote, VoteSource, EspionageMission, Project, Tech | done |
| infos/unit.rs | Unit, UnitClass, Promotion, Spell, Build, SpecialUnit, Automate, Command, Mission | done |
| infos/building.rs, civ.rs | Building, BuildingClass, SpecialBuilding, Trait, Civilization, LeaderHead | done |
| infos/terrain.rs, basic.rs | Yield, Terrain, Feature, BonusClass, Bonus, Improvement, Route + 12 basic | done |
| infos/event.rs | Event, EventTrigger | done |

## Rules engine (ffh-core)

| C++ | Rust | Status |
|---|---|---|
| CvRandom | rng.rs | done (bit-exact, parity-tested) |
| CvStructs.h | structs.rs | done |
| CvDefines.h | defs.rs | done |
| CvMap / CvPlot / CvArea / CvPlotGroup | map.rs, plot/, area.rs, plot_group.rs | ported (all of map.md) |
| CvUnit | unit/ | lifecycle.rs ported; movement, combat in progress; promotions, actions, missions, ffh todo |
| CvCity | city/ | turn.rs + production.rs in progress; citizens, buildings, yields_culture todo |
| CvPlayer | player/ | mod.rs, turn.rs, start.rs ported; economy in progress; units, cities, civics_religion, diplomacy_trade, events todo |
| CvTeam / CvSelectionGroup / CvDeal | team/, group/, deal.rs | team tech+diplomacy, group (all) in progress; team misc, deal todo |
| CvGame / CvGameCoreUtils / CvMapGenerator | game/, utils.rs, path.rs, mapgen/ | setup_init.rs + mapgen ported; turn.rs, utils.rs, path.rs in progress; world.rs, votes.rs todo |
| CvPlayerAI::AI_foundValue (+ bonus values) | found_value.rs | ported (rules need it: start plots, barbarian cities); `AI_baseBonusVal` is recomputed instead of cached between resets |
| CvCityAI::AI_assignWorkingPlots | governor.rs | new code: C++ rules-visible steps kept, choice by the governor |
| CvFractal | mapgen/fractal.rs | done (bit-exact, parity-tested) |
| Map script (BTS Fractal.py) | mapscript.rs | done (re-implementation; Python map scripts are not FFH2 rules) |

## Python (ffh-core/src/ffh)

| Python | Status |
|---|---|
| XML call-expression parser | done (ffh/pycall.rs) |
| CvSpellInterface.py | done (spells_a.rs, spells_b.rs: 242 callbacks) |
| CvEventManager.py (rules handlers) | in progress |
| CustomFunctions.py | done (custom.rs; AI war policy and UI functions excluded) |
| CvGameUtils.py (enabled callbacks: cannotDoCivic, cannotConstruct, cannotResearch, getBuildingCostMod, canBuild, cannotTrain, ...) | done (game_utils.rs) |
| CvRandomEventInterface.py | in progress (random_events_a.rs, random_events_b.rs) |
| ScenarioFunctions.py | out of scope (scenarios are not the base game) |

## Not ported (by design)

- C++ AI (CvUnitAI, CvCityAI, CvPlayerAI, CvTeamAI, CvSelectionGroupAI, CvGameAI) and
  the Python AI hooks (`AI_chooseTech`, `AI_chooseProduction`, `AI_MageTurn`,
  `CustomFunctions.warScript`, ...): replaced by the new AI in `ffh-ai`.
- UI, graphics, sound, networking, Python screens, worldbuilder, replays.

## Deliberate deviations

- **AI-only Python restrictions are AI policy, not rules.** FFH2's CvGameUtils
  `cannotTrain`, `cannotConstruct`, `cannotCreate`, `cannotResearch` and `canBuild`
  contain `if not pPlayer.isHuman(): ...` branches that stop *AI* players from
  building/researching certain things (e.g. more than one scout, airships, religion
  techs the leader dislikes). These encode the old AI's build policy; the new AI
  makes its own choices, so these branches are not ported. All branches that apply
  to every player are ported.
- **AI-only branches in spell requirements are kept.** Unlike CvGameUtils, the
  `isHuman() == False` checks in CvSpellInterface `req*` functions are ported as written:
  they decide whether a spell is castable at all for AI players (their game, their rules).
- **Unspecified C++ evaluation order** (two RNG draws in one expression) follows MSVC x86:
  call arguments right to left, binary operands left to right (see CLAUDE.md).
- **`AI_baseBonusVal` cache**: C++ keeps the first value per bonus until `AI_updateBonusValue`
  resets it (each turn, team merge, empire split); the port recomputes it, so values can
  differ only after cities/techs change between two resets.
- **Barbarian slot**: the exe (not the DLL) gives it `BARBARIAN_CIVILIZATION` /
  `BARBARIAN_LEADER` and an AI slot status; `GameSetup::standard` does the same.
- **Path finding** replaces the exe's closed-source `FAStar` with an A* over the same DLL
  callbacks; tie-breaking between equal-cost paths cannot match the original exactly.
- **Python `getUnitPlayerID`** (CustomFunctions) finds any unit by id; the original only found
  units whose per-player slot was below the unit count, so some lair events silently failed.
- **`CustomFunctions.warScript`/`dogpile`** (AI war declarations called from
  `onBeginPlayerTurn` for AI players) are AI behaviour and are replaced by the new
  AI's diplomacy.
