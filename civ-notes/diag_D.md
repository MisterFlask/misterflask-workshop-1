# Diagnosis D: Malakim, Sheaim, Sidar, Svartalfar, Infernals, Mercurians

This is a read-only diagnosis. I did not build anything, did not run games and did not edit the repository.

## 0. Method and sources

- **Code.** I read the code at HEAD `88ac708`, plus the working tree as it stood while I worked.
  - Another agent is splitting `crates/ffh-ai/src/civs.rs` into `civs/mod.rs` and one file per civ.
  - The hook points in that split are `production`, `turn`, `unit`, `sea_worker`, `spell_value`, `building_value` and `tech_value`.
  - The files for my six civs are empty stubs: `civs/{malakim,sheaim,sidar,svartalfar,infernal,mercurians}.rs`.
  - The proposals below are written against those hooks.
  - Line numbers refer to the working tree when I read it: `casting.rs` (spell_value hook at :190), `production.rs` (building_value at :957), `research.rs`.
- **Original sources.**
  - `/home/user/ref/ffh2/Assets/python/entrypoints/CvSpellInterface.py`, `CvEventManager.py`, `CvGameUtils.py`, `CustomFunctions.py`.
  - `/home/user/ref/ffh2/CvGameCoreDLL/*.cpp`.
- **Observations.** These come from a read-only scan of the shared run cache `/home/user/.cache/ffh2-rs/runs`.
  - It holds 80 `ffh stats --detail` results. Those are 72 unique seed and forced-civ games:
    - 64 full-length games (`--until-supremacy`, cap 690) with Amurites or Balseraphs forced;
    - 8 games of 300 turns with Kuriotates or Lanun forced.
  - My civs appear only as random opponents, played by the generic AI:

    | Civ | Games |
    |---|---|
    | Malakim | 22 |
    | Sheaim | 24 |
    | Sidar | 11 |
    | Svartalfar | 5 |
    | Infernals | 4 |
    | Mercurians | 49 |

  - The snapshot fields used were `world_spell`, `unit_types`, `building_types`, `project_types`, `armageddon` and `team`.
  - The scripts are in `/tmp/claude-0/-home-user-misterflask-workshop-1/9e5b75fb-dfc7-5d3b-99bb-5086dd9f0081/scratchpad/tools/` (`cachescan.py`, `civscan.py`, `mycivs.py`).
  - The samples are small. Treat the rates as indications, not measurements.

Outcomes in the 64 full games: mean final rank by score among the 6 starting civs, and the number of games won by the civ's team.

| Civ | n | Mean rank | Team wins | Alive at end |
|---|---|---|---|---|
| Sidar | 11 | 1.91 | 3 | 11 |
| Malakim | 18 | 3.50 | 0 | 15 |
| Svartalfar | 5 | 3.80 | 2 | 3 |
| Sheaim | 20 | 4.10 | 0 | 12 |
| (all civs) | | 3.50 | 16.7% | 72% |

The Sheaim are among the weakest civs under the current AI, along with Kuriotates (rank 4.31) and Amurites (4.06). Their production falls after turn 400 (53 at t300, 54 at t400, 40 at t500).

---

## 1. Cross-cutting findings that matter for these civs

### 1.1 No civ builds a Mage Guild, so no civ trains adepts or gets casters (AI gap)

- **The rules.**
  - `UNIT_ADEPT` and `UNIT_IMP` have `PrereqBuildingClass = BUILDINGCLASS_MAGE_GUILD`.
  - Mages, Illusionists and Archmages have `iMinLevel` 4 or 6, so they come only from upgrading adepts.
  - The Mage Guild (120 hammers, Knowledge of the Ether) has no yields and no `iAIWeight`.
- **The AI.**
  - `production::generic_building_value` (production.rs:962-1040) gives the Mage Guild about 0, and step 9 skips buildings with a value of 0 or less.
  - `production::enabler_building` (production.rs:777) builds the prerequisite building only for units that pass `role_ok(.., Defense)` or `role_ok(.., Attack)`. Those require `fighter`, which excludes units whose default AI is `UNITAI_MAGE`.
  - Step 10 (production.rs:653-660) asks `best_unit(g, c, Role::Mage)`. That needs `city_can_train`, so it finds nothing without a Mage Guild in the city.
  - `research.rs::unit_available` (research.rs:66) still counts the adept as available once the tech is known, because it checks `prereq_building` but not `prereq_building_class`. Research therefore pays for "the first arcane unit", and production never builds one.
- **Cache.**
  - At turn 300, 4 of 379 civ snapshots have a Mage Guild and 7 have an adept or imp. At turn 400 it is 5 and 3 of 288.
  - Sheaim: 0 of 24 games with a Mage Guild, 1 with an adept.
  - Svartalfar: 0 of 5. Sidar: 0 of 11. Malakim: 2 of 22.
- **Who is hurt.**
  - **Sheaim** most. Both leaders are Summoner, and Tebryn is also Arcane. The Sundered trait gives Stigmata to adepts' summons. The palace gives Death, Chaos and Fire mana, which means Raise Skeleton, Spectre, Dance of Blades and Fireball. The Planar Gate's Mobius Witch needs a Mage Guild. The research plan beelines Corruption of Spirit, Necromancy and Sorcery, all for casters it never builds.
  - Malakim: Sand Lions need an adept with Channeling 2.
  - Svartalfar: Faeryl is Arcane, and the Illusionist is their mage.
  - Amurites as well, outside my scope.

### 1.2 No civ builds a Siege Workshop, so no catapults, trebuchets or cannons (AI bug)

- All siege engines (`UNIT_CATAPULT`, `UNIT_TREBUCHET`, `UNIT_CANNON`, `UNIT_DWARVEN_CANNON`) use **`PrereqBuilding` = BUILDING_SIEGE_WORKSHOP**, not `PrereqBuildingClass`. The rules check this correctly (`plot/units.rs:1418`).
- The AI looks only at `info.prereq_building_class`:
  - `campaign::siege_building` (campaign.rs:1207: `let Some(bc) = info.prereq_building_class else { continue }`);
  - `production::enabler_building`.
- So the Workshop is never proposed. Its generic value is 0 (no yields, `iAIWeight` 0).
- Cache: 0 of 4860 civ snapshots have a Siege Workshop or a catapult.
- docs/AI.md says the campaign builds "siege engines -- or the Siege Workshop that trains them", and the guns-vs-butter measurements in docs/AI.md were taken with this bug in place.
- This is not specific to my civs. The Svartalfar cannot have a Workshop at all (it is a blocked building), so the fix changes nothing for them.

### 1.3 World spells are cast the first turn the AI is allowed to

- `casting::choose_spell` (casting.rs:100, a port of `CvUnit::chooseSpell`, CvUnit.cpp:15917) adds `iAIWeight` (300 for every world spell) to terms that are usually 0. The world spells' effects live in Python, so no XML field feeds the value.
- So the first unit in the end-of-turn loop (`casting::end_turn`, casting.rs:16) casts the world spell the first turn the Python `req*` returns true.
- Timing is therefore set entirely by the AI-only gates in the ported `req*` functions:

| World spell | AI-only gate | Cast in cached games |
|---|---|---|
| Worldbreak | counter ≥ 50 | Sheaim 0/24 |
| Religious Fervor | 5 or more cities with the state religion | Malakim 12/22, turns 200-300 |
| Into the Mist | 40 or more units | Sidar 10/11, turns 250-350 |
| Veil of Night | 25 or more units and at war with no civ team | Svartalfar 5/5, turns 200-350 |
| Divine Retribution | at war with 2 or more civ teams | Mercurians 7/49 |
| Hyborem's Whisper | Malevolent Designs and an Ashen Veil city | Infernals 0/4 |

- FfH's own C++ AI behaves the same way, end-of-turn `chooseSpell` (CvGame.cpp:7041), so this is faithful.
- The new `civs::turn` hook (lib.rs:128) runs before units move. That is the better place for spells whose value depends on striking in the same turn.

### 1.4 The Armageddon counter in AI games

- **Raw limit.** The limit is 50 plus 12 for each initialized player (`player/mod.rs:757`), about 122-134 with 6 civs plus the barbarians. Every Mercurian, Infernal or rebel player that joins adds 12.
- **Sources.**
  - Ashen Veil founded +5, and +1 per city it spreads to.
  - Razing a city: +1, plus 1 per population over 5.
  - Elegy of the Sheaim +5, Purge the Unfaithful +3, The Draw +10.
  - Veil wonders: Stigmata on the Unborn +5, Necronomicon +3, Pillar of Chains +4.
  - Units: Basium +5, Hyborem +5, Rosier, Mardero, Meshabber.
- **Cache (72 games).**
  - The median counter is 5 at t300, 14 at t400 and 19 at t500.
  - The median of each game's maximum is 20; the 90th percentile is 39.
  - Games reaching each level: 17 reached 30, 5 reached 40, **2 reached 50**, both only at turns 550-600.
- **Consequences.**
  - Worldbreak's AI gate (≥ 50) is reached in about 3% of games, and only near the end.
  - Planar Gate tier 2 (≥ 50) and Stigmata bonuses stay small.
  - The Armageddon events can trigger in many games, the first ones at least (rules XML `iPrereqGlobalCounter`):

    | Counter | Event |
    |---|---|
    | 30 | Blight (25% death damage to every living unit) |
    | 35 | Pestilence |
    | 40 | Stephanos |
    | 50 | Buboes |
    | 60 | Yersinia |
    | 70 | Ars Moriendi |
    | 80 | Hellfire |
    | 90 | Wrath |
    | 100 | Apocalypse |

  - At 70 the rituals Bane Divine (evil: kills every disciple unit) and Glory Everlasting (good: kills every demon and undead) become possible. `best_project` excludes both (production.rs:698-715), so the AI never builds them.

### 1.5 Hidden Nationality is invisible to the AI's logic

Rules (CvUnit.cpp:18339 `isAlwaysHostile`, CvUnit.cpp:8829 `isNoCapture`, CvGameCoreUtils.cpp:1061 `PUF_isEnemy`, CvPlot.cpp:3700; ported in `unit/movement.rs:1489` and `unit/promotions.rs:811`):

- A unit with Hidden Nationality (HN) is always hostile to every other team, in its own cities as well.
- It cannot capture cities.
- It treats every foreign unit as a visible enemy: `isVisibleEnemyUnit(pUnit)` passes `pUnit->isAlwaysHostile()` as iData2. Foreign units treat it as an enemy too.

What FfH's C++ AI did: every HN unit gets `GROUPFLAG_HNGROUP` (CvUnitAI.cpp:19146), and `HNgroupMove` is just `MISSION_SKIP` (CvUnitAI.cpp:20870). HN units are parked.

What the port does: nothing special. The consequences are spelled out under Svartalfar (Veil of Night) below. This also applies to Alazkan, who is born HN.

---

## 2. Sheaim (Tebryn: Arcane, Summoner; Os-Gabella: Industrious, Summoner; civ trait Sundered)

### 2.1 Planar Gate (BUILDING_PLANAR_GATE, CIV_BUILDING1)

**Rules**
- 300 hammers, Knowledge of the Ether. It has no yields and no `iAIWeight`.
- Each turn `ev_planar_gate` (ffh/events.rs:1914, a port of CvEventManager.py:2310) rolls `10000·rand ≤ 600·mult`, giving a spawn chance per gate city of:

  | Counter | Chance per turn | Cap per class (× gates) |
  |---|---|---|
  | below 50 | 6% | 1 |
  | 50 or more | 9% | 2 |
  | 75 or more | 12% | 3 |
  | 100 | 15% | 4 |

- On success it creates one creature, drawn uniformly from the classes whose building is in **that city** and whose unit-class count is below the cap times the player's number of Planar Gates.
- The creatures:

  | Building | Creature | Notes |
  |---|---|---|
  | Gambling House | Revelers | 6/6 + 1 poison |
  | Mage Guild | Mobius Witch | level-4 caster with random sphere-1 promotions |
  | Carnival | Chaos Marauder | 4/4 demon |
  | Grove (Feral Bond) | Manticore | 12/8, flying, 3 moves, tier 4 |
  | Public Baths | Succubus | 4 + 2 unholy, Mind2 |
  | Obsidian Gate (Pass Through the Ether) | Minotaur | 8/11, tier 4 |
  | Temple of the Veil | Tar Demon | 2/7 |

- The creatures cost nothing to build, but most have `bMilitarySupport`, so they cost unit support.
- Raising the counter raises both the chance and the caps. That is the link to the Armageddon counter.

**AI:** never used.
- `generic_building_value` is about 0, so step 9 skips it (`value <= 0`).
- Cache: 0 Planar Gates in 24 Sheaim games, and none of the seven creatures ever appears.
- FfH's own Python AI never built it either: CvGameUtils.py:2447 gives `BUILDING_PLANAR_GATE` a value of `-400*30*productionDifference`, which is negative.

**Value:** medium, conditional on the gate city's buildings. In the cache the Sheaim build Carnival (1.8 cities per civ at t400), Public Baths (2.3) and Temple of the Veil (1.6 when on the Veil). That makes Chaos Marauders, Succubi and Tar Demons the realistic creatures, about 60-90 hammers each.
- With 2 gates in such cities: up to 6 free tier 2-3 units over about 50 turns (6% a turn per city), about 400-500 hammers for 600 invested.
- With a Mage Guild, Gambling House, Grove or Obsidian Gate in the gate city, the creatures are tier 3-4. A Manticore or Minotaur alone is worth about 250 hammers.
- The return grows with each extra gate, because every gate raises every class cap by one.

**Proposal** (`civs/sheaim.rs::building_value`, applied to `BUILDING_PLANAR_GATE`). Plain hammer weighting is about 15 value points per hammer per turn, as in `building_value`.
- Value = 15 × spawn chance × mean hammer worth of the eligible classes with headroom in this city.
  - Eligible means: the class's building is in this city and its count is below cap × (gates + 1).
  - Use about tier × 60 for summons with `iCost -1`.
  - Multiply by the number of eligible classes over the creatures expected in about 50 turns, capped by the headroom.
- Return 0 when `economy::can_afford_unit` is false, since free units still cost support.
- Also add a small bonus to the eligible buildings (Mage Guild, Gambling House, Carnival, Public Baths) in a city that already has a gate.
- Expected magnitude: about 100-200 points for one gate in a city with 2-3 eligible classes. That is a normal mid-value building, so it competes with others at step 9 and does not preempt the army.

**Risks**
- 300 hammers that the army needs (docs/AI.md records buildings costing cities).
- Creatures add support cost, and `empire::disband` may then delete the weak ones. Gate only when affordable, and prefer cities with tier-3/4 classes.
- **Observe:** `building_types.PLANAR_GATE` and `unit_types` MANTICORE, MINOTAUR, CHAOS_MARAUDER and so on, through `civ_ab.py --civ CIVILIZATION_SHEAIM`.

### 2.2 Casters: Sundered, Summoner, Arcane, Eater of Dreams

**Rules**
- **TRAIT_SUNDERED** (data/xml/Civilizations/CIV4TraitInfos.xml) gives adepts `PROMOTION_SUNDERED`: 25% unholy resistance, and summons get `PROMOTION_STIGMATA` (+counter/2 % combat, `iCombatPercentGlobalCounter` 50).
- **TRAIT_SUMMONER** gives summons +1 duration, and adepts get `PROMOTION_SUMMONER`, whose summons receive Combat1.
- **TRAIT_ARCANE** (Tebryn) gives adepts Potency.
- The palace gives Death, Chaos and Fire mana:
  - Death1: Raise Skeleton, a *permanent* 2 + 1 death undead unit (`bPermanentUnitCreate`);
  - Death2: Spectre;
  - Fire2: Fireball;
  - Chaos1: Dance of Blades.
- **UNIT_EATER_OF_DREAMS** (archmage replacement, Strength of Will) has SPELL_CONSUME_SOUL, which has no `bAllowAI`. `unit_can_cast` returns false for the AI (unit/ffh.rs:189), so the AI can never cast it.

**AI:** never used, because no Mage Guild is ever built (see 1.1). If adepts existed, the generic `casting::summon_cast` (casting.rs:239) and the caster role (`units::caster`, units.rs:605, which goes next to the war target with the army) would already use them sensibly. Raise Skeleton is valued when an enemy is within the skeleton's move range, so adjacent.

**Value:** medium to big for the Sheaim.
- They cannot build Training Yards, which are blocked, so they have no Champions or other tier-3+ melee that need one. Their army in the cache is Pyre Zombies and archers.
- Summons are their designed substitute: one free permanent skeleton per adept per turn at the front, with Combat1 and Stigmata, plus Fireballs.
- It also makes the Planar Gate's Mobius Witch reachable, and the existing Necromancy/Sorcery beeline finally pays.

**Proposal**
- **Generic fix.** Extend `production::enabler_building` to `Role::Mage`: when `count_ai(MAGE)` is below `mage_target` (production.rs:655) and no city can train an adept, build the Mage Guild in the most productive safe city, the same pattern as `campaign::siege_building`.
- **Sheaim.** In `civs/sheaim.rs::production` or `building_value`, raise the mage target to about cities/2 for Summoner or Arcane leaders, and give the Mage Guild a value of about 200 in the two best production cities.
- **Risks**
  - Adepts are weak (strength 3) and cost support. They only pay at war, through summons. A peacetime adept is an idle unit, so gate extra adepts on war or a prepared war.
  - Watch `unit_types` ADEPT, SKELETON and SPECTRE, and captures (`taken_civ`).

### 2.3 Worldbreak (SPELL_WORLDBREAK, world spell)

**Rules**
- `req_worldbreak` (ffh/spells_b.rs:2432, a port of CvSpellInterface.py:3503) requires counter > 0, and **for the AI counter ≥ 50**.
- `spell_worldbreak` (spells_b.rs:2446) acts on every plot not owned by the caster:
  - every unit in every such city takes `counter`% fire damage, with a damage limit of 100;
  - each city has a `rand(100) ≤ counter/4` chance of `cf_do_city_fire` (destroys buildings);
  - forest and jungle become Smoke with the same chance.
- It hits teammates and civs at peace, but declares no war.
- At counter 50 every foreign garrison is at half health; at counter 100, units without fire resistance die.

**AI:** never used (0 of 24 games). The rule gate is almost never reached (1.4: 2 of 72 games reach 50, at turns 550-600). When it is reached, `choose_spell` would cast it on the first turn at the end of the turn, whether at war or not. The enemy then heals in its own turn before anyone attacks.

**Value**
- Very big when timed with a strike: half-health defenders in every enemy city in one turn, and building fires.
- Overall small, because the AI can almost never cast it. The AI-only gate is a rule; the AI may not work around it.

**Proposal**
- **Timing**, for the rare case it becomes legal: `civs/sheaim.rs::spell_value` returns 0 for Worldbreak. The `civs::turn` hook (before units move) casts it when one of these holds:
  1. we are at war and our campaign op is in StepIn or Strike against a city within reach this turn (`v.campaign.ops`);
  2. the counter is 75 or more and we are at war with anyone.
- **Elegy push** (see 2.4) only when the counter is already 40 or more.
- **Rules question for the owner.** The `if not human: counter < 50` gate exists because FfH's AI could not time the spell. With a timing rule like the one above, a deliberate rules change could relax it, say to 20 or more for the AI. That has to be recorded in docs/PORTING.md and is not the AI's call.

### 2.4 Elegy of the Sheaim (PROJECT_ELEGY_OF_THE_SHEAIM)

**Rules**
- Sheaim only, Way of the Wicked (280 beakers, early), 600 hammers, unlimited instances, `iModifyGlobalCounter` +5.
- +5 raw is about 3.7-4 counter points (`city/turn.rs:809`).
- It has no Python effect.

**AI:** never used.
- `production::best_project` skips projects with `ai_weight <= 0` (production.rs:703), and the Elegy's `iAIWeight` is 0. Cache: 0 Elegies.
- FfH's Python AI lists it in `sProd` (CvGameUtils.py:1282) but never gives it a value.

**Value:** small, and risky.
- Going from the typical counter of about 15 at t400 to 50 takes about 10 Elegies, roughly 6000 hammers.
- On the way it triggers Blight at 30, Pestilence at 35, Stephanos at 40 and Buboes at 50. These hurt everyone. Pyre Zombies and skeletons are undead and therefore immune to Blight's death damage.
- It also makes Glory Everlasting (counter ≥ 70) available to good civs; that ritual would kill every Sheaim undead.
- The benefits: Planar Gate tier 2 at 50 (×1.5 chance, ×2 caps), Stigmata (+counter/2 % on summons), and unlocking Worldbreak.

**Proposal:** only as a finishing step. In `civs/sheaim.rs::production`, build an Elegy in the best production city when all of these hold:
- counter ≥ 40 (so 50 is 2-3 Elegies away);
- Worldbreak not yet cast;
- at war or preparing one;
- the city finishes it in 25 turns or fewer;
- the treasury is not in financial trouble.

Otherwise never. Measure with `world_at.armageddon`.

### 2.5 Pyre Zombie (UNIT_PYRE_ZOMBIE, axeman class)

**Rules**
- 3 + 1 fire, 60 hammers, Bronze Working, no Training Yard needed.
- Undead: immune to death and poison, -50% against holy.
- Fire resistance, `bAIblockPermDefense`.
- `PythonPostCombatLost = postCombatExplode` (ffh/spells_a.rs:413): when it loses, every unit on the 8 plots around it takes 10% fire damage, including a city it attacked and our own adjacent stacks. Forests around it may become Smoke.

**AI:** handled adequately.
- It is the Sheaim's main attacker in the cache: 14.8 per civ at t300.
- The explosion is not counted in the assault estimate, which only counts `collateral_damage`. That makes zombies slightly undervalued as fodder.
- `bAIblockPermDefense` is ignored, but archers win the defense role anyway (5 defense, +25%, first strike).

**Value:** small. **Proposal:** none needed. Optionally count about 10% of each adjacent defender's HP per expected lost zombie in `campaign` and `military::assess`.

### 2.6 Training Yard blocked (`civ_building(Sheaim, TRAINING_YARD) = None`)

The rules block Champion, Berserker, Immortal and so on for the Sheaim. The AI handles this correctly: `best_unit` and `enabler_building` go through `city_can_train` and `civ_building`. This is why 2.2 matters so much.

### 2.7 Abashi (hero), Ashen Veil and the Infernal Pact

**Abashi**
- Divine Essence (9600 beakers), 21/21, magic immune, `iAIWeight` 200.
- Built by `Role::Hero` if it is ever reached. Cache: never.
- Value small, since it comes late.

**Ashen Veil and Infernal Pact**
- The Sheaim beeline Corruption of Spirit (tech_plan.rs:313), which founds the Veil (+5 counter).
- They are on the Veil in only 4 of 15 games at t400; they mostly run Octopus Overlords or Council of Esus.
- Infernal Pact (`iAIWeight` 2000) needs the Veil as state religion. `research::tech_value_static` caps the weight at +100 (research.rs:561).
- Two of the four Infernal spawns in the cache were triggered by the Sheaim.
- Spawning the Infernals is double-edged for the Sheaim:
  - they declare war on the founder's enemies and get +4 attitude toward the founder;
  - they take empty land, and their territory spreads hell terrain into Veil and evil neighbours once the counter allows (CustomFunctions.py:651ff).
- No proposal.

### 2.8 Sheaim ranking

1. Mage Guild and adepts (2.2): medium to big.
2. Planar Gate valuation (2.1): medium.
3. Worldbreak timing plus a conditional Elegy finisher (2.3, 2.4): big when it fires, rare overall.

---

## 3. Malakim (Varn: Adaptive, Creative, Spiritual; Decius: Organized, Raiders; race Nomad)

### 3.1 Religious Fervor (SPELL_RELIGIOUS_FERVOR, world spell, Priesthood)

**Rules**
- `req_religious_fervor` (spells_b.rs:751, a port of CvSpellInterface.py:2214) needs a state religion, and **for the AI the religion in 5 or more cities**.
- `spell_religious_fervor` (spells_b.rs:764) puts one tier-3 priest of the state religion in **every** city (Council of Esus gives Assassins), each with XP equal to the number of cities that have the religion.
- The cache Malakim are on Runes of Kilmorph in 13 of 18 games at t300. Their Priests of Kilmorph are 5/5 with Medic 1-2, Divine and Channeling 2, 120 hammers each, and default AI `UNITAI_MEDIC`.

**AI:** handled adequately.
- Cast in 12 of 22 games, between turns 200 and 300, producing 5-13 priests. That is roughly 600-1500 hammers of units for free.
- The priests take the Caster role (`units::caster`, units.rs:605): next to the war target at war, otherwise to the city with the most danger. At peace they all walk to the same city and sit there, paying support.

**Value:** medium, already captured.

**Proposal** (optional): `civs/malakim.rs::spell_value` returns 0 while not at war or preparing one **and** the number of religious cities is still growing (it rose in the last 20 turns, fewer than 8). Each extra city adds a priest and +1 XP to all of them. Cast at once when at war.
- **Risk:** the civ may never qualify again, for example after losing cities. Add an age cap: always cast by 60 turns after the first time it became castable.

### 3.2 Citadel of Light (BUILDING_CITADEL_OF_LIGHT, CIV_BUILDING1)

**Rules**
- 240 hammers, Sorcery, +5% defense.
- Each turn `ev_citadel_of_light` (ffh/events.rs:1787, a port of CvEventManager.py:2223) picks the plot within 2 tiles holding the most enemy strength at war, scored 5 × base combat strength. The plot is skipped if it also holds non-war units.
- Every unit on that plot takes 20% fire damage, up to 40%. Forest and jungle catch fire with a 20% chance.

**AI:** handled poorly.
- Its value is `defense_modifier*1/2` (2 points), or 15 under threat (production.rs:1023). It is never chosen except as a leftover.
- Cache: built in 1 of 22 games (seed 6, at t690).

**Value:** small to medium, defensive.
- An enemy stack on the AI campaign's assembly ring (2 plots out) and then adjacent takes 20% and then 40% before it strikes, which collapses its odds.
- It does nothing for offense and little for winning, but it keeps frontier cities.

**Proposal** (`civs/malakim.rs::building_value`):
- Value about 250, plus 50 per visible enemy unit within 3 plots, for a city that meets either condition:
  - within 4 plots of a rival civ's border while at war or preparing war (`diplomacy::preparing_target`);
  - with `local_threat > 0` in the last turns.
- Otherwise leave the generic value.
- **Risk:** production diverted from the army. Limit it to a third of the cities.

### 3.3 Desert Shrine (BUILDING_DESERT_SHRINE, replaces the Pagan Temple)

**Rules:** the Pagan Temple's effects (+10% gold, a priest slot, +1 happiness with Incense), plus 2 XP for disciple units.

**AI:** handled adequately. It is built (19 of 22 games) through the generic value. **Value:** small. **Proposal:** none.

### 3.4 Sand Lion (SPELL_SUMMON_SAND_LION)

**Rules:** an adept with Channeling 2 standing on desert or burning sands (`req_sand_lion`, spells_b.rs:1374) summons a Sand Lion: 5 + 1 fire, +40% in desert, Nomad.

**AI:** never used. There are no adepts (see 1.1). If adepts existed, `summon_cast` would value it when an enemy is in reach, but only when the caster happens to stand on desert. **Value:** small. **Proposal:** comes with the Mage Guild fix in 2.2.

### 3.5 Nomad race (PROMOTION_NOMAD)

**Rules:** +20% attack and defense on desert, double moves on desert. **AI:** nothing uses it. **Value:** small. **Proposal:** none.

### 3.6 Teutorix, Camel Archer, Lightbringer

- **Teutorix:** Blasting Powder, a 12/12 archer hero, +1 Overcouncil vote. Built when reached (7 of 22 games). Handled.
- **Camel Archer:** a Horse Archer that needs no horse resource. Handled, because `best_unit` goes through `city_can_train` (4 of 22 games).
- **Lightbringer:** a tier-1 disciple with no religion requirement, 2/2, 60 hammers, upgrades to any disciple. Trained early (18 of 22 games) as a Reserve army unit. Handled; small value.

### 3.7 An observation, not a civ mechanic

In 3 of 22 games (seeds 1 and 39) the Malakim stay at **1 city until turn 200 or later**: population 5-7, food surplus 0, no settlers, on landmasses with room (seed 39: alone on 280 tiles). Desert sites may score too low in `AI_foundValue` and sites.rs, or the settler gate may never pass.
- **Observe:** `FFH_AI_TRACE=<p> ffh stats --seed 39 --civ CIVILIZATION_MALAKIM`, then look at the `builds` lines and `sites`.
- This is generic: the expansion logic on desert starts.

### 3.8 Malakim ranking

1. Citadel of Light in threatened frontier cities: small to medium.
2. Religious Fervor timing (wait while expanding, cast at war): small to medium.
3. Sand Lions through the generic Mage Guild fix: small.

Also check the 1-city stall in 3.7.

---

## 4. Sidar (Sandalphon: Defender, Industrious)

### 4.1 Into the Mist (SPELL_INTO_THE_MIST, world spell)

**Rules**
- `req_into_the_mist` (spells_a.rs:2280, a port of CvSpellInterface.py:1572): for the AI, 40 or more units.
- `spell_into_the_mist` (spells_a.rs:2289) gives `PROMOTION_HIDDEN` to every unit, workers and settlers included (`setHasPromotion` ignores the promotion's unit-combat list).
- Hidden means invisible and +1 first strike, removed by combat or by casting.
- FfH makes units in their own team's cities visible (CvUnit.cpp:10270-10279), so garrisons are not hidden; field units, workers and settlers are.

**AI:** cast in 10 of 11 games between turns 250 and 350, at the 40-unit gate and usually at peace.
- The promotion persists on every unit that does not fight. Hidden workers and settlers stay invisible to barbarians indefinitely, so casting early is not wasted.
- Units that hunt barbarians lose it, and units trained afterwards never get it.

**Value:** small to medium.
- A war-timed cast would give the whole army an unseen approach and +1 first strike on its opening attacks.
- The Sidar are already the strongest of my civs (rank 1.91).

**Proposal** (`civs/sidar.rs::spell_value` returns 0, and `civs::turn` casts):
- cast when a campaign op of ours is in March or StepIn;
- or when we have been at war for fewer than 5 turns with an op committed;
- or, failing those, 50 turns after the gate first opened. This keeps the worker and settler protection.

**Risk:** a late cast loses the peacetime protection of workers and settlers for those turns.

### 4.2 Wane (SPELL_WANE) and Shades

**Rules**
- A level 6+ living unit, not an animal or beast, not immortal, with at most 4 Shades (`req_wane`, spells_b.rs:2196) converts into `UNIT_SHADE`.
- The Shade is 2/2, but has `GreatPeoples`: artist, engineer, merchant, scientist. It can join a city as a settled great specialist.
- The Sidar palace adds +1 commerce to artists, merchants and scientists (great ones included) and +1 production to engineers.
- The XML has **no `bAllowAI`**, so `unit_can_cast` returns false for the AI (unit/ffh.rs:189; C++ CvUnit.cpp:13688).

**AI:** never used, because the rules forbid it to the AI. Cache: 0 Shades.

**Value:** small to medium. Four settled great specialists are worth about 3-6 yield each per turn with the palace bonus, for the cost of four veteran units.

**Proposal**
- This needs a **rules change** (`bAllowAI=1` on SPELL_WANE). It is the owner's decision and must be recorded in docs/PORTING.md.
- With it, `civs/sidar.rs`:
  - `spell_value`: Wane only at peace, only on garrison or reserve units at level 6 or more, never on the army's best units.
  - `unit`: Shades go to the city with the best specialist yield and join.

### 4.3 Ghost, Divided Soul, Rathus

- **Ghost** (assassin class): Hidden and Stealth from birth, Marksman, -50% city attack. It is built (5 of 11 games) as a generic attacker. `bAIblockPermDefense` does not matter, because its default AI is EXPLORE, which excludes it from the Defense role. Handled; small.
- **Divided Soul** (hunter class): the same as a Hunter, plus SPELL_SEVER. Sever creates `UNIT_SEVERED_SOUL`, a 0-strength flying explorer. `summon_value` is 0 for it, so it is never cast. Small; no proposal.
- **Rathus** (hero, Poisons): built in 10 of 11 games. Handled.

### 4.4 Sidar ranking

1. Into the Mist war timing: small to medium.
2. Wane and Shades, if the owner opens it to the AI: small to medium.

Nothing else matters much; the Sidar win with the generic AI.

---

## 5. Svartalfar (Faeryl: Arcane, Raiders; civ trait Sinister; race Dark Elf)

### 5.1 Veil of Night (SPELL_VEIL_OF_NIGHT, world spell)

**Rules**
- `req_veil_of_night` (spells_b.rs:2122, a port of CvSpellInterface.py:3204): for the AI, 25 or more units and at war with no civ team.
- `spell_veil_of_night` (spells_b.rs:2137) gives `PROMOTION_HIDDEN_NATIONALITY` permanently to every unit with combat strength above 0.
- HN units (1.5):
  - cannot capture cities;
  - are hostile to every team at all times, so neighbours at peace can attack them anywhere outside their cities and in them;
  - see all foreign units as enemies, so they can attack anyone without declaring war;
  - cause no war weariness.
- With Faeryl's Raiders trait, pillage gold is doubled.

**AI:** cast in 5 of 5 games, between turns 200 and 350, as soon as the gate opens. The AI has no HN logic. Statically:
- `units::attack_adjacent` (units.rs:375, called by garrisons at 80% odds and explorers at 75%) and `military::best_adjacent` (military.rs:440) both use `plot_is_visible_enemy_unit_for_unit`. So HN garrison, explorer and army units **attack adjacent units of civs at peace** when the odds are good.
  - `best_adjacent` skips civ cities; `attack_adjacent` does not.
  - `best_hunt_target` (units.rs:408) also lets HN stacks hunt peaceful neighbours' units in their land (it skips cities only).
- The neighbours' units see Svartalfar HN units as enemies and attack them in `attack_adjacent`, `best_adjacent` and `best_hunt_target`.
  - But their danger maps (`view.rs::build_danger`) and `military::find_targets` key on the owner's team, so the neighbours' workers do not flee from HN raiders.
- In a real war the campaign can kill the defenders, but HN attackers cannot move in (`unit_is_no_capture`, unit/promotions.rs:811). Only units trained after the Veil capture cities.
- FfH's C++ AI parked HN units (`HNgroupMove` = skip), so the cast was nearly worthless there.

**Cache** (n=5, inconclusive):
- In 2 games (seed 11, both runs) the Svartalfar went on to take 6-11 cities after the Veil, with new units.
- In 3 games they collapsed in the 150-200 turns after casting:
  - seed 5 Balseraphs run: 10 cities at t300, 0 at t500;
  - seed 5 Amurites run: 3 cities at t300, 0 at t500;
  - seed 26 Amurites run: 4 cities at t450, 1 at t500.

**Value:** medium, of unknown sign. The current cast removes city capture from the whole existing army for the rest of its life, and starts uncontrolled skirmishes with neighbours.

**Proposal** (`civs/svartalfar.rs::spell_value`):
- Return 0 for Veil of Night. This is the safe default, and the Svartalfar have no other world spell to protect.
- Measure the with and without arms with `civ_ab.py --civ CIVILIZATION_SVARTALFAR` and `FFH_AI_CIV_OFF=SVARTALFAR`. Compare `taken_civ`, `lost_civ` and survival.
- Optional later (bigger): a deliberate HN raiding mode, cast when at peace with a weaker neighbour.
  - HN stacks pillage improvements (double gold with Raiders) and kill workers and settlers. They never siege.
  - Non-HN units keep the campaign role, and `campaign` counts only non-HN units as able to enter a city.
- **Risk:** not casting forfeits a once-per-game effect, which is not worth much as it stands.

### 5.2 Kidnap (SPELL_KIDNAP)

**Rules** (`req_kidnap` and `spell_kidnap`, spells_a.rs:2333-2390)
- A recon unit in a foreign city with free great-person specialists steals one as a unit.
- The chance is base strength × 8 (95% at most).
- On failure the unit is teleported home or killed, and war with the city owner follows (WARPLAN_TOTAL).

**AI:** **no `bAllowAI`**, so the AI can never cast it (the rules forbid it). **Value:** small. **Proposal:** none without a rules change.

### 5.3 Sinister (TRAIT_SINISTER)

**Rules:** recon units get `PROMOTION_SINISTER` (+1 strength).

**AI:** not seen by `best_unit`, which uses the unit info's base combat and not trait promotions, so Hunters, Rangers and Assassins are slightly undervalued. Small.

**Proposal** (optional, generic): count the owner's trait free promotions (`extra_combat_str`, combat %) in `production::best_unit` power.

### 5.4 Dark Elf race, forest maintenance, unique units

- **Dark Elf race:** +10% attack in forest, double moves in forest, -20% work rate, -25% cold resistance. Not used by the AI. Small.
- **Forest maintenance** (`bMaintain`): handled (civs/svartalfar.rs comment, docs/AI.md).
- **Swordsman:** an axeman with art changes. Handled.
- **Nyxkin:** a Horse Archer that needs no horses. Handled.
- **Illusionist:** a mage; needs adepts, so see 1.1.
- **Alazkan:** HN hero, Marksman, Black Mirror. Built (3 of 5). Being HN, he has the attack-anyone and cannot-capture behaviour of 5.1.

### 5.5 Blocked Siege Workshop and Alchemy Lab

**Rules:** no catapult, trebuchet or cannon ever.

**AI**
- Production is fine: `best_unit(Siege)` finds nothing, and `army_choice` falls back to attackers.
- `research::siege_step_tech` (research.rs:388) can still beeline Construction for a catapult the Svartalfar can never build. It finds the siege unit through `civ_unit`, which is not blocked; only the building is. Small.

**Proposal** (generic): in `siege_step_tech` and `has_siege`, skip siege units whose `prereq_building` (or class) maps to `civ_building = None` for our civ.

### 5.6 Svartalfar ranking

1. Do not cast Veil of Night (spell_value 0) and measure: medium, uncertain sign.
2. Adepts through the Mage Guild fix: small to medium.
3. Siege research skip, Sinister valuation: small.

---

## 6. Infernals (Hyborem: Barbarian ally, Spiritual, Summoner; civ trait Fallow; race Demon)

### 6.1 How they come into existence

**Trigger:** `ev_tech_acquired` (ffh/events.rs:1205-1220, a port of CvEventManager.py:1760). When the **first** team researches TECH_INFERNAL_PACT and the game option `GAMEOPTION_NO_HYBOREM_OR_BASIUM` is off, `ev_infernal_pact` (events.rs:1426) runs. Infernal Pact needs Corruption of Spirit and **state religion Ashen Veil** (`player/economy.rs:954`).

What `ev_infernal_pact` does:
- Takes the first never-alive slot (`cf_get_open_player`, ffh/custom.rs:1232; `GameSetup::SPARE_SLOTS = 4`, setup.rs:151).
- Places the new player on the best empty land plot: random 0-500, plus 2 × area size, plus 10 × unowned area tiles, +500 if the plot is unowned, +200 if it is in the founder's territory.
- Gives it all of the founder's techs, peace with the barbarians, war on everyone the founder is at war with, and +4 attitude toward the founder.
- Creates: Hyborem (Immortal), 2 Longbowmen, 2 Champions, a Worker, an Imp, 3 Manes and 2 Settlers with `PROMOTION_STARTING_SETTLER`.

**Controller:** `ffh-cli/src/run.rs:155-158` assigns the AI controller to every player id, spare slots included, so the new player is played by `ffh_ai`.

**How often** (cache): 4 of 64 full games, appearing at turns 350-450. Founders: Sheaim 2, Clan of Embers 2. Ashen Veil is a rare state religion: about 16 of roughly 480 non-Infernal civ end-states across the 80 result files (Clan of Embers 6, Sheaim 4, Calabim 2, others 1 each).

### 6.2 How the AI plays them

**Cache**
- They found 2 cities immediately, using the settler logic's "first city in place".
- They grow to 3-5 cities, population 9-46, with an army of 10-33 units. Their cities get the free Mage Guild, so they train Imps.
- 3 of 4 survive; one fell from 4 cities to 1 (seed 17).
- Hyborem stays alive.
- 3-5 Manes sit in the army until the end of the game.

**Assessment:** mostly sensible.

### 6.3 Fallow (TRAIT_FALLOW, `bIgnoreFood`), Demonic Citizens, free buildings

**Rules**
- Food yield is 0 on owned plots (`plot/geography.rs:1183`), and cities skip `doGrowth` (`city/turn.rs:350`; CvCity.cpp:938).
- Population comes only from:
  - a city's starting 3 population (`ev_city_built`, events.rs:385);
  - Manes: `SPELL_ADD_TO_CITY`, or the AI shortcut in `cf_gift_unit` (custom.rs:1282, a port of CustomFunctions.py:1080), where an AI-owned gifted Manes joins a random city with chance (100 - 5 × pop)%.
  - Manes are gifted when units with Esus, Veil or Overlords religion, or Death1 or Entropy1 promotions, die anywhere (2 per unit, events.rs:820-836), and when evil or neutral cities are razed (`ev_city_razed`, events.rs:435).
- Every Infernal city gets Elder Council, Training Yard, Obsidian Gate, Forge, Mage Guild and Demonic Citizens (no unhappiness, no unhealthiness).

**AI:** handled.
- The governor ignores food when `ignore_food` is set (governor.rs:133).
- Workers respect FfH's AI rule against farms.
- Blocked buildings are skipped through `civ_building`.

### 6.4 Manes: Add to City (SPELL_ADD_TO_CITY)

**Rules:** `UNITCLASS_MANES`, in its own city, with `bAllowAI`, sacrifices the caster for +1 population (`iChangePopulation` 1).

**AI:** never cast.
- `choose_spell` values it at 50 × 1 for population minus level × `unit_value(Manes)` for the sacrifice = 50 - 300 = **-250** (casting.rs:172-183).
- `iAIWeightCity` (10) is not read anywhere. In C++ it is only read by `AI_mageCast` for UNITAI_MAGE perm-defense units (CvUnitAI.cpp:21768; CvPlayerAI.cpp:511), so FfH's own AI never cast it either.
- The 3 starting Manes, and gifted ones the random roll did not absorb, stay as weak (2 + 1) army units: 3-5 per Infernal civ at the end.

**Value:** small. That is +3-5 population (each about +2-3 yields per turn), for a player that exists in 6% of games.

**Proposal** (`civs/infernal.rs::spell_value`): for SPELL_ADD_TO_CITY, return about 400 when the Manes stands in its own city and that city's garrison holds its threat without it; otherwise 0. In `civs/infernal.rs::unit`, send idle Manes to the smallest Infernal city.

### 6.5 Hyborem's Whisper (SPELL_HYBOREMS_WHISPER, world spell)

**Rules** (spells_a.rs:2193-2207): Hyborem, Malevolent Designs (7400 beakers), and an Ashen Veil city to target (`cf_get_ashen_veil_city`). It triggers `EVENTTRIGGER_HYBOREMS_WHISPER`.

**AI:** never in the cache; Malevolent Designs is not reached. **Value:** small. **Proposal:** none.

### 6.6 Hellfire (SPELL_HELLFIRE)

**Rules:** Infernal only (`req_hellfire`, spells_a.rs:2080), Channeling 3, Divine and Ashen Veil religion. It creates the Hellfire improvement, which spawns demon Champions.

**AI:** rare, through the generic `choose_spell` (+10 for creating an improvement). Small; no proposal.

### 6.7 Infernal ranking

1. Add to City for idle Manes: small.

Nothing else is needed: the AI already plays the new player sensibly.

---

## 7. Mercurians (Basium: Aggressive, Ingenuity, Raiders)

### 7.1 How they come into existence

**Trigger**
- Any civ completes **BUILDING_MERCURIAN_GATE**: a world wonder, Fanaticism, 600 hammers, `iAIWeight` 300, +2 prophet GP points, +4 obsolete-safe commerce.
- `cannotConstruct` (ffh/game_utils.rs:89) forbids it in the capital and under Ashen Veil state religion.
- `ev_building_built` then calls `ev_mercurian_gate` (events.rs:944 → 1524, a port of CvEventManager.py:778), which:
  - takes an open slot and adds the Mercurians **to the builder's team**;
  - moves the city's units aside;
  - creates in the gate city: Basium, a Settler, 6 Angels and 2 Workers.

**What happens next**
- Basium's SPELL_CONVERT_CITY_BASIUM (`iAIWeight` 1000; `req` and `spell` at spells_a.rs:1120 and 1129) converts the gate city, which belongs to the builder, into the Mercurians' city. `choose_spell` casts it at the end of their first turn.
- Cache: the Mercurians own the gate in 48 of 49 games.

**How often:** 49 of the 64 full games (77%). They appear at turns 300-650, median 450. Builders (the team-mates):

| Builder | Games | Builder | Games |
|---|---|---|---|
| Luchuirp | 8 | Lanun | 3 |
| Bannor | 5 | Khazad | 3 |
| Ljosalfar | 5 | Malakim | 3 |
| Balseraphs | 4 | Svartalfar | 2 |
| Hippus | 4 | Illians | 2 |
| Sidar | 3 | others | 1 each |
| Amurites | 3 | | |

The generic `building_value` (`iAIWeight` 300, +80 for GP, +320 for the commerce) makes it a popular wonder.

**Controller:** the AI, as for the Infernals.

### 7.2 How the AI plays them

**Cache**
- Usually **1 city** (the converted gate city) for the rest of the game.
- An **idle Settler** kept in 44 of 49 games, still there at the end in 41.
- Angels accumulate: 5-50, from deaths of Empyrean, Order and Runes of Kilmorph units anywhere and of Mercurian units, plus razed good and neutral cities (events.rs:837-849, `cf_gift_unit`).
- Angels upgrade into Heralds, Seraphs, Angels of Death and so on through the generic upkeep.
- The angels fight the team's wars through the Mercurians' own campaign.
- Divine Retribution is cast in 7 of 49 games.

**Idle settler:** `sites.rs` scores sites only within `SITE_RANGE` = 7 of our own cities (sites.rs:15). The single gate city sits inside the builder's culture, where no site is valid, so `expansion::settler` waits. The settler costs support all game.

### 7.3 Proposals

- **Idle settler** (`civs/mercurians.rs::unit`): use the team-mate's site list (`sites::sites` for the builder) and found there with an angel escort. Otherwise, after 20 idle turns, disband it. Small value: a second city, or about 1-2 gold a turn saved.
- **Divine Retribution** (SPELL_DIVINE_RETRIBUTION, world spell, Basium)
  - Rules: `req` at spells_a.rs:1516, a port of CvSpellInterface.py:988; for the AI, at war with 2 or more civ teams. Every demon or undead unit of **every** player, our own team included, takes 50% holy damage (spells_a.rs:1525).
  - Proposal (`civs/mercurians.rs::spell_value`): the summed power of enemy demon and undead units minus that of our team and of civs at peace; 0 unless the net is clearly positive (for example at war with the Infernals or Sheaim, or a Calabim with undead). Small value.
- **The builder's side:** neither the AI's Mercurian Gate valuation nor `victory.rs` knows that the builder hands its gate city to a new team-mate. For victory this is neutral to positive (the team keeps the city and gains Basium and an angel stream). The per-player score and share in `ffh stats` drop for the builder. Note it for measurement (aggregate by team), no proposal.

---

## 8. Suspected rules bugs

I checked each ported function against the Python or C++ original for these mechanics and found no rules bug:

- **Spells:** reqWorldbreak / spellWorldbreak, reqReligiousFervor / spellReligiousFervor, reqIntoTheMist / spellIntoTheMist, reqVeilOfNight / spellVeilOfNight, reqWane, reqDivineRetribution / spellDivineRetribution, reqConvertCityBasium, reqSandLion, reqKidnap / spellKidnap, postCombatExplode.
- **Events and helpers:** the Planar Gate, the Citadel of Light (including the noted Python quirk on `pPlot`), the Infernal Pact and Mercurian Gate spawns, `giftUnit`, `getAtWarCount(true)`.
- **Hidden Nationality:** `isAlwaysHostile`, `isNoCapture`.

Two observations for the rules owner (not bugs):

1. The AI-only gates make three world spells nearly unreachable or badly timed for any AI:
   - Worldbreak (counter ≥ 50, reached in about 3% of games);
   - Wane and Kidnap (no `bAllowAI` at all).

   Relaxing them would be a deliberate rules change.
2. `GameSetup::SPARE_SLOTS = 4`. Once rebels or `formEmpire` events use the slots (the cache shows Ljosalfar, Lanun, Elohim, Luchuirp and Sheaim extra players at slot 6 or above), a later Mercurian Gate or Infernal Pact finds no open player, and `ev_mercurian_gate` / `ev_infernal_pact` silently do nothing. The gate is still built.

AI bugs found on the way (not rules):

- **1.2:** `campaign::siege_building` and `production::enabler_building` ignore `UnitInfo::prereq_building`, so no Siege Workshop and no siege engine are ever built (0 of 4860 snapshots).
- **1.1:** adepts are never built, because `enabler_building` has no Mage role and the Mage Guild's value is 0. `research::unit_available` counts adepts as available even though nobody can train them.

## 9. Ranked proposals (expected value for winning)

| Civ | # | Proposal | Hook | Value |
|---|---|---|---|---|
| Sheaim | 1 | Mage Guild in the best cities, then adepts at war (summons with Summoner, Sundered and Stigmata); generic `enabler_building` for `Role::Mage` | production.rs:777 / civs/sheaim.rs::building_value | medium to big |
| Sheaim | 2 | Planar Gate valued by expected free creatures (eligible buildings × cap headroom × 6% a turn), when units are affordable | civs/sheaim.rs::building_value | medium |
| Sheaim | 3 | Worldbreak cast only at a strike (turn hook), Elegy only as a finisher at counter ≥ 40; rules question on the ≥ 50 AI gate | civs/sheaim.rs::spell_value, turn, production | big when it fires, rare |
| Malakim | 1 | Citadel of Light in threatened or frontier cities at war | civs/malakim.rs::building_value | small to medium |
| Malakim | 2 | Religious Fervor held while religious cities still grow, cast at once at war (with an age cap) | civs/malakim.rs::spell_value | small to medium |
| Malakim | 3 | Sand Lions through the generic Mage Guild fix; investigate the 1-city desert stalls (3/22) | generic | small |
| Sidar | 1 | Into the Mist cast at a campaign's March or StepIn (turn hook), else 50 turns after it opened | civs/sidar.rs::spell_value, turn | small to medium |
| Sidar | 2 | Wane and Shades as settled great specialists, only if the owner gives SPELL_WANE `bAllowAI` | rules, then civs/sidar.rs | small to medium |
| Svartalfar | 1 | Do not cast Veil of Night (HN army cannot capture, fights neighbours); measure with and without | civs/svartalfar.rs::spell_value | medium, uncertain sign |
| Svartalfar | 2 | Adepts through the Mage Guild fix (Arcane Faeryl, Illusionist) | generic | small to medium |
| Svartalfar | 3 | No Construction beeline for unbuildable siege; count Sinister +1 | research.rs:388, production.rs best_unit | small |
| Infernal | 1 | Manes Add to City in own cities (fallow growth) | civs/infernal.rs::spell_value, unit | small |
| Mercurians | 1 | Idle starting settler: use the team-mate's sites, or disband | civs/mercurians.rs::unit | small |
| Mercurians | 2 | Divine Retribution only when net enemy demon or undead damage is positive | civs/mercurians.rs::spell_value | small |

## 10. Observations to confirm before and after

- `ffh stats --civ CIVILIZATION_X --detail`, comparing the arms with `civ_ab.py` and `FFH_AI_CIV_OFF=X`:
  - `world_spell` together with its snapshot turn;
  - `unit_types` (ADEPT, SKELETON, SPECTRE, the Planar creatures, PRIEST_OF_*, MANES, SETTLER);
  - `building_types` (MAGE_GUILD, PLANAR_GATE, CITADEL_OF_LIGHT);
  - `project_types` (ELEGY_OF_THE_SHEAIM);
  - `world_at.armageddon`.
- `FFH_AI_TRACE=<p>` for one Svartalfar game after the Veil, counting attacks by HN units on civs at peace (the `attack`, `assault` and hunt lines). This confirms the static reading in 5.1.
- `FFH_LOG=<p>` shows the game log, including world-spell casts and "Planar Gate" messages if a gate is built.
