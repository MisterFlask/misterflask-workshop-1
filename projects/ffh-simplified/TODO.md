# FFH Simplified - Implementation Todo List

## Priority 1: Critical Bug Fixes
These must be fixed for the game to function correctly.

- [x] **Fix terrain defense bonus bug** - Combat.ts:208 correctly applies terrain bonus only when defender is being attacked
- [x] **Fix combat HP persistence** - Combat result HP changes now propagated back to original soldiers via two-phase combat system
- [x] **Implement retreat logic** - Defeated legions with survivors retreat to adjacent tile away from attacker; destroyed only if cornered

## Priority 2: Core Gameplay Loop
These are required for a minimally playable game.

- [x] **City Management UI** - Panel for building construction and soldier recruitment
  - [x] Show available buildings based on population slots
  - [x] Show building costs and turn times
  - [x] Build queue with progress tracking
  - [x] Cancel queued items with partial refund
  - [x] Show recruitable soldiers based on buildings (when legion present)
  - [x] Recruit soldiers to legion stationed at city

- [x] **Legion Management UI** - Panel for managing legion composition
  - [x] Show soldiers in formation grid (3x3)
  - [x] Allow repositioning soldiers between rows/columns (Edit Formation mode)
  - [x] Show legion stats summary
  - [x] Unit tooltip with full stats on hover

- [x] **Movement validation improvements**
  - [x] Prevent moving through enemy legions
  - [x] Show movement path preview
  - [x] Validate movement points remaining (shows cost in tooltip)

## Priority 3: AI System (Single Faction First)
Implement one AI faction to make the game playable.

- [x] **AI Turn Execution Framework** - Game.ts processAITurn()
  - [x] Loop through AI factions
  - [x] Execute behavior based on faction state
  - [x] Transition states based on conditions

- [x] **Hippus AI (Raider)** - Simplest behavior pattern
  - [x] Idle → Raiding when troops >= 3
  - [x] Target selection: nearest player legion/city
  - [x] Pathfinding to target
  - [x] Combat initiation
  - [x] Retreat when legion HP < 30%
  - [x] Return to home city to heal

- [x] **AI Legion Movement** - Pathfinding implementation
  - [x] A* pathfinding (using existing findPath)
  - [x] Avoid impassable terrain (mountains, water)
  - [x] Move toward target tile

- [x] **AI Recruitment** - Basic army building
  - [x] Recruit soldiers when gold available (50+ gold threshold)
  - [x] Assign to legion at home city

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
| Combat System | ✅ Complete |
| Player Turn System | ✅ Complete |
| AI System | ✅ Complete (basic) |
| Endgame/Boss | ❌ 0% |
| Visual Polish | 🟡 50% (no sprites) |

**Estimated effort to playable MVP**: Priority 4 (boss/endgame)
