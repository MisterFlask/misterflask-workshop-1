import type { SpriteCategory, SpriteStyle } from '../api/types.js';

// All sprite IDs from the game's types.ts
export const SPRITE_IDS: Record<SpriteCategory, string[]> = {
  soldier: [
    'fighter', 'archer', 'cleric', 'knight', 'mage', 'cavalry', 'catapult',
    'paladin', 'champion', 'fire_elemental', 'ice_elemental', 'sage', 'archmage',
    'lich', 'dragon_knight', 'siege_titan', 'treant', 'summoned_demon', 'golem',
    'titan', 'demon', 'city_garrison',
  ],
  building: [
    'barracks', 'market', 'temple', 'granary', 'walls', 'archery_range',
    'war_academy', 'mage_tower', 'stables', 'trade_hall', 'fortifications',
    'siege_workshop', 'elite_barracks', 'arcane_sanctum', 'cathedral', 'treasury',
    'grand_walls', 'masons_guild', 'legendary_forge', 'signal_towers', 'ritual_site',
    'mine', 'mana_well', 'archive', 'deep_mine', 'crystal_sanctum', 'necropolis',
    'dragon_shrine', 'master_forge', 'grove_of_ages', 'binding_circle', 'titan_forge',
  ],
  terrain: ['grass', 'forest', 'hills', 'mountain', 'water', 'swamp'],
  terrain_feature: [
    'ancient_ruins', 'mana_spring', 'iron_vein', 'gold_mine', 'sacred_grove',
    'watchtower', 'fertile_plains', 'haunted_barrow', 'dragon_bones', 'crystal_cave',
    'mineral_deposit', 'adamantine_vein', 'world_tree', 'hellgate', 'titans_grave',
  ],
  icon: ['gold', 'mana', 'population', 'attack', 'defense', 'movement', 'hp'],
  effect: ['fire', 'ice', 'heal', 'lightning', 'poison', 'holy', 'dark'],
};

// Style descriptions for prompt generation
export const STYLE_DESCRIPTIONS: Record<SpriteStyle, string> = {
  fantasy: 'dark fantasy style, detailed with rich colors and dramatic lighting',
  retro: '16-bit SNES/Genesis era style, limited color palette, nostalgic pixel art aesthetic',
  modern_pixel: 'modern indie game pixel art, clean lines, vibrant colors, smooth gradients',
};

// Category-specific prompt templates
export const CATEGORY_PROMPTS: Record<SpriteCategory, { base: string; requirements: string }> = {
  soldier: {
    base: 'Create a 32x32 pixel art game sprite of a {name} character for a fantasy strategy game.',
    requirements: `
- Character should be clearly recognizable as {name}
- Facing right (standard game sprite orientation)
- Action-ready stance, not static
- Show key identifying features (weapons, armor, magical effects)
- Clear silhouette visible at small size
- Transparent background`,
  },
  building: {
    base: 'Create a 32x32 pixel art building icon of a {name} for a fantasy city-builder game.',
    requirements: `
- Top-down or isometric perspective
- Building should be immediately recognizable as {name}
- Include distinctive architectural details
- Show activity indicators if appropriate (smoke, lights, etc.)
- Fit within a square tile
- Slight drop shadow for depth`,
  },
  terrain: {
    base: 'Create a 32x32 pixel art terrain tile showing {name} for a fantasy strategy game map.',
    requirements: `
- Seamlessly tileable design
- Clear terrain type at a glance
- Natural, organic look
- Appropriate ground texture and coloring
- No strong directional lighting
- Edges should blend when tiled`,
  },
  terrain_feature: {
    base: 'Create a 32x32 pixel art map feature icon of {name} for a fantasy strategy game.',
    requirements: `
- Feature should be the focal point
- Include subtle environmental context
- Mystical or ancient atmosphere where appropriate
- Clear silhouette
- Transparent or semi-transparent background
- Glowing or magical effects for magical features`,
  },
  icon: {
    base: 'Create a 32x32 pixel art UI icon representing {name} for a fantasy game interface.',
    requirements: `
- Clean, readable at small sizes
- Clear symbolic representation
- Consistent with fantasy RPG aesthetics
- High contrast for visibility
- Transparent background
- Simple but detailed design`,
  },
  effect: {
    base: 'Create a 32x32 pixel art visual effect sprite showing {name} magic/ability for a fantasy game.',
    requirements: `
- Dynamic, action-oriented
- Glowing or particle-like appearance
- Transparent background
- Could be overlaid on characters
- Energy and movement conveyed
- Appropriate color scheme for element type`,
  },
};
