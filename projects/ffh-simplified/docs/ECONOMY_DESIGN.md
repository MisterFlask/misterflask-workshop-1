# FFH Simplified - Economy & Progression Design Document

## Overview

This document outlines the economic systems, technology progression, and city specialization mechanics. The core philosophy: **the best content is gated behind territorial control**, forcing players to balance long-term planning with opportunistic adaptation.

---

## Core Design Pillars

### 1. Territory as Destiny
The most powerful technologies, buildings, and units are **locked behind controlling specific terrain improvements**. You cannot simply research your way to victory—you must expand, conquer, and hold the right locations.

### 2. Planned Opportunism
Players should:
- **Plan**: Chart a course through "mundane" techs they know they can achieve
- **Adapt**: Pivot strategy when valuable improvements become available (or are lost)

### 3. City Specialization
Cities are not interchangeable. Each city should develop a **distinct economic role** based on:
- What improvements exist in its territory
- Strategic needs of the empire
- Building slot limitations

### 4. Scarcity Creates Conflict
Rare improvements are worth fighting over. If only one Crystal Cave exists on the map, whoever controls it has a monopoly on certain magical capabilities.

---

## Terrain Improvements Reclassified

### Improvement Tiers

#### Common Improvements
Found frequently across the map. Provide modest bonuses. Multiple empires can have access.

| Improvement | Valid Terrain | Base Effect | Requires Building |
|-------------|---------------|-------------|-------------------|
| Fertile Plains | Grass | +1 Growth | Farmstead |
| Iron Vein | Hills, Mountain | +2 Gold | Mine |
| Timber Stand | Forest | +1 Gold, +1 Production | Lumber Mill |
| Quarry Site | Hills | +2 Production | Quarry |
| Pastureland | Grass | Enables cavalry units | Stables |

#### Uncommon Improvements
2-4 per map. Provide significant bonuses or unlock mid-tier content.

| Improvement | Valid Terrain | Base Effect | Requires Building | Unlocks Tech Branch |
|-------------|---------------|-------------|-------------------|---------------------|
| Mana Spring | Grass, Forest | +3 Mana Cap | Mana Well | Elemental Magic |
| Ancient Ruins | Grass, Hills | +3 Gold, +1 Research | Archive | Ancient Lore |
| Sacred Grove | Forest | +2 Mana Cap, +1 Growth | Shrine | Nature Magic |
| Watchtower Ruins | Grass, Hills | +10% City Defense | Garrison | Fortification |
| Mineral Deposit | Hills, Mountain | +3 Gold | Deep Mine | Advanced Metallurgy |

#### Rare Improvements
1-2 per map. Gate access to powerful late-game content.

| Improvement | Valid Terrain | Base Effect | Requires Building | Unlocks Tech Branch |
|-------------|---------------|-------------|-------------------|---------------------|
| Crystal Cave | Hills, Mountain | +5 Mana Cap | Crystal Sanctum | High Sorcery |
| Haunted Barrow | Grass, Hills | +4 Mana Cap | Necropolis | Death Magic |
| Dragon Bones | Any land | +3 Mana Cap, +2 Research | Dragon Shrine | Draconic Pact |
| Adamantine Vein | Mountain | +5 Production | Master Forge | Legendary Smithing |
| Ley Line Nexus | Any land | +6 Mana Cap | Arcane Nexus | Archmage Traditions |

#### Unique Improvements (Legendary)
0-1 per map. Each is one-of-a-kind. Whoever controls it has exclusive access to specific legendary content.

| Improvement | Effect | Requires Building | Exclusive Unlock |
|-------------|--------|-------------------|------------------|
| The World Tree | +5 Mana, +3 Growth, heals units | Grove of Ages | Treant units, Nature's Wrath spell |
| The Hellgate | +8 Mana, +Armageddon | Binding Circle | Demon summoning (for player) |
| The Oracle's Pool | See enemy movements, +4 Research | Oracle Temple | Prophecy techs, no fog of war |
| Titan's Grave | +10 Production | Titan Forge | Golem upgrades, Siege Titans |
| The Sunken Library | +6 Research | Restoration Project | Any one Lost Art tech free |

---

## Technology Tree Structure

### Tree Organization

Instead of a rotating random selection (Collegia), the tech tree is **fixed and visible** like Civilization. However, large branches are **grayed out and locked** until you control the prerequisite improvement.

```
MARTIAL BRANCH (Always Available)
├── Basic Training
│   ├── Shield Wall → Pikeman
│   ├── Archery Drills → Crossbowman
│   └── Cavalry Basics → Knight
├── Advanced Warfare (Requires: Iron Vein + Mine)
│   ├── Steel Weapons → +5 ATK all units
│   ├── Plate Armor → +5 DEF frontline
│   └── Siege Engineering → Siege weapons
└── Legendary Smithing (Requires: Adamantine Vein + Master Forge)
    ├── Adamantine Weapons → +10 ATK elite units
    ├── Impenetrable Armor → +15 DEF Champions
    └── Siege Titans → Unstoppable siege unit

ARCANE BRANCH (Requires: Any Mana Source)
├── Cantrip Theory (Requires: Mana Spring OR Sacred Grove)
│   ├── Basic Wards → City magic defense
│   └── Mage Training → Mage unit
├── Elemental Magic (Requires: Mana Spring + Mana Well)
│   ├── Fire Mastery → Battlemage fire attacks
│   ├── Ice Mastery → Slow enemy units
│   └── Storm Mastery → AoE damage
├── High Sorcery (Requires: Crystal Cave + Crystal Sanctum)
│   ├── Arcane Amplification → +50% spell damage
│   ├── Archmage Ascension → Archmage unit
│   └── Elemental Binding → Summoned Elemental unit
└── Death Magic (Requires: Haunted Barrow + Necropolis)
    ├── Soul Harvest → Mana from kills
    ├── Undead Legion → Skeleton summons
    └── Lich Transformation → Lich unit

DIVINE BRANCH (Requires: Sacred Grove OR Temple)
├── Lay Healing (Always Available)
│   └── Acolyte unit
├── Temple Traditions (Requires: Temple building)
│   ├── Divine Favor → Cleric unit
│   ├── Consecration → Holy ground defense
│   └── Healing Traditions → +healing power
├── Nature Magic (Requires: Sacred Grove + Shrine)
│   ├── Druidic Lore → Druid unit
│   ├── Beast Calling → Animal allies
│   └── Overgrowth → Terrain manipulation
└── Divine Champion (Requires: Sacred Grove + Dragon Bones)
    ├── Paladin Training → Paladin unit
    └── Holy Crusade → Bonus vs demons

DRACONIC BRANCH (Requires: Dragon Bones + Dragon Shrine)
├── Dragon Lore
│   ├── Draconic Language → Diplomacy bonus
│   └── Scale Armor → Dragon-scale equipment
├── Dragon Pact
│   ├── Dragon Knight → Dragon Knight unit
│   └── Wyrm's Blessing → Fire immunity
└── Dragon Mastery (Requires: Dragon Bones + Crystal Cave)
    └── Ancient Wyrm → Summon dragon ally (1 per game)
```

### Tech Gating Rules

1. **Visible but Locked**: Players can see the entire tree but locked branches show their requirements
2. **Improvement + Building**: Both the terrain improvement AND the exploitation building are required
3. **Permanent Unlock**: Once unlocked, losing the improvement doesn't re-lock the tech (you keep what you learned)
4. **Research Still Costs Time**: Unlocking the branch just makes it available—still need to research each tech

---

## City Specialization

### Building Slot System

Each city has **limited building slots** based on population:
- Population 1-2: 2 slots
- Population 3-4: 4 slots
- Population 5-6: 6 slots
- Population 7+: 8 slots (maximum)

This forces meaningful choices about city roles.

### City Archetypes

#### The Exploitation City
**Focus**: Maximize a specific terrain improvement

Required: Valuable improvement in territory

Key Buildings:
- Exploitation building (Mine, Mana Well, etc.)
- Enhancement buildings that boost the improvement
- Minimal military

Example: City near Crystal Cave
- Crystal Sanctum (exploits Crystal Cave, +5 Mana)
- Arcane Library (+1 Mana, +1 Research)
- Mage Tower (+5 Mana, trains mages)
- = 11 Mana cap from one city!

#### The Military Hub
**Focus**: Produce armies efficiently

Key Buildings:
- Barracks (basic troops)
- Stables (cavalry, requires Pastureland)
- Armory (+unit HP)
- Training Grounds (-training time)
- Walls (defense)

Production Bonuses Stack:
- Base: 1 unit/2 turns
- Barracks: 1 unit/turn
- Training Grounds: 2 units/turn
- Armory + Stables: +quality

#### The Mana Battery
**Focus**: Maximize mana cap for magical armies

Key Buildings:
- Mage Tower (+5 Mana)
- Temple (+3 Mana)
- Mana Well (if improvement available, +3 Mana)
- Arcane Library (+1 Mana, +1 Research)
- Crystal Sanctum (if Crystal Cave available, +5 Mana)

A fully developed Mana Battery city can provide 12-17 mana cap.

#### The Economic Engine
**Focus**: Generate gold for empire-wide spending

Key Buildings:
- Market (+3 Gold)
- Bank (+5 Gold, requires Market)
- Trade Post (+2 Gold per friendly city connection)
- Mine (if Iron Vein available, +2 Gold)
- Tax Office (+1 Gold per population)

#### The Research Center
**Focus**: Accelerate technology progress (future feature)

Key Buildings:
- Library (+2 Research)
- Academy (+3 Research, requires Library)
- Archive (if Ancient Ruins available, +3 Research)
- Observatory (+2 Research)

---

## Resource Flow

### Gold Economy

**Income Sources:**
| Source | Amount | Notes |
|--------|--------|-------|
| Base City Income | 5/city | Always |
| Population | +1/pop | Scales with growth |
| Market | +3 | Building |
| Bank | +5 | Requires Market |
| Mine | +2 | Requires Iron Vein |
| Deep Mine | +3 | Requires Mineral Deposit |
| Trade Post | +2/connection | Per friendly city |
| Gold Deposit (feature) | +5 | If in territory |

**Expenses:**
| Expense | Cost | Notes |
|---------|------|-------|
| Unit Recruitment | Varies | One-time |
| Building Construction | Varies | One-time |
| Legion Creation | 100 | One-time |
| Unit Maintenance | 0 | Units don't cost upkeep (design choice) |

**Design Note**: Gold is for building/recruiting. No ongoing upkeep keeps the game simpler and more aggressive (no incentive to disband).

### Mana Economy (Capacity System)

**Cap Sources:**
| Source | Mana Cap | Notes |
|--------|----------|-------|
| Mage Tower | +5 | Building, repeatable |
| Temple | +3 | Building, repeatable |
| Mana Well | +3 | Requires Mana Spring |
| Crystal Sanctum | +5 | Requires Crystal Cave |
| Necropolis | +4 | Requires Haunted Barrow |
| Dragon Shrine | +3 | Requires Dragon Bones |
| Arcane Nexus | +6 | Requires Ley Line Nexus |
| Sacred Grove (feature) | +1 | Automatic if in territory |
| Mana Spring (feature) | +2 | Automatic if in territory |

**Cap Consumers:**
See Unit Design Doc for per-unit upkeep costs.

**Key Insight**: Your mana cap is determined by:
1. How many Mage Towers/Temples you build
2. What magical terrain features you control
3. What exploitation buildings you've constructed

A player with no magical terrain features is limited to Mage Tower mana only (5 per tower). A player who controls Crystal Cave + Haunted Barrow + multiple Mana Springs could have 20+ mana cap advantage.

### Production Economy (Future)

Production determines how fast cities build things.

**Production Sources:**
| Source | Production | Notes |
|--------|------------|-------|
| Base | 2 | Per city |
| Population | +0.5/pop | Scales |
| Quarry | +2 | Requires Quarry Site |
| Lumber Mill | +1 | Requires Timber Stand |
| Master Forge | +5 | Requires Adamantine Vein |
| Titan Forge | +10 | Requires Titan's Grave |

Production reduces build times for units and buildings.

---

## Strategic Implications

### Early Game (Turns 1-20)

**Priorities:**
1. Scout for valuable improvements
2. Expand to claim key terrain
3. Research basic military techs
4. Build economy foundation

**Key Decisions:**
- Rush for a specific improvement vs. broad expansion?
- Military first (to take improvements) vs. economy first (to fund expansion)?

### Mid Game (Turns 21-50)

**Priorities:**
1. Build exploitation buildings for controlled improvements
2. Develop city specializations
3. Research improvement-gated tech branches
4. Contest rare improvements with enemies

**Key Decisions:**
- Which tech branches to pursue based on controlled territory?
- When to attack enemies to steal their improvements?
- How to balance mana cap vs. gold economy?

### Late Game (Turns 51+)

**Priorities:**
1. Control or deny legendary improvements
2. Field armies with improvement-gated elite units
3. Counter enemy compositions with appropriate units
4. Push for victory condition

**Key Decisions:**
- Can you break enemy control of key improvements?
- Is the Armageddon counter manageable?
- Final army composition for boss fight?

---

## Asymmetric Dynamics

### The "Haves" vs "Have-Nots"

If Player A controls Crystal Cave and Player B doesn't:
- A can research High Sorcery branch
- A can build Crystal Sanctum (+5 mana cap)
- A can recruit Archmages and Summoned Elementals
- B cannot access any of this

**B's Options:**
1. **Conquer**: Take the Crystal Cave city
2. **Deny**: Raze the city so neither can use it
3. **Pivot**: Focus on branches B CAN access
4. **Counter**: Tech specifically to beat A's magic (anti-magic units?)

### Map Generation Fairness

To prevent unwinnable starts:
- Each player starts within range of at least 1 uncommon improvement
- Rare/Legendary improvements spawn in contested neutral territory
- No player starts with a rare improvement in their initial territory

### The Legendary Race

If there's one World Tree on the map:
- All players know where it is (visible after scouting)
- Whoever takes it first gets exclusive benefits
- Creates natural mid-game conflict point
- Worth dedicating significant resources to capture

---

## Improvement Exploitation Details

### The Exploitation Chain

1. **Discover**: Scout reveals improvement on map
2. **Control**: Expand cultural borders to include tile
3. **Build**: Construct the exploitation building in a nearby city
4. **Benefit**: Receive the improvement's bonuses
5. **Unlock**: Tech branches requiring this improvement become available

### Exploitation Building Costs

| Building | Gold Cost | Build Time | Requires Improvement |
|----------|-----------|------------|---------------------|
| Farmstead | 50 | 2 | Fertile Plains |
| Mine | 75 | 3 | Iron Vein |
| Lumber Mill | 60 | 2 | Timber Stand |
| Quarry | 80 | 3 | Quarry Site |
| Stables | 100 | 4 | Pastureland |
| Mana Well | 120 | 4 | Mana Spring |
| Shrine | 100 | 3 | Sacred Grove |
| Archive | 150 | 5 | Ancient Ruins |
| Garrison | 80 | 3 | Watchtower Ruins |
| Deep Mine | 150 | 5 | Mineral Deposit |
| Crystal Sanctum | 200 | 6 | Crystal Cave |
| Necropolis | 200 | 6 | Haunted Barrow |
| Dragon Shrine | 180 | 5 | Dragon Bones |
| Master Forge | 250 | 7 | Adamantine Vein |
| Arcane Nexus | 300 | 8 | Ley Line Nexus |

### Legendary Exploitation Buildings

| Building | Gold Cost | Build Time | Requires | Exclusive Benefit |
|----------|-----------|------------|----------|-------------------|
| Grove of Ages | 400 | 10 | World Tree | Treants, Nature's Wrath |
| Binding Circle | 350 | 8 | Hellgate | Player-controlled demons |
| Oracle Temple | 300 | 8 | Oracle's Pool | Map vision, Prophecy |
| Titan Forge | 400 | 10 | Titan's Grave | Siege Titans |
| Restoration Project | 350 | 10 | Sunken Library | Free Lost Art tech |

---

## UI/UX Implications

### Tech Tree Display
- Full tree visible at all times
- Locked branches clearly marked with requirement icons
- Tooltip shows: "Requires: [Improvement] + [Building]"
- Unlocked-but-not-researched shown in full color
- Currently researching shown with progress bar

### City Screen
- Show available building slots clearly
- Gray out buildings requiring unavailable improvements
- Show which improvements are in city's territory
- Specialization suggestions based on available improvements

### Map Display
- Improvement icons visible on tiles
- Rarity indicated by icon border (common=white, uncommon=blue, rare=purple, legendary=gold)
- Contested improvements highlighted
- "You need this" indicator for improvements that unlock desired techs

---

## Migration from Collegia System

The current Collegia (rotating tech bazaar) will be **replaced** with:

1. **Standard tech tree** (always visible, some branches locked)
2. **Improvement-gating** (unlock branches by controlling territory)
3. **Building exploitation** (build specific structures to activate improvements)

This is a significant refactor but creates deeper strategic gameplay.

---

## Resolved Design Decisions

1. **Improvement Destruction**: Not implemented for now. Improvements are permanent fixtures.

2. **Boss Spawn / Corruption**: Deferred. Armageddon mechanics are a later problem.

3. **Trading Exploitation Rights**: Not implemented for now.

4. **Improvement Upgrading**: No. Common improvements cannot be upgraded to higher tiers.

5. **Multiple Cities, One Improvement**: No. Only one city can exploit a given improvement. Border reconciliation system needed when cities are nearby.

6. **Losing Territory**: If you lose the tile containing the improvement, the exploitation building is **destroyed**. You lose both the building and access to the improvement's benefits.

7. **Tech Permanence**: Losing your last mana source doesn't disable existing magical units—they become **non-replaceable**. You keep what you have but can't build more.

8. **Exploitation Range**: Improvements must be **within cultural borders** to be exploited. Expanding borders can bring new improvements into range.

9. **Building Slot Scarcity**: Intentional hard constraint. If you're out of slots, you cannot build more. Plan accordingly.

10. **Map Fairness**: Not a concern. This is an **asymmetric game**. Starting positions may favor certain strategies—adapt or conquer.

11. **Research System**: Standard Civilization-style system. Buildings generate research points ("beakers") which accumulate toward a selected technology. Player chooses which tech to research from available options.

12. **Corruption & Improvements**: Armageddon corruption **can destroy** common, uncommon, and rare improvements. **Unique/Legendary improvements are immune** to corruption—they're too magically significant to be erased.

---

## Summary

The economy design creates a game where:

- **Territory matters**: Controlling the right tiles unlocks entire capability branches
- **Cities specialize**: Building slots force meaningful choices
- **Plans adapt**: You can't fully plan your tech path until you see the map
- **Conflict has purpose**: Fighting over improvements has strategic meaning beyond just "more cities"
- **Asymmetry emerges**: Different players will have different capabilities based on map position

This should create games that feel different each time based on map generation and the emergent dynamics of territorial control.
