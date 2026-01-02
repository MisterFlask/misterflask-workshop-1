// Core type definitions for FFH Simplified

// ============ Coordinates & Map ============

export interface Coord {
  x: number;
  y: number;
}

export type TerrainType = 'grass' | 'forest' | 'hills' | 'mountain' | 'water';

export interface Tile {
  coord: Coord;
  terrain: TerrainType;
  city?: City;
  resource?: ResourceType;
  owner?: FactionId;
}

export type ResourceType = 'iron' | 'gold_deposit' | 'mana_node' | 'horses';

// ============ Factions ============

export type FactionId = 'player' | 'hippus' | 'sheaim' | 'elves' | 'boss';

export type FactionType = 'player' | 'raider' | 'ritualist' | 'defender' | 'boss';

export interface Faction {
  id: FactionId;
  type: FactionType;
  name: string;
  gold: number;
  mana: number;
  state: FactionState;
  color: string;
}

export type FactionState =
  | { type: 'idle' }
  | { type: 'building' }
  | { type: 'raiding'; target: Coord }
  | { type: 'ritual'; turnsRemaining: number }
  | { type: 'defending' }
  | { type: 'attacking'; target: Coord };

// ============ Soldiers & Legions ============

export type SoldierTypeId = 'fighter' | 'knight' | 'archer' | 'mage' | 'cleric' | 'demon';

export type FormationRow = 'front' | 'mid' | 'back';

export interface FormationPosition {
  row: FormationRow;
  column: number; // 0, 1, 2
}

export interface SoldierType {
  id: SoldierTypeId;
  name: string;
  hp: number;
  attack: number;
  defense: number;
  speed: number;
  preferredRow: FormationRow;
  attacksTarget: 'front' | 'back';
  attackCount: Record<FormationRow, number>;
  cost: { gold: number; mana: number };
  sprite: string;
}

export interface Soldier {
  id: string;
  type: SoldierTypeId;
  hp: number;
  maxHp: number;
  position: FormationPosition;
}

export interface Legion {
  id: string;
  owner: FactionId;
  soldiers: Soldier[];
  location: Coord;
  movementRemaining: number;
}

// ============ Cities & Buildings ============

export type BuildingId = 'barracks' | 'market' | 'temple' | 'mage_tower' | 'stables' | 'walls' | 'granary' | 'ritual_site';

export interface BuildingType {
  id: BuildingId;
  name: string;
  cost: number;
  effects: BuildingEffect[];
  sprite: string;
}

export type BuildingEffect =
  | { type: 'unlock_soldier'; soldier: SoldierTypeId }
  | { type: 'gold_per_turn'; amount: number }
  | { type: 'mana_per_turn'; amount: number }
  | { type: 'defense_bonus'; amount: number }
  | { type: 'growth_bonus'; amount: number };

export interface City {
  id: string;
  name: string;
  owner: FactionId;
  coord: Coord;
  population: number;
  buildings: BuildingId[];
  occupationTurns: number; // 0 = normal, >0 = recently captured
}

// ============ Combat ============

export interface CombatResult {
  attackerWon: boolean;
  attackerCasualties: string[]; // soldier IDs
  defenderCasualties: string[];
  attackerDamageDealt: number;
  defenderDamageDealt: number;
}

// ============ Game State ============

export type GamePhase =
  | 'player_turn'
  | 'ai_turn'
  | 'combat_resolution'
  | 'end_turn'
  | 'game_over';

export interface GameState {
  phase: GamePhase;
  turn: number;
  armageddonCounter: number;
  factions: Map<FactionId, Faction>;
  legions: Map<string, Legion>;
  cities: Map<string, City>;
  map: Tile[][];
  mapWidth: number;
  mapHeight: number;
  selectedLegionId: string | null;
  selectedCityId: string | null;
  gameOver: boolean;
  winner: FactionId | null;
}

// ============ Actions ============

export type GameAction =
  | { type: 'select_tile'; coord: Coord }
  | { type: 'move_legion'; legionId: string; to: Coord }
  | { type: 'end_turn' }
  | { type: 'build'; cityId: string; building: BuildingId }
  | { type: 'recruit'; cityId: string; soldierType: SoldierTypeId; legionId: string }
  | { type: 'create_legion'; cityId: string };

// ============ UI State ============

export interface UIState {
  hoveredTile: Coord | null;
  validMoves: Coord[];
  showingCityPanel: boolean;
  showingLegionPanel: boolean;
  camera: {
    x: number;
    y: number;
    zoom: number;
  };
}
