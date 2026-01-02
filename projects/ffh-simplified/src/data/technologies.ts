import type { Technology } from '../types';

export const TECHNOLOGIES: Record<string, Technology> = {
  // ============ MARTIAL - Common ============
  shield_wall: {
    id: 'shield_wall',
    name: 'Shield Wall Tactics',
    description: 'Disciplined formations that protect frontline soldiers.',
    category: 'martial',
    tier: 'common',
    researchTurns: 3,
    effects: [{ type: 'soldier_defense_bonus', soldier: 'fighter', amount: 5 }],
  },
  archery_drills: {
    id: 'archery_drills',
    name: 'Archery Drills',
    description: 'Rigorous training improves archer accuracy.',
    category: 'martial',
    tier: 'common',
    researchTurns: 3,
    effects: [{ type: 'soldier_attack_bonus', soldier: 'archer', amount: 5 }],
  },
  forced_march: {
    id: 'forced_march',
    name: 'Forced March',
    description: 'Legions learn to cover ground quickly when needed.',
    category: 'martial',
    tier: 'common',
    researchTurns: 4,
    effects: [{ type: 'legion_movement_bonus', amount: 1 }],
  },

  // ============ MARTIAL - Guild Secret ============
  cavalry_doctrine: {
    id: 'cavalry_doctrine',
    name: 'Cavalry Doctrine',
    description: 'Advanced mounted warfare techniques.',
    category: 'martial',
    tier: 'guild_secret',
    researchTurns: 5,
    effects: [
      { type: 'soldier_attack_bonus', soldier: 'knight', amount: 8 },
      { type: 'soldier_hp_bonus', soldier: 'knight', amount: 10 },
    ],
  },
  veteran_training: {
    id: 'veteran_training',
    name: 'Veteran Training',
    description: 'Elite instructors toughen all soldiers.',
    category: 'martial',
    tier: 'guild_secret',
    researchTurns: 6,
    effects: [
      { type: 'soldier_hp_bonus', soldier: 'fighter', amount: 15 },
      { type: 'soldier_hp_bonus', soldier: 'archer', amount: 10 },
    ],
  },

  // ============ MARTIAL - Master's Teaching ============
  phalanx_formation: {
    id: 'phalanx_formation',
    name: 'Phalanx Formation',
    description: 'An ancient and devastating defensive formation.',
    category: 'martial',
    tier: 'masters_teaching',
    researchTurns: 8,
    effects: [
      { type: 'soldier_defense_bonus', soldier: 'fighter', amount: 12 },
      { type: 'global_defense_bonus', amount: 5 },
    ],
  },

  // ============ INDUSTRIAL - Common ============
  improved_tools: {
    id: 'improved_tools',
    name: 'Improved Tools',
    description: 'Better tools mean faster construction.',
    category: 'industrial',
    tier: 'common',
    researchTurns: 3,
    effects: [{ type: 'global_gold_bonus', amount: 5 }],
  },
  ironworking: {
    id: 'ironworking',
    name: 'Ironworking',
    description: 'Knowledge of advanced metalwork enables new constructions.',
    category: 'industrial',
    tier: 'common',
    researchTurns: 4,
    effects: [{ type: 'unlock_building', building: 'stables' }],
  },
  masonry: {
    id: 'masonry',
    name: 'Masonry',
    description: 'Stone construction techniques for fortifications.',
    category: 'industrial',
    tier: 'common',
    researchTurns: 4,
    effects: [{ type: 'unlock_building', building: 'walls' }],
  },

  // ============ INDUSTRIAL - Guild Secret ============
  guild_charters: {
    id: 'guild_charters',
    name: 'Guild Charters',
    description: 'Formal agreements with merchant guilds increase profits.',
    category: 'industrial',
    tier: 'guild_secret',
    researchTurns: 5,
    effects: [{ type: 'building_gold_bonus', building: 'market', amount: 5 }],
  },
  agriculture: {
    id: 'agriculture',
    name: 'Advanced Agriculture',
    description: 'Better farming techniques accelerate population growth.',
    category: 'industrial',
    tier: 'guild_secret',
    researchTurns: 5,
    effects: [
      { type: 'unlock_building', building: 'granary' },
      { type: 'global_growth_bonus', amount: 1 },
    ],
  },

  // ============ INDUSTRIAL - Master's Teaching ============
  master_architects: {
    id: 'master_architects',
    name: 'Master Architects',
    description: 'Legendary builders who can construct anything.',
    category: 'industrial',
    tier: 'masters_teaching',
    researchTurns: 8,
    effects: [
      { type: 'global_gold_bonus', amount: 10 },
      { type: 'global_growth_bonus', amount: 1 },
    ],
  },

  // ============ ARCANE - Common ============
  cantrip_methodology: {
    id: 'cantrip_methodology',
    name: 'Cantrip Methodology',
    description: 'Basic magical theory improves spellcasting.',
    category: 'arcane',
    tier: 'common',
    researchTurns: 3,
    effects: [{ type: 'soldier_attack_bonus', soldier: 'mage', amount: 5 }],
  },
  ritual_basics: {
    id: 'ritual_basics',
    name: 'Ritual Basics',
    description: 'Fundamental knowledge for constructing magical sites.',
    category: 'arcane',
    tier: 'common',
    researchTurns: 4,
    effects: [{ type: 'unlock_building', building: 'mage_tower' }],
  },

  // ============ ARCANE - Guild Secret ============
  mana_focusing: {
    id: 'mana_focusing',
    name: 'Mana Focusing',
    description: 'Techniques to amplify magical energy collection.',
    category: 'arcane',
    tier: 'guild_secret',
    researchTurns: 5,
    effects: [{ type: 'building_mana_bonus', building: 'mage_tower', amount: 2 }],
  },
  battle_magic: {
    id: 'battle_magic',
    name: 'Battle Magic',
    description: 'Combat-focused spellcasting techniques.',
    category: 'arcane',
    tier: 'guild_secret',
    researchTurns: 6,
    effects: [
      { type: 'soldier_attack_bonus', soldier: 'mage', amount: 10 },
      { type: 'soldier_hp_bonus', soldier: 'mage', amount: 10 },
    ],
  },

  // ============ ARCANE - Master's Teaching ============
  consecrated_ground: {
    id: 'consecrated_ground',
    name: 'Consecrated Ground',
    description: 'Holy sites that channel divine energy.',
    category: 'arcane',
    tier: 'masters_teaching',
    researchTurns: 7,
    effects: [
      { type: 'unlock_building', building: 'temple' },
      { type: 'global_mana_bonus', amount: 2 },
    ],
  },

  // ============ ARCANE - Lost Art ============
  forbidden_summoning: {
    id: 'forbidden_summoning',
    name: 'Forbidden Summoning',
    description: 'Dark rituals to call forth demons from the pit.',
    category: 'arcane',
    tier: 'lost_art',
    researchTurns: 12,
    effects: [{ type: 'unlock_building', building: 'ritual_site' }],
  },

  // ============ SOCIAL - Common ============
  organized_militia: {
    id: 'organized_militia',
    name: 'Organized Militia',
    description: 'A system for quickly training new recruits.',
    category: 'social',
    tier: 'common',
    researchTurns: 3,
    effects: [{ type: 'unlock_building', building: 'barracks' }],
  },
  trade_routes: {
    id: 'trade_routes',
    name: 'Trade Routes',
    description: 'Established paths for commerce between cities.',
    category: 'social',
    tier: 'common',
    researchTurns: 3,
    effects: [{ type: 'global_gold_bonus', amount: 5 }],
  },

  // ============ SOCIAL - Guild Secret ============
  diplomatic_corps: {
    id: 'diplomatic_corps',
    name: 'Diplomatic Corps',
    description: 'Trained negotiators who smooth relations.',
    category: 'social',
    tier: 'guild_secret',
    researchTurns: 5,
    effects: [{ type: 'global_gold_bonus', amount: 10 }],
  },
  healing_traditions: {
    id: 'healing_traditions',
    name: 'Healing Traditions',
    description: 'Medical knowledge that strengthens clerics.',
    category: 'social',
    tier: 'guild_secret',
    researchTurns: 5,
    effects: [
      { type: 'soldier_hp_bonus', soldier: 'cleric', amount: 15 },
      { type: 'soldier_defense_bonus', soldier: 'cleric', amount: 5 },
    ],
  },

  // ============ SOCIAL - Master's Teaching ============
  code_of_laws: {
    id: 'code_of_laws',
    name: 'Code of Laws',
    description: 'A formal legal system that improves all aspects of society.',
    category: 'social',
    tier: 'masters_teaching',
    researchTurns: 8,
    effects: [
      { type: 'global_gold_bonus', amount: 10 },
      { type: 'global_growth_bonus', amount: 1 },
    ],
  },

  // ============ SOCIAL - Lost Art ============
  ancient_oaths: {
    id: 'ancient_oaths',
    name: 'Ancient Oaths',
    description: 'Binding vows that grant supernatural protection.',
    category: 'social',
    tier: 'lost_art',
    researchTurns: 10,
    effects: [
      { type: 'global_defense_bonus', amount: 10 },
      { type: 'soldier_defense_bonus', soldier: 'knight', amount: 10 },
    ],
  },
};

// Get all technology IDs
export function getAllTechnologyIds(): string[] {
  return Object.keys(TECHNOLOGIES);
}

// Get technologies by tier
export function getTechnologiesByTier(tier: string): Technology[] {
  return Object.values(TECHNOLOGIES).filter(t => t.tier === tier);
}

// Get technologies by category
export function getTechnologiesByCategory(category: string): Technology[] {
  return Object.values(TECHNOLOGIES).filter(t => t.category === category);
}
