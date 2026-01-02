# Technical Design Document

## Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | TypeScript | Type safety, good tooling, compiles to JS |
| Rendering | HTML5 Canvas | Simple, no dependencies, sufficient for 2D grid |
| Build | Vite | Fast dev server, simple config, good TS support |
| State Management | Plain classes | No framework overhead; game state is straightforward |
| Testing | Vitest | Matches Vite, fast, good TS support |
| Art Pipeline | Python + Pillow | Generate PNGs from text-based sprite definitions |

**No frameworks**: React/Vue/etc. add complexity without benefit for a game loop. We'll use vanilla DOM for UI overlays (menus, tooltips) and Canvas for the game world.

---

## Directory Structure

```
ffh-simplified/
├── src/
│   ├── main.ts                 # Entry point, game loop
│   ├── game/
│   │   ├── Game.ts             # Main game state container
│   │   ├── Map.ts              # Square grid, terrain, cities
│   │   ├── Legion.ts           # Legion and soldier management
│   │   ├── Combat.ts           # OB64-style combat resolution
│   │   ├── Faction.ts          # AI faction state machines
│   │   ├── City.ts             # City, buildings, production
│   │   └── ArmageddonCounter.ts
│   ├── rendering/
│   │   ├── Renderer.ts         # Main canvas rendering
│   │   ├── GridRenderer.ts     # Square grid drawing
│   │   ├── SpriteSheet.ts      # Sprite loading and drawing
│   │   └── UI.ts               # DOM-based UI overlays
│   ├── input/
│   │   ├── InputHandler.ts     # Mouse/keyboard events
│   │   └── Camera.ts           # Pan/zoom
│   ├── data/
│   │   ├── soldiers.ts         # Soldier type definitions
│   │   ├── buildings.ts        # Building definitions
│   │   └── factions.ts         # Faction behavior definitions
│   └── utils/
│       ├── grid.ts             # Grid math utilities
│       └── random.ts           # Seeded RNG
├── assets/
│   ├── sprites/                # Generated PNGs
│   │   ├── soldiers/
│   │   ├── terrain/
│   │   └── ui/
│   └── sprite_definitions/     # Text-based sprite sources
├── tools/
│   └── generate_sprites.py     # Sprite generation script
├── index.html
├── package.json
├── tsconfig.json
└── vite.config.ts
```

---

## Art Pipeline

### Sprite Definition Format

Sprites defined in human-readable text files:

```
# fighter.sprite
name: fighter
size: 32x32
palette:
  .: transparent
  x: #000000
  s: #8B4513
  m: #C0C0C0
  f: #FFD700

pixels: |
  ................................
  ................................
  ..........xxxxxx..............
  .........xffffffx.............
  .........xffffffx.............
  ..........xxxxxx..............
  ...........xmmx...............
  ..........xmmmmx..............
  .........xmmmmmmx.............
  .........xmmmmmmx.............
  ..........xmmmmx..............
  ...........xssx...............
  ..........xssssxx.............
  .........xsssssssx............
  ................................
  (etc)
```

### Generation Script

`tools/generate_sprites.py`:
- Reads `.sprite` files from `assets/sprite_definitions/`
- Generates PNGs to `assets/sprites/`
- Run manually or as build step
- Uses Pillow for PNG output

### Swapping Art Later

To replace placeholder art:
1. Drop new 32x32 PNG in `assets/sprites/`
2. Keep same filename
3. Done - no code changes needed

---

## Core Architecture

### Game Loop

```typescript
class Game {
  private state: GameState;
  private renderer: Renderer;
  private inputHandler: InputHandler;

  start() {
    this.gameLoop();
  }

  private gameLoop() {
    this.handleInput();
    this.update();
    this.render();
    requestAnimationFrame(() => this.gameLoop());
  }

  private update() {
    if (this.state.phase === 'player_turn') {
      // Wait for player input
    } else if (this.state.phase === 'ai_turn') {
      this.processAITurn();
    } else if (this.state.phase === 'combat') {
      this.resolveCombat();
    }
  }
}
```

### Turn Structure

```typescript
type GamePhase =
  | 'player_turn'      // Player moves legions, builds, recruits
  | 'ai_turn'          // Each AI faction executes behavior
  | 'combat'           // Resolve any pending combats
  | 'end_turn'         // Advance counters, check victory
  | 'endgame';         // Boss faction active

interface GameState {
  phase: GamePhase;
  turn: number;
  armageddonCounter: number;
  player: PlayerState;
  factions: Faction[];
  map: GridMap;
  pendingCombats: Combat[];
}
```

### Square Grid

Using simple (x, y) coordinates:

```typescript
interface Coord {
  x: number;
  y: number;
}

class GridMap {
  private tiles: Tile[][];  // 2D array, tiles[y][x]
  readonly width: number;
  readonly height: number;

  get(coord: Coord): Tile | undefined {
    if (coord.x < 0 || coord.x >= this.width) return undefined;
    if (coord.y < 0 || coord.y >= this.height) return undefined;
    return this.tiles[coord.y][coord.x];
  }

  neighbors(coord: Coord): Coord[] {
    // Return 4 adjacent tiles (no diagonals for movement simplicity)
    const dirs = [{x:0,y:-1}, {x:1,y:0}, {x:0,y:1}, {x:-1,y:0}];
    return dirs
      .map(d => ({ x: coord.x + d.x, y: coord.y + d.y }))
      .filter(c => this.get(c) !== undefined);
  }

  distance(a: Coord, b: Coord): number {
    // Manhattan distance
    return Math.abs(a.x - b.x) + Math.abs(a.y - b.y);
  }

  pathfind(from: Coord, to: Coord): Coord[] {
    // A* pathfinding
  }
}

interface Tile {
  coord: Coord;
  terrain: TerrainType;
  city?: City;
  legion?: Legion;
  owner?: FactionId;
}
```

### Legion & Combat

```typescript
interface Soldier {
  type: SoldierType;
  hp: number;
  maxHp: number;
  position: FormationPosition; // 0-8 for 3x3 grid
}

interface Legion {
  id: string;
  owner: FactionId;
  soldiers: Soldier[];
  location: Coord;
  canMove: boolean;
}

interface FormationPosition {
  row: 'front' | 'mid' | 'back';  // 0, 1, 2
  column: number;                  // 0, 1, 2
}

class CombatResolver {
  resolve(attacker: Legion, defender: Legion, terrain: TerrainType): CombatResult {
    // 1. Build attack timeline based on speed
    // 2. Execute attacks in order
    // 3. Remove dead soldiers
    // 4. Tally damage
    // 5. Determine winner/loser
    // 6. Return result (who retreats, casualties)
  }
}
```

### AI Faction State Machine

```typescript
interface Faction {
  id: FactionId;
  type: FactionType;
  state: FactionState;
  cities: City[];
  legions: Legion[];
}

type FactionState =
  | { type: 'building' }
  | { type: 'raiding', target: Coord }
  | { type: 'ritual', turnsRemaining: number }
  | { type: 'defending' }
  | { type: 'expanding', direction: Coord };

class FactionAI {
  // Each faction type has its own AI class
  abstract evaluateStateTransition(faction: Faction, gameState: GameState): FactionState;
  abstract executeState(faction: Faction, gameState: GameState): Action[];
}

class SheaimAI extends FactionAI {
  evaluateStateTransition(faction, gameState) {
    // If no ritual in progress and have ritual building, start ritual
    // If threatened, switch to defending
    // etc.
  }

  executeState(faction, gameState) {
    if (faction.state.type === 'ritual') {
      // Advance ritual, maybe spawn demons
    } else if (faction.state.type === 'building') {
      // Build ritual buildings
    }
  }
}
```

---

## AI Turn Execution (Detailed)

### Turn Phase Flow

```
Player Turn
    ↓
Player clicks "End Turn"
    ↓
┌─────────────────────────────────────┐
│  AI Turn Phase                      │
│  For each AI faction (in order):    │
│    1. Evaluate state transition     │
│    2. Execute faction actions       │
│    3. Move each legion              │
│    4. Resolve any combats           │
│  End for                            │
└─────────────────────────────────────┘
    ↓
End Turn Phase (income, growth, counter)
    ↓
Next Player Turn
```

### AI Faction Turn Steps

Each AI faction executes these steps in order:

```typescript
class AITurnProcessor {
  processFactionTurn(faction: Faction, gameState: GameState): void {
    const ai = this.getAIForFaction(faction.type);

    // Step 1: Decide what state the faction should be in
    const newState = ai.evaluateStateTransition(faction, gameState);
    faction.state = newState;

    // Step 2: Execute faction-level actions (building, rituals, spawning)
    const factionActions = ai.executeFactionActions(faction, gameState);
    this.applyActions(factionActions, gameState);

    // Step 3: Move each legion according to current state
    for (const legion of faction.legions) {
      const movement = ai.decideLegionMovement(legion, faction.state, gameState);
      this.moveLegion(legion, movement, gameState);
    }

    // Step 4: Resolve combats (handled by main game loop after all moves)
  }
}
```

### State Transition Logic (Per Faction Type)

#### Hippus (Raiders)

```typescript
class HippusAI extends FactionAI {
  evaluateStateTransition(faction: Faction, gameState: GameState): FactionState {
    // Priority 1: If any legion is weak (< 50% soldiers), go home to heal
    if (this.hasWeakLegion(faction)) {
      return { type: 'defending' };
    }

    // Priority 2: If currently raiding and target is destroyed/captured, pick new target
    if (faction.state.type === 'raiding') {
      const target = gameState.map.get(faction.state.target);
      if (!target?.city || target.city.owner === faction.id) {
        return { type: 'raiding', target: this.pickRaidTarget(faction, gameState) };
      }
      return faction.state; // Continue current raid
    }

    // Priority 3: Default to raiding
    return { type: 'raiding', target: this.pickRaidTarget(faction, gameState) };
  }

  pickRaidTarget(faction: Faction, gameState: GameState): Coord {
    // Find nearest enemy city that isn't heavily defended
    const enemyCities = this.findEnemyCities(faction, gameState);
    const scored = enemyCities.map(city => ({
      city,
      score: this.scoreRaidTarget(city, faction, gameState)
    }));
    scored.sort((a, b) => b.score - a.score);
    return scored[0]?.city.coord ?? faction.legions[0].location;
  }

  scoreRaidTarget(city: City, faction: Faction, gameState: GameState): number {
    const distance = this.distanceToNearestLegion(city.coord, faction);
    const wealth = city.buildings.length * 10; // More buildings = juicier target
    const defense = this.estimateDefense(city, gameState);

    // Prefer close, wealthy, poorly defended cities
    return wealth - distance * 2 - defense * 3;
  }

  decideLegionMovement(
    legion: Legion,
    state: FactionState,
    gameState: GameState
  ): Coord | null {
    if (state.type === 'raiding') {
      // Move toward raid target
      const path = gameState.map.pathfind(legion.location, state.target);
      return path[1] ?? null; // Next step in path
    }

    if (state.type === 'defending') {
      // Move toward nearest friendly city
      const homeCity = this.nearestFriendlyCity(legion, gameState);
      if (this.distance(legion.location, homeCity.coord) > 0) {
        const path = gameState.map.pathfind(legion.location, homeCity.coord);
        return path[1] ?? null;
      }
      return null; // Stay put and heal
    }

    return null;
  }
}
```

#### Sheaim (Apocalypse Cultists)

```typescript
class SheaimAI extends FactionAI {
  evaluateStateTransition(faction: Faction, gameState: GameState): FactionState {
    // Priority 1: If under attack, defend
    if (this.citiesUnderThreat(faction, gameState).length > 0) {
      return { type: 'defending' };
    }

    // Priority 2: If ritual in progress, continue
    if (faction.state.type === 'ritual' && faction.state.turnsRemaining > 0) {
      return {
        type: 'ritual',
        turnsRemaining: faction.state.turnsRemaining - 1
      };
    }

    // Priority 3: If have ritual building and not in ritual, start one
    if (this.hasRitualBuilding(faction) && !this.ritualOnCooldown(faction)) {
      return { type: 'ritual', turnsRemaining: 5 }; // 5 turn ritual
    }

    // Priority 4: Build up
    return { type: 'building' };
  }

  executeFactionActions(faction: Faction, gameState: GameState): Action[] {
    const actions: Action[] = [];

    if (faction.state.type === 'ritual') {
      // Ritual is progressing - no action needed, just wait
      if (faction.state.turnsRemaining === 0) {
        // Ritual complete!
        actions.push({
          type: 'advance_armageddon',
          amount: 10
        });
        actions.push({
          type: 'spawn_legion',
          location: this.getRitualCity(faction).coord,
          composition: this.getDemonLegionComposition()
        });
      }
    }

    if (faction.state.type === 'building') {
      // Try to build ritual building if we don't have one
      const cityNeedingBuilding = this.cityWithoutRitualBuilding(faction);
      if (cityNeedingBuilding && faction.gold >= 200) {
        actions.push({
          type: 'build',
          city: cityNeedingBuilding.id,
          building: 'ritual_site'
        });
      }
    }

    return actions;
  }

  decideLegionMovement(
    legion: Legion,
    state: FactionState,
    gameState: GameState
  ): Coord | null {
    if (state.type === 'defending') {
      // Move to defend threatened city
      const threatened = this.citiesUnderThreat(faction, gameState);
      if (threatened.length > 0) {
        const target = threatened[0].coord;
        const path = gameState.map.pathfind(legion.location, target);
        return path[1] ?? null;
      }
    }

    // Otherwise, patrol near our cities
    return this.patrolNearCity(legion, faction, gameState);
  }
}
```

### Combat During AI Turn

When an AI legion moves into a tile with an enemy legion:

```typescript
moveLegion(legion: Legion, destination: Coord, gameState: GameState): void {
  const destTile = gameState.map.get(destination);

  if (destTile?.legion && destTile.legion.owner !== legion.owner) {
    // Combat! Resolve immediately
    const result = this.combatResolver.resolve(
      legion,           // Attacker
      destTile.legion,  // Defender
      destTile.terrain
    );

    // Apply casualties
    this.applyCasualties(legion, result.attackerCasualties);
    this.applyCasualties(destTile.legion, result.defenderCasualties);

    // Loser retreats
    if (result.attackerWon) {
      this.retreatLegion(destTile.legion, destination, gameState);
      legion.location = destination; // Attacker takes the tile
    } else {
      this.retreatLegion(legion, legion.location, gameState);
      // Attacker stays where they were (or retreats further)
    }
  } else if (!destTile?.legion) {
    // No combat, just move
    legion.location = destination;
  }
  // If friendly legion there, can't move (skip)
}

retreatLegion(legion: Legion, fromCoord: Coord, gameState: GameState): void {
  // Find empty adjacent tile away from the enemy
  const neighbors = gameState.map.neighbors(fromCoord);
  const emptyNeighbors = neighbors.filter(c => !gameState.map.get(c)?.legion);

  if (emptyNeighbors.length > 0) {
    // Pick one (prefer toward friendly territory)
    legion.location = this.pickRetreatTile(emptyNeighbors, legion, gameState);
  } else {
    // No retreat possible - legion is destroyed? Or pushed further?
    // For simplicity: legion is destroyed
    this.destroyLegion(legion, gameState);
  }
}
```

### End Turn Processing

After all factions have acted:

```typescript
processEndTurn(gameState: GameState): void {
  // 1. Income
  for (const faction of [gameState.player, ...gameState.factions]) {
    faction.gold += this.calculateIncome(faction, gameState);
    faction.mana += this.calculateManaIncome(faction, gameState);
  }

  // 2. Population growth
  for (const city of this.getAllCities(gameState)) {
    city.population += city.growthRate;
    city.buildingSlots = this.slotsForPopulation(city.population);
  }

  // 3. Healing (garrisoned legions)
  for (const legion of this.getAllLegions(gameState)) {
    const tile = gameState.map.get(legion.location);
    if (tile?.city && tile.city.owner === legion.owner) {
      this.healLegion(legion);
    }
  }

  // 4. Advance Armageddon Counter
  gameState.armageddonCounter += 1; // Base increment per turn
  // (Faction actions may have added more during their turns)

  // 5. Check for endgame trigger
  if (gameState.armageddonCounter >= 100) {
    this.triggerEndgame(gameState);
  }

  // 6. Advance turn
  gameState.turn += 1;
}
```

---

## Implementation Plan

### Phase 0: Project Setup (Foundation)
- [ ] Initialize Vite + TypeScript project
- [ ] Set up directory structure
- [ ] Create sprite generation script
- [ ] Generate placeholder sprites (handful of soldiers, terrain, UI icons)
- [ ] Basic HTML canvas rendering test

### Phase 1: Grid Map & Rendering
- [ ] Implement grid coordinate system (x, y)
- [ ] GridMap class with get/set/neighbors
- [ ] Procedural map generation (simple: scatter terrain types)
- [ ] Render square grid to canvas
- [ ] Camera pan with mouse drag
- [ ] Click-to-select tile

### Phase 2: Legions & Movement
- [ ] Legion data structure
- [ ] Soldier types (Fighter, Archer, Mage, Cleric)
- [ ] Render legions on map
- [ ] Player legion selection
- [ ] Movement range display
- [ ] Move legion to tile
- [ ] Turn structure (player moves, then "end turn")

### Phase 3: Combat System
- [ ] Combat resolver (OB64-style)
- [ ] Attack timeline based on speed
- [ ] Damage calculation
- [ ] Retreat mechanics
- [ ] Combat initiation (move into enemy tile)
- [ ] Combat result display (simple: show casualties)
- [ ] Post-combat retreat positioning

### Phase 4: Cities & Economy
- [ ] City data structure
- [ ] Building slots based on population
- [ ] Building types (Barracks, Market, Temple, Mage Tower)
- [ ] Gold income per turn
- [ ] Soldier recruitment from cities
- [ ] Population growth (automatic)
- [ ] Mana as global resource

### Phase 5: Single AI Faction
- [ ] Faction state machine framework
- [ ] Implement Hippus (raider) as first AI
  - Behavior: roam, raid nearby cities, retreat when losing
- [ ] AI turn execution
- [ ] AI legion movement and combat

### Phase 6: Armageddon Counter & Endgame
- [ ] Counter UI display
- [ ] Counter advancement (per turn, per faction action)
- [ ] Endgame boss spawn when counter fills
- [ ] Boss faction AI (aggressive, attacks all)
- [ ] Victory condition (take boss capital)
- [ ] Game over screen

### Phase 7: Polish & Iterate
- [ ] Add more AI factions (Sheaim, Elves)
- [ ] Add remaining soldier types
- [ ] Terrain defense bonuses
- [ ] Building prerequisites (tech tree via buildings)
- [ ] Pillaging mechanics
- [ ] Hero units
- [ ] Better UI (tooltips, info panels)
- [ ] Sound effects (optional)

---

## Prototype Scope (MVP)

For a playable prototype, target Phases 0-6:

**Included**:
- Square grid map with terrain
- Player controls 1-3 legions
- 4 soldier types
- Basic combat resolution
- 2-3 cities with buildings
- Gold + recruitment
- 1 AI faction (Hippus raiders)
- Armageddon counter + boss endgame

**Deferred**:
- Multiple AI factions
- Diplomacy
- Heroes with abilities
- Mana/magic system
- Building prerequisites
- Pillaging
- Terrain modifiers beyond cities

---

## Data-Driven Design

Game content defined in TypeScript data files for easy tweaking:

```typescript
// src/data/soldiers.ts
export const SOLDIER_TYPES: Record<SoldierTypeId, SoldierType> = {
  fighter: {
    name: 'Fighter',
    hp: 100,
    attack: 20,
    defense: 10,
    speed: 50,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 3, mid: 2, back: 1 },
    cost: { gold: 50 },
    sprite: 'soldiers/fighter.png',
  },
  archer: {
    name: 'Archer',
    hp: 60,
    attack: 25,
    defense: 5,
    speed: 60,
    preferredRow: 'back',
    attacksTarget: 'back',
    attackCount: { front: 1, mid: 2, back: 2 },
    cost: { gold: 60 },
    sprite: 'soldiers/archer.png',
  },
  // ...
};

// src/data/buildings.ts
export const BUILDING_TYPES: Record<BuildingId, BuildingType> = {
  barracks: {
    name: 'Barracks',
    cost: 100,
    effects: [{ type: 'unlock_soldier', soldier: 'fighter' }],
    sprite: 'buildings/barracks.png',
  },
  market: {
    name: 'Market',
    cost: 80,
    effects: [{ type: 'gold_per_turn', amount: 10 }],
    sprite: 'buildings/market.png',
  },
  // ...
};
```

---

## Open Technical Questions

These can be resolved during implementation:

1. **Save/Load**: LocalStorage? Export to JSON file? (Defer to Phase 7)
2. **Map Size**: How many tiles? Start with 30x30, adjust based on performance
3. **Animation**: Animate combat? Or just show results? (Start with instant results)
4. **Multiplayer**: Out of scope for prototype; architecture doesn't preclude it
5. **Mobile**: Touch support? (Defer; mouse-first for prototype)
