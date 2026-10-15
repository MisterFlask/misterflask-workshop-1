#!/usr/bin/env node
// bake-terrain.mjs: authoring aid. Bakes seamless 64x64 terrain textures from the recipes of
// docs/ART.md 5.2 (base, ordered-dither mottling, signature marks) into sprite text.
//
//   node tools/bake-terrain.mjs [names...] > terrain/textures.spr
//
// The output text is the source of truth afterwards: edit it by hand freely. Re-baking replaces
// hand edits, so bake only new terrains (pass their names) and paste them in.
// Hell terrains are not baked: the manifest maps them to their twin with a colour remap.

const SIZE = 64;

function mulberry32(seed) {
  return () => {
    seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

class Tex {
  constructor(base) { this.g = Array.from({ length: SIZE }, () => Array(SIZE).fill(base)); this.marked = Array.from({ length: SIZE }, () => Array(SIZE).fill(false)); }
  w(v) { return ((v % SIZE) + SIZE) % SIZE; }
  get(x, y) { return this.g[this.w(y)][this.w(x)]; }
  set(x, y, k) { this.g[this.w(y)][this.w(x)] = k; this.marked[this.w(y)][this.w(x)] = true; }
  free(x, y, r = 1) {
    for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) if (this.marked[this.w(y + dy)][this.w(x + dx)]) return false;
    return true;
  }
  /** Mottling blotch: ellipse filled with a 50% checker or 25% grid of `k`. */
  blotch(cx, cy, rx, ry, k, pattern) {
    for (let y = -ry; y <= ry; y++) for (let x = -rx; x <= rx; x++) {
      if ((x * x) / (rx * rx) + (y * y) / (ry * ry) > 1) continue;
      const X = this.w(cx + x), Y = this.w(cy + y);
      const on = pattern === 'checker' ? ((X + Y) & 1) === 0 : (X & 1) === 0 && (Y & 1) === 0;
      if (on) this.g[Y][X] = k;
    }
  }
  rows() { return this.g.map((r) => r.join('')); }
}

/** Place `count` marks with `draw(tex, x, y, rnd)`, keeping them apart (`gap` px). */
function scatter(tex, rnd, count, gap, draw) {
  let placed = 0;
  for (let tries = 0; placed < count && tries < count * 40; tries++) {
    const x = Math.floor(rnd() * SIZE), y = Math.floor(rnd() * SIZE);
    if (!tex.free(x, y, gap)) continue;
    draw(tex, x, y, rnd);
    placed++;
  }
}

/** `n` blotches 4 to 12 px across; pattern 'checker' (50%), 'grid' (25%) or 'mix' (alternating). */
function mottle(tex, rnd, k, n, pattern) {
  for (let i = 0; i < n; i++) {
    const rx = 2 + Math.floor(rnd() * 4), ry = 1 + Math.floor(rnd() * 3);
    const pat = pattern === 'mix' ? (i % 2 ? 'grid' : 'checker') : pattern;
    tex.blotch(Math.floor(rnd() * SIZE), Math.floor(rnd() * SIZE), rx, ry, k, pat);
  }
}

/** Horizontal streaks on every third row (water). */
function streaks(tex, rnd, k, n) {
  for (let i = 0; i < n; i++) {
    const y = Math.floor(rnd() * (SIZE / 3)) * 3;
    const x = Math.floor(rnd() * SIZE), len = 4 + Math.floor(rnd() * 10);
    for (let j = 0; j < len; j++) tex.g[y][tex.w(x + j)] = k;
  }
}

const RECIPES = {
  grass: { seed: 11, base: 'm', make(t, r) {
    mottle(t, r, 'q', 8, 'mix');
    scatter(t, r, 26, 3, (t, x, y, r) => { // tuft: a small inverted V, o O o
      t.set(x - 1, y, 'o'); t.set(x, y - 1, r() < 0.7 ? 'O' : 'o'); t.set(x + 1, y, 'o');
    });
  } },
  plains: { seed: 23, base: 'Q', make(t, r) {
    mottle(t, r, 'u', 8, 'mix');
    scatter(t, r, 30, 2, (t, x, y, r) => { // stubble dash, alternating y 5, umber under
      const len = 2 + Math.floor(r() * 3);
      for (let j = 0; j < len; j++) { t.set(x + j, y, j % 2 ? '5' : 'y'); if (r() < 0.6) t.set(x + j, y + 1, 'u'); }
    });
  } },
  desert: { seed: 37, base: 'Y', make(t, r) {
    mottle(t, r, '6', 8, 'grid');
    scatter(t, r, 11, 4, (t, x, y, r) => { // dune arc: crest 7, underside y
      const len = 5 + Math.floor(r() * 4);
      for (let j = 0; j < len; j++) {
        const lift = j === 0 || j === len - 1 ? 1 : 0;
        t.set(x + j, y + lift, '7');
        if (!lift) t.set(x + j, y + 1, 'y');
      }
    });
  } },
  tundra: { seed: 41, base: '4', make(t, r) {
    mottle(t, r, '3', 8, 'mix');
    scatter(t, r, 22, 2, (t, x, y, r) => t.set(x, y, 'O')); // lichen dots (one colour: the texture limit is 5)
    scatter(t, r, 7, 3, (t, x, y, r) => { // crack line 2 with a 5 lip
      const len = 3 + Math.floor(r() * 3);
      for (let j = 0; j < len; j++) { t.set(x + j, y + (j > 1 ? 1 : 0), '2'); t.set(x + j, y - 1 + (j > 1 ? 1 : 0), j === 0 ? '5' : t.get(x + j, y - 1)); }
    });
  } },
  snow: { seed: 53, base: 'f', make(t, r) {
    mottle(t, r, 'g', 6, 'grid');
    scatter(t, r, 16, 3, (t, x, y, r) => { // drift line g with F crest pixels
      const len = 4 + Math.floor(r() * 5);
      for (let j = 0; j < len; j++) t.set(x + j, y, 'g');
      t.set(x + 1, y - 1, 'F'); if (len > 5) t.set(x + 3, y - 1, 'F');
    });
  } },
  marsh: { seed: 67, base: 'q', make(t, r) {
    mottle(t, r, 'k', 9, 'mix');
    scatter(t, r, 12, 3, (t, x, y, r) => { // water slit n with one N glint
      const len = 3 + Math.floor(r() * 4);
      for (let j = 0; j < len; j++) t.set(x + j, y, j === 1 ? 'N' : 'n');
    });
    scatter(t, r, 16, 2, (t, x, y, r) => { // reed tick
      t.set(x, y, 'o'); t.set(x, y - 1, 'o');
    });
  } },
  coast: { seed: 71, base: 'n', make(t, r) {
    streaks(t, r, 'd', 34);
    scatter(t, r, 16, 3, (t, x, y, r) => { // ripple dash N, 2 to 5 px
      const len = 2 + Math.floor(r() * 4);
      for (let j = 0; j < len; j++) t.set(x + j, y, 'N');
    });
  } },
  ocean: { seed: 89, base: 'd', make(t, r) {
    streaks(t, r, '0', 30);
    scatter(t, r, 9, 4, (t, x, y, r) => { // sparse ripple dashes n, rare N
      const len = 2 + Math.floor(r() * 3);
      for (let j = 0; j < len; j++) t.set(x + j, y, r() < 0.12 ? 'N' : 'n');
    });
  } },
};

const names = process.argv.slice(2).length ? process.argv.slice(2) : Object.keys(RECIPES);
const out = ['; terrain textures: seamless 64x64, sampled in map space (docs/ART.md 5.1-5.2).',
  '; Baked once by tools/bake-terrain.mjs, then hand-editable: this text is the source.', ''];
for (const n of names) {
  const rec = RECIPES[n];
  if (!rec) { console.error(`unknown recipe ${n}`); process.exit(2); }
  const t = new Tex(rec.base);
  rec.make(t, mulberry32(rec.seed));
  out.push(`= terrain/${n} 64x64 0,0`, `: types TERRAIN_${n.toUpperCase()}`, ...t.rows(), '');
}
process.stdout.write(out.join('\n'));
