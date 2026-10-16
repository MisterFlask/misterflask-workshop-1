#!/usr/bin/env node
// validate.mjs: the art toolkit's test (docs/ART.md, "Toolkit"). Exit status 1 on any error.
//
//   node tools/validate.mjs [-v]
//
// Checks: palette.txt equals the guide's palette and remap blocks; every sprite parses, uses only
// palette keys and slots, has the size its namespace requires; every manifest rule targets an
// existing sprite and parses; types.json matches data/xml; every game type of every kind
// resolves to a sprite (or to `none`) through the mapping; loadArt installs the set passes (art.js
// PASSES). Warnings (colour counts, unknown metadata) do not fail the run. Also run by tools/check.sh.
import { readFile } from 'node:fs/promises';
import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { loadLibrary, splitBundle } from '../library.js';
import { makeBundle } from './bundle.mjs';
import { KINDS, NON_SPRITE_KINDS, KIND_SOURCE } from '../mapping.js';
import { parsePalette, TRANSPARENT, NUM_COLORS, REMAP_TABLES, SLOT_CHARS } from '../spritefmt.js';
import { ART_DIR, XML_DIR, buildTypes, formatTypes } from './gen-types.mjs';

const verbose = process.argv.includes('-v');
const errors = [];
const warnings = [];
const err = (m) => errors.push(m);
const warn = (m) => warnings.push(m);

const lib = await loadLibrary((p) => readFile(join(ART_DIR, p), 'utf8'));
errors.push(...lib.errors);
warnings.push(...lib.warnings);

// ---- palette vs the guide
const guide = resolve(ART_DIR, '../../../../docs/ART.md');
if (existsSync(guide)) {
  const md = readFileSync(guide, 'utf8');
  const block = (tag) => (new RegExp('```' + tag + '\\n([\\s\\S]*?)```').exec(md) || [])[1];
  const pal = block('palette'), rem = block('remap');
  if (!pal || !rem) err('docs/ART.md: palette or remap block not found');
  else {
    const g = parsePalette(`palette\n${pal}\nremap\n${rem}`, 'docs/ART.md');
    if (g.errors.length) err(`docs/ART.md blocks do not parse: ${g.errors[0]}`);
    g.colors.forEach((c, i) => {
      const m = lib.palette.colors[i];
      if (!m || m.key !== c.key || m.hex !== c.hex || m.name !== c.name) err(`palette.txt colour ${i} differs from docs/ART.md (${c.key} ${c.name} ${c.hex})`);
    });
    for (const t of Object.keys(g.remaps)) {
      if (!lib.palette.remaps[t]) { err(`palette.txt lacks remap table ${t}`); continue; }
      for (let i = 0; i < NUM_COLORS; i++) if (g.remaps[t][i] !== lib.palette.remaps[t][i]) err(`palette.txt remap ${t} differs from docs/ART.md at key ${g.colors[i].key}`);
    }
  }
} else warn('docs/ART.md not found; palette not compared with the guide');
for (const t of REMAP_TABLES) if (!lib.palette.remaps[t]) err(`palette.txt: missing remap table ${t}`);
if (Object.keys(lib.palette.tinctures).length !== 10) err(`palette.txt: expected 10 tinctures, got ${Object.keys(lib.palette.tinctures).length}`);

// ---- sprite sizes and colour budgets per namespace (docs/ART.md 3.3, 4.3)
const RULES = [
  [/^terrain\//, (s) => s.w === 64 && s.h === 64, '64x64 seamless texture', 5],
  [/^unit\//, (s) => (s.w === 24 && s.h === 24 && s.ax === 11 && s.ay === 21) || (s.w === 32 && s.h === 32 && s.ax === 15 && s.ay === 29)
    || (s.w === 40 && s.h === 40 && s.ax === 19 && s.ay === 37), '24x24 feet 11,21, 32x32 feet 15,29 or 40x40 feet 19,37', 20],
  [/^res\//, (s) => s.w === 13 && s.h === 13 && s.ax === 6 && s.ay === 12, '13x13 object, feet at 6,12', 8],
  [/^icon16\//, (s) => s.w === 16 && s.h === 16, '16x16', 8],
  [/^icon8\//, (s) => s.w === 8 && s.h === 8, '8x8', 6],
  [/^ui\/mark_/, (s) => s.w === 8 && s.h === 8, '8x8', 6],
  [/^sigil\//, (s) => (s.w === 5 && s.h === 5) || (s.w === 7 && s.h === 7), '5x5 or 7x7', 1],
  [/^font\/tally\//, (s) => s.h === 5 && s.w <= 3, '3x5', 1],
  [/^font\/vigil\/.*_sc$/, (s) => s.h === 5 && s.w <= 5, 'small capital, 5 tall', 1],
  [/^font\/vigil\//, (s) => s.h === 7 && s.w <= 6, 'capital or digit, 7 tall', 1],
  [/^city\/[a-z]+_[1-5]$/, (s) => s.w <= 34 && s.h <= 34 && s.ay >= s.h - 2, 'at most 34x34 with the base at the bottom', 10],
  [/^city\//, () => false, "named city/<style>_<tier 1-5>", 10],
  [/^improvement\//, (s) => s.w <= 34 && s.h <= 34, 'at most 34x34', 8],
  [/^feature\//, (s) => s.w <= 34 && s.h <= 40, 'at most 34x40', 8],
  // peaks: 5.3 itself names 4/5, 6, 3/2, 1 and a 7/f snow cap
  [/^(hill|peak)\//, (s) => s.w <= 34 && s.h <= 42, 'at most 34x42', 8],
  [/^route\//, (s) => s.w <= 4 && s.h <= 4, 'a dot of at most 4x4', 3],
  [/^ui\//, () => true, '', 8],
];
const keyOf = (i) => lib.palette.colors[i]?.key;
for (const s of lib.sprites.values()) {
  const rule = RULES.find(([re]) => re.test(s.name));
  if (!rule) { err(`${s.file}:${s.line}: ${s.name}: unknown namespace (terrain/ hill/ peak/ feature/ route/ unit/ city/ improvement/ res/ icon16/ icon8/ ui/ sigil/ font/)`); continue; }
  const [, ok, what, maxColours] = rule;
  if (!ok(s)) err(`${s.file}:${s.line}: ${s.name} is ${s.w}x${s.h} @${s.ax},${s.ay}; expected ${what}`);
  const used = new Set();
  let opaque = 0;
  for (const v of s.data) { if (v === TRANSPARENT) continue; opaque++; if (v < NUM_COLORS && v !== 0) used.add(keyOf(v)); }
  if (!opaque) err(`${s.file}:${s.line}: ${s.name} is empty`);
  if (used.size > maxColours) warn(`${s.file}:${s.line}: ${s.name} uses ${used.size} colours (${[...used].join('')}); the guide's limit is ${maxColours}`);
  for (const l of s.meta.lights) if (l.x < -8 || l.y < -8 || l.x > s.w + 8 || l.y > s.h + 8) err(`${s.file}:${s.line}: ${s.name} light at ${l.x},${l.y} is far outside the sprite`);
}
for (const g of lib.groups.values()) {
  if (g.frames.length && g.variants.length) err(`${g.base}: has both frames and variants`);
  const members = [g.single, ...g.variants, ...g.frames].filter(Boolean).map((n) => lib.sprites.get(n));
  const sizes = new Set(members.map((m) => `${m.w}x${m.h}`));
  if (g.frames.length && sizes.size > 1) err(`${g.base}: animation frames differ in size (${[...sizes].join(' ')})`);
}

// ---- manifest rules
const remapOk = (spec) => String(spec).split(',').every((p) => p.length === 2 && [...p].every((c) => lib.palette.keyIndex.has(c) || c in SLOT_CHARS));
const attrsOf = new Map();
for (const [k, list] of Object.entries(lib.types)) {
  const a = new Set(['type']);
  for (const e of list) for (const key of Object.keys(e)) a.add(key);
  attrsOf.set(k, a);
}
for (const r of lib.rules) {
  const where = `${r.file}:${r.line}`;
  if (!KINDS.includes(r.kind)) { err(`${where}: unknown kind '${r.kind}' (${KINDS.join(' ')})`); continue; }
  const src = KIND_SOURCE[r.kind] || r.kind;
  for (const c of r.conds) {
    const attr = c.attr.replace(/!$/, '');
    if (!attrsOf.get(src)?.has(attr)) err(`${where}: kind ${r.kind} has no attribute '${attr}'`);
    if (attr === 'type') for (const v of c.values) if (!lib.mapper.entry(r.kind, v)) err(`${where}: ${v} is not a ${src} type in types.json`);
  }
  if (NON_SPRITE_KINDS.has(r.kind)) {
    if (r.sprite !== 'none' && ![...lib.groups.keys()].some((n) => n.startsWith(`city/${r.sprite}_`))) {
      if (verbose) warn(`${where}: no city/${r.sprite}_<tier> sprites yet (falls back to cloister)`);
    }
  } else if (r.sprite !== 'none' && !lib.groups.has(r.sprite)) err(`${where}: target sprite '${r.sprite}' does not exist`);
  if (r.opts.remap && !remapOk(r.opts.remap)) err(`${where}: bad remap '${r.opts.remap}' (comma-separated pairs of keys or slots)`);
  if (r.opts.rim && !lib.palette.keyIndex.has(r.opts.rim)) err(`${where}: rim '${r.opts.rim}' is not a palette key`);
  if (r.opts.light && !lib.palette.keyIndex.has(r.opts.light)) err(`${where}: light '${r.opts.light}' is not a palette key`);
  if (r.opts.pool && r.opts.pool !== 'none' && !REMAP_TABLES.includes(String(r.opts.pool).split(':')[0])) err(`${where}: pool '${r.opts.pool}' is not a remap table`);
  if (r.opts.tincture && !lib.palette.tinctures[r.opts.tincture]) err(`${where}: unknown tincture '${r.opts.tincture}'`);
  if (r.opts.scatter && !/^\d+(-\d+)?$/.test(r.opts.scatter)) err(`${where}: scatter must be N or A-B`);
}
for (const s of lib.sprites.values()) for (const t of s.meta.types) {
  if (![...Object.values(lib.types)].some((list) => list.some((e) => e.type === t))) err(`${s.file}:${s.line}: ${s.name} claims unknown type ${t}`);
}

// ---- types.json vs the XML
if (existsSync(XML_DIR)) {
  const fresh = formatTypes(buildTypes(XML_DIR));
  if (fresh !== readFileSync(join(ART_DIR, 'types.json'), 'utf8')) err('types.json is stale: run node tools/gen-types.mjs');
} else warn(`data/xml not found at ${XML_DIR}; types.json not checked against it`);

// ---- the one-file bundle loads the same library
{
  const files = splitBundle(makeBundle());
  const b = await loadLibrary(async (p) => { if (!(p in files)) throw new Error(`bundle lacks ${p}`); return files[p]; });
  if (b.sprites.size !== lib.sprites.size || b.rules.length !== lib.rules.length || b.errors.length !== lib.errors.length) {
    err(`bundle round trip differs: ${b.sprites.size} sprites / ${b.rules.length} rules vs ${lib.sprites.size} / ${lib.rules.length}`);
  }
}

// ---- every game type resolves
const cov = lib.mapper.coverage(KINDS);
for (const [kind, c] of Object.entries(cov)) {
  for (const t of c.missing) err(`mapping: ${kind} ${t} resolves to no sprite`);
  for (const t of lib.mapper.typeNames(kind)) {
    const r = lib.mapper.resolve(kind, t);
    if (r && !NON_SPRITE_KINDS.has(kind) && r.sprite !== 'none' && !lib.groups.has(r.sprite)) err(`mapping: ${kind} ${t} -> missing sprite ${r.sprite}`);
  }
}

// ---- the set passes (art.js PASSES) load and install; loadArt({ passes: false }) leaves them out
try {
  const { loadArt, PASSES } = await import('../art.js');
  const read = (p) => readFile(join(ART_DIR, p), 'utf8');
  const wrapped = ['drawTerrain', 'drawUnit', 'drawCity', 'drawLight'];
  const own = (a) => wrapped.filter((k) => Object.hasOwn(a, k));
  for (const [path, fn] of PASSES) {
    const m = await import(new URL(path, new URL('../art.js', import.meta.url)));
    if (typeof m[fn] !== 'function') err(`art.js PASSES: ${path} exports no ${fn}`);
  }
  const seed = await loadArt({ read, lenient: true, passes: false });
  const full = await loadArt({ read, lenient: true });
  if (own(seed).length) err(`loadArt({ passes: false }) installed ${own(seed).join(' ')}`);
  if (own(full).length !== wrapped.length) err(`loadArt: set passes not installed (only ${own(full).join(' ') || 'none'})`);
} catch (e) { err(`art.js passes: ${e.stack || e}`); }

// ---- report
const pad = (s, n) => String(s).padStart(n);
console.log(`art: ${lib.sprites.size} sprites (${lib.groups.size} groups) in ${lib.files.length} files, ${lib.rules.length} rules, sets: ${lib.sets.join(' ')}`);
console.log('coverage    kind  types  exact  base  category  none');
for (const [kind, c] of Object.entries(cov)) {
  console.log(`${kind.padStart(16)} ${pad(c.total, 6)} ${pad(c.rule + c.meta + c.name, 6)} ${pad(c.base, 5)} ${pad(c.category, 9)} ${pad(c.none, 5)}`);
}
if (warnings.length) {
  console.log(`${warnings.length} warning(s)${verbose ? ':' : ' (-v to list)'}`);
  if (verbose) for (const w of warnings) console.log(`  warning: ${w}`);
}
if (errors.length) {
  console.log(`${errors.length} error(s):`);
  for (const e of errors.slice(0, 200)) console.log(`  error: ${e}`);
  process.exit(1);
}
console.log('art: ok');
