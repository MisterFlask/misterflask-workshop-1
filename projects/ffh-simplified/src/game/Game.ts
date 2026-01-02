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
} from '../types';
import { SOLDIER_TYPES } from '../data/soldiers';
import { BUILDING_TYPES, getBuildingSlots, BASE_CITY_INCOME, TURNS_PER_POPULATION } from '../data/buildings';
import { FACTION_TEMPLATES, createFaction } from '../data/factions';
import { generateMap, placeStartingEntities } from './MapGenerator';
import { resolveCombat, applyCombatResult } from './Combat';
import { coordToKey, coordsEqual, getTilesInRange, findPath } from '../utils/grid';
import { generateId } from '../utils/random';

const MOVEMENT_RANGE = 3;
const HEAL_RATE = 20;
const LEGION_COST = 100;
const MAX_LEGIONS_PER_FACTION = 5;
const MAX_SOLDIERS_PER_LEGION = 8;
const ARMAGEDDON_MAX = 100;

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

function isPassableTerrain(tile: Tile): boolean {
  return tile.terrain !== 'mountain' && tile.terrain !== 'water';
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

  const result = resolveCombat(attacker, defender, tile.terrain, hasWalls);

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
      // TODO: Implement retreat logic
      // For now, just destroy the legion
      newLegions.delete(defender.id);
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

function handleEndTurn(state: GameState): GameState {
  // Process AI turns, then end turn phase
  let newState = { ...state, phase: 'ai_turn' as const };

  // TODO: Process AI faction turns

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

    // Reset player legion movement
    const newLegions = new Map(newState.legions);
    for (const [id, legion] of newLegions) {
      if (legion.owner === 'player') {
        newLegions.set(id, { ...legion, movementRemaining: MOVEMENT_RANGE });
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
        }
      }
    }

    newFactions.set(factionId, {
      ...faction,
      gold: faction.gold + goldIncome,
      mana: faction.mana + manaIncome,
    });
  }

  // Population growth and occupation countdown
  for (const [cityId, city] of newCities) {
    let newCity = { ...city };

    // Occupation countdown
    if (newCity.occupationTurns > 0) {
      newCity.occupationTurns--;
    }

    // Population growth (every TURNS_PER_POPULATION turns)
    if (state.turn % TURNS_PER_POPULATION === 0) {
      newCity.population++;
    }

    newCities.set(cityId, newCity);
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

  return {
    ...state,
    factions: newFactions,
    cities: newCities,
    legions: newLegions,
    armageddonCounter: newArmageddon,
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

  const newSoldier: Soldier = {
    id: generateId('soldier'),
    type: soldierTypeId,
    hp: soldierType.hp,
    maxHp: soldierType.hp,
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
