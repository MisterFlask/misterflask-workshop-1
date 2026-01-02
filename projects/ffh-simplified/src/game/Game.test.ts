import { describe, it, expect } from 'vitest';
import { createInitialGameState, processAction, getValidMoves, getLegionAt, getCityAt } from './Game';
import { resolveCombat } from './Combat';
import type { Legion, Soldier } from '../types';
import { SOLDIER_TYPES } from '../data/soldiers';

describe('Game State', () => {
  it('creates initial state with player faction', () => {
    const state = createInitialGameState(12345);

    expect(state.turn).toBe(1);
    expect(state.phase).toBe('player_turn');
    expect(state.factions.has('player')).toBe(true);
    expect(state.armageddonCounter).toBe(0);
  });

  it('player starts with gold', () => {
    const state = createInitialGameState(12345);
    const player = state.factions.get('player');

    expect(player).toBeDefined();
    expect(player!.gold).toBeGreaterThan(0);
  });

  it('player has at least one legion', () => {
    const state = createInitialGameState(12345);
    const playerLegions = Array.from(state.legions.values()).filter(l => l.owner === 'player');

    expect(playerLegions.length).toBeGreaterThan(0);
  });

  it('player has at least one city', () => {
    const state = createInitialGameState(12345);
    const playerCities = Array.from(state.cities.values()).filter(c => c.owner === 'player');

    expect(playerCities.length).toBeGreaterThan(0);
  });
});

describe('Legion Movement', () => {
  it('legion has valid moves on its turn', () => {
    const state = createInitialGameState(12345);
    const playerLegion = Array.from(state.legions.values()).find(l => l.owner === 'player');

    expect(playerLegion).toBeDefined();
    const moves = getValidMoves(state, playerLegion!);

    expect(moves.length).toBeGreaterThan(0);
  });

  it('moving legion updates its position', () => {
    const state = createInitialGameState(12345);
    const playerLegion = Array.from(state.legions.values()).find(l => l.owner === 'player');
    expect(playerLegion).toBeDefined();

    const moves = getValidMoves(state, playerLegion!);
    expect(moves.length).toBeGreaterThan(0);

    const targetMove = moves[0];
    const newState = processAction(state, {
      type: 'move_legion',
      legionId: playerLegion!.id,
      to: targetMove,
    });

    const movedLegion = newState.legions.get(playerLegion!.id);
    expect(movedLegion).toBeDefined();
    expect(movedLegion!.location).toEqual(targetMove);
  });
});

describe('Combat System', () => {
  function createTestLegion(owner: string, soldiers: { type: keyof typeof SOLDIER_TYPES; row: 'front' | 'mid' | 'back' }[]): Legion {
    return {
      id: `test-legion-${owner}`,
      owner: owner as any,
      location: { x: 0, y: 0 },
      movementRemaining: 3,
      soldiers: soldiers.map((s, i) => ({
        id: `soldier-${i}`,
        type: s.type,
        hp: SOLDIER_TYPES[s.type].hp,
        maxHp: SOLDIER_TYPES[s.type].hp,
        position: { row: s.row, column: i % 3 },
      })),
    };
  }

  it('combat produces a result', () => {
    const attacker = createTestLegion('player', [
      { type: 'fighter', row: 'front' },
      { type: 'fighter', row: 'front' },
    ]);
    const defender = createTestLegion('hippus', [
      { type: 'fighter', row: 'front' },
    ]);

    const result = resolveCombat(attacker, defender, 'grass', false);

    expect(result).toBeDefined();
    expect(typeof result.attackerWon).toBe('boolean');
    expect(result.attackerDamageDealt).toBeGreaterThan(0);
    expect(result.defenderDamageDealt).toBeGreaterThan(0);
  });

  it('larger army tends to win', () => {
    const attacker = createTestLegion('player', [
      { type: 'fighter', row: 'front' },
      { type: 'fighter', row: 'front' },
      { type: 'fighter', row: 'front' },
      { type: 'archer', row: 'back' },
    ]);
    const defender = createTestLegion('hippus', [
      { type: 'fighter', row: 'front' },
    ]);

    const result = resolveCombat(attacker, defender, 'grass', false);

    // With 4v1, attacker should usually win
    expect(result.attackerWon).toBe(true);
  });

  it('walls provide defense bonus', () => {
    const attacker = createTestLegion('player', [
      { type: 'fighter', row: 'front' },
      { type: 'fighter', row: 'front' },
    ]);
    const defender = createTestLegion('hippus', [
      { type: 'fighter', row: 'front' },
      { type: 'fighter', row: 'front' },
    ]);

    const resultNoWalls = resolveCombat(attacker, defender, 'grass', false);
    const resultWithWalls = resolveCombat(attacker, defender, 'grass', true);

    // Defender should take less damage with walls
    expect(resultWithWalls.attackerDamageDealt).toBeLessThanOrEqual(resultNoWalls.attackerDamageDealt);
  });

  it('clicking enemy tile triggers combat via select_tile', () => {
    let state = createInitialGameState(12345);
    const playerLegion = Array.from(state.legions.values()).find(l => l.owner === 'player')!;
    const enemyLegion = Array.from(state.legions.values()).find(l => l.owner !== 'player')!;

    // Create strong player army
    const strongPlayer = {
      ...playerLegion,
      soldiers: [
        ...playerLegion.soldiers,
        { id: 'e1', type: 'fighter' as const, hp: 100, maxHp: 100, position: { row: 'front' as const, column: 0 } },
        { id: 'e2', type: 'fighter' as const, hp: 100, maxHp: 100, position: { row: 'front' as const, column: 1 } },
        { id: 'e3', type: 'fighter' as const, hp: 100, maxHp: 100, position: { row: 'front' as const, column: 2 } },
        { id: 'e4', type: 'archer' as const, hp: 60, maxHp: 60, position: { row: 'back' as const, column: 0 } },
      ]
    };

    // Position enemy adjacent to player
    const enemyLoc = { x: playerLegion.location.x + 1, y: playerLegion.location.y };
    const newLegions = new Map(state.legions);
    newLegions.set(playerLegion.id, strongPlayer);
    newLegions.set(enemyLegion.id, { ...enemyLegion, location: enemyLoc });
    state = { ...state, legions: newLegions };

    // Step 1: Click on player legion to select it
    state = processAction(state, { type: 'select_tile', coord: playerLegion.location });
    expect(state.selectedLegionId).toBe(playerLegion.id);

    // Step 2: Click on enemy location to attack
    state = processAction(state, { type: 'select_tile', coord: enemyLoc });

    // Combat should be pending (phase is combat_resolution)
    expect(state.phase).toBe('combat_resolution');
    expect(state.pendingCombat).not.toBeNull();
    expect(state.pendingCombat!.attackerId).toBe(playerLegion.id);
    expect(state.pendingCombat!.defenderId).toBe(enemyLegion.id);

    // Step 3: Apply combat results (normally done after combat scene plays)
    state = processAction(state, { type: 'apply_combat_results' });

    // Now enemy should be destroyed, player should move to enemy location
    expect(state.legions.has(enemyLegion.id)).toBe(false);
    expect(state.legions.get(playerLegion.id)?.location).toEqual(enemyLoc);
    expect(state.phase).toBe('player_turn');
  });
});

describe('Turn Processing', () => {
  it('ending turn advances turn counter', () => {
    const state = createInitialGameState(12345);
    expect(state.turn).toBe(1);

    const newState = processAction(state, { type: 'end_turn' });

    expect(newState.turn).toBe(2);
  });

  it('ending turn advances armageddon counter', () => {
    const state = createInitialGameState(12345);
    const initialCounter = state.armageddonCounter;

    const newState = processAction(state, { type: 'end_turn' });

    expect(newState.armageddonCounter).toBeGreaterThan(initialCounter);
  });

  it('ending turn gives player gold income', () => {
    const state = createInitialGameState(12345);
    const player = state.factions.get('player')!;
    const initialGold = player.gold;

    const newState = processAction(state, { type: 'end_turn' });
    const newPlayer = newState.factions.get('player')!;

    expect(newPlayer.gold).toBeGreaterThan(initialGold);
  });
});
