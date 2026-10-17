// art.js: the ffh-play art toolkit (docs/ART.md, "Toolkit").
//
//   import { loadArt } from './art/art.js';
//   const art = await loadArt();          // fetches palette, sets, manifests, sprites, types.json
//                                         // and installs the set passes (PASSES)
//   art.scale = 2;                        // canvas pixels per art pixel (integer)
//   art.setWorld({ width, height, wrapX });
//   art.drawTerrain(ctx, px, py, plot);   // px, py: canvas position of the tile's top-left corner
//
// Every drawing function takes the canvas position of a tile (or of a sprite anchor where noted)
// in canvas units. Sprites are rasterised from their palette-indexed text into atlas pages at
// the current integer scale (nearest neighbour), recoloured by palette swap: owner slots
// (#, %, *), the accent slot (@), mapping remaps and the guide's remap tables (FOG, FIRE...).
// Fog and light pools are index remaps applied by reading the canvas back (every pixel the
// toolkit draws is a palette colour, so a colour maps back to its index exactly).
//
// A `plot` is { x, y, terrain, plotType, feature, bonus, improvement, route, river, owner, vis,
// city, at(dx, dy) } with type names (or indices, see setIndexNames), in screen orientation
// (row 0 = north); at() returns the neighbouring plot or null off the map. compose.js builds
// these from a map and draws a whole view in the guide's order.

import { TRANSPARENT, SLOT_FIELD, SLOT_SHADE, SLOT_CHARGE, SLOT_ACCENT, SLOT_CHARS, NUM_COLORS, splitName } from './spritefmt.js';
import { loadLibrary, splitBundle } from './library.js';

export const TILE = 32;
export const PLOT = { PEAK: 0, HILLS: 1, LAND: 2, OCEAN: 3 };
const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map((v) => (v + 0.5) / 16);
const bayer = (x, y) => BAYER[(y & 3) * 4 + (x & 3)];
const DIRS8 = [[0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1]];
const DIVISIONS = ['plain', 'pale', 'fess', 'bend', 'quarterly'];

/**
 * The sets' passes over the seed drawing below, installed by loadArt (docs/ART.md 16): each wraps
 * or replaces some Art methods and is idempotent. Paths are relative to this module.
 *   terrain/ground.js    the ground pass: drawTerrain, drawFog, drawRiver, drawRoad, drawRelief,
 *                        drawFeature
 *   units/stack.js       drawUnit's overlay: the stack standard, count, health, marks, reflections
 *   cities/citylight.js  drawLight's city pools (opts.inner) and drawCity's wall glow and lights
 */
export const PASSES = [
  ['./terrain/ground.js', 'installTerrain'],
  ['./units/stack.js', 'installStackOverlay'],
  ['./cities/citylight.js', 'installCityLight'],
];

/** Install the set passes on an Art (idempotent). loadArt calls it unless opts.passes === false. */
export async function installPasses(art) {
  for (const [path, fn] of PASSES) (await import(path))[fn](art);
  return art;
}

/**
 * Load the toolkit. opts: { base (URL of the art directory), bundle (text from tools/bundle.mjs),
 * read (async path => text), passes (default true: install the set passes, PASSES), lenient }.
 * The pass modules are always imported next to this module, whatever `base` or `bundle` say.
 */
export async function loadArt(opts = {}) {
  let read = opts.read;
  if (!read && opts.bundle) {
    const files = splitBundle(opts.bundle);
    read = async (p) => { if (!(p in files)) throw new Error(`art bundle: no ${p}`); return files[p]; };
  }
  if (!read) {
    const base = opts.base ? new URL(opts.base, globalThis.location?.href) : new URL('./', import.meta.url);
    read = async (p) => {
      const r = await fetch(new URL(p, base));
      if (!r.ok) throw new Error(`art: ${p}: HTTP ${r.status}`);
      return r.text();
    };
  }
  const lib = await loadLibrary(read);
  if (lib.errors.length && !opts.lenient) console.warn(`art: ${lib.errors.length} source errors\n` + lib.errors.slice(0, 20).join('\n'));
  const art = new Art(lib);
  if (opts.passes !== false) await installPasses(art);
  return art;
}

function makeCanvas(w, h) {
  if (typeof document !== 'undefined') {
    const c = document.createElement('canvas');
    c.width = w; c.height = h;
    return c;
  }
  return new OffscreenCanvas(w, h);
}

function hash3(x, y, z) {
  let h = Math.imul(x | 0, 374761393) ^ Math.imul(y | 0, 668265263) ^ Math.imul(z | 0, 2246822519);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return h >>> 0;
}
const rnd3 = (x, y, z) => hash3(x, y, z) / 4294967296;

function tierOf(size) { return size <= 3 ? 1 : size <= 7 ? 2 : size <= 12 ? 3 : size <= 19 ? 4 : 5; }

export class Art {
  constructor(lib) {
    this.lib = lib;
    this.palette = lib.palette;
    this.mapper = lib.mapper;
    this.scale = 2;
    this.time = 0;
    this.world = { width: 0, height: 0, wrapX: false };
    this.indexNames = {};
    const P = this.palette.colors;
    this.rgb = new Uint8Array(NUM_COLORS * 3);
    this.colorIndex = new Map();
    P.forEach((c, i) => {
      this.rgb.set(c.rgb, i * 3);
      this.colorIndex.set((c.rgb[0] << 16) | (c.rgb[1] << 8) | c.rgb[2], i);
    });
    this.keys = P.map((c) => c.key);
    this.tables = { ...this.palette.remaps };
    this.tinctures = this.palette.tinctures;
    this._atlas = new Map();
    this._entries = new Map();
    this._tex = new Map();
    this._ground = new Map();
    this._scratch = null;
    this._remapCache = new Map();
  }

  // ---------------------------------------------------------------- basics

  /** Palette index of a key ('0'..'8', 'n', 'L', ...). */
  K(key) {
    const v = this.palette.keyIndex.get(key);
    if (v === undefined) throw new Error(`art: unknown palette key ${key}`);
    return v;
  }
  /** CSS colour of a palette key. */
  css(key) { return this.palette.colors[this.K(key)].hex; }

  setWorld({ width, height, wrapX }) {
    const w = { width: width | 0, height: height | 0, wrapX: !!wrapX };
    // renderMap sets the world on every call (a client draws its view in chunks): the ground
    // cache is kept while the world stays the same.
    if (w.width === this.world.width && w.height === this.world.height && w.wrapX === this.world.wrapX) return;
    this.world = w;
    this._ground.clear();
  }

  /** Let plots carry type indices: names[kind] is the engine's name list (from /api/data). */
  setIndexNames(kind, names) { this.indexNames[kind] = names; }
  typeName(kind, v) {
    if (v === null || v === undefined || v === -1 || v === '') return null;
    if (typeof v === 'number') return this.indexNames[kind]?.[v] ?? this.mapper.typeNames(kind)[v] ?? null;
    return v;
  }

  /** Resolve a game type: {sprite, opts, via} (see mapping.js). */
  resolve(kind, type) {
    const t = this.typeName(kind, type);
    return t ? this.mapper.resolve(kind, t) : null;
  }

  sprite(name) { return this.lib.sprites.get(name) || null; }
  hasSprite(name) { return this.lib.groups.has(name); }

  /** Pick a concrete sprite of a group: variant by `seed`, animation frame by this.time. */
  pick(base, seed = 0, period = 400) {
    const g = this.lib.groups.get(base);
    if (!g) return this.lib.sprites.get(base) || null;
    if (g.single) return this.lib.sprites.get(g.single);
    if (g.frames.length) return this.lib.sprites.get(g.frames[Math.floor(this.time / period) % g.frames.length]);
    return this.lib.sprites.get(g.variants[(seed >>> 0) % g.variants.length]);
  }

  /** Variants of a group (sprites), in name order. */
  variants(base) {
    const g = this.lib.groups.get(base);
    if (!g) return [];
    return (g.single ? [g.single] : g.variants.length ? g.variants : g.frames).map((n) => this.lib.sprites.get(n));
  }

  /** Remap from a mapping option string 'from,to' pairs: 't2,k1' (keys or slot chars). */
  remapOf(spec) {
    if (!spec) return null;
    if (this._remapCache.has(spec)) return this._remapCache.get(spec);
    const m = new Uint8Array(256).map((_, i) => i);
    const code = (ch) => (SLOT_CHARS[ch] !== undefined ? SLOT_CHARS[ch] : ch === '.' ? TRANSPARENT : this.K(ch));
    for (const pair of String(spec).split(',')) {
      if (pair.length !== 2) throw new Error(`art: bad remap pair '${pair}' in '${spec}'`);
      m[code(pair[0])] = code(pair[1]);
    }
    this._remapCache.set(spec, m);
    return m;
  }

  // ---------------------------------------------------------------- owners

  /**
   * Owner style for a civilization: { civ, tincture, division, second, sigil, cityStyle, pool, light,
   * barbarian, animal, hidden }. opts may override tincture / division / sigil.
   */
  ownerStyle(civ, opts = {}) {
    const r = civ ? this.resolve('civ', civ) : null;
    const cs = civ ? this.resolve('citystyle', civ) : null;
    const o = { ...(cs?.opts || {}), style: cs && cs.sprite !== 'none' ? cs.sprite : undefined, ...(r?.opts || {}) };
    const barbarian = civ === 'CIVILIZATION_BARBARIAN' || !!opts.barbarian;
    const tName = opts.tincture || o.tincture || (barbarian ? 'Sable' : 'Argent');
    const tincture = this.tinctures[tName] || this.tinctures.Argent;
    const division = opts.division || 'plain';
    const second = this.tinctures[tName === 'Sable' ? 'Argent' : 'Sable'];
    return {
      id: opts.id ?? civ ?? 'none', civ, tinctureName: tName, tincture, division, second,
      sigil: opts.sigil || (r && r.sprite !== 'none' ? r.sprite : null),
      cityStyle: opts.cityStyle || o.style || 'cloister', pool: opts.pool || o.pool || 'FIRE', light: opts.light || o.light || 'l',
      barbarian, animal: !!opts.animal, hidden: !!opts.hidden,
    };
  }

  /**
   * Assign heraldry to players in slot order (docs/ART.md 7.2): each takes its civilization's
   * tincture plain, or the next free division of it. players: [{id, civ}] -> Map(id -> style).
   */
  assignOwners(players) {
    const taken = new Set();
    const out = new Map();
    for (const p of players) {
      if (p.civ === 'CIVILIZATION_BARBARIAN') { out.set(p.id, this.ownerStyle(p.civ, { id: p.id })); continue; }
      const base = this.ownerStyle(p.civ, { id: p.id });
      let division = DIVISIONS.find((d) => !taken.has(`${base.tinctureName}/${d}`)) || 'plain';
      taken.add(`${base.tinctureName}/${division}`);
      out.set(p.id, this.ownerStyle(p.civ, { id: p.id, division }));
    }
    return out;
  }

  _slotFill(style, flagRect, accent) {
    const t = style ? (style.hidden ? this.tinctures.Sable : style.tincture) : this.tinctures.Argent;
    const s2 = style && !style.hidden ? style.second : null;
    const div = style && !style.hidden && flagRect ? style.division : 'plain';
    const acc = accent ?? this.K('P');
    return (x, y, slot) => {
      if (slot === SLOT_ACCENT) return acc;
      let tt = t;
      if (div !== 'plain' && s2 && slot !== SLOT_CHARGE) {
        const [fx, fy, fw, fh] = flagRect;
        const u = (x - fx + 0.5) / fw, v = (y - fy + 0.5) / fh;
        const second = div === 'pale' ? u >= 0.5 : div === 'fess' ? v >= 0.5 : div === 'bend' ? v > u : (u >= 0.5) !== (v >= 0.5);
        if (second) tt = s2;
      }
      return slot === SLOT_FIELD ? tt.field : slot === SLOT_SHADE ? tt.shade : tt.charge;
    };
  }

  // ---------------------------------------------------------------- sprite variants and atlas

  /**
   * Build a sprite variant's index grid. v: { remap (string), owner (style), flag (bool: apply the
   * division), accent (key), table ('FOG'|...), silhouette (key), rim (0 none, 1 moon, 2 selected),
   * checker (bool: 50% translucency) }. Returns { w, h, ax, ay, data }.
   */
  variantData(spr, v = {}) {
    let w = spr.w, h = spr.h, ax = spr.ax, ay = spr.ay;
    const src = spr.data;
    const remap = v.remap ? this.remapOf(v.remap) : null;
    const accentKey = v.accent || spr.meta.accent;
    const fill = this._slotFill(v.owner || null, v.flag ? spr.meta.flag || [0, 0, w, h] : null, accentKey ? this.K(accentKey) : undefined);
    const table = v.table ? this.tables[v.table] : null;
    const sil = v.silhouette ? this.K(v.silhouette) : null;
    let data = new Uint8Array(w * h);
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      let c = src[y * w + x];
      if (remap) c = remap[c];
      if (c === TRANSPARENT) { data[y * w + x] = c; continue; }
      if (c >= NUM_COLORS) c = fill(x, y, c);
      if (table) c = table[c];
      if (sil !== null && c !== 0) c = sil;
      if (v.checker && ((x + y) & 1)) c = TRANSPARENT;
      data[y * w + x] = c;
    }
    if (v.rim) {
      const W = w + 2, H = h + 2;
      const out = new Uint8Array(W * H).fill(TRANSPARENT);
      for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) out[(y + 1) * W + x + 1] = data[y * w + x];
      const op = (x, y) => x >= 0 && y >= 0 && x < W && y < H && out[y * W + x] !== TRANSPARENT && out[y * W + x] !== 254;
      const rimC = v.rim === 2 ? this.K('8') : this.K('5');
      const add = [];
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        if (out[y * W + x] !== TRANSPARENT) continue;
        const hit = v.rim === 2 ? op(x + 1, y) || op(x - 1, y) || op(x, y + 1) || op(x, y - 1) : op(x + 1, y) || op(x, y + 1);
        if (hit) add.push(y * W + x);
      }
      for (const p of add) out[p] = rimC;
      data = out; w = W; h = H; ax += 1; ay += 1;
    }
    return { w, h, ax, ay, data };
  }

  _page(scale, w, h) {
    let A = this._atlas.get(scale);
    if (!A) { A = { pages: [], x: 0, y: 0, rowH: 0 }; this._atlas.set(scale, A); }
    const P = Math.max(1024, w, h);
    if (!A.pages.length || A.x + w > A.size) { A.y += A.rowH; A.x = 0; A.rowH = 0; }
    if (!A.pages.length || A.y + h > A.size) {
      const c = makeCanvas(P, P);
      A.pages.push({ canvas: c, ctx: c.getContext('2d') });
      A.size = P; A.x = 0; A.y = 0; A.rowH = 0;
    }
    const page = A.pages[A.pages.length - 1];
    const at = { page, x: A.x, y: A.y };
    A.x += w + 1;
    A.rowH = Math.max(A.rowH, h + 1);
    return at;
  }

  _imageData(data, w, h, s) {
    const img = new ImageData(w * s, h * s);
    const d = img.data;
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const c = data[y * w + x];
      if (c === TRANSPARENT || c >= NUM_COLORS) continue;
      const r = this.rgb[c * 3], g = this.rgb[c * 3 + 1], b = this.rgb[c * 3 + 2];
      for (let j = 0; j < s; j++) {
        let p = ((y * s + j) * w * s + x * s) * 4;
        for (let i = 0; i < s; i++, p += 4) { d[p] = r; d[p + 1] = g; d[p + 2] = b; d[p + 3] = 255; }
      }
    }
    return img;
  }

  /** Atlas entry for a sprite variant at the current scale: { canvas, sx, sy, w, h, ax, ay } (art px). */
  entry(name, v = {}) {
    const s = this.scale;
    const key = `${name}|${v.remap || ''}|${v.owner ? v.owner.tinctureName + '/' + v.owner.division + (v.owner.hidden ? 'h' : '') : ''}|${v.flag ? 1 : 0}|${v.accent || ''}|${v.table || ''}|${v.silhouette || ''}|${v.rim || 0}|${v.checker ? 1 : 0}|${s}`;
    let e = this._entries.get(key);
    if (e) return e;
    const spr = this.lib.sprites.get(name);
    if (!spr) return null;
    const vd = this.variantData(spr, v);
    const at = this._page(s, vd.w * s, vd.h * s);
    at.page.ctx.putImageData(this._imageData(vd.data, vd.w, vd.h, s), at.x, at.y);
    e = { canvas: at.page.canvas, sx: at.x, sy: at.y, w: vd.w, h: vd.h, ax: vd.ax, ay: vd.ay, spr };
    this._entries.set(key, e);
    return e;
  }

  /** Draw a sprite (by concrete name) with its anchor at canvas point (x, y). v: variant options + mirror. */
  drawSprite(ctx, x, y, name, v = {}) {
    const e = this.entry(name, v);
    if (!e) return null;
    this._drawEntry(ctx, e, x, y, v.mirror);
    return e;
  }

  _drawEntry(ctx, e, x, y, mirror) {
    const s = this.scale;
    if (!mirror) {
      ctx.drawImage(e.canvas, e.sx, e.sy, e.w * s, e.h * s, x - e.ax * s, y - e.ay * s, e.w * s, e.h * s);
    } else {
      ctx.save();
      ctx.translate(x - (e.w - 1 - e.ax) * s + e.w * s, y - e.ay * s);
      ctx.scale(-1, 1);
      ctx.drawImage(e.canvas, e.sx, e.sy, e.w * s, e.h * s, 0, 0, e.w * s, e.h * s);
      ctx.restore();
    }
  }

  /** Blit an index buffer (w x h art px, TRANSPARENT skipped) with its top-left at canvas (x, y). */
  blit(ctx, x, y, data, w, h) {
    const s = this.scale;
    if (!this._scratch || this._scratch.width < w * s || this._scratch.height < h * s) {
      this._scratch = makeCanvas(Math.max(w * s, this._scratch?.width || 0), Math.max(h * s, this._scratch?.height || 0));
      this._scratchCtx = this._scratch.getContext('2d');
    }
    this._scratchCtx.clearRect(0, 0, w * s, h * s);
    this._scratchCtx.putImageData(this._imageData(data, w, h, s), 0, 0);
    ctx.drawImage(this._scratch, 0, 0, w * s, h * s, x, y, w * s, h * s);
  }

  // ---------------------------------------------------------------- canvas read-back (remaps)

  _nearest(rgb) {
    let v = this.colorIndex.get(rgb);
    if (v !== undefined) return v;
    const r = rgb >> 16, g = (rgb >> 8) & 255, b = rgb & 255;
    let best = 0, bd = Infinity;
    for (let i = 0; i < NUM_COLORS; i++) {
      const d = (this.rgb[i * 3] - r) ** 2 + (this.rgb[i * 3 + 1] - g) ** 2 + (this.rgb[i * 3 + 2] - b) ** 2;
      if (d < bd) { bd = d; best = i; }
    }
    if (this.colorIndex.size < 65536) this.colorIndex.set(rgb, best);
    return best;
  }

  /**
   * Remap a canvas region in index space. (x, y): canvas position, (w, h): size in art px.
   * fn(ax, ay, index) returns the new index (ax, ay: art px relative to the region).
   * Works through the context's transform when it is a translation and/or integer scale.
   */
  remapRegion(ctx, x, y, w, h, fn) {
    const T = ctx.getTransform ? ctx.getTransform() : { a: 1, d: 1, e: 0, f: 0 };
    const b = Math.max(1, Math.round(this.scale * T.a));
    let dx = Math.round(x * T.a + T.e), dy = Math.round(y * T.d + T.f);
    const cw = ctx.canvas.width, ch = ctx.canvas.height;
    let i0 = 0, j0 = 0, i1 = w, j1 = h;
    if (dx < 0) i0 = Math.ceil(-dx / b);
    if (dy < 0) j0 = Math.ceil(-dy / b);
    i1 = Math.min(w, Math.floor((cw - dx) / b));
    j1 = Math.min(h, Math.floor((ch - dy) / b));
    if (i1 <= i0 || j1 <= j0) return;
    const X = dx + i0 * b, Y = dy + j0 * b, W = (i1 - i0) * b, H = (j1 - j0) * b;
    const img = ctx.getImageData(X, Y, W, H);
    const d = img.data;
    let changed = false;
    for (let j = j0; j < j1; j++) for (let i = i0; i < i1; i++) {
      const p0 = ((j - j0) * b * W + (i - i0) * b) * 4;
      if (d[p0 + 3] === 0) continue;
      const c = this._nearest((d[p0] << 16) | (d[p0 + 1] << 8) | d[p0 + 2]);
      const n = fn(i, j, c);
      if (n === c || n === undefined || n === null) continue;
      changed = true;
      const r = this.rgb[n * 3], g = this.rgb[n * 3 + 1], bb = this.rgb[n * 3 + 2];
      for (let yy = 0; yy < b; yy++) {
        let p = ((j - j0) * b * W + yy * W + (i - i0) * b) * 4;
        for (let xx = 0; xx < b; xx++, p += 4) { d[p] = r; d[p + 1] = g; d[p + 2] = bb; d[p + 3] = 255; }
      }
    }
    if (changed) ctx.putImageData(img, X, Y);
  }

  /**
   * Light pool (docs/ART.md 4.4): an elliptical index remap toward a table, with Bayer falloff.
   * (cx, cy): canvas position of the source's ground point; radius in art px; opts.inner: also
   * apply a second pass at half radius (cities).
   */
  drawLight(ctx, cx, cy, table, radius, opts = {}) {
    const T = this.tables[table];
    if (!T || radius <= 0) return;
    const s = this.scale;
    const rx = radius, ry = radius / 1.4;
    const passes = opts.inner ? [rx, rx / 2] : [rx];
    const x0 = cx - Math.ceil(rx) * s, y0 = cy - Math.ceil(ry) * s;
    const W = Math.ceil(rx) * 2 + 1, H = Math.ceil(ry) * 2 + 1;
    const ox = Math.round(cx / s), oy = Math.round(cy / s);
    this.remapRegion(ctx, x0, y0, W, H, (i, j, c) => {
      const dx = i - Math.ceil(rx), dy = j - Math.ceil(ry);
      let out = c;
      for (const r of passes) {
        const d = Math.sqrt((dx / r) ** 2 + ((dy * 1.4) / r) ** 2);
        if (d < 1 && 1 - d > bayer(ox + dx, oy + dy) * 0.999) out = T[out];
      }
      return out;
    });
  }

  // ---------------------------------------------------------------- terrain

  /** Value noise in [-1, 1] at map art pixel (X, Y); the lattice wraps every `wpx` px in x when set. */
  _noise(X, Y, cell, seed, wpx) {
    let c = cell, px = 0;
    if (wpx) { px = Math.max(1, Math.round(wpx / cell)); c = wpx / px; }
    const fx = X / c, fy = Y / c;
    let ix = Math.floor(fx);
    const iy = Math.floor(fy);
    const tx = fx - ix, ty = fy - iy;
    const sx = tx * tx * (3 - 2 * tx), sy = ty * ty * (3 - 2 * ty);
    let ix1 = ix + 1;
    if (px) { ix = ((ix % px) + px) % px; ix1 = ((ix1 % px) + px) % px; }
    const a = rnd3(ix, iy, seed), b = rnd3(ix1, iy, seed), cc = rnd3(ix, iy + 1, seed), d = rnd3(ix1, iy + 1, seed);
    return (a + (b - a) * sx + (cc - a) * sy + (a - b - cc + d) * sx * sy) * 2 - 1;
  }

  _warpRaw(X, Y, seed) {
    const wpx = this.world.wrapX && this.world.width ? this.world.width * TILE : 0;
    let wx = 6.5 * this._noise(X, Y, 22, seed + 1, wpx) + 1.5 * this._noise(X, Y, 7, seed + 2, wpx);
    let wy = 6.5 * this._noise(X, Y, 22, seed + 3, wpx) + 1.5 * this._noise(X, Y, 7, seed + 4, wpx);
    wx = Math.max(-8, Math.min(8, wx));
    wy = Math.max(-8, Math.min(8, wy));
    return [wx, wy];
  }

  /** The terrain domain warp at map art pixel (X, Y) (docs/ART.md 5.1): [wx, wy], |w| <= 8. */
  warp(X, Y) { return this._warpRaw(X, Y, 100); }

  _terrain(type) {
    const t = this.typeName('terrain', type) || 'TERRAIN_GRASS';
    let info = this._tex.get(t);
    if (info) return info;
    const r = this.mapper.resolve('terrain', t);
    const spr = r && r.sprite !== 'none' ? this.pick(r.sprite, 0) : null;
    const entry = this.mapper.entry('terrain', t);
    let tex;
    if (spr) {
      const vd = this.variantData(spr, { remap: r.opts.remap });
      tex = { w: vd.w, h: vd.h, data: vd.data };
    } else tex = { w: 1, h: 1, data: new Uint8Array([this.K(entry?.water ? 'n' : '4')]) };
    info = { type: t, tex, water: !!entry?.water, deep: !!r?.opts?.deep };
    this._tex.set(t, info);
    return info;
  }

  _plotType(p) {
    const v = p.plotType;
    if (typeof v === 'number') return v;
    if (!v) return this._terrain(p.terrain).water ? PLOT.OCEAN : PLOT.LAND;
    const s = String(v).replace('PLOT_', '');
    return PLOT[s] ?? PLOT.LAND;
  }

  _isWater(p) { return this._plotType(p) === PLOT.OCEAN || this._terrain(p.terrain).water; }

  /** Per-pixel source cell (0..8 of the 3x3 neighbourhood) for a padded tile region. */
  _cells(plot, pad, seed = 100, amp = 1) {
    const N = TILE + 2 * pad;
    const src = new Uint8Array(N * N);
    for (let j = 0; j < N; j++) for (let i = 0; i < N; i++) {
      const X = plot.x * TILE + i - pad, Y = plot.y * TILE + j - pad;
      let [wx, wy] = this._warpRaw(X, Y, seed);
      wx *= amp; wy *= amp;
      const cx = Math.max(-1, Math.min(1, Math.floor((i - pad + wx) / TILE)));
      const cy = Math.max(-1, Math.min(1, Math.floor((j - pad + wy) / TILE)));
      src[j * N + i] = (cy + 1) * 3 + (cx + 1);
    }
    return src;
  }

  /** The 3x3 neighbourhood (row-major, centre at 4). Off-map neighbours, and unrevealed ones when
   *  `revealedOnly` (their terrain is unknown), stand in as the centre plot. */
  _nbhd(plot, revealedOnly = false) {
    const out = [];
    for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
      const n = (dx || dy) ? plot.at?.(dx, dy) : plot;
      out.push(!n || (revealedOnly && n.vis === 0) ? plot : n);
    }
    return out;
  }

  /** Ground indices of a tile (32x32): texture, shoreline ink, foam, depth contour. Cached. */
  groundIndices(plot) {
    const nb = this._nbhd(plot, true);
    const infos = nb.map((p) => this._terrain(p.terrain));
    const water = nb.map((p) => this._isWater(p));
    const key = `${plot.x},${plot.y}|` + nb.map((p, k) => infos[k].type + (water[k] ? 'w' : '')).join(',');
    let out = this._ground.get(key);
    if (out) return out;
    if (this._ground.size > 8192) this._ground.clear();
    // fast path: one terrain all around, so the warp changes nothing
    if (infos.every((t, k) => t === infos[4] && water[k] === water[4])) {
      const t = infos[4].tex;
      out = new Uint8Array(TILE * TILE);
      for (let j = 0; j < TILE; j++) {
        const Y = plot.y * TILE + j, row = (((Y % t.h) + t.h) % t.h) * t.w;
        for (let i = 0; i < TILE; i++) { const X = plot.x * TILE + i; out[j * TILE + i] = t.data[row + (((X % t.w) + t.w) % t.w)]; }
      }
      this._ground.set(key, out);
      return out;
    }
    const shore = water.some((w) => w !== water[4]) || water.some((w) => w);
    const PAD = 3, N = TILE + 2 * PAD;
    const src = this._cells(plot, PAD);
    const cls = new Uint8Array(N * N); // 0 land, 1 shallow water, 2 deep water
    for (let k = 0; k < N * N; k++) { const c = src[k]; cls[k] = water[c] ? (infos[c].deep ? 2 : 1) : 0; }
    const C = (i, j) => cls[(j + PAD) * N + i + PAD];
    out = new Uint8Array(TILE * TILE);
    const k7 = this.K('7'), k6 = this.K('6'), k2 = this.K('2'), kN = this.K('N'), kg = this.K('g');
    for (let j = 0; j < TILE; j++) for (let i = 0; i < TILE; i++) {
      const X = plot.x * TILE + i, Y = plot.y * TILE + j;
      const c = src[(j + PAD) * N + i + PAD];
      const t = infos[c].tex;
      let v = t.data[(((Y % t.h) + t.h) % t.h) * t.w + (((X % t.w) + t.w) % t.w)];
      const me = C(i, j);
      if (!shore) { /* no water near: texture only */ } else if (me === 0) {
        if (C(i - 1, j) || C(i + 1, j) || C(i, j - 1) || C(i, j + 1)) v = (X + Y) % 5 === 0 ? k6 : k7;
        else {
          let near = false;
          for (let dj = -2; dj <= 2 && !near; dj++) for (let di = -2; di <= 2; di++) if (C(i + di, j + dj)) { near = true; break; }
          if (near) v = k2;
        }
      } else {
        let land = false;
        for (let dj = -2; dj <= 2 && !land; dj++) for (let di = -2; di <= 2; di++) if (!C(i + di, j + dj)) { land = true; break; }
        if (land) { if (((X + Y) & 1) === 0) v = kN; }
        else if (me === 2 && (C(i - 1, j) === 1 || C(i + 1, j) === 1 || C(i, j - 1) === 1 || C(i, j + 1) === 1)) {
          if ((X + Y) % 4 === 0) v = kg;
        }
      }
      out[j * TILE + i] = v;
    }
    this._ground.set(key, out);
    return out;
  }

  /** Ground of a tile: terrain texture through the domain warp, shoreline, foam. Unrevealed: void. */
  drawTerrain(ctx, x, y, plot) {
    if (plot.vis === 0) {
      ctx.fillStyle = this.css('0');
      ctx.fillRect(x, y, TILE * this.scale, TILE * this.scale);
      return;
    }
    this.blit(ctx, x, y, this.groundIndices(plot), TILE, TILE);
  }

  /** Hills (two humps) and peaks (one or two masses) of a plot, in the terrain's colours. */
  drawRelief(ctx, x, y, plot) {
    const pt = this._plotType(plot);
    if (pt !== PLOT.HILLS && pt !== PLOT.PEAK) return;
    const s = this.scale;
    const terrain = this.typeName('terrain', plot.terrain);
    const h = hash3(plot.x, plot.y, 7);
    if (pt === PLOT.HILLS) {
      const r = this.mapper.resolve('hill', terrain);
      if (!r || r.sprite === 'none') return;
      const vs = this.variants(r.sprite);
      const spots = [[10 + (h % 3), 19 + ((h >>> 3) % 3), 0], [21 + ((h >>> 5) % 3), 27 + ((h >>> 7) % 2), 1]];
      for (const [px, py, k] of spots) {
        const spr = vs[(k + (h >>> 9)) % vs.length];
        this.drawSprite(ctx, x + px * s, y + py * s, spr.name, { remap: r.opts.remap, mirror: ((h >>> (11 + k)) & 1) === 1 });
      }
    } else {
      const r = this.mapper.resolve('peak', terrain);
      if (!r || r.sprite === 'none') return;
      const vs = this.variants(r.sprite);
      const two = (h & 3) !== 0;
      const spots = two ? [[10 + ((h >>> 2) % 3), 27], [22 + ((h >>> 4) % 2), 31]] : [[16, 31]];
      spots.forEach(([px, py], k) => {
        const spr = vs[((h >>> 6) + k) % vs.length];
        this.drawSprite(ctx, x + px * s, y + py * s, spr.name, { remap: r.opts.remap });
      });
    }
  }

  /**
   * Feature of a plot: scattered (`scatter=9-12`: forests) or single sprites, with light pools
   * from `: light` metadata on visible plots. plot.feature may also be passed as `type`.
   */
  drawFeature(ctx, x, y, plot, type = plot.feature) {
    const r = this.resolve('feature', type);
    if (!r || r.sprite === 'none') return;
    const s = this.scale;
    const h = hash3(plot.x, plot.y, 11);
    const vis = plot.vis === undefined || plot.vis === 2;
    const v = { remap: r.opts.remap, table: r.opts.table };
    if (r.opts.scatter) {
      const [a, b] = String(r.opts.scatter).split('-').map(Number);
      const n = a + (h % ((b || a) - a + 1));
      // jittered grid: 4 columns x 3 rows of cells, filled in a hashed order
      const spots = [];
      const cells = [...Array(12).keys()].sort((p, q) => hash3(plot.x, plot.y, 50 + p) - hash3(plot.x, plot.y, 50 + q));
      for (let k = 0; k < n; k++) {
        const cell = cells[k % 12];
        const cx = cell % 4, cy = Math.floor(cell / 4);
        const jx = hash3(plot.x, plot.y, 70 + k) % 7, jy = hash3(plot.x, plot.y, 90 + k) % 6;
        spots.push([2 + cx * 7 + jx, 11 + cy * 7 + jy, k]);
      }
      spots.sort((p, q) => p[1] - q[1]);
      for (const [px, py, k] of spots) {
        const spr = this.pick(r.sprite, hash3(plot.x, plot.y, 30 + k));
        if (spr) this.drawSprite(ctx, x + px * s, y + py * s, spr.name, { ...v, mirror: (hash3(plot.x, plot.y, 110 + k) & 1) === 1 });
      }
    } else {
      const spr = this.pick(r.sprite, h);
      if (!spr) return;
      const [px, py] = spr.meta.place || [16, 28];
      if (vis) for (const l of spr.meta.lights) this.drawLight(ctx, x + (px - spr.ax + l.x) * s, y + (py - spr.ay + l.y) * s, l.table, l.radius);
      this.drawSprite(ctx, x + px * s, y + py * s, spr.name, v);
    }
  }

  /**
   * River ink on the tile's south and east edges (plot.river bit 0 = south, bit 1 = east), as
   * the API's n_of_river / w_of_river give them. Draw after all ground: the line meanders up to
   * 8 px into neighbouring tiles. `edges` may be passed explicitly as {s, e}.
   */
  drawRiver(ctx, x, y, plot, edges = null) {
    const bits = plot.river | 0;
    const S = edges ? !!edges.s : !!(bits & 1), E = edges ? !!edges.e : !!(bits & 2);
    if (!S && !E) return;
    const PAD = 8, N = TILE + 2 * PAD;
    const nb = this._nbhd(plot, true);
    const water = nb.map((p) => this._isWater(p));
    const src = this._cells(plot, PAD);
    const out = new Uint8Array(N * N).fill(TRANSPARENT);
    const kn = this.K('n'), kg = this.K('g'), k1 = this.K('1');
    const X0 = plot.x * TILE, Y0 = plot.y * TILE;
    for (let j = 0; j < N; j++) for (let i = 0; i < N; i++) {
      if (water[src[j * N + i]]) continue;
      const X = X0 + i - PAD, Y = Y0 + j - PAD;
      const [wx, wy] = this.warp(X, Y);
      const px = X + 0.5 + 0.8 * wx - X0, py = Y + 0.5 + 0.8 * wy - Y0;
      let best = null;
      if (S && px >= -1 && px <= TILE + 1) { const d = py - TILE; best = { d, t: px }; }
      if (E && py >= -1 && py <= TILE + 1) { const d = px - TILE; if (!best || Math.abs(d) < Math.abs(best.d)) best = { d, t: py }; }
      if (!best) continue;
      const ad = Math.abs(best.d);
      if (ad < 1) out[j * N + i] = Math.floor(best.t + 600) % 6 === 0 && best.d < 0 ? kg : kn;
      else if (ad < 2) out[j * N + i] = k1;
    }
    this.blit(ctx, x - PAD * this.scale, y - PAD * this.scale, out, N, N);
  }

  /** River edges of a plot from its own and its neighbours' bits: {n, e, s, w}. */
  riverEdges(plot) {
    const b = (p) => (p ? p.river | 0 : 0);
    return { s: !!(b(plot) & 1), e: !!(b(plot) & 2), n: !!(b(plot.at?.(0, -1)) & 1), w: !!(b(plot.at?.(-1, 0)) & 2) };
  }

  /**
   * Road: a dotted ink line from the tile anchor (16, 18) toward each neighbour with a route or
   * a city. `dirs` may be passed explicitly: an array of 8 booleans (N, NE, E, SE, S, SW, W, NW).
   */
  drawRoad(ctx, x, y, plot, dirs = null) {
    const type = this.typeName('route', plot.route);
    if (!type && !plot.city) return;
    const r = this.mapper.resolve('route', type || 'ROUTE_ROAD');
    if (!r || r.sprite === 'none') return;
    const dot = this.pick(r.sprite, 0);
    if (!dot) return;
    const has = dirs || DIRS8.map(([dx, dy]) => { const n = plot.at?.(dx, dy); return !!n && n.vis !== 0 && (!!this.typeName('route', n.route) || !!n.city); });
    const out = new Uint8Array(TILE * TILE).fill(TRANSPARENT);
    const stamp = (px, py) => {
      for (let j = 0; j < dot.h; j++) for (let i = 0; i < dot.w; i++) {
        const c = dot.data[j * dot.w + i];
        const X = px + i - dot.ax, Y = py + j - dot.ay;
        if (c !== TRANSPARENT && X >= 0 && Y >= 0 && X < TILE && Y < TILE) out[Y * TILE + X] = c;
      }
    };
    const ax = 16, ay = 18;
    let any = false;
    has.forEach((on, k) => {
      if (!on) return;
      any = true;
      const [dx, dy] = DIRS8[k];
      const ex = ax + dx * 16, ey = ay + dy * 16;
      const steps = Math.max(Math.abs(ex - ax), Math.abs(ey - ay));
      for (let t = 0; t <= steps; t += 3) {
        const px = Math.round(ax + ((ex - ax) * t) / steps), py = Math.round(ay + ((ey - ay) * t) / steps);
        if (r.opts.double) {
          const ox = dy !== 0 ? 1 : 0, oy = dx !== 0 ? 1 : 0;
          stamp(px - ox, py - oy); stamp(px + ox, py + oy);
        } else stamp(px, py);
      }
    });
    if (!any) stamp(ax, ay);
    this.blit(ctx, x, y, out, TILE, TILE);
  }

  /**
   * Border ribbon (docs/ART.md 8.1) on this tile's side of every edge where the owner changes.
   * `edges` is a plot (edges computed from plot.at and plot.owner) or {n, e, s, w} booleans.
   */
  drawBorder(ctx, x, y, edges, style, mx = edges.x | 0, my = edges.y | 0) {
    if (!style || style.barbarian || style.animal) return;
    let E = edges;
    if (typeof edges.at === 'function') {
      const o = edges.owner;
      if (o === null || o === undefined || o === -1) return;
      const diff = (dx, dy) => { const n = edges.at(dx, dy); return !n || n.owner !== o; };
      E = { n: diff(0, -1), e: diff(1, 0), s: diff(0, 1), w: diff(-1, 0) };
    }
    if (!E.n && !E.e && !E.s && !E.w) return;
    const t = style.tincture;
    const t2 = style.division !== 'plain' ? style.second : null;
    const out = new Uint8Array(TILE * TILE).fill(TRANSPARENT);
    // a solid line, not a dashed stitch: a dark edge (the shade) where the owners meet, the bright
    // core (the stitch), then a checker fringe of the stitch fading into the territory. A divided
    // owner alternates its two tinctures every 6 px; Sable keeps its bone tick.
    const put = (i, j, depth, along) => {
      const g = along % 6;
      const seg = Math.floor(along / 6);
      const tt = t2 && seg % 2 ? t2 : t;
      if (depth === 2 && ((i + j) & 1)) return;
      let c = depth === 0 ? tt.under : tt.stitch;
      if (depth === 1 && tt.tick !== null && g === 0) c = tt.tick;
      const p = j * TILE + i;
      if (out[p] === TRANSPARENT || depth < 2) out[p] = c;
    };
    for (let k = 0; k < TILE; k++) {
      const X = mx * TILE + k, Y = my * TILE + k;
      for (let d = 0; d < 3; d++) {
        if (E.n) put(k, d, d, X);
        if (E.s) put(k, TILE - 1 - d, d, X);
        if (E.w) put(d, k, d, Y);
        if (E.e) put(TILE - 1 - d, k, d, Y);
      }
    }
    this.blit(ctx, x, y, out, TILE, TILE);
  }

  /**
   * Fog pass for a tile (docs/ART.md 8.2-8.3): fogged pixels through FOG with ink hatching,
   * unrevealed pixels void; both edges ragged (warped). Run after everything that fog covers.
   */
  drawFog(ctx, x, y, plot) {
    const nb = this._nbhd(plot);
    const vis = nb.map((p) => (p.vis === undefined ? 2 : p.vis));
    if (vis.every((v) => v === 2)) return;
    const FOG = this.tables.FOG;
    const k0 = this.K('0');
    if (vis.every((v) => v === 0)) { ctx.fillStyle = this.css('0'); ctx.fillRect(x, y, TILE * this.scale, TILE * this.scale); return; }
    const fogCells = this._cells(plot, 0, 100, 0.6);
    const revCells = this._cells(plot, 0, 300, 1);
    const X0 = plot.x * TILE, Y0 = plot.y * TILE;
    this.remapRegion(ctx, x, y, TILE, TILE, (i, j, c) => {
      if (vis[revCells[j * TILE + i]] === 0) return k0;
      if (vis[fogCells[j * TILE + i]] === 1) {
        const v = FOG[c];
        return (X0 + i + Y0 + j) % 4 === 0 ? FOG[v] : v;
      }
      return c;
    });
  }

  /** Viewport vignette (docs/ART.md 8.3): FOG with a Bayer threshold beyond 75% of the half-diagonal. */
  drawVignette(ctx, x, y, w, h) {
    const s = this.scale;
    const W = Math.floor(w / s), H = Math.floor(h / s);
    const cx = W / 2, cy = H / 2, R = Math.hypot(cx, cy);
    const FOG = this.tables.FOG;
    this.remapRegion(ctx, x, y, W, H, (i, j, c) => {
      const d = Math.hypot(i - cx, j - cy) / R;
      if (d <= 0.75) return c;
      const t = (d - 0.75) / 0.25;
      return t > bayer(i, j) ? FOG[c] : c;
    });
  }

  // ---------------------------------------------------------------- map objects

  /** Resource (docs/ART.md 6.1): a small object standing in the tile's lower right, on a soft shadow. */
  drawResource(ctx, x, y, bonusType) {
    const r = this.resolve('bonus', bonusType);
    if (!r || r.sprite === 'none') return;
    const s = this.scale;
    const g = this.pick(r.sprite, 0);
    if (!g) return;
    this._contactShadow(ctx, x + 25 * s, y + 30 * s, 10, 2);
    this.drawSprite(ctx, x + 25 * s, y + 30 * s, g.name, { remap: r.opts.remap });
  }

  /** Improvement (docs/ART.md 5.7-5.8, 6.2): placed at `: place` (default tile 16,29), with its pools. */
  drawImprovement(ctx, x, y, type, plot = null) {
    const r = this.resolve('improvement', type);
    if (!r || r.sprite === 'none') return;
    const s = this.scale;
    const spr = this.pick(r.sprite, plot ? hash3(plot.x, plot.y, 13) : 0);
    if (!spr) return;
    const [px, py] = spr.meta.place || [16, 29];
    const vis = !plot || plot.vis === undefined || plot.vis === 2;
    if (vis) {
      for (const l of spr.meta.lights) this.drawLight(ctx, x + (px - spr.ax + l.x) * s, y + (py - spr.ay + l.y) * s, l.table, l.radius);
      if (r.opts.pool) { const [tb, rad] = String(r.opts.pool).split(':'); this.drawLight(ctx, x + px * s, y + (py - 2) * s, tb, Number(rad) || 6); }
    }
    this.drawSprite(ctx, x + px * s, y + py * s, spr.name, { remap: vis ? r.opts.remap : this._dark(r.opts.remap), accent: r.opts.accent });
  }

  _dark(remap) { return remap ? `${remap},l3,L3` : 'l3,L3'; }

  /** Is an improvement drawn flat (in the ground pass, under uprights)? */
  isFlatImprovement(type) {
    const r = this.resolve('improvement', type);
    if (!r || r.sprite === 'none') return false;
    const spr = this.pick(r.sprite, 0);
    return !!spr?.meta.flat;
  }

  // ---------------------------------------------------------------- units

  /**
   * One stack on a tile (docs/ART.md 10). state: { facing ('left'|'right'), selected, hp (0-100),
   * moves (bool), count (stack size), mark ('fortify'|'sleep'|'sentry'|'heal'|'auto'|'promote'),
   * hero (stack has a hero), translucent, accent (key), noPennant, layer ('all'|'sprite'|'overlay') }.
   */
  drawUnit(ctx, x, y, unitType, owner, state = {}) {
    const type = this.typeName('unit', unitType);
    const r = type ? this.mapper.resolve('unit', type) : null;
    const entry = type ? this.mapper.entry('unit', type) : null;
    const s = this.scale;
    const layer = state.layer || 'all';
    const spr = r && r.sprite !== 'none' ? this.pick(r.sprite, 0) : null;
    const huge = spr && spr.w >= 32;
    const fx = huge ? 15 : 17, fy = huge ? 29 : 28;
    const mirror = state.facing === 'left';
    const animal = entry?.combat === 'ANIMAL' || owner?.animal;
    const sea = entry?.domain === 'SEA';
    if (spr && layer !== 'overlay') {
      const gy = y + (fy + (entry?.domain === 'AIR' ? 3 : 0)) * s;
      // the owner's ring on the ground (docs/ART.md 10): who a unit belongs to, at a glance
      if (owner && !animal && !owner.hidden) this._teamBase(ctx, x + fx * s, gy, state.ringOwner || owner, spr.w >= 40);
      else if (!sea) this._contactShadow(ctx, x + fx * s, gy);
      const base = { remap: r.opts.remap, owner: owner || null, accent: state.accent || r.opts.accent, mirror };
      if (state.lit !== false) {
        for (const l of spr.meta.lights) {
          const lx = mirror ? spr.w - 1 - l.x : l.x;
          this.drawLight(ctx, x + (fx - (mirror ? spr.w - 1 - spr.ax : spr.ax) + lx) * s, y + (fy - spr.ay + l.y) * s, l.table, l.radius);
        }
      }
      const echoes = Math.min(2, Math.max(0, (state.count | 0) - 1));
      for (let k = echoes; k >= 1; k--) this.drawSprite(ctx, x + (fx - 2 * k) * s, y + (fy - 2 * k) * s, spr.name, { ...base, silhouette: '2' });
      this.drawSprite(ctx, x + fx * s, y + fy * s, spr.name, {
        ...base, rim: state.selected ? 2 : 0, checker: !!(state.translucent || r.opts.ghost),
      });
    }
    if (layer === 'sprite') return;
    // overlays
    if (owner && !animal && !owner.hidden && !state.noPennant && this.hasSprite('ui/pennant')) {
      const pen = this.pick('ui/pennant');
      this.drawSprite(ctx, x + 25 * s, y + 11 * s, pen.name, { owner, flag: true, rim: 1 });
      if (state.hero) this._px(ctx, x, y, [[25, 0, 'y'], [25, -1, 'L']]);
    }
    if (state.hp !== undefined && state.hp !== null) {
      const fill = Math.round((12 * Math.max(0, Math.min(100, state.hp))) / 100);
      const col = state.hp > 50 ? '7' : state.hp >= 25 ? 'y' : 'C';
      const px = [];
      for (let i = 0; i < 12; i++) px.push([10 + i, 31, i < fill ? col : '2']);
      this._px(ctx, x, y, px);
    }
    if (state.moves !== undefined && state.moves !== null) this._px(ctx, x, y, [[23, 31, state.moves ? '8' : '3']]);
    if ((state.count | 0) > 1) this._chip(ctx, x + 1 * s, y + 1 * s, String(state.count));
    else if (state.mark) {
      const m = this.pick(`ui/mark_${state.mark}`);
      if (m) this.drawSprite(ctx, x + 1 * s, y + 1 * s, m.name);
    }
  }

  /** The contact shadow (4.1): a 14x3 50% checker ellipse of void centred on (x, y). */
  _contactShadow(ctx, x, y, w = 14, h = 3) {
    const name = w === 14 && h === 3 ? '_shadow' : `_shadow${w}x${h}`;
    if (!this.lib.sprites.has(name)) {
      const data = new Uint8Array(w * h).fill(TRANSPARENT);
      const cx = (w - 1) / 2, cy = (h - 1) / 2;
      for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) {
        const u = (i - cx) / (w / 2), v = (j - cy) / (h / 2 + 0.1);
        if (u * u + v * v <= 1 && ((i + j) & 1) === 0) data[j * w + i] = 0;
      }
      this.lib.sprites.set(name, { name, w, h, ax: Math.floor(w / 2), ay: h - 1, data, meta: { lights: [], types: [], tags: [] } });
    }
    this.drawSprite(ctx, x, y, name);
  }

  /**
   * The owner's ground ring under a unit: an ellipse in the tincture's field with a shade lip at
   * the front and a checker shadow inside, centred on (x, y). Drawn through the owner slots with
   * the division applied, so two players sharing a tincture show different rings.
   */
  _teamBase(ctx, x, y, owner, big = false) {
    const name = big ? '_teambase_big' : '_teambase';
    if (!this.lib.sprites.has(name)) {
      const w = big ? 30 : 24, h = big ? 9 : 8, rx = w / 2 - 0.5, ry = h / 2 - 0.5;
      const data = new Uint8Array(w * h).fill(TRANSPARENT);
      for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) {
        const u = (i - rx) / rx, v = (j - ry) / ry, d = u * u + v * v;
        if (d > 1) continue;
        if (d > 0.55) data[j * w + i] = j > ry && d > 0.78 ? SLOT_SHADE : SLOT_FIELD;
        else if (((i + j) & 1) === 0) data[j * w + i] = 0;
      }
      this.lib.sprites.set(name, { name, w, h, ax: Math.floor(w / 2), ay: Math.floor(h / 2), data, meta: { lights: [], types: [], tags: [], flag: null } });
    }
    this.drawSprite(ctx, x, y, name, { owner, flag: true });
  }

  /** Paint single art pixels: list of [tileX, tileY, key] relative to canvas point (x, y). */
  _px(ctx, x, y, list) {
    const s = this.scale;
    for (const [i, j, k] of list) { ctx.fillStyle = this.css(k); ctx.fillRect(x + i * s, y + j * s, s, s); }
  }

  /** A soot chip with a number in the tally font (stack counts). */
  _chip(ctx, x, y, text, fg = '8', bg = '1') {
    const s = this.scale;
    const w = this.measureText(text, 'tally') + 2;
    ctx.fillStyle = this.css(bg);
    ctx.fillRect(x, y, w * s, 7 * s);
    this.drawText(ctx, x + 1 * s, y + 6 * s, text, { font: 'tally', color: fg });
  }

  // ---------------------------------------------------------------- cities

  /** City sprite name for a style and size tier: the nearest tier of the style (lower first), then Cloister. */
  citySprite(style, tier) {
    for (const st of [style, 'cloister']) {
      for (let d = 0; d < 5; d++) for (const t of [tier - d, tier + d]) if (t >= 1 && t <= 5 && this.hasSprite(`city/${st}_${t}`)) return `city/${st}_${t}`;
    }
    return null;
  }

  /**
   * City (docs/ART.md 9): sprite by style and size tier, light pool, window light, capital finial,
   * banner and plaque. city: { size, name, capital, visible (lit), disorder, own, turns, style,
   * plaque ('full'|'chip'|false), layer ('all'|'sprite'|'overlay') }. Position: the tile's top-left.
   */
  drawCity(ctx, x, y, city, owner) {
    const s = this.scale;
    const layer = city.layer || 'all';
    const tier = tierOf(city.size | 0 || 1);
    const style = city.style || owner?.cityStyle || 'cloister';
    const name = this.citySprite(style, tier);
    const lit = city.visible !== false && !city.disorder;
    if (name && layer !== 'overlay') {
      const g = this.pick(name, hash3(city.x | 0, city.y | 0, 17));
      const pool = owner?.pool || 'FIRE';
      if (lit && pool !== 'none' && city.pool !== false) this.drawLight(ctx, x + 16 * s, y + 28 * s, pool, 12 + 3 * tier, { inner: true });
      const light = owner?.light || 'l';
      let remap = null;
      if (!lit) remap = 'l3,L3';
      else if (light !== 'l') remap = `l${light},L${light === 'R' ? 'l' : '8'}`;
      this.drawSprite(ctx, x + 16 * s, y + 30 * s, g.name, { remap, owner });
      if (city.capital && g.meta.finial && owner) {
        const [fx, fy] = g.meta.finial;
        const key = this.keys[owner.tincture.field];
        this._px(ctx, x + (16 - g.ax) * s, y + (30 - g.ay) * s, [[fx, fy, key], [fx, fy - 1, key]]);
      }
      if (city.walls) this._cityWalls(ctx, x, y, g);
      if (city.disorder) this._citySmoke(ctx, x, y, g);
    }
    if (layer === 'sprite') return;
    if (owner && this.hasSprite('ui/banner') && city.banner !== false) {
      this.drawSprite(ctx, x + 1 * s, y + 1 * s, this.pick('ui/banner').name, { owner, flag: true, rim: 1 });
      if (owner.sigil && this.hasSprite(owner.sigil)) this.drawSprite(ctx, x + 2 * s, y + 3 * s, this.pick(owner.sigil).name, { owner });
    }
    const mode = city.plaque === undefined ? (s >= 2 ? 'full' : 'chip') : city.plaque;
    if (mode) this._plaque(ctx, x, y, city, owner, mode);
  }

  /**
   * Walls (docs/ART.md 9.3): a low curtain wall in front of the city, 3 px tall on the ground
   * line (tile rows 25-27) a pixel wider than the sprite on each side, merlons on top, a dark
   * gate in the middle, void-outlined. `g` is the city sprite (its width and anchor).
   */
  _cityWalls(ctx, x, y, g) {
    const x0 = Math.max(1, 16 - g.ax - 1), x1 = Math.min(30, 16 - g.ax + g.w);
    const px = [];
    for (let i = x0; i <= x1; i++) {
      const edge = i === x0 || i === x1;
      const gate = i === 15 || i === 16;
      px.push([i, 24, (i - x0) % 2 === 0 || edge ? '0' : '6'], [i, 28, '0']);
      if (edge) { px.push([i, 25, '0'], [i, 26, '0'], [i, 27, '0']); continue; }
      px.push([i, 25, gate ? '5' : '6'], [i, 26, gate ? '1' : '4'], [i, 27, gate ? '1' : '3']);
    }
    for (let i = x0 + 1; i < x1; i += 2) px.push([i, 23, '0']);
    this._px(ctx, x, y, px);
  }

  /** Disorder (docs/ART.md 9.3): an ash smoke column rising from the city's roofs. */
  _citySmoke(ctx, x, y, g) {
    const top = 30 - g.ay;
    const cx = 16 - g.ax + Math.round(g.w * 0.65);
    const wave = [0, 0, 1, 1, 1, 0, -1, -1, -1, 0];
    const px = [];
    for (let k = 1; k <= wave.length; k++) {
      const j = top - k;
      if (j < -12) break;
      const i = cx + wave[k - 1];
      px.push([i, j, k <= 4 ? '5' : '4']);
      if (k <= 3) px.push([i + 1, j, '4']);
      if (k % 3 === 0) px.push([i - 1, j, '3']);
    }
    this._px(ctx, x, y, px);
  }

  _plaque(ctx, x, y, city, owner, mode) {
    const s = this.scale;
    const size = String(city.size | 0);
    const chipW = size.length * 4 + 1;
    const nameW = mode === 'full' && city.name ? this.measureText(city.name, 'vigil', true) : 0;
    const turns = mode === 'full' && city.own && city.turns != null ? String(city.turns) : '';
    // the production glyph (a 5 px mallet) before the turns, so the digit cannot pass for a count
    const glyph = turns && this.hasSprite('ui/plaque_production') ? 6 : 0;
    const turnsW = turns ? glyph + this.measureText(turns, 'tally') + 2 : 0;
    const w = chipW + (nameW ? nameW + 4 : 1) + turnsW + 1;
    const x0 = x + Math.round(16 - w / 2) * s, y0 = y + 29 * s;
    ctx.fillStyle = this.css('5');
    ctx.fillRect(x0, y0, w * s, 7 * s);
    ctx.fillStyle = this.css('1');
    ctx.fillRect(x0 + s, y0 + s, (w - 2) * s, 5 * s);
    const field = owner ? this.keys[owner.tincture.field] : '6';
    const charge = owner ? this.keys[owner.tincture.charge] : '1';
    ctx.fillStyle = this.css(field);
    ctx.fillRect(x0, y0, chipW * s, 7 * s);
    this.drawText(ctx, x0 + 1 * s, y0 + 6 * s, size, { font: 'tally', color: charge });
    if (nameW) this.drawText(ctx, x0 + (chipW + 2) * s, y0 + 6 * s, city.name, { font: 'vigil', color: city.capital && city.own ? '8' : '7', smallCaps: true });
    if (turns) {
      const tx = x0 + (chipW + nameW + 4) * s;
      if (glyph) this.drawSprite(ctx, tx, y0 + s, 'ui/plaque_production');
      this.drawText(ctx, tx + glyph * s, y0 + 6 * s, turns, { font: 'tally', color: '6' });
    }
  }

  // ---------------------------------------------------------------- text and icons

  _glyph(font, ch, smallCaps) {
    const P = { '.': 'period', ',': 'comma', ':': 'colon', ';': 'semicolon', "'": 'apos', '-': 'hyphen', '–': 'dash', '(': 'lparen',
      ')': 'rparen', '/': 'slash', '+': 'plus', '×': 'times', '!': 'excl', '?': 'quest', '&': 'amp', '#': 'hash', '−': 'minus' };
    let n = P[ch] || ch;
    if (font === 'tally' && (ch === '-' || ch === '−')) n = 'minus';
    if (font === 'tally' && (ch === 'x' || ch === '×')) n = 'times';
    if (font === 'vigil' && /[A-Za-z]/.test(ch)) {
      const lower = `font/vigil/${ch.toLowerCase()}_sc`;
      if ((smallCaps || /[a-z]/.test(ch)) && this.lib.sprites.has(lower)) return this.lib.sprites.get(lower);
      n = ch.toUpperCase();
    }
    return this.lib.sprites.get(`font/${font}/${n}`) || null;
  }

  /** Width of a text in art px. font: 'vigil' or 'tally'. */
  measureText(text, font = 'vigil', smallCaps = false) {
    let w = 0;
    for (const ch of text) {
      if (ch === ' ') { w += 3; continue; }
      const g = this._glyph(font, ch, smallCaps);
      w += g ? g.w + 1 : 4;
    }
    return Math.max(0, w - 1);
  }

  /** Draw text with its baseline-left at canvas (x, y). opts: { font, color (key), shadow, smallCaps } */
  drawText(ctx, x, y, text, opts = {}) {
    const font = opts.font || 'vigil';
    const color = opts.color || '7';
    const s = this.scale;
    let cx = 0;
    for (const ch of text) {
      if (ch === ' ') { cx += 3; continue; }
      const g = this._glyph(font, ch, opts.smallCaps);
      if (!g) { cx += 4; continue; }
      const gx = x + cx * s, gy = y - (g.h - 1) * s;
      if (opts.shadow) this.drawSprite(ctx, gx + s + g.ax * s, gy + s + g.ay * s, g.name, { remap: '80' });
      this.drawSprite(ctx, gx + g.ax * s, gy + g.ay * s, g.name, { remap: `8${color}` });
      cx += g.w + 1;
    }
    return cx - 1;
  }

  /** An icon as its own small canvas (cached), e.g. icon('icon16/food'). opts: { scale, owner, remap } */
  icon(name, opts = {}) {
    const scale = opts.scale || this.scale;
    const key = `icon|${name}|${scale}|${opts.remap || ''}|${opts.owner?.id || ''}`;
    let c = this._entries.get(key);
    if (c) return c;
    const spr = this.pick(name);
    if (!spr) return null;
    const vd = this.variantData(spr, { remap: opts.remap, owner: opts.owner, flag: !!opts.owner });
    c = makeCanvas(vd.w * scale, vd.h * scale);
    c.getContext('2d').putImageData(this._imageData(vd.data, vd.w, vd.h, scale), 0, 0);
    this._entries.set(key, c);
    return c;
  }

  /** Icon for a game type (religion, promotion, building, unitcombat, specialist, bonus, civ). */
  iconFor(kind, type, opts = {}) {
    const r = this.resolve(kind, type);
    if (!r || r.sprite === 'none') return null;
    return this.icon(r.sprite, { ...opts, remap: opts.remap || r.opts.remap });
  }

  /** Draw an icon with its top-left at canvas (x, y). */
  drawIcon(ctx, x, y, name, opts = {}) {
    const spr = this.pick(name);
    if (!spr) return;
    this.drawSprite(ctx, x + spr.ax * this.scale, y + spr.ay * this.scale, spr.name, opts);
  }

  /** Tile cursor (docs/ART.md 11.7): four corner brackets, stone on hover, vellum when selected. */
  drawTileCursor(ctx, x, y, selected = false) {
    const k = selected ? '8' : '6';
    const px = [];
    for (let i = 0; i < 5; i++) {
      px.push([i, 0, k], [0, i, k], [31 - i, 0, k], [31, i, k], [i, 31, k], [0, 31 - i, k], [31 - i, 31, k], [31, 31 - i, k]);
    }
    this._px(ctx, x, y, px);
  }
}

export { tierOf, hash3 };
