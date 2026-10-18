# Civilization-specific AI, group C: Khazad, Kuriotates, Lanun, Ljosalfar, Luchuirp

Sections in the style of docs/AI.md "Civilization-specific", for the orchestrator to integrate.
All numbers are `civ_ab.py` A/Bs: the civilization forced into slot 0, the same seeds on both
sides, base = `2d95e9c`, 6 civs, standard map. "Full games" play to a decision or turn 690
(`--until-supremacy`); share is score v2 in basis points of the world (dead = 0; fair share 1667),
legacy is FFH2's score; t is the paired t over seeds, win differences are the discordant seeds
(won by one side only). Each experiment was measured *alone* (a tree with only that civilization's
change), because a change to one civilization moves every game it is an opponent in: the first
round, measured together, showed the Kuriotates' turn-200 share move by -80 (t -2.0) in exactly the
games that had Luchuirp opponents.

## Kept

- **Kuriotates** (`civs/kuriotates.rs`): past their limit of full cities (3 on a standard map) a
  new city is a settlement (radius 2, -75% yields, nothing to build, no maintenance). The base AI
  builds settlement settlers from full cities of size 5+ (docs/AI.md: +16% score at turn 300). In
  full games that logic *hurts* (base AI with and without it, seeds 1-40): population at turn 300
  41 without against 37 with, commerce 159 against 117, research 158 against 111, and at the end
  world share 1385 against 1095 (t -1.8), alive 0.8 against 0.6 (t -2.2) -- though share at turn
  200 is +202 with it (t 4.9: the land). The mechanism: a plot is worked by the city nearest to it
  (`CvPlot::updateWorkingCity`, ring priority, older city on ties), so a settlement placed 4-5
  plots from a full city takes the plots of the full city's third ring it is closer to, and a
  size-1 settlement works one of them at a quarter of its yield. What the AI does now:
  - at the limit, no settlement site within 5 plots (a full city's radius plus a settlement's) of
    a full city (new hook `CivPlay::site_value`, called in `sites::compute`);
  - settlements cannot train units: the nearest full city trains a defender for a settlement that
    has none (unless one is on its way), when one more unit is affordable;
  - after losing a full city, the next city founded is a full one (`CvCity::init` counts full
    cities), but the generic settler step counts settlements towards the limit and never builds
    another: a settler for a full city then (one at a time, size 4+, a site on the landmass).

  Full games, this change alone: seeds 1-40 win 5.1% -> 5.1% (0/0), share@300 1761 -> 1830 (+69,
  t 1.07), share@end 1141 -> 1185 (+45, t 0.26), legacy@300 +37 (t 0.97), alive 0.6 -> 0.7;
  seeds 41-80 win 2.5% -> 12.5% (won only by the new side 4, by the base 0, p 0.125), share@300
  1648 -> 1715 (+68, t 1.38), share@end 957 -> 1006 (+48, t 0.30). By turn 300 the full cities are
  bigger (population 41 against 37, production 40 against 34). The Kuriotates' late decline is not
  fixed: they still lose cities to conquest after turn 400 (5-7 lost by turn 450-500 in both
  versions) with three producing cities against rivals' seven.

  Tried, not kept: no settlements at all (seeds 1-40: share@300 -183, t -1.9; share@end +3);
  settlement sites away from full cities without the garrisons (share@end -4 on 1-40, +16 on
  41-80, share@300 +81 and +72); one more defender in every full city (share@end -318, t -2.0;
  unit support); the full-city rebuild alone fires in a few games, late (seeds 1-40: identical
  to the base through turn 300, share@end -64, t -1.05; kept with the rest because it repairs a
  real dead end -- seed 6 of the base: one full city and eight settlements from turn 233 on, no
  settler possible).

  Legends (world spell, +300 culture in every city; AI gate: 5 cities, settlements counted) is
  cast at about turn 200 by the generic end-of-turn choice, unchanged.

## Tried, nothing kept

- **Khazad** (`civs/khazad.rs`): their siege engines (Trebuchet, Catapult class, 100% collateral,
  80% withdrawal; Dwarven Cannon) need a Siege Workshop and their knight (Hornguard, 12 strength,
  no horses) a Stable -- specific buildings (`PrereqBuilding`) that the generic production never
  builds (`enabler_building` and `campaign::siege_building` only read `PrereqBuildingClass`).
  Tried: the Workshop in the most productive city when the operation wants siege engines and no
  city can train one (as `siege_building` does), a Stable where it opens a unit a third stronger
  (as `enabler_building`). Trebuchets and Cannons are built (2.5 and 1.4 a game at the end,
  Workshops 0.4); no Hornguard ever (the Stable needs Warhorses, which research never values:
  `research::unit_available` skips `PrereqBuilding` units). Full games, seeds 1-40: win 28.2% ->
  23.1% (1 vs 3), share@300 -6 (t -0.4), share@end +50 (t 0.36), alive +0.1 (t 1.4). Not kept.
  Mother Lode (world spell; AI gate: 10 mines) is cast at about turn 250 in 28 of 39 games,
  unchanged.
- **Luchuirp** (`civs/luchuirp.rs`): the Wood Golem (Axeman class, 6/6, 90 hammers) and the
  Gargoyle (Longbowman class, 8/8, +25% city defense) need a Sculptor's Studio (`PrereqBuilding`);
  the Studio is also the Luchuirp Training Yard, so the generic logic builds it late for the
  Clockwork Golem (3.3 Studios a game at the end in the base). Tried: the Studio where it opens a
  unit a third stronger. With both golems (300 turns, seeds 1-40): Studios 0.3 -> 3.6, Wood Golems
  1 -> 10, Dwarven Slingers 19.5 -> 12.6, share@300 -91 (t -1.28), legacy@200 -27 (t -2.47): the
  golems (no promotions, slow healing) replaced the slingers as defenders, and the Studios cost
  markets. Gargoyle only, full games: win 25% -> 20% (2 vs 4), share@300 +7, share@end -86
  (t -0.8), Gargoyles 2.2 -> 4.3. Not kept. Note: once the Studio exists it also opens the Clan of
  Embers' Ogre (Training Yard class, `UpgradeCiv`; cross-cutting finding 4), which the Luchuirp
  then train instead of golems. Gifts of Nantosuelta (AI gate: 5 cities) is cast at about turn 190,
  unchanged.
- **Lanun** (`civs/lanun.rs`): the diagnosis's first proposal (research Fishing early) is already
  the case: traced seeds 1-8 research Fishing at turns 30-65 (one at 110), and the base keeps 4-9
  coves, harbors and ports a game, nearly all worked. Tried: (1) FfH's end-of-turn spell choice
  casts the cove (AI weight 300) also from a boat sailing to a sea resource; the cove left to boats
  without orders (0-14 prevented casts a game). Seeds 1-40 full games: share@300 +34 (t 1.39),
  legacy@300 +34 (t 2.19), share@end +84 (t 0.56), win 40% -> 37.5% (3 vs 4). Seeds 41-80: win
  35% -> 30% (1 vs 3), share@300 -58 (t -1.75), share@end -83 (t -0.71), legacy@300 -37. Reverted.
  (2) A Siege Workshop where it opens the War Tortoise (Knight class, 11/19, walks on water,
  `PrereqBuilding`): 0.3 Tortoises a game, results identical through turn 300 and -8 at the end
  (t -1.4). Not kept. Raging Seas is cast in 5 of 40 games, at about turn 590 (AI gate below).
- **Ljosalfar** (`civs/ljosalfar.rs`): `MaintainFeatures` keeps the forest under their
  improvements, but the build time still adds the forest's removal time (cottage 400 + 250 on
  Forest, + 500 on Ancient Forest) and the Elf promotion is -20% work rate: about twice the
  worker-turns per improvement. The base Ljosalfar have as many workers as others but fewer
  improvements (18.7 against 26.0 at turn 200) and less commerce (47 against 64). Tried: workers
  to two per three unimproved worked plots (the generic target has one per three), from a
  garrisoned city at peace. 300 turns, seeds 1-40: improvements +1.4 at turn 150 and +3.2 at 200,
  but fewer cities and less population: legacy@100 -14.5 (t -2.48), legacy@200 -37.5 (t -2.46),
  share@300 -8. Only once no site is left on the landmass (full games): win 27.5% -> 25% (1 vs 2),
  share@300 -46 (t -1.73), share@end -17. Not kept. March of the Trees is cast in 16 of 40 games,
  at about turn 460 (AI gate below).

## World spells: the AI-only gates and the timing I would choose

| civ | spell | AI gate (ported `req*`, rules code) | base AI: cast in / first seen | with the gate open | how much it would matter |
|---|---|---|---|---|---|
| Khazad | Mother Lode | 10 mines | 28/39 games, ~t250 | cast when the windfall (25 gold a mine) would lift the Dwarven Vault a level (50 gold per city), or at 10 mines when that never comes; the hills conversion favours early | small: 250-500 gold once, ~10% more hills |
| Kuriotates | Legends | 5 cities (settlements count) | 36/40, ~t200 | right after the third full city is founded (its borders need the culture most), not waiting for settlements | small-medium: borders of the radius-3 cities 50-100 turns sooner |
| Lanun | Raging Seas | at war with a civ whose weighted fleet is above 10 | 5/40, ~t590 | the turn before our stack strikes a coastal enemy city (its defenders on a coast plot take ~35-50% damage), or when an enemy stack stands on our coast | medium-large in a Lanun war; today it is effectively never available |
| Ljosalfar | March of the Trees | at war with 2+ civ teams | 16/40, ~t460 | at war with one civ, when one of our cities is in peril and 8+ of our forest plots lie within 2 plots of the threat (str-10 Treants for 5 turns) | large when invaded; the AI fights one war at a time, so the gate almost never opens |
| Luchuirp | Gifts of Nantosuelta | 5 cities | 35/40, ~t193 | after the last planned city is founded (each city's hammer becomes a free Engineer, +2 hammers forever) | small |

Raging Seas' and March of the Trees' gates are AI heuristics in the original Python (`if not
human:` branches), not rules; the other three are mild and close to what an AI would choose.

## Findings for the orchestrator (not fixed here)

1. **ffh-play T1 test premise** (`crates/ffh-play/tests/bot_game.rs`): `GAMEOPTION_NO_BARBARIANS`
   does not remove every barbarian. Barbarian units spawned by FfH's Python still exist (e.g. the
   Dragon's Horde in a barbarian city placed at map creation sends Sons of the Inferno; barbarian
   warriors roam). In seed 2 the bot (Kuriotates) loses cities to them even on the base (3 founded,
   1 left at turn 150); the first commit on this branch changed the opponents' play enough that it
   loses the last one at turn 142, and the test's `Stop::Human` assertion failed. The test now
   accepts `Stop::Over` (a stuck turn is still an error). Whether NO_BARBARIANS should stop those
   spawns is a rules question.
2. **Workers stuck on a move the path finder refuses** (generic, every civ): the worker logic
   chooses a target by its own danger map, but `MISSION_MOVE_TO` with `MOVE_SAFE_TERRITORY` is
   refused by the rules' AI path finder (its `AI_getPlotDanger` check), and the worker retries the
   same target every turn. Traced: a Lanun work boat at (42,23), seed 7, turns 200-239, one plot
   from its target fish; `group_can_start_mission(MOVE_TO)` true, `group_generate_path` false even
   with no flags. The flee code already passes `MOVE_IGNORE_DANGER` for this reason; the build move
   does not.
3. **`research::unit_available` skips every `PrereqBuilding` unit**, so research never values
   Warhorses for the Hornguard or Engineering for the Gargoyle (cross-cutting finding 1).
4. **Kuriotates and C++ `AI_foundValue`** (rules code, faithful): the 37-plot/9-plot site switch
   compares `getNumCities()` (settlements included) with the limit, so after losing a full city
   the replacement site is still scored as a settlement-sized one. Legends' and Gifts' AI gates
   also count settlements as cities.
5. **Foreign units through `UpgradeCiv`** (cross-cutting finding 4) shape these results: the
   Luchuirp Studio (their Training Yard) opens Clan Ogres; Khazad and Lanun field Centaurs and Ogres
   by the dozen in full games. A fix there changes what the Studio, Stable and Workshop
   experiments above are worth (the golems, Hornguard and War Tortoise would stop competing with
   other civilizations' units), so they are worth re-measuring after it.
