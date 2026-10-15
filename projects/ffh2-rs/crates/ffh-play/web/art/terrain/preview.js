// terrain/preview.js: plates for the terrain set, rendered with compose.js through terrain/ground.js.
//   sample     the shared 20x14 sample map (preview/scene.js's layers), with cities and units
//   mosaic     single contrasting tiles and small blocks in grass, fog and the unrevealed edge
//   north      a polar band: ocean, ice, snow peaks, tundra, a fogged north
//   relief     hills on every land terrain, bare and under forest, ancient forest and jungle
//   woods      forest, ancient forest and jungle masses side by side; roads and a river with verges
//   landmarks  the 17 landmarks on suitable ground, lit
// ?scale=N (default 2; plates other than sample also at 3), ?pass=0 shows art.js's seed ground, ?light=0
// leaves out the light pass (cities/citylight.js: pools of landmarks, flames and cities).
import { loadArt, TILE } from '../art.js';
import { makePlots, renderMap } from '../compose.js';
import { installTerrain } from './ground.js';
import { installCityLight } from '../cities/citylight.js';

const q = new URLSearchParams(location.search);
const ONLY = q.get('map');
const SCALE = Number(q.get('scale')) || 0;
const PASS = q.get('pass') !== '0';

const T = { G: 'TERRAIN_GRASS', P: 'TERRAIN_PLAINS', D: 'TERRAIN_DESERT', T: 'TERRAIN_TUNDRA', S: 'TERRAIN_SNOW', M: 'TERRAIN_MARSH',
  C: 'TERRAIN_COAST', O: 'TERRAIN_OCEAN', B: 'TERRAIN_BROKEN_LANDS', F: 'TERRAIN_FIELDS_OF_PERDITION', U: 'TERRAIN_BURNING_SANDS', H: 'TERRAIN_SHALLOWS' };
const FEAT = { f: 'FEATURE_FOREST', a: 'FEATURE_FOREST_ANCIENT', j: 'FEATURE_JUNGLE', n: 'FEATURE_FOREST_NEW', b: 'FEATURE_FOREST_BURNT',
  x: 'FEATURE_FLAMES', i: 'FEATURE_ICE', v: 'FEATURE_VOLCANO', o: 'FEATURE_OASIS', p: 'FEATURE_FLOOD_PLAINS', s: 'FEATURE_SCRUB',
  z: 'FEATURE_BLIZZARD', k: 'FEATURE_OBSIDIAN_PLAINS' };

/** A map from layer strings (one character per tile): terrain, relief (^ peak, h hills), feature, vis. */
function build(L) {
  const H = L.terrain.length, W = L.terrain[0].length;
  const tiles = [];
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const t = L.terrain[y][x];
    const r = L.relief?.[y][x] || '.';
    const f = L.feature?.[y][x] || '.';
    const o = L.owner?.[y][x] || '.';
    tiles.push({
      terrain: T[t], plotType: t === 'O' || t === 'C' ? 'OCEAN' : r === '^' ? 'PEAK' : r === 'h' ? 'HILLS' : 'LAND',
      feature: FEAT[f] || null, owner: o === '.' ? null : o, vis: L.vis ? +L.vis[y][x] : 2, river: 0,
      improvement: L.improve?.[`${x},${y}`] || null, bonus: L.bonus?.[`${x},${y}`] || null,
      route: (L.roads || []).includes(`${x},${y}`) ? 'ROUTE_ROAD' : null, city: null,
    });
  }
  for (const [x, y, e] of L.river || []) tiles[y * W + x].river |= e === 's' ? 1 : 2;
  for (const c of L.cities || []) tiles[c.y * W + c.x].city = c.name;
  return { width: W, height: H, wrapX: false, tiles };
}

// ---- the shared sample map (preview/scene.js)
const SAMPLE = {
  terrain: [
    'OOOCCTTSSSSTTTGGPPPP', 'OOCCGTTTSSTTGGGGPPPF', 'OCCGGGTTTTGGGMMGPPFF', 'OCGGGGGGGGGGGMGPPFBB', 'OCCGGGGGGGGGGGPPFBBB',
    'OOCGGGGGPPGGGPPFBBUU', 'OOCCGGGPPPPGGPPFBUUU', 'OCOCCGGGPPPPPPPFBBUU', 'OOOOCGGGGPPPPDDPFFBB', 'OOOOCCGGGGPPDDDDPPFF',
    'OOOOOCCGGGGPPDDDDPPP', 'OOOCOOCCGGGGPPPDDPPP', 'OOCCCOOCCGGGGGPPPPPP', 'OOOOOOOOCCGGGGGGPPPP',
  ],
  relief: [
    '.....^^^^^^h........', '......^^^^h.........', '.......hh...........', '....................', '..........h.........',
    '...h.............h..', '..............h.....', '....................', '.........h......^...', '...............^^...',
    '....................', '....................', '....................', '....................',
  ],
  feature: [
    '....................', '....ff......ff......', '...fff....ffff......', '..fff....ffaaf......', '...ff....faaaf..x...',
    '....f.....ff.....x..', '..................xx', '.....ff.............', '.....fff.........j..', '....................',
    '........ff..........', '..........ff........', '....................', '....................',
  ],
  owner: [
    '....................', '....................', '................bbbb', '..aaaa.........bbbbb', '..aaaaaa......bbbbbb',
    '..aaaaaaa.....bbbbbb', '...aaaaaa......bbbbb', '....aaaa........bbbb', '.....aaa............', '.....ccccc..........',
    '.....ccccc..........', '......cccc..........', '.......ccc..........', '....................',
  ],
  vis: [
    '00011111111111111100', '01111111111111111110', '11112222222222222211', '11122222222222222221', '11222222222222222221',
    '11222222222222222221', '11222222222222222221', '11222222222222222211', '11122222222222222111', '11122222222222221111',
    '11112222222222211100', '11111222222221111000', '01111112222111110000', '00011111111111000000',
  ],
  improve: {
    '6,4': 'IMPROVEMENT_FARM', '5,5': 'IMPROVEMENT_FARM', '7,5': 'IMPROVEMENT_COTTAGE', '6,6': 'IMPROVEMENT_HAMLET',
    '3,5': 'IMPROVEMENT_MINE', '7,10': 'IMPROVEMENT_FARM', '8,9': 'IMPROVEMENT_COTTAGE', '12,7': 'IMPROVEMENT_STANDING_STONES',
    '17,7': 'IMPROVEMENT_MANA_CHAOS', '11,2': 'IMPROVEMENT_GOODY_HUT', '13,9': 'IMPROVEMENT_BEAR_DEN', '19,4': 'IMPROVEMENT_HELLFIRE',
  },
  bonus: { '4,6': 'BONUS_IRON', '3,9': 'BONUS_FISH', '9,6': 'BONUS_WHEAT', '14,10': 'BONUS_GOLD', '17,5': 'BONUS_MANA_CHAOS', '10,3': 'BONUS_DEER' },
  roads: ['5,4', '6,4', '7,4', '8,5', '9,6', '10,6', '11,6', '12,6', '13,6', '14,6', '15,5', '16,4', '6,5', '6,6', '6,7', '6,8', '7,9', '7,10', '4,4'],
  river: [[8, 1, 's'], [8, 2, 'e'], [9, 3, 's'], [9, 3, 'e'], [9, 4, 'e'], [9, 5, 'e'], [9, 6, 's'], [8, 6, 's'], [7, 7, 'e'], [7, 7, 's'], [6, 7, 's'], [5, 7, 's'], [4, 7, 's']],
  cities: [
    { x: 6, y: 5, owner: 'a', size: 9, name: 'Aldmere', capital: true, own: true, turns: 3 },
    { x: 16, y: 4, owner: 'b', size: 6, name: 'Gloam', capital: true },
    { x: 7, y: 10, owner: 'c', size: 3, name: 'Kerrow' },
  ],
  stacks: [
    { x: 8, y: 4, unit: 'UNIT_SWORDSMAN', owner: 'a', count: 3, state: { selected: true, hp: 100, moves: true } },
    { x: 10, y: 7, unit: 'UNIT_HORSEMAN', owner: 'a', count: 1, state: { hp: 60, moves: true } },
    { x: 4, y: 5, unit: 'UNIT_ARCHER', owner: 'a', count: 1, state: { hp: 100, moves: false, mark: 'fortify' } },
    { x: 11, y: 4, unit: 'UNIT_ADEPT', owner: 'a', count: 1, state: { hp: 100, moves: true } },
    { x: 2, y: 8, unit: 'UNIT_TRIREME', owner: 'a', count: 2, state: { hp: 100, moves: true } },
    { x: 14, y: 5, unit: 'UNIT_IMP', owner: 'b', count: 2, state: { hp: 80, facing: 'left' } },
    { x: 15, y: 7, unit: 'UNIT_DISCIPLE_THE_ASHEN_VEIL', owner: 'b', count: 1, state: { hp: 100, facing: 'left', accent: 'C' } },
    { x: 12, y: 3, unit: 'UNIT_SKELETON', owner: 'barb', count: 1, state: { hp: 30, facing: 'left' } },
    { x: 13, y: 8, unit: 'UNIT_WOLF', owner: 'animal', count: 1, state: { hp: 100, facing: 'left' } },
    { x: 8, y: 11, unit: 'UNIT_WORKER', owner: 'c', count: 1, state: { hp: 100, moves: true } },
    { x: 9, y: 9, unit: 'UNIT_SCOUT', owner: 'c', count: 1, state: { hp: 100, moves: true, facing: 'left' } },
  ],
};

// ---- single contrasting tiles, small blocks, fog and the unrevealed edge
const MOSAIC = {
  terrain: [
    'GGGGGGGGGGGGGG', 'GGBGGGUGGGFGGG', 'GGGGGGGGGGGGGG', 'GGHGGGDGGGSGGG', 'GGGGGGGGGGGGGG',
    'GPPGGDDGGTTGGM', 'GPPGGDDGGTTGMM', 'GGGGGGGGGGGGGG', 'GGUUGGGGFFGGSS', 'GGUUGGGGFFGGSS',
  ],
  vis: [
    '22222222211111', '22222222211111', '22222222211111', '22222222211111', '22222222222211',
    '22222221122221', '22222211112222', '22222222222220', '22222222222200', '22222222222000',
  ],
};

// ---- the polar band, its north fogged
const NORTH = {
  terrain: [
    'OOOOOOOOOOOOOOOO', 'OOCCSSSSSSCCOOOO', 'OCSSSSSSSSSSCOOO', 'CSSSSSSTTSSSSCCO', 'SSSSTTTTTTTSSSSC',
    'TTTTTTTGGTTTTTTT', 'TTGGGGGGGGGGGTTT', 'GGGGGGPPPGGGGGGG', 'GGGGGPPPPPGGGGGG', 'GGGGGGGGGGGGGGGG',
  ],
  relief: [
    '................', '......^^........', '.....^^^^.......', '..........h.....', '...h............',
    '.h...........h..', '................', '................', '................', '................',
  ],
  feature: [
    '.ii.........ii..', 'i.........i..i..', '................', '................', '........ff......',
    '.......fff..f...', '..ff.......ff...', '................', '................', '................',
  ],
  vis: [
    '1111111111111111', '1111111111111111', '1111111122222222', '1111112222222222', '1122222222222222',
    '2222222222222222', '2222222222222222', '2222222222222222', '2222222222222222', '2222222222222222',
  ],
};

// ---- hills on every land terrain: bare, under forest, ancient forest, jungle; flat woods; peaks
const RELIEF = {
  terrain: [
    'GGGGGGGGGGGGGGGGGGGGG',
    'GGGPGDGTGSGMGBGFGUGHG',
    'GGGGGGGGGGGGGGGGGGGGG',
    'GGGPGTGGGPGTGGGMGGGPG',
    'GGGGGGGGGGGGGGGGGGGGG',
    'GGGPGTGGGPGTGGGMGGGPG',
    'GGGGGGGGGGGGGGGGGGGGG',
    'GGGPGDGTGSGMGBGFGUGHG',
    'GGGGGGGGGGGGGGGGGGGGG',
  ],
  relief: [
    '.....................',
    '.h.h.h.h.h.h.h.h.h.h.',
    '.....................',
    '.h.h.h.h.h.h.h.h.h.h.',
    '.....................',
    '.....................',
    '.....................',
    '.^.^.^.^.^.^.^.^.^.^.',
    '.....................',
  ],
  feature: [
    '.....................',
    '.....................',
    '.....................',
    '.f.f.f.a.a.a.j.j.n.b.',
    '.....................',
    '.f.f.f.a.a.a.j.j.n.b.',
    '.....................',
    '.....................',
    '.....................',
  ],
};

// ---- wood masses, roads and a river with verges
const WOODS = {
  terrain: [
    'GGGGGGGGGGGGGGGGGG', 'GGGGGGGGGGGGGGGGMG', 'GGGGGGGGGGGGGGGMMG', 'GGGGGGGGGGGGGGGGGG', 'PPPPPPGGGGGGDDDDDD',
    'PPPPPPGGGGGGDDDDDD', 'TTTTTTGGGGGGDDDDDD',
  ],
  feature: [
    '..................', '.fff..aaa..jjj....', '.fff..aaa..jjj.jj.', '.ff....a...jj.....', '..................',
    '.nnn....bbb...ss..', '...............p..',
  ],
  roads: ['0,4', '1,4', '2,4', '3,4', '4,4', '5,4', '6,4', '7,5', '8,5', '9,5', '10,4', '11,4', '12,4', '13,4', '14,5', '15,6', '16,6'],
  river: [[4, 0, 'e'], [4, 1, 'e'], [4, 2, 'e'], [5, 2, 's'], [6, 2, 's'], [7, 2, 's'], [8, 2, 's'], [8, 3, 'e'], [8, 4, 'e'], [8, 5, 'e'], [8, 6, 'e']],
};

// ---- the landmarks on their grounds
const LANDMARKS = [
  ['STANDING_STONES', 'G'], ['POOL_OF_TEARS', 'G'], ['SEVEN_PINES', 'T'], ['YGGDRASIL', 'G'], ['BRADELINES_WELL', 'P'],
  ['TOMB_OF_SUCELLUS', 'P'], ['AIFON_ISLE', 'C'], ['MIRROR_OF_HEAVEN', 'D'], ['PYRE_OF_THE_SERAPHIC', 'P'], ['BROKEN_SEPULCHER', 'B'],
  ['RING_OF_CARCER', 'B'], ['ODIOS_PRISON', 'F'], ['LETUM_FRIGUS', 'S'], ['MAELSTROM', 'O'], ['GUARDIAN', 'T'],
  ['DRAGON_BONES', 'D'], ['REMNANTS_OF_PATRIA', 'G'],
];
function landmarkMap() {
  // one landmark every other tile on a 3-row strip of its ground
  const W = LANDMARKS.length * 2 + 1, H = 3;
  const terrain = [], improve = {};
  for (let y = 0; y < H; y++) {
    let row = '';
    for (let x = 0; x < W; x++) {
      const k = Math.floor((x - 1) / 2);
      const t = LANDMARKS[Math.max(0, Math.min(LANDMARKS.length - 1, k))][1];
      row += y === 1 ? t : (t === 'O' || t === 'C' ? t : 'G');
    }
    terrain.push(row);
  }
  LANDMARKS.forEach(([name], k) => { improve[`${k * 2 + 1},1`] = `IMPROVEMENT_${name}`; });
  return { terrain, improve };
}

async function plate(art, title, L, scale, extra = {}) {
  const map = build(L);
  const s = scale;
  art.scale = s;
  const owners = new Map([
    ['a', art.ownerStyle('CIVILIZATION_BANNOR', { id: 'a' })],
    ['b', art.ownerStyle('CIVILIZATION_SHEAIM', { id: 'b' })],
    ['c', art.ownerStyle('CIVILIZATION_DOVIELLO', { id: 'c' })],
    ['barb', art.ownerStyle('CIVILIZATION_BARBARIAN', { id: 'barb' })],
    ['animal', { ...art.ownerStyle('CIVILIZATION_BARBARIAN', { id: 'animal' }), animal: true }],
  ]);
  const c = document.createElement('canvas');
  c.width = map.width * TILE * s; c.height = map.height * TILE * s;
  const ctx = c.getContext('2d', { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  const t0 = performance.now();
  renderMap(art, ctx, map, { owners, cities: L.cities || [], stacks: L.stacks || [], vignette: false, ...extra });
  const ms = Math.round(performance.now() - t0);
  const div = document.createElement('div');
  div.className = 'plate';
  const h = document.createElement('h3');
  h.textContent = `${title} at ${s}x (${ms} ms)${PASS ? '' : ' - seed ground'}`;
  div.append(h, c);
  document.getElementById('out').append(div);
}

async function main() {
  const art = await loadArt({ base: new URL('../', import.meta.url), lenient: true, passes: false });
  if (art.lib.errors.length) throw new Error(art.lib.errors.join('\n'));
  if (PASS) installTerrain(art, q.get('rim') ? { rimKey: q.get('rim') } : {});
  if (q.get('light') !== '0') installCityLight(art); // the light pass: pools of landmarks, flames, cities
  const want = (m) => !ONLY || ONLY === m;
  const sc = (d) => SCALE || d;
  if (q.get('bench')) {
    // terrain layer of the sample map, three times from cold caches; the last run's phases
    const map = build(SAMPLE);
    art.scale = 2;
    const c = document.createElement('canvas');
    c.width = map.width * TILE * 2; c.height = map.height * TILE * 2;
    const ctx = c.getContext('2d', { willReadFrequently: true });
    let timings = {};
    for (let k = 0; k < 3; k++) {
      art._ground.clear();
      if (art._terrainCaches) art._terrainCaches();
      timings = {};
      renderMap(art, ctx, map, { cities: SAMPLE.cities, only: 'terrain', timings });
    }
    console.log(`bench ${PASS ? 'pass' : 'seed'}: ${JSON.stringify(timings)}`);
  }
  if (want('sample')) await plate(art, 'sample', SAMPLE, sc(2), { cursor: { x: 8, y: 4, selected: true }, vignette: true });
  if (want('mosaic')) await plate(art, 'mosaic', MOSAIC, sc(3));
  if (want('north')) await plate(art, 'north', NORTH, sc(3));
  if (want('relief')) await plate(art, 'relief', RELIEF, sc(3));
  if (want('woods')) await plate(art, 'woods', WOODS, sc(3));
  if (want('landmarks')) await plate(art, 'landmarks', landmarkMap(), sc(3));
}

try {
  await main();
  window.__artReady = true;
} catch (e) {
  console.error(e);
  document.getElementById('err').textContent = String(e.stack || e);
  window.__artReady = String(e);
}
