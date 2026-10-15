// units/preview.js: the units set at work. Parts (?part=a,b,... from the list below, default all):
//   stacks     the stack presentation (units/stack.js) case by case at 1x, 2x and 3x;
//   standards  every tincture (rows) in every standard state (columns): movable, selected, spent,
//              foreign, foreign with a hero, hurt, in a city; at 1x and 2x;
//   lights     the naval line on coast and ocean (lanterns, reflections), the settler, the disciple,
//              and the archer and melee lines side by side (weapon silhouettes); 1x to 3x;
//   muster     a 16x10 map with armies of four civilizations, barbarians and animals (?scale=N;
//              ?nostd=1 leaves the standards out, to measure what they add);
//   roster     every bespoke unit sprite through drawUnit at 2x, grouped by combat class.
// ?scales=1,2 limits the scales of stacks, standards and lights.
// Sets window.__artReady = true when done (for screenshots).
import { loadArt, TILE } from '../art.js';
import { renderMap } from '../compose.js';
import { installStackOverlay } from './stack.js';

const q = new URLSearchParams(location.search);
const parts = new Set((q.get('part') || 'all').split(','));
const want = (p) => parts.has('all') || parts.has(p);
const only = q.get('scales') ? q.get('scales').split(',').map(Number) : null;
const scales = (list) => (only ? list.filter((s) => only.includes(s)) : list);
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

function tile(terrain, extra = {}) {
  const p = { x: extra.mx || 0, y: extra.my || 0, terrain, plotType: 'LAND', vis: 2, river: 0, ...extra };
  p.at = () => null;
  return p;
}

// ---------------------------------------------------------------- stacks
const CASES = [
  ['own, moves left', 'UNIT_SWORDSMAN', 'a', { hp: 100, moves: true }],
  ['own, moves spent', 'UNIT_SWORDSMAN', 'a', { hp: 100, moves: false }],
  ['stack of 3 with a hero', 'UNIT_DONAL', 'a', { hp: 100, moves: true, count: 3, hero: true }],
  ['selected, hurt (60)', 'UNIT_KNIGHT', 'a', { hp: 60, moves: true, selected: true }],
  ['hurt (30), spent', 'UNIT_ARCHER', 'a', { hp: 30, moves: false }],
  ['dying (10), fortified', 'UNIT_PHALANX', 'a', { hp: 10, moves: true, mark: 'fortify' }],
  ['foreign stack of 2', 'UNIT_IMP', 'b', { hp: 100, count: 2, facing: 'left' }],
  ['foreign, per pale', 'UNIT_ARCHER', 'c2', { hp: 100 }],
  ['foreign stack of 12', 'UNIT_AXEMAN', 'd', { hp: 100, count: 12 }],
  ['hidden nationality (own)', 'UNIT_PRIVATEER', 'h', { hp: 100, moves: true }, 'TERRAIN_COAST'],
  ['barbarian', 'UNIT_SKELETON', 'barb', { hp: 70, facing: 'left' }],
  ['animal', 'UNIT_WOLF', 'animal', { hp: 100, facing: 'left' }],
  ['in a city, stack of 4', 'UNIT_LONGBOWMAN', 'a', { hp: 100, moves: true, count: 4, inCity: true, mark: 'fortify' }, 'TERRAIN_PLAINS', 5],
];

function stacks(art, owners) {
  const sec = el('section');
  sec.append(el('h2', {}, 'stack presentation'));
  for (const scale of scales([1, 2, 3])) {
    art.scale = scale;
    const s = scale;
    const gap = 6;
    const W = CASES.length * (TILE + gap) + gap;
    const [c, ctx] = canvas(art, W * s, (TILE + 2 * gap) * s, '0');
    CASES.forEach(([, unit, owner, state, terrain, city], i) => {
      const x = (gap + i * (TILE + gap)) * s, y = gap * s;
      const t = terrain || 'TERRAIN_GRASS';
      art.drawTerrain(ctx, x, y, tile(t, { mx: i, plotType: t === 'TERRAIN_COAST' ? 'OCEAN' : 'LAND' }));
      if (city) art.drawCity(ctx, x, y, { size: city, name: '', plaque: false }, owners.get(owner));
      art.drawUnit(ctx, x, y, unit, owners.get(owner), state);
    });
    sec.append(el('h3', {}, `${scale}x`), c);
  }
  const legend = el('div', { class: 'meta' });
  legend.textContent = CASES.map(([l], i) => `${i + 1}. ${l}`).join('   ');
  sec.append(legend);
  return sec;
}

// ---------------------------------------------------------------- standards: every tincture in every state
const STANDARD_OWNERS = [
  ['Argent (Bannor)', 'CIVILIZATION_BANNOR'], ['Argent per pale', 'CIVILIZATION_BANNOR', 'pale'], ['Or (Khazad)', 'CIVILIZATION_KHAZAD'],
  ['Gules (Sheaim)', 'CIVILIZATION_SHEAIM'], ['Azure (Lanun)', 'CIVILIZATION_LANUN'], ['Vert (Ljosalfar)', 'CIVILIZATION_LJOSALFAR'],
  ['Purpure (Amurites)', 'CIVILIZATION_AMURITES'], ['Tenne (Hippus)', 'CIVILIZATION_HIPPUS'], ['Celeste (Illians)', 'CIVILIZATION_ILLIANS'],
  ['Sable (Doviello)', 'CIVILIZATION_DOVIELLO'], ['Cendree (Luchuirp)', 'CIVILIZATION_LUCHUIRP'], ['barbarian', 'CIVILIZATION_BARBARIAN'],
  ['Vert per pale', 'CIVILIZATION_LJOSALFAR', 'pale'], ['Or per bend', 'CIVILIZATION_KHAZAD', 'bend'], ['Sable quarterly', 'CIVILIZATION_DOVIELLO', 'quarterly'],
];
const STANDARD_STATES = [
  ['movable', 'UNIT_SWORDSMAN', { hp: 100, moves: true }],
  ['selected, 3', 'UNIT_SWORDSMAN', { hp: 100, moves: true, selected: true, count: 3 }],
  ['spent', 'UNIT_SWORDSMAN', { hp: 100, moves: false }],
  ['foreign', 'UNIT_AXEMAN', { hp: 100, facing: 'left' }],
  ['foreign, 4, hero', 'UNIT_KNIGHT', { hp: 100, facing: 'left', count: 4, hero: true }],
  ['foreign, hurt', 'UNIT_ARCHER', { hp: 40, facing: 'left' }],
  ['in a city', 'UNIT_LONGBOWMAN', { hp: 100, count: 2, inCity: true, mark: 'fortify' }],
];

function standards(art) {
  const sec = el('section');
  sec.append(el('h2', {}, 'standards: tinctures (rows) by state (columns)'));
  for (const scale of scales([1, 2])) {
    art.scale = scale;
    const s = scale;
    const gap = 2;
    const W = STANDARD_STATES.length * (TILE + gap) + gap, H = STANDARD_OWNERS.length * (TILE + gap) + gap;
    const [c, ctx] = canvas(art, W * s, H * s, '0');
    STANDARD_OWNERS.forEach(([, civ, division], row) => {
      const owner = art.ownerStyle(civ, { id: `${civ}/${division || ''}`, division });
      STANDARD_STATES.forEach(([, unit, state], col) => {
        const x = (gap + col * (TILE + gap)) * s, y = (gap + row * (TILE + gap)) * s;
        const t = (row + col) % 3 === 0 ? 'TERRAIN_PLAINS' : (row + col) % 3 === 1 ? 'TERRAIN_GRASS' : 'TERRAIN_TUNDRA';
        art.drawTerrain(ctx, x, y, tile(t, { mx: col, my: row }));
        if (state.inCity) art.drawCity(ctx, x, y, { size: 5, name: '', plaque: false }, owner);
        art.drawUnit(ctx, x, y, unit, owner, state);
      });
    });
    sec.append(el('h3', {}, `${scale}x`), c);
  }
  const legend = el('div', { class: 'meta' });
  legend.textContent = `columns: ${STANDARD_STATES.map(([l]) => l).join(' | ')}. rows: ${STANDARD_OWNERS.map(([l]) => l).join(' | ')}`;
  sec.append(legend);
  return sec;
}

// ---------------------------------------------------------------- lights and lines: ships and lanterns at 1x-3x
const NAVAL = ['UNIT_WORKBOAT', 'UNIT_TRIREME', 'UNIT_GALLEY', 'UNIT_CARAVEL', 'UNIT_GALLEON', 'UNIT_FRIGATE', 'UNIT_MAN_O_WAR',
  'UNIT_QUEEN_OF_THE_LINE', 'UNIT_PRIVATEER', 'UNIT_BLACK_WIND', 'UNIT_SAILORS_DIRGE', 'UNIT_ARCANE_BARGE', 'UNIT_SEA_SERPENT'];
const LAND_LIGHTS = ['UNIT_SETTLER', 'UNIT_DISCIPLE_THE_ORDER', 'UNIT_WORKER'];
const LINES = [
  ['archers', ['UNIT_ARCHER', 'UNIT_LONGBOWMAN', 'UNIT_CROSSBOWMAN', 'UNIT_MARKSMAN']],
  ['melee', ['UNIT_WARRIOR', 'UNIT_AXEMAN', 'UNIT_SWORDSMAN', 'UNIT_CHAMPION', 'UNIT_IMMORTAL']],
];

function lights(art, owners) {
  const sec = el('section');
  sec.append(el('h2', {}, 'naval line and lanterns; weapon silhouettes within a class'));
  const own = owners.get('a');
  for (const scale of scales([1, 2, 3])) {
    art.scale = scale;
    const s = scale;
    const n = NAVAL.length + LAND_LIGHTS.length;
    const [c, ctx] = canvas(art, n * TILE * s, TILE * 2 * s, '0');
    NAVAL.forEach((u, i) => {
      for (const [row, t] of [[0, 'TERRAIN_COAST'], [1, 'TERRAIN_OCEAN']]) {
        const x = i * TILE * s, y = row * TILE * s;
        art.drawTerrain(ctx, x, y, tile(t, { mx: i, my: row, plotType: 'OCEAN' }));
        art.drawUnit(ctx, x, y, u, row ? owners.get('b') : own, row ? { hp: 100, facing: 'left' } : { hp: 100, moves: true });
      }
    });
    LAND_LIGHTS.forEach((u, k) => {
      for (const [row, t] of [[0, 'TERRAIN_GRASS'], [1, 'TERRAIN_PLAINS']]) {
        const i = NAVAL.length + k;
        const x = i * TILE * s, y = row * TILE * s;
        art.drawTerrain(ctx, x, y, tile(t, { mx: i, my: row }));
        art.drawUnit(ctx, x, y, u, own, row ? { hp: 100, moves: false, facing: 'left' } : { hp: 100, moves: true });
      }
    });
    sec.append(el('h3', {}, `${scale}x`), c);
  }
  for (const scale of scales([1, 2])) {
    art.scale = scale;
    const s = scale;
    const w = Math.max(...LINES.map(([, l]) => l.length));
    const [c, ctx] = canvas(art, (w * TILE + 8) * 2 * s, LINES.length * TILE * s, '0');
    LINES.forEach(([, list], row) => list.forEach((u, i) => {
      for (const [half, t] of [[0, 'TERRAIN_GRASS'], [1, 'TERRAIN_PLAINS']]) {
        const x = (half * (w * TILE + 8) + i * TILE) * s, y = row * TILE * s;
        art.drawTerrain(ctx, x, y, tile(t, { mx: i + half * 9, my: row }));
        art.drawUnit(ctx, x, y, u, half ? owners.get('d') : own, half ? { hp: 100, facing: 'left' } : { hp: 100 });
      }
    }));
    sec.append(el('h3', {}, `lines at ${scale}x: ${LINES.map(([k, l]) => `${k}: ${l.map((u) => u.slice(5).toLowerCase()).join(', ')}`).join('; ')}`), c);
  }
  return sec;
}

// ---------------------------------------------------------------- muster
const TERRAIN = [
  'OOCGGGPPTTSSSTTP',
  'OCGGGGPPPTTTTPPF',
  'OCGGMGGPPPPPPPFF',
  'OCGGGGGGPPPPPFFB',
  'OOCGGGGPPPDDPFBB',
  'OCCGGGPPPDDDDPBU',
  'OCGGGPPPDDDDDPUU',
  'OOCGGGPPPDDDPPPU',
  'OOCCGGGGPPPPPPPP',
  'OOOCCCGGGGPPPPPP',
];
const RELIEF = [
  '........^^^.....',
  '.........h......',
  '................',
  '.......h........',
  '................',
  '..........h.....',
  '................',
  '................',
  '......h.........',
  '................',
];
const FEATURE = [
  '...ff...........',
  '..fff...........',
  '..ff......ff....',
  '...f.....ff.....',
  '...........aa...',
  '.....f.....a....',
  '................',
  '..ff............',
  '...ff...........',
  '................',
];
const OWNER = [
  '.aaaa...........',
  '.aaaaa......bbbb',
  '.aaaaa......bbbb',
  '..aaaa.......bbb',
  '...aa.........bb',
  '.........dddd...',
  '.........dddd...',
  '..ccc....dddd...',
  '.cccc...........',
  '.cccc...........',
];
const STACKS = [
  // Bannor (a)
  [3, 1, 'UNIT_WARRIOR', 'a', { hp: 100, moves: true }],
  [4, 2, 'UNIT_SWORDSMAN', 'a', { hp: 100, moves: true, count: 4, selected: true }],
  [5, 3, 'UNIT_CHAMPION', 'a', { hp: 100, moves: false }],
  [2, 3, 'UNIT_ARCHER', 'a', { hp: 100, moves: true, mark: 'fortify' }],
  [5, 1, 'UNIT_KNIGHT', 'a', { hp: 70, moves: true }],
  [6, 2, 'UNIT_HORSEMAN', 'a', { hp: 100, moves: false }],
  [3, 4, 'UNIT_CATAPULT', 'a', { hp: 100, moves: true, count: 2 }],
  [4, 0, 'UNIT_DISCIPLE_THE_ORDER', 'a', { hp: 100, moves: true }],
  [2, 2, 'UNIT_WORKER', 'a', { hp: 100, moves: true }],
  [7, 1, 'UNIT_DONAL', 'a', { hp: 100, moves: true, hero: true, count: 2 }],
  [1, 1, 'UNIT_TRIREME', 'a', { hp: 100, moves: true, count: 2 }],
  [0, 3, 'UNIT_FRIGATE', 'a', { hp: 100, moves: false }],
  // Sheaim (b)
  [13, 1, 'UNIT_IMP', 'b', { hp: 100, facing: 'left' }],
  [12, 2, 'UNIT_CHAOS_MARAUDER', 'b', { hp: 100, facing: 'left', count: 3 }],
  [14, 2, 'UNIT_PYRE_ZOMBIE', 'b', { hp: 100, facing: 'left' }],
  [15, 3, 'UNIT_MAGE', 'b', { hp: 100, facing: 'left' }],
  [13, 3, 'UNIT_PRIEST_OF_THE_VEIL', 'b', { hp: 100, facing: 'left' }],
  [14, 4, 'UNIT_DEATH_KNIGHT', 'b', { hp: 40, facing: 'left' }],
  [15, 5, 'UNIT_BALOR', 'b', { hp: 100, facing: 'left' }],
  // Ljosalfar (c)
  [2, 7, 'UNIT_LONGBOWMAN', 'c', { hp: 100 }],
  [3, 8, 'UNIT_GILDEN', 'c', { hp: 100, hero: true }],
  [4, 7, 'UNIT_RANGER', 'c', { hp: 100 }],
  [2, 9, 'UNIT_DRUID', 'c', { hp: 100 }],
  [1, 8, 'UNIT_HUNTER', 'c', { hp: 100, count: 2 }],
  [4, 9, 'UNIT_SETTLER', 'c', { hp: 100 }],
  // Khazad (d)
  [10, 5, 'UNIT_DWARVEN_SOLDIER_RUNES', 'd', { hp: 100, facing: 'left', count: 5 }],
  [11, 6, 'UNIT_MAROS', 'd', { hp: 100, facing: 'left', hero: true }],
  [9, 6, 'UNIT_BOAR_RIDER', 'd', { hp: 100 }],
  [12, 7, 'UNIT_DWARVEN_CANNON', 'd', { hp: 100, facing: 'left' }],
  [10, 7, 'UNIT_PRIEST_OF_KILMORPH', 'd', { hp: 100 }],
  [11, 5, 'UNIT_ADEPT', 'd', { hp: 100, facing: 'left' }],
  // barbarians and animals
  [9, 1, 'UNIT_ORTHUS', 'barb', { hp: 100, facing: 'left' }],
  [10, 3, 'UNIT_GOBLIN', 'barb', { hp: 100, facing: 'left', count: 3 }],
  [8, 4, 'UNIT_SKELETON', 'barb', { hp: 30 }],
  [12, 0, 'UNIT_WOLF_RIDER', 'barb', { hp: 100, facing: 'left' }],
  [7, 6, 'UNIT_LION', 'animal', { hp: 100 }],
  [6, 8, 'UNIT_BEAR', 'animal', { hp: 100 }],
  [8, 8, 'UNIT_GIANT_SPIDER', 'animal', { hp: 100, facing: 'left' }],
  [11, 9, 'UNIT_ELEPHANT', 'animal', { hp: 100 }],
  [1, 6, 'UNIT_SEA_SERPENT', 'animal', { hp: 100, facing: 'left' }],
  [0, 9, 'UNIT_KRAKEN', 'barb', { hp: 100 }],
  [14, 8, 'UNIT_HILL_GIANT', 'barb', { hp: 100, facing: 'left' }],
  [9, 9, 'UNIT_GRIFFON', 'animal', { hp: 100 }],
  [7, 3, 'UNIT_ANGEL', 'a', { hp: 100, moves: true }],
  [3, 2, 'UNIT_PHALANX', 'a', { hp: 100, moves: false, count: 2, mark: 'fortify', inCity: true }],
  [10, 6, 'UNIT_DWARVEN_SLINGER', 'd', { hp: 100, inCity: true, facing: 'left' }],
  [13, 9, 'UNIT_ACHERON', 'barb', { hp: 100, facing: 'left' }],
];
const CITIES = [
  { x: 3, y: 2, owner: 'a', size: 9, name: 'Aldmere', capital: true, own: true },
  { x: 14, y: 1, owner: 'b', size: 6, name: 'Gloam' },
  { x: 2, y: 8, owner: 'c', size: 4, name: 'Thorn' },
  { x: 10, y: 6, owner: 'd', size: 7, name: 'Karak' },
];

function buildMuster() {
  const T = { G: 'TERRAIN_GRASS', P: 'TERRAIN_PLAINS', D: 'TERRAIN_DESERT', T: 'TERRAIN_TUNDRA', S: 'TERRAIN_SNOW', M: 'TERRAIN_MARSH',
    C: 'TERRAIN_COAST', O: 'TERRAIN_OCEAN', B: 'TERRAIN_BROKEN_LANDS', F: 'TERRAIN_FIELDS_OF_PERDITION', U: 'TERRAIN_BURNING_SANDS' };
  const FEAT = { f: 'FEATURE_FOREST', a: 'FEATURE_FOREST_ANCIENT' };
  const W = TERRAIN[0].length, H = TERRAIN.length;
  const tiles = [];
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const t = TERRAIN[y][x];
    const water = t === 'O' || t === 'C';
    const r = RELIEF[y][x];
    const o = OWNER[y][x];
    tiles.push({ terrain: T[t], plotType: water ? 'OCEAN' : r === '^' ? 'PEAK' : r === 'h' ? 'HILLS' : 'LAND',
      feature: FEAT[FEATURE[y][x]] || null, owner: o === '.' ? null : o, vis: 2, river: 0, improvement: null, bonus: null, route: null, city: null });
  }
  for (const c of CITIES) tiles[c.y * W + c.x].city = c.name;
  return { width: W, height: H, wrapX: false, tiles };
}

function muster(art, owners, scale) {
  art.scale = scale;
  const map = buildMuster();
  const s = scale;
  const [c, ctx] = canvas(art, map.width * TILE * s, map.height * TILE * s, '0');
  // ?nostd=1 leaves the standards out (to measure what they add to the map's values)
  const nostd = q.get('nostd') === '1';
  const stacks = STACKS.map(([x, y, unit, owner, st]) => ({ x, y, unit, owner, count: st.count || 1, state: nostd ? { ...st, noPennant: true } : st }));
  renderMap(art, ctx, map, { owners, cities: CITIES, stacks, vignette: false });
  const sec = el('section');
  sec.append(el('h2', {}, `muster at ${scale}x`), c);
  return sec;
}

// ---------------------------------------------------------------- roster
function roster(art, owners) {
  const sec = el('section');
  sec.append(el('h2', {}, 'roster: bespoke sprites through drawUnit (2x)'));
  const byClass = new Map();
  for (const t of art.mapper.typeNames('unit')) {
    const r = art.mapper.resolve('unit', t);
    if (!r || r.via === 'category' || r.via === 'base') continue;
    const e = art.mapper.entry('unit', t);
    const k = e.combat === 'NONE' ? (e.domain === 'IMMOBILE' ? 'EQUIPMENT' : 'NONE') : e.combat;
    if (!byClass.has(k)) byClass.set(k, []);
    byClass.get(k).push([t, e]);
  }
  art.scale = 2;
  const s = 2;
  const civs = ['a', 'b', 'c', 'd'];
  for (const [k, list] of byClass) {
    const cols = 16;
    const rows = Math.ceil(list.length / cols);
    const cell = TILE + 4;
    const [c, ctx] = canvas(art, (cols * cell + 4) * s, (rows * (cell + 12) + 4) * s, '1');
    list.forEach(([t, e], i) => {
      const x = (4 + (i % cols) * cell) * s, y = (4 + 12 + Math.floor(i / cols) * (cell + 12)) * s;
      const sea = e.domain === 'SEA';
      art.drawTerrain(ctx, x, y, tile(sea ? 'TERRAIN_COAST' : 'TERRAIN_GRASS', { mx: i, my: rows, plotType: sea ? 'OCEAN' : 'LAND' }));
      const owner = e.combat === 'ANIMAL' ? owners.get('animal') : owners.get(civs[i % civs.length]);
      art.drawUnit(ctx, x, y, t, owner, { hp: 100, hero: !!e.hero });
    });
    sec.append(el('h3', {}, `${k} (${list.length})`), c);
  }
  return sec;
}

async function main() {
  const art = await loadArt({ base: new URL('../', import.meta.url), lenient: true, passes: false });
  if (art.lib.errors.length) throw new Error(art.lib.errors.join('\n'));
  installStackOverlay(art);
  const owners = new Map([
    ['a', art.ownerStyle('CIVILIZATION_BANNOR', { id: 'a' })],
    ['b', art.ownerStyle('CIVILIZATION_SHEAIM', { id: 'b' })],
    ['c', art.ownerStyle('CIVILIZATION_LJOSALFAR', { id: 'c' })],
    ['c2', art.ownerStyle('CIVILIZATION_LJOSALFAR', { id: 'c2', division: 'pale' })],
    ['d', art.ownerStyle('CIVILIZATION_KHAZAD', { id: 'd' })],
    ['h', art.ownerStyle('CIVILIZATION_LANUN', { id: 'h', hidden: true })],
    ['barb', art.ownerStyle('CIVILIZATION_BARBARIAN', { id: 'barb' })],
    ['animal', { ...art.ownerStyle('CIVILIZATION_BARBARIAN', { id: 'animal' }), animal: true }],
  ]);
  if (want('stacks')) out.append(stacks(art, owners));
  if (want('standards')) out.append(standards(art));
  if (want('lights')) out.append(lights(art, owners));
  if (want('muster')) {
    const sc = Number(q.get('scale')) || 0;
    for (const scale of sc ? [sc] : [2, 1]) out.append(muster(art, owners, scale));
  }
  if (want('roster')) out.append(roster(art, owners));
}

try {
  await main();
  window.__artReady = true;
} catch (e) {
  console.error(e);
  document.getElementById('err').textContent = String(e.stack || e);
  window.__artReady = String(e);
}
