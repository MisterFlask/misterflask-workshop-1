// scene.js: a 20x14 sample map in the mood-board composition: the map as a framed night page,
// a thin top band, a minimap. Rendered with compose.js. ?scale=N sets the art scale (default 2);
// ?passes=0 leaves out the set passes (art.js PASSES: ground, stack standards, city light).
import { loadArt, TILE } from '../art.js';
import { makePlots, renderMap } from '../compose.js';

const q = new URLSearchParams(location.search);
const SCALE = Math.max(1, Math.min(4, Number(q.get('scale')) || 2));

// ---- the sample map (north at the top). One character per tile per layer.
const TERRAIN = [
  'OOOCCTTSSSSTTTGGPPPP',
  'OOCCGTTTSSTTGGGGPPPF',
  'OCCGGGTTTTGGGMMGPPFF',
  'OCGGGGGGGGGGGMGPPFBB',
  'OCCGGGGGGGGGGGPPFBBB',
  'OOCGGGGGPPGGGPPFBBUU',
  'OOCCGGGPPPPGGPPFBUUU',
  'OCOCCGGGPPPPPPPFBBUU',
  'OOOOCGGGGPPPPDDPFFBB',
  'OOOOCCGGGGPPDDDDPPFF',
  'OOOOOCCGGGGPPDDDDPPP',
  'OOOCOOCCGGGGPPPDDPPP',
  'OOCCCOOCCGGGGGPPPPPP',
  'OOOOOOOOCCGGGGGGPPPP',
];
const RELIEF = [
  '.....^^^^^^h........',
  '......^^^^h.........',
  '.......hh...........',
  '....................',
  '..........h.........',
  '...h.............h..',
  '..............h.....',
  '....................',
  '.........h......^...',
  '...............^^...',
  '....................',
  '....................',
  '....................',
  '....................',
];
const FEATURE = [
  '....................',
  '....ff......ff......',
  '...fff....ffff......',
  '..fff....ffaaf......',
  '...ff....faaaf..x...',
  '....f.....ff.....x..',
  '..................xx',
  '.....ff.............',
  '.....fff.........j..',
  '....................',
  '........ff..........',
  '..........ff........',
  '....................',
  '....................',
];
const OWNER = [
  '....................',
  '....................',
  '................bbbb',
  '..aaaa.........bbbbb',
  '..aaaaaa......bbbbbb',
  '..aaaaaaa.....bbbbbb',
  '...aaaaaa......bbbbb',
  '....aaaa........bbbb',
  '.....aaa............',
  '.....ccccc..........',
  '.....ccccc..........',
  '......cccc..........',
  '.......ccc..........',
  '....................',
];
// 2 visible, 1 fogged, 0 unrevealed
const VIS = [
  '00011111111111111100',
  '01111111111111111110',
  '11112222222222222211',
  '11122222222222222221',
  '11222222222222222221',
  '11222222222222222221',
  '11222222222222222221',
  '11222222222222222211',
  '11122222222222222111',
  '11122222222222221111',
  '11112222222222211100',
  '11111222222221111000',
  '01111112222111110000',
  '00011111111111000000',
];
const IMPROVE = {
  '6,4': 'IMPROVEMENT_FARM', '5,5': 'IMPROVEMENT_FARM', '7,5': 'IMPROVEMENT_COTTAGE', '6,6': 'IMPROVEMENT_HAMLET',
  '3,5': 'IMPROVEMENT_MINE', '7,10': 'IMPROVEMENT_FARM', '8,9': 'IMPROVEMENT_COTTAGE', '12,7': 'IMPROVEMENT_STANDING_STONES',
  '17,7': 'IMPROVEMENT_MANA_CHAOS', '11,2': 'IMPROVEMENT_GOODY_HUT', '13,9': 'IMPROVEMENT_BEAR_DEN', '19,4': 'IMPROVEMENT_HELLFIRE',
};
const BONUS = { '4,6': 'BONUS_IRON', '3,9': 'BONUS_FISH', '9,6': 'BONUS_WHEAT', '14,10': 'BONUS_GOLD', '17,5': 'BONUS_MANA_CHAOS', '10,3': 'BONUS_DEER' };
const ROADS = ['5,4', '6,4', '7,4', '8,5', '9,6', '10,6', '11,6', '12,6', '13,6', '14,6', '15,5', '16,4', '6,5', '6,6', '6,7', '6,8', '7,9', '7,10', '4,4'];
// river: edges as [x, y, 's'|'e'] (south or east edge of the tile)
const RIVER = [[8, 1, 's'], [8, 2, 'e'], [9, 3, 's'], [9, 3, 'e'], [9, 4, 'e'], [9, 5, 'e'], [9, 6, 's'], [8, 6, 's'], [7, 7, 'e'], [7, 7, 's'], [6, 7, 's'], [5, 7, 's'], [4, 7, 's']];
const CITIES = [
  { x: 6, y: 5, owner: 'a', size: 9, name: 'Aldmere', capital: true, own: true, turns: 3 },
  { x: 16, y: 4, owner: 'b', size: 6, name: 'Gloam', capital: true },
  { x: 7, y: 10, owner: 'c', size: 3, name: 'Kerrow' },
];
const STACKS = [
  { x: 8, y: 4, unit: 'UNIT_SWORDSMAN', owner: 'a', count: 3, state: { selected: true, hp: 100, moves: true } },
  { x: 10, y: 7, unit: 'UNIT_HORSEMAN', owner: 'a', count: 1, state: { hp: 60, moves: true } },
  { x: 4, y: 5, unit: 'UNIT_ARCHER', owner: 'a', count: 1, state: { hp: 100, moves: false, mark: 'fortify' } },
  { x: 6, y: 5, unit: 'UNIT_LONGBOWMAN', owner: 'a', count: 2, state: { hp: 100, moves: false, mark: 'fortify' } },
  { x: 11, y: 4, unit: 'UNIT_ADEPT', owner: 'a', count: 1, state: { hp: 100, moves: true } },
  { x: 2, y: 8, unit: 'UNIT_TRIREME', owner: 'a', count: 2, state: { hp: 100, moves: true } },
  { x: 14, y: 5, unit: 'UNIT_IMP', owner: 'b', count: 2, state: { hp: 80, facing: 'left' } },
  { x: 15, y: 7, unit: 'UNIT_DISCIPLE_THE_ASHEN_VEIL', owner: 'b', count: 1, state: { hp: 100, facing: 'left', accent: 'C' } },
  { x: 12, y: 3, unit: 'UNIT_SKELETON', owner: 'barb', count: 1, state: { hp: 30, facing: 'left' } },
  { x: 13, y: 8, unit: 'UNIT_WOLF', owner: 'animal', count: 1, state: { hp: 100, facing: 'left' } },
  { x: 8, y: 11, unit: 'UNIT_WORKER', owner: 'c', count: 1, state: { hp: 100, moves: true } },
  { x: 9, y: 9, unit: 'UNIT_SCOUT', owner: 'c', count: 1, state: { hp: 100, moves: true, facing: 'left' } },
];

function buildMap() {
  const T = { G: 'TERRAIN_GRASS', P: 'TERRAIN_PLAINS', D: 'TERRAIN_DESERT', T: 'TERRAIN_TUNDRA', S: 'TERRAIN_SNOW', M: 'TERRAIN_MARSH',
    C: 'TERRAIN_COAST', O: 'TERRAIN_OCEAN', B: 'TERRAIN_BROKEN_LANDS', F: 'TERRAIN_FIELDS_OF_PERDITION', U: 'TERRAIN_BURNING_SANDS', H: 'TERRAIN_SHALLOWS' };
  const FEAT = { f: 'FEATURE_FOREST', a: 'FEATURE_FOREST_ANCIENT', j: 'FEATURE_JUNGLE', x: 'FEATURE_FLAMES' };
  const W = 20, H = 14;
  const tiles = [];
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const t = TERRAIN[y][x];
    const water = t === 'O' || t === 'C';
    const r = RELIEF[y][x];
    const o = OWNER[y][x];
    tiles.push({
      terrain: T[t], plotType: water ? 'OCEAN' : r === '^' ? 'PEAK' : r === 'h' ? 'HILLS' : 'LAND',
      feature: FEAT[FEATURE[y][x]] || null, owner: o === '.' ? null : o, vis: +VIS[y][x], river: 0,
      improvement: IMPROVE[`${x},${y}`] || null, bonus: BONUS[`${x},${y}`] || null,
      route: ROADS.includes(`${x},${y}`) ? 'ROUTE_ROAD' : null, city: null,
    });
  }
  for (const [x, y, e] of RIVER) tiles[y * W + x].river |= e === 's' ? 1 : 2;
  for (const c of CITIES) tiles[c.y * W + c.x].city = c.name;
  return { width: W, height: H, wrapX: false, tiles };
}

function topBand(art, ctx, x, y, w) {
  const s = art.scale;
  const h = 14;
  ctx.fillStyle = art.css('1');
  ctx.fillRect(x, y, w, h * s);
  ctx.fillStyle = art.css('3');
  ctx.fillRect(x, y + (h - 1) * s, w, s);
  let cx = x + 4 * s;
  const item = (icon, text) => {
    const ic = art.pick(icon);
    if (ic) art.drawSprite(ctx, cx, y - 1 * s, ic.name, {});
    cx += 17 * s;
    cx += (art.drawText(ctx, cx, y + 10 * s, text, { font: 'vigil', color: '7' }) + 9) * s;
  };
  item('icon16/food', '+6');
  item('icon16/production', '14');
  item('icon16/commerce', '76');
  // turn and armageddon at the right
  const label = 'TURN 112';
  const lw = art.measureText(label, 'vigil');
  art.drawText(ctx, x + w - (lw + 6) * s, y + 10 * s, label, { font: 'vigil', color: '8' });
}

function minimap(art, ctx, x, y, map, owners) {
  const s = art.scale;
  const k = 2; // art px per tile
  const w = map.width * k, h = map.height * k;
  ctx.fillStyle = art.css('5');
  ctx.fillRect(x - 2 * s, y - 2 * s, (w + 4) * s, (h + 4) * s);
  ctx.fillStyle = art.css('1');
  ctx.fillRect(x - 1 * s, y - 1 * s, (w + 2) * s, (h + 2) * s);
  const base = { TERRAIN_OCEAN: 'd', TERRAIN_COAST: 'n', TERRAIN_GRASS: 'm', TERRAIN_PLAINS: 'Q', TERRAIN_DESERT: 'Y', TERRAIN_TUNDRA: '4',
    TERRAIN_SNOW: 'f', TERRAIN_MARSH: 'q', TERRAIN_BROKEN_LANDS: 'u', TERRAIN_FIELDS_OF_PERDITION: 'p', TERRAIN_BURNING_SANDS: 'x', TERRAIN_SHALLOWS: 'v' };
  map.tiles.forEach((t, i) => {
    const tx = i % map.width, ty = Math.floor(i / map.width);
    let key = t.vis === 0 ? '0' : t.owner && owners.get(t.owner) ? art.keys[owners.get(t.owner).tincture.field] : base[t.terrain] || '4';
    if (t.vis === 1) key = art.keys[art.tables.FOG[art.K(key)]];
    if (t.city) key = '8';
    ctx.fillStyle = art.css(key);
    ctx.fillRect(x + tx * k * s, y + ty * k * s, k * s, k * s);
  });
  ctx.strokeStyle = art.css('8');
  ctx.lineWidth = s;
  ctx.strokeRect(x + 3 * k * s + s / 2, y + 2 * k * s + s / 2, 12 * k * s - s, 9 * k * s - s);
}

async function main() {
  const art = await loadArt({ base: new URL('../', import.meta.url), lenient: true, passes: q.get('passes') !== '0' });
  if (art.lib.errors.length) throw new Error(art.lib.errors.join('\n'));
  art.scale = SCALE;
  const s = SCALE;
  const map = buildMap();
  const owners = new Map([
    ['a', art.ownerStyle('CIVILIZATION_BANNOR', { id: 'a' })],
    ['b', art.ownerStyle('CIVILIZATION_SHEAIM', { id: 'b' })],
    ['c', art.ownerStyle('CIVILIZATION_DOVIELLO', { id: 'c' })],
    ['barb', art.ownerStyle('CIVILIZATION_BARBARIAN', { id: 'barb' })],
    ['animal', { ...art.ownerStyle('CIVILIZATION_BARBARIAN', { id: 'animal' }), animal: true }],
  ]);
  const M = 6; // page margin in art px
  const band = 14;
  const mapW = map.width * TILE, mapH = map.height * TILE;
  const W = mapW + 2 * M, H = mapH + band + 3 * M;
  const c = document.createElement('canvas');
  c.width = W * s; c.height = H * s;
  c.className = 'pix';
  document.getElementById('page').append(c);
  const ctx = c.getContext('2d', { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  ctx.fillStyle = art.css('1');
  ctx.fillRect(0, 0, c.width, c.height);
  topBand(art, ctx, M * s, M * s, mapW * s);
  const mx = M * s, my = (M * 2 + band) * s;
  const t0 = performance.now();
  const timings = {};
  const view = { owners, timings, cities: CITIES, stacks: STACKS, cursor: { x: 8, y: 4, selected: true } };
  if (q.get('cache')) {
    // the client's way: terrain into an offscreen cache, then blit, vignette, units on top
    const cache = document.createElement('canvas');
    cache.width = mapW * s; cache.height = mapH * s;
    renderMap(art, cache.getContext('2d', { willReadFrequently: true }), map, { ...view, x0: 0, y0: 0, only: 'terrain' });
    ctx.drawImage(cache, mx, my);
    art.drawVignette(ctx, mx, my, mapW * s, mapH * s);
    renderMap(art, ctx, map, { ...view, x0: mx, y0: my, only: 'units' });
  } else {
    renderMap(art, ctx, map, { ...view, x0: mx, y0: my });
  }
  const ms = Math.round(performance.now() - t0);
  // the page frame: a double rule, ash outside and iron inside, with gilt nails at the corners
  const frame = (x, y, w, h) => {
    ctx.fillStyle = art.css('5');
    ctx.fillRect(x - 3 * s, y - 3 * s, w + 6 * s, s); ctx.fillRect(x - 3 * s, y + h + 2 * s, w + 6 * s, s);
    ctx.fillRect(x - 3 * s, y - 3 * s, s, h + 6 * s); ctx.fillRect(x + w + 2 * s, y - 3 * s, s, h + 6 * s);
    ctx.fillStyle = art.css('3');
    ctx.fillRect(x - 1 * s, y - 1 * s, w + 2 * s, s); ctx.fillRect(x - 1 * s, y + h, w + 2 * s, s);
    ctx.fillRect(x - 1 * s, y - 1 * s, s, h + 2 * s); ctx.fillRect(x + w, y - 1 * s, s, h + 2 * s);
    for (const [px, py] of [[x - 3 * s, y - 3 * s], [x + w + 2 * s, y - 3 * s], [x - 3 * s, y + h + 2 * s], [x + w + 2 * s, y + h + 2 * s]]) {
      ctx.fillStyle = art.css('7');
      ctx.fillRect(px - s, py, 3 * s, s); ctx.fillRect(px, py - s, s, 3 * s);
      ctx.fillStyle = art.css('y');
      ctx.fillRect(px, py, s, s);
    }
  };
  frame(mx, my, mapW * s, mapH * s);
  minimap(art, ctx, mx + (mapW - map.width * 2 - 6) * s, my + (mapH - map.height * 2 - 6) * s, map, owners);
  const info = document.createElement('div');
  info.className = 'meta';
  info.textContent = `20x14 tiles at ${s}x, rendered in ${ms} ms (${Object.entries(timings).map(([k, v]) => `${k} ${v}`).join(', ')})`;
  document.getElementById('page').append(info);
}

try {
  await main();
  window.__artReady = true;
} catch (e) {
  console.error(e);
  document.getElementById('err').textContent = String(e.stack || e);
  window.__artReady = String(e);
}
