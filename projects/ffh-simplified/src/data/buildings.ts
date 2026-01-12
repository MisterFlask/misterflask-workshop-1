import type { BuildingType, BuildingId } from '../types';

export const BUILDING_TYPES: Record<BuildingId, BuildingType> = {
  // ============ TIER 1 (Available at game start) ============
  barracks: {
    id: 'barracks',
    name: 'Barracks',
    cost: 100,
    buildTurns: 2,
    effects: [
      { type: 'unlock_soldier', soldier: 'fighter' },
    ],
    sprite: 'buildings/barracks.png',
  },
  market: {
    id: 'market',
    name: 'Market',
    cost: 80,
    buildTurns: 2,
    effects: [{ type: 'gold_per_turn', amount: 10 }],
    sprite: 'buildings/market.png',
  },
  temple: {
    id: 'temple',
    name: 'Temple',
    cost: 100,
    buildTurns: 2,
    effects: [
      { type: 'unlock_soldier', soldier: 'cleric' },
      { type: 'mana_per_turn', amount: 3 },
    ],
    sprite: 'buildings/temple.png',
  },
  granary: {
    id: 'granary',
    name: 'Granary',
    cost: 60,
    buildTurns: 2,
    effects: [{ type: 'growth_bonus', amount: 1 }],
    sprite: 'buildings/granary.png',
  },
  walls: {
    id: 'walls',
    name: 'Walls',
    cost: 150,
    buildTurns: 3,
    effects: [{ type: 'defense_bonus', amount: 25 }],
    sprite: 'buildings/walls.png',
  },

  // ============ TIER 2 (Require Pop 3+) ============
  archery_range: {
    id: 'archery_range',
    name: 'Archery Range',
    cost: 80,
    buildTurns: 3,
    effects: [
      { type: 'unlock_soldier', soldier: 'archer' },
    ],
    sprite: 'buildings/archery_range.png',
  },
  war_academy: {
    id: 'war_academy',
    name: 'War Academy',
    cost: 150,
    buildTurns: 4,
    effects: [
      { type: 'unlock_soldier', soldier: 'knight' },
    ],
    sprite: 'buildings/war_academy.png',
  },
  mage_tower: {
    id: 'mage_tower',
    name: 'Mage Tower',
    cost: 150,
    buildTurns: 4,
    effects: [
      { type: 'unlock_soldier', soldier: 'mage' },
      { type: 'mana_per_turn', amount: 5 },
    ],
    sprite: 'buildings/mage_tower.png',
  },
  stables: {
    id: 'stables',
    name: 'Stables',
    cost: 140,
    buildTurns: 3,
    effects: [
      { type: 'unlock_soldier', soldier: 'cavalry' },
    ],
    sprite: 'buildings/stables.png',
  },
  trade_hall: {
    id: 'trade_hall',
    name: 'Trade Hall',
    cost: 120,
    buildTurns: 3,
    effects: [
      { type: 'gold_per_turn', amount: 15 },
    ],
    sprite: 'buildings/trade_hall.png',
  },
  fortifications: {
    id: 'fortifications',
    name: 'Fortifications',
    cost: 180,
    buildTurns: 4,
    effects: [
      { type: 'defense_bonus', amount: 30 },
    ],
    sprite: 'buildings/fortifications.png',
  },
  siege_workshop: {
    id: 'siege_workshop',
    name: 'Siege Workshop',
    cost: 150,
    buildTurns: 4,
    effects: [
      { type: 'unlock_soldier', soldier: 'catapult' },
    ],
    sprite: 'buildings/siege_workshop.png',
  },

  // ============ TIER 3 (Require Pop 5+) ============
  elite_barracks: {
    id: 'elite_barracks',
    name: 'Elite Barracks',
    cost: 200,
    buildTurns: 5,
    effects: [
      { type: 'unlock_soldier', soldier: 'champion' },
    ],
    sprite: 'buildings/elite_barracks.png',
  },
  arcane_sanctum: {
    id: 'arcane_sanctum',
    name: 'Arcane Sanctum',
    cost: 250,
    buildTurns: 6,
    effects: [
      { type: 'mana_per_turn', amount: 5 },
    ],
    sprite: 'buildings/arcane_sanctum.png',
  },
  cathedral: {
    id: 'cathedral',
    name: 'Cathedral',
    cost: 250,
    buildTurns: 6,
    effects: [
      { type: 'unlock_soldier', soldier: 'paladin' },
      { type: 'mana_per_turn', amount: 3 },
    ],
    sprite: 'buildings/cathedral.png',
  },
  treasury: {
    id: 'treasury',
    name: 'Treasury',
    cost: 200,
    buildTurns: 5,
    effects: [
      { type: 'gold_per_turn', amount: 20 },
    ],
    sprite: 'buildings/treasury.png',
  },
  grand_walls: {
    id: 'grand_walls',
    name: 'Grand Walls',
    cost: 250,
    buildTurns: 6,
    effects: [
      { type: 'defense_bonus', amount: 40 },
    ],
    sprite: 'buildings/grand_walls.png',
  },
  masons_guild: {
    id: 'masons_guild',
    name: "Mason's Guild",
    cost: 180,
    buildTurns: 5,
    effects: [], // Special: -20% build time - handled in code
    sprite: 'buildings/masons_guild.png',
  },

  // ============ TIER 4 (Require Pop 7+) ============
  legendary_forge: {
    id: 'legendary_forge',
    name: 'Legendary Forge',
    cost: 350,
    buildTurns: 8,
    effects: [], // Special: Craft hero equipment
    sprite: 'buildings/legendary_forge.png',
  },
  signal_towers: {
    id: 'signal_towers',
    name: 'Signal Towers',
    cost: 200,
    buildTurns: 7,
    effects: [], // Special: See enemy movement in territory
    sprite: 'buildings/signal_towers.png',
  },

  // ============ SPECIAL ============
  ritual_site: {
    id: 'ritual_site',
    name: 'Ritual Site',
    cost: 200,
    buildTurns: 5,
    effects: [], // Special building for Sheaim
    sprite: 'buildings/ritual_site.png',
  },

  // ============ EXPLOITATION BUILDINGS (Require Terrain Improvements) ============
  mine: {
    id: 'mine',
    name: 'Mine',
    cost: 80,
    buildTurns: 3,
    effects: [
      { type: 'gold_per_turn', amount: 5 }, // Base bonus, actual bonus depends on resource
    ],
    sprite: 'buildings/mine.png',
  },
  mana_well: {
    id: 'mana_well',
    name: 'Mana Well',
    cost: 150,
    buildTurns: 4,
    effects: [
      { type: 'mana_per_turn', amount: 3 },
      { type: 'unlock_soldier', soldier: 'fire_elemental' },
      { type: 'unlock_soldier', soldier: 'ice_elemental' },
    ],
    sprite: 'buildings/mana_well.png',
  },
  archive: {
    id: 'archive',
    name: 'Archive',
    cost: 120,
    buildTurns: 4,
    effects: [
      { type: 'gold_per_turn', amount: 10 },
      { type: 'unlock_soldier', soldier: 'sage' },
    ],
    sprite: 'buildings/archive.png',
  },
  deep_mine: {
    id: 'deep_mine',
    name: 'Deep Mine',
    cost: 150,
    buildTurns: 5,
    effects: [
      { type: 'gold_per_turn', amount: 8 },
    ],
    sprite: 'buildings/deep_mine.png',
  },
  crystal_sanctum: {
    id: 'crystal_sanctum',
    name: 'Crystal Sanctum',
    cost: 250,
    buildTurns: 6,
    effects: [
      { type: 'mana_per_turn', amount: 5 },
      { type: 'unlock_soldier', soldier: 'archmage' },
    ],
    sprite: 'buildings/crystal_sanctum.png',
  },
  necropolis: {
    id: 'necropolis',
    name: 'Necropolis',
    cost: 250,
    buildTurns: 6,
    effects: [
      { type: 'mana_per_turn', amount: 4 },
      { type: 'unlock_soldier', soldier: 'lich' },
    ],
    sprite: 'buildings/necropolis.png',
  },
  dragon_shrine: {
    id: 'dragon_shrine',
    name: 'Dragon Shrine',
    cost: 200,
    buildTurns: 5,
    effects: [
      { type: 'mana_per_turn', amount: 3 },
      { type: 'unlock_soldier', soldier: 'dragon_knight' },
    ],
    sprite: 'buildings/dragon_shrine.png',
  },
  master_forge: {
    id: 'master_forge',
    name: 'Master Forge',
    cost: 300,
    buildTurns: 7,
    effects: [
      { type: 'gold_per_turn', amount: 5 },
      { type: 'unlock_soldier', soldier: 'siege_titan' },
    ],
    sprite: 'buildings/master_forge.png',
  },
  grove_of_ages: {
    id: 'grove_of_ages',
    name: 'Grove of Ages',
    cost: 400,
    buildTurns: 8,
    effects: [
      { type: 'mana_per_turn', amount: 5 },
      { type: 'unlock_soldier', soldier: 'treant' },
    ],
    sprite: 'buildings/grove_of_ages.png',
  },
  binding_circle: {
    id: 'binding_circle',
    name: 'Binding Circle',
    cost: 400,
    buildTurns: 8,
    effects: [
      { type: 'mana_per_turn', amount: 8 },
      { type: 'unlock_soldier', soldier: 'summoned_demon' },
    ],
    sprite: 'buildings/binding_circle.png',
  },
  titan_forge: {
    id: 'titan_forge',
    name: 'Titan Forge',
    cost: 450,
    buildTurns: 10,
    effects: [
      { type: 'gold_per_turn', amount: 10 },
      { type: 'unlock_soldier', soldier: 'titan' },
      { type: 'unlock_soldier', soldier: 'golem' },
    ],
    sprite: 'buildings/titan_forge.png',
  },
};

// Building tier requirements (minimum city population)
export const BUILDING_TIER_REQUIREMENTS: Partial<Record<BuildingId, number>> = {
  // Tier 2 buildings require Pop 3+
  archery_range: 3,
  war_academy: 3,
  mage_tower: 3,
  stables: 3,
  trade_hall: 3,
  fortifications: 3,
  siege_workshop: 3,
  // Tier 3 buildings require Pop 5+
  elite_barracks: 5,
  arcane_sanctum: 5,
  cathedral: 5,
  treasury: 5,
  grand_walls: 5,
  masons_guild: 5,
  // Tier 4 buildings require Pop 7+
  legendary_forge: 7,
  signal_towers: 7,
};

// Building prerequisites (must have these buildings first)
export const BUILDING_PREREQUISITES: Partial<Record<BuildingId, BuildingId[]>> = {
  war_academy: ['barracks'],
  elite_barracks: ['war_academy'],
  fortifications: ['walls'],
  grand_walls: ['fortifications'],
  arcane_sanctum: ['mage_tower'],
  cathedral: ['temple'],
  treasury: ['trade_hall'],
  masons_guild: ['trade_hall'],
  signal_towers: ['fortifications'],
  legendary_forge: ['elite_barracks'],
};

// Exploitation buildings require specific terrain features
export const EXPLOITATION_REQUIREMENTS: Partial<Record<BuildingId, string[]>> = {
  mine: ['iron_vein', 'gold_mine'],
  mana_well: ['mana_spring'],
  archive: ['ancient_ruins'],
  deep_mine: ['mineral_deposit'],
  crystal_sanctum: ['crystal_cave'],
  necropolis: ['haunted_barrow'],
  dragon_shrine: ['dragon_bones'],
  master_forge: ['adamantine_vein'],
  grove_of_ages: ['world_tree'],
  binding_circle: ['hellgate'],
  titan_forge: ['titans_grave'],
};

// How many building slots a city has based on population
// Per design doc: Pop 1-2 = 2 slots, Pop 3-4 = 3 slots, Pop 5-6 = 3 slots, Pop 7+ = 4 slots
export function getBuildingSlots(population: number): number {
  if (population >= 7) return 4;
  if (population >= 3) return 3;
  return 2; // Pop 1-2 gets 2 slots
}

// Check if a building can be built in a city
export function canBuildBuilding(city: { population: number; buildings: BuildingId[] }, buildingId: BuildingId): { canBuild: boolean; reason?: string } {
  // Check population requirement
  const popRequired = BUILDING_TIER_REQUIREMENTS[buildingId];
  if (popRequired && city.population < popRequired) {
    return { canBuild: false, reason: `Requires population ${popRequired}` };
  }

  // Check prerequisites
  const prereqs = BUILDING_PREREQUISITES[buildingId];
  if (prereqs) {
    for (const prereq of prereqs) {
      if (!city.buildings.includes(prereq)) {
        const prereqBuilding = BUILDING_TYPES[prereq];
        return { canBuild: false, reason: `Requires ${prereqBuilding.name}` };
      }
    }
  }

  return { canBuild: true };
}

// Base gold income per city
export const BASE_CITY_INCOME = 10;

// Base population growth rate (turns per +1 pop)
export const TURNS_PER_POPULATION = 5;
