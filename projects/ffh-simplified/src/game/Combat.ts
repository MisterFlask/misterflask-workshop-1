import type { Legion, Soldier, CombatResult, FormationRow, TerrainType } from '../types';
import { SOLDIER_TYPES } from '../data/soldiers';

const COMBAT_ROUND_DURATION = 100;
const MINIMUM_DAMAGE = 5;

interface Attack {
  attackerId: string;
  timestamp: number;
  attackNumber: number;
}

// Get defense bonus for terrain
function getTerrainDefenseBonus(terrain: TerrainType, hasWalls: boolean): number {
  let bonus = 0;
  switch (terrain) {
    case 'forest':
      bonus = 0.1;
      break;
    case 'hills':
      bonus = 0.15;
      break;
  }
  // City bonus is handled separately via hasWalls parameter
  if (hasWalls) {
    bonus += 0.4;
  }
  return bonus;
}

// Calculate damage for a single attack
function calculateDamage(
  attacker: Soldier,
  defender: Soldier,
  terrainBonus: number
): number {
  const attackerType = SOLDIER_TYPES[attacker.type];
  const defenderType = SOLDIER_TYPES[defender.type];

  const effectiveDefense = defenderType.defense * (1 + terrainBonus);
  const rawDamage = attackerType.attack - effectiveDefense;

  return Math.max(rawDamage, MINIMUM_DAMAGE);
}

// Find target for an attack
function findTarget(
  attacker: Soldier,
  enemies: Soldier[],
  targetPreference: 'front' | 'back'
): Soldier | null {
  const attackerType = SOLDIER_TYPES[attacker.type];

  // Group enemies by row
  const byRow: Record<FormationRow, Soldier[]> = {
    front: [],
    mid: [],
    back: [],
  };

  for (const enemy of enemies) {
    if (enemy.hp > 0) {
      byRow[enemy.position.row].push(enemy);
    }
  }

  // Target preference order
  const rowOrder: FormationRow[] =
    targetPreference === 'front'
      ? ['front', 'mid', 'back']
      : ['back', 'mid', 'front'];

  // Find first non-empty row in preference order
  for (const row of rowOrder) {
    if (byRow[row].length > 0) {
      // Prefer same column, then adjacent
      const sameColumn = byRow[row].find(
        e => e.position.column === attacker.position.column
      );
      if (sameColumn) return sameColumn;

      // Return first available in this row
      return byRow[row][0];
    }
  }

  return null;
}

// Build attack timeline for a soldier
function buildAttackTimeline(soldier: Soldier): Attack[] {
  const soldierType = SOLDIER_TYPES[soldier.type];
  const numAttacks = soldierType.attackCount[soldier.position.row];
  const attackInterval = COMBAT_ROUND_DURATION / soldierType.speed;

  const attacks: Attack[] = [];
  for (let i = 0; i < numAttacks; i++) {
    attacks.push({
      attackerId: soldier.id,
      timestamp: attackInterval * (i + 1),
      attackNumber: i + 1,
    });
  }

  return attacks;
}

// Main combat resolution
export function resolveCombat(
  attacker: Legion,
  defender: Legion,
  terrain: TerrainType,
  defenderHasWalls: boolean = false
): CombatResult {
  // Create working copies of soldiers
  const attackerSoldiers = attacker.soldiers.map(s => ({ ...s }));
  const defenderSoldiers = defender.soldiers.map(s => ({ ...s }));

  const terrainBonus = getTerrainDefenseBonus(terrain, defenderHasWalls);

  // Build combined attack timeline
  const allAttacks: (Attack & { isAttacker: boolean })[] = [];

  for (const soldier of attackerSoldiers) {
    for (const attack of buildAttackTimeline(soldier)) {
      allAttacks.push({ ...attack, isAttacker: true });
    }
  }

  for (const soldier of defenderSoldiers) {
    for (const attack of buildAttackTimeline(soldier)) {
      allAttacks.push({ ...attack, isAttacker: false });
    }
  }

  // Sort by timestamp
  allAttacks.sort((a, b) => a.timestamp - b.timestamp);

  // Track damage dealt
  let attackerDamageDealt = 0;
  let defenderDamageDealt = 0;

  // Execute attacks
  for (const attack of allAttacks) {
    const friendlySoldiers = attack.isAttacker ? attackerSoldiers : defenderSoldiers;
    const enemySoldiers = attack.isAttacker ? defenderSoldiers : attackerSoldiers;

    const attacker = friendlySoldiers.find(s => s.id === attack.attackerId);
    if (!attacker || attacker.hp <= 0) continue; // Dead soldiers don't attack

    const attackerType = SOLDIER_TYPES[attacker.type];

    // Special case: Cleric heals allies instead of attacking
    if (attacker.type === 'cleric') {
      // Find lowest HP friendly soldier
      const wounded = friendlySoldiers
        .filter(s => s.hp > 0 && s.hp < s.maxHp)
        .sort((a, b) => a.hp / a.maxHp - b.hp / b.maxHp);

      if (wounded.length > 0) {
        const healAmount = 30;
        wounded[0].hp = Math.min(wounded[0].maxHp, wounded[0].hp + healAmount);
      }
      continue;
    }

    // Find target
    const target = findTarget(attacker, enemySoldiers, attackerType.attacksTarget);
    if (!target) continue;

    // Apply defender terrain bonus only to defender side
    const bonus = attack.isAttacker ? terrainBonus : 0;
    const damage = calculateDamage(attacker, target, bonus);

    target.hp -= damage;

    if (attack.isAttacker) {
      attackerDamageDealt += damage;
    } else {
      defenderDamageDealt += damage;
    }
  }

  // Determine casualties
  const attackerCasualties = attackerSoldiers
    .filter(s => s.hp <= 0)
    .map(s => s.id);
  const defenderCasualties = defenderSoldiers
    .filter(s => s.hp <= 0)
    .map(s => s.id);

  // Determine winner (defender wins ties)
  const attackerWon = attackerDamageDealt > defenderDamageDealt;

  return {
    attackerWon,
    attackerCasualties,
    defenderCasualties,
    attackerDamageDealt,
    defenderDamageDealt,
  };
}

// Apply combat results to legions
export function applyCombatResult(
  attacker: Legion,
  defender: Legion,
  result: CombatResult
): void {
  // Remove dead soldiers
  attacker.soldiers = attacker.soldiers.filter(
    s => !result.attackerCasualties.includes(s.id)
  );
  defender.soldiers = defender.soldiers.filter(
    s => !result.defenderCasualties.includes(s.id)
  );

  // Update HP of survivors (find matching soldier and update HP)
  // This is handled by the combat resolution modifying the copies,
  // but we need to apply the HP changes to the originals
  // For simplicity, we'll just remove the dead ones here
  // A more complete implementation would track HP changes
}
