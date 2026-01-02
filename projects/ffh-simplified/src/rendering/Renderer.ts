import type { GameState, Coord, Tile, Legion, City, UIState } from '../types';
import { coordsEqual } from '../utils/grid';
import { getValidMoves, getLegionAt, getCityAt } from '../game/Game';

const TILE_SIZE = 32;

const TERRAIN_COLORS: Record<string, string> = {
  grass: '#4a7c23',
  forest: '#2d5016',
  hills: '#8b7355',
  mountain: '#6b6b6b',
  water: '#2266aa',
};

const FACTION_COLORS: Record<string, string> = {
  player: '#4a90d9',
  hippus: '#d4a24c',
  sheaim: '#8b0000',
  elves: '#228b22',
  boss: '#ff4500',
};

export class Renderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private uiState: UIState;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d')!;
    this.uiState = {
      hoveredTile: null,
      validMoves: [],
      showingCityPanel: false,
      showingLegionPanel: false,
      camera: { x: 0, y: 0, zoom: 1 },
    };

    this.resize();
    window.addEventListener('resize', () => this.resize());
  }

  private resize(): void {
    const container = this.canvas.parentElement;
    if (container) {
      this.canvas.width = container.clientWidth;
      this.canvas.height = container.clientHeight;
    }
  }

  setHoveredTile(coord: Coord | null): void {
    this.uiState.hoveredTile = coord;
  }

  setValidMoves(moves: Coord[]): void {
    this.uiState.validMoves = moves;
  }

  setCamera(x: number, y: number): void {
    this.uiState.camera.x = x;
    this.uiState.camera.y = y;
  }

  screenToWorld(screenX: number, screenY: number): Coord {
    const { x: camX, y: camY, zoom } = this.uiState.camera;
    return {
      x: Math.floor((screenX / zoom + camX) / TILE_SIZE),
      y: Math.floor((screenY / zoom + camY) / TILE_SIZE),
    };
  }

  worldToScreen(coord: Coord): { x: number; y: number } {
    const { x: camX, y: camY, zoom } = this.uiState.camera;
    return {
      x: (coord.x * TILE_SIZE - camX) * zoom,
      y: (coord.y * TILE_SIZE - camY) * zoom,
    };
  }

  render(state: GameState): void {
    const { ctx, canvas } = this;
    const { camera } = this.uiState;

    // Clear
    ctx.fillStyle = '#1a1a2e';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    ctx.save();
    ctx.scale(camera.zoom, camera.zoom);
    ctx.translate(-camera.x, -camera.y);

    // Render map
    this.renderMap(state);

    // Render valid moves
    this.renderValidMoves(state);

    // Render cities
    this.renderCities(state);

    // Render legions
    this.renderLegions(state);

    // Render selection
    this.renderSelection(state);

    // Render hover
    this.renderHover(state);

    ctx.restore();
  }

  private renderMap(state: GameState): void {
    const { ctx } = this;

    for (let y = 0; y < state.mapHeight; y++) {
      for (let x = 0; x < state.mapWidth; x++) {
        const tile = state.map[y][x];
        const screenX = x * TILE_SIZE;
        const screenY = y * TILE_SIZE;

        // Terrain
        ctx.fillStyle = TERRAIN_COLORS[tile.terrain] || '#333';
        ctx.fillRect(screenX, screenY, TILE_SIZE, TILE_SIZE);

        // Territory border
        if (tile.owner) {
          ctx.strokeStyle = FACTION_COLORS[tile.owner] || '#fff';
          ctx.lineWidth = 2;
          ctx.strokeRect(screenX + 1, screenY + 1, TILE_SIZE - 2, TILE_SIZE - 2);
        }

        // Grid lines
        ctx.strokeStyle = 'rgba(0, 0, 0, 0.3)';
        ctx.lineWidth = 1;
        ctx.strokeRect(screenX, screenY, TILE_SIZE, TILE_SIZE);
      }
    }
  }

  private renderValidMoves(_state: GameState): void {
    const { ctx } = this;

    for (const move of this.uiState.validMoves) {
      const screenX = move.x * TILE_SIZE;
      const screenY = move.y * TILE_SIZE;

      ctx.fillStyle = 'rgba(100, 200, 100, 0.4)';
      ctx.fillRect(screenX + 2, screenY + 2, TILE_SIZE - 4, TILE_SIZE - 4);
    }
  }

  private renderCities(state: GameState): void {
    const { ctx } = this;

    for (const city of state.cities.values()) {
      const screenX = city.coord.x * TILE_SIZE;
      const screenY = city.coord.y * TILE_SIZE;

      // City background
      ctx.fillStyle = FACTION_COLORS[city.owner] || '#888';
      ctx.fillRect(screenX + 4, screenY + 4, TILE_SIZE - 8, TILE_SIZE - 8);

      // City icon (simple house shape)
      ctx.fillStyle = '#fff';
      ctx.beginPath();
      ctx.moveTo(screenX + TILE_SIZE / 2, screenY + 8);
      ctx.lineTo(screenX + TILE_SIZE - 8, screenY + 16);
      ctx.lineTo(screenX + TILE_SIZE - 8, screenY + TILE_SIZE - 8);
      ctx.lineTo(screenX + 8, screenY + TILE_SIZE - 8);
      ctx.lineTo(screenX + 8, screenY + 16);
      ctx.closePath();
      ctx.fill();

      // Population indicator
      ctx.fillStyle = '#000';
      ctx.font = '10px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(
        city.population.toString(),
        screenX + TILE_SIZE / 2,
        screenY + TILE_SIZE - 2
      );
    }
  }

  private renderLegions(state: GameState): void {
    const { ctx } = this;

    for (const legion of state.legions.values()) {
      const screenX = legion.location.x * TILE_SIZE;
      const screenY = legion.location.y * TILE_SIZE;

      // Legion marker (circle with faction color)
      ctx.fillStyle = FACTION_COLORS[legion.owner] || '#888';
      ctx.beginPath();
      ctx.arc(
        screenX + TILE_SIZE / 2,
        screenY + TILE_SIZE / 2,
        TILE_SIZE / 3,
        0,
        Math.PI * 2
      );
      ctx.fill();

      // Soldier count
      ctx.fillStyle = '#fff';
      ctx.font = 'bold 12px sans-serif';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(
        legion.soldiers.length.toString(),
        screenX + TILE_SIZE / 2,
        screenY + TILE_SIZE / 2
      );

      // Movement indicator
      if (legion.owner === 'player' && legion.movementRemaining > 0) {
        ctx.strokeStyle = '#0f0';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.arc(
          screenX + TILE_SIZE / 2,
          screenY + TILE_SIZE / 2,
          TILE_SIZE / 3 + 3,
          0,
          Math.PI * 2
        );
        ctx.stroke();
      }
    }
  }

  private renderSelection(state: GameState): void {
    const { ctx } = this;

    if (state.selectedLegionId) {
      const legion = state.legions.get(state.selectedLegionId);
      if (legion) {
        const screenX = legion.location.x * TILE_SIZE;
        const screenY = legion.location.y * TILE_SIZE;

        ctx.strokeStyle = '#fff';
        ctx.lineWidth = 3;
        ctx.strokeRect(screenX, screenY, TILE_SIZE, TILE_SIZE);
      }
    }

    if (state.selectedCityId) {
      const city = state.cities.get(state.selectedCityId);
      if (city) {
        const screenX = city.coord.x * TILE_SIZE;
        const screenY = city.coord.y * TILE_SIZE;

        ctx.strokeStyle = '#fff';
        ctx.lineWidth = 3;
        ctx.strokeRect(screenX, screenY, TILE_SIZE, TILE_SIZE);
      }
    }
  }

  private renderHover(_state: GameState): void {
    const { ctx } = this;
    const { hoveredTile } = this.uiState;

    if (hoveredTile) {
      const screenX = hoveredTile.x * TILE_SIZE;
      const screenY = hoveredTile.y * TILE_SIZE;

      ctx.strokeStyle = 'rgba(255, 255, 255, 0.5)';
      ctx.lineWidth = 2;
      ctx.strokeRect(screenX + 1, screenY + 1, TILE_SIZE - 2, TILE_SIZE - 2);
    }
  }
}
