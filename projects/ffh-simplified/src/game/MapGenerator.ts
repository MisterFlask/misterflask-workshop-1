import type { Tile, TerrainType, City, Legion, FactionId, Coord, Soldier, TerrainFeatureId, GameState } from '../types';
import { createRNG, randomInt, randomElement, generateId, generateSoldierName, generateCityNameSeeded, resetCityNames } from '../utils/random';
import { SOLDIER_TYPES } from '../data/soldiers';
import { TERRAIN_FEATURES, getFeaturesForTerrain } from '../data/terrainFeatures';
import { createGarrison } from './Game';

const TERRAIN_WEIGHTS: Record<TerrainType, number> = {
  grass: 45,
  forest: 23,
  hills: 14,
  mountain: 7,
  water: 3,
  swamp: 8,
};

function pickTerrain(rng: () => number): TerrainType {
  const total = Object.values(TERRAIN_WEIGHTS).reduce((a, b) => a + b, 0);
  let roll = rng() * total;

  for (const [terrain, weight] of Object.entries(TERRAIN_WEIGHTS)) {
    roll -= weight;
    if (roll <= 0) return terrain as TerrainType;
  }

  return 'grass';
}

export function generateMap(width: number, height: number, seed: number): Tile[][] {
  const rng = createRNG(seed);
  const map: Tile[][] = [];

  for (let y = 0; y < height; y++) {
    const row: Tile[] = [];
    for (let x = 0; x < width; x++) {
      row.push({
        coord: { x, y },
        terrain: pickTerrain(rng),
      });
    }
    map.push(row);
  }

  // Smooth terrain a bit - make clusters
  for (let pass = 0; pass < 2; pass++) {
    for (let y = 1; y < height - 1; y++) {
      for (let x = 1; x < width - 1; x++) {
        if (rng() < 0.3) {
          // Copy neighbor's terrain sometimes
          const neighbors = [
            map[y - 1][x],
            map[y + 1][x],
            map[y][x - 1],
            map[y][x + 1],
          ];
          const neighbor = randomElement(rng, neighbors);
          if (neighbor.terrain !== 'mountain' && neighbor.terrain !== 'water') {
            map[y][x].terrain = neighbor.terrain;
          }
        }
      }
    }
  }

  // Place terrain features
  placeTerrainFeatures(map, rng);

  return map;
}

// Place terrain features on the map
function placeTerrainFeatures(map: Tile[][], rng: () => number): void {
  const width = map[0].length;
  const height = map.length;

  // Feature density settings
  const COMMON_CHANCE = 0.03;      // 3% chance per tile
  const UNCOMMON_CHANCE = 0.015;   // 1.5% chance per tile
  const RARE_CHANCE = 0.005;       // 0.5% chance per tile
  const LEGENDARY_CHANCE = 0.001;  // 0.1% chance per tile (very rare)

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const tile = map[y][x];

      // Skip impassable terrain (but allow mountains for some features)
      if (tile.terrain === 'water') continue;

      // Get valid features for this terrain
      const validFeatures = getFeaturesForTerrain(tile.terrain);
      if (validFeatures.length === 0) continue;

      // Roll for feature placement by rarity
      const roll = rng();
      let targetRarity: 'common' | 'uncommon' | 'rare' | 'legendary' | null = null;

      if (roll < LEGENDARY_CHANCE) {
        targetRarity = 'legendary';
      } else if (roll < RARE_CHANCE) {
        targetRarity = 'rare';
      } else if (roll < UNCOMMON_CHANCE) {
        targetRarity = 'uncommon';
      } else if (roll < COMMON_CHANCE) {
        targetRarity = 'common';
      }

      if (targetRarity) {
        const candidates = validFeatures.filter(f => f.rarity === targetRarity);
        if (candidates.length > 0) {
          const feature = randomElement(rng, candidates);
          tile.feature = feature.id;
        }
      }
    }
  }
}

// Calculate cultural borders radius based on city population
export function getCityBorderRadius(population: number): number {
  // Base radius of 2, +1 for every 2 population
  return 2 + Math.floor(population / 2);
}

// Update cultural borders for all cities on the map
export function updateCulturalBorders(map: Tile[][], cities: Map<string, City>): void {
  const width = map[0].length;
  const height = map.length;

  // Clear all existing ownership (except tiles with cities)
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (!map[y][x].city) {
        map[y][x].owner = undefined;
      }
    }
  }

  // For each city, claim tiles within its border radius
  // Process cities by population (larger cities have priority)
  const sortedCities = Array.from(cities.values()).sort((a, b) => b.population - a.population);

  for (const city of sortedCities) {
    const radius = getCityBorderRadius(city.population);
    const centerX = city.coord.x;
    const centerY = city.coord.y;

    for (let dy = -radius; dy <= radius; dy++) {
      for (let dx = -radius; dx <= radius; dx++) {
        const x = centerX + dx;
        const y = centerY + dy;

        // Check bounds
        if (x < 0 || x >= width || y < 0 || y >= height) continue;

        // Calculate distance (using Manhattan for simplicity, or Euclidean for rounder borders)
        const distance = Math.sqrt(dx * dx + dy * dy);
        if (distance > radius) continue;

        const tile = map[y][x];

        // Skip impassable terrain for border expansion
        if (tile.terrain === 'water' || tile.terrain === 'mountain') continue;

        // Only claim if unclaimed or if this city is closer
        // (For simplicity, first city to claim wins due to population sorting)
        if (!tile.owner) {
          tile.owner = city.owner;
        }
      }
    }

    // Always set the city tile to be owned
    map[city.coord.y][city.coord.x].owner = city.owner;
  }
}

interface StartPosition {
  cityCoord: Coord;
  legionCoord: Coord;
}

function findStartPositions(
  map: Tile[][],
  numFactions: number,
  rng: () => number
): StartPosition[] {
  const width = map[0].length;
  const height = map.length;
  const positions: StartPosition[] = [];

  // Define starting positions - hippus starts close to player for testing
  const startZones: Coord[] = [
    { x: 5, y: 5 },                           // player: near top-left
    { x: 10, y: 5 },                          // hippus: close to player for testing
    { x: 3, y: height - 4 },                  // bottom-left
    { x: width - 4, y: height - 4 },          // bottom-right
    { x: Math.floor(width / 2), y: 3 },       // top-center
    { x: Math.floor(width / 2), y: height - 4 }, // bottom-center
  ];

  for (let i = 0; i < numFactions && i < startZones.length; i++) {
    const zone = startZones[i];

    // Find a valid city location near this zone
    let cityCoord: Coord | null = null;
    for (let attempt = 0; attempt < 20; attempt++) {
      const x = zone.x + randomInt(rng, -2, 2);
      const y = zone.y + randomInt(rng, -2, 2);

      if (x < 0 || x >= width || y < 0 || y >= height) continue;

      const tile = map[y][x];
      if (tile.terrain !== 'mountain' && tile.terrain !== 'water') {
        cityCoord = { x, y };
        break;
      }
    }

    if (!cityCoord) {
      cityCoord = zone; // Fallback
      map[zone.y][zone.x].terrain = 'grass'; // Force valid terrain
    }

    // Legion starts adjacent to city (cities and legions are mutually exclusive on tiles)
    // Find a valid adjacent tile for the legion
    let legionCoord = cityCoord; // Default fallback
    const adjacentOffsets = [
      { dx: 1, dy: 0 }, { dx: -1, dy: 0 }, { dx: 0, dy: 1 }, { dx: 0, dy: -1 },
      { dx: 1, dy: 1 }, { dx: -1, dy: 1 }, { dx: 1, dy: -1 }, { dx: -1, dy: -1 },
    ];
    for (const offset of adjacentOffsets) {
      const adjX = cityCoord.x + offset.dx;
      const adjY = cityCoord.y + offset.dy;
      if (adjX < 0 || adjX >= width || adjY < 0 || adjY >= height) continue;
      const adjTile = map[adjY][adjX];
      if (adjTile.terrain !== 'mountain' && adjTile.terrain !== 'water') {
        legionCoord = { x: adjX, y: adjY };
        break;
      }
    }

    positions.push({
      cityCoord,
      legionCoord,
    });
  }

  return positions;
}

function createStartingLegion(factionId: FactionId, location: Coord): Legion {
  const soldiers: Soldier[] = [];

  // Different compositions for different factions
  let composition: { type: keyof typeof SOLDIER_TYPES; row: 'front' | 'mid' | 'back'; col: number }[];

  if (factionId === 'player') {
    composition = [
      { type: 'fighter', row: 'front', col: 0 },
      { type: 'fighter', row: 'front', col: 1 },
      { type: 'archer', row: 'back', col: 1 },
      { type: 'cleric', row: 'back', col: 2 },
    ];
  } else if (factionId === 'hippus') {
    composition = [
      { type: 'knight', row: 'front', col: 0 },
      { type: 'knight', row: 'front', col: 1 },
      { type: 'archer', row: 'back', col: 1 },
    ];
  } else if (factionId === 'sheaim') {
    composition = [
      { type: 'fighter', row: 'front', col: 1 },
      { type: 'mage', row: 'back', col: 0 },
      { type: 'mage', row: 'back', col: 2 },
    ];
  } else {
    composition = [
      { type: 'fighter', row: 'front', col: 1 },
      { type: 'archer', row: 'back', col: 1 },
    ];
  }

  for (let i = 0; i < composition.length; i++) {
    const { type, row, col } = composition[i];
    const soldierType = SOLDIER_TYPES[type];
    soldiers.push({
      id: generateId('soldier'),
      name: generateSoldierName(),
      type,
      hp: soldierType.hp,
      maxHp: soldierType.hp,
      position: { row, column: col },
      isLeader: i === 0, // First soldier is the leader
    });
  }

  return {
    id: generateId('legion'),
    owner: factionId,
    soldiers,
    location,
    movementRemaining: 3,
  };
}

function createStartingCity(
  factionId: FactionId,
  coord: Coord,
  isCapital: boolean,
  rng: () => number
): City {
  const cityName = generateCityNameSeeded(rng);
  return {
    id: generateId('city'),
    name: cityName,
    owner: factionId,
    coord,
    population: isCapital ? 3 : 1,
    buildings: isCapital ? ['barracks', 'market'] : [],
    buildQueue: [],
    roster: [],
    occupationTurns: 0,
    isCapital,
    growthProgress: 0,
    garrison: createGarrison(true), // Full HP garrison
  };
}

export function placeStartingEntities(
  map: Tile[][],
  factions: FactionId[],
  cities: Map<string, City>,
  legions: Map<string, Legion>,
  seed: number
): void {
  // Reset city names for a fresh game
  resetCityNames();

  const rng = createRNG(seed + 1000);
  const positions = findStartPositions(map, factions.length, rng);

  for (let i = 0; i < factions.length; i++) {
    const factionId = factions[i];
    const pos = positions[i];

    // Create capital city
    const city = createStartingCity(factionId, pos.cityCoord, true, rng);
    cities.set(city.id, city);

    // Create starting legion
    const legion = createStartingLegion(factionId, pos.legionCoord);
    legions.set(legion.id, legion);

    // Update tile owner
    map[pos.cityCoord.y][pos.cityCoord.x].owner = factionId;
  }

  // Add some neutral cities
  const width = map[0].length;
  const height = map.length;
  const numNeutralCities = 4;

  for (let i = 0; i < numNeutralCities; i++) {
    for (let attempt = 0; attempt < 30; attempt++) {
      const x = randomInt(rng, 5, width - 6);
      const y = randomInt(rng, 5, height - 6);

      const tile = map[y][x];
      if (tile.terrain === 'mountain' || tile.terrain === 'water') continue;

      // Check distance from other cities
      let tooClose = false;
      for (const city of cities.values()) {
        const dist = Math.abs(city.coord.x - x) + Math.abs(city.coord.y - y);
        if (dist < 5) {
          tooClose = true;
          break;
        }
      }
      if (tooClose) continue;

      // Create neutral city with a proper name
      const cityName = generateCityNameSeeded(rng);
      const city: City = {
        id: generateId('city'),
        name: cityName,
        owner: 'player', // Will be set to neutral/unclaimed
        coord: { x, y },
        population: 2,
        buildings: [],
        buildQueue: [],
        roster: [],
        occupationTurns: 0,
        isCapital: false,
        growthProgress: 0,
        garrison: createGarrison(true), // Full HP garrison
      };
      // Actually mark as unclaimed by not setting owner
      // For now, leave as neutral (could add a 'neutral' faction)
      cities.set(city.id, city);
      break;
    }
  }

  // Calculate initial cultural borders
  updateCulturalBorders(map, cities);
}
