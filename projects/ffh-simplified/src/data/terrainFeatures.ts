import type { TerrainFeature, TerrainFeatureId } from '../types';

export const TERRAIN_FEATURES: Record<TerrainFeatureId, TerrainFeature> = {
  ancient_ruins: {
    id: 'ancient_ruins',
    name: 'Ancient Ruins',
    description: 'Crumbling remnants of a forgotten civilization, hiding secrets and treasures.',
    validTerrain: ['grass', 'hills', 'forest'],
    rarity: 'uncommon',
    effects: [
      { type: 'gold_bonus', amount: 3 },
      { type: 'research_bonus', amount: 1 },
    ],
  },
  mana_spring: {
    id: 'mana_spring',
    name: 'Mana Spring',
    description: 'A natural wellspring of arcane energy bubbles up from the earth.',
    validTerrain: ['grass', 'forest'],
    rarity: 'uncommon',
    effects: [{ type: 'mana_bonus', amount: 2 }],
  },
  iron_vein: {
    id: 'iron_vein',
    name: 'Iron Vein',
    description: 'Rich deposits of iron ore visible in exposed rock faces.',
    validTerrain: ['hills', 'mountain'],
    rarity: 'common',
    effects: [{ type: 'gold_bonus', amount: 2 }],
  },
  gold_mine: {
    id: 'gold_mine',
    name: 'Gold Deposits',
    description: 'Glittering veins of gold run through the stone here.',
    validTerrain: ['hills', 'mountain'],
    rarity: 'rare',
    effects: [{ type: 'gold_bonus', amount: 5 }],
  },
  sacred_grove: {
    id: 'sacred_grove',
    name: 'Sacred Grove',
    description: 'An ancient circle of trees blessed by nature spirits.',
    validTerrain: ['forest'],
    rarity: 'uncommon',
    effects: [
      { type: 'mana_bonus', amount: 1 },
      { type: 'growth_bonus', amount: 1 },
    ],
  },
  watchtower: {
    id: 'watchtower',
    name: 'Ruined Watchtower',
    description: 'The remains of an old defensive tower still provide tactical advantage.',
    validTerrain: ['grass', 'hills'],
    rarity: 'common',
    effects: [{ type: 'defense_bonus', amount: 10 }],
  },
  haunted_barrow: {
    id: 'haunted_barrow',
    name: 'Haunted Barrow',
    description: 'An ancient burial mound where restless spirits linger.',
    validTerrain: ['grass', 'hills'],
    rarity: 'rare',
    effects: [
      { type: 'mana_bonus', amount: 3 },
      { type: 'defense_bonus', amount: 5 },
    ],
  },
  dragon_bones: {
    id: 'dragon_bones',
    name: 'Dragon Bones',
    description: 'The massive skeleton of a fallen dragon, its bones still radiating power.',
    validTerrain: ['grass', 'hills', 'mountain'],
    rarity: 'rare',
    effects: [
      { type: 'mana_bonus', amount: 2 },
      { type: 'research_bonus', amount: 2 },
    ],
  },
  crystal_cave: {
    id: 'crystal_cave',
    name: 'Crystal Cave',
    description: 'A cavern filled with luminescent crystals of immense magical potential.',
    validTerrain: ['hills', 'mountain'],
    rarity: 'rare',
    effects: [{ type: 'mana_bonus', amount: 4 }],
  },
  fertile_plains: {
    id: 'fertile_plains',
    name: 'Fertile Plains',
    description: 'Exceptionally rich soil that yields bountiful harvests.',
    validTerrain: ['grass'],
    rarity: 'common',
    effects: [
      { type: 'gold_bonus', amount: 1 },
      { type: 'growth_bonus', amount: 2 },
    ],
  },
};

// Get all feature IDs
export function getAllFeatureIds(): TerrainFeatureId[] {
  return Object.keys(TERRAIN_FEATURES) as TerrainFeatureId[];
}

// Get features by rarity
export function getFeaturesByRarity(rarity: 'common' | 'uncommon' | 'rare'): TerrainFeature[] {
  return Object.values(TERRAIN_FEATURES).filter(f => f.rarity === rarity);
}

// Get features valid for a terrain type
export function getFeaturesForTerrain(terrain: string): TerrainFeature[] {
  return Object.values(TERRAIN_FEATURES).filter(f =>
    f.validTerrain.includes(terrain as any)
  );
}
