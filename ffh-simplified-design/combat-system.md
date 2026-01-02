# Combat System Design

Based on Ogre Battle 64's combat mechanics, adapted for a simplified 4X context.

## Overview

Combat occurs when legions meet on the strategic map. Resolution is automatic based on legion composition, formation, and soldier stats. The player's decisions happen *before* combat (army composition, formation, positioning) rather than during.

---

## Legion Structure

### Composition
- Each legion contains 5-8 soldiers
- Soldiers are arranged in a 3x3 grid (front/middle/back row)
- Position determines attack frequency and targeting

### Formation Grid
```
       Column 1    Column 2    Column 3
      ┌──────────┬──────────┬──────────┐
Front │ Soldier  │ Soldier  │ Soldier  │  ← Takes hits first, melee attacks
      ├──────────┼──────────┼──────────┤
Mid   │ Soldier  │ Soldier  │ Soldier  │  ← Protected, reduced attacks
      ├──────────┼──────────┼──────────┤
Back  │ Soldier  │ Soldier  │ Soldier  │  ← Most protected, ranged/magic
      └──────────┴──────────┴──────────┘
```

---

## Attack Resolution

### Row-Based Attack Count
Position determines how many attacks a soldier gets per combat round:

| Row | Attacks |
|-----|---------|
| Front | 3 |
| Middle | 2 |
| Back | 1 |

### Attack Targeting
Each soldier type has attacks that preferentially target either:
- **Front** of enemy column (melee attacks)
- **Back** of enemy column (ranged/magic attacks)

Attacks hit the closest soldier in their preferred row within a column (or nearest adjacent column if empty).

### Speed-Based Ordering (ATB-Style)
Combat runs on a timeline:

1. Each soldier has a "next attack at" timestamp based on their speed stat
2. When their time comes, they execute their attack
3. Their next timestamp = current time + delay (based on speed)
4. All soldiers get their full allocation of attacks (1/2/3 based on row)
5. Slow soldiers clump their attacks at the end of the round; fast soldiers spread them out

**Key Insight**: Speed matters primarily for *killing before retaliation*. A dead soldier loses their remaining attacks. Thus:
- Most units: Speed is not critical (everyone gets their attacks eventually)
- Debuff units: Speed is crucial (disabling an enemy before they act denies their attacks)

### Combat Round Resolution
1. Sort all attacks by timestamp
2. Execute attacks in order, removing dead soldiers
3. Dead soldiers lose any remaining attacks
4. After all attacks resolve, tally damage dealt by each side
5. Side that dealt less total damage loses and retreats

---

## Soldier Types (Steal from OB64)

### Starter Palette

| Type | Row | Attacks Target | Notes |
|------|-----|----------------|-------|
| Fighter | Front | Enemy Front | Basic melee, durable |
| Knight | Front | Enemy Front | Stronger melee, slower |
| Archer | Back | Enemy Back | Ranged physical damage |
| Mage | Back | Enemy Back | AoE magical damage |
| Cleric | Back | Allies | Healing instead of damage |

### Later Additions (for iteration)
- **Cavalry/Flying**: Special movement or flanking
- **Witch**: Fast debuffs (paralysis, petrify) - makes speed critical
- **Heroes**: Elite units with special abilities, possibly legion commanders

---

## Retreat Mechanics

### When Retreat Happens
- The side that dealt less damage in a combat round retreats
- Retreat direction: away from the enemy (toward the closest empty tile in the opposite direction)

### Consequences of Retreat
- Legion is displaced on strategic map
- Dead soldiers remain dead
- Surviving soldiers may be wounded (health reduced)
- **Not** a death sentence—legions can retreat and re-engage

### Healing vs Replenishment
- **Healing** (recovering soldier HP): Requires garrison in a friendly city
- **Replenishment** (adding new soldiers): Requires being anywhere in friendly borders

---

## Design Space

The tactical depth comes from:

1. **Composition choices**: What soldier types to include
2. **Formation choices**: Where to place each soldier (front row knight or mid row?)
3. **Counter-building**: Responding to known enemy compositions
4. **Attrition management**: When to commit, when to retreat, when to heal up

### Example Tactical Scenarios

**vs Mage-heavy enemy**:
- Problem: Their back-row mages hit your back row
- Solutions: Fast units to kill mages first; stack front row to absorb; clerics to outheal

**vs Knight-heavy enemy**:
- Problem: High damage to your front row
- Solutions: Deep front row to absorb; ranged/magic to kill from back; flank if possible

---

## Concrete Numbers

### Movement
- **Legion movement**: 3 tiles per turn (4-directional, no diagonals)
- **Terrain costs**: All terrain costs 1 movement (simplest thing that works)
- **Mountains**: Impassable (legions cannot enter)

### Damage Formula
```
Base Damage = Attacker.attack - Defender.defense
Minimum Damage = 5 (attacks always do something)
Final Damage = max(Base Damage, 5)
```

With terrain defense bonus:
```
Effective Defense = Defender.defense × (1 + terrain_bonus)
```

### Healing
- **Rate**: 20 HP per soldier per turn while garrisoned
- **Full heal**: ~5 turns to fully heal a badly wounded soldier

### Soldier Replenishment
- **Cost**: Same as original recruitment cost
- **Location**: Any tile within your borders (not just cities)
- **Limit**: Cannot exceed legion capacity (8)

### Starter Soldier Stats

| Type | HP | Attack | Defense | Speed | Gold Cost | Mana Cost |
|------|-----|--------|---------|-------|-----------|-----------|
| Fighter | 100 | 20 | 15 | 50 | 50 | 0 |
| Knight | 150 | 30 | 20 | 30 | 100 | 0 |
| Archer | 60 | 25 | 5 | 60 | 60 | 0 |
| Mage | 50 | 35 | 5 | 40 | 80 | 10 |
| Cleric | 70 | 10 | 10 | 45 | 70 | 5 |

### Speed and Attack Timing
Combat round duration: 100 time units

```
Time between attacks = 100 / speed
```

Example: Fighter (speed 50) attacks at t=2, t=4, t=6 (if in front row, 3 attacks)
Example: Knight (speed 30) attacks at t=3.3, t=6.6, t=10 (third attack may be late)

### Win/Loss Calculation
After all attacks resolve:
```
Attacker Total Damage = sum of damage dealt to defender's soldiers
Defender Total Damage = sum of damage dealt to attacker's soldiers

Winner = side that dealt more total damage
Loser retreats
```

Tie: Defender wins (defender's advantage)

---

## Prototype Implementation Notes

For initial prototype:
1. Implement 3-4 soldier types (Fighter, Archer, Mage, Cleric)
2. Use the damage formula above
3. Speed can be mostly uniform initially
4. Skip debuffs/status effects until base combat works
5. Test with small legions (3-5 soldiers) before scaling up

---

## Design Decisions (Resolved)

### Combat Rounds: One and Done

**Decision**: Single combat round per engagement. Legions clash, damage is dealt, loser retreats.

**Rationale**:
- Keeps combats quick and the strategic map moving
- Matches OB64 pattern (clash → retreat → re-engage if desired)
- If you want sustained pressure, move back into contact next turn
- Avoids "trapped in combat for 5 turns" frustration

**For prototype**: Implement single-round combat. Multi-round siege mechanics can be iteration content.

---

### Terrain Modifiers: Yes, But Simple

**Decision**: Terrain provides defense bonuses to the defender.

| Terrain | Defense Bonus |
|---------|---------------|
| Open | None |
| Forest | +10% |
| Hills | +15% |
| City (garrisoned) | +25% |
| City with Walls | +40% |

**Rationale**:
- Gives terrain strategic value beyond resources
- Creates meaningful positional play
- Simple enough to implement and understand

**For prototype**: Start with city bonus only; add terrain bonuses in iteration.

---

### Tactics Settings: Deferred

**Decision**: Skip tactics settings for prototype. Use default targeting (attack nearest valid target in preferred row).

**Future iteration**: Add 2-3 tactics options:
- **Aggressive**: Target enemy back row (kill their mages/archers)
- **Defensive**: Target enemy front row (eliminate their damage dealers)
- **Leader Hunt**: Prioritize heroes

**Rationale**: OB64 had these, they add depth, but they're not core to proving the combat system works.

---

### Heroes: Elite Soldiers with Abilities

**Decision**: Heroes are special soldiers that occupy a normal slot but have:
- Enhanced stats (more HP, damage, speed)
- One unique ability (passive or active)
- Maximum one hero per legion

**Hero Properties**:
- Can die in combat (like any soldier)
- Expensive to recruit (high gold cost, possibly mana)
- Slow to replace (limited availability or long cooldown)
- Named characters with personality (FFH flavor)

**Example Abilities**:
- **Warlord**: Front-row soldiers in this legion get +1 attack
- **Archmage**: Mages in this legion hit all enemies in target row (AoE)
- **Champion**: This hero attacks first regardless of speed
- **Healer-Saint**: Cleric healing in this legion is doubled

**Rationale**:
- Heroes are core FFH flavor
- Gives the player "special" units to care about
- Creates meaningful composition decisions (which hero for which legion?)
- Death of a hero feels significant without being game-ending

**For prototype**: Implement heroes as stat-boosted soldiers. Add abilities in iteration.
