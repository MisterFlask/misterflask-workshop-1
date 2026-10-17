// vegetation.js: the trees and plants of the 3D map and the catalog's `forest` and `flora` entries
// (docs/R3D.md "Asset recipes").
//
// Visual language: every tree is a readable silhouette first -- a visible trunk with a root flare
// sunk below the ground, a few limbs, and a canopy built from clusters of lumpy, flat-bottomed leaf
// masses (never one sphere) with gaps and overhangs. Foliage is painted in two to three tones baked
// into the vertices (a cool dark underside, a mid body, a warm sunlit crown) by height and normal,
// with soft spherical normals per mass and per crown so the light reads as broad painted masses.
// Conifers are ragged, drooping, star-edged tiers; the ancient trees are twisted, buttressed giants
// with a tiered, mallorn-like canopy and a few lantern-fruits; burnt and blighted woods are black,
// clawed snags with sparse embers. Variety stays inside each identity: size, lean, mass count and a
// small hue drift per variant. Colours come from palette.js (PAL) plus a few vegetation tones below.
//
// Recipes (names are a contract): tree.oak tree.birch tree.pine (param "snow") tree.pine.snow
// tree.jungle tree.palm tree.ancient tree.dead tree.blighted (the infernal woods) tree.willow (marsh)
// tree.sapling (param "broad" | "pine" | "snow" | "ancient"; none: broadleaf variants 0-3, conifer
// 4-5) tree.sapling.pine tree.sapling.ancient bush shrub.dry reeds plant.fern plant.mushroom
// plant.mushroom.glow plant.grass. The ".snow"/".pine"/".ancient" aliases exist because the
// catalog's flora and undergrowth entries cannot pass a param.

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

const OAK = ["#182a22", "#28402a", "#3f5a30", "#6f7d40"];
const OAK_AUTUMN = ["#1a2a20", "#2e4128", "#4c5a2e", "#807238"];
const BIRCH = ["#22362a", "#38502f", "#5a7038", "#929a54"];
const PINE = ["#0d1f1c", "#183429", "#274a3d", "#557259"];
const JUNGLE = ["#0a1e14", "#14341e", "#234b25", "#4a6e33"];
const WILLOW = ["#1a2c22", "#2c4429", "#465e34", "#728048"];
const ANCIENT = ["#0b2321", "#15372f", "#2c554a", "#688a66", "#a6bf8e"];
const BLIGHT = ["#120a0e", "#24121a", "#3a1a1e", "#6a3424"];
const BUSH = ["#1a2c20", "#2a4024", "#3a5228", "#66743a"];
const PALM = ["#1e3622", "#2d4c26", "#486c30", "#86904c"];
const SAGE = ["#2c3428", "#434d3a", "#5f6a50", "#8a9272"];
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
 *  leaves have flat bellies), normals bent out from its own centre by `self`. Returns its mark. */
function leafMass(mb, c, rad, { detail = 1, amp = 0.3, squash = 0.8, flat = 0.4, seed = 0, freq = 1.6, self = 0.5 } = {}) {
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
 *  detail]. Returns the foliage mark (the limbs come before it). */
function crown(mb, r, { C, W, hc, fork, layout, squash = 0.78, amp = 0.32, flat = 0.4, limbR = 0.01, limbs = 3, bark, nTufts = 0 }) {
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
    leafMass(mb, c, rad * W, { detail: detail ?? 1, amp, squash, flat, seed: r.range(0, 50), self: 0.55 });
    cores.push([c, rad * W, squash]);
  }
  if (nTufts) tufts(mb, r, cores, nTufts);
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
function tufts(mb, r, cores, n, { size = [0.32, 0.46], minUp = -0.25, out = 0.82 } = {}) {
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
    leafMass(mb, p, rad * r.range(size[0], size[1]), { detail: 0, amp: 0.15, squash: 0.85, flat: 0.3, seed: r.range(0, 50), self: 0.4 });
    made++;
  }
}

// --- tree.oak -----------------------------------------------------------------------------------------

/** The common broadleaf: a stout trunk with a root flare forking into a few boughs under a broad,
 *  lumpy crown of separate masses with overhangs and gaps, scalloped by small tufts. Shapes cycle:
 *  broad dome, twin-stemmed (forked low, two lobes), leaning with a heavy low bough, split crown. */
function buildOak(mb, r, variant, cols = OAK) {
  const shape = variant % 4;
  const H = r.range(0.3, 0.35) * (shape === 1 ? 1.08 : 1);
  const W = r.range(0.122, 0.136) * (shape === 0 ? 1.06 : 1);
  const forkY = H * (shape === 1 ? r.range(0.18, 0.22) : r.range(0.28, 0.33));
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
  const hc = (H - Math.max(forkY, H * 0.3)) * 0.5;
  const C = [lx * 1.6, H - hc * 0.95, lz * 1.6];
  let layout;
  if (shape === 0) {
    // Broad dome with a scalloped, overhanging skirt.
    layout = [[0, 0, 0.3, 0.56], ...ringLayout(r, 4, { dist: [0.66, 0.82], dy: [-0.5, 0.05], rad: [0.4, 0.48] })];
  } else if (shape === 1) {
    // Twin-stemmed: forked low into two leaders, each carrying its own lobe of the crown.
    const a = r.range(0, TAU);
    layout = [[a, 0.6, 0.15, 0.52], [a + Math.PI, 0.6, 0.0, 0.52], [a + 0.9, 0.75, -0.4, 0.4], [a + Math.PI + 0.9, 0.7, 0.4, 0.42], [a + Math.PI / 2, 0.15, 0.6, 0.42]];
  } else if (shape === 2) {
    // A heavy bough reaching out on the leaning side, low.
    layout = [[0, 0, 0.25, 0.58], [la + r.range(-0.3, 0.3), 0.95, -0.4, 0.46], [la + Math.PI + r.range(-0.5, 0.5), 0.6, 0.4, 0.44], ...ringLayout(r, 2, { dist: [0.6, 0.75], dy: [-0.35, 0.25], rad: [0.38, 0.46] })];
  } else {
    // Split crown: two lobes with a gap between them.
    const a = r.range(0, TAU);
    layout = [[a, 0.55, 0.3, 0.55], [a + Math.PI, 0.6, 0.05, 0.55], [a + 1.5, 0.75, -0.45, 0.42], [a + Math.PI + 1.3, 0.5, 0.65, 0.45]];
  }
  const m0 = crown(mb, r, { C, W, hc, fork, layout, limbR: tr * (shape === 1 ? 0.75 : 0.55), limbs: shape === 1 ? 2 : 3, bark, squash: 0.72, nTufts: layout.length > 4 ? 4 : 7 });
  shadeBark(mb, w0, { foot: MOSS, footAmt: 0.55, y0: -0.02, y1: 0.07 });
  mb.heightAO(w0, -0.02, forkY, 0.55);
  mb.sphereNormals(m0, [C[0], C[1] - hc * 0.6, C[2]], 0.45);
  shadeFoliage(mb, m0, { lo: forkY, hi: C[1] + hc * 1.2, cols, center: C, rad: W * 1.1, seed: r.range(0, 9) });
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
  const W = [0.07, 0.082, 0.092][stems - 1] * r.range(0.92, 1.08);
  const y0 = Math.max(0.12, H * 0.45), y1 = H - 0.01;
  const lo = y0 - W * 0.5, hi = y1 + W * 0.4;
  const ph = r.range(0, TAU);
  const n = 6;
  for (let k = 0; k < n; k++) {
    const u = k / (n - 1);
    const a = ph + k * 2.4;
    const d = W * (0.6 - 0.42 * u) * r.range(0.8, 1.1);
    const c = [ox + Math.cos(a) * d, y0 + (y1 - y0) * u, oz + Math.sin(a) * d];
    mb.color = jitter(WHITE, r, 0.07);
    leafMass(mb, c, W * (0.72 - 0.3 * u) * r.range(0.9, 1.1), { detail: 1, amp: 0.4, squash: 0.9, flat: 0.2, seed: r.range(0, 50), self: 0.45, freq: 2.0 });
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
  const H = small ? r.range(0.15, 0.21) : r.range(0.36, 0.43) * (slim ? 1.05 : scraggly ? 0.95 : 1);
  const R = (slim ? 0.072 : scraggly ? 0.1 : 0.095) * r.range(0.9, 1.1) * (small ? 0.68 : 1);
  const tiers = small ? 3 : scraggly ? 5 : slim ? 7 : r.int(5, 6);
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
      jag: r.range(0.32, 0.46),
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

/** A rainforest emergent: a tall, pale, slightly S-curved trunk on thin buttress fins, a broad dark
 *  umbrella crown of flattened masses with a lower side tier, and a few hanging vines. */
function buildJungle(mb, r, variant) {
  const H = r.range(0.37, 0.43);
  const W = r.range(0.13, 0.15) * (variant % 3 === 2 ? 1.08 : 1);
  const forkY = H * r.range(0.56, 0.64);
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
  const fork = [lx, forkY, lz];
  const C = [lx * 1.3, H - W * 0.28, lz * 1.3];
  shadeBark(mb, w0, { foot: MOSS, footAmt: 0.5, y0: -0.02, y1: 0.12 });
  mb.heightAO(w0, -0.02, forkY, 0.5);
  // Hanging vines (lianas) from the crown's underside.
  const v0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  mb.color = pal("#26401f");
  const vines = r.int(2, 4);
  for (let k = 0; k < vines; k++) {
    const a = r.range(0, TAU), d = W * r.range(0.45, 0.85);
    const top = [C[0] + Math.cos(a) * d, C[1] - W * 0.15, C[2] + Math.sin(a) * d];
    const len = r.range(0.07, 0.15);
    mb.tube([top, [top[0] + r.range(-0.01, 0.01), top[1] - len * 0.5, top[2] + r.range(-0.01, 0.01)], [top[0] + r.range(-0.01, 0.01), top[1] - len, top[2]]], [0.0032, 0.0026, 0.0016], 3);
  }
  eachVert(mb, v0, (v, i) => {
    v[i + 10] = 0.7 * clamp((C[1] - v[i + 1]) / 0.12, 0, 1);
  });
  // The umbrella: a centre and a ring of four flattened masses.
  // The umbrella: a centre, a ring of three flattened masses and a lower side tier.
  const layout = [[0, 0, 0.2, 0.56], [r.range(0, TAU), 1.05, -1.0, 0.5], ...ringLayout(r, 3, { dist: [0.6, 0.75], dy: [-0.3, 0.1], rad: [0.44, 0.52] })];
  const m0 = crown(mb, r, { C, W, hc: W * 0.5, fork, layout, squash: 0.5, flat: 0.55, amp: 0.3, limbR: tr * 0.6, limbs: 3, bark });
  mb.sphereNormals(m0, [C[0], C[1] - W * 0.4, C[2]], 0.35);
  shadeFoliage(mb, m0, { lo: C[1] - W * 0.8, hi: H, cols: JUNGLE, center: C, rad: W * 1.2, seed: r.range(0, 9), hw: 0.3, upw: 0.42 });
  swayFoliage(mb, m0, forkY - 0.05, H, W, 0.75, C[0], C[2]);
}

recipe("tree.jungle", { variants: 6, build: (mb, r, { variant }) => buildJungle(mb, r, variant) });

// --- tree.palm ----------------------------------------------------------------------------------------

/** A palm: a ringed, gracefully bent trunk swelling at its foot, a starburst of arching, drooping
 *  fronds (folded leaf blades, yellowing at the tips), a dead frond or two hanging and a few nuts. */
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
  const [deep, dark, mid, lite] = PALM.map(lin);
  const dry = pal("#9a8248");
  const nf = r.int(7, 9);
  const fph = r.range(0, TAU);
  const frond = (a, L, elev, drop, isDry) => {
    const dir = [Math.cos(a), 0, Math.sin(a)];
    const side = [-Math.sin(a), 0, Math.cos(a)];
    const L0 = [], S0 = [], R0 = [], kL = [], kS = [], kR = [];
    const m = 6;
    for (let j = 0; j < m; j++) {
      const s = j / (m - 1);
      const sp = [top[0] + dir[0] * L * s, top[1] + L * (elev * s - drop * s * s), top[2] + dir[2] * L * s];
      const w = L * 0.24 * Math.pow(Math.sin(Math.PI * Math.min(1, 0.15 + s * 0.95)), 0.7);
      const sag = w * 0.55;
      L0.push([sp[0] + side[0] * w, sp[1] - sag, sp[2] + side[2] * w]);
      S0.push(sp);
      R0.push([sp[0] - side[0] * w, sp[1] - sag, sp[2] - side[2] * w]);
      const c = isDry ? vlerp(dry, pal("#5e4a30"), 0.5 * (1 - s)) : s < 0.5 ? vlerp(dark, mid, s * 2) : vlerp(mid, lite, (s - 0.5) * 2);
      kS.push(c);
      kL.push(isDry ? c : vlerp(c, deep, 0.35));
      kR.push(isDry ? c : vlerp(c, deep, 0.35));
    }
    surface(mb, [L0, S0, R0], [kL, kS, kR], { up: [0, 1, 0] });
  };
  for (let k = 0; k < nf; k++) {
    const a = fph + (k / nf) * TAU + r.range(-0.25, 0.25);
    frond(a, r.range(0.12, 0.155), r.range(0.35, 0.75), r.range(0.9, 1.25), false);
  }
  // A young upright frond in the heart and a dead one hanging.
  frond(r.range(0, TAU), 0.07, 1.6, 1.1, false);
  if (variant % 2 === 0) frond(fph + 0.4, 0.1, -0.7, 0.4, true);
  mb.sphereNormals(m0, [top[0], top[1] - 0.04, top[2]], 0.35);
  swayFoliage(mb, m0, top[1] - 0.04, top[1] + 0.04, 0.14, 1, top[0], top[2]);
}

recipe("tree.palm", { variants: 6, build: (mb, r, { variant }) => buildPalm(mb, r, variant) });

// --- tree.ancient -------------------------------------------------------------------------------------

/** The great trees of the ancient forests (elven, mallorn-like): a massive trunk of twisted strands
 *  on sprawling buttress roots, each strand unravelling into a great bough that carries a broad, flat
 *  mass of the lower canopy, under a dome of foliage over the trunk -- a two-tiered, spreading crown,
 *  teal with pale sunlit tops; silvered bark, mossy at the foot; a few lantern-fruits glowing pale
 *  under the boughs. Variants: grand, broad, gnarled and leaning, younger. */
function buildAncient(mb, r, variant) {
  const kind = variant % 4;
  const H = [1.0, 0.92, 0.88, 0.8][kind] * r.range(0.96, 1.04);
  const Rc = [0.4, 0.44, 0.38, 0.33][kind] * r.range(0.95, 1.05);
  const forkY = H * [0.42, 0.4, 0.42, 0.45][kind];
  const la = r.range(0, TAU);
  const lean = kind === 2 ? 0.08 : 0.02;
  const R0 = [0.095, 0.1, 0.09, 0.075][kind];
  const axis = (t) => [Math.cos(la) * lean * t * t, Math.sin(la) * lean * t * t];
  const [ax, az] = axis(1);
  // The crown: great boughs, each ending in a broad flat mass of the lower canopy (at varied
  // heights), and a dome over the trunk.
  const nb = kind < 2 ? 6 : 5;
  const th0 = r.range(0, TAU);
  const boughs = [];
  for (let k = 0; k < nb; k++) {
    const a = th0 + (k / nb) * TAU + r.range(-0.25, 0.25);
    const d = Rc * r.range(0.56, 0.72);
    boughs.push({ a, c: [ax + Math.cos(a) * d, forkY + H * r.range(0.05, 0.17), az + Math.sin(a) * d], rad: Rc * r.range(0.4, 0.48) });
  }
  const nd = kind === 3 ? 3 : 4;
  const dph = r.range(0, TAU);
  const dome = [];
  for (let k = 0; k < nd; k++) {
    const a = dph + (k / nd) * TAU + r.range(-0.3, 0.3);
    const d = Rc * r.range(0.24, 0.34);
    dome.push({ c: [ax + Math.cos(a) * d, H - Rc * 0.22 + r.range(-0.03, 0.02), az + Math.sin(a) * d], rad: Rc * r.range(0.42, 0.48) });
  }
  const bark = jitter(pal(BARK_ANCIENT), r, 0.04);
  const w0 = mb.mark();
  mb.color = bark;
  // The core of the trunk, rising on as the leader into the dome.
  const lead = [ax, H - Rc * 0.4, az];
  mb.tube([0, 0.33, 0.66, 1].map((t) => { const [x, z] = axis(t); return [x, -0.05 + (forkY + 0.08) * t, z]; }).concat([lead]), [R0 * 0.9, R0 * 0.78, R0 * 0.7, R0 * 0.6, R0 * 0.38], 8);
  for (const m of dome) limb(mb, [lead[0], lead[1] - 0.06, lead[2]], [m.c[0], m.c[1] - m.rad * 0.3, m.c[2]], R0 * 0.24, R0 * 0.12, 4, 0.5);
  // Twisted strands, each unravelling into a bough.
  const twist = r.range(0.9, 1.5) * r.sign();
  boughs.forEach((m, k) => {
    const thk = m.a - twist;
    const P = [], Rr = [];
    const rs = R0 * 0.5;
    for (let j = 0; j <= 5; j++) {
      const t = j / 5;
      const th = thk + twist * t;
      const rr = R0 * (0.68 + 0.9 * Math.pow(1 - t, 3));
      const [x, z] = axis(t);
      P.push([x + Math.cos(th) * rr, -0.05 + (forkY + 0.02) * t, z + Math.sin(th) * rr]);
      Rr.push(rs * (1 - 0.3 * t) * (1 + 0.7 * Math.pow(1 - t, 4)));
    }
    const p5 = P[5];
    const end = [m.c[0], m.c[1] - m.rad * 0.15, m.c[2]];
    P.push([p5[0] + (end[0] - p5[0]) * 0.35, p5[1] + (end[1] - p5[1]) * 0.6 + 0.025, p5[2] + (end[2] - p5[2]) * 0.35]);
    P.push([p5[0] + (end[0] - p5[0]) * 0.72, p5[1] + (end[1] - p5[1]) * 0.9 + 0.012, p5[2] + (end[2] - p5[2]) * 0.72]);
    P.push(end);
    Rr.push(rs * 0.62, rs * 0.45, rs * 0.25);
    mb.tube(P, Rr, 6);
  });
  // Buttress roots between the strands, arching out over the ground and diving into it.
  for (let k = 0; k < nb; k++) {
    const a = th0 + ((k + 0.5) / nb) * TAU + r.range(-0.25, 0.25);
    const ca = Math.cos(a), sa = Math.sin(a);
    const L = r.range(0.17, 0.24) * (kind === 3 ? 0.8 : 1);
    const q = (d, y) => [ca * d + r.range(-0.006, 0.006), y, sa * d + r.range(-0.006, 0.006)];
    mb.tube([q(R0 * 0.6, 0.11), q(R0 * 1.5, 0.055), q(L * 0.62, 0.012), q(L, -0.055)], [R0 * 0.42, R0 * 0.36, R0 * 0.22, R0 * 0.1], 5);
  }
  shadeBark(mb, w0, { foot: "#33402f", footAmt: 0.7, y0: 0, y1: 0.22, up: "#b4b8ae", upAmt: 0.45 });
  mb.heightAO(w0, -0.03, forkY * 0.8, 0.5);
  // The foliage: broad, flat-bellied masses on the boughs, a rounder dome, small tufts scalloping
  // the rims.
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  const cores = [];
  for (const m of boughs) {
    mb.color = jitter(WHITE, r, 0.06);
    leafMass(mb, m.c, m.rad, { detail: 1, amp: 0.3, squash: 0.4, flat: 0.6, seed: r.range(0, 50), self: 0.5, freq: 1.8 });
    cores.push([m.c, m.rad, 0.4]);
  }
  for (const m of dome) {
    mb.color = jitter(WHITE, r, 0.06);
    leafMass(mb, m.c, m.rad, { detail: 1, amp: 0.3, squash: 0.48, flat: 0.5, seed: r.range(0, 50), self: 0.5, freq: 1.8 });
    cores.push([m.c, m.rad, 0.48]);
  }
  tufts(mb, r, cores, 10, { size: [0.3, 0.42], minUp: -0.1, out: 0.86 });
  const mid = [ax, forkY + (H - forkY) * 0.35, az];
  mb.sphereNormals(m0, mid, 0.3);
  // Each giant drifts a little in hue: bluer, greener or more golden teal.
  const drift = [[0.96, 1, 1.06], [1, 1.02, 0.94], [1.06, 1.03, 0.9], [0.98, 1.01, 1.02]][kind];
  shadeFoliage(mb, m0, { lo: forkY, hi: H, cols: ANCIENT.map((c) => lin(c).map((x, q) => x * drift[q])), center: [ax, (forkY + H) * 0.55, az], rad: Rc * 1.15, seed: r.range(0, 9), hw: 0.14, upw: 0.5, bias: 0.04, nw: 0.28, aoMin: 0.5 });
  swayFoliage(mb, m0, forkY, H, Rc, 0.45, ax, az);
  // Beards of pale moss hanging from the boughs.
  const b0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  for (let k = 0; k < 7; k++) {
    const m = boughs[k % boughs.length];
    const a = r.range(0, TAU), d = m.rad * r.range(0.2, 0.7);
    const top = [m.c[0] + Math.cos(a) * d, m.c[1] - m.rad * 0.3, m.c[2] + Math.sin(a) * d];
    const len = r.range(0.05, 0.1);
    mb.color = jitter(pal("#7b8a6c"), r, 0.08);
    mb.tube([top, [top[0] + r.range(-0.006, 0.006), top[1] - len * 0.55, top[2] + r.range(-0.006, 0.006)], [top[0], top[1] - len, top[2]]], [0.005, 0.0035, 0.001], 3);
  }
  eachVert(mb, b0, (v, i) => (v[i + 10] = 0.45));
  // Lantern-fruits: a few pale lights hanging on threads at the rims of the boughs.
  const glows = [5, 5, 4, 3][kind];
  for (let k = 0; k < glows; k++) {
    const m = boughs[(k * 2 + 1) % boughs.length];
    const a = m.a + r.range(-0.7, 0.7), d = m.rad * r.range(0.6, 0.85);
    const hang = [m.c[0] + Math.cos(a) * d, m.c[1] - m.rad * 0.25, m.c[2] + Math.sin(a) * d];
    const fruit = [hang[0], hang[1] - r.range(0.04, 0.08), hang[2]];
    const g0 = mb.mark();
    mb.mat = MAT.MATTE;
    mb.color = pal("#4f5e48");
    mb.tube([hang, fruit], 0.0018, 3);
    mb.mat = MAT.EMISSIVE;
    mb.color = vlerp(pal(PAL.ancientGlow), pal("#9ee0c0"), r());
    mb.part(() => {
      mb.translate(fruit[0], fruit[1], fruit[2]);
      mb.sphere(r.range(0.014, 0.018), 0, { squash: 1.3 });
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
  shadeBark(mb, w0, { foot: "#140c0e", footAmt: 0.5, y0: 0, y1: 0.08, up: "#5a4248", upAmt: 0.4 });
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

/** A marsh willow: a short, thick, leaning trunk, arching limbs, a domed crown and, around it, long
 *  hanging curtains of leaves with a ragged hem, lit on the outside, dark within. */
function buildWillow(mb, r, variant) {
  const H = r.range(0.27, 0.32);
  const W = r.range(0.11, 0.13);
  const forkY = r.range(0.1, 0.12);
  const la = r.range(0, TAU), ln = r.range(0.015, 0.035);
  const lx = Math.cos(la) * ln, lz = Math.sin(la) * ln;
  const tr = r.range(0.019, 0.023);
  const bark = jitter(pal(BARK_WILLOW), r, 0.05);
  const w0 = mb.mark();
  mb.color = bark;
  rootFlare(mb, 0, 0, tr, { h: 0.035, lobes: 4, seed: r.range(0, 6) });
  mb.tube([[0, 0.025, 0], [lx * 0.5, forkY * 0.6, lz * 0.5], [lx, forkY, lz], [lx * 1.2, forkY + 0.04, lz * 1.2]], [tr, tr * 0.9, tr * 0.75, tr * 0.5], 6);
  const hc = (H - forkY) * 0.36;
  const C = [lx * 1.3, H - hc, lz * 1.3];
  const layout = [[0, 0, 0.25, 0.6], ...ringLayout(r, 3, { dist: [0.5, 0.62], dy: [-0.1, 0.25], rad: [0.44, 0.52] })];
  const m0 = crown(mb, r, { C, W, hc, fork: [lx, forkY, lz], layout, limbR: tr * 0.5, limbs: 3, bark, squash: 0.7, amp: 0.25, flat: 0.3 });
  shadeBark(mb, w0, { foot: MOSS, footAmt: 0.6, y0: -0.02, y1: 0.08 });
  mb.heightAO(w0, -0.02, forkY, 0.5);
  const y0 = C[1] - hc * 0.15;
  mb.sphereNormals(m0, [C[0], C[1] - hc * 0.5, C[2]], 0.45);
  shadeFoliage(mb, m0, { lo: y0 - 0.02, hi: H, cols: WILLOW, center: C, rad: W * 1.1, seed: r.range(0, 9), hw: 0.4, upw: 0.3 });
  swayFoliage(mb, m0, y0, H, W, 0.8, C[0], C[2]);
  // The curtains: hanging streamers of leaves around the crown's waist, arching out a little and
  // falling, ragged in length, with gaps that show the dark interior and the trunk.
  const S = 18;
  const ph = r.range(0, TAU);
  const [deep, dark, mid, lite] = WILLOW.map(lin);
  const c0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  for (let s = 0; s < S; s++) {
    const a = ph + (s / S) * TAU + r.range(-0.12, 0.12);
    const ca = Math.cos(a), sa = Math.sin(a);
    const side = [-sa, 0, ca];
    const rr = W * r.range(0.8, 0.95);
    const len = (s % 3 === 1 ? r.range(0.45, 0.65) : r.range(0.75, 1.0)) * (y0 - 0.02);
    const w = r.range(0.014, 0.019);
    const spine = [0, 0.3, 0.65, 1].map((u) => [C[0] + ca * rr * (1 + 0.16 * Math.sin(u * 2.2)), y0 + 0.01 - len * u, C[2] + sa * rr * (1 + 0.16 * Math.sin(u * 2.2))]);
    const left = spine.map((p, j) => vadd(p, vscale(side, w * (1 - 0.6 * (j / 3)))));
    const right = spine.map((p, j) => vsub(p, vscale(side, w * (1 - 0.6 * (j / 3)))));
    const j = jitter(WHITE, r, 0.1);
    const ks = spine.map((_, q) => vlerp(vlerp(dark, mid, 0.45), vlerp(mid, lite, 0.4), q / 3).map((x, c) => x * j[c]));
    surface(mb, [left, spine, right], [ks.map((k) => vlerp(k, deep, 0.25)), ks, ks.map((k) => vlerp(k, deep, 0.25))], { ref: [C[0], y0 - len * 0.5, C[2]] });
  }
  eachVert(mb, c0, (v, i) => {
    v[i + 10] = 0.3 + 0.7 * clamp((y0 - v[i + 1]) / 0.15, 0, 1);
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

/** A tiny octahedron (berries, blossom, specks): eight triangles. */
function octa(mb, p, rad) {
  const A = [[rad, 0, 0], [0, rad, 0], [-rad, 0, 0], [0, -rad * 0.7, 0], [0, 0, rad], [0, 0, -rad]].map((d) => vadd(p, d));
  for (const [a, b] of [[0, 1], [1, 2], [2, 3], [3, 0]]) {
    for (const c of [4, 5]) {
      const n = vnorm(vsub(vscale(vadd(vadd(A[a], A[b]), A[c]), 1 / 3), p));
      triC(mb, A[a], A[b], A[c], n, mb.color, mb.color, mb.color);
    }
  }
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
  // An upright, irregular young crown: a tall main mass, a smaller one low on one side and a tuft
  // at the top, leaning a little.
  const W = r.range(0.042, 0.05);
  const C = [top[0] * 0.8, H * 0.6, top[2] * 0.8];
  mb.color = jitter(WHITE, r, 0.06);
  leafMass(mb, C, W * 0.72, { detail: 1, squash: kind === "ancient" ? 0.9 : 1.3, flat: 0.25, seed: r.range(0, 50), amp: 0.35 });
  const b = a + r.range(1.5, 3.5);
  mb.color = jitter(WHITE, r, 0.08);
  leafMass(mb, [C[0] + Math.cos(b) * W * 0.62, H * 0.42, C[2] + Math.sin(b) * W * 0.62], W * r.range(0.46, 0.54), { detail: 0, amp: 0.15, squash: 0.85, flat: 0.3, seed: r.range(0, 50) });
  mb.color = jitter(WHITE, r, 0.08);
  leafMass(mb, [top[0] + Math.cos(a) * W * 0.3, H - W * 0.38, top[2] + Math.sin(a) * W * 0.3], W * r.range(0.4, 0.48), { detail: 0, amp: 0.15, squash: 1.1, flat: 0.2, seed: r.range(0, 50) });
  mb.sphereNormals(m0, [C[0], C[1] - W * 0.3, C[2]], 0.45);
  shadeFoliage(mb, m0, { lo: H * 0.35, hi: H, cols: kind === "ancient" ? ANCIENT : OAK, center: C, rad: W * 1.2, seed: r.range(0, 9), bias: 0.04, aoMin: 0.65 });
  swayFoliage(mb, m0, H * 0.35, H, W, 1, C[0], C[2]);
}

recipe("tree.sapling", { variants: 6, build: (mb, r, { variant, param }) => buildSapling(mb, r, variant, param) });
recipe("tree.sapling.pine", { variants: 6, build: (mb, r, { variant }) => buildSapling(mb, r, variant, "pine") });
recipe("tree.sapling.ancient", { variants: 6, build: (mb, r, { variant }) => buildSapling(mb, r, variant, "ancient") });

/** A bush: one leafy mound and two or three smaller ones leaning on it, sunk into the ground; one
 *  variant carries dark berries, one pale blossom (soft specks painted into the vertices). */
function buildBush(mb, r, variant) {
  const H = r.range(0.06, 0.1);
  const W = r.range(0.06, 0.08);
  const m0 = mb.mark();
  mb.mat = MAT.FOLIAGE;
  mb.color = jitter(WHITE, r, 0.06);
  const C = [0, H * 0.45, 0];
  leafMass(mb, C, W * 0.62, { detail: 1, squash: (H * 0.55) / (W * 0.62), flat: 0, seed: r.range(0, 50), amp: 0.3 });
  const n = variant >= 4 ? 2 : r.int(2, 3), ph = r.range(0, TAU);
  for (let k = 0; k < n; k++) {
    const a = ph + (k / n) * TAU + r.range(-0.4, 0.4);
    mb.color = jitter(WHITE, r, 0.08);
    leafMass(mb, [Math.cos(a) * W * 0.55, H * r.range(0.2, 0.35), Math.sin(a) * W * 0.55], W * r.range(0.36, 0.46), { detail: 0, amp: 0.15, squash: 0.75, flat: 0, seed: r.range(0, 50) });
  }
  // Sink the bottom into the ground.
  eachVert(mb, m0, (v, i) => {
    if (v[i + 1] < 0.004) v[i + 1] = Math.max(-0.03, v[i + 1] * 1.5 - 0.01);
  });
  mb.sphereNormals(m0, [0, 0, 0], 0.5);
  shadeFoliage(mb, m0, { lo: 0, hi: H, cols: BUSH, center: [0, H * 0.3, 0], rad: W, seed: r.range(0, 9), aoMin: 0.6, hw: 0.45 });
  // Berries or blossom: small specks over the upper faces.
  const speck = variant === 4 ? "#6a1e26" : variant === 5 ? "#d9d2b4" : null;
  if (speck) {
    mb.mat = MAT.MATTE;
    for (let k = 0; k < 5; k++) {
      const u = r.range(0.2, 0.95), b = r.range(0, TAU), q = Math.sqrt(1 - u * u);
      const p = [Math.cos(b) * q * W * 0.6, H * 0.45 + u * H * 0.52, Math.sin(b) * q * W * 0.6];
      mb.color = jitter(pal(speck), r, 0.1);
      octa(mb, p, r.range(0.005, 0.007));
    }
  }
  swayFoliage(mb, m0, 0, H, W, 0.35);
}

recipe("bush", { variants: 6, build: (mb, r, { variant }) => buildBush(mb, r, variant) });

/** Reeds: a clump of arching blades, olive at the foot, straw at the tips, with a few cattails. */
function buildReeds(mb, r, variant) {
  const n = 22 + (variant % 2) * 4;
  const base = pal("#34452a"), tip = pal(PAL.reed), pale = pal("#6f7c40");
  mb.mat = MAT.FOLIAGE;
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
    // Sagebrush: a low, wide, dusty clump.
    const m0 = mb.mark();
    mb.mat = MAT.FOLIAGE;
    mb.color = jitter(WHITE, r, 0.06);
    leafMass(mb, [0, 0.014, 0], 0.042, { detail: 1, squash: 0.65, flat: 0.3, seed: r.range(0, 50), amp: 0.4, freq: 2.4 });
    for (let k = 0; k < 2; k++) {
      const a = r.range(0, TAU);
      mb.color = jitter(WHITE, r, 0.08);
      leafMass(mb, [Math.cos(a) * 0.035, 0.012, Math.sin(a) * 0.035], 0.024, { detail: 0, amp: 0.2, squash: 0.7, flat: 0.4, seed: r.range(0, 50) });
    }
    eachVert(mb, m0, (v, i) => {
      if (v[i + 1] < 0) v[i + 1] -= 0.015;
    });
    mb.sphereNormals(m0, [0, 0, 0], 0.5);
    shadeFoliage(mb, m0, { lo: -0.01, hi: 0.05, cols: SAGE, seed: r.range(0, 9), center: [0, 0.01, 0], rad: 0.06, aoMin: 0.65 });
    swayFoliage(mb, m0, 0, 0.05, 0.06, 0.25);
  } else if (kind === 2) {
    // A column cactus with an arm or two turning up.
    const g = jitter(pal("#4c6544"), r, 0.05);
    const H = r.range(0.11, 0.15);
    mb.color = g;
    mb.lathe([[0.014, -0.04], [0.0145, H * 0.6], [0.012, H * 0.92], [0.006, H], [0, H + 0.003]], 8, { caps: false, wobble: (a) => 0.08 * Math.cos(a * 8) });
    const arms = r.int(1, 2), a0 = r.range(0, TAU);
    for (let k = 0; k < arms; k++) {
      const a = a0 + k * Math.PI + r.range(-0.4, 0.4);
      const ca = Math.cos(a), sa = Math.sin(a);
      const y = H * r.range(0.3, 0.5), out = r.range(0.042, 0.052), up = r.range(0.04, 0.06);
      mb.tube([[ca * 0.008, y, sa * 0.008], [ca * out * 0.75, y + 0.003, sa * out * 0.75], [ca * out, y + up * 0.45, sa * out], [ca * out, y + up, sa * out]], [0.0095, 0.0095, 0.009, 0.007], 6, { caps: true });
    }
    // Sunlit tops: lighter toward the crown.
    eachVert(mb, 0, (v, i) => {
      const k = 0.75 + 0.45 * clamp(v[i + 1] / H, 0, 1) + 0.2 * Math.max(0, v[i + 4]);
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
      const k = 0.8 + 0.4 * Math.max(0, v[i + 4]);
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

/** A tuft of meadow grass (the plains' and grasslands' loose flora). */
function buildGrass(mb, r, variant) {
  const n = 12 + variant * 2;
  const base = pal("#56662f"), tip = pal(variant % 2 ? "#a8a058" : "#8e9a4a");
  mb.mat = MAT.FOLIAGE;
  for (let k = 0; k < n; k++) {
    const a = r.range(0, TAU), d = Math.sqrt(r()) * 0.025;
    const la = a + r.range(-0.5, 0.5);
    blade(mb, [Math.cos(a) * d, -0.015, Math.sin(a) * d], [Math.cos(la), 0, Math.sin(la)], r.range(0.035, 0.06), 0.003, r.range(0.01, 0.03), [Math.cos(a), 0, Math.sin(a)], jitter(base, r, 0.1), jitter(tip, r, 0.08), r.range(0, 0.01));
  }
  mb.heightSway(0, 0, 0.06, 1);
}

recipe("plant.grass", { variants: 4, build: (mb, r, { variant }) => buildGrass(mb, r, variant) });

// --- catalog ------------------------------------------------------------------------------------------
//
// forest(feat, ctx): the species mix of a wooded plot ({asset, scale, weight, variant?, param?}; the
// first entry may carry `grid`, trees per plot side) and its undergrowth (`under`). dress.js picks a
// variant at random and multiplies the scale by 0.8-1.2; the mixes keep trees under ~0.45 tall
// (the ancient giants excepted). flora(ctx): sparse loose plants on open ground.

const COLD = (g) => g === GROUND.TUNDRA || g === GROUND.SNOW;
const mix = (entries, under = [], grid = null) => {
  if (grid) entries[0].grid = grid;
  entries.under = under;
  return entries;
};

function forestMix(feat, ctx) {
  const g = ctx.ground;
  const hell = isHell(g);
  if (feat === FEAT.ANCIENT) {
    // Giants, a little smaller where something stands among them (a glade under the boughs).
    return mix([{ asset: "tree.ancient", scale: ctx.held ? 0.66 : 0.8, weight: 1 }], [
      { asset: "plant.fern", scale: 1.2, count: 3 },
      { asset: "tree.sapling.ancient", scale: 1.1, count: 2 },
      { asset: "plant.mushroom.glow", scale: 1.2, count: 1 },
    ], 2);
  }
  if (feat === FEAT.BURNT) {
    if (hell) return mix([{ asset: "tree.dead", scale: 0.95, weight: 2 }, { asset: "tree.blighted", scale: 0.9, weight: 1 }]);
    // Charred snags, and green coming back between them.
    return mix([{ asset: "tree.dead", scale: 0.95, weight: 5 }, { asset: "tree.sapling", param: "broad", scale: 0.65, weight: 0.6 }, { asset: "bush", scale: 0.7, weight: 0.5 }], [{ asset: "plant.grass", scale: 0.9, count: 2 }]);
  }
  if (feat === FEAT.JUNGLE) {
    return mix([{ asset: "tree.jungle", scale: 1, weight: 3 }, { asset: "tree.palm", scale: 0.95, weight: 1 }], [
      { asset: "plant.fern", scale: 1.4, count: 4 },
      { asset: "bush", scale: 1.1, count: 1 },
    ]);
  }
  if (feat === FEAT.YOUNG) {
    if (hell) return mix([{ asset: "tree.blighted", scale: 0.55, weight: 1 }, { asset: "tree.dead", scale: 0.55, weight: 1 }]);
    if (COLD(g)) return mix([{ asset: "tree.sapling", param: "snow", scale: 1, weight: 3 }, { asset: "tree.pine", param: "snow", scale: 0.55, weight: 1 }, { asset: "bush", scale: 0.8, weight: 0.3 }]);
    return mix([
      { asset: "tree.sapling", param: "broad", scale: 1, weight: 3 },
      { asset: "tree.sapling", param: "pine", scale: 1, weight: ctx.hills ? 2 : 1 },
      { asset: "bush", scale: 1.1, weight: 1 },
      { asset: "tree.birch", scale: 0.6, weight: 0.6 },
    ], [{ asset: "plant.grass", scale: 1, count: 2 }]);
  }
  if (feat !== FEAT.FOREST) return null;
  // Woods by ground.
  if (hell) return mix([{ asset: "tree.blighted", scale: 0.95, weight: 3 }, { asset: "tree.dead", scale: 0.9, weight: 1 }]);
  if (g === GROUND.SNOW) return mix([{ asset: "tree.pine", param: "snow", scale: 0.88, weight: 1 }]);
  if (g === GROUND.TUNDRA) return mix([{ asset: "tree.pine", param: "snow", scale: 0.88, weight: 4 }, { asset: "tree.pine", scale: 0.85, weight: 1 }, { asset: "tree.birch", scale: 0.85, weight: 0.6 }]);
  if (ctx.hills) return mix([{ asset: "tree.pine", scale: 0.88, weight: 3 }, { asset: "tree.oak", scale: 0.9, weight: 1.2 }, { asset: "tree.birch", scale: 0.9, weight: 0.4 }], [{ asset: "bush", scale: 1, count: 1 }]);
  if (g === GROUND.MARSH) return mix([{ asset: "tree.willow", scale: 0.95, weight: 2.5 }, { asset: "tree.oak", scale: 0.85, weight: 1 }, { asset: "tree.birch", scale: 0.9, weight: 0.8 }], [{ asset: "reeds", scale: 1, count: 3 }]);
  if (g === GROUND.DESERT) return mix([{ asset: "tree.oak", scale: 0.8, weight: 1 }, { asset: "tree.pine", scale: 0.8, weight: 1 }, { asset: "tree.palm", scale: 0.9, weight: 0.6 }], [{ asset: "shrub.dry", scale: 1, count: 2 }], 3);
  if (g === GROUND.PLAINS) return mix([{ asset: "tree.oak", scale: 0.92, weight: 3 }, { asset: "tree.pine", scale: 0.88, weight: 1.5 }, { asset: "tree.birch", scale: 0.9, weight: 0.8 }], [{ asset: "bush", scale: 1, count: 1 }]);
  return mix([{ asset: "tree.oak", scale: 0.9, weight: 3 }, { asset: "tree.birch", scale: 0.9, weight: 1.3 }, { asset: "tree.pine", scale: 0.86, weight: 1 }], [{ asset: "bush", scale: 1, count: 1 }, { asset: "plant.fern", scale: 1, count: 1 }], 5);
}

function floraOf(ctx) {
  const { r, ground: g } = ctx;
  if (ctx.peak) return [];
  const one = (p) => (r() < p ? 1 : 0);
  if (isHell(g)) {
    if (g === GROUND.BURNING_SANDS) return [{ asset: "tree.dead", scale: 0.45, count: one(0.12) }];
    return [{ asset: "tree.blighted", scale: 0.5, count: one(0.2) }, { asset: "tree.dead", scale: 0.45, count: one(0.15) }];
  }
  if (ctx.feat === FEAT.SCRUB) return [{ asset: "shrub.dry", scale: 1.1, count: r.int(3, 5) }];
  if (ctx.feat === FEAT.FLOOD) return [{ asset: "reeds", scale: 1, count: r.int(2, 3) }, { asset: "plant.grass", scale: 1.1, count: r.int(1, 3) }];
  switch (g) {
    case GROUND.GRASS:
      return [
        { asset: "bush", scale: 1, count: one(0.45) },
        { asset: "plant.grass", scale: 1.1, count: one(0.5) },
        { asset: ctx.hills ? "tree.pine" : "tree.oak", scale: 0.85, count: one(0.12) },
      ];
    case GROUND.PLAINS:
      return [
        { asset: "bush", scale: 0.85, count: one(0.25) },
        { asset: "plant.grass", scale: 1.1, count: r.int(0, 2) },
        { asset: "shrub.dry", scale: 0.9, count: one(0.15) },
        { asset: "tree.oak", scale: 0.8, count: one(0.06) },
      ];
    case GROUND.TUNDRA:
      return [{ asset: "tree.pine.snow", scale: 0.62, count: one(0.25) }, { asset: "plant.grass", scale: 0.9, count: one(0.4) }];
    case GROUND.SNOW:
      return [{ asset: "tree.pine.snow", scale: 0.55, count: one(0.1) }];
    case GROUND.DESERT:
      return [{ asset: "shrub.dry", scale: 1, count: r.int(0, 2) }];
    case GROUND.MARSH:
      return [{ asset: "reeds", scale: 1, count: r.int(2, 4) }, { asset: "tree.willow", scale: 0.7, count: one(0.15) }];
    default:
      return [];
  }
}

extendCatalog({ forest: forestMix, flora: floraOf });
