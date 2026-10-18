# Diagnosis B: civ-specific mechanics: Doviello, Elohim, Grigori, Hippus, Illians

Read-only static diagnosis of `projects/ffh2-rs` at commit `2d95e9c` ("one module per civilization
behind a CivPlay trait"). No games were run. The hooks named below refer to that framework
(`crates/ffh-ai/src/civs/mod.rs`): `production`, `turn` (once per turn, after diplomacy and the
view, before units move; `lib.rs:128`), `unit`, `spell_value` (end-of-turn spell choice; a value
of 0 or less means "not cast"), `building_value`, and `tech_value`. All five civ modules are empty
stubs right now (`civs/doviello.rs`, `elohim.rs`, `grigori.rs`, `hippus.rs`, `illians.rs`).

Paths: `core` = `crates/ffh-core/src`, `ai` = `crates/ffh-ai/src`, `py` =
`/home/user/ref/ffh2/Assets/python`.

---

## 0. Ranked proposals (expected value for winning)

| # | civ | proposal | value | hook |
|---|---|---|---|---|
| 1 | Illians | Hold **Stasis** (20 turns of disorder, no production and no commerce for every other player). Cast it to stop a victory rival, to protect our own near-won victory, or while our war campaign marches. Use a deadline fallback. | big | `spell_value`=0 + `turn` |
| 2 | all (Doviello first) | **Siege Workshop and Stable are never built.** `enabler_building` and `siege_building` only read `PrereqBuildingClass`, but Catapult, Chariot, War Machine and Bison Rider use `PrereqBuilding`, and `building_value` gives such buildings 0 | big (generic), medium (Doviello: War Machine, Bison Rider) | `production.rs`, `campaign.rs` |
| 3 | Hippus | Cast **Warcry** on the strike turn, not on the first turn of the war. Let `campaign::strike` reuse blitz units, which `!made_attack` currently filters out. | medium | `turn` + `campaign.rs:1005` |
| 4 | Doviello | Cast **Wild Hunt** on the strike turn, or when a city is in peril. Make the wolves army units (now `UNITAI_ANIMAL`: they wander like barbarians, are never disbanded and cost support). | medium | `spell_value`=0 + `turn`/`unit` |
| 5 | Doviello (Charadon) | Barbarian ally: barbarians count as a threat in the garrison quota, emergency and research pressure even though the team is at peace with them | small-medium | `production.rs`, `research.rs` |
| 6 | Grigori | Adventurer great people (Hero, +1 XP a turn, not able to attack): keep them safe and upgrade on purpose. Add a Grigori Medic to siege stacks. | small-medium | `unit`, `production` |
| 7 | Illians | Projects: The White Hand is built only after every building. Stir from Slumber is excluded with no reason. The Deepening is built blind. | small-medium | `production` |
| 8 | Elohim | Corlindale is never built (combat 0 fails `Role::Hero`). Hallowing (AIWeight 0) is never built. Hold Sanctuary until enemies stand in our borders. | small | `production`, `turn` |
| 9 | Illians | Wilboman is rarely built (`DefaultUnitAI` ATTACK, so not `Role::Hero`) | small | `production.rs:765` |
| 10 | Doviello | Worker to Beastman at war: a 75-hammer worker becomes a 25-hammer unit for 20 gold. Upgrade spells ignore any gold reserve. | small | `spell_value` |

Rules bugs: none confirmed for these mechanics (section 7 lists what was checked).

---

## 1. Cross-cutting findings (they decide several civ items below)

### X1. Buildings that only enable units are never built (AI gap, not a rules bug)
- **Rules:** units have two building gates. `PrereqBuildingClass` (FfH) and `PrereqBuilding`
  (BtS, one specific building) are both checked in `core/plot/units.rs:1418-1437` (`CvPlot::canTrain`).
  41 units use `PrereqBuilding`, among them Catapult, Trebuchet and Cannon (Siege Workshop), Chariot,
  War Tortoise, **War Machine** (Siege Workshop), Knight, **Bison Rider**, Death Knight and Hornguard
  (Stable), **Grigori Medic** (Infirmary), the Druids and priests (Grove, temples). The original XML
  is the same (`/home/user/ref/ffh2/Assets/XML/Units/CIV4UnitInfos.xml:19269`).
- **AI:** `production::enabler_building` (`ai/production.rs:777`, `let Some(bc) =
  info.prereq_building_class else { continue }` at :786) and `campaign::siege_building`
  (`ai/campaign.rs:1183`, the same at :1207) only look at `prereq_building_class`. BUILDING_SIEGE_WORKSHOP and
  BUILDING_STABLE have no yields, commerce, happiness, XP or AIWeight. So `building_value` returns 0
  and step 9 skips them (`if value <= 0 { continue }`, `production.rs:600`). The only remaining
  path is `fallback` (`production.rs:132`), which runs only when `decide` returns nothing, not even a
  process. So **no AI ever trains a catapult or a War Machine**. A Stable exists only where
  `enabler_building` built one for Horsemen or Horse Archers (`PrereqBuildingClass` STABLE, horse
  needed). Research is consistent with this: `research::unit_available` drops `prereq_building`
  units (`ai/research.rs:66`), so Construction's siege goal (`siege_offense`) can only ever unlock
  catapults the AI cannot train.
- **Observe:** `ffh stats --detail` → `building_types.SIEGE_WORKSHOP`, `unit_types.CATAPULT`,
  `unit_types.WAR_MACHINE`, `unit_types.BISON_RIDER` should be about 0 in every game.
- **Proposal:** in `enabler_building` and `siege_building`, treat `info.prereq_building`
  (mapped to its class, and the civ's building of that class) like `prereq_building_class`. Keep the
  133% gain test and the cap of "about half the cities". For a hero (War Machine) one city is enough:
  the most productive one.
- **Risk:** catapults become trainable for everyone. Round-3 "guns vs butter" was measured with
  siege production effectively off, so re-measure the siege share (`campaign` third-of-army rule)
  and economy on fresh seeds. The docs record that buildings taking production from the army lost
  cities. These buildings cost 100-120 hammers each.

### X2. Blitz is never used in strikes
- `campaign::strike` builds its attacker list with `!g.units[u].made_attack`
  (`ai/campaign.rs:1005`). The rules allow a unit that has attacked to attack again when it has
  `isBlitz()` (`core/unit/movement.rs:401`). `unit_can_attack` (`core/unit/combat.rs:1335`) does not
  look at `made_attack`. So Warcry (Blitz, +1 move) and every other blitz promotion give no extra
  attacks in the AI's main offensive. Fix: `!(made_attack && !g.unit_is_blitz(u))`. The "no attack
  happened" detection at :1055 still works because moves drop on every attack.

### X3. Hero detection
- `Role::Hero` is `is_hero(info) && info.combat > 0`, where `is_hero` means `DefaultUnitAI ==
  UNITAI_HERO` (`ai/production.rs:750-765`). Production step 7 (`:559`) only builds that role.
  - **Corlindale** (Elohim): `iCombat 0`, a caster hero → excluded from every role, never built.
  - **Wilboman** (Illians, str 7, 360 hammers) and **War Machine** (Doviello, str 19) have
    DefaultUnitAI ATTACK. They compete as ordinary attackers by power² per turn
    (`best_unit`, `:836`). The War Machine would win that competition but is blocked by X1.
    Wilboman (power 700, 360 hammers) loses to the Axeman and Champion in every city (worked
    example in 6.6). He is picked only by `strongest_unit` in the unaffordable fallback.
  - Magnadine (Hippus) has DefaultUnitAI HERO and combat 11, so he is handled.
- Fix: a hero is a unit whose free promotions include PROMOTION_HERO, or whose class has a global
  or player instance limit (the test `empire.rs:161` already uses). Casters among them (combat 0,
  UNITCOMBAT_ADEPT) go to `Role::Mage` and train as UNITAI_MAGE.

### X4. Owned animals act like barbarian animals
- Units created for a player with NO_UNITAI get their DefaultUnitAI (`core/player/units.rs:151-153`).
  For UNIT_WOLF that is UNITAI_ANIMAL, which `units::role_of` maps to `Role::Animal`
  (`ai/units.rs:96`), and that role is played by `units::barbarian` (`:194`, `:495`). The wolves
  attack adjacent targets at 50% odds or better, hunt weak targets within 4 turns, and otherwise
  wander to a pseudo-random reachable plot. They never join stacks or campaigns (`is_army_unit` needs an
  army UNITAI, `ai/military.rs:35`). They are never disbanded because `disband_field_units` only
  takes army UNITAIs (`ai/empire.rs:140-145`). The rules let owned animals enter owned land
  (`CvUnit.cpp:3233` "So owned animals dont have limited movement"). UNIT_WOLF has
  `bMilitarySupport 1`, so every wolf costs unit support.

### X5. Projects
- `best_project` (`ai/production.rs:698`) is reached only when the city has no building with a
  positive value (`decide`, `:678`). It skips AIWeight ≤ 0 and an exclusion list (`:708`: Samhain,
  The Draw, **Stir from Slumber**, Bane Divine, Glory Everlasting, Purge). In practice civ projects
  come very late, and Hallowing of the Elohim (AIWeight 0) never comes.

### X6. How world spells are cast now
- `casting::end_turn` (`ai/casting.rs:16`) runs `choose_spell` for every unit at the end of every
  turn. A world spell whose effect is only Python (`PyResult`) gets none of the value terms, so its
  value is its iAIWeight: Wild Hunt, Warcry, Sanctuary and Ardor 300, Stasis 50 (`:187`). Any
  positive value is cast. **The moment of the cast is therefore exactly the moment the spell's
  AI-only Python gate first passes.** Gates (all `if not human`):
  - Wild Hunt: at war with a civ and 20+ units. `core/ffh/spells_b.rs:2254`, `py
    CvSpellInterface.py:3291`.
  - Warcry: 25+ units and at war with a civ. `spells_b.rs:2215`, `py :3265`.
  - Sanctuary: at war with 2+ civs. `spells_b.rs:1339`, `py :2614`.
  - Stasis: 5+ cities. `spells_b.rs:1617`, `py :2813`.
  - Ardor: 5+ great people created. `core/ffh/spells_a.rs:863`, `py :477`.

  "At war" uses `getAtWarCount(true)`, which counts civ teams only, not the barbarians
  (`core/team/diplomacy.rs:483`). The cast logs `World spell: <name>`
  (`core/unit/ffh.rs:790`); `ffh stats` has `world_spell` (FEAT_GLOBAL_SPELL).
- To time one: return 0 from the civ's `spell_value` for that spell, and cast it from `turn` with
  `g.unit_cast(u, spell)` when the decision rule fires. Pick a caster that would not cast at end of
  turn anyway (a garrison unit), because `bHasCasted` uses up its cast for the turn. `turn` runs before
  units move. It can call `crate::campaign::plan(g, v)`, which is idempotent per View
  (`campaign.rs:503-507`), to read `v.campaign.ops[..].phase`. A spell cast there acts on this
  turn's strike.

### X7. Unit-conversion spells ignore any gold reserve
- `choose_spell` charges `-4 * cost` (`casting.rs:176`), and the rules only require `gold >= cost`
  (`core/unit/ffh.rs:388-396`; `cost` includes the leader's upgrade-cost modifier). The regular
  upgrade path keeps a reserve (`units::upkeep`, `ai/units.rs:215`: 20 + 3 × cities, doubled for
  non-defenders). End-of-turn conversions can empty the treasury. This matters for the Doviello.

---

## 2. Doviello (Charadon: Aggressive, Barbarian; Mahala: Ingenuity, Raiders; race Winterborn)

### 2.1 Wild Hunt (world spell)
- **Rules:** `spellWildHunt` (`core/ffh/spells_b.rs:2269`, `py CvSpellInterface.py:3303`). For
  every unit of ours with base strength > 0, a UNIT_WOLF appears on its plot. The wolf's strength
  is 2, or 2 + (s−2)/2 when s > 3: Beastman 3 → 2, Sons of Asena 4 → 3, Battlemaster 6 → 4,
  str 9 → 5, Bison Rider 12 → 7, War Machine 19 → 10. Wolves have 2 moves, −25% city attack and
  `UnitConvertFromCombat` Wolf Pack. AI gate: at war with a civ and 20+ units. Cast once per game.
  Nature's Revolt (any civ, 1000 hammers, AIWeight 0) would also give every animal Heroic
  Strength/Defense I-II (`events.rs:1563`). FfH's own AI favoured Nature's Revolt for the
  Doviello (`py CvGameUtils.py:2780`, +10000).
- **AI now:** cast at the end of the first turn at war with a civ with 20+ units. That is usually
  the declaration turn, while the stacks are still gathering in the rally city (`Phase::Gather`),
  so most wolves appear in our cities. The wolves are `Role::Animal` (X4): they wander, are never
  disbanded, and each costs support while the war already pushes research down. They are not in the
  campaign's groups, so `strike` never uses them.
- **Value:** medium. 20-30 free bodies, weak but expendable. Next to a target city they wear
  defenders down: `strike`'s sacrifice rule `sacrifice()` (`campaign.rs:1031`) prefers cheap units,
  and a wolf's production cost is −1, treated as 1. Today the value is close to zero, possibly
  negative through support costs.
- **Proposal** (`civs/doviello.rs`): `spell_value` returns 0 for SPELL_WILD_HUNT. In `turn`, cast
  it when one of these holds:
  - (a) A civ-war operation (`op.civ && op.at_war`) is in `Phase::Strike`, or is in `StepIn` with
    most of its committed units on the ring. The wolves then spawn next to the target and can
    attack this turn.
  - (b) A city is in `v.peril` against civ enemies. The wolves spawn on the garrison.
  - (c) The war has lasted 25 turns or more without (a) or (b), as a fallback.

  Right after the cast, set every new UNIT_WOLF to `UNITAI_ATTACK`
  (`unit_ai_set_unit_ai_type`) and `unit_join_group` it to the army group on its plot. They then
  sit in the strike pool, merge, and are disbanded first at peace. Optionally, turn the
  same-plot Form Wolf Pack casts into army units too.
- **Risk:** holding past the end of the war forfeits the window (the gate needs war), hence the
  fallback. Wolves as attackers die fast and give the enemy XP. The `strike` cover test may count
  them as force and launch weaker strikes. Measure captures per war and losses with
  `FFH_AI_CIV_OFF=DOVIELLO` against the change (`civ_ab.py`).

### 2.2 Upgrade spells (Doviello replace normal upgrades)
- **Rules** (XML `CIV4SpellInfos.xml`, no PyResult; conversion through `ConvertUnitType`):
  - UPGRADE_DOVIELLO_WARRIOR and _SLAVE: a Worker or Slave becomes a Beastman for 20 gold. AI gate
    `reqUpgradeDovielloWarrior`: at war with a civ, and no more cities than workers
    (`spells_b.rs:2107`, `py :3193`).
  - UPGRADE_DOVIELLO_AXEMAN: a warrior-class unit becomes Sons of Asena for 40 gold at Bronze
    Working, anywhere, no gate.
  - UPGRADE_DOVIELLO_CHAMPION: an axeman-class unit becomes a Battlemaster for 80 gold at Iron
    Working.

  Sons of Asena and Battlemaster have `bDisableUpgradeTo`, so the normal upgrade to them is
  impossible (`core/unit/promotions.rs:402`). Mahala's Ingenuity halves the gold
  (`unit/ffh.rs:390-391`).
- **AI now:** `choose_spell` values a conversion as `unit_value(new) − unit_value(old) − 4·cost +
  AIWeight` (`casting.rs:149-187`), with `unit_value` = attack·100 + defense·50 + AIWeight. That gives:
  - Worker → Beastman: 450 − 0 − 80 + 25 = **+395**. A worker counts as 0 because it is only
    defensive (`view.rs:76`).
  - Beastman → Sons of Asena: 600 − 450 − 160 + 50 = **+40**.
  - Sons of Asena → Battlemaster: 900 − 600 − 320 + 100 = **+80**.

  All are cast whenever gold ≥ cost. The warrior and axeman chain is adequate: it is FfH's design
  and the only Doviello upgrade path. The Worker → Beastman step is not. A worker costs **75
  hammers** (`iCost 75`); a Beastman costs 25. At war, every worker above cities − 1 is converted.
  Production step 6 then sees workers below target and trains replacements
  (`production.rs:542-551`), so the cycle repeats every few turns for the whole war. Separately, a
  Bronze Working or Iron Working tech pays 40-80 gold per eligible unit in one end of turn, with no
  reserve (X7).
- **Value:** small. The cycle costs about 50 hammers plus 20 gold per worker, and land goes
  unimproved. The gold sweep leaves no reserve for normal upgrades or slider buffers. Observe:
  `unit_types` BEASTMAN, SONS_OF_ASENA and BATTLEMASTER plus `workers` and `gold` in `ffh stats
  --detail --civ CIVILIZATION_DOVIELLO` at war turns.
- **Proposal:** in `spell_value`, value a convert spell with an old-unit hammer term: subtract the
  old unit's production cost × 15 (the hammer weight used in `building_value`) when the old unit
  is a worker. This drops Worker → Beastman except when workers are idle; also convert when the
  worker stands where it would otherwise be captured. Return 0 when `gold − cost` falls below the
  `upkeep` reserve (20 + 3 × cities).
- **Risk:** small. Fewer cheap Battlemasters at war.

### 2.3 Unique units without building prerequisites
- **Rules:** the Doviello versions of tier-2 and tier-3 units drop FfH's PrereqBuildingClass:
  Sons of Asena (no Training Yard), Javelin Thrower (no Archery Range, str 4/4, 90 hammers),
  Battlemaster (no Training Yard, 150), Circle of Urd, Skuld, Verdandi and Shaman (no Mage Guild).
  Beastman: +20% city attack. Bison Rider: Stable (PrereqBuilding) and Warhorses, **no horse
  needed**, 12/10, 2 moves. Library, Mage Guild and Alchemy Lab are blocked.
- **AI now:** adequate for the units without building prerequisites. `best_unit` simply finds
  them trainable, and `enabler_building` correctly builds no Training Yards for them (it checks
  `civ_unit(civ, class)`, `production.rs:786-793`). **Bison Rider: handled poorly** (X1). A Stable
  is built only for Horsemen or Horse Archers, which need horse, so a Doviello without horse never
  gets Bison Riders.
- **Value:** medium for the Bison Rider, the Doviello's tier-4 mounted unit.
- **Proposal:** X1. Also count the Bison Rider in `research::unit_available` once its building is
  buildable.

### 2.4 War Machine (hero)
- **Rules:** UNIT_WAR_MACHINE: str 19/19, 3 moves, HERO (+1 XP a turn), `bAutoRaze` (destroys
  non-permanent improvements on enemy plots it enters at war, `core/unit/lifecycle.rs:1165`), Machinery,
  Siege Workshop (PrereqBuilding), 240 hammers, one in the world.
- **AI now:** never built: no Siege Workshop (X1). If it were, `best_unit(Attack)` would choose it
  (power 1900 ×1.5 for moves +200, about 3.3M score against about 0.8M for a Bison Rider), as a
  normal UNITAI_ATTACK. Research never values Machinery for it (`research.rs:66-69` drops heroes
  and `prereq_building` units).
- **Value:** medium. One unit about three times as strong as anything else of its era.
- **Proposal:** X1 (one Siege Workshop in the best production city once Machinery is known).
  Optionally add a `tech_value` bonus to Machinery for the Doviello once Iron Working and
  Engineering are known.

### 2.5 Charadon's TRAIT_BARBARIAN (barbarian ally, −10% research)
- **Rules:** the team starts at peace with the barbarians (`core/game/setup_init.rs:447`). The
  barbarians never declare war on it, so animals and barbarian units ignore it and it cannot attack
  them. FfH's city AI sets the defenders such a team needs to 1 (`CvCityAI.cpp:10079`).
- **AI now:** campaigns correctly target barbarian cities only at war (`campaign.rs:358`), and the
  danger map only counts enemies. But `production::barbarians_near` (`production.rs:235`) checks
  no war status. Through it, a city near barbarian units or lairs adds +1 to the garrison quota
  (`:401`), wants a second defender after turn 30 (`:113`, `:438`), and keeps 2 defenders in
  `empire.rs:98` and `finance.rs:341`. Research pressure counts barbarian units within 8 plots
  (`research.rs:176-189`), which can force the Archery defense goal (`research.rs:308`).
- **Value:** small-medium. It hits Charadon games only: hammers and early research spent defending
  against units that cannot attack.
- **Proposal:** in `barbarians_near` and the research pressure count, ignore barbarian units and
  lairs when `!g.at_war(team, barb_team)`. This is generic: it also fixes the Clan of Embers.
  Lairs still spawn barbarians who are allies, so they are not a threat either.
- **Risk:** low. If the barbarian team ever goes to war with Charadon (an event), the war check
  restores the old behaviour.

### 2.6 Other Doviello items
- Lucian, free at start (a UNITAI_HERO melee unit, str 4, one in the world): an Army unit. The
  disband code protects him (`empire.rs:161`). Adequate.
- Conquest mode at Festivals (`core/ffh/events.rs:1378`): an FfH AI signal nothing reads (only the
  heroes' groupflags, `events.rs:614`). Ignore.
- Scorch is blocked on our own snow (`spells_b.rs:1403`): an AI gate that already protects us.
- Palace: Chaos, Earth and Body mana. Generic.

---

## 3. Elohim (Ethne: Creative, Defender, Tolerant; Einion: Defender, Philosophical, Tolerant; free Ancient Chants)

### 3.1 Sanctuary (world spell)
- **Rules:** `spellSanctuary` (`spells_b.rs:1354`, `py :2625`): `sanctuary_timer += 30`, and every
  foreign unit in our borders jumps out. While the timer runs, no unit of another team can enter
  our plots (`core/unit/movement.rs:493`). AI gate: at war with 2+ civs.
- **AI now:** cast at the end of the first turn with 2 civ wars, whether or not any enemy is near.
  The AI declares one war at a time (diplomacy), so this needs a second civ to declare on us. It is
  rare; defensive pacts may make it less so.
- **Value:** small. It is rare, but it can save a civ that is being overrun.
- **Proposal** (`civs/elohim.rs`): `spell_value` = 0. In `turn`, cast when the gate passes and
  either enemy civ units stand inside our borders within 3 plots of a city, or `v.peril` lists a
  city. Fall back to casting after 15 turns of 2+ wars.
- **Risk:** low. Holding while the enemy has not arrived spends none of the 30 turns.

### 3.2 Corlindale (hero)
- **Rules:** UNIT_CORLINDALE: UNITCOMBAT_ADEPT, `iCombat 0`, 2 moves, Fanaticism (1800 beakers), 300
  hammers. Promotions: HERO, Channeling I-III, Earth I, Spirit I, Mind I. Spell SPELL_PEACE
  (sacrifice him: peace with everyone and half the Armageddon counter; `spells_a.rs:2533-2558`).
  The Shrine of the Champion requires the civ's hero (`core/ffh/game_utils.rs:76-86`).
- **AI now:** **never built** (X3: combat 0, not `Role::Mage` because DefaultUnitAI is HERO). Peace
  would never be cast anyway: its value is −level × unit_value + 0. That is correct.
- **Value:** small. One high-channeling caster hero who gains XP; end-of-turn casting would use
  his buffs and summons.
- **Proposal:** X3. A combat-0 hero caster goes to `Role::Mage` and trains as UNITAI_MAGE.
  `Role::Caster` already moves casters with the army.

### 3.3 Hallowing of the Elohim (project, repeatable)
- **Rules:** Way of the Wise (280 beakers), 600 hammers, Armageddon counter −5, needs the counter
  ≥ 1. Unlimited instances, AIWeight 0 (`CIV4ProjectInfo.xml`, around line 273).
- **AI now:** never built (X5).
- **Value:** small, and depends on how far the counter climbs in AI games (`armageddon` in
  `world_at`).
- **Proposal:** `production` hook: build it in the best production city when the counter is within
  10 of the next Armageddon threshold, or ≥ 30. Use the alignment weight that `events.rs` already
  uses for the counter.

### 3.4 Monk, Chancel of Guardians, Reliquary, Devout
- **Monk:** Priesthood (beelined, `tech_plan.rs:328`), str 6/6, 2 moves, Medic I, Demon Slaying,
  no building, 120 hammers. `best_unit` picks it as attacker and counter (power 750 at 120
  hammers, against a Champion's 600 that needs a Training Yard and Iron Working). Adequate.
- **Chancel of Guardians:** Priesthood, 120 hammers, +25% defense; new units have a 20% chance of
  Defensive (`core/ffh/events.rs:683`). `building_value` counts the defense (12, or 75 under
  threat) and not the promotion. Adequate; the value is small.
- **Reliquary:** Way of the Wise, +1 Prophet point, +2 culture, free Spirit Guide (half the XP of
  a dying unit passes on, `events.rs:851`). Valued through great people and culture. Adequate.
- **Devout** (assassin class): FfH used them as terraformers, which this AI does not port. Small.
- Conquest mode at Priesthood: unused, as everywhere.

---

## 4. Grigori (Cassiel: Adaptive, Industrious, Philosophical; civ trait Agnostic; free Crafting)

### 4.1 Ardor (world spell)
- **Rules:** `spellArdor` (`spells_a.rs:875`, `py :486`) sets great_people_created and the
  threshold modifier to 0. AI gate: 5+ great people created. Each great person adds +100 (own) and
  +100 (team, which includes us) to the modifier while fewer than 10 have been created (Alt defines
  are 100; `core/player/units.rs:865-875`). After 5, the threshold is 1100% of base; after Ardor,
  100%.
- **AI now:** cast at the end of the turn the 5th great person appears.
- **Value:** large effect, already captured. With one reset, getting N great people costs about
  n² + (N−n)², which is smallest at n = N/2. A gate at 5 is near-optimal for 10-15 great people a
  game. **Handled adequately.** Optional refinement: delay to about half the great people
  expected by the end (from the current rate), which matters only for Grigori with a very high GP
  rate.

### 4.2 Adventurers (great-person class)
- **Rules:** the Palace (+2), Adventurers' Guild (+2, +25% GP rate, +2 XP for land units, Currency,
  120 hammers) and Grigori Tavern (+1) produce UNITCLASS_ADVENTURER points. Early Grigori great
  people are therefore mostly Adventurers, and each one raises the GP threshold. The Adventurer:
  str 2/2, only defensive, HERO (+1 XP a turn, 20% magic resistance), DefaultUnitAI ATTACK, upgrades
  to the Adept, Warrior or Scout classes.
- **AI now:** born with UNITAI_ATTACK, so they become `Role::Army` (`units.rs` `role_of`). With 0
  attack power they join stacks or hold in cities, and may march with siege stacks. `upkeep`
  upgrades them as soon as gold exceeds twice the reserve, choosing the target with the highest
  attack power. Adept (300) and Warrior (300) tie, and the Adept class comes first in class order;
  the Adept needs a city able to train it (Mage Guild). The XP and Hero promotion carry over, so
  the generic chain (Warrior → Axeman → Champion/Dragon Slayer) eventually turns them into strong,
  high-level units. It works, but no plan behind it.
- **Value:** small-medium. 3-6 extra hero units a game whose level grows by one about every 10-20
  turns. As high-level Adepts they could reach tier-2/3 spells (in FfH, adventurers become archmages).
- **Proposal** (`civs/grigori.rs` `unit`): an Adventurer stays as a garrison in the safest city
  until it upgrades. Upgrade to the Adept/Mage line once the empire knows Sorcery (or the first
  Mage-level tech), with Channeling promotions from its XP. Otherwise go melee once the best melee
  unit is tier 3 or higher. Never upgrade at level 1 to a Warrior.
- **Risk:** low. They are units that would otherwise idle.

### 4.3 Grigori Medic
- **Rules:** Medicine (beelined after Iron Working, `tech_plan.rs:337`), Infirmary
  (PrereqBuilding), 180 hammers, str 4/4, Medic I-III (heals its plot and neighbours), upgrades to
  Luonnotar at level 6 (Druid class: str 10, magic immune, Strength of Will).
- **AI now:** `best_unit(Attack)` rates it at power 400 for 180 hammers, so it is never chosen.
  The Infirmary is valued by its health and heal rate, so a few cities have one. Handled poorly.
  FfH's AI built one per military unit count (`py CvGameUtils.py:2026-2069`).
- **Value:** small-medium. Campaign stacks spend turns healing (`military.rs` step 5). Medic III
  roughly doubles the healing of a stack and its neighbours.
- **Proposal:** `production` hook: while a civ-war operation exists and has fewer medics than
  stacks, train a Grigori Medic in a city with an Infirmary as UNITAI_ATTACK so it joins stacks.
  Optional: add a `building_value` bonus for one Infirmary in the best military city.
- **Risk:** 180 hammers per medic taken from attackers.

### 4.4 Other Grigori items
- **Agnostic:** no state religion; temples blocked. The research plan takes Sanitation instead of
  an early religion (`tech_plan.rs:180`). Adequate.
- **Dragon Slayer** (a Champion with Dragon Slaying and Courage) and **Luonnotar** (above): generic.
- **Adaptive:** the trait choice every 100 turns is valued by `events.rs` `trait_value`. Adequate.
- Conquest mode at Medicine: unused.

---

## 5. Hippus (Tasunke: Aggressive, Raiders; Rhoanna: Expansive, Financial; civ trait Horselord)

### 5.1 Warcry (world spell)
- **Rules:** `spellWarcry` (`spells_b.rs:2230`, `py :3276`) gives every unit with a unit combat
  PROMOTION_WARCRY. Warcry: +1 strength, +1 defense, +1 move, Blitz, 5% chance a turn to expire
  (about 20 turns on average). AI gate: 25+ units and at war with a civ.
- **AI now:** cast at the end of the first turn of a civ war with 25+ units, normally while the
  operation is in `Gather`. Campaigns spend about half their turns gathering (docs/AI.md "Sieges"),
  so a large part of the expected 20 turns is spent before any strike. Blitz is then unused in the
  strike (X2).
- **Value:** medium. On str 4-6 mounted units, +1 strength is +17-25%. Blitz turns each unit with
  moves left into several attacks: Horselord gives +1 move and Warcry another.
- **Proposal** (`civs/hippus.rs`): `spell_value` = 0. In `turn` (after `campaign::plan`), cast
  when a civ-war operation is in `Strike`, or in `StepIn` and expected to strike next turn. Also
  cast when an enemy stack makes `v.peril` list a city. Fall back to casting after 20 turns at war.
  Together with X2.
- **Risk:** no strike comes before the war ends, hence the fallback. Blitz extra attacks at low
  health can lose units; the strike's odds and cover checks still apply.

### 5.2 Magnadine (hero)
- **Rules:** Warhorses (7400 beakers), horse, 360 hammers, str 11, 4 moves, Commando, Courage,
  HERO. SPELL_HIRE_UNITS has no `bAllowAI`, so the AI may never cast it (rules).
- **AI now:** DefaultUnitAI HERO and combat > 0, so production step 7 builds him once he is
  trainable. The palace's free Horse covers the resource. Research does not value Warhorses for
  him (heroes are excluded); the Knight term may get there. Adequate once Warhorses is known.

### 5.3 Horselord, palace horse, mounted play
- PROMOTION_HORSELORD: +1 move and +10% withdrawal for mounted units. Passive.
  `best_unit`/`type_attack_power` ignore free promotions, a small tilt against mounted units.
- `enabler_building` builds Stables when a Horseman (400 against a Warrior's 300, a gain of exactly
  133%) or a Horse Archer (Stirrups, beelined) opens. Knights then work in those cities.
  Adequate.
- Arquebus and Cannon are blocked (Cannon is blocked by X1 anyway). The Alchemy Lab is blocked.
- Conquest mode at Horseback Riding or Stirrups: unused.
- `EVENT_MERC_PAYMENT_3` and the Hippus-only Guild of the Nine mercenary (Recruit Mercenary costs
  180 gold, so its spell value is −720): generic, not cast. Fine.

---

## 6. Illians (Auric: Agnostic, Charismatic, Defender; race Winterborn; free Exploration)

### 6.1 Stasis (world spell): the biggest lever in this group
- **Rules:** `spellStasis` (`spells_b.rs:1628`, `py :2820`): every other living player, barbarians
  included, gets disable_production and disable_research += 20 (normal speed; 14, 30 or 60 on the
  other speeds). Production 0 (`core/city/production.rs:888`), every commerce type 0, including gold
  and culture (`core/player/economy.rs:689`), every city in **disorder** (`core/city/yields_culture.rs:210`),
  no maintenance (`economy.rs:381`). In effect, 20 turns of anarchy for the whole world. AI gate:
  5+ cities.
- **AI now:** value 50 (AIWeight), cast at the end of the first turn with 5 cities, whatever the
  situation.
- **Value:** big. Cast at any time, it is a 20-turn setback for every rival (FfH's design). In
  this AI, where nearly every decided full game is a **cultural** victory (docs/AI.md "Known gaps",
  Victory pursuit), the decisive uses are these:
  - (i) freeze a rival's culture when `victory::plan(g, p).rival` is near its goal;
  - (ii) protect our own cultural run: no rival can build an army to stop us, and their culture
    stops;
  - (iii) war conversion: the target cannot train a single defender for 20 turns, while our
    campaign's need stays fixed.
- **Proposal** (`civs/illians.rs`): `spell_value` = 0 for SPELL_STASIS. In `turn`, cast when the
  gate passes and one of these holds:
  - (a) a rival's victory ETA is ≤ 40 turns (`victory::plan().rival`);
  - (b) our own goal's ETA is ≤ 40 turns;
  - (c) at war with a civ (our declaration or theirs), with an operation against that civ in
    `March`, `StepIn` or `Strike`;
  - (d) a city in `v.peril` against civ enemies;
  - (e) fallback: eligible for 80 turns without (a)-(d), or game turn ≥ 60% of the turn limit.

  Optionally let `diplomacy` count "Stasis available" as extra relative power when preparing a war.
- **Risk:** holding it gives up the generic setback in games where no trigger fires before the
  fallback. A/B against the current cast-at-5-cities (`FFH_AI_CIV_OFF=ILLIANS`, `civ_ab.py`,
  full games with `--until-supremacy`; decisive measures: wins, rival cultural wins prevented,
  captures). Few Illian AIs may reach 5 cities: observe `cities` and `world_spell` for Illians.

### 6.2 Ritual projects
- **Rules** (`CIV4ProjectInfo.xml`; effects in `core/ffh/events.rs:1115-1150`):

  | project | tech, hammers, AIWeight | effect |
  |---|---|---|
  | **The White Hand** | Philosophy, 400, 300 | three Priests of Winter (str 5 + 2 cold, Medic I-II, Channeling I-II, Ice I, Divine, Winterborn; upgrade at level 6 and Theology to High Priest) |
  | **The Deepening** | Priesthood, 600, 300 | `events.rs:1655`: a 25% roll on every land plot of the world: tundra becomes snow, grass, marsh and plains become tundra, desert becomes plains, each for rand(40 + 20 × speed index) + 10 turns (10-89 on normal); rare blizzards |
  | **Stir from Slumber** | Divine Essence, 900, 300 | Drifa: a dragon, str 14 + 7 cold, 3 moves, first strike, magic immune, crosses impassable terrain. `cannotCreate` requires players_killed > 0 (`game_utils.rs:257`) |
  | **The Draw** | Strength of Will, 900, 300 | war on every team, our units ≥ 50% damage, our cities' population halved, Armageddon +10 (`events.rs:1704`) |
  | **Ascension** | Omniscience, needs The Draw, 1200, 1000 | Auric Ascended (str 30 + 30 cold, flying); the strongest rival gets the Godslayer (`events.rs:1739`) |
  | **Samhain** | no tech, 200, 100 | barbarian Frostlings and Mokka spawn; 20 turns of happiness for us |

  Genesis is blocked for the Illians (`game_utils.rs:260`).
- **AI now:**
  - The White Hand and The Deepening are built by `best_project`, only after every
    positive-value building (X5), so late.
  - The Draw and Samhain are correctly excluded: they harm us, or spawn chaos that hurts us too.
  - Ascension is unreachable without The Draw. That is fine: too late and too risky.
  - **Stir from Slumber is excluded although it has no side effect.** It is gated only by the
    players-killed rule.
  - The Deepening is built blind. The Illians gain only where snow replaces tundra on plots they
    own. The Illian snow yield (+2 food) applies only to plots owned by an Illian player
    (`core/plot/geography.rs:1075`, C++ quirk). The Illians lose like everyone else on their own
    grass and plains.
- **Value:** small-medium. The White Hand gives three good units for 400 hammers. Drifa is one
  monster unit, but rare. The Deepening is a temporary, symmetric-ish yield cut whose sign depends
  on the map.
- **Proposal:** `production` hook:
  - White Hand: build it like an army item. When the army is short or a war is near, one city
    builds it once Philosophy is known.
  - Stir from Slumber: build it in the best city when cannotCreate passes.
  - The Deepening: build it only if the expected food change on our worked plots (tundra becoming
    snow: +1; grass or plains becoming tundra: −1 food or −1 hammer, all × 25%) beats the rivals'
    average by a margin. Use the rules' yield functions. Otherwise never.

  Remove PROJECT_STIR_FROM_SLUMBER from the `best_project` exclusion list (`production.rs:708`).
- **Risk:** project hammers taken from buildings. All are one-time.

### 6.3 Temple of the Hand (unique pagan temple)
- **Rules:** on build (`core/ffh/events.rs:976-1011`, `py CvEventManager.py:830`), every non-water
  plot within 2 of the city becomes **snow** permanently. Smoke is removed; jungle becomes forest;
  flames, flood plains and scrub are removed. For Illian-owned plots, snow yields 2/0/0: grass is
  unchanged, plains lose the hammer and gain a food, tundra gains a food, desert gains two.
  Farms and cottages stay valid; workshops do not. The city plot becomes snow, which gives Illian
  Winterborn defenders +10% defense. A non-Illian conqueror gets 0-food land.
- **AI now:** `building_value` counts only the pagan temple's +10% culture, the priest slot and
  incense happiness, so it is built late and blind, including in plains- or flood-plain-heavy
  production cities.
- **Value:** small. In tundra or desert cities it is a food gain and a defense gain.
- **Proposal:** `building_value` hook: add the yield change of the snow conversion on the city's
  plots (worked plots fully, others at 60%, food at the governor's weights), plus a small defense
  term. Skip it when the conversion would lose more than one hammer a turn on worked plots.

### 6.4 Snow yield and site choice
- **Rules:** see above (owned plots only).
- **AI now:** `found_value` (`core/found_value.rs:207-213, 489-500`) uses
  `plot_calculate_best_nature_yield` on unowned plots, so Illian snow reads as 0/0/0 and snow
  sites are avoided. This is faithful to C++.
- **Value:** small (usually few snow sites on generic maps).
- **Proposal:** none now. If ever, adjust the site scores in `sites.rs` for Illians only; do not
  change the rules function.

### 6.5 Priest of Winter spells, Whiteout
- Call Blizzard (iAIWeight 50) moves an adjacent blizzard onto the caster and turns grass, plains
  or desert there into tundra **permanently** (`spells_a.rs:940`). When a blizzard is adjacent,
  the AI casts it anywhere, including on our own good land. Small. Proposal: in `spell_value`,
  return 0 when the caster stands on a plot of ours that is grass or plains.
- Whiteout (an assassin on snow becomes Hidden): harmless buff; adequate.

### 6.6 Wilboman (hero)
- Iron Working (beelined), 360 hammers, str 7, HERO, Sentry, immune to cold. X3: rarely built.
  At 10 hammers a turn, score = power² / 100 × 1000 / (turns + 4): Wilboman 4900×1000/40 ≈ 122k;
  Axeman 1600×1000/10 = 160k; Champion 3600×1000/16 = 225k. He loses even in a strong city.
  Small. Proposal: X3.

### 6.7 Other Illian items
- Agnostic: research takes Sanitation instead of a religion (`tech_plan.rs:180`). Adequate.
- Conquest mode at Iron Working: unused.
- Letum Frigus (Aggressive for the Illians) and the other unique-feature events trigger for humans
  only (`core/ffh/random_events_a.rs:601-606`). Not available to the AI.

---

## 7. Rules checks (possible port bugs)

Compared the port with the Python: reqWildHunt, spellWildHunt, reqWarcry, spellWarcry,
reqSanctuary, spellSanctuary, reqStasis, spellStasis, reqArdor, spellArdor, reqAddToWolfPack,
spellAddToWolfPack, reqFormWolfPack, spellFormWolfPack, reqUpgradeDovielloWarrior, reqWhiteout,
reqCallBlizzard, the Temple of the Hand onBuildingBuilt, and the project effects. No discrepancy
found. Notes, none a bug:
- `py_cannot_create` (`core/ffh/game_utils.rs:247`) deliberately omits the Python's AI-only
  branches: Samhain only with 4+ cities, and the early "≤2 warriors / base production ≤10"
  limits. The doc comment says so. The AI's `best_project` exclusion covers Samhain.
- The Illian snow bonus applies only to owned plots: a C++ quirk, ported faithfully
  (`plot/geography.rs:1073-1078`).
- The Python's AI unit-AI picks for new adepts and Devouts in `onUnitBuilt` are left to the AI
  (`events.rs:665`). That is intended, but it is why Devouts and Shamans get no terraformer role.

## 8. Observations to confirm the static claims (no game was run)

All of these come from `ffh stats --detail --civ CIVILIZATION_<X>` (unit, building and project
type counts, `world_spell`) and `FFH_LOG` / `FFH_AI_TRACE`:
- **X1:** `building_types.SIEGE_WORKSHOP` and `unit_types.CATAPULT` / `WAR_MACHINE` / `BISON_RIDER`
  are about 0 for every civ; the trace never shows "builds BUILDING_SIEGE_WORKSHOP".
- **World spells:** the turn of `world_spell` becoming true (`--every 5`) against `wars`. The log
  line "World spell: Wild Hunt" / "Stasis" / "Warcry" falls on the first war turn, or the first
  turn with 5 cities.
- **Wolves:** `unit_types.WOLF` stays high long after the Wild Hunt; `ucost` (unit support) rises
  at the cast.
- **Doviello conversions:** `unit_types.WORKER` and `BEASTMAN` while at war, against `workers` at
  peace.
- **Projects:** the turn of `project_types.THE_WHITE_HAND` / `THE_DEEPENING`. Hallowing never
  appears.
- **Heroes:** `unit_types.CORLINDALE` never appears; `WILBOMAN` is rare; `MAGNADINE` appears after
  Warhorses.
