// Ground materials of the 3D renderer (docs/R3D.md §4): tiling textures of the ground's fine
// detail -- grass blades, wind-combed straw, packed dirt with pebbles, tilled soil, ripe grain,
// vegetable beds, sand, tundra's moss and lichen, wind-carved snow, reeds, rock (the hell grounds
// borrow the value of sand, lichen and rock) -- painted by code into typed arrays once at start-up
// (about 100-250 ms; no image is loaded, docs/R3D.md §15) and kept in one TEXTURE_2D_ARRAY with
// mipmaps. The terrain shader (shaders.js TERRAIN_FS, matAt/matPair) samples them anchored to the
// map and keeps the broad, plot-scale painting procedural (which ground lies where, the meadow's
// golden patches, a plot's centre pinned to its own ground, fields and roads from the decal); the
// mipmaps fade the detail with distance, so it is not faded by hand with the zoom and does not
// shimmer.
//
// Each layer is RGBA8: the albedo (authored like the shader's colours, which converts them with
// srgb()) and a height (the relief the light picks out, and what decides which of two layers
// shows where they interleave). Every tile wraps -- each stroke, stamp and noise is periodic over
// it -- so it repeats without a seam; rows of crops are laid a whole number to a tile, centred
// where the shader's rows are, so they line up with the rows the field props stand on. A pure
// function of this code (seeded generators): the same on every machine.

import { rng } from "./noise.js";

/** Texels along a tile's side (a power of two: every access wraps with a mask). */
export const MAT_SIZE = 256;

/** The layers in the texture array; shaders.js reads these indices. */
export const MAT = { GRASS: 0, STRAW: 1, DIRT: 2, SOIL: 3, GRAIN: 4, VEG: 5, NOISE: 6, SAND: 7, LICHEN: 8, SNOW: 9, REEDS: 10, ROCK: 11 };

/** Crop rows a tile (shaders.js lays the tile once a plot for crops): grain rows run north-south,
 *  vegetable beds east-west, as in the shader and the field props (fields.js). */
export const GRAIN_ROWS = 18;
export const BED_ROWS = 14;
/** Furrows of tilled soil a tile, running east-west. */
const FURROWS = 28;

// The sun in a tile's frame (u east, v north, up): from the south-west, as config.js LIGHTING.
const SUN = [-0.589, -0.317, 0.743];

/** A tile being painted: albedo (r, g, b) and height per texel, floats; every access wraps. */
class Tile {
  constructor(n) {
    this.n = n;
    this.mask = n - 1;
    this.p = new Float32Array(n * n * 4);
  }

  /** A tapered stroke from its root (x, y) heading `ang` (radians from u toward v), `len` long,
   *  `w0` wide at the root narrowing to `w1`, its colour running from c0 to c1 and its height from
   *  h0 to h1 along it, laid over what is there with cover `a` (an edge a texel wide). With `tint`
   *  c0 and c1 multiply the colour there instead (a blade drawn in the light and shade of the paint
   *  under it, not a colour of its own). */
  stroke(x, y, ang, len, w0, w1, c0, c1, h0, h1, a = 1, tint = false) {
    const ca = Math.cos(ang), sa = Math.sin(ang);
    const lx = ca * len, ly = sa * len;
    // How far from the centre line any cover reaches (texels), and its square (a cheap first test).
    const reach = Math.max(w0, w1) * 0.5 + 0.5, reach2 = reach * reach;
    const i0 = Math.floor(Math.min(x, x + lx) - reach), i1 = Math.ceil(Math.max(x, x + lx) + reach);
    const j0 = Math.floor(Math.min(y, y + ly) - reach), j1 = Math.ceil(Math.max(y, y + ly) + reach);
    const { n, mask, p } = this;
    const il = 1 / len, dw = w1 - w0, dh = h1 - h0;
    const r0 = c0[0], g0 = c0[1], b0 = c0[2], dr = c1[0] - r0, dg = c1[1] - g0, db = c1[2] - b0;
    // Along each row only the span within reach of the centre line (a slanting stroke's box is
    // mostly empty).
    const as = Math.abs(sa), cot = as > 0.05 ? ca / sa : 0, span = as > 0.05 ? reach / as + 1 : 0;
    for (let j = j0; j <= j1; j++) {
      const dy = j + 0.5 - y;
      const row = (j & mask) * n;
      const ty = dy * sa;
      let lo = i0, hi = i1;
      if (span > 0) {
        const xc = x + dy * cot;
        lo = Math.max(i0, Math.floor(xc - span));
        hi = Math.min(i1, Math.ceil(xc + span));
      }
      for (let i = lo; i <= hi; i++) {
        const dx = i + 0.5 - x;
        let t = (dx * ca + ty) * il;
        t = t < 0 ? 0 : t > 1 ? 1 : t;
        const ex = dx - lx * t, ey = dy - ly * t;
        const d2 = ex * ex + ey * ey;
        if (d2 >= reach2) continue;
        const d = Math.sqrt(d2);
        const hw = (w0 + dw * t) * 0.5;
        let cov = hw + 0.5 - d;
        if (cov <= 0) continue;
        cov = (cov > 1 ? 1 : cov) * a;
        const k = (row + (i & mask)) * 4;
        // Rounded across: a blade is highest along its middle.
        const h = (h0 + dh * t) * (0.8 + 0.2 * Math.max(0, 1 - d / (hw > 0.5 ? hw : 0.5)));
        if (tint) {
          p[k] += p[k] * (r0 + dr * t - 1) * cov;
          p[k + 1] += p[k + 1] * (g0 + dg * t - 1) * cov;
          p[k + 2] += p[k + 2] * (b0 + db * t - 1) * cov;
        } else {
          p[k] += (r0 + dr * t - p[k]) * cov;
          p[k + 1] += (g0 + dg * t - p[k + 1]) * cov;
          p[k + 2] += (b0 + db * t - p[k + 2]) * cov;
        }
        p[k + 3] += (h - p[k + 3]) * cov;
      }
    }
  }

  /** An ellipse (radii rx along `ang`, ry across) at (x, y): a low dome of colour c, lit from the
   *  sun's side by `shade` (0 flat), its height h at the top (and `flat` of it at the rim), laid
   *  with cover `a`. */
  blob(x, y, ang, rx, ry, c, h, shade = 0.4, a = 1, flat = 0.3) {
    const ca = Math.cos(ang), sa = Math.sin(ang);
    const R = Math.max(rx, ry) + 1;
    const rm = Math.min(rx, ry);
    const lim = (1 + 1 / rm) * (1 + 1 / rm);
    const i0 = Math.floor(x - R), i1 = Math.ceil(x + R), j0 = Math.floor(y - R), j1 = Math.ceil(y + R);
    const { n, mask, p } = this;
    for (let j = j0; j <= j1; j++) {
      const dy = j + 0.5 - y;
      const row = (j & mask) * n;
      for (let i = i0; i <= i1; i++) {
        const dx = i + 0.5 - x;
        const qx = (dx * ca + dy * sa) / rx, qy = (-dx * sa + dy * ca) / ry;
        const d2 = qx * qx + qy * qy;
        if (d2 >= lim) continue;
        const d = Math.sqrt(d2);
        let cov = (1 - d) * rm + 0.5;
        if (cov <= 0) continue;
        cov = (cov > 1 ? 1 : cov) * a;
        const z = Math.sqrt(Math.max(0, 1 - d2));
        // The dome's slope, turned back to the tile's frame, against the sun.
        const nx = qx * ca - qy * sa, ny = qx * sa + qy * ca;
        const lit = 1 + shade * ((nx * SUN[0] + ny * SUN[1]) * 0.8 + (z - 0.7) * 0.6);
        const k = (row + (i & mask)) * 4;
        p[k] += (c[0] * lit - p[k]) * cov;
        p[k + 1] += (c[1] * lit - p[k + 1]) * cov;
        p[k + 2] += (c[2] * lit - p[k + 2]) * cov;
        p[k + 3] += (h * (flat + (1 - flat) * z) - p[k + 3]) * cov;
      }
    }
  }

  /** Every texel from f(k, i, j, o), which writes [r, g, b, h] into o (k the texel's index). */
  fill(f) {
    const { n, p } = this;
    const o = new Float64Array(4);
    for (let j = 0, k = 0; j < n; j++) {
      for (let i = 0; i < n; i++, k++) {
        f(k, i, j, o);
        p[k * 4] = o[0];
        p[k * 4 + 1] = o[1];
        p[k * 4 + 2] = o[2];
        p[k * 4 + 3] = o[3];
      }
    }
  }
}

/** Value noise periodic over the tile: `cells` lattice cells across, smooth, 0..1 (n * n); with
 *  `out`, `amp` times it is added to out instead. */
function tnoise(n, cells, seed, out = null, amp = 1) {
  const r = rng(seed);
  const g = new Float32Array(cells * cells);
  for (let i = 0; i < g.length; i++) g[i] = r();
  const X0 = new Int32Array(n), X1 = new Int32Array(n), F = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const x = (i + 0.5) * cells / n;
    const xi = Math.floor(x);
    const f = x - xi;
    X0[i] = xi % cells;
    X1[i] = (xi + 1) % cells;
    F[i] = f * f * (3 - 2 * f);
  }
  const add = !!out;
  if (!out) out = new Float32Array(n * n);
  for (let j = 0, k = 0; j < n; j++) {
    const r0 = X0[j] * cells, r1 = X1[j] * cells, fy = F[j];
    for (let i = 0; i < n; i++, k++) {
      const fx = F[i];
      const a = g[r0 + X0[i]], b = g[r0 + X1[i]], c = g[r1 + X0[i]], d = g[r1 + X1[i]];
      const top = a + (b - a) * fx, bot = c + (d - c) * fx;
      const v = top + (bot - top) * fy;
      out[k] = add ? out[k] + v * amp : v;
    }
  }
  return out;
}

/** A fractal sum of tnoise octaves (cells doubling), 0..1. */
function tfbm(n, cells, oct, seed) {
  const out = new Float32Array(n * n);
  let amp = 1, sum = 0;
  for (let o = 0; o < oct; o++) {
    tnoise(n, cells << o, seed * 7 + o, out, amp);
    sum += amp;
    amp *= 0.5;
  }
  for (let k = 0; k < out.length; k++) out[k] /= sum;
  return out;
}

/** Cellular noise periodic over the tile (`cells` across): the gap between the nearest and second
 *  nearest feature point, in cells (0 on a cell's edge). */
function tcells(n, cells, seed) {
  const r = rng(seed);
  const pts = new Float32Array(cells * cells * 2);
  for (let i = 0; i < pts.length; i++) pts[i] = r();
  // Per column (and row): the lattice cell, the fraction across it and the three wrapped cells round it.
  const C = new Int32Array(n * 3), F = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const x = (i + 0.5) * cells / n, xi = Math.floor(x);
    F[i] = x - xi;
    for (let o = 0; o < 3; o++) C[i * 3 + o] = (xi + o - 1 + cells) % cells;
  }
  const out = new Float32Array(n * n);
  for (let j = 0, k = 0; j < n; j++) {
    const fy = F[j];
    for (let i = 0; i < n; i++, k++) {
      const fx = F[i];
      let d1 = 9, d2 = 9;
      for (let oy = 0; oy < 3; oy++) {
        const rowc = C[j * 3 + oy] * cells;
        for (let ox = 0; ox < 3; ox++) {
          const c = (rowc + C[i * 3 + ox]) * 2;
          const ex = ox - 1 + pts[c] - fx, ey = oy - 1 + pts[c + 1] - fy;
          const d = ex * ex + ey * ey;
          if (d < d1) { d2 = d1; d1 = d; } else if (d < d2) d2 = d;
        }
      }
      out[k] = Math.sqrt(d2) - Math.sqrt(d1);
    }
  }
  return out;
}

const mix3 = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
/** o's colour = mix(a, b, t) times s, its height h (a fill's texel, without allocating). */
const put = (o, a, b, t, s, h) => {
  o[0] = (a[0] + (b[0] - a[0]) * t) * s;
  o[1] = (a[1] + (b[1] - a[1]) * t) * s;
  o[2] = (a[2] + (b[2] - a[2]) * t) * s;
  o[3] = h;
};
const mul3 = (a, s) => [a[0] * s, a[1] * s, a[2] * s];
/** One of `pal` by the cumulative weights `cuts` (the last is 1). */
const pick = (r, pal, cuts) => {
  const x = r();
  for (let i = 0; i < cuts.length; i++) if (x < cuts[i]) return pal[i];
  return pal[pal.length - 1];
};

/** Tufts over a jittered grid (an even cover without clumps of tufts): `count` tufts of `blades`
 *  strokes each from about one root (`spread` texels), fanned about `ang`; `blade(r, b)` -> the
 *  stroke's arguments for blade b of its tuft (b = 0 starts a tuft). */
function tufts(t, r, count, blades, ang, fan, blade, spread = 1.2) {
  const g = Math.max(1, Math.round(Math.sqrt(count)));
  const cell = t.n / g;
  for (let gy = 0; gy < g; gy++) {
    for (let gx = 0; gx < g; gx++) {
      const x = (gx + r()) * cell, y = (gy + r()) * cell;
      const nb = blades[0] + Math.floor(r() * (blades[1] - blades[0] + 1));
      // Each tuft its own lean and spread (no stamped look).
      const lean = r.gauss(0.14), fn = fan * (0.6 + 0.8 * r());
      for (let b = 0; b < nb; b++) {
        const a = ang + lean + (nb > 1 ? (b / (nb - 1) - 0.5) * fn : 0) + r.gauss(0.12);
        const s = blade(r, b);
        t.stroke(x + r.gauss(spread), y + r.gauss(spread * 0.6), a, s.len, s.w, s.w1 ?? 0.35, s.c0, s.c1, s.h0, s.h1, s.a ?? 1, s.tint);
      }
    }
  }
}

// --- The recipes. Sizes are in texels of a MAT_SIZE tile; the shader lays the meadow's and the
// crops' tiles about once a plot, so 256 texels are about a plot.

/** The meadow's green sward, as the look target paints it: soft and warm, its variety mostly at the
 *  middle scale -- a mottled under-layer, then broad soft upright dabs of lighter and deeper greens
 *  (yellow-green, sap, olive, a little cool green) -- and the grass in it fine and quiet: thin blades
 *  drawn in the light and shade of the paint under them (darker at the root, lit at the tip), a few
 *  bright yellow-green blades catching the sun. */
function grass(t, r) {
  const n = t.n;
  const a = tfbm(n, 4, 3, 11), b = tnoise(n, 10, 12);
  const lo = [0.19, 0.27, 0.12], hi = [0.29, 0.37, 0.15];
  t.fill((k, i, j, o) => {
    const v = a[k] * 0.5 + b[k] * 0.5;
    put(o, lo, hi, v, 1, 0.2 + 0.2 * v);
  });
  const DAB = [[0.36, 0.45, 0.17], [0.26, 0.36, 0.14], [0.33, 0.41, 0.15], [0.26, 0.37, 0.18]];
  const CUTS = [0.32, 0.6, 0.84];
  tufts(t, r, 240, [1, 2], Math.PI / 2, 0.5, (r) => {
    const c = mul3(pick(r, DAB, CUTS), 0.9 + 0.2 * r());
    return { len: 12 + 16 * r(), w: 6 + 5 * r(), w1: 3, c0: mul3(c, 0.94), c1: mul3(c, 1.04), h0: 0.3, h1: 0.45, a: 0.6 };
  }, 3);
  const blades = (count, nb, len, w, root, tip, h0, h1) =>
    tufts(t, r, count, nb, Math.PI / 2, 0.9, (r) => {
      const v = 0.92 + 0.16 * r();
      return { len: len[0] + (len[1] - len[0]) * r(), w: w[0] + (w[1] - w[0]) * r(), c0: mul3(root, v), c1: mul3(tip, v), h0, h1, tint: true };
    }, 1.5);
  blades(1000, [2, 4], [6, 13], [1.3, 2.0], [0.84, 0.86, 0.84], [1.1, 1.12, 0.98], 0.25, 0.75);
  blades(420, [2, 3], [7, 14], [1.3, 1.9], [0.92, 0.94, 0.88], [1.22, 1.22, 1.0], 0.5, 1.0);
  // A few bright blades in the sun.
  tufts(t, r, 120, [1, 3], Math.PI / 2, 0.8, (r) => {
    const c = mix3([0.46, 0.55, 0.2], [0.55, 0.6, 0.24], r());
    return { len: 7 + 6 * r(), w: 1.3 + 0.5 * r(), c0: mul3(c, 0.8), c1: c, h0: 0.6, h1: 1.0 };
  }, 1.5);
}

/** The golden dry grass, as the target paints it: light and smooth, combed along the wind
 *  (east-north-east, about the mean of the shader's gWind) -- a mottled straw under-layer, long soft
 *  dabs of lighter straw and deeper ochre swept along the wind, and fine strokes in the light and
 *  shade of that paint, a few pale straws and green flecks over it. */
function straw(t, r) {
  const n = t.n;
  const a = tfbm(n, 4, 3, 21), b = tnoise(n, 12, 22);
  const lo = [0.44, 0.36, 0.21], hi = [0.55, 0.47, 0.29];
  t.fill((k, i, j, o) => {
    const v = a[k] * 0.6 + b[k] * 0.4;
    put(o, lo, hi, v, 1, 0.2 + 0.2 * v);
  });
  const DAB = [[0.60, 0.52, 0.33], [0.47, 0.38, 0.23], [0.55, 0.46, 0.27], [0.50, 0.44, 0.27]];
  const CUTS = [0.36, 0.62, 0.9];
  tufts(t, r, 220, [1, 2], 0.35, 0.2, (r) => {
    const c = mul3(pick(r, DAB, CUTS), 0.92 + 0.16 * r());
    return { len: 26 + 26 * r(), w: 5 + 4 * r(), w1: 2.5, c0: mul3(c, 0.95), c1: mul3(c, 1.04), h0: 0.3, h1: 0.45, a: 0.55 };
  }, 3);
  const strokes = (count, len, w, root, tip, h0, h1) =>
    tufts(t, r, count, [2, 3], 0.35, 0.3, (r) => {
      const v = 0.94 + 0.12 * r();
      return { len: len[0] + (len[1] - len[0]) * r(), w: w[0] + (w[1] - w[0]) * r(), c0: mul3(root, v), c1: mul3(tip, v), h0, h1, tint: true };
    }, 2);
  strokes(700, [14, 30], [1.3, 2.1], [0.88, 0.87, 0.85], [1.08, 1.08, 1.05], 0.3, 0.75);
  strokes(320, [14, 28], [1.2, 1.8], [0.95, 0.94, 0.92], [1.16, 1.15, 1.1], 0.5, 1.0);
  tufts(t, r, 110, [1, 2], 0.35, 0.3, (r) => {
    const green = r() < 0.25;
    const c = green ? [0.36, 0.40, 0.21] : mix3([0.66, 0.58, 0.39], [0.74, 0.66, 0.46], r());
    return { len: 12 + 14 * r(), w: 1.2 + 0.5 * r(), c0: mul3(c, 0.85), c1: c, h0: 0.6, h1: 1.0 };
  }, 2);
}

/** Packed dirt of the roads: mottled tan (lighter dust, darker damp patches), fine grit, a few
 *  hairline cracks, and pebbles -- grey, buff and brown -- each a small dome lit from the sun's side
 *  with a dark rim where it sits in the dirt. */
function dirt(t, r) {
  const n = t.n;
  const a = tfbm(n, 4, 4, 31), b = tnoise(n, 16, 32), cl = tcells(n, 6, 33), w = tnoise(n, 8, 34);
  const lo = [0.47, 0.37, 0.26], hi = [0.65, 0.54, 0.39];
  t.fill((k, i, j, o) => {
    const v = Math.min(1, Math.max(0, (a[k] - 0.5) * 1.6 + 0.5)) * 0.75 + b[k] * 0.25;
    const crack = Math.min(1, (1 - Math.min(1, cl[k] / 0.05)) * Math.max(0, w[k] * 2.2 - 1.1));
    put(o, lo, hi, v, 1 - 0.3 * crack, 0.3 + 0.25 * v - 0.2 * crack);
  });
  for (let k = 0; k < 2600; k++) {
    const dark = r() < 0.55;
    t.blob(r() * n, r() * n, 0, 0.6 + 0.6 * r(), 0.6 + 0.5 * r(), dark ? [0.37, 0.29, 0.21] : [0.71, 0.63, 0.49], dark ? 0.25 : 0.55, 0, 0.6 + 0.3 * r());
  }
  const PEB = [[0.58, 0.55, 0.50], [0.69, 0.64, 0.55], [0.53, 0.45, 0.35], [0.46, 0.44, 0.42]];
  for (let k = 0; k < 110; k++) {
    const x = r() * n, y = r() * n, ang = r() * Math.PI, rx = 2 + 3.2 * r() * r(), ry = rx * (0.6 + 0.35 * r());
    t.blob(x + 0.8, y + 0.45, ang, rx + 1.2, ry + 1.2, [0.30, 0.23, 0.17], 0.3, 0, 0.55);
    t.blob(x, y, ang, rx, ry, mul3(PEB[Math.floor(r() * 4)], 0.9 + 0.2 * r()), 0.8 + 0.2 * r(), 0.55, 1, 0.4);
  }
}

/** Tilled soil: furrows running east-west (FURROWS a tile, wavering), dark in the trough and lighter
 *  on the ridge, broken into clods along the ridges -- each a small dome lit from the sun's side --
 *  with a few pale stones. */
function soil(t, r) {
  const n = t.n;
  const a = tfbm(n, 4, 3, 41), b = tnoise(n, 32, 42);
  const lo = [0.25, 0.20, 0.15], hi = [0.43, 0.35, 0.26];
  t.fill((k, i, j, o) => {
    const f = 0.5 + 0.5 * Math.sin(Math.PI * 2 * (j + 0.5) * FURROWS / n + (a[k] - 0.5) * 3);
    put(o, lo, hi, Math.min(1, f * (0.65 + 0.5 * b[k])), 1, 0.15 + 0.55 * f);
  });
  const rowH = n / FURROWS;
  for (let k = 0; k < 1500; k++) {
    // On a ridge (the sine's crest), give or take.
    const row = Math.floor(r() * FURROWS);
    const y = (row + 0.25) * rowH + r.gauss(rowH * 0.18);
    const x = r() * n;
    const rx = 1.1 + 1.8 * r(), ry = rx * (0.6 + 0.4 * r());
    const c = mix3([0.33, 0.27, 0.20], [0.48, 0.39, 0.29], r());
    t.blob(x, y, r() * Math.PI, rx, ry, c, 0.65 + 0.3 * r(), 0.6, 0.9, 0.5);
  }
  for (let k = 0; k < 40; k++) t.blob(r() * n, r() * n, r() * Math.PI, 0.9 + 1.2 * r(), 0.8 + 0.8 * r(), [0.62, 0.58, 0.52], 0.9, 0.5);
}

/** Ripe grain: GRAIN_ROWS rows a tile running north-south (centred where the shader's and the
 *  props' rows are), each a dense band of upright stalks, gold darkening to the root, crowned with
 *  pale ears; dark gaps between the rows. */
function grain(t, r) {
  const n = t.n;
  const a = tnoise(n, 16, 51);
  const rw = n / GRAIN_ROWS;
  const lo = [0.62, 0.44, 0.17], hi = [0.40, 0.27, 0.10];
  t.fill((k, i, j, o) => {
    const across = Math.abs(((i + 0.5) / rw) % 1 - 0.5) * 2;
    put(o, lo, hi, Math.min(1, across * 1.2) * (0.7 + 0.3 * a[k]), 1, 0.1 + 0.2 * (1 - across));
  });
  const GOLD = [[0.82, 0.62, 0.27], [0.88, 0.70, 0.34], [0.74, 0.54, 0.22], [0.80, 0.66, 0.36]];
  for (let row = 0; row < GRAIN_ROWS; row++) {
    const cx = (row + 0.5) * rw;
    for (let k = 0; k < 150; k++) {
      const x = cx + r.gauss(rw * 0.2), y = r() * n;
      const c = mul3(GOLD[Math.floor(r() * 4)], 0.85 + 0.25 * r());
      const ang = Math.PI / 2 + r.gauss(0.12), len = 8 + 8 * r();
      t.stroke(x, y, ang, len, 1.3 + 0.5 * r(), 0.8, mul3(c, 0.62), c, 0.3, 0.8);
      t.blob(x + Math.cos(ang) * len, y + Math.sin(ang) * len, ang, 2.4 + 0.8 * r(), 1.1 + 0.3 * r(), mix3(c, [0.95, 0.82, 0.5], 0.4), 0.95, 0.45, 1, 0.5);
    }
  }
}

/** Vegetable beds: BED_ROWS rows a tile running east-west (centred where the shader's and the
 *  props' rows are) of leafy plants -- rosettes of rounded leaves in four greens, each lit from the
 *  sun's side over a dark heart -- on dark tilled earth. */
function veg(t, r) {
  const n = t.n;
  const a = tfbm(n, 8, 2, 61);
  const lo = [0.22, 0.18, 0.13], hi = [0.33, 0.27, 0.20];
  t.fill((k, i, j, o) => put(o, lo, hi, a[k], 1, 0.1 + 0.1 * a[k]));
  const GREEN = [[0.30, 0.48, 0.16], [0.40, 0.55, 0.21], [0.24, 0.40, 0.17], [0.35, 0.50, 0.25]];
  const rh = n / BED_ROWS;
  for (let row = 0; row < BED_ROWS; row++) {
    const cy = (row + 0.5) * rh;
    for (let x = r() * 4; x < n; x += 7 + 3 * r()) {
      const px = x, py = cy + r.gauss(rh * 0.06);
      t.blob(px, py, 0, 6.5, 5, [0.12, 0.15, 0.08], 0.35, 0, 0.7);
      const nl = 5 + Math.floor(r() * 3), a0 = r() * Math.PI * 2;
      const base = GREEN[Math.floor(r() * 4)];
      for (let l = 0; l < nl; l++) {
        const ang = a0 + (l / nl) * Math.PI * 2 + r.gauss(0.2), d = 2 + 1.6 * r();
        const c = mul3(base, 0.85 + 0.3 * r());
        t.blob(px + Math.cos(ang) * d, py + Math.sin(ang) * d * 0.8, ang, 3.2 + 1.6 * r(), 1.9 + 0.8 * r(), c, 0.6 + 0.4 * r(), 0.55, 1, 0.45);
      }
    }
  }
}

/** Utility noise for the broad ground logic (not an albedo): r brush-dab tones on a jittered
 *  lattice (6 a tile, each an oriented ellipse of one tone, 0.5 between them; the GLSL mdabs as a
 *  texture), g a fractal noise, b cell edges (24 cells a tile, 0 on an edge), a = g. */
function utility(t, r) {
  const n = t.n;
  const g = tfbm(n, 4, 4, 71), cl = tcells(n, 24, 72);
  const best = new Float32Array(n * n), tone = new Float32Array(n * n).fill(0.5);
  const C = 6, cell = n / C;
  for (let cy = 0; cy < C; cy++) {
    for (let cx = 0; cx < C; cx++) {
      const x = (cx + r()) * cell, y = (cy + r()) * cell;
      const ang = 0.35 + r() * 1.05, s = 0.6 + 0.4 * r(), tn = r();
      const ca = Math.cos(ang), sa = Math.sin(ang);
      const R = Math.ceil(cell * 0.8);
      for (let j = Math.floor(y - R); j <= y + R; j++) {
        for (let i = Math.floor(x - R); i <= x + R; i++) {
          const dx = (i + 0.5 - x) / cell, dy = (j + 0.5 - y) / cell;
          const qx = (ca * dx + sa * dy) / 0.78, qy = (-sa * dx + ca * dy) / 0.3;
          const d2 = qx * qx + qy * qy;
          if (d2 >= 1) continue; // outside the dab (most of its box)
          const e = Math.max(0, (Math.sqrt(d2) - 0.5) / 0.5);
          const w = (1 - e * e * (3 - 2 * e)) * s;
          const k = (j & (n - 1)) * n + (i & (n - 1));
          if (w > best[k]) {
            best[k] = w;
            tone[k] = tn;
          }
        }
      }
    }
  }
  t.fill((k, i, j, o) => {
    o[0] = 0.5 + (tone[k] - 0.5) * best[k];
    o[1] = o[3] = g[k];
    o[2] = Math.min(1, cl[k]);
  });
}

/** Desert sand: pale and fine -- a soft mottling, a grain of light and dark specks, faint wind
 *  ripples running along the dunes' crests (a whole number of them over the tile, about 16 a plot;
 *  the shader adds broader ones on the dunes' lit slopes), and a few small pebbles. */
function sand(t, r) {
  const n = t.n;
  const a = tfbm(n, 4, 3, 81), b = tnoise(n, 24, 82), wob = tnoise(n, 6, 83);
  const lo = [0.69, 0.58, 0.42], hi = [0.80, 0.70, 0.53];
  // Ripple crests across the wave vector (10, 12) a tile: about the dunes' heading.
  const kx = (Math.PI * 2 * 10) / n, ky = (Math.PI * 2 * 12) / n;
  t.fill((k, i, j, o) => {
    const rip = Math.sin(i * kx + j * ky + (wob[k] - 0.5) * 5);
    const v = a[k] * 0.7 + b[k] * 0.3;
    put(o, lo, hi, v, 1 + 0.035 * rip + (r() - 0.5) * 0.07, 0.35 + 0.15 * v + 0.12 * rip);
  });
  for (let k = 0; k < 900; k++) {
    const dark = r() < 0.5;
    t.blob(r() * n, r() * n, 0, 0.55 + 0.4 * r(), 0.55 + 0.4 * r(), dark ? [0.56, 0.45, 0.33] : [0.88, 0.80, 0.64], 0.5, 0, 0.5 + 0.3 * r());
  }
  for (let k = 0; k < 28; k++) {
    const x = r() * n, y = r() * n, rx = 1.2 + 1.6 * r(), ang = r() * Math.PI;
    t.blob(x + 0.4, y + 0.25, ang, rx + 0.7, rx * 0.8 + 0.7, [0.55, 0.44, 0.32], 0.3, 0, 0.4);
    t.blob(x, y, ang, rx, rx * 0.8, mix3([0.62, 0.55, 0.46], [0.72, 0.62, 0.5], r()), 0.85, 0.5, 1, 0.4);
  }
}

/** Tundra: a mosaic of low clumps -- moss, rust dwarf shrub, pale lichen, slate grey, all near one
 *  value -- each a few lumps lit from the sun's side, packed over a darker peat with a few stones. */
function lichen(t, r) {
  const n = t.n;
  const a = tfbm(n, 4, 3, 91);
  const lo = [0.37, 0.38, 0.31], hi = [0.45, 0.45, 0.37];
  t.fill((k, i, j, o) => put(o, lo, hi, a[k], 1, 0.15 + 0.15 * a[k]));
  const PAL = [[0.42, 0.45, 0.32], [0.48, 0.42, 0.32], [0.55, 0.56, 0.48], [0.40, 0.41, 0.39]];
  const CUTS = [0.38, 0.6, 0.84];
  // Clumps, each a few lumps of one colour, the lumps varying a little (cushions of moss).
  const g = 15, cell = n / g;
  for (let gy = 0; gy < g; gy++) {
    for (let gx = 0; gx < g; gx++) {
      const x = (gx + r()) * cell, y = (gy + r()) * cell;
      const c = mul3(pick(r, PAL, CUTS), 0.92 + 0.16 * r());
      const lumps = 4 + Math.floor(r() * 4), size = 5 + 5 * r();
      for (let l = 0; l < lumps; l++) {
        const rx = size * (0.5 + 0.4 * r());
        t.blob(x + r.gauss(size * 0.55), y + r.gauss(size * 0.55), r() * Math.PI, rx, rx * (0.7 + 0.3 * r()), mul3(c, 0.94 + 0.12 * r()), 0.5 + 0.4 * r(), 0.12, 1, 0.7);
      }
    }
  }
  for (let k = 0; k < 30; k++) t.blob(r() * n, r() * n, r() * Math.PI, 1.2 + 1.8 * r(), 1 + 1.2 * r(), [0.52, 0.52, 0.50], 0.9, 0.5);
}

/** Snow carved by the wind: warm white, soft blue and lilac shadow streaks swept along the wind (as
 *  the shader's gWind on the prairie, so it runs on across a border), thin wind-cut ridges lit on
 *  their sunward side. */
function snow(t, r) {
  const n = t.n;
  const a = tfbm(n, 4, 3, 101);
  const lo = [0.84, 0.87, 0.92], hi = [0.95, 0.95, 0.95];
  t.fill((k, i, j, o) => put(o, lo, hi, a[k], 1, 0.3 + 0.2 * a[k]));
  const PAL = [[0.74, 0.80, 0.92], [0.80, 0.78, 0.90], [0.97, 0.96, 0.94], [0.86, 0.89, 0.95]];
  tufts(t, r, 160, [1, 2], 0.35, 0.2, (r) => {
    const c = PAL[Math.floor(r() * 4)];
    return { len: 30 + 40 * r(), w: 4 + 5 * r(), w1: 2, c0: c, c1: c, h0: 0.35, h1: 0.5, a: 0.4 };
  }, 4);
  tufts(t, r, 260, [1, 2], 0.35, 0.25, (r) => {
    const c = r() < 0.5 ? [0.70, 0.76, 0.90] : [0.99, 0.98, 0.96];
    return { len: 10 + 22 * r(), w: 1.0 + 0.7 * r(), w1: 0.4, c0: c, c1: c, h0: 0.8, h1: 0.7, a: 0.6 };
  }, 3);
}

/** Marsh: reed beds -- tufts of stiff upright blades in straw-olive, dark sedge, bronze and green --
 *  over wet dark peat. */
function reeds(t, r) {
  const n = t.n;
  const a = tfbm(n, 4, 3, 111);
  const lo = [0.15, 0.17, 0.12], hi = [0.24, 0.26, 0.17];
  t.fill((k, i, j, o) => put(o, lo, hi, a[k], 1, 0.1 + 0.1 * a[k]));
  const PAL = [[0.42, 0.40, 0.24], [0.22, 0.28, 0.20], [0.36, 0.31, 0.19], [0.30, 0.37, 0.20]];
  const CUTS = [0.3, 0.6, 0.8];
  const pass = (count, val, h0, h1) => {
    let hue = PAL[0];
    tufts(t, r, count, [3, 6], Math.PI / 2, 0.45, (r, b) => {
      if (b === 0) hue = mul3(pick(r, PAL, CUTS), val[0] + (val[1] - val[0]) * r());
      const c = mul3(hue, 0.9 + 0.2 * r());
      return { len: 13 + 13 * r(), w: 1.4 + 1.0 * r(), c0: mul3(c, 0.65), c1: mul3(c, 1.15), h0, h1 };
    }, 2.2);
  };
  pass(260, [0.75, 0.95], 0.3, 0.75);
  pass(200, [0.95, 1.2], 0.5, 1.0);
}

/** Rock: grey stone broken into facets of slightly different tone, hairline cracks between them,
 *  a fine grain, and spots of ochre and pale green lichen. */
function rock(t, r) {
  const n = t.n;
  const a = tfbm(n, 4, 4, 121), cl = tcells(n, 7, 122), w = tnoise(n, 6, 123);
  const lo = [0.30, 0.29, 0.29], hi = [0.46, 0.44, 0.42];
  t.fill((k, i, j, o) => {
    const crack = (1 - Math.min(1, cl[k] / 0.06)) * Math.min(1, Math.max(0, w[k] * 2 - 0.5));
    put(o, lo, hi, a[k], (1 - 0.35 * crack) * (1 + (r() - 0.5) * 0.08), 0.4 + 0.3 * a[k] - 0.3 * crack);
  });
  for (let k = 0; k < 70; k++) {
    const rx = 8 + 14 * r();
    t.blob(r() * n, r() * n, r() * Math.PI, rx, rx * (0.5 + 0.4 * r()), mul3(mix3([0.33, 0.32, 0.33], [0.47, 0.45, 0.42], r()), 1), 0.55 + 0.3 * r(), 0.25, 0.35, 0.85);
  }
  const LICH = [[0.62, 0.55, 0.32], [0.52, 0.56, 0.42], [0.66, 0.64, 0.56]];
  for (let k = 0; k < 140; k++) {
    const rx = 0.8 + 2.2 * r() * r();
    t.blob(r() * n, r() * n, r() * Math.PI, rx, rx * (0.6 + 0.4 * r()), LICH[Math.floor(r() * 3)], 0.6, 0.3, 0.8, 0.8);
  }
}

const RECIPES = [
  [MAT.GRASS, grass],
  [MAT.STRAW, straw],
  [MAT.DIRT, dirt],
  [MAT.SOIL, soil],
  [MAT.GRAIN, grain],
  [MAT.VEG, veg],
  [MAT.NOISE, utility],
  [MAT.SAND, sand],
  [MAT.LICHEN, lichen],
  [MAT.SNOW, snow],
  [MAT.REEDS, reeds],
  [MAT.ROCK, rock],
];

/** Paints every layer: {size, layers, data (RGBA8, layer after layer), mean (rgb a layer, the
 *  albedo's mean: the shader scales a material by its broad colour over this), ms, times (ms a
 *  layer)}. */
export function bakeMaterials(size = MAT_SIZE) {
  const t0 = performance.now();
  const layers = RECIPES.length;
  const data = new Uint8Array(size * size * 4 * layers);
  const mean = new Float32Array(layers * 3);
  const times = [];
  for (const [layer, paint] of RECIPES) {
    const tl = performance.now();
    const t = new Tile(size);
    paint(t, rng(1000 + layer));
    const p = t.p, o = layer * size * size * 4;
    let mr = 0, mg = 0, mb = 0;
    for (let k = 0; k < p.length; k += 4) {
      mr += p[k];
      mg += p[k + 1];
      mb += p[k + 2];
    }
    // (A clamped view rounds and clamps each value to a byte.)
    const bytes = new Uint8ClampedArray(data.buffer, o, p.length);
    for (let k = 0; k < p.length; k++) bytes[k] = p[k] * 255;
    const nn = size * size;
    mean.set([mr / nn, mg / nn, mb / nn], layer * 3);
    times.push(performance.now() - tl);
  }
  return { size, layers, data, mean, ms: performance.now() - t0, times };
}

/** The layers on the GPU: a TEXTURE_2D_ARRAY (RGBA8, REPEAT, trilinear, mipmapped, a little
 *  anisotropic filtering where offered), with the bake's mean colours and costs. */
export function materialTexture(gl, baked = bakeMaterials()) {
  const t0 = performance.now();
  const { size, layers, data } = baked;
  const tex = gl.createTexture();
  const T = gl.TEXTURE_2D_ARRAY;
  gl.bindTexture(T, tex);
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  gl.texImage3D(T, 0, gl.RGBA8, size, size, layers, 0, gl.RGBA, gl.UNSIGNED_BYTE, data);
  gl.texParameteri(T, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
  gl.texParameteri(T, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(T, gl.TEXTURE_WRAP_S, gl.REPEAT);
  gl.texParameteri(T, gl.TEXTURE_WRAP_T, gl.REPEAT);
  const an = gl.getExtension("EXT_texture_filter_anisotropic");
  if (an) gl.texParameterf(T, an.TEXTURE_MAX_ANISOTROPY_EXT, Math.min(4, gl.getParameter(an.MAX_TEXTURE_MAX_ANISOTROPY_EXT)));
  gl.generateMipmap(T);
  gl.bindTexture(T, null);
  // Bytes on the GPU: the levels add a third.
  const bytes = Math.round(size * size * 4 * layers * 4 / 3);
  return { tex, mean: baked.mean, layers, bakeMs: baked.ms, uploadMs: performance.now() - t0, bytes };
}
