import type { Tile, TerrainType, City, Legion, FactionId, Coord, Soldier } from '../types';
import { createRNG, randomInt, randomElement, generateId } from '../utils/random';
import { SOLDIER_TYPES } from '../data/soldiers';

const TERRAIN_WEIGHTS: Record<TerrainType, number> = {
  grass: 50,
  forest: 25,
  hills: 15,
  mountain: 7,
  water: 3,
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

  return map;
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

    // Legion starts at city
    positions.push({
      cityCoord,
      legionCoord: cityCoord,
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

  for (const { type, row, col } of composition) {
    const soldierType = SOLDIER_TYPES[type];
    soldiers.push({
      id: generateId('soldier'),
      type,
      hp: soldierType.hp,
      maxHp: soldierType.hp,
      position: { row, column: col },
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
  isCapital: boolean
): City {
  return {
    id: generateId('city'),
    name: `${factionId} ${isCapital ? 'Capital' : 'Outpost'}`,
    owner: factionId,
    coord,
    population: isCapital ? 3 : 1,
    buildings: isCapital ? ['barracks', 'market'] : [],
    occupationTurns: 0,
  };
}

export function placeStartingEntities(
  map: Tile[][],
  factions: FactionId[],
  cities: Map<string, City>,
  legions: Map<string, Legion>,
  seed: number
): void {
  const rng = createRNG(seed + 1000);
  const positions = findStartPositions(map, factions.length, rng);

  for (let i = 0; i < factions.length; i++) {
    const factionId = factions[i];
    const pos = positions[i];

    // Create capital city
    const city = createStartingCity(factionId, pos.cityCoord, true);
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

      // Create neutral city
      const city: City = {
        id: generateId('city'),
        name: `Neutral Town ${i + 1}`,
        owner: 'player', // Will be set to neutral/unclaimed
        coord: { x, y },
        population: 2,
        buildings: [],
        occupationTurns: 0,
      };
      // Actually mark as unclaimed by not setting owner
      // For now, leave as neutral (could add a 'neutral' faction)
      cities.set(city.id, city);
      break;
    }
  }
}
