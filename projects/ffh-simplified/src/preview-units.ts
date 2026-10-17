// Dev-only preview: builds a fixed scene and renders it with the real game Renderer,
// so we can see the HD "Songs of Conquest"-style units + team-tinted banner layer on the
// actual map. Not part of the game build; served at /preview-units.html during `vite dev`.
import { Renderer } from './rendering/Renderer';
import type { GameState, Legion, Soldier, Tile, SoldierTypeId, FactionId, FormationRow, TerrainType } from './types';

const W = 15;
const H = 6;

function tile(x: number, y: number, terrain: TerrainType = 'grass'): Tile {
  return { coord: { x, y }, terrain };
}

const map: Tile[][] = [];
for (let y = 0; y < H; y++) {
  const row: Tile[] = [];
  for (let x = 0; x < W; x++) row.push(tile(x, y));
  map.push(row);
}
// A little terrain variety so units sit on a believable board.
map[1][4].terrain = 'forest';
map[1][5].terrain = 'forest';
map[4][6].terrain = 'forest';
map[2][9].terrain = 'hills';
map[4][11].terrain = 'hills';
map[1][12].terrain = 'forest';
map[4][4].feature = 'gold_mine';

let sid = 0;
function soldier(type: SoldierTypeId, row: FormationRow, col: number, leader = false): Soldier {
  return { id: `s${sid++}`, name: '', type, hp: 100, maxHp: 100, position: { row, column: col }, isLeader: leader };
}
function legion(id: string, owner: FactionId, x: number, y: number, soldiers: Soldier[]): Legion {
  return { id, owner, soldiers, location: { x, y }, movementRemaining: owner === 'player' ? 3 : 0 };
}

const legions = new Map<string, Legion>();
// Player (blue banner)
legions.set('p_knight', legion('p_knight', 'player', 3, 3, [soldier('knight', 'front', 1, true)]));
legions.set('p_fighter', legion('p_fighter', 'player', 5, 3, [
  soldier('fighter', 'front', 0), soldier('fighter', 'front', 1), soldier('fighter', 'front', 2),
]));
// Enemy (red banner)
legions.set('e_fighter', legion('e_fighter', 'sheaim', 8, 3, [
  soldier('fighter', 'front', 0), soldier('fighter', 'front', 1),
]));
legions.set('e_knight', legion('e_knight', 'sheaim', 10, 3, [soldier('knight', 'front', 1)]));
// Classic (non-HD) unit, for before/after contrast on the same board.
legions.set('p_mage', legion('p_mage', 'player', 12, 3, [soldier('mage', 'back', 1, true)]));

const state = { mapWidth: W, mapHeight: H, map, legions, cities: new Map() } as unknown as GameState;

const canvas = document.getElementById('preview') as HTMLCanvasElement;
const renderer = new Renderer(canvas);
// Frame the row of units at a comfortable zoom.
(renderer as unknown as { uiState: { camera: { zoom: number } } }).uiState.camera.zoom = 2.4;
renderer.setCamera(1.6 * 32, 1.1 * 32);

function loop(): void {
  renderer.render(state);
  requestAnimationFrame(loop);
}
loop();
// Signal to the screenshot harness once sprites have had time to load.
setTimeout(() => document.body.setAttribute('data-ready', '1'), 1800);
