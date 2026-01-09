# FFH Simplified - Unit Design Document

## Overview

This document outlines the complete unit roster for FFH Simplified, including building requirements, stats, and strategic roles. Units are organized by their unlock method and tier.

**Note**: This document supplements the Master Design Document. For the authoritative system design, see `FFH_SIMPLIFIED_MASTER_DESIGN.md`.

## Design Principles

1. **Role Clarity**: Each unit should have a clear tactical purpose
2. **Building-Gated Progression**: Units unlock via buildings (multi-turn construction), not research
3. **Terrain-Gated Elites**: The most powerful units require controlling specific rare terrain improvements
4. **Mana as Capacity**: Magical power is a limit, not a currency (see below)
5. **Formation Matters**: Position in formation should meaningfully affect performance
6. **Counter-Play**: Units should have strengths and weaknesses against other unit types

---

## The Mana System

### Core Concept: Mana as Capacity, Not Currency

Mana represents your civilization's **capacity to sustain magical forces**, not a spendable resource. Think of it like supply in StarCraft or command points in other strategy games.

### How It Works

- **Mana Cap**: Your empire has a maximum mana capacity (starts at 0, increased by buildings/terrain)
- **Mana Upkeep**: Magic units and certain buildings consume mana while they exist
- **Mana Balance**: `Current Mana Usage` must be ≤ `Mana Cap`
- **Blocking**: You cannot recruit/build anything that would exceed your cap
- **No Refunds**: Mana isn't "spent" - it's allocated. Losing a mage frees up that mana.

### Mana Sources (Increase Cap)

**From Buildings:**
| Source | Mana Provided |
|--------|---------------|
| Mage Tower | +5 |
| Temple | +3 |

**From Terrain Improvements (requires exploitation building):**
| Terrain Improvement | Building Required | Mana Provided |
|---------------------|-------------------|---------------|
| Mana Node | None | +2 |
| Mana Spring | Mana Well | +3 |
| Crystal Cave | Crystal Sanctum | +5 |
| Haunted Barrow | Necropolis | +4 |
| Dragon Bones | Dragon Shrine | +3 |
| World Tree | Grove of Ages | +5 |
| Hellgate | Binding Circle | +8 |

See Master Design Section C7 for complete terrain improvement details.

### Mana Sinks (Consume Cap)
| Consumer | Mana Upkeep |
|----------|-------------|
| Mage | 2 |
| Cleric | 1 |
| Battlemage | 3 |
| Archmage | 5 |
| Lich | 6 |
| Dragon Knight | 4 |
| Summoned Elemental | 8 |
| High Priest | 3 |
| Paladin | 2 |
| Acolyte | 1 |
| War Priest | 2 |
| Assassin | 1 |
| Ritual Site (building) | 3 |

### Strategic Implications

1. **Mage Tower Rush**: Building Mage Towers becomes essential to field magical units
2. **Territory Value**: Mana-producing terrain features become high-priority conquests
3. **Army Composition**: You must balance powerful mages against mana constraints
4. **Attrition Matters**: Losing mages in battle frees mana for replacements
5. **Building Choices**: Some buildings consume mana (Ritual Site), creating tension

---

## Current Units (Already Implemented)

### Fighter
- **Role**: Basic frontline infantry
- **Unlock**: None (always available)
- **Stats**: 100 HP, 20 ATK, 15 DEF, Speed 50
- **Cost**: 50 Gold
- **Position**: Front row
- **Notes**: The backbone of any army. Reliable but not exceptional.

### Archer
- **Role**: Ranged damage dealer
- **Unlock**: None (always available)
- **Stats**: 60 HP, 25 ATK, 5 DEF, Speed 60
- **Cost**: 60 Gold
- **Position**: Back row, targets enemy back row
- **Notes**: Glass cannon that can snipe enemy support units.

### Knight
- **Role**: Heavy cavalry, elite frontline
- **Unlock**: Stables building (requires Ironworking tech)
- **Stats**: 150 HP, 30 ATK, 20 DEF, Speed 30
- **Cost**: 100 Gold
- **Position**: Front row
- **Notes**: Slow but devastating. Best against other melee units.

### Mage
- **Role**: Magical damage dealer
- **Unlock**: Mage Tower building (requires Ritual Basics tech)
- **Stats**: 50 HP, 35 ATK, 5 DEF, Speed 40
- **Cost**: 80 Gold | **Mana Upkeep**: 2
- **Position**: Back row, targets enemy back row
- **Notes**: High damage but extremely fragile. Your first mana-consuming unit.

### Cleric
- **Role**: Healer/Support
- **Unlock**: Temple building (requires Consecrated Ground tech)
- **Stats**: 70 HP, 10 ATK (heals instead), 10 DEF, Speed 45
- **Cost**: 70 Gold | **Mana Upkeep**: 1
- **Position**: Back row
- **Notes**: Heals lowest HP ally instead of attacking. Low mana upkeep makes them efficient.

### Demon
- **Role**: Elite monster unit
- **Unlock**: Boss faction only
- **Stats**: 120 HP, 35 ATK, 15 DEF, Speed 55
- **Position**: Front row
- **Notes**: Not recruitable by player. Boss-exclusive terror unit.

---

## Proposed New Units

### TIER 1: No Tech Required

#### Militia
- **Role**: Cheap defensive unit
- **Unlock**: None
- **Stats**: 60 HP, 12 ATK, 20 DEF, Speed 35
- **Cost**: 25 Gold
- **Position**: Front row
- **Notes**: Half the cost of a Fighter but much weaker offensively. Good for emergency defense or padding formations.

#### Scout
- **Role**: Fast skirmisher
- **Unlock**: None
- **Stats**: 45 HP, 18 ATK, 8 DEF, Speed 80
- **Cost**: 40 Gold
- **Position**: Mid row, targets back row
- **Attack Count**: { front: 2, mid: 3, back: 2 }
- **Notes**: Fastest unit in the game. Gets extra attacks from mid row. Useful for hit-and-run tactics.

---

### TIER 2: Common Tech Requirements

#### Pikeman
- **Role**: Anti-cavalry specialist
- **Unlock Tech**: "Pike Squares" (Martial - Common, 3 turns)
- **Stats**: 90 HP, 15 ATK, 25 DEF, Speed 40
- **Cost**: 55 Gold
- **Position**: Front row
- **Special**: Deals double damage to mounted units (Knight, future cavalry)
- **Notes**: Defensive frontliner that hard-counters cavalry charges.

#### Crossbowman
- **Role**: Armor-piercing ranged
- **Unlock Tech**: "Mechanical Crossbows" (Industrial - Common, 4 turns)
- **Stats**: 55 HP, 30 ATK, 8 DEF, Speed 45
- **Cost**: 75 Gold
- **Position**: Back row, targets front row
- **Special**: Ignores 50% of target's defense
- **Notes**: Unlike archers who target back row, crossbowmen punch through frontline armor.

#### Acolyte
- **Role**: Minor healer / buffer
- **Unlock Tech**: "Lay Healing" (Social - Common, 3 turns)
- **Stats**: 55 HP, 8 ATK (heals), 8 DEF, Speed 50
- **Cost**: 45 Gold | **Mana Upkeep**: 1
- **Position**: Mid row
- **Special**: Heals for 15 (half of Cleric) but can be positioned more flexibly
- **Notes**: Budget healing option before Temple is available. Minimal mana investment.

---

### TIER 3: Guild Secret Tech Requirements

#### Berserker
- **Role**: High-risk damage dealer
- **Unlock Tech**: "Blood Rage" (Martial - Guild Secret, 5 turns)
- **Stats**: 80 HP, 40 ATK, 5 DEF, Speed 65
- **Cost**: 70 Gold
- **Position**: Front row
- **Special**: Attack increases by 20% for each 25% HP lost
- **Notes**: Glass cannon melee. Devastating when wounded but dies easily.

#### Assassin
- **Role**: Priority target eliminator
- **Unlock Tech**: "Shadow Guild" (Social - Guild Secret, 6 turns)
- **Stats**: 50 HP, 45 ATK, 5 DEF, Speed 90
- **Cost**: 90 Gold | **Mana Upkeep**: 1
- **Position**: Mid row, targets back row
- **Attack Count**: { front: 1, mid: 2, back: 3 }
- **Special**: Always targets the lowest HP enemy in preferred row
- **Notes**: The ultimate support-killer. Shadow magic requires minimal mana.

#### War Priest
- **Role**: Combat healer hybrid
- **Unlock Tech**: "Militant Faith" (Social - Guild Secret, 5 turns)
- **Stats**: 85 HP, 22 ATK, 15 DEF, Speed 45
- **Cost**: 85 Gold | **Mana Upkeep**: 2
- **Position**: Mid row
- **Special**: Attacks enemies AND heals lowest ally for 10 each round
- **Notes**: Does both jobs adequately but neither excellently.

#### Siege Engineer
- **Role**: Structure specialist
- **Unlock Tech**: "Siege Warfare" (Industrial - Guild Secret, 6 turns)
- **Stats**: 70 HP, 25 ATK, 12 DEF, Speed 30
- **Cost**: 80 Gold
- **Position**: Back row, targets front row
- **Special**: Deals +50% damage when attacking cities with walls
- **Notes**: Essential for breaking fortified positions.

#### Battlemage
- **Role**: Durable spellcaster
- **Unlock Tech**: "Arcane Armor" (Arcane - Guild Secret, 5 turns)
- **Stats**: 75 HP, 30 ATK, 15 DEF, Speed 35
- **Cost**: 90 Gold | **Mana Upkeep**: 3
- **Position**: Mid row, targets back row
- **Notes**: A mage that can survive being targeted. Higher mana upkeep for durability.

#### Horse Archer
- **Role**: Mobile ranged harasser
- **Unlock Tech**: "Mounted Archery" (Martial - Guild Secret, 5 turns)
- **Stats**: 65 HP, 22 ATK, 10 DEF, Speed 70
- **Cost**: 85 Gold
- **Position**: Mid row, targets back row
- **Attack Count**: { front: 2, mid: 3, back: 2 }
- **Notes**: Fast archer that excels in the mid row. Combines mobility with range.

---

### TIER 4: Master's Teaching Tech Requirements

#### Champion
- **Role**: Elite all-rounder
- **Unlock Tech**: "Heroes of Legend" (Martial - Master's Teaching, 8 turns)
- **Stats**: 140 HP, 35 ATK, 25 DEF, Speed 50
- **Cost**: 150 Gold
- **Position**: Front row
- **Special**: Inspires adjacent allies, giving them +5 ATK
- **Notes**: Expensive but excellent. A single Champion anchors a formation.

#### High Priest
- **Role**: Mass healer
- **Unlock Tech**: "Divine Channeling" (Social - Master's Teaching, 7 turns)
- **Stats**: 80 HP, 5 ATK (heals), 15 DEF, Speed 40
- **Cost**: 100 Gold | **Mana Upkeep**: 3
- **Position**: Back row
- **Special**: Heals ALL allies for 15 HP per round instead of single target
- **Notes**: Game-changing sustain. Countered by burst damage or assassins.

#### Archmage
- **Role**: Area damage dealer
- **Unlock**: **TERRAIN-GATED** - Requires Crystal Cave + Crystal Sanctum building
- **Stats**: 60 HP, 45 ATK, 8 DEF, Speed 45
- **Cost**: 120 Gold | **Mana Upkeep**: 5
- **Position**: Back row
- **Special**: Attacks ALL enemies in target row instead of single target. +50% spell damage while alive.
- **Notes**: Heavy mana investment but devastating against grouped enemies. **Cannot be recruited without controlling Crystal Cave.**

#### Golem
- **Role**: Ultimate tank
- **Unlock Tech**: "Construct Animation" (Industrial - Master's Teaching, 8 turns)
- **Stats**: 200 HP, 20 ATK, 30 DEF, Speed 20
- **Cost**: 100 Gold | **Mana Upkeep**: 0
- **Position**: Front row
- **Special**: Immune to healing but takes 25% less damage from all sources
- **Notes**: Animated by binding magic during creation, requires no ongoing mana. Can't be healed.

---

### TIER 5: Lost Art Tech Requirements

#### Paladin
- **Role**: Holy warrior
- **Unlock Tech**: "Divine Champion" (Social - Lost Art, 10 turns)
- **Stats**: 130 HP, 35 ATK, 25 DEF, Speed 45
- **Cost**: 140 Gold | **Mana Upkeep**: 2
- **Position**: Front row
- **Special**: Self-heals for 10 HP per round. Deals +50% damage to undead/demons.
- **Notes**: Self-sustaining elite. Low mana for its power - divine blessing is efficient.

#### Lich
- **Role**: Dark spellcaster
- **Unlock**: **TERRAIN-GATED** - Requires Haunted Barrow + Necropolis building
- **Stats**: 100 HP, 40 ATK, 12 DEF, Speed 35
- **Cost**: 150 Gold | **Mana Upkeep**: 6
- **Position**: Back row, targets back row
- **Special**: When Lich kills an enemy, raises a Skeleton (40 HP, 15 ATK, 5 DEF) in your front row. Skeletons cost 0 mana upkeep.
- **Notes**: Massive mana drain but snowballs if it gets kills. The dark path. **Cannot be recruited without controlling Haunted Barrow.**

#### Dragon Knight
- **Role**: Flying elite
- **Unlock**: **TERRAIN-GATED** - Requires Dragon Bones + Dragon Shrine building
- **Stats**: 140 HP, 40 ATK, 25 DEF, Speed 45
- **Cost**: 180 Gold | **Mana Upkeep**: 4
- **Position**: Front row
- **Special**: Can target any row regardless of position. Ignores terrain defense bonuses. Fire immunity for legion.
- **Notes**: Powerful elite with flexibility. **Cannot be recruited without controlling Dragon Bones.**

#### Summoned Elemental (Fire/Ice/Storm)
- **Role**: Expendable powerhouse
- **Unlock**: **TERRAIN-GATED** - Requires Mana Spring + Mana Well building
- **Stats**: 70-80 HP, 25-30 ATK, 10-15 DEF, Speed 45-50 (varies by type)
- **Cost**: 80 Gold + 15 Mana | **Mana Upkeep**: 3
- **Position**: Front row
- **Special**: Each type has elemental bonus (Fire: +damage, Ice: slows enemies, Storm: chain attacks)
- **Notes**: Mid-tier exclusive units. **Cannot be recruited without controlling Mana Spring.**

---

## ~~Tech Tree Additions Required~~ [DEPRECATED]

> **NOTE**: The tech tree system described below has been **superseded** by the building-gated progression system in the Master Design Document. Units unlock via buildings (instant construction with gold), not research.
>
> For the authoritative unlock system, see:
> - **Core units**: Building prerequisites in Master Design Section C5
> - **Terrain-gated units**: Terrain improvements in Master Design Section C7
>
> The tech tiers below are preserved for reference but should not be implemented.

<details>
<summary>Legacy Tech Tree Reference (Click to expand)</summary>

### Martial
| Tech | Tier | Turns | Unlocks |
|------|------|-------|---------|
| Pike Squares | Common | 3 | Pikeman |
| Blood Rage | Guild Secret | 5 | Berserker |
| Mounted Archery | Guild Secret | 5 | Horse Archer |
| Heroes of Legend | Master's Teaching | 8 | Champion |

### Industrial
| Tech | Tier | Turns | Unlocks |
|------|------|-------|---------|
| Mechanical Crossbows | Common | 4 | Crossbowman |
| Siege Warfare | Guild Secret | 6 | Siege Engineer |
| Construct Animation | Master's Teaching | 8 | Golem |

### Arcane
| Tech | Tier | Turns | Unlocks |
|------|------|-------|---------|
| Arcane Armor | Guild Secret | 5 | Battlemage |
| Mastery of Elements | Master's Teaching | 8 | Archmage |
| Elemental Binding | Lost Art | 10 | Summoned Elemental |
| Dragon Pact | Lost Art | 12 | Dragon Knight |
| *(existing)* Forbidden Summoning | Lost Art | 12 | Lich |

### Social
| Tech | Tier | Turns | Unlocks |
|------|------|-------|---------|
| Lay Healing | Common | 3 | Acolyte |
| Shadow Guild | Guild Secret | 6 | Assassin |
| Militant Faith | Guild Secret | 5 | War Priest |
| Divine Channeling | Master's Teaching | 7 | High Priest |
| Divine Champion | Lost Art | 10 | Paladin |

</details>

---

## Building & Terrain Requirements

Units unlock through two systems:

### Core Units (Building-Gated)
Available to all players who build the prerequisite building.

| Unit | Building Required | Era |
|------|-------------------|-----|
| Fighter | Barracks | 1 |
| Archer | Archery Range | 1 |
| Cleric | Temple | 1 |
| Knight | War Academy | 2 |
| Mage | Mage Tower | 2 |
| Cavalry | Stables (+ Horses resource) | 2 |
| Catapult | Siege Workshop | 2 |
| Paladin | Cathedral | 3 |
| Champion | Elite Barracks | 3 |

### Terrain-Gated Units (Exclusive)
Only available if you control the specific terrain improvement AND build the exploitation building.

| Unit | Terrain Required | Building Required |
|------|------------------|-------------------|
| Fire/Ice/Storm Elemental | Mana Spring | Mana Well |
| Archmage | Crystal Cave | Crystal Sanctum |
| Lich | Haunted Barrow | Necropolis |
| Dragon Knight | Dragon Bones | Dragon Shrine |
| Siege Titan | Adamantine Vein | Master Forge |
| Treant | World Tree | Grove of Ages |
| Summoned Demon | Hellgate | Binding Circle |
| Titan | Titan's Grave | Titan Forge |
| Golem | Titan's Grave | Titan Forge |

**Critical Rule**: If you lose control of the terrain improvement, existing units survive but **cannot be replaced**. This creates "stranded elite" scenarios where your last Archmage becomes irreplaceable.

---

## Formation Strategy Guide

### Aggressive Composition
- **Front**: Berserker, Champion, Berserker
- **Mid**: Assassin, War Priest, Horse Archer
- **Back**: Archmage, Mage, Archer

### Defensive Composition
- **Front**: Golem, Pikeman, Golem
- **Mid**: Battlemage, War Priest, Crossbowman
- **Back**: High Priest, Cleric, Archer

### Anti-Boss Composition
- **Front**: Paladin, Champion, Paladin
- **Mid**: War Priest, Cleric, Battlemage
- **Back**: High Priest, Archmage, Archer

---

## Balance Considerations

1. **Mana as Strategic Limit**: Your mana cap defines how magical your army can be
2. **Tech Depth vs Breadth**: Players must choose between rushing powerful single units or diversifying
3. **Counter System**: Pikemen counter Knights, Assassins counter Mages, Paladins counter Demons
4. **Healing Economy**: Multiple healing options at different mana upkeeps prevent spam
5. **Late Game Power Spike**: Lost Art units are extremely powerful but require 10-12 turn investment
6. **Territory Incentive**: Mana-producing terrain features drive expansion and conflict

---

## Example Mana Budgets

### Early Game (1 Mage Tower = 5 Mana Cap)
- 2 Mages (4 mana) + 1 Acolyte (1 mana) = **5 mana used**
- OR 1 Mage (2 mana) + 1 Cleric (1 mana) + 2 Acolytes (2 mana) = **5 mana used**

### Mid Game (2 Mage Towers + 1 Temple + Mana Spring = 15 Mana Cap)
- 2 Battlemages (6) + 2 Clerics (2) + 1 War Priest (2) + 1 Assassin (1) = **11 mana**, room to grow
- OR 1 Archmage (5) + 1 High Priest (3) + 3 Mages (6) = **14 mana**, nearly capped

### Late Game (3 Mage Towers + 2 Temples + Multiple Features = 25+ Mana Cap)
- 1 Dragon Knight (4) + 1 Lich (6) + 1 Archmage (5) + 1 High Priest (3) + 2 Battlemages (6) = **24 mana**
- OR go wide: 6 Mages (12) + 3 Clerics (3) + 3 War Priests (6) + 2 Assassins (2) = **23 mana**

### The Elemental Gambit
- Summoned Elemental costs 8 mana upkeep but dissolves after battle
- Useful for a decisive strike when you need temporary overwhelming force
- After the battle, that 8 mana is free again for rebuilding

---

## Implementation Priority

### Phase 1: Basic Roster Expansion
- Militia, Scout, Pikeman, Crossbowman, Acolyte

### Phase 2: Mid-Game Diversity
- Berserker, Assassin, War Priest, Battlemage, Horse Archer

### Phase 3: Elite Units
- Champion, High Priest, Archmage, Golem, Siege Engineer

### Phase 4: Endgame Power
- Paladin, Lich, Dragon Knight, Summoned Elemental
