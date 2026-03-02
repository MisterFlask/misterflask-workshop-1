# Eldritch Cases - Game Design Document

## High Concept

**XCOM meets Slay the Spire in 1920s Lovecraftian horror.**

Players run a paranormal investigation agency in the 1920s, selecting cases from an escalating supernatural crisis. Each case is a tactical combat encounter using Slay the Spire mechanics, but instead of one character, the player deploys a team of 3 investigators whose card decks shuffle together into a single draw pile.

Investigators have persistent decks built from their class card pools. They can die permanently. The horror escalates on a clock. Win before time runs out or the world ends.

**Target run length:** 2 hours

---

## Core Loop

```
STRATEGIC LAYER (Between Cases)
├── View available cases (2-3 options with different rewards/difficulty)
├── Select 3 investigators from roster for the mission
├── (Optional) Spend resources on cards, healing, recruitment
└── Deploy to case

TACTICAL LAYER (During Case)
├── Slay the Spire combat with merged 3-investigator deck
├── Defeat enemies before investigators die
├── Downed investigators can be executed (permadeath)
└── Complete case objectives

RESOLUTION
├── Gain rewards (cards, resources, artifacts)
├── Investigators heal partially between cases
├── Doom clock advances
└── Return to strategic layer
```

---

## Tactical Combat (Slay the Spire Core)

### Basic Mechanics

| Element | Value | Notes |
|---------|-------|-------|
| Energy per turn | 3 | Shared across all investigators |
| Hand size | 5 cards | Drawn from merged deck |
| Block persistence | None | Block resets each turn |
| Draw pile | All 3 investigators' decks shuffled | ~30-45 cards total |

### Investigators in Combat

Each investigator is a separate entity on the field with:
- **Individual HP pool** (class-dependent, roughly 40-60)
- **Individual block** (applied per-investigator)
- **Status effects** tracked individually (vulnerable, weak, etc.)

Cards may:
- Target a specific investigator ("Give the Occultist 10 block")
- Target all investigators ("All investigators gain 5 block")
- Require a specific investigator to "use" them (class-locked cards)

### Targeting

**Enemies** can target:
- A specific investigator (shown via intent icons)
- All investigators (AoE attacks)
- Random investigator

**Player cards** can target:
- A specific enemy (single-target attacks)
- All enemies (AoE)
- Self (the investigator whose card it is)
- Specific ally investigator
- All investigators

### Death and Permadeath

1. **Downed state**: When an investigator hits 0 HP, they are *downed* (not dead)
2. **Execution**: If a downed investigator takes ANY damage, they die permanently
3. **Recovery**: Downed investigators can be revived during combat via specific cards
4. **Mission end**: Downed investigators survive if the mission succeeds, but with penalties

This creates tense moments where you must protect downed allies or lose them forever.

---

## Investigator Classes

Classes are defined in separate design documents. See the `classes/` subdirectory.

### Starting Decks

Each investigator begins with a **10-card starter deck**:
- 4 Basic cards (shared across all classes - Strike and Defend equivalents)
- 6 Class cards (define initial class identity)

With 3 investigators, starting merged deck = **30 cards**.

### Deck Building

Investigators add cards to their personal decks through:
- **Case rewards** (choose 1 of 3 cards for a specific investigator)
- **Card shop** (spend resources between cases)
- **Events** (narrative moments with card rewards)

Cards are **class-locked** - investigators can only add cards from their own class.

---

## Strategic Layer

### The Case Board

Between tactical missions, players see **2-3 available cases**:

```
┌─────────────────────────────────────────────────────────┐
│  CASE BOARD                          DOOM: ████████░░  │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  [THE MARSH LIGHTS]        [MISSING PROFESSOR]          │
│  Difficulty: ★★☆          Difficulty: ★☆☆              │
│  Enemies: Deep Ones        Enemies: Cultists            │
│  Reward: Rare Card         Reward: 50 Resources         │
│  Doom +1                   Doom +2                      │
│                                                         │
│                  [THE DREAMLANDS GATE]                  │
│                  Difficulty: ★★★                       │
│                  Enemies: Nightgaunts                   │
│                  Reward: Artifact + Card                │
│                  Doom +0 (urgent case)                  │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

Each case shows:
- **Difficulty rating** (enemy strength)
- **Enemy types** (lets you pick appropriate investigators)
- **Rewards** (cards, resources, artifacts)
- **Doom impact** (how much the clock advances if you take another case instead)

### The Doom Clock

A **10-segment doom track** represents escalating supernatural crisis.

- Doom advances when you **don't** take urgent cases
- Doom advances via certain **enemy abilities** during combat
- Doom advances through **failed cases** (retreat or TPK)
- At **Doom 10**, the final case unlocks - win or lose the run

The doom clock creates:
- Meaningful case selection (can't do everything)
- Time pressure without turn limits in combat
- Narrative escalation toward climax

### Roster Management

**Starting roster:** 4 investigators (can deploy 3, keep 1 reserve)

**Recruitment:** Spend resources between cases to recruit new investigators
- New investigators start with basic 10-card decks
- Replaces permadeath losses
- Allows roster expansion for flexibility

**Recovery:**
- Investigators heal **50% of missing HP** between cases
- Downed investigators recover with a **permanent wound** (reduced max HP? negative card?)
- Full healing available for resource cost

### Resources

Single resource type: **Clues** (thematic, simple)

Used for:
- Buying cards from shop
- Recruiting new investigators
- Full healing
- Removing cards from decks

---

## Enemies

### Enemy Design Philosophy

Enemies should:
- Have clear **intent display** (Slay the Spire style)
- Create **tactical puzzles** (not just HP sponges)
- Threaten **specific investigators** (forces protection decisions)
- Scale in **complexity** not just numbers

### Enemy Types by Tier

**Tier 1 - Cultists & Mundane Threats**
- Low HP, predictable patterns
- Introduce basic mechanics
- Examples: Cultist, Thug, Guard Dog

**Tier 2 - Lesser Horrors**
- Medium HP, special abilities
- Require tactical responses
- Examples: Deep One, Ghoul, Mi-Go

**Tier 3 - Greater Horrors**
- High HP, complex patterns
- Multi-phase or summoning
- Examples: Shoggoth, Star Spawn, Dimensional Shambler

**Bosses - Case Climaxes**
- Unique mechanics per boss
- Often tied to specific case narratives
- Examples: High Priest, The Thing in the Marsh, Dreaming God's Avatar

### Example Enemy: Deep One

```
DEEP ONE
HP: 30
Pattern: Attack 8 → Attack 12 → Buff All (+2 strength) → Repeat

Special: CALL OF THE DEEP
- When 3+ Deep Ones present, all gain +1 strength
- Encourages AoE or priority targeting
```

---

## Artifacts (Relics)

Artifacts are persistent bonuses acquired through cases. Shared across the team (not per-investigator).

### Example Artifacts

| Artifact | Effect |
|----------|--------|
| Elder Sign | At start of combat, apply 1 Weak to all enemies |
| Pocket Watch | Once per combat, gain 1 additional energy |
| Necronomicon Page | Cards that cost HP deal +3 damage |
| Detective's Badge | Start each combat with 1 extra card drawn |
| Lucky Rabbit's Foot | Once per combat, redraw your hand |
| Ritual Candles | Exhaust effects trigger twice |

---

## Run Structure

### Typical 2-Hour Run

```
Phase 1: RISING DREAD (Doom 0-3) ~30 min
├── 3-4 cases
├── Tier 1 enemies primarily
├── Build investigator decks
└── Establish team composition

Phase 2: GATHERING STORM (Doom 4-6) ~40 min
├── 3-4 cases
├── Tier 2 enemies appear
├── Harder choices between cases
└── Likely first permadeaths

Phase 3: DARKEST HOUR (Doom 7-9) ~30 min
├── 2-3 cases
├── Tier 3 enemies common
├── Desperate resource management
└── Recruit replacements or push wounded

Phase 4: FINAL CONFRONTATION (Doom 10) ~20 min
├── 1 climactic case
├── Boss encounter
├── Win or lose the run
└── No second chances
```

### Win/Loss Conditions

**Victory:** Complete the final case at Doom 10
**Defeat:** All deployed investigators die (TPK) during any case

---

## Open Design Questions

### High Priority (Blocks Implementation)

- [ ] Define investigator classes (see `classes/` subdirectory)
- [ ] Case generation algorithm (how to create variety?)
- [ ] Enemy roster (need ~15-20 enemies minimum)
- [ ] Boss designs (need 3-5 for variety)
- [ ] Card reward balance (how many cards per case?)
- [ ] Resource economy (clue costs for everything)

### Medium Priority (Can Iterate)

- [ ] Artifact pool size and balance
- [ ] Events between cases?
- [ ] Wound/trauma system for downed investigators
- [ ] Difficulty settings
- [ ] Investigator naming (random generation? player choice?)
- [ ] Card removal mechanics

### Low Priority (Polish)

- [ ] Specific narrative text for cases
- [ ] Art direction details
- [ ] Sound design
- [ ] Tutorial flow
- [ ] Statistics/run history

---

## Technical Notes

### Target Stack

- TypeScript
- Vite for build
- Canvas or DOM-based rendering (TBD)
- Local storage for run persistence

### Key Systems to Build

1. **Card engine** - draw, discard, exhaust, shuffle, effects
2. **Combat state machine** - turns, intents, resolution
3. **Entity system** - investigators, enemies, status effects
4. **Strategic layer** - case selection, roster, shop
5. **Progression** - doom clock, run phases, victory/defeat

---

## References

- **Slay the Spire** - Core combat loop, card mechanics, intent system
- **XCOM** - Strategic roster management, permadeath weight, mission selection
- **Arkham Horror LCG** - Theme, investigator archetypes, Lovecraftian tone
- **Monster Train** - Multi-unit tactical deckbuilder precedent
- **Griftlands** - Narrative deckbuilder structure

