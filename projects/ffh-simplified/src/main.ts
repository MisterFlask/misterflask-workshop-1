import { createInitialGameState, processAction, getValidMoves } from './game/Game';
import { Renderer } from './rendering/Renderer';
import type { GameState, Coord } from './types';

class GameApp {
  private state: GameState;
  private renderer: Renderer;
  private canvas: HTMLCanvasElement;
  private isDragging = false;
  private lastMousePos = { x: 0, y: 0 };

  constructor() {
    this.canvas = document.getElementById('game-canvas') as HTMLCanvasElement;
    this.renderer = new Renderer(this.canvas);
    this.state = createInitialGameState();

    this.setupEventListeners();
    this.updateUI();
    this.gameLoop();

    console.log('FFH Simplified initialized!');
    console.log('State:', this.state);
  }

  private setupEventListeners(): void {
    // Canvas mouse events
    this.canvas.addEventListener('mousedown', (e) => this.onMouseDown(e));
    this.canvas.addEventListener('mousemove', (e) => this.onMouseMove(e));
    this.canvas.addEventListener('mouseup', () => this.onMouseUp());
    this.canvas.addEventListener('mouseleave', () => this.onMouseUp());
    this.canvas.addEventListener('click', (e) => this.onClick(e));

    // End turn button
    const endTurnBtn = document.getElementById('end-turn-btn');
    endTurnBtn?.addEventListener('click', () => this.endTurn());

    // Keyboard
    document.addEventListener('keydown', (e) => this.onKeyDown(e));
  }

  private onMouseDown(e: MouseEvent): void {
    if (e.button === 1 || (e.button === 0 && e.shiftKey)) {
      // Middle mouse or shift+left for panning
      this.isDragging = true;
      this.lastMousePos = { x: e.clientX, y: e.clientY };
    }
  }

  private onMouseMove(e: MouseEvent): void {
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    if (this.isDragging) {
      const dx = e.clientX - this.lastMousePos.x;
      const dy = e.clientY - this.lastMousePos.y;
      this.renderer.setCamera(
        this.renderer['uiState'].camera.x - dx,
        this.renderer['uiState'].camera.y - dy
      );
      this.lastMousePos = { x: e.clientX, y: e.clientY };
    } else {
      const worldPos = this.renderer.screenToWorld(x, y);
      this.renderer.setHoveredTile(worldPos);
    }
  }

  private onMouseUp(): void {
    this.isDragging = false;
  }

  private onClick(e: MouseEvent): void {
    if (this.state.phase !== 'player_turn') return;

    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const worldPos = this.renderer.screenToWorld(x, y);

    // Validate coordinates
    if (
      worldPos.x < 0 ||
      worldPos.x >= this.state.mapWidth ||
      worldPos.y < 0 ||
      worldPos.y >= this.state.mapHeight
    ) {
      return;
    }

    this.state = processAction(this.state, {
      type: 'select_tile',
      coord: worldPos,
    });

    this.updateValidMoves();
    this.updateUI();
  }

  private onKeyDown(e: KeyboardEvent): void {
    if (e.key === 'Enter' || e.key === ' ') {
      this.endTurn();
    }
    if (e.key === 'Escape') {
      this.state = { ...this.state, selectedLegionId: null, selectedCityId: null };
      this.updateValidMoves();
    }
  }

  private endTurn(): void {
    if (this.state.phase !== 'player_turn') return;

    this.state = processAction(this.state, { type: 'end_turn' });
    this.updateUI();
    this.updateValidMoves();
  }

  private updateValidMoves(): void {
    if (this.state.selectedLegionId) {
      const legion = this.state.legions.get(this.state.selectedLegionId);
      if (legion && legion.owner === 'player') {
        this.renderer.setValidMoves(getValidMoves(this.state, legion));
        return;
      }
    }
    this.renderer.setValidMoves([]);
  }

  private updateUI(): void {
    const player = this.state.factions.get('player');

    // Update resource display
    const goldEl = document.getElementById('gold-amount');
    const manaEl = document.getElementById('mana-amount');
    const turnEl = document.getElementById('turn-indicator');
    const armageddonEl = document.getElementById('armageddon-value');
    const armageddonFill = document.getElementById('armageddon-fill');

    if (goldEl && player) goldEl.textContent = player.gold.toString();
    if (manaEl && player) manaEl.textContent = player.mana.toString();
    if (turnEl) turnEl.textContent = `Turn ${this.state.turn}`;
    if (armageddonEl) armageddonEl.textContent = `${this.state.armageddonCounter}/100`;
    if (armageddonFill) armageddonFill.style.width = `${this.state.armageddonCounter}%`;

    // Update selection panel
    this.updateSelectionPanel();

    // Game over check
    if (this.state.gameOver) {
      const message = this.state.winner === 'player'
        ? 'Victory! You have defeated the Infernal Legion!'
        : 'Defeat! Your empire has fallen.';
      alert(message);
    }
  }

  private updateSelectionPanel(): void {
    const panel = document.getElementById('selection-panel');
    if (!panel) return;

    if (this.state.selectedCityId) {
      const city = this.state.cities.get(this.state.selectedCityId);
      if (city) {
        panel.style.display = 'block';
        panel.innerHTML = `
          <h3>${city.name}</h3>
          <p>Population: ${city.population}</p>
          <p>Buildings: ${city.buildings.length > 0 ? city.buildings.join(', ') : 'None'}</p>
          ${city.occupationTurns > 0 ? `<p style="color: #f88">Occupied (${city.occupationTurns} turns)</p>` : ''}
        `;
        return;
      }
    }

    if (this.state.selectedLegionId) {
      const legion = this.state.legions.get(this.state.selectedLegionId);
      if (legion) {
        panel.style.display = 'block';
        panel.innerHTML = `
          <h3>Legion</h3>
          <p>Soldiers: ${legion.soldiers.length}/8</p>
          <p>Movement: ${legion.movementRemaining}/3</p>
          <p>Composition:</p>
          <ul style="margin-left: 16px; font-size: 12px">
            ${legion.soldiers.map(s => `<li>${s.type} (${s.hp}/${s.maxHp} HP)</li>`).join('')}
          </ul>
        `;
        return;
      }
    }

    panel.style.display = 'none';
  }

  private gameLoop(): void {
    this.renderer.render(this.state);
    requestAnimationFrame(() => this.gameLoop());
  }
}

// Start the game when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  new GameApp();
});
