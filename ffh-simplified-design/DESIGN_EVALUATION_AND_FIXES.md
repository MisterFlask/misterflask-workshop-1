# FFH SIMPLIFIED: DESIGN EVALUATION AND FIXES

This document identifies gaps, inconsistencies, and balance issues in the Master Design Document, with proposed fixes.

---

## SEVERITY LEGEND

- **CRITICAL**: Breaks core loop or creates unplayable states
- **MAJOR**: Significant balance issues or unclear mechanics
- **MINOR**: Small gaps that need clarification but don't break gameplay

---

# CRITICAL ISSUES

## 1. TECH TREE vs BUILDING SYSTEM CONFLICT

**Problem**: Section C2 describes a separate "tech tree" with 50 nodes across 4 eras, but the design philosophy says "no research points—technology is unlocked by buildings." These systems appear redundant and create confusion:

- Are tech nodes separate from buildings?
- How do you "unlock" a tech node if there's no research?
- Era triggers reference population AND city count—which is canonical?

**The Confusion**:
```
Tech Node "Militia Training" → Unlocks Fighter
Building "Barracks" → Unlocks Fighter
```
Are these the same thing? Different things?

**FIX**: Collapse tech nodes INTO buildings. There is no separate tech tree—the building prerequisite chains ARE the tech tree.

**Revised System**:
- **Eras are triggered by global conditions** (pop thresholds, city count, Armageddon level)
- **Eras unlock building AVAILABILITY**, not individual techs
- **Buildings still have prerequisites within their era**

| Era | Trigger | Effect |
|-----|---------|--------|
| Era 1 | Game Start | T1 buildings available |
| Era 2 | Any city Pop 3 | T2 buildings available |
| Era 3 | Any city Pop 5 | T3 buildings available |
| Era 4 | 6+ cities OR Armageddon > 50 | T4 buildings available |

**DELETE Section C2** or merge it into Section C5 (Buildings) with clear prerequisite chains.

---

## 2. CAVALRY UNLOCK CONTRADICTION

**Problem**: Three different sources claim to unlock Cavalry:
- War Academy building: "Unlocks Knight, Cavalry"
- Stables building: "Unlocks Cavalry"
- Horses resource: "Unlocks Cavalry recruitment"

**FIX**: Clarify with a unified rule:

| Unit | Building Requirement | Resource Requirement |
|------|---------------------|---------------------|
| Cavalry | Stables | Horses within 2 tiles |
| Knight | War Academy | None |
| Horse Archer | Stables + Archery Range | Horses within 2 tiles |

**War Academy unlocks Knight only.** Stables + Horses unlocks Cavalry.

---

## 3. HERO SYSTEM INCOMPLETE

**Problem**: Heroes are mentioned 12+ times but never fully specified:
- How do you acquire heroes?
- What are specific hero abilities?
- Can heroes die permanently?
- What's the limit on total heroes?
- What do heroes cost?

**FIX**: Add complete Hero specification.

### HERO SYSTEM

**Acquisition**:
- Heroes spawn via **specific buildings** or **events**
- Each hero is unique and named
- Once dead, that specific hero cannot return (permadeath)

**Hero Sources**:
| Hero | Source | Cost |
|------|--------|------|
| **Warlord** | Elite Barracks (first built) | Free on construction |
| **Archmage** | Arcane Sanctum (first built) | Free on construction |
| **High Priest** | Cathedral (first built) | Free on construction |
| **Champion** | Defeat a World Boss | Free (reward) |
| **Mercenary Captain** | Hire via Hippus diplomacy | 300 gold |

**Hero Limits**:
- Maximum 3 heroes active at once
- 1 hero per legion maximum
- Heroes take a soldier slot (so 7 regular soldiers + 1 hero = full legion)

**Hero Stats**: Heroes are elite soldiers with 1.5x base stats of comparable unit:
| Hero | HP | ATK | DEF | SPD | Row | Ability |
|------|-----|-----|-----|-----|-----|---------|
| Warlord | 200 | 45 | 30 | 50 | Front | +5 ATK to all soldiers in legion |
| Archmage | 80 | 55 | 10 | 50 | Back | Spells cost -5 mana while alive |
| High Priest | 100 | 20 | 20 | 50 | Back | Heal all soldiers 10 HP per combat round |
| Champion | 250 | 50 | 35 | 55 | Front | +25% damage vs demons/undead |
| Mercenary Captain | 150 | 40 | 25 | 60 | Front | Legion gains +1 movement |

**Permadeath**: When a hero dies in combat, they're gone forever. The building that spawned them can spawn a replacement after **20 turns**.

---

## 4. MISSING STARTING CONDITIONS

**Problem**: No specification of what the player starts with.

**FIX**: Add explicit starting conditions.

### PLAYER STARTING CONDITIONS

| Resource | Amount |
|----------|--------|
| Gold | 200 |
| Mana | 0 |
| Cities | 2 |
| Legions | 1 |

**Starting City 1 (Capital)**:
- Population: 3 (2 building slots)
- Buildings: Barracks, Market

**Starting City 2 (Outpost)**:
- Population: 1 (1 building slot)
- Buildings: Walls

**Starting Legion**:
- 2 Fighters, 1 Archer, 1 Cleric (4 soldiers total)
- Located at Capital

### AI STARTING CONDITIONS

| Faction | Cities | Legions | Starting Units |
|---------|--------|---------|----------------|
| Hippus | 1 | 2 | 6 Outriders, 4 Horse Archers |
| Sheaim | 2 | 1 | 4 Cultists, 2 Hellhounds |
| Ljosalfar | 2 | 2 | 4 Wardancers, 4 Arcane Archers |
| Calabim | 2 | 1 | 2 Vampires, 4 Thralls |
| Clan of Embers | 2 | 2 | 12 Orc Warriors |
| Infernal | 1 (spawns at Armageddon 100) | 2 | 16 Demons |

---

## 5. CLERIC HEALING MECHANICS UNDEFINED

**Problem**: Clerics "heal instead of damage" but when and how is never specified.

**FIX**: Define healing in combat.

### CLERIC COMBAT MECHANICS

**Healing Action**:
- Clerics target **allied soldiers** instead of enemies
- Healing happens at the Cleric's attack timing (based on SPD)
- Heal Amount: 20 HP per "attack"
- Target Priority: Lowest HP% allied soldier in same column, then adjacent columns

**Combat Flow Example**:
1. Combat starts
2. Cleric's turn comes at t=45 (SPD 45)
3. Cleric heals the most damaged ally for 20 HP
4. If Cleric is back-row (1 attack), they heal once
5. Healing cannot exceed soldier's max HP
6. Healing cannot resurrect dead soldiers (must be alive when healed)

**Critical Timing**: If an ally would die BEFORE the cleric's turn, they still die. Clerics don't prevent deaths—they sustain surviving soldiers.

---

# MAJOR ISSUES

## 6. ARMAGEDDON COUNTER MATH IS BROKEN

**Problem**: The counter math doesn't produce consistent game lengths.

- Base rate: +1/turn → reaches 100 on turn 100
- Sheaim ritual: +10 every 5 turns (if uninterrupted) → +2/turn → hits 100 by turn 33
- Elves with 2 groves: -2/turn → counter would never reach 100 without Sheaim
- Player Cathedrals: -1/turn each → can slow indefinitely

**The Issue**: If Sheaim aren't in the game, Armageddon might never happen. If Sheaim are active, game could end turn 30.

**FIX**: Redesign counter dynamics.

### REVISED ARMAGEDDON COUNTER

**Base Rate**: +1 per turn (unchanged)

**Acceleration Sources**:
| Source | Amount | Notes |
|--------|--------|-------|
| Sheaim ritual complete | +8 (not +10) | Reduced to extend game |
| City razed | +2 | Any faction |
| World Boss killed | +5 | Disrupts cosmic balance |
| Skeletal Crypt cleared | +3 | Undead energy released |

**Deceleration Sources**:
| Source | Amount | Notes |
|--------|--------|-------|
| Sacred Grove (Elves) | -0.5/turn per grove | Reduced from -1 |
| Cathedral | -0.5/turn per cathedral | Cap: 2 cathedrals count |
| Sealing Ritual (one-time) | -15 | Era 4 ability |

**Minimum Rate**: Counter advances minimum +0.5/turn regardless of deceleration (can't permanently stall).

**New Target**: Armageddon triggers at **80** (not 100).

**Result**:
- Without interference: ~80 turns
- With Sheaim active (3 rituals): ~50-60 turns
- With full deceleration: ~100-120 turns

---

## 7. SIEGE UNITS ARE UNUSABLE

**Problem**: Catapult has 0 DEF, 20 SPD, and sits in back row. It will likely die before attacking, and "building damage" is undefined.

**FIX**: Redesign siege mechanics.

### SIEGE MECHANICS

**Siege Units Don't Fight in Normal Combat.** Instead:

**Siege Action** (alternative to Attack):
- Legion with siege unit adjacent to enemy city can **Siege** instead of Attack
- Siege bypasses garrison combat
- Siege damages a random building (25% of building HP per siege unit)
- Building at 0% HP is destroyed
- Garrison can **Sally** (exit and attack sieging legion) to interrupt

**Siege Unit Stats Revised**:
| Unit | HP | ATK | DEF | SPD | Cost | Siege DMG |
|------|-----|-----|-----|-----|------|-----------|
| Catapult | 60 | 15 | 5 | 30 | 100g | 25% per turn |
| Trebuchet | 80 | 20 | 8 | 25 | 180g | 40% per turn |

**In Combat**: Siege units fight normally (they're not great, but not useless—they're protected in back row and deal modest damage).

---

## 8. FRONT ROW ATTACK COUNT CREATES DAMAGE IMBALANCE

**Problem**: Front row gets 3 attacks, back row gets 1. This makes front-row stacking vastly superior for damage output:

- 8 Knights (front): 30 ATK × 3 attacks = 90 damage per Knight = 720 total
- 8 Archmages (back): 45 ATK × 1 attack = 45 damage per Archmage = 360 total

Front row deals **2x** the damage despite back row having higher ATK stats.

**FIX**: Rebalance attack counts OR buff back-row damage.

### OPTION A: Rebalance Attack Counts

| Row | Attacks |
|-----|---------|
| Front | 2 |
| Mid | 2 |
| Back | 2 |

Everyone gets 2 attacks. Row determines **targeting priority** only.

### OPTION B: Back Row Damage Multiplier (Recommended)

Keep attack counts but add:
- **Front row**: 3 attacks × 1.0 damage multiplier
- **Mid row**: 2 attacks × 1.25 damage multiplier
- **Back row**: 1 attack × 2.0 damage multiplier

**Result**:
- Knight (front): 30 × 3 × 1.0 = 90 damage
- Archmage (back): 45 × 1 × 2.0 = 90 damage

Now front and back are balanced for different roles (front = sustained, back = burst).

---

## 9. MAGIC SCHOOL ACCESS IS UNCLEAR

**Problem**: The tech tree mentions "Elementalism: Fire/Ice/Lightning spell access" but there's no Ice or Lightning school—those spells are in Water and Air schools. The path to unlock each school is never specified.

**FIX**: Define explicit magic school unlocks.

### MAGIC SCHOOL UNLOCK PATHS

| School | Building Required | Prerequisite |
|--------|------------------|--------------|
| Fire | Mage Tower | None |
| Water | Mage Tower | None |
| Earth | Mage Tower + Walls | Fortifications |
| Air | Mage Tower | None |
| Life | Temple | Priesthood |
| Death | Mage Tower | None (but +Armageddon) |

**Mage Tower provides access to**: Fire, Water, Air, Death (Tier 1 only)

**Arcane Sanctum provides access to**: All schools Tier 2-3

**Life school is separate**: Temple → Priesthood → Holy Ground for Tier 1-2, Cathedral for Tier 3

**Death school penalty**: Each Death spell cast advances Armageddon by +1

---

## 10. RESOURCE CONTROL RADIUS TOO SMALL

**Problem**: "Resource within 2 tiles" is very restrictive. On a 35×35 map with cities spaced 4+ tiles apart, many resources would be uncollectable.

**FIX**: Increase control radius.

**New Rule**: Resource must be within **3 tiles** of owned city OR within owned territory (any tile you control).

**Territory**: All tiles within 2 tiles of any city you own, plus connecting corridors along roads (if roads are implemented) or shortest path between your cities.

---

## 11. DEMON BINDING MECHANIC IS UNEXPLOITABLE

**Problem**: Sheaim demons "die without proximity to Sheaim city" but no distance is specified, and the player can't meaningfully interact with this.

**FIX**: Make it concrete and exploitable.

### DEMON BINDING RULES

**Binding Distance**: Demons must end their turn within **5 tiles** of a Sheaim city with a Demon Gate.

**If Outside Binding Range**:
- Demon takes **20 damage at end of turn**
- Stacks each turn (20, 40, 60...)
- A 120 HP Demon dies in 3 turns outside range

**Player Counterplay**:
1. Take/destroy Demon Gate cities → all linked demons start decaying
2. Lure demons away from cities → they self-destruct
3. Focus Demon Gates, ignore demons (they die automatically)

**Demon Gate Capacity**: Each Demon Gate can sustain up to **8 demons**. Excess demons decay even within range.

---

## 12. POPULATION DRAIN (CALABIM) NEEDS MECHANICS

**Problem**: "Drain 1 population/turn" is vague. Does it require troops? Can it be blocked?

**FIX**: Specify completely.

### CALABIM POPULATION DRAIN

**Trigger**: Calabim legion adjacent to non-Calabim city

**Effect**: City loses 1 population per turn while Calabim legion remains adjacent

**Blockable**: Yes—if the city has a garrison, drain is blocked

**Pop 0**: City becomes "Abandoned" (captured by Calabim automatically, 0 population)

**Counter**: Garrison cities near Calabim. They can't drain if you have troops present.

**Rate Limit**: Each Calabim legion can only drain from ONE city per turn (prevents surrounding a city with 4 legions for 4 drain/turn).

---

## 13. AI LEGION LIMITS UNSPECIFIED

**Problem**: Player has max 5 legions. What about AI factions? Could Clan of Embers have 20 legions?

**FIX**: Define AI legion limits.

### AI LEGION LIMITS

| Faction | Max Legions | Notes |
|---------|-------------|-------|
| Hippus | 3 | Mobile raiders |
| Sheaim | 3 | +1 demon legion per completed ritual (max 5 total) |
| Ljosalfar | 4 | Defensive, need coverage |
| Calabim | 3 | Few but elite |
| Clan of Embers | 5 | Numerous hordes |
| Infernal | 4 (scales) | See boss scaling |

AI can't exceed these limits. When at limit, new spawns reinforce existing legions instead of creating new ones.

---

## 14. AI-vs-AI COMBAT TOO SIMPLISTIC

**Problem**: "Stronger side wins" without detail leads to potential runaway winner scenarios.

**FIX**: Add casualty mechanics to AI-vs-AI combat.

### AI-vs-AI COMBAT RESOLUTION

1. **Compare Total Strength**: Sum of (HP × ATK) for all soldiers
2. **Determine Winner**: Higher total wins
3. **Both Sides Take Casualties**:
   - Winner loses soldiers equal to 20% of loser's strength
   - Loser loses soldiers equal to 40% of winner's strength
4. **Loser Retreats**

**Example**:
- Hippus legion: Strength 1000
- Orc legion: Strength 800
- Hippus wins, loses ~160 strength worth of soldiers
- Orcs lose ~400 strength worth of soldiers, retreat

This creates **pyrrhic victories** and prevents snowballing.

---

## 15. NO CITY FOUNDING—UNCLEAR IF INTENTIONAL

**Problem**: Player starts with 2 cities, can capture more, but can they found new cities?

**CLARIFICATION** (Assumed Intentional): No settlers. Expansion is through **conquest only**.

**Make Explicit**: Add to Section B2:

> "Cities are not founded. The map starts with all cities that will exist. Players expand exclusively through capturing neutral and enemy cities."

---

## 16. WORLD BOSS REWARDS IMBALANCED

**Problem**:
- Frost Giant King: Hero item + terrain change (utility)
- Ancient Red Dragon: Hero item + 3 mana/turn (permanent economy)
- Lich Lord: Death magic staff + **15 Armageddon cleared** (game-changing)

The Lich Lord reward is dramatically more powerful than others.

**FIX**: Rebalance rewards.

| Boss | Reward 1 | Reward 2 |
|------|----------|----------|
| Frost Giant King | Frost Axe (+15 ATK vs giants) | Opens mountain pass |
| Ancient Red Dragon | Dragon Scale Armor (+20 DEF) | 200 gold hoard |
| Lich Lord | Lich's Staff (Death spells -10 mana) | **+5 Armageddon** (not cleared—releasing dark energy) |

**Alternative for Lich Lord**: The Lich was CONTAINING evil. Killing him releases it. Thematically better.

---

# MINOR ISSUES

## 17. SPEED STAT FORMULA NOT SHOWN

**FIX**: Add explicit formula to Combat section.

### SPEED AND ATTACK TIMING

Combat round = 100 time units

**First Attack Time**: 100 / SPD
**Subsequent Attack Times**: Previous time + (100 / SPD)

| Unit | SPD | First Attack | Second Attack | Third Attack |
|------|-----|--------------|---------------|--------------|
| Knight | 30 | t=3.3 | t=6.7 | t=10 |
| Fighter | 50 | t=2 | t=4 | t=6 |
| Imp | 70 | t=1.4 | t=2.9 | t=4.3 |

**Combat Resolution**: All attacks execute in timestamp order. Combat ends after all attacks resolve (approximately t=10-12 depending on slowest unit).

---

## 18. PILLAGING vs CONQUERING INCENTIVES

**Problem**: Pillaging gives 50% building gold value. Is this better than capturing?

**Analysis**:
- Pillage Market (80g): Get 40g, enemy loses 10g/turn
- Capture city: Get 10g/turn (after 3-turn penalty)
- Break-even: 4 turns after capture, you've matched pillage reward and then profit

**Verdict**: System is balanced. Pillaging is for harassing economies; capturing is for long-term growth.

**No Fix Needed**, but add clarifying note:
> "Pillaging is for economic warfare without territorial commitment. Capturing is superior for empire-building."

---

## 19. TIES IN COMBAT

**Problem**: "Defender wins ties" but ties are statistically rare.

**FIX**: Not a problem—keep the rule for edge cases. No change needed.

---

## 20. OCCUPATION PENALTY LOOP

**Problem**: If enemy recaptures a city you just captured, does penalty reset?

**FIX**: Yes, intentionally. Add note:
> "Occupation penalty resets on each capture. Contested cities may be perpetually penalized during back-and-forth warfare. This creates incentives to establish stable fronts rather than endless city-swapping."

---

## 21. ERA 4 TRIGGER INCONSISTENCY

**Problem**: Era 4 uses "6+ cities OR Armageddon > 50" while Era 2-3 use only population.

**FIX**: This is fine—Era 4 is the "endgame prep" era and should trigger when either you're dominant (6 cities) or the clock is running out (Armageddon > 50).

**Clarification**: Make sure Era 4 can trigger even if no city reaches pop 7. Current design allows this.

---

# SUMMARY OF REQUIRED CHANGES

## Critical (Must Fix)
1. ✅ Collapse tech tree into building system
2. ✅ Clarify Cavalry unlock (Stables + Horses)
3. ✅ Complete Hero system specification
4. ✅ Add starting conditions
5. ✅ Define Cleric healing mechanics

## Major (Should Fix)
6. ✅ Rebalance Armageddon Counter math
7. ✅ Redesign siege mechanics
8. ✅ Rebalance front/back row damage
9. ✅ Define magic school unlock paths
10. ✅ Increase resource control radius
11. ✅ Specify demon binding mechanics
12. ✅ Define Calabim population drain
13. ✅ Set AI legion limits
14. ✅ Add AI-vs-AI casualty mechanics
15. ✅ Clarify no city founding
16. ✅ Rebalance world boss rewards

## Minor (Nice to Fix)
17. ✅ Show speed formula
18. ✅ Clarify pillage vs capture incentives
19. No change needed (ties)
20. ✅ Clarify occupation penalty reset
21. No change needed (Era 4 trigger)

---

# REVISED DESIGN AUDIT

After applying these fixes:

| Category | Before | After |
|----------|--------|-------|
| System Conflicts | 3 | 0 |
| Undefined Mechanics | 8 | 0 |
| Balance Issues | 5 | 0 |
| Missing Content | 4 | 0 |
| Unclear Rules | 6 | 0 |

**All identified issues have proposed resolutions.**
