import type {
  GameState,
  GameAction,
  GamePhase,
  Coord,
  Tile,
  Legion,
  City,
  Faction,
  FactionId,
  Soldier,
  SoldierTypeId,
  BuildingId,
  BuildQueueItem,
  RosterSoldier,
  CollegiaState,
  FormationRow,
} from '../types';
import { SOLDIER_TYPES } from '../data/soldiers';
import { BUILDING_TYPES, getBuildingSlots, BASE_CITY_INCOME, TURNS_PER_POPULATION } from '../data/buildings';
import { FACTION_TEMPLATES, createFaction } from '../data/factions';
import { TECHNOLOGIES, getAllTechnologyIds } from '../data/technologies';
import { TERRAIN_FEATURES } from '../data/terrainFeatures';
import { generateMap, placeStartingEntities, updateCulturalBorders } from './MapGenerator';
import { resolveCombat, applyCombatResult, SoldierTechBonuses } from './Combat';
import { coordToKey, coordsEqual, getTilesInRange, findPath, getPathCost, getNeighbors, manhattanDistance } from '../utils/grid';
import { generateId, generateSoldierName } from '../utils/random';

const BASE_MOVEMENT_RANGE = 3;
const HEAL_RATE = 20;

// Get movement range with technology bonus
function getMovementRange(state: GameState, factionId: FactionId): number {
  if (factionId !== 'player') return BASE_MOVEMENT_RANGE;

  let bonus = 0;
  for (const techId of state.collegia.ownedTechnologies) {
    const tech = TECHNOLOGIES[techId];
    if (!tech) continue;

    for (const effect of tech.effects) {
      if (effect.type === 'legion_movement_bonus') {
        bonus += effect.amount;
      }
    }
  }

  return BASE_MOVEMENT_RANGE + bonus;
}
const LEGION_COST = 100;
const MAX_LEGIONS_PER_FACTION = 5;
const MAX_SOLDIERS_PER_LEGION = 8;
const ARMAGEDDON_MAX = 80; // Per design doc - Armageddon triggers at 80
const BOSS_SPAWN_THRESHOLD = 40; // Boss spawns at 50% Armageddon (40 out of 80)
const COLLEGIA_OFFERINGS_COUNT = 5;
const COLLEGIA_REROLL_COST = 100;

// Generate random Collegia offerings (excluding already owned techs)
function generateCollegiaOfferings(ownedTechnologies: string[]): string[] {
  const allTechs = getAllTechnologyIds();
  const availableTechs = allTechs.filter(id => !ownedTechnologies.includes(id));

  // Shuffle and take the first N
  const shuffled = [...availableTechs];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
  }

  return shuffled.slice(0, Math.min(COLLEGIA_OFFERINGS_COUNT, shuffled.length));
}

// Create initial Collegia state
function createInitialCollegia(): CollegiaState {
  return {
    currentResearch: null,
    availableOfferings: generateCollegiaOfferings([]),
    ownedTechnologies: [],
    rerollAvailable: true,
  };
}

export function createInitialGameState(seed: number = Date.now()): GameState {
  const mapWidth = 30;
  const mapHeight = 30;

  const map = generateMap(mapWidth, mapHeight, seed);
  const factions = new Map<FactionId, Faction>();
  const legions = new Map<string, Legion>();
  const cities = new Map<string, City>();

  // Create factions (player + AI)
  const activeFactions: FactionId[] = ['player', 'hippus', 'sheaim'];
  for (const factionId of activeFactions) {
    const template = FACTION_TEMPLATES[factionId];
    factions.set(factionId, createFaction(template));
  }

  // Place starting cities and legions
  placeStartingEntities(map, activeFactions, cities, legions, seed);

  return {
    phase: 'player_turn',
    turn: 1,
    armageddonCounter: 0,
    factions,
    legions,
    cities,
    map,
    mapWidth,
    mapHeight,
    selectedLegionId: null,
    selectedCityId: null,
    gameOver: false,
    winner: null,
    pendingCombat: null,
    collegia: createInitialCollegia(),
  };
}

export function getTile(state: GameState, coord: Coord): Tile | null {
  if (coord.x < 0 || coord.x >= state.mapWidth || coord.y < 0 || coord.y >= state.mapHeight) {
    return null;
  }
  return state.map[coord.y][coord.x];
}

export function getLegionAt(state: GameState, coord: Coord): Legion | null {
  for (const legion of state.legions.values()) {
    if (coordsEqual(legion.location, coord)) {
      return legion;
    }
  }
  return null;
}

export function getCityAt(state: GameState, coord: Coord): City | null {
  for (const city of state.cities.values()) {
    if (coordsEqual(city.coord, coord)) {
      return city;
    }
  }
  return null;
}

export function getPlayerLegions(state: GameState): Legion[] {
  return Array.from(state.legions.values()).filter(l => l.owner === 'player');
}

export function getPlayerCities(state: GameState): City[] {
  return Array.from(state.cities.values()).filter(c => c.owner === 'player');
}

export function getFactionLegions(state: GameState, factionId: FactionId): Legion[] {
  return Array.from(state.legions.values()).filter(l => l.owner === factionId);
}

export function getFactionCities(state: GameState, factionId: FactionId): City[] {
  return Array.from(state.cities.values()).filter(c => c.owner === factionId);
}

// Get the name of a legion based on its leader
export function getLegionName(legion: Legion): string {
  const leader = legion.soldiers.find(s => s.isLeader);
  if (leader) {
    return `${leader.name}'s Legion`;
  }
  // Fallback if no leader (shouldn't happen, but just in case)
  if (legion.soldiers.length > 0) {
    return `${legion.soldiers[0].name}'s Legion`;
  }
  return 'Empty Legion';
}

// Ensure a legion has a leader (promotes first soldier if needed)
function ensureLegionHasLeader(legion: Legion): void {
  if (legion.soldiers.length === 0) return;

  const hasLeader = legion.soldiers.some(s => s.isLeader);
  if (!hasLeader) {
    // Promote the first soldier to leader
    legion.soldiers[0].isLeader = true;
  }
}

const CITY_RECRUIT_RANGE = 2; // Legions within this range can receive soldiers from city

export function getLegionsInRangeOfCity(state: GameState, city: City, factionId: FactionId): Legion[] {
  return Array.from(state.legions.values()).filter(legion => {
    if (legion.owner !== factionId) return false;
    const distance = manhattanDistance(legion.location, city.coord);
    return distance <= CITY_RECRUIT_RANGE;
  });
}

export function getCitiesInRangeOfLegion(state: GameState, legion: Legion, factionId: FactionId): City[] {
  return Array.from(state.cities.values()).filter(city => {
    if (city.owner !== factionId) return false;
    const distance = manhattanDistance(legion.location, city.coord);
    return distance <= CITY_RECRUIT_RANGE;
  });
}

export function getLegionsInRangeOfLegion(state: GameState, legion: Legion, factionId: FactionId): Legion[] {
  return Array.from(state.legions.values()).filter(otherLegion => {
    if (otherLegion.id === legion.id) return false; // Exclude self
    if (otherLegion.owner !== factionId) return false;
    const distance = manhattanDistance(legion.location, otherLegion.location);
    return distance <= CITY_RECRUIT_RANGE;
  });
}

// Calculate income breakdown for a city
export interface CityIncomeBreakdown {
  baseGold: number;
  buildingGold: number;
  buildingMana: number;
  featureGold: number;
  featureMana: number;
  totalGold: number;
  totalMana: number;
  goldSources: { name: string; amount: number }[];
  manaSources: { name: string; amount: number }[];
}

export function getCityIncome(city: City, state?: GameState): CityIncomeBreakdown {
  const baseGold = city.occupationTurns > 0 ? Math.floor(BASE_CITY_INCOME / 2) : BASE_CITY_INCOME;
  let buildingGold = 0;
  let buildingMana = 0;
  let featureGold = 0;
  let featureMana = 0;
  const goldSources: { name: string; amount: number }[] = [];
  const manaSources: { name: string; amount: number }[] = [];

  if (city.occupationTurns === 0) {
    // Building effects
    for (const buildingId of city.buildings) {
      const building = BUILDING_TYPES[buildingId];
      for (const effect of building.effects) {
        if (effect.type === 'gold_per_turn') {
          buildingGold += effect.amount;
          goldSources.push({ name: building.name, amount: effect.amount });
        }
        if (effect.type === 'mana_per_turn') {
          buildingMana += effect.amount;
          manaSources.push({ name: building.name, amount: effect.amount });
        }
      }
    }

    // Terrain feature effects (if state is provided)
    if (state) {
      const tile = getTile(state, city.coord);
      if (tile?.feature) {
        const feature = TERRAIN_FEATURES[tile.feature];
        if (feature) {
          for (const effect of feature.effects) {
            if (effect.type === 'gold_bonus') {
              featureGold += effect.amount;
              goldSources.push({ name: feature.name, amount: effect.amount });
            }
            if (effect.type === 'mana_bonus') {
              featureMana += effect.amount;
              manaSources.push({ name: feature.name, amount: effect.amount });
            }
          }
        }
      }
    }
  }

  return {
    baseGold,
    buildingGold,
    buildingMana,
    featureGold,
    featureMana,
    totalGold: baseGold + buildingGold + featureGold,
    totalMana: buildingMana + featureMana,
    goldSources,
    manaSources,
  };
}

// Calculate total faction income
export interface FactionIncomeBreakdown {
  totalGold: number;
  totalMana: number;
  cityBreakdowns: { cityName: string; gold: number; mana: number }[];
}

export function getFactionIncome(state: GameState, factionId: FactionId): FactionIncomeBreakdown {
  const cities = getFactionCities(state, factionId);
  let totalGold = 0;
  let totalMana = 0;
  const cityBreakdowns: { cityName: string; gold: number; mana: number }[] = [];

  // Calculate tech bonuses (only for player)
  let globalGoldBonus = 0;
  let globalManaBonus = 0;
  const buildingGoldBonuses: Partial<Record<BuildingId, number>> = {};
  const buildingManaBonuses: Partial<Record<BuildingId, number>> = {};

  if (factionId === 'player') {
    for (const techId of state.collegia.ownedTechnologies) {
      const tech = TECHNOLOGIES[techId];
      if (!tech) continue;

      for (const effect of tech.effects) {
        if (effect.type === 'global_gold_bonus') {
          globalGoldBonus += effect.amount;
        } else if (effect.type === 'global_mana_bonus') {
          globalManaBonus += effect.amount;
        } else if (effect.type === 'building_gold_bonus') {
          buildingGoldBonuses[effect.building] = (buildingGoldBonuses[effect.building] ?? 0) + effect.amount;
        } else if (effect.type === 'building_mana_bonus') {
          buildingManaBonuses[effect.building] = (buildingManaBonuses[effect.building] ?? 0) + effect.amount;
        }
      }
    }
  }

  for (const city of cities) {
    const income = getCityIncome(city, state);
    let cityGold = income.totalGold;
    let cityMana = income.totalMana;

    // Add building-specific tech bonuses for this city
    for (const buildingId of city.buildings) {
      if (buildingGoldBonuses[buildingId]) {
        cityGold += buildingGoldBonuses[buildingId]!;
      }
      if (buildingManaBonuses[buildingId]) {
        cityMana += buildingManaBonuses[buildingId]!;
      }
    }

    totalGold += cityGold;
    totalMana += cityMana;
    cityBreakdowns.push({
      cityName: city.name,
      gold: cityGold,
      mana: cityMana,
    });
  }

  // Add global bonuses
  totalGold += globalGoldBonus;
  totalMana += globalManaBonus;

  return { totalGold, totalMana, cityBreakdowns };
}

// Calculate growth info for a city
export interface CityGrowthInfo {
  currentProgress: number;
  progressNeeded: number;
  turnsUntilGrowth: number;
  growthRate: number; // Points per turn
  hasGranary: boolean;
}

export function getCityGrowthInfo(city: City, state?: GameState): CityGrowthInfo {
  let growthBonus = 0;
  let hasGranary = false;

  // Building growth bonuses
  for (const buildingId of city.buildings) {
    const building = BUILDING_TYPES[buildingId];
    for (const effect of building.effects) {
      if (effect.type === 'growth_bonus') {
        growthBonus += effect.amount;
        if (buildingId === 'granary') hasGranary = true;
      }
    }
  }

  // Terrain feature growth bonuses
  if (state) {
    const tile = getTile(state, city.coord);
    if (tile?.feature) {
      const feature = TERRAIN_FEATURES[tile.feature];
      if (feature) {
        for (const effect of feature.effects) {
          if (effect.type === 'growth_bonus') {
            growthBonus += effect.amount;
          }
        }
      }
    }
  }

  const growthRate = 1 + growthBonus;
  const currentProgress = city.growthProgress || 0;
  const remaining = TURNS_PER_POPULATION - currentProgress;
  const turnsUntilGrowth = Math.ceil(remaining / growthRate);

  return {
    currentProgress,
    progressNeeded: TURNS_PER_POPULATION,
    turnsUntilGrowth,
    growthRate,
    hasGranary,
  };
}

// Get defense bonus for a city
export function getCityDefenseBonus(city: City, state?: GameState): number {
  let bonus = 0;

  // Building defense bonuses
  for (const buildingId of city.buildings) {
    const building = BUILDING_TYPES[buildingId];
    for (const effect of building.effects) {
      if (effect.type === 'defense_bonus') {
        bonus += effect.amount;
      }
    }
  }

  // Terrain feature defense bonuses
  if (state) {
    const tile = getTile(state, city.coord);
    if (tile?.feature) {
      const feature = TERRAIN_FEATURES[tile.feature];
      if (feature) {
        for (const effect of feature.effects) {
          if (effect.type === 'defense_bonus') {
            bonus += effect.amount;
          }
        }
      }
    }
  }

  return bonus;
}

function isPassableTerrain(tile: Tile): boolean {
  return tile.terrain !== 'mountain' && tile.terrain !== 'water';
}

// ============ City Garrison Helpers ============

const GARRISON_SIZE = 5;
const GARRISON_RESPAWN_HP_PERCENT = 0.5;

// Create garrison soldiers for a city
export function createGarrison(fullHp: boolean = true): Soldier[] {
  const soldiers: Soldier[] = [];
  const soldierType = SOLDIER_TYPES.city_garrison;
  const hp = fullHp ? soldierType.hp : Math.floor(soldierType.hp * GARRISON_RESPAWN_HP_PERCENT);

  // Position garrison in front row (3) and mid row (2)
  const positions: { row: FormationRow; column: number }[] = [
    { row: 'front', column: 0 },
    { row: 'front', column: 1 },
    { row: 'front', column: 2 },
    { row: 'mid', column: 0 },
    { row: 'mid', column: 2 },
  ];

  for (let i = 0; i < GARRISON_SIZE; i++) {
    soldiers.push({
      id: generateId('garrison'),
      name: '', // No name for garrison soldiers
      type: 'city_garrison',
      hp,
      maxHp: soldierType.hp,
      position: positions[i],
      isGarrison: true,
    });
  }

  return soldiers;
}

// Convert city garrison to a pseudo-Legion for combat
export function getCityGarrisonAsLegion(city: City): Legion {
  return {
    id: `garrison_${city.id}`,
    owner: city.owner,
    soldiers: city.garrison,
    location: city.coord,
    movementRemaining: 0,
  };
}

// Find nearest empty adjacent tile for legion spawning
export function findEmptyAdjacentTile(state: GameState, coord: Coord): Coord | null {
  const neighbors = getNeighbors(coord, state.mapWidth, state.mapHeight);

  for (const neighbor of neighbors) {
    const tile = getTile(state, neighbor);
    if (!tile) continue;
    if (!isPassableTerrain(tile)) continue;
    if (getLegionAt(state, neighbor)) continue;
    if (getCityAt(state, neighbor)) continue; // Cannot spawn on another city

    return neighbor;
  }

  return null;
}

// Get movement cost for a terrain type (swamp costs 2 movement)
function getTerrainMovementCost(terrain: string): number {
  if (terrain === 'swamp') return 2;
  return 1;
}

function findRetreatTile(
  state: GameState,
  combatLocation: Coord,
  attackerOrigin: Coord,
  retreatingLegionOwner: FactionId
): Coord | null {
  const neighbors = getNeighbors(combatLocation, state.mapWidth, state.mapHeight);

  // Filter to passable tiles without legions
  const validTiles = neighbors.filter(coord => {
    const tile = getTile(state, coord);
    if (!tile || !isPassableTerrain(tile)) return false;

    // Can't retreat to a tile occupied by another legion
    const legionAtTile = getLegionAt(state, coord);
    if (legionAtTile) return false;

    return true;
  });

  if (validTiles.length === 0) return null;

  // Prioritize retreating away from attacker (maximize distance from attacker origin)
  validTiles.sort((a, b) => {
    const distA = manhattanDistance(a, attackerOrigin);
    const distB = manhattanDistance(b, attackerOrigin);
    return distB - distA; // Higher distance = better retreat
  });

  return validTiles[0];
}

export function getValidMoves(state: GameState, legion: Legion): Coord[] {
  if (legion.movementRemaining <= 0) return [];

  // Get tiles reachable without passing through ANY legion (friendly or enemy) or city
  const reachableTiles = getTilesInRange(
    legion.location,
    legion.movementRemaining,
    state.map,
    (tile) => {
      if (!isPassableTerrain(tile)) return false;
      // Cannot pass through any legion (friendly or enemy)
      const legionAtTile = getLegionAt(state, tile.coord);
      if (legionAtTile) return false;
      // Cannot pass through any city (cities and legions are mutually exclusive)
      const cityAtTile = getCityAt(state, tile.coord);
      if (cityAtTile) return false;
      return true;
    },
    (tile) => getTerrainMovementCost(tile.terrain)
  );

  // Filter out tiles with friendly legions or any city (can't move onto cities)
  const validMoves = reachableTiles.filter(coord => {
    const legionAtTile = getLegionAt(state, coord);
    if (legionAtTile && legionAtTile.owner === legion.owner) return false;
    // Cannot move onto any city tile
    const cityAtTile = getCityAt(state, coord);
    if (cityAtTile) return false;
    return true;
  });

  // Add adjacent enemy tiles that we can attack (even if we couldn't path through them)
  const neighbors = getNeighbors(legion.location, state.mapWidth, state.mapHeight);
  for (const neighbor of neighbors) {
    const tile = getTile(state, neighbor);
    if (!tile || !isPassableTerrain(tile)) continue;

    // Check for enemy legion
    const legionAtTile = getLegionAt(state, neighbor);
    if (legionAtTile && legionAtTile.owner !== legion.owner) {
      // Adjacent enemy legion - can attack
      if (!validMoves.some(c => c.x === neighbor.x && c.y === neighbor.y)) {
        validMoves.push(neighbor);
      }
    }

    // Check for enemy city (to attack garrison)
    const cityAtTile = getCityAt(state, neighbor);
    if (cityAtTile && cityAtTile.owner !== legion.owner) {
      // Adjacent enemy city - can attack garrison
      if (!validMoves.some(c => c.x === neighbor.x && c.y === neighbor.y)) {
        validMoves.push(neighbor);
      }
    }
  }

  return validMoves;
}

// Get the movement path from a legion to a destination (for path preview)
export function getMovementPath(state: GameState, legion: Legion, to: Coord): Coord[] | null {
  // Check if destination is adjacent (for attacking enemies or cities)
  const isAdjacent = Math.abs(legion.location.x - to.x) + Math.abs(legion.location.y - to.y) === 1;
  const legionAtDest = getLegionAt(state, to);
  const cityAtDest = getCityAt(state, to);

  // If attacking adjacent enemy legion, path is just [from, to]
  if (isAdjacent && legionAtDest && legionAtDest.owner !== legion.owner) {
    return [legion.location, to];
  }

  // If attacking adjacent enemy city, path is just [from, to]
  if (isAdjacent && cityAtDest && cityAtDest.owner !== legion.owner) {
    return [legion.location, to];
  }

  // Otherwise, find path avoiding all legions and cities
  return findPath(
    legion.location,
    to,
    state.map,
    (tile) => {
      if (!isPassableTerrain(tile)) return false;
      const legionAtTile = getLegionAt(state, tile.coord);
      if (legionAtTile) return false;
      const cityAtTile = getCityAt(state, tile.coord);
      if (cityAtTile) return false;
      return true;
    },
    (tile) => getTerrainMovementCost(tile.terrain)
  );
}

// Get the movement cost for a path
export function getMovementCost(state: GameState, path: Coord[]): number {
  if (!path || path.length < 2) return 0;
  const moveCostFn = (tile: Tile) => getTerrainMovementCost(tile.terrain);
  return getPathCost(path, state.map, moveCostFn);
}

export function processAction(state: GameState, action: GameAction): GameState {
  switch (action.type) {
    case 'select_tile':
      return handleSelectTile(state, action.coord);

    case 'move_legion':
      return handleMoveLegion(state, action.legionId, action.to);

    case 'end_turn':
      return handleEndTurn(state);

    case 'build':
      return handleBuild(state, action.cityId, action.building);

    case 'recruit':
      return handleRecruit(state, action.cityId, action.soldierType, action.legionId);

    case 'create_legion':
      return handleCreateLegion(state, action.cityId);

    case 'apply_combat_results':
      return handleApplyCombatResults(state);

    case 'queue_building':
      return handleQueueBuilding(state, action.cityId, action.buildingId);

    case 'queue_soldier':
      return handleQueueSoldier(state, action.cityId, action.soldierType, action.targetLegionId);

    case 'cancel_queue_item':
      return handleCancelQueueItem(state, action.cityId, action.queueItemId);

    case 'assign_soldier':
      return handleAssignSoldier(state, action.cityId, action.soldierId, action.legionId);

    case 'unassign_soldier':
      return handleUnassignSoldier(state, action.legionId, action.soldierId, action.cityId);

    case 'create_legion_from_roster':
      return handleCreateLegionFromRoster(state, action.cityId, action.soldierId);

    case 'transfer_soldier':
      return handleTransferSoldier(state, action.fromLegionId, action.soldierId, action.toLegionId);

    case 'select_research':
      return handleSelectResearch(state, action.technologyId);

    case 'reroll_collegia':
      return handleRerollCollegia(state);

    default:
      return state;
  }
}

function handleSelectTile(state: GameState, coord: Coord): GameState {
  const legion = getLegionAt(state, coord);
  const city = getCityAt(state, coord);

  // FIRST: If we have a legion selected and clicked a valid move tile, move there
  // This must come before city/legion selection to allow moving onto friendly cities
  if (state.selectedLegionId) {
    const selectedLegion = state.legions.get(state.selectedLegionId);
    if (selectedLegion) {
      const validMoves = getValidMoves(state, selectedLegion);
      const isValidMove = validMoves.some(m => coordsEqual(m, coord));

      if (isValidMove) {
        return handleMoveLegion(state, state.selectedLegionId, coord);
      }
    }
  }

  // THEN: Check for selecting a player legion
  if (legion && legion.owner === 'player') {
    return { ...state, selectedLegionId: legion.id, selectedCityId: null };
  }

  // THEN: Check for selecting a player city
  if (city && city.owner === 'player') {
    return { ...state, selectedLegionId: null, selectedCityId: city.id };
  }

  // Allow selecting enemy legions for viewing (but not controlling)
  if (legion) {
    return { ...state, selectedLegionId: legion.id, selectedCityId: null };
  }

  return { ...state, selectedLegionId: null, selectedCityId: null };
}

function handleMoveLegion(state: GameState, legionId: string, to: Coord): GameState {
  const legion = state.legions.get(legionId);
  if (!legion || legion.owner !== 'player') return state;
  if (legion.movementRemaining <= 0) return state;

  const validMoves = getValidMoves(state, legion);
  if (!validMoves.some(m => coordsEqual(m, to))) return state;

  // Check for city attack (must be adjacent - cities and legions are mutually exclusive)
  const city = getCityAt(state, to);
  if (city && city.owner !== legion.owner) {
    return handleCityCombat(state, legion, city);
  }

  // Check for combat with enemy legion
  const enemyLegion = getLegionAt(state, to);
  if (enemyLegion && enemyLegion.owner !== legion.owner) {
    return handleCombat(state, legion, enemyLegion, to);
  }

  // Simple move (cannot move onto city tiles)
  const newLegions = new Map(state.legions);
  const moveCostFn = (tile: Tile) => getTerrainMovementCost(tile.terrain);
  const path = findPath(legion.location, to, state.map, isPassableTerrain, moveCostFn);
  const movementCost = path ? getPathCost(path, state.map, moveCostFn) : 1;

  newLegions.set(legionId, {
    ...legion,
    location: to,
    movementRemaining: Math.max(0, legion.movementRemaining - movementCost),
  });

  return { ...state, legions: newLegions, selectedLegionId: null };
}

// Get soldier max HP with technology bonus applied
function getSoldierMaxHp(state: GameState, factionId: FactionId, soldierType: SoldierTypeId): number {
  const baseHp = SOLDIER_TYPES[soldierType].hp;

  // Only the player has technologies
  if (factionId !== 'player') return baseHp;

  let hpBonus = 0;
  for (const techId of state.collegia.ownedTechnologies) {
    const tech = TECHNOLOGIES[techId];
    if (!tech) continue;

    for (const effect of tech.effects) {
      if (effect.type === 'soldier_hp_bonus' && effect.soldier === soldierType) {
        hpBonus += effect.amount;
      }
    }
  }

  return baseHp + hpBonus;
}

// Calculate tech bonuses for a faction's soldiers in combat
function getSoldierTechBonuses(state: GameState, factionId: FactionId): SoldierTechBonuses | undefined {
  // Only the player has technologies (AI doesn't use the Collegia system)
  if (factionId !== 'player') return undefined;

  const attack: Partial<Record<SoldierTypeId, number>> = {};
  const defense: Partial<Record<SoldierTypeId, number>> = {};
  let globalDefense = 0;

  for (const techId of state.collegia.ownedTechnologies) {
    const tech = TECHNOLOGIES[techId];
    if (!tech) continue;

    for (const effect of tech.effects) {
      if (effect.type === 'soldier_attack_bonus') {
        attack[effect.soldier] = (attack[effect.soldier] ?? 0) + effect.amount;
      } else if (effect.type === 'soldier_defense_bonus') {
        defense[effect.soldier] = (defense[effect.soldier] ?? 0) + effect.amount;
      } else if (effect.type === 'global_defense_bonus') {
        globalDefense += effect.amount;
      }
    }
  }

  return { attack, defense, globalDefense };
}

function handleCombat(
  state: GameState,
  attacker: Legion,
  defender: Legion,
  combatLocation: Coord
): GameState {
  const tile = getTile(state, combatLocation);
  if (!tile) return state;

  const city = getCityAt(state, combatLocation);
  const hasWalls = city?.buildings.includes('walls') ?? false;

  // Calculate tech bonuses for both sides
  const attackerTechBonuses = getSoldierTechBonuses(state, attacker.owner);
  const defenderTechBonuses = getSoldierTechBonuses(state, defender.owner);

  const result = resolveCombat(attacker, defender, tile.terrain, hasWalls, attackerTechBonuses, defenderTechBonuses);

  // Set pending combat and change phase - combat scene will display and call apply_combat_results when done
  return {
    ...state,
    phase: 'combat_resolution',
    pendingCombat: {
      attackerId: attacker.id,
      defenderId: defender.id,
      combatLocation,
      result,
    },
    selectedLegionId: null,
  };
}

// Handle combat between a legion and a city's garrison
function handleCityCombat(
  state: GameState,
  attacker: Legion,
  city: City
): GameState {
  // Create pseudo-legion from garrison
  const defenderLegion = getCityGarrisonAsLegion(city);

  // If garrison is empty (all dead), capture immediately
  if (defenderLegion.soldiers.length === 0) {
    return handleUndefendedCityCapture(state, attacker, city);
  }

  const tile = getTile(state, city.coord);
  if (!tile) return state;

  const hasWalls = city.buildings.includes('walls');

  // Calculate tech bonuses for both sides
  const attackerTechBonuses = getSoldierTechBonuses(state, attacker.owner);
  const defenderTechBonuses = getSoldierTechBonuses(state, city.owner);

  const result = resolveCombat(attacker, defenderLegion, tile.terrain, hasWalls, attackerTechBonuses, defenderTechBonuses);

  // Set pending combat with garrison ID prefix
  return {
    ...state,
    phase: 'combat_resolution',
    pendingCombat: {
      attackerId: attacker.id,
      defenderId: `garrison_${city.id}`, // Special ID for garrison combat
      combatLocation: city.coord,
      result,
    },
    selectedLegionId: null,
  };
}

// Handle capturing an undefended city (empty garrison)
function handleUndefendedCityCapture(
  state: GameState,
  attacker: Legion,
  city: City
): GameState {
  const newCities = new Map(state.cities);
  newCities.set(city.id, {
    ...city,
    owner: attacker.owner,
    occupationTurns: 3,
    garrison: createGarrison(false), // Respawn garrison at 50% HP
  });

  // Attacker uses movement but stays in place (adjacent to city)
  const newLegions = new Map(state.legions);
  newLegions.set(attacker.id, {
    ...attacker,
    movementRemaining: 0,
  });

  return { ...state, legions: newLegions, cities: newCities, selectedLegionId: null };
}

function handleApplyCombatResults(state: GameState): GameState {
  if (!state.pendingCombat) return state;

  const { attackerId, defenderId, combatLocation, result } = state.pendingCombat;

  // Check if this is a garrison combat (defenderId starts with 'garrison_')
  if (defenderId.startsWith('garrison_')) {
    return handleGarrisonCombatResults(state, attackerId, defenderId, combatLocation, result);
  }

  // Standard legion vs legion combat
  const attacker = state.legions.get(attackerId);
  const defender = state.legions.get(defenderId);

  if (!attacker || !defender) {
    return { ...state, pendingCombat: null, phase: 'player_turn' };
  }

  const newLegions = new Map(state.legions);

  // Apply combat results - use survivors which have updated HP
  const updatedAttacker = {
    ...attacker,
    soldiers: result.attackerSurvivors,
    movementRemaining: 0,
  };

  const updatedDefender = {
    ...defender,
    soldiers: result.defenderSurvivors,
  };

  if (result.attackerWon) {
    // Attacker moves to the tile, defender retreats or is destroyed
    updatedAttacker.location = combatLocation;

    if (updatedDefender.soldiers.length > 0) {
      // Find retreat tile for defender
      const retreatTile = findRetreatTile(
        state,
        combatLocation,
        attacker.location, // Attacker's original position
        defender.owner
      );

      if (retreatTile) {
        // Defender retreats with survivors
        updatedDefender.location = retreatTile;
        updatedDefender.movementRemaining = 0; // Can't move after retreating
        newLegions.set(defender.id, updatedDefender);
      } else {
        // No retreat path - legion is destroyed (cornered)
        newLegions.delete(defender.id);
      }
    } else {
      newLegions.delete(defender.id);
    }

    if (updatedAttacker.soldiers.length > 0) {
      newLegions.set(attacker.id, updatedAttacker);
    } else {
      newLegions.delete(attacker.id);
    }
  } else {
    // Attacker stays in place or retreats
    if (updatedAttacker.soldiers.length > 0) {
      newLegions.set(attacker.id, updatedAttacker);
    } else {
      newLegions.delete(attacker.id);
    }

    if (updatedDefender.soldiers.length > 0) {
      newLegions.set(defender.id, updatedDefender);
    } else {
      newLegions.delete(defender.id);
    }
  }

  // Determine next phase: if attacker was AI, continue AI turn; otherwise player turn
  const wasAIAttack = attacker.owner !== 'player';
  const resultState = { ...state, legions: newLegions, pendingCombat: null, phase: 'ai_turn' as GamePhase };

  if (wasAIAttack) {
    // Continue AI turn processing
    return continueAITurn(resultState);
  }

  return { ...resultState, phase: 'player_turn' };
}

// Handle the results of garrison combat
function handleGarrisonCombatResults(
  state: GameState,
  attackerId: string,
  garrisonDefenderId: string,
  combatLocation: Coord,
  result: import('../types').CombatResult
): GameState {
  const attacker = state.legions.get(attackerId);
  const cityId = garrisonDefenderId.replace('garrison_', '');
  const city = state.cities.get(cityId);

  if (!attacker || !city) {
    // If we can't find the attacker, we don't know if it was AI - default to player turn
    return { ...state, pendingCombat: null, phase: 'player_turn' };
  }

  const newLegions = new Map(state.legions);
  const newCities = new Map(state.cities);

  // Update attacker (stays in original position, doesn't move onto city)
  const updatedAttacker = {
    ...attacker,
    soldiers: result.attackerSurvivors,
    movementRemaining: 0,
  };

  if (result.attackerWon) {
    // City is captured
    // Attacker stays adjacent to city (doesn't move onto it)
    if (updatedAttacker.soldiers.length > 0) {
      newLegions.set(attacker.id, updatedAttacker);
    } else {
      newLegions.delete(attacker.id);
    }

    // Capture city and regenerate garrison at 50% HP
    newCities.set(city.id, {
      ...city,
      owner: attacker.owner,
      occupationTurns: 3,
      garrison: createGarrison(false), // 50% HP garrison
    });
  } else {
    // Attacker failed
    if (updatedAttacker.soldiers.length > 0) {
      newLegions.set(attacker.id, updatedAttacker);
    } else {
      newLegions.delete(attacker.id);
    }

    // Update garrison survivors
    newCities.set(city.id, {
      ...city,
      garrison: result.defenderSurvivors,
    });
  }

  // Determine next phase: if attacker was AI, continue AI turn; otherwise player turn
  const wasAIAttack = attacker.owner !== 'player';
  const resultState = { ...state, legions: newLegions, cities: newCities, pendingCombat: null, phase: 'ai_turn' as GamePhase };

  if (wasAIAttack) {
    // Continue AI turn processing
    return continueAITurn(resultState);
  }

  return { ...resultState, phase: 'player_turn' };
}

function handleCityCapture(
  state: GameState,
  legion: Legion,
  city: City,
  location: Coord
): GameState {
  const newLegions = new Map(state.legions);
  const newCities = new Map(state.cities);

  newLegions.set(legion.id, {
    ...legion,
    location,
    movementRemaining: 0,
  });

  newCities.set(city.id, {
    ...city,
    owner: legion.owner,
    occupationTurns: city.owner === 'player' ? 3 : 3, // Both directions have penalty
  });

  return { ...state, legions: newLegions, cities: newCities, selectedLegionId: null };
}

// ============ AI Turn Processing ============

// Calculate legion HP percentage
function getLegionHpPercentage(legion: Legion): number {
  if (legion.soldiers.length === 0) return 0;
  const totalHp = legion.soldiers.reduce((sum, s) => sum + s.hp, 0);
  const totalMaxHp = legion.soldiers.reduce((sum, s) => sum + s.maxHp, 0);
  return totalMaxHp > 0 ? totalHp / totalMaxHp : 0;
}

// Get the home city for a faction (first city owned)
function getFactionHomeCity(state: GameState, factionId: FactionId): City | null {
  for (const city of state.cities.values()) {
    if (city.owner === factionId) return city;
  }
  return null;
}

// Check if legion is at a friendly city (for healing)
function isLegionAtFriendlyCity(state: GameState, legion: Legion): boolean {
  const city = getCityAt(state, legion.location);
  return city !== null && city.owner === legion.owner;
}

function processAITurn(state: GameState): GameState {
  let newState = { ...state };

  // Get all AI factions
  const aiFactions = Array.from(newState.factions.values()).filter(f => f.id !== 'player');

  for (const faction of aiFactions) {
    // Reset movement for this faction's legions
    const newLegions = new Map(newState.legions);
    for (const [id, legion] of newLegions) {
      if (legion.owner === faction.id) {
        newLegions.set(id, { ...legion, movementRemaining: BASE_MOVEMENT_RANGE });
      }
    }
    newState = { ...newState, legions: newLegions };

    // Update faction state based on conditions
    newState = updateFactionState(newState, faction);

    // Process recruitment for this faction
    newState = processAIRecruitment(newState, faction);

    // Process each legion for this faction
    const factionLegions = Array.from(newState.legions.values()).filter(l => l.owner === faction.id);

    for (const legion of factionLegions) {
      // Skip if legion has no soldiers
      if (legion.soldiers.length === 0) continue;

      // Check if legion needs to retreat/heal
      const hpPercent = getLegionHpPercentage(legion);
      const homeCity = getFactionHomeCity(newState, faction.id);

      if (hpPercent < 0.3 && homeCity) {
        // Low HP - retreat to home city to heal
        if (!isLegionAtFriendlyCity(newState, legion)) {
          newState = processAILegionMove(newState, legion, homeCity.coord);
          // If combat triggered against player, return immediately
          if (newState.phase === 'combat_resolution') return newState;
        } else {
          // At home city - heal soldiers
          newState = healLegionAtCity(newState, legion);
        }
        continue;
      }

      // Find targets based on faction type and state
      const targets = findAITargets(newState, newState.factions.get(faction.id)!, legion);

      if (targets.length > 0) {
        // Move toward nearest target
        newState = processAILegionMove(newState, legion, targets[0]);
        // If combat triggered against player, return immediately
        if (newState.phase === 'combat_resolution') return newState;
      }
    }
  }

  return newState;
}

// Update faction state based on conditions
function updateFactionState(state: GameState, faction: Faction): GameState {
  const newFactions = new Map(state.factions);
  let newState = faction.state;

  if (faction.type === 'raider') {
    // Raiders: idle -> raiding when they have enough gold/troops
    const factionLegions = Array.from(state.legions.values()).filter(l => l.owner === faction.id);
    const totalSoldiers = factionLegions.reduce((sum, l) => sum + l.soldiers.length, 0);

    if (faction.state.type === 'idle' && totalSoldiers >= 3) {
      // Start raiding
      const playerCities = Array.from(state.cities.values()).filter(c => c.owner === 'player');
      if (playerCities.length > 0) {
        newState = { type: 'raiding', target: playerCities[0].coord };
      }
    } else if (faction.state.type === 'raiding' && totalSoldiers === 0) {
      // No troops left, go back to idle
      newState = { type: 'idle' };
    }
  } else if (faction.type === 'defender') {
    // Defenders: always defending
    if (faction.state.type !== 'defending') {
      newState = { type: 'defending' };
    }
  }
  // Ritualists stay idle for now

  if (newState !== faction.state) {
    newFactions.set(faction.id, { ...faction, state: newState });
    return { ...state, factions: newFactions };
  }

  return state;
}

// Heal soldiers in a legion at a friendly city
function healLegionAtCity(state: GameState, legion: Legion): GameState {
  const HEAL_AMOUNT = 20; // HP healed per turn at city

  const newLegions = new Map(state.legions);
  const healedSoldiers = legion.soldiers.map(s => ({
    ...s,
    hp: Math.min(s.maxHp, s.hp + HEAL_AMOUNT),
  }));

  newLegions.set(legion.id, { ...legion, soldiers: healedSoldiers });
  return { ...state, legions: newLegions };
}

// Process AI recruitment for a faction
function processAIRecruitment(state: GameState, faction: Faction): GameState {
  let newState = state;

  // Get faction's cities
  const factionCities = Array.from(state.cities.values()).filter(c => c.owner === faction.id);
  if (factionCities.length === 0) return state;

  // Get faction's gold
  const factionData = state.factions.get(faction.id);
  if (!factionData || factionData.gold < 50) return state; // Need at least 50 gold

  // Get faction's legions
  const factionLegions = Array.from(state.legions.values()).filter(l => l.owner === faction.id);
  const totalSoldiers = factionLegions.reduce((sum, l) => sum + l.soldiers.length, 0);

  // Don't recruit if we already have enough soldiers
  if (totalSoldiers >= 8) return state;

  // Find a city with a legion that has room for soldiers
  for (const city of factionCities) {
    const legionAtCity = getLegionAt(state, city.coord);
    if (legionAtCity && legionAtCity.owner === faction.id && legionAtCity.soldiers.length < 8) {
      // Recruit a soldier
      const soldierType: SoldierTypeId = faction.type === 'raider' ? 'fighter' : 'archer';
      const soldierCost = SOLDIER_TYPES[soldierType].cost;

      if (factionData.gold >= soldierCost) {
        // Create new soldier
        const newSoldier: Soldier = {
          id: `${faction.id}-soldier-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
          name: `${faction.name} ${SOLDIER_TYPES[soldierType].name}`,
          type: soldierType,
          hp: SOLDIER_TYPES[soldierType].hp,
          maxHp: SOLDIER_TYPES[soldierType].hp,
          position: getNextAvailablePosition(legionAtCity),
        };

        // Update legion
        const newLegions = new Map(newState.legions);
        newLegions.set(legionAtCity.id, {
          ...legionAtCity,
          soldiers: [...legionAtCity.soldiers, newSoldier],
        });

        // Update faction gold
        const newFactions = new Map(newState.factions);
        newFactions.set(faction.id, {
          ...factionData,
          gold: factionData.gold - soldierCost,
        });

        newState = { ...newState, legions: newLegions, factions: newFactions };
        break; // Only recruit one per turn
      }
    }
  }

  return newState;
}

// Get next available position in a legion's formation
function getNextAvailablePosition(legion: Legion): { row: FormationRow; column: number } {
  const occupied = new Set(legion.soldiers.map(s => `${s.position.row}-${s.position.column}`));
  const rows: FormationRow[] = ['front', 'mid', 'back'];

  for (const row of rows) {
    for (let col = 0; col < 3; col++) {
      if (!occupied.has(`${row}-${col}`)) {
        return { row, column: col };
      }
    }
  }
  // Fallback (shouldn't happen with 8 soldier limit)
  return { row: 'front', column: 0 };
}

// Find potential targets for an AI legion
function findAITargets(state: GameState, faction: Faction, legion: Legion): Coord[] {
  const targets: { coord: Coord; priority: number; distance: number }[] = [];

  // Raiders prioritize player legions, then cities
  // Defenders prioritize defending their own territory
  // Ritualists are passive (for now)

  if (faction.type === 'raider' || faction.type === 'boss') {
    // Raiders and Boss target player legions, then cities
    // Boss is more aggressive with higher priority
    const priorityBonus = faction.type === 'boss' ? 5 : 0;

    // Target player legions
    for (const otherLegion of state.legions.values()) {
      if (otherLegion.owner === 'player') {
        const distance = manhattanDistance(legion.location, otherLegion.location);
        targets.push({ coord: otherLegion.location, priority: 10 + priorityBonus, distance });
      }
    }

    // Target player cities
    for (const city of state.cities.values()) {
      if (city.owner === 'player') {
        const distance = manhattanDistance(legion.location, city.coord);
        targets.push({ coord: city.coord, priority: 5 + priorityBonus, distance });
      }
    }
  } else if (faction.type === 'defender') {
    // Defenders attack nearby player units that are close to their cities
    const ownCities = Array.from(state.cities.values()).filter(c => c.owner === faction.id);

    for (const otherLegion of state.legions.values()) {
      if (otherLegion.owner === 'player') {
        // Check if player legion is near any of our cities
        for (const city of ownCities) {
          const distanceToCity = manhattanDistance(otherLegion.location, city.coord);
          if (distanceToCity <= 5) {
            const distance = manhattanDistance(legion.location, otherLegion.location);
            targets.push({ coord: otherLegion.location, priority: 10, distance });
            break;
          }
        }
      }
    }
  }
  // Ritualists are passive for now

  // Sort by priority (descending), then distance (ascending)
  targets.sort((a, b) => {
    if (a.priority !== b.priority) return b.priority - a.priority;
    return a.distance - b.distance;
  });

  return targets.map(t => t.coord);
}

// Move an AI legion toward a target
function processAILegionMove(state: GameState, legion: Legion, target: Coord): GameState {
  let newState = state;
  const currentLegion = newState.legions.get(legion.id);
  if (!currentLegion || currentLegion.movementRemaining <= 0) return newState;

  // Check if already at target
  if (coordsEqual(currentLegion.location, target)) return newState;

  // Check if adjacent to target - attack if there's an enemy there
  const distance = manhattanDistance(currentLegion.location, target);
  if (distance === 1) {
    // Check for enemy legion at target
    const enemyLegion = getLegionAt(newState, target);
    if (enemyLegion && enemyLegion.owner !== currentLegion.owner) {
      // Attack!
      return handleAICombat(newState, currentLegion, enemyLegion, target);
    }

    // Check for enemy city at target (attack garrison)
    const enemyCity = getCityAt(newState, target);
    if (enemyCity && enemyCity.owner !== currentLegion.owner) {
      // Cities now have garrisons - always attack the garrison
      return handleAICityCombat(newState, currentLegion, enemyCity);
    }

    // Cannot move onto city tiles (cities and legions are mutually exclusive)
    const anyCity = getCityAt(newState, target);
    if (anyCity) {
      // Can't move onto any city, even friendly
      return newState;
    }

    // Move to empty tile
    return moveAILegion(newState, currentLegion, target);
  }

  // Find path to target (avoiding cities since legions can't move onto them)
  const path = findPath(currentLegion.location, target, newState.map, (tile) => {
    if (!isPassableTerrain(tile)) return false;
    // Cities block movement (except the target if it's an enemy city to attack)
    const cityAtTile = getCityAt(newState, tile.coord);
    if (cityAtTile && !coordsEqual(tile.coord, target)) return false;
    return true;
  });
  if (!path || path.length < 2) return newState;

  // Move along path as far as movement allows
  let stepsToTake = Math.min(currentLegion.movementRemaining, path.length - 1);
  let currentPos = currentLegion.location;

  for (let i = 1; i <= stepsToTake; i++) {
    const nextPos = path[i];

    // Check for blocking units
    const blockingLegion = getLegionAt(newState, nextPos);
    if (blockingLegion) {
      if (blockingLegion.owner !== currentLegion.owner) {
        // Attack!
        return handleAICombat(newState, newState.legions.get(currentLegion.id)!, blockingLegion, nextPos);
      } else {
        // Friendly legion blocking - stop before it
        break;
      }
    }

    // Check for blocking city (attack enemy garrison or stop before friendly)
    const blockingCity = getCityAt(newState, nextPos);
    if (blockingCity) {
      if (blockingCity.owner !== currentLegion.owner) {
        // Attack enemy city garrison!
        return handleAICityCombat(newState, newState.legions.get(currentLegion.id)!, blockingCity);
      } else {
        // Friendly city blocking - stop before it
        break;
      }
    }

    currentPos = nextPos;
  }

  // Move to the furthest position we can reach
  if (!coordsEqual(currentPos, currentLegion.location)) {
    return moveAILegion(newState, currentLegion, currentPos);
  }

  return newState;
}

// Move an AI legion to a position
function moveAILegion(state: GameState, legion: Legion, to: Coord): GameState {
  const path = findPath(legion.location, to, state.map, isPassableTerrain);
  const distance = path ? path.length - 1 : manhattanDistance(legion.location, to);

  const newLegions = new Map(state.legions);
  newLegions.set(legion.id, {
    ...legion,
    location: to,
    movementRemaining: Math.max(0, legion.movementRemaining - distance),
  });

  return { ...state, legions: newLegions };
}

// Handle AI-initiated combat
function handleAICombat(state: GameState, attacker: Legion, defender: Legion, combatLocation: Coord): GameState {
  const tile = getTile(state, combatLocation);
  if (!tile) return state;

  const city = getCityAt(state, combatLocation);
  const hasWalls = city?.buildings.includes('walls') ?? false;

  // Calculate tech bonuses - defender may be player
  const attackerTechBonuses = getSoldierTechBonuses(state, attacker.owner);
  const defenderTechBonuses = getSoldierTechBonuses(state, defender.owner);
  const result = resolveCombat(attacker, defender, tile.terrain, hasWalls, attackerTechBonuses, defenderTechBonuses);

  // If defender is player, show combat scene
  if (defender.owner === 'player') {
    return {
      ...state,
      phase: 'combat_resolution',
      pendingCombat: {
        attackerId: attacker.id,
        defenderId: defender.id,
        combatLocation,
        result,
      },
    };
  }

  // AI vs AI: apply combat results immediately (no combat scene)
  const newLegions = new Map(state.legions);

  // Update attacker
  const updatedAttacker = {
    ...attacker,
    soldiers: result.attackerSurvivors,
    movementRemaining: 0,
  };

  // Update defender
  const updatedDefender = {
    ...defender,
    soldiers: result.defenderSurvivors,
  };

  if (result.attackerWon) {
    // Attacker wins - defender retreats or is destroyed
    if (result.defenderSurvivors.length === 0) {
      // Defender eliminated
      newLegions.delete(defender.id);
    } else {
      // Defender retreats
      const retreatDir = {
        x: defender.location.x - attacker.location.x,
        y: defender.location.y - attacker.location.y,
      };
      const retreatCoord = { x: defender.location.x + retreatDir.x, y: defender.location.y + retreatDir.y };
      const retreatTile = getTile(state, retreatCoord);

      if (retreatTile && isPassableTerrain(retreatTile) && !getLegionAt(state, retreatCoord)) {
        updatedDefender.location = retreatCoord;
        updatedDefender.movementRemaining = 0;
        newLegions.set(defender.id, updatedDefender);
      } else {
        // No retreat path - destroyed
        newLegions.delete(defender.id);
      }
    }

    // Attacker moves to combat location
    updatedAttacker.location = combatLocation;
    if (result.attackerSurvivors.length > 0) {
      newLegions.set(attacker.id, updatedAttacker);
    } else {
      newLegions.delete(attacker.id);
    }

    // Check for city capture
    let newCities = state.cities;
    if (city && city.owner !== attacker.owner) {
      const newCitiesMap = new Map(state.cities);
      newCitiesMap.set(city.id, {
        ...city,
        owner: attacker.owner,
        occupationTurns: 3,
      });
      newCities = newCitiesMap;
    }

    return { ...state, legions: newLegions, cities: newCities };
  } else {
    // Defender wins - attacker retreats
    if (result.attackerSurvivors.length === 0) {
      newLegions.delete(attacker.id);
    } else {
      updatedAttacker.movementRemaining = 0;
      newLegions.set(attacker.id, updatedAttacker);
    }

    if (result.defenderSurvivors.length > 0) {
      newLegions.set(defender.id, updatedDefender);
    } else {
      newLegions.delete(defender.id);
    }

    return { ...state, legions: newLegions };
  }
}

// Handle AI combat against city garrison
function handleAICityCombat(state: GameState, attacker: Legion, city: City): GameState {
  const defenderLegion = getCityGarrisonAsLegion(city);

  // If garrison is empty, capture immediately
  if (defenderLegion.soldiers.length === 0) {
    return handleAIUndefendedCityCapture(state, attacker, city);
  }

  const tile = getTile(state, city.coord);
  if (!tile) return state;

  const hasWalls = city.buildings.includes('walls');

  // Calculate tech bonuses - defender (city owner) may be player
  const attackerTechBonuses = getSoldierTechBonuses(state, attacker.owner);
  const defenderTechBonuses = getSoldierTechBonuses(state, city.owner);
  const result = resolveCombat(attacker, defenderLegion, tile.terrain, hasWalls, attackerTechBonuses, defenderTechBonuses);

  // If city belongs to player, show combat scene
  if (city.owner === 'player') {
    return {
      ...state,
      phase: 'combat_resolution',
      pendingCombat: {
        attackerId: attacker.id,
        defenderId: `garrison_${city.id}`, // Special ID for garrison combat
        combatLocation: city.coord,
        result,
      },
    };
  }

  // AI vs AI: apply combat results immediately (no combat scene)
  const newLegions = new Map(state.legions);
  const newCities = new Map(state.cities);

  const updatedAttacker = {
    ...attacker,
    soldiers: result.attackerSurvivors,
    movementRemaining: 0,
  };

  if (result.attackerWon) {
    // City captured - attacker stays adjacent (doesn't move onto city)
    if (updatedAttacker.soldiers.length > 0) {
      newLegions.set(attacker.id, updatedAttacker);
    } else {
      newLegions.delete(attacker.id);
    }

    // Capture city and regenerate garrison at 50% HP
    newCities.set(city.id, {
      ...city,
      owner: attacker.owner,
      occupationTurns: 3,
      garrison: createGarrison(false),
    });
  } else {
    // Attacker failed
    if (updatedAttacker.soldiers.length > 0) {
      newLegions.set(attacker.id, updatedAttacker);
    } else {
      newLegions.delete(attacker.id);
    }

    // Update garrison survivors
    newCities.set(city.id, {
      ...city,
      garrison: result.defenderSurvivors,
    });
  }

  return { ...state, legions: newLegions, cities: newCities };
}

// Handle AI capturing an undefended city (empty garrison)
function handleAIUndefendedCityCapture(state: GameState, legion: Legion, city: City): GameState {
  const newLegions = new Map(state.legions);
  // Legion stays adjacent to city (doesn't move onto it)
  newLegions.set(legion.id, {
    ...legion,
    movementRemaining: 0,
  });

  const newCities = new Map(state.cities);
  newCities.set(city.id, {
    ...city,
    owner: legion.owner,
    occupationTurns: 3,
    garrison: createGarrison(false), // Respawn garrison at 50% HP
  });

  return { ...state, legions: newLegions, cities: newCities };
}

// ============ End AI Processing ============

function handleEndTurn(state: GameState): GameState {
  // Process AI turns, then end turn phase
  let newState: GameState = { ...state, phase: 'ai_turn' };

  // Process AI faction turns
  newState = processAITurn(newState);

  // If combat was triggered against player, pause AI turn for combat resolution
  if (newState.phase === 'combat_resolution') {
    return newState;
  }

  return finishEndTurn(newState);
}

// Called after AI combat resolution to continue processing
function continueAITurn(state: GameState): GameState {
  // Continue processing remaining AI legions
  let newState = processAITurn(state);

  // If another combat was triggered, pause again
  if (newState.phase === 'combat_resolution') {
    return newState;
  }

  return finishEndTurn(newState);
}

// Finish end turn processing after AI is done
function finishEndTurn(state: GameState): GameState {
  // End turn processing
  let newState = processEndTurnPhase(state);

  // Check win/loss conditions
  newState = checkGameOver(newState);

  if (!newState.gameOver) {
    newState = {
      ...newState,
      phase: 'player_turn',
      turn: newState.turn + 1,
    };

    // Reset player legion movement (includes tech bonus)
    const playerMovementRange = getMovementRange(newState, 'player');
    const newLegions = new Map(newState.legions);
    for (const [id, legion] of newLegions) {
      if (legion.owner === 'player') {
        newLegions.set(id, { ...legion, movementRemaining: playerMovementRange });
      }
    }
    newState.legions = newLegions;
  }

  return newState;
}

function processEndTurnPhase(state: GameState): GameState {
  const newFactions = new Map(state.factions);
  const newCities = new Map(state.cities);
  const newLegions = new Map(state.legions);

  // Process each faction
  for (const [factionId, faction] of newFactions) {
    let goldIncome = 0;
    let manaIncome = 0;

    // Calculate tech bonuses (only for player)
    let globalGoldBonus = 0;
    let globalManaBonus = 0;
    const buildingGoldBonuses: Partial<Record<BuildingId, number>> = {};
    const buildingManaBonuses: Partial<Record<BuildingId, number>> = {};

    if (factionId === 'player') {
      for (const techId of state.collegia.ownedTechnologies) {
        const tech = TECHNOLOGIES[techId];
        if (!tech) continue;

        for (const effect of tech.effects) {
          if (effect.type === 'global_gold_bonus') {
            globalGoldBonus += effect.amount;
          } else if (effect.type === 'global_mana_bonus') {
            globalManaBonus += effect.amount;
          } else if (effect.type === 'building_gold_bonus') {
            buildingGoldBonuses[effect.building] = (buildingGoldBonuses[effect.building] ?? 0) + effect.amount;
          } else if (effect.type === 'building_mana_bonus') {
            buildingManaBonuses[effect.building] = (buildingManaBonuses[effect.building] ?? 0) + effect.amount;
          }
        }
      }
    }

    // Calculate income from cities
    for (const city of state.cities.values()) {
      if (city.owner !== factionId) continue;
      if (city.occupationTurns > 0) {
        // Reduced income during occupation
        goldIncome += Math.floor(BASE_CITY_INCOME / 2);
      } else {
        goldIncome += BASE_CITY_INCOME;

        // Building bonuses
        for (const buildingId of city.buildings) {
          const building = BUILDING_TYPES[buildingId];
          for (const effect of building.effects) {
            if (effect.type === 'gold_per_turn') goldIncome += effect.amount;
            if (effect.type === 'mana_per_turn') manaIncome += effect.amount;
          }

          // Tech building bonuses
          if (buildingGoldBonuses[buildingId]) {
            goldIncome += buildingGoldBonuses[buildingId]!;
          }
          if (buildingManaBonuses[buildingId]) {
            manaIncome += buildingManaBonuses[buildingId]!;
          }
        }

        // Terrain feature bonuses
        const tile = getTile(state, city.coord);
        if (tile?.feature) {
          const feature = TERRAIN_FEATURES[tile.feature];
          if (feature) {
            for (const effect of feature.effects) {
              if (effect.type === 'gold_bonus') goldIncome += effect.amount;
              if (effect.type === 'mana_bonus') manaIncome += effect.amount;
            }
          }
        }
      }
    }

    // Add global tech bonuses
    goldIncome += globalGoldBonus;
    manaIncome += globalManaBonus;

    newFactions.set(factionId, {
      ...faction,
      gold: faction.gold + goldIncome,
      mana: faction.mana + manaIncome,
    });
  }

  // Calculate global growth bonus from technologies (player only)
  let techGrowthBonus = 0;
  for (const techId of state.collegia.ownedTechnologies) {
    const tech = TECHNOLOGIES[techId];
    if (!tech) continue;

    for (const effect of tech.effects) {
      if (effect.type === 'global_growth_bonus') {
        techGrowthBonus += effect.amount;
      }
    }
  }

  // Population growth and occupation countdown
  for (const [cityId, city] of newCities) {
    let newCity = { ...city };

    // Occupation countdown
    if (newCity.occupationTurns > 0) {
      newCity.occupationTurns--;
    }

    // Calculate growth bonus from buildings
    let growthBonus = 0;
    for (const buildingId of newCity.buildings) {
      const building = BUILDING_TYPES[buildingId];
      for (const effect of building.effects) {
        if (effect.type === 'growth_bonus') {
          growthBonus += effect.amount;
        }
      }
    }

    // Terrain feature growth bonuses
    const tile = getTile(state, newCity.coord);
    if (tile?.feature) {
      const feature = TERRAIN_FEATURES[tile.feature];
      if (feature) {
        for (const effect of feature.effects) {
          if (effect.type === 'growth_bonus') {
            growthBonus += effect.amount;
          }
        }
      }
    }

    // Add tech growth bonus for player cities
    if (newCity.owner === 'player') {
      growthBonus += techGrowthBonus;
    }

    // Population growth - accumulate growth points each turn
    // Base growth is 1 point per turn, plus any growth bonus
    const growthRate = 1 + growthBonus;
    newCity.growthProgress = (newCity.growthProgress || 0) + growthRate;

    // When growth reaches threshold, add population
    if (newCity.growthProgress >= TURNS_PER_POPULATION) {
      newCity.population++;
      newCity.growthProgress -= TURNS_PER_POPULATION;
    }

    newCities.set(cityId, newCity);
  }

  // Process build queues for all cities
  for (const [cityId, city] of newCities) {
    if (city.buildQueue.length === 0) continue;

    const updatedQueue: BuildQueueItem[] = [];
    let cityChanged = false;
    let updatedCity = { ...city };

    for (const item of city.buildQueue) {
      const newTurnsRemaining = item.turnsRemaining - 1;

      if (newTurnsRemaining <= 0) {
        // Item completed
        cityChanged = true;

        if (item.itemType === 'building') {
          // Add building to city
          updatedCity = {
            ...updatedCity,
            buildings: [...updatedCity.buildings, item.itemId as BuildingId],
          };
        } else if (item.itemType === 'soldier') {
          const soldierType = SOLDIER_TYPES[item.itemId as SoldierTypeId];

          if (item.targetLegionId) {
            // Add soldier to legion
            const legion = newLegions.get(item.targetLegionId);
            if (legion && legion.soldiers.length < MAX_SOLDIERS_PER_LEGION) {
              // Find open position
              const usedPositions = new Set(
                legion.soldiers.map(s => `${s.position.row}-${s.position.column}`)
              );
              let position: { row: 'front' | 'mid' | 'back'; column: number } | null = null;
              const preferredRow = soldierType.preferredRow;
              const rowOrder: ('front' | 'mid' | 'back')[] =
                preferredRow === 'front'
                  ? ['front', 'mid', 'back']
                  : ['back', 'mid', 'front'];

              for (const row of rowOrder) {
                for (let col = 0; col < 3; col++) {
                  if (!usedPositions.has(`${row}-${col}`)) {
                    position = { row, column: col };
                    break;
                  }
                }
                if (position) break;
              }

              if (position) {
                // If legion is empty, this soldier becomes the leader
                const isLeader = legion.soldiers.length === 0;
                const soldierMaxHp = getSoldierMaxHp(state, city.owner, item.itemId as SoldierTypeId);
                const newSoldier: Soldier = {
                  id: generateId('soldier'),
                  name: generateSoldierName(),
                  type: item.itemId as SoldierTypeId,
                  hp: soldierMaxHp,
                  maxHp: soldierMaxHp,
                  position,
                  isLeader,
                };

                newLegions.set(item.targetLegionId, {
                  ...legion,
                  soldiers: [...legion.soldiers, newSoldier],
                });
              }
            }
          } else {
            // No target legion - add to city roster
            const rosterMaxHp = getSoldierMaxHp(state, city.owner, item.itemId as SoldierTypeId);
            const rosterSoldier: RosterSoldier = {
              id: generateId('soldier'),
              name: generateSoldierName(),
              type: item.itemId as SoldierTypeId,
              hp: rosterMaxHp,
              maxHp: rosterMaxHp,
            };
            updatedCity = {
              ...updatedCity,
              roster: [...updatedCity.roster, rosterSoldier],
            };
          }
        }
      } else {
        // Still in progress
        updatedQueue.push({
          ...item,
          turnsRemaining: newTurnsRemaining,
        });
      }
    }

    // Always update if there was a queue (turns remaining changed)
    if (city.buildQueue.length > 0) {
      newCities.set(cityId, {
        ...updatedCity,
        buildQueue: updatedQueue,
      });
    }
  }

  // Heal garrisoned legions
  for (const [legionId, legion] of newLegions) {
    const city = getCityAt(state, legion.location);
    if (city && city.owner === legion.owner) {
      const healedSoldiers = legion.soldiers.map(soldier => ({
        ...soldier,
        hp: Math.min(soldier.maxHp, soldier.hp + HEAL_RATE),
      }));
      newLegions.set(legionId, { ...legion, soldiers: healedSoldiers });
    }
  }

  // Advance Armageddon counter
  const newArmageddon = Math.min(ARMAGEDDON_MAX, state.armageddonCounter + 1);

  // Create intermediate state
  let stateAfterTurn: GameState = {
    ...state,
    factions: newFactions,
    cities: newCities,
    legions: newLegions,
    armageddonCounter: newArmageddon,
  };

  // Check if boss should spawn
  if (newArmageddon >= BOSS_SPAWN_THRESHOLD && state.armageddonCounter < BOSS_SPAWN_THRESHOLD) {
    stateAfterTurn = spawnBoss(stateAfterTurn);
  }

  // Update cultural borders (in case population changed)
  updateCulturalBorders(stateAfterTurn.map, stateAfterTurn.cities);

  // Process Collegia research
  return processCollegiaResearch(stateAfterTurn);
}

// Spawn the Infernal Legion boss when Armageddon threshold is reached
function spawnBoss(state: GameState): GameState {
  // Check if boss already exists
  if (state.factions.has('boss')) {
    return state;
  }

  const newFactions = new Map(state.factions);
  const newCities = new Map(state.cities);
  const newLegions = new Map(state.legions);

  // Create boss faction
  const bossTemplate = FACTION_TEMPLATES['boss'];
  newFactions.set('boss', createFaction(bossTemplate));

  // Find a suitable spawn location (far from player, on passable terrain)
  const playerCities = Array.from(state.cities.values()).filter(c => c.owner === 'player');
  const playerCenter = playerCities.length > 0
    ? {
        x: Math.round(playerCities.reduce((sum, c) => sum + c.coord.x, 0) / playerCities.length),
        y: Math.round(playerCities.reduce((sum, c) => sum + c.coord.y, 0) / playerCities.length),
      }
    : { x: state.mapWidth / 2, y: state.mapHeight / 2 };

  // Find spawn location on opposite side of map from player
  const targetX = playerCenter.x < state.mapWidth / 2 ? state.mapWidth - 5 : 5;
  const targetY = playerCenter.y < state.mapHeight / 2 ? state.mapHeight - 5 : 5;

  // Search for passable tile near target
  let spawnCoord: Coord | null = null;
  for (let radius = 0; radius < 10; radius++) {
    for (let dx = -radius; dx <= radius; dx++) {
      for (let dy = -radius; dy <= radius; dy++) {
        const coord = { x: targetX + dx, y: targetY + dy };
        if (coord.x < 0 || coord.x >= state.mapWidth || coord.y < 0 || coord.y >= state.mapHeight) continue;

        const tile = getTile(state, coord);
        if (tile && isPassableTerrain(tile) && !getCityAt(state, coord) && !getLegionAt(state, coord)) {
          spawnCoord = coord;
          break;
        }
      }
      if (spawnCoord) break;
    }
    if (spawnCoord) break;
  }

  if (!spawnCoord) {
    // Fallback - no suitable location found, skip spawning
    console.warn('Could not find suitable spawn location for boss');
    return state;
  }

  // Create boss city
  const bossCity: City = {
    id: generateId('city'),
    name: 'The Pit',
    owner: 'boss',
    coord: spawnCoord,
    population: 5,
    buildings: [],
    buildQueue: [],
    roster: [],
    occupationTurns: 0,
    isCapital: true,
    growthProgress: 0,
    garrison: createGarrison(true),
  };
  newCities.set(bossCity.id, bossCity);

  // Update map tile ownership
  const mapCopy = state.map.map(row => row.map(tile => ({ ...tile })));
  mapCopy[spawnCoord.y][spawnCoord.x].city = bossCity;
  mapCopy[spawnCoord.y][spawnCoord.x].owner = 'boss';

  // Create boss legion with demons - spawn on adjacent tile (cities and legions are mutually exclusive)
  const demonType = SOLDIER_TYPES['demon'];

  // Find adjacent tile for boss legion
  const adjacentOffsets = [
    { dx: 1, dy: 0 }, { dx: -1, dy: 0 }, { dx: 0, dy: 1 }, { dx: 0, dy: -1 },
    { dx: 1, dy: 1 }, { dx: -1, dy: 1 }, { dx: 1, dy: -1 }, { dx: -1, dy: -1 },
  ];
  let bossLegionLocation: Coord = spawnCoord; // Fallback (shouldn't happen)
  for (const offset of adjacentOffsets) {
    const adjCoord = { x: spawnCoord.x + offset.dx, y: spawnCoord.y + offset.dy };
    if (adjCoord.x < 0 || adjCoord.x >= state.mapWidth || adjCoord.y < 0 || adjCoord.y >= state.mapHeight) continue;
    const adjTile = getTile(state, adjCoord);
    if (adjTile && isPassableTerrain(adjTile) && !getCityAt(state, adjCoord) && !getLegionAt(state, adjCoord)) {
      bossLegionLocation = adjCoord;
      break;
    }
  }

  const bossLegion: Legion = {
    id: generateId('legion'),
    owner: 'boss',
    soldiers: Array.from({ length: 6 }, (_, i) => ({
      id: generateId('soldier'),
      name: `Demon ${i + 1}`,
      type: 'demon' as SoldierTypeId,
      hp: demonType.hp,
      maxHp: demonType.hp,
      position: {
        row: i < 3 ? 'front' : 'mid',
        column: i % 3,
      },
      isLeader: i === 0,
    })),
    location: bossLegionLocation,
    movementRemaining: BASE_MOVEMENT_RANGE,
  };
  newLegions.set(bossLegion.id, bossLegion);

  return {
    ...state,
    factions: newFactions,
    cities: newCities,
    legions: newLegions,
    map: mapCopy,
  };
}

function checkGameOver(state: GameState): GameState {
  const playerCities = getPlayerCities(state);
  const playerLegions = getPlayerLegions(state);
  const playerFaction = state.factions.get('player');

  // Loss: no cities
  if (playerCities.length === 0) {
    return { ...state, gameOver: true, winner: null, phase: 'game_over' };
  }

  // Loss: no legions and can't afford one
  if (playerLegions.length === 0 && playerFaction && playerFaction.gold < LEGION_COST) {
    return { ...state, gameOver: true, winner: null, phase: 'game_over' };
  }

  // Victory: boss spawned and defeated (check if boss exists and has no cities)
  if (state.armageddonCounter >= ARMAGEDDON_MAX) {
    const bossCities = getFactionCities(state, 'boss');
    if (bossCities.length === 0 && state.factions.has('boss')) {
      return { ...state, gameOver: true, winner: 'player', phase: 'game_over' };
    }
  }

  return state;
}

function handleBuild(state: GameState, cityId: string, buildingId: BuildingId): GameState {
  const city = state.cities.get(cityId);
  if (!city || city.owner !== 'player') return state;
  if (city.occupationTurns > 0) return state;

  const building = BUILDING_TYPES[buildingId];
  const faction = state.factions.get('player');
  if (!faction || faction.gold < building.cost) return state;

  const slots = getBuildingSlots(city.population);
  if (city.buildings.length >= slots) return state;
  if (city.buildings.includes(buildingId)) return state;

  const newCities = new Map(state.cities);
  newCities.set(cityId, {
    ...city,
    buildings: [...city.buildings, buildingId],
  });

  const newFactions = new Map(state.factions);
  newFactions.set('player', {
    ...faction,
    gold: faction.gold - building.cost,
  });

  return { ...state, cities: newCities, factions: newFactions };
}

function handleRecruit(
  state: GameState,
  cityId: string,
  soldierTypeId: SoldierTypeId,
  legionId: string
): GameState {
  const city = state.cities.get(cityId);
  const legion = state.legions.get(legionId);
  if (!city || !legion) return state;
  if (city.owner !== 'player' || legion.owner !== 'player') return state;
  if (!coordsEqual(city.coord, legion.location)) return state;
  if (city.occupationTurns > 0) return state;
  if (legion.soldiers.length >= MAX_SOLDIERS_PER_LEGION) return state;

  const soldierType = SOLDIER_TYPES[soldierTypeId];
  const faction = state.factions.get('player');
  if (!faction) return state;
  if (faction.gold < soldierType.cost.gold) return state;
  if (faction.mana < soldierType.cost.mana) return state;

  // Check if city can recruit this type
  const canRecruit = city.buildings.some(b => {
    const building = BUILDING_TYPES[b];
    return building.effects.some(
      e => e.type === 'unlock_soldier' && e.soldier === soldierTypeId
    );
  });
  if (!canRecruit && soldierTypeId !== 'fighter') return state; // Fighters are always available

  // Find open position in formation
  const usedPositions = new Set(
    legion.soldiers.map(s => `${s.position.row}-${s.position.column}`)
  );
  let position: { row: 'front' | 'mid' | 'back'; column: number } | null = null;

  const preferredRow = soldierType.preferredRow;
  const rowOrder: ('front' | 'mid' | 'back')[] =
    preferredRow === 'front'
      ? ['front', 'mid', 'back']
      : ['back', 'mid', 'front'];

  for (const row of rowOrder) {
    for (let col = 0; col < 3; col++) {
      if (!usedPositions.has(`${row}-${col}`)) {
        position = { row, column: col };
        break;
      }
    }
    if (position) break;
  }

  if (!position) return state; // No room

  const recruitedMaxHp = getSoldierMaxHp(state, legion.owner, soldierTypeId);
  const newSoldier: Soldier = {
    id: generateId('soldier'),
    name: generateSoldierName(),
    type: soldierTypeId,
    hp: recruitedMaxHp,
    maxHp: recruitedMaxHp,
    position,
  };

  const newLegions = new Map(state.legions);
  newLegions.set(legionId, {
    ...legion,
    soldiers: [...legion.soldiers, newSoldier],
  });

  const newFactions = new Map(state.factions);
  newFactions.set('player', {
    ...faction,
    gold: faction.gold - soldierType.cost.gold,
    mana: faction.mana - soldierType.cost.mana,
  });

  return { ...state, legions: newLegions, factions: newFactions };
}

function handleCreateLegion(state: GameState, cityId: string): GameState {
  const city = state.cities.get(cityId);
  if (!city || city.owner !== 'player') return state;

  const faction = state.factions.get('player');
  if (!faction || faction.gold < LEGION_COST) return state;

  const playerLegions = getPlayerLegions(state);
  if (playerLegions.length >= MAX_LEGIONS_PER_FACTION) return state;

  // Find empty adjacent tile for new legion (cities and legions are mutually exclusive)
  const spawnLocation = findEmptyAdjacentTile(state, city.coord);
  if (!spawnLocation) return state; // No valid spawn location

  const newLegion: Legion = {
    id: generateId('legion'),
    owner: 'player',
    soldiers: [],
    location: spawnLocation, // Spawn on adjacent tile, not city
    movementRemaining: 0, // Can't move on creation turn
  };

  const newLegions = new Map(state.legions);
  newLegions.set(newLegion.id, newLegion);

  const newFactions = new Map(state.factions);
  newFactions.set('player', {
    ...faction,
    gold: faction.gold - LEGION_COST,
  });

  return { ...state, legions: newLegions, factions: newFactions };
}

function handleQueueBuilding(state: GameState, cityId: string, buildingId: BuildingId): GameState {
  const city = state.cities.get(cityId);
  if (!city || city.owner !== 'player') return state;
  if (city.occupationTurns > 0) return state;

  const building = BUILDING_TYPES[buildingId];
  const faction = state.factions.get('player');
  if (!faction) return state;

  // Check if player has enough gold
  if (faction.gold < building.cost) return state;

  // Check if building already exists
  if (city.buildings.includes(buildingId)) return state;

  // Check if building is already in queue
  if (city.buildQueue.some(item => item.itemType === 'building' && item.itemId === buildingId)) {
    return state;
  }

  // Check building slots (existing + queued buildings)
  const slots = getBuildingSlots(city.population);
  const queuedBuildings = city.buildQueue.filter(item => item.itemType === 'building').length;
  if (city.buildings.length + queuedBuildings >= slots) return state;

  const queueItem: BuildQueueItem = {
    id: generateId('queue'),
    itemType: 'building',
    itemId: buildingId,
    turnsRemaining: building.buildTurns,
    totalTurns: building.buildTurns,
    cost: { gold: building.cost, mana: 0 },
  };

  // Deduct gold immediately
  const newFactions = new Map(state.factions);
  newFactions.set('player', {
    ...faction,
    gold: faction.gold - building.cost,
  });

  const newCities = new Map(state.cities);
  newCities.set(cityId, {
    ...city,
    buildQueue: [...city.buildQueue, queueItem],
  });

  return { ...state, cities: newCities, factions: newFactions };
}

function handleQueueSoldier(
  state: GameState,
  cityId: string,
  soldierTypeId: SoldierTypeId,
  targetLegionId?: string
): GameState {
  const city = state.cities.get(cityId);
  if (!city || city.owner !== 'player') return state;
  if (city.occupationTurns > 0) return state;

  // If targeting a legion, validate it
  if (targetLegionId) {
    const legion = state.legions.get(targetLegionId);
    if (!legion || legion.owner !== 'player') return state;

    // Check if legion is within range of city
    const distance = manhattanDistance(legion.location, city.coord);
    if (distance > CITY_RECRUIT_RANGE) return state;

    // Check legion capacity (current + queued soldiers for this legion)
    const queuedForLegion = city.buildQueue.filter(
      item => item.itemType === 'soldier' && item.targetLegionId === targetLegionId
    ).length;
    if (legion.soldiers.length + queuedForLegion >= MAX_SOLDIERS_PER_LEGION) return state;
  }

  const soldierType = SOLDIER_TYPES[soldierTypeId];
  const faction = state.factions.get('player');
  if (!faction) return state;

  // Check resources
  if (faction.gold < soldierType.cost.gold) return state;
  if (faction.mana < soldierType.cost.mana) return state;

  // Check if city can recruit this type (has the right building)
  const canRecruit = city.buildings.some(b => {
    const building = BUILDING_TYPES[b];
    return building.effects.some(
      e => e.type === 'unlock_soldier' && e.soldier === soldierTypeId
    );
  });
  if (!canRecruit && soldierTypeId !== 'fighter') return state;

  const queueItem: BuildQueueItem = {
    id: generateId('queue'),
    itemType: 'soldier',
    itemId: soldierTypeId,
    turnsRemaining: soldierType.buildTurns,
    totalTurns: soldierType.buildTurns,
    cost: { gold: soldierType.cost.gold, mana: soldierType.cost.mana },
    targetLegionId, // Can be undefined - soldier goes to roster
  };

  // Deduct resources immediately
  const newFactions = new Map(state.factions);
  newFactions.set('player', {
    ...faction,
    gold: faction.gold - soldierType.cost.gold,
    mana: faction.mana - soldierType.cost.mana,
  });

  const newCities = new Map(state.cities);
  newCities.set(cityId, {
    ...city,
    buildQueue: [...city.buildQueue, queueItem],
  });

  return { ...state, cities: newCities, factions: newFactions };
}

function handleCancelQueueItem(state: GameState, cityId: string, queueItemId: string): GameState {
  const city = state.cities.get(cityId);
  if (!city || city.owner !== 'player') return state;

  const queueItem = city.buildQueue.find(item => item.id === queueItemId);
  if (!queueItem) return state;

  const faction = state.factions.get('player');
  if (!faction) return state;

  // Refund partial cost (50% of remaining progress)
  const progressPercent = queueItem.turnsRemaining / queueItem.totalTurns;
  const refundGold = Math.floor(queueItem.cost.gold * progressPercent * 0.5);
  const refundMana = Math.floor(queueItem.cost.mana * progressPercent * 0.5);

  const newFactions = new Map(state.factions);
  newFactions.set('player', {
    ...faction,
    gold: faction.gold + refundGold,
    mana: faction.mana + refundMana,
  });

  const newCities = new Map(state.cities);
  newCities.set(cityId, {
    ...city,
    buildQueue: city.buildQueue.filter(item => item.id !== queueItemId),
  });

  return { ...state, cities: newCities, factions: newFactions };
}

function handleAssignSoldier(
  state: GameState,
  cityId: string,
  soldierId: string,
  legionId: string
): GameState {
  const city = state.cities.get(cityId);
  const legion = state.legions.get(legionId);
  if (!city || !legion) return state;
  if (city.owner !== 'player' || legion.owner !== 'player') return state;

  // Check if legion is within range of city
  const distance = manhattanDistance(legion.location, city.coord);
  if (distance > CITY_RECRUIT_RANGE) return state;

  // Find soldier in roster
  const rosterSoldier = city.roster.find(s => s.id === soldierId);
  if (!rosterSoldier) return state;

  // Check legion capacity
  if (legion.soldiers.length >= MAX_SOLDIERS_PER_LEGION) return state;

  // Find open position in formation
  const soldierType = SOLDIER_TYPES[rosterSoldier.type];
  const usedPositions = new Set(
    legion.soldiers.map(s => `${s.position.row}-${s.position.column}`)
  );
  let position: { row: 'front' | 'mid' | 'back'; column: number } | null = null;

  const preferredRow = soldierType.preferredRow;
  const rowOrder: ('front' | 'mid' | 'back')[] =
    preferredRow === 'front'
      ? ['front', 'mid', 'back']
      : ['back', 'mid', 'front'];

  for (const row of rowOrder) {
    for (let col = 0; col < 3; col++) {
      if (!usedPositions.has(`${row}-${col}`)) {
        position = { row, column: col };
        break;
      }
    }
    if (position) break;
  }

  if (!position) return state; // No room

  // If legion is empty, this soldier becomes the leader
  const isLeader = legion.soldiers.length === 0;

  // Create soldier with position
  const newSoldier: Soldier = {
    id: rosterSoldier.id,
    name: rosterSoldier.name,
    type: rosterSoldier.type,
    hp: rosterSoldier.hp,
    maxHp: rosterSoldier.maxHp,
    position,
    isLeader,
  };

  // Update legion
  const newLegions = new Map(state.legions);
  newLegions.set(legionId, {
    ...legion,
    soldiers: [...legion.soldiers, newSoldier],
  });

  // Remove from roster
  const newCities = new Map(state.cities);
  newCities.set(cityId, {
    ...city,
    roster: city.roster.filter(s => s.id !== soldierId),
  });

  return { ...state, legions: newLegions, cities: newCities };
}

function handleUnassignSoldier(
  state: GameState,
  legionId: string,
  soldierId: string,
  cityId: string
): GameState {
  const legion = state.legions.get(legionId);
  const city = state.cities.get(cityId);
  if (!legion || !city) return state;
  if (legion.owner !== 'player' || city.owner !== 'player') return state;

  // Check if legion is within range of city
  const distance = manhattanDistance(legion.location, city.coord);
  if (distance > CITY_RECRUIT_RANGE) return state;

  // Find soldier in legion
  const soldier = legion.soldiers.find(s => s.id === soldierId);
  if (!soldier) return state;

  // Cannot unassign garrison soldiers
  if (soldier.isGarrison) return state;

  // Create roster soldier (without position)
  const rosterSoldier: RosterSoldier = {
    id: soldier.id,
    name: soldier.name,
    type: soldier.type,
    hp: soldier.hp,
    maxHp: soldier.maxHp,
  };

  // Update city roster
  const newCities = new Map(state.cities);
  newCities.set(cityId, {
    ...city,
    roster: [...city.roster, rosterSoldier],
  });

  // Remove from legion and handle leader succession
  const remainingSoldiers = legion.soldiers.filter(s => s.id !== soldierId);

  // If we removed the leader and there are remaining soldiers, promote a new leader
  if (soldier.isLeader && remainingSoldiers.length > 0) {
    remainingSoldiers[0] = { ...remainingSoldiers[0], isLeader: true };
  }

  const newLegions = new Map(state.legions);
  newLegions.set(legionId, {
    ...legion,
    soldiers: remainingSoldiers,
  });

  return { ...state, legions: newLegions, cities: newCities };
}

function handleCreateLegionFromRoster(
  state: GameState,
  cityId: string,
  soldierId: string
): GameState {
  const city = state.cities.get(cityId);
  if (!city || city.owner !== 'player') return state;

  // Find soldier in roster
  const rosterSoldier = city.roster.find(s => s.id === soldierId);
  if (!rosterSoldier) return state;

  // Find empty adjacent tile for new legion (cities and legions are mutually exclusive)
  const spawnLocation = findEmptyAdjacentTile(state, city.coord);
  if (!spawnLocation) return state; // No valid spawn location

  // Create new legion on adjacent tile
  const newLegionId = generateId('legion');

  // Create the soldier for the legion (as leader)
  const newSoldier: Soldier = {
    id: rosterSoldier.id,
    name: rosterSoldier.name,
    type: rosterSoldier.type,
    hp: rosterSoldier.hp,
    maxHp: rosterSoldier.maxHp,
    position: { row: 'front', column: 1 }, // Default center-front position
    isLeader: true,
  };

  const newLegion: Legion = {
    id: newLegionId,
    owner: 'player',
    soldiers: [newSoldier],
    location: spawnLocation, // Spawn on adjacent tile, not city
    movementRemaining: 0, // New legion can't move this turn
  };

  // Remove soldier from roster
  const newCities = new Map(state.cities);
  newCities.set(cityId, {
    ...city,
    roster: city.roster.filter(s => s.id !== soldierId),
  });

  // Add new legion
  const newLegions = new Map(state.legions);
  newLegions.set(newLegionId, newLegion);

  return { ...state, legions: newLegions, cities: newCities };
}

function handleTransferSoldier(
  state: GameState,
  fromLegionId: string,
  soldierId: string,
  toLegionId: string
): GameState {
  const fromLegion = state.legions.get(fromLegionId);
  const toLegion = state.legions.get(toLegionId);
  if (!fromLegion || !toLegion) return state;
  if (fromLegion.owner !== 'player' || toLegion.owner !== 'player') return state;

  // Check if legions are within range of each other
  const distance = manhattanDistance(fromLegion.location, toLegion.location);
  if (distance > CITY_RECRUIT_RANGE) return state;

  // Find soldier in source legion
  const soldier = fromLegion.soldiers.find(s => s.id === soldierId);
  if (!soldier) return state;

  // Cannot transfer garrison soldiers
  if (soldier.isGarrison) return state;

  // Check target legion capacity
  if (toLegion.soldiers.length >= MAX_SOLDIERS_PER_LEGION) return state;

  // Find open position in target legion
  const soldierType = SOLDIER_TYPES[soldier.type];
  const usedPositions = new Set(
    toLegion.soldiers.map(s => `${s.position.row}-${s.position.column}`)
  );
  let position: { row: 'front' | 'mid' | 'back'; column: number } | null = null;

  const preferredRow = soldierType.preferredRow;
  const rowOrder: ('front' | 'mid' | 'back')[] =
    preferredRow === 'front'
      ? ['front', 'mid', 'back']
      : ['back', 'mid', 'front'];

  for (const row of rowOrder) {
    for (let col = 0; col < 3; col++) {
      if (!usedPositions.has(`${row}-${col}`)) {
        position = { row, column: col };
        break;
      }
    }
    if (position) break;
  }

  if (!position) return state; // No room

  // Create new soldier for target legion (not a leader unless target is empty)
  const isLeader = toLegion.soldiers.length === 0;
  const transferredSoldier: Soldier = {
    id: soldier.id,
    name: soldier.name,
    type: soldier.type,
    hp: soldier.hp,
    maxHp: soldier.maxHp,
    position,
    isLeader,
  };

  // Remove from source legion and handle leader succession
  const remainingSoldiers = fromLegion.soldiers.filter(s => s.id !== soldierId);
  if (soldier.isLeader && remainingSoldiers.length > 0) {
    remainingSoldiers[0] = { ...remainingSoldiers[0], isLeader: true };
  }

  // Update both legions
  const newLegions = new Map(state.legions);
  newLegions.set(fromLegionId, {
    ...fromLegion,
    soldiers: remainingSoldiers,
  });
  newLegions.set(toLegionId, {
    ...toLegion,
    soldiers: [...toLegion.soldiers, transferredSoldier],
  });

  return { ...state, legions: newLegions };
}

// ============ Collegia Handlers ============

function handleSelectResearch(state: GameState, technologyId: string): GameState {
  // Can't select if already researching
  if (state.collegia.currentResearch) return state;

  // Must be in the available offerings
  if (!state.collegia.availableOfferings.includes(technologyId)) return state;

  // Must not already own it
  if (state.collegia.ownedTechnologies.includes(technologyId)) return state;

  const tech = TECHNOLOGIES[technologyId];
  if (!tech) return state;

  return {
    ...state,
    collegia: {
      ...state.collegia,
      currentResearch: {
        technologyId,
        turnsRemaining: tech.researchTurns,
      },
    },
  };
}

function handleRerollCollegia(state: GameState): GameState {
  // Can't reroll if not available
  if (!state.collegia.rerollAvailable) return state;

  // Can't reroll while researching
  if (state.collegia.currentResearch) return state;

  // Check if player can afford it
  const player = state.factions.get('player');
  if (!player || player.gold < COLLEGIA_REROLL_COST) return state;

  // Deduct cost and generate new offerings
  const newFactions = new Map(state.factions);
  newFactions.set('player', {
    ...player,
    gold: player.gold - COLLEGIA_REROLL_COST,
  });

  return {
    ...state,
    factions: newFactions,
    collegia: {
      ...state.collegia,
      availableOfferings: generateCollegiaOfferings(state.collegia.ownedTechnologies),
      rerollAvailable: false,
    },
  };
}

// Process research progress at end of turn
function processCollegiaResearch(state: GameState): GameState {
  if (!state.collegia.currentResearch) return state;

  const newTurnsRemaining = state.collegia.currentResearch.turnsRemaining - 1;

  if (newTurnsRemaining <= 0) {
    // Research complete!
    const completedTechId = state.collegia.currentResearch.technologyId;
    const newOwnedTechnologies = [...state.collegia.ownedTechnologies, completedTechId];

    return {
      ...state,
      collegia: {
        currentResearch: null,
        availableOfferings: generateCollegiaOfferings(newOwnedTechnologies),
        ownedTechnologies: newOwnedTechnologies,
        rerollAvailable: true,
      },
    };
  }

  // Still researching
  return {
    ...state,
    collegia: {
      ...state.collegia,
      currentResearch: {
        ...state.collegia.currentResearch,
        turnsRemaining: newTurnsRemaining,
      },
    },
  };
}

// ============ Technology Bonus Helpers ============

// Get total bonus of a specific type from all owned technologies
export function getTechnologyBonus(
  state: GameState,
  effectType: string,
  filterKey?: string,
  filterValue?: string
): number {
  let bonus = 0;
  for (const techId of state.collegia.ownedTechnologies) {
    const tech = TECHNOLOGIES[techId];
    if (!tech) continue;
    for (const effect of tech.effects) {
      if (effect.type === effectType) {
        // Check filter if provided
        if (filterKey && filterValue) {
          if ((effect as Record<string, unknown>)[filterKey] !== filterValue) continue;
        }
        if ('amount' in effect) {
          bonus += effect.amount;
        }
      }
    }
  }
  return bonus;
}

// Check if a building is unlocked by technology
export function isBuildingUnlockedByTech(state: GameState, buildingId: BuildingId): boolean {
  for (const techId of state.collegia.ownedTechnologies) {
    const tech = TECHNOLOGIES[techId];
    if (!tech) continue;
    for (const effect of tech.effects) {
      if (effect.type === 'unlock_building' && effect.building === buildingId) {
        return true;
      }
    }
  }
  return false;
}

// Get all buildings unlocked by technology
export function getUnlockedBuildings(state: GameState): BuildingId[] {
  const unlocked: BuildingId[] = [];
  for (const techId of state.collegia.ownedTechnologies) {
    const tech = TECHNOLOGIES[techId];
    if (!tech) continue;
    for (const effect of tech.effects) {
      if (effect.type === 'unlock_building' && !unlocked.includes(effect.building)) {
        unlocked.push(effect.building);
      }
    }
  }
  return unlocked;
}
