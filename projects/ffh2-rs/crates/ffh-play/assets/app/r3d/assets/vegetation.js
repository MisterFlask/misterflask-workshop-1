// vegetation.js: the trees and plants of the 3D map and the catalog's `forest` and `flora` entries
// (docs/R3D.md "Asset recipes").
//
// Visual language: every tree is a readable silhouette first -- a visible trunk with a root flare
// sunk below the ground, a few limbs, and a canopy built from clusters of lumpy, flat-bottomed leaf
// masses (never one sphere) with gaps and overhangs. Foliage is painted in two to three tones baked
// into the vertices (a cool dark underside, a mid body, a warm sunlit crown) by height and normal,
// with soft spherical normals per mass and per crown so the light reads as broad painted masses.
// Conifers are ragged, drooping, star-edged tiers; the ancient trees are silver-barked, buttressed
// giants whose crowns are cedar-like tiers of broad foliage plates (dark teal below, lit sage-gold on
// top) with a few lantern-fruits; jungle emergents hold flat umbrella plates over banana-leaved
// understory; willows weep in curtains; burnt and blighted woods are black,
// clawed snags with sparse embers. Variety stays inside each identity: size, lean, mass count and a
// small hue drift per variant. Colours come from palette.js (PAL) plus a few vegetation tones below.
//
// Recipes (names are a contract): tree.oak tree.birch tree.pine (param "snow") tree.pine.snow
// tree.jungle (variants 4-5: understory) tree.palm tree.ancient tree.dead tree.blighted (the infernal
// woods) tree.willow (marsh)
// tree.sapling (param "broad" | "pine" | "snow" | "ancient"; none: broadleaf variants 0-3, conifer
// 4-5) tree.sapling.pine tree.sapling.ancient bush shrub.dry reeds plant.fern plant.mushroom
// plant.mushroom.glow plant.grass (param "dry": golden straw) plant.flowers. The
// ".snow"/".pine"/".ancient" aliases predate params on flora and undergrowth entries; they stay as
// names other modules may use. The ground clutter (grass, flowers) casts no shadow-map shadow.

import { recipe } from "./registry.js";
import { extendCatalog } from "./catalog.js";
import { MAT, FLOATS } from "../mesh.js";
import { PAL, pal, jitter } from "./palette.js";
import { noise3 } from "../noise.js";
import { vadd, vsub, vscale, vdot, vcross, vlen, vnorm, vlerp, clamp, smoothstep } from "../math.js";
import { FEAT, GROUND, isHell } from "../world.js";

const TAU = Math.PI * 2;

// --- colours --------------------------------------------------------------------------------------
// Foliage ramps run deep (cool, shadowed underside) -> dark -> mid -> light (warm, sunlit crown).

const OAK = ["#0f1f19", "#1c3a26", "#33562c", "#6e8c3e"];
const OAK_AUTUMN = ["#16261e", "#2c4127", "#4e5c2e", "#868340"];
const BIRCH = ["#1a3222", "#2e4f30", "#527a3a", "#92ac58"];
const PINE = ["#0b1e17", "#163522", "#284f33", "#5a7e48"];
const JUNGLE = ["#08191a", "#112e22", "#1d4630", "#456a3c"];
const WILLOW = ["#1b2a23", "#2e4330", "#4b6244", "#7e8e68"];
const ANCIENT = ["#0b1d1f", "#15322f", "#244840", "#486a53", "#aab27e"];
const BLIGHT = ["#120a0e", "#24121a", "#3a1a1e", "#6a3424"];
const BUSH = ["#0e1f13", "#1b3519", "#2f4e21", "#5f7e30"];
const PALM = ["#1a3024", "#28442a", "#3b5a33", "#687a45"];
const SAGE = ["#222b24", "#344234", "#4c5e46", "#6c7e5c"];
const SNOW = "#f0f4f6";
const SNOW_SHADE = "#c4d0dc";
const ASH = "#6a645f";
const MOSS = "#3d4a35";
const BARK_JUNGLE = "#6e6455";
const BARK_ANCIENT = "#74766f";
const BARK_BLIGHT = "#2a1a1c";
const BARK_PALM = "#7a6a50";
const BARK_WILLOW = "#4f4636";

const lin = (c) => (typeof c === "string" ? pal(c) : c);
const ramp = (stops, t) => {
  const s = clamp(t, 0, 0.9999) * (stops.length - 1);
  const i = Math.floor(s);
  return vlerp(stops[i], stops[i + 1], s - i);
};
const WHITE = [1, 1, 1];

// --- vertex-level helpers -------------------------------------------------------------------------

function eachVert(mb, from, f) {
  const v = mb.v;
  for (let i = from * FLOATS; i < v.length; i += FLOATS) f(v, i);
}

/** A smooth surface over a grid of points `P` (rows of equal length) with per-point colours `K`:
 *  quads between neighbouring rows, normals averaged from the faces and oriented away from `ref`
 *  (or toward `up`), each triangle wound counter-clockwise about its normal (the prop shader
 *  flips the normals of back faces, so winding and normal must agree); `wrap` closes each row
 *  into a ring. Degenerate triangles (fans at an apex) are skipped. */
function surface(mb, P, K, { wrap = false, ref = null, up = null } = {}) {
  const R = P.length, N = P[0].length;
  const cols = wrap ? N : N - 1;
  const nrm = P.map((row) => row.map(() => [0, 0, 0]));
  const tris = [];
  const add = (a, b, c) => {
    const pa = P[a[0]][a[1]], pb = P[b[0]][b[1]], pc = P[c[0]][c[1]];
    let n = vcross(vsub(pb, pa), vsub(pc, pa));
    if (vlen(n) < 1e-12) return;
    let flip = false;
    if (ref) {
      const cen = [(pa[0] + pb[0] + pc[0]) / 3 - ref[0], (pa[1] + pb[1] + pc[1]) / 3 - ref[1], (pa[2] + pb[2] + pc[2]) / 3 - ref[2]];
      flip = vdot(n, cen) < 0;
    } else if (up) flip = vdot(n, up) < 0;
    if (flip) n = vscale(n, -1);
    for (const [ri, ci] of [a, b, c]) nrm[ri][ci] = vadd(nrm[ri][ci], n);
    // The winding follows the normal: the shader flips normals of back faces.
    tris.push(flip ? [a, c, b] : [a, b, c]);
  };
  for (let i = 0; i + 1 < R; i++) {
    for (let j = 0; j < cols; j++) {
      const j1 = (j + 1) % N;
      add([i, j], [i + 1, j], [i + 1, j1]);
      add([i, j], [i + 1, j1], [i, j1]);
    }
  }
  for (const t of tris) for (const [ri, ci] of t) mb.vert(P[ri][ci], vnorm(nrm[ri][ci]), K[ri][ci]);
}

/** One irregular leaf mass: a noisy blob, squashed, its underside pressed flat (painted clouds of
 *  leaves have flat bellies), normals bent out from its own centre by `self`. `tone` paints the
 *  mass's own light into its vertices (0..1: a darker belly, a lighter top), so each clump of a
 *  crown reads as a clump. Returns its mark. */
function leafMass(mb, c, rad, { detail = 1, amp = 0.3, squash = 0.8, flat = 0.4, seed = 0, freq = 1.6, self = 0.5, tone = 0 } = {}) {
  const m = mb.mark();
  mb.part(() => {
    mb.translate(c[0], c[1], c[2]);
    mb.blob(rad, { detail, amp, squash, noise: (v) => noise3(v[0] * freq + seed, v[1] * freq + seed * 0.37 + 0.5, v[2] * freq - seed * 0.71) });
  });
  if (flat > 0) {
    const cut = c[1] - rad * squash * (1 - flat);
    eachVert(mb, m, (v, i) => {
      if (v[i + 1] < cut) v[i + 1] = cut + (v[i + 1] - cut) * 0.3;
    });
  }
  if (self > 0) mb.sphereNormals(m, [c[0], c[1] - rad * 0.3, c[2]], self);
  if (tone > 0) {
    eachVert(mb, m, (v, i) => {
      const k = 1 - 0.5 * tone + 0.68 * tone * smoothstep(-0.85, 0.75, (v[i + 1] - c[1]) / (rad * squash));
      v[i + 6] *= k;
      v[i + 7] *= k;
      v[i + 8] *= k;
    });
  }
  return m;
}

/** Paints foliage from `from` on: tone by height (`lo`..`hi`) and normal, broken by brush noise,
 *  through the colour ramp `cols`, multiplied into each vertex's colour (a mass's own jitter);
 *  occlusion deepens toward the crown's heart (`center`, `rad`). */
function shadeFoliage(mb, from, { lo, hi, cols, center = null, rad = 1, bias = 0, hw = 0.42, upw = 0.3, nw = 0.2, nf = 11, seed = 0, aoMin = 0.5 }) {
  const stops = cols.map(lin);
  eachVert(mb, from, (v, i) => {
    const x = v[i], y = v[i + 1], z = v[i + 2];
    const h = clamp((y - lo) / (hi - lo), 0, 1);
    const t = 0.05 + hw * h + upw * v[i + 4] + nw * noise3(x * nf + seed, y * nf + 3.1, z * nf - seed) + bias;
    const c = ramp(stops, t);
    v[i + 6] *= c[0];
    v[i + 7] *= c[1];
    v[i + 8] *= c[2];
    if (center) {
      const d = Math.hypot(x - center[0], (y - center[1]) * 1.25, z - center[2]) / rad;
      v[i + 12] *= aoMin + (1 - aoMin) * smoothstep(0.25, 0.95, d);
    }
  });
}

/** Wind: sway rising with height (`lo`..`hi`) and with distance from the trunk axis (`w`). */
function swayFoliage(mb, from, lo, hi, w, max = 1, ax = 0, az = 0) {
  eachVert(mb, from, (v, i) => {
    const h = clamp((v[i + 1] - lo) / (hi - lo), 0, 1);
    const d = clamp(Math.hypot(v[i] - ax, v[i + 2] - az) / w, 0, 1);
    v[i + 10] = Math.max(v[i + 10], max * clamp(0.15 + 0.6 * h + 0.35 * d, 0, 1));
  });
}

/** A trunk's foot: a lobed flare from below the ground (y -0.05) up into the trunk of radius `r0`. */
function rootFlare(mb, x, z, r0, { h = 0.05, spread = 2.3, lobes = 4, seed = 0, seg = 10 } = {}) {
  mb.part(() => {
    mb.translate(x, 0, z);
    mb.lathe([[r0 * spread, -0.055], [r0 * spread * 0.8, -0.01], [r0 * 1.4, h * 0.4], [r0 * 1.02, h]], seg, {
      caps: false,
      phase: seed,
      wobble: (a, y, i) => (i < 3 ? (1 - i / 3) * (0.7 * Math.pow(Math.max(0, Math.cos(a * lobes)), 3) - 0.22) : 0),
    });
  });
}

/** A limb from `a` to `b`: out first, then up (elbowed like an oak bough). */
function limb(mb, a, b, r0, r1, seg = 4, up = 0.4) {
  const m = [a[0] + (b[0] - a[0]) * 0.62, a[1] + (b[1] - a[1]) * up, a[2] + (b[2] - a[2]) * 0.62];
  mb.tube([a, m, b], [r0, (r0 + r1) * 0.5, r1], seg);
}

/** Bark painted toward `foot` near the ground (moss, soot) and toward `up` on upward-facing
 *  surfaces (ash, lichen), mixed into the vertices' colour. */
function shadeBark(mb, from, { foot = null, footAmt = 0.6, y0 = 0, y1 = 0.08, up = null, upAmt = 0.5 }) {
  const F = foot ? lin(foot) : null, U = up ? lin(up) : null;
  eachVert(mb, from, (v, i) => {
    let c = [v[i + 6], v[i + 7], v[i + 8]];
    if (F) c = vlerp(c, F, footAmt * (1 - smoothstep(y0, y1, v[i + 1])));
    if (U) c = vlerp(c, U, upAmt * clamp(v[i + 4], 0, 1));
    v[i + 6] = c[0];
    v[i + 7] = c[1];
    v[i + 8] = c[2];
  });
}

// --- broadleaf crowns -------------------------------------------------------------------------------

/** A broadleaf crown: masses around `C` (half-width `W`, half-height `hc`), with limbs from the
 *  trunk point `fork` to the outer ones. `layout` lists [angle, dist (of W), dy (of hc), rad (of W),
 *  detail]; `tone` paints each mass's own light, `amp` and `freq` its lumpiness (leafMass). Returns
 *  the foliage mark (the limbs come before it). */
function crown(mb, r, { C, W, hc, fork, layout, squash = 0.78, amp = 0.32, flat = 0.4, limbR = 0.01, limbs = 3, bark, nTufts = 0, self = 0.55, tone = 0, freq = 1.6 }) {
  // Limbs first (wood), toward the outer masses.
  const outer = layout.filter((m) => m[1] > 0.3).slice(0, limbs);
  mb.color = bark;
  mb.mat = MAT.MATTE;
  for (const [a, d, dy] of outer) {
    const end = [C[0] + Math.cos(a) * d * W * 0.8, C[1] + dy * hc - hc * 0.15, C[2] + Math.sin(a) * d * W * 0.8];
    limb(mb, fork, end, limbR, limbR * 0.4, 4, 0.3);
  }
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  const cores = [];
  for (const [a, d, dy, rad, detail] of layout) {
    mb.color = jitter(WHITE, r, 0.07);
    const c = [C[0] + Math.cos(a) * d * W, C[1] + dy * hc, C[2] + Math.sin(a) * d * W];
    leafMass(mb, c, rad * W, { detail: detail ?? 1, amp, squash, flat, seed: r.range(0, 50), self, tone, freq });
    cores.push([c, rad * W, squash]);
  }
  if (nTufts) tufts(mb, r, cores, nTufts, { tone });
  return m0;
}

/** A ring of `n` outer masses with jittered angles, distances, heights and sizes. */
function ringLayout(r, n, { dist = [0.5, 0.65], dy = [-0.4, 0.35], rad = [0.42, 0.55], phase = r.range(0, TAU), detail = 1 } = {}) {
  const out = [];
  for (let k = 0; k < n; k++) {
    out.push([phase + (k / n) * TAU + r.range(-0.35, 0.35), r.range(dist[0], dist[1]), r.range(dy[0], dy[1]), r.range(rad[0], rad[1]), detail]);
  }
  return out;
}

/** Small leaf tufts scattered over the outside of the core masses (`cores`: [[c, rad, squash]]):
 *  they scallop the crown's silhouette, as painted foliage reads. Tufts buried inside another
 *  core are skipped. */
function tufts(mb, r, cores, n, { size = [0.32, 0.46], minUp = -0.25, out = 0.82, tone = 0 } = {}) {
  const total = cores.reduce((a, c) => a + c[1], 0);
  let made = 0;
  for (let k = 0; k < n * 3 && made < n; k++) {
    let t = r() * total;
    let core = cores[0];
    for (const c of cores) if ((t -= c[1]) <= 0) { core = c; break; }
    const [c, rad, sq] = core;
    const y = r.range(minUp, 1);
    const a = r.range(0, TAU);
    const ring = Math.sqrt(1 - y * y);
    const p = [c[0] + Math.cos(a) * ring * rad * out, c[1] + y * rad * sq * out, c[2] + Math.sin(a) * ring * rad * out];
    const buried = cores.some((o) => o !== core && Math.hypot(p[0] - o[0][0], (p[1] - o[0][1]) / o[2], p[2] - o[0][2]) < o[1] * 0.75);
    if (buried) continue;
    mb.color = jitter(WHITE, r, 0.08);
    leafMass(mb, p, rad * r.range(size[0], size[1]), { detail: 0, amp: 0.15, squash: 0.85, flat: 0.3, seed: r.range(0, 50), self: 0.4, tone });
    made++;
  }
}

/** A canopy plate (the mallorns' tiers, umbrella crowns): a broad, flat-bellied, gently domed disc of
 *  foliage around `c` (its waist) of radius `R` and thickness `T`, its rim scalloped into irregular
 *  lobes and drooping a little, its top lumpy. Smooth-shaded (a lathe), 10 * `seg` triangles.
 *  Returns its mark. */
function plate(mb, r, c, R, T, { seg = 11, lobes = 5, scallop = 0.2, lump = 0.3, droop = 0.3, dome = 1 } = {}) {
  const m = mb.mark();
  const ph = r.range(0, TAU), seed = r.range(0, 50), lp = r.range(0, TAU);
  const prof = [[0, -0.34], [0.68, -0.5], [1, -0.08], [0.8, 0.32 * dome], [0.4, 0.52 * dome], [0, 0.58 * dome]].map(([x, y]) => [x * R, y * T]);
  const lobe = (a) => scallop * (0.6 * Math.cos(lobes * a + lp) + 0.6 * noise3(Math.cos(a) * 1.7 + seed, Math.sin(a) * 1.7, seed * 0.3));
  mb.part(() => {
    mb.translate(c[0], c[1], c[2]);
    mb.lathe(prof, seg, { caps: false, phase: ph, wobble: (a, y, i) => (i === 0 || i === prof.length - 1 ? 0 : lobe(a) * (i === 2 || i === 3 ? 1 : 0.55)) });
  });
  mb.displace(m, (x, y, z) => {
    const d = Math.hypot(x - c[0], z - c[2]) / R;
    const up = clamp((y - c[1]) / T + 0.2, 0, 1);
    const n = noise3(x * 12 + seed, z * 12 - seed, seed) + 0.5 * noise3(x * 27 - seed, z * 27 + seed, seed * 0.7);
    return [0, T * (lump * up * n - droop * d * d), 0];
  });
  return m;
}

// --- tree.oak -----------------------------------------------------------------------------------------

/** The common broadleaf: a stout trunk with a root flare forking into a few boughs under a broad,
 *  full crown of separate rounded clumps (each lit on top, dark at its belly) with overhangs and
 *  gaps, a skirt of smaller clumps reaching down round the boughs, scalloped by small tufts. Shapes
 *  cycle: broad dome, twin-stemmed (forked low, two lobes), leaning with a heavy low bough, split
 *  crown. */
function buildOak(mb, r, variant, cols = OAK) {
  const shape = variant % 4;
  const H = r.range(0.3, 0.355) * (shape === 1 ? 1.06 : 1);
  const W = r.range(0.134, 0.146) * (shape === 0 ? 1.05 : 1);
  const forkY = H * (shape === 1 ? r.range(0.15, 0.19) : r.range(0.19, 0.24));
  const la = r.range(0, TAU);
  const ln = (shape === 2 ? r.range(0.025, 0.035) : r.range(0.004, 0.015));
  const lx = Math.cos(la) * ln, lz = Math.sin(la) * ln;
  const tr = r.range(0.018, 0.022);
  const bark = jitter(pal("barkOak"), r, 0.05);
  const w0 = mb.mark();
  mb.color = bark;
  rootFlare(mb, 0, 0, tr, { h: 0.04, lobes: r.int(3, 5), seed: r.range(0, 6) });
  const wig = () => r.range(-0.005, 0.005);
  const fork = [lx, forkY, lz];
  mb.tube([[0, 0.03, 0], [lx * 0.3 + wig(), forkY * 0.55, lz * 0.3 + wig()], [lx, forkY, lz], [lx * 1.2, forkY + 0.06, lz * 1.2]], [tr, tr * 0.85, tr * 0.72, tr * 0.45], 6);
  const hc = (H - Math.max(forkY, H * 0.22)) * 0.5;
  const C = [lx * 1.6, H - hc * 0.95, lz * 1.6];
  let layout;
  if (shape === 0) {
    // Broad dome with a scalloped, overhanging skirt.
    layout = [[0, 0, 0.3, 0.64], ...ringLayout(r, 4, { dist: [0.56, 0.7], dy: [-0.5, 0.05], rad: [0.47, 0.55] })];
  } else if (shape === 1) {
    // Twin-stemmed: forked low into two leaders, each carrying its own lobe of the crown.
    const a = r.range(0, TAU);
    layout = [[a, 0.55, 0.15, 0.58], [a + Math.PI, 0.55, 0.0, 0.58], [a + 0.9, 0.7, -0.4, 0.46], [a + Math.PI + 0.9, 0.65, 0.4, 0.48], [a + Math.PI / 2, 0.15, 0.6, 0.48]];
  } else if (shape === 2) {
    // A heavy bough reaching out on the leaning side, low.
    layout = [[0, 0, 0.25, 0.62], [la + r.range(-0.3, 0.3), 0.9, -0.4, 0.52], [la + Math.PI + r.range(-0.5, 0.5), 0.55, 0.4, 0.5], ...ringLayout(r, 2, { dist: [0.55, 0.7], dy: [-0.35, 0.25], rad: [0.44, 0.52] })];
  } else {
    // Split crown: two lobes with a gap between them.
    const a = r.range(0, TAU);
    layout = [[a, 0.55, 0.3, 0.6], [a + Math.PI, 0.6, 0.05, 0.6], [a + 1.5, 0.72, -0.45, 0.48], [a + Math.PI + 1.3, 0.5, 0.65, 0.5]];
  }
  // The skirt: small clumps low round the boughs, so the crown reaches down full instead of
  // sitting on the trunk like a ball on a stick.
  layout.push(...ringLayout(r, shape === 3 ? 3 : 2, { dist: [0.62, 0.8], dy: [-0.8, -0.58], rad: [0.34, 0.42], detail: 0 }));
  const m0 = crown(mb, r, { C, W, hc, fork, layout, limbR: tr * (shape === 1 ? 0.75 : 0.55), limbs: shape === 1 ? 2 : 3, bark, squash: 0.76, nTufts: 5, self: 0.5, tone: 0.6, amp: 0.38, freq: 2.3 });
  shadeBark(mb, w0, { foot: MOSS, footAmt: 0.55, y0: -0.02, y1: 0.07 });
  mb.heightAO(w0, -0.02, forkY, 0.55);
  mb.sphereNormals(m0, [C[0], C[1] - hc * 0.6, C[2]], 0.45);
  // Each tree drifts a little in hue inside the identity (bluer, greener, more olive).
  const drift = [[0.94, 1, 1.08], [1, 1, 1], [1.06, 1.02, 0.9], [0.97, 1.03, 0.98]][r.int(0, 3)];
  shadeFoliage(mb, m0, { lo: forkY, hi: C[1] + hc * 1.2, cols: cols.map((c) => lin(c).map((x, q) => x * drift[q])), center: C, rad: W * 1.1, seed: r.range(0, 9), hw: 0.5 });
  swayFoliage(mb, m0, forkY, H, W, 0.9, C[0], C[2]);
}

recipe("tree.oak", {
  variants: 8,
  build(mb, r, { variant }) {
    // One variant in eight drifts toward autumn (a small warm accent inside the identity).
    buildOak(mb, r, variant, variant === 5 ? OAK_AUTUMN : OAK);
  },
});

// --- tree.birch ---------------------------------------------------------------------------------------

/** Pale slender stems (one, or a clump of two or three leaning apart) with dark bark marks, under a
 *  narrow, airy, drooping oval crown of loosely packed masses in a lighter, yellower green. */
function buildBirch(mb, r, variant) {
  const stems = variant === 2 || variant === 5 ? 2 : variant === 4 ? 3 : 1;
  const H = r.range(0.33, 0.39);
  const w0 = mb.mark();
  const birch = jitter(pal("barkBirch"), r, 0.04);
  const mark = pal("#3a3430");
  const tops = [];
  const a0 = r.range(0, TAU);
  for (let s = 0; s < stems; s++) {
    const a = a0 + (s / stems) * TAU + r.range(-0.4, 0.4);
    const sp = stems > 1 ? r.range(0.035, 0.05) : r.range(0, 0.015);
    const h = H * (s === 0 ? 1 : r.range(0.82, 0.93));
    const bx = stems > 1 ? Math.cos(a) * 0.01 : 0, bz = stems > 1 ? Math.sin(a) * 0.01 : 0;
    const tx = bx + Math.cos(a) * sp, tz = bz + Math.sin(a) * sp;
    const tr = (stems > 1 ? 0.0085 : 0.011) * r.range(0.9, 1.1);
    const n = 7;
    const pts = [], rad = [], colors = [];
    for (let k = 0; k < n; k++) {
      const t = k / (n - 1);
      const bend = Math.sin(t * Math.PI) * 0.006;
      pts.push([bx + (tx - bx) * t * t + bend, -0.05 + (h * 0.8 + 0.05) * t, bz + (tz - bz) * t * t]);
      rad.push(tr * (1.3 - 0.8 * t));
      colors.push(k === 0 ? mark : r() < 0.35 ? vlerp(birch, mark, 0.75) : birch);
    }
    mb.color = birch;
    mb.tube(pts, rad, 5, { colors });
    tops.push({ p: pts[n - 1], low: pts[2], h, s });
  }
  // One airy oval crown over all the stems: wider low, narrowing to a pointed top.
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  const ox = tops.reduce((acc, t) => acc + t.p[0], 0) / stems, oz = tops.reduce((acc, t) => acc + t.p[2], 0) / stems;
  // Airy: separate small clumps spiralling up the stems, with gaps between them where the pale
  // stems and the light show through (never one dense ball).
  const W = [0.082, 0.092, 0.1][stems - 1] * r.range(0.92, 1.08);
  const y0 = Math.max(0.12, H * 0.42), y1 = H - 0.015;
  const lo = y0 - W * 0.5, hi = y1 + W * 0.4;
  const ph = r.range(0, TAU);
  const n = 6;
  for (let k = 0; k < n; k++) {
    const u = k / (n - 1);
    const a = ph + k * 2.4 + r.range(-0.3, 0.3);
    const d = W * (k === n - 1 ? 0.12 : (0.68 - 0.36 * u) * r.range(0.85, 1.1));
    const c = [ox + Math.cos(a) * d, y0 + (y1 - y0) * u, oz + Math.sin(a) * d];
    mb.color = jitter(WHITE, r, 0.08);
    leafMass(mb, c, W * (0.56 - 0.16 * u) * r.range(0.88, 1.1), { detail: 1, amp: 0.36, squash: 0.92, flat: 0.2, seed: r.range(0, 50), self: 0.4, freq: 2.0 });
  }
  const cx = ox, cz = oz;
  const cy = (lo + hi) * 0.5;
  // Twigs hang: the crown's rim droops.
  mb.displace(m0, (x, y, z) => [0, -0.45 * Math.max(0, Math.hypot(x - cx, z - cz) - 0.03) * (y < cy ? 1.4 : 1), 0]);
  mb.sphereNormals(m0, [cx, cy - 0.02, cz], 0.35);
  mb.heightAO(w0, -0.02, H * 0.4, 0.6);
  shadeFoliage(mb, m0, { lo, hi, cols: BIRCH, center: [cx, cy, cz], rad: 0.09, seed: r.range(0, 9), aoMin: 0.6 });
  swayFoliage(mb, m0, lo, hi, 0.08, 1, cx, cz);
}

recipe("tree.birch", { variants: 6, build: (mb, r, { variant }) => buildBirch(mb, r, variant) });

// --- tree.pine ----------------------------------------------------------------------------------------

/** One conifer tier: a star-edged, drooping skirt of `spokes` branch tips around (cx, y, cz) of
 *  radius `rad` and height `tall`, its own colours baked (underside dark, upper face lighter, snow
 *  on the upper face and the tips' tops when `snow`). */
function pineTier(mb, r, { cx, cz, y, rad, tall, spokes, droop, jag, light, cols, snow, snowAmt = 0.7 }) {
  const S = spokes * 2;
  const ph = r.range(0, TAU);
  const apex = [cx, y + tall, cz];
  const rowA = [], rowB = [], rowC = [], rowD = [];
  const kA = [], kB = [], kC = [], kD = [];
  const [deep, dark, mid, lite] = cols.map(lin);
  const sn = lin(SNOW), sns = lin(SNOW_SHADE);
  for (let s = 0; s < S; s++) {
    const tip = s % 2 === 0;
    const a = ph + (s / S) * TAU + (tip ? r.range(-0.18, 0.18) : 0);
    const rr = rad * (tip ? r.range(0.85, 1.15) : 1 - jag * r.range(0.8, 1.2));
    const ca = Math.cos(a), sa = Math.sin(a);
    const dr = droop * (tip ? 1 : 0.45) * (rr / rad);
    rowA.push(apex);
    rowB.push([cx + ca * rr * 0.55, y + tall * 0.24 + (tip ? 0 : tall * 0.06), cz + sa * rr * 0.55]);
    rowC.push([cx + ca * rr, y - dr, cz + sa * rr]);
    rowD.push([cx + ca * rr * 0.32, y + tall * 0.06, cz + sa * rr * 0.32]);
    let cB = vlerp(mid, lite, light);
    let cC = tip ? vlerp(dark, mid, 0.5 + light * 0.5) : vlerp(deep, dark, 0.6);
    let cA = vlerp(mid, lite, 0.4 + light * 0.6);
    if (snow) {
      const lay = r() < snowAmt;
      if (lay) cB = vlerp(sns, sn, 0.4 + 0.6 * light);
      if (tip && r() < snowAmt * 0.5) cC = vlerp(cC, sn, 0.5);
      if (r() < snowAmt) cA = vlerp(cA, sn, 0.7);
    }
    kA.push(cA);
    kB.push(cB);
    kC.push(cC);
    kD.push(vlerp(deep, dark, 0.3));
  }
  surface(mb, [rowA, rowB, rowC, rowD], [kA, kB, kC, kD], { wrap: true, ref: [cx, y + tall * 0.2, cz] });
}

/** A conifer: a straight trunk with a little flare, and ragged, drooping tiers that narrow upward,
 *  slightly offset from each other (never a perfect cone). Shapes: firs (0-4), a slim spruce (5),
 *  scraggly pines with gaps showing the trunk (6, 7). `small` builds a young one. */
function buildPine(mb, r, variant, { snow = false, small = false, cols = PINE } = {}) {
  const type = small ? 0 : variant % 8;
  const slim = type === 5, scraggly = type >= 6;
  const H = small ? r.range(0.14, 0.18) : r.range(0.36, 0.43) * (slim ? 1.05 : scraggly ? 0.95 : 1);
  const R = (slim ? 0.08 : scraggly ? 0.108 : 0.106) * r.range(0.9, 1.1) * (small ? 0.66 : 1);
  const tiers = small ? 3 : scraggly ? 5 : slim ? 7 : r.int(6, 7);
  const base = H * (scraggly ? 0.24 : small ? 0.14 : r.range(0.11, 0.16));
  const lean = [r.range(-0.012, 0.012), 0, r.range(-0.012, 0.012)];
  const top = [lean[0], H, lean[2]];
  const w0 = mb.mark();
  mb.color = jitter(pal("barkPine"), r, 0.05);
  const tr = small ? 0.007 : 0.013;
  if (!small) rootFlare(mb, 0, 0, tr, { h: 0.035, lobes: 4, seed: r.range(0, 6), seg: 8 });
  mb.tube([[0, small ? -0.05 : 0.02, 0], [lean[0] * 0.4, H * 0.45, lean[2] * 0.4], [top[0] * 0.95, H * 0.93, top[2] * 0.95]], [tr, tr * 0.6, tr * 0.2], small ? 4 : 5);
  mb.heightAO(w0, -0.02, base + 0.04, 0.55);
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  const span = H * 0.84 - base;
  for (let k = 0; k < tiers; k++) {
    const t = k / (tiers - 1);
    const y = base + span * Math.pow(t, 0.95) + (scraggly && k > 0 ? r.range(-0.012, 0.012) : 0);
    const rad = R * (1 - t * 0.74) * r.range(0.86, 1.12) * (scraggly && k % 2 === 1 ? 0.8 : 1);
    const tall = (span / (tiers - 1)) * (scraggly ? 1.1 : 1.22) * (1 - 0.15 * t) + 0.012;
    const off = (1 - t) * (scraggly ? 0.016 : 0.008);
    pineTier(mb, r, {
      cx: lean[0] * t + r.range(-off, off),
      cz: lean[2] * t + r.range(-off, off),
      y,
      rad,
      tall,
      spokes: small ? 4 : r.int(5, 7),
      droop: rad * r.range(0.4, 0.55),
      jag: r.range(0.36, 0.5),
      light: 0.15 + 0.85 * t,
      cols,
      snow,
      snowAmt: 0.3 + 0.35 * t,
    });
  }
  // The leader: a thin spire above the top tier.
  mb.color = vlerp(lin(cols[2]), lin(cols[3]), 0.6);
  if (snow) mb.color = vlerp(mb.color, lin(SNOW), 0.5);
  mb.part(() => {
    mb.translate(top[0], H * 0.84, top[2]);
    mb.lathe([[R * 0.18, 0], [R * 0.08, (H * 0.16) * 0.6], [0, H * 0.16 + 0.01]], 5, { caps: false, phase: r.range(0, 6) });
  });
  mb.sphereNormals(m0, [lean[0] * 0.5, H * 0.45, lean[2] * 0.5], 0.4);
  // Painted light: lower tiers sit in the forest's shade, the top catches the sun.
  eachVert(mb, m0, (v, i) => {
    const k = 0.72 + 0.4 * clamp((v[i + 1] - base) / (H - base), 0, 1);
    v[i + 6] *= k;
    v[i + 7] *= k;
    v[i + 8] *= k;
    v[i + 12] *= 0.6 + 0.4 * clamp(Math.hypot(v[i] - lean[0], v[i + 2] - lean[2]) / (R * 0.7), 0, 1);
  });
  swayFoliage(mb, m0, base, H, R, 0.7, 0, 0);
}

recipe("tree.pine", {
  variants: 8,
  build: (mb, r, { variant, param }) => buildPine(mb, r, variant, { snow: param === "snow" }),
});
// The snowy pine under its own name too (the catalog's flora and undergrowth carry no param).
recipe("tree.pine.snow", { variants: 8, build: (mb, r, { variant }) => buildPine(mb, r, variant, { snow: true }) });

// --- tree.jungle --------------------------------------------------------------------------------------

/** A long leaf blade (palm fronds, banana and elephant-ear leaves) from `top` out along the angle `a`:
 *  `L` long, rising by `elev` and falling by `drop` (of L) along a parabola, folded along its spine
 *  (`fold`, of its half-width), `width` (of L) wide at its widest; coloured from `cols` (deep, dark,
 *  mid, light) heart to tip, or `dry` (a dead, straw frond). */
function frondBlade(mb, top, a, L, elev, drop, { width = 0.24, fold = 0.55, cols, dry = null, m = 6, round = 0.7 } = {}) {
  const dir = [Math.cos(a), 0, Math.sin(a)];
  const side = [-Math.sin(a), 0, Math.cos(a)];
  const [deep, dark, mid, lite] = cols;
  const L0 = [], S0 = [], R0 = [], kL = [], kS = [], kR = [];
  for (let j = 0; j < m; j++) {
    const s = j / (m - 1);
    const sp = [top[0] + dir[0] * L * s, top[1] + L * (elev * s - drop * s * s), top[2] + dir[2] * L * s];
    const w = L * width * Math.pow(Math.sin(Math.PI * Math.min(1, 0.15 + s * 0.95)), round);
    const sag = w * fold;
    L0.push([sp[0] + side[0] * w, sp[1] - sag, sp[2] + side[2] * w]);
    S0.push(sp);
    R0.push([sp[0] - side[0] * w, sp[1] - sag, sp[2] - side[2] * w]);
    const c = dry ? vlerp(dry, pal("#5e4a30"), 0.5 * (1 - s)) : s < 0.5 ? vlerp(dark, mid, s * 2) : vlerp(mid, lite, (s - 0.5) * 2);
    kS.push(c);
    kL.push(dry ? c : vlerp(c, deep, 0.35));
    kR.push(dry ? c : vlerp(c, deep, 0.35));
  }
  surface(mb, [L0, S0, R0], [kL, kS, kR], { up: [0, 1, 0] });
}

/** Rainforest trees. Emergents (variants 0-3): a tall, pale, slightly S-curved trunk on thin
 *  buttress fins, holding up a broad, flat-topped umbrella of foliage plates (a big one on top, two
 *  or three lower at its sides, like a kapok), big leaves bursting from under its rim and lianas
 *  hanging. Understory (4, 5): a short stem in a burst of huge, drooping, banana-like leaves around a
 *  dark heart. Deep, cool greens, darker than the temperate woods. */
function buildJungle(mb, r, variant) {
  if (variant % 6 >= 4) return buildJungleUnder(mb, r);
  const H = r.range(0.38, 0.44);
  const W = r.range(0.135, 0.155) * (variant % 3 === 2 ? 1.08 : 1);
  const forkY = H * r.range(0.6, 0.68);
  const la = r.range(0, TAU), ln = r.range(0.006, 0.022);
  const lx = Math.cos(la) * ln, lz = Math.sin(la) * ln;
  const tr = r.range(0.016, 0.019);
  const bark = jitter(pal(BARK_JUNGLE), r, 0.05);
  const w0 = mb.mark();
  mb.color = bark;
  const s1 = r.range(-0.008, 0.008), s2 = r.range(-0.008, 0.008);
  mb.tube([[0, -0.05, 0], [lx * 0.2 + s1, forkY * 0.35, lz * 0.2 + s2], [lx * 0.6 - s1, forkY * 0.75, lz * 0.6 - s2], [lx, forkY + 0.03, lz]], [tr * 1.3, tr, tr * 0.85, tr * 0.6], 6);
  // Buttress fins: thin plates whose outer edge sweeps concavely from high on the trunk to the ground.
  const fins = r.int(4, 5), fph = r.range(0, TAU);
  for (let k = 0; k < fins; k++) {
    const a = fph + (k / fins) * TAU + r.range(-0.3, 0.3);
    const ca = Math.cos(a), sa = Math.sin(a);
    const d = r.range(0.07, 0.095), hf = r.range(0.09, 0.13);
    const inner = [], outer = [], kin = [], kout = [];
    for (let j = 0; j < 4; j++) {
      const u = j / 3;
      const y = hf * (1 - u) * (1 - u) - 0.05 * u;
      const x = tr + (d - tr) * u;
      inner.push([ca * tr * 0.6, y, sa * tr * 0.6]);
      outer.push([ca * x, y, sa * x]);
      kin.push(bark);
      kout.push(vlerp(bark, lin(MOSS), 0.35 * u));
    }
    surface(mb, [inner, outer], [kin, kout], { up: [-sa, 0, ca] });
  }
  // The umbrella's plates: the crown on top, two or three lower ones at its sides.
  const C = [lx * 1.3, H - W * 0.26, lz * 1.3];
  const ph = r.range(0, TAU);
  const plates = [{ a: ph, c: [C[0], C[1], C[2]], R: W * r.range(0.66, 0.72), T: W * 0.3 }];
  const ns = r.int(2, 3);
  for (let k = 0; k < ns; k++) {
    const a = ph + (k / ns) * TAU + r.range(-0.4, 0.4);
    const d = W * r.range(0.62, 0.78);
    plates.push({ a, c: [C[0] + Math.cos(a) * d, C[1] - W * r.range(0.2, 0.42), C[2] + Math.sin(a) * d], R: W * r.range(0.46, 0.54), T: W * 0.24 });
  }
  const fork = [lx, forkY, lz];
  for (const p of plates.slice(1)) limb(mb, fork, [p.c[0], p.c[1] - p.T * 0.3, p.c[2]], tr * 0.6, tr * 0.3, 4, 0.35);
  limb(mb, fork, [C[0], C[1] - plates[0].T * 0.3, C[2]], tr * 0.65, tr * 0.35, 4, 0.5);
  shadeBark(mb, w0, { foot: MOSS, footAmt: 0.5, y0: -0.02, y1: 0.12 });
  mb.heightAO(w0, -0.02, forkY, 0.5);
  // Hanging vines (lianas) from the plates' bellies.
  const v0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  mb.color = pal("#22391f");
  const vines = r.int(2, 4);
  for (let k = 0; k < vines; k++) {
    const p = plates[1 + (k % ns)];
    const a = r.range(0, TAU), d = p.R * r.range(0.3, 0.8);
    const top = [p.c[0] + Math.cos(a) * d, p.c[1] - p.T * 0.4, p.c[2] + Math.sin(a) * d];
    const len = r.range(0.07, 0.15);
    mb.tube([top, [top[0] + r.range(-0.01, 0.01), top[1] - len * 0.5, top[2] + r.range(-0.01, 0.01)], [top[0] + r.range(-0.01, 0.01), top[1] - len, top[2]]], [0.0032, 0.0026, 0.0016], 3);
  }
  eachVert(mb, v0, (v, i) => {
    v[i + 10] = 0.7 * clamp((C[1] - v[i + 1]) / 0.12, 0, 1);
  });
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  for (const p of plates) {
    mb.color = jitter(WHITE, r, 0.07);
    plate(mb, r, p.c, p.R, p.T, { seg: 12, lobes: r.int(4, 5), scallop: 0.24, lump: 0.35, droop: 0.4, dome: 1.1 });
  }
  mb.sphereNormals(m0, [C[0], C[1] - W * 0.5, C[2]], 0.35);
  const drift = [[0.95, 1, 1.05], [1, 1, 1], [1.04, 1.02, 0.94]][variant % 3];
  const cols = JUNGLE.map((c) => lin(c).map((x, q) => x * drift[q]));
  shadeFoliage(mb, m0, { lo: C[1] - W * 0.6, hi: H, cols, center: C, rad: W * 1.2, seed: r.range(0, 9), hw: 0.36, upw: 0.36, aoMin: 0.45 });
  // Big leaves bursting from under the plates' rims: the tropical edge of the silhouette.
  const f0 = mb.mark();
  const fc = cols.map((c) => c);
  const nl = r.int(4, 5);
  for (let k = 0; k < nl; k++) {
    const p = plates[1 + (k % ns)];
    const a = p.a + r.range(-0.9, 0.9);
    const base = [p.c[0] + Math.cos(a) * p.R * 0.55, p.c[1] - p.T * 0.2, p.c[2] + Math.sin(a) * p.R * 0.55];
    frondBlade(mb, base, a, r.range(0.07, 0.09), r.range(0.1, 0.35), r.range(0.7, 1.0), { width: 0.2, cols: [fc[0], fc[1], fc[2], vlerp(fc[2], fc[3], 0.6)], m: 4 });
  }
  swayFoliage(mb, m0, forkY - 0.05, H, W, 0.75, C[0], C[2]);
  swayFoliage(mb, f0, forkY, H, W, 1, C[0], C[2]);
}

/** The jungle's understory: a short, leaning stem in a burst of huge, drooping, banana-like leaves
 *  around a dark heart, a few young leaves standing up in the middle. */
function buildJungleUnder(mb, r) {
  const H = r.range(0.11, 0.15);
  const la = r.range(0, TAU), ln = r.range(0.005, 0.015);
  const top = [Math.cos(la) * ln, H, Math.sin(la) * ln];
  mb.color = jitter(pal(BARK_JUNGLE), r, 0.06);
  const w0 = mb.mark();
  mb.tube([[0, -0.05, 0], [top[0] * 0.5, H * 0.5, top[2] * 0.5], top], [0.012, 0.009, 0.006], 5);
  mb.heightAO(w0, -0.02, H, 0.55);
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  const cols = JUNGLE.map(lin);
  // A dark heart the leaves spring from.
  mb.color = jitter(WHITE, r, 0.06);
  leafMass(mb, [top[0], H - 0.005, top[2]], 0.04, { detail: 1, amp: 0.3, squash: 0.8, flat: 0.3, seed: r.range(0, 50), self: 0.5 });
  shadeFoliage(mb, m0, { lo: H - 0.04, hi: H + 0.04, cols, seed: r.range(0, 9), bias: -0.12 });
  const n = r.int(7, 8), ph = r.range(0, TAU);
  for (let k = 0; k < n; k++) {
    const a = ph + (k / n) * TAU + r.range(-0.25, 0.25);
    const L = r.range(0.11, 0.15);
    const j = jitter(WHITE, r, 0.08);
    frondBlade(mb, [top[0], H - 0.005, top[2]], a, L, r.range(0.5, 0.9), r.range(1.1, 1.5), { width: 0.26, fold: 0.35, round: 0.5, cols: cols.map((c) => c.map((x, q) => x * j[q])), m: 5 });
  }
  for (let k = 0; k < 2; k++) {
    frondBlade(mb, [top[0], H - 0.005, top[2]], r.range(0, TAU), r.range(0.06, 0.08), r.range(1.6, 2.2), 0.8, { width: 0.2, cols, m: 4 });
  }
  swayFoliage(mb, m0, H - 0.04, H + 0.08, 0.14, 1, top[0], top[2]);
}

recipe("tree.jungle", { variants: 6, build: (mb, r, { variant }) => buildJungle(mb, r, variant) });

// --- tree.palm ----------------------------------------------------------------------------------------

/** A palm: a ringed, gracefully bent trunk swelling at its foot, a starburst of arching, drooping
 *  fronds (folded leaf blades, yellowing a little at the tips), a dead frond hanging and a few nuts. */
function buildPalm(mb, r, variant) {
  const H = r.range(0.29, 0.36);
  const ba = r.range(0, TAU), bend = r.range(0.03, 0.08);
  const n = 7;
  const pts = [], rad = [], colors = [];
  const b1 = jitter(pal(BARK_PALM), r, 0.05), b2 = vlerp(b1, pal("#3e3428"), 0.45);
  for (let k = 0; k < n; k++) {
    const t = k / (n - 1);
    const off = bend * Math.pow(t, 1.8);
    pts.push([Math.cos(ba) * off, -0.05 + (H + 0.05) * t, Math.sin(ba) * off]);
    rad.push(0.0085 * (1 + 0.9 * Math.pow(1 - t, 4)));
    colors.push(k % 2 ? b1 : b2);
  }
  const w0 = mb.mark();
  mb.color = b1;
  mb.tube(pts, rad, 6, { colors });
  mb.heightAO(w0, -0.02, 0.08, 0.6);
  const top = pts[n - 1];
  // Nuts and the crown's heart.
  mb.color = pal("#4a3a26");
  for (let k = 0; k < 3; k++) {
    const a = r.range(0, TAU);
    mb.part(() => {
      mb.translate(top[0] + Math.cos(a) * 0.009, top[1] - 0.008, top[2] + Math.sin(a) * 0.009);
      mb.sphere(0.0075, 0);
    });
  }
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  const j = jitter(WHITE, r, 0.05);
  const cols = PALM.map((c) => lin(c).map((x, q) => x * j[q]));
  const dry = pal("#8a7448");
  const nf = r.int(7, 9);
  const fph = r.range(0, TAU);
  for (let k = 0; k < nf; k++) {
    const a = fph + (k / nf) * TAU + r.range(-0.25, 0.25);
    frondBlade(mb, top, a, r.range(0.12, 0.155), r.range(0.35, 0.75), r.range(0.9, 1.25), { cols });
  }
  // A young upright frond in the heart and a dead one hanging.
  frondBlade(mb, top, r.range(0, TAU), 0.07, 1.6, 1.1, { cols });
  if (variant % 2 === 0) frondBlade(mb, top, fph + 0.4, 0.1, -0.7, 0.4, { cols, dry });
  mb.sphereNormals(m0, [top[0], top[1] - 0.04, top[2]], 0.35);
  swayFoliage(mb, m0, top[1] - 0.04, top[1] + 0.04, 0.14, 1, top[0], top[2]);
}

recipe("tree.palm", { variants: 6, build: (mb, r, { variant }) => buildPalm(mb, r, variant) });

// --- tree.ancient -------------------------------------------------------------------------------------

const ANCIENT_BARK = "#7a7d74";
const ANCIENT_BARK_LIGHT = "#c9cabd";
const ANCIENT_MOSS = "#252f22";

/** The great trees of the ancient forests (elven, mallorn-like): a massive, silver-barked trunk of
 *  twisted strands on mossy buttress roots, carrying a crown of distinct horizontal tiers -- broad,
 *  flat plates of foliage on great boughs, each tier smaller and higher than the one below, dark
 *  teal in the lower tiers and lit to pale sage-gold at the top, with dark gaps between them where
 *  the boughs and the trunk show -- moss beards and a few lantern-fruits glowing at the tiers' rims.
 *  Variants: grand (three tiers), broad (three wide tiers), gnarled and leaning, younger. */
function buildAncient(mb, r, variant) {
  const kind = variant % 4;
  const H = [0.98, 0.9, 0.88, 0.8][kind] * r.range(0.97, 1.03);
  const Rc = [0.4, 0.44, 0.38, 0.34][kind] * r.range(0.95, 1.05);
  const forkY = H * [0.34, 0.32, 0.38, 0.4][kind];
  const la = r.range(0, TAU);
  const lean = kind === 2 ? 0.1 : 0.025;
  const R0 = [0.08, 0.085, 0.078, 0.064][kind];
  // The trunk's axis bends with height (fraction of H).
  const axis = (t) => [Math.cos(la) * lean * t * t, Math.sin(la) * lean * t * t];
  const at = (y) => {
    const [x, z] = axis(clamp(y / H, 0, 1));
    return [x, y, z];
  };
  // The tiers, cedar-like: broad, flat plates (each a bough's load of foliage) in layers that stay
  // wide as they rise, each layer's plates in the gaps of the one below, so every layer shows.
  const nt = 3;
  const topY = H - 0.1;
  const y0 = forkY + 0.07;
  const tiers = [];
  let ph = r.range(0, TAU);
  for (let k = 0; k < nt; k++) {
    const u = k / (nt - 1);
    const top = k === nt - 1;
    const y = y0 + (topY - y0) * Math.pow(u, 0.85);
    const Rk = Rc * (1 - 0.3 * u);
    const np = top ? 2 : k === 0 ? 4 : 3;
    const plates = [];
    for (let j = 0; j < np; j++) {
      const a = ph + (j / np) * TAU + r.range(-0.3, 0.3);
      // The gnarled one reaches out on its leaning side.
      const reach = kind === 2 ? 1 + 0.4 * Math.max(0, Math.cos(a - la)) : 1;
      const d = Rk * (top ? r.range(0.2, 0.3) : r.range(0.5, 0.62)) * reach;
      const o = at(y);
      const pr = Rk * (top ? r.range(0.58, 0.66) : r.range(0.5, 0.58));
      plates.push({ a, c: [o[0] + Math.cos(a) * d, y + r.range(-0.025, 0.025), o[2] + Math.sin(a) * d], R: pr, T: pr * (top ? r.range(0.56, 0.64) : r.range(0.42, 0.52)), top });
    }
    tiers.push({ y, R: Rk, plates });
    ph += Math.PI / np + r.range(-0.3, 0.3);
  }
  const crownTop = at(topY);

  // --- wood: core, twisted strands unravelling into the lowest tier's boughs, the leader with the
  // upper tiers' boughs, buttress roots.
  const bark = jitter(pal(ANCIENT_BARK), r, 0.04);
  const w0 = mb.mark();
  mb.color = bark;
  const core = [0, 0.3, 0.6, 1].map((t) => at(-0.05 + (forkY + 0.1) * t));
  mb.tube(core.concat([at((forkY + topY) * 0.5), at(topY - 0.02)]), [R0 * 0.85, R0 * 0.74, R0 * 0.66, R0 * 0.56, R0 * 0.36, R0 * 0.2], 7);
  const low = tiers[0].plates;
  const twist = r.range(0.9, 1.5) * r.sign();
  low.forEach((m) => {
    const P = [], Rr = [];
    const rs = R0 * 0.52;
    for (let j = 0; j <= 4; j++) {
      const t = j / 4;
      const th = m.a - twist + twist * t;
      const rr = R0 * (0.62 + 0.95 * Math.pow(1 - t, 3));
      const o = at(-0.05 + (forkY + 0.02) * t);
      P.push([o[0] + Math.cos(th) * rr, o[1], o[2] + Math.sin(th) * rr]);
      Rr.push(rs * (1 - 0.3 * t) * (1 + 0.6 * Math.pow(1 - t, 4)));
    }
    const p4 = P[4];
    const end = [m.c[0] - Math.cos(m.a) * m.R * 0.15, m.c[1] - m.T * 0.35, m.c[2] - Math.sin(m.a) * m.R * 0.15];
    P.push([p4[0] + (end[0] - p4[0]) * 0.45, p4[1] + (end[1] - p4[1]) * 0.75 + 0.02, p4[2] + (end[2] - p4[2]) * 0.45]);
    P.push(end);
    Rr.push(rs * 0.5, rs * 0.26);
    mb.tube(P, Rr, 6);
  });
  for (const t of tiers.slice(1)) {
    for (const m of t.plates) {
      const from = at(t.y - 0.07);
      limb(mb, from, [m.c[0] - Math.cos(m.a) * m.R * 0.2, m.c[1] - m.T * 0.35, m.c[2] - Math.sin(m.a) * m.R * 0.2], R0 * 0.3, R0 * 0.14, 4, 0.55);
    }
  }
  const nr = low.length + 1;
  const rph = low[0].a + Math.PI / nr;
  const roots0 = mb.mark();
  for (let k = 0; k < nr; k++) {
    const a = rph + (k / nr) * TAU + r.range(-0.25, 0.25);
    const ca = Math.cos(a), sa = Math.sin(a);
    const L = r.range(0.16, 0.22) * (kind === 3 ? 0.8 : 1);
    const q = (d, y) => [ca * d + r.range(-0.006, 0.006), y, sa * d + r.range(-0.006, 0.006)];
    mb.tube([q(R0 * 0.55, 0.12), q(R0 * 1.4, 0.06), q(L * 0.6, 0.014), q(L, -0.055)], [R0 * 0.44, R0 * 0.36, R0 * 0.22, R0 * 0.09], 5);
  }
  // Silvered bark, its upper faces catching the light; the foot and the roots dark with moss.
  // Up in the crown the wood is in the shade of the tiers.
  const pale = pal(ANCIENT_BARK_LIGHT), moss = pal(ANCIENT_MOSS), shade = pal("#33392f");
  eachVert(mb, w0, (v, i) => {
    const y = v[i + 1];
    let c = [v[i + 6], v[i + 7], v[i + 8]];
    c = vlerp(c, pale, 0.42 * clamp(v[i + 4] + 0.3, 0, 1) * smoothstep(0.1, 0.25, y) * (1 - smoothstep(forkY * 0.8, forkY + 0.08, y)));
    c = vlerp(c, shade, 0.6 * smoothstep(forkY * 0.9, forkY + 0.12, y));
    c = vlerp(c, moss, 0.85 * (1 - smoothstep(0.03, 0.24, y)));
    v[i + 6] = c[0];
    v[i + 7] = c[1];
    v[i + 8] = c[2];
  });
  mb.heightAO(w0, -0.03, forkY * 0.7, 0.45);
  mb.heightAO(roots0, -0.04, 0.1, 0.6);

  // --- foliage: the tiers' plates. Each tier is painted on its own ramp position so the tiers read
  // as separate layers: the lowest dark teal, the top lit pale.
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  const drift = [[0.95, 1, 1.07], [1, 1.02, 0.96], [1.05, 1.03, 0.92], [0.97, 1.01, 1.03]][kind];
  const cols = ANCIENT.map((c) => lin(c).map((x, q) => x * drift[q]));
  tiers.forEach((t, k) => {
    const tm = mb.mark();
    for (const m of t.plates) {
      mb.color = jitter(WHITE, r, 0.06);
      plate(mb, r, m.c, m.R, m.T, { seg: 16, lobes: r.int(3, 4), scallop: m.top ? 0.16 : 0.22, dome: m.top ? 1.25 : 1.1, droop: m.top ? 0.15 : 0.42, lump: 0.42 });
    }
    const o = at(t.y);
    mb.sphereNormals(tm, [o[0], t.y - t.R * 0.5, o[2]], 0.4);
    const u = k / (nt - 1);
    shadeFoliage(mb, tm, { lo: t.y - t.R * 0.25, hi: t.y + t.R * 0.25, cols, center: [o[0], t.y - t.R * 0.1, o[2]], rad: t.R * 1.1, seed: r.range(0, 9), hw: 0.12, upw: 0.36, nw: 0.2, nf: 9, bias: -0.04 + 0.28 * u, aoMin: 0.45 });
  });
  swayFoliage(mb, m0, forkY, H, Rc, 0.4, crownTop[0], crownTop[2]);

  // Beards of pale moss hanging from the lowest tier's bellies.
  const b0 = mb.mark();
  for (let k = 0; k < 6; k++) {
    const m = low[k % low.length];
    const a = m.a + r.range(-1.2, 1.2), d = m.R * r.range(0.3, 0.8);
    const top = [m.c[0] + Math.cos(a) * d, m.c[1] - m.T * 0.4, m.c[2] + Math.sin(a) * d];
    const len = r.range(0.05, 0.1);
    mb.color = jitter(pal("#7e8c72"), r, 0.08);
    mb.tube([top, [top[0] + r.range(-0.006, 0.006), top[1] - len * 0.55, top[2] + r.range(-0.006, 0.006)], [top[0], top[1] - len, top[2]]], [0.0055, 0.0035, 0.001], 3);
  }
  eachVert(mb, b0, (v, i) => (v[i + 10] = 0.45));

  // Lantern-fruits: a few pale lights hanging at the tiers' rims, out where they show from the
  // camera's height -- one or two from the upper tiers (seen over the canopy), the rest lower.
  const glows = [4, 4, 3, 3][kind];
  const upper = tiers.slice(1).flatMap((t) => t.plates), lower = tiers[0].plates;
  for (let k = 0; k < glows; k++) {
    const hi = k < 2;
    const list = hi ? upper : lower;
    const m = list[(k * 2 + 1) % list.length];
    const a = m.a + r.range(-0.6, 0.6), d = m.R * r.range(0.95, 1.05);
    const hang = [m.c[0] + Math.cos(a) * d * 0.9, m.c[1] - m.T * 0.2, m.c[2] + Math.sin(a) * d * 0.9];
    const fruit = [m.c[0] + Math.cos(a) * d, hang[1] - r.range(0.025, 0.045), m.c[2] + Math.sin(a) * d];
    const g0 = mb.mark();
    mb.mat = MAT.MATTE;
    mb.color = pal("#4f5e48");
    mb.tube([hang, fruit], 0.0018, 3);
    mb.mat = MAT.EMISSIVE;
    mb.color = vlerp(pal(PAL.ancientGlow), pal("#9ee0c8"), r());
    mb.part(() => {
      mb.translate(fruit[0], fruit[1], fruit[2]);
      mb.sphere(hi ? r.range(0.017, 0.02) : r.range(0.014, 0.017), 0, { squash: 1.3 });
    });
    eachVert(mb, g0, (v, i) => (v[i + 10] = 0.5));
  }
}

recipe("tree.ancient", { variants: 4, build: (mb, r, { variant }) => buildAncient(mb, r, variant) });

// --- tree.dead ----------------------------------------------------------------------------------------

/** A branch and its children: a tapering, kinked tube; `depth` more levels fork from its end. */
function branch(mb, r, p, dir, len, rad, depth, { up = 0.25, spread = 0.65, kink = 0.3, claw = 0 } = {}) {
  const d = vnorm([dir[0] + r.range(-kink, kink), dir[1] + up, dir[2] + r.range(-kink, kink)]);
  const mid = vadd(p, vadd(vscale(dir, len * 0.5), [r.range(-0.006, 0.006), r.range(-0.004, 0.006), r.range(-0.006, 0.006)]));
  let end = vadd(mid, vscale(d, len * 0.5));
  const P = [p, mid, end], R = [rad, rad * 0.68, rad * 0.32];
  if (claw) {
    // Claws: the tip curls down.
    const tip = vadd(end, [d[0] * len * 0.3, -len * claw, d[2] * len * 0.3]);
    P.push(tip);
    R[2] = rad * 0.4;
    R.push(rad * 0.08);
    end = tip;
  }
  mb.tube(P, R, 4);
  if (depth > 0) {
    const n = r() < 0.7 ? 2 : 1;
    for (let k = 0; k < n; k++) {
      const a = Math.atan2(d[2], d[0]) + (k === 0 ? 1 : -1) * spread * r.range(0.6, 1.2);
      const cd = [Math.cos(a) * 0.8, d[1] * 0.6 + 0.2, Math.sin(a) * 0.8];
      const at = vlerp(mid, P[2], r.range(0.3, 0.9));
      branch(mb, r, at, vnorm(cd), len * r.range(0.5, 0.7), rad * 0.55, depth - 1, { up, spread, kink, claw });
    }
  }
}

/** Dead and burnt snags: a blackened, twisting trunk (broken off in some), bare forking limbs, ash
 *  catching the light on the upper faces, and on most a few embers still glowing in the cracks. */
function buildDead(mb, r, variant) {
  const H = r.range(0.25, 0.34);
  const broken = variant % 3 === 1;
  const tr = r.range(0.02, 0.025);
  const w0 = mb.mark();
  mb.color = vlerp(jitter(pal("barkDead"), r, 0.06), pal("char"), r.range(0.6, 0.85));
  rootFlare(mb, 0, 0, tr, { h: 0.045, lobes: r.int(3, 5), seed: r.range(0, 6), seg: 8, spread: 2.5 });
  const P = [], R = [];
  const ph = r.range(0, TAU);
  const topY = H * (broken ? 0.68 : 0.82);
  for (let k = 0; k < 5; k++) {
    const t = k / 4;
    P.push([Math.cos(ph + t * 2.2) * 0.014 * t, 0.02 + (topY - 0.02) * t, Math.sin(ph + t * 2.2) * 0.014 * t]);
    R.push(tr * (1 - (broken ? 0.4 : 0.65) * t));
  }
  mb.tube(P, R, 6, { caps: broken });
  if (broken) {
    // A splintered top: a few jagged shards.
    for (let k = 0; k < 3; k++) {
      const a = ph + k * 2.1 + r.range(-0.3, 0.3);
      mb.part(() => {
        mb.translate(P[4][0] + Math.cos(a) * R[4] * 0.5, P[4][1] - 0.004, P[4][2] + Math.sin(a) * R[4] * 0.5);
        mb.rotateY(-a);
        mb.rotateZ(r.range(-0.25, 0.05));
        mb.cone(R[4] * 0.5, r.range(0.02, 0.04), 3);
      });
    }
  }
  const nb = broken ? 2 : r.int(3, 4);
  for (let k = 0; k < nb; k++) {
    const t = broken ? r.range(0.4, 0.85) : 0.4 + 0.5 * (k / Math.max(1, nb - 1));
    const i = Math.min(3, Math.floor(t * 4));
    const at = vlerp(P[i], P[i + 1], t * 4 - i);
    const a = ph + k * 2.3 + r.range(-0.4, 0.4);
    branch(mb, r, at, vnorm([Math.cos(a), r.range(0.3, 0.75), Math.sin(a)]), r.range(0.08, 0.12) * (broken ? 0.65 : 1), tr * 0.6, broken ? 0 : 1, { up: 0.18, kink: 0.35 });
  }
  if (!broken) branch(mb, r, P[4], [0, 1, 0], H * 0.24, R[4] * 0.9, 1, { up: 0.1, kink: 0.3 });
  shadeBark(mb, w0, { foot: "char", footAmt: 0.7, y0: 0, y1: 0.12, up: ASH, upAmt: 0.45 });
  // Embers in the cracks (sparing: most snags have one or two, some none).
  const embers = variant % 3 === 2 ? 0 : 1 + (variant % 2);
  mb.mat = MAT.EMISSIVE;
  for (let k = 0; k < embers; k++) {
    const a = r.range(0, TAU), y = k === 0 ? r.range(0.012, 0.05) : topY * r.range(0.4, 0.9);
    const t = clamp((y - 0.02) / (topY - 0.02), 0, 1);
    const i = Math.min(3, Math.floor(t * 4));
    const c = vlerp(P[i], P[i + 1], t * 4 - i);
    const rr = tr * (1 - 0.6 * t) * 0.92;
    mb.color = vlerp(pal(PAL.ember), pal("#7a1e08"), r.range(0.1, 0.35));
    mb.part(() => {
      mb.translate(c[0] + Math.cos(a) * rr, y, c[2] + Math.sin(a) * rr);
      mb.sphere(r.range(0.005, 0.007), 0, { squash: 1.8 });
    });
  }
}

recipe("tree.dead", { variants: 6, build: (mb, r, { variant }) => buildDead(mb, r, variant) });

// --- tree.blighted ------------------------------------------------------------------------------------

/** The infernal wood: a tortured, spiralling black trunk with clawed limbs curling down, thorns, a
 *  few sparse clumps of bruised, rust-edged leaves and, in some, a hot glow in a split. */
function buildBlighted(mb, r, variant) {
  const H = r.range(0.33, 0.41);
  const tr = r.range(0.022, 0.027);
  const w0 = mb.mark();
  mb.color = jitter(pal(BARK_BLIGHT), r, 0.05);
  rootFlare(mb, 0, 0, tr, { h: 0.045, lobes: 3, seed: r.range(0, 6), seg: 8, spread: 3 });
  const P = [], R = [];
  const ph = r.range(0, TAU), turns = r.range(3, 4.5) * r.sign();
  const topY = H * 0.74;
  const la = r.range(0, TAU), lean = r.range(0.02, 0.04);
  for (let k = 0; k < 6; k++) {
    const t = k / 5;
    const sw = 0.02 * Math.sin(t * Math.PI);
    P.push([Math.cos(ph + t * turns) * sw + Math.cos(la) * lean * t * t, 0.02 + (topY - 0.02) * t, Math.sin(ph + t * turns) * sw + Math.sin(la) * lean * t * t]);
    R.push(tr * (1.15 - 0.8 * t));
  }
  mb.tube(P, R, 6);
  const ends = [];
  const nb = r.int(4, 5);
  for (let k = 0; k < nb; k++) {
    const t = 0.45 + 0.55 * (k / (nb - 1));
    const i = Math.min(4, Math.floor(t * 5));
    const at = vlerp(P[i], P[Math.min(5, i + 1)], t * 5 - i);
    const a = ph + k * 2.5 + r.range(-0.3, 0.3);
    const len = r.range(0.11, 0.15) * (1.15 - 0.4 * t);
    const d = vnorm([Math.cos(a), r.range(0.5, 1.0), Math.sin(a)]);
    branch(mb, r, at, d, len, tr * 0.5, k % 2, { up: 0.35, kink: 0.25, spread: 0.8, claw: 0.4 });
    ends.push(vadd(at, vscale(d, len * 0.8)));
  }
  // Thorns, on the trunk and bristling at the claws' roots.
  for (const e of ends) {
    const a = r.range(0, TAU);
    mb.part(() => {
      mb.translate(e[0], e[1], e[2]);
      mb.rotateY(-a);
      mb.rotateZ(-0.5);
      mb.cone(tr * 0.3, tr * 2.2, 3);
    });
  }
  for (let k = 0; k < 5; k++) {
    const t = r.range(0.15, 0.9);
    const i = Math.min(4, Math.floor(t * 5));
    const c = vlerp(P[i], P[i + 1], t * 5 - i);
    const a = r.range(0, TAU);
    mb.part(() => {
      mb.translate(c[0], c[1], c[2]);
      mb.rotateY(-a);
      mb.rotateZ(-1.2);
      mb.cone(tr * 0.35, tr * 1.8, 3);
    });
  }
  shadeBark(mb, w0, { foot: "#140c0e", footAmt: 0.5, y0: 0, y1: 0.08, up: "#6e565a", upAmt: 0.5 });
  if (variant % 2 === 0) {
    // A split in the trunk, glowing from within.
    mb.mat = MAT.EMISSIVE;
    mb.color = vlerp(pal(PAL.hellGlow), pal("#5a0a04"), 0.3);
    const c = vlerp(P[1], P[2], 0.4);
    const a = r.range(0, TAU);
    mb.part(() => {
      mb.translate(c[0] + Math.cos(a) * R[1] * 0.75, c[1], c[2] + Math.sin(a) * R[1] * 0.75);
      mb.rotateY(-a);
      mb.scale(0.6, 3.2, 1);
      mb.sphere(tr * 0.6, 0);
    });
  }
  // Hanging tatters of dead moss from the claws, and on some one small clump of bruised leaves.
  const t0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  for (const [k, e] of ends.entries()) {
    if (k % 2 && variant % 2) continue;
    const len = r.range(0.04, 0.08);
    mb.color = jitter(pal(k % 2 ? "#3a2c34" : "#4a3a3c"), r, 0.1);
    mb.tube([e, [e[0] + r.range(-0.006, 0.006), e[1] - len * 0.5, e[2] + r.range(-0.006, 0.006)], [e[0], e[1] - len, e[2]]], [0.004, 0.0032, 0.001], 3);
  }
  eachVert(mb, t0, (v, i) => (v[i + 10] = 0.6));
  for (let q = 0; q < (variant % 3 === 2 ? 1 : 2); q++) {
    const m0 = mb.mark();
    const e = ends[(1 + q * 2) % ends.length];
    mb.color = jitter(WHITE, r, 0.1);
    leafMass(mb, [e[0], e[1] + 0.006, e[2]], r.range(0.022, 0.03), { squash: 0.5, flat: 0.5, amp: 0.5, seed: r.range(0, 50), freq: 2.4 });
    shadeFoliage(mb, m0, { lo: e[1] - 0.02, hi: e[1] + 0.02, cols: BLIGHT, seed: r.range(0, 9), hw: 0.3, upw: 0.4 });
    swayFoliage(mb, m0, e[1] - 0.02, e[1] + 0.02, 0.1, 0.5);
  }
}

recipe("tree.blighted", { variants: 6, build: (mb, r, { variant }) => buildBlighted(mb, r, variant) });

// --- tree.willow --------------------------------------------------------------------------------------

/** A marsh willow: a short, thick, leaning trunk forking low under a rounded crown that weeps -- long
 *  streamers of silvery leaves spring from all over the dome, arch out over it and fall in a ragged
 *  curtain almost to the ground, wider at the hem (a fountain, not a cap), with gaps showing the dark
 *  interior and the trunk. */
function buildWillow(mb, r, variant) {
  const H = r.range(0.27, 0.32);
  const W = r.range(0.1, 0.115);
  const forkY = r.range(0.09, 0.11);
  const la = r.range(0, TAU), ln = r.range(0.015, 0.035);
  const lx = Math.cos(la) * ln, lz = Math.sin(la) * ln;
  const tr = r.range(0.019, 0.023);
  const bark = jitter(pal(BARK_WILLOW), r, 0.05);
  const w0 = mb.mark();
  mb.color = bark;
  rootFlare(mb, 0, 0, tr, { h: 0.035, lobes: 4, seed: r.range(0, 6) });
  mb.tube([[0, 0.025, 0], [lx * 0.5, forkY * 0.6, lz * 0.5], [lx, forkY, lz], [lx * 1.2, forkY + 0.04, lz * 1.2]], [tr, tr * 0.9, tr * 0.75, tr * 0.5], 6);
  const hc = (H - forkY) * 0.3;
  const C = [lx * 1.3, H - hc * 1.1, lz * 1.3];
  // The dome under the streamers (mostly hidden: it gives the crown its dark body).
  const layout = [[0, 0, 0.3, 0.62], ...ringLayout(r, 2, { dist: [0.45, 0.55], dy: [-0.2, 0.1], rad: [0.48, 0.55] })];
  const m0 = crown(mb, r, { C, W, hc, fork: [lx, forkY, lz], layout, limbR: tr * 0.5, limbs: 3, bark, squash: 0.75, amp: 0.25, flat: 0.3, self: 0.4 });
  shadeBark(mb, w0, { foot: MOSS, footAmt: 0.6, y0: -0.02, y1: 0.08 });
  mb.heightAO(w0, -0.02, forkY, 0.5);
  mb.sphereNormals(m0, [C[0], C[1] - hc * 0.5, C[2]], 0.5);
  const cols = WILLOW.map(lin);
  shadeFoliage(mb, m0, { lo: C[1] - hc, hi: H, cols, center: C, rad: W * 1.1, seed: r.range(0, 9), hw: 0.3, upw: 0.3, bias: -0.08 });
  swayFoliage(mb, m0, C[1] - hc, H, W, 0.6, C[0], C[2]);
  // The streamers: from the dome's shoulders up near its top, arching outward, then falling.
  const [deep, dark, mid, lite] = cols;
  const S = 16;
  const ph = r.range(0, TAU);
  const c0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  const top = C[1] + hc * 0.9;
  for (let s = 0; s < S; s++) {
    const a = ph + (s / S) * TAU + r.range(-0.15, 0.15);
    const ca = Math.cos(a), sa = Math.sin(a);
    const side = [-sa, 0, ca];
    const inner = s % 2 === 0;
    const r0 = W * (inner ? r.range(0.15, 0.35) : r.range(0.45, 0.6));
    const y0 = inner ? top : top - hc * 0.6;
    const hem = W * r.range(1.0, 1.18);
    const end = r.range(0.035, 0.08) + (s % 3 === 1 ? r.range(0.03, 0.06) : 0);
    const w = r.range(0.016, 0.022);
    // Spine: start, shoulder (out and barely down), then falling with a slight outward drift.
    const spine = [
      [r0, y0],
      [W * 0.86, C[1] + hc * 0.2],
      [W * 0.98, C[1] - hc * 0.6],
      [hem * 0.98, (C[1] - hc * 0.6 + end) * 0.5],
      [hem, end],
    ].map(([d, y]) => [C[0] + ca * d, y, C[2] + sa * d]);
    const left = spine.map((p, j) => vadd(p, vscale(side, w * (1 - 0.45 * (j / 4)))));
    const right = spine.map((p, j) => vsub(p, vscale(side, w * (1 - 0.45 * (j / 4)))));
    const j = jitter(WHITE, r, 0.1);
    // Lit on the crown, darker in the curtain, a little lighter again at the hem's tips.
    const ks = spine.map((_, q) => (q < 2 ? vlerp(mid, lite, (inner ? 0.5 : 0.3) - 0.25 * q) : vlerp(dark, mid, 0.4 + 0.15 * (q - 2))).map((x, c) => x * j[c]));
    surface(mb, [left, spine, right], [ks.map((k) => vlerp(k, deep, 0.2)), ks, ks.map((k) => vlerp(k, deep, 0.2))], { ref: [C[0], C[1] - hc, C[2]] });
  }
  eachVert(mb, c0, (v, i) => {
    v[i + 10] = 0.25 + 0.75 * clamp((C[1] - v[i + 1]) / 0.15, 0, 1);
  });
}

recipe("tree.willow", { variants: 6, build: (mb, r, { variant }) => buildWillow(mb, r, variant) });

// --- small plants --------------------------------------------------------------------------------------

/** A triangle with normal `n` (wound to agree with it) and per-corner colours. */
function triC(mb, a, b, c, n, ka, kb, kc) {
  const f = vcross(vsub(b, a), vsub(c, a));
  if (vdot(f, n) < 0) [b, c, kb, kc] = [c, b, kc, kb];
  mb.vert(a, n, ka);
  mb.vert(b, n, kb);
  mb.vert(c, n, kc);
}

/** A grass-like blade from `p` leaning along `dir` (unit, horizontal) to height `h`: a quad and a
 *  tip triangle, normals facing out of the clump (`out`), coloured base -> tip. */
function blade(mb, p, dir, h, w, lean, out, kBase, kTip, droop = 0) {
  const side = [-dir[2] * w, 0, dir[0] * w];
  const m = [p[0] + dir[0] * lean * 0.35, p[1] + h * 0.55, p[2] + dir[2] * lean * 0.35];
  const t = [p[0] + dir[0] * lean, p[1] + h - droop, p[2] + dir[2] * lean];
  const n = vnorm([out[0], 0.6, out[2]]);
  const km = vlerp(kBase, kTip, 0.55);
  const a = vsub(p, side), b = vadd(p, side), c = vadd(m, vscale(side, 0.6)), d = vsub(m, vscale(side, 0.6));
  triC(mb, a, b, c, n, kBase, kBase, km);
  triC(mb, a, c, d, n, kBase, km, km);
  triC(mb, d, c, t, n, km, km, kTip);
}

/** Young trees: a broadleaf whip with a few small leaf masses, a young conifer (`pine`), or a
 *  young tree of the ancient forest (`ancient`, teal). */
function buildSapling(mb, r, variant, param) {
  const kind = ["pine", "snow", "broad", "ancient"].includes(param) ? param : variant >= 4 ? "pine" : "broad";
  if (kind === "pine" || kind === "snow") return buildPine(mb, r, variant, { small: true, snow: kind === "snow" });
  const H = r.range(0.16, 0.21) * (kind === "ancient" ? 1.1 : 1);
  const a = r.range(0, TAU), ln = r.range(0.004, 0.014);
  const top = [Math.cos(a) * ln, H * 0.86, Math.sin(a) * ln];
  const w0 = mb.mark();
  mb.color = jitter(pal(kind === "ancient" ? BARK_ANCIENT : "barkOak"), r, 0.05);
  mb.tube([[0, -0.05, 0], [top[0] * 0.4, H * 0.35, top[2] * 0.4], top], [0.0075, 0.0055, 0.0035], 4);
  mb.heightAO(w0, -0.02, H * 0.4, 0.6);
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  // An upright, irregular young crown (never a ball on a stick): five small clumps spiralling up
  // the stem from low down, the lower ones out on short twigs, narrowing to a tuft at the top.
  const W = r.range(0.046, 0.056) * (kind === "ancient" ? 1.1 : 1);
  const C = [top[0] * 0.7, H * 0.62, top[2] * 0.7];
  const n = 5, a0 = r.range(0, TAU);
  for (let k = 0; k < n; k++) {
    const u = k / (n - 1);
    const b = a0 + k * 2.3 + r.range(-0.3, 0.3);
    const d = W * (0.55 - 0.42 * u) * r.range(0.85, 1.15);
    const p = [top[0] * (0.3 + 0.7 * u) + Math.cos(b) * d, H * (0.36 + 0.56 * u), top[2] * (0.3 + 0.7 * u) + Math.sin(b) * d];
    mb.color = jitter(WHITE, r, 0.08);
    leafMass(mb, p, W * (0.62 - 0.2 * u) * r.range(0.9, 1.1), { detail: 0, amp: 0.2, squash: 0.85, flat: 0.25, seed: r.range(0, 50), self: 0.3 });
  }
  mb.sphereNormals(m0, [C[0], C[1] - W * 0.4, C[2]], 0.6);
  shadeFoliage(mb, m0, { lo: H * 0.3, hi: H, cols: kind === "ancient" ? ANCIENT : OAK, center: C, rad: W * 1.4, seed: r.range(0, 9), bias: kind === "ancient" ? 0.1 : 0.04, aoMin: 0.6 });
  swayFoliage(mb, m0, H * 0.3, H, W, 1, C[0], C[2]);
}

recipe("tree.sapling", { variants: 6, build: (mb, r, { variant, param }) => buildSapling(mb, r, variant, param) });
recipe("tree.sapling.pine", { variants: 6, build: (mb, r, { variant }) => buildSapling(mb, r, variant, "pine") });
recipe("tree.sapling.ancient", { variants: 6, build: (mb, r, { variant }) => buildSapling(mb, r, variant, "ancient") });

/** A flower head (blossom, berries, a speck of colour): a small square of two triangles turned by
 *  `a`, facing up and leaning toward the camera (+z) so it shows at the game's view. */
function blossom(mb, p, s, a) {
  const c = Math.cos(a) * s, d = Math.sin(a) * s;
  const n = vnorm([0, 1, 0.45]);
  const q = (dx, dz) => [p[0] + dx, p[1] - dz * 0.45, p[2] + dz];
  const A = q(c, d), B = q(-d, c), C = q(-c, -d), D = q(d, -c);
  const k = mb.color;
  triC(mb, A, B, C, n, k, k, k);
  triC(mb, A, C, D, n, k, k, k);
}

/** Flower and berry colours of the bushes and wild flowers. */
const BLOOM = { white: "#f4efdc", cream: "#efe2b0", yellow: "#f0c838", gold: "#e8a83a", pink: "#e690b0", lilac: "#b6a0e6", red: "#b8323a" };

/** A bush: a leafy mound of a few clumps, each lit on top and dark at its belly (so the clumps
 *  read), sunk into the ground. Shapes: a round mound (0, 1), low and wide (2), tall and upright
 *  (3), two-lobed (4); in flower, flecked over its upper faces: white blossom (5), yellow broom (6),
 *  red berries (7). 100-140 triangles. */
function buildBush(mb, r, variant) {
  const bloom = variant >= 5 ? [BLOOM.white, BLOOM.yellow, BLOOM.red][variant - 5] : null;
  const shape = bloom ? variant - 5 : variant;
  const H = r.range(0.07, 0.1) * (shape === 2 ? 0.75 : shape === 3 ? 1.35 : 1);
  const W = r.range(0.065, 0.08) * (shape === 2 ? 1.3 : shape === 3 ? 0.8 : 1);
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  const ph = r.range(0, TAU);
  // The main mass (two side by side for the two-lobed shape), then smaller clumps leaning on it.
  const mains = shape === 4 ? [[Math.cos(ph) * W * 0.32, Math.sin(ph) * W * 0.32, 0.5], [-Math.cos(ph) * W * 0.36, -Math.sin(ph) * W * 0.36, 0.44]] : [[0, 0, 0.62]];
  for (const [x, z, s] of mains) {
    mb.color = jitter(WHITE, r, 0.06);
    leafMass(mb, [x, H * 0.45, z], W * s, { detail: 1, squash: (H * 0.55) / (W * s), flat: 0, seed: r.range(0, 50), amp: 0.3, self: 0.6, tone: 0.5 });
  }
  const n = shape === 4 ? 1 : bloom ? 2 : 3;
  for (let k = 0; k < n; k++) {
    const a = ph + 1.2 + (k / n) * TAU + r.range(-0.4, 0.4);
    mb.color = jitter(WHITE, r, 0.08);
    leafMass(mb, [Math.cos(a) * W * 0.6, H * r.range(0.2, 0.4), Math.sin(a) * W * 0.6], W * r.range(0.36, 0.46), { detail: 0, amp: 0.15, squash: 0.8, flat: 0, seed: r.range(0, 50), self: 0.6, tone: 0.5 });
  }
  // Sink the bottom into the ground.
  eachVert(mb, m0, (v, i) => {
    if (v[i + 1] < 0.004) v[i + 1] = Math.max(-0.03, v[i + 1] * 1.5 - 0.01);
  });
  mb.sphereNormals(m0, [0, 0, 0], 0.35);
  const drift = [[0.95, 1, 1.06], [1, 1, 1], [1.06, 1.03, 0.88]][r.int(0, 2)];
  shadeFoliage(mb, m0, { lo: 0, hi: H, cols: BUSH.map((c) => lin(c).map((x, q) => x * drift[q])), center: [0, H * 0.3, 0], rad: W, seed: r.range(0, 9), aoMin: 0.6, hw: 0.45 });
  // Blossom or berries: flecks over the upper faces, toward the light.
  if (bloom) {
    mb.mat = MAT.MATTE;
    const nb = variant === 7 ? 6 : 9;
    for (let k = 0; k < nb; k++) {
      const u = r.range(0.15, 0.9), b = r.range(-0.5, Math.PI + 0.5), q = Math.sqrt(1 - u * u);
      const p = [Math.cos(b) * q * W * 0.62, H * 0.47 + u * H * 0.55, Math.sin(b) * q * W * 0.62];
      mb.color = jitter(pal(bloom), r, 0.1);
      blossom(mb, p, r.range(0.007, 0.01) * (variant === 7 ? 0.8 : 1), r.range(0, TAU));
    }
  }
  swayFoliage(mb, m0, 0, H, W, 0.35);
}

recipe("bush", { variants: 8, build: (mb, r, { variant }) => buildBush(mb, r, variant) });

/** Reeds: a clump of arching blades, olive at the foot, straw at the tips, with a few cattails. */
function buildReeds(mb, r, variant) {
  const n = 22 + (variant % 2) * 4;
  const base = pal("#34452a"), tip = pal(PAL.reed), pale = pal("#6f7c40");
  mb.mat = MAT.BLADE;
  for (let k = 0; k < n; k++) {
    const a = r.range(0, TAU), d = Math.sqrt(r()) * 0.045;
    const p = [Math.cos(a) * d, -0.02, Math.sin(a) * d];
    const la = a + r.range(-0.6, 0.6);
    const dir = [Math.cos(la), 0, Math.sin(la)];
    const h = r.range(0.08, 0.13) * (1 - d * 4);
    blade(mb, p, dir, h + 0.02, r.range(0.005, 0.0075), r.range(0.01, 0.045), [Math.cos(a), 0, Math.sin(a)], jitter(base, r, 0.08), vlerp(tip, pale, r()), r.range(0, 0.02));
  }
  const tails = variant === 3 ? 0 : r.int(2, 3);
  for (let k = 0; k < tails; k++) {
    const a = r.range(0, TAU), d = r.range(0, 0.03);
    const h = r.range(0.12, 0.15);
    const p = [Math.cos(a) * d, -0.02, Math.sin(a) * d];
    const tipP = [p[0] + r.range(-0.012, 0.012), h, p[2] + r.range(-0.012, 0.012)];
    mb.color = pal("#5a5a32");
    mb.mat = MAT.MATTE;
    mb.tube([p, tipP], [0.0016, 0.001], 3);
    mb.color = pal("#4a2e1c");
    const hp = vlerp(p, tipP, 0.82);
    mb.tube([hp, vlerp(p, tipP, 0.96)], 0.0045, 4, { caps: true });
  }
  mb.heightSway(0, 0, 0.14, 1);
}

recipe("reeds", { variants: 4, build: (mb, r, { variant }) => buildReeds(mb, r, variant) });

/** Desert and scrub plants: a thorny dry shrub, a dusty sage clump, a branching column cactus and a
 *  cluster of ribbed barrel cacti with a flower or two. */
function buildDryShrub(mb, r, variant) {
  const kind = variant % 4;
  if (kind === 0) {
    // Thorny twigs fanning from the root, with two small grey-olive clumps.
    mb.color = jitter(pal("#5a4632"), r, 0.06);
    const n = r.int(5, 6);
    const tips = [];
    for (let k = 0; k < n; k++) {
      const a = r.range(0, TAU), e = r.range(0.5, 1.1), L = r.range(0.06, 0.09);
      const d = [Math.cos(a) * Math.cos(e), Math.sin(e), Math.sin(a) * Math.cos(e)];
      const mid = [d[0] * L * 0.5 + r.range(-0.008, 0.008), d[1] * L * 0.5, d[2] * L * 0.5 + r.range(-0.008, 0.008)];
      const tp = vscale(d, L);
      mb.tube([[0, -0.02, 0], mid, tp], [0.0035, 0.0022, 0.0008], 3);
      tips.push(tp);
    }
    const m0 = mb.mark();
    mb.mat = MAT.FOLIAGE;
    for (let k = 0; k < 2; k++) {
      const t = tips[k * 2];
      mb.color = jitter(WHITE, r, 0.08);
      leafMass(mb, [t[0] * 0.8, t[1] * 0.8, t[2] * 0.8], r.range(0.016, 0.022), { detail: 0, amp: 0.2, squash: 0.7, flat: 0.3, seed: r.range(0, 50) });
    }
    shadeFoliage(mb, m0, { lo: 0.02, hi: 0.08, cols: SAGE, seed: r.range(0, 9), bias: -0.1 });
    swayFoliage(mb, m0, 0, 0.09, 0.08, 0.4);
  } else if (kind === 1) {
    // Sagebrush: a low, wide, ragged clump of small silvery tufts on dark twigs (not one smooth
    // mound, which reads as a stone).
    mb.color = jitter(pal("#4a3c2c"), r, 0.06);
    const tips = [];
    const n = 6, ph = r.range(0, TAU);
    for (let k = 0; k < n; k++) {
      const a = ph + (k / n) * TAU + r.range(-0.3, 0.3), d = k === 0 ? 0.004 : r.range(0.025, 0.045);
      const tp = [Math.cos(a) * d, k === 0 ? r.range(0.045, 0.055) : r.range(0.022, 0.04), Math.sin(a) * d];
      mb.tube([[tp[0] * 0.2, -0.02, tp[2] * 0.2], tp], [0.003, 0.0015], 3);
      tips.push(tp);
    }
    const m0 = mb.mark();
    mb.mat = MAT.FOLIAGE;
    for (const t of tips) {
      mb.color = jitter(WHITE, r, 0.1);
      leafMass(mb, t, r.range(0.016, 0.022), { detail: 0, amp: 0.3, squash: 0.8, flat: 0.35, seed: r.range(0, 50), self: 0.5 });
    }
    mb.sphereNormals(m0, [0, -0.01, 0], 0.4);
    shadeFoliage(mb, m0, { lo: 0, hi: 0.06, cols: SAGE, seed: r.range(0, 9), center: [0, 0.015, 0], rad: 0.06, aoMin: 0.6, nw: 0.3 });
    swayFoliage(mb, m0, 0, 0.05, 0.06, 0.3);
  } else if (kind === 2) {
    // A column cactus with an arm or two turning up.
    const g = jitter(pal("#4c6544"), r, 0.05);
    const H = r.range(0.13, 0.17);
    mb.color = g;
    mb.lathe([[0.017, -0.04], [0.0175, H * 0.6], [0.0145, H * 0.92], [0.007, H], [0, H + 0.003]], 8, { caps: false, wobble: (a) => 0.08 * Math.cos(a * 8) });
    const arms = r.int(1, 2), a0 = r.range(0, TAU);
    for (let k = 0; k < arms; k++) {
      const a = a0 + k * Math.PI + r.range(-0.4, 0.4);
      const ca = Math.cos(a), sa = Math.sin(a);
      const y = H * r.range(0.3, 0.5), out = r.range(0.042, 0.052), up = r.range(0.04, 0.06);
      mb.tube([[ca * 0.008, y, sa * 0.008], [ca * out * 0.75, y + 0.003, sa * out * 0.75], [ca * out, y + up * 0.45, sa * out], [ca * out, y + up, sa * out]], [0.0095, 0.0095, 0.009, 0.007], 6, { caps: true });
    }
    // Sunlit tops: lighter toward the crown.
    eachVert(mb, 0, (v, i) => {
      const k = 0.72 + 0.3 * clamp(v[i + 1] / H, 0, 1) + 0.12 * Math.max(0, v[i + 4]);
      v[i + 6] *= k;
      v[i + 7] *= k;
      v[i + 8] *= k;
    });
    mb.heightAO(0, -0.01, 0.04, 0.6);
  } else {
    // Ribbed barrel cacti, one with a flower.
    const n = r.int(2, 3);
    for (let k = 0; k < n; k++) {
      const a = r.range(0, TAU), d = k === 0 ? 0 : r.range(0.028, 0.036);
      const R = k === 0 ? r.range(0.024, 0.03) : r.range(0.015, 0.02);
      const h = R * r.range(1.5, 2.1);
      mb.color = jitter(pal("#56704a"), r, 0.06);
      mb.part(() => {
        mb.translate(Math.cos(a) * d, -0.015, Math.sin(a) * d);
        mb.lathe([[R * 0.8, 0], [R, h * 0.35], [R * 0.85, h * 0.85], [0, h]], 7, { caps: false, wobble: (b) => 0.1 * Math.cos(b * 7) });
        if (k === 0) {
          mb.mat = MAT.MATTE;
          mb.color = pal(r() < 0.5 ? "#c0584a" : "#d8b048");
          mb.translate(0, h - 0.002, 0);
          mb.cone(R * 0.4, 0.008, 5);
        }
      });
    }
    eachVert(mb, 0, (v, i) => {
      const k = 0.78 + 0.22 * Math.max(0, v[i + 4]);
      v[i + 6] *= k;
      v[i + 7] *= k;
      v[i + 8] *= k;
    });
  }
}

recipe("shrub.dry", { variants: 4, build: (mb, r, { variant }) => buildDryShrub(mb, r, variant) });

/** A fern: arching fronds in a rosette, deep green at the heart, fresh at the tips. */
function buildFern(mb, r, variant) {
  const n = r.int(6, 7);
  const ph = r.range(0, TAU);
  const [deep, dark, mid, lite] = BUSH.map(lin);
  mb.mat = MAT.FOLIAGE;
  for (let k = 0; k < n; k++) {
    const a = ph + (k / n) * TAU + r.range(-0.3, 0.3);
    const dir = [Math.cos(a), 0, Math.sin(a)];
    const side = [-dir[2], 0, dir[0]];
    const L = r.range(0.055, 0.075) * (variant % 2 ? 1.15 : 1);
    const el = r.range(1.4, 1.9);
    const rows = [[], [], []], ks = [[], [], []];
    for (let j = 0; j < 4; j++) {
      const s = j / 3;
      const sp = [dir[0] * L * s, -0.006 + L * (el * s - 1.5 * s * s), dir[2] * L * s];
      const w = L * 0.22 * Math.sin(Math.PI * Math.min(1, 0.12 + s * 0.9));
      rows[0].push([sp[0] + side[0] * w, sp[1] - w * 0.3, sp[2] + side[2] * w]);
      rows[1].push(sp);
      rows[2].push([sp[0] - side[0] * w, sp[1] - w * 0.3, sp[2] - side[2] * w]);
      const c = s < 0.5 ? vlerp(deep, dark, s * 2) : vlerp(mid, lite, (s - 0.5) * 2);
      ks[0].push(vlerp(c, dark, 0.3));
      ks[1].push(c);
      ks[2].push(vlerp(c, dark, 0.3));
    }
    surface(mb, rows, ks, { up: [0, 1, 0] });
  }
  swayFoliage(mb, 0, 0, 0.04, 0.07, 0.8);
}

recipe("plant.fern", { variants: 4, build: (mb, r, { variant }) => buildFern(mb, r, variant) });

/** A ring of toadstools; the glowing kind (`glow`) are the pale lights of the ancient forests' floor. */
function buildMushrooms(mb, r, variant, glow) {
  const n = r.int(2, 3);
  for (let k = 0; k < n; k++) {
    const a = r.range(0, TAU), d = k === 0 ? 0 : r.range(0.015, 0.028);
    const h = (k === 0 ? 0.03 : r.range(0.014, 0.024)) * (glow ? 1 : 0.9);
    const rc = h * r.range(0.45, 0.65);
    mb.part(() => {
      mb.translate(Math.cos(a) * d, -0.01, Math.sin(a) * d);
      mb.rotateZ(r.range(-0.2, 0.2));
      mb.mat = MAT.MATTE;
      mb.color = pal("#cfc4ac");
      mb.lathe([[0.003, 0], [0.0024, h + 0.01]], 5, { caps: false });
      mb.mat = glow ? MAT.EMISSIVE : MAT.MATTE;
      mb.color = glow ? vlerp(pal("#3fa08a"), pal("#86d8b0"), r()) : pal(variant % 3 === 2 ? "#8a3424" : "#7a5434");
      mb.translate(0, h + 0.01, 0);
      mb.lathe([[rc * 0.3, -rc * 0.1], [rc, -rc * 0.05], [rc * 0.75, rc * 0.35], [0, rc * 0.55]], 6, { caps: true });
    });
  }
}

recipe("plant.mushroom", { variants: 4, build: (mb, r, { variant }) => buildMushrooms(mb, r, variant, false) });
recipe("plant.mushroom.glow", { variants: 4, build: (mb, r, { variant }) => buildMushrooms(mb, r, variant, true) });

/** A tuft of meadow grass (the open ground's commonest clutter): a fan of broad, pointed leaves
 *  splaying from a dense foot, dark at the root, lit at the tips -- a painted clump of strokes, not
 *  hair-fine blades (which would read as scribbles at the game's zoom); green (variants 0-3, 4-5 tall
 *  and seeding) or, with param "dry", golden straw (the plains). 36 triangles. */
function buildGrass(mb, r, variant, param) {
  const dry = param === "dry";
  const tips = (dry ? ["#e2c25a", "#f0d478", "#cfa444"] : ["#a6c44e", "#c2d064", "#8cb244"]).map(pal);
  const base = vlerp(pal(dry ? "#6a5a24" : "#2a4619"), tips[0], 0.3);
  const tall = (variant >= 4 ? 1.3 : 1) * r.range(0.9, 1.1);
  mb.mat = MAT.FOLIAGE;
  const ph = r.range(0, TAU);
  for (let k = 0; k < 12; k++) {
    const a = ph + (k / 12) * TAU + r.range(-0.3, 0.3), u = k < 4 ? 0.2 : r.range(0.5, 1), d = u * 0.016;
    const la = a + r.range(-0.3, 0.3);
    const h = r.range(0.06, 0.09) * tall * (1 - 0.3 * u);
    blade(mb, [Math.cos(a) * d, -0.012, Math.sin(a) * d], [Math.cos(la), 0, Math.sin(la)], h, r.range(0.007, 0.0095), r.range(0.012, 0.03) + 0.03 * u, [Math.cos(a), 0, Math.sin(a)], jitter(base, r, 0.1), jitter(r.pick(tips), r, 0.08), r.range(0, 0.015) * tall);
  }
  mb.heightSway(0, 0, 0.08 * tall, 1);
}

recipe("plant.grass", { variants: 6, shadow: false, build: (mb, r, { variant, param }) => buildGrass(mb, r, variant, param) });

/** Wild flowers: a low cushion of broad leaves under a spray of flower heads in one colour by
 *  variant -- white (0, 1), yellow (2, 3), pink (4), lilac (5) -- the flecks of colour over a
 *  meadow. 39 triangles. */
function buildFlowers(mb, r, variant) {
  const leaf = pal("#2c4a1e"), leafTip = pal("#7ea444");
  mb.mat = MAT.FOLIAGE;
  const ph = r.range(0, TAU);
  for (let k = 0; k < 7; k++) {
    const a = ph + (k / 7) * TAU + r.range(-0.3, 0.3), d = r.range(0.004, 0.014);
    const la = a + r.range(-0.3, 0.3);
    blade(mb, [Math.cos(a) * d, -0.01, Math.sin(a) * d], [Math.cos(la), 0, Math.sin(la)], r.range(0.035, 0.05), r.range(0.009, 0.012), r.range(0.025, 0.04), [Math.cos(a), 0, Math.sin(a)], jitter(leaf, r, 0.1), jitter(leafTip, r, 0.08), r.range(0.01, 0.02));
  }
  mb.mat = MAT.MATTE;
  const c = pal([BLOOM.white, BLOOM.white, BLOOM.yellow, BLOOM.gold, BLOOM.pink, BLOOM.lilac][variant % 6]);
  for (let k = 0; k < 9; k++) {
    const a = r.range(0, TAU), d = Math.sqrt(r()) * 0.032;
    mb.color = jitter(c, r, 0.08);
    blossom(mb, [Math.cos(a) * d, r.range(0.036, 0.055) - d * 0.4, Math.sin(a) * d], r.range(0.011, 0.015), r.range(0, TAU));
  }
  mb.heightSway(0, 0, 0.05, 0.8);
}

recipe("plant.flowers", { variants: 6, shadow: false, build: (mb, r, { variant }) => buildFlowers(mb, r, variant) });

// --- catalog ------------------------------------------------------------------------------------------
//
// forest(feat, ctx): the species mix of a wooded plot ({asset, scale, weight, variant?, param?}; the
// first entry may carry `grid`, trees per plot side), its undergrowth (`under`) and what stands
// where the trees thin out toward open ground (`edge`: bushes, saplings). dress.js picks a variant
// at random and multiplies the scale by 0.75-1.25 (an uneven canopy). flora(ctx): open ground's
// plants, which dress.js scatters in clumps (catalog.js).
//
// On the map trees stand 1.2-1.3 times their recipes' size, as tall as the look target's: a
// broadleaf ~0.4-0.46 of a plot, a conifer ~0.43-0.52 (then the 0.75-1.25 spread; the ancient
// giants excepted).

const COLD = (g) => g === GROUND.TUNDRA || g === GROUND.SNOW;
const mix = (entries, under = [], grid = null, edge = []) => {
  if (grid) entries[0].grid = grid;
  entries.under = under;
  entries.edge = edge;
  return entries;
};
// A temperate wood's edge: bushes (some in flower), saplings and tall grass.
const EDGE = [
  { asset: "bush", scale: 1.25 },
  { asset: "bush", scale: 0.95 },
  { asset: "tree.sapling", param: "broad", scale: 1.15 },
  { asset: "tree.sapling", param: "pine", scale: 1.15 },
  { asset: "plant.grass", scale: 1.6 },
];

function forestMix(feat, ctx) {
  const g = ctx.ground;
  const hell = isHell(g);
  if (feat === FEAT.ANCIENT) {
    // Giants, a little smaller where something stands among them (a glade under the boughs).
    const k = ctx.held ? 0.8 : 1;
    return mix([{ asset: "tree.ancient", scale: 0.84 * k, weight: 3 }, { asset: "tree.ancient", variant: 3, scale: 0.72 * k, weight: 1 }], [
      { asset: "plant.fern", scale: 1.2, count: 3 },
      { asset: "tree.sapling.ancient", scale: 1.1, count: 2 },
      { asset: "plant.mushroom.glow", scale: 1.2, count: 1 },
    ], 2);
  }
  if (feat === FEAT.BURNT) {
    if (hell) return mix([{ asset: "tree.dead", scale: 1.05, weight: 2 }, { asset: "tree.blighted", scale: 1, weight: 1 }]);
    // Charred snags, and green coming back between them.
    return mix([{ asset: "tree.dead", scale: 1.05, weight: 5 }, { asset: "tree.sapling", param: "broad", scale: 0.8, weight: 0.6 }, { asset: "bush", scale: 0.8, weight: 0.5 }], [{ asset: "plant.grass", param: "dry", scale: 1.4, count: 3 }]);
  }
  if (feat === FEAT.JUNGLE) {
    return mix([{ asset: "tree.jungle", scale: 1.1, weight: 3 }, { asset: "tree.palm", scale: 1.05, weight: 1 }], [
      { asset: "plant.fern", scale: 1.4, count: 4 },
      { asset: "bush", scale: 1.2, count: 2 },
    ], null, [{ asset: "bush", scale: 1.3 }, { asset: "plant.fern", scale: 1.5 }, { asset: "tree.jungle", variant: 4, scale: 1 }]);
  }
  if (feat === FEAT.YOUNG) {
    if (hell) return mix([{ asset: "tree.blighted", scale: 0.6, weight: 1 }, { asset: "tree.dead", scale: 0.6, weight: 1 }]);
    if (COLD(g)) return mix([{ asset: "tree.sapling", param: "snow", scale: 1.2, weight: 3 }, { asset: "tree.pine", param: "snow", scale: 0.65, weight: 1 }, { asset: "bush", scale: 0.9, weight: 0.3 }], [], 5);
    // A thicket: saplings close together, a few bushes and a young birch.
    return mix([
      { asset: "tree.sapling", param: "broad", scale: 1.25, weight: 3 },
      { asset: "tree.sapling", param: "pine", scale: 1.2, weight: ctx.hills ? 2 : 1 },
      { asset: "bush", scale: 1.3, weight: 1 },
      { asset: "tree.birch", scale: 0.72, weight: 0.6 },
    ], [{ asset: "plant.grass", scale: 1.4, count: 3 }], 5, EDGE);
  }
  if (feat !== FEAT.FOREST) return null;
  // Woods by ground.
  if (hell) return mix([{ asset: "tree.blighted", scale: 1.05, weight: 3 }, { asset: "tree.dead", scale: 1, weight: 1 }]);
  if (g === GROUND.SNOW) return mix([{ asset: "tree.pine", param: "snow", scale: 1.05, weight: 1 }]);
  if (g === GROUND.TUNDRA) return mix([{ asset: "tree.pine", param: "snow", scale: 1.05, weight: 4 }, { asset: "tree.pine", scale: 1.02, weight: 1 }, { asset: "tree.birch", scale: 1.05, weight: 0.6 }], [{ asset: "plant.grass", param: "dry", scale: 1.3, count: 2 }]);
  if (ctx.hills) return mix([{ asset: "tree.pine", scale: 1.15, weight: 3 }, { asset: "tree.oak", scale: 1.2, weight: 1.2 }, { asset: "tree.birch", scale: 1.1, weight: 0.4 }], [{ asset: "bush", scale: 1.1, count: 2 }], 4, EDGE);
  if (g === GROUND.MARSH) return mix([{ asset: "tree.willow", scale: 1.1, weight: 2.5 }, { asset: "tree.oak", scale: 1.05, weight: 1 }, { asset: "tree.birch", scale: 1.1, weight: 0.8 }], [{ asset: "reeds", scale: 1, count: 4 }], null, [{ asset: "reeds", scale: 1.1 }, { asset: "bush", scale: 1 }]);
  if (g === GROUND.DESERT) return mix([{ asset: "tree.oak", scale: 1, weight: 1 }, { asset: "tree.pine", scale: 1, weight: 1 }, { asset: "tree.palm", scale: 1.05, weight: 0.6 }], [{ asset: "shrub.dry", scale: 1.25, count: 2 }], 3);
  // Temperate woods: oaks of two sizes (an uneven canopy), birch and pine, bushes and ferns under
  // them, bushes and saplings at the edge.
  if (g === GROUND.PLAINS) return mix([{ asset: "tree.oak", scale: 1.25, weight: 2 }, { asset: "tree.oak", scale: 0.95, weight: 1 }, { asset: "tree.pine", scale: 1.15, weight: 1.5 }, { asset: "tree.birch", scale: 1.1, weight: 0.8 }], [{ asset: "bush", scale: 1.1, count: 2 }, { asset: "plant.grass", param: "dry", scale: 1.4, count: 2 }], 4, EDGE);
  return mix([{ asset: "tree.oak", scale: 1.25, weight: 2 }, { asset: "tree.oak", scale: 0.95, weight: 1.2 }, { asset: "tree.birch", scale: 1.15, weight: 1.1 }, { asset: "tree.pine", scale: 1.15, weight: 1.1 }], [{ asset: "bush", scale: 1.1, count: 2 }, { asset: "plant.fern", scale: 1.3, count: 2 }], 4, EDGE);
}

// Lone trees and groves on open ground, by ground.
const OPEN_TREES = {
  grass: [{ asset: "tree.oak", scale: 1.3, weight: 2 }, { asset: "tree.pine", scale: 1.2, weight: 1.8 }, { asset: "tree.birch", scale: 1.2, weight: 0.5 }],
  hills: [{ asset: "tree.pine", scale: 1.2, weight: 2.2 }, { asset: "tree.oak", scale: 1.25, weight: 1 }],
  plains: [{ asset: "tree.pine", scale: 1.2, weight: 1.6 }, { asset: "tree.oak", scale: 1.2, weight: 1 }],
  tundra: [{ asset: "tree.pine.snow", scale: 0.95, weight: 2 }, { asset: "tree.pine", scale: 0.95, weight: 1 }, { asset: "tree.birch", scale: 1, weight: 0.4 }],
};

/** Open ground's plants: entries for dress.js `scatterFlora` -- `count` at most (the density field
 *  thins it) in clumps of `clump`, `size` 0 for low clutter, 1 bushes, 2 trees (a `mix` of
 *  species). A meadow's flowers share a colour (one variant for the plot). */
function floraOf(ctx) {
  const { r, ground: g } = ctx;
  if (ctx.peak) return [];
  const woods = ctx.woods ?? 0;
  const trees = (p, n) => (r() < p + 0.1 * woods ? r.int(1, n) : 0);
  // (Scaled up: the clutter reads as clumps at the game's zoom -- a tuft ~0.1 of a plot across, a
  // bush ~0.15 -- while staying under a unit's knee.)
  const grass = (n, o = {}) => ({ asset: "plant.grass", scale: 1.55, count: n, clump: [2, 5], spread: 0.08, dense: 0.75, ...o });
  const flowers = (n, colours) => ({ asset: "plant.flowers", variant: r.pick(colours), scale: 1.4, count: n, clump: [2, 4], spread: 0.06, dense: 0.8 });
  const bushes = (n, o = {}) => ({ asset: "bush", scale: 1.3, count: n, clump: [1, 3], spread: 0.08, size: 1, ...o });
  const scrub = (n, o = {}) => ({ mix: [{ asset: "shrub.dry", variant: 0 }, { asset: "shrub.dry", variant: 1 }], scale: 1.3, count: n, size: 1, ...o });
  const grove = (list, n, clump = [1, 4]) => ({ mix: list, count: n, clump, spread: 0.11, size: 2, dense: 0.8 });
  if (isHell(g)) {
    if (g === GROUND.BURNING_SANDS) return [{ asset: "tree.dead", scale: 0.55, count: r() < 0.15 ? 1 : 0, size: 2 }];
    return [{ asset: "tree.blighted", scale: 0.6, count: r() < 0.25 ? r.int(1, 2) : 0, size: 2 }, { asset: "tree.dead", scale: 0.55, count: r() < 0.2 ? 1 : 0, size: 2 }];
  }
  if (ctx.feat === FEAT.SCRUB) return [scrub(r.int(3, 6), { clump: [1, 2], spread: 0.08 }), { asset: "shrub.dry", scale: 1.3, count: r.int(0, 2), size: 1 }, grass(r.int(4, 8), { param: "dry", scale: 1.3 })];
  if (ctx.feat === FEAT.FLOOD) return [{ asset: "reeds", scale: 1.1, count: r.int(3, 6), clump: [1, 3], spread: 0.08, size: 1 }, grass(r.int(4, 8))];
  switch (g) {
    case GROUND.GRASS:
      // Meadow: tufts and flowers in drifts, bushes, and trees in clumps, conifers among the
      // broadleaves (pines leading on hills).
      return [
        grass(r.int(18, 30)),
        grass(r.int(0, 6), { param: "dry" }),
        flowers(r.int(4, 10), [0, 1, 2, 3, 4, 5]),
        bushes(r.int(3, 6) + (woods ? 2 : 0) + (ctx.hills ? 1 : 0)),
        bushes(r.int(0, 2), { scale: 1.9, clump: [1, 2] }),
        grove(ctx.hills ? OPEN_TREES.hills : OPEN_TREES.grass, trees(ctx.hills ? 0.6 : 0.5, 5)),
      ];
    case GROUND.PLAINS:
      // Golden grass: dry tufts in drifts, a few bushes and flowers, lone conifers.
      return [
        grass(r.int(18, 30), { param: "dry", scale: 1.65, clump: [2, 6], spread: 0.09 }),
        grass(r.int(1, 4), { clump: [1, 3] }),
        flowers(r.int(0, 4), [2, 3, 0]),
        bushes(r.int(0, 2) + (woods ? 1 : 0) + (ctx.hills ? 1 : 0), { scale: 1.15 }),
        scrub(r.int(0, 2)),
        grove(ctx.hills ? OPEN_TREES.hills : OPEN_TREES.plains, trees(0.32, 3), [1, 2]),
      ];
    case GROUND.TUNDRA:
      return [
        grass(r.int(5, 10), { param: "dry", scale: 1.3 }),
        flowers(r.int(0, 2), [0, 5]),
        bushes(r.int(0, 2), { scale: 0.95 }),
        grove(OPEN_TREES.tundra, trees(0.35, 3), [1, 3]),
      ];
    case GROUND.SNOW:
      return [grove([{ asset: "tree.pine.snow", scale: 0.85 }], trees(0.15, 2), [1, 2])];
    case GROUND.DESERT:
      return [{ asset: "shrub.dry", scale: 1.3, count: r.int(1, 3), clump: [1, 2], spread: 0.08, size: 1 }, grass(r.int(0, 3), { param: "dry", scale: 1.2, clump: [1, 2] })];
    case GROUND.MARSH:
      return [
        { asset: "reeds", scale: 1.1, count: r.int(4, 7), clump: [1, 3], spread: 0.08, size: 1 },
        grass(r.int(4, 8)),
        bushes(r.int(0, 2), { scale: 1.1 }),
        grove([{ asset: "tree.willow", scale: 0.95 }], trees(0.2, 2), [1, 2]),
      ];
    default:
      return [];
  }
}

extendCatalog({ forest: forestMix, flora: floraOf });
