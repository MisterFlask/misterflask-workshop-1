import type { SoldierType, SoldierTypeId } from '../types';

export const SOLDIER_TYPES: Record<SoldierTypeId, SoldierType> = {
  // ============ CORE UNITS (Building-gated) ============

  // Tier 1 - Available from Barracks
  fighter: {
    id: 'fighter',
    name: 'Fighter',
    hp: 100,
    attack: 20,
    defense: 15,
    speed: 50,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 3, mid: 2, back: 1 },
    cost: { gold: 50, mana: 0 },
    buildTurns: 1,
    sprite: 'soldiers/fighter.png',
  },

  // Tier 1 - Available from Archery Range
  archer: {
    id: 'archer',
    name: 'Archer',
    hp: 60,
    attack: 25,
    defense: 5,
    speed: 60,
    preferredRow: 'back',
    attacksTarget: 'back',
    attackCount: { front: 1, mid: 2, back: 2 },
    cost: { gold: 60, mana: 0 },
    buildTurns: 1,
    sprite: 'soldiers/archer.png',
  },

  // Tier 1 - Available from Temple
  cleric: {
    id: 'cleric',
    name: 'Cleric',
    hp: 70,
    attack: 10,
    defense: 10,
    speed: 45,
    preferredRow: 'back',
    attacksTarget: 'front', // Heals allies instead of attacking
    attackCount: { front: 1, mid: 1, back: 1 },
    cost: { gold: 70, mana: 5 },
    buildTurns: 2,
    sprite: 'soldiers/cleric.png',
  },

  // Tier 2 - Available from War Academy
  knight: {
    id: 'knight',
    name: 'Knight',
    hp: 150,
    attack: 30,
    defense: 20,
    speed: 30,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 3, mid: 2, back: 1 },
    cost: { gold: 100, mana: 0 },
    buildTurns: 2,
    sprite: 'soldiers/knight.png',
  },

  // Tier 2 - Available from Mage Tower
  mage: {
    id: 'mage',
    name: 'Mage',
    hp: 50,
    attack: 35,
    defense: 5,
    speed: 40,
    preferredRow: 'back',
    attacksTarget: 'back',
    attackCount: { front: 1, mid: 1, back: 1 },
    cost: { gold: 80, mana: 10 },
    buildTurns: 2,
    sprite: 'soldiers/mage.png',
  },

  // Tier 2 - Available from Stables
  cavalry: {
    id: 'cavalry',
    name: 'Cavalry',
    hp: 100,
    attack: 25,
    defense: 10,
    speed: 80, // Fastest unit
    preferredRow: 'front',
    attacksTarget: 'back', // Flanking attacks
    attackCount: { front: 2, mid: 2, back: 2 },
    cost: { gold: 90, mana: 0 },
    buildTurns: 2,
    sprite: 'soldiers/cavalry.png',
  },

  // Tier 2 - Available from Siege Workshop
  catapult: {
    id: 'catapult',
    name: 'Catapult',
    hp: 80,
    attack: 40,
    defense: 5,
    speed: 10, // Slowest unit
    preferredRow: 'back',
    attacksTarget: 'front', // Siege attacks
    attackCount: { front: 2, mid: 2, back: 1 },
    cost: { gold: 120, mana: 0 },
    buildTurns: 3,
    sprite: 'soldiers/catapult.png',
  },

  // Tier 3 - Available from Elite Barracks
  champion: {
    id: 'champion',
    name: 'Champion',
    hp: 180,
    attack: 40,
    defense: 25,
    speed: 45,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 3, mid: 2, back: 1 },
    cost: { gold: 150, mana: 0 },
    buildTurns: 3,
    sprite: 'soldiers/champion.png',
  },

  // Tier 3 - Available from Cathedral
  paladin: {
    id: 'paladin',
    name: 'Paladin',
    hp: 160,
    attack: 35,
    defense: 25,
    speed: 35,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 2, mid: 2, back: 1 },
    cost: { gold: 140, mana: 10 },
    buildTurns: 3,
    sprite: 'soldiers/paladin.png',
  },

  // ============ TERRAIN-GATED UNITS (Exploitation buildings) ============

  // From Mana Well (requires mana_spring)
  fire_elemental: {
    id: 'fire_elemental',
    name: 'Fire Elemental',
    hp: 80,
    attack: 45,
    defense: 10,
    speed: 55,
    preferredRow: 'mid',
    attacksTarget: 'front',
    attackCount: { front: 2, mid: 2, back: 1 },
    cost: { gold: 60, mana: 15 },
    buildTurns: 2,
    sprite: 'soldiers/fire_elemental.png',
  },

  ice_elemental: {
    id: 'ice_elemental',
    name: 'Ice Elemental',
    hp: 100,
    attack: 30,
    defense: 20,
    speed: 35,
    preferredRow: 'mid',
    attacksTarget: 'front',
    attackCount: { front: 2, mid: 2, back: 1 },
    cost: { gold: 60, mana: 15 },
    buildTurns: 2,
    sprite: 'soldiers/ice_elemental.png',
  },

  // From Archive (requires ancient_ruins)
  sage: {
    id: 'sage',
    name: 'Sage',
    hp: 60,
    attack: 25,
    defense: 10,
    speed: 50,
    preferredRow: 'back',
    attacksTarget: 'back',
    attackCount: { front: 1, mid: 1, back: 2 },
    cost: { gold: 80, mana: 10 },
    buildTurns: 2,
    sprite: 'soldiers/sage.png',
  },

  // From Crystal Sanctum (requires crystal_cave)
  archmage: {
    id: 'archmage',
    name: 'Archmage',
    hp: 70,
    attack: 55,
    defense: 10,
    speed: 45,
    preferredRow: 'back',
    attacksTarget: 'back',
    attackCount: { front: 1, mid: 2, back: 2 },
    cost: { gold: 150, mana: 25 },
    buildTurns: 4,
    sprite: 'soldiers/archmage.png',
  },

  // From Necropolis (requires haunted_barrow)
  lich: {
    id: 'lich',
    name: 'Lich',
    hp: 90,
    attack: 50,
    defense: 15,
    speed: 40,
    preferredRow: 'back',
    attacksTarget: 'back',
    attackCount: { front: 1, mid: 2, back: 2 },
    cost: { gold: 140, mana: 30 },
    buildTurns: 4,
    sprite: 'soldiers/lich.png',
  },

  // From Dragon Shrine (requires dragon_bones)
  dragon_knight: {
    id: 'dragon_knight',
    name: 'Dragon Knight',
    hp: 200,
    attack: 50,
    defense: 30,
    speed: 60,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 3, mid: 2, back: 1 },
    cost: { gold: 200, mana: 20 },
    buildTurns: 5,
    sprite: 'soldiers/dragon_knight.png',
  },

  // From Master Forge (requires adamantine_vein)
  siege_titan: {
    id: 'siege_titan',
    name: 'Siege Titan',
    hp: 250,
    attack: 60,
    defense: 35,
    speed: 15,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 2, mid: 2, back: 1 },
    cost: { gold: 250, mana: 10 },
    buildTurns: 5,
    sprite: 'soldiers/siege_titan.png',
  },

  // From Grove of Ages (requires world_tree)
  treant: {
    id: 'treant',
    name: 'Treant',
    hp: 300,
    attack: 35,
    defense: 40,
    speed: 20,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 2, mid: 1, back: 1 },
    cost: { gold: 180, mana: 25 },
    buildTurns: 5,
    sprite: 'soldiers/treant.png',
  },

  // From Binding Circle (requires hellgate)
  summoned_demon: {
    id: 'summoned_demon',
    name: 'Summoned Demon',
    hp: 150,
    attack: 45,
    defense: 20,
    speed: 50,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 3, mid: 2, back: 1 },
    cost: { gold: 120, mana: 35 },
    buildTurns: 3,
    sprite: 'soldiers/summoned_demon.png',
  },

  // From Titan Forge (requires titans_grave)
  golem: {
    id: 'golem',
    name: 'Golem',
    hp: 200,
    attack: 40,
    defense: 30,
    speed: 25,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 2, mid: 2, back: 1 },
    cost: { gold: 180, mana: 15 },
    buildTurns: 4,
    sprite: 'soldiers/golem.png',
  },

  titan: {
    id: 'titan',
    name: 'Titan',
    hp: 400,
    attack: 70,
    defense: 45,
    speed: 30,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 3, mid: 2, back: 1 },
    cost: { gold: 350, mana: 30 },
    buildTurns: 6,
    sprite: 'soldiers/titan.png',
  },

  // ============ BOSS/AI FACTION UNITS ============

  demon: {
    id: 'demon',
    name: 'Demon',
    hp: 120,
    attack: 35,
    defense: 15,
    speed: 55,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 3, mid: 2, back: 1 },
    cost: { gold: 100, mana: 0 }, // Boss-only, doesn't use normal recruitment
    buildTurns: 1,
    sprite: 'soldiers/demon.png',
  },

  // ============ CITY GARRISON (Non-recruitable) ============

  city_garrison: {
    id: 'city_garrison',
    name: 'City Garrison',
    hp: 100,
    attack: 20,
    defense: 18, // Slightly higher defense than fighter (defensive role)
    speed: 45,
    preferredRow: 'front',
    attacksTarget: 'front',
    attackCount: { front: 3, mid: 2, back: 1 },
    cost: { gold: 0, mana: 0 }, // Not purchasable
    buildTurns: 0,
    sprite: 'soldiers/city_garrison.png',
  },
};
