# FFH Simplified - Implementation Todo List

## Priority 1: Critical Bug Fixes
These must be fixed for the game to function correctly.

- [ ] **Fix terrain defense bonus bug** - Combat.ts:172 applies terrain bonus to attacker instead of defender
- [ ] **Fix combat HP persistence** - Combat result HP changes not propagated back to original soldiers
- [ ] **Implement retreat logic** - Defeated legions are destroyed instead of retreating; need to move surviving soldiers

## Priority 2: Core Gameplay Loop
These are required for a minimally playable game.

- [ ] **City Management UI** - Panel for building construction and soldier recruitment
  - [ ] Show available buildings based on population slots
  - [ ] Show building costs and effects
  - [ ] Build button with gold deduction
  - [ ] Show recruitable soldiers based on buildings
  - [ ] Recruit button with gold deduction and legion assignment

- [ ] **Legion Management UI** - Panel for managing legion composition
  - [ ] Show soldiers in formation grid (3x3)
  - [ ] Allow repositioning soldiers between rows/columns
  - [ ] Show legion stats summary

- [ ] **Movement validation improvements**
  - [ ] Prevent moving through enemy legions
  - [ ] Show movement path preview
  - [ ] Validate movement points remaining

## Priority 3: AI System (Single Faction First)
Implement one AI faction to make the game playable.

- [ ] **AI Turn Execution Framework** - Game.ts processAITurn()
  - [ ] Loop through AI factions
  - [ ] Execute behavior based on faction state
  - [ ] Transition states based on conditions

- [ ] **Hippus AI (Raider)** - Simplest behavior pattern
  - [ ] Idle → Raiding when gold > threshold
  - [ ] Target selection: nearest weak player city
  - [ ] Pathfinding to target
  - [ ] Combat initiation
  - [ ] Retreat when legion HP < 30%
  - [ ] Return to home city to heal

- [ ] **AI Legion Movement** - Pathfinding implementation
  - [ ] A* or simple BFS pathfinding
  - [ ] Avoid impassable terrain (mountains, water)
  - [ ] Move toward target tile

- [ ] **AI Recruitment** - Basic army building
  - [ ] Build barracks if none exists
  - [ ] Recruit soldiers when gold available
  - [ ] Assign to existing or new legions

## Priority 4: Endgame System
Required for win/loss conditions to be meaningful.

- [ ] **Boss Spawn Trigger** - When armageddonCounter >= 100
  - [ ] Spawn boss faction with starting city
  - [ ] Create boss legion with demons
  - [ ] Set game phase to endgame

- [ ] **Boss AI Behavior**
  - [ ] Aggressive expansion toward player
  - [ ] Continuous demon spawning each turn
  - [ ] Target player cities directly
  - [ ] No retreat behavior

- [ ] **Victory/Defeat Polish**
  - [ ] Victory screen when boss defeated
  - [ ] Defeat screen when player eliminated
  - [ ] Option to restart game

## Priority 5: Additional AI Factions
Add variety and strategic depth.

- [ ] **Sheaim AI (Ritualist)**
  - [ ] Build ritual sites
  - [ ] Execute rituals that advance armageddon counter
  - [ ] Defensive posture until rituals complete
  - [ ] Summon demons when mana available

- [ ] **Elves AI (Defender)**
  - [ ] Protect home territory (grove)
  - [ ] Minimal expansion
  - [ ] Strong defensive response to intrusion
  - [ ] Passive if left alone

## Priority 6: Economy Polish
Make resource management more interesting.

- [ ] **Pillaging System**
  - [ ] Option to pillage city after capture
  - [ ] Destroy buildings for immediate gold
  - [ ] Reduce city population

- [ ] **Granary Growth Bonus** - Apply growth bonus effect in code

- [ ] **Mana Spending**
  - [ ] Cleric healing ability (costs mana)
  - [ ] Mage special attack (costs mana)
  - [ ] Ritual site mana generation

## Priority 7: Combat Polish
Improve combat experience.

- [ ] **Combat Log** - Show what happened during auto-resolve
  - [ ] List of attacks in speed order
  - [ ] Damage dealt per attack
  - [ ] Casualties on each side

- [ ] **Combat Preview** - Before confirming attack
  - [ ] Show estimated casualties
  - [ ] Show terrain bonuses
  - [ ] Show army comparison

- [ ] **Tactics Settings**
  - [ ] Aggressive (focus damage)
  - [ ] Defensive (protect wounded)
  - [ ] Leader Hunt (target enemy commander)

## Priority 8: Visual Polish
Make the game look better.

- [ ] **Sprite Rendering**
  - [ ] Load sprite assets
  - [ ] Render terrain sprites instead of colored rectangles
  - [ ] Render unit sprites
  - [ ] Render building icons on cities

- [ ] **Animations**
  - [ ] Legion movement animation
  - [ ] Combat shake/flash effects
  - [ ] Turn transition effects

- [ ] **UI Improvements**
  - [ ] Tooltips on hover
  - [ ] Minimap
  - [ ] Better selection highlights
  - [ ] Sound effects

## Priority 9: Advanced Features
Nice-to-have features for deeper gameplay.

- [ ] **Hero Units** - Named characters with special abilities
- [ ] **Additional Soldier Types** - Cavalry, Witch, etc.
- [ ] **Diplomacy** - Bribes, truces, intelligence
- [ ] **Multi-round Sieges** - Extended city battles
- [ ] **Save/Load** - Persist game state

---

## Current Status Summary

| Category | Status |
|----------|--------|
| Core Infrastructure | ✅ Complete |
| Map System | ✅ Complete |
| Soldier/Building Data | ✅ Complete |
| Combat System | 🟡 90% (has bugs) |
| Player Turn System | 🟡 70% (missing UI) |
| AI System | ❌ 0% |
| Endgame/Boss | ❌ 0% |
| Visual Polish | 🟡 50% (no sprites) |

**Estimated effort to playable MVP**: Priorities 1-4 (bug fixes + city UI + one AI + boss)
