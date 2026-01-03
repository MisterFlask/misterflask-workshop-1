import type { Legion, Soldier, CombatResult, CombatEvent, FormationRow, TerrainType, SoldierTypeId } from '../types';
import { SOLDIER_TYPES } from '../data/soldiers';

const COMBAT_ROUND_DURATION = 100;
const MINIMUM_DAMAGE = 5;

// Tech bonuses that can be applied in combat
export interface SoldierTechBonuses {
  attack: Partial<Record<SoldierTypeId, number>>;
  defense: Partial<Record<SoldierTypeId, number>>;
  globalDefense: number;
}

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
  terrainBonus: number,
  attackerTechBonuses?: SoldierTechBonuses,
  defenderTechBonuses?: SoldierTechBonuses
): number {
  const attackerType = SOLDIER_TYPES[attacker.type];
  const defenderType = SOLDIER_TYPES[defender.type];

  // Apply tech attack bonus to attacker
  const techAttackBonus = attackerTechBonuses?.attack[attacker.type] ?? 0;
  const effectiveAttack = attackerType.attack + techAttackBonus;

  // Apply tech defense bonuses to defender
  const techDefenseBonus = defenderTechBonuses?.defense[defender.type] ?? 0;
  const techGlobalDefense = defenderTechBonuses?.globalDefense ?? 0;
  const baseDefense = defenderType.defense + techDefenseBonus;
  const effectiveDefense = baseDefense * (1 + terrainBonus + techGlobalDefense / 100);

  const rawDamage = effectiveAttack - effectiveDefense;

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
  defenderHasWalls: boolean = false,
  attackerTechBonuses?: SoldierTechBonuses,
  defenderTechBonuses?: SoldierTechBonuses
): CombatResult {
  // Store initial state for replay
  const initialAttackerSoldiers = attacker.soldiers.map(s => ({ ...s }));
  const initialDefenderSoldiers = defender.soldiers.map(s => ({ ...s }));

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

  // Track damage dealt and combat events
  let attackerDamageDealt = 0;
  let defenderDamageDealt = 0;
  const events: CombatEvent[] = [];
  const deadSoldiers = new Set<string>();

  // Execute attacks
  for (const attack of allAttacks) {
    const friendlySoldiers = attack.isAttacker ? attackerSoldiers : defenderSoldiers;
    const enemySoldiers = attack.isAttacker ? defenderSoldiers : attackerSoldiers;

    const attackingSoldier = friendlySoldiers.find(s => s.id === attack.attackerId);
    if (!attackingSoldier || attackingSoldier.hp <= 0) continue; // Dead soldiers don't attack

    const attackerType = SOLDIER_TYPES[attackingSoldier.type];

    // Special case: Cleric heals allies instead of attacking
    if (attackingSoldier.type === 'cleric') {
      // Find lowest HP friendly soldier
      const wounded = friendlySoldiers
        .filter(s => s.hp > 0 && s.hp < s.maxHp)
        .sort((a, b) => a.hp / a.maxHp - b.hp / b.maxHp);

      if (wounded.length > 0) {
        const healAmount = 30;
        const actualHeal = Math.min(healAmount, wounded[0].maxHp - wounded[0].hp);
        wounded[0].hp = Math.min(wounded[0].maxHp, wounded[0].hp + healAmount);

        events.push({
          type: 'heal',
          timestamp: attack.timestamp,
          attackerId: attackingSoldier.id,
          attackerIsPlayer: attack.isAttacker,
          targetId: wounded[0].id,
          healing: actualHeal,
        });
      }
      continue;
    }

    // Find target
    const target = findTarget(attackingSoldier, enemySoldiers, attackerType.attacksTarget);
    if (!target) continue;

    // Apply defender terrain bonus only to defender side
    const bonus = attack.isAttacker ? terrainBonus : 0;
    // Tech bonuses: attacker's attack bonus vs target's defense bonus
    const currentAttackerTech = attack.isAttacker ? attackerTechBonuses : defenderTechBonuses;
    const currentDefenderTech = attack.isAttacker ? defenderTechBonuses : attackerTechBonuses;
    const damage = calculateDamage(attackingSoldier, target, bonus, currentAttackerTech, currentDefenderTech);

    const hpBefore = target.hp;
    target.hp -= damage;

    // Record attack event
    events.push({
      type: 'attack',
      timestamp: attack.timestamp,
      attackerId: attackingSoldier.id,
      attackerIsPlayer: attack.isAttacker,
      targetId: target.id,
      damage: damage,
    });

    // Check for death
    if (target.hp <= 0 && hpBefore > 0 && !deadSoldiers.has(target.id)) {
      deadSoldiers.add(target.id);
      events.push({
        type: 'death',
        timestamp: attack.timestamp + 0.1, // Slightly after the attack
        attackerId: target.id,
        attackerIsPlayer: !attack.isAttacker, // The one who died
      });
    }

    if (attack.isAttacker) {
      attackerDamageDealt += damage;
    } else {
      defenderDamageDealt += damage;
    }
  }

  // Determine casualties and survivors
  const attackerCasualties = attackerSoldiers
    .filter(s => s.hp <= 0)
    .map(s => s.id);
  const defenderCasualties = defenderSoldiers
    .filter(s => s.hp <= 0)
    .map(s => s.id);

  // Survivors with updated HP
  const attackerSurvivors = attackerSoldiers.filter(s => s.hp > 0);
  const defenderSurvivors = defenderSoldiers.filter(s => s.hp > 0);

  // Determine winner (defender wins ties)
  const attackerWon = attackerDamageDealt > defenderDamageDealt;

  return {
    attackerWon,
    attackerCasualties,
    defenderCasualties,
    attackerDamageDealt,
    defenderDamageDealt,
    attackerSurvivors,
    defenderSurvivors,
    events,
    initialAttackerSoldiers,
    initialDefenderSoldiers,
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
