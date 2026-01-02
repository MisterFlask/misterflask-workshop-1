import type { Faction, FactionId, FactionType } from '../types';

export interface FactionTemplate {
  id: FactionId;
  type: FactionType;
  name: string;
  color: string;
  startingGold: number;
  startingMana: number;
}

export const FACTION_TEMPLATES: Record<FactionId, FactionTemplate> = {
  player: {
    id: 'player',
    type: 'player',
    name: 'Your Empire',
    color: '#4a90d9',
    startingGold: 200,
    startingMana: 0,
  },
  hippus: {
    id: 'hippus',
    type: 'raider',
    name: 'Hippus Raiders',
    color: '#d4a24c',
    startingGold: 150,
    startingMana: 0,
  },
  sheaim: {
    id: 'sheaim',
    type: 'ritualist',
    name: 'Sheaim Cultists',
    color: '#8b0000',
    startingGold: 100,
    startingMana: 20,
  },
  elves: {
    id: 'elves',
    type: 'defender',
    name: 'Elven Wardens',
    color: '#228b22',
    startingGold: 100,
    startingMana: 10,
  },
  boss: {
    id: 'boss',
    type: 'boss',
    name: 'The Infernal Legion',
    color: '#ff4500',
    startingGold: 500,
    startingMana: 0,
  },
};

export function createFaction(template: FactionTemplate): Faction {
  return {
    id: template.id,
    type: template.type,
    name: template.name,
    gold: template.startingGold,
    mana: template.startingMana,
    state: { type: 'idle' },
    color: template.color,
  };
}
