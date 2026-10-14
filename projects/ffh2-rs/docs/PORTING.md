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
| CvMap / CvPlot / CvArea / CvPlotGroup | map.rs, plot/, area.rs, plot_group.rs | skeleton (docs/porting/map.md) |
| CvUnit | unit/ | skeleton (docs/porting/unit.md) |
| CvCity | city/ | skeleton (docs/porting/city.md) |
| CvPlayer | player/ | skeleton (docs/porting/player.md) |
| CvTeam / CvSelectionGroup / CvDeal | team/, group/, deal.rs | skeleton (docs/porting/team_group_deal.md) |
| CvGame / CvGameCoreUtils / CvMapGenerator | game/, utils.rs, path.rs, mapgen/ | skeleton (docs/porting/game.md) |
| CvFractal | mapgen/fractal.rs | done (bit-exact, parity-tested) |
| Map script (BTS Fractal.py) | mapscript.rs | done (re-implementation; Python map scripts are not FFH2 rules) |

## Python (ffh-core/src/ffh)

| Python | Status |
|---|---|
| XML call-expression parser | done (ffh/pycall.rs) |
| CvSpellInterface.py | todo |
| CvEventManager.py (rules handlers) | todo |
| CustomFunctions.py | todo |
| CvGameUtils.py (enabled callbacks: cannotDoCivic, cannotConstruct, cannotResearch, getBuildingCostMod, canBuild, cannotTrain) | todo |
| CvRandomEventInterface.py | todo |
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
- **`CustomFunctions.warScript`/`dogpile`** (AI war declarations called from
  `onBeginPlayerTurn` for AI players) are AI behaviour and are replaced by the new
  AI's diplomacy.
