// cities/citylight.js: the light a city casts on its ground (docs/ART.md 4.4, 9.1), as the cities
// set proposes it to the toolkit. Vanilla ES module, no dependencies; only art.js's public surface.
//
//   import { installCityLight } from './cities/citylight.js';
//   installCityLight(art);        // once, after loadArt(): drawCity and renderMap pick it up
//
// 1. The hearth pool. art.drawLight's city pool applies FIRE twice (full radius, then half), and
//    FIRE twice walks moss to dun, then ochre: the city stands in mud. Here a lit city sits in two
//    rings instead, each pixel remapped once from its own ground colour:
//      inner ring  round the walls: the pool's INNER table (HEARTH for FIRE: dark ground to ember e,
//                  mid ground to rust x, light marks and sand to flame E);
//      outer ring  the pool's own table (FIRE: ground to umber u and dun Q), falling off to the
//                  bare ground,
//    both elliptical (vertical radius / 1.4) with the 4x4 Bayer falloff, so the inner ring dithers
//    into the outer and the outer into the ground. BLOOD (Infernal, Sheaim) gets GORE inside (dark
//    to blood c, the rest to crimson C; scarlet only where the ground was already pale), ARC
//    (Amurites, Svartalfar) gets AURA (dusk p, lilac P). Radii by size tier: outer 16 + 3 x tier
//    (three quarters for BLOOD and ARC, which turn the ground's hue), inner 9 + 2 x tier. Open
//    water (deep d, navy n) keeps its night; ripples and glints warm.
// 2. The wall glow. The sprite covers the pool's middle, so the light that shows is the light at
//    its feet: the ground within 4 px of the sprite's lower walls takes the inner table once more
//    (ember and rust to flame, dun to rust), dithered by distance and by height up the walls.
// 3. Sprite lights. drawCity draws the city sprite's own `: light` pools under it: the tribal and
//    barbarian bonfire (FIRE radius 8), so a camp of tents can still be found at 1x.
//
// The tables are written like palette.txt's remap section (key, then the target per table), so the
// toolkit can move them there as new columns. Integration in art.js: call drawCityPool from
// drawCity and renderMap instead of drawLight(..., { inner: true }), and in drawCity's sprite layer
// draw spr.meta.lights and drawWallGlow before the sprite (lit cities only, as for the pool).
// installCityLight does exactly that by wrapping: a drawLight call with opts.inner is a city pool
// (the only caller of that option), and drawCity's sprite layer draws the lights and glow first.

const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map((v) => (v + 0.5) / 16);
const bayer = (x, y) => BAYER[(y & 3) * 4 + (x & 3)];

// key HEARTH GORE AURA: the inner ring of FIRE, BLOOD and ARC pools
export const INNER_TABLES = `
key HEARTH GORE AURA
0   e      c    p
1   e      c    p
2   e      c    p
3   e      c    p
4   x      C    p
5   x      C    P
6   E      C    P
7   8      R    P
8   8      R    8
d   n      c    v
n   N      c    p
N   g      c    p
g   f      C    P
f   Y      C    P
F   7      C    P
k   e      c    p
t   x      C    p
T   E      C    P
q   e      c    p
m   x      C    p
o   E      C    P
O   E      C    P
u   e      c    p
Q   x      C    p
x   E      C    P
y   E      C    P
Y   E      C    P
v   e      c    p
p   x      C    P
P   E      C    8
e   E      C    P
E   l      R    P
l   L      R    P
L   L      R    8
c   x      C    p
C   E      C    P
R   l      R    P
s   x      c    p
S   E      C    P
W   8      R    8
`;
const INNER_OF = { FIRE: 'HEARTH', BLOOD: 'GORE', ARC: 'AURA' };

/** Parse INNER_TABLES into index tables for an art instance (cached on it). */
function innerTables(art) {
  if (art._cityInner) return art._cityInner;
  const rows = INNER_TABLES.trim().split('\n').map((l) => l.trim().split(/\s+/));
  const names = rows[0].slice(1);
  const out = {};
  for (const n of names) out[n] = new Uint8Array(art.keys.length).map((_, i) => i);
  for (const r of rows.slice(1)) names.forEach((n, k) => { out[n][art.K(r[0])] = art.K(r[k + 1]); });
  art._cityInner = out;
  return out;
}

/**
 * Radii in art px of a city pool by size tier: [outer, inner]. FIRE's outer ring only warms the
 * ground a step (umber, dun), so it may spread; BLOOD and ARC turn its hue outright, so their outer
 * ring keeps closer to the walls (three quarters).
 */
export function poolRadii(tier, table = 'FIRE') {
  const outer = 16 + 3 * tier;
  return [table === 'FIRE' ? outer : Math.round(outer * 0.75), 9 + 2 * tier];
}

/**
 * Draw a city's ground pool centred at canvas (cx, cy) (the tile's 16, 28): `table` is the owner's
 * pool (FIRE, BLOOD, ARC; any other table falls back to a single ring of itself).
 */
export function drawCityPool(art, ctx, cx, cy, table, tier) {
  const outer = art.tables[table];
  if (!outer) return;
  const inner = innerTables(art)[INNER_OF[table]] || null;
  const s = art.scale;
  const [R, Ri] = poolRadii(tier, table);
  const ry = R / 1.4, ryi = Ri / 1.4;
  const W = Math.ceil(R) * 2 + 1, H = Math.ceil(ry) * 2 + 1;
  const x0 = cx - Math.ceil(R) * s, y0 = cy - Math.ceil(ry) * s;
  const ox = Math.round(cx / s), oy = Math.round(cy / s);
  const still = new Set([art.K('d'), art.K('n')]); // open water keeps its night: only ripples catch the light
  art.remapRegion(ctx, x0, y0, W, H, (i, j, c) => {
    if (still.has(c)) return c;
    const dx = i - Math.ceil(R), dy = j - Math.ceil(ry);
    const b = bayer(ox + dx, oy + dy) * 0.999;
    if (inner) {
      const di = Math.sqrt((dx / Ri) ** 2 + (dy / ryi) ** 2);
      if (di < 1 && 1 - di > b) return inner[c];
    }
    const d = Math.sqrt((dx / R) ** 2 + (dy / ry) ** 2);
    return d < 1 && 1 - d > b ? outer[c] : c;
  });
}

/**
 * Wall glow strength around a city sprite: for each pixel of the sprite's box grown by GROW, how
 * much of the firelight from its doors and windows falls there (0 to 0.85, so even the brightest
 * band stays dithered): strongest on the ground
 * beside the walls, falling off over 4 px from the silhouette, over 7 rows up the walls (higher up,
 * the ground behind a building is farther from its light) and over 3 rows below the ground line.
 * Cached per sprite.
 */
const GROW = 4;
function glowMap(spr) {
  if (spr._glow) return spr._glow;
  const W = spr.w + 2 * GROW, H = spr.h + 2 * GROW;
  const op = (i, j) => i >= 0 && j >= 0 && i < spr.w && j < spr.h && spr.data[j * spr.w + i] !== 255;
  const out = new Float32Array(W * H);
  const base = spr.h - 1; // the sprite's ground line
  for (let j = 0; j < H; j++) for (let i = 0; i < W; i++) {
    const si = i - GROW, sj = j - GROW;
    if (op(si, sj)) continue;
    let d = Infinity;
    for (let b = -GROW; b <= GROW; b++) for (let a = -GROW; a <= GROW; a++) if (op(si + a, sj + b)) d = Math.min(d, Math.hypot(a, b));
    if (d > GROW) continue;
    const fd = 1 - (d - 1) / 3.5;
    const up = base - sj;
    const fh = up >= 0 ? 1 - up / 7 : 1 + up / 3;
    out[j * W + i] = 0.85 * Math.max(0, fd) * Math.max(0, fh);
  }
  spr._glow = out;
  return out;
}

/**
 * The wall glow: the ground beside a lit city's walls takes the pool's inner table once more
 * (ember to flame, rust to flame, dun to rust), dithered by glowMap, before the sprite is drawn.
 * (x, y): the canvas position of the tile; the sprite's anchor sits at tile (16, 30).
 */
export function drawWallGlow(art, ctx, x, y, spr, table) {
  const inner = innerTables(art)[INNER_OF[table]];
  if (!inner || !spr) return;
  const s = art.scale;
  const g = glowMap(spr);
  const W = spr.w + 2 * GROW, H = spr.h + 2 * GROW;
  const tx = 16 - spr.ax - GROW, ty = 30 - spr.ay - GROW; // tile position of the glow box
  const ox = Math.round(x / s) + tx, oy = Math.round(y / s) + ty;
  art.remapRegion(ctx, x + tx * s, y + ty * s, W, H, (i, j, c) => {
    const v = g[j * W + i];
    return v > 0 && v > bayer(ox + i, oy + j) * 0.999 ? inner[c] : c;
  });
}

/** The city's tier from the radius art.js passes for its pool (12 + 3 x tier). */
const tierOfRadius = (r) => Math.max(1, Math.min(5, Math.round((r - 12) / 3)));

/** The sprite drawCity would draw for a city (same choice as art.js). */
function citySpriteOf(art, city, owner) {
  const size = city.size | 0 || 1;
  const tier = size <= 3 ? 1 : size <= 7 ? 2 : size <= 12 ? 3 : size <= 19 ? 4 : 5;
  const name = art.citySprite(city.style || owner?.cityStyle || 'cloister', tier);
  return name ? art.pick(name) : null;
}

/** Wrap art.drawLight (city pools) and art.drawCity (sprite lights). Idempotent; returns art. */
export function installCityLight(art) {
  if (art._cityLight) return art;
  const baseLight = art.drawLight.bind(art);
  art.drawLight = (ctx, cx, cy, table, radius, opts = {}) => {
    if (opts.inner) drawCityPool(art, ctx, cx, cy, table, tierOfRadius(radius));
    else baseLight(ctx, cx, cy, table, radius, opts);
  };
  const baseCity = art.drawCity.bind(art);
  art.drawCity = (ctx, x, y, city, owner) => {
    const layer = city.layer || 'all';
    const lit = city.visible !== false && !city.disorder;
    if (layer !== 'overlay' && lit) {
      const spr = citySpriteOf(art, city, owner);
      const s = art.scale;
      const pool = owner?.pool || 'FIRE';
      for (const l of spr?.meta.lights || []) baseLight(ctx, x + (16 - spr.ax + l.x) * s, y + (30 - spr.ay + l.y) * s, l.table, l.radius);
      if (pool !== 'none' && city.glow !== false) drawWallGlow(art, ctx, x, y, spr, pool);
    }
    baseCity(ctx, x, y, city, owner);
  };
  art._cityLight = true;
  return art;
}
