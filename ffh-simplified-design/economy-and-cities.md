# Economy and Cities Design

## Design Goals

1. **Few decisions, not many**: Replace 40-click city management with 2-3 meaningful choices
2. **Genuine choices**: No solved optimization problems ("always build granary first")
3. **Context-dependent answers**: The right choice depends on game state, geography, and strategy
4. **City identity through geography and buildings**: Cities differ because of where they are and what you build, not citizen micromanagement

---

## Population System

### Core Principle: Population is a Timer, Not a Resource

Population is **not** actively managed. It exists to:
- Gate building slots for new cities
- Prevent instant-metropolis settlements

### How It Works

1. Found a city → starts small (1 building slot)
2. Population grows automatically over time
3. Higher population → more building slots (e.g., 2 slots at pop 3, 3 slots at pop 5)
4. Certain buildings can accelerate growth (but this is optional optimization, not required)

### What This Eliminates

- Tile/citizen assignment
- Food micromanagement
- Granary-first solved puzzles
- "Work the food tiles" tedium

### Tall vs Wide

The classic 4X tension survives in a different form:
- **Wide**: Many small cities, each with 1-2 slots, spread across territory
- **Tall**: Fewer mature cities with 3+ slots and specialized buildings

---

## Building System (Total War Style)

### Building Slots

Cities have limited building slots based on population:

| Population | Slots |
|------------|-------|
| 1-2 | 1 |
| 3-4 | 2 |
| 5+ | 3 |

### Building Types

Buildings do one or more of:
- **Produce gold** (economy)
- **Unlock soldier types** (military options)
- **Provide other effects** (growth bonus, defense, mana production)

### Example Buildings

| Building | Effect |
|----------|--------|
| Market | +Gold per turn |
| Barracks | Unlocks Fighter recruitment |
| Stables | Unlocks Cavalry recruitment |
| Mage Tower | Unlocks Mage recruitment, +Mana |
| Temple | Unlocks Cleric recruitment |
| Walls | Defense bonus when garrisoned |
| Granary | Faster population growth |

### City Identity

Cities become specialized through:
1. **Geography**: Nearby resources (iron = military, river = trade, etc.)
2. **Building choices**: A city with Barracks + Stables is military; Market + Temple is economic/support

Losing a city isn't just "-1 city"—it's "I can't recruit cavalry anymore."

---

## Resource Model

### Primary Resource: Gold

Gold is the universal currency:
- Buildings produce it
- Soldier recruitment costs it
- Building construction costs it (maybe?)
- Trade generates it (if diplomacy exists)

### Secondary Resource: Population (Per-Province)

Each city has its own population:
- Used as the timer/gate for building slots
- Possibly used as recruitment pool (soldiers come from population?)

### Tertiary Resource: Mana (Optional)

For magic-heavy FFH flavor:
- Certain buildings produce mana
- Mana spent on spells, rituals, magical units
- Can be global or per-city

### What We're NOT Tracking

- Food as a separate resource
- Production/hammers as a separate resource
- Research/beakers (tech could be event-driven or tied to buildings)
- Culture/influence
- Happiness/health

---

## Recruitment System

### Soldiers Come From Cities

1. City must have appropriate building (Barracks for Fighters, etc.)
2. Pay gold cost
3. Soldier is added to a legion

### Recruitment Location

- Must recruit at city with the building
- Legion must be in (or sent to) that city, OR
- Soldiers are produced and can be "picked up" when legion visits

### Replacement vs Healing

| Action | Requirement | Effect |
|--------|-------------|--------|
| Heal wounded soldiers | Garrison in friendly city | Restore HP |
| Replace dead soldiers | Be in friendly territory | Add new soldiers (costs gold) |

---

## Production Queue: Gone

There is no production queue. Instead:
- Buildings are built by spending gold (instant or 1-turn)
- Soldiers are recruited by spending gold (instant or 1-turn)
- No "build swordsman, 4 turns remaining" tedium

If construction takes time, it's a simple "under construction" state, not a queue to manage.

---

## Economic Decisions

What the player actually decides each turn (economy-related):

1. **What buildings to construct** (when slots are available)
2. **What soldiers to recruit** (when gold is available)
3. **Legion composition** (which soldiers go where)
4. **City defense priority** (which cities get garrisoned legions)

That's it. No worker actions, no tile assignments, no production queues.

---

## Geographic Significance

Since we've removed tile-based bonuses, geography matters through:

### Resources
Specific tiles contain resources that provide bonuses:
- Iron: Military building effectiveness
- Gold deposit: Extra gold income
- Mana node: Mana production
- Horses: Cavalry recruitment

### Terrain
- Rivers: Trade bonuses? Defense bonuses?
- Mountains: Defense, possibly mage bonuses
- Forests: Elf-themed bonuses, ambush potential
- Coast: Naval options (if implemented)

### Strategic Position
- Chokepoints worth controlling
- Distance from threats
- Access to enemy territory

---

## Capturing vs Razing Cities

### Capturing (Default Choice)
- You gain a developed city
- Retains population (and thus building slots)
- You get their buildings (or can rebuild)
- Strictly better than razing in most cases

### Razing
- City is destroyed
- May advance Armageddon Counter
- Some factions benefit (Sheaim gain resources? Infernals summon units?)
- Generally worse unless your faction specifically rewards it

### Faction-Specific Rules
- **Most factions**: Capture is obviously better
- **Destruction factions**: Razing gives resources or advances their goals
- **Non-empire factions** (Elves): May not use cities the same way at all

---

## Design Decisions (Resolved)

### Technology System: Buildings ARE the Tech Tree

**Decision**: No separate technology/research system. Progression comes entirely from buildings.

**How it works**:
- Some buildings unlock other buildings (prerequisites)
- Example: Barracks → War Academy → Elite Barracks
- Example: Mage Tower → Arcane Library → Summoning Circle
- Faction-specific buildings exist (Sheaim get ritual buildings, Elves get grove buildings)

**Progression example**:
```
Tier 1 (available immediately):
  - Market, Barracks, Temple, Walls

Tier 2 (requires Tier 1 building + population 3+):
  - War Academy (requires Barracks) → unlocks Knights
  - Mage Tower (requires Temple) → unlocks Mages, produces Mana
  - Trade Hall (requires Market) → more gold

Tier 3 (requires Tier 2 building + population 5+):
  - Elite Barracks (requires War Academy) → unlocks elite soldiers
  - Arcane Sanctum (requires Mage Tower) → unlocks advanced magic
```

**Rationale**:
- Eliminates research micromanagement entirely
- Progression is tied to city development, not abstract "beakers"
- Creates natural pacing (can't rush to elite units without building up cities)
- Each city's buildings tell a story of specialization

**For prototype**: Flat building list, no prerequisites. Add tiered progression in iteration.

---

### Conquered Cities: Simple Occupation Penalty

**Decision**: Conquered cities have a temporary occupation penalty. No complex loyalty system.

**Mechanics**:
- Newly conquered cities cannot recruit soldiers for 3 turns
- Gold production reduced by 50% for 3 turns
- After 3 turns, city functions normally
- No rebellion mechanics, no loyalty meters, no garrisons required

**Rationale**:
- Creates a real cost to conquest (can't instantly use captured infrastructure)
- Simple timer is easy to understand and track
- Avoids "happiness/loyalty micromanagement" trap
- 3 turns is long enough to matter, short enough not to be frustrating

**Edge case**: If you lose and recapture the same city, penalty resets.

**For prototype**: Implement the 3-turn penalty. Adjust duration based on playtesting.

---

### Building Pillaging: Yes, Raiding Matters

**Decision**: Buildings can be destroyed without capturing the city. This is how economic raiding works.

**Mechanics**:
- **Pillage action**: Legion adjacent to enemy city can pillage instead of attacking
- Pillaging destroys one building (defender's choice or random)
- Pillager gains gold equal to ~50% of building cost
- City retains population and remaining buildings
- Pillaging does NOT trigger full combat (lighter resistance)

**Why this matters**:
- Gives the Hippus (and player raiding strategies) meaningful impact
- Economic warfare without territorial commitment
- Creates "defend your infrastructure" decisions
- Damaged cities need rebuilding, creating ongoing costs

**Rationale**:
- Raiding should matter economically, not just be harassment
- "Take the city" vs "just burn their stuff" is a real choice
- Supports asymmetric faction design (some factions raid, some conquer)

**For prototype**: Implement basic pillage action. Skip the "pillager gains gold" part initially if needed.

---

### Mana System: Global Pool, Simple Uses

**Decision**: Mana exists as a global resource (pooled from all cities). It has three uses.

**Mana Production**:
- Mage Tower: +2 mana/turn
- Arcane Sanctum: +4 mana/turn
- Mana Node (map resource): +2 mana/turn to nearby city
- Some factions generate mana differently (Sheaim rituals, etc.)

**Mana Spending**:

| Use | Cost | Effect |
|-----|------|--------|
| Recruit magical units | Varies | Mages cost gold + mana; elite magical units cost more |
| Hero abilities | 5-20 | Some hero abilities cost mana to activate |
| Strategic rituals | 20-50 | Faction-specific powerful effects (future iteration) |

**What mana is NOT**:
- Per-city (it's global, pooled)
- Required for non-magical units
- Used for buildings (buildings cost gold only)

**Rationale**:
- Adds FFH magical flavor without complexity
- Creates tension: recruit more mages OR save for big ritual?
- Global pool means you don't micromanage per-city mana
- Clear distinction: gold is universal, mana is magical

**For prototype**: Implement mana as a number that goes up (from Mage Towers) and down (from recruiting mages). Skip rituals until core loop works.

---

### Resource Interaction Summary

| Resource | Source | Spent On |
|----------|--------|----------|
| Gold | Buildings, trade, pillaging | Soldiers, buildings, diplomacy |
| Population | Automatic growth | Gates building slots (not spent) |
| Mana | Mage buildings, nodes | Magical units, hero abilities, rituals |

This creates a simple but functional economy with meaningful choices.
