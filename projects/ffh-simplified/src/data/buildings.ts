import type { BuildingType, BuildingId } from '../types';

export const BUILDING_TYPES: Record<BuildingId, BuildingType> = {
  barracks: {
    id: 'barracks',
    name: 'Barracks',
    cost: 100,
    effects: [{ type: 'unlock_soldier', soldier: 'fighter' }],
    sprite: 'buildings/barracks.png',
  },
  market: {
    id: 'market',
    name: 'Market',
    cost: 80,
    effects: [{ type: 'gold_per_turn', amount: 10 }],
    sprite: 'buildings/market.png',
  },
  temple: {
    id: 'temple',
    name: 'Temple',
    cost: 100,
    effects: [{ type: 'unlock_soldier', soldier: 'cleric' }],
    sprite: 'buildings/temple.png',
  },
  mage_tower: {
    id: 'mage_tower',
    name: 'Mage Tower',
    cost: 150,
    effects: [
      { type: 'unlock_soldier', soldier: 'mage' },
      { type: 'mana_per_turn', amount: 2 },
    ],
    sprite: 'buildings/mage_tower.png',
  },
  stables: {
    id: 'stables',
    name: 'Stables',
    cost: 120,
    effects: [{ type: 'unlock_soldier', soldier: 'knight' }],
    sprite: 'buildings/stables.png',
  },
  walls: {
    id: 'walls',
    name: 'Walls',
    cost: 150,
    effects: [{ type: 'defense_bonus', amount: 40 }],
    sprite: 'buildings/walls.png',
  },
  granary: {
    id: 'granary',
    name: 'Granary',
    cost: 60,
    effects: [{ type: 'growth_bonus', amount: 1 }],
    sprite: 'buildings/granary.png',
  },
  ritual_site: {
    id: 'ritual_site',
    name: 'Ritual Site',
    cost: 200,
    effects: [], // Special building for Sheaim
    sprite: 'buildings/ritual_site.png',
  },
};

// How many building slots a city has based on population
export function getBuildingSlots(population: number): number {
  if (population >= 5) return 3;
  if (population >= 3) return 2;
  return 1;
}

// Base gold income per city
export const BASE_CITY_INCOME = 10;

// Base population growth rate (turns per +1 pop)
export const TURNS_PER_POPULATION = 5;
