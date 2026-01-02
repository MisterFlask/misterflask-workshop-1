# AI Faction Design

## Philosophy: AI as Dynamic Terrain

AI factions are **not** opponents trying to "win" in the optimization sense. They are **state machines** with defined behaviors that create interesting problems for the player.

This is an explicit acknowledgment of reality: designing an AI that plays a complex 4X game as well as a human is an unsolved (perhaps unsolvable) problem. Rather than pretend otherwise and resort to "cheating" bonuses, we give each AI faction bespoke rules that create emergent gameplay.

**Key Insight**: "Stop the faction that's burning everything" is a much more tractable AI problem than "execute a 200-turn economic optimization strategy."

---

## Faction Design Framework

Every AI faction needs three components:

### 1. Behavior Loop
What the faction does when left alone. This should be:
- Simple enough to implement as a state machine
- Interesting enough to create varied game states
- Faction-thematic

### 2. Threat Profile
How the faction makes the player's life harder:
- Military pressure?
- Economic damage?
- Advancing the Armageddon Counter?
- Enabling other factions?

### 3. Player Interaction Point
What the player can DO about it:
- Military solutions (destroy their legions, take their cities)
- Economic solutions (bribe, trade)
- Targeted disruption (assassinate leaders, raid specific sites)
- Diplomatic solutions (redirect aggression elsewhere)

---

## Example Factions

### The Sheaim (Apocalypse Cultists)

**Behavior Loop**:
- Build up cities with ritual buildings
- Run rituals that advance the Armageddon Counter
- Summon demon units when rituals complete

**Threat Profile**:
- Accelerates endgame before player is ready
- Demon legions are powerful late-game threats
- Makes other "evil" factions stronger

**Player Interaction Points**:
- Raid ritual sites to disrupt ceremonies
- Assassinate priests/cultists (if assassination mechanic exists)
- Diplomacy: can you convince them to slow down?
- Military: take their ritual cities
- Alliance: if player is also going for fast Armageddon, support them

---

### The Hippus (Horse Raiders)

**Behavior Loop**:
- Roam with fast cavalry legions
- Raid nearby cities (any faction, including player)
- Retreat when opposed by superior force
- Move to new hunting grounds periodically

**Threat Profile**:
- Constant economic harassment
- Difficult to pin down and destroy
- Can destabilize regions, making other threats worse

**Player Interaction Points**:
- Bribe them to raid someone else
- Destroy their horse supply/breeding grounds
- Wall up and become a less appealing target than neighbors
- Trap and destroy their mobile legions
- Hire them as mercenaries

---

### The Illians (Winter Faction)

**Behavior Loop**:
- Expand territory based on weather/cold spreading mechanic
- Expand slowly but inevitably
- Don't use settlers—territory grows "organically" based on their presence

**Threat Profile**:
- Gradual territorial loss
- Terrain modification (frozen lands less productive?)
- Long-term inevitability if not checked

**Player Interaction Points**:
- Military: push back the frost line
- Special actions: rituals/magic to counter the cold
- Take key "anchor" cities that spread the cold
- Alliance: if player benefits from cold somehow

---

### The Elves (Sacred Grove Guardians)

**Behavior Loop**:
- Control and protect sacred grove tiles
- Minimal expansion—don't empire-build
- Aggressive defense of grove territory
- Passive elsewhere

**Threat Profile**:
- Block access to valuable grove hexes
- Strong defensive position
- Don't actively threaten but don't help either

**Player Interaction Points**:
- Diplomacy: negotiate access to specific groves
- Military: costly to attack, but possible
- Trade: offer something they value for passage
- Ignore: if groves aren't strategically critical to you

---

## Implementation Notes

### State Machine Structure

Each faction AI can be modeled as:

```
State Machine:
  Current State: [Building / Raiding / Ritual / Defending / ...]

  Each Turn:
    1. Evaluate transition conditions
    2. If condition met → change state
    3. Execute current state's action

  Actions are simple:
    - Move legion toward target
    - Build specific building
    - Run ritual
    - Retreat to safe location
```

### Legibility

AI behavior should be **readable** by the player:
- Visual tells for faction state (Sheaim ritual sites glow, Hippus camps are visible)
- Predictable patterns (Hippus raid every N turns, Sheaim need M turns to complete rituals)
- Scouts/intelligence can reveal upcoming actions

This creates counterplay: you can see the Sheaim preparing a ritual and choose to disrupt it.

---

## Faction Interaction with Armageddon Counter

| Faction | Counter Relationship |
|---------|---------------------|
| Sheaim | Actively advances it (rituals) |
| Hippus | Advances slowly (destruction from raids) |
| Illians | Neutral or slight advance (cold = death) |
| Elves | Tries to slow it (protects natural order) |
| [Evil empire] | Advances through war |
| [Good kingdom] | Tries to slow it |

This creates natural faction alliances/conflicts and gives the player diplomatic levers.

---

## Endgame Boss Faction

When the Armageddon Counter reaches 100:

### Spawn Mechanics
- **Location**: Predetermined "Hell Gate" tile (marked on map from game start, center-ish)
- **Capital**: Spawns with a fortified capital city (3 building slots, Walls pre-built)
- **Initial Forces**: 2 legions of demons (8 soldiers each, powerful stats)

### Boss Soldier Type: Demon

| Stat | Value |
|------|-------|
| HP | 120 |
| Attack | 35 |
| Defense | 15 |
| Speed | 55 |
| Preferred Row | Front |
| Targets | Front |

Demons are individually stronger than most player units. The boss wins through attrition if not stopped quickly.

### Behavior Loop
```
Each turn:
  1. Spawn 1-2 new demons at capital (if gold allows)
  2. If any legion has < 4 soldiers, merge legions or reinforce
  3. For each legion:
     - Find nearest enemy city (any faction, including player)
     - Move toward it
     - Attack if adjacent
  4. Never retreat (fights to the death)
```

### Boss Economy
- **Starting Gold**: 500
- **Income**: 50 gold/turn (from unholy sources, not buildings)
- **Demon Cost**: 100 gold each
- **Maximum Legions**: 4

### Threat Scaling
The boss gets stronger the longer the game goes:
- Turns 1-5 after spawn: 2 legions, slow buildup
- Turns 6-10: 3 legions, aggressive expansion
- Turns 11+: 4 legions, full assault on all fronts

### Defeating the Boss
- **Victory Condition**: Capture the boss capital (move a legion onto it after defeating any garrison)
- **No Retreat**: Boss legions fight to destruction, never retreat
- **Capital Defense**: Boss always keeps at least 1 legion garrisoned if possible

### Strategic Implications
- Players who rush Armageddon face the boss with fewer resources
- Players who delay too long face a prepared boss with multiple strong legions
- Surviving AI factions become temporary allies (enemy of my enemy)
- Map position matters: being close to the Hell Gate is dangerous

**Player Interaction Points**:
- Survive the initial onslaught and counterattack
- Race other factions to claim the capital
- Use surviving AI factions as meat shields or allies
- Exploit the boss's predictable "attack nearest" behavior

---

## Design Decisions (Resolved)

### Faction Count: 4-6 Per Game

**Decision**: 4 factions minimum, 6 maximum per game.

| Game Size | Factions |
|-----------|----------|
| Small map | 3-4 |
| Standard | 4-5 |
| Large | 5-6 |

**Rationale**:
- Fewer than 4 lacks variety and diplomatic complexity
- More than 6 becomes hard to track and dilutes each faction's impact
- Each faction should be meaningfully different (not just palette swaps)

**For prototype**: Start with 3 factions (e.g., Sheaim, Hippus, one defensive faction). Add more after core loop works.

---

### Faction Elimination: Yes, Permanent

**Decision**: Factions can be fully eliminated mid-game. Take all their cities and destroy all their legions → they're gone.

**What happens**:
- Their territory becomes neutral/unclaimed OR absorbed by conquering player
- No more legion spawns, no more faction actions
- The game gets simpler as factions are eliminated

**Rationale**:
- Gives the player meaningful mid-game victories
- Creates strategic choice: eliminate a threat now vs. let them pressure your rivals
- Reduces late-game complexity naturally
- "I killed the Hippus" is a satisfying achievement

**Edge case**: If an AI faction eliminates another AI faction, same rules apply. Map naturally consolidates.

---

### Eliminated Faction Remnants: No Mechanical Effects

**Decision**: Eliminated factions leave no persistent mechanical effects. They're just gone.

**What remains**:
- Their former cities (now owned by whoever conquered them, or neutral)
- Flavor/visual remnants if desired (ruins, corrupted terrain for Sheaim, etc.)
- No ongoing mechanics, spawns, or effects

**Rationale**:
- Keeps the game clean and comprehensible
- "Dead means dead" is easy to understand
- Avoids zombie-faction complexity
- Flavor remnants can be added for atmosphere without mechanical burden

**For prototype**: No remnants at all. Add visual flavor in polish phase.

---

### Modifying Faction Behavior: Limited Diplomacy

**Decision**: Player can influence faction behavior through diplomacy, but cannot change a faction's fundamental nature.

**What diplomacy can do**:
- **Redirect aggression**: Pay the Hippus to raid someone else instead of you
- **Temporary truces**: Buy N turns of peace (costs gold, possibly other resources)
- **Information**: Learn what a faction is planning
- **Request aid**: Ask a faction to attack your enemy (costs heavily, may fail)

**What diplomacy cannot do**:
- Make the Hippus stop raiding entirely (they will always raid *someone*)
- Make the Sheaim abandon their apocalyptic goals
- Convert a faction to permanent alliance
- Change a faction's core behavior loop

**Rationale**:
- Factions are "dynamic terrain" with fixed behaviors—this is a feature, not a bug
- Diplomacy gives player agency without making AI behavior unpredictable
- "Redirect the threat" is tractable; "become friends forever" is not
- Maintains faction identity and ensures they remain interesting problems

**Implementation**:
Each faction has 2-3 diplomatic options:
- Hippus: Bribe (redirect raids), Hire (temporary mercenary legion)
- Sheaim: Cannot be reasoned with (or: slow rituals in exchange for resources?)
- Elves: Negotiate passage, Trade for grove access
- Illians: Cannot be negotiated with (force of nature)

**For prototype**: Skip diplomacy entirely. Add it after core combat/economy works.
