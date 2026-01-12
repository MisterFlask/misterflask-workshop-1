import type { GameState, Coord, Tile, Legion, City, UIState, TerrainFeatureId } from '../types';
import { coordsEqual } from '../utils/grid';
import { getValidMoves, getLegionAt, getCityAt } from '../game/Game';
import { TERRAIN_FEATURES } from '../data/terrainFeatures';

const TILE_SIZE = 32;

const TERRAIN_COLORS: Record<string, string> = {
  grass: '#4a7c23',
  forest: '#2d5016',
  hills: '#8b7355',
  mountain: '#6b6b6b',
  water: '#2266aa',
  swamp: '#4a6030',
};

const FACTION_COLORS: Record<string, string> = {
  player: '#4a90d9',
  hippus: '#d4a24c',
  sheaim: '#8b0000',
  elves: '#228b22',
  boss: '#ff4500',
};

// Icons for terrain features (simple text symbols for now)
const FEATURE_ICONS: Record<TerrainFeatureId, { symbol: string; color: string }> = {
  // Common features
  ancient_ruins: { symbol: '⌂', color: '#a08060' },
  mana_spring: { symbol: '✧', color: '#8080ff' },
  iron_vein: { symbol: '⚒', color: '#708090' },
  gold_mine: { symbol: '◆', color: '#ffd700' },
  sacred_grove: { symbol: '❀', color: '#90ee90' },
  watchtower: { symbol: '▲', color: '#808080' },
  fertile_plains: { symbol: '✿', color: '#90c020' },
  // Uncommon features
  haunted_barrow: { symbol: '☠', color: '#9060a0' },
  dragon_bones: { symbol: '☆', color: '#ff8844' },
  crystal_cave: { symbol: '◇', color: '#88ffff' },
  mineral_deposit: { symbol: '●', color: '#c0c0c0' },
  // Rare features
  adamantine_vein: { symbol: '★', color: '#4080ff' },
  world_tree: { symbol: '♠', color: '#00ff80' },
  // Legendary features
  hellgate: { symbol: '☼', color: '#ff2020' },
  titans_grave: { symbol: '⚑', color: '#c080ff' },
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
      movementPath: [],
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

  setMovementPath(path: Coord[]): void {
    this.uiState.movementPath = path;
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

    // Render movement path preview
    this.renderMovementPath(state);

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

        // Terrain base color
        ctx.fillStyle = TERRAIN_COLORS[tile.terrain] || '#333';
        ctx.fillRect(screenX, screenY, TILE_SIZE, TILE_SIZE);

        // Cultural territory overlay (semi-transparent faction color)
        if (tile.owner) {
          const factionColor = FACTION_COLORS[tile.owner] || '#fff';
          ctx.fillStyle = this.hexToRgba(factionColor, 0.2);
          ctx.fillRect(screenX, screenY, TILE_SIZE, TILE_SIZE);
        }

        // Terrain feature icon
        if (tile.feature) {
          const featureInfo = FEATURE_ICONS[tile.feature];
          if (featureInfo) {
            ctx.fillStyle = featureInfo.color;
            ctx.font = 'bold 14px sans-serif';
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';
            ctx.fillText(featureInfo.symbol, screenX + TILE_SIZE / 2, screenY + TILE_SIZE / 2);
          }
        }

        // Cultural border edge (only draw where adjacent tile has different owner)
        if (tile.owner) {
          this.renderBorderEdges(state, x, y, tile.owner);
        }

        // Grid lines
        ctx.strokeStyle = 'rgba(0, 0, 0, 0.2)';
        ctx.lineWidth = 1;
        ctx.strokeRect(screenX, screenY, TILE_SIZE, TILE_SIZE);
      }
    }
  }

  // Render thick border edges where territory meets different/no territory
  private renderBorderEdges(state: GameState, x: number, y: number, owner: string): void {
    const { ctx } = this;
    const screenX = x * TILE_SIZE;
    const screenY = y * TILE_SIZE;
    const color = FACTION_COLORS[owner] || '#fff';

    ctx.strokeStyle = color;
    ctx.lineWidth = 2;

    // Check each adjacent tile
    const neighbors = [
      { dx: 0, dy: -1, side: 'top' },    // top
      { dx: 1, dy: 0, side: 'right' },   // right
      { dx: 0, dy: 1, side: 'bottom' },  // bottom
      { dx: -1, dy: 0, side: 'left' },   // left
    ];

    for (const { dx, dy, side } of neighbors) {
      const nx = x + dx;
      const ny = y + dy;

      // Check if neighbor is different territory
      let isDifferent = true;
      if (nx >= 0 && nx < state.mapWidth && ny >= 0 && ny < state.mapHeight) {
        const neighborTile = state.map[ny][nx];
        isDifferent = neighborTile.owner !== owner;
      }

      if (isDifferent) {
        ctx.beginPath();
        switch (side) {
          case 'top':
            ctx.moveTo(screenX, screenY);
            ctx.lineTo(screenX + TILE_SIZE, screenY);
            break;
          case 'right':
            ctx.moveTo(screenX + TILE_SIZE, screenY);
            ctx.lineTo(screenX + TILE_SIZE, screenY + TILE_SIZE);
            break;
          case 'bottom':
            ctx.moveTo(screenX, screenY + TILE_SIZE);
            ctx.lineTo(screenX + TILE_SIZE, screenY + TILE_SIZE);
            break;
          case 'left':
            ctx.moveTo(screenX, screenY);
            ctx.lineTo(screenX, screenY + TILE_SIZE);
            break;
        }
        ctx.stroke();
      }
    }
  }

  // Helper to convert hex color to rgba
  private hexToRgba(hex: string, alpha: number): string {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
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

  private renderMovementPath(_state: GameState): void {
    const { ctx } = this;
    const path = this.uiState.movementPath;

    if (path.length < 2) return;

    // Draw path line connecting tiles
    ctx.strokeStyle = 'rgba(255, 255, 100, 0.8)';
    ctx.lineWidth = 3;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';

    ctx.beginPath();
    const startX = path[0].x * TILE_SIZE + TILE_SIZE / 2;
    const startY = path[0].y * TILE_SIZE + TILE_SIZE / 2;
    ctx.moveTo(startX, startY);

    for (let i = 1; i < path.length; i++) {
      const x = path[i].x * TILE_SIZE + TILE_SIZE / 2;
      const y = path[i].y * TILE_SIZE + TILE_SIZE / 2;
      ctx.lineTo(x, y);
    }
    ctx.stroke();

    // Draw dots at each waypoint
    ctx.fillStyle = 'rgba(255, 255, 100, 0.9)';
    for (let i = 1; i < path.length - 1; i++) {
      const x = path[i].x * TILE_SIZE + TILE_SIZE / 2;
      const y = path[i].y * TILE_SIZE + TILE_SIZE / 2;
      ctx.beginPath();
      ctx.arc(x, y, 4, 0, Math.PI * 2);
      ctx.fill();
    }

    // Draw larger dot at destination
    if (path.length > 1) {
      const dest = path[path.length - 1];
      const x = dest.x * TILE_SIZE + TILE_SIZE / 2;
      const y = dest.y * TILE_SIZE + TILE_SIZE / 2;
      ctx.fillStyle = 'rgba(255, 200, 50, 1)';
      ctx.beginPath();
      ctx.arc(x, y, 6, 0, Math.PI * 2);
      ctx.fill();
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
