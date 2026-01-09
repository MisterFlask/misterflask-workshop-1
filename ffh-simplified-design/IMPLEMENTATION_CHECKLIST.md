# FFH Simplified - Implementation Checklist

This checklist tracks features from the Master Design Document against what's currently implemented in `projects/ffh-simplified/`.

**Last Updated**: Based on codebase exploration

---

## Legend

- [x] Implemented and working
- [~] Partially implemented / needs work
- [ ] Not implemented

---

## A. CORE SYSTEMS

### Map System
- [x] Square grid map
- [x] Terrain types: Grassland, Forest, Hills, Mountain, Water
- [ ] Terrain type: Swamp (with movement penalty)
- [x] Procedural map generation
- [x] Terrain clustering/smoothing
- [ ] Map size selection (25x25, 35x35, 45x45) - currently fixed 30x30
- [x] Impassable terrain (mountains, water)

### City System
- [x] Pre-placed cities (no founding)
- [x] City capture mechanics
- [x] 3-turn occupation penalty
- [x] Building slots (1-4 based on population)
- [x] Population growth system
- [x] Capital tracking
- [ ] Neutral cities scattered on map
- [x] City income (base + buildings)
- [ ] City razing (+3 Armageddon, 50% building value)

### Building System
- [x] Building definitions with costs and effects
- [x] Building construction queue
- [x] Multi-turn construction times
- [x] Building prerequisites (population-gated tiers)
- [~] Building UI (queue exists but limited player interaction)
- [ ] Mason's Guild construction time reduction
- [x] 8 buildings implemented:
  - [x] Barracks
  - [x] Market
  - [x] Temple
  - [x] Mage Tower
  - [x] Stables
  - [x] Walls
  - [x] Granary
  - [~] Ritual Site (building exists, mechanics missing)

### Buildings NOT Implemented (from design doc)
- [ ] Archery Range
- [ ] War Academy
- [ ] Siege Workshop
- [ ] Trade Hall
- [ ] Fortifications
- [ ] Elite Barracks
- [ ] Arcane Sanctum
- [ ] Cathedral
- [ ] Treasury
- [ ] Grand Walls
- [ ] Legendary Forge
- [ ] Signal Towers
- [ ] All Exploitation Buildings (Mine, Mana Well, Crystal Sanctum, etc.)

---

## B. UNITS & LEGIONS

### Legion System
- [x] Legion creation (100 gold)
- [x] Max 5 legions per faction
- [x] Legion capacity 1-8 soldiers
- [x] Legion movement (3 tiles/turn)
- [x] Formation system (3x3 grid)
- [x] Soldier assignment to legions
- [ ] Legion merging/splitting UI
- [~] Soldier recruitment (actions exist, UI missing)

### Core Units Implemented
- [x] Fighter (HP 100, ATK 20, DEF 15, SPD 50)
- [x] Archer (HP 60, ATK 25, DEF 5, SPD 60)
- [x] Knight (HP 150, ATK 30, DEF 20, SPD 30)
- [x] Mage (HP 50, ATK 35, DEF 5, SPD 40)
- [x] Cleric (HP 70, ATK 10, DEF 10, SPD 45) - heals allies
- [x] Demon (HP 120, ATK 35, DEF 15, SPD 55) - boss only

### Core Units NOT Implemented
- [ ] Cavalry (requires Stables + Horses)
- [ ] Catapult / Siege units
- [ ] Paladin
- [ ] Champion

### Terrain-Gated Exclusive Units (ALL MISSING)
- [ ] Fire Elemental (Mana Spring)
- [ ] Ice Elemental (Mana Spring)
- [ ] Storm Elemental (Mana Spring)
- [ ] Sage (Ancient Ruins)
- [ ] Archmage (Crystal Cave)
- [ ] Lich (Haunted Barrow)
- [ ] Dragon Knight (Dragon Bones)
- [ ] Siege Titan (Adamantine Vein)
- [ ] Treant (World Tree)
- [ ] Summoned Demon (Hellgate)
- [ ] Golem (Titan's Grave)
- [ ] Titan (Titan's Grave)

### Faction-Specific Units (ALL MISSING)
- [ ] Hippus: Horse Archer, Outrider
- [ ] Sheaim: Cultist, Hellhound, Pit Fiend
- [ ] Ljosalfar: Wardancer, Arcane Archer, Treant
- [ ] Calabim: Thrall, Vampire, Vampire Lord
- [ ] Clan of Embers: Orc Warrior, Orc Berserker, Warboss
- [ ] Infernal: Imp, Demon, Balor

---

## C. COMBAT SYSTEM

### Core Combat
- [x] OB64-style timeline combat
- [x] Speed-based attack ordering
- [x] Attack count by row (Front: 3, Mid: 2, Back: 1)
- [x] Damage calculation (ATK vs DEF)
- [x] Targeting priorities (front/back)
- [x] Combat events logging
- [x] Winner determination
- [x] Retreat mechanics

### Combat Features
- [x] Terrain defense bonuses (Forest +10%, Hills +15%, City +25%)
- [x] Walls defense bonus (+40%)
- [x] Cleric healing in combat
- [ ] Row damage multipliers (Back row ×2.0)
- [ ] Siege unit mechanics (building damage)
- [ ] Sally mechanic (garrison exits to attack siegers)

---

## D. ECONOMY & RESOURCES

### Gold System
- [x] Base city income
- [x] Building income bonuses
- [x] Technology income bonuses
- [ ] Resource income bonuses (Iron, Gold Deposit)
- [ ] Pillaging (50% building value)
- [ ] Trade Hall bonuses
- [ ] Treasury percentage bonus

### Mana System
- [x] Mana generation from Mage Tower
- [x] Mana generation from terrain features (mana nodes)
- [~] Mana as capacity (upkeep system) - partially implemented
- [ ] Mana from exploitation buildings
- [ ] Unit mana upkeep enforcement

### Strategic Resources (ALL MISSING)
- [ ] Iron Vein (+5 ATK melee)
- [ ] Horses (required for Cavalry)
- [ ] Gold Deposit (+15 gold/turn)
- [ ] Mana Crystals
- [ ] Sacred Grove

---

## E. TERRAIN IMPROVEMENTS (Section C7)

### Terrain Feature System
- [x] Features placed on map during generation
- [x] Feature definitions with effects
- [ ] Feature effects applied to nearby cities
- [ ] 3-tile control radius enforcement

### Common Improvements
- [~] Iron Vein - on map, effects not functional
- [ ] Horses - on map, cavalry gating not functional
- [~] Gold Deposit - on map, effects not functional
- [~] Mana Node - on map, effects partially work

### Uncommon Improvements
- [~] Mana Spring - on map, exploitation not implemented
- [~] Ancient Ruins - on map, effects not implemented
- [ ] Mineral Deposit

### Rare Improvements
- [~] Crystal Cave - on map, Archmage gating not implemented
- [~] Haunted Barrow - on map, Lich gating not implemented
- [~] Dragon Bones - on map, Dragon Knight gating not implemented
- [ ] Adamantine Vein

### Legendary Improvements
- [ ] World Tree
- [ ] Hellgate
- [ ] Titan's Grave

### Exploitation Buildings (ALL MISSING)
- [ ] Mine (Iron/Gold)
- [ ] Mana Well
- [ ] Archive
- [ ] Deep Mine
- [ ] Crystal Sanctum
- [ ] Necropolis
- [ ] Dragon Shrine
- [ ] Master Forge
- [ ] Grove of Ages
- [ ] Binding Circle
- [ ] Titan Forge

---

## F. MAGIC SYSTEM (Section C3)

### Magic Schools (ALL MISSING)
- [ ] Fire school (Tier 1-3)
- [ ] Water school (Tier 1-3)
- [ ] Earth school (Tier 1-3)
- [ ] Air school (Tier 1-3)
- [ ] Life school (Tier 1-3)
- [ ] Death school (Tier 1-3)

### Spell System
- [ ] Spell definitions
- [ ] Spell costs (mana + cooldown)
- [ ] Combat-time spells
- [ ] Strategic-time spells
- [ ] Spell casting UI
- [ ] Counter matrix implementation
- [ ] Death magic Armageddon penalty (+1 per cast)

---

## G. HERO SYSTEM (Section B9)

- [ ] Hero acquisition (from buildings/events)
- [ ] Hero stats and abilities
- [ ] Max 3 heroes active
- [ ] 1 hero per legion limit
- [ ] Permadeath mechanic
- [ ] 20-turn respawn timer

### Heroes (ALL MISSING)
- [ ] Warlord (from Elite Barracks)
- [ ] Archmage Hero (from Arcane Sanctum)
- [ ] High Priest (from Cathedral)
- [ ] Champion (from World Boss)
- [ ] Mercenary Captain (from Hippus diplomacy)

---

## H. AI SYSTEM (Section E)

### AI Framework
- [x] AI turn processing
- [x] Basic pathfinding (A*)
- [x] Target selection
- [x] Combat initiation
- [x] City capture AI

### AI Factions Implemented
- [x] Hippus (Raiders) - basic behavior
- [x] Sheaim (Ritualists) - basic behavior, no rituals
- [x] Infernal (Boss) - spawns at Armageddon 50
- [~] Ljosalfar (Elves) - defined but not active
- [ ] Calabim (Vampires) - not implemented
- [ ] Clan of Embers (Orcs) - not implemented

### AI Behaviors (MOSTLY MISSING)
- [ ] Hippus raiding behavior (raid → heal → reposition)
- [ ] Sheaim ritual behavior (build altar → ritual → summon)
- [ ] Ljosalfar defensive behavior (patrol groves → defend → vengeance)
- [ ] Calabim drain behavior (feed → drain → expand)
- [ ] Orc aggression behavior (muster → attack → regroup)
- [ ] Infernal endless spawn behavior

### AI Economics
- [ ] AI faction income scaling
- [ ] AI unit spawning on timer
- [ ] AI building priorities
- [ ] AI-vs-AI casualty mechanics (pyrrhic victories)

---

## I. DIPLOMACY SYSTEM (Section B6)

- [ ] Faction dispositions (Hostile/Wary/Neutral/Friendly)
- [ ] Redirect Aggression action
- [ ] Temporary Truce action
- [ ] Request Aid action
- [ ] Trade Access action
- [ ] Diplomacy UI panel
- [ ] Disposition modifiers
- [ ] Faction-specific diplomacy options

---

## J. ARMAGEDDON & VICTORY (Section B7)

### Armageddon Counter
- [x] Counter increments +1/turn
- [x] Boss spawns at threshold (currently 50, design says 80)
- [ ] Counter target should be 80 (not 100)
- [ ] Minimum rate +0.5/turn regardless of deceleration
- [ ] Acceleration sources:
  - [ ] Sheaim ritual complete (+8)
  - [ ] City razed (+2)
  - [ ] World Boss killed (+5)
  - [ ] Skeletal Crypt cleared (+3)
  - [ ] Death magic spell cast (+1)
- [ ] Deceleration sources:
  - [ ] Sacred Grove (-0.5/turn per grove)
  - [ ] Cathedral (-0.5/turn, max 2)
  - [ ] Sealing Ritual (-15 one-time)

### Victory/Loss
- [x] Win: Capture Infernal Capital
- [x] Loss: Lose all cities
- [ ] Loss: All legions destroyed AND can't afford new one
- [ ] Proper boss scaling (income 50→75→100 over time)

---

## K. NEUTRAL THREATS (Section C8)

### Monster Lairs (ALL MISSING)
- [ ] Goblin Cave (spawns goblins)
- [ ] Wolf Den (spawns dire wolves)
- [ ] Bandit Camp (spawns brigands)
- [ ] Skeletal Crypt (spawns skeletons, +10 Armageddon on clear)
- [ ] Dragon Lair (static young dragon)

### World Bosses (ALL MISSING)
- [ ] Frost Giant King
- [ ] Ancient Red Dragon
- [ ] Lich Lord (+5 Armageddon on kill)

---

## L. UI & POLISH

### Implemented
- [x] Canvas tile rendering
- [x] Camera pan
- [x] Tile selection
- [x] Legion selection
- [x] City panel (basic)
- [x] Legion panel
- [x] Collegia/tech panel
- [x] Knowledge panel
- [x] End turn button
- [x] Tooltips (tiles, units)

### Missing/Needed
- [ ] Building construction UI (player-facing)
- [ ] Soldier recruitment UI
- [ ] Diplomacy panel
- [ ] Spell casting UI
- [ ] Hero management UI
- [ ] Formation editing UI (drag-drop)
- [ ] Combat visualization (animations)
- [ ] Minimap
- [ ] Victory/defeat screens
- [ ] Save/load game
- [ ] Settings/options menu
- [ ] Tutorial/onboarding

---

## M. STARTING CONDITIONS (Section B8)

### Player Start
- [x] 200 gold
- [x] 0 mana
- [x] 2 cities
- [x] 1 legion
- [~] Starting buildings (Barracks, Market, Walls) - may need verification
- [~] Starting soldiers (2 Fighters, 1 Archer, 1 Cleric) - may need verification

### AI Starting Conditions
- [~] AI factions start with preset units - partially implemented
- [ ] Correct starting cities per faction
- [ ] Correct starting legions per faction
- [ ] Faction-specific starting units

---

## PRIORITY SUMMARY

### Critical (Blocks Core Loop)
1. [ ] Building construction UI for players
2. [ ] Soldier recruitment UI
3. [ ] Terrain improvement effects on cities
4. [ ] Fix Armageddon target (80 not 100)

### High (Major Features)
5. [ ] Exploitation buildings and terrain-gated units
6. [ ] Magic/spell system (6 schools)
7. [ ] Hero system
8. [ ] AI state machines per faction
9. [ ] Diplomacy system
10. [ ] Neutral cities on map

### Medium (Systems)
11. [ ] Monster lairs and world bosses
12. [ ] Siege mechanics
13. [ ] All missing buildings
14. [ ] All missing units (faction-specific)
15. [ ] Swamp terrain

### Low (Polish)
16. [ ] Save/load
17. [ ] Combat animations
18. [ ] Map size selection
19. [ ] Tutorial
20. [ ] Sound/music

---

## File References

- **Game Logic**: `projects/ffh-simplified/src/game/Game.ts`
- **Combat**: `projects/ffh-simplified/src/game/Combat.ts`
- **Types**: `projects/ffh-simplified/src/types.ts`
- **Data**: `projects/ffh-simplified/src/data/*.ts`
- **Tests**: `projects/ffh-simplified/src/game/Game.test.ts`
- **Design Doc**: `ffh-simplified-design/FFH_SIMPLIFIED_MASTER_DESIGN.md`
