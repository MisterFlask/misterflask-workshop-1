import type { Soldier, CombatResult, CombatEvent } from '../types';
import { SOLDIER_TYPES } from '../data/soldiers';

const SCENE_WIDTH = 800;
const SCENE_HEIGHT = 400;
const SOLDIER_SIZE = 48;
const EVENT_DURATION = 800; // ms per event (slower = clearer)
const ATTACK_ANIMATION_TIME = 400; // ms for attack animation
const PAUSE_BETWEEN_EVENTS = 200; // ms pause after each event

interface SoldierState {
  soldier: Soldier;
  x: number;
  y: number;
  hp: number;
  maxHp: number;
  isPlayer: boolean;
  isDead: boolean;
  animationState: 'idle' | 'attacking' | 'hurt' | 'dead';
  animationTimer: number;
}

interface DamagePopup {
  x: number;
  y: number;
  value: number;
  isHeal: boolean;
  age: number;
}

interface AttackLine {
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  progress: number; // 0 to 1
  isHeal: boolean;
}

export class CombatScene {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private result: CombatResult;
  private soldierStates: Map<string, SoldierState> = new Map();
  private damagePopups: DamagePopup[] = [];
  private eventIndex: number = 0;
  private isPlaying: boolean = false;
  private isComplete: boolean = false;
  private onComplete: (() => void) | null = null;
  private lastFrameTime: number = 0;

  // Current action tracking
  private currentAttackerId: string | null = null;
  private currentTargetId: string | null = null;
  private attackLine: AttackLine | null = null;
  private eventTimer: number = 0;
  private waitingForNextEvent: boolean = false;
  private waitTimer: number = 0;

  constructor(canvas: HTMLCanvasElement, result: CombatResult, onComplete: () => void) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d')!;
    this.result = result;
    this.onComplete = onComplete;

    // Initialize soldier states from initial positions
    this.initializeSoldiers();
  }

  private initializeSoldiers(): void {
    // Position attacker soldiers on left side in formation
    const attackerSoldiers = this.result.initialAttackerSoldiers;
    for (const soldier of attackerSoldiers) {
      const rowIndex = soldier.position.row === 'front' ? 0 : soldier.position.row === 'mid' ? 1 : 2;
      const col = soldier.position.column;
      this.soldierStates.set(soldier.id, {
        soldier,
        x: 80 + (2 - rowIndex) * 55, // front row closer to center
        y: 100 + col * 90,
        hp: soldier.hp,
        maxHp: soldier.maxHp,
        isPlayer: true,
        isDead: false,
        animationState: 'idle',
        animationTimer: 0,
      });
    }

    // Position defender soldiers on right side in formation
    const defenderSoldiers = this.result.initialDefenderSoldiers;
    for (const soldier of defenderSoldiers) {
      const rowIndex = soldier.position.row === 'front' ? 0 : soldier.position.row === 'mid' ? 1 : 2;
      const col = soldier.position.column;
      this.soldierStates.set(soldier.id, {
        soldier,
        x: SCENE_WIDTH - 80 - (2 - rowIndex) * 55, // front row closer to center
        y: 100 + col * 90,
        hp: soldier.hp,
        maxHp: soldier.maxHp,
        isPlayer: false,
        isDead: false,
        animationState: 'idle',
        animationTimer: 0,
      });
    }
  }

  start(): void {
    this.isPlaying = true;
    this.lastFrameTime = performance.now();
    // Start first event after a brief delay
    this.waitingForNextEvent = true;
    this.waitTimer = 500;
    this.gameLoop();
  }

  private gameLoop = (): void => {
    if (!this.isPlaying) return;

    const now = performance.now();
    const delta = now - this.lastFrameTime;
    this.lastFrameTime = now;

    this.update(delta);
    this.render();

    if (!this.isComplete) {
      requestAnimationFrame(this.gameLoop);
    }
  };

  private update(delta: number): void {
    // Handle waiting between events
    if (this.waitingForNextEvent) {
      this.waitTimer -= delta;
      if (this.waitTimer <= 0) {
        this.waitingForNextEvent = false;
        this.startNextEvent();
      }
    } else if (this.eventTimer > 0) {
      // Currently playing an event
      this.eventTimer -= delta;

      // Update attack line progress
      if (this.attackLine) {
        this.attackLine.progress = Math.min(1, 1 - (this.eventTimer / ATTACK_ANIMATION_TIME));
      }

      if (this.eventTimer <= 0) {
        // Event finished, wait before next
        this.finishCurrentEvent();
        this.waitingForNextEvent = true;
        this.waitTimer = PAUSE_BETWEEN_EVENTS;
      }
    }

    // Update animations
    for (const state of this.soldierStates.values()) {
      if (state.animationTimer > 0) {
        state.animationTimer -= delta;
        if (state.animationTimer <= 0 && state.animationState !== 'dead') {
          state.animationState = state.isDead ? 'dead' : 'idle';
        }
      }
    }

    // Update damage popups
    this.damagePopups = this.damagePopups.filter(p => {
      p.age += delta;
      p.y -= delta * 0.03; // Float upward slower
      return p.age < 1500; // Remove after 1.5 seconds
    });

    // Check if combat is complete
    if (this.eventIndex >= this.result.events.length &&
        !this.waitingForNextEvent &&
        this.eventTimer <= 0 &&
        this.damagePopups.length === 0) {
      // Wait a moment before completing
      setTimeout(() => {
        this.isComplete = true;
        this.isPlaying = false;
        if (this.onComplete) {
          this.onComplete();
        }
      }, 2000);
    }
  }

  private startNextEvent(): void {
    if (this.eventIndex >= this.result.events.length) return;

    const event = this.result.events[this.eventIndex];

    // Skip death events - they're handled when HP reaches 0
    if (event.type === 'death') {
      this.eventIndex++;
      this.waitingForNextEvent = true;
      this.waitTimer = 100;
      return;
    }

    this.currentAttackerId = event.attackerId;
    this.currentTargetId = event.targetId || null;
    this.eventTimer = EVENT_DURATION;

    const attackerState = this.soldierStates.get(event.attackerId);
    if (attackerState) {
      attackerState.animationState = 'attacking';
      attackerState.animationTimer = ATTACK_ANIMATION_TIME;
    }

    // Create attack line
    if (this.currentTargetId) {
      const targetState = this.soldierStates.get(this.currentTargetId);
      if (attackerState && targetState) {
        this.attackLine = {
          fromX: attackerState.x,
          fromY: attackerState.y,
          toX: targetState.x,
          toY: targetState.y,
          progress: 0,
          isHeal: event.type === 'heal',
        };
      }
    }
  }

  private finishCurrentEvent(): void {
    if (this.eventIndex >= this.result.events.length) return;

    const event = this.result.events[this.eventIndex];

    switch (event.type) {
      case 'attack': {
        const targetState = this.soldierStates.get(event.targetId!);
        if (targetState && event.damage) {
          targetState.hp = Math.max(0, targetState.hp - event.damage);
          targetState.animationState = 'hurt';
          targetState.animationTimer = 300;

          // Add damage popup
          this.damagePopups.push({
            x: targetState.x,
            y: targetState.y - 30,
            value: event.damage,
            isHeal: false,
            age: 0,
          });

          // Check for death
          if (targetState.hp <= 0 && !targetState.isDead) {
            targetState.isDead = true;
            targetState.animationState = 'dead';
          }
        }
        break;
      }

      case 'heal': {
        const targetState = this.soldierStates.get(event.targetId!);
        if (targetState && event.healing) {
          targetState.hp = Math.min(targetState.maxHp, targetState.hp + event.healing);

          // Add heal popup
          this.damagePopups.push({
            x: targetState.x,
            y: targetState.y - 30,
            value: event.healing,
            isHeal: true,
            age: 0,
          });
        }
        break;
      }
    }

    // Clear current action
    this.currentAttackerId = null;
    this.currentTargetId = null;
    this.attackLine = null;
    this.eventIndex++;
  }

  private render(): void {
    const { ctx } = this;

    // Clear with dark background
    ctx.fillStyle = '#1a1a2e';
    ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

    // Draw battlefield divider
    ctx.strokeStyle = '#333';
    ctx.lineWidth = 2;
    ctx.setLineDash([10, 10]);
    ctx.beginPath();
    ctx.moveTo(SCENE_WIDTH / 2, 50);
    ctx.lineTo(SCENE_WIDTH / 2, SCENE_HEIGHT - 50);
    ctx.stroke();
    ctx.setLineDash([]);

    // Draw labels
    ctx.font = 'bold 16px sans-serif';
    ctx.fillStyle = '#4af';
    ctx.textAlign = 'center';
    ctx.fillText('YOUR ARMY', 140, 35);
    ctx.fillStyle = '#f44';
    ctx.fillText('ENEMY ARMY', SCENE_WIDTH - 140, 35);

    // Draw attack line first (behind soldiers)
    if (this.attackLine && this.attackLine.progress > 0) {
      this.renderAttackLine();
    }

    // Draw soldiers
    for (const state of this.soldierStates.values()) {
      this.renderSoldier(state);
    }

    // Draw damage popups
    for (const popup of this.damagePopups) {
      this.renderDamagePopup(popup);
    }

    // Draw current action text
    if (this.currentAttackerId && this.eventIndex < this.result.events.length) {
      this.renderActionText();
    }

    // Draw result if combat is complete
    if (this.eventIndex >= this.result.events.length && !this.currentAttackerId) {
      ctx.font = 'bold 36px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillStyle = this.result.attackerWon ? '#4f4' : '#f44';
      ctx.shadowColor = this.result.attackerWon ? '#4f4' : '#f44';
      ctx.shadowBlur = 20;
      ctx.fillText(
        this.result.attackerWon ? 'VICTORY!' : 'DEFEAT!',
        SCENE_WIDTH / 2,
        SCENE_HEIGHT - 25
      );
      ctx.shadowBlur = 0;
    }
  }

  private renderAttackLine(): void {
    if (!this.attackLine) return;

    const { ctx } = this;
    const { fromX, fromY, toX, toY, progress, isHeal } = this.attackLine;

    // Calculate current end point based on progress
    const currentX = fromX + (toX - fromX) * progress;
    const currentY = fromY + (toY - fromY) * progress;

    // Draw the line
    ctx.strokeStyle = isHeal ? '#4f4' : '#f84';
    ctx.lineWidth = 4;
    ctx.lineCap = 'round';
    ctx.globalAlpha = 0.8;

    // Glow effect
    ctx.shadowColor = isHeal ? '#4f4' : '#f84';
    ctx.shadowBlur = 10;

    ctx.beginPath();
    ctx.moveTo(fromX, fromY);
    ctx.lineTo(currentX, currentY);
    ctx.stroke();

    // Draw arrowhead at current position
    if (progress > 0.3) {
      const angle = Math.atan2(toY - fromY, toX - fromX);
      const arrowSize = 12;

      ctx.fillStyle = isHeal ? '#4f4' : '#f84';
      ctx.beginPath();
      ctx.moveTo(currentX, currentY);
      ctx.lineTo(
        currentX - arrowSize * Math.cos(angle - Math.PI / 6),
        currentY - arrowSize * Math.sin(angle - Math.PI / 6)
      );
      ctx.lineTo(
        currentX - arrowSize * Math.cos(angle + Math.PI / 6),
        currentY - arrowSize * Math.sin(angle + Math.PI / 6)
      );
      ctx.closePath();
      ctx.fill();
    }

    ctx.shadowBlur = 0;
    ctx.globalAlpha = 1;
  }

  private renderActionText(): void {
    const { ctx } = this;
    const event = this.result.events[this.eventIndex];

    const attackerState = this.soldierStates.get(event.attackerId);
    const targetState = event.targetId ? this.soldierStates.get(event.targetId) : null;

    if (!attackerState) return;

    const attackerType = SOLDIER_TYPES[attackerState.soldier.type];
    const attackerName = attackerType.name;

    let actionText = '';
    if (event.type === 'attack' && targetState) {
      const targetType = SOLDIER_TYPES[targetState.soldier.type];
      actionText = `${attackerName} attacks ${targetType.name}!`;
    } else if (event.type === 'heal' && targetState) {
      const targetType = SOLDIER_TYPES[targetState.soldier.type];
      actionText = `${attackerName} heals ${targetType.name}!`;
    }

    // Draw action text at bottom
    ctx.font = 'bold 14px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillStyle = '#fff';
    ctx.fillText(actionText, SCENE_WIDTH / 2, SCENE_HEIGHT - 60);
  }

  private renderSoldier(state: SoldierState): void {
    const { ctx } = this;
    const { x, y, soldier, hp, maxHp, animationState, isPlayer, isDead } = state;

    const isAttacker = soldier.id === this.currentAttackerId;
    const isTarget = soldier.id === this.currentTargetId;

    // Animation offsets
    let offsetX = 0;
    let offsetY = 0;
    let alpha = 1;
    let scale = 1;

    switch (animationState) {
      case 'attacking':
        offsetX = isPlayer ? 20 : -20;
        scale = 1.1;
        break;
      case 'hurt':
        offsetX = isPlayer ? -8 : 8;
        break;
      case 'dead':
        alpha = 0.3;
        offsetY = 10;
        break;
    }

    ctx.globalAlpha = alpha;

    // Draw highlight ring for active attacker/target
    if (isAttacker || isTarget) {
      ctx.strokeStyle = isAttacker ? '#ff0' : '#f00';
      ctx.lineWidth = 3;
      ctx.shadowColor = isAttacker ? '#ff0' : '#f00';
      ctx.shadowBlur = 15;
      ctx.beginPath();
      ctx.arc(x + offsetX, y + offsetY, SOLDIER_SIZE / 2 + 8, 0, Math.PI * 2);
      ctx.stroke();
      ctx.shadowBlur = 0;
    }

    // Draw soldier body (simple colored rectangle based on type)
    const colors: Record<string, string> = {
      fighter: '#c84',
      archer: '#4c8',
      knight: '#88c',
      mage: '#c4c',
      cleric: '#fff',
      skeleton: '#888',
      demon: '#c44',
    };
    const color = colors[soldier.type] || '#aaa';

    const size = SOLDIER_SIZE * scale;

    // Body
    ctx.fillStyle = color;
    ctx.fillRect(x + offsetX - size / 2, y + offsetY - size / 2, size, size);

    // Border
    ctx.strokeStyle = isPlayer ? '#4af' : '#f44';
    ctx.lineWidth = 3;
    ctx.strokeRect(x + offsetX - size / 2, y + offsetY - size / 2, size, size);

    // Type label
    ctx.font = `bold ${11 * scale}px sans-serif`;
    ctx.fillStyle = '#000';
    ctx.textAlign = 'center';
    ctx.fillText(soldier.type.slice(0, 3).toUpperCase(), x + offsetX, y + offsetY + 4);

    // HP bar background
    const barWidth = size;
    const barHeight = 6;
    const barX = x + offsetX - barWidth / 2;
    const barY = y + offsetY + size / 2 + 4;

    ctx.fillStyle = '#333';
    ctx.fillRect(barX, barY, barWidth, barHeight);

    // HP bar fill
    const hpPercent = Math.max(0, hp / maxHp);
    ctx.fillStyle = hpPercent > 0.5 ? '#4f4' : hpPercent > 0.25 ? '#ff4' : '#f44';
    ctx.fillRect(barX, barY, barWidth * hpPercent, barHeight);

    // HP text
    ctx.font = '10px sans-serif';
    ctx.fillStyle = '#fff';
    ctx.textAlign = 'center';
    ctx.fillText(`${Math.max(0, hp)}/${maxHp}`, x + offsetX, barY + barHeight + 12);

    ctx.globalAlpha = 1;
  }

  private renderDamagePopup(popup: DamagePopup): void {
    const { ctx } = this;
    const alpha = 1 - popup.age / 1500;

    ctx.globalAlpha = alpha;
    ctx.font = 'bold 24px sans-serif';
    ctx.textAlign = 'center';

    // Add shadow for visibility
    ctx.shadowColor = '#000';
    ctx.shadowBlur = 4;

    ctx.fillStyle = popup.isHeal ? '#4f4' : '#f44';
    ctx.fillText(
      popup.isHeal ? `+${popup.value}` : `-${popup.value}`,
      popup.x,
      popup.y
    );

    ctx.shadowBlur = 0;
    ctx.globalAlpha = 1;
  }

  stop(): void {
    this.isPlaying = false;
  }
}
