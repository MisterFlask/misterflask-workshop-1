# Fall From Heaven Simplified: Design Overview

## Vision Statement

A radically simplified 4X game that preserves Fall From Heaven 2's distinctive flavor—asymmetric civilizations, the Armageddon Counter, heroes, magic, and emergent storytelling—while eliminating the micromanagement that plagues Civilization-style games.

## Core Problems Being Solved

### 1. Micromanagement Overload
Civ4/FFH2 requires dozens of clicks to execute simple strategic intent. "I want this to be a military production city" becomes: worker actions, building queues, tile assignments, unit commands, etc.

### 2. AI Asymmetry Failure
Symmetric 4X games give players complex tools the AI can't use intelligently. This leads to AI "cheating" via bonus resources—boring because the player can't interact with the AI's internal economy.

### 3. Late-Game Slog
Standard 4X games often result in "I've clearly won but need 2 hours to mop up."

## Core Design Pillars

### Pillar 1: Collapse Decision Layers
Replace dozens of micro-decisions with few, genuine, contextual choices. The question is "what should this city focus on," not "execute the 40-click sequence to make it happen."

### Pillar 2: Legions, Not Units
Maximum 5 player-controlled legions (groups of 5-8 soldiers). Each legion is a strategic asset worth caring about. Combat auto-resolves based on composition and formation choices.

### Pillar 3: AI as Dynamic Terrain
AI factions aren't opponents trying to "win"—they're state machines with defined behaviors. Each creates a different shaped problem for the player to manage, exploit, or eliminate.

### Pillar 4: The Armageddon Endgame
The Armageddon Counter advances through faction actions and time. When full, an endgame boss faction activates. Player wins by taking its capital. This creates a natural climax and eliminates victory condition fragmentation.

---

## Core Game Loop

```
1. Cities produce resources and unlock soldier types (via building slots)
2. Player recruits soldiers with gold, slots them into legions
3. Player positions legions on the strategic map
4. Combat auto-resolves when legions meet (OB64-style)
5. Soldiers die, legions retreat; healing requires garrison, replenishment requires being in borders
6. AI factions execute their behavior loops, advancing/interacting with the Armageddon Counter
7. Counter fills → Endgame boss activates → Final confrontation
```

---

## Key Systems (Detailed in Separate Documents)

| System | Document | Summary |
|--------|----------|---------|
| Combat | [combat-system.md](combat-system.md) | OB64-style auto-resolve with formation, soldier types, speed-based attack ordering |
| AI Factions | [ai-faction-design.md](ai-faction-design.md) | State machines with behavior loops, threat profiles, player interaction points |
| Economy & Cities | [economy-and-cities.md](economy-and-cities.md) | Building slots, automatic population, gold-based recruitment |
| Technical | [technical-design.md](technical-design.md) | HTML/TypeScript implementation, architecture, phased build plan |

---

## What We're Stealing

### From Ogre Battle 64
- Combat resolution mechanics
- Soldier types (fighter, knight, archer, mage, cleric, etc.)
- Formation/tactics system
- Front row/back row targeting
- Legion retreat on combat loss

### From Total War
- Cities have limited building slots gated by population
- Population grows mostly automatically
- Building choices create city identity

### From King of Dragon Pass
- Event-driven elements possible
- Strategic decisions presented rather than micromanaged

---

## Simplest-Thing-That-Works Decisions

These can be iterated upon:

- **Map**: Square grid
- **Movement**: One legion move per turn
- **Turn Order**: Player, then each AI faction in sequence
- **Map Generation**: Standard procedural
- **Endgame Boss**: Just sends legions toward players
- **Resources**: Start with gold and per-province population only

---

## Victory and Loss Conditions

### Victory
**Defeat the endgame boss faction by taking its capital** when the Armageddon Counter fills.

All factions are playing toward the same endgame, just with different strategies for positioning themselves when it triggers.

### Loss
The player loses if:
- All player cities are captured (no cities remaining)
- All player legions are destroyed AND player cannot afford to recruit a new one
- The endgame boss captures the player's last city

---

## Starting Conditions

### Player Start
- **Cities**: 2 (one developed with 2 slots, one new with 1 slot)
- **Legions**: 1 (with 4 soldiers: 2 Fighters, 1 Archer, 1 Cleric)
- **Gold**: 200
- **Mana**: 0

### AI Faction Start
Each AI faction starts with:
- **Cities**: 1-2 depending on faction type
- **Legions**: 1 (composition varies by faction)
- **Resources**: Enough to function but not overwhelming

### Map Setup
- Player starts in one corner/edge region
- AI factions distributed around the map with buffer zones
- Neutral cities scattered in between (can be captured by anyone)
- Endgame boss spawn location marked but empty until triggered

---

## Legion Rules

### Creating New Legions
- **Cost**: 100 gold to create an empty legion
- **Location**: Must be created at a city you control
- **Limit**: Maximum 5 legions at any time
- **Starts empty**: Must recruit soldiers into it separately

### Legion Capacity
- Minimum: 1 soldier (below this, legion is destroyed)
- Maximum: 8 soldiers
- Can reorganize soldiers between legions when both are in the same city

---

## City Capture Rules

### Capturing a City
1. Move a legion onto a tile containing an enemy city
2. If the city has a garrisoned legion, combat occurs first
3. If attacker wins (or no garrison), city is captured
4. Captured city enters 3-turn occupation penalty

### Garrisoned vs Passing Through
- A legion ON a city tile is considered "garrisoned" (heals, defends)
- Only one legion can occupy a tile at a time
- If a city has no garrison, any enemy legion can capture it by moving there

### Neutral Cities
- Start unowned, have no garrison
- First faction to move a legion there claims it
- No occupation penalty for neutral cities

---

## Next Steps for Prototyping

1. Implement basic square grid map with cities and legion movement
2. Create 3-4 soldier types with OB64 combat resolution
3. Build one AI faction as proof-of-concept state machine
4. Implement Armageddon Counter and placeholder endgame boss
5. Iterate based on playtesting
