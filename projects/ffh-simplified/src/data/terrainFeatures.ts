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
    validTerrain: ['grass', 'forest', 'swamp'],
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
    validTerrain: ['grass', 'hills', 'swamp'],
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

  // ============ UNCOMMON FEATURES ============

  mineral_deposit: {
    id: 'mineral_deposit',
    name: 'Mineral Deposit',
    description: 'A rich vein of rare minerals valuable for crafting and trade.',
    validTerrain: ['hills', 'mountain'],
    rarity: 'uncommon',
    effects: [{ type: 'gold_bonus', amount: 4 }],
  },

  // ============ RARE FEATURES ============

  adamantine_vein: {
    id: 'adamantine_vein',
    name: 'Adamantine Vein',
    description: 'An extraordinarily rare deposit of the legendary unbreakable metal.',
    validTerrain: ['mountain'],
    rarity: 'rare',
    effects: [
      { type: 'gold_bonus', amount: 8 },
      { type: 'defense_bonus', amount: 10 },
    ],
  },

  world_tree: {
    id: 'world_tree',
    name: 'World Tree',
    description: 'A titanic ancient tree whose roots span dimensions, a living conduit of nature magic.',
    validTerrain: ['forest'],
    rarity: 'rare',
    effects: [
      { type: 'mana_bonus', amount: 5 },
      { type: 'growth_bonus', amount: 3 },
    ],
  },

  // ============ LEGENDARY FEATURES ============

  hellgate: {
    id: 'hellgate',
    name: 'Hellgate',
    description: 'A tear in reality leading to the infernal planes. Dangerous, but a source of immense dark power.',
    validTerrain: ['grass', 'hills', 'swamp'],
    rarity: 'legendary',
    effects: [
      { type: 'mana_bonus', amount: 8 },
    ],
  },

  titans_grave: {
    id: 'titans_grave',
    name: "Titan's Grave",
    description: 'The burial site of a primordial titan, its bones still resonating with ancient power.',
    validTerrain: ['mountain', 'hills'],
    rarity: 'legendary',
    effects: [
      { type: 'gold_bonus', amount: 10 },
      { type: 'mana_bonus', amount: 5 },
    ],
  },
};

// Get all feature IDs
export function getAllFeatureIds(): TerrainFeatureId[] {
  return Object.keys(TERRAIN_FEATURES) as TerrainFeatureId[];
}

// Get features by rarity
export function getFeaturesByRarity(rarity: 'common' | 'uncommon' | 'rare' | 'legendary'): TerrainFeature[] {
  return Object.values(TERRAIN_FEATURES).filter(f => f.rarity === rarity);
}

// Get features valid for a terrain type
export function getFeaturesForTerrain(terrain: string): TerrainFeature[] {
  return Object.values(TERRAIN_FEATURES).filter(f =>
    f.validTerrain.includes(terrain as any)
  );
}
