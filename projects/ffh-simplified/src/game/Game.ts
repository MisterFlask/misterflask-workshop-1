import type {
  GameState,
  GameAction,
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
  Technology,
} from '../types';
import { SOLDIER_TYPES } from '../data/soldiers';
import { BUILDING_TYPES, getBuildingSlots, BASE_CITY_INCOME, TURNS_PER_POPULATION } from '../data/buildings';
import { FACTION_TEMPLATES, createFaction } from '../data/factions';
import { TECHNOLOGIES, getAllTechnologyIds } from '../data/technologies';
import { generateMap, placeStartingEntities, updateCulturalBorders } from './MapGenerator';
import { resolveCombat, applyCombatResult, SoldierTechBonuses } from './Combat';
import { coordToKey, coordsEqual, getTilesInRange, findPath, getNeighbors, manhattanDistance } from '../utils/grid';
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
const ARMAGEDDON_MAX = 100;
const BOSS_SPAWN_THRESHOLD = 50; // Boss spawns at 50% Armageddon
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
  totalGold: number;
  totalMana: number;
  goldSources: { name: string; amount: number }[];
  manaSources: { name: string; amount: number }[];
}

export function getCityIncome(city: City): CityIncomeBreakdown {
  const baseGold = city.occupationTurns > 0 ? Math.floor(BASE_CITY_INCOME / 2) : BASE_CITY_INCOME;
  let buildingGold = 0;
  let buildingMana = 0;
  const goldSources: { name: string; amount: number }[] = [];
  const manaSources: { name: string; amount: number }[] = [];

  if (city.occupationTurns === 0) {
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
  }

  return {
    baseGold,
    buildingGold,
    buildingMana,
    totalGold: baseGold + buildingGold,
    totalMana: buildingMana,
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
    const income = getCityIncome(city);
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

export function getCityGrowthInfo(city: City): CityGrowthInfo {
  let growthBonus = 0;
  let hasGranary = false;

  for (const buildingId of city.buildings) {
    const building = BUILDING_TYPES[buildingId];
    for (const effect of building.effects) {
      if (effect.type === 'growth_bonus') {
        growthBonus += effect.amount;
        if (buildingId === 'granary') hasGranary = true;
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
export function getCityDefenseBonus(city: City): number {
  let bonus = 0;
  for (const buildingId of city.buildings) {
    const building = BUILDING_TYPES[buildingId];
    for (const effect of building.effects) {
      if (effect.type === 'defense_bonus') {
        bonus += effect.amount;
      }
    }
  }
  return bonus;
}

function isPassableTerrain(tile: Tile): boolean {
  return tile.terrain !== 'mountain' && tile.terrain !== 'water';
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

  return getTilesInRange(
    legion.location,
    legion.movementRemaining,
    state.map,
    (tile) => {
      if (!isPassableTerrain(tile)) return false;
      // Can move to tiles with enemy legions (triggers combat)
      // Cannot move to tiles with friendly legions
      const legionAtTile = getLegionAt(state, tile.coord);
      if (legionAtTile && legionAtTile.owner === legion.owner) return false;
      return true;
    }
  );
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

  // Check for combat
  const enemyLegion = getLegionAt(state, to);
  if (enemyLegion && enemyLegion.owner !== legion.owner) {
    return handleCombat(state, legion, enemyLegion, to);
  }

  // Check for city capture
  const city = getCityAt(state, to);
  if (city && city.owner !== legion.owner) {
    return handleCityCapture(state, legion, city, to);
  }

  // Simple move
  const newLegions = new Map(state.legions);
  const path = findPath(legion.location, to, state.map, isPassableTerrain);
  const distance = path ? path.length - 1 : 1;

  newLegions.set(legionId, {
    ...legion,
    location: to,
    movementRemaining: Math.max(0, legion.movementRemaining - distance),
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

function handleApplyCombatResults(state: GameState): GameState {
  if (!state.pendingCombat) return state;

  const { attackerId, defenderId, combatLocation, result } = state.pendingCombat;
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

    // Check for city capture
    const capturedCity = getCityAt(state, combatLocation);
    if (capturedCity && capturedCity.owner !== attacker.owner) {
      const newCities = new Map(state.cities);
      newCities.set(capturedCity.id, {
        ...capturedCity,
        owner: attacker.owner,
        occupationTurns: 3,
      });
      return { ...state, legions: newLegions, cities: newCities, pendingCombat: null, phase: 'player_turn' };
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

  return { ...state, legions: newLegions, pendingCombat: null, phase: 'player_turn' };
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

function processAITurn(state: GameState): GameState {
  let newState = { ...state };
  const newLegions = new Map(newState.legions);

  // Get all AI factions
  const aiFactions = Array.from(newState.factions.values()).filter(f => f.id !== 'player');

  for (const faction of aiFactions) {
    // Reset movement for this faction's legions
    for (const [id, legion] of newLegions) {
      if (legion.owner === faction.id) {
        newLegions.set(id, { ...legion, movementRemaining: BASE_MOVEMENT_RANGE });
      }
    }
    newState = { ...newState, legions: newLegions };

    // Process each legion for this faction
    const factionLegions = Array.from(newLegions.values()).filter(l => l.owner === faction.id);

    for (const legion of factionLegions) {
      // Skip if legion has no soldiers
      if (legion.soldiers.length === 0) continue;

      // Find targets based on faction type
      const targets = findAITargets(newState, faction, legion);

      if (targets.length > 0) {
        // Move toward nearest target
        newState = processAILegionMove(newState, legion, targets[0]);
      }
    }
  }

  return newState;
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

    // Check for enemy city at target
    const enemyCity = getCityAt(newState, target);
    if (enemyCity && enemyCity.owner !== currentLegion.owner) {
      // Check if city has a defending legion
      const defender = getLegionAt(newState, target);
      if (defender) {
        return handleAICombat(newState, currentLegion, defender, target);
      } else {
        // Capture undefended city
        return handleAICaptureCity(newState, currentLegion, enemyCity);
      }
    }

    // Move to empty tile
    return moveAILegion(newState, currentLegion, target);
  }

  // Find path to target
  const path = findPath(currentLegion.location, target, newState.map, isPassableTerrain);
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

  // AI doesn't have tech bonuses (only player has Collegia)
  const result = resolveCombat(attacker, defender, tile.terrain, hasWalls);

  // Apply combat results immediately for AI (no combat scene)
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

// Handle AI capturing an undefended city
function handleAICaptureCity(state: GameState, legion: Legion, city: City): GameState {
  const newLegions = new Map(state.legions);
  newLegions.set(legion.id, {
    ...legion,
    location: city.coord,
    movementRemaining: 0,
  });

  const newCities = new Map(state.cities);
  newCities.set(city.id, {
    ...city,
    owner: legion.owner,
    occupationTurns: 3,
  });

  return { ...state, legions: newLegions, cities: newCities };
}

// ============ End AI Processing ============

function handleEndTurn(state: GameState): GameState {
  // Process AI turns, then end turn phase
  let newState = { ...state, phase: 'ai_turn' as const };

  // Process AI faction turns
  newState = processAITurn(newState);

  // End turn processing
  newState = processEndTurnPhase(newState);

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
  };
  newCities.set(bossCity.id, bossCity);

  // Update map tile ownership
  const mapCopy = state.map.map(row => row.map(tile => ({ ...tile })));
  mapCopy[spawnCoord.y][spawnCoord.x].city = bossCity;
  mapCopy[spawnCoord.y][spawnCoord.x].owner = 'boss';

  // Create boss legion with demons
  const demonType = SOLDIER_TYPES['demon'];
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
    location: spawnCoord,
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

  // Check if city tile is empty
  if (getLegionAt(state, city.coord)) return state;

  const newLegion: Legion = {
    id: generateId('legion'),
    owner: 'player',
    soldiers: [],
    location: city.coord,
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

  // Create new legion at city location
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
    location: city.coord,
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
