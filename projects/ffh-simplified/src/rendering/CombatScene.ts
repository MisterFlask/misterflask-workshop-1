import type { Soldier, CombatResult, CombatEvent } from '../types';
import { SOLDIER_TYPES } from '../data/soldiers';

const SCENE_WIDTH = 800;
const SCENE_HEIGHT = 400;
const SOLDIER_SIZE = 48;
const PLAYBACK_SPEED = 50; // ms per combat timestamp unit

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

export class CombatScene {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private result: CombatResult;
  private soldierStates: Map<string, SoldierState> = new Map();
  private damagePopups: DamagePopup[] = [];
  private currentTime: number = 0;
  private eventIndex: number = 0;
  private isPlaying: boolean = false;
  private isComplete: boolean = false;
  private onComplete: (() => void) | null = null;
  private lastFrameTime: number = 0;

  constructor(canvas: HTMLCanvasElement, result: CombatResult, onComplete: () => void) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d')!;
    this.result = result;
    this.onComplete = onComplete;

    // Initialize soldier states from initial positions
    this.initializeSoldiers();
  }

  private initializeSoldiers(): void {
    // Position attacker soldiers on left side
    const attackerCount = this.result.initialAttackerSoldiers.length;
    for (let i = 0; i < attackerCount; i++) {
      const soldier = this.result.initialAttackerSoldiers[i];
      const row = Math.floor(i / 3);
      const col = i % 3;
      this.soldierStates.set(soldier.id, {
        soldier,
        x: 100 + col * 60,
        y: 100 + row * 80,
        hp: soldier.hp,
        maxHp: soldier.maxHp,
        isPlayer: true,
        isDead: false,
        animationState: 'idle',
        animationTimer: 0,
      });
    }

    // Position defender soldiers on right side
    const defenderCount = this.result.initialDefenderSoldiers.length;
    for (let i = 0; i < defenderCount; i++) {
      const soldier = this.result.initialDefenderSoldiers[i];
      const row = Math.floor(i / 3);
      const col = i % 3;
      this.soldierStates.set(soldier.id, {
        soldier,
        x: SCENE_WIDTH - 100 - col * 60,
        y: 100 + row * 80,
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
    // Advance combat time
    this.currentTime += delta / PLAYBACK_SPEED;

    // Process events up to current time
    while (this.eventIndex < this.result.events.length) {
      const event = this.result.events[this.eventIndex];
      if (event.timestamp > this.currentTime) break;

      this.processEvent(event);
      this.eventIndex++;
    }

    // Update animations
    for (const state of this.soldierStates.values()) {
      if (state.animationTimer > 0) {
        state.animationTimer -= delta;
        if (state.animationTimer <= 0) {
          state.animationState = state.isDead ? 'dead' : 'idle';
        }
      }
    }

    // Update damage popups
    this.damagePopups = this.damagePopups.filter(p => {
      p.age += delta;
      p.y -= delta * 0.05; // Float upward
      return p.age < 1000; // Remove after 1 second
    });

    // Check if combat is complete
    if (this.eventIndex >= this.result.events.length && this.damagePopups.length === 0) {
      // Wait a moment before completing
      setTimeout(() => {
        this.isComplete = true;
        this.isPlaying = false;
        if (this.onComplete) {
          this.onComplete();
        }
      }, 1500);
    }
  }

  private processEvent(event: CombatEvent): void {
    const attackerState = this.soldierStates.get(event.attackerId);

    switch (event.type) {
      case 'attack': {
        if (attackerState) {
          attackerState.animationState = 'attacking';
          attackerState.animationTimer = 300;
        }

        const targetState = this.soldierStates.get(event.targetId!);
        if (targetState && event.damage) {
          targetState.hp = Math.max(0, targetState.hp - event.damage);
          targetState.animationState = 'hurt';
          targetState.animationTimer = 200;

          // Add damage popup
          this.damagePopups.push({
            x: targetState.x,
            y: targetState.y - 20,
            value: event.damage,
            isHeal: false,
            age: 0,
          });
        }
        break;
      }

      case 'heal': {
        if (attackerState) {
          attackerState.animationState = 'attacking';
          attackerState.animationTimer = 300;
        }

        const targetState = this.soldierStates.get(event.targetId!);
        if (targetState && event.healing) {
          targetState.hp = Math.min(targetState.maxHp, targetState.hp + event.healing);

          // Add heal popup
          this.damagePopups.push({
            x: targetState.x,
            y: targetState.y - 20,
            value: event.healing,
            isHeal: true,
            age: 0,
          });
        }
        break;
      }

      case 'death': {
        if (attackerState) {
          attackerState.isDead = true;
          attackerState.animationState = 'dead';
          attackerState.animationTimer = 0;
        }
        break;
      }
    }
  }

  private render(): void {
    const { ctx } = this;

    // Clear with dark background
    ctx.fillStyle = '#1a1a2e';
    ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

    // Draw battlefield divider
    ctx.strokeStyle = '#444';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(SCENE_WIDTH / 2, 50);
    ctx.lineTo(SCENE_WIDTH / 2, SCENE_HEIGHT - 50);
    ctx.stroke();

    // Draw labels
    ctx.font = 'bold 18px sans-serif';
    ctx.fillStyle = '#4af';
    ctx.textAlign = 'center';
    ctx.fillText('YOUR ARMY', 150, 40);
    ctx.fillStyle = '#f44';
    ctx.fillText('ENEMY ARMY', SCENE_WIDTH - 150, 40);

    // Draw soldiers
    for (const state of this.soldierStates.values()) {
      this.renderSoldier(state);
    }

    // Draw damage popups
    for (const popup of this.damagePopups) {
      this.renderDamagePopup(popup);
    }

    // Draw result if combat is complete
    if (this.eventIndex >= this.result.events.length) {
      ctx.font = 'bold 32px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillStyle = this.result.attackerWon ? '#4f4' : '#f44';
      ctx.fillText(
        this.result.attackerWon ? 'VICTORY!' : 'DEFEAT!',
        SCENE_WIDTH / 2,
        SCENE_HEIGHT - 30
      );
    }
  }

  private renderSoldier(state: SoldierState): void {
    const { ctx } = this;
    const { x, y, soldier, hp, maxHp, animationState, isPlayer } = state;

    // Animation offsets
    let offsetX = 0;
    let offsetY = 0;
    let alpha = 1;

    switch (animationState) {
      case 'attacking':
        offsetX = isPlayer ? 15 : -15;
        break;
      case 'hurt':
        offsetX = isPlayer ? -5 : 5;
        break;
      case 'dead':
        alpha = 0.4;
        offsetY = 10;
        break;
    }

    ctx.globalAlpha = alpha;

    // Draw soldier body (simple colored rectangle based on type)
    const soldierType = SOLDIER_TYPES[soldier.type];
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

    // Body
    ctx.fillStyle = color;
    ctx.fillRect(x + offsetX - SOLDIER_SIZE / 2, y + offsetY - SOLDIER_SIZE / 2, SOLDIER_SIZE, SOLDIER_SIZE);

    // Border
    ctx.strokeStyle = isPlayer ? '#4af' : '#f44';
    ctx.lineWidth = 3;
    ctx.strokeRect(x + offsetX - SOLDIER_SIZE / 2, y + offsetY - SOLDIER_SIZE / 2, SOLDIER_SIZE, SOLDIER_SIZE);

    // Type label
    ctx.font = 'bold 10px sans-serif';
    ctx.fillStyle = '#000';
    ctx.textAlign = 'center';
    ctx.fillText(soldier.type.slice(0, 3).toUpperCase(), x + offsetX, y + offsetY + 4);

    // HP bar background
    const barWidth = SOLDIER_SIZE;
    const barHeight = 6;
    const barX = x + offsetX - barWidth / 2;
    const barY = y + offsetY + SOLDIER_SIZE / 2 + 4;

    ctx.fillStyle = '#333';
    ctx.fillRect(barX, barY, barWidth, barHeight);

    // HP bar fill
    const hpPercent = hp / maxHp;
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
    const alpha = 1 - popup.age / 1000;

    ctx.globalAlpha = alpha;
    ctx.font = 'bold 20px sans-serif';
    ctx.textAlign = 'center';
    ctx.fillStyle = popup.isHeal ? '#4f4' : '#f44';
    ctx.fillText(
      popup.isHeal ? `+${popup.value}` : `-${popup.value}`,
      popup.x,
      popup.y
    );
    ctx.globalAlpha = 1;
  }

  stop(): void {
    this.isPlaying = false;
  }
}
