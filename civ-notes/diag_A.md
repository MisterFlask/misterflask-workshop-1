# Diagnosis A: civilization mechanics vs. the AI. Amurites, Balseraphs, Bannor, Calabim, Clan of Embers

This was a read-only static diagnosis. I ran no games and built nothing. The tree was read at
`2d95e9c` ("one module per civilization behind a CivPlay trait"). In that commit `civs.rs` became
`crates/ffh-ai/src/civs/`, and the five civs here have empty modules (`civs/amurites.rs` and so on,
each `impl CivPlay for X {}`). The hooks that now exist are `production`, `turn`, `unit`,
`sea_worker`, `spell_value`, `building_value` and `tech_value` (`civs/mod.rs`). The proposals below
use these hooks. There is one new one: a `civic_value` hook, needed for the Bannor.

All paths are under `projects/ffh2-rs/` unless they start with `/home/user/ref/ffh2` (the original
FfH2 sources).

## 0. How the generic AI treats civ-specific content (applies to everything below)

1. **End-of-turn spells.** `casting::end_turn` (`crates/ffh-ai/src/casting.rs:16`) makes every unit
   cast the spell `choose_spell` picks (`casting.rs:100`). `choose_spell` is a port of
   `CvUnit::chooseSpell` (`/home/user/ref/ffh2/CvGameCoreDLL/CvUnit.cpp:15917`): the value is a sum of
   terms read from the XML, plus `iAIWeight`, and a spell is cast only if that value is **> 0**
   (`casting.rs:191`). Two consequences:
   - A spell that works only through Python (`PyResult`), has no XML effect fields and has
     `iAIWeight` 0 scores 0 and is **never cast**. Examples: Teach Spellcasting, Gift Vampirism,
     Escape.
   - World spells have `iAIWeight` 300 and no unit prerequisite beyond the civilization, so **any
     unit of the civ** (a warrior, a worker) casts them at the **first end of turn** the spell's
     `req*` passes. The only timing control is the AI-only gate inside the ported `req*` function.
     Those gates are kept in the rules port (unlike the AI-only branches of `cannotTrain` and
     `cannotConstruct`, which `ffh/game_utils.rs:10-12` deliberately omits).
   - `iAIWeightCity` and `iAIWeightWar` are dead data: read in `CvInfos.cpp:3391` but unused by C++,
     and unused by the port.
2. **Buildings whose effect lives in Python** (unit-built hooks and the like) score 0 in
   `generic_building_value`. `decide` skips any value ≤ 0 (`crates/ffh-ai/src/production.rs:600`).
   Such a building is reachable only through `fallback` (`production.rs:132`), which `decide` almost
   never reaches. So these buildings are **never built**. The fix point is
   `CivPlay::building_value`, called from `production.rs:958`.
3. **Heroes** (`DefaultUnitAI = UNITAI_HERO`) are trained by step 7 of `decide`
   (`production.rs:559`) once a city builds one in ≤25 turns or the civ has ≥3 cities.
   `units::role_of` gives them the Army role through its default branch (`units.rs:97`), and the
   campaigns accept hero-led groups (`campaign.rs:419`).
4. **Unit AI types set by the rules are not all in the AI's sets.**
   - `military::is_army_unit` (`military.rs:35`) accepts only ATTACK, ATTACK_CITY, COLLATERAL,
     COUNTER, RESERVE and PILLAGE.
   - `campaign::army_group` (`campaign.rs:414`) adds HERO.
   - `production::count_ai(UNITAI_ATTACK)` counts by AI type.
   - The rules put Vampires on `UNITAI_FEASTING` (`crates/ffh-core/src/ffh/events.rs:609`), which none
     of these accept (see D1).
5. **Civics ignore anarchy.**
   - `empire::civics` switches whenever the summed gain is > 100 (`empire.rs:281`). docs/AI.md ("A
     turn", item 1) says civics change "when the gain beats the anarchy", but the code never charges
     the anarchy.
   - In FfH, anarchy puts **every city in disorder**: `city_is_disorder` includes
     `player_is_anarchy` (`crates/ffh-core/src/city/yields_culture.rs:210`). That means no production
     (`city/production.rs:878`, `city/turn.rs:1041`) and no commerce (`yields_culture.rs:862`).
   - Each revolution costs `iAnarchyLength` + `BASE_CIVIC_ANARCHY_LENGTH` (1) + cities × 8%
     (standard map) turns (`player/civics_religion.rs:446`). That is ≥2 turns of total stoppage for
     an ordinary civic and ~6 for Crusade.
   - A golden age makes anarchy 0 (`civics_religion.rs:454`).
6. **AI-initiated deals skip `player_can_contact`.**
   - `diplomacy::manage` offers peace (`diplomacy.rs:147-176`) after checking only
     `player_can_trade_item`.
   - C++ `CvPlayer::canContact` (`CvPlayer.cpp:3893`; port `player/diplomacy_trade.rs:99`) refuses
     contact with enemies under `bNoDiplomacyWithEnemies` (the Crusade civic). C++ `AI_doDiplo` only
     talks to players it `canContact`.
   - This is an AI legality issue, not a rules bug (see C7).
7. **Typed mana nodes are built only for the Tower victory.**
   - Raw `BONUS_MANA` is placed on the map. `BONUS_MANA_*` have no placement data (`iPlacementOrder`
     unset), so the typed kinds appear only when an adept builds `BUILD_MANA_X` (its improvement's
     `BonusConvert`).
   - The AI's only node builder is `victory::mana_node` (`victory.rs:896`), which returns
     immediately unless the victory goal is the Tower. docs/AI.md calls that goal out of reach in
     every measured game.
   - So the world holds almost no typed mana. This matters for the Amurites (A1, A6).
8. **Measuring.** `ffh stats --detail` gives each civ's `unit_types`, `building_types` and
   `project_types` counts plus a `world_spell` flag (`crates/ffh-cli/src/stats.rs:442`).
   `crates/ffh-cli/scripts/civ_ab.py --civ CIVILIZATION_X` forces a civ into slot 0 and reports its
   usage counts and win and share deltas. `FFH_AI_CIV_OFF=CIVILIZATION_X` turns its civ logic off.
   The rules log `World spell: <name>` when a world spell is cast (`unit/ffh.rs:790`).
   `FFH_AI_TRACE` shows production choices (`production.rs` traces) but not civics or individual
   casts.

---

## A. Amurites

Leaders: Valledia (Arcane, Organized) and Dain (Arcane, Philosophical); both have `iAIValueMage`
100. Hero: Govannon. Palace gives Metamagic, Fire and Body mana (`BUILDING_PALACE_AMURITES`).
Unique units: Swordsman, Wizard, Firebow, Chanter. Unique building: Cave of Ancestors. World spell:
Arcane Lacuna. Tech plan beelines Bowyers, then Sorcery (`crates/ffh-ai/src/tech_plan.rs`
`civ_beelines`).

### A1. Arcane Lacuna (world spell)

- **Rules.**
  - XML `data/xml/Units/CIV4SpellInfos.xml:176`: needs a `UNITCOMBAT_ADEPT` caster, `bGlobal`,
    `iAIWeight` 300.
  - Port `crates/ffh-core/src/ffh/spells_a.rs:819` (req) and `:832` (effect); Python
    `CvSpellInterface.py:438` and `:452`.
  - Effect: every adept-combat unit of ours gains XP equal to the number of typed mana nodes on the
    **whole map**. Every other team gets spellcasting disabled for 20 turns (14/30/60 at
    quick/epic/marathon). `unit_can_cast` then refuses them every non-`bAbility` spell, enemy world
    spells included (`unit/ffh.rs:176`, the `disable_spellcasting` check).
  - **AI-only gate:** at least 7 typed mana nodes on the map.
- **Current AI.**
  - `choose_spell` value is 300 (AIWeight only), so the first adept to reach the end of a turn with
    the gate open casts it.
  - The gate counts `map.num_bonus` of the 17 `BONUS_MANA_*` types. Typed nodes are almost never
    built (section 0, item 7).
  - So the gate is **probably never met**, and the spell is never cast in AI-only games. If it ever
    is, it is cast at a random moment (peace, with 1-2 adepts).
  - To confirm: the `world_spell` flag of Amurite players (`civ_ab.py --civ CIVILIZATION_AMURITES`)
    and the absence of the log line `World spell: Arcane Lacuna`.
- **Value: small-medium.**
  - XP equals the node count per adept, e.g. +8 XP to 3 adepts.
  - The real prize is 20 turns of enemy casters switched off during a war against a magic civ
    (Sheaim, Svartalfar, Luchuirp, Amurites).
- **Proposal.**
  - Fix the node gap first (A6).
  - `Amurites::spell_value`: return 0 for `SPELL_ARCANE_LACUNA` except when either:
    - we are at war (or `diplomacy::preparing_target`) with a team owning ≥3 adept-combat units; or
    - we own ≥ max(4, cities/2) adepts and the map has ≥10 typed nodes.
  - Fallback: release it after turn ~450 so it is not wasted.
  - **Risk:** holding it until the civ dies. The deadline covers that.

### A2. Cave of Ancestors

- **Rules.**
  - XML `data/xml/Buildings/CIV4BuildingInfos.xml:5617`: CIV_BUILDING1, Sorcery, 120 hammers, no
    yields, `iAIWeight` 0.
  - Effect (`ffh/events.rs:689`; Python `CvEventManager.py:1397`): an adept-combat unit trained in
    the city gains XP equal to the number of `BONUSCLASS_MANA` resources the city has. That is ≥3
    for any city connected to the Amurite palace (Metamagic, Fire, Body).
- **Current AI.** Never built: value 0, filtered at `production.rs:600`. FfH's Python AI gave it
  +630 in cities with a Mage Guild (`CvGameUtils.py:1715`).
- **Value: small.**
  - +3 XP (about one promotion: level 2 needs 2 XP) per adept from that city.
  - Grows with mana types (A6).
- **Proposal.**
  - `Amurites::building_value(BUILDING_CAVE_OF_ANCESTORS)` = 40 × (mana types in the city), but
    only in the city that trains casters. Take that as the best-production city with a Mage Guild.
  - **Risk:** negligible (120 hammers).

### A3. Govannon and Teach Spellcasting

- **Rules.**
  - Govannon: Arcane Lore, 480 hammers, Channeling 1-3, Hero (`data/xml/Units/CIV4UnitInfos.xml:34309`).
  - Teach Spellcasting (`CIV4SpellInfos.xml:4087`; port `spells_b.rs:1994` and `:2014`; Python
    `CvSpellInterface.py:3041` and `:3095`): every alive non-animal, non-bird unit of ours on his
    plot, **of any type**, gains each tier-1 sphere promotion Govannon has, plus Channeling 1.
- **Current AI.**
  - He is trained by step 7 (`production.rs:559`) and acts as Army.
  - Teach Spellcasting scores 0 (no `iAIWeight`, no XML effect), so it is **never cast**.
  - His spheres come from the free mana promotions in `ev_unit_created` (needs ≥2 of a mana type)
    and from XP picks (`units::promotion_value`; sphere-1 promotions have `iAIWeight` 50).
- **Value: small, medium with good spheres.**
  - The taught units become buffers and casters for the end-of-turn loop. For example, Body 1 gives
    Haste. Spirit 1, Enchantment 1 and Chaos 1 buff a whole stack.
  - With only the palace spheres (Fire 1 is Blaze, which is useless), the gain is modest.
- **Proposal.**
  - `Amurites::spell_value(SPELL_TEACH_SPELLCASTING)` = 60 × the number of (learner, new
    promotion) pairs on the plot whose promotion unlocks a spell with `AddPromotionType1`, a summon
    or damage. In practice he casts when standing with an army stack.
  - **Risk:** taught soldiers cast odd spells (Blaze). Ignore promotions whose only spells create
    features or improvements.

### A4. Wizard, Firebow, Chanter

- **Wizard** (`CIV4UnitInfos.xml:49174`): a Mage with a free Spellstaff. Spellstaff
  (`CIV4SpellInfos.xml:2788`) consumes the promotion to reset `has_casted`. `choose_spell` scores it
  as −`promotion_value(Spellstaff)` < 0, so it is never used. That is one lost extra cast per Wizard.
  **Small**, no proposal.
- **Firebow:** a Longbowman with Fire 1 and Channeling 2. Generic class member; Fire 1 gives Blaze
  only. Adequate.
- **Chanter:** an Assassin with Escape (teleport to the capital). The AI-only gate allows it only
  under 50% damage (`CvSpellInterface.py:1141`, port `spells_a.rs:1683`), and it scores 0, so it is
  never cast. **Small.**

### A5. Leaders, events

- **Arcane** (free Potency for adepts), **Organized**, **Philosophical:** passive.
- `startConquestMode` on Bowyers (`ffh/events.rs:1351`) only feeds religion-hero group flags, which
  this AI does not read.
- Civ-gated events (Amurite trial, `ffh/random_events_a.rs:774`; `EVENT_LETUM_FRIGUS_2`) go through
  the generic `events.rs` valuation. Adequate.

### A6. Arcane play: casters, mana nodes, free sphere promotions

- **Rules.**
  - An adept-combat unit created when the player owns ≥2 of a mana type gets that sphere's tier 1.
    With ≥3 and Channeling 2 it gets tier 2; with ≥4 and Channeling 3, tier 3
    (`ffh/events.rs:592`, the `ADEPT_MANA_PROMOTIONS` loop; Python `CvEventManager.py:1187`).
  - The palace gives 1 Fire, 1 Body and 1 Metamagic.
  - One Fire node (Elementalism; `BUILD_MANA_FIRE`, iTime 1200) gives every new adept Fire 1.
    A second gives every new Wizard Fire 2, i.e. Fireball, a summon the end-of-turn loop already
    uses near enemies.
  - Mana resources also have empire effects (e.g. Law: −5% maintenance per node).
- **Current AI.**
  - Caster target = (cities + 2)/4, +1 only if `iAIValueMage` > 100, which no Amurite leader has
    (`production.rs:655`). That is about 2 casters at 6 cities, built after buildings.
  - Casters follow the army or sit in the most threatened city (`units.rs:605`).
  - No typed nodes are built (section 0, item 7).
  - FfH's own AI let Arcane leaders train adepts without its AI limit (`CvGameUtils.py:332-337`)
    and pushed Mage Guilds and Archery Ranges for the Amurites (`CvGameUtils.py:1593`).
- **Value: medium.** This is the Amurite identity. Typed nodes also open A1 and feed A2. The gain
  is unmeasured.
- **Proposal.**
  1. `Amurites::unit` for adepts and mages in peacetime: the nearest raw `BONUS_MANA` plot inside our
     borders. Build the type that lifts a count from 1 to 2 or from 2 to 3 for a sphere with
     combat value, Fire first (Fireball), then Body, Enchantment, Spirit, Death, Law.
     Reuse `victory::mana_jobs`-style job claims.
  2. `Amurites::production`: caster target (cities + 2)/2 from the Cave city once Sorcery is known.
- **Risk.** The docs record production taken from the army costing cities ("Looked promising...
  failed on fresh ones"). Casters are part of the army here, but they are weak alone. Pair with a
  cap at war (the existing `ctx.afford`) and measure with `civ_ab.py --civ CIVILIZATION_AMURITES`.
  A known gap also stands: "casters do not join the strike" (docs/AI.md, "Sieges").

---

## B. Balseraphs

Leaders: Perpentach (Arcane, Charismatic, Creative, **Insane**) and Keelyn (Creative, **Summoner**).
Hero: Loki. Unique buildings: Arena (Training Yard), Freak Show (CIV_BUILDING1), Hall of Mirrors
(CIV_BUILDING2). Unique units: Swordsman, Mimic, Harlequin, Taskmaster, Courtesan, plus the
civ-only Freak. Spells: Revelry (world spell), Create Puppet (adepts), Add to Freak Show (slaves),
Arena Battle. Tech plan beelines Iron Working.

### B1. Revelry (world spell)

- **Rules.**
  - XML `CIV4SpellInfos.xml:2430`: Festivals, `bGlobal`, `iAIWeight` 300.
  - Port `spells_b.rs:947` and `:961`; Python `CvSpellInterface.py:2348` and `:2357`.
  - Effect: a golden age of 2 × `golden_age_length()`. `GOLDEN_AGE_LENGTH` is 12 in
    `data/xml/GlobalDefinesAlt.xml:66`, so 24 turns at normal speed.
  - **AI-only gate:** total population ≥ 25, and not already in a golden age.
- **Current AI.** Any Balseraph unit casts it at the first end of turn with 25 population. That is
  **adequate**: the bonus starts early and compounds.
- **Value: medium.** +1 hammer and +1 commerce on every worked plot that produces some, for 24
  turns: about 600 hammers and 600 commerce at 25 population. A golden age also makes anarchy 0
  (section 0, item 5), and that is not exploited.
- **Proposal (generic, cheap).** In `empire::civics` (`empire.rs:250`), re-check civics every turn
  while `player_is_golden_age` instead of on the 10-turn cadence, because a revolution is free then.
  Optionally hold Revelry (`Balseraphs::spell_value` 0) until a civic switch is due, a war is being
  prepared, or 35 population, with a deadline.
  **Risk:** low. The cadence change also affects other civs' golden ages (Great Person ones), so
  measure it generically.

### B2. Loki: Disrupt, Entertain, Create Puppet Loki

- **Rules.**
  - Loki has no tech prerequisite, costs 180 hammers, has 2 strength, `bRivalTerritory`, Magic
    Immune and 2 moves (`CIV4UnitInfos.xml:46985`).
  - `bRivalTerritory` lets him enter any territory (`CvUnit::canEnterTerritory`, `CvUnit.cpp:2926`),
    and so stand inside a non-enemy rival city.
  - **Disrupt** (`CIV4SpellInfos.xml:839`, `iAIWeight` 50; port `spells_a.rs:1491` and `:1500`;
    Python `CvSpellInterface.py:964`), cast in a city not of his team:
    - +2 turns of hurry anger;
    - the owner's culture there drops by 0-2;
    - if that culture falls below 1, the city is acquired with no war, and the victim's attitude
      gets −4.
  - **Entertain** (`iAIWeight` 50; `spells_a.rs:1669`), in any city: +2 turns on the happiness timer.
    In a foreign city it also takes population/2 + 1 gold from the owner.
  - **Create Puppet Loki** (`iAIWeight` 1000).
- **Current AI.**
  - Production step 7 trains Loki early (≤25 turns or ≥3 cities). FfH's AI never built Loki
    through its hero routine (`CvGameUtils.py:1529-1531`) and required Festivals
    (`cannotTrain`, `CvGameUtils.py:364`). The port omits that branch as AI policy.
  - As Army he is a 2-strength unit that hunts and joins stacks.
  - At end of turn, Create Puppet Loki (≥1000) beats Disrupt and Entertain (50). So every turn he
    summons a 2-strength puppet that suicide-attacks at ≥5% odds (`casting.rs:44-66`).
  - Disrupt is **never used** (he is never sent into a foreign city).
- **Value: medium.**
  - Stealing freshly founded rival cities: their culture is a few points with a near-zero rate, and
    a 0-2 per turn Disrupt drains that in a few turns. Cities with ≥1 culture per turn will not fall.
  - Today, 180 early hammers are wasted.
- **Proposal.**
  - `Balseraphs::production`: no Loki before Festivals and before a Disrupt target exists.
  - `Balseraphs::unit` for Loki:
    - Pick the reachable city of a rival at peace (or neutral) whose owner culture is lowest and
      whose culture rate is ≤1. Prefer cities fewer than ~20 turns old.
    - Go there and stay. Re-target if the city's culture outgrows him.
  - `Balseraphs::spell_value`:
    - Disrupt = 2000 inside such a city;
    - Create Puppet Loki = 0 unless enemies are adjacent at war;
    - Entertain = 300 in a big rival city when there is no Disrupt target.
  - **Risk.** Each flip costs −4 attitude (wars). He is bumped out when war is declared. A rival
    city's rate may outpace him.

### B3. Create Puppet (adepts)

- **Rules.**
  - XML `CIV4SpellInfos.xml:613`: `iAIWeight` 1000, copies the caster's promotions.
  - The puppet is a summon with duration 2 (+1 with Summoner; `unit/ffh.rs:1210`), −20% combat,
    and can cast the adept's sphere spells from the next turn.
- **Current AI.**
  - Every Balseraph adept casts it every turn: 1000 beats any summon or damage value (combat × 100,
    about 300-600).
  - So the adepts never cast their own Fireballs or buffs (`buff_cast` and `summon_cast` skip
    casters that have cast).
  - The puppet attacks at once at ≥5% odds and usually dies before it can cast.
  - This is FfH's own behaviour.
- **Value: small-medium.** It decides whether Balseraph magic does anything.
- **Proposal.**
  - `Balseraphs::spell_value(SPELL_CREATE_PUPPET)` = the caster's best other castable value, + 1
    only when no enemy is within the summon's reach (a free extra caster in peace or defence).
    Otherwise 0, so the adept casts its own spell now.
  - Optionally make suicide summons attack only at ≥30% odds (generic, in `cast_and_act`).
  - **Risk:** low.

### B4. Hall of Mirrors

- **Rules.**
  - XML `CIV4BuildingInfos.xml:7436`: Alteration, 180 hammers, needs a Carnival in the city,
    +1 happiness.
  - Each turn, if an at-war enemy unit is adjacent, an Illusion copy of it (duration 3, or 5 with
    Summoner) appears in the city (`ffh/events.rs:521`; Python `CvEventManager.py:2261`).
- **Current AI.** Only the +1 happiness is valued, so it is built only when happiness is short.
- **Value: small** (defence of border cities).
- **Proposal.** `Balseraphs::building_value` adds ~150 in cities within 6 plots of an at-war
  enemy's border. Low priority.

### B5. Arena

- **Rules.** It is the Training Yard (Bronze Working), so it enables tier-2 melee. Arena Battle
  (`CIV4SpellInfos.xml:211`) has no `bAllowAI`, so `unit_can_cast` refuses it to the AI.
- **Current AI.** `enabler_building` (`production.rs`) builds it when it opens a stronger unit.
  **Adequate.**

### B6. Slaves into cages; the Freak Show

- **Rules.**
  - Add to Freak Show Human/Elf/Dwarf/Orc/Dark Elf (`CIV4SpellInfos.xml:112`): `bAllowAI`,
    `iAIWeight` 30, in our city, sacrifices a slave.
  - It creates that race's cage: +1 happiness, +1 culture, +1 artist Great Person point; one cage
    per race per city.
  - Slaves come from enslavement (Taskmaster and Courtesan have 25%).
- **Current AI.**
  - The value is 30 + `building_value(cage)` − level × unit value (0 for a slave), so it is cast
    whenever a slave happens to end a turn in our city.
  - Slaves are workers (`UNITAI_WORKER`) and rarely do. **Rarely used.**
- **Freak Show building:** `iCost` −1 and nothing grants it, so it is unobtainable in FfH 0.41 data.
  That is not a port bug.
- **Value: small.**
- **Proposal.** A worker step: a slave with no job above a threshold walks to the nearest city
  lacking its race's cage.

### B7. Freak, Mimic, Harlequin, Taskmaster, Courtesan

- **Freak** (`CIV4UnitInfos.xml:31332`: Festivals, 3 strength, Mutated, 60 hammers): `best_unit`
  never picks it over 4-strength units at the same cost. FfH's AI valued it as an Axeman + 3.
  Small.
- **Mimic** gains a defeated foe's promotion (`postCombatMimic`, `CvSpellInterface.py:153`).
  **Harlequin**'s Taunt has no `bAllowAI`. **Taskmaster** and **Courtesan** enslave. All generic.
  **Adequate.**

### B8. Traits

- **Insane** (`ffh/events.rs:277`; `random_events_a.rs:2298`): 2% per turn, every trait is replaced
  by 3 random selectable ones. Nothing to decide.
- **Summoner:** +1 summon duration. Passive. Nothing to do.

---

## C. Bannor

Leaders: Sabathiel (Charismatic, Organized), Capria (Industrious, Spiritual), Decius (Organized,
Raiders). Civ trait: Guardsman. Hero: Donal Lugh. Unique civic: **Crusade**. Units: Demagog and
Flagbearer, both Crusade-only. World spell: Rally. Tech plan beelines Fanaticism.

### C1. Crusade civic (the Bannor's central mechanic)

- **Rules.**
  - XML `data/xml/Gameinfo/CIV4CivicInfos.xml:1359`: Fanaticism, Bannor only, **`bPrereqWar`**
    (needs a war with a civ), `iAnarchyLength` 5, `iAIWeight` 1000.
  - Benefits:
    - +2 happiness from the state religion;
    - +25% unit production under the state religion;
    - free unit support worth 50% of population;
    - −75% war weariness.
  - **`bNoDiplomacyWithEnemies`:** the Bannor cannot contact its enemies (`diplomacy_trade.rs:99`).
  - Unlocks the **Demagog** (Iron Working, Training Yard, 60 hammers, 5 strength, **`bAbandon`**;
    `CIV4UnitInfos.xml:24108`) and the **Flagbearer** (Fanaticism, 120 hammers, Morale aura;
    `:30886`).
  - Every turn, each town we own with no visible enemy spawns a Demagog with
    `CRUSADE_SPAWN_CHANCE` = 20% and becomes a village (`ffh/custom.rs:94`; Python
    `CustomFunctions.py:80`, `CvEventManager.py:536`).
  - **Blocks:**
    - Workers, Settlers and Workboats (`ffh/game_utils.rs:191`; `CvGameUtils.py:382`);
    - 17 buildings: Elder Council, Market, Monument, Moneychanger, Theatre, Aqueduct, Public Baths,
      Herbalist, Carnival, Courthouse, Gambling House, Granary, Smokehouse, Library, Harbor,
      Alchemy Lab, Brewery (`game_utils.rs:47-71`; `CvGameUtils.py:438-471`).
  - At peace the civic is lost (`player_can_do_civics`, `civics_religion.rs:97`;
    `player_verify_civics`, `:210`), and every Demagog then abandons at its next turn
    (`unit/lifecycle.rs:522`).
  - Anarchy on adopting it is 5 + 1 + cities × 8% ≈ 6 turns (standard, normal), with no production
    and no commerce anywhere (section 0, item 5).
- **Current AI.**
  - `empire::civic_value` (`empire.rs:183`) gives Crusade 1000 × cities (from `iAIWeight`), +40 ×
    cities for the state-religion happiness, +18 for war weariness, minus upkeep.
  - Its rivals in the option are Religion at 200 × cities, Social Order and Sacrifice the Weak at
    700 (each needs a specific religion), Scholarship at 300.
  - `empire::civics` checks every 10 turns and switches on gain > 100, with no anarchy term.
  - Result: a Bannor AI that knows Fanaticism switches to Crusade **at the first 10-turn check of
    any war with a civ**, even a defensive war about to end. Then:
    - ~6 turns of empire-wide disorder, in the middle of the war;
    - no settlers or workers, and the core economy buildings are blocked;
    - its towns turn into villages and Demagogs within ~5 turns;
    - at peace the civic drops automatically, the Demagogs vanish, and the next war repeats the
      cycle.
  - **Handled poorly, likely net harmful.** I cannot settle this statically. To confirm (no civic
    trace exists):
    - the `anarchy_turns` field in Bannor snapshots (`ffh stats`), sampled densely;
    - DEMAGOG and FLAGBEARER in `unit_types` (`--detail`);
    - the Bannor's `world_spell` flag (Rally needs Crusade);
    - better, add a trace line in `empire::civics` when a revolution happens.
- **Value: big, both ways.** Now it is probably a recurring 6-turn production and commerce
  blackout per war plus a lost economy. Used deliberately, it is a free army (C2) and +25% unit
  production in a decisive war.
- **Proposal.**
  1. **Generic anarchy cost.** In `empire::civics`, require gain > anarchy turns × (empire production
     × 15 + commerce × 10). Use `player_get_civic_anarchy_length`; it is free in a golden age.
     This fixes the docs/code mismatch for all civs.
  2. **A `civic_value` hook on `CivPlay`**, called from `empire::civic_value`. `Bannor` returns 0
     for Crusade unless all of these hold:
     - at war with a civ, in a war we chose: a `campaign` operation at war, `victory::wants_conquest`,
       or `press_conquest`;
     - the war is not expected to end soon (`want_peace` false, enemy defensive power below ours);
     - ≥5 cities (Rally fires immediately, C2);
     - no settler sites left on our landmasses;
     - no structural deficit.
     When they hold, value = (cities + towns) × 5-strength unit value + 25% of unit production +
     the support saved, minus the blocked buildings in queue.
- **Risk.**
  - Crusade locks diplomacy: after fix C7 the Bannor cannot sue for peace while in it.
    So adopt it only when winning.
  - A Bannor that never crusades plays like a generic civ, the safe default.

### C2. Rally (world spell)

- **Rules.**
  - XML `CIV4SpellInfos.xml:2215`: `bGlobal`, `iAIWeight` 300.
  - Port `spells_b.rs:526` and `:541`; Python `CvSpellInterface.py:2056` and `:2065`.
  - Requires Crusade. **AI-only gate:** ≥5 cities.
  - Effect: a Demagog in every city and on every town without visible enemies; each town becomes a
    village.
- **Current AI.**
  - Cast at the first end of turn under Crusade with ≥5 cities, even during the anarchy:
    `bAbandon` treats anarchy as valid (`lifecycle.rs:533`).
  - The Demagogs come with `NO_UNITAI`, so the default `UNITAI_ATTACK`. They are army units
    (`is_army_unit`) and join the war campaign.
  - Timing is fine *given* a good Crusade, but it inherits C1's bad timing: a short war means the
    Demagogs abandon at peace.
- **Value: medium.** About cities + towns 5-strength units: e.g. 8 + 6 = 14 units, roughly 840
  hammers, once per game (Birthright Regained, Omniscience, has no `iAIWeight` and is never built:
  `production.rs:698`).
- **Proposal.** `Bannor::spell_value(SPELL_RALLY)` = 0 unless the C1 conditions hold, i.e. the
  war has a campaign target and is not winding down. **Risk:** low.

### C3. Demagog, Flagbearer, Donal

- **Demagog:** a 5-strength unit for 60 hammers, the best `best_unit` choice while under Crusade.
  It abandons at peace, so hammers are lost if the war ends. Proposal:
  `Bannor::production` prefers non-abandoning attackers once `want_peace` is true.
- **Flagbearer:** Morale is an `AddPromotion` buff (Morale `iAIWeight` 50), cast by `buff_cast`
  (`casting.rs:210`). **Adequate.**
- **Donal Lugh** (Fanaticism, 300 hammers): step 7, Army. **Adequate.**

### C4. Guardsman trait, leaders

- **Guardsman:** free on archer and melee units; passive.
- **Leaders:** generic.
- **Conquest mode at Fanaticism** (`events.rs:1363`): no effect in this AI.
- **Bannor events** (`random_events_a.rs:2578`, `EVENT_IMMIGRANTS_7`): generic valuation.
  **Adequate.**

### C7. AI legality under Crusade (report)

- Under `bNoDiplomacyWithEnemies`, C++ `canContact(eEnemy)` is false (`CvPlayer.cpp:3893`), so a
  Crusading Bannor may not open talks with an enemy.
- `diplomacy::manage` initiates AI-to-AI peace (`diplomacy.rs:147-176`) through `implement_deal`
  without `player_can_contact(p, other)`.
- The enemy offering peace to the Bannor is legal: the flag is checked only on the contacting side.
- **Fix:** check `player_can_contact` before any AI-initiated deal (peace, trades, war bribes) in
  `diplomacy.rs` and `trade.rs`, as `offers.rs:61` already does for human recipients.

---

## D. Calabim

Leaders: Alexis (Aggressive, Philosophical), Flauros (Financial, Organized), Decius. Hero: Losha
Valas. Unique buildings: Governor's Manor (Courthouse), Breeding Pit (CIV_BUILDING1). Blocked: Alchemy
Lab, Elder Council. Unique units: Bloodpet (Warrior), Moroi (Axeman), Vampire (Champion), Brujah
(Berserker), Vampire Lord (Immortal). Vampire spells: Feast, Feed, Gift Vampirism. World spell:
River of Blood. Tech plan beelines Feudalism (Vampires).

### D1. Vampires are on `UNITAI_FEASTING` and fall outside the army (the main Calabim gap)

- **Rules.**
  - Every `UNIT_VAMPIRE` created gets `UNITAI_FEASTING` (`ffh/events.rs:609`; Python
    `CvEventManager.py:1175`). That includes Moroi upgraded into the Champion class. `unit_init`
    runs `ai_init` first, then `ev_unit_created` (`unit/lifecycle.rs:156` and `:186`), so the
    trained `UNITAI_ATTACK` is overwritten.
  - The Vampire Lord's XML default AI is also FEASTING, and upgrades keep the AI type.
  - The C++ AI's `AI_feastingmove` (`CvUnitAI.cpp:20068`) fed in cities above size 13 until 30 XP,
    then joined conquest. ffh-ai has no FEASTING logic (grep: none).
- **Current AI.**
  - `role_of` sends them to Army through the default branch.
  - But `is_army_unit` (`military.rs:35`) rejects FEASTING, and so do `campaign::army_group` and
    `force_on` (`campaign.rs:397`, `:414`). Vampires therefore:
    - are never counted in a siege force or committed to an operation;
    - are never merged into stacks by `merge_here` (`military.rs:158`);
    - and a vampire at the head of its own group **absorbs** fresh ATTACK units on its plot into a
      non-army group, pulling them out of the campaign too.
  - `production` counts attackers by `count_ai(UNITAI_ATTACK)`, so vampires never fill the army
    target, and more attackers are trained with their unit support.
  - Vampires still defend cities in peril and hunt.
  - **Handled poorly (a gap from the rules setting a unit AI the new AI does not know).**
- **Value: medium-big.** Vampires (and every upgraded Moroi) are the Calabim's tier-3 to 4 melee.
  Feudalism is their beeline. Their wars then muster and strike without them.
- **Proposal.** Two options:
  - `Calabim::unit`: at the start of a vampire group's decision, set FEASTING units to
    `UNITAI_ATTACK` with `unit_ai_set_unit_ai_type`. That is AI bookkeeping, as `production.rs:52`
    already does for garrisons. Unless D2 decides they feast this turn.
  - Or generically add `UNITAI_FEASTING` to `is_army_unit`, `army_group` and the attacker count.
  - **Risk:** none rules-wise. FEASTING is read only by the unported C++ AI.
  - To confirm: `FFH_AI_TRACE` "campaign vs" / "next to" lines listing the units around a target
    (VAMPIRE absent), or count Calabim vampires per `unit_types` vs. stacks at war.

### D2. Feast, Gift Vampirism, Feed

- **Rules.**
  - **Feast** (`CIV4SpellInfos.xml:1228`; port `spells_a.rs:1774` and `:1783`; Python
    `CvSpellInterface.py:1207`): a vampire in our city of population ≥3. The city loses 1
    population (`iChangePopulation` −1), the vampire gains population − 2 XP, and the city gets 3
    turns of hurry anger.
  - **Gift Vampirism** (`:1443`; `spells_a.rs:1941`; Python `:1321`): Vampire (+10% attack and
    defence, +5 healing everywhere) goes to every level ≥6 unit, and every level ≥4 Moroi, in the
    caster's stack.
  - **Feed** (`:1247`, `iAIWeight` 10; `spells_a.rs:1791`): needs a Bloodpet in the stack.
    **AI-only gate:** damage ≥ 20. Heals 20 damage and kills the lowest-level Bloodpet there.
- **Current AI.**
  - Feast scores 50 × (−1) = −50, so it is **never cast**.
  - Gift Vampirism scores 0, so it is **never cast**.
  - Feed scores 10, so it is cast whenever a hurt vampire shares a plot with a Bloodpet. That
    includes a city's only garrison Bloodpet. The emergency code then rebuilds the defender.
- **Value: medium.**
  - A capped size-12 city feasts for +10 XP (1-2 promotions) at the cost of a citizen who does not
    work anyway.
  - Gift gives +10% and healing to veterans.
- **Proposal.**
  - `Calabim::spell_value`:
    - Feast = 400 when the city is at its happy or health cap (`happy_room ≤ 0`), population ≥ 8,
      the unit has < 30 XP, and its stack is not mid-strike;
    - Gift Vampirism = 150 × qualifying stack-mates;
    - Feed = 0 if the Bloodpet is the city's last defender.
  - `Calabim::unit`: a vampire with < 30 XP in such a city holds one turn to feast. That mirrors
    FfH's own rule.
  - **Risk:** hurry anger in a city at the cap. Use the cap test, and prefer the largest city.

### D3. River of Blood (world spell)

- **Rules.**
  - XML `CIV4SpellInfos.xml:2467`; port `spells_b.rs:1073` and `:1087`; Python
    `CvSpellInterface.py:2426`.
  - Every city of every other living player (barbarians included) with population > 2 loses 2.
    Every Calabim city gains 2.
  - **AI-only gate:** ≥5 cities.
- **Current AI.** `iAIWeight` 300, so any Calabim unit casts it at the first end of turn with 5
  cities. **Adequate, but the own half is mostly wasted.** The AI's cities sit at their happy cap,
  and +2 citizens there do not work, eat, and starve back down.
- **Value: medium.** For example +10 own population at 5 cities, and roughly −2 × (rival cities
  above size 2), about 40-60 rival population on a standard map. It grows with the world's size,
  and score v2 is a share of the world's people.
- **Proposal.**
  - `Calabim::spell_value` = 0 unless one of these holds:
    - the sum over our cities of min(2, happy room, health room) ≥ cities/2;
    - a war against a neighbour is starting (`preparing_target`);
    - a domination goal is active (`victory::plan`).
  - Deadline fallback around turn 350.
  - **Risk:** dying with it unused (the deadline covers that). Holding a world spell has no other
    cost.

### D4. Governor's Manor, Breeding Pit, Bloodpet, Losha

- **Governor's Manor** (`CIV4BuildingInfos.xml:9060`: −20% maintenance, war weariness, crime,
  `bUnhappyProduction`): valued as a courthouse through its maintenance. It is also the city
  prerequisite for training Vampires, which nothing values.
  `Calabim::building_value` could add ~200 in the main military city. **Adequate to small.**
- **Breeding Pit** (`:5394`: +2 food, 20% food kept): valued by the yield and `food_kept` terms.
  **Adequate.**
- **Bloodpet** (Warrior with 3 strength) and **Losha** (hero, step 7): **Adequate.**
- Calabim events (Calabim Sanctuary etc., `random_events_a.rs:957`, `:2412`): generic valuation.

---

## E. Clan of Embers

Leaders: Jonas Endain (**Barbarian**, Expansive, Spiritual) and Sheelba (Aggressive, **Barbarian**,
Organized). Race: Orc. Hero: Rantine. Unique building: **Warrens**. Blocked: Library, Mage Guild,
Alchemy Lab, Stable. Units: Goblin, Shaman, Wolf Rider, the Lizardman line, the Ogre line.
World spell: For the Horde. Tech plan: no Writing (`tech_plan.rs:192`) and an Iron Working beeline.

### E1. Warrens (the biggest single gap in this group)

- **Rules.**
  - XML `CIV4BuildingInfos.xml:6992`: Masonry (140 beakers), 180 hammers, no yields, `iAIWeight` 0.
  - Effect (`ffh/events.rs:764`; Python `CvEventManager.py:1444`): **every unit finished in the
    city that is not a world, national, siege or naval unit spawns a free copy**, with the city's
    production experience. **Settlers and workers are copied too**: they have no unit combat and
    are not national units.
- **Current AI.** Value 0, filtered at `production.rs:600`, so **never built.** FfH's Python AI
  valued it at 70 × production, and 12001 in conquest mode (`CvGameUtils.py:1891`, `:1921`).
- **Value: big.** It doubles the military output of every city that has one. 180 hammers are
  repaid by the third 60-hammer unit. It doubles settlers (expansion) and workers. For a civ with
  −10% research and no Libraries, conquest is the road to winning.
- **Proposal.**
  - `ClanOfEmbers::building_value(BUILDING_WARRENS)` = city production × 15 (a hammer's worth in
    `building_value`) × 6/10 (the share of production that goes to units).
  - It then ranks like a top economy building and comes early in the best-production cities.
  - **Risk.**
    - Doubled units mean more unit support. The finance guard disbands the weakest; watch
      `support_heavy`.
    - The twin of a settler waits for its own escort, which is fine.
    - The documented failure mode, buildings that take hammers from the army, is inverted here: the
      Warrens *increases* army output.
  - Measure with `civ_ab.py --civ CIVILIZATION_CLAN_OF_EMBERS --off-base`; the `BUILDING_WARRENS`
    usage count goes from 0 up.

### E2. Rantine: Convert City

- **Rules.**
  - Rantine: Bronze Working, 180 hammers, 4 strength, Orc, Hero (`CIV4UnitInfos.xml:60872`).
  - Convert City (`CIV4SpellInfos.xml:525`, `iAIWeight` 1000; port `spells_a.rs:1140` and
    `:1160`; Python `CvSpellInterface.py:693`), in a **barbarian** city where no barbarian unit has
    a higher base strength: the city is acquired (not as a conquest). Rantine is not sacrificed, so
    he can repeat it.
  - At peace with the barbarians, Clan units may enter barbarian land and cities:
    `canEnterTerritory` returns true for the barbarian team (`CvUnit.cpp:2966`), and a peaceful
    city is not an enemy city.
- **Current AI.**
  - Rantine is trained at step 7 and acts as Army.
  - At peace with the barbarians, the Clan's armies have no barbarian targets: `find_targets` and
    lairs need war (`military.rs:590`), and peacetime campaigns need war (`campaign.rs:358`).
  - So he never walks into a barbarian city. The cast would fire (1000) if he stood in one.
  - **Never used.**
- **Value: big, early to mid game.** Free cities. The Clan **cannot take barbarian cities any
  other way while at peace**, and docs/AI.md counts ~6 barbarian cities per game, half within 6
  plots of a civ city. Garrisons stay ≤ 4 strength until the barbarians reach Iron Working units.
- **Proposal.**
  - `ClanOfEmbers::unit` for Rantine: the nearest revealed barbarian city reachable within ~15
    turns whose visible units all have `unit_base_combat_str` ≤ his. Move there and hold; the
    end-of-turn cast converts it.
  - Re-target after each conversion.
  - Prefer cities in or near our land.
  - **Risk.** Hidden stronger units (re-check on arrival). Rantine absent from a war (skip while
    at war with a civ near him).

### E3. For the Horde (world spell)

- **Rules.**
  - XML `CIV4SpellInfos.xml:1306`; port `spells_a.rs:1824` and `:1840`; Python
    `CvSpellInterface.py:1242` and `:1255`.
  - Requires peace with the barbarians. **AI-only gate:** the barbarian player owns ≥60 units,
    animals included.
  - Effect: each non-hero orc barbarian unit has a 50% chance to become a Clan unit where it stands.
    Every non-animal, alive, land barbarian unit gets the Orc race (`unit/lifecycle.rs:173`).
- **Current AI.** Any Clan unit casts it at the first end of turn the gate opens, in peace or war.
  The converted units, with default AI (ATTACK, so Army), are scattered over the map and cost unit
  support at peace. `empire::disband_field_units` trims field units to 1 + cities/4 when research
  falls to ≤20% (`empire.rs:129`). Hammers are free, but much of the value is lost.
- **Value: medium-big.** 10-30 units at once. Converting them also removes the barbarians that
  harass the Clan's rivals.
- **Proposal.**
  - `ClanOfEmbers::spell_value` = 0 unless at war with a civ or `preparing_target` is set, or
    Rantine has a barbarian city target whose garrison this would thin (E2).
  - Deadline fallback when the barbarian unit count starts falling.
  - **Risk:** the gate closes later as barbarians shrink (low on most maps).

### E4. TRAIT_BARBARIAN: permanent peace with barbarians and animals

- **Rules.**
  - `bBarbarianAlly`, −10% research (`data/xml/Civilizations/CIV4TraitInfos.xml:540`).
  - The barbarian team never declares war on the Clan (`init_diplomacy`, `game/setup_init.rs:447`).
    The Python war on far-ahead barbarian allies is for humans only (`CvEventManager.py:569`).
- **Current AI.**
  - Danger, threat, hunting and campaigns read at-war teams only (`view.rs:92`,
    `production::local_threat`). That part is correct.
  - But `production::barbarians_near` (`production.rs:235`) ignores war state, so the Clan over-
    defends against neighbours who cannot attack it:
    - +1 to the garrison quota near any barbarian or lair (`production.rs:401`);
    - a second defender after turn 30 (`:438`; emergency `:113`);
    - two defenders kept near barbarians when disbanding (`empire.rs:98`, `finance.rs:341`).
  - Research pressure counts barbarian units within 8 plots (`research.rs:179-189`). That pushes
    the Archery `defense_goal` and the weight of military techs.
  - The minimum mobile force exists "for barbarians and animals" (`production.rs:535`), which the
    Clan cannot fight.
  - Settler escorts are still built.
  - FfH's own AI gave barbarian allies 1 defender per city and patrols of 2
    (`CvCityAI.cpp:10079`, `:10167`; `CvUnitAI.cpp:1719`).
- **Value: medium-small.** About one defender (60 hammers plus support) per border city, an early
  research detour, and slower expansion.
- **Proposal (generic, changes only barbarian allies).**
  - `barbarians_near` returns false when `!g.at_war(team, barbarian_team)`.
  - Skip barbarian units in research pressure under the same condition.
  - Build the minimum mobile force only at war or while preparing one.
  - Escort settlers only when civ danger exists on the path.
  - **Risk:** civ enemies. The at-war threat logic stays.

### E5. Blocked buildings, units, race

- Library, Mage Guild, Alchemy Lab and Stable are blocked by the rules, and the tech plan already
  skips Writing.
- Orc race (+25% fire resistance, −10% work rate), Lizardmen (amphibious, marsh bonuses), Ogres
  (8-strength Champion class), Stoneskin Ogre, Wolf Rider, Goblin: `best_unit` picks them by power.
- Conquest mode at Masonry: no effect.
- **Adequate.**

---

## F. Suspected rules bugs and data issues

I checked these ported functions against the Python line by line:
- req/effect pairs: Arcane Lacuna, Rally, Revelry, River of Blood, For the Horde, Feast, Feed,
  Gift Vampirism, Convert City Rantine, Disrupt, Entertain, Escape, Arena Battle, Add to Freak Show
  Human, Teach Spellcasting (all 18 spheres, Ice included);
- `doCrusade`, the Warrens and Cave of Ancestors hooks in `onUnitBuilt` (`applyBuildEffects` is
  `addProductionExperience`, `CyCity.cpp:2261`), Hall of Mirrors, the Crusade `cannotTrain` /
  `cannotConstruct` lists, and Insane.

I found **no faithfulness bug.** Notes:
- **Freak Show** (`CIV4BuildingInfos.xml:6285`): `iCost` −1 and no grant anywhere, so it is
  unobtainable. That is FfH 0.41 data, not the port.
- **`bAbandon`** decrements the class counts twice (`unit/lifecycle.rs:553`). It is already marked
  `// C++ quirk:`.
- **docs/AI.md vs code:** "civics ... when the gain beats the anarchy" is not what `empire::civics`
  does (section 0, item 5).
- **AI legality, not rules:** AI-initiated AI-to-AI deals skip `player_can_contact`. That is
  illegal under `bNoDiplomacyWithEnemies`, the Crusade case (C7).

---

## G. Ranked proposals (expected value for winning)

| # | Civ | Proposal | Hook | Value |
|---|---|---|---|---|
| 1 | Clan | Value the Warrens (city production × 15 × 0.6) so it is built early in the best production cities | `ClanOfEmbers::building_value` | big |
| 2 | Bannor | Adopt Crusade only for a decisive war with ≥5 cities, no expansion left and anarchy paid; and charge anarchy in all civic switches | new `CivPlay::civic_value` + `empire::civics` | big |
| 3 | Calabim | Vampires (`UNITAI_FEASTING`) back into the army: set ATTACK, or accept FEASTING in `is_army_unit`, `army_group` and the attacker count | `Calabim::unit` (or generic) | medium-big |
| 4 | Clan | Rantine walks into weakly held barbarian cities to Convert City (the Clan's only way to absorb them) | `ClanOfEmbers::unit` | big early |
| 5 | Clan | For the Horde only at war or while preparing one (or to thin a Rantine target) | `ClanOfEmbers::spell_value` | medium |
| 6 | Balseraphs | Loki: no early build; park him in fresh low-culture rival cities and Disrupt them | `Balseraphs::production` / `unit` / `spell_value` | medium |
| 7 | Calabim | Feast in capped cities (XP < 30), Gift Vampirism to veterans, no Feed on the last garrison | `Calabim::spell_value` / `unit` | medium |
| 8 | Amurites | Casters build typed mana nodes (free sphere promotions, Fireball), caster target (cities + 2)/2, Cave of Ancestors valued in the caster city | `Amurites::unit` / `production` / `building_value` | medium |
| 9 | Clan | Barbarian-ally adjustments: no barbarian defenders, research pressure or mobile force, settlers unescorted vs barbarians | generic `barbarians_near`, research pressure | medium-small |
| 10 | Bannor | Rally held until the Crusade war has a campaign target; legality fix for peace offers under Crusade | `Bannor::spell_value`; `diplomacy.rs` | medium / legality |
| 11 | Calabim | River of Blood held for happy and health room, a war start or a domination push | `Calabim::spell_value` | small-medium |
| 12 | Balseraphs | Create Puppet no longer outbids the adept's own spells; re-check civics in golden ages (Revelry) | `Balseraphs::spell_value`; `empire::civics` | small-medium |
| 13 | Amurites | Arcane Lacuna held for a war against casters (after node building makes the gate reachable); Teach Spellcasting valued by buffs taught | `Amurites::spell_value` | small |

Measurement for each: `crates/ffh-cli/scripts/civ_ab.py --civ CIVILIZATION_X --off-base` on fresh
seeds (usage counts for WARRENS, VAMPIRE, DEMAGOG and the cages, plus `world_spell`), then the
round's usual 300-turn and full-game A/B. Lessons from docs/AI.md apply: development seeds lie, and
anything that takes production from the army has to show it on full games.
