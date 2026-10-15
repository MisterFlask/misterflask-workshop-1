// cities/preview.js: the cities set at night. Parts (?part=styles,towns,ref; default styles,towns):
//   styles  every city style at tiers I-V on its home ground, with pool, banner and plaque (?scale=N,
//           default 2; ?styles=cloister,infernal for a subset);
//   towns   a 16x11 map with cities of several styles and sizes among forests, hills, a river, roads,
//           coast and borders, at 1x and 2x;
//   ref     the contact sheet kept as cities/ref-styles.png (every style and tier at 1x).
// ?light=0 shows the toolkit's own pool (art.drawLight twice FIRE) for comparison; ?ground=0 the seed
// ground instead of the terrain set's ground pass (terrain/ground.js). The city banner is
// drawn as the pole-less gonfalon study below unless ?banner=ui (the ui set's ui/banner as loaded).
// Sets window.__artReady = true when done (for screenshots).
import { loadArt, TILE } from '../art.js';
import { renderMap } from '../compose.js';
import { parseSprites } from '../spritefmt.js';
import { installCityLight } from './citylight.js';
import { installTerrain } from '../terrain/ground.js';

const q = new URLSearchParams(location.search);
const parts = (q.get('part') || 'styles,towns').split(',');
const out = document.getElementById('out');

function el(tag, attrs = {}, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  for (const k of kids) e.append(k);
  return e;
}

function canvas(art, w, h, bg = '1') {
  const c = el('canvas', { width: w, height: h });
  const ctx = c.getContext('2d', { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  ctx.fillStyle = art.css(bg);
  ctx.fillRect(0, 0, w, h);
  return [c, ctx];
}

// The city banner as the gonfalon without its pole (a study for the ui set: the same 5x8 field,
// swallow tails and rod as units/stack.js's 7x12 standard). Anchor 0,1: drawCity puts it at tile
// (1, 1), so the field covers tile columns 2-6 and rows 2-9 and the 5x5 sigil (tile 2,3) sits as it
// does on the standard.
const BANNER_STUDY = `
= ui/banner 7x12 0,1
: flag 1,2,5,8
: note study: the city banner as the stack standard without its pole
5.....5
4444444
0#####0
0#####0
0#####0
0#####0
0#####0
0#####0
0#####0
0%#0#%0
0%0.0%0
.0...0.
`;

function useBannerStudy(art) {
  const r = parseSprites(BANNER_STUDY, 'preview', art.palette.keyIndex);
  if (r.errors.length) throw new Error(r.errors.join('\n'));
  const s = r.sprites[0];
  const old = art.lib.sprites.get('ui/banner');
  if (old) Object.assign(old, { w: s.w, h: s.h, ax: s.ax, ay: s.ay, data: s.data, meta: s.meta });
}

// ---------------------------------------------------------------- styles
// style -> [civilization, home terrain, plot type, the row below (coast for the harbour)]
const STYLES = [
  ['cloister', 'CIVILIZATION_BANNOR', 'G'],
  ['amurite', 'CIVILIZATION_AMURITES', 'G'],
  ['calabim', 'CIVILIZATION_CALABIM', 'P'],
  ['sidar', 'CIVILIZATION_SIDAR', 'T'],
  ['sundomes', 'CIVILIZATION_MALAKIM', 'D'],
  ['elven', 'CIVILIZATION_LJOSALFAR', 'G', 'forest'],
  ['svartalfar', 'CIVILIZATION_SVARTALFAR', 'G', 'forest'],
  ['dwarven', 'CIVILIZATION_KHAZAD', 'T', 'hills'],
  ['luchuirp', 'CIVILIZATION_LUCHUIRP', 'P', 'hills'],
  ['tribal', 'CIVILIZATION_DOVIELLO', 'P'],
  ['barbarian', 'CIVILIZATION_BARBARIAN', 'P'],
  ['harbour', 'CIVILIZATION_LANUN', 'G', 'coast'],
  ['carnival', 'CIVILIZATION_BALSERAPHS', 'P'],
  ['infernal', 'CIVILIZATION_INFERNAL', 'U'],
  ['celestial', 'CIVILIZATION_MERCURIANS', 'G'],
  ['frost', 'CIVILIZATION_ILLIANS', 'S'],
];
const TERRAIN = { G: 'TERRAIN_GRASS', P: 'TERRAIN_PLAINS', D: 'TERRAIN_DESERT', T: 'TERRAIN_TUNDRA', S: 'TERRAIN_SNOW', M: 'TERRAIN_MARSH',
  C: 'TERRAIN_COAST', O: 'TERRAIN_OCEAN', B: 'TERRAIN_BROKEN_LANDS', F: 'TERRAIN_FIELDS_OF_PERDITION', U: 'TERRAIN_BURNING_SANDS' };
const SIZES = [2, 5, 9, 15, 22];
const NAMES = ['Ash', 'Vigil', 'Aldmere', 'Thornwall', 'Hallowmere'];

function styleMap(home, extra) {
  const W = 11, H = 3;
  const tiles = [];
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const city = y === 1 && x % 2 === 1;
    let t = home, plotType = 'LAND', feature = null;
    if (extra === 'coast' && y === 2) { t = x % 2 ? 'C' : 'O'; plotType = 'OCEAN'; }
    if (extra === 'forest' && !city && (x + y) % 3 === 0) feature = 'FEATURE_FOREST';
    if (extra === 'hills' && city) plotType = 'HILLS';
    tiles.push({ terrain: TERRAIN[t], plotType, feature, vis: 2, river: 0, owner: city || (y === 1) ? 'o' : null });
  }
  return { width: W, height: H, wrapX: false, tiles };
}

function styles(art) {
  const sec = el('section');
  const scale = Number(q.get('scale')) || 2;
  const only = q.get('styles') ? q.get('styles').split(',') : null;
  art.scale = scale;
  sec.append(el('h2', {}, `styles at ${scale}x (tiers I-V: size ${SIZES.join(', ')})`));
  for (const [style, civ, home, extra] of STYLES) {
    if (only && !only.includes(style)) continue;
    const map = styleMap(home, extra);
    const owners = new Map([['o', art.ownerStyle(civ, { id: 'o' })]]);
    const cities = SIZES.map((size, t) => ({ x: 1 + 2 * t, y: 1, owner: 'o', size, name: NAMES[t], capital: t === 4 }));
    const [c, ctx] = canvas(art, map.width * TILE * scale, map.height * TILE * scale, '0');
    renderMap(art, ctx, map, { owners, cities, stacks: [], vignette: false, skip: new Set(['borders']) });
    sec.append(el('h3', {}, `${style} (${civ.replace('CIVILIZATION_', '').toLowerCase()})`), c);
  }
  return sec;
}

// ---------------------------------------------------------------- reference sheet
// Every style at tiers I-V at 1x on its home ground, in two columns of eight: the contact sheet kept
// as cities/ref-styles.png.
function reference(art) {
  const sec = el('section');
  art.scale = 1;
  const W = 22, H = 24;
  const tiles = new Array(W * H);
  const owners = new Map();
  const cities = [];
  STYLES.forEach(([style, civ, home, extra], k) => {
    const c = k >> 3, r = k & 7;
    const block = styleMap(home, extra);
    owners.set(style, art.ownerStyle(civ, { id: style }));
    for (let y = 0; y < 3; y++) for (let x = 0; x < 11; x++) tiles[(3 * r + y) * W + 11 * c + x] = { ...block.tiles[y * 11 + x], owner: null };
    SIZES.forEach((size, t) => cities.push({ x: 11 * c + 1 + 2 * t, y: 3 * r + 1, owner: style, size, name: '' }));
  });
  const map = { width: W, height: H, wrapX: false, tiles };
  const [cv, ctx] = canvas(art, W * TILE, H * TILE, '0');
  renderMap(art, ctx, map, { owners, cities, stacks: [], vignette: false });
  STYLES.forEach(([style], k) => {
    const x = (k >> 3) * 11 * TILE + 3, y = (k & 7) * 3 * TILE + 9;
    art.drawText(ctx, x, y, style, { font: 'vigil', color: '7', smallCaps: true, shadow: true });
  });
  sec.append(el('h2', {}, 'reference: styles at 1x'), cv);
  return sec;
}

// ---------------------------------------------------------------- towns
const T_TERRAIN = [
  'OOCGGGGGPPPPTTTT',
  'OCGGGGGGPPPPTTTS',
  'OCGGGGGPPPPPTTTS',
  'OCCGGGGPPPDDPTTT',
  'OOCGGGGPPDDDPPTT',
  'OOCCGGGPPDDUUPPT',
  'OOOCGGGGPPUUUFPP',
  'OOOCCGGGPPPUFFFP',
  'OOOOCGGGGPPPFFPP',
  'OOOOCCGGGGPPPPPP',
  'OOOOOCCCGGGGPPPP',
];
const T_RELIEF = [
  '..........hh^^..',
  '.........h^^^^..',
  '..........h^h...',
  '................',
  '....h...........',
  '...hh...........',
  '................',
  '.........h......',
  '................',
  '................',
  '................',
];
const T_FEATURE = [
  '...ff.ff........',
  '...f.ffff.......',
  '..ff.....f......',
  '......f.........',
  '.....f..........',
  '..........a.....',
  '.....ff.........',
  '.....fff........',
  '......f....aa...',
  '.......ff.......',
  '................',
];
const T_OWNER = [
  '..aaaa..........',
  '..aaaa......dddd',
  '..aaaaa.....dddd',
  '..aaaaa.....dddd',
  '..aaaaa.........',
  '...aaa.......bbb',
  '....ccc.....bbbb',
  '....cccc....bbbb',
  '....cccc....bbbb',
  '.....ccc........',
  '................',
];
const T_CITIES = [
  { x: 4, y: 2, owner: 'a', size: 14, name: 'Aldmere', capital: true, own: true, turns: 4 },
  { x: 3, y: 5, owner: 'a', size: 4, name: 'Kell' },
  { x: 6, y: 8, owner: 'c', size: 8, name: 'Saltreach' },
  { x: 14, y: 2, owner: 'd', size: 6, name: 'Rimehold' },
  { x: 13, y: 7, owner: 'b', size: 11, name: 'Gloam', capital: true },
  { x: 9, y: 4, owner: 'e', size: 3, name: 'Ember' },
  { x: 10, y: 8, owner: 'f', size: 2, name: 'Raider' },
];
const T_ROADS = ['4,2', '4,3', '4,4', '3,5', '5,4', '6,5', '7,5', '8,5', '9,4', '6,6', '6,7', '6,8', '10,5', '11,6', '12,7', '13,7'];
const T_RIVER = [[7, 0, 'e'], [7, 1, 'e'], [7, 2, 's'], [6, 3, 'e'], [6, 3, 's'], [5, 4, 's'], [4, 4, 's']];

function townMap() {
  const W = 16, H = 11;
  const tiles = [];
  const F = { f: 'FEATURE_FOREST', a: 'FEATURE_FOREST_ANCIENT' };
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const t = T_TERRAIN[y][x];
    const water = t === 'O' || t === 'C';
    const r = T_RELIEF[y][x];
    const o = T_OWNER[y][x];
    tiles.push({
      terrain: TERRAIN[t], plotType: water ? 'OCEAN' : r === '^' ? 'PEAK' : r === 'h' ? 'HILLS' : 'LAND',
      feature: F[T_FEATURE[y][x]] || null, owner: o === '.' ? null : o, vis: 2, river: 0,
      route: T_ROADS.includes(`${x},${y}`) ? 'ROUTE_ROAD' : null,
    });
  }
  for (const [x, y, e] of T_RIVER) tiles[y * W + x].river |= e === 's' ? 1 : 2;
  for (const c of T_CITIES) { const t = tiles[c.y * W + c.x]; t.feature = null; t.city = c.name; }
  return { width: W, height: H, wrapX: false, tiles };
}

function towns(art) {
  const sec = el('section');
  sec.append(el('h2', {}, 'towns at 1x and 2x'));
  const owners = new Map([
    ['a', art.ownerStyle('CIVILIZATION_BANNOR', { id: 'a' })],
    ['b', art.ownerStyle('CIVILIZATION_SHEAIM', { id: 'b' })],
    ['c', art.ownerStyle('CIVILIZATION_LANUN', { id: 'c' })],
    ['d', art.ownerStyle('CIVILIZATION_ILLIANS', { id: 'd' })],
    ['e', art.ownerStyle('CIVILIZATION_CLAN_OF_EMBERS', { id: 'e' })],
    ['f', art.ownerStyle('CIVILIZATION_BARBARIAN', { id: 'f' })],
  ]);
  const map = townMap();
  for (const scale of [1, 2]) {
    art.scale = scale;
    const [c, ctx] = canvas(art, map.width * TILE * scale, map.height * TILE * scale, '0');
    renderMap(art, ctx, map, { owners, cities: T_CITIES, stacks: [] });
    sec.append(el('h3', {}, `${scale}x`), c);
  }
  return sec;
}

async function main() {
  const art = await loadArt({ passes: false });
  if (q.get('ground') !== '0') installTerrain(art);
  if (q.get('light') !== '0') installCityLight(art);
  if (q.get('banner') !== 'ui') useBannerStudy(art);
  if (parts.includes('styles')) out.append(styles(art));
  if (parts.includes('towns')) out.append(towns(art));
  if (parts.includes('ref')) out.append(reference(art));
}

try {
  await main();
  window.__artReady = true;
} catch (e) {
  console.error(e);
  document.getElementById('err').textContent = String(e.stack || e);
  window.__artReady = String(e);
}
