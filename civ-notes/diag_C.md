# Diagnosis C: Khazad, Kuriotates, Lanun, Ljosalfar, Luchuirp

Read-only diagnosis. Nothing was built or run. Code references are to the tree at
`2d95e9c` ("one module per civilization behind a CivPlay trait"), which landed while this was
being written: the old `crates/ffh-ai/src/civs.rs` is now `civs/mod.rs` plus one module per
civilization. The hooks are `production`, `turn`, `unit`, `sea_worker`, `spell_value`,
`building_value` and `tech_value`, and `FFH_AI_CIV_OFF` turns a civ's logic off. Paths below
are relative to `projects/ffh2-rs/`. "C++" means `/home/user/ref/ffh2/CvGameCoreDLL/` and
"Py" means `/home/user/ref/ffh2/Assets/python/`.

**Observational data.** No games were played for this report. I mined existing
`ffh stats --detail` outputs that another agent left in
`scratchpad/census/base_{amurites,balseraphs,bannor,calabim}.json`: 200 full games
(`--until-supremacy`, 6 civs, standard map, base AI at 88ac708/2d95e9c, which play the same).
My five civs appear in them as random opponents: Khazad 46 player-games, Kuriotates 39,
Lanun 66, Ljosalfar 99, Luchuirp 82. Snapshots come every 50 turns, so a unit that lives
less than 50 turns can be missed. The scripts are in `scratchpad/diagC_tools/`
(`census.py`, `foreign.py`, `wins.py`, `kurio.py`, `econ.py`, `more.py`).

| civ (n) | win | alive at end | share at end | research t200 / t300 | prod t300 / t400 | pop t300 / t400 | cities lost to civs t400 |
|---|---|---|---|---|---|---|---|
| Khazad (46) | 0.24 | 0.93 | 2399 | 85 / 180 | 72 / 103 | 57 / 76 | 0.86 |
| Kuriotates (39) | **0.05** | **0.54** | 937 | 75 / 134 | **37 / 38** | 45 / 49 | **2.67** |
| Lanun (66) | 0.27 | 0.86 | 1878 | 79 / 176 | 62 / 88 | 62 / 104 | 1.43 |
| Ljosalfar (99) | 0.24 | 0.71 | 1408 | **28** / 99 | 69 / 84 | 49 / 71 | 1.77 |
| Luchuirp (82) | 0.13 | 0.77 | 1703 | 53 / 173 | 71 / 100 | 60 / 82 | 1.47 |
| all other civs | 0.16 avg | | | 42 / 130 | 63 / 72 | 53 / 73 | 1.46 |

---

## Part 1. Cross-civ findings

These are generic AI gaps, plus one rules quirk. They decide more about these five civs than
most civ-specific mechanics do.

### G1. Units whose `PrereqBuilding` is a specific building can never be enabled. The AI never builds the Siege Workshop.

- **Rules.** FfH units name either a building class (`PrereqBuildingClass`: Archery Range,
  Training Yard, Stable for horsemen) or a specific building (`PrereqBuilding`). Each city that
  trains the unit must have it (`crates/ffh-core/src/plot/units.rs:1406-1438`, port of
  `CvPlot::canTrain`). The data:
  - Siege Workshop: Catapult, Trebuchet (Khazad), Cannon, Dwarven Cannon (Khazad), Chariot,
    War Tortoise (Lanun Knight class), War Machine.
  - Stable: Knight, Hornguard (Khazad/Luchuirp Knight class).
  - Sculptor's Studio: Wood Golem, Gargoyle (Luchuirp).
  - Grove: Druid, Dwarven Druid, Myconid, Beastmaster.
  - Temples: the religions' priests.
- **AI.** `production::enabler_building` (`crates/ffh-ai/src/production.rs:777-829`, the
  `let Some(bc) = info.prereq_building_class else { continue }` at 786) and
  `campaign::siege_building` (`crates/ffh-ai/src/campaign.rs:1183-1216`, the same test at 1207)
  only look at `prereq_building_class`. The generic `building_value` of the Siege Workshop is
  0: it has no yields or other effects. Step 9 of `decide` skips `value <= 0`
  (`production.rs:600`), so the workshop is only reachable through `fallback`. FfH's own AI
  valued both kinds of enabler (`CvCityAI::AI_buildingValue`, `CvCityAI.cpp:3613`:
  `getPrereqBuilding() == eBuilding || getPrereqBuildingClass() == ...`).
  `research::unit_available` also skips every `prereq_building` unit (`research.rs:66`), so
  research never values a Trebuchet, Wood Golem or Gargoyle.
- **Observed.** In 1353 civ player-games, 2 ever had a Siege Workshop (both Luchuirp) and 1
  ever had a siege unit. Across the five civs, Trebuchet, Dwarven Cannon, War Tortoise and
  Hornguard never appear. Stables are rare: Khazad 2%, Lanun 20%, Ljosalfar 8%.
- **Value: big.** This is where the Khazad and Lanun unique units sit, and the existing
  campaign logic already asks for siege (`army_choice` wants `f.siege`) but can never get it.
- **Proposal.**
  - In `siege_building`, also accept `info.prereq_building`. The building is then
    `info.prereq_building` itself; check `city_can_construct`. This keeps the existing gating:
    one city (the most productive), only when the operation needs siege.
  - In `enabler_building`, also accept `prereq_building`, keeping the 133% gain test and
    possibly excluding units whose default AI is MAGE or EXPLORE (Grove druids, 300 hammers).
  - In `research::unit_available`, count `prereq_building` units whose building the civ can
    construct.
- **Risk.** The guns-vs-butter history (docs/AI.md): siege production competes with the
  army and with economy buildings. Hook it only to `needed_force().siege > 0`, as it is now.
  Check `ffh stats --detail` `building_types.SIEGE_WORKSHOP` and `ucombat.SIEGE`. This is a
  generic change: measure it with `civ_ab.py --civ all`.

### G6. Every civ trains other civs' units (the Kuriotates Centaur line, Clan Ogres and Goblins). Rules quirk, AI exploit.

- **Rules.**
  - FfH moved the "this civ's unit for the class" check from `CvPlayer::canTrain`
    (`CvPlayer.cpp:5745-5750`, commented out) to `CvCity::canTrain` (`CvCity.cpp:1928-1944`).
    There, a unit with `UpgradeCiv` set only needs to be that civ's unit of its class, so any
    city may train it.
  - The port is faithful (`crates/ffh-core/src/city/production.rs:180-205`).
  - Affected units: Centaur, Centaur Archer, Centaur Charger, Centaur Lancer (Kuriotates);
    Ogre, Wolf Rider, Ogre Warchief, Stoneskin Ogre, Goblin (Clan of Embers); Vampire,
    Vampire Lord, Brujah (Calabim); Balor.
  - FfH never exposed this. Its city screen and its AI enumerate only
    `getCivilizationUnits(class)` for the city's civilization (`CvMainInterface.py:1671`,
    `CvCityAI.cpp:2567-2569`).
- **AI.** `production::best_unit` (`production.rs:836`) and `strongest_unit` iterate all
  `d.units` and keep whatever `city_can_train` allows. There is no `d.civ_unit(civ, class) == u`
  filter. `diplomacy::best_trainable_attack` (`diplomacy.rs:521`) is the same.
- **Observed.** Share of a civ's units at turn 300 that belong to another civ:

  | civ | foreign share of units, t300 | players that ever fielded |
  |---|---|---|
  | Khazad | 19% | Ogres 91%, Centaurs 85%, Centaur Archers 85% |
  | Lanun | 17% | Centaurs 84% |
  | Luchuirp | 16% | Centaur Chargers 77%, Centaurs 73% |
  | Ljosalfar | 7% | |
  | Hippus | 20% | |

  At turn 300 the Luchuirp field 4.3 Centaur Chargers, 2.4 Centaurs and 1.9 Centaur Archers
  against 0.9 Wood Golems. The loophole also bypasses the Stable (Centaurs need none) and the
  Siege Workshop (the Centaur Charger is the Chariot class), which partly hides G1.
- **Value: big for balance.** It removes the Kuriotates' main unique-unit edge (their win rate
  is the lowest of all civs, 0.05) and props up the others. It is a rules question for the user:
  - either the AI filters to the civ's own unit per class, which is what FfH's AI and UI do;
  - or `CvCity::canTrain` gets a deliberate rules change (owner or city civ must match).

  Do not silently "fix" it in only one place. Fixing it changes every civ's army: measure with
  `civ_ab.py --civ all`. It interacts with G1 (once Centaurs are gone, Stables and Studios
  matter much more).

### G2. Spells valued only by `iAIWeightWar` / `iAIWeightCity` are never cast (affects Luchuirp Repair)

- `casting::choose_spell` adds only `info.ai_weight` (`casting.rs:187`), as C++ `chooseSpell`
  does (`CvUnit.cpp:16079`). FfH cast `iAIWeightWar` spells from `CvUnitAI::AI_mageCast`
  (`CvUnitAI.cpp:21767-21791`), which is not ported (PORTING.md "Omitted AI calls").
- Spells with only those weights: Repair, Enchanted Blade, Flaming Arrows, Rust, Slow, Blur,
  Dance of Blades, Mutation, Hope, Inspiration, Wall of Stone, Add to City. The buffs among
  them still go through `buff_cast`; non-buff spells such as Repair are never cast.
- **Hook:** `CivPlay::spell_value` is called for every castable spell with FfH's value, and may
  return a positive value for a spell FfH valued at 0 (`casting.rs:190`). See Luchuirp L7.

### G3. AI-only gates in world-spell requirements

The `if not human:` branches of the `req*` functions were kept on purpose (PORTING.md "AI-only
branches in spell requirements are kept"). They fix when, or whether, the AI may cast:

| spell | gate | cast in (census) |
|---|---|---|
| Mother Lode | 10 mines | 41/46 Khazad |
| Legends | 5 cities | 38/39 Kuriotates |
| Gifts of Nantosuelta | 5 cities | 74/82 Luchuirp |
| Raging Seas | enemy weighted fleet > 10 | **9/66 Lanun, first seen t400-690** |
| March of the Trees | at war with 2+ civ teams | **23/99 Ljosalfar, mean first seen t457** |

The same kind of `isHuman` branches in CvGameUtils were treated as AI policy and dropped.
For the last two spells the gate means the AI effectively never gets the spell. Lifting it is a
rules decision for the user (see Lanun N2 and Ljosalfar E1). The new AI would then own the
timing through `CivPlay::turn` and `spell_value`.

### G4. Minor: every civ values cottages as if they grow into Kuriotates Enclaves

`workers.rs:161` and `research.rs:499` follow `improvement_upgrade` four steps:
cottage, hamlet, village, town, then Town → `IMPROVEMENT_ENCLAVE` (`PrereqCivilization`
Kuriotates; 1/0/5 against the Town's 0/0/4). Non-Kuriotates plots never upgrade to it
(`crates/ffh-core/src/plot/terrain.rs:190-212`). The grown form is over-valued by about
+1 food and +1 commerce for every civ except the Kuriotates. Fix: stop the chain at an upgrade
whose `prereq_civilization` is set and differs from ours.

### G5. Kuriotates only: after losing a full city, nobody builds the replacement

`expansion::settler_choice` stops at `ctx.n_cities + settlers >= max_cities`
(`expansion.rs:77-78`), and `n_cities` counts settlements. `kuriotates::settlement_settler`
requires `full >= max_cities` (`civs/kuriotates.rs`). A Kuriotates civ with 2 full cities plus
settlements builds no settler at all. Its next city would be full (radius 3), and Promote
Settlement is human-only (no `bAllowAI`). Fix: in the generic check compare
`n_cities - num_settlements` with the limit, or handle it in `Kuriotates::production`. Value is
small but the fix is cheap. Kuriotates lose 2.67 cities to civs by t400 (others 1.46), so the
case does occur.

---

## Part 2. Per civilization

Value scale: big = can change who wins; medium = several % of production, research or power;
small = cosmetic or rare.

### KHAZAD (win 0.24, alive 0.93; strong economy, research 180 vs 130 at t300)

**K1. Mother Lode (world spell)**
- **Rules.** `spells_b.rs:326-353` (`CvSpellInterface.py:1898-1915`); XML
  `CIV4SpellInfos.xml:1988` (Mining, `bGlobal`, `iAIWeight` 300). AI gate: at least 10
  `IMPROVEMENT_MINE`. Effect: +25 gold per mine, and each owned flat land plot (not water,
  peak or hills) has a 10% chance to become hills (-1 food, +1 production, dwarves double move,
  +25% defense).
- **AI.** Any Khazad unit casts it at end of turn (value 300) as soon as the gate passes.
  Observed: cast in 41/46 games, first seen t100-250 (mean 207). **Handled adequately.**
- **Value: small-medium.** 250-500 gold once, plus about 10% more hills.
- **Proposal (optional), `Khazad::spell_value`.** Return 0 until mines ≥ max(10, 2 × cities),
  or until the windfall would lift the Dwarven Vault a level (gold + 25 × mines ≥ 50 × cities);
  pair it with K2's hold. Risk: delay costs compounding, and the hills conversion is random and
  can cost food on farmed grassland.

**K2. Dwarven Vault (per-turn Python)**
- **Rules.** `custom.rs:934-972` (`CustomFunctions.py:741-775`), called each turn
  (`events.rs:267-269`). Every city gets the level matching gold per city:

  | gold per city | <50 | 50-99 | 100-149 | 150-199 | 200-299 | 300-499 | 500+ |
  |---|---|---|---|---|---|---|---|
  | happiness | -2 | 0 | 0 | +1 | +2 | +2 | +3 |
  | production | | | | | +10% | +25% | +40% (+25% GPP) |

- **AI.** `empire::sliders` keeps only 20 + 5 × cities gold (`empire.rs:323-352`, reserve at
  328). Observed: the Empty Vault (-2) in 93% of Khazad games at t300 (8.4 of 8.7 cities); gold
  about 25 per city. Two reserve variants were measured and not kept (docs/AI.md
  "Civilization-specific"): population +2%, techs -3%. FfH's AI kept up to 500 per city for the
  Khazad (`CvPlayerAI::AI_getGoldReserve`, `CvPlayerAI.cpp:3566-3589`). **Handled poorly, but
  the obvious fix measured neutral.**
- **Value: medium in theory** (2 happiness in every city). The measured cost is the research
  needed to accumulate the gold.
- **Proposal, windfall-only.** Never lower the slider to save. Keep gold the empire receives as
  a lump sum (Mother Lode, peace tribute, goody huts, events) up to the next threshold
  (50 × cities), only while half the cities sit at the happy cap. This needs a small hook such as
  `CivPlay::gold_reserve(g, p) -> i32` read by `empire::sliders` and `units::upkeep`. Risk:
  low; it never cuts research below today's level.

**K3. Unique units (Trebuchet, Dwarven Cannon, Hornguard, Dwarven Slinger, Boar Rider, Myconid, Dwarven Druid, Dwarven Shadow)**
- **Rules.**
  - Trebuchet (Catapult class; Construction, 90 hammers; withdraw 80%, collateral 100% up to
    6 units, bombard 25) and Dwarven Cannon need the Siege Workshop (`PrereqBuilding`).
  - Hornguard (Knight class; Warhorses, 240 hammers, str 12, **no horse resource needed**)
    needs a Stable (`PrereqBuilding`).
  - Boar Rider needs pig plus the Stable class.
  - The Khazad have no Mage, Archmage, Longbowman, Ranger, Horse Archer or Marksman class.
- **AI.** Trebuchet, Dwarven Cannon and Hornguard are never trainable (G1). Observed: none
  ever. The Khazad army at t300 is 19.6 Slingers, 3.3 Champions, 3.6 Axemen, 5 Centaurs and
  1.2 Centaur Archers. The last two are foreign (G6). **Never used.**
- **Value: big for the conquest path.** The Khazad have the economy (research 180, production
  72 at t300) but no way to crack walls.
- **Proposal.** G1 (siege_building and enabler on `PrereqBuilding`). After that, check that
  the campaign uses Trebuchets: `ffh stats --civ CIVILIZATION_KHAZAD --detail`
  `unit_types.TREBUCHET`, and the `FFH_AI_TRACE` "campaign wants" and "strike on" lines.

**K4. Dwarf race and the Brewery hook**
- **Rules.** Every living Khazad unit gets `PROMOTION_DWARF` (hills double move, +25% work
  rate, 25% poison resist). Units built in a city with a Brewery gain +2 XP
  (`events.rs:747-751`, `CvEventManager.onUnitBuilt`).
- **AI.** `building_value` ignores the Brewery's free Ale (`FreeBonus` is valued nowhere in
  ffh-ai) and the XP hook. Observed: Brewery in 2% of Khazad games. **Never used.**
- **Value: small.** It is river-only and 240 hammers; +2 XP is about one promotion level per
  unit from that city. FfH's AI gave the capital's Brewery +250 (`CvGameUtils.py:2163-2166`).
- **Proposal.** In `Khazad::building_value`, value the Brewery in the top military-production
  city as `free_experience = 2` (the same xp term `building_value` already has). Low priority.

**K5. Maros (hero)**
- Iron Working, 180 hammers, 6 attack / 12 defense, +50% city defense, HERO (+1 XP per turn).
  Built through `Role::Hero` (`production.rs:560`) by 83% of Khazad. As `UNITAI_HERO` he
  falls to `Role::Army` (`units.rs:73`) and acts as a field unit. He is a defender.
- **Small.** In `Khazad::unit`, keep Maros as the capital's garrison
  (`UNITAI_CITY_DEFENSE`).

**K6. Minor**
- The Battering Ram spell has no `bAllowAI` (`CIV4SpellInfos.xml:559`): human-only.
- `reqRecruitMercenary` refuses AI Khazad (`spells_b.rs:704-710`).
- Conquest mode at Iron Working is set (`events.rs:1342-1420`), but ffh-ai never reads
  `conquest_mode`.
- The plan's Magic step (4 sphere techs, 640 beakers each) is of little use to the Khazad:
  Adepts cannot reach Channeling 2 and the Mage class is blocked. Skip the step for civs whose
  Mage class is NONE (small).

**Khazad ranking:** K3/G1 (siege and Hornguard) >> K2 windfall vault > K1 timing / K4 / K5.

### KURIOTATES (win 0.05, alive 0.54: the weakest of all civs in the census)

The census profile:
- They look strong mid-game: t300 legacy 1383 against 1127, land 150 against 111.
- Production stalls: 37 at t300 and 38 at t400, against 63 and 72 for the others.
- Population trails (45 against 53 at t300) and they lose cities: 2.67 to civs by t400
  against 1.46.

**U1. Sprawling: city limit, settlements, radius-3 full cities**
- **Rules.**
  - The trait (`CIV4TraitInfos.xml`) gives max 3 full cities (+ the world-size mod) and
    +3 happiness.
  - Past the limit, a new city is a settlement (`crates/ffh-core/src/city/turn.rs:132-151`,
    `CvCity.cpp:286-310`): -75% yields (`yields_culture.rs:576-578`), cannot train or build
    (`city/production.rs:140, 409`), no maintenance.
  - AI full cities get radius 3.
  - A captured city past the limit is also a settlement (`CvCity::init` runs on acquisition).
- **AI.**
  - `civs/kuriotates.rs`: one settler at a time for a settlement site, from a full city of
    size 5+ (or at its happy cap), at peace, not broke, at most 20 turns.
  - `sites.rs` / `found_value.rs`: 37-plot sites below the limit, 9-plot sites at it.
  - Measured +16% score (docs/AI.md).
  - Gaps:
    - (a) Settlements cannot train defenders. Production only fills a city's own quota
      (`production.rs` item 4). `production::emergency` skips cities with empty queues, which
      settlements always have. Settlements are therefore garrisoned only by the settler's
      escort and by surplus garrison units (`units.rs` `garrison_unit`).
    - (b) Full cities keep building settlement settlers from size 5. Each costs about 220
      hammers and freezes growth while it is built. This is a plausible cause of the
      production stall; it is not proven.
    - (c) G5.
- **Value: big.** Survival is 54%. Production and population stop growing after t300.
- **Proposals, in `Kuriotates::production`:**
  1. **Garrison settlements.** If a settlement within about 6 plots has fewer defenders than
     1 + (barbarians or enemies near), the nearest full city trains a `UNITAI_CITY_DEFENSE`
     unit. Existing garrison logic walks it there. Optionally, in `Kuriotates::turn`, send a
     spare unit to a threatened settlement.
  2. **Audit the full cities first.** With `ffh autoplay --civ CIVILIZATION_KURIOTATES
     --report 300`, check whether the three full cities are capped by food, happiness or
     settler production. If settlers are the brake, require size ≥ 8 (or ≥ 6 plus the
     city's core buildings done) for a settlement settler after the second settlement.
  3. G5.
- **Risk.** Garrisons cost unit support; the docs show the support/finance balance is
  sensitive. Fewer settlements means less land share.

**U2. Legends (world spell)**
- **Rules.** `spells_a.rs:2390-2408` (`CvSpellInterface.py:1642-1656`); +300 culture in
  every city including settlements. AI gate: 5 cities.
- **AI.** Cast by `choose_spell`. Observed: 38/39, first seen t150-250. A new city goes
  straight to Developing (100) and most of the way to Refined (500), which the radius-3 full
  cities need to own their third ring. **Handled adequately.**
- **Value: small-medium.** No change proposed.

**U3. Promote Settlement**
- **Rules.** `spells_b.rs:408-431`. No `bAllowAI` in XML (`CIV4SpellInfos.xml:2147`), so the
  AI can never cast it.
- It matters only after losing a full city (see G5). Rules/data decision. **Small.**

**U4. Enclave**
- Town → Enclave (1/0/5, +1 commerce with Taxation) for the Kuriotates only. It is automatic
  through cottage growth; the AI values it correctly for them (and wrongly for everyone
  else, G4). **Handled.**

**U5. Tailor and Jeweler**
- **Rules.** A free Fine Clothes or Jewels resource (+1 happiness empire-wide, tradeable),
  +1 happiness and +10% commerce per matching resource.
- **AI.** `building_value` ignores `FreeBonus` and `BonusYieldModifiers`. Observed: a Tailor
  in about 50% of Kuriotates games (from matching resources), a Jeweler in 8%.
- **Value: small.** Kuriotates happiness is not binding: happy margin 39 at t200 against 9 for
  others. The only real gain is a tradeable luxury (`trade.rs`). Leave it.

**U6. Centaur line, Eurabatres, Sprint**
- Centaurs need no Stable (the Kuriotates have no Stable class). Eurabatres: Divine Essence,
  str 23. Both are generic hero/unit production and **handled**.
- Sprint has no `bAllowAI`.
- The Centaur edge is diluted by G6: every other civ trains Centaurs too.

**U7. Conquest past the limit**
- Every capture becomes a -75% settlement. `diplomacy` and `campaign` value targets by
  population and wonders, and `raze_conquered_city` (`diplomacy.rs:971`) ignores this.
  Kuriotates took 1.06 civ cities by t300, then lost more.
- **Medium, uncertain.** Observe first: the population trajectory of captured cities for the
  Kuriotates, via `taken_civ` and pop. If they starve, lower the value of non-holy,
  non-wonder targets for a sprawling civ at its limit.

**Kuriotates ranking:** U1-1 (garrison settlements) > U1-2 (full-city growth audit/gate) > G6 rules decision (restores the Centaur edge) > G5.

### LANUN (win 0.27, alive 0.86)

**N1. Pirate Coves (handled, but gated by Fishing and leaky)**
- **Rules.** `SPELL_PIRATE_COVE` (`CIV4SpellInfos.xml:2083`; work boat, sacrificed,
  `iAIWeight` 300); req at `spells_b.rs:251-288`. Cove (+2 commerce), then Pirate Harbor after
  10 turns, then Pirate Port (2/2/4 on coast) after 20 more. The upgrades proceed unworked
  (`bOutsideBorders`).
- **AI.** `civs/lanun.rs`: `cove_boat` (one boat at a time, city size ≥ 3, the city works a
  valid plot) and `sea_worker`. Measured +23% score, with the caveat "Lanun that never research
  Fishing are unchanged" (docs/AI.md).
- Gaps:
  - (a) **Fishing is not prioritized.** `tech_plan.rs` has no Fishing step; the Lanun beeline
    is Iron Working (`tech_plan.rs:331`). `tech_value` knows nothing of coves. FfH's AI
    doubled Fishing for the Lanun, +150 with a coastal capital and +200 per pearl
    (`CvGameUtils.py:1054-1170`).
  - (b) **Coves and resources compete.**
    - `Lanun::sea_worker` casts the cove "here" before it looks for resources.
    - At end of turn, `choose_spell` values the cove at about 310 (10 for the improvement,
      300 weight, minus level × `unit_value` of a 0-strength boat).
    - So a work boat sent to a fish or clam more than one turn away likely becomes a cove on
      the first valid plot where it ends a turn, often an unworked one, and the resource stays
      unimproved.
    - Static reading only. Verify with `FFH_AI_TRACE` ("work boat ... resource at ... first"
      followed by `FFH_LOG` cove creation). Lanun `impr` is below average (t200 22.6 against
      25.0) despite coastal land.
  - (c) One boat at a time across the empire.
- **Value: big-medium.** The +23% measured where coves happened.
- **Proposals.**
  1. `Lanun::tech_value`: when Lanun have a coastal city and lack Fishing, add about the
     plan's `PLAN_VALUE` (6000) plus 150 per valid cove site in our cities' radius. It must
     beat the plan's goal bonus to be researched early.
  2. `Lanun::spell_value(SPELL_PIRATE_COVE)`: return 0 unless the plot is worked by one of our
     cities, or no unimproved sea resource of ours is within the boat's reach. In
     `sea_worker`, move the cast-here test after the resource test.
  3. Optionally allow two cove boats while 3+ worked cove sites exist.
- **Risk.** Fishing ahead of the tier-2 military or Education delays them. Measure with
  `civ_ab.py --civ CIVILIZATION_LANUN`. Census proxy: work boats in only 14/66 games by t200
  (snapshots miss short-lived boats, so this undercounts).

**N2. Raging Seas (world spell): effectively unavailable**
- **Rules.** `spells_b.rs:433-505` (`CvSpellInterface.py:1990-2048`); Optics.
  - Every non-Lanun unit on any plot adjacent to water (all ships, coastal stacks and coastal
    city garrisons, worldwide) takes about `(rand 0-74 + rand 0-74) × (1 - resist%)` cold
    damage, up to death (`unit_do_damage`, limit 100).
  - Each non-permanent improvement on non-Lanun owned coastal plots has a 26% chance to be
    destroyed.
  - AI gate: at war with someone whose weighted fleet exceeds 10.
- **AI.** Observed: 9/66, first seen t400-690. **Never used to any purpose.**
- **Value: big in a Lanun war.** A coastal city's defenders at about 35-50% health turn a
  marginal strike into a sure one, and enemy transports and fleets are wiped.
- **Proposal (needs a rules decision, G3).** Drop the AI-only branch of `reqRagingSeas`. In
  `Lanun::turn` (it runs before units move), cast when the campaign is in Strike phase against
  a coastal enemy city, or an enemy stack of power ≥ X is adjacent to water inside our
  borders. Choose the caster with any spare unit.
- **Risk.** The spell is once per game and hits neutrals and allies too. The code applies no
  attitude penalty, but it kills their units. Casting it too early wastes it.

**N3. War Tortoise (Knight class)**
- Warhorses, 240 hammers, 11/19, water walking, no horse needed. It needs the Siege Workshop:
  never built (G1). **Medium.** Fixed by G1.

**N4. Pirate, Boarding Party, Guybrush**
- Their abilities are naval (hidden nationality, capturing ships), and the AI has no naval
  combat. Sing has no `bAllowAI`.
- Guybrush (Iron Working, 360 hammers) is fielded by 50% of Lanun as a plain army hero.
  **Small.** No change.

**N5. Seafaring (free; pearls visible only to the Lanun), Lanun Harbor, `isPirate` coastal sites**
- Automatic or generic: pearls go through work boats, the Harbor's -10% maintenance is in
  `building_value`, and `found_value.rs:86,429,533` biases sites to the coast. **Handled.**

**Lanun ranking:** N1-1 Fishing beeline > N1-2 cove/resource fix > N2 Raging Seas (if the rules gate is lifted) > N3 via G1.

### LJOSALFAR (win 0.24, alive 0.71; economy well behind: commerce 45 vs 60, research 28 vs 42 at t200)

**E1. March of the Trees (world spell): effectively unavailable**
- **Rules.** `spells_b.rs:290-324` (`CvSpellInterface.py:1872-1896`); Way of the Forests.
  - Every Forest or Ancient Forest plot we own without a visible enemy becomes a Treant
    (str 10/10, Woodsman I+II, elemental) for 5 turns, and the forest becomes New Forest.
  - AI gate: `getAtWarCount(true) >= 2`, counting civ teams only
    (`team/diplomacy.rs:483-497`).
- **AI.** The AI fights one war at a time (diplomacy.rs), so the gate passes only when a
  second civ joins against it. Observed: 23/99, mean first seen t457. **Practically never
  used.**
- **Value: big when invaded.** Elves keep their forests, so dozens of str-10 units appear for 5
  turns. It is also an offensive option against a border city.
- **Proposal (needs the G3 rules decision).** Drop the AI-only branch. In `Ljosalfar::turn`,
  cast when at war (barbarians excluded) and either a city of ours is in peril (`View.peril`),
  or an enemy stack with power ≥ 1.5 × our nearby garrison is inside our borders, provided at
  least 8 forest plots lie within 2 of the threat.
- **Risk.**
  - Once per game.
  - Ancient forest becomes New Forest, which loses Fellowship of Leaves bonuses until it
    regrows.
  - Treants spawn as `UNITAI_ATTACK` and go through `military::army`; check that they attack
    rather than walk off.

**E2. Elf race, Dexterous, forest preservation**
- **Rules.**
  - The Elf promotion: forest double move, +10% attack in forest, **-20% work rate**.
  - Dexterous: +1 strength for archers.
  - `MaintainFeatures` keeps the forest (`plot/terrain.rs:1500-1512`), but build time still
    adds the feature's removal time (`plot/terrain.rs:1368-1377`, as C++
    `CvPlot::getBuildTime`): +250 on Forest for a cottage.
  - So an elf improvement on forest takes about 1.6× longer, plus the 20% slower rate.
- **AI.** docs/AI.md: "nothing needed" for the forest itself, which is correct. But the
  census shows fewer improved worked plots (18.9 against 25.0 at t200) and the economy lags:
  commerce 44.6 against 59.5, research slider 52% against 61%, techs 11.9 against 13.6.
  Worker count is the same (5.5 against 5.4). **Handled poorly**: the worker target
  (`production.rs` item 6) ignores the slower build rate.
- **Value: medium.**
- **Proposal.** `Ljosalfar::production`: train an extra worker while
  `unimproved_worked_plots > 2 × workers`, up to 1.5× the generic target.
- **Risk.** The generic "more workers" variants measured flat (docs/AI.md "Workers"). Here the
  bottleneck is specific; measure with `civ_ab.py --civ CIVILIZATION_LJOSALFAR`. A rules
  alternative for the user: skip the feature time when the feature is maintained.

**E3. Gilden (hero)**
- Archery, 120 hammers; an archer and city defender with HERO (+1 XP per turn). Fielded by
  67% of Ljosalfar, but as `Role::Army` (`units.rs:73`).
- **Small.** In `Ljosalfar::unit`, garrison him in the capital (the same rule as Maros).

**E4. No siege at all**
- No Catapult, Chariot, Berserker, Arquebus or Cannon classes; Siege Workshop and Alchemy
  Lab blocked.
- `can_take_cities` always uses the 120% bar (`diplomacy.rs:578-591`). `army_choice`
  substitutes attackers when no siege exists (`campaign.rs:1161-1167`). **Handled.**

**E5. Fellowship of Leaves terraforming, Pact of Nilhorn / Nature's Revolt, Hidden Paths**
- FfH's AI planted forests with Priests of Leaves and gave elves +10000 for these projects
  (`CvGameUtils.py:2782-2810, 3283`). **Small** until E1 exists; more forest means more
  treants.

**Ljosalfar ranking:** E1 (if the gate is lifted) > E2 workers > E3.

### LUCHUIRP (win 0.13, alive 0.77; good economy, army of slingers and foreign centaurs)

**L1. Golems and the Sculptor's Studio**
- **Rules.** Golems have no UnitCombat, so no XP promotions, and `bDisableUpgradeTo`. They
  are immune to death, poison and fear and heal slowly (friendly -10, neutral -5 against
  rates of 15 / 10 / 20 in city).
  - Wood Golem (Axeman class): Construction, 90 hammers, 6/6; needs the Studio
    (`PrereqBuilding`).
  - Gargoyle (Longbowman class): Engineering, 180 hammers, 8/8, +25% city defense,
    Defensive; needs the Studio.
  - Iron Golem (Champion class): Iron Working + iron or mithril + Forge class, str 10.
  - Clockwork Golem (Machinery, 15, collateral), Nullstone Golem (Mithril Weapons, 13,
    magic immune), Bone Golem (Divine Essence + ivory, 13, immortal): Training Yard class,
    which for the Luchuirp is the Studio.
- **AI.**
  - The Studio's `building_value` is 40 (+1 culture). `enabler_building` cannot see
    `PrereqBuilding` (G1). Research skips Wood Golem and Gargoyle (`research.rs:66`), though
    the plan forces Construction early (`tech_plan.rs:142,167,322`).
  - Observed: a Studio in 18% of Luchuirp games at t300; Wood Golems 0.9, Iron Golems 0.04
    and Gargoyles about 0 per player at t300, against 20.4 Dwarven Slingers, 4.9 Warriors and
    8.6 foreign Centaurs (G6).
  - **Handled poorly.** The plan heads for a tech whose units the cities cannot build.
- **Value: big.** Gargoyles are the Luchuirp's best city defender. The golem line is their
  army, and Barnaxus' Empower (L3) multiplies it. If G6 is fixed, Luchuirp melee falls back to
  Warriors without the Studio.
- **Proposal.** `Luchuirp::building_value(Studio)`: value it as the enabler of the best golem
  it unlocks, e.g. 15 × `type_attack_power` gain over the current best attacker. Or do it
  generically through G1 plus a research fix. The aim is a Studio in most producing cities
  soon after Construction. Note that the generic 133% gain test would fail against foreign
  Centaur Chargers (5 str) until G6 is fixed, so the civ hook is the more robust route.
- **Risk.** 90 hammers per city early, competing with settlers and economy buildings. Gate it
  on Construction plus a city size of 3 or more.

**L2. Gifts of Nantosuelta (world spell) and Golden Hammers**
- **Rules.** `spells_a.rs:1907-1926` (`CvSpellInterface.py:1304-1318`); Masonry. AI gate:
  5 cities. Each city gets a Golden Hammer, which becomes a free Engineer (+2 hammers,
  +3 GPP, permanent; `spells_a.rs:1972-1976`) or gives +1 strength to a unit that takes it.
- **AI.** `civs/luchuirp.rs` zeroes the take in our own cities. Measured +11.8% production.
  Observed: cast in 74/82, first seen t150-300. **Handled adequately.**
- **Value: small** for the refinement below.
- **Optional refinement.** `spell_value` returns 0 while a settler is planned and sites
  remain, for at most 30 turns after the gate first passes, since each extra city at cast time
  is +2 hammers forever. Measure before keeping it.

**L3. Barnaxus: Empower and rebuild**
- **Rules.**
  - Construction, 180 hammers, golem, melee.
  - Each turn, golems get Empower I-V (+10% attack and +10% defense each) matching his
    Combat I-V (`custom.rs:974-1018`, `CustomFunctions.py:777-822`; `events.rs:271-273`).
  - Faithful quirk: when Barnaxus is gone (class count 0) the block is skipped, so existing
    golems keep their Empower and new golems do not get it.
  - On death he drops `EQUIPMENT_PIECES_OF_BARNAXUS`. Rebuild Barnaxus in a Luchuirp city
    (`spells_b.rs:591-623`) restores him at 75% damage.
- **AI.**
  - Built by 66% of Luchuirp, used as Army.
  - `promotion_value` (`units.rs:263`) scores Combat at 40, below City Raider (60, with
    Warfare), so Barnaxus may skip the Combat line.
  - Rebuild's value is `-promotion_value(PIECES)` = -1, so it is never cast, and no unit
    carries the pieces home. Observed: pieces lying around in 4% of games.
  - **Handled poorly.**
- **Value: medium.** Each Combat level on Barnaxus is +10% on every golem, which only counts
  once L1 gives the Luchuirp golems.
- **Proposal.**
  1. Add `CivPlay::promotion_value` (`upkeep` runs before the `unit` hook, `units.rs:179/183`).
     Luchuirp: for Barnaxus, the Combat line counts the golem count × 20.
  2. `Luchuirp::spell_value(SPELL_REBUILD_BARNAXUS)`: a large value when castable. In
     `Luchuirp::unit`, a unit holding the pieces goes to the nearest own city.
  3. Optionally keep Barnaxus out of risky attacks (odds ≥ 85%).

**L4. Blasting Workshop, Pallens Engine, Adularia Chamber**
- **Rules.** Sorcery + Fire mana, Divination + Sun mana, Necromancy + Shadow mana. Golems
  trained in the city get, respectively:
  - Fire II: the Fireball summon each turn, which `summon_cast` uses;
  - Perfect Sight: see invisible;
  - Hidden: invisible, +1 first strike.

  (`events.rs:703-713`, `CvEventManager.onUnitBuilt`.)
- **AI.** `building_value` is 0, so they are never built (census: never). FfH valued them at
  400 / 200 / 200 + 20 × production (`CvGameUtils.py:2346-2352`). **Never used.**
- **Value: small-medium.** They are mana-gated: the palace gives Enchantment, Life and Earth,
  not Fire, Sun or Shadow.
- **Proposal.** `Luchuirp::building_value`: the Blasting Workshop in the top golem-producing
  city, valued about (golems trained there per 50 turns) × the value of one fireball. Only once
  L1 is in.

**L5. Repair (Enchantment I + Dwarf adepts)**
- **Rules.** `spells_b.rs:803-852`. Heals 20 damage on every golem, siege or naval unit on the
  plot. `iAIWeightWar` is 50, `iAIWeight` is 0.
- **AI.** Never cast (G2). Casters only move (`units.rs:605-637`). **Never used.**
- **Value: medium in a golem war,** since golems barely heal.
- **Proposal.** `Luchuirp::spell_value(SPELL_REPAIR)` = 10 × the damaged golems on the plot,
  and in `Luchuirp::unit`, casters stay with the golem stack. A promotion hook could also
  prefer Enchantment I for Luchuirp adepts, as FfH's AI did (`CvGameUtils.py:3627-3660`).

**L6. Minor**
- The Brewery +2 XP applies to living Luchuirp units (slingers, boar riders), not golems.
- The AI-only Stirrups block was dropped as AI policy.
- Conquest mode at Sorcery is not read.

**Luchuirp ranking:** L1 Studio > L3 Barnaxus Combat line + rebuild > L5 Repair > L4 Blasting Workshop.

---

## Part 3. Rules issues to report (not to work around)

1. **`CvCity::canTrain` with `UpgradeCiv`** (G6). A faithful C++ quirk that FfH's UI and AI
   never exercised. ffh-ai uses it heavily (16-20% of several civs' units). Needs a user
   decision: an AI filter, or a deliberate rules change recorded in PORTING.md.
2. **AI-only world-spell gates** (G3). Raging Seas and March of the Trees are effectively
   unavailable to AI players. This is a documented decision ("their game, their rules") but
   inconsistent with how CvGameUtils' AI-only branches were treated.
3. **Spells without `bAllowAI`**: Promote Settlement, Battering Ram, Sprint, Sing. Data
   choices that make these mechanics human-only.
4. **Elf build time.** `getBuildTime` adds the feature removal time even when
   `MaintainFeatures` keeps the forest. Faithful; a candidate for a rules change if the elves
   should not be the slowest builders. Not a bug.
5. **Checked line by line against the Python, faithful:** Mother Lode, Legends, Gifts,
   March, Raging Seas (including the early `return` mirroring the Python exception), Promote
   Settlement, Rebuild Barnaxus, Repair, Pirate Cove, Dwarven Vault, Luchuirp Empower, and
   the settlement rule in `CvCity::init`. No rules bug was found in these.

## Part 4. How to observe

- `ffh stats --civ CIVILIZATION_X --detail` (`unit_types`, `building_types`,
  `project_types`, `world_spell`) and `civ_ab.py --civ CIVILIZATION_X --off-base` for A/B
  against the civ's logic turned off.
- `FFH_LOG=<p>` for "World spell: …"; `FFH_AI_TRACE=<p>` for production ("… builds …",
  "the campaign wants …") and work boats.
- **Useful tooling additions:**
  - per-improvement counts in `--detail`: `IMPROVEMENT_PIRATE_COVE/HARBOR/PORT`, `MINE`,
    `ENCLAVE`, Lanun sea resources unimproved;
  - the turn each tech was learned (Fishing for the Lanun);
  - the split between full cities and settlements for the Kuriotates.

## Summary: top proposals by expected value for winning

| civ | 1 | 2 | 3 |
|---|---|---|---|
| Khazad | G1: Siege Workshop/Stable via `PrereqBuilding` (Trebuchet, Dwarven Cannon, Hornguard) | windfall-only vault reserve (K2) | Brewery / Maros as defender |
| Kuriotates | garrison settlements from full cities (U1-1) | audit, then gate settlement settlers so full cities grow (U1-2) | G6 rules decision (Centaurs become theirs again) / G5 |
| Lanun | Fishing beeline via `Lanun::tech_value` (N1-1) | stop work boats turning into coves in transit (N1-2) | Raging Seas at the strike, if the AI gate is lifted (N2) |
| Ljosalfar | March of the Trees on invasion, if the AI gate is lifted (E1) | extra workers for the forest build penalty (E2) | Gilden as capital defender (E3) |
| Luchuirp | Sculptor's Studio valued as the golem enabler (L1) | Barnaxus Combat-line priority + rebuild (L3) | Repair for golem stacks (L5) |
