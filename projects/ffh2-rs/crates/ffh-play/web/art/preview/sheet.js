// sheet.js: contact sheets. ?set=terrain|units|cities|ui shows one set; ?set=all every set plus the
// coverage summary; ?set=coverage draws every game type through the mapping (fallbacks greyed).
// Sets window.__artReady = true when done (tools/screenshot.mjs waits for it).
import { loadArt, TILE } from '../art.js';
import { KINDS, NON_SPRITE_KINDS } from '../mapping.js';
import { makePlots, renderMap } from '../compose.js';

const q = new URLSearchParams(location.search);
const which = q.get('set') || 'all';
const out = document.getElementById('out');
const el = (tag, attrs = {}, ...kids) => {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  for (const k of kids) e.append(k);
  return e;
};

let art;
const TINCTURES = ['Argent', 'Gules', 'Azure', 'Or', 'Vert', 'Purpure', 'Tenne', 'Celeste', 'Sable', 'Cendree'];

function canvas(w, h, bg = '1') {
  const c = el('canvas', { width: w, height: h });
  const ctx = c.getContext('2d', { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  if (bg) { ctx.fillStyle = art.css(bg); ctx.fillRect(0, 0, w, h); }
  return [c, ctx];
}

function cell(label, sub, c, fallback = false) {
  const lbl = el('div', { class: 'lbl' });
  lbl.innerHTML = `<b>${label}</b>${sub ? '<br>' + sub : ''}`;
  return el('div', { class: 'cell' + (fallback ? ' fallback' : '') }, c, lbl);
}

/** A 1-tile plot for drawing in isolation. */
function tile(terrain, extra = {}) {
  const p = { x: extra.mx || 0, y: extra.my || 0, terrain, plotType: 'LAND', vis: 2, river: 0, ...extra };
  p.at = () => null;
  return p;
}

function groundTile(ctx, x, y, terrain = 'TERRAIN_GRASS', extra = {}) {
  art.drawTerrain(ctx, x, y, tile(terrain, extra));
}

function namespaceOf(name) {
  const parts = name.split('/');
  return parts.length > 2 ? parts.slice(0, -1).join('/') : parts[0];
}

// ---- per-namespace cell renderers -------------------------------------------------------------

function spriteCell(spr, scale, bg = '1', opts = {}) {
  const s = (art.scale = scale);
  const pad = opts.pad ?? 2;
  const [c, ctx] = canvas((spr.w + pad * 2) * s, (spr.h + pad * 2) * s, bg);
  art.drawSprite(ctx, (pad + spr.ax) * s, (pad + spr.ay) * s, spr.name, opts);
  return c;
}

function textureCell(spr) {
  const s = (art.scale = 2);
  const [c, ctx] = canvas(3 * TILE * s, 2 * TILE * s);
  const type = spr.meta.types[0] || 'TERRAIN_GRASS';
  for (let y = 0; y < 2; y++) for (let x = 0; x < 3; x++) art.drawTerrain(ctx, x * TILE * s, y * TILE * s, tile(type, { mx: x, my: y }));
  return c;
}

function onGround(spr, terrain, scale, place, opts = {}) {
  const s = (art.scale = scale);
  const top = Math.max(0, spr.ay - place[1]);
  const [c, ctx] = canvas(TILE * s, (TILE + top) * s);
  groundTile(ctx, 0, top * s, terrain);
  art.drawSprite(ctx, place[0] * s, (top + place[1]) * s, spr.name, opts);
  return c;
}

function unitCells(base, sprs) {
  const cells = [];
  const grounds = [['TERRAIN_GRASS', 'Argent'], ['TERRAIN_DESERT', 'Gules'], ['TERRAIN_OCEAN', 'Azure']];
  const spr = sprs[0];
  const s = (art.scale = 3);
  const huge = spr.w >= 32;
  const top = huge ? 12 : 0;
  // drawUnit resolves game types; the sheet draws the concrete sprite: moon rim, selected rim
  // (vellum), mirrored (facing west)
  const [c2, ctx2] = canvas(grounds.length * TILE * s, (TILE + top) * s);
  grounds.forEach(([t, tinc], i) => {
    groundTile(ctx2, i * TILE * s, top * s, t, { plotType: t === 'TERRAIN_OCEAN' ? 'OCEAN' : 'LAND' });
    const owner = art.ownerStyle(null, { tincture: tinc, id: tinc });
    const fx = huge ? 15 : 17, fy = huge ? 29 : 28;
    art._contactShadow(ctx2, (i * TILE + fx) * s, (top + fy) * s);
    art.drawSprite(ctx2, (i * TILE + fx) * s, (top + fy) * s, spr.name, { owner, rim: i === 1 ? 2 : 1, mirror: i === 2 });
  });
  const types = spr.meta.types.length ? spr.meta.types.join(' ') : '';
  cells.push(cell(base, `${spr.w}x${spr.h} @${spr.ax},${spr.ay}${types ? '<br>' + types : ''}`, c2));
  return cells;
}

function cityCell(spr) {
  const s = (art.scale = 3);
  const [c, ctx] = canvas(TILE * s, (TILE + 14 + 8) * s, '0');
  const m = /^city\/(.+)_(\d)$/.exec(spr.name);
  groundTile(ctx, 0, 14 * s, 'TERRAIN_PLAINS');
  const owner = art.ownerStyle(m && m[1] === 'infernal' ? 'CIVILIZATION_SHEAIM' : 'CIVILIZATION_BANNOR');
  const size = m ? [2, 5, 10, 16, 22][+m[2] - 1] : 3;
  art.drawCity(ctx, 0, 14 * s, { size, name: 'Vigil', capital: true, own: true, turns: 4, style: m ? m[1] : 'cloister' }, owner);
  return c;
}

function improvementCell(spr) {
  const s = (art.scale = 3);
  const top = 10;
  const [c, ctx] = canvas(TILE * s, (TILE + top) * s);
  groundTile(ctx, 0, top * s, 'TERRAIN_GRASS');
  const [px, py] = spr.meta.place || [16, 29];
  for (const l of spr.meta.lights) art.drawLight(ctx, (px - spr.ax + l.x) * s, (top + py - spr.ay + l.y) * s, l.table, l.radius);
  art.drawSprite(ctx, px * s, (top + py) * s, spr.name);
  return c;
}

function resourceCell(spr) {
  const s = (art.scale = 4);
  const [c, ctx] = canvas(15 * s, 15 * s, 'm');
  art.drawSprite(ctx, 7 * s, 13 * s, spr.name);
  return c;
}

function fontBlock(prefix) {
  const s = (art.scale = 3);
  const font = prefix.split('/')[1];
  const lines = font === 'tally' ? ['0123456789 +-/x'] : ['ABCDEFGHIJKLM', 'NOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz', "0123456789 .,:;'-()/+!?&#", 'Taranis of the Bannor'];
  const W = Math.max(...lines.map((l) => art.measureText(l, font))) + 6;
  const [c, ctx] = canvas(W * s, (lines.length * 10 + 4) * s);
  lines.forEach((l, i) => art.drawText(ctx, 3 * s, (9 + i * 10) * s, l, { font, color: i === 4 ? '8' : '7', shadow: true }));
  return cell(prefix, `${lines.join(' ').length} chars`, c);
}

function heraldryCells(spr) {
  const s = (art.scale = 4);
  const cols = TINCTURES.length;
  const [c, ctx] = canvas((cols * (spr.w + 3) + 3) * s, (5 * (spr.h + 3) + 3) * s, '2');
  ['plain', 'pale', 'fess', 'bend', 'quarterly'].forEach((division, j) => {
    TINCTURES.forEach((t, i) => {
      const owner = art.ownerStyle(null, { tincture: t, division, id: t + division });
      art.drawSprite(ctx, (3 + i * (spr.w + 3) + spr.ax) * s, (3 + j * (spr.h + 3) + spr.ay) * s, spr.name, { owner, flag: true });
    });
  });
  return cell(spr.name, 'ten tinctures x five divisions', c);
}

function sigilCell(spr) {
  const s = (art.scale = 4);
  const ban = art.pick('ui/banner');
  const [c, ctx] = canvas((ban.w + 4) * s, (ban.h + 4) * s, '2');
  const civ = [...art.lib.rules].find((r) => r.kind === 'civ' && r.sprite === spr.name);
  const owner = art.ownerStyle(civ ? civ.conds[0].values[0] : null);
  art.drawSprite(ctx, (2 + ban.ax) * s, (2 + ban.ay) * s, ban.name, { owner, flag: true });
  art.drawSprite(ctx, (3 + spr.ax) * s, (4 + spr.ay) * s, spr.name, { owner });
  return cell(spr.name, civ ? civ.conds[0].values.map((v) => v.replace('CIVILIZATION_', '')).join(' ') : '', c);
}

function renderSet(set) {
  const sec = el('section');
  sec.append(el('h2', {}, `${set}/`));
  const sprites = [...art.lib.sprites.values()].filter((s) => s.set === set);
  const files = art.lib.files.filter((f) => f.set === set).map((f) => `${f.file} (${f.count})`).join(', ');
  sec.append(el('div', { class: 'meta' }, `${sprites.length} sprites in ${files}`));
  const byNs = new Map();
  for (const s of sprites) {
    const ns = namespaceOf(s.name);
    if (!byNs.has(ns)) byNs.set(ns, []);
    byNs.get(ns).push(s);
  }
  for (const [ns, list] of byNs) {
    sec.append(el('h3', {}, ns));
    const grid = el('div', { class: 'grid' });
    sec.append(grid);
    if (ns.startsWith('font/')) { grid.append(fontBlock(ns)); continue; }
    const groups = new Map();
    for (const s of list) {
      const base = art.lib.groups.has(s.name) && art.lib.groups.get(s.name).single ? s.name : s.name.replace(/_(f\d+|[a-z])$/, '');
      if (!groups.has(base)) groups.set(base, []);
      groups.get(base).push(s);
    }
    for (const [base, sprs] of groups) {
      if (ns === 'terrain') { for (const s of sprs) grid.append(cell(s.name, `${s.w}x${s.h} ${s.meta.types.join(' ')}`, textureCell(s))); continue; }
      if (ns === 'unit') { grid.append(...unitCells(base, sprs)); continue; }
      for (const s of sprs) {
        let c;
        if (ns === 'hill' || ns === 'peak') c = onGround(s, ns === 'peak' && s.name.includes('snow') ? 'TERRAIN_SNOW' : 'TERRAIN_GRASS', 3, [16, 30]);
        else if (ns === 'feature') c = onGround(s, s.name.includes('flame') ? 'TERRAIN_BURNING_SANDS' : 'TERRAIN_GRASS', 4, [16, 26]);
        else if (ns === 'city') c = cityCell(s);
        else if (ns === 'improvement') c = improvementCell(s);
        else if (ns === 'res') c = resourceCell(s);
        else if (ns === 'route') c = spriteCell(s, 6, 'Q');
        else if (ns === 'ui' && (s.name === 'ui/stack_banner' || s.name === 'ui/banner')) { grid.append(heraldryCells(s)); continue; }
        else if (ns === 'sigil') { grid.append(sigilCell(s)); continue; }
        else c = spriteCell(s, ns === 'icon8' ? 5 : 4, '1');
        const sub = `${s.w}x${s.h}${s.meta.types.length ? ' ' + s.meta.types.join(' ') : ''}${s.meta.note ? '<br>' + s.meta.note : ''}`;
        grid.append(cell(s.name, sub, c));
      }
    }
  }
  if (set === 'terrain') sec.append(el('h3', {}, 'every terrain type through the mapping'), terrainTypes(), el('h3', {}, 'sampler (rendered with compose.js)'), sampler());
  return sec;
}

function terrainTypes() {
  const grid = el('div', { class: 'grid' });
  for (const t of art.mapper.typeNames('terrain')) {
    const r = art.mapper.resolve('terrain', t);
    const s = (art.scale = 2);
    const [c, ctx] = canvas(2 * TILE * s, TILE * s);
    for (let x = 0; x < 2; x++) art.drawTerrain(ctx, x * TILE * s, 0, tile(t, { mx: x, plotType: art.mapper.entry('terrain', t).water ? 'OCEAN' : 'LAND' }));
    grid.append(cell(t.replace('TERRAIN_', ''), r ? `${r.sprite}${r.opts.remap ? ' remap' : ''}` : 'unmapped', c, r && r.via === 'category'));
  }
  return grid;
}

/** A small map exercising coast, rivers, relief, features and roads. */
function sampler() {
  const T = { G: 'TERRAIN_GRASS', P: 'TERRAIN_PLAINS', D: 'TERRAIN_DESERT', T: 'TERRAIN_TUNDRA', S: 'TERRAIN_SNOW', M: 'TERRAIN_MARSH',
    C: 'TERRAIN_COAST', O: 'TERRAIN_OCEAN', B: 'TERRAIN_BROKEN_LANDS', F: 'TERRAIN_FIELDS_OF_PERDITION', U: 'TERRAIN_BURNING_SANDS', H: 'TERRAIN_SHALLOWS' };
  const rows = ['OCGGTSSTPPDD', 'OCGGGTTPPDDU', 'CCGMGGPPPDUU', 'CGGGGPPFFBUU', 'OCCGGPFFBBHH'];
  const relief = ['....^^^.....', '.....h......', '..........h.', '...h....h...', '............'];
  const feats = ['..ff.......', '...ff....', '.......', '....a.......', '.....jj....'];
  const tiles = [];
  rows.forEach((r, y) => [...r].forEach((ch, x) => {
    const water = ch === 'C' || ch === 'O';
    const rel = relief[y][x];
    const f = (feats[y] || '')[x];
    tiles.push({ terrain: T[ch], plotType: water ? 'OCEAN' : rel === '^' ? 'PEAK' : rel === 'h' ? 'HILLS' : 'LAND', vis: 2,
      feature: f === 'f' ? 'FEATURE_FOREST' : f === 'a' ? 'FEATURE_FOREST_ANCIENT' : f === 'j' ? 'FEATURE_JUNGLE' : null,
      river: 0, route: null });
  }));
  const map = { width: 12, height: 5, wrapX: false, tiles };
  const at = (x, y) => tiles[y * 12 + x];
  at(4, 1).river = 1; at(5, 1).river = 2; at(5, 2).river = 1; at(4, 2).river = 1; at(3, 2).river = 1; at(2, 2).river = 1;
  for (const [x, y] of [[6, 3], [7, 3], [8, 3], [9, 2], [10, 2]]) at(x, y).route = 'ROUTE_ROAD';
  const s = (art.scale = 2);
  const [c, ctx] = canvas(12 * TILE * s, 5 * TILE * s, '0');
  renderMap(art, ctx, map, { vignette: false });
  return c;
}

// ---- coverage ---------------------------------------------------------------------------------

function covGrid(kind) {
  const sec = el('section');
  const names = art.mapper.typeNames(kind);
  sec.append(el('h2', {}, `${kind} (${names.length})`));
  const grid = el('div', { class: 'grid tight' });
  sec.append(grid);
  const owner = art.ownerStyle('CIVILIZATION_BANNOR');
  for (const t of names) {
    const r = art.mapper.resolve(kind, t);
    const short = t.replace(/^[A-Z]+_/, '').toLowerCase();
    const via = r ? (r.sprite === 'none' ? 'none' : r.via) : 'MISSING';
    let c = null;
    const s = (art.scale = 2);
    if (kind === 'unit') {
      [c] = canvas(TILE * s, (TILE + 10) * s, '0');
      const ctx = c.getContext('2d');
      const e = art.mapper.entry('unit', t);
      groundTile(ctx, 0, 10 * s, e.domain === 'SEA' ? 'TERRAIN_COAST' : 'TERRAIN_GRASS', { plotType: e.domain === 'SEA' ? 'OCEAN' : 'LAND' });
      art.drawUnit(ctx, 0, 10 * s, t, e.combat === 'ANIMAL' ? null : owner, { hp: 100 });
    } else if (kind === 'improvement' || kind === 'feature' || kind === 'bonus') {
      [c] = canvas(TILE * s, (TILE + 12) * s, '0');
      const ctx = c.getContext('2d');
      const wet = kind === 'improvement' ? art.mapper.entry('improvement', t).water : t === 'FEATURE_ICE' || ['BONUS_FISH', 'BONUS_CLAM', 'BONUS_CRAB', 'BONUS_WHALE', 'BONUS_PEARL'].includes(t);
      const p = tile(wet ? 'TERRAIN_COAST' : t === 'FEATURE_FLAMES' || t === 'FEATURE_OBSIDIAN_PLAINS' ? 'TERRAIN_BURNING_SANDS' : 'TERRAIN_GRASS', { plotType: wet ? 'OCEAN' : 'LAND' });
      art.drawTerrain(ctx, 0, 12 * s, p);
      if (kind === 'improvement') art.drawImprovement(ctx, 0, 12 * s, t, p);
      else if (kind === 'feature') art.drawFeature(ctx, 0, 12 * s, p, t);
      else art.drawResource(ctx, 0, 12 * s, t);
    } else if (kind === 'terrain' || kind === 'hill' || kind === 'peak') {
      [c] = canvas(TILE * s, (TILE + 10) * s, '0');
      const ctx = c.getContext('2d');
      const water = art.mapper.entry('terrain', t).water;
      const p = tile(t, { plotType: water ? 'OCEAN' : kind === 'hill' ? 'HILLS' : kind === 'peak' ? 'PEAK' : 'LAND' });
      art.drawTerrain(ctx, 0, 10 * s, p);
      art.drawRelief(ctx, 0, 10 * s, p);
    } else if (kind === 'civ' || kind === 'citystyle') {
      [c] = canvas(TILE * s, (TILE + 14 + 8) * s, '0');
      const ctx = c.getContext('2d');
      groundTile(ctx, 0, 14 * s, 'TERRAIN_PLAINS');
      art.drawCity(ctx, 0, 14 * s, { size: 6, name: '', own: false, plaque: 'chip' }, art.ownerStyle(t));
    } else if (kind === 'route') {
      [c] = canvas(TILE * s, TILE * s, '0');
      const ctx = c.getContext('2d');
      groundTile(ctx, 0, 0, 'TERRAIN_PLAINS');
      const p = tile('TERRAIN_PLAINS', { route: t });
      art.drawRoad(ctx, 0, 0, p, [true, false, true, false, true, false, false, true]);
    } else if (r && r.sprite !== 'none') {
      const spr = art.pick(r.sprite);
      art.scale = spr.w <= 8 ? 4 : 2;
      c = spriteCell(spr, art.scale, '1', { remap: r.opts.remap, pad: 1 });
    }
    if (!c) [c] = canvas(16, 16, '0');
    grid.append(cell(short, via, c, !r || r.via === 'category' || r.via === 'base'));
  }
  return sec;
}

function coverageTable() {
  const cov = art.mapper.coverage(KINDS);
  const t = el('table', { class: 'cov' });
  t.innerHTML = '<tr><th>kind</th><th>types</th><th>rule</th><th>meta</th><th>name</th><th>base</th><th>category</th><th>none</th><th>missing</th></tr>';
  for (const [k, c] of Object.entries(cov)) {
    const tr = el('tr');
    tr.innerHTML = `<td>${k}</td><td>${c.total}</td><td>${c.rule}</td><td>${c.meta}</td><td>${c.name}</td><td>${c.base}</td><td>${c.category}</td><td>${c.none}</td><td>${c.missing.length}</td>`;
    t.append(tr);
  }
  return t;
}

async function main() {
  art = await loadArt({ base: new URL('../', import.meta.url), lenient: true });
  const lib = art.lib;
  document.title = `art: ${which}`;
  out.append(el('h1', {}, `Art toolkit: ${which}`));
  out.append(el('div', { class: 'meta' }, `${lib.sprites.size} sprites, ${lib.groups.size} groups, ${lib.rules.length} mapping rules, sets: ${lib.sets.join(' ')}`));
  if (lib.errors.length) {
    out.append(el('div', { class: 'errors' }, lib.errors.join('\n')));
    console.error(`art: ${lib.errors.length} source errors`);
  }
  if (which === 'coverage') {
    out.append(el('h2', {}, 'coverage'), coverageTable());
    for (const k of KINDS) if (!NON_SPRITE_KINDS.has(k) || k === 'citystyle') out.append(covGrid(k));
  } else {
    const sets = which === 'all' ? lib.sets : [which];
    if (which === 'all') out.append(el('h2', {}, 'coverage'), coverageTable());
    for (const s of sets) out.append(renderSet(s));
  }
}

try {
  await main();
  window.__artReady = true;
} catch (e) {
  console.error(e);
  out.append(el('div', { class: 'errors' }, String(e.stack || e)));
  window.__artReady = String(e);
}
