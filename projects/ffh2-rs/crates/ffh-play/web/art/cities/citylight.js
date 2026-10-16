// cities/citylight.js: the light pass (docs/ART.md 4.4, 9.1): how a city, and every other light
// source on the map, lights the ground and the walls around it. Vanilla ES module, no dependencies
// beyond art.js's public surface.
//
//   import { installCityLight } from './cities/citylight.js';
//   installCityLight(art);        // once, after loadArt(): drawLight, drawCity and renderMap pick it up
//   (loadArt() installs it by default, art.js PASSES; this is for loadArt({ passes: false }))
//
// Second round (the art director's light review: the map was under-lit against the board, its pools
// read as mud). A pool is no longer a recolouring of the ground toward one table: it is three
// nested rings, each remapping a pixel once from its own ground colour, with 4x4 Bayer falloff, so
// the light rises in value toward its source the way light does:
//
//   outer  the glow: WARM for firelight (ground one step lighter and warmer: moss to ochre, dark
//          moss to rust, plains to ochre, ash to sand), SMOULDER for BLOOD, the 2.4 table for ARC,
//          HAZE for RITE; at most three quarters of the pixels, falling off to the bare ground;
//   inner  the hearth: HEARTH (ground to flame E, dark ground to ember e and rust x, pale marks to
//          amber l), or GORE, AURA, HALLOW for the other families (SMOULDER is BLOOD's outer ring,
//          HAZE RITE's);
//   core   the source's own pool of light: EMBER (to amber l, pale to glow L), only for city pools
//          and point lights of radius 6 and more.
//
// Open water (deep d, navy n) keeps its night under every ring; its ripples and glints catch it.
//
// 1. City pools. Radii by size tier t: outer 24 + 4t (three quarters for BLOOD and ARC, which turn
//    the ground's hue), inner 12 + 4t, core 6 + 2t, elliptical (vertical radius / 1.4), centred on
//    the city's ground point (tile 16, 28). Frost, celestial and the faded Sidar cast no pool.
// 2. Wall glow. The sprite covers the pool's middle, so the light that shows is the light at its
//    feet: the ground within 4 px of the sprite's lower walls takes HEARTH, and EMBER close in.
// 3. Facade light. The lowest rows of a lit city's walls catch the pool (HEARTH from the sprite's
//    own colours: dark stone to ember, mid stone to rust and flame), falling off over 5 + t rows, so
//    the town stands in its light instead of on it. Outlines and windows are left alone.
// 4. Point lights. Every other drawLight (lanterns, campfires, flames, forges, mana nodes,
//    landmarks, hellfire) is the same three rings at radius r, 0.55 r and 0.25 r (the core only
//    from r = 6); a ritual light (RITE) is mostly its teal haze (HAZE at r) with a sparse ritual
//    heart (HALLOW at 0.5 r and 0.22 r), so a ring drawn in ritual green still reads against it.
// 5. Sprite lights. drawCity draws the city sprite's own `: light` pools under it (the tribal and
//    barbarian bonfire), as point lights.
//
// The ring tables (WARM, HEARTH, EMBER, SMOULDER, GORE, AURA, HALLOW, HAZE) are palette.txt remap
// columns beside the derived FIRE, BLOOD, RITE and ARC (docs/ART.md 2.4); RINGS names each family's three.

import { hash3 } from '../art.js';

const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map((v) => (v + 0.5) / 16);
const bayer = (x, y) => BAYER[(y & 3) * 4 + (x & 3)];

/** The rings of each pool family: [outer, inner, core] table names (palette.txt remap columns, 2.4). */
export const RINGS = {
  FIRE: ['WARM', 'HEARTH', 'EMBER'],
  BLOOD: ['SMOULDER', 'GORE', 'GORE'],
  ARC: ['ARC', 'AURA', 'AURA'],
  RITE: ['HAZE', 'HALLOW', 'HALLOW'],
};

/** Shares of each ring at its centre (the Bayer threshold falls off linearly to 0 at its edge). */
export const SHARE = { outer: 0.7, inner: 0.78, core: 0.75 };
const FALL = { outer: 0.6, inner: 0.9, core: 1 }; // the share falls off as (1 - d)^FALL

/** The three ring tables of a pool family (null where a table is missing). */
function ringTables(art, table) {
  const names = RINGS[table] || [table, null, null];
  return names.map((n) => (n ? art.tables[n] || null : null));
}

/**
 * Radii in art px of a city pool by size tier: [outer, inner, core]. FIRE's outer ring only warms and
 * lifts the ground a step, so it may spread; BLOOD and ARC turn its hue outright, so their outer
 * ring keeps closer to the walls (three quarters).
 */
export function poolRadii(tier, table = 'FIRE') {
  const outer = 24 + 4 * tier;
  return [table === 'FIRE' ? outer : Math.round(outer * 0.75), 12 + 4 * tier, 6 + 2 * tier];
}

/**
 * Three nested elliptical rings centred at canvas (cx, cy): radii [outer, inner, core] in art px
 * (0 leaves a ring out), tables from the pool family.
 */
export function drawRings(art, ctx, cx, cy, table, radii, shares = SHARE) {
  const [To, Ti, Tc] = ringTables(art, table);
  if (!To) return;
  const [Ro, Ri, Rc] = radii;
  const s = art.scale;
  const R = Math.max(Ro, Ri, Rc);
  const ry = R / 1.4;
  const W = Math.ceil(R) * 2 + 1, H = Math.ceil(ry) * 2 + 1;
  const x0 = cx - Math.ceil(R) * s, y0 = cy - Math.ceil(ry) * s;
  const ox = Math.round(cx / s), oy = Math.round(cy / s);
  const kd = art.K('d'), kn = art.K('n');
  const share = (dx, dy, r, ring) => {
    if (!r) return 0;
    const d = Math.sqrt((dx / r) ** 2 + ((dy * 1.4) / r) ** 2);
    return d < 1 ? shares[ring] * (1 - d) ** FALL[ring] : 0;
  };
  art.remapRegion(ctx, x0, y0, W, H, (i, j, c) => {
    if (c === kd || c === kn) return c; // open water keeps its night
    const dx = i - Math.ceil(R), dy = j - Math.ceil(ry);
    const b = bayer(ox + dx, oy + dy) * 0.999;
    if (Tc && b < share(dx, dy, Rc, 'core')) return Tc[c];
    if (Ti && b < share(dx, dy, Ri, 'inner')) return Ti[c];
    if (b < share(dx, dy, Ro, 'outer')) return To[c];
    return c;
  });
}

/**
 * Draw a city's ground pool centred at canvas (cx, cy) (the tile's 16, 28): `table` is the owner's
 * pool (FIRE, BLOOD, ARC; any other table falls back to a single ring of itself).
 */
export function drawCityPool(art, ctx, cx, cy, table, tier) {
  const k = DAYLIGHT[table] ?? 1;
  if (k <= 0) return;
  drawRings(art, ctx, cx, cy, table, poolRadii(tier, table).map((r) => Math.round(r * k)));
}

/**
 * Daylight (docs/ART.md 4.4): the map is lit by the sun, so firelight barely shows. City pools are
 * scaled per family -- firelight none, while corruption, the arcane and ritual auras stay as faction
 * identity, closer to the walls -- firelight point lights (lanterns, campfires) are halved, and the
 * night's wall glow and facade light are off.
 */
export const DAYLIGHT = { FIRE: 0, BLOOD: 0.55, ARC: 0.55, RITE: 0.7, firePoint: 0.5, glow: false };

// a ritual light is mostly its teal haze, with a sparse heart of ritual green (so a drawn ring of
// ritual pixels still reads against it)
const RITE_SHARE = { outer: 0.8, inner: 0.6, core: 0.85 };

/**
 * A point light (lantern, campfire, flames, node, landmark) of radius r: rings r, 0.55 r, 0.25 r
 * (ritual lights: r, 0.5 r, 0.22 r, the inner ring sparser).
 */
export function drawPointLight(art, ctx, cx, cy, table, r) {
  if (table === 'FIRE') r *= DAYLIGHT.firePoint;
  if (r < 1) return;
  if (table === 'RITE') drawRings(art, ctx, cx, cy, table, [r, r * 0.5, r * 0.22], RITE_SHARE);
  else drawRings(art, ctx, cx, cy, table, [r, r * 0.55, r >= 6 ? r * 0.25 : 0]);
}

/**
 * Wall glow strength around a city sprite: for each pixel of the sprite's box grown by GROW, how
 * much of the firelight from its doors and windows falls there (0 to 0.85, so even the brightest
 * band stays dithered): strongest on the ground beside the walls, falling off over 4 px from the
 * silhouette, over 7 rows up the walls (higher up, the ground behind a building is farther from its
 * light) and over 3 rows below the ground line. Cached per sprite.
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
 * The wall glow: the ground beside a lit city's walls takes the pool's core table where the glow is
 * strong and its inner table elsewhere, dithered by glowMap, before the sprite is drawn.
 * (x, y): the canvas position of the tile; the sprite's anchor sits at tile (16, 30).
 */
export function drawWallGlow(art, ctx, x, y, spr, table) {
  const [, Ti, Tc] = ringTables(art, table);
  if (!Ti || !spr) return;
  const s = art.scale;
  const g = glowMap(spr);
  const W = spr.w + 2 * GROW, H = spr.h + 2 * GROW;
  const tx = 16 - spr.ax - GROW, ty = 30 - spr.ay - GROW; // tile position of the glow box
  const ox = Math.round(x / s) + tx, oy = Math.round(y / s) + ty;
  art.remapRegion(ctx, x + tx * s, y + ty * s, W, H, (i, j, c) => {
    const v = g[j * W + i];
    if (!v) return c;
    const b = bayer(ox + i, oy + j) * 0.999;
    if (Tc && b < v * 0.6) return Tc[c];
    return b < v ? Ti[c] : c;
  });
}

/**
 * The facade light: the lowest rows of the sprite's walls catch the pool (its inner table from the
 * sprite's own colours), with a Bayer share of 0.8 on the ground line falling to 0 over 4 + tier
 * rows. Only dark and mid colours take it: outline, windows, pale stone and hide are left alone. Call right after the sprite is drawn.
 */
export function drawFacadeLight(art, ctx, x, y, spr, table, tier) {
  const [, Ti] = ringTables(art, table);
  if (!Ti || !spr) return;
  const s = art.scale;
  const rows = 5 + tier;
  const base = spr.h - 1;
  // only dark and mid colours catch it: outlines, windows, pale stone, hide and cloth stay as drawn
  const take = new Set([...'1234uQxktqmovpecsgnN'].map((k) => art.K(k)));
  const j0 = Math.max(0, base - rows);
  const tx = 16 - spr.ax, ty = 30 - spr.ay + j0;
  const ox = Math.round(x / s) + tx, oy = Math.round(y / s) + ty;
  art.remapRegion(ctx, x + tx * s, y + ty * s, spr.w, base - j0 + 1, (i, j, c) => {
    const sj = j0 + j;
    const v = spr.data[sj * spr.w + i];
    if (v === 255 || !take.has(c)) return c;
    const share = 0.8 * (1 - (base - sj) / rows);
    return bayer(ox + i, oy + j) * 0.999 < share ? Ti[c] : c;
  });
}

/** The city's tier from the radius art.js passes for its pool (12 + 3 x tier). */
const tierOfRadius = (r) => Math.max(1, Math.min(5, Math.round((r - 12) / 3)));
const tierOfSize = (size) => (size <= 3 ? 1 : size <= 7 ? 2 : size <= 12 ? 3 : size <= 19 ? 4 : 5);

/** The sprite drawCity would draw for a city (same choice as art.js). */
function citySpriteOf(art, city, owner) {
  const tier = tierOfSize(city.size | 0 || 1);
  const name = art.citySprite(city.style || owner?.cityStyle || 'cloister', tier);
  return name ? art.pick(name, hash3(city.x | 0, city.y | 0, 17)) : null;
}

/** Wrap art.drawLight (city pools and point lights) and art.drawCity (sprite lights, glow, facade). Idempotent. */
export function installCityLight(art) {
  if (art._cityLight) return art;
  art.drawLight = (ctx, cx, cy, table, radius, opts = {}) => {
    if (!art.tables[table] && !RINGS[table]) return;
    if (radius <= 0) return;
    if (opts.inner) drawCityPool(art, ctx, cx, cy, table, tierOfRadius(radius));
    else drawPointLight(art, ctx, cx, cy, table, radius);
  };
  const baseCity = art.drawCity.bind(art);
  art.drawCity = (ctx, x, y, city, owner) => {
    const layer = city.layer || 'all';
    const lit = city.visible !== false && !city.disorder;
    const pool = owner?.pool || 'FIRE';
    if (layer === 'overlay' || !lit) return baseCity(ctx, x, y, city, owner);
    const spr = citySpriteOf(art, city, owner);
    const s = art.scale;
    const tier = tierOfSize(city.size | 0 || 1);
    if (pool !== 'none' && city.pool !== false) drawCityPool(art, ctx, x + 16 * s, y + 28 * s, pool, tier);
    for (const l of spr?.meta.lights || []) drawPointLight(art, ctx, x + (16 - spr.ax + l.x) * s, y + (30 - spr.ay + l.y) * s, l.table, l.radius);
    const glow = DAYLIGHT.glow && pool !== 'none' && city.glow !== false;
    if (glow) drawWallGlow(art, ctx, x, y, spr, pool);
    baseCity(ctx, x, y, { ...city, layer: 'sprite', pool: false }, owner);
    if (glow) drawFacadeLight(art, ctx, x, y, spr, pool, tier);
    if (layer === 'all') baseCity(ctx, x, y, { ...city, layer: 'overlay' }, owner);
  };
  art._cityLight = true;
  return art;
}
