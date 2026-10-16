// ui/preview.js: the ui set in use. Default: a mock screen (top bar, map, city panel stub, unit panel,
// tooltip with combat odds, path preview, minimap); ?modal adds an event modal; ?kit shows every
// piece of the kit (frames, buttons, bars, icons, seals, marks, cursors, fonts). ?u=N sets the UI
// scale (default 2). Sets window.__artReady = true when done (the screenshot tools wait for it).
import { loadArt, TILE } from '../art.js';
import { renderMap, makePlots } from '../compose.js';
import * as UI from './ui.js';

const q = new URLSearchParams(location.search);
const U = Math.max(1, Math.min(4, Number(q.get('u')) || 2));
const DPR = Math.max(1, Math.round(window.devicePixelRatio || 1));
const K = U * DPR;
const O = { u: U, dpr: DPR };

function el(tag, cls, ...kids) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  for (const k of kids) if (k !== null && k !== undefined) e.append(k);
  return e;
}
const span = (cls, text) => el('span', cls, text);

// ---------------------------------------------------------------------------------- sample world

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
  '.....^^^^^^h........', '......^^^^h.........', '.......hh...........', '....................',
  '..........h.........', '...h.............h..', '..............h.....', '....................',
  '.........h......^...', '...............^^...', '....................', '....................',
  '....................', '....................',
];
const FEATURE = [
  '....................', '....ff......ff......', '...fff....ffff......', '..fff....ffaaf......',
  '...ff....faaaf..x...', '....f.....ff.....x..', '..................xx', '.....ff.............',
  '.....fff.........j..', '....................', '........ff..........', '..........ff........',
  '....................', '....................',
];
const OWNER = [
  '....................', '....................', '................bbbb', '..aaaa.........bbbbb',
  '..aaaaaa......bbbbbb', '..aaaaaaa.....bbbbbb', '...aaaaaa......bbbbb', '....aaaa........bbbb',
  '.....aaa............', '.....ccccc..........', '.....ccccc..........', '......cccc..........',
  '.......ccc..........', '....................',
];
const VIS = [
  '00011111111111111100', '01111111111111111110', '11112222222222222211', '11122222222222222221',
  '11222222222222222221', '11222222222222222221', '11222222222222222221', '11222222222222222211',
  '11122222222222222111', '11122222222222221111', '11112222222222211100', '11111222222221111000',
  '01111112222111110000', '00011111111111000000',
];
const IMPROVE = {
  '6,4': 'IMPROVEMENT_FARM', '5,5': 'IMPROVEMENT_FARM', '7,5': 'IMPROVEMENT_COTTAGE', '6,6': 'IMPROVEMENT_HAMLET',
  '3,5': 'IMPROVEMENT_MINE', '7,10': 'IMPROVEMENT_FARM', '8,9': 'IMPROVEMENT_COTTAGE', '12,7': 'IMPROVEMENT_STANDING_STONES',
  '17,7': 'IMPROVEMENT_MANA_CHAOS', '11,2': 'IMPROVEMENT_GOODY_HUT', '13,9': 'IMPROVEMENT_BEAR_DEN', '19,4': 'IMPROVEMENT_HELLFIRE',
};
const BONUS = { '4,6': 'BONUS_IRON', '3,9': 'BONUS_FISH', '9,6': 'BONUS_WHEAT', '14,10': 'BONUS_GOLD', '17,5': 'BONUS_MANA_CHAOS', '10,3': 'BONUS_DEER' };
const ROADS = ['5,4', '6,4', '7,4', '8,5', '9,6', '10,6', '11,6', '12,6', '13,6', '14,6', '15,5', '16,4', '6,5', '6,6', '6,7', '6,8', '7,9', '7,10', '4,4'];
const RIVER = [[8, 1, 's'], [8, 2, 'e'], [9, 3, 's'], [9, 3, 'e'], [9, 4, 'e'], [9, 5, 'e'], [9, 6, 's'], [8, 6, 's'], [7, 7, 'e'], [7, 7, 's'], [6, 7, 's'], [5, 7, 's'], [4, 7, 's']];
const CITIES = [
  { x: 6, y: 5, owner: 'a', size: 9, name: 'Aldmere', capital: true, own: true, turns: 3 },
  { x: 16, y: 4, owner: 'b', size: 6, name: 'Gloam', capital: true },
  { x: 7, y: 10, owner: 'c', size: 3, name: 'Kerrow' },
];
// own stacks (owner a) say whether they can still move; foreign stacks do not (their moves are unknown)
const STACKS = [
  { x: 9, y: 4, unit: 'UNIT_SWORDSMAN', owner: 'a', count: 3, state: { own: true, selected: true, hp: 100, moves: true, hero: true } },
  { x: 10, y: 7, unit: 'UNIT_HORSEMAN', owner: 'a', count: 1, state: { own: true, hp: 60, moves: true } },
  { x: 4, y: 5, unit: 'UNIT_ARCHER', owner: 'a', count: 1, state: { own: true, hp: 100, moves: false, mark: 'fortify' } },
  { x: 6, y: 5, unit: 'UNIT_ARCHER', owner: 'a', count: 2, state: { own: true, hp: 100, moves: false, mark: 'fortify', inCity: true } },
  { x: 11, y: 3, unit: 'UNIT_ADEPT', owner: 'a', count: 1, state: { own: true, hp: 100, moves: true, mark: 'sentry' } },
  { x: 2, y: 8, unit: 'UNIT_TRIREME', owner: 'a', count: 2, state: { own: true, hp: 100, moves: true } },
  { x: 14, y: 5, unit: 'UNIT_IMP', owner: 'b', count: 2, state: { hp: 80, facing: 'left' } },
  { x: 15, y: 7, unit: 'UNIT_DISCIPLE_THE_ASHEN_VEIL', owner: 'b', count: 1, state: { hp: 100, facing: 'left', accent: 'C' } },
  { x: 12, y: 2, unit: 'UNIT_SKELETON', owner: 'barb', count: 1, state: { hp: 30, facing: 'left' } },
  { x: 13, y: 8, unit: 'UNIT_WOLF', owner: 'animal', count: 1, state: { hp: 100, facing: 'left' } },
  { x: 8, y: 11, unit: 'UNIT_WORKER', owner: 'c', count: 1, state: { hp: 100 } },
  { x: 9, y: 9, unit: 'UNIT_SCOUT', owner: 'c', count: 1, state: { hp: 100, facing: 'left', mark: 'heal' } },
];
// the selected stack's path: two turns of march, then an attack on the imps
const ARMAGEDDON = Math.max(0, Math.min(100, Number(q.get('doom')) || 34)); // the counter shown in the top bar (?doom=N)
const PATH = [{ x: 9, y: 4 }, { x: 10, y: 4, turn: 1 }, { x: 11, y: 5, turn: 1 }, { x: 12, y: 5, turn: 2 }, { x: 13, y: 5, turn: 2 }, { x: 14, y: 5, turn: 2 }];

function buildMap() {
  const T = { G: 'TERRAIN_GRASS', P: 'TERRAIN_PLAINS', D: 'TERRAIN_DESERT', T: 'TERRAIN_TUNDRA', S: 'TERRAIN_SNOW', M: 'TERRAIN_MARSH',
    C: 'TERRAIN_COAST', O: 'TERRAIN_OCEAN', B: 'TERRAIN_BROKEN_LANDS', F: 'TERRAIN_FIELDS_OF_PERDITION', U: 'TERRAIN_BURNING_SANDS' };
  const FEAT = { f: 'FEATURE_FOREST', a: 'FEATURE_FOREST_ANCIENT', j: 'FEATURE_JUNGLE', x: 'FEATURE_FLAMES' };
  const W = 20, H = 14, tiles = [];
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const t = TERRAIN[y][x], r = RELIEF[y][x], o = OWNER[y][x];
    const water = t === 'O' || t === 'C';
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

function owners(art) {
  return new Map([
    ['a', art.ownerStyle('CIVILIZATION_BANNOR', { id: 'a' })],
    ['b', art.ownerStyle('CIVILIZATION_SHEAIM', { id: 'b' })],
    ['c', art.ownerStyle('CIVILIZATION_DOVIELLO', { id: 'c' })],
    ['barb', art.ownerStyle('CIVILIZATION_BARBARIAN', { id: 'barb' })],
    ['animal', { ...art.ownerStyle('CIVILIZATION_BARBARIAN', { id: 'animal' }), animal: true }],
  ]);
}

// ---------------------------------------------------------------------------------- small pieces

const icon = (art, name, title, cls, remap) => UI.iconEl(art, name, { ...O, title, cls, remap });
const vigil = (art, text, color = '7', extra = {}) => UI.textEl(art, text, { ...O, color, ...extra });
// bitmap text whose colour follows CSS (ui.js textSpan): the body face, and Vigil small capitals
const txt = (art, text, cls) => UI.textSpan(art, text, { ...O, cls });
const caps = (art, text, cls) => UI.textSpan(art, text, { ...O, font: 'vigil', smallCaps: true, cls });
const tally = (art, text, color = '7') => UI.textEl(art, text, { ...O, font: 'tally', color });

function canvasEl(w, h) {
  const c = document.createElement('canvas');
  c.width = w * K; c.height = h * K;
  c.style.width = `${w * U}px`; c.style.height = `${h * U}px`;
  c.className = 'ui-px';
  const ctx = c.getContext('2d', { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  return [c, ctx];
}

function bar(art, w, frac, kind, next) {
  const [c, ctx] = canvasEl(w, 5);
  art.scale = K;
  UI.drawBar(art, ctx, 0, 0, w, frac, kind, { next });
  return c;
}

function slider(art, w, pct) {
  const [c, ctx] = canvasEl(w + 6, 10);
  art.scale = K;
  UI.drawSlider(art, ctx, 3 * K, 0, w, pct);
  return c;
}

function rule(art, w) {
  const [c, ctx] = canvasEl(w, 3);
  art.scale = K;
  UI.drawRule(art, ctx, 0, K, w);
  c.classList.add('rule');
  return c;
}

/** A unit on a coal card, drawn through the mapping (contact shadow, rim, no overlays). */
function unitCard(art, unit, owner, selected, hp) {
  const card = el('div', 'card ' + (selected ? 'ui-frame-card-selected' : 'ui-frame-raised'));
  const [c, ctx] = canvasEl(26, 24);
  art.scale = K;
  art.drawUnit(ctx, -5 * K, -6 * K, unit, owner, { layer: 'sprite', selected: false, lit: false });
  ctx.fillStyle = art.css('2');
  ctx.fillRect(4 * K, 22 * K, 18 * K, K);
  const f = Math.round((18 * hp) / 100);
  ctx.fillStyle = art.css(hp > 50 ? '7' : hp >= 25 ? 'y' : 'C');
  ctx.fillRect(4 * K, 22 * K, f * K, K);
  card.append(c);
  return card;
}

// ---------------------------------------------------------------------------------- the screen

function topBar(art) {
  const bar = el('div', 'topbar');
  const box = (...kids) => el('div', 'box ui-frame-panel', ...kids);
  const item = (...kids) => el('div', 'item', ...kids);
  const sep = () => el('div', 'sep');
  // treasury and research
  bar.append(box(
    item(icon(art, 'icon16/gold', 'Gold'), vigil(art, '214', '8'), txt(art, '+12', 'pos')),
    item(slider(art, 40, 60), tally(art, '60%', '7')),
    sep(),
    item(icon(art, 'icon16/research', 'Research'), txt(art, 'Way of the Wise'), bar_(art, 40, 0.62, 'research', 0.7), vigil(art, '4', '7')),
    sep(),
    item(icon(art, 'icon16/culture', 'Culture'), vigil(art, '38', '7'), txt(art, '+3', 'dim')),
  ));
  // mana and the Armageddon counter
  const mana = item(icon(art, 'icon16/mana', 'Mana'));
  for (const [sphere, n] of [['FIRE', 2], ['DEATH', 1], ['NATURE', 1], ['BODY', 1]]) {
    const name = UI.iconName('mana', sphere, 8);
    mana.append(art.hasSprite(name) ? icon(art, name, sphere) : icon(art, 'icon8/bullet', sphere), tally(art, String(n), '7'));
  }
  bar.append(box(mana, sep(), item(icon(art, UI.iconName('armageddon', ARMAGEDDON), 'Armageddon counter'), vigil(art, String(ARMAGEDDON), '7'))));
  bar.append(el('div', 'grow'));
  // golden age, AI-controlled units idle
  bar.append(box(item(icon(art, 'icon16/golden_age', 'Golden age: 6 turns'), vigil(art, '6', '7')), sep(),
    item(icon(art, 'icon16/ai_idle', 'AI-controlled units idle'), vigil(art, '2', '7'))));
  // turn and End Turn (the one firelit control: the amber crescent)
  bar.append(box(item(caps(art, 'Turn', 'dim'), vigil(art, '112', '8')), sep(), item(caps(art, 'Year', 'dim'), vigil(art, '274', '7'))));
  const end = el('button', 'ui-button primary endturn', icon(art, 'icon16/turn'), caps(art, 'End Turn'));
  bar.append(end);
  return bar;
}
const bar_ = bar;

/** A list row: an 8 px icon, the name, and what sits at the right. */
function listRow(art, cls, ic, name, ...right) {
  const i = ic ? icon(art, ic.name, null, null, ic.remap) : el('span', 'noicon');
  return el('div', 'li ' + cls, i, txt(art, name), el('span', 'r row', ...right));
}

function cityPanel(art) {
  const p = el('div', 'panel ui-frame-panel side');
  const title = el('div', 'title ui-frame-title', icon(art, 'icon16/civic_government', 'Capital'), vigil(art, 'Aldmere', '8'), txt(art, 'size 9', 'sub'));
  p.append(title);
  const row = (...kids) => el('div', 'row', ...kids);
  p.append(row(icon(art, 'icon16/food'), txt(art, 'Growth'), bar(art, 56, 0.55, 'food', 0.68), txt(art, '6 turns', 'r dim')));
  p.append(row(icon(art, 'icon16/production'), txt(art, 'Swordsman'), bar(art, 52, 0.7, 'production', 0.85), txt(art, '3 turns', 'r dim')));
  p.append(rule(art, 140));
  const y = el('div', 'yields');
  for (const [n, v] of [['food', '14'], ['production', '9'], ['commerce', '12']]) y.append(el('div', 'item', icon(art, 'icon16/' + n, n), vigil(art, v, '7')));
  p.append(y);
  const c = el('div', 'yields section');
  for (const [n, v] of [['gold', '6'], ['research', '5'], ['culture', '3']]) c.append(el('div', 'item', icon(art, 'icon8/' + n, n), txt(art, v)));
  p.append(c);
  const h = el('div', 'yields section');
  h.append(el('div', 'item', icon(art, 'icon8/happy'), txt(art, '7')), el('div', 'item', icon(art, 'icon8/unhappy'), txt(art, '5')),
    el('div', 'item', icon(art, 'icon8/healthy'), txt(art, '6')), el('div', 'item', icon(art, 'icon8/unhealthy'), txt(art, '4')),
    el('div', 'item', icon(art, 'icon8/defense'), txt(art, '40%')));
  p.append(h);
  const rel = el('div', 'row section', caps(art, 'Faiths', 'cap'));
  for (const r of ['RELIGION_THE_ORDER', 'RELIGION_FELLOWSHIP_OF_LEAVES']) {
    const ic = UI.iconFor(art, 'religion', r, 8);
    rel.append(icon(art, ic.name, r, null, ic.remap));
  }
  rel.append(caps(art, 'Great person', 'r cap'), bar(art, 30, 0.3, 'gp'));
  p.append(rel);
  const cit = el('div', 'row section', caps(art, 'Citizens', 'cap'));
  for (const [sp, n] of [['SPECIALIST_CITIZEN', 2], ['SPECIALIST_PRIEST', 1], ['SPECIALIST_SCIENTIST', 1], ['SPECIALIST_GREAT_ENGINEER', 1]]) {
    const r = art.resolve('specialist', sp);
    cit.append(icon(art, r.sprite, sp, null, r.opts.remap), txt(art, String(n)));
  }
  p.append(cit);
  p.append(rule(art, 140));
  // the queue: units show their combat class, buildings their class
  const queue = el('div', 'list');
  const unitIc = (u) => UI.iconFor(art, 'unitcombat', art.mapper.entry('unit', u)?.combat ? `UNITCOMBAT_${art.mapper.entry('unit', u).combat}` : null, 8) || { name: 'icon8/production' };
  const bIc = (b) => UI.iconFor(art, 'building', b, 8);
  queue.append(listRow(art, 'sel', unitIc('UNIT_SWORDSMAN'), 'Swordsman', txt(art, '3')),
    listRow(art, '', bIc('BUILDING_TEMPLE_OF_KILMORPH'), 'Temple of Kilmorph', txt(art, '9')),
    listRow(art, '', bIc('BUILDING_GREAT_LIBRARY'), 'The Great Library', txt(art, '31')),
    listRow(art, '', unitIc('UNIT_WORKER'), 'Worker', txt(art, '13')));
  p.append(caps(art, 'Queue', 'cap head'), queue);
  p.append(rule(art, 140));
  p.append(caps(art, 'Buildings', 'cap head'));
  const list = el('div', 'list ui-frame-inset');
  const eff = (...names) => names.map((n) => icon(art, 'icon8/' + n));
  list.append(listRow(art, '', bIc('BUILDING_PALACE_BANNOR'), 'Palace', ...eff('research', 'gold')),
    listRow(art, '', bIc('BUILDING_GRANARY'), 'Granary', ...eff('food')),
    listRow(art, 'hover', bIc('BUILDING_TRAINING_YARD'), 'Training Yard', ...eff('experience')),
    listRow(art, 'sel', bIc('BUILDING_ELDER_COUNCIL'), 'Elder Council', ...eff('research')),
    listRow(art, '', bIc('BUILDING_MARKET'), 'Market', ...eff('gold', 'happy')),
    listRow(art, '', bIc('BUILDING_TEMPLE_OF_THE_ORDER'), 'Temple of the Order', ...eff('happy', 'culture')),
    listRow(art, '', bIc('BUILDING_MAGE_GUILD'), 'Mage Guild', ...eff('research')),
    listRow(art, '', bIc('BUILDING_MONUMENT'), 'Monument', ...eff('culture')),
    listRow(art, '', bIc('BUILDING_WALLS'), 'Walls', ...eff('defense')));
  p.append(list);
  const btns = el('div', 'buttons');
  const b = (label, cls = '', disabled = false) => { const e = el('button', 'ui-button ' + cls, caps(art, label)); if (disabled) e.disabled = true; return e; };
  btns.append(b('Hurry'), b('Queue', 'primary'), b('Conscript', '', true), b('Raze', 'danger'));
  p.append(btns);
  return p;
}

function unitPanel(art, own) {
  const p = el('div', 'overlay unitpanel panel ui-frame-panel');
  p.append(el('div', 'title ui-frame-title', icon(art, 'icon8/combat_melee'), vigil(art, 'Swordsman', '8'), txt(art, 'stack of 3', 'sub')));
  const cards = el('div', 'cards');
  cards.append(unitCard(art, 'UNIT_SWORDSMAN', own, true, 100), unitCard(art, 'UNIT_SWORDSMAN', own, false, 70), unitCard(art, 'UNIT_ARCHER', own, false, 20));
  const stats = el('div', '');
  const r = (...k) => el('div', 'row', ...k);
  stats.append(r(icon(art, 'icon8/strength'), txt(art, '6'), icon(art, 'icon8/moves'), txt(art, '1/1'), icon(art, 'icon8/experience'), bar(art, 30, 0.6, 'xp'), txt(art, '6/10', 'dim')));
  const seals = el('div', 'seals');
  for (const t of ['PROMOTION_COMBAT2', 'PROMOTION_SHOCK', 'PROMOTION_DRILL1', 'PROMOTION_CITY_RAIDER1', 'PROMOTION_HERO', 'PROMOTION_FIRE2', 'PROMOTION_BRONZE_WEAPONS', 'PROMOTION_POISONED']) seals.append(UI.promotionEl(art, t, O));
  stats.append(el('div', 'section', seals));
  const top = el('div', 'row', cards, stats);
  top.style.alignItems = 'flex-start';
  p.append(top);
  const btns = el('div', 'buttons');
  const b = (label, cls = '', disabled = false, ic = null) => { const e = el('button', 'ui-button ' + cls, ic ? icon(art, ic) : null, caps(art, label)); if (disabled) e.disabled = true; return e; };
  btns.append(b('Fortify', '', false, 'icon8/defense'), b('Sleep'), b('Pillage', '', true), b('Attack', 'primary', false, 'icon8/strength'), b('Delete', 'danger'));
  p.append(btns);
  return p;
}

function tooltip(art) {
  const t = el('div', 'overlay tip ui-frame-tooltip');
  const r = (...k) => el('div', 'row', ...k);
  t.append(r(vigil(art, 'Imp', '8'), txt(art, 'Sheaim', 'r dim')));
  t.append(r(icon(art, 'icon8/strength'), txt(art, '4 vs 6'), el('span', 'r'), icon(art, 'icon8/defense'), txt(art, '+25%', 'dim')));
  t.append(r(txt(art, 'Odds', 'dim'), txt(art, '73%', 'em'), txt(art, 'withdraw 0%', 'r dim')));
  t.append(r(icon(art, 'icon8/bullet'), txt(art, 'Shock: +25% vs melee')));
  t.append(r(icon(art, 'icon8/bullet'), txt(art, 'Fortified: +25%', 'neg')));
  return t;
}

function minimap(art, map, own, view) {
  const k = 2, w = map.width * k, h = map.height * k;
  const box = el('div', 'overlay minimap ui-frame-panel');
  const [c, ctx] = canvasEl(w, h);
  const base = { TERRAIN_OCEAN: 'd', TERRAIN_COAST: 'n', TERRAIN_GRASS: 'm', TERRAIN_PLAINS: 'Q', TERRAIN_DESERT: 'Y', TERRAIN_TUNDRA: '4',
    TERRAIN_SNOW: 'f', TERRAIN_MARSH: 'q', TERRAIN_BROKEN_LANDS: 'u', TERRAIN_FIELDS_OF_PERDITION: 'p', TERRAIN_BURNING_SANDS: 'x' };
  map.tiles.forEach((t, i) => {
    const tx = i % map.width, ty = Math.floor(i / map.width);
    let key = t.vis === 0 ? '0' : t.owner && own.get(t.owner) && !own.get(t.owner).animal ? art.keys[own.get(t.owner).tincture.field] : base[t.terrain] || '4';
    if (t.vis === 1) key = art.keys[art.tables.FOG[art.K(key)]];
    if (t.city) key = '8';
    ctx.fillStyle = art.css(key);
    ctx.fillRect(tx * k * K, ty * k * K, k * K, k * K);
  });
  ctx.strokeStyle = art.css('8');
  ctx.lineWidth = K;
  ctx.strokeRect((view.x * k + 0.5) * K, (view.y * k + 0.5) * K, (view.w * k - 1) * K, (view.h * k - 1) * K);
  box.append(c);
  return box;
}

function modal(art) {
  const back = el('div', 'backdrop');
  const m = el('div', 'modal ui-frame-modal');
  const head = el('div', 'title ui-frame-title', icon(art, 'icon16/religion_the_ashen_veil'), vigil(art, 'The Pale Procession', '8'));
  m.append(head);
  const body = el('div', 'body');
  const [ini, ictx] = canvasEl(22, 22);
  art.scale = K;
  UI.drawInitial(art, ictx, 0, 0, 'A');
  ini.classList.add('initial');
  body.append(ini, el('p', '', 'cloaked company of pilgrims has come down the old road to Aldmere at dusk, bearing a reliquary sealed with black wax. ',
    span('link', 'The Ashen Veil'), ' has no temple here; the elders of the city are uneasy, and the people stay behind shuttered windows.'));
  m.append(body);
  const opts = el('div', 'opts');
  const opt = (label, effect, cls = '', disabled = false) => {
    const b = el('button', 'ui-button ' + cls, txt(art, label), UI.textBlock(art, effect, 180, { ...O, cls: 'effect' }));
    if (disabled) b.disabled = true;
    return b;
  };
  opts.append(opt('Welcome them into the city.', 'Spreads The Ashen Veil. +1 unhappiness for 10 turns.'),
    opt('Turn them away at the gate.', 'Nothing happens.', 'primary'),
    opt('Put the procession to the sword.', '+2 Armageddon counter. The Sheaim remember this.', 'danger'),
    opt('Ask the Order to judge them.', 'Requires The Order as state religion.', '', true));
  m.append(opts);
  back.append(m);
  return back;
}

async function screen(art) {
  const app = document.getElementById('app');
  const own = owners(art);
  const map = buildMap();
  const plots = makePlots(map);
  const scr = el('div', 'screen ui-cursor');
  scr.append(topBar(art));
  const main = el('div', 'main');
  const page = el('div', 'mapwrap ui-frame-page');
  const inner = el('div', 'mapwrap');
  inner.style.width = '100%'; inner.style.height = '100%';
  page.append(inner);
  main.append(page, cityPanel(art));
  scr.append(main);
  app.append(scr);
  // the map fills the frame at its own zoom (?zoom=N, default 2: 64 CSS px per tile), independent of
  // the UI scale; tile (TX, TY) at the top-left
  const Z = Math.max(1, Math.min(4, Number(q.get('zoom')) || 2));
  const MK = Z * DPR;
  const TX = 2, TY = 0;
  const cssW = inner.clientWidth, cssH = inner.clientHeight;
  const c = document.createElement('canvas');
  c.width = cssW * DPR; c.height = cssH * DPR;
  c.style.width = `${cssW}px`; c.style.height = `${cssH}px`;
  c.className = 'ui-px map';
  const ctx = c.getContext('2d', { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  inner.append(c);
  const W = Math.ceil(cssW / Z), H = Math.ceil(cssH / Z);
  art.scale = MK;
  const cols = Math.min(map.width - TX, Math.ceil(W / TILE) + 1), rows = Math.min(map.height - TY, Math.ceil(H / TILE) + 1);
  renderMap(art, ctx, map, { plots, owners: own, cities: CITIES, stacks: STACKS, x0: 0, y0: 0, tx0: TX, ty0: TY, cols, rows });
  const ox = -TX * TILE * MK, oy = -TY * TILE * MK;
  UI.drawTileMark(art, ctx, ox + 9 * TILE * MK, oy + 4 * TILE * MK, 'select');
  UI.drawPath(art, ctx, ox, oy, PATH, { kind: 'attack', odds: 73 });
  UI.drawTileMark(art, ctx, ox + 11 * TILE * MK, oy + 3 * TILE * MK, 'hover');
  UI.drawRange(art, ctx, ox + 12 * TILE * MK, oy + 2 * TILE * MK, { mx: 12, my: 2 });
  // overlays: tooltip by the target, unit panel bottom-left, minimap bottom-right
  const tip = tooltip(art);
  inner.append(tip);
  // follow the hovered tile at +12, +12 art px, clamped to the map view (11.7)
  tip.style.left = `${Math.min(((14 - TX) * TILE + 30) * Z, cssW - tip.offsetWidth - 4 * U)}px`;
  tip.style.top = `${Math.min(((5 - TY) * TILE + 6) * Z, cssH - tip.offsetHeight - 4 * U)}px`;
  inner.append(unitPanel(art, own.get('a')), minimap(art, map, own, { x: TX, y: TY, w: Math.floor(W / TILE), h: Math.floor(H / TILE) }));
  if (q.has('modal')) app.append(modal(art));
}

// ---------------------------------------------------------------------------------- the kit sheet

function cell(label, ...kids) {
  return el('div', 'cell', ...kids, el('div', 'lbl', label));
}

function kit(art) {
  const app = document.getElementById('app');
  const k = el('div', 'kit');
  app.append(k);
  const only = (q.get('kit') || '').split(',').filter(Boolean);
  const want = (s) => !only.length || only.includes(s);
  let cur = k;
  const h2 = (t) => { cur = el('section'); cur.dataset.key = t.split(/[ :,]/)[0]; k.append(cur); cur.append(el('h2', '', t)); };
  const grid = () => { const g = el('div', 'grid'); cur.append(g); return g; };
  k.append(el('h1', '', 'ui kit'), el('div', 'note', `UI scale u = ${U} (one art pixel = ${U} CSS px). Frames and buttons are nine-slice sprites, shown here stretched to several sizes; the DOM rows use the generated CSS (ui.js cssText).`));

  h2('frames');
  let g = grid();
  for (const name of [...art.lib.sprites.keys()].filter((n) => n.startsWith('ui/frame_')).sort()) {
    g.append(cell(name, UI.nineEl(art, name, 64, 28, O), UI.nineEl(art, name, 24, 16, O)));
  }
  g = grid();
  const ttl = el('div', 'ui-frame-panel panel');
  ttl.style.width = `${120 * U}px`;
  ttl.append(el('div', 'title ui-frame-title', icon(art, 'icon8/research'), vigil(art, 'Panel Title', '8'), span('sub', 'aside')), el('div', 'small', 'A soot panel with a double rule: ash outside, iron inside, gilt nails at the corners.'), rule(art, 110));
  g.append(cell('DOM: .ui-frame-panel + .ui-frame-title', ttl));
  const tip = el('div', 'ui-frame-tooltip tip');
  tip.append(el('div', 'head', 'Tooltip'), el('div', 'small', 'Soot, stone border, 4 art px of padding.'));
  g.append(cell('DOM: .ui-frame-tooltip', tip));

  h2('buttons');
  g = grid();
  for (const kind of ['normal', 'primary', 'danger']) {
    const [c, ctx] = canvasEl(4 * 60 + 3 * 6 + 64, 15);
    art.scale = K;
    ['normal', 'hover', 'pressed', 'disabled'].forEach((state, i) => UI.drawButton(art, ctx, i * 66 * K, 2 * K, 60, 11, state === 'normal' ? `${kind}` : state, { kind, state, icon: kind === 'primary' ? 'icon8/defense' : null }));
    UI.drawButton(art, ctx, 4 * 66 * K + 2 * K, 2 * K, 56, 11, 'focus', { kind, focus: true });
    g.append(cell(`${kind}: normal, hover, pressed, disabled, focus (canvas)`, c));
  }
  g = grid();
  const dom = el('div', 'buttons');
  const b = (label, cls = '', disabled = false) => { const e = el('button', 'ui-button ' + cls, caps(art, label)); if (disabled) e.disabled = true; return e; };
  dom.append(b('Fortify'), b('Queue', 'primary'), b('Disabled', '', true), b('Raze', 'danger'), el('button', 'ui-button primary', icon(art, 'icon16/turn'), caps(art, 'End Turn')));
  g.append(cell('DOM: .ui-button, .primary, :disabled, .danger (labels: textSpan, Vigil small capitals; hover to see them follow the state)', dom));

  h2('bars, slider, rule');
  g = grid();
  for (const kind of ['food', 'production', 'research', 'culture', 'gp', 'xp', 'hp', 'hp_low', 'doom']) g.append(cell(kind, bar(art, 48, 0.6, kind, 0.75)));
  {
    const [c, ctx] = canvasEl(64, 10);
    art.scale = K;
    UI.drawSlider(art, ctx, 2 * K, 0, 60, 70);
    g.append(cell('slider 70%', c), cell('rule with ornament', rule(art, 64)));
  }

  const icons = (prefix, filter = () => true) => [...art.lib.sprites.keys()].filter((n) => n.startsWith(prefix) && filter(n)).sort();
  const iconRow = (names, title) => {
    h2(title);
    const gg = grid();
    for (const n of names) {
      const strip = el('div', 'strip');
      strip.append(UI.iconEl(art, n, { u: 1, dpr: DPR }), UI.iconEl(art, n, O));
      const coal = el('div', 'strip on-coal');
      coal.append(UI.iconEl(art, n, O));
      gg.append(cell(n.replace(/^icon(16|8)\//, ''), el('div', 'row', strip, coal)));
    }
  };
  iconRow(icons('icon16/', (n) => !/religion_|alignment_|civic_|sphere_|promo_|specialist_|bclass_|^icon16\/_/.test(n)), 'icons 16: yields, commerce, mood, health, stats, empire (1x, u, on coal)');
  iconRow(icons('icon8/', (n) => !/religion_|alignment_|civic_|sphere_|combat_|bclass_|^icon8\/_/.test(n)), 'icons 8: inline and on map tiles');
  iconRow([...icons('icon16/religion_'), ...icons('icon8/religion_'), ...icons('icon16/alignment_'), ...icons('icon8/alignment_')], 'religions and alignments');
  iconRow([...icons('icon16/civic_'), ...icons('icon8/civic_')], 'civic categories');
  if (icons('icon8/sphere_').length) iconRow([...icons('icon8/sphere_'), ...icons('icon16/sphere_')], 'mana spheres');
  if (icons('icon8/combat_').length) iconRow(icons('icon8/combat_'), 'unit combat classes');
  if (icons('icon16/specialist_').length) iconRow(icons('icon16/specialist_'), 'specialists');

  if (icons('icon16/bclass_').length) {
    h2('buildings: class icons (16 px at 1x and u, 8 px at u; how many of the 170 buildings take each)');
    const gb = grid();
    const byIcon = new Map();
    for (const t of art.mapper.typeNames('building')) {
      const r = art.mapper.resolve('building', t);
      if (!byIcon.has(r.sprite)) byIcon.set(r.sprite, []);
      byIcon.get(r.sprite).push(t);
    }
    for (const n of icons('icon16/bclass_')) {
      const list = byIcon.get(n) || [];
      const small = 'icon8/' + n.slice(7).replace(/^bclass_temple_/, 'religion_'); // as UI.iconFor(..., 8)
      const strip = el('div', 'strip');
      strip.append(UI.iconEl(art, n, { u: 1, dpr: DPR }), UI.iconEl(art, n, O), art.hasSprite(small) ? UI.iconEl(art, small, O) : null);
      strip.title = list.join(' ');
      gb.append(cell(`${n.replace('icon16/bclass_', '')} (${list.length})`, strip));
    }
  }
  h2('promotion seals (rim = family, pips = level)');
  g = grid();
  const seen = new Set();
  for (const t of art.mapper.typeNames('promotion')) {
    const r = art.mapper.resolve('promotion', t);
    const key = `${r.sprite}|${r.opts.remap || ''}`;
    if (seen.has(key) && !/1$|2$|3$/.test(t)) continue;
    if (seen.has(key) && seen.size > 400) continue;
    if (seen.has(key)) continue;
    seen.add(key);
    g.append(cell(t.replace('PROMOTION_', '').toLowerCase(), UI.promotionEl(art, t, O)));
  }
  g = grid();
  for (const t of ['PROMOTION_COMBAT1', 'PROMOTION_COMBAT2', 'PROMOTION_COMBAT3', 'PROMOTION_COMBAT4', 'PROMOTION_COMBAT5']) g.append(cell(t.replace('PROMOTION_', '').toLowerCase(), UI.promotionEl(art, t, O)));

  h2('map marks: tile cursors, path, odds, range, state marks');
  g = grid();
  const mini = (fn, w = 3, h = 2, terrain = 'TERRAIN_GRASS') => {
    const [c, ctx] = canvasEl(w * TILE, h * TILE);
    art.scale = K;
    const tiles = [];
    for (let i = 0; i < w * h; i++) tiles.push({ terrain, plotType: 'LAND', vis: 2, river: 0 });
    renderMap(art, ctx, { width: w, height: h, wrapX: false, tiles }, { vignette: false });
    fn(ctx);
    return c;
  };
  for (const kind of ['hover', 'select', 'attack', 'route', 'target']) g.append(cell(`tile ${kind}`, mini((ctx) => UI.drawTileMark(art, ctx, 0, 0, kind), 1, 1)));
  g.append(cell('path: two turns, then an attack (73%)', mini((ctx) => UI.drawPath(art, ctx, 0, 0, [{ x: 0, y: 2 }, { x: 1, y: 2, turn: 1 }, { x: 2, y: 2, turn: 2 }, { x: 3, y: 1, turn: 2 }, { x: 4, y: 1, turn: 2 }], { kind: 'attack', odds: 73 }), 5, 3)));
  g.append(cell('path: unreachable, beyond this turn', mini((ctx) => UI.drawPath(art, ctx, 0, 0, [{ x: 0, y: 0 }, { x: 1, y: 1, turn: 1 }, { x: 2, y: 1, turn: 2 }, { x: 3, y: 1, turn: 2 }], { unreachable: true }), 4, 2, 'TERRAIN_PLAINS')));
  g.append(cell('route-to path (ochre), 3 turns', mini((ctx) => UI.drawPath(art, ctx, 0, 0, [{ x: 0, y: 0 }, { x: 1, y: 0, turn: 1 }, { x: 2, y: 1, turn: 2 }, { x: 3, y: 1, turn: 3 }], { kind: 'route' }), 4, 2, 'TERRAIN_PLAINS')));
  g.append(cell('odds chips: 73%, 31%, 99.2%', mini((ctx) => { UI.drawOdds(art, ctx, 16 * K, 16 * K, 73); UI.drawOdds(art, ctx, 48 * K, 16 * K, 31); UI.drawOdds(art, ctx, 80 * K, 16 * K, 99.2); }, 3, 1, 'TERRAIN_DESERT')));
  g.append(cell('yield layer (icon8 + Tally)', mini((ctx) => { [[2, 1, 0], [1, 2, 1], [3, 0, 2], [0, 4, 0], [1, 1, 5], [5, 3, 1]].forEach((yl, i) => UI.drawTileYields(art, ctx, (i % 3) * TILE * K, Math.floor(i / 3) * TILE * K, yl)); }, 3, 2, 'TERRAIN_PLAINS')));
  g.append(cell('range preview (ARC grid + ring)', mini((ctx) => { for (const [x, y] of [[1, 0], [0, 1], [1, 1], [2, 1]]) UI.drawRange(art, ctx, x * TILE * K, y * TILE * K, { mx: x, my: y }); }, 3, 2)));
  g = grid();
  for (const n of icons('ui/mark_')) g.append(cell(n, mini((ctx) => art.drawSprite(ctx, 4 * K, 4 * K, n), 1, 1, 'TERRAIN_SNOW')));
  for (const n of icons('ui/path_')) { const spr = art.sprite(n); g.append(cell(n, mini((ctx) => art.drawSprite(ctx, 16 * K, 16 * K, n), 1, 1, 'TERRAIN_PLAINS'))); }

  h2('cursors (hover each: the strip uses the CSS cursors)');
  g = grid();
  for (const n of icons('ui/cursor')) {
    const strip = el('div', 'strip on-coal ' + (n === 'ui/cursor' ? 'ui-cursor' : 'ui-' + n.slice(3).replace(/_/g, '-')));
    strip.append(UI.iconEl(art, n, O));
    g.append(cell(n, strip));
  }

  h2('fonts');
  g = grid();
  const lines = ['ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz', "0123456789 .,:;'-–()/+×!?&#", 'The Pale Procession', 'Turn 112 · 214 gold · 73%'];
  for (const scale of [1, 2, 3]) {
    const box = el('div', 'strip');
    box.style.flexDirection = 'column'; box.style.alignItems = 'flex-start';
    for (const l of lines) box.append(UI.textEl(art, l, { u: scale, dpr: DPR, color: '7', pad: 1 }));
    g.append(cell(`Vigil at ${scale}x`, box));
  }
  const tb = el('div', 'strip');
  tb.style.flexDirection = 'column'; tb.style.alignItems = 'flex-start';
  for (const scale of [1, 2, 3]) tb.append(UI.textEl(art, '0123456789 +-/x %.', { u: scale, dpr: DPR, font: 'tally', color: '7', pad: 1 }));
  g.append(cell('Tally at 1x, 2x, 3x', tb));
  g = grid();
  const book = ['abcdefghijklmnopqrstuvwxyz', 'ABCDEFGHIJKLMNOPQRSTUVWXYZ', "0123456789 .,:;'\"-–()/+×!?&#%·=[]<>*", 'Temple of Kilmorph  9 turns', 'Shock: +25% vs melee', 'Ljósálfar, Bannor, the Order'];
  for (const scale of [1, 2, 3]) {
    const box = el('div', 'strip');
    box.style.flexDirection = 'column'; box.style.alignItems = 'flex-start'; box.style.gap = `${3 * scale}px`;
    box.style.color = 'var(--c-bone)';
    for (const l of book) box.append(UI.textSpan(art, l, { u: scale, dpr: DPR }));
    g.append(cell(`Vigil Book (font 'vigil/book') at ${scale}x: the body face for short UI text`, box));
  }
  {
    const box = el('div', 'strip ui-frame-panel');
    box.style.width = `${150 * U}px`; box.style.flexDirection = 'column'; box.style.alignItems = 'flex-start'; box.style.color = 'var(--c-bone)';
    box.append(UI.textSpan(art, 'Elder Council', { ...O, cls: 'em' }),
      UI.textBlock(art, 'A gathering of the oldest voices of the city. +2 research, and the first step on the Way of the Wise.', 140, { ...O }),
      UI.textSpan(art, 'Requires: Writing', { ...O, cls: 'dim' }));
    g.append(cell('textBlock wrapped to 140 art px (colours by CSS: .em, .dim)', box));
  }
  for (const s of k.querySelectorAll('section')) if (!want(s.dataset.key)) s.remove();
}

/** Import a module of another set if it is there, and install it. */
async function optional(path, install) {
  try { install(await import(path)); } catch (e) { console.warn(`${path}: not installed (${e.message})`); }
}

async function main() {
  const art = await loadArt({ base: new URL('../', import.meta.url), lenient: true, passes: false });
  if (art.lib.errors.length) throw new Error(art.lib.errors.join('\n'));
  // the other sets' drawing modules, picked here (passes: false; loadArt installs all three by
  // default): the units set's stack overlay (count chip, health bar, marks, quiet rings); with
  // ?ground the terrain set's
  // ground pass and with ?citylight the cities set's hearth pools (both from their second round)
  await optional('../units/stack.js', (m) => m.installStackOverlay(art));
  if (q.has('ground')) await optional('../terrain/ground.js', (m) => m.installTerrain(art));
  if (q.has('citylight')) await optional('../cities/citylight.js', (m) => m.installCityLight(art));
  document.documentElement.style.setProperty('--u', `${U}px`);
  UI.installCSS(art, O);
  window.__art = art; window.__ui = UI; // for poking at the kit from the console or a test
  if (q.has('kit')) kit(art);
  else await screen(art);
}

try {
  await main();
  window.__artReady = true;
} catch (e) {
  console.error(e);
  document.getElementById('err').textContent = String(e.stack || e);
  window.__artReady = String(e);
}
