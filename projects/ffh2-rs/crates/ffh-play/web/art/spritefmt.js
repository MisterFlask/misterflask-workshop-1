// spritefmt.js: parsers for the art toolkit's text formats (docs/ART.md, "Toolkit").
// Pure ES module with no DOM use: the browser (art.js) and node (tools/*.mjs) both import it.
//
//   palette.txt          `palette`, `remap` and `tincture` sections
//   sets.txt             one set directory per line
//   <set>/manifest.txt   `set`, `source` and `map` lines
//   <set>/*.spr          sprites: `= name WxH ax,ay`, `: key value` metadata, then H rows of W chars

/** The palette's size, and the index values used inside a parsed sprite grid besides its colours. */
export const NUM_COLORS = 56;
export const TRANSPARENT = 255;
export const SLOT_FIELD = NUM_COLORS; // '#': owner tincture field
export const SLOT_SHADE = NUM_COLORS + 1; // '%': owner tincture shade
export const SLOT_CHARGE = NUM_COLORS + 2; // '*': owner tincture charge
export const SLOT_ACCENT = NUM_COLORS + 3; // '@': per-unit accent (religion or sphere colour)
export const SLOT_CHARS = { '#': SLOT_FIELD, '%': SLOT_SHADE, '*': SLOT_CHARGE, '@': SLOT_ACCENT };

/** Metadata keys the toolkit understands. Others are kept and reported as warnings by the validator. */
export const META_KEYS = new Set(['types', 'tags', 'light', 'place', 'flag', 'finial', 'gate', 'note', 'accent', 'flat']);

/** Pool tables that may be named in `light` metadata and mapping options. */
export const REMAP_TABLES = ['FOG', 'FIRE', 'BLOOD', 'RITE', 'ARC'];

function stripComment(line) {
  const t = line.replace(/\r$/, '');
  const i = t.indexOf(';');
  return (i >= 0 ? t.slice(0, i) : t).trim();
}

/**
 * Parse palette.txt.
 * Returns { colors: [{index,key,name,hex,rgb,family}], keyIndex: Map<char,int>,
 *           remaps: {FOG: Uint8Array(NUM_COLORS), ...}, tinctures: {name: {...}}, errors: [] }.
 */
export function parsePalette(text, file = 'palette.txt') {
  const colors = [];
  const keyIndex = new Map();
  const remapRows = [];
  let remapHeader = null;
  const tinctureLines = [];
  const errors = [];
  let section = null;
  const lines = text.split('\n');
  lines.forEach((raw, i) => {
    const line = stripComment(raw);
    if (!line) return;
    const f = line.split(/\s+/);
    if (f.length === 1 && ['palette', 'remap', 'tinctures'].includes(f[0])) { section = f[0]; return; }
    const where = `${file}:${i + 1}`;
    if (section === 'palette') {
      if (f.length !== 5) { errors.push(`${where}: palette line needs 'index key name #hex family'`); return; }
      const [idx, key, name, hex, family] = f;
      const index = Number(idx);
      if (!/^#[0-9a-fA-F]{6}$/.test(hex)) errors.push(`${where}: bad hex ${hex}`);
      if (key.length !== 1) errors.push(`${where}: key must be one character`);
      if (keyIndex.has(key)) errors.push(`${where}: duplicate key ${key}`);
      if (index !== colors.length) errors.push(`${where}: index ${index} out of order (expected ${colors.length})`);
      const rgb = [1, 3, 5].map((k) => parseInt(hex.slice(k, k + 2), 16));
      keyIndex.set(key, index);
      colors.push({ index, key, name, hex: hex.toLowerCase(), rgb, family });
    } else if (section === 'remap') {
      if (!remapHeader) { remapHeader = f; return; }
      remapRows.push({ f, where });
    } else if (section === 'tinctures') {
      tinctureLines.push({ f, where });
    } else {
      errors.push(`${where}: line outside a section`);
    }
  });
  if (colors.length !== NUM_COLORS) errors.push(`${file}: expected ${NUM_COLORS} colours, got ${colors.length}`);
  for (const ch of '.#%*@') if (keyIndex.has(ch)) errors.push(`${file}: '${ch}' is reserved and cannot be a palette key`);

  const remaps = {};
  if (!remapHeader || remapHeader[0] !== 'key') errors.push(`${file}: remap section needs a 'key T1 T2 ...' header`);
  else {
    const names = remapHeader.slice(1);
    for (const n of names) remaps[n] = new Uint8Array(NUM_COLORS).map((_, k) => k);
    const seen = new Set();
    for (const { f, where } of remapRows) {
      const from = keyIndex.get(f[0]);
      if (from === undefined) { errors.push(`${where}: unknown key ${f[0]}`); continue; }
      if (f.length !== names.length + 1) { errors.push(`${where}: expected ${names.length} targets`); continue; }
      seen.add(f[0]);
      names.forEach((n, k) => {
        const to = keyIndex.get(f[k + 1]);
        if (to === undefined) errors.push(`${where}: unknown target key ${f[k + 1]}`);
        else remaps[n][from] = to;
      });
    }
    for (const c of colors) if (!seen.has(c.key)) errors.push(`${file}: remap row missing for key ${c.key}`);
  }

  const tinctures = {};
  for (const { f, where } of tinctureLines) {
    // name field shade charge stitch stitch-under [tick]
    if (f.length < 6) { errors.push(`${where}: tincture needs 'name field shade charge stitch under [tick]'`); continue; }
    const ks = f.slice(1);
    const idx = ks.map((k) => keyIndex.get(k));
    if (idx.some((v) => v === undefined)) { errors.push(`${where}: unknown key in tincture ${f[0]}`); continue; }
    tinctures[f[0]] = {
      name: f[0], field: idx[0], shade: idx[1], charge: idx[2], stitch: idx[3], under: idx[4],
      tick: idx.length > 5 ? idx[5] : null,
    };
  }
  return { colors, keyIndex, remaps, tinctures, errors };
}

/** Parse sets.txt: one set directory per non-comment line. */
export function parseSetList(text) {
  return text.split('\n').map(stripComment).filter(Boolean);
}

/**
 * Parse a set manifest.
 *   set <name>
 *   source <file>
 *   map <kind> <cond> [<cond>...] -> <sprite|none> [key=value ...]
 * A cond is `*`, a type name (`UNIT_WARRIOR`, alternatives with `|`) or `attr=value` (`|` alternatives).
 */
export function parseManifest(text, file) {
  const out = { set: null, sources: [], rules: [], errors: [] };
  text.split('\n').forEach((raw, i) => {
    const line = stripComment(raw);
    if (!line) return;
    const where = `${file}:${i + 1}`;
    const f = line.split(/\s+/);
    if (f[0] === 'set') out.set = f[1];
    else if (f[0] === 'source') { if (f.length !== 2) out.errors.push(`${where}: source takes one file`); else out.sources.push(f[1]); }
    else if (f[0] === 'map') {
      const arrow = f.indexOf('->');
      if (arrow < 3 || arrow === f.length - 1) { out.errors.push(`${where}: expected 'map <kind> <cond...> -> <sprite> [opts]'`); return; }
      const kind = f[1];
      const conds = [];
      for (const c of f.slice(2, arrow)) {
        if (c === '*') continue;
        const eq = c.indexOf('=');
        if (eq > 0) conds.push({ attr: c.slice(0, eq), values: c.slice(eq + 1).split('|') });
        else if (/^[A-Z][A-Z0-9_|]*$/.test(c)) conds.push({ attr: 'type', values: c.split('|') });
        else out.errors.push(`${where}: bad condition '${c}'`);
      }
      const sprite = f[arrow + 1];
      const opts = {};
      for (const o of f.slice(arrow + 2)) {
        const eq = o.indexOf('=');
        if (eq > 0) opts[o.slice(0, eq)] = o.slice(eq + 1);
        else opts[o] = true;
      }
      out.rules.push({ kind, conds, sprite, opts, file, line: i + 1 });
    } else out.errors.push(`${where}: unknown directive '${f[0]}'`);
  });
  if (!out.set) out.errors.push(`${file}: missing 'set <name>'`);
  return out;
}

/**
 * Parse a sprite source file. `keyIndex` maps palette keys to indices.
 * Returns { sprites: [{name, w, h, ax, ay, data: Uint8Array, meta, file, line}], errors, warnings }.
 * meta: { types: [..], tags: [..], lights: [{x,y,table,radius}], place: [x,y], flag: [x,y,w,h],
 *         finial: [x,y], gate: [x,y], accent: key, flat: bool, note: str, other: {key: str} }
 */
export function parseSprites(text, file, keyIndex) {
  const sprites = [];
  const errors = [];
  const warnings = [];
  const lines = text.split('\n').map((l) => l.replace(/\r$/, ''));
  let i = 0;
  while (i < lines.length) {
    const raw = lines[i];
    const t = raw.trim();
    if (!t || t.startsWith(';')) { i++; continue; }
    if (!t.startsWith('=')) { errors.push(`${file}:${i + 1}: expected a '= name WxH ax,ay' header, got '${t.slice(0, 30)}'`); i++; continue; }
    const m = t.match(/^=\s*(\S+)\s+(\d+)x(\d+)\s+(-?\d+),(-?\d+)\s*$/);
    if (!m) { errors.push(`${file}:${i + 1}: bad header '${t}'`); i++; continue; }
    const name = m[1];
    const w = +m[2], h = +m[3], ax = +m[4], ay = +m[5];
    const headerLine = i + 1;
    i++;
    const meta = { types: [], tags: [], lights: [], place: null, flag: null, finial: null, gate: null, accent: null, flat: false, note: '', other: {} };
    while (i < lines.length && lines[i].trimStart().startsWith(':')) {
      const ml = lines[i].trim().slice(1).trim();
      const [key, ...rest] = ml.split(/\s+/);
      const where = `${file}:${i + 1}`;
      const pt = (s) => { const mm = /^(-?\d+),(-?\d+)$/.exec(s || ''); if (!mm) { errors.push(`${where}: expected x,y`); return null; } return [+mm[1], +mm[2]]; };
      switch (key) {
        case 'types': meta.types.push(...rest); break;
        case 'tags': meta.tags.push(...rest); break;
        case 'note': meta.note += (meta.note ? ' ' : '') + rest.join(' '); break;
        case 'flat': meta.flat = true; break;
        case 'place': meta.place = pt(rest[0]); break;
        case 'finial': meta.finial = pt(rest[0]); break;
        case 'gate': meta.gate = pt(rest[0]); break;
        case 'accent':
          if (!keyIndex.has(rest[0])) errors.push(`${where}: accent '${rest[0]}' is not a palette key`);
          meta.accent = rest[0]; break;
        case 'flag': {
          const mm = /^(\d+),(\d+),(\d+),(\d+)$/.exec(rest[0] || '');
          if (!mm) errors.push(`${where}: flag expects x,y,w,h`); else meta.flag = [+mm[1], +mm[2], +mm[3], +mm[4]];
          break;
        }
        case 'light': {
          // light x,y TABLE radius
          const p = pt(rest[0]);
          const table = rest[1];
          const radius = Number(rest[2]);
          if (!REMAP_TABLES.includes(table)) errors.push(`${where}: light table must be one of ${REMAP_TABLES.join(' ')}`);
          if (!(radius >= 0)) errors.push(`${where}: light radius must be a number`);
          if (p) meta.lights.push({ x: p[0], y: p[1], table, radius });
          break;
        }
        default:
          meta.other[key] = rest.join(' ');
          warnings.push(`${where}: unknown metadata key '${key}'`);
      }
      i++;
    }
    const data = new Uint8Array(w * h).fill(TRANSPARENT);
    let rows = 0;
    while (rows < h && i < lines.length) {
      const row = lines[i];
      const where = `${file}:${i + 1}`;
      if (row.trim().startsWith('=') || row.trim() === '') break;
      if (row.length !== w) errors.push(`${where}: ${name} row ${rows} has ${row.length} chars, expected ${w}`);
      for (let x = 0; x < Math.min(w, row.length); x++) {
        const ch = row[x];
        let v;
        if (ch === '.') v = TRANSPARENT;
        else if (SLOT_CHARS[ch] !== undefined) v = SLOT_CHARS[ch];
        else if (keyIndex.has(ch)) v = keyIndex.get(ch);
        else { errors.push(`${where}: ${name} col ${x}: '${ch}' is not a palette key or slot`); v = TRANSPARENT; }
        data[rows * w + x] = v;
      }
      rows++;
      i++;
    }
    if (rows !== h) errors.push(`${file}:${headerLine}: ${name} has ${rows} rows, expected ${h}`);
    if (ax < -16 || ay < -16 || ax > w + 16 || ay > h + 16) errors.push(`${file}:${headerLine}: ${name} anchor ${ax},${ay} far outside its ${w}x${h} box`);
    sprites.push({ name, w, h, ax, ay, data, meta, file, line: headerLine });
  }
  return { sprites, errors, warnings };
}

/** Split a sprite name into base, variant letter and frame number: `x/tree_b` -> {base:'x/tree', variant:'b'}. */
export function splitName(name) {
  let m = /^(.*)_f(\d+)$/.exec(name);
  if (m) return { base: m[1], variant: null, frame: +m[2] };
  m = /^(.*)_([a-z])$/.exec(name);
  if (m) return { base: m[1], variant: m[2], frame: null };
  return { base: name, variant: null, frame: null };
}

/** Serialize sprites back to text (used by the authoring tools). `keys[i]` is the palette key of index i. */
export function formatSprite(spr, keys) {
  const inv = { [SLOT_FIELD]: '#', [SLOT_SHADE]: '%', [SLOT_CHARGE]: '*', [SLOT_ACCENT]: '@', [TRANSPARENT]: '.' };
  const out = [`= ${spr.name} ${spr.w}x${spr.h} ${spr.ax},${spr.ay}`];
  const m = spr.meta || {};
  if (m.types && m.types.length) out.push(`: types ${m.types.join(' ')}`);
  if (m.tags && m.tags.length) out.push(`: tags ${m.tags.join(' ')}`);
  if (m.flat) out.push(': flat');
  if (m.place) out.push(`: place ${m.place.join(',')}`);
  if (m.flag) out.push(`: flag ${m.flag.join(',')}`);
  if (m.finial) out.push(`: finial ${m.finial.join(',')}`);
  if (m.gate) out.push(`: gate ${m.gate.join(',')}`);
  if (m.accent) out.push(`: accent ${m.accent}`);
  for (const l of m.lights || []) out.push(`: light ${l.x},${l.y} ${l.table} ${l.radius}`);
  if (m.note) out.push(`: note ${m.note}`);
  for (const [k, v] of Object.entries(m.other || {})) out.push(`: ${k} ${v}`);
  for (let y = 0; y < spr.h; y++) {
    let row = '';
    for (let x = 0; x < spr.w; x++) {
      const v = spr.data[y * spr.w + x];
      row += inv[v] !== undefined ? inv[v] : keys[v];
    }
    out.push(row);
  }
  return out.join('\n');
}
