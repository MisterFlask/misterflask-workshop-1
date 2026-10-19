// props.js: rocks, one-off features, improvements, landmarks and resource markers of the 3D map,
// and the catalog's `rocks`, `feature`, `improvement` and `bonus` entries (docs/R3D.md "Asset
// recipes").
//
// Visual language: props sit IN the land -- every base is sunk below y = 0 (rock bellies, plinths,
// mounds), so nothing floats on a slope. Silhouette first: rocks are chiselled masses whose facets
// are painted in two or three values (warm lit tops, cool shaded sides, darker undersides) with
// lichen, moss, snow or strata bands baked in; structures are a few bold timber/stone masses with
// one telling detail (a dark adit, a lit lantern, a haystack). Colour is earthy and shared with the
// palette; saturation and glow are spent on magic (mana crystals in their school's colour), fire and
// lava, and the corrupted places. Landmarks are small dioramas with a focal point.
//
// Recipes (names are a contract):
//   rocks     rock.boulder (6; param "" | "lichen" | "moss" | "snow" | "sand" | "basalt"),
//             rock.boulder.lichen / .moss / .snow (aliases by name),
//             rock.outcrop (6, layered strata; param "" | "moss" | "sand" | "snow"),
//             rock.outcrop.moss, rock.crag (4, splintered rock towers; param "snow"),
//             rock.crag.snow, rock.shard (4, obsidian), rock.sandstone (4, wind-carved),
//             rock.basalt (4), rock.thorn (4, infernal horns of rock, ember at the roots)
//   features  feat.pool (oasis), feat.palm (fallback palm), feat.volcano, feat.flames,
//             feat.icefloe, feat.silt, feat.tuft, feat.wisp, feat.fallout, feat.overhang (bank
//             grass hanging over a river's edge; param "dry")
//   (more below: improvements imp.*, landmarks lm.*, resource markers res.*)
//
// Sizes: rocks, improvements and landmarks are modelled at map scale and the catalog scales the
// parts of a composition; resource markers (res.*) carry their full map size in the recipe
// (markerRecipe), so the lab's grid shows them as the map does and the bonus entry needs no scale.

import { recipe, hasRecipe } from "./registry.js";
import { extendCatalog } from "./catalog.js";
import { MAT, icosphere, MeshBuilder } from "../mesh.js";
import { pal, jitter, MANA } from "./palette.js";
import { noise3, hashf } from "../noise.js";
import { vnorm, vsub, vcross, vadd, vscale, clamp } from "../math.js";
import { KIND, FEAT, GROUND, DRAFT, RIVER_HALF, RIVER_WANDER, isHell } from "../world.js";
import { fieldLayout, inField, roadLinks, onRoad, ROAD_CLEAR, CROP, ROWS } from "../fields.js";

const TAU = Math.PI * 2;
const QT = Math.PI / 2;

// --- colour ---------------------------------------------------------------------------------------

const C = (c) => (typeof c === "string" ? pal(c) : c);
const mulc = (c, k) => {
  const v = C(c);
  return [v[0] * k, v[1] * k, v[2] * k];
};
const mixl = (a, b, t) => {
  const x = C(a), y = C(b);
  return [x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t];
};
const J = (c, r, a = 0.06) => jitter(C(c), r, a);
const TEAM = "#d8d8d8";
const TEAM_SHADE = "#9a9a9a";

/** Extra tones of the props (sRGB hex, linearised by C()). */
const T = {
  lichen: "#84803e",
  lichenOrange: "#a2713a",
  moss: "#5d6a2e",
  mossDark: "#3e4a24",
  grass: "#6e7a32",
  grassDry: "#8a7d48",
  snow: "#e8eef2",
  snowShade: "#b8c6d6",
  rockWarm: "#8a8070",
  rockCool: "#5a5860",
  rockDeep: "#3a383e",
  sandLight: "#bea886",
  sandBand: "#957a5a",
  sandDeep: "#735a44",
  basaltLit: "#4a4650",
  obsidianLit: "#4a4060",
  obsidianEdge: "#6a5a88",
  mud: "#4a3a2c",
  mudWet: "#33291f",
  silt: "#8a7a5a",
  ice: "#dce8ee",
  iceSide: "#9cc4d6",
  iceDeep: "#4f8aa0",
  waterDeep: "#173238",
  waterPool: "#2a5258",
  ash: "#57504c",
  char: "#211b19",
  ember: "#b8341a",
  flameCore: "#ffc860",
  flameMid: "#ff8a2a",
  flameTip: "#c8301a",
  lavaHot: "#ffb040",
  ghost: "#86b8b0",
  ghostCore: "#c8f0e8",
  toxic: "#6fa22a",
  toxicDim: "#3a5a1a",
  bone: "#d9cfb6",
  boneShade: "#a89c84",
};

/** Colour by face orientation: lit tops warm, sides as given, undersides dark (painted massing). */
function faceShade(base, n, { top = null, under = null, lit = 0.18 } = {}) {
  const up = n[1];
  let c = C(base);
  if (up > 0.45 && top) c = mixl(c, top, clamp((up - 0.45) / 0.35, 0, 1));
  if (up < -0.1) c = mixl(c, under ?? mulc(c, 0.55), clamp(-up * 1.6, 0, 1));
  // The sun's side (west-south-west) a touch warmer, the far side a touch cooler.
  const s = n[0] * -0.78 + n[1] * 0.46 + n[2] * 0.42;
  const k = 1 + lit * s;
  return [c[0] * k * (1 + 0.05 * s), c[1] * k, c[2] * k * (1 - 0.05 * s)];
}

// --- geometry helpers -----------------------------------------------------------------------------

/** Runs f with the transform moved to (x, y, z) and turned by yaw (state restored after). */
function at(mb, x, y, z, yaw, f) {
  mb.part(() => {
    mb.translate(x, y, z);
    if (yaw) mb.rotateY(yaw);
    f();
  });
}

/** A thin straight member from a to b (posts, rails, poles, bones): a `seg`-sided tube. */
function stick(mb, a, b, rad, seg = 4, caps = false) {
  mb.tube([a, b], rad, seg, { caps });
}

/** A flat-shaded triangle facing away from `out` (a point inside; the winding is turned to match,
 *  since the shader flips the normals of back faces), coloured by `shade(n, y, a, b, c)`. */
function ftri(mb, a, b, c, out, shade) {
  let n = vnorm(vcross(vsub(b, a), vsub(c, a)));
  const m = [(a[0] + b[0] + c[0]) / 3 - out[0], (a[1] + b[1] + c[1]) / 3 - out[1], (a[2] + b[2] + c[2]) / 3 - out[2]];
  if (n[0] * m[0] + n[1] * m[1] + n[2] * m[2] < 0) {
    n = [-n[0], -n[1], -n[2]];
    [b, c] = [c, b];
  }
  mb.color = shade(n, (a[1] + b[1] + c[1]) / 3, a, b, c);
  mb.triN(a, b, c, n, n, n);
}

/** A chiselled rock mass: an icosphere warped by noise and cut by planes (flat facets), scaled
 *  (sx, sy, sz) * rad around `c`, each facet painted by `shade(n, y, a, b, c)`. */
function rockMass(mb, r, o) {
  const { rad = 0.05, sx = 1, sy = 0.75, sz = 1, detail = 1, amp = 0.22, freq = 1.5, cuts = 4, cut = 0.7, c = [0, 0, 0], floor = -0.07, extra = [], shade } = o;
  const { verts, faces } = icosphere(detail);
  const seed = r.range(0, 100);
  const planes = [...extra];
  for (let i = 0; i < cuts; i++) {
    const a = r.range(0, TAU), el = r.range(-0.15, 1.1);
    planes.push([[Math.cos(a) * Math.cos(el), Math.sin(el), Math.sin(a) * Math.cos(el)], r.range(cut, cut + 0.2)]);
  }
  const P = verts.map((v) => {
    const k = 1 + amp * noise3(v[0] * freq + seed, v[1] * freq, v[2] * freq);
    let p = [v[0] * k, v[1] * k, v[2] * k];
    for (const [n, d] of planes) {
      const t = p[0] * n[0] + p[1] * n[1] + p[2] * n[2] - d;
      if (t > 0) p = [p[0] - n[0] * t, p[1] - n[1] * t, p[2] - n[2] * t];
    }
    return [c[0] + p[0] * rad * sx, Math.max(floor, c[1] + p[1] * rad * sy), c[2] + p[2] * rad * sz];
  });
  for (const [a, b, d] of faces) ftri(mb, P[a], P[b], P[d], c, shade);
}

/** A closed irregular polygon (counter-clockwise from above) of n points around radius rad. */
function jagPoly(r, n, rad, irr = 0.25, sx = 1, sz = 1) {
  const out = [];
  const ph = r.range(0, TAU);
  for (let i = 0; i < n; i++) {
    const a = ph + ((i + r.range(-0.3, 0.3)) / n) * TAU;
    const rr = rad * (1 + r.range(-irr, irr));
    out.push([Math.cos(a) * rr * sx, Math.sin(a) * rr * sz]);
  }
  return out;
}

/** A slab: polygon `poly` (offset by [ox, oz]) from y0 to y1, top scaled by `top`, flat-shaded
 *  with colour by face (`shade(n, y)`); `skew` shifts the top. */
function slab(mb, poly, y0, y1, { top = 0.9, skew = [0, 0], shade, cap = true }) {
  const n = poly.length;
  const cx = poly.reduce((s, p) => s + p[0], 0) / n, cz = poly.reduce((s, p) => s + p[1], 0) / n;
  const B = poly.map(([x, z]) => [x, y0, z]);
  const Tp = poly.map(([x, z]) => [cx + (x - cx) * top + skew[0], y1, cz + (z - cz) * top + skew[1]]);
  const inside = [cx, (y0 + y1) / 2, cz];
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    ftri(mb, B[i], Tp[i], Tp[j], inside, shade);
    ftri(mb, B[i], Tp[j], B[j], inside, shade);
  }
  if (cap) {
    const ct = [cx + skew[0], y1, cz + skew[1]];
    for (let i = 0; i < n; i++) ftri(mb, ct, Tp[i], Tp[(i + 1) % n], [ct[0], y1 - 1, ct[2]], shade);
  }
}

/** A jagged spire from below the ground up to `h`: rings of `sides` points shrinking upward,
 *  leaning by `lean` [x, z] at the top, ending in an off-centre point. */
function spire(mb, r, { x = 0, z = 0, h = 0.2, rad = 0.05, sides = 5, rings = 3, lean = [0, 0], twist = 0.5, taper = 0.85, tp = 1, base = -0.09, cut = 0, sq = 1, rot = 0, shade }) {
  const R = [];
  const cr = Math.cos(rot), sr = Math.sin(rot);
  for (let j = 0; j <= rings; j++) {
    const t = j / rings;
    const y = base + (h * 0.88 - base) * t;
    const k = Math.pow(t, 1.6);
    const cx = x + lean[0] * k, cz = z + lean[1] * k;
    const rad0 = rad * (1 - taper * Math.pow(t, tp)) * r.range(0.85, 1.1);
    const ring = [];
    for (let s = 0; s < sides; s++) {
      const a = (s / sides) * TAU + twist * t + r.range(-0.25, 0.25);
      const rr = rad0 * r.range(0.7, 1.2);
      // A blade (sq < 1): the ring squashed across `rot`.
      const u = Math.cos(a) * rr, v = Math.sin(a) * rr * sq;
      ring.push([cx + u * cr - v * sr, y + r.range(-0.01, 0.01) * (j ? 1 : 0), cz + u * sr + v * cr]);
    }
    R.push(ring);
  }
  // A point, or (cut) a broken, slanted top only a little above the last ring.
  const apex = [x + lean[0] + r.range(-0.3, 0.3) * rad, cut ? h * 0.88 + cut : h, z + lean[1] + r.range(-0.3, 0.3) * rad];
  const mid = (ring) => ring.reduce((s, p) => [s[0] + p[0] / sides, s[1] + p[1] / sides, s[2] + p[2] / sides], [0, 0, 0]);
  for (let j = 0; j < rings; j++) {
    const a = R[j], b = R[j + 1];
    const ins = vscale(vadd(mid(a), mid(b)), 0.5);
    for (let s = 0; s < sides; s++) {
      const q = (s + 1) % sides;
      ftri(mb, a[s], b[s], b[q], ins, shade);
      ftri(mb, a[s], b[q], a[q], ins, shade);
    }
  }
  const top = R[rings], ins = mid(top);
  ins[1] -= 0.01;
  for (let s = 0; s < sides; s++) ftri(mb, top[s], apex, top[(s + 1) % sides], ins, shade);
}

/** A faceted crystal (a prism of `sides` with a pointed tip, and a short point below), from
 *  `base` along the direction `dir`, length `len`. */
function crystal(mb, base, dir, len, rad, sides = 6, { tip = 0.3, foot = 0.12, phase = 0, shade = null } = {}) {
  const d = vnorm(dir);
  const u0 = Math.abs(d[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0];
  const e1 = vnorm(vcross(d, u0)), e2 = vcross(d, e1);
  const P = (t, a, rr) => vadd(vadd(base, vscale(d, t * len)), vadd(vscale(e1, Math.cos(a) * rr), vscale(e2, Math.sin(a) * rr)));
  const lo = [], hi = [];
  for (let s = 0; s < sides; s++) {
    const a = phase + (s / sides) * TAU;
    lo.push(P(0, a, rad));
    hi.push(P(1 - tip, a, rad * 0.92));
  }
  const top = vadd(base, vscale(d, len));
  const bot = vadd(base, vscale(d, -len * foot));
  const ctr = vadd(base, vscale(d, len * 0.45));
  const sh = shade ?? (() => mb.color);
  const keep = mb.color;
  for (let s = 0; s < sides; s++) {
    const q = (s + 1) % sides;
    ftri(mb, lo[s], hi[s], hi[q], ctr, (n, y) => sh(n, y, 0));
    ftri(mb, lo[s], hi[q], lo[q], ctr, (n, y) => sh(n, y, 0));
    ftri(mb, hi[s], top, hi[q], ctr, (n, y) => sh(n, y, 1));
    if (foot > 0) ftri(mb, lo[s], lo[q], bot, ctr, (n, y) => sh(n, y, -1));
  }
  mb.color = keep;
}

/** A resource marker's recipe: modelled at a small working scale, then enlarged to its size on
 *  the map (`k`) as a whole, so its per-vertex effects (tints, sway, occlusion by height) keep
 *  their modelled proportions, and the lab's grid shows a marker at the size the map does. */
function markerRecipe(name, k, def) {
  recipe(name, {
    ...def,
    build(mb, r, ctx) {
      const inner = new MeshBuilder();
      def.build(inner, r, ctx);
      mb.part(() => {
        mb.scale(k);
        mb.append(inner);
      });
    },
  });
}

/** Darkens vertices from `m0` on toward the ground (contact occlusion), y0 = darkest. */
function groundAO(mb, m0, y0 = -0.02, y1 = 0.05, min = 0.5) {
  mb.heightAO(m0, y0, y1, min);
}

// ================================================================================================
// Rocks
// ================================================================================================

/** The paint of a rock kind: base, lit-top, underside colours and a top coat (lichen, moss, snow,
 *  dust) with its coverage. */
function rockPaint(kind, r) {
  // Stone of the same family as the terrain's own rock (shaders.js: cool grey-violet in shadow,
  // warm grey where lit). The sun is warm (config.js: in linear light its blue is half its red),
  // so a neutral grey reads sandy beige under it: the stone is painted cool -- violet-grey sides,
  // mauve-grey tops -- so it lands on the look target's grey (warm-grey tops, mauve-grey flanks,
  // dark clefts). A mid value, not a pale one: under the strong sun a pale stone reads as white
  // confetti on the meadow (the target's rocks sit at a median lightness of ~0.4 on screen, their
  // lit tops ~0.7, their flanks ~0.27). The coats are muted (the sun saturates greens and yellows
  // a lot: moss stays a dark olive, lichen a grey-ochre, never lime). The desert's sandstone stays
  // warm and paler than the grey stone, basalt near black.
  switch (kind) {
    case "lichen":
      return { base: J("#3c3940", r, 0.04), top: J("#5f5a5f", r, 0.04), coat: ["#6a6650", "#74603e", "#5e5e50"], cover: 0.42 };
    case "moss":
      return { base: J("#3a383f", r, 0.04), top: J("#5e595e", r, 0.04), coat: ["#363e22", "#3e4726", "#474e2e"], cover: 0.36 };
    case "snow":
      return { base: J("#36363f", r, 0.04), top: J("#4c4a53", r, 0.04), coat: [T.snow, T.snow, T.snowShade], cover: 0.6 };
    case "sand":
      return { base: mulc(J(T.sandBand, r, 0.05), 0.66), top: mulc(J(T.sandLight, r), 0.66), coat: ["#ae9670"], cover: 0.25 };
    case "basalt":
      return { base: J("#252228", r, 0.05), top: J("#3a3640", r), coat: ["#4a2e26", "#3e3836"], cover: 0.25 };
    default:
      return { base: J("#3d3841", r, 0.04), top: J("#625b60", r, 0.04), coat: ["#5e5e46", "#444c2a"], cover: 0.16 };
  }
}

/** A shade function for a painted rock (facet value + top coat by noise): lit tops, the flanks in
 *  the base colour and darker the steeper they stand, the steep and undercut facets low down sunk
 *  toward a dark cleft colour (the dark crevices that make a boulder read as split stone, not a
 *  smooth lump), and the foot dark where it meets the ground. The value structure is painted in
 *  (the sun stands on the camera's side, so its light alone flattens a rock's front): a rock reads
 *  as a form without ink outlines. */
function rockShader(p, seed, { bands = 0, bandC = null, bandF = 70 } = {}) {
  const cleft = mulc(p.base, 0.42);
  return (n, y, a, b, c) => {
    let base = p.base;
    if (bands) {
      const s = Math.sin(y * bandF + seed * 3);
      if (s > 0.35) base = mixl(base, bandC, bands);
    }
    // Flanks darken as they steepen (a standing face at 0.6 of the base).
    base = mulc(base, 0.6 + 0.4 * clamp(n[1] / 0.7, 0, 1));
    // Steep facets facing down or away (the clefts between the chisel cuts) darken, more so near
    // the ground.
    if (n[1] < 0.2) base = mixl(base, cleft, clamp((0.2 - n[1]) * 1.6, 0, 0.7) * clamp(1.2 - y * 14, 0.35, 1));
    let col = faceShade(base, n, { top: p.top, under: mulc(base, 0.45), lit: 0.26 });
    if (n[1] > 0.38 && p.cover > 0) {
      const cx = (a[0] + b[0] + c[0]) / 3, cz = (a[2] + b[2] + c[2]) / 3;
      const k = noise3(cx * 40 + seed, y * 40, cz * 40) * 0.5 + 0.5;
      const want = p.cover * clamp((n[1] - 0.38) / 0.4, 0, 1);
      if (k < want * 1.4) {
        const coat = p.coat[Math.floor(hashf(Math.round(cx * 997), Math.round(cz * 991), Math.round(y * 983)) * p.coat.length)];
        col = mixl(col, faceShade(coat, n, { lit: 0.1 }), 0.85);
      }
    }
    // Faint per-facet value variation (painted, not flat), and the foot in the grass: the contact
    // shadow, darkest where the stone meets the ground.
    const v = (0.94 + 0.12 * hashf(Math.round(a[0] * 1e4), Math.round(b[1] * 1e4), Math.round(c[2] * 1e4))) * (0.55 + 0.45 * clamp((y + 0.004) / 0.03, 0, 1));
    return [col[0] * v, col[1] * v, col[2] * v];
  };
}

/** A boulder: a chiselled mass half sunk in the ground -- an erratic, a flat-topped slab, a
 *  cluster, a split boulder, a leaning block or a small pile (by variant). */
function buildBoulder(mb, r, variant, kind = "") {
  const p = rockPaint(kind, r);
  const sh = rockShader(p, r.range(0, 50));
  const m0 = mb.mark();
  const R = r.range(0.068, 0.084);
  const put = (rad, x, z, o = {}) => {
    const sy = o.sy ?? 0.85;
    rockMass(mb, r, { rad, sx: o.sx ?? r.range(0.95, 1.2), sy, sz: o.sz ?? r.range(0.85, 1.05), amp: o.amp ?? 0.2, cuts: o.cuts ?? 5, cut: o.cut ?? 0.66, detail: o.detail ?? 1, extra: o.extra ?? [], c: [x, rad * sy * (o.lift ?? 0.3), z], shade: sh });
  };
  const ang = r.range(0, TAU);
  const ring = (k, n, d) => [Math.cos(ang + (k / n) * TAU) * d, Math.sin(ang + (k / n) * TAU) * d];
  switch (variant) {
    case 0: // a tall rounded erratic
      put(R, 0, 0, { sy: r.range(0.95, 1.1), amp: 0.16, cuts: 4, cut: 0.72 });
      break;
    case 1: // a broad slab with a flat, weathered top
      put(R * 1.15, 0, 0, { sy: 0.62, sx: 1.25, cuts: 4, extra: [[[0, 1, 0], 0.45]] });
      put(R * 0.35, ...ring(0, 1, R * 1.3), { detail: 0, sy: 0.7 });
      break;
    case 2: // a cluster: one big, two smaller
      put(R * 0.95, 0, 0, { sy: 0.9 });
      put(R * 0.55, ...ring(0, 2, R * 1.15), { sy: 0.8 });
      put(R * 0.4, ...ring(1, 2.6, R * 1.1), { detail: 0, sy: 0.7 });
      break;
    case 3: { // a boulder split in two by frost
      const n = [Math.cos(ang), 0, Math.sin(ang)];
      const gap = R * 0.12;
      put(R, n[0] * gap, n[2] * gap, { sy: 0.9, cuts: 3, extra: [[[-n[0], 0, -n[2]], 0.05]] });
      put(R * 0.95, -n[0] * gap, -n[2] * gap, { sy: 0.85, cuts: 3, extra: [[[n[0], 0, n[2]], 0.05]] });
      break;
    }
    case 4: // an angular block
      put(R, 0, 0, { sy: r.range(0.95, 1.15), amp: 0.12, cuts: 8, cut: 0.55 });
      put(R * 0.3, ...ring(0, 1, R * 1.25), { detail: 0 });
      break;
    default: // a small pile
      for (let k = 0; k < 4; k++) put(R * r.range(0.42, 0.62), ...ring(k, 4, k === 3 ? 0 : R * 0.8), { detail: k === 3 ? 1 : 0, sy: 0.8, lift: k === 3 ? 0.6 : 0.3 });
  }
  groundAO(mb, m0, -0.01, R * 0.9, 0.55);
}

for (const [name, kind] of [["rock.boulder", null], ["rock.boulder.lichen", "lichen"], ["rock.boulder.moss", "moss"], ["rock.boulder.snow", "snow"], ["rock.basalt", "basalt"]]) {
  recipe(name, { variants: name === "rock.boulder" ? 6 : 4, build: (mb, r, { variant, param }) => buildBoulder(mb, r, kind === "basalt" ? variant + 2 : variant, kind ?? param) });
}

/** An outcrop: tilted beds of layered rock stepping up to a broken edge, scree at the foot. */
function buildOutcrop(mb, r, variant, kind = "") {
  const sand = kind === "sand";
  const p = rockPaint(kind || "moss", r);
  if (!kind) p.cover = 0.22;
  // Beds alternating in value (a pale bed, a dark one): the strata must read at a glance.
  // (Grey beds of the cool stone, rockPaint; the desert's in sand.)
  const strata = sand ? [T.sandLight, T.sandDeep, "#b08a60", T.sandBand] : [mixl(p.base, "#9a929c", 0.55), mulc(p.base, 0.76), mixl(p.base, "#a49ca4", 0.45), mixl(p.base, "#54525e", 0.5)];
  const m0 = mb.mark();
  const layers = r.int(3, 4);
  const R = r.range(0.11, 0.14);
  const dip = r.range(0, TAU);
  const dx = Math.cos(dip), dz = Math.sin(dip);
  const mseed = r.range(0, 50);
  let y = -0.07;
  mb.part(() => {
    // The beds dip: tilt the stack about a horizontal axis across the dip direction, so one side
    // is a stepped scarp and the other sinks into the hill.
    const tilt = r.range(0.22, 0.4) * (variant % 2 ? 1 : 0.7);
    mb.rotateY(-dip);
    mb.rotateZ(tilt);
    mb.rotateY(dip);
    for (let i = 0; i < layers; i++) {
      const t = i / layers;
      const rad = R * (1 - t * 0.45) * r.range(0.9, 1.08);
      const th = (i === 0 ? 0.072 : r.range(0.022, 0.034)) * (variant >= 4 ? 1.2 : 1);
      const off = R * 0.5 * t;
      const poly = jagPoly(r, r.int(6, 8), rad, 0.24, r.range(1.0, 1.45), 1).map(([x, z]) => [x + dx * off, z + dz * off]);
      const band = strata[(i + variant) % strata.length];
      // Bed tops: weathered pale stone, a few patches of dark moss (or snow, or sand dust).
      const topStone = sand ? mixl(band, "#d8c09a", 0.4) : mixl(band, "#b0a8ae", 0.45);
      slab(mb, poly, y, y + th, {
        top: r.range(0.8, 0.92),
        skew: [-dx * 0.006, -dz * 0.006],
        shade: (n, yy, a, b, c) => {
          let col = faceShade(band, n, { top: n[1] > 0.7 ? topStone : null, under: mulc(band, 0.4), lit: 0.3 });
          if (n[1] > 0.7) {
            const cx = (a[0] + b[0] + c[0]) / 3, cz = (a[2] + b[2] + c[2]) / 3;
            const k = noise3(cx * 30 + mseed, yy * 30, cz * 30) * 0.5 + 0.5;
            if (kind === "snow") col = mixl(col, faceShade(T.snow, n, { lit: 0.08 }), 0.9);
            else if (k < p.cover * 1.5) col = mixl(col, faceShade(p.coat[Math.floor(k * 7) % p.coat.length], n, { lit: 0.1 }), 0.85);
          }
          const v = 0.92 + 0.16 * hashf(Math.round(a[0] * 1e4 + mseed), Math.round(c[2] * 1e4));
          return [col[0] * v, col[1] * v, col[2] * v];
        },
      });
      y += th * r.range(0.9, 1.0);
    }
  });
  // Fallen blocks and scree at the foot of the scarp.
  const sh = rockShader(p, r.range(0, 50));
  for (let k = 0; k < (variant >= 3 ? 3 : 2); k++) {
    const a = dip + Math.PI + r.range(-1.1, 1.1), d = R * r.range(1.05, 1.4);
    const rr = r.range(0.016, 0.028);
    rockMass(mb, r, { rad: rr, detail: 0, sx: 1.2, sy: 0.8, amp: 0.2, cuts: 2, cut: 0.6, c: [Math.cos(a) * d, rr * 0.2, Math.sin(a) * d], shade: sh });
  }
  groundAO(mb, m0, -0.02, 0.06, 0.55);
}

recipe("rock.outcrop", { variants: 6, build: (mb, r, { variant, param }) => buildOutcrop(mb, r, variant, param) });
recipe("rock.outcrop.moss", { variants: 6, build: (mb, r, { variant }) => buildOutcrop(mb, r, variant, "moss") });
recipe("rock.outcrop.sand", { variants: 6, build: (mb, r, { variant }) => buildOutcrop(mb, r, variant, "sand") });

/** A crag: two to four tall jagged spires leaning together over a broken base -- the teeth that
 *  break a mountain's silhouette. Snow (param) catches on their upward facets. */
function buildCrag(mb, r, variant, snow = false) {
  // The mountain's own rock (terrain.js paints its flanks grey with a faint violet): cool grey,
  // warm only where the sun strikes, dark in the clefts.
  // A cluster of tall, splintered rock blades (aiguilles) leaning out from one master tooth, set
  // deep in the slope (a peak's flank falls away fast, so the roots reach far below y = 0).
  // (Painted cool against the warm sun, as the boulders are: rockPaint.)
  const base = J(mixl("#5a5660", "#605a62", r()), r, 0.04);
  const lit = mixl(base, "#b0a8ae", 0.55);
  const dark = mixl(base, "#24222a", 0.5);
  const seed = r.range(0, 50);
  const sh = (n, y, a, b, c) => {
    // Strata across the blades, darker toward the roots, lit tops warm.
    const band = Math.sin(y * 46 + seed) > 0.55 ? 0.84 : 1;
    let col = faceShade(mixl(dark, base, clamp((y + 0.08) / 0.18, 0, 1)), n, { top: lit, under: dark, lit: 0.4 });
    // Snow lodges on every upward ledge; the steep faces stay dark rock.
    if (snow && n[1] > 0.22) col = mixl(col, faceShade(T.snow, n, { lit: 0.08 }), clamp((n[1] - 0.22) * 3, 0, 0.94) * clamp((y + 0.02) / 0.08, 0, 1));
    const v = band * (0.92 + 0.16 * hashf(Math.round(a[0] * 1e4), Math.round(b[2] * 1e4), Math.round(c[1] * 1e4)));
    return [col[0] * v, col[1] * v, col[2] * v];
  };
  const m0 = mb.mark();
  const n = [2, 3, 3, 4][variant] ?? 3;
  const H = r.range(0.18, 0.23) * (variant === 3 ? 0.9 : 1);
  const la = r.range(0, TAU);
  const grain = r.range(0, Math.PI); // the blades share the rock's cleavage
  for (let i = 0; i < n; i++) {
    const a = la + (i / n) * TAU + r.range(-0.5, 0.5);
    const d = i === 0 ? 0 : r.range(0.045, 0.065);
    const h = i === 0 ? H : H * r.range(0.45, 0.7);
    const lean = [Math.cos(a) * h * (i ? 0.28 : 0.1), Math.sin(a) * h * (i ? 0.28 : 0.1)];
    spire(mb, r, { x: Math.cos(a) * d, z: Math.sin(a) * d, h, rad: (i === 0 ? 0.1 : 0.068) * r.range(0.9, 1.12), sides: i === 0 ? 5 : 4, rings: 3, lean, twist: r.range(-0.3, 0.3), taper: i === 0 ? 0.72 : 0.86, tp: 1.7, base: -0.2, cut: i === 0 ? r.range(0.004, 0.012) : i % 2 ? r.range(0.006, 0.02) : 0, sq: i === 0 ? r.range(0.75, 0.9) : r.range(0.6, 0.8), rot: grain + r.range(-0.3, 0.3), shade: sh });
  }
  // Broken scree at the foot, half buried (the blades must not stand on the slope like pegs).
  for (let k = 0; k < 2; k++) {
    const a = la + Math.PI * (0.6 + k * 0.8) + r.range(-0.3, 0.3), d = r.range(0.06, 0.085);
    rockMass(mb, r, { rad: r.range(0.026, 0.034), detail: 0, sx: 1.3, sy: 0.7, sz: 1.0, amp: 0.2, cuts: 3, cut: 0.55, c: [Math.cos(a) * d, -0.01, Math.sin(a) * d], floor: -0.1, shade: sh });
  }
  groundAO(mb, m0, -0.06, 0.1, 0.5);
}

recipe("rock.crag", { variants: 4, build: (mb, r, { variant, param }) => buildCrag(mb, r, variant, param === "snow") });
recipe("rock.crag.snow", { variants: 4, build: (mb, r, { variant }) => buildCrag(mb, r, variant, true) });

/** Obsidian: a cluster of glassy black blades thrust up through a basalt foot; lit facets catch a
 *  cold violet sheen (MAT.METAL highlight), and the hell variants keep an ember in the cracks. */
function buildShard(mb, r, variant) {
  const m0 = mb.mark();
  mb.mat = MAT.METAL;
  const glass = (n, y, part) => {
    const s = n[0] * -0.78 + n[1] * 0.46 + n[2] * 0.42;
    const c = s > 0.25 ? mixl("obsidian", T.obsidianLit, clamp(s, 0, 1)) : mixl("#120e16", "#1c1824", clamp(s + 0.5, 0, 1));
    return part === 1 ? mixl(c, T.obsidianEdge, 0.35) : c;
  };
  const n = r.int(3, 5) + (variant === 2 ? 1 : 0);
  const lean0 = r.range(0, TAU);
  for (let i = 0; i < n; i++) {
    const a = lean0 + (i / n) * TAU + r.range(-0.5, 0.5);
    const d = i === 0 ? 0 : r.range(0.012, 0.03);
    const len = (i === 0 ? r.range(0.12, 0.17) : r.range(0.05, 0.11)) * (variant === 3 ? 0.8 : 1);
    const tilt = i === 0 ? r.range(0.05, 0.2) : r.range(0.25, 0.65);
    const dir = [Math.cos(a) * Math.sin(tilt), Math.cos(tilt), Math.sin(a) * Math.sin(tilt)];
    crystal(mb, [Math.cos(a) * d, -0.03, Math.sin(a) * d], dir, len + 0.03, (i === 0 ? 0.018 : 0.012) * r.range(0.8, 1.2), r.pick([3, 4, 4, 5]), { tip: r.range(0.35, 0.6), foot: 0, phase: r.range(0, TAU), shade: glass });
  }
  mb.mat = MAT.MATTE;
  const p = rockPaint("basalt", r);
  rockMass(mb, r, { rad: 0.042, detail: 0, sx: 1.3, sy: 0.45, sz: 1.2, amp: 0.2, cuts: 3, cut: 0.6, c: [0, 0, 0], shade: rockShader(p, 3) });
  if (variant >= 2) {
    // A faint ember in the cracks at the foot (the hell variants).
    mb.mat = MAT.EMISSIVE;
    mb.color = C(T.ember);
    for (let k = 0; k < 2; k++) {
      const a = r.range(0, TAU);
      at(mb, Math.cos(a) * 0.03, 0.004, Math.sin(a) * 0.03, -a, () => mb.quad([-0.012, 0, -0.002], [0.012, 0, -0.002], [0.012, 0.002, 0.002], [-0.012, 0.002, 0.002]));
    }
    mb.mat = MAT.MATTE;
  }
  groundAO(mb, m0, -0.02, 0.05, 0.6);
}

recipe("rock.shard", { variants: 4, build: (mb, r, { variant }) => buildShard(mb, r, variant) });

/** Infernal thorns: great black horns of rock curling up out of the hell ground like the claws
 *  of something buried, ember-red in the cracks at their roots -- the hells' own silhouette. */
function buildThorns(mb, r, variant) {
  const m0 = mb.mark();
  const n = 2 + (variant % 3);
  const a0 = r.range(0, TAU);
  for (let k = 0; k < n; k++) {
    const a = a0 + (k / n) * TAU + r.range(-0.4, 0.4);
    const d = k ? r.range(0.025, 0.045) : 0;
    const h = (k ? r.range(0.08, 0.13) : r.range(0.16, 0.21)) * (variant === 3 ? 0.85 : 1);
    const bend = r.range(0.35, 0.6) * h;
    const dir = [Math.cos(a), Math.sin(a)];
    const pts = [], radii = [], cols = [];
    for (let i = 0; i <= 4; i++) {
      const t = i / 4;
      const out = d + bend * t * t;
      pts.push([dir[0] * out, -0.04 + (h + 0.04) * Math.sin(t * Math.PI * 0.5), dir[1] * out]);
      radii.push((k ? 0.024 : 0.034) * (1 - t * 0.9) + 0.001);
      cols.push(mixl("#1c1416", "#5e4844", t * 0.85));
    }
    // Curl the tip back over (a hook).
    pts[4] = vadd(pts[4], [-dir[0] * bend * 0.25, -0.012, -dir[1] * bend * 0.25]);
    mb.color = C("#2a2224");
    mb.tube(pts, radii, 4, { caps: true, colors: cols });
  }
  rockMass(mb, r, { rad: 0.04, detail: 0, sx: 1.3, sy: 0.45, sz: 1.2, amp: 0.2, cuts: 3, cut: 0.6, c: [0, -0.004, 0], shade: rockShader(rockPaint("basalt", r), 5) });
  // Ember in the cracks at the roots.
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#a82a12");
    for (let k = 0; k < 3; k++) {
      const a = a0 + (k / 3) * TAU + 0.5;
      at(mb, Math.cos(a) * 0.035, 0.006, Math.sin(a) * 0.035, -a, () => mb.quad([-0.014, 0, -0.002], [0.014, 0, -0.002], [0.014, 0.002, 0.002], [-0.014, 0.002, 0.002]));
    }
  });
  groundAO(mb, m0, -0.03, 0.06, 0.55);
}

recipe("rock.thorn", { variants: 4, build: (mb, r, { variant }) => buildThorns(mb, r, variant) });

/** Wind-carved sandstone: banded, undercut forms -- a mushroom rock, a hoodoo, a low mesa block
 *  and a natural arch. */
function buildSandstone(mb, r, variant) {
  const m0 = mb.mark();
  const bands = [T.sandLight, T.sandBand, "#aa906c", T.sandDeep, "#b8a07a"];
  const ph = r.range(0, TAU), ph2 = r.range(0, TAU);
  const bandAt = (y) => bands[Math.floor(clamp((y + 0.05) * 38 + Math.sin(y * 90 + ph) * 0.4, 0, 99)) % bands.length];
  const wob = (a, y) => 0.12 * Math.sin(a * 3 + ph + y * 20) + 0.07 * Math.sin(a * 5 + ph2);
  const flat = (profile, sx, sz, seg = 9) => {
    // A lathe with banded colours per ring, flattened into an oval.
    const s0 = mb.mark();
    mb.part(() => {
      mb.scale(sx, 1, sz);
      mb.lathe(profile, seg, { wobble: (a, y) => wob(a, y) });
    });
    mb.tint(s0, (x, y, z) => {
      const b = C(bandAt(y));
      const base = C(T.sandBand);
      return [b[0] / base[0], b[1] / base[1], b[2] / base[2]];
    });
  };
  mb.color = C(T.sandBand);
  if (variant === 0) {
    // Mushroom rock: a heavy cap on a wind-gnawed neck.
    flat([[0.05, -0.05], [0.045, 0.0], [0.028, 0.03], [0.022, 0.055], [0.04, 0.075], [0.07, 0.09], [0.068, 0.115], [0.04, 0.13], [0, 0.133]], 1.2, 0.9);
  } else if (variant === 1) {
    // Hoodoo: a tall stack of pinched bands with a capstone.
    flat([[0.05, -0.05], [0.045, 0.02], [0.034, 0.05], [0.04, 0.08], [0.028, 0.11], [0.034, 0.14], [0.024, 0.165], [0.04, 0.18], [0.03, 0.195], [0, 0.198]], 1.0, 0.85);
  } else if (variant === 2) {
    // A low mesa block, undercut at the foot, with a smaller one beside.
    flat([[0.085, -0.05], [0.07, 0.01], [0.075, 0.04], [0.09, 0.065], [0.085, 0.075], [0, 0.077]], 1.3, 0.8, 8);
    mb.part(() => {
      mb.translate(0.1, 0, 0.03);
      flat([[0.035, -0.05], [0.03, 0.02], [0.038, 0.04], [0, 0.043]], 1, 0.9, 7);
    });
  } else {
    // A natural arch: two worn legs and a sagging span.
    const pts = [];
    for (let i = 0; i <= 8; i++) {
      const a = Math.PI * (i / 8);
      pts.push([-Math.cos(a) * 0.075, -0.04 + Math.sin(a) * 0.15 * (1 - 0.15 * Math.sin(a * 2)), 0]);
    }
    const radii = pts.map((_, i) => 0.03 * (1.35 - 0.6 * Math.sin((Math.PI * i) / 8)));
    const s0 = mb.mark();
    mb.tube(pts, radii, 7, { caps: true });
    mb.tint(s0, (x, y) => {
      const b = C(bandAt(y));
      const base = C(T.sandBand);
      return [b[0] / base[0], b[1] / base[1], b[2] / base[2]];
    });
  }
  // A lit cap and darker foot (sun-bleached tops, shaded undercut).
  mb.tint(m0, (x, y) => 0.78 + 0.32 * clamp(y / 0.15, 0, 1));
  groundAO(mb, m0, -0.02, 0.05, 0.55);
}

recipe("rock.sandstone", { variants: 4, build: (mb, r, { variant }) => buildSandstone(mb, r, variant) });

// ================================================================================================
// Features
// ================================================================================================

/** A ring-shaped wobble (shared by a pool's bank and its water so they meet). */
function wobbler(r, amt = 0.12) {
  const p1 = r.range(0, TAU), p2 = r.range(0, TAU);
  return (a) => amt * Math.sin(a * 2 + p1) + amt * 0.55 * Math.sin(a * 5 + p2);
}

/** A pool: a sunken bank (grass or sand outside, wet mud at the water) around still water. */
function pool(mb, r, { rad = 0.13, water = T.waterPool, outer = T.grass, bank = T.mud, glow = null, metal = true } = {}) {
  const w = wobbler(r, 0.1);
  const m0 = mb.mark();
  mb.color = C(outer);
  mb.lathe([[rad * 1.55, -0.03], [rad * 1.38, 0.006], [rad * 1.2, 0.013], [rad * 1.06, 0.008], [rad * 0.96, 0.0]], 14, { caps: false, wobble: (a) => w(a) });
  // Grass outside, mud toward the water.
  mb.tint(m0, (x, y, z) => {
    const d = Math.hypot(x, z) / rad;
    const t = clamp((1.3 - d) / 0.25, 0, 1);
    const o = C(outer), b = C(bank);
    return [(o[0] + (b[0] - o[0]) * t) / o[0], (o[1] + (b[1] - o[1]) * t) / o[1], (o[2] + (b[2] - o[2]) * t) / o[2]];
  });
  mb.part(() => {
    if (glow) {
      mb.mat = MAT.EMISSIVE;
      mb.color = C(glow);
    } else {
      mb.mat = metal ? MAT.METAL : MAT.MATTE;
      mb.color = C(water);
    }
    const s0 = mb.mark();
    mb.lathe([[rad * 1.04, 0.005], [rad * 0.6, 0.006], [0, 0.006]], 14, { caps: false, wobble: (a) => w(a) });
    if (!glow) mb.tint(s0, (x, y, z) => 1.5 - 0.8 * clamp(1 - Math.hypot(x, z) / rad, 0, 1));
  });
}

/** An oasis pool with a few flat stones on its rim. */
function buildOasisPool(mb, r) {
  pool(mb, r, { rad: r.range(0.12, 0.15), outer: "#5e6a2c", bank: "#4a3c2a", water: "#123036" });
  const sh = rockShader(rockPaint("sand", r), 3);
  for (let k = 0; k < 3; k++) {
    const a = r.range(0, TAU), d = r.range(0.15, 0.19);
    rockMass(mb, r, { rad: r.range(0.014, 0.022), detail: 0, sy: 0.5, cuts: 2, cut: 0.5, c: [Math.cos(a) * d, 0.006, Math.sin(a) * d], shade: sh });
  }
}

recipe("feat.pool", { variants: 4, build: (mb, r) => buildOasisPool(mb, r) });

/** A small date palm (only used when vegetation.js has no tree.palm). */
function buildPalm(mb, r) {
  const h = r.range(0.22, 0.3);
  const lean = [r.range(-0.05, 0.05), r.range(-0.05, 0.05)];
  const pts = [];
  for (let i = 0; i <= 4; i++) {
    const t = i / 4;
    pts.push([lean[0] * t * t, -0.03 + (h + 0.03) * t, lean[1] * t * t]);
  }
  mb.color = C("#7a6a50");
  mb.tube(pts, [0.014, 0.012, 0.011, 0.01, 0.009], 5);
  const top = pts[4];
  mb.mat = MAT.FOLIAGE;
  const m0 = mb.mark();
  for (let k = 0; k < 7; k++) {
    const a = (k / 7) * TAU + r.range(-0.2, 0.2);
    const L = r.range(0.09, 0.12);
    const tip = [top[0] + Math.cos(a) * L, top[1] - L * 0.45, top[2] + Math.sin(a) * L];
    const mid = [top[0] + Math.cos(a) * L * 0.5, top[1] + 0.012, top[2] + Math.sin(a) * L * 0.5];
    const side = [-Math.sin(a) * 0.018, 0, Math.cos(a) * 0.018];
    mb.color = J(k % 2 ? "#3e5a2a" : "#56702e", r);
    mb.tri(top, vadd(mid, side), mid);
    mb.tri(top, mid, vsub(mid, side));
    mb.tri(mid, vadd(mid, side), tip);
    mb.tri(mid, tip, vsub(mid, side));
  }
  mb.heightSway(m0, top[1] - 0.05, top[1], 0.8);
}

recipe("feat.palm", { variants: 4, build: (mb, r) => buildPalm(mb, r) });

/** The surface radius of a lathe profile at height y on its outer slope (profile outer part
 *  ordered bottom to top). */
function profileR(profile, y) {
  for (let i = 0; i + 1 < profile.length; i++) {
    const [r0, y0] = profile[i], [r1, y1] = profile[i + 1];
    if (y1 < y0) break;
    if (y >= y0 && y <= y1) return r0 + ((r1 - r0) * (y - y0)) / (y1 - y0 || 1);
  }
  return profile[0][0];
}

/** A faceted surface of revolution: rings from `profile` [[r, y], ...] with a radial wobble
 *  `wob(a, y)` and per-vertex jitter, flat-shaded through `shade` (normals facing the way the
 *  profile's slope faces: outward on a rising flank, inward on a crater's descending wall). */
function facetLathe(mb, r, profile, seg, wob, shade, jit = 0.04) {
  const rings = profile.map(([rr, y], i) => {
    const out = [];
    for (let s = 0; s < seg; s++) {
      const a = (s / seg) * TAU + (i % 2) * (Math.PI / seg) * 0.5;
      const k = rr * (1 + wob(a, y) + r.range(-jit, jit));
      out.push([Math.cos(a) * k, y + r.range(-jit, jit) * 0.3 * rr, Math.sin(a) * k]);
    }
    return out;
  });
  for (let i = 0; i + 1 < profile.length; i++) {
    const dr = profile[i + 1][0] - profile[i][0], dy = profile[i + 1][1] - profile[i][1];
    for (let s = 0; s < seg; s++) {
      const q = (s + 1) % seg;
      const a = ((s + 0.5) / seg) * TAU;
      const hint = [Math.cos(a) * dy, -dr, Math.sin(a) * dy];
      const A = rings[i][s], B = rings[i][q], Cc = rings[i + 1][q], D = rings[i + 1][s];
      const cen = vscale(vadd(vadd(A, B), vadd(Cc, D)), 0.25);
      const out = vsub(cen, hint);
      ftri(mb, A, B, Cc, out, shade);
      ftri(mb, A, Cc, D, out, shade);
    }
  }
  return rings;
}

/** A volcano's crater: a ragged, gullied basalt cone (the peak's own summit) with a glowing
 *  caldera and lava tongues spilling down its flanks. */
function buildVolcano(mb, r, variant) {
  const ph = r.range(0, TAU), ph2 = r.range(0, TAU);
  // Gullies down the flanks (stronger toward the foot), a broken rim.
  const wob = (a, y) => (0.05 * Math.sin(a * 3 + ph) + 0.035 * Math.sin(a * 7 + ph2)) * clamp((y + 0.1) / 0.28, 0.3, 1.3) + 0.06 * Math.pow(0.5 + 0.5 * Math.sin(a * 11 + ph2 + y * 6), 3) * clamp(0.6 - y * 3, 0, 1);
  // Concave flanks steepening to a narrow, broken rim (the peak below is the volcano's body).
  const outer = [[0.4, -0.4], [0.27, -0.2], [0.19, -0.06], [0.14, 0.05], [0.115, 0.13], [0.105, 0.17]];
  const inner = [[0.105, 0.17], [0.085, 0.15], [0.07, 0.115]];
  const seed = r.range(0, 9);
  const flank = (n, y, a, b, c) => {
    const t = clamp((y + 0.25) / 0.42, 0, 1);
    let base = mixl("#151315", "#2e2a2c", t);
    base = mixl(base, "#4a2418", clamp((y - 0.13) / 0.05, 0, 1) * 0.8);
    // Ash streaks lie on the upward facets.
    if (n[1] > 0.6 && hashf(Math.round(a[0] * 1e3 + seed), Math.round(c[2] * 1e3)) < 0.4) base = mixl(base, "#4a4442", 0.5);
    return faceShade(base, n, { top: null, lit: 0.35 });
  };
  const m0 = mb.mark();
  facetLathe(mb, r, outer, 16, wob, flank, 0.035);
  groundAO(mb, m0, -0.2, 0.1, 0.6);
  // The caldera's wall glows from below; the lava lake at its floor.
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    facetLathe(mb, r, inner, 16, wob, (n, y) => mixl("#200c08", "#7a2410", clamp((0.175 - y) / 0.05, 0, 1)), 0.03);
    mb.color = C(T.lavaHot);
    const l0 = mb.mark();
    mb.lathe([[0.074, 0.114], [0.04, 0.118], [0, 0.119]], 12, { caps: false, wobble: (a, y) => wob(a, y) });
    mb.tint(l0, (x, y, z) => 0.5 + 0.5 * (0.5 + 0.5 * Math.sin(x * 120 + z * 90)));
  });
  // Lava tongues down the flanks (they follow the cone's surface, just proud of it).
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    const n = 3 + (variant % 2);
    for (let k = 0; k < n; k++) {
      let a = ph + (k / n) * TAU + r.range(-0.4, 0.4);
      const pts = [], radii = [], cols = [];
      const steps = 7;
      const y1 = r.range(-0.18, -0.04);
      for (let i = 0; i <= steps; i++) {
        const t = i / steps;
        const y = 0.165 + (y1 - 0.165) * t;
        a += r.range(-0.07, 0.07);
        const rr = profileR(outer, y) * (1 + wob(a, y)) + 0.006;
        pts.push([Math.cos(a) * rr, y + 0.004, Math.sin(a) * rr]);
        radii.push(0.014 * (1 - t * 0.5));
        cols.push(mixl(T.lavaHot, "#7a1a08", Math.pow(t, 0.8) * 0.95));
      }
      mb.tube(pts, radii, 4, { colors: cols });
    }
  });
  // Jagged teeth on the rim and basalt blocks on the shoulders.
  const sh = rockShader(rockPaint("basalt", r), 7);
  for (let k = 0; k < 5; k++) {
    const a = ph + r.range(0, TAU);
    const rr = 0.105 * (1 + wob(a, 0.17));
    spire(mb, r, { x: Math.cos(a) * rr, z: Math.sin(a) * rr, h: 0.19 + r.range(0.0, 0.035), rad: 0.022, sides: 4, rings: 1, base: 0.13, taper: 0.5, cut: 0.006, shade: (n, y) => faceShade(y > 0.18 ? "#5a3026" : "#2e2a2e", n, { lit: 0.35 }) });
  }
  for (let k = 0; k < 5; k++) {
    const a = r.range(0, TAU), y = r.range(-0.12, 0.06);
    const rr = profileR(outer, y) * (1 + wob(a, y));
    rockMass(mb, r, { rad: r.range(0.03, 0.045), detail: 0, sy: 0.7, cuts: 3, cut: 0.55, c: [Math.cos(a) * rr, y, Math.sin(a) * rr], floor: -0.4, shade: sh });
  }
}

recipe("feat.volcano", { variants: 2, build: (mb, r, { variant }) => buildVolcano(mb, r, variant) });

const FIRE = [T.flameCore, T.flameMid, T.flameTip];
const WITCHFIRE = ["#c8ffb0", "#5ad860", "#1a5a3a"];
const HOLYFIRE = ["#fff4d0", "#ffd878", "#e09a40"];

/** One licking flame: a swelling tongue that curls to a point, hot gold at the root and red at
 *  the tip (emissive; the tips flutter in the wind). */
function flameTongue(mb, r, x, z, h, w, lean = [0, 0], hot = 1, fc = FIRE) {
  const ph = r.range(0, TAU);
  const prof = [0.55, 1, 0.92, 0.66, 0.36, 0.12, 0.0];
  const pts = [], radii = [], cols = [];
  const n = prof.length - 1;
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    const s = Math.sin(t * Math.PI * 1.3 + ph) * w * 0.9 * t;
    pts.push([x + lean[0] * t * t + s, -0.006 + h * t, z + lean[1] * t * t + s * 0.3]);
    radii.push(Math.max(0.0006, w * prof[i]));
    cols.push(t < 0.35 ? mixl(fc[0], fc[1], (t / 0.35) * (1 - hot * 0.6)) : mixl(fc[1], fc[2], Math.min(1, (t - 0.35) / 0.55)));
  }
  const m0 = mb.mark();
  mb.tube(pts, radii, 5, { colors: cols });
  mb.heightSway(m0, h * 0.3, h, 0.5);
}

/** A patch of burning ground: charred earth, a burnt log or two, and a knot of flame tongues. */
function buildFlames(mb, r, variant) {
  mb.color = C(T.char);
  const m0 = mb.mark();
  mb.lathe([[0.12, -0.02], [0.1, 0.004], [0.05, 0.007], [0, 0.007]], 9, { caps: false, wobble: wobbler(r, 0.18) });
  mb.tint(m0, (x, y, z) => 0.8 + 0.4 * clamp(Math.hypot(x, z) / 0.1, 0, 1));
  for (let k = 0; k < (variant % 2) + 1; k++) {
    const a = r.range(0, TAU), L = r.range(0.07, 0.1);
    const c = [Math.cos(a) * L * 0.5, 0.008, Math.sin(a) * L * 0.5];
    mb.color = C("#2a2220");
    stick(mb, vsub(c, [Math.cos(a) * L, 0, Math.sin(a) * L]), vadd(c, [Math.cos(a) * L * 0.2, 0.006, Math.sin(a) * L * 0.2]), 0.009, 5, true);
  }
  mb.mat = MAT.EMISSIVE;
  const n = r.int(4, 5);
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + r.range(-0.3, 0.3);
    const d = k === 0 ? 0 : r.range(0.03, 0.05);
    const h = (k === 0 ? r.range(0.17, 0.22) : r.range(0.08, 0.13)) * (1 - d * 3);
    flameTongue(mb, r, Math.cos(a) * d, Math.sin(a) * d, h, k === 0 ? 0.03 : 0.02, [Math.cos(a) * d * 0.6, Math.sin(a) * d * 0.6], k === 0 ? 1 : 0.3);
  }
  mb.mat = MAT.MATTE;
}

recipe("feat.flames", { variants: 4, build: (mb, r, { variant }) => buildFlames(mb, r, variant) });

/** An ice floe: a flat, broken slab riding low in the water, sometimes with a pressure ridge. */
function buildIceFloe(mb, r, variant) {
  const sh = (n, y) => {
    if (y < 0) return mixl(T.iceDeep, T.iceSide, clamp((y + 0.03) / 0.03, 0, 1));
    return faceShade(n[1] > 0.6 ? T.ice : T.iceSide, n, { lit: 0.15 });
  };
  const rad = r.range(0.07, 0.11);
  slab(mb, jagPoly(r, r.int(6, 8), rad, 0.3, r.range(1, 1.5), 1), -0.035, 0.012, { top: 0.93, shade: sh });
  if (variant % 2) {
    // A pressure ridge: tilted plates pushed up along a crack.
    const a = r.range(0, TAU);
    for (let k = -1; k <= 1; k++) {
      mb.part(() => {
        mb.translate(Math.cos(a) * k * rad * 0.35, 0.01, Math.sin(a) * k * rad * 0.35);
        mb.rotateY(-a);
        mb.rotateX(r.range(-0.6, 0.6));
        slab(mb, jagPoly(r, 4, rad * 0.22, 0.2, 1.4, 0.5), -0.01, 0.012, { top: 0.8, shade: sh });
      });
    }
  }
  if (variant >= 2) slab(mb, jagPoly(r, 5, rad * 0.35, 0.3), -0.03, 0.008, { top: 0.9, shade: sh, skew: [rad * 1.2, rad * 0.2] });
}

recipe("feat.icefloe", { variants: 4, build: (mb, r, { variant }) => buildIceFloe(mb, r, variant) });

/** A silt bank of a flood plain: a low, wet-edged mound with a few blades of grass. */
function buildSilt(mb, r) {
  // A low bank of river silt left by the flood: dark and wet at its rim, drying paler on its
  // crown, sprouting fresh green (the flood plain's fertility, not a pebble).
  const seed = r.range(0, 50);
  mb.color = C(T.silt);
  const m0 = mb.mark();
  mb.part(() => {
    mb.translate(0, -0.008, 0);
    mb.scale(r.range(1.4, 1.9), 1, 1);
    mb.blob(r.range(0.065, 0.085), { detail: 1, amp: 0.25, squash: 0.18, noise: (v) => noise3(v[0] * 2 + seed, v[1] * 2, v[2] * 2) });
  });
  mb.tint(m0, (x, y) => {
    const t = clamp((y + 0.004) / 0.012, 0, 1);
    const wet = C("#3e3326"), dry = C("#7e6e52"), b = C(T.silt);
    return [(wet[0] + (dry[0] - wet[0]) * t) / b[0], (wet[1] + (dry[1] - wet[1]) * t) / b[1], (wet[2] + (dry[2] - wet[2]) * t) / b[2]];
  });
  mb.mat = MAT.FOLIAGE;
  tufts(mb, r, 4, 0.06, 0.04, ["#5e7a2e", "#6e8a34", "#7a8238"]);
  mb.mat = MAT.MATTE;
}

/** Grass tufts: `n` clumps of blades within `spread` of the centre, blades up to `h` tall. */
function tufts(mb, r, n, spread, h, cols) {
  const m0 = mb.mark();
  const mat0 = mb.mat;
  mb.mat = MAT.BLADE;
  for (let k = 0; k < n; k++) {
    const cx = r.range(-spread, spread), cz = r.range(-spread, spread);
    const blades = r.int(5, 8);
    for (let b = 0; b < blades; b++) {
      const a = r.range(0, TAU), tilt = r.range(0.15, 0.6);
      const len = h * r.range(0.6, 1.1);
      const tip = [cx + Math.cos(a) * Math.sin(tilt) * len, len * Math.cos(tilt), cz + Math.sin(a) * Math.sin(tilt) * len];
      const w = 0.0055;
      mb.color = J(cols[b % cols.length], r, 0.1);
      mb.tri([cx - Math.sin(a) * w, -0.004, cz + Math.cos(a) * w], [cx + Math.sin(a) * w, -0.004, cz - Math.cos(a) * w], tip);
    }
  }
  mb.heightSway(m0, 0, h, 0.7);
  mb.mat = mat0;
}

recipe("feat.silt", { variants: 4, build: (mb, r) => buildSilt(mb, r) });

/** A dry tuft of scrub grass with a thorny twig or two. */
function buildTuft(mb, r, variant) {
  mb.mat = MAT.FOLIAGE;
  tufts(mb, r, 3 + (variant % 2), 0.035, 0.085, [T.grassDry, "#9a8a54", "#6e6a3a", "#a89a64"]);
  mb.mat = MAT.MATTE;
  mb.color = C("#4a3a2c");
  for (let k = 0; k < 2; k++) {
    const a = r.range(0, TAU);
    stick(mb, [0, -0.004, 0], [Math.cos(a) * 0.035, r.range(0.04, 0.06), Math.sin(a) * 0.035], 0.0022, 3);
  }
}

recipe("feat.tuft", { variants: 4, build: (mb, r, { variant }) => buildTuft(mb, r, variant) });

/** Bank grass hanging over a river's edge (dress.js placeBank): rooted on the bank top along z,
 *  long fronds combed out toward the water (+x), arching up and drooping over the edge so their
 *  tips hang just above the surface; the lowest ones dark and wet, a shadowed line along the
 *  water, the upper ones catching the light, and a few short blades standing on the bank behind.
 *  `param` "dry": the straw-gold grass of the plains. ~50 triangles, no shadow-map shadow. */
function buildOverhang(mb, r, variant, param) {
  const dry = param === "dry";
  const roots = (dry ? ["#3a3818", "#46401c"] : ["#1c3014", "#22381a"]).map(C);
  const mids = (dry ? ["#8a8034", "#9a8a3c", "#7a7a30"] : ["#3e6a24", "#4a7628", "#56802e"]).map(C);
  const tips = (dry ? ["#d8bc58", "#e6cc6a", "#c8a848"] : ["#94b844", "#a6c44e", "#86ac40"]).map(C);
  mb.mat = MAT.BLADE;
  // A blade: from its root along `dir` (horizontal, unit) up to a crest `h` high, a little under
  // halfway, then down to its tip `drop` from the root (a quad and a tip triangle); colour,
  // occlusion and sway grow toward the tip.
  const blade = (x0, z0, dir, L, w, h, drop, kr, km, kt, crest = 0.42) => {
    const side = [-dir[2], 0, dir[0]];
    const T3 = [0, crest, 1];
    const P = [[x0, -0.006, z0], [x0 + dir[0] * L * crest, h, z0 + dir[2] * L * crest], [x0 + dir[0] * L, drop, z0 + dir[2] * L]];
    const W = [w, w * 0.7, 0];
    const K = [kr, km, kt];
    const n = vnorm([dir[0] * 0.25, 1, dir[2] * 0.25]);
    const V = (k, s) => [P[k][0] + side[0] * W[k] * s, P[k][1], P[k][2] + side[2] * W[k] * s];
    const vtx = (p, k) => {
      mb.sway = 0.95 * Math.pow(T3[k], 1.3);
      mb.vert(p, n, K[k], 0.6 + 0.4 * T3[k]);
    };
    const tri = (a, ka, b, kb, c, kc) => {
      // Wound to face the normal (the shader flips a back face's normal).
      const f = vcross(vsub(b, a), vsub(c, a));
      if (f[0] * n[0] + f[1] * n[1] + f[2] * n[2] < 0) [b, kb, c, kc] = [c, kc, b, kb];
      vtx(a, ka);
      vtx(b, kb);
      vtx(c, kc);
    };
    tri(V(0, -1), 0, V(0, 1), 0, V(1, 1), 1);
    tri(V(0, -1), 0, V(1, 1), 1, V(1, -1), 1);
    tri(V(1, -1), 1, V(1, 1), 1, P[2], 2);
  };
  const n = 13 + (variant % 2);
  for (let k = 0; k < n; k++) {
    // The first few are the under-layer: low, dark, hanging furthest down (the wet line); the
    // rest arch higher and catch the light. (The tips stay just above the water: the roots stand
    // ~0.04 above it, on the bank top just behind the cut bank, dress.js placeBank.)
    const under = k < 4;
    const a = r.range(-0.65, 0.65) * (under ? 0.7 : 1);
    const dir = [Math.cos(a), 0, Math.sin(a)];
    const L = r.range(0.1, 0.15) * (under ? 0.85 : 1);
    const h = under ? r.range(0.008, 0.016) : r.range(0.022, 0.042);
    const drop = under ? r.range(-0.022, -0.014) : r.range(-0.016, 0.004);
    const kr = jitter(r.pick(roots), r, 0.06), km = jitter(r.pick(mids), r, 0.06), kt = jitter(r.pick(tips), r, 0.08);
    blade(r.range(-0.045, -0.005), r.range(-0.06, 0.06), dir, L, r.range(0.0055, 0.008), h, drop, kr, under ? mulc(km, 0.6) : km, under ? mulc(kt, 0.62) : kt, r.range(0.36, 0.5));
  }
  // Short blades standing on the bank top behind, leaning out a little (the meadow running into
  // the overhang): the crest high up, the tip a little over.
  for (let k = 0; k < 3; k++) {
    const a = r.range(-1.3, 1.3);
    const h = r.range(0.05, 0.075);
    blade(r.range(-0.05, -0.02), r.range(-0.05, 0.05), [Math.cos(a), 0, Math.sin(a)], r.range(0.025, 0.04), 0.0065, h, h * 0.8, jitter(r.pick(roots), r, 0.06), jitter(r.pick(mids), r, 0.06), jitter(r.pick(tips), r, 0.08), 0.6);
  }
  mb.sway = 0;
  mb.mat = MAT.MATTE;
}

recipe("feat.overhang", { variants: 4, shadow: false, build: (mb, r, { variant, param }) => buildOverhang(mb, r, variant, param) });

/** Tormented souls: pale wisps writhing up out of the ground (dim emissive, fluttering). */
function buildWisps(mb, r, variant) {
  mb.mat = MAT.EMISSIVE;
  const n = 2 + (variant % 2);
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + r.range(-0.4, 0.4), d = k === 0 ? 0 : r.range(0.03, 0.05);
    const h = r.range(0.15, 0.22) * (k === 0 ? 1 : 0.75);
    const ph = r.range(0, TAU);
    const pts = [], radii = [], cols = [];
    const steps = 7;
    for (let i = 0; i <= steps; i++) {
      const t = i / steps;
      const sw = Math.sin(t * Math.PI * 2 + ph) * 0.02 * (1 - t * 0.5);
      pts.push([Math.cos(a) * d + sw, 0.004 + h * t, Math.sin(a) * d + Math.cos(t * 5 + ph) * 0.01]);
      // A thin tail swelling into a hooded head that closes at the top.
      radii.push(t < 0.75 ? 0.003 + 0.013 * Math.pow(t / 0.75, 1.5) : 0.016 * Math.sqrt(Math.max(0, 1 - ((t - 0.75) / 0.25) ** 2)) + 0.0005);
      cols.push(mixl("#1c2a2a", t > 0.7 ? T.ghostCore : T.ghost, clamp(t * 1.4, 0, 1)));
    }
    const m0 = mb.mark();
    mb.tube(pts, radii, 6, { colors: cols });
    mb.heightSway(m0, 0, h, 0.9);
  }
  mb.mat = MAT.MATTE;
  // Dark, clawing stones they rise between.
  const sh = rockShader(rockPaint("basalt", r), 2);
  for (let k = 0; k < 2; k++) {
    const a = r.range(0, TAU);
    spire(mb, r, { x: Math.cos(a) * 0.06, z: Math.sin(a) * 0.06, h: r.range(0.03, 0.05), rad: 0.012, sides: 4, rings: 1, base: -0.03, lean: [Math.cos(a) * 0.015, Math.sin(a) * 0.015], shade: sh });
  }
}

recipe("feat.wisp", { variants: 4, build: (mb, r, { variant }) => buildWisps(mb, r, variant) });

/** Fallout: a poisoned seep -- a sickly glowing pool in a crust of cracked, dead ground, with
 *  pale crystalline growths. */
function buildFallout(mb, r) {
  mb.color = C("#3a3428");
  const m0 = mb.mark();
  const w = wobbler(r, 0.2);
  mb.lathe([[0.1, -0.02], [0.085, 0.006], [0.055, 0.008], [0.045, 0.004]], 10, { caps: false, wobble: (a) => w(a) });
  mb.tint(m0, (x, y, z) => 0.7 + 0.5 * clamp(Math.hypot(x, z) / 0.09, 0, 1));
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C(T.toxicDim);
    const s0 = mb.mark();
    mb.lathe([[0.047, 0.004], [0, 0.005]], 10, { caps: false, wobble: (a) => w(a) });
    mb.tint(s0, (x, y, z) => 0.6 + 0.8 * clamp(1 - Math.hypot(x, z) / 0.045, 0, 1));
    mb.color = C("#7a8a3a");
    for (let k = 0; k < 4; k++) {
      const a = r.range(0, TAU), d = r.range(0.055, 0.08);
      crystal(mb, [Math.cos(a) * d, -0.005, Math.sin(a) * d], [Math.cos(a) * 0.4, 1, Math.sin(a) * 0.4], r.range(0.025, 0.045), 0.005, 4, { tip: 0.4, foot: 0 });
    }
  });
  mb.color = C("#2c2420");
  for (let k = 0; k < 3; k++) {
    const a = r.range(0, TAU), d = r.range(0.06, 0.09);
    stick(mb, [Math.cos(a) * d, -0.005, Math.sin(a) * d], [Math.cos(a) * (d + 0.01), r.range(0.03, 0.06), Math.sin(a) * (d + 0.01)], 0.0025, 3);
  }
}

recipe("feat.fallout", { variants: 3, build: (mb, r) => buildFallout(mb, r) });

/** n positions in the plot (plot-local x east, z south), off the centre when something holds
 *  the plot, clear of the resource marker's corner and of each other. */
function scatter(ctx, n, { min = 0.16, edge = 0.38, clear = 0.27, avoid = [] } = {}) {
  const r = ctx.r;
  const out = [];
  for (let tries = 0; out.length < n && tries < n * 30; tries++) {
    const x = r.range(-edge, edge), z = r.range(-edge, edge);
    if (ctx.held && Math.hypot(x, z - 0.02) < clear) continue;
    if (ctx.bonus && Math.hypot(x + 0.26, z - 0.2) < 0.15) continue;
    if (avoid.some(([ax, az, ar]) => Math.hypot(x - ax, z - az) < ar)) continue;
    if (out.some(([ox, oz]) => Math.hypot(x - ox, z - oz) < min)) continue;
    out.push([x, z]);
  }
  return out;
}

function featureOf(feat, ctx) {
  const r = ctx.r;
  const yaw = () => r.range(0, TAU);
  switch (feat) {
    case FEAT.OASIS: {
      const palm = hasRecipe("tree.palm") ? "tree.palm" : "feat.palm";
      const reeds = hasRecipe("reeds") ? "reeds" : "feat.tuft";
      const px = 0.17, pz = -0.15;
      return [
        { asset: "feat.pool", dx: px, dz: pz, scale: 1, yaw: yaw() },
        { asset: palm, dx: px - 0.17, dz: pz - 0.12, scale: r.range(0.85, 1.0), yaw: yaw() },
        { asset: palm, dx: px + 0.17, dz: pz + 0.09, scale: r.range(0.9, 1.05), yaw: yaw() },
        { asset: palm, dx: px + 0.08, dz: pz - 0.2, scale: r.range(0.7, 0.85), yaw: yaw() },
        { asset: reeds, dx: px - 0.13, dz: pz + 0.06, scale: 0.6, yaw: yaw() },
        { asset: reeds, dx: px + 0.13, dz: pz - 0.06, scale: 0.55, yaw: yaw() },
      ];
    }
    case FEAT.VOLCANO:
      return [{ asset: "feat.volcano", dx: 0, dz: 0, scale: r.range(0.8, 1.0), yaw: yaw() }];
    case FEAT.FLAMES:
      return scatter(ctx, 3, { min: 0.24 }).map(([dx, dz], k) => ({ asset: "feat.flames", dx, dz, scale: k ? r.range(0.8, 1.0) : 1.15, yaw: yaw() }));
    case FEAT.ICE:
      return scatter({ ...ctx, held: false, bonus: null }, r.int(3, 5), { min: 0.2, edge: 0.36 }).map(([dx, dz]) => ({ asset: "feat.icefloe", dx, dz, scale: r.range(0.8, 1.3), yaw: yaw(), float: true }));
    case FEAT.FLOOD: {
      const reeds = hasRecipe("reeds") ? "reeds" : "feat.tuft";
      const pts = scatter(ctx, 5, { min: 0.16 });
      return pts.map(([dx, dz], k) => (k < 2 ? { asset: "feat.silt", dx, dz, scale: r.range(0.9, 1.2), yaw: yaw() } : { asset: reeds, dx, dz, scale: r.range(0.6, 0.85), yaw: yaw() }));
    }
    case FEAT.SCRUB: {
      const shrub = hasRecipe("shrub.dry") ? "shrub.dry" : "feat.tuft";
      return scatter(ctx, 6, { min: 0.14 }).map(([dx, dz], k) => ({ asset: k < 2 ? shrub : "feat.tuft", dx, dz, scale: r.range(0.8, 1.15), yaw: yaw() }));
    }
    case FEAT.OBSIDIAN:
      return scatter(ctx, 4, { min: 0.18 }).map(([dx, dz]) => ({ asset: "rock.shard", dx, dz, scale: r.range(0.8, 1.3), yaw: yaw() }));
    case FEAT.SOULS:
      return scatter(ctx, 3, { min: 0.22 }).map(([dx, dz]) => ({ asset: "feat.wisp", dx, dz, scale: r.range(0.9, 1.2), yaw: yaw() }));
    case FEAT.FALLOUT: {
      const pts = scatter(ctx, 3, { min: 0.22 });
      return pts.map(([dx, dz], k) => (k < 2 ? { asset: "feat.fallout", dx, dz, scale: r.range(1.0, 1.4), yaw: yaw() } : { asset: hasRecipe("tree.dead") ? "tree.dead" : "feat.tuft", dx, dz, scale: 0.6, yaw: yaw() }));
    }
    default:
      return null;
  }
}

// ================================================================================================
// Improvements: shared pieces
// ================================================================================================

/** Materials of a structure by the owner's architectural family (param "human", "elven.dark"...). */
const FAMILY = {
  human: { wall: "#cfc2a2", wall2: "#a8987a", timber: "#4e3626", roof: "#86703e", roof2: "#8a3d2b", stone: "#6e6862", door: "#2e2018", trim: "#7a5a3c" },
  elven: { wall: "#a89a72", wall2: "#7e7254", timber: "#5e4a30", roof: "#55703c", roof2: "#6a7a3a", stone: "#7e7c6c", door: "#3a2e1e", trim: "#a08a52" },
  dwarven: { wall: "#7a746c", wall2: "#5e5852", timber: "#4a3626", roof: "#4a5260", roof2: "#3b3b42", stone: "#5a5650", door: "#2a2018", trim: "#9a6a3a" },
  infernal: { wall: "#3e2c2a", wall2: "#2a1e1e", timber: "#22181a", roof: "#2b1516", roof2: "#3a1a1a", stone: "#2e2a2e", door: "#120a0a", trim: "#7a2a1a" },
  tribal: { wall: "#7a6448", wall2: "#5a4a36", timber: "#4a3826", roof: "#8a6a4a", roof2: "#6a5038", stone: "#62605a", door: "#2a1e16", trim: "#9a7a52" },
  desert: { wall: "#c8aa7c", wall2: "#a68a62", timber: "#5e4630", roof: "#b09468", roof2: "#9a6a48", stone: "#a08a6a", door: "#3a2a1c", trim: "#4a7a8a" },
  coastal: { wall: "#c8c0b0", wall2: "#9c9484", timber: "#4a3a2e", roof: "#4a5260", roof2: "#6a7480", stone: "#6e6a66", door: "#2a2620", trim: "#5a6a7a" },
};

function famOf(param) {
  const k = String(param ?? "").split(".")[0];
  const f = FAMILY[k] ?? FAMILY.human;
  return { ...f, key: FAMILY[k] ? k : "human", dark: String(param ?? "").includes("dark") };
}

/** A gable roof (ridge along x) over a w x d wall top at y = 0: two slopes with an overhang, a
 *  darker slab edge, the gable ends filled with `ends` and the slopes darker toward the eaves. */
function roofGable(mb, w, d, h, { over = 0.012, col, ends = null, edge = null, sag = 0 } = {}) {
  const X = w / 2 + over * 0.8, Z = d / 2 + over;
  const eave = -over * (h / (d / 2)) * 0.6;
  const m0 = mb.mark();
  const rc = C(col), ec = edge ? C(edge) : mulc(rc, 0.5);
  const segs = sag ? 3 : 1;
  for (let i = 0; i < segs; i++) {
    const x0 = -X + (2 * X * i) / segs, x1 = -X + (2 * X * (i + 1)) / segs;
    const y0 = h - sag * Math.sin((Math.PI * i) / segs), y1 = h - sag * Math.sin((Math.PI * (i + 1)) / segs);
    mb.color = rc;
    mb.quad([x0, y0, 0], [x1, y1, 0], [x1, eave, -Z], [x0, eave, -Z]);
    mb.quad([x0, y0, 0], [x0, eave, Z], [x1, eave, Z], [x1, y1, 0]);
    mb.color = ec;
    mb.quad([x0, eave, Z], [x0, eave - 0.006, Z], [x1, eave - 0.006, Z], [x1, eave, Z]);
    mb.quad([x0, eave, -Z], [x1, eave, -Z], [x1, eave - 0.006, -Z], [x0, eave - 0.006, -Z]);
  }
  mb.tint(m0, (x, y) => 0.76 + 0.3 * clamp((y - eave) / (h - eave), 0, 1));
  mb.color = C(ends ?? col);
  for (const e of [-1, 1]) {
    const x = (e * w) / 2;
    if (e < 0) mb.tri([x, 0, -d / 2], [x, 0, d / 2], [x, h * 0.98, 0]);
    else mb.tri([x, 0, d / 2], [x, 0, -d / 2], [x, h * 0.98, 0]);
  }
  // Verge boards.
  mb.color = ec;
  for (const e of [-1, 1]) {
    for (const s of [-1, 1]) {
      const x = e * X;
      if (e * s > 0) mb.quad([x, h, 0], [x, eave, s * Z], [x, eave - 0.005, s * Z], [x, h - 0.005, 0]);
      else mb.quad([x, h, 0], [x, h - 0.005, 0], [x, eave - 0.005, s * Z], [x, eave, s * Z]);
    }
  }
}

/** A plain block (walls, plinths): box from y0 to y1, no bottom face. */
function block(mb, w, d, y0, y1, col, top = 1) {
  mb.part(() => {
    mb.color = C(col);
    mb.translate(0, y0, 0);
    mb.box(w, y1 - y0, d, { top });
  });
}

/** A rectangle on the plane z = const (doors, windows, boards), facing +z. */
function plate(mb, x0, y0, x1, y1, z, col) {
  mb.color = C(col);
  mb.quad([x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]);
}

/** A small timber-and-plaster building: a stone footing sunk into the ground, walls with dark
 *  corner posts, a door and a lit window on the front (+z), a gable roof. Returns the wall top. */
function cottage(mb, r, F, w, d, h, { roof = null, roofH = null, door = true, lit = true, posts = true, sag = 0.004 } = {}) {
  block(mb, w + 0.008, d + 0.008, -0.05, 0.008, J(F.stone, r, 0.04));
  block(mb, w, d, 0.006, h, J(F.wall, r, 0.04));
  if (posts && F.key !== "desert") {
    mb.color = C(F.timber);
    for (const [sx, sz] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) at(mb, (sx * w) / 2, 0.006, (sz * d) / 2, 0, () => mb.box(0.008, h - 0.006, 0.008));
    at(mb, 0, h - 0.008, d / 2 + 0.0015, 0, () => mb.box(w, 0.007, 0.003));
  }
  if (door) plate(mb, -0.012, 0.008, 0.012, 0.045, d / 2 + 0.002, F.door);
  if (lit) {
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      plate(mb, w * 0.22, h * 0.45, w * 0.22 + 0.012, h * 0.45 + 0.014, d / 2 + 0.002, "#e8a050");
    });
  }
  at(mb, 0, h, 0, 0, () => roofGable(mb, w, d, roofH ?? d * 0.75, { col: roof ?? F.roof, ends: F.wall2, sag }));
  return h;
}

/** A line of fence: posts along the polyline `pts` ([x, z] ...) with two rails between them. */
function fenceLine(mb, pts, { h = 0.042, col = "#857358", post = 0.005, step = 0.05 } = {}) {
  mb.color = C(col);
  const P = [];
  for (let i = 0; i + 1 < pts.length; i++) {
    const [ax, az] = pts[i], [bx, bz] = pts[i + 1];
    const L = Math.hypot(bx - ax, bz - az);
    const n = Math.max(1, Math.round(L / step));
    for (let k = 0; k < n; k++) P.push([ax + ((bx - ax) * k) / n, az + ((bz - az) * k) / n]);
  }
  P.push(pts[pts.length - 1]);
  for (const [x, z] of P) stick(mb, [x, -0.02, z], [x, h, z], post, 3);
  // Two rails along the polyline (one member per leg of it: cheap, and they read as lines).
  mb.color = mulc(col, 1.2);
  for (let i = 0; i + 1 < pts.length; i++) {
    for (const y of [h * 0.45, h * 0.85]) stick(mb, [pts[i][0], y, pts[i][1]], [pts[i + 1][0], y + 0.001, pts[i + 1][1]], post * 0.65, 3);
  }
}

// --- farm ---------------------------------------------------------------------------------------

/** A farm's main building by variant: a big barn (0-1), a granary raised on staddle stones (2) or
 *  a round granary with a conical thatch (3). */
function buildBarn(mb, r, variant, param) {
  const F = famOf(param);
  // Weathered thatch: grey-brown, not straw-yellow (the low sun warms it enough).
  const thatch = F.key === "human" || F.key === "tribal" ? J(variant === 1 ? "#9a8a62" : "#ac9866", r, 0.05) : J(F.roof, r, 0.05);
  if (variant <= 1) {
    // The barn stands gable-end on to the viewer: its ridge runs north-south, so both slopes of
    // the big roof show (one sunlit, one in shade) over the great doors -- the farm's emblem.
    const w = r.range(0.105, 0.115), d = r.range(0.16, 0.18), h = 0.078, rh = 0.1;
    const boards = J(variant ? "#6a4630" : F.key === "human" ? "#7c4632" : F.wall, r, 0.05);
    block(mb, w + 0.012, d + 0.012, -0.05, 0.01, J(F.stone, r, 0.04));
    block(mb, w, d, 0.008, h, boards);
    mb.color = C(F.timber);
    for (const [sx, sz] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) at(mb, (sx * w) / 2, 0.008, (sz * d) / 2, 0, () => mb.box(0.011, h - 0.008, 0.011));
    // Great doors (dark, framed) and the hay loft's hatch in the gable above them.
    plate(mb, -0.03, 0.009, 0.03, 0.066, d / 2 + 0.002, "#2a1c14");
    mb.color = C("#a08058");
    for (const x of [-0.032, 0.032]) at(mb, x, 0.009, d / 2 + 0.002, 0, () => mb.box(0.006, 0.06, 0.004));
    at(mb, 0, 0.066, d / 2 + 0.002, 0, () => mb.box(0.07, 0.006, 0.004));
    plate(mb, -0.012, h + 0.018, 0.012, h + 0.042, d / 2 + 0.002, "#2a1c14");
    at(mb, 0, h, 0, Math.PI / 2, () => roofGable(mb, d, w, rh, { col: thatch, ends: mixl(boards, "#000000", 0.12), over: 0.016, sag: 0.006 }));
    // A low lean-to along one side.
    const s = variant ? -1 : 1;
    at(mb, s * (w / 2 + 0.028), 0, -0.01, 0, () => {
      block(mb, 0.05, d * 0.7, 0.0, 0.045, J("#5e4430", r, 0.05));
      mb.color = mulc(thatch, 0.85);
      const x0 = -s * 0.03, x1 = s * 0.032;
      mb.quad([x0, 0.066, -d * 0.38], [x1, 0.042, -d * 0.38], [x1, 0.042, d * 0.38], [x0, 0.066, d * 0.38]);
      mb.quad([x0, 0.066, -d * 0.38], [x0, 0.066, d * 0.38], [x1, 0.042, d * 0.38], [x1, 0.042, -d * 0.38]);
    });
  } else if (variant === 2) {
    // Granary on staddle stones (keeps the grain from the rats).
    const w = 0.1, d = 0.08;
    mb.color = C("#7e7a72");
    for (const [sx, sz] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
      at(mb, sx * w * 0.4, 0, sz * d * 0.38, 0.4, () => {
        mb.lathe([[0.009, -0.03], [0.006, 0.018], [0, 0.018]], 4);
        mb.lathe([[0.015, 0.018], [0.015, 0.024], [0, 0.024]], 5);
      });
    }
    block(mb, w, d, 0.024, 0.075, J("#7a5a3c", r, 0.05));
    plate(mb, -0.012, 0.03, 0.012, 0.065, d / 2 + 0.002, "#3a281c");
    mb.color = C("#5a4430");
    stick(mb, [0, 0.0, d / 2 + 0.04], [0, 0.03, d / 2 + 0.003], 0.003, 3);
    at(mb, 0, 0.075, 0, 0, () => roofGable(mb, w, d, 0.06, { col: thatch, ends: "#6e4c34", over: 0.016 }));
  } else {
    // Round granary: a plastered drum under a tall cone of thatch.
    mb.color = J(F.stone, r, 0.04);
    mb.lathe([[0.058, -0.04], [0.058, 0.012], [0, 0.012]], 9);
    mb.color = J(F.key === "human" ? "#bfae8c" : F.wall, r, 0.04);
    mb.lathe([[0.05, 0.01], [0.048, 0.07]], 9, { caps: false });
    plate(mb, -0.011, 0.012, 0.011, 0.05, 0.05, "#3a281c");
    mb.color = thatch;
    const m0 = mb.mark();
    mb.lathe([[0.07, 0.062], [0.064, 0.07], [0.03, 0.115], [0.006, 0.14], [0, 0.142]], 9);
    mb.tint(m0, (x, y) => 0.75 + 0.35 * clamp((y - 0.06) / 0.08, 0, 1));
  }
  groundAO(mb, 0, -0.02, 0.05, 0.6);
}

recipe("imp.barn", { variants: 4, build: (mb, r, { variant, param }) => buildBarn(mb, r, variant, param) });

/** Haystacks: two or three domed ricks and a few sheaves. */
function buildHaystacks(mb, r, variant) {
  // The brightest thing on a farm: fat, soft ricks of pale straw (they say "farm" from far away),
  // rounded like loaves as the look target's are, combed light and dark down their flanks.
  const hay = J("#d2b264", r, 0.05);
  const n = 2 + (variant % 2);
  const seed = r.range(0, 50);
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + r.range(-0.3, 0.3), d = k ? r.range(0.06, 0.075) : 0;
    const s = (k ? r.range(0.75, 0.9) : 1) * 1.25;
    at(mb, Math.cos(a) * d, 0, Math.sin(a) * d, r.range(0, TAU), () => {
      mb.color = hay;
      const m0 = mb.mark();
      if (variant >= 2 && k === 0) {
        // A long rick, its top rounded over.
        mb.scale(1.5, 1, 1);
        mb.lathe([[0.034 * s, -0.01], [0.037 * s, 0.014], [0.034 * s, 0.032], [0.025 * s, 0.047], [0.012 * s, 0.056], [0, 0.059 * s]], 9);
      } else mb.lathe([[0.03 * s, -0.01], [0.033 * s, 0.012], [0.031 * s, 0.029], [0.023 * s, 0.045 * s], [0.011 * s, 0.056 * s], [0, 0.06 * s]], 9);
      // Lit crown, shaded foot, and streaks of the straw down the flanks.
      mb.tint(m0, (x, y, z) => (0.66 + 0.46 * clamp(y / 0.055, 0, 1)) * (0.93 + 0.1 * noise3(Math.atan2(z, x) * 2.2 + seed, y * 18, k)));
    });
  }
  // Sheaves leaning together.
  mb.color = C("#c4aa5c");
  const a = r.range(0, TAU);
  for (let k = 0; k < 3; k++) {
    const b = a + (k / 3) * TAU;
    stick(mb, [Math.cos(a) * 0.09 + Math.cos(b) * 0.012, -0.004, Math.sin(a) * 0.09 + Math.sin(b) * 0.012], [Math.cos(a) * 0.09, 0.035, Math.sin(a) * 0.09], 0.006, 4);
  }
}

recipe("imp.haystacks", { variants: 4, build: (mb, r, { variant }) => buildHaystacks(mb, r, variant) });

/** A scarecrow: a post and crossbar, a sack head under a battered hat, a ragged coat. */
function buildScarecrow(mb, r) {
  mb.color = C("#5a4430");
  stick(mb, [0, -0.03, 0], [0, 0.088, 0], 0.0028, 4);
  stick(mb, [-0.035, 0.064, 0], [0.035, 0.066, 0], 0.0022, 4);
  mb.color = C("#b8a070");
  at(mb, 0, 0.088, 0, 0, () => mb.sphere(0.011, 0, { squash: 1.1 }));
  mb.color = C("#3a2e24");
  at(mb, 0, 0.096, 0, r.range(-0.4, 0.4), () => {
    mb.rotateZ(r.range(-0.2, 0.2));
    mb.lathe([[0.02, 0], [0.018, 0.003], [0.009, 0.004], [0.008, 0.016], [0, 0.017]], 7);
  });
  // A ragged coat flapping in the wind.
  mb.color = J("#6a5a3e", r, 0.08);
  const m0 = mb.mark();
  mb.card([-0.03, 0.068, 0.004], [0.03, 0.068, 0.004], [0.022, 0.03, 0.006], [-0.024, 0.026, 0.006]);
  mb.card([-0.034, 0.068, 0.0], [-0.022, 0.068, 0.0], [-0.03, 0.05, 0.002], [-0.04, 0.052, 0.002]);
  mb.card([0.022, 0.068, 0.0], [0.034, 0.068, 0.0], [0.04, 0.05, 0.002], [0.03, 0.05, 0.002]);
  mb.heightSway(m0, 0.07, 0.025, 0.6);
}

recipe("imp.scarecrow", { variants: 2, build: (mb, r) => buildScarecrow(mb, r) });

/** A farm cart (a farmyard's, or a town yard's): a plank bed on two spoked wheels, its shafts
 *  resting on the ground in front; v0 loaded with hay, v1 with sacks and a barrel. Along x, the
 *  shafts toward +x. */
function buildCart(mb, r, variant) {
  const wood = J("#7a5a3a", r, 0.06), dark = mulc(wood, 0.62);
  const L = 0.07, W = 0.04, y0 = 0.02;
  // The bed: a floor and low plank sides.
  mb.color = wood;
  at(mb, 0, y0, 0, 0, () => mb.box(L, 0.004, W));
  mb.color = mulc(wood, 0.88);
  for (const s of [-1, 1]) at(mb, 0, y0, (s * W) / 2, 0, () => mb.box(L, 0.011, 0.003));
  at(mb, -L / 2, y0, 0, 0, () => mb.box(0.003, 0.011, W));
  // Shafts down to the ground in front.
  mb.color = dark;
  for (const s of [-1, 1]) stick(mb, [L * 0.2, y0, s * W * 0.32], [L / 2 + 0.045, -0.002, s * W * 0.26], 0.0022, 4);
  // Wheels: a dark tyre disc with a lighter hub, either side of the axle.
  for (const s of [-1, 1]) {
    at(mb, -0.004, 0.018, s * (W / 2 + 0.004), 0, () => {
      mb.rotateX(QT);
      mb.color = dark;
      mb.cylinder(0.018, 0.018, 0.004, 9, { caps: true });
      mb.color = mulc(wood, 1.1);
      mb.translate(0, s > 0 ? 0.0045 : -0.0005, 0);
      mb.cylinder(0.0045, 0.0045, 0.001, 5, { caps: true });
    });
  }
  if (variant % 2 === 0) {
    // A load of hay heaped over the bed.
    mb.color = J("#c8a858", r, 0.05);
    const m0 = mb.mark();
    at(mb, -0.004, y0 + 0.004, 0, 0, () => {
      mb.scale(1.55, 1, 1);
      mb.lathe([[0.022, 0], [0.023, 0.01], [0.017, 0.022], [0.006, 0.029], [0, 0.03]], 7);
    });
    mb.tint(m0, (x, y) => 0.72 + 0.38 * clamp((y - y0) / 0.03, 0, 1));
  } else {
    // Sacks and a barrel.
    for (let k = 0; k < 3; k++) {
      mb.color = J(k % 2 ? "#b8a07a" : "#a48a64", r, 0.06);
      at(mb, -0.022 + k * 0.016, y0 + 0.01, r.range(-0.006, 0.006), r.range(-0.4, 0.4), () => mb.sphere(0.0095, 0, { squash: 0.75 }));
    }
    mb.color = J("#6a4a30", r, 0.06);
    at(mb, 0.024, y0 + 0.002, 0, 0, () => mb.lathe([[0.008, 0], [0.0095, 0.01], [0.008, 0.02], [0, 0.0205]], 7));
  }
  groundAO(mb, 0, -0.005, 0.03, 0.65);
}

recipe("imp.cart", { variants: 2, build: (mb, r, { variant }) => buildCart(mb, r, variant) });

/** Farm tools by the yard: v0 a pitchfork, a rake and a scythe leaning on a trestle beside a
 *  wheelbarrow; v1 a chopping block with its axe, split logs and a basket. */
function buildTools(mb, r, variant) {
  const wood = J("#7a5a3a", r, 0.06), iron = C("#55545a");
  if (variant % 2 === 0) {
    // The trestle: a bar on two pairs of splayed legs.
    mb.color = mulc(wood, 0.85);
    stick(mb, [-0.024, 0.026, 0], [0.024, 0.026, 0], 0.0024, 4);
    for (const x of [-0.02, 0.02]) for (const s of [-1, 1]) stick(mb, [x, 0.026, 0], [x + s * 0.003, -0.004, s * 0.012], 0.0018, 3);
    // Tools leaning on it: hafts from the ground behind up over the bar, iron heads on top.
    for (let k = 0; k < 3; k++) {
      const x = -0.016 + k * 0.016 + r.range(-0.003, 0.003);
      const foot = [x, -0.002, -0.026], top = [x + r.range(-0.004, 0.004), 0.05, 0.012];
      mb.color = mulc(wood, 1.12);
      stick(mb, foot, top, 0.0016, 3);
      mb.mat = MAT.METAL;
      mb.color = iron;
      if (k === 0) for (const s of [-1, 0, 1]) stick(mb, top, [top[0] + s * 0.004, top[1] + 0.012, top[2] + 0.003], 0.0008, 3); // fork
      else if (k === 1) stick(mb, [top[0] - 0.008, top[1], top[2]], [top[0] + 0.008, top[1], top[2]], 0.0012, 3); // rake
      else mb.quad([top[0], top[1], top[2]], [top[0] + 0.003, top[1] + 0.002, top[2]], [top[0] + 0.024, top[1] - 0.008, top[2] - 0.004], [top[0] + 0.022, top[1] - 0.009, top[2] - 0.004]); // scythe
      mb.mat = MAT.MATTE;
    }
    // A wheelbarrow beside it.
    mb.color = wood;
    at(mb, 0.044, 0.012, 0.004, 0.4, () => {
      mb.box(0.026, 0.012, 0.02, { top: 1.25 });
      mb.color = mulc(wood, 0.62);
      at(mb, 0.017, -0.004, 0, 0, () => {
        mb.rotateX(QT);
        mb.cylinder(0.008, 0.008, 0.004, 7, { caps: true });
      });
      for (const s of [-1, 1]) stick(mb, [-0.01, 0.002, s * 0.008], [-0.032, -0.006, s * 0.01], 0.0016, 3);
    });
  } else {
    // A chopping block with its axe, split logs about it and a basket of kindling.
    mb.color = J("#8a6a48", r, 0.05);
    mb.cylinder(0.012, 0.011, 0.016, 7, { caps: true });
    mb.color = mulc(wood, 1.1);
    stick(mb, [0.002, 0.016, 0], [0.016, 0.034, 0.004], 0.0014, 3);
    mb.mat = MAT.METAL;
    mb.color = iron;
    at(mb, 0.002, 0.017, 0.0, 0.3, () => mb.box(0.008, 0.006, 0.0025));
    mb.mat = MAT.MATTE;
    mb.color = J("#a4825a", r, 0.06);
    for (let k = 0; k < 5; k++) {
      const a = r.range(0, TAU), d = r.range(0.018, 0.03);
      at(mb, Math.cos(a) * d, 0.003, Math.sin(a) * d, r.range(0, TAU), () => mb.box(0.014, 0.005, 0.005));
    }
    mb.color = J("#9a7a4a", r, 0.06);
    at(mb, -0.03, 0, 0.01, 0, () => mb.lathe([[0.008, 0], [0.011, 0.012], [0.0105, 0.013], [0, 0.009]], 7, { caps: false }));
  }
  groundAO(mb, 0, -0.005, 0.03, 0.65);
}

recipe("imp.tools", { variants: 2, build: (mb, r, { variant }) => buildTools(mb, r, variant) });

// --- animals (pastures and the livestock markers) -------------------------------------------------

/** Body plans: length, leg height, girth, colours, head kind. One unit ~ a plot; these are model
 *  sizes (the catalog scales them). */
const ANIMALS = {
  cow: { len: 0.11, leg: 0.035, girth: 0.03, coat: "#d8d0c0", patch: "#3a2e28", head: "bovine", horns: true, tail: 0.04 },
  ox: { len: 0.115, leg: 0.034, girth: 0.032, coat: "#7a5a40", patch: "#5a4030", head: "bovine", horns: true, tail: 0.04 },
  horse: { len: 0.11, leg: 0.055, girth: 0.024, coat: "#6a4630", patch: "#5a3a26", mane: "#241a14", head: "horse", tail: 0.05 },
  sheep: { len: 0.075, leg: 0.026, girth: 0.03, coat: "#d8d0bc", patch: "#d8d0bc", face: "#2a2420", head: "sheep", wool: true, tail: 0.012 },
  pig: { len: 0.085, leg: 0.02, girth: 0.03, coat: "#d4a090", patch: "#b8806e", head: "pig", tail: 0.015 },
  deer: { len: 0.085, leg: 0.05, girth: 0.02, coat: "#8a6040", patch: "#d8c8a8", head: "deer", antlers: true, tail: 0.012 },
  toad: null,
};

/** A small, readable quadruped silhouette facing +z: barrel body, four legs, neck and head, tail;
 *  the coat two-toned (patches, a pale belly). ~120-170 triangles. */
function animal(mb, r, kind, { pose = 0 } = {}) {
  const A = ANIMALS[kind] ?? ANIMALS.cow;
  const L = A.len, G = A.girth, H = A.leg;
  const yb = H + G * 0.85;
  const coat = J(A.coat, r, 0.06);
  const seed = r.range(0, 50);
  // Legs.
  mb.color = kind === "sheep" ? C(A.face) : mulc(coat, 0.7);
  for (const [sx, sz] of [[-1, 1], [1, 1], [-1, -1], [1, -1]]) {
    const x = sx * G * 0.5, z = sz * L * 0.32;
    const swing = pose ? sz * 0.006 : 0;
    stick(mb, [x, yb - G * 0.3, z], [x, -0.004, z + swing], kind === "horse" || kind === "deer" ? 0.0045 : 0.006, 3);
  }
  // Body: a lathe along z, pinched at the waist.
  const m0 = mb.mark();
  mb.color = coat;
  mb.part(() => {
    mb.translate(0, yb, 0);
    mb.rotateX(Math.PI / 2);
    const prof = kind === "pig" ? [[0.0, -L / 2], [G * 0.75, -L * 0.42], [G, -L * 0.2], [G, L * 0.15], [G * 0.85, L * 0.38], [0, L / 2]] : [[0.0, -L / 2], [G * 0.8, -L * 0.42], [G, -L * 0.2], [G * 0.92, 0], [G, L * 0.22], [G * 0.75, L * 0.42], [0, L / 2]];
    if (A.wool) mb.blob(G * 1.25, { detail: 1, amp: 0.12, squash: 1, noise: (v) => noise3(v[0] * 6 + seed, v[1] * 6, v[2] * 6) });
    else mb.lathe(prof.map(([rr, y]) => [rr, y]), 6, { caps: false });
  });
  if (A.wool) mb.displace(m0, (x, y, z) => [0, 0, z * 0.4]);
  // Pale belly, patches.
  mb.tint(m0, (x, y, z) => {
    let k = 0.82 + 0.3 * clamp((y - yb + G) / (2 * G), 0, 1);
    if (A.patch !== A.coat && noise3(x * 60 + seed, y * 60, z * 60) > 0.2) {
      const p = C(A.patch), c = coat;
      return [(p[0] / c[0]) * k, (p[1] / c[1]) * k, (p[2] / c[2]) * k];
    }
    return k;
  });
  // Neck and head.
  const front = L / 2;
  const hy = kind === "horse" ? yb + G * 1.9 : kind === "deer" ? yb + G * 2.3 : kind === "pig" ? yb + G * 0.1 : yb + G * 0.6;
  const hz = front + (kind === "horse" ? 0.012 : kind === "pig" ? -0.004 : 0.008);
  mb.color = kind === "sheep" ? C(A.face) : coat;
  if (kind === "horse" || kind === "deer") stick(mb, [0, yb + G * 0.4, front - G * 0.6], [0, hy, hz], G * (kind === "horse" ? 0.55 : 0.4), 5);
  else if (kind !== "pig") stick(mb, [0, yb + G * 0.2, front - G * 0.5], [0, hy, hz], G * 0.55, 5);
  const headLen = kind === "horse" ? 0.034 : kind === "pig" ? 0.02 : kind === "deer" ? 0.024 : 0.026;
  const down = kind === "horse" ? 0.75 : kind === "deer" ? 0.35 : kind === "pig" ? 0.1 : 0.45;
  const tip = [0, hy - Math.sin(down) * headLen, hz + Math.cos(down) * headLen];
  mb.tube([[0, hy, hz - 0.004], tip], [G * (kind === "pig" ? 0.75 : 0.5), G * (kind === "pig" ? 0.45 : 0.3)], 5, { caps: true });
  if (kind === "pig") {
    mb.color = mulc(coat, 0.85);
    stick(mb, tip, vadd(tip, [0, 0, 0.004]), G * 0.4, 5, true);
  }
  // Ears.
  mb.color = mulc(mb.color, 0.85);
  for (const s of [-1, 1]) mb.tri([s * G * 0.25, hy + 0.002, hz], [s * G * 0.9, hy + 0.006, hz - 0.004], [s * G * 0.35, hy + 0.008, hz - 0.006]);
  if (A.horns) {
    mb.color = C("#d8ccb0");
    for (const s of [-1, 1]) mb.tube([[s * G * 0.25, hy + 0.004, hz], [s * G * 0.75, hy + 0.01, hz + 0.002], [s * G * 0.85, hy + 0.017, hz + 0.006]], [0.0025, 0.002, 0.0008], 3);
  }
  if (A.antlers) {
    mb.color = C("#b8a888");
    for (const s of [-1, 1]) {
      const b = [s * 0.004, hy + 0.006, hz - 0.002];
      const t1 = [s * 0.016, hy + 0.03, hz - 0.006];
      stick(mb, b, t1, 0.0016, 3);
      stick(mb, vadd(b, vscale(vsub(t1, b), 0.5)), [s * 0.02, hy + 0.024, hz + 0.006], 0.0012, 3);
      stick(mb, t1, [s * 0.012, hy + 0.04, hz + 0.002], 0.001, 3);
    }
  }
  if (A.mane) {
    mb.color = C(A.mane);
    mb.tube([[0, hy + 0.004, hz - 0.004], [0, yb + G * 1.1, front - G * 0.7]], [G * 0.22, G * 0.3], 4);
  }
  // Tail.
  mb.color = A.mane ? C(A.mane) : mulc(coat, 0.8);
  const tb = [0, yb + G * 0.5, -L / 2 + 0.002];
  stick(mb, tb, [0, yb + G * 0.5 - A.tail * 0.9, -L / 2 - A.tail * 0.3], kind === "horse" ? 0.006 : 0.0025, 4);
  groundAO(mb, 0, 0, yb, 0.65);
}

// Seen in profile (head to the east, turned a little toward the camera): the readable view.
for (const k of ["cow", "ox", "horse", "sheep", "pig", "deer"]) {
  recipe(`anim.${k}`, {
    variants: 3,
    build: (mb, r, { variant }) =>
      mb.part(() => {
        mb.rotateY(Math.PI / 2 - 0.45 + (variant === 2 ? Math.PI - 0.0 : 0));
        animal(mb, r, k, { pose: variant % 2 });
      }),
  });
}

/** A pasture's paddock: a ring of post-and-rail fence (open toward the front) and a lean-to
 *  shelter with a trough at the back, left of the middle or right by variant (fields.js paddock
 *  treads the ground round it bare). */
function buildPaddock(mb, r, variant, param) {
  const F = famOf(param);
  const pts = [];
  const gap = variant % 2 ? 1.1 : 0.8;
  for (let k = 0; k <= 12; k++) {
    const a = Math.PI / 2 + gap / 2 + (k / 12) * (TAU - gap);
    pts.push([Math.cos(a) * 0.36 * (1 + 0.04 * Math.sin(a * 3)), Math.sin(a) * 0.3 * (1 + 0.04 * Math.cos(a * 2))]);
  }
  fenceLine(mb, pts, { col: F.key === "infernal" ? "#3a2c28" : "#8a785c", step: 0.11 });
  // Lean-to shelter and trough.
  at(mb, variant % 2 ? 0.08 : -0.08, 0, -0.22, 0, () => {
    mb.color = C(F.timber);
    for (const x of [-0.04, 0.04]) {
      stick(mb, [x, -0.02, 0.025], [x, 0.05, 0.025], 0.003, 4);
      stick(mb, [x, -0.02, -0.025], [x, 0.065, -0.025], 0.003, 4);
    }
    mb.color = J(F.key === "human" ? "#a08a50" : F.roof, r, 0.05);
    mb.card([-0.05, 0.07, -0.032], [0.05, 0.07, -0.032], [0.05, 0.05, 0.034], [-0.05, 0.05, 0.034]);
    block(mb, 0.09, 0.006, 0.0, 0.03, J("#5e4632", r));
    mb.color = C("#4a3a2a");
    at(mb, 0, 0, 0.05, 0, () => mb.box(0.05, 0.012, 0.014));
    mb.color = C("#a89048");
    at(mb, 0, 0.012, 0.05, 0, () => mb.box(0.044, 0.002, 0.009));
  });
}

recipe("imp.paddock", { variants: 2, build: (mb, r, { variant, param }) => buildPaddock(mb, r, variant, param) });

// --- hunting camp ---------------------------------------------------------------------------------

/** A tent: a hide tipi with its poles crossing at the top (0-1) or a low ridge tent (2). */
function buildTent(mb, r, variant) {
  const hide = J(r.pick(["#9a7a56", "#8a6a4a", "#a88a62"]), r, 0.05);
  if (variant < 2) {
    const h = r.range(0.085, 0.1), rad = r.range(0.036, 0.042);
    mb.color = hide;
    const m0 = mb.mark();
    mb.lathe([[rad, -0.006], [rad * 0.55, h * 0.5], [rad * 0.12, h * 0.9]], 7, { caps: false });
    mb.tint(m0, (x, y) => (y < 0.012 ? 0.7 : 1) * (0.85 + 0.25 * clamp(y / h, 0, 1)));
    // A painted band and the door flap.
    mb.color = C(variant ? "#7a2e1e" : "#3a4a5a");
    mb.lathe([[rad * 0.8, h * 0.28], [rad * 0.7, h * 0.36]], 7, { caps: false });
    mb.color = C("#2a1e16");
    mb.tri([rad * 0.0, 0.0, rad * 1.0], [rad * 0.45, 0.0, rad * 0.85], [rad * 0.12, h * 0.4, rad * 0.62]);
    mb.color = C("#4a3826");
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * TAU + 0.4;
      stick(mb, [Math.cos(a) * rad * 0.9, -0.005, Math.sin(a) * rad * 0.9], [-Math.cos(a) * rad * 0.18, h * 1.18, -Math.sin(a) * rad * 0.18], 0.0018, 3);
    }
  } else {
    const L = 0.08, W = 0.05, H = 0.045;
    mb.color = hide;
    mb.quad([-L / 2, H, 0], [L / 2, H, 0], [L / 2, -0.004, -W / 2], [-L / 2, -0.004, -W / 2]);
    mb.quad([-L / 2, H, 0], [-L / 2, -0.004, W / 2], [L / 2, -0.004, W / 2], [L / 2, H, 0]);
    mb.color = mulc(hide, 0.75);
    mb.tri([-L / 2, -0.004, -W / 2], [-L / 2, -0.004, W / 2], [-L / 2, H, 0]);
    mb.color = C("#2a1e16");
    mb.tri([L / 2, -0.004, W / 2], [L / 2, -0.004, -W / 2], [L / 2, H, 0]);
    mb.color = C("#4a3826");
    stick(mb, [-L / 2 - 0.004, -0.004, 0], [-L / 2 - 0.004, H + 0.008, 0], 0.002, 3);
    stick(mb, [L / 2 + 0.004, -0.004, 0], [L / 2 + 0.004, H + 0.008, 0], 0.002, 3);
  }
}

recipe("imp.tent", { variants: 3, build: (mb, r, { variant }) => buildTent(mb, r, variant) });

/** A drying rack with a stretched hide, and a pair of antlers nailed to its post. */
function buildHideRack(mb, r) {
  mb.color = C("#4a3826");
  stick(mb, [-0.035, -0.02, 0], [-0.033, 0.075, 0], 0.0028, 4);
  stick(mb, [0.035, -0.02, 0], [0.034, 0.072, 0], 0.0028, 4);
  stick(mb, [-0.042, 0.07, 0], [0.042, 0.068, 0], 0.0022, 4);
  stick(mb, [-0.034, 0.012, 0], [0.034, 0.012, 0], 0.0018, 3);
  mb.color = J("#8a6040", r, 0.06);
  const m0 = mb.mark();
  mb.card([-0.028, 0.064, 0.002], [0.028, 0.064, 0.002], [0.024, 0.018, 0.002], [-0.026, 0.02, 0.002]);
  mb.card([-0.028, 0.064, 0.002], [-0.026, 0.02, 0.002], [-0.034, 0.04, 0.002], [-0.033, 0.06, 0.002]);
  mb.card([0.028, 0.064, 0.002], [0.033, 0.058, 0.002], [0.034, 0.03, 0.002], [0.024, 0.018, 0.002]);
  mb.tint(m0, (x, y) => 0.85 + 0.3 * Math.abs(Math.sin(x * 120)));
  mb.heightSway(m0, 0.06, 0.02, 0.25);
  // A second hide draped over a log.
  mb.color = C("#5a4430");
  stick(mb, [-0.03, 0.006, 0.05], [0.04, 0.006, 0.045], 0.006, 5, true);
  mb.color = J("#7a6a58", r, 0.06);
  mb.card([-0.01, 0.013, 0.04], [0.03, 0.013, 0.04], [0.032, -0.002, 0.058], [-0.012, -0.002, 0.058]);
}

recipe("imp.hiderack", { variants: 2, build: (mb, r) => buildHideRack(mb, r) });

/** A fire pit: a ring of stones round a few low flames (or embers) and a log to sit on. */
function buildFirePit(mb, r, variant) {
  const sh = rockShader(rockPaint("", r), 1);
  for (let k = 0; k < 7; k++) {
    const a = (k / 7) * TAU;
    rockMass(mb, r, { rad: 0.009, detail: 0, sy: 0.7, cuts: 2, cut: 0.55, c: [Math.cos(a) * 0.024, 0.002, Math.sin(a) * 0.024], shade: sh });
  }
  mb.color = C(T.char);
  at(mb, 0, 0.001, 0, 0, () => mb.lathe([[0.02, 0], [0, 0.003]], 7, { caps: false }));
  mb.mat = MAT.EMISSIVE;
  if (variant === 0) {
    for (let k = 0; k < 3; k++) {
      const a = (k / 3) * TAU;
      flameTongue(mb, r, Math.cos(a) * 0.006, Math.sin(a) * 0.006, k ? 0.022 : 0.034, 0.006, [0, 0], 1);
    }
  } else {
    mb.color = C(T.ember);
    for (let k = 0; k < 3; k++) {
      const a = r.range(0, TAU);
      stick(mb, [Math.cos(a) * 0.012, 0.004, Math.sin(a) * 0.012], [-Math.cos(a) * 0.008, 0.006, -Math.sin(a) * 0.008], 0.0035, 4, true);
    }
  }
  mb.mat = MAT.MATTE;
  mb.color = C("#5a4430");
  stick(mb, [-0.035, 0.006, 0.042], [0.03, 0.006, 0.05], 0.007, 5, true);
}

recipe("imp.firepit", { variants: 2, build: (mb, r, { variant }) => buildFirePit(mb, r, variant) });

// --- mine, quarry, lumber ---------------------------------------------------------------------

/** Ore colours by metal (resource markers and the mine's cart): [rock, vein, metallic]. */
const ORES = {
  // Dark host rock so the metal's colour is what one sees (it must name the metal at a glance).
  copper: ["#3e3a3a", "#d8783a", true],
  iron: ["#3a3638", "#b4552c", false],
  gold: ["#3c3836", "#e4a41c", true],
  silver: ["#3a3a42", "#e4e8f0", true],
  mithril: ["#363a44", "#a0e4f8", true],
  gems: ["#3c3840", "#c03a6a", false],
  coal: ["#3a3638", "#1a1818", false],
  "": ["#4e4a48", "#8a7a6a", false],
};

/** A barrel (or a keg) standing on y = 0: staves and two dark hoops. */
function barrel(mb, x, z, s = 1, col = "#6e4c30") {
  at(mb, x, 0, z, 0, () => {
    mb.scale(s);
    mb.color = C(col);
    mb.lathe([[0.012, -0.002], [0.0145, 0.014], [0.012, 0.03], [0, 0.03]], 6);
    mb.color = C("#2e2420");
    mb.lathe([[0.0143, 0.019], [0.0137, 0.023]], 6, { caps: false });
  });
}

/** A crate on y = 0 (planks with a darker frame). */
function crate(mb, x, z, s = 0.03, yaw = 0, col = "#8a6a46", y = 0) {
  at(mb, x, y, z, yaw, () => {
    block(mb, s, s, 0, s, col);
    mb.color = mulc(col, 0.6);
    mb.quad([-s / 2 - 0.0005, 0.002, s / 2 + 0.0005], [s / 2 + 0.0005, s - 0.002, s / 2 + 0.0005], [s / 2 + 0.0005, s, s / 2 + 0.0005], [-s / 2 - 0.0005, 0.004, s / 2 + 0.0005]);
  });
}

/** A mine: a timber-framed adit driven into a rock face, rails running out to an ore cart, a
 *  spoil heap and a lantern by the door. `param` is the ore ("gold", "iron", ...). */
function buildMine(mb, r, variant, param) {
  const ore = ORES[param] ?? ORES[""];
  const p = rockPaint(variant % 2 ? "moss" : "", r);
  const sh = rockShader(p, r.range(0, 9));
  const m0 = mb.mark();
  // The rock face: a big mass with a flat face toward the front, shoulders either side.
  rockMass(mb, r, { rad: 0.14, sx: 1.25, sy: 0.9, sz: 0.75, amp: 0.15, cuts: 3, cut: 0.7, c: [0, 0.035, -0.13], extra: [[[0, 0.15, 0.99], 0.55]], shade: sh });
  rockMass(mb, r, { rad: 0.065, detail: 0, sx: 1.1, sy: 0.8, cuts: 3, cut: 0.6, c: [-0.16, 0.0, -0.1], shade: sh });
  rockMass(mb, r, { rad: 0.065, detail: 0, sx: 1.1, sy: 0.9, cuts: 3, cut: 0.6, c: [0.15, 0.0, -0.08], shade: sh });
  groundAO(mb, m0, -0.02, 0.08, 0.55);
  const fz = -0.068;
  // The adit: a black mouth in a heavy timber frame, a short boarded entry with a plank roof.
  plate(mb, -0.04, -0.002, 0.04, 0.085, fz + 0.004, "#0a0807");
  mb.color = C("#6a4a32");
  for (const x of [-0.044, 0.044]) {
    stick(mb, [x, -0.01, fz + 0.012], [x * 0.9, 0.09, fz + 0.012], 0.007, 4);
    stick(mb, [x, -0.01, fz + 0.04], [x * 0.9, 0.085, fz + 0.04], 0.006, 4);
  }
  stick(mb, [-0.054, 0.091, fz + 0.012], [0.054, 0.091, fz + 0.012], 0.0075, 4);
  stick(mb, [-0.052, 0.086, fz + 0.04], [0.052, 0.086, fz + 0.04], 0.0065, 4);
  mb.color = C("#7a5a3c");
  mb.quad([-0.056, 0.098, fz + 0.0], [-0.056, 0.088, fz + 0.052], [0.056, 0.088, fz + 0.052], [0.056, 0.098, fz + 0.0]);
  // A timber headframe over a shaft beside the adit, its wheel against the sky.
  at(mb, -0.13, 0, -0.03, 0.3, () => {
    mb.color = C("#5a4030");
    const H = 0.2;
    for (const [x, z] of [[-0.03, -0.03], [0.03, -0.03], [0.03, 0.03], [-0.03, 0.03]]) stick(mb, [x, -0.02, z], [x * 0.35, H, z * 0.35], 0.0045, 3);
    for (const y of [0.06, 0.13]) {
      const k = 1 - (y / H) * 0.65;
      stick(mb, [-0.03 * k, y, 0.03 * k], [0.03 * k, y, 0.03 * k], 0.003, 3);
      stick(mb, [-0.03 * k, y, -0.03 * k], [-0.03 * k, y, 0.03 * k], 0.003, 3);
    }
    mb.color = C("#3a3634");
    const wheel = [];
    for (let i = 0; i <= 8; i++) wheel.push([Math.cos((i / 8) * TAU) * 0.026, H + 0.012 + Math.sin((i / 8) * TAU) * 0.026, 0]);
    mb.tube(wheel, 0.003, 3);
    stick(mb, [-0.022, H + 0.012, 0], [0.022, H + 0.012, 0], 0.002, 3);
    stick(mb, [0, H - 0.01, 0], [0, H + 0.034, 0], 0.002, 3);
    mb.color = C("#8a8478");
    stick(mb, [0.026, H + 0.012, 0], [0.026, 0.02, 0], 0.0012, 3);
  });
  // A lantern hung on the frame.
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#ffb050");
    at(mb, 0.042, 0.05, fz + 0.042, 0, () => mb.box(0.008, 0.011, 0.008));
  });
  // Rails out of the mouth, curving to the cart.
  const path = [];
  for (let i = 0; i <= 5; i++) {
    const t = i / 5;
    path.push([0.06 * t * t, 0.003, fz + 0.01 + 0.15 * t]);
  }
  mb.color = C("#5a4a3a");
  for (let i = 1; i < path.length; i += 1.4) { const k = Math.round(i); at(mb, path[k][0], -0.002, path[k][2], -Math.atan2(0.12 * (k / 5), 0.15), () => mb.box(0.034, 0.004, 0.006)); }
  mb.color = C("#3a3a3e");
  for (const s of [-1, 1]) mb.tube(path.map(([x, y, z]) => [x + s * 0.011, y + 0.002, z]), 0.0018, 3);
  // The ore cart, heaped with ore.
  const cp = path[5];
  at(mb, cp[0], 0, cp[2] - 0.01, -0.35, () => {
    mb.color = C("#2e2a28");
    for (const [x, z] of [[0.013, -0.012], [0.013, 0.012]]) at(mb, x * 1.15, 0.008, z, 0, () => { mb.rotateZ(Math.PI / 2); mb.cylinder(0.0075, 0.0075, 0.004, 6); });
    block(mb, 0.03, 0.036, 0.008, 0.03, "#5a4030", 1.15);
    mb.color = C("#3a3634");
    at(mb, 0, 0.026, 0, 0, () => mb.box(0.036, 0.004, 0.042));
    mb.mat = ore[2] ? MAT.METAL : MAT.MATTE;
    const ps = { base: C(ore[0]), top: C(ore[1]), coat: [ore[1]], cover: 0.6 };
    for (let k = 0; k < 3; k++) rockMass(mb, r, { rad: 0.011, detail: 0, sy: 0.8, cuts: 2, cut: 0.55, c: [(k - 1) * 0.009, 0.033, r.range(-0.006, 0.006)], floor: 0.02, shade: rockShader(ps, k) });
    mb.mat = MAT.MATTE;
  });
  // The spoil heap.
  at(mb, -0.1, 0, 0.06, 0, () => {
    mb.color = C("#5e5650");
    const s0 = mb.mark();
    mb.lathe([[0.07, -0.02], [0.055, 0.012], [0.028, 0.034], [0.006, 0.042], [0, 0.043]], 8, { wobble: wobbler(r, 0.12) });
    mb.tint(s0, (x, y, z) => 0.75 + 0.35 * clamp(y / 0.04, 0, 1) * (0.8 + 0.4 * hashf(Math.round(x * 300), Math.round(z * 300))));
  });
}

recipe("imp.mine", { variants: 4, build: (mb, r, { variant, param }) => buildMine(mb, r, variant, param) });

/** A quarry: pale stone cut back into the hillside in square terraces, cut blocks stacked on the
 *  floor and a timber crane lifting one. */
function buildQuarry(mb, r, variant) {
  const lime = J(variant % 2 ? "#948a76" : "#8a7e6a", r, 0.04);
  const cutShade = (n, y) => faceShade(n[1] > 0.6 ? mixl(lime, "#b4aa94", 0.6) : mulc(lime, 0.85), n, { lit: 0.3 });
  const p = rockPaint("moss", r);
  const sh = rockShader(p, 4);
  // Rough rock framing the cut.
  rockMass(mb, r, { rad: 0.12, sx: 1.6, sy: 0.9, sz: 0.7, amp: 0.15, cuts: 3, cut: 0.65, c: [0, 0.05, -0.2], shade: sh });
  rockMass(mb, r, { rad: 0.08, detail: 0, sx: 1.0, sy: 1.0, cuts: 3, cut: 0.6, c: [-0.17, 0.01, -0.1], shade: sh });
  rockMass(mb, r, { rad: 0.07, detail: 0, sx: 1.0, sy: 1.0, cuts: 3, cut: 0.6, c: [0.17, 0.01, -0.12], shade: sh });
  // The benches: square-cut steps down toward the front.
  const benches = [[0.22, 0.11, -0.16], [0.2, 0.075, -0.11], [0.17, 0.04, -0.06]];
  for (const [w, h, z] of benches) {
    const poly = [[-w / 2, z - 0.03], [w / 2, z - 0.03], [w / 2, z + 0.03], [-w / 2, z + 0.03]].map(([x, zz]) => [x + r.range(-0.004, 0.004), zz]);
    slab(mb, poly, -0.04, h, { top: 1, shade: cutShade });
  }
  // Cut blocks on the floor, some stacked.
  for (let k = 0; k < 4; k++) {
    const x = -0.09 + k * 0.05 + r.range(-0.01, 0.01), z = r.range(0.0, 0.05);
    at(mb, x, 0, z, r.range(-0.3, 0.3), () => slab(mb, [[-0.016, -0.012], [0.016, -0.012], [0.016, 0.012], [-0.016, 0.012]], -0.01, 0.02, { top: 1, shade: cutShade }));
    if (k === 1) at(mb, x, 0.02, z, 0.2, () => slab(mb, [[-0.015, -0.011], [0.015, -0.011], [0.015, 0.011], [-0.015, 0.011]], 0, 0.022, { top: 1, shade: cutShade }));
  }
  // A crane: an A-frame, a jib and a hanging block.
  const bx = 0.1, bz = 0.06;
  mb.color = C("#4a3626");
  stick(mb, [bx - 0.025, -0.01, bz + 0.015], [bx, 0.13, bz], 0.003, 4);
  stick(mb, [bx + 0.025, -0.01, bz + 0.015], [bx, 0.13, bz], 0.003, 4);
  stick(mb, [bx, -0.01, bz - 0.03], [bx, 0.13, bz], 0.003, 4);
  stick(mb, [bx + 0.012, 0.115, bz], [bx - 0.07, 0.15, bz - 0.06], 0.0025, 4);
  mb.color = C("#9a8460");
  stick(mb, [bx - 0.068, 0.148, bz - 0.06], [bx - 0.068, 0.08, bz - 0.06], 0.0009, 3);
  at(mb, bx - 0.068, 0.062, bz - 0.06, 0.3, () => slab(mb, [[-0.012, -0.01], [0.012, -0.01], [0.012, 0.01], [-0.012, 0.01]], 0, 0.018, { top: 1, shade: cutShade }));
}

recipe("imp.quarry", { variants: 2, build: (mb, r, { variant }) => buildQuarry(mb, r, variant) });

/** A sawmill: a timber shed, an open saw-pit with a log on trestles under a lean-to, planks. */
function buildSawmill(mb, r, variant, param) {
  const F = famOf(param);
  at(mb, -0.05, 0, -0.02, 0, () => cottage(mb, r, { ...F, wall: "#9a7a56", wall2: "#7a5e42" }, 0.11, 0.085, 0.07, { roof: F.key === "human" ? "#57504a" : F.roof, posts: true }));
  // Lean-to over the saw-pit.
  at(mb, 0.07, 0, 0.0, 0, () => {
    mb.color = C(F.timber);
    for (const [x, z, h] of [[-0.035, -0.04, 0.075], [0.035, -0.04, 0.072], [-0.035, 0.04, 0.055], [0.035, 0.04, 0.054]]) stick(mb, [x, -0.02, z], [x, h, z], 0.0035, 4);
    mb.color = C("#5e4c3a");
    mb.card([-0.045, 0.077, -0.05], [0.045, 0.074, -0.05], [0.045, 0.054, 0.05], [-0.045, 0.056, 0.05]);
    // The log on trestles, with the long saw.
    mb.color = C("#4a3424");
    for (const x of [-0.025, 0.025]) {
      stick(mb, [x, -0.004, -0.01], [x, 0.03, 0], 0.002, 3);
      stick(mb, [x, -0.004, 0.01], [x, 0.03, 0], 0.002, 3);
    }
    mb.color = C("#6a4e34");
    stick(mb, [-0.05, 0.036, 0], [0.05, 0.036, 0], 0.009, 6, true);
    mb.color = C("#c8a878");
    for (const x of [-0.0505, 0.0505]) at(mb, x, 0.036, 0, 0, () => { mb.rotateZ(Math.PI / 2); mb.cylinder(0.0085, 0.0085, 0.001, 6); });
    mb.color = C("#8a8e94");
    mb.mat = MAT.METAL;
    mb.quad([0.0, 0.02, 0.0], [0.0, 0.07, 0.0], [0.002, 0.07, 0.0], [0.002, 0.02, 0.0]);
    mb.mat = MAT.MATTE;
  });
  // A stack of sawn planks.
  mb.color = C("#a07a50");
  for (let k = 0; k < 3; k++) at(mb, 0.04 + k * 0.002, 0.004 + k * 0.006, 0.08, 0.1, () => mb.box(0.08, 0.005, 0.03));
}

recipe("imp.sawmill", { variants: 2, build: (mb, r, { variant, param }) => buildSawmill(mb, r, variant, param) });

/** A log pile: logs stacked in a pyramid, their pale cut ends toward the viewer. */
function buildLogPile(mb, r, variant) {
  const rows = variant % 2 ? [3, 2, 1] : [4, 3, 2];
  const lr = 0.011, L = 0.1;
  rows.forEach((n, j) => {
    for (let k = 0; k < n; k++) {
      const x = (k - (n - 1) / 2) * lr * 2.05, y = lr + j * lr * 1.75;
      const dz = r.range(-0.006, 0.006);
      at(mb, x, y, dz, 0, () => {
        mb.color = J("#5a4230", r, 0.08);
        mb.part(() => {
          mb.rotateX(Math.PI / 2);
          mb.translate(0, -L / 2, 0);
          mb.cylinder(lr, lr * 0.95, L, 5, { caps: false });
        });
        mb.color = J("#c8a070", r, 0.06);
        mb.part(() => {
          mb.translate(0, 0, L / 2);
          mb.rotateX(Math.PI / 2);
          mb.lathe([[lr * 0.95, 0], [0, 0]], 5, { caps: false });
        });
      });
    }
  });
  // Stumps nearby.
  for (let k = 0; k < 2; k++) {
    const a = r.range(0, TAU), d = r.range(0.08, 0.11);
    at(mb, Math.cos(a) * d, 0, Math.sin(a) * d, 0, () => {
      mb.color = C("#4e3a2a");
      mb.lathe([[0.017, -0.02], [0.016, 0.0], [0.012, 0.016]], 6, { caps: false });
      mb.color = C("#b89468");
      mb.lathe([[0.012, 0.016], [0, 0.017]], 6, { caps: false });
    });
  }
}

recipe("imp.logpile", { variants: 2, build: (mb, r, { variant }) => buildLogPile(mb, r, variant) });

/** A windmill: a tapering tower, a turning cap and four lattice sails. */
function buildWindmill(mb, r, variant, param) {
  const F = famOf(param);
  const wall = J(variant ? "#8a7a68" : F.wall, r, 0.04);
  mb.color = C(F.stone);
  mb.lathe([[0.058, -0.05], [0.058, 0.01], [0, 0.01]], 8);
  mb.color = wall;
  const m0 = mb.mark();
  mb.lathe([[0.052, 0.008], [0.042, 0.12], [0.036, 0.165]], 8, { caps: false });
  mb.tint(m0, (x, y) => 0.85 + 0.2 * clamp(y / 0.16, 0, 1));
  plate(mb, -0.01, 0.01, 0.01, 0.042, 0.0515, F.door);
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    plate(mb, -0.005, 0.085, 0.005, 0.097, 0.0435, "#e8a050");
  });
  mb.color = J(F.key === "human" ? "#5a4a3a" : F.roof, r, 0.05);
  const c0 = mb.mark();
  mb.lathe([[0.044, 0.162], [0.04, 0.175], [0.024, 0.2], [0, 0.212]], 8);
  mb.tint(c0, (x, y) => 0.7 + 0.4 * clamp((y - 0.16) / 0.05, 0, 1));
  // Sails on a hub facing the front.
  const hub = [0, 0.175, 0.048];
  mb.color = C("#4a3424");
  stick(mb, [0, 0.175, 0.03], hub, 0.005, 5, true);
  const tw = r.range(0, 0.6);
  for (let k = 0; k < 4; k++) {
    const a = tw + (k / 4) * TAU;
    const dir = [Math.cos(a), Math.sin(a), 0], side = [-Math.sin(a), Math.cos(a), 0];
    const P = (d, s) => vadd(hub, vadd(vscale(dir, d), vscale(side, s)));
    mb.color = C("#4a3424");
    stick(mb, hub, P(0.13, 0), 0.0025, 3);
    mb.color = J("#d8ccb0", r, 0.04);
    mb.card(P(0.03, 0.003), P(0.125, 0.003), P(0.125, 0.026), P(0.03, 0.024));
    mb.color = C("#5a4430");
    for (const d of [0.06, 0.095]) stick(mb, P(d, 0.002), P(d, 0.025), 0.001, 3);
  }
  groundAO(mb, 0, -0.01, 0.05, 0.6);
}

recipe("imp.windmill", { variants: 2, build: (mb, r, { variant, param }) => buildWindmill(mb, r, variant, param) });

/** A workshop: a timber house with a big stone chimney, the forge glowing at its foot, an anvil,
 *  barrels and timber outside. */
function buildWorkshop(mb, r, variant, param) {
  const F = famOf(param);
  cottage(mb, r, F, 0.12, 0.085, 0.072, { roof: F.key === "human" ? "#5e5048" : F.roof, roofH: 0.075 });
  at(mb, 0.07, 0, -0.01, 0, () => {
    block(mb, 0.026, 0.03, -0.04, 0.17, J(F.stone, r, 0.04), 0.85);
    mb.color = C("#2a2422");
    at(mb, 0, 0.17, 0, 0, () => mb.box(0.02, 0.006, 0.024));
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      plate(mb, -0.008, 0.006, 0.008, 0.024, 0.0155, "#ff7a2a");
    });
  });
  // Anvil on a stump, barrels, a stack of bars.
  at(mb, 0.035, 0, 0.07, 0.3, () => {
    mb.color = C("#4e3a2a");
    mb.cylinder(0.009, 0.009, 0.014, 6);
    mb.color = C("#3a3a40");
    mb.mat = MAT.METAL;
    at(mb, 0, 0.014, 0, 0, () => mb.box(0.018, 0.008, 0.008, { top: 0.8 }));
    mb.mat = MAT.MATTE;
  });
  barrel(mb, -0.075, 0.06);
  barrel(mb, -0.05, 0.075, 0.85);
  crate(mb, -0.09, 0.03, 0.022, 0.4);
}

recipe("imp.workshop", { variants: 2, build: (mb, r, { variant, param }) => buildWorkshop(mb, r, variant, param) });

// --- vineyard and plantation ------------------------------------------------------------------

/** Rows of vines on stakes and wires: leafy masses heavy with dark grapes. */
function buildVines(mb, r, variant) {
  const rows = 3, posts = 4, len = 0.26, gap = 0.08;
  for (let j = 0; j < rows; j++) {
    const z = (j - (rows - 1) / 2) * gap;
    mb.color = C("#4a3a2a");
    const P = [];
    for (let k = 0; k < posts; k++) {
      const x = -len / 2 + (len * k) / (posts - 1);
      P.push([x, z]);
      stick(mb, [x, -0.02, z], [x, 0.05, z], 0.0022, 3);
    }
    void P;
    mb.color = C("#6a6a66");
    stick(mb, [-len / 2, 0.045, z], [len / 2, 0.045, z], 0.0008, 3);
    void 0;
    mb.mat = MAT.FOLIAGE;
    const m0 = mb.mark();
    for (let k = 0; k < posts - 1; k++) {
      const x = (P[k][0] + P[k + 1][0]) / 2;
      mb.color = J(r() < 0.5 ? "#4a6a2e" : "#3a5626", r, 0.06);
      const seed = r.range(0, 9);
      at(mb, x, 0.038, z, 0, () => mb.blob(0.026, { detail: 0, amp: 0.25, squash: 0.75, noise: (v) => noise3(v[0] * 3 + seed, v[1] * 3, v[2] * 3) }));
      mb.mat = MAT.MATTE;
      mb.color = C(variant ? "#c8b050" : "#3a1e3a");
      at(mb, x + r.range(-0.01, 0.01), 0.01, z + 0.016, 0, () => mb.lathe([[0, 0], [0.0065, 0.014]], 4));
      mb.mat = MAT.FOLIAGE;
    }
    mb.sphereNormals(m0, [0, 0.03, z], 0.5);
    mb.mat = MAT.MATTE;
  }
}

recipe("imp.vines", { variants: 2, build: (mb, r, { variant }) => buildVines(mb, r, variant) });

/** A wine press house: a small stone house, barrels racked outside under an awning. */
function buildPressHouse(mb, r, variant, param) {
  const F = famOf(param);
  cottage(mb, r, { ...F, wall: F.key === "human" ? "#b0a48a" : F.wall }, 0.1, 0.08, 0.065, { roof: F.key === "human" ? "#7a4030" : F.roof, roofH: 0.065 });
  for (let k = 0; k < 1; k++) {
    at(mb, 0.0, 0.013, 0.075, 0, () => {
      mb.rotateX(Math.PI / 2);
      mb.translate(0, -0.015, 0);
      barrel(mb, 0, 0, 0.9, "#5a3a26");
    });
  }
}

recipe("imp.press", { variants: 2, build: (mb, r, { variant, param }) => buildPressHouse(mb, r, variant, param) });

/** Rows of a plantation's crop (param): broad-leaved bananas, sugar cane, cotton or dye bushes,
 *  or low tea-like hedges. */
function buildCropRows(mb, r, variant, crop) {
  const rows = 3, n = 4, len = 0.26, gap = 0.085;
  mb.mat = MAT.FOLIAGE;
  for (let j = 0; j < rows; j++) {
    const z = (j - 1) * gap;
    for (let k = 0; k < n; k++) {
      const x = -len / 2 + (len * k) / (n - 1) + r.range(-0.008, 0.008);
      const m0 = mb.mark();
      if (crop === "banana") {
        mb.color = C("#6a6a3a");
        stick(mb, [x, -0.01, z], [x, 0.05, z], 0.004, 4);
        for (let l = 0; l < 4; l++) {
          const a = (l / 4) * TAU + r.range(-0.3, 0.3);
          const tip = [x + Math.cos(a) * 0.04, 0.035, z + Math.sin(a) * 0.04];
          const side = [-Math.sin(a) * 0.01, 0.0, Math.cos(a) * 0.01];
          mb.color = J(l % 2 ? "#5a7a2e" : "#6e8a34", r, 0.06);
          mb.quad([x, 0.05, z], vadd([x + Math.cos(a) * 0.02, 0.058, z + Math.sin(a) * 0.02], side), tip, vsub([x + Math.cos(a) * 0.02, 0.058, z + Math.sin(a) * 0.02], side));
        }
        mb.heightSway(m0, 0.03, 0.06, 0.5);
      } else if (crop === "sugar") {
        mb.mat = MAT.BLADE;
        for (let l = 0; l < 4; l++) {
          const dx = r.range(-0.012, 0.012), dz = r.range(-0.008, 0.008);
          mb.color = J("#8a9a4a", r, 0.08);
          stick(mb, [x + dx, -0.01, z + dz], [x + dx * 1.6, 0.07, z + dz * 1.6], 0.0022, 3);
          mb.color = J("#5e7a30", r, 0.08);
          mb.tri([x + dx * 1.6, 0.07, z + dz * 1.6], [x + dx * 1.6 + 0.02, 0.05, z + dz], [x + dx * 1.6 + 0.004, 0.055, z + dz + 0.004]);
        }
        mb.heightSway(m0, 0.02, 0.07, 0.5);
        mb.mat = MAT.FOLIAGE;
      } else {
        const seed = r.range(0, 9);
        mb.color = J(crop === "tea" ? "#3a5a2a" : "#4a6230", r, 0.06);
        at(mb, x, 0.012, z, 0, () => mb.blob(0.022, { detail: 0, amp: 0.25, squash: 0.8, noise: (v) => noise3(v[0] * 3 + seed, v[1] * 3, v[2] * 3) }));
        mb.sphereNormals(m0, [x, 0.0, z], 0.6);
        if (crop === "cotton" || crop === "dye") {
          mb.mat = MAT.MATTE;
          mb.color = C(crop === "cotton" ? "#ece6da" : variant % 2 ? "#3a3a8a" : "#9a2a2a");
          for (let l = 0; l < 3; l++) at(mb, x + r.range(-0.014, 0.014), 0.026 + r.range(0, 0.008), z + r.range(-0.012, 0.014), 0, () => mb.sphere(0.005, 0));
          mb.mat = MAT.FOLIAGE;
        }
      }
    }
  }
  mb.mat = MAT.MATTE;
}

recipe("imp.croprows", { variants: 2, build: (mb, r, { variant, param }) => buildCropRows(mb, r, variant, param) });

/** An open-sided drying shed on four posts. */
function buildShed(mb, r, variant, param) {
  const F = famOf(param);
  mb.color = C(F.timber);
  for (const [x, z] of [[-0.045, -0.035], [0.045, -0.035], [-0.045, 0.035], [0.045, 0.035]]) stick(mb, [x, -0.02, z], [x, 0.055, z], 0.0035, 4);
  at(mb, 0, 0.055, 0, 0, () => roofGable(mb, 0.09, 0.07, 0.04, { col: J(F.key === "desert" ? "#a08a5a" : "#7a6440", r, 0.05), ends: null, over: 0.012 }));
  // Bundles hung to dry.
  mb.color = C(variant ? "#8a6a3a" : "#6a7a3a");
  for (let k = 0; k < 4; k++) stick(mb, [-0.03 + k * 0.02, 0.052, 0], [-0.03 + k * 0.02, 0.03, 0.002], 0.004, 4);
}

recipe("imp.shed", { variants: 2, build: (mb, r, { variant, param }) => buildShed(mb, r, variant, param) });

// --- fields: stooks, fences ----------------------------------------------------------------------

/** A stook: sheaves of grain stood together to dry, ears up and tied at the waist (0-1 a ring of
 *  sheaves, 2 a ridge of them leaning together in pairs). */
function buildStook(mb, r, variant) {
  // Pale, soft straw (the look target's stooks are the palest gold on the field), fat sheaves
  // flaring into full heads of ears above the tie, the tie a darker band.
  const straw = J("#dcbc66", r, 0.05);
  const sheaf = (x, z, lx, lz) => {
    // From the spread butt on the ground, in to the tie, out to the full head of ears; leaning
    // by (lx, lz).
    const P = [[x - lx * 0.12, -0.012, z - lz * 0.12], [x + lx * 0.3, 0.02, z + lz * 0.3], [x + lx * 0.55, 0.034, z + lz * 0.55], [x + lx * 0.85, 0.054, z + lz * 0.85], [x + lx, 0.07, z + lz]];
    mb.color = straw;
    const m0 = mb.mark();
    mb.tube(P, [0.017, 0.012, 0.0105, 0.018, 0.006], 4);
    mb.tint(m0, (px, y) => (Math.abs(y - 0.034) < 0.004 ? 0.66 : 0.78 + 0.36 * clamp(y / 0.066, 0, 1)));
  };
  if (variant < 2) {
    const n = 5 + variant;
    const a0 = r.range(0, TAU);
    for (let k = 0; k < n; k++) {
      const a = a0 + (k / n) * TAU + r.range(-0.2, 0.2);
      const c = Math.cos(a), s = Math.sin(a);
      sheaf(c * 0.022, s * 0.022, -c * 0.013, -s * 0.013);
    }
  } else {
    for (let k = 0; k < 3; k++) {
      const z = (k - 1) * 0.024 + r.range(-0.003, 0.003);
      for (const s of [-1, 1]) sheaf(s * 0.02, z, -s * 0.013, r.range(-0.003, 0.003));
    }
  }
  groundAO(mb, 0, -0.01, 0.025, 0.72);
}

recipe("imp.stook", { variants: 3, build: (mb, r, { variant }) => buildStook(mb, r, variant) });

/** A run of field fence along x (0.2 long, centred): three weathered posts and two rails between
 *  them, a little out of true. Runs are laid end to end, each at its own ground height. */
function buildFenceRun(mb, r, variant) {
  const col = J(variant === 3 ? "#6e5a44" : "#8c7556", r, 0.05);
  const h = 0.042;
  mb.color = col;
  const posts = [-0.1, 0, 0.1].map((x) => [x + r.range(-0.006, 0.006), r.range(-0.003, 0.003)]);
  for (const [x, z] of posts) stick(mb, [x, -0.03, z], [x + r.range(-0.004, 0.004), h + r.range(0, 0.008), z + r.range(-0.003, 0.003)], 0.0052, 3);
  mb.color = mulc(col, 1.12);
  for (const y of [h * 0.42, h * 0.84]) {
    for (let k = 0; k < 2; k++) {
      const [ax, az] = posts[k], [bx, bz] = posts[k + 1];
      stick(mb, [ax, y + r.range(-0.002, 0.002), az + 0.005], [bx, y + r.range(-0.002, 0.002), bz + 0.005], 0.0034, 3);
    }
  }
}

recipe("imp.fence", { variants: 4, build: (mb, r, { variant }) => buildFenceRun(mb, r, variant) });

/** A quad with vertex normals, wound to face `n` (props are lit on both sides, so the winding must
 *  match the normals). */
function qnN(mb, a, b, c, d, na, nb, nc, nd, n) {
  const f = vcross(vsub(b, a), vsub(c, a));
  if (f[0] * n[0] + f[1] * n[1] + f[2] * n[2] < 0) mb.quadN(a, d, c, b, na, nd, nc, nb);
  else mb.quadN(a, b, c, d, na, nb, nc, nd);
}

/** A crop row along z centred on x = 0: a roll of triangular section (half-width hw, feet sunk to
 *  y = -0.012) whose crest runs through `crest` ([y, x-offset] at even stations over the length L),
 *  with soft normals so it reads round; `tone(k, side)` colours each segment. Low: two faces a
 *  segment, and two end caps. */
function cropRow(mb, L, hw, crest, tone) {
  const n = crest.length - 1, foot = -0.012;
  const z = (k) => -L / 2 + (L * k) / n;
  for (let k = 0; k < n; k++) {
    const [ya, xa] = crest[k], [yb, xb] = crest[k + 1];
    for (const s of [-1, 1]) {
      const nf = vnorm([s * 0.75, 0.66, 0]), nc = vnorm([s * 0.25, 1, 0]);
      mb.color = tone(k, s);
      qnN(mb, [s * hw, foot, z(k)], [xa, ya, z(k)], [xb, yb, z(k + 1)], [s * hw, foot, z(k + 1)], nf, nc, nc, nf, nf);
    }
  }
  for (const [k, s] of [[0, -1], [n, 1]]) {
    mb.color = mulc(tone(Math.min(k, n - 1), 1), 0.8);
    const [y, xo] = crest[k];
    const a = [-hw, foot, z(k)], b = [hw, foot, z(k)], c = [xo, y, z(k)];
    const nn = [0, 0.3, s];
    const f = vcross(vsub(b, a), vsub(c, a));
    if (f[2] * s < 0) mb.triN(a, c, b, nn, nn, nn);
    else mb.triN(a, b, c, nn, nn, nn);
  }
}

/** Vegetable beds: rows of leafy crops (variant + 1 of them, 1/14 plot apart -- the rows the field
 *  decal paints, fields.js ROWS) along x, 0.2 long, lumpy and mottled light and dark. Laid in
 *  patches over a field, each at its own ground height; sunk, so a slope buries an end rather than
 *  lifting one. */
function buildBeds(mb, r, variant) {
  const rows = variant + 1, gap = 1 / 14, L = 0.2, n = 5;
  const greens = ["#46682a", "#527430", "#5c7e36", "#3e6028"].map((c) => J(c, r, 0.05));
  mb.mat = MAT.FOLIAGE;
  for (let j = 0; j < rows; j++) {
    const crest = [];
    for (let k = 0; k <= n; k++) crest.push([k === 0 || k === n ? 0.006 : r.range(0.014, 0.022), r.range(-0.004, 0.004)]);
    const tones = crest.map(() => r.pick(greens));
    at(mb, 0, 0, (j - (rows - 1) / 2) * gap, Math.PI / 2, () => cropRow(mb, L, 0.026, crest, (k, s) => (s < 0 ? tones[k] : mulc(tones[k], 0.88))));
  }
  mb.mat = MAT.MATTE;
}

recipe("imp.beds", { variants: 3, build: (mb, r, { variant }) => buildBeds(mb, r, variant) });

/** A row of standing grain along z centred on x = 0: a soft, rounded sheaf of section foot (half
 *  width hw, sunk to y = -0.012) -- shoulder -- crown (a flattish top `hs` either side of the
 *  crest), the crest through `crest` ([y, x-offset] at even stations over the length L), with
 *  normals rounding over it; `tone(k)` colours each station (the ears combed light and dark along
 *  the row). Three faces a segment and low end caps. */
function grainRow(mb, L, hw, crest, tone) {
  const n = crest.length - 1, foot = -0.012, hs = hw * 0.42;
  const z = (k) => -L / 2 + (L * k) / n;
  // The section's points (x, y) and normals at station k: foot, shoulder, shoulder, foot.
  const sec = (k) => {
    const [y, xo] = crest[k];
    return [[-hw, foot], [xo - hs, y], [xo + hs, y], [hw, foot]];
  };
  const NS = [vnorm([-0.8, 0.6, 0]), vnorm([-0.3, 1, 0]), vnorm([0.3, 1, 0]), vnorm([0.8, 0.6, 0])];
  for (let k = 0; k < n; k++) {
    const a = sec(k), b = sec(k + 1), ka = tone(k), kb = tone(k + 1);
    for (let f = 0; f < 3; f++) {
      const p = [[a[f][0], a[f][1], z(k)], [a[f + 1][0], a[f + 1][1], z(k)], [b[f + 1][0], b[f + 1][1], z(k + 1)], [b[f][0], b[f][1], z(k + 1)]];
      const nf = [NS[f], NS[f + 1], NS[f + 1], NS[f]];
      const ks = [ka, ka, kb, kb];
      // Wound to face up and out (the shader flips a back face's normal).
      const fn = vcross(vsub(p[1], p[0]), vsub(p[2], p[0]));
      const order = fn[1] < 0 ? [0, 3, 2, 1] : [0, 1, 2, 3];
      const tri = (i, j, l) => {
        mb.vert(p[order[i]], nf[order[i]], ks[order[i]]);
        mb.vert(p[order[j]], nf[order[j]], ks[order[j]]);
        mb.vert(p[order[l]], nf[order[l]], ks[order[l]]);
      };
      tri(0, 1, 2);
      tri(0, 2, 3);
    }
  }
  for (const [k, s] of [[0, -1], [n, 1]]) {
    mb.color = mulc(tone(k), 0.82);
    const q = sec(k).map(([x, y]) => [x, y, z(k)]);
    const nn = [0, 0.4, s];
    for (const [i, j, l] of [[0, 1, 2], [0, 2, 3]]) {
      const f = vcross(vsub(q[j], q[i]), vsub(q[l], q[i]));
      if (f[2] * s < 0) mb.triN(q[i], q[l], q[j], nn, nn, nn);
      else mb.triN(q[i], q[j], q[l], nn, nn, nn);
    }
  }
}

/** Standing grain: rows of ripe ears (variant + 1 of them, 1/18 plot apart -- the rows the field
 *  decal paints) along z, 0.2 long: soft, rounded golden sheaves with a shadowed furrow between
 *  them (as the look target's wheat: combed rows, not one flat mat), pale on the crown, ochre
 *  down the stalks, each row's ears combed light and dark along it. Laid in patches like the
 *  beds (their ends overlapping, so a field reads as one run of rows). */
function buildGrain(mb, r, variant) {
  const rows = variant + 1, gap = 1 / 18, L = 0.2, n = 3;
  const gold = J("#b6803a", r, 0.05);
  for (let j = 0; j < rows; j++) {
    const crest = [], tones = [];
    for (let k = 0; k <= n; k++) {
      crest.push([0.025 + r.range(-0.002, 0.002), r.range(-0.0025, 0.0025)]);
      tones.push(mulc(gold, r.range(0.9, 1.08)));
    }
    const m0 = mb.mark();
    at(mb, (j - (rows - 1) / 2) * gap, 0, 0, 0, () => grainRow(mb, L, gap * 0.6, crest, (k) => tones[k]));
    mb.tint(m0, (px, y) => 0.46 + 0.56 * clamp((y + 0.008) / 0.033, 0, 1));
  }
}

recipe("imp.grain", { variants: 4, build: (mb, r, { variant }) => buildGrain(mb, r, variant) });

// --- bridges -----------------------------------------------------------------------------------

/** A quad wound to face `n` (props are lit on both sides, so the winding must match the normal). */
function qn(mb, a, b, c, d, n) {
  const f = vcross(vsub(b, a), vsub(c, a));
  if (f[0] * n[0] + f[1] * n[1] + f[2] * n[2] < 0) mb.quad(a, d, c, b);
  else mb.quad(a, b, c, d);
}

/** Pale dressed stone of the bridges (the target's bridges are the brightest stone on the map). */
const BRIDGE_STONE = { wall: "#887b68", ring: "#cabea8", deck: "#b8aa92", cap: "#d4cab6", shade: "#4a423c" };

/** Square posts with a cap on the four corners of a bridge's parapets (x = +-x, z = +-zEnd). */
function bridgePosts(mb, r, x, zEnd, y, h, s) {
  for (const sx of [-1, 1]) {
    for (const sz of [-1, 1]) {
      at(mb, sx * x, y - 0.03, sz * zEnd, 0, () => {
        mb.color = J(BRIDGE_STONE.wall, r, 0.04);
        mb.box(s, h + 0.03, s);
        mb.color = J(BRIDGE_STONE.cap, r, 0.03);
        at(mb, 0, h + 0.03, 0, 0, () => mb.box(s * 1.3, 0.008, s * 1.3, { top: 0.75 }));
      });
    }
  }
}

/** Abutments: a broad stone footing at each end, sunk into the bank under the deck's end. */
function bridgeFootings(mb, r, X, zEnd) {
  for (const sz of [-1, 1]) {
    at(mb, 0, -0.14, sz * (zEnd - 0.03), 0, () => {
      mb.color = J(BRIDGE_STONE.wall, r, 0.05);
      mb.box(2 * X + 0.05, 0.14, 0.13, { top: 0.92 });
    });
  }
}

/** An arched stone bridge along z: a humped deck between parapets with a coping over one arch (0)
 *  or two arches on a pier with cutwaters (1), each arch ringed with lighter voussoirs and dark
 *  inside, square end posts, footings down into both banks. The deck's ends (y = 0) meet the
 *  banks: it spans 0.68 and its arches 0.44, over a river whose water is at most 0.44 wide and
 *  whose bank tops are 0.3 from its line (world.js RIVER_BANK), where the footings stand. */
function buildArchBridge(mb, r, variant) {
  // A humpback: the deck rises well over the banks, so the arches stand clear of the water (a
  // river's surface is only a few hundredths under its banks).
  const Lh = 0.34, W = 0.1, pt = 0.015, ph = 0.026, rt = 0.017;
  const X = W / 2 + pt, deep = -0.14, ys = -0.075;
  const top = (z) => 0.004 + 0.064 * Math.pow(Math.max(0, Math.cos((Math.PI * z) / (2 * Lh))), 1.3);
  const arches = (variant ? [[-0.115, 0.11], [0.115, 0.11]] : [[0, 0.22]]).map(([c, a]) => ({ c, a, yc: top(c) - 0.032 }));
  const archAt = (za, zb) => arches.find((q) => za >= q.c - q.a - 1e-6 && zb <= q.c + q.a + 1e-6);
  const curve = (q, z) => ys + (q.yc - ys) * Math.sqrt(Math.max(0, 1 - ((z - q.c) / q.a) ** 2));
  // Stations along z: the ends, the solid stretches' middles, dense over each arch.
  const zs = [-Lh, Lh];
  const N = arches.length > 1 ? 7 : 10;
  for (const q of arches) for (let k = 0; k <= N; k++) zs.push(q.c - q.a * Math.cos((Math.PI * k) / N));
  zs.push((-Lh + arches[0].c - arches[0].a) / 2, (Lh + arches[arches.length - 1].c + arches[arches.length - 1].a) / 2);
  zs.sort((a, b) => a - b);
  const wall = J(BRIDGE_STONE.wall, r, 0.04);
  for (let k = 0; k + 1 < zs.length; k++) {
    const za = zs[k], zb = zs[k + 1];
    if (zb - za < 1e-6) continue;
    const ta = top(za), tb = top(zb), q = archAt(za, zb);
    // Deck, parapets (inner faces and coping), both outer faces.
    mb.color = J(BRIDGE_STONE.deck, r, 0.05);
    qn(mb, [-W / 2, ta, za], [W / 2, ta, za], [W / 2, tb, zb], [-W / 2, tb, zb], [0, 1, 0]);
    for (const s of [-1, 1]) {
      mb.color = J(wall, r, 0.05);
      qn(mb, [(s * W) / 2, ta, za], [(s * W) / 2, ta + ph, za], [(s * W) / 2, tb + ph, zb], [(s * W) / 2, tb, zb], [-s, 0, 0]);
      mb.color = J(BRIDGE_STONE.cap, r, 0.04);
      qn(mb, [s * (W / 2 - 0.002), ta + ph, za], [s * (X + 0.003), ta + ph, za], [s * (X + 0.003), tb + ph, zb], [s * (W / 2 - 0.002), tb + ph, zb], [0, 1, 0]);
      mb.color = mulc(BRIDGE_STONE.cap, 0.8);
      qn(mb, [s * (X + 0.003), ta + ph - 0.006, za], [s * (X + 0.003), ta + ph, za], [s * (X + 0.003), tb + ph, zb], [s * (X + 0.003), tb + ph - 0.006, zb], [s, 0, 0]);
      const x = s * X;
      if (q) {
        const aa = curve(q, za), ab = curve(q, zb);
        mb.color = J(wall, r, 0.06);
        qn(mb, [x, aa + rt, za], [x, ta + ph - 0.006, za], [x, tb + ph - 0.006, zb], [x, ab + rt, zb], [s, 0, 0]);
        // The ring: voussoirs alternating light and a little darker (their joints read).
        mb.color = J(k % 2 ? BRIDGE_STONE.ring : mixl(BRIDGE_STONE.ring, BRIDGE_STONE.wall, 0.45), r, 0.05);
        qn(mb, [x, aa, za], [x, aa + rt, za], [x, ab + rt, zb], [x, ab, zb], [s, 0, 0]);
      } else {
        mb.color = J(wall, r, 0.06);
        qn(mb, [x, deep, za], [x, ta + ph - 0.006, za], [x, tb + ph - 0.006, zb], [x, deep, zb], [s, 0, 0]);
      }
    }
    if (q) {
      // The arch's underside, and its shadowed inside: a dark screen down the middle (seen from
      // above, the opening otherwise shows only the bright water through it).
      const aa = curve(q, za), ab = curve(q, zb);
      mb.color = C(BRIDGE_STONE.shade);
      qn(mb, [-X, aa, za], [X, aa, za], [X, ab, zb], [-X, ab, zb], [0, -1, 0]);
      mb.color = C("#2a2420");
      mb.card([0, ys, za], [0, ys, zb], [0, ab, zb], [0, aa, za]);
    }
  }
  // The piers' faces toward the water, under each arch's springing; a cutwater on a middle pier.
  for (const q of arches) {
    for (const s of [-1, 1]) {
      mb.color = mulc(wall, 0.75);
      qn(mb, [-X, deep, q.c + s * q.a], [X, deep, q.c + s * q.a], [X, ys, q.c + s * q.a], [-X, ys, q.c + s * q.a], [0, 0, -s]);
    }
  }
  if (arches.length > 1) {
    at(mb, 0, deep, 0, 0, () => {
      mb.color = J(wall, r, 0.05);
      const d = 0.034, tip = 0.03;
      mb.prism([[-X, -d / 2], [X, -d / 2], [X + tip, 0], [X, d / 2], [-X, d / 2], [-X - tip, 0]], ys + 0.03 - deep, { top: 0.9 });
    });
  }
  bridgePosts(mb, r, X - pt / 2 + 0.002, Lh - 0.014, top(Lh), ph + 0.012, 0.024);
  bridgeFootings(mb, r, X, Lh);
}

recipe("bridge.arch", { variants: 2, build: (mb, r, { variant }) => buildArchBridge(mb, r, variant) });

/** A flat slab bridge along z: a deck of big pale flagstones on piers with cutwaters (two, or one),
 *  low walls of blocks along both sides, square posts at the ends, footings in both banks; it
 *  spans 0.68, its footings on the bank tops 0.3 from the river's line (as the arched bridge). */
function buildSlabBridge(mb, r, variant) {
  const Lh = 0.34, W = 0.112, pt = 0.016, ph = 0.017, th = 0.03, y0 = 0.008;
  const X = W / 2 + pt, deep = -0.14;
  // The flagstones over a dark bed (their joints).
  mb.color = C(BRIDGE_STONE.shade);
  qn(mb, [-W / 2, y0 - 0.002, -Lh], [W / 2, y0 - 0.002, -Lh], [W / 2, y0 - 0.002, Lh], [-W / 2, y0 - 0.002, Lh], [0, 1, 0]);
  const n = 9;
  for (let k = 0; k < n; k++) {
    const za = -Lh + ((2 * Lh) / n) * k + 0.002, zb = -Lh + ((2 * Lh) / n) * (k + 1) - 0.002;
    // Some courses are two stones side by side.
    const split = r() < 0.45 ? r.range(-0.02, 0.02) : null;
    const xs = split === null ? [[-W / 2 + 0.002, W / 2 - 0.002]] : [[-W / 2 + 0.002, split - 0.0015], [split + 0.0015, W / 2 - 0.002]];
    for (const [xa, xb] of xs) {
      const y = y0 + r.range(-0.0015, 0.0015);
      mb.color = J(BRIDGE_STONE.deck, r, 0.07);
      qn(mb, [xa, y, za], [xb, y, za], [xb, y, zb], [xa, y, zb], [0, 1, 0]);
    }
  }
  // Fascia and walls: blocks of their own heights along both sides, a coping on top.
  for (const s of [-1, 1]) {
    let z = -Lh;
    while (z < Lh - 1e-6) {
      const L = Math.min(Lh - z, r.range(0.06, 0.09));
      at(mb, s * (W / 2 + pt / 2), y0 - th, z + L / 2, 0, () => {
        mb.color = J(BRIDGE_STONE.wall, r, 0.06);
        mb.box(pt, th + ph + r.range(-0.002, 0.003), L - 0.0025);
      });
      z += L;
    }
    at(mb, s * (W / 2 + pt / 2), y0 + ph, 0, 0, () => {
      mb.color = J(BRIDGE_STONE.cap, r, 0.03);
      mb.box(pt + 0.004, 0.005, 2 * Lh - 0.03);
    });
  }
  // The deck's underside.
  mb.color = C(BRIDGE_STONE.shade);
  qn(mb, [-X, y0 - th, -Lh], [X, y0 - th, -Lh], [X, y0 - th, Lh], [-X, y0 - th, Lh], [0, -1, 0]);
  // Piers in the water, pointed up- and downstream (the river runs along x).
  for (const pz of variant ? [0] : [-0.11, 0.11]) {
    at(mb, 0, deep, pz, 0, () => {
      mb.color = J(BRIDGE_STONE.wall, r, 0.05);
      const d = 0.05, w = X + 0.012, tip = 0.035;
      mb.prism([[-w, -d / 2], [w, -d / 2], [w + tip, 0], [w, d / 2], [-w, d / 2], [-w - tip, 0]], y0 - th - deep + 0.002);
    });
  }
  bridgePosts(mb, r, X - pt / 2, Lh - 0.012, y0, ph + 0.014, 0.026);
  bridgeFootings(mb, r, X, Lh);
}

recipe("bridge.slab", { variants: 2, build: (mb, r, { variant }) => buildSlabBridge(mb, r, variant) });

// --- boats ------------------------------------------------------------------------------------

/** A clinker hull along z (bow +z): stations of a U section, sheer rising at the ends, from the
 *  keel below the water to the gunwale. */
function hull(mb, L, W, H, col, { sink = 0.018, rake = 0.012 } = {}) {
  const N = 6;
  const st = [];
  for (let i = 0; i <= N; i++) {
    const t = i / N;
    const z = -L / 2 + L * t;
    const w = (W / 2) * Math.pow(Math.sin(Math.PI * clamp(t * 0.92 + 0.04, 0, 1)), 0.7);
    const sheer = H + rake * Math.pow(2 * t - 1, 2) * 1.5;
    st.push([[-w, sheer - sink, z], [-w * 0.8, H * 0.3 - sink, z], [0, -sink, z], [w * 0.8, H * 0.3 - sink, z], [w, sheer - sink, z]]);
  }
  const m0 = mb.mark();
  mb.color = C(col);
  for (let i = 0; i < N; i++) {
    for (let k = 0; k < 4; k++) {
      const a = st[i][k], b = st[i][k + 1], c = st[i + 1][k + 1], d = st[i + 1][k];
      const cen = vscale(vadd(vadd(a, b), vadd(c, d)), 0.25);
      const ins = [0, H, cen[2]];
      const sh = (n, y) => {
        const base = y < 0.003 ? mulc(col, 0.55) : col;
        return faceShade(base, n, { lit: 0.25 });
      };
      ftri(mb, a, b, c, ins, sh);
      ftri(mb, a, c, d, ins, sh);
    }
  }
  // A darker strake at the gunwale.
  mb.color = mulc(col, 0.6);
  for (const k of [0, 4]) mb.tube(st.map((s) => vadd(s[k], [0, 0.001, 0])), 0.0022, 3);
  groundAO(mb, m0, -sink, H, 0.6);
  return st;
}

/** A fishing boat: a small hull, a stubby mast with a furled sail, nets heaped over the side and
 *  a line of floats; variant 2 is a bigger whaler with a harpoon at the bow. */
function buildBoat(mb, r, variant) {
  // A fishing smack seen side-on: a tarred hull, a mast with its lateen sail set (pale canvas with
  // a band in the owner's colour -- what makes a boat read on dark water), nets over the side.
  const whaler = variant >= 2;
  const L = whaler ? 0.22 : 0.19, W = whaler ? 0.07 : 0.062, H = 0.034;
  mb.part(() => {
    mb.rotateY(Math.PI / 2 - 0.4);
    hull(mb, L, W, H, J(r.pick(["#5e4430", "#56402e", "#6a4c32"]), r, 0.05), { sink: 0.02 });
    mb.color = C("#8a6a46");
    for (const z of [-0.035, 0.025]) at(mb, 0, H - 0.028, z, 0, () => mb.box(W * 0.85, 0.004, 0.014));
    const mz = 0.012, mh = whaler ? 0.21 : 0.19;
    mb.color = C("#3e2c20");
    stick(mb, [0, -0.01, mz], [0, mh, mz], 0.0035, 4);
    // The lateen yard, slanting from low at the bow to high over the stern, and the sail under it.
    const y0 = [0, 0.05, mz + L * 0.42], y1 = [0, mh + 0.03, mz - L * 0.36];
    stick(mb, y0, y1, 0.0025, 3);
    const foot = [0, 0.04, mz - L * 0.3];
    const s0 = mb.mark();
    mb.color = J("#d8ceb4", r, 0.04);
    mb.card(vadd(y0, [0, -0.004, 0]), vadd(y1, [0, -0.006, 0]), foot, foot);
    mb.tint(s0, (x, y) => 0.8 + 0.25 * clamp(y / mh, 0, 1));
    mb.heightSway(s0, 0.05, mh, 0.35);
    // A band of the owner's colour across the sail.
    mb.part(() => {
      mb.mat = MAT.TEAM;
      mb.color = TEAM;
      const lerp = (a, b, t) => vadd(a, vscale(vsub(b, a), t));
      const p0 = lerp(y0, y1, 0.42), p1 = lerp(y0, y1, 0.62), q0 = lerp(foot, y0, 0.08), q1 = lerp(foot, y1, 0.32);
      for (const o of [0.0015, -0.0015]) mb.quad(vadd(p0, [o, -0.006, 0]), vadd(p1, [o, -0.006, 0]), vadd(q1, [o, 0, 0]), vadd(q0, [o, 0, 0]));
    });
    // Heaped nets over the gunwale.
    mb.color = C("#3e3a30");
    const nm = mb.mark();
    mb.quad([-W * 0.4, H - 0.014, -0.06], [-W * 0.4, H - 0.014, -0.01], [-W * 0.62, -0.006, -0.0], [-W * 0.62, -0.006, -0.07]);
    mb.quad([-W * 0.4, H - 0.014, -0.06], [-W * 0.62, -0.006, -0.07], [-W * 0.62, -0.006, -0.0], [-W * 0.4, H - 0.014, -0.01]);
    mb.tint(nm, (x, y) => 0.8 + 0.4 * clamp(y / 0.02, 0, 1));
    if (whaler) {
      mb.color = C("#6a6e74");
      mb.mat = MAT.METAL;
      stick(mb, [0, 0.03, L / 2 - 0.01], [0, 0.05, L / 2 + 0.05], 0.0022, 3);
      mb.mat = MAT.MATTE;
    }
  });
  // Floats on the water.
  mb.color = C("#c8b890");
  for (let k = 0; k < 4; k++) at(mb, -0.08 - k * 0.026, 0.002, 0.045 + k * 0.012, 0, () => mb.sphere(0.005, 0, { squash: 0.7 }));
}

recipe("imp.boat", { variants: 3, build: (mb, r, { variant }) => buildBoat(mb, r, variant) });

// --- lairs, graves and ruins ------------------------------------------------------------------

/** A shade for dressed stone: value by facet, a lighter top, moss creeping up from the ground. */
function stoneShade(col, { moss = 0.35, top = null, seed = 0 } = {}) {
  return (n, y, a, b, c) => {
    let base = C(col);
    if (moss > 0 && y < 0.03 && hashf(Math.round(a[0] * 1e3 + seed), Math.round(c[2] * 1e3), Math.round(y * 1e3)) < moss) base = mixl(base, T.moss, 0.6);
    const v = 0.94 + 0.12 * hashf(Math.round(a[0] * 1e4), Math.round(b[2] * 1e4), seed);
    const k = faceShade(base, n, { top: top ?? mixl(base, "#ffffff", 0.15), lit: 0.25 });
    return [k[0] * v, k[1] * v, k[2] * v];
  };
}

/** A rectangular slab (w x d, y0..y1), flat-shaded with `shade`; `top` tapers it. */
function sblock(mb, w, d, y0, y1, shade, top = 1) {
  slab(mb, [[-w / 2, -d / 2], [w / 2, -d / 2], [w / 2, d / 2], [-w / 2, d / 2]], y0, y1, { top, shade });
}

/** A grassy mound: an earth dome with a grass skin, darker toward its foot. */
function mound(mb, r, rx, rz, h, { grass = "#5e6a2e", earth = "#5a4632", y = -0.01 } = {}) {
  const seed = r.range(0, 50);
  const m0 = mb.mark();
  mb.color = C(grass);
  mb.part(() => {
    mb.translate(0, y, 0);
    mb.scale(rx / h, 1, rz / h);
    mb.blob(h, { detail: 1, amp: 0.12, squash: 1, noise: (v) => noise3(v[0] * 2 + seed, v[1] * 2, v[2] * 2) });
  });
  mb.displace(m0, (x, yy) => [0, yy < y ? (y - yy) * 0.5 : 0, 0]);
  mb.sphereNormals(m0, [0, y - h, 0], 0.5);
  mb.tint(m0, (x, yy) => {
    const t = clamp((yy + 0.005) / (h * 0.3), 0, 1);
    const g = C(grass), e = C(earth);
    return [(e[0] + (g[0] - e[0]) * t) / g[0], (e[1] + (g[1] - e[1]) * t) / g[1], (e[2] + (g[2] - e[2]) * t) / g[2]];
  });
}

/** A bone from a to b with knobbed ends. */
function bone(mb, a, b, rad) {
  mb.tube([a, vadd(vscale(a, 0.5), vscale(b, 0.5)), b], [rad * 1.3, rad * 0.8, rad * 1.3], 4, { caps: true });
}

/** A skull facing +z (beast or man): a rounded cranium, a snout or jaw, dark sockets. */
function skull(mb, s = 0.02, { beast = false, horns = false } = {}) {
  mb.part(() => {
    mb.scale(s / 0.02);
    mb.color = C(T.bone);
    mb.sphere(0.02, s >= 0.016 ? 1 : 0, { squash: 0.85 });
    if (beast) {
      mb.tube([[0, -0.004, 0.01], [0, -0.008, 0.04]], [0.012, 0.007], 5, { caps: true });
      mb.tube([[0, -0.016, 0.006], [0, -0.02, 0.034]], [0.007, 0.004], 4, { caps: true });
    } else mb.tube([[0, -0.01, 0.004], [0, -0.018, 0.012]], [0.012, 0.008], 5, { caps: true });
    mb.color = C("#1a1412");
    for (const x of [-0.008, 0.008]) mb.quad([x - 0.004, -0.004, 0.0195], [x + 0.004, -0.004, 0.0195], [x + 0.004, 0.004, 0.0175], [x - 0.004, 0.004, 0.0175]);
    if (horns) {
      mb.color = C("#3a3230");
      for (const sx of [-1, 1]) mb.tube([[sx * 0.014, 0.01, -0.004], [sx * 0.03, 0.022, -0.01], [sx * 0.036, 0.04, 0.0]], [0.005, 0.0035, 0.0008], 4);
    }
  });
}

/** The way into a dungeon: a carved stone portal half sunk into a grassy mound, steps going down
 *  into the dark, a skull keystone, pale witch-fire in two braziers, bones on the threshold. */
function buildDungeon(mb, r, variant) {
  mound(mb, r, 0.19, 0.15, 0.1, { grass: variant % 2 ? "#3e5226" : "#4a5a2a", y: -0.035 });
  const st = stoneShade("#55504e", { moss: 0.4, seed: 3 });
  mb.scale(1.35);
  // Steps down into the mound and a black void behind them.
  at(mb, 0, 0, 0.115, 0, () => {
    plate(mb, -0.035, -0.04, 0.035, 0.075, -0.01, "#050404");
    for (let k = 0; k < 3; k++) sblock(mb, 0.07, 0.02, -0.04, 0.0 - k * 0.012, stoneShade("#3a3638", { moss: 0 }), 1), mb.translate(0, 0, -0.012);
  });
  at(mb, 0, 0, 0.11, 0, () => {
    // Pillars, a heavy lintel, a skull keystone.
    for (const x of [-0.045, 0.045]) at(mb, x, 0, 0, 0, () => sblock(mb, 0.022, 0.026, -0.04, 0.085, st, 0.85));
    at(mb, 0, 0.083, 0, 0, () => sblock(mb, 0.12, 0.032, 0, 0.026, st, 0.9));
    at(mb, 0, 0.11, 0.0, 0, () => sblock(mb, 0.05, 0.026, 0, 0.022, st, 0.6));
    at(mb, 0, 0.096, 0.02, 0, () => skull(mb, 0.011));
    // Witch-fire braziers.
    for (const x of [-0.075, 0.075]) {
      at(mb, x, 0, 0.02, 0, () => {
        mb.color = C("#2e2a2c");
        mb.lathe([[0.008, -0.01], [0.004, 0.03], [0.012, 0.04], [0.01, 0.045]], 6);
        mb.mat = MAT.EMISSIVE;
        flameTongue(mb, r, 0, 0, 0.03, 0.007, [0, 0], 0, WITCHFIRE);
        mb.mat = MAT.MATTE;
      });
    }
  });
  // Bones on the threshold.
  mb.color = C(T.bone);
  for (let k = 0; k < 3; k++) {
    const a = r.range(0, TAU), x = r.range(-0.05, 0.05), z = r.range(0.14, 0.17);
    bone(mb, [x - Math.cos(a) * 0.012, 0.002, z - Math.sin(a) * 0.012], [x + Math.cos(a) * 0.012, 0.002, z + Math.sin(a) * 0.012], 0.0022);
  }
  at(mb, r.range(-0.08, 0.08), 0.004, 0.16, r.range(-0.6, 0.6), () => skull(mb, 0.008));
  // Leaning standing stones on the mound's shoulders.
  for (const sx of [-1, 1]) {
    mb.part(() => {
      mb.translate(sx * r.range(0.12, 0.15), 0, r.range(-0.04, 0.02));
      mb.rotateZ(sx * r.range(0.1, 0.25));
      spire(mb, r, { h: r.range(0.08, 0.11), rad: 0.017, sides: 4, rings: 1, base: -0.04, taper: 0.4, cut: 0.006, shade: st });
    });
  }
}

recipe("imp.dungeon", { variants: 2, build: (mb, r, { variant }) => buildDungeon(mb, r, variant) });

/** A barrow: a long grassy burial mound, its entrance a stone trilithon with a door slab, a few
 *  standing stones keeping watch. */
function buildBarrow(mb, r, variant) {
  mound(mb, r, 0.24, 0.14, 0.11, { grass: "#4a5a28", y: -0.04 });
  const st = stoneShade("#6a6660", { moss: 0.5, seed: 5 });
  at(mb, 0, 0, 0.11, 0, () => {
    mb.scale(1.4);
    for (const x of [-0.03, 0.03]) at(mb, x, 0, 0, 0, () => sblock(mb, 0.018, 0.018, -0.04, 0.06, st, 0.85));
    at(mb, 0, 0.058, 0, 0, () => sblock(mb, 0.085, 0.024, 0, 0.018, st, 0.9));
    plate(mb, -0.021, -0.01, 0.021, 0.056, -0.004, "#2a2826");
    at(mb, 0.004, 0, 0.006, -0.12, () => sblock(mb, 0.036, 0.008, -0.01, 0.05, stoneShade("#55524e", { moss: 0.3 }), 1));
  });
  // Kerb stones and a couple of watchers.
  for (let k = 0; k < 4 + variant; k++) {
    const a = Math.PI * 0.62 + (k / (4 + variant)) * Math.PI * 1.76;
    const x = Math.cos(a) * 0.27, z = Math.sin(a) * 0.17;
    spire(mb, r, { x, z, h: k % 2 ? r.range(0.05, 0.08) : r.range(0.025, 0.04), rad: 0.015, sides: 4, rings: 1, base: -0.04, taper: 0.35, cut: 0.006, shade: st });
  }
}

recipe("imp.barrow", { variants: 2, build: (mb, r, { variant }) => buildBarrow(mb, r, variant) });

/** A headstone: a round-topped slab (0), a cross (1) or a pointed slab (2), leaning. */
function headstone(mb, r, kind, shade) {
  mb.part(() => {
    mb.rotateX(r.range(-0.25, 0.2));
    mb.rotateZ(r.range(-0.2, 0.2));
    if (kind === 1) {
      sblock(mb, 0.009, 0.008, -0.02, 0.062, shade);
      at(mb, 0, 0.042, 0, 0, () => sblock(mb, 0.034, 0.008, 0, 0.009, shade));
    } else {
      const w = 0.018, h = r.range(0.034, 0.05);
      const poly = kind === 0 ? [[-w, -0.0035], [w, -0.0035], [w, 0.0035], [-w, 0.0035]] : [[-w, -0.004], [w, -0.004], [w, 0.004], [-w, 0.004]];
      mb.part(() => {
        mb.rotateX(-Math.PI / 2);
        // Profile in x-y, extruded along z (thickness).
        const pts = kind === 0 ? [[-w, -0.02], [w, -0.02], [w, h], [w * 0.6, h + w * 0.6], [0, h + w * 0.8], [-w * 0.6, h + w * 0.6], [-w, h]] : [[-w, -0.02], [w, -0.02], [w, h], [0, h + w * 1.2], [-w, h]];
        slab(mb, pts.map(([x, y]) => [x, -y]), -0.0035, 0.0035, { top: 1, shade });
        void poly;
      });
    }
  });
}

/** A graveyard: rows of crooked headstones and crosses, a small crypt at the back, a broken
 *  wall; the dead tree comes as a separate part. */
function buildGraveyard(mb, r, variant) {
  const st = stoneShade("#7a7670", { moss: 0.45, seed: 9 });
  // The crypt.
  at(mb, -0.13, 0, -0.14, 0.15, () => {
    sblock(mb, 0.1, 0.085, -0.04, 0.075, stoneShade("#6a6660", { moss: 0.4 }));
    at(mb, 0, 0.075, 0, 0, () => roofGable(mb, 0.1, 0.085, 0.07, { col: "#3e3c40", ends: "#6a6660", over: 0.008 }));
    plate(mb, -0.016, 0.0, 0.016, 0.05, 0.0437, "#14100e");
    at(mb, 0, 0.145, 0.0, 0, () => {
      sblock(mb, 0.007, 0.007, -0.02, 0.03, st);
      at(mb, 0, 0.016, 0, 0, () => sblock(mb, 0.022, 0.007, 0, 0.007, st));
    });
  });
  // Graves in rough rows, each with a low mound.
  for (let j = 0; j < 3; j++) {
    for (let k = 0; k < 3; k++) {
      if (r() < 0.15) continue;
      const x = 0.0 + k * 0.07 + r.range(-0.01, 0.01), z = -0.13 + j * 0.085 + r.range(-0.01, 0.01);
      at(mb, x, 0, z, r.range(-0.2, 0.2), () => {
        mb.scale(1.3);
        headstone(mb, r, r.pick([0, 0, 1, 2]), st);
        mb.color = J("#4a4030", r, 0.06);
        at(mb, 0, -0.004, 0.024, 0, () => mb.lathe([[0.012, 0], [0.008, 0.006], [0, 0.007]], 5, { wobble: () => 0 }));
      });
    }
  }
  // A broken wall along the back.
  for (let k = 0; k < 5; k++) {
    if (k === 2 && variant) continue;
    at(mb, -0.06 + k * 0.06, 0, -0.24, 0, () => sblock(mb, 0.058, 0.018, -0.03, r.range(0.018, 0.035), stoneShade("#5a5650", { moss: 0.5 }), 0.95));
  }
}

recipe("imp.graveyard", { variants: 2, build: (mb, r, { variant }) => buildGraveyard(mb, r, variant) });

/** A column: base block, shaft, capital; broken ones end in a jagged stump. */
function column(mb, r, h, rad, shade, broken = false) {
  sblock(mb, rad * 2.6, rad * 2.6, -0.03, 0.008, shade);
  const sides = 7;
  const lo = [], hi = [];
  for (let s = 0; s < sides; s++) {
    const a = (s / sides) * TAU;
    lo.push([Math.cos(a) * rad, 0.008, Math.sin(a) * rad]);
    hi.push([Math.cos(a) * rad * 0.9, broken ? h * r.range(0.82, 1.0) : h, Math.sin(a) * rad * 0.9]);
  }
  const ins = [0, h / 2, 0];
  for (let s = 0; s < sides; s++) {
    const q = (s + 1) % sides;
    ftri(mb, lo[s], hi[s], hi[q], ins, shade);
    ftri(mb, lo[s], hi[q], lo[q], ins, shade);
  }
  const ct = [0, broken ? h * 0.85 : h, 0];
  for (let s = 0; s < sides; s++) ftri(mb, ct, hi[s], hi[(s + 1) % sides], [0, ct[1] - 1, 0], shade);
  if (!broken) at(mb, 0, h, 0, 0, () => sblock(mb, rad * 2.6, rad * 2.6, 0, 0.01, shade, 1.0));
}

/** A broken wall: slices of masonry with ragged tops, sometimes pierced by an arched window. */
function brokenWall(mb, r, w, h, th, shade, { arch = false } = {}) {
  const n = 6;
  const peak = r.range(1.2, 2.6);
  for (let k = 0; k < n; k++) {
    const x0 = -w / 2 + (w * k) / n;
    const top = h * clamp(1.05 - Math.abs(k - peak) * r.range(0.12, 0.3) - (r() < 0.3 ? 0.25 : 0), 0.12, 1);
    if (top < 0.01) continue;
    at(mb, x0 + w / n / 2, 0, 0, 0, () => sblock(mb, (w / n) * 1.02, th, -0.03, top, shade));
    if (arch && k === 2 && top > 0.07) {
      const xc = x0 + w / n / 2;
      plate(mb, xc - 0.012, 0.03, xc + 0.012, 0.062, th / 2 + 0.001, "#1e1a18");
      mb.tri([xc - 0.012, 0.062, th / 2 + 0.001], [xc + 0.012, 0.062, th / 2 + 0.001], [xc, 0.074, th / 2 + 0.001]);
    }
  }
}

/** Ruins: the corner of a great hall left standing (an L of broken wall, an empty window), a
 *  colonnade with one column whole, rubble heaps and fallen drums. */
function buildRuins(mb, r, variant, big = false) {
  const col = J(variant % 2 ? "#8a8274" : "#7e786c", r, 0.04);
  const st = stoneShade(col, { moss: 0.5, seed: variant });
  // The corner: back wall and side wall.
  at(mb, 0.02, 0, -0.14, r.range(-0.1, 0.1), () => brokenWall(mb, r, 0.26, 0.15, 0.03, st, { arch: true }));
  at(mb, -0.12, 0, -0.04, Math.PI / 2 + r.range(-0.1, 0.1), () => brokenWall(mb, r, 0.18, 0.1, 0.03, st));
  // A colonnade in front, one column whole.
  const n = big ? 5 : 3;
  for (let k = 0; k < n; k++) {
    const x = 0.02 + k * 0.065, z = 0.02 + r.range(-0.01, 0.01);
    const whole = k === 0;
    at(mb, x, 0, z, 0, () => column(mb, r, whole ? 0.15 : r.range(0.05, 0.1), 0.014, st, !whole));
  }
  // Rubble and fallen drums.
  const sh = rockShader({ base: col, top: mixl(col, "#a8a090", 0.4), coat: [T.moss, T.grass], cover: 0.3 }, variant);
  for (let k = 0; k < (big ? 6 : 4); k++) {
    const a = r.range(0, TAU), d = r.range(0.06, 0.13);
    rockMass(mb, r, { rad: r.range(0.014, 0.024), detail: 0, sx: 1.3, sy: 0.7, cuts: 4, cut: 0.5, c: [-0.06 + Math.cos(a) * d, 0.0, -0.06 + Math.sin(a) * d], shade: sh });
  }
  for (let k = 0; k < 2; k++) at(mb, 0.06 + k * 0.08, 0.012, 0.1 + r.range(-0.02, 0.02), r.range(-0.6, 0.6), () => { mb.color = col; mb.rotateZ(Math.PI / 2); mb.translate(0, -0.016, 0); mb.cylinder(0.013, 0.013, 0.032, 7); });
  if (big) {
    at(mb, 0.22, 0, 0.1, Math.PI / 2 - 0.3, () => brokenWall(mb, r, 0.16, 0.08, 0.026, st));
    // A broken tower stump.
    at(mb, 0.19, 0, -0.18, 0, () => {
      // A broken tower: an open drum with a ragged top, dark inside.
      const sides = 9, lo = [], hi = [], hiIn = [];
      for (let s2 = 0; s2 < sides; s2++) {
        const a = (s2 / sides) * TAU;
        const top = 0.06 + 0.1 * Math.pow(0.5 + 0.5 * Math.sin(a + 2.2), 2) * r.range(0.8, 1.1);
        lo.push([Math.cos(a) * 0.05, -0.04, Math.sin(a) * 0.05]);
        hi.push([Math.cos(a) * 0.047, top, Math.sin(a) * 0.047]);
        hiIn.push([Math.cos(a) * 0.036, top, Math.sin(a) * 0.036]);
      }
      for (let s2 = 0; s2 < sides; s2++) {
        const q = (s2 + 1) % sides;
        ftri(mb, lo[s2], hi[s2], hi[q], [0, 0.05, 0], st);
        ftri(mb, lo[s2], hi[q], lo[q], [0, 0.05, 0], st);
        ftri(mb, hi[s2], hiIn[s2], hiIn[q], [hi[s2][0] * 0.8, hi[s2][1] - 0.1, hi[s2][2] * 0.8], st);
        ftri(mb, hi[s2], hiIn[q], hi[q], [hi[s2][0] * 0.8, hi[s2][1] - 0.1, hi[s2][2] * 0.8], st);
        const inner = (n) => mulc(col, 0.35);
        ftri(mb, hiIn[s2], [hiIn[s2][0], -0.02, hiIn[s2][2]], [hiIn[q][0], -0.02, hiIn[q][2]], [0, 0.2, 0].map((v, i) => (i === 1 ? 0.05 : hiIn[s2][i] * 2)), inner);
        ftri(mb, hiIn[s2], [hiIn[q][0], -0.02, hiIn[q][2]], hiIn[q], [0, 0.2, 0].map((v, i) => (i === 1 ? 0.05 : hiIn[s2][i] * 2)), inner);
      }
      plate(mb, -0.009, 0.03, 0.009, 0.06, 0.0485, "#1a1614");
    });
  }
}

recipe("imp.ruins", { variants: 2, build: (mb, r, { variant }) => buildRuins(mb, r, variant) });
recipe("imp.cityruins", { variants: 2, build: (mb, r, { variant }) => buildRuins(mb, r, variant, true) });

/** A great beast's bones: an arched ribcage half sunk in the ground, a horned skull, a broken
 *  spine and a few long bones. */
function buildBones(mb, r, variant) {
  const s = variant % 2 ? 1.2 : 1;
  mb.color = C(T.bone);
  const m0 = mb.mark();
  // Spine along x.
  const spine = [];
  for (let i = 0; i <= 6; i++) spine.push([-0.1 * s + 0.04 * s * i, 0.035 * s * Math.sin((i / 6) * Math.PI) + 0.004, 0.008 * Math.sin(i)]);
  for (let i = 0; i < spine.length; i++) at(mb, ...spine[i], 0, () => mb.sphere(0.008 * s, 0, { squash: 0.8 }));
  // Ribs arching down into the ground on both sides.
  for (let i = 1; i < 5; i++) {
    const p = spine[i];
    for (const sz of [-1, 1]) {
      const L = 0.06 * s * (1 - Math.abs(i - 2.5) * 0.12);
      mb.tube([p, [p[0] + 0.005, p[1] + 0.012 * s, p[2] + sz * L * 0.6], [p[0] + 0.01, p[1] - 0.01, p[2] + sz * L], [p[0] + 0.012, -0.01, p[2] + sz * L * 0.95]], [0.004 * s, 0.0035 * s, 0.003 * s, 0.0025 * s], 3);
    }
  }
  at(mb, 0.15 * s, 0.012 * s, 0.01, Math.PI / 2 + 0.3, () => {
    mb.rotateX(0.3);
    skull(mb, 0.024 * s, { beast: true, horns: variant >= 1 });
  });
  bone(mb, [-0.06, 0.004, 0.09], [0.02, 0.004, 0.11], 0.004 * s);
  bone(mb, [-0.14, 0.004, -0.05], [-0.1, 0.004, -0.11], 0.0035 * s);
  mb.tint(m0, (x, y) => 0.7 + 0.35 * clamp(y / 0.05, 0, 1));
}

recipe("imp.bones", { variants: 3, build: (mb, r, { variant }) => buildBones(mb, r, variant) });

/** A shipwreck: half a hull heeled over and sunk to the gunwale, ribs bare at the broken end,
 *  the mast snapped and a torn sail trailing. */
function buildWreck(mb, r, variant) {
  mb.part(() => {
    mb.rotateY(variant % 2 ? 0.6 : -0.5);
    mb.translate(0, -0.012, 0);
    mb.rotateZ(0.45);
    const wood = J("#4a3a2c", r, 0.05);
    const st = hull(mb, 0.24, 0.08, 0.04, wood, { sink: 0.02, rake: 0.016 });
    // Bare ribs past the break.
    mb.color = mulc(wood, 0.8);
    for (let k = 0; k < 4; k++) {
      const z = -0.12 - k * 0.025;
      mb.tube([[-0.035, 0.02, z], [-0.03, -0.01, z], [0, -0.02, z], [0.03, -0.01, z], [0.035, 0.02 - k * 0.006, z]], 0.0028, 3);
    }
    mb.color = C("#3a2e24");
    stick(mb, [0, -0.01, 0.02], [0.03, 0.09, 0.0], 0.004, 4);
    mb.color = J("#a89a7a", r, 0.05);
    const s0 = mb.mark();
    mb.card([0.028, 0.085, 0.0], [0.016, 0.05, 0.003], [0.055, 0.02, 0.03], [0.07, 0.06, 0.02]);
    mb.heightSway(s0, 0.02, 0.09, 0.4);
    void st;
  });
  // Flotsam.
  mb.color = C("#5a4632");
  for (let k = 0; k < 3; k++) at(mb, r.range(-0.12, 0.12), 0.0, r.range(0.05, 0.12), r.range(0, TAU), () => mb.box(0.03, 0.004, 0.008));
}

recipe("imp.wreck", { variants: 2, build: (mb, r, { variant }) => buildWreck(mb, r, variant) });

/** A beast's den: a cave mouth in a pile of boulders, gnawed bones at the threshold (param "lion"
 *  dresses it in dry grass, else it is a forest den with a fallen log). */
function buildDen(mb, r, variant, kind) {
  const lion = kind === "lion";
  const p = rockPaint(lion ? "sand" : "moss", r);
  const sh = rockShader(p, 2);
  rockMass(mb, r, { rad: 0.1, sx: 1.4, sy: 0.85, sz: 0.9, cuts: 4, cut: 0.65, c: [0, 0.03, -0.08], shade: sh });
  rockMass(mb, r, { rad: 0.06, sx: 1.0, sy: 1.1, cuts: 3, cut: 0.6, c: [-0.08, 0.02, 0.0], shade: sh });
  rockMass(mb, r, { rad: 0.055, sx: 1.0, sy: 1.2, cuts: 3, cut: 0.6, c: [0.08, 0.02, -0.01], shade: sh });
  // The black mouth between them.
  mb.color = C("#070505");
  mb.part(() => {
    mb.translate(0, 0, 0.0);
    mb.lathe([[0.042, 0], [0.036, 0.03], [0.02, 0.05], [0, 0.056]], 7, { caps: false, phase: 0 });
  });
  mb.color = C(T.bone);
  for (let k = 0; k < 4; k++) {
    const a = r.range(0, TAU), x = r.range(-0.06, 0.06), z = r.range(0.05, 0.11);
    bone(mb, [x - Math.cos(a) * 0.012, 0.002, z - Math.sin(a) * 0.012], [x + Math.cos(a) * 0.012, 0.002, z + Math.sin(a) * 0.012], 0.0025);
  }
  at(mb, r.range(-0.04, 0.04), 0.004, 0.09, r.range(-0.5, 0.5), () => skull(mb, 0.009, { beast: true }));
  if (lion) {
    mb.mat = MAT.FOLIAGE;
    tufts(mb, r, 4, 0.14, 0.05, [T.grassDry, "#a89a64"]);
    mb.mat = MAT.MATTE;
  } else {
    mb.color = C("#4a3626");
    stick(mb, [0.06, 0.008, 0.06], [0.16, 0.012, 0.02], 0.009, 5, true);
  }
}

recipe("imp.den", { variants: 2, build: (mb, r, { variant, param }) => buildDen(mb, r, variant, param) });

/** A goody hut: a lone round hut of the wild folk, a totem and a cold fire. */
function buildGoodyHut(mb, r, variant) {
  mb.color = C("#6e5a40");
  mb.lathe([[0.05, -0.01], [0.048, 0.04]], 8, { caps: false });
  plate(mb, -0.01, 0.0, 0.01, 0.032, 0.05, "#1e1610");
  mb.color = J("#7a6a40", r, 0.05);
  const m0 = mb.mark();
  mb.lathe([[0.064, 0.032], [0.04, 0.06], [0.01, 0.088], [0, 0.092]], 8);
  mb.tint(m0, (x, y) => 0.7 + 0.4 * clamp((y - 0.03) / 0.06, 0, 1));
  at(mb, 0.08, 0, 0.03, 0, () => {
    mb.color = C("#5a4430");
    mb.cylinder(0.008, 0.007, 0.11, 6);
    mb.color = C("#8a3a2a");
    at(mb, 0, 0.07, 0, 0, () => mb.box(0.018, 0.016, 0.018));
    mb.color = C("#d8c8a0");
    at(mb, 0, 0.095, 0, 0, () => mb.box(0.03, 0.006, 0.006));
  });
  at(mb, -0.06, 0, 0.06, 0, () => buildFirePit(mb, r, 1));
}

recipe("imp.goodyhut", { variants: 2, build: (mb, r, { variant }) => buildGoodyHut(mb, r, variant) });

/** A wooden cage on a cart bed, a hunched shape in the dark inside. */
function buildCage(mb, r) {
  mb.color = C("#4a3626");
  block(mb, 0.09, 0.06, 0.012, 0.02, "#4a3626");
  mb.color = C("#2a2420");
  for (const [x, z] of [[-0.035, 0.034], [0.035, 0.034], [-0.035, -0.034], [0.035, -0.034]]) at(mb, x, 0.012, z, 0, () => { mb.rotateX(Math.PI / 2); mb.translate(0, -0.002, 0); mb.cylinder(0.012, 0.012, 0.004, 7); });
  mb.color = C("#5a4430");
  for (let k = 0; k <= 6; k++) {
    const x = -0.042 + k * 0.014;
    stick(mb, [x, 0.02, 0.028], [x, 0.08, 0.028], 0.0016, 3);
    stick(mb, [x, 0.02, -0.028], [x, 0.08, -0.028], 0.0016, 3);
  }
  for (const x of [-0.042, 0.042]) for (const z of [-0.014, 0, 0.014]) stick(mb, [x, 0.02, z], [x, 0.08, z], 0.0016, 3);
  block(mb, 0.094, 0.064, 0.08, 0.086, "#4a3626");
  mb.color = C("#2a2420");
  at(mb, 0.0, 0.02, -0.005, 0, () => mb.sphere(0.02, 1, { squash: 0.9 }));
}

recipe("imp.cage", { variants: 1, build: (mb, r) => buildCage(mb, r) });

/** A necrototem: a crooked pole stacked with skulls, bones hung from a crossbar, sickly green
 *  light in the eye sockets, on a cairn of bones. */
function buildNecrototem(mb, r) {
  mb.color = C("#3a2e28");
  mb.tube([[0, -0.03, 0], [0.004, 0.08, 0], [-0.003, 0.17, 0.002]], [0.007, 0.006, 0.005], 5);
  for (let k = 0; k < 3; k++) at(mb, 0, 0.09 + k * 0.03, 0.004, r.range(-0.3, 0.3), () => skull(mb, 0.012 - k * 0.0015, { horns: k === 2 }));
  mb.mat = MAT.EMISSIVE;
  mb.color = C("#7ae05a");
  for (let k = 0; k < 3; k++) {
    const s = (0.012 - k * 0.0015) / 0.02;
    for (const x of [-0.008, 0.008]) at(mb, x * s, 0.09 + k * 0.03, 0.004 + 0.0198 * s, 0, () => mb.quad([-0.0025, -0.0025, 0], [0.0025, -0.0025, 0], [0.0025, 0.0025, 0], [-0.0025, 0.0025, 0]));
  }
  mb.mat = MAT.MATTE;
  mb.color = C("#3a2e28");
  stick(mb, [-0.035, 0.15, 0], [0.035, 0.155, 0], 0.003, 3);
  mb.color = C(T.bone);
  for (const x of [-0.03, -0.012, 0.016, 0.032]) {
    const m0 = mb.mark();
    stick(mb, [x, 0.152, 0], [x + 0.002, 0.12 - Math.abs(x) * 0.5, 0.002], 0.0018, 3);
    mb.heightSway(m0, 0.15, 0.11, 0.3);
  }
  // A cairn of bones and skulls.
  mb.color = J("#a89c84", r, 0.05);
  mb.lathe([[0.04, -0.01], [0.03, 0.015], [0.01, 0.028], [0, 0.03]], 7, { wobble: wobbler(r, 0.2) });
  for (let k = 0; k < 3; k++) at(mb, Math.cos(k * 2.1) * 0.03, 0.012, Math.sin(k * 2.1) * 0.03 + 0.01, k, () => skull(mb, 0.008));
}

recipe("imp.necrototem", { variants: 1, build: (mb, r) => buildNecrototem(mb, r) });

/** A snake pillar: a carved jungle-stone column with a great serpent coiled up it, its hooded
 *  head rearing over the top. */
function buildSnakePillar(mb, r) {
  const st = stoneShade("#6a6c58", { moss: 0.6, seed: 2 });
  sblock(mb, 0.07, 0.07, -0.04, 0.02, st, 0.9);
  column(mb, r, 0.16, 0.018, st, false);
  const pts = [], radii = [];
  for (let i = 0; i <= 20; i++) {
    const t = i / 20, a = t * TAU * 2.4;
    pts.push([Math.cos(a) * 0.027, 0.025 + t * 0.15, Math.sin(a) * 0.027]);
    radii.push(0.008 * (0.5 + t * 0.6));
  }
  pts.push([0.0, 0.2, 0.03], [0.0, 0.215, 0.05]);
  radii.push(0.011, 0.007);
  mb.color = C("#4a6a3a");
  mb.mat = MAT.METAL;
  const m0 = mb.mark();
  mb.tube(pts, radii, 5, { caps: true });
  mb.tint(m0, (x, y, z) => 0.8 + 0.35 * (0.5 + 0.5 * Math.sin(y * 300)));
  mb.mat = MAT.MATTE;
  // The hood.
  mb.color = C("#3e5a30");
  mb.card([-0.018, 0.21, 0.035], [0.018, 0.21, 0.035], [0.012, 0.186, 0.028], [-0.012, 0.186, 0.028]);
  mb.mat = MAT.EMISSIVE;
  mb.color = C("#e8c040");
  for (const x of [-0.005, 0.005]) at(mb, x, 0.218, 0.052, 0, () => mb.sphere(0.0022, 0));
  mb.mat = MAT.MATTE;
}

recipe("imp.snakepillar", { variants: 1, build: (mb, r) => buildSnakePillar(mb, r) });

/** A jungle altar: a stepped pyramid of mossy stone, a stair up its face, the altar stone on top
 *  stained dark, a fire bowl at each corner. */
function buildJungleAltar(mb, r) {
  const st = stoneShade("#55564a", { moss: 0.55, seed: 6 });
  const steps = [[0.26, 0.03], [0.2, 0.06], [0.14, 0.09], [0.08, 0.115]];
  let y0 = -0.04;
  for (const [w, y1] of steps) {
    sblock(mb, w, w * 0.9, y0, y1, st, 0.96);
    y0 = y1 - 0.004;
  }
  // The stair.
  mb.color = C("#4a4a3c");
  for (let k = 0; k < 8; k++) at(mb, 0, k * 0.014, 0.117 - k * 0.0095, 0, () => mb.box(0.04, 0.012, 0.012));
  at(mb, 0, 0.115, 0, 0, () => sblock(mb, 0.04, 0.026, 0, 0.018, stoneShade("#4e4a40", { moss: 0 })));
  mb.color = C("#4a1a14");
  at(mb, 0, 0.1335, 0, 0, () => mb.box(0.03, 0.001, 0.018));
  for (const [x, z] of [[-0.06, 0.04], [0.06, 0.04]]) {
    at(mb, x, 0.09, z, 0, () => {
      mb.color = C("#3a3630");
      mb.lathe([[0.006, 0], [0.012, 0.012], [0.01, 0.014]], 6);
      mb.mat = MAT.EMISSIVE;
      flameTongue(mb, r, 0, 0, 0.026, 0.006, [0, 0], 1);
      mb.mat = MAT.MATTE;
    });
  }
}

recipe("imp.junglealtar", { variants: 1, build: (mb, r) => buildJungleAltar(mb, r) });

/** A guardian: a colossal hooded stone warrior on a plinth, hands folded on the pommel of a
 *  sword planted point-down before it. */
function buildGuardian(mb, r, variant) {
  const st = stoneShade(variant ? "#7a766c" : "#6e6a66", { moss: 0.5, seed: 4 });
  sblock(mb, 0.13, 0.11, -0.04, 0.03, st, 0.92);
  // Robe (a tapering prism), shoulders, hooded head.
  slab(mb, [[-0.04, -0.028], [0.04, -0.028], [0.046, 0.024], [-0.046, 0.024]], 0.03, 0.2, { top: 0.72, shade: st });
  at(mb, 0, 0.2, 0, 0, () => sblock(mb, 0.07, 0.04, 0, 0.022, st, 0.8));
  at(mb, 0, 0.222, -0.003, 0, () => {
    slab(mb, jagPoly({ range: (a, b) => (a + b) / 2 }, 6, 0.022, 0), 0, 0.04, { top: 0.45, shade: st });
    plate(mb, -0.009, 0.006, 0.009, 0.026, 0.02, "#1a1816");
  });
  // The sword: blade down to the plinth, crossguard, hands over the pommel.
  at(mb, 0, 0, 0.034, 0, () => {
    sblock(mb, 0.012, 0.004, 0.03, 0.15, st, 0.6);
    at(mb, 0, 0.15, 0, 0, () => sblock(mb, 0.05, 0.008, 0, 0.008, st));
    at(mb, 0, 0.158, 0, 0, () => sblock(mb, 0.024, 0.016, 0, 0.022, st, 0.8));
  });
}

recipe("imp.guardian", { variants: 2, build: (mb, r, { variant }) => buildGuardian(mb, r, variant) });

/** A portal: a standing ring of dark stone on a stepped plinth, an arcane whirl inside it. */
function buildPortal(mb, r, variant, school = "") {
  const st = stoneShade("#48444c", { moss: 0.2, seed: 1 });
  sblock(mb, 0.18, 0.08, -0.04, 0.012, st);
  sblock(mb, 0.14, 0.05, 0.01, 0.022, st);
  const R = 0.075, cy = 0.1;
  const pts = [];
  for (let i = 0; i <= 18; i++) {
    const a = -Math.PI / 2 + (i / 18) * TAU;
    pts.push([Math.cos(a) * R, cy + Math.sin(a) * R, 0]);
  }
  mb.color = C("#3e3a42");
  mb.tube(pts, 0.012, 6);
  // Rune stones on the ring.
  for (let k = 0; k < 5; k++) {
    const a = Math.PI / 2 + (k - 2) * 0.5;
    at(mb, Math.cos(a) * R, cy + Math.sin(a) * R, 0, 0, () => sblock(mb, 0.02, 0.026, -0.012, 0.012, st));
  }
  // The whirl: rings of glow, bright at the heart.
  const glow = school && MANA[school] ? MANA[school] : PURPLE;
  mb.mat = MAT.EMISSIVE;
  const m0 = mb.mark();
  mb.color = C(glow);
  mb.part(() => {
    mb.translate(0, cy, 0);
    mb.rotateX(Math.PI / 2);
    mb.lathe([[R * 0.96, 0], [R * 0.6, 0.001], [R * 0.25, 0.002], [0, 0.002]], 14, { caps: false, wobble: (a, y) => 0.06 * Math.sin(a * 3 + y * 900) });
  });
  mb.tint(m0, (x, y, z) => {
    const d = Math.hypot(x, y - cy) / R;
    const sw = 0.5 + 0.5 * Math.sin(Math.atan2(y - cy, x) * 2 + d * 9);
    return 0.2 + (1 - d) * 0.65 + sw * 0.3;
  });
  mb.mat = MAT.MATTE;
}

const PURPLE = "#6a3ad0";
recipe("imp.portal", { variants: 1, build: (mb, r, { variant, param }) => buildPortal(mb, r, variant, param) });

/** Hellfire: a pit of infernal flame ringed by black thorns and skulls. */
function buildHellfire(mb, r) {
  mb.color = C(T.char);
  mb.lathe([[0.12, -0.02], [0.1, 0.006], [0.06, 0.01], [0.04, 0.0], [0, -0.005]], 9, { caps: false, wobble: wobbler(r, 0.12) });
  mb.mat = MAT.METAL;
  for (let k = 0; k < 7; k++) {
    const a = (k / 7) * TAU + r.range(-0.2, 0.2);
    const d = 0.085;
    crystal(mb, [Math.cos(a) * d, -0.01, Math.sin(a) * d], [Math.cos(a) * 0.5, 1, Math.sin(a) * 0.5], r.range(0.06, 0.1), 0.008, 4, { tip: 0.6, foot: 0, shade: (n) => faceShade("#1a1418", n, { lit: 0.5 }) });
  }
  mb.mat = MAT.MATTE;
  for (let k = 0; k < 3; k++) at(mb, Math.cos(k * 2.3) * 0.1, 0.006, Math.sin(k * 2.3) * 0.1, -k * 2.3 + Math.PI / 2, () => skull(mb, 0.009, { horns: k === 0 }));
  mb.mat = MAT.EMISSIVE;
  for (let k = 0; k < 5; k++) {
    const a = (k / 5) * TAU, d = k ? 0.025 : 0;
    flameTongue(mb, r, Math.cos(a) * d, Math.sin(a) * d, k ? 0.1 : 0.16, k ? 0.016 : 0.022, [Math.cos(a) * d * 0.5, Math.sin(a) * d * 0.5], 0.2);
  }
  mb.mat = MAT.MATTE;
}

recipe("imp.hellfire", { variants: 1, build: (mb, r) => buildHellfire(mb, r) });

/** A mushroom grove: giant toadstools on fat stalks over a mossy floor, caps spotted pale. */
function buildMushrooms(mb, r, variant) {
  const caps = variant ? ["#6a3a5a", "#7a4a6a", "#5a3a4a"] : ["#8a5a32", "#9a6a3a", "#7a4a2a"];
  const n = 5;
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + r.range(-0.3, 0.3), d = k ? r.range(0.05, 0.1) : 0;
    const h = (k ? r.range(0.04, 0.08) : 0.11) * (variant ? 1.1 : 1);
    const cr = h * r.range(0.45, 0.6);
    at(mb, Math.cos(a) * d, 0, Math.sin(a) * d, r.range(0, TAU), () => {
      mb.rotateZ(r.range(-0.15, 0.15));
      mb.color = C("#d8ccb0");
      mb.lathe([[cr * 0.3, -0.01], [cr * 0.24, h * 0.5], [cr * 0.2, h]], 6, { caps: false });
      mb.color = J(caps[k % 3], r, 0.06);
      const m0 = mb.mark();
      mb.lathe([[cr * 0.25, h - 0.002], [cr, h - 0.004], [cr * 0.95, h + cr * 0.25], [cr * 0.55, h + cr * 0.55], [0, h + cr * 0.62]], 7, { caps: false });
      mb.tint(m0, (x, y) => (y < h ? 0.55 : 0.85 + 0.25 * clamp((y - h) / (cr * 0.6), 0, 1)));
      mb.color = C("#e8dcc0");
      for (let l = 0; l < 3; l++) {
        const b = r.range(0, TAU), sx = Math.cos(b) * cr * 0.55, sz = Math.sin(b) * cr * 0.55, sy = h + cr * 0.47, e = cr * 0.12;
        mb.tri([sx - e, sy, sz - e * 0.5], [sx + e, sy, sz - e * 0.5], [sx, sy + e * 0.4, sz + e]);
      }
    });
  }
}

recipe("imp.mushrooms", { variants: 2, build: (mb, r, { variant }) => buildMushrooms(mb, r, variant) });

/** A huddle of penguins on the ice. */
function buildPenguins(mb, r) {
  for (let k = 0; k < 6; k++) {
    const a = r.range(0, TAU), d = k ? r.range(0.03, 0.09) : 0;
    const s = r.range(0.8, 1.1) * (k === 5 ? 0.6 : 1);
    at(mb, Math.cos(a) * d, 0, Math.sin(a) * d, r.range(-0.9, 0.9), () => {
      mb.scale(s);
      mb.color = C("#1e1e24");
      mb.lathe([[0.006, -0.002], [0.011, 0.012], [0.009, 0.028], [0.006, 0.036], [0, 0.04]], 5);
      mb.color = C("#e8e8e4");
      mb.part(() => {
        mb.translate(0, 0, 0.003);
        mb.lathe([[0.008, 0.002], [0.0095, 0.014], [0.006, 0.028], [0, 0.031]], 6, { caps: false, phase: Math.PI / 2 - 0.9 });
      });
      mb.color = C("#e0a030");
      stick(mb, [0, 0.033, 0.006], [0, 0.031, 0.013], 0.0018, 3);
    });
  }
}

recipe("imp.penguins", { variants: 2, build: (mb, r) => buildPenguins(mb, r) });

/** A pirates' cove: a plank jetty on stilts, a tarred shack, a black flag on a pole. */
function buildPirateCove(mb, r, variant) {
  mb.color = C("#4a3a2c");
  for (let k = 0; k < 4; k++) for (const x of [-0.02, 0.02]) stick(mb, [x, -0.06, 0.04 + k * 0.05], [x, 0.016, 0.04 + k * 0.05], 0.004, 4);
  mb.color = C("#7a5e40");
  at(mb, 0, 0.012, 0.115, 0, () => mb.box(0.05, 0.005, 0.18));
  at(mb, -0.02, 0, -0.06, 0, () => cottage(mb, r, { ...FAMILY.coastal, key: "coastal", wall: "#4a4038", wall2: "#3a322c" }, 0.1, 0.08, 0.06, { roof: "#2e2a28", lit: true }));
  mb.color = C("#3a2e24");
  stick(mb, [0.07, -0.02, -0.02], [0.07, 0.17, -0.02], 0.003, 4);
  mb.color = C("#18161a");
  const m0 = mb.mark();
  mb.card([0.072, 0.168, -0.02], [0.12, 0.165, -0.02], [0.118, 0.135, -0.018], [0.072, 0.138, -0.02]);
  mb.heightSway(m0, 0.13, 0.17, 0.6);
  mb.color = C("#d8d0c0");
  at(mb, 0.095, 0.152, -0.017, 0, () => mb.sphere(0.006, 0));
  for (let k = 0; k < 2; k++) barrel(mb, -0.08 + k * 0.028, 0.05);
}

recipe("imp.piratecove", { variants: 2, build: (mb, r, { variant }) => buildPirateCove(mb, r, variant) });

// --- mana nodes -------------------------------------------------------------------------------

/** How each school's node is dressed: stone colour, stone shape (slab, pillar, rough, spike,
 *  shard), extras round the dais, crystal height and form. */
const SCHOOLS = {
  "": { stone: "#6a6870", shape: "slab" },
  AIR: { stone: "#9a9ea8", shape: "pillar", lift: 0.24, extra: "ring" },
  BODY: { stone: "#7a5650", shape: "slab", extra: "moss" },
  CHAOS: { stone: "#4e3e4a", shape: "spike", crystal: "cluster" },
  DEATH: { stone: "#34303a", shape: "slab", extra: "bones" },
  EARTH: { stone: "#6a5a46", shape: "rough", crystal: "cluster", lift: 0.14 },
  ENCHANTMENT: { stone: "#7a7488", shape: "pillar", extra: "gold" },
  ENTROPY: { stone: "#3a3036", shape: "broken", crystal: "cluster" },
  FIRE: { stone: "#2e2a2c", shape: "spike", extra: "embers" },
  ICE: { stone: "#a8c4d0", shape: "shard", extra: "frost" },
  LAW: { stone: "#a8a090", shape: "pillar", extra: "gold" },
  LIFE: { stone: "#7a7a62", shape: "slab", extra: "flowers" },
  METAMAGIC: { stone: "#5a6670", shape: "pillar", extra: "ring" },
  MIND: { stone: "#9a8a96", shape: "slab", extra: "ring" },
  NATURE: { stone: "#5e6650", shape: "rough", extra: "moss" },
  SHADOW: { stone: "#26222c", shape: "spike" },
  SPIRIT: { stone: "#b0aab8", shape: "pillar" },
  SUN: { stone: "#b09a6a", shape: "pillar", extra: "gold", lift: 0.22 },
  WATER: { stone: "#5a7480", shape: "slab", extra: "pool" },
  DIMENSIONAL: { stone: "#4a4458", shape: "spike", extra: "ring" },
  CREATION: { stone: "#b8b0a0", shape: "pillar", extra: "gold" },
  FORCE: { stone: "#5a4a60", shape: "spike", extra: "ring" },
};

/** A floating mana crystal: an elongated double point, its facets painted in two values of the
 *  school colour (emissive light shows no shading, so the faceting is baked in). */
function manaCrystal(mb, c, len, rad, glow, { sides = 5, tilt = 0, phase = 0 } = {}) {
  const g = C(glow);
  const dark = mixl(g, "#000000", 0.45), mid = g, hot = mixl(g, "#ffffff", 0.35);
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.translate(c[0], c[1], c[2]);
    mb.rotateZ(tilt);
    const shade = (n, y, part) => {
      const s = n[0] * -0.6 + n[1] * 0.5 + n[2] * 0.6;
      return s > 0.3 ? hot : s > -0.2 ? mid : dark;
    };
    crystal(mb, [0, -len * 0.35, 0], [0, 1, 0], len, rad, sides, { tip: 0.4, foot: 0.55, phase, shade });
  });
}

/** A mana node: a ring of standing stones (shaped by the school) around a low dais, a rune circle
 *  glowing faintly in the dais, and the school's crystal hanging in the air at its heart. */
function buildManaNode(mb, r, variant, school) {
  const S = SCHOOLS[school] ?? SCHOOLS[""];
  const glow = MANA[school] ?? MANA[""];
  const g = C(glow);
  const stoneC = J(S.stone, r, 0.04);
  const st0 = stoneShade(stoneC, { moss: S.extra === "moss" || S.extra === "flowers" ? 0.7 : 0.25, seed: variant });
  // The stones' inner faces (local -z, toward the crystal) catch its light: a wash of the school's
  // colour on the faces that look in -- the ring reads as lit from within.
  const st = (n, y, a, b, c) => {
    const k = st0(n, y, a, b, c);
    const t = clamp(-n[2] * 0.9 - 0.1, 0, 1) * clamp((y + 0.02) / 0.12, 0.3, 1) * 0.42;
    return mixl(k, mixl(g, k, 0.35), t);
  };
  // The dais: a stepped round platform of fitted stone sunk in the turf, darker than the stones
  // so the inlaid ring of runes stands out on it.
  const daisC = J(mixl(S.stone, "#2e2c30", 0.55), r, 0.03);
  mb.color = daisC;
  const d0 = mb.mark();
  mb.lathe([[0.168, -0.05], [0.164, 0.012], [0.152, 0.017], [0.112, 0.017], [0.108, 0.034], [0.096, 0.038], [0, 0.038]], 14);
  mb.tint(d0, (x, y, z) => (y < 0.0 ? 0.55 : y < 0.02 ? 0.85 : 1.08) * (1 + 0.08 * Math.sin(Math.atan2(z, x) * 7)));
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    // The rune ring on the lower step (bright: it is the node's signature from afar), and a fine
    // inner circle on the upper.
    mb.color = mixl(g, "#000000", 0.5);
    const N = 14;
    for (let k = 0; k < N; k++) {
      const a0 = ((k + 0.2) / N) * TAU, a1 = ((k + 0.7) / N) * TAU;
      const R0 = 0.126, R1 = 0.138, y = 0.0175;
      mb.quad([Math.cos(a0) * R0, y, Math.sin(a0) * R0], [Math.cos(a1) * R0, y, Math.sin(a1) * R0], [Math.cos(a1) * R1, y, Math.sin(a1) * R1], [Math.cos(a0) * R1, y, Math.sin(a0) * R1]);
    }
    mb.color = mixl(g, "#000000", 0.6);
    for (let k = 0; k < 12; k++) {
      const a0 = (k / 12) * TAU, a1 = ((k + 1) / 12) * TAU;
      const R0 = 0.058, R1 = 0.066, y = 0.0385;
      mb.quad([Math.cos(a0) * R0, y, Math.sin(a0) * R0], [Math.cos(a1) * R0, y, Math.sin(a1) * R0], [Math.cos(a1) * R1, y, Math.sin(a1) * R1], [Math.cos(a0) * R1, y, Math.sin(a0) * R1]);
    }
    if (S.extra === "embers") {
      mb.color = C(T.ember);
      for (let k = 0; k < 6; k++) {
        const a = r.range(0, TAU), d = r.range(0.03, 0.09);
        at(mb, Math.cos(a) * d, d > 0.1 ? 0.018 : 0.0388, Math.sin(a) * d, a, () => mb.quad([-0.014, 0, -0.002], [0.014, 0, -0.002], [0.014, 0, 0.002], [-0.014, 0, 0.002]));
      }
    }
  });
  if (S.extra === "pool") {
    mb.part(() => {
      mb.mat = MAT.METAL;
      mb.color = C("#1a4050");
      mb.lathe([[0.054, 0.0388], [0, 0.0388]], 12, { caps: false });
    });
  } else {
    // A squat altar-stone at the heart, the crystal hanging over it.
    mb.color = J(mixl(S.stone, "#3a383c", 0.35), r, 0.03);
    const p0 = mb.mark();
    mb.lathe([[0.042, 0.03], [0.04, 0.05], [0.033, 0.066], [0.036, 0.072], [0, 0.072]], 6, { phase: r.range(0, 1) });
    mb.tint(p0, (x, y) => 0.75 + 0.35 * clamp((y - 0.03) / 0.04, 0, 1));
  }
  // The standing stones: heavy monoliths leaning in toward the crystal.
  const n = S.shape === "pillar" ? 6 : 5 + (variant === 2 ? 2 : 0);
  const R = 0.2;
  const a0 = r.range(0, TAU);
  for (let k = 0; k < n; k++) {
    const a = a0 + (k / n) * TAU + r.range(-0.12, 0.12);
    const x = Math.cos(a) * R, z = Math.sin(a) * R * 0.94;
    const h = r.range(0.18, 0.24) * (S.shape === "pillar" ? 1.08 : 1) * (k % 2 ? 0.88 : 1);
    mb.part(() => {
      mb.translate(x, 0, z);
      mb.rotateY(-a + Math.PI / 2);
      let face = -0.022, topK = 0.7, hh = h;
      if (S.shape === "rough") {
        rockMass(mb, r, { rad: 0.054, detail: 0, sx: 0.95, sy: 2.1, sz: 0.8, amp: 0.12, cuts: 3, cut: 0.62, c: [0, 0.05, 0], shade: st });
        face = -0.04;
        hh = 0.15;
      } else if (S.shape === "spike" || S.shape === "shard") {
        mb.rotateX(-0.12);
        spire(mb, r, { h: h * 1.15, rad: 0.054, sides: S.shape === "shard" ? 4 : 5, rings: 2, base: -0.05, lean: [0, -0.018], taper: 0.92, tp: 1.3, twist: r.range(-0.6, 0.6), sq: 0.75, shade: S.shape === "shard" ? (nn, y) => faceShade(mixl(S.stone, "#e8f4ff", clamp(y / h, 0, 0.6)), nn, { lit: 0.35 }) : st });
        face = -0.02;
        topK = 0.3;
      } else if (S.shape === "broken" && k % 2) {
        // A fallen stone, half buried.
        mb.translate(0, 0.012, 0);
        mb.rotateX(r.range(1.2, 1.45));
        sblock(mb, 0.058, 0.04, -0.02, h * 0.75, st, 0.85);
        return;
      } else {
        const pillar = S.shape === "pillar";
        const w = pillar ? 0.05 : 0.066, d = pillar ? 0.05 : 0.044;
        topK = pillar ? 0.85 : 0.68;
        mb.rotateX(-0.13);
        sblock(mb, w, d, -0.05, h, st, topK);
        face = -d / 2;
        if (S.extra === "gold") {
          mb.color = C("gold");
          mb.mat = MAT.METAL;
          at(mb, 0, h, 0, 0, () => mb.box(w * topK * 1.1, 0.01, d * topK * 1.1, { top: 0.6 }));
          mb.mat = MAT.MATTE;
        }
      }
      // A rune cut into the inner face, glowing: a stem and two strokes.
      mb.mat = MAT.EMISSIVE;
      mb.color = mixl(g, "#000000", 0.1);
      const zf = (y) => (face * (1 - (1 - topK) * clamp(y / hh, 0, 1))) - 0.0015;
      const y0 = hh * 0.32, y1 = hh * 0.74, ym = hh * 0.56;
      mb.quad([-0.004, y0, zf(y0)], [0.004, y0, zf(y0)], [0.004, y1, zf(y1)], [-0.004, y1, zf(y1)]);
      mb.quad([-0.013, ym - 0.012, zf(ym)], [-0.004, ym - 0.004, zf(ym)], [-0.004, ym + 0.004, zf(ym)], [-0.013, ym - 0.004, zf(ym)]);
      mb.quad([0.004, ym + 0.012, zf(ym)], [0.013, ym + 0.02, zf(ym)], [0.013, ym + 0.028, zf(ym)], [0.004, ym + 0.02, zf(ym)]);
      mb.mat = MAT.MATTE;
    });
  }
  // School extras round the ring.
  if (S.extra === "bones") {
    for (let k = 0; k < 3; k++) at(mb, Math.cos(a0 + k * 2.1 + 0.5) * 0.135, 0.028, Math.sin(a0 + k * 2.1 + 0.5) * 0.135, -(a0 + k * 2.1 + 0.5) - Math.PI / 2, () => skull(mb, 0.014));
  } else if (S.extra === "moss" || S.extra === "flowers") {
    mb.mat = MAT.FOLIAGE;
    tufts(mb, r, 4, 0.22, 0.04, ["#4e6a2a", "#5a7a30"]);
    if (S.extra === "flowers") {
      mb.mat = MAT.MATTE;
      mb.color = C("#e8e0b0");
      for (let k = 0; k < 7; k++) {
        const a = r.range(0, TAU), d = r.range(0.17, 0.24);
        at(mb, Math.cos(a) * d, 0.01, Math.sin(a) * d, 0, () => mb.sphere(0.007, 0));
      }
    }
    mb.mat = MAT.MATTE;
  } else if (S.extra === "frost") {
    mb.mat = MAT.METAL;
    for (let k = 0; k < 5; k++) {
      const a = r.range(0, TAU), d = r.range(0.11, 0.16);
      crystal(mb, [Math.cos(a) * d, 0.0, Math.sin(a) * d], [Math.cos(a) * 0.35, 1, Math.sin(a) * 0.35], r.range(0.04, 0.065), 0.011, 4, { tip: 0.4, foot: 0, shade: (nn) => faceShade("#cfe6f0", nn, { lit: 0.3 }) });
    }
    mb.mat = MAT.MATTE;
  }
  // The crystal, and its satellites, hanging over the dais.
  const lift = (S.lift ?? 0.2) + 0.05;
  const cl = S.crystal === "cluster";
  manaCrystal(mb, [0, lift, 0], cl ? 0.13 : 0.16, cl ? 0.03 : 0.036, glow, { sides: 5, phase: r.range(0, TAU) });
  const sat = cl ? 4 : 3;
  for (let k = 0; k < sat; k++) {
    const a = a0 + (k / sat) * TAU + r.range(-0.4, 0.4), d = r.range(0.07, 0.085);
    manaCrystal(mb, [Math.cos(a) * d, lift + r.range(-0.06, 0.03), Math.sin(a) * d], 0.05, 0.012, glow, { sides: 4, tilt: r.range(-0.5, 0.5) });
  }
  if (S.extra === "ring") {
    // A slow ring of light round the crystal (a thin emissive band).
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      mb.color = mixl(glow, "#000000", 0.2);
      mb.translate(0, lift, 0);
      mb.rotateX(0.35);
      const pts = [];
      for (let i = 0; i <= 16; i++) pts.push([Math.cos((i / 16) * TAU) * 0.075, 0, Math.sin((i / 16) * TAU) * 0.075]);
      mb.tube(pts, 0.003, 3);
    });
  }
}

recipe("imp.mananode", { variants: 3, build: (mb, r, { variant, param }) => buildManaNode(mb, r, variant, param ?? "") });

/** A raw mana crystal shard breaking out of the ground (the resource marker), in the school's
 *  colour; the raw node (no school) is a pale violet. */
function buildManaShard(mb, r, variant, school) {
  const glow = MANA[school] ?? MANA[""];
  const p = rockPaint("", r);
  rockMass(mb, r, { rad: 0.045, detail: 0, sx: 1.4, sy: 0.55, sz: 1.2, cuts: 3, cut: 0.6, c: [0, 0.0, 0], shade: rockShader(p, 1) });
  const n = 3 + (variant % 2);
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + r.range(-0.3, 0.3), d = k ? 0.018 : 0;
    const tilt = k ? r.range(0.35, 0.6) : r.range(0.05, 0.15);
    const len = k ? r.range(0.06, 0.085) : r.range(0.13, 0.15);
    const g = C(glow), hot = mixl(g, "#ffffff", 0.35), dark = mixl(g, "#000000", 0.45);
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      crystal(mb, [Math.cos(a) * d, -0.01, Math.sin(a) * d], [Math.cos(a) * Math.sin(tilt), Math.cos(tilt), Math.sin(a) * Math.sin(tilt)], len, k ? 0.011 : 0.018, 5, { tip: 0.35, foot: 0, phase: r.range(0, TAU), shade: (nn) => { const s2 = nn[0] * -0.6 + nn[1] * 0.5 + nn[2] * 0.6; return s2 > 0.3 ? hot : s2 > -0.2 ? g : dark; } });
    });
  }
}

markerRecipe("res.mana", 1.45, { variants: 4, build: (mb, r, { variant, param }) => buildManaShard(mb, r, variant, param ?? "") });

// ================================================================================================
// Landmarks (unique features): small dioramas with a focal point, up to ~0.9 of a plot
// ================================================================================================

/** A trilithon: two uprights and a lintel (or a lone upright, maybe fallen). */
function trilithon(mb, r, w, h, shade, { lintel = true, fallen = false } = {}) {
  if (fallen) {
    mb.part(() => {
      mb.rotateX(Math.PI / 2 - 0.08);
      mb.translate(0, 0.0, -0.012);
      sblock(mb, w * 0.42, 0.03, 0, h * 0.95, shade, 0.85);
    });
    return;
  }
  for (const sx of [-1, 1]) at(mb, (sx * w) / 2 - sx * w * 0.2, 0, 0, 0, () => sblock(mb, w * 0.36, 0.036, -0.05, h, shade, 0.86));
  if (lintel) at(mb, 0, h - 0.004, 0, 0, () => sblock(mb, w * 1.05, 0.038, 0, 0.03, shade, 0.95));
}

/** The Standing Stones: a ring of lichened sarsens, some capped with lintels, some fallen, a horseshoe
 *  of great trilithons within and an altar stone at the heart. */
function buildStandingStones(mb, r, variant) {
  const st = (k) => rockShader({ base: J("#5e5c5a", r, 0.04), top: J("#7a766e", r), coat: [T.lichen, "#8a8a6a", T.moss], cover: 0.35 }, k);
  const R = 0.34, n = 12;
  for (let k = 0; k < n; k += 2) {
    const a = (k / n) * TAU + 0.15;
    if (Math.abs(Math.sin(a)) > 0.92 && Math.sin(a) > 0) continue; // keep the front open
    at(mb, Math.cos(a) * R, 0, Math.sin(a) * R * 0.85, -a + Math.PI / 2, () => trilithon(mb, r, 0.14, r.range(0.13, 0.16), st(k), { lintel: r() < 0.6, fallen: r() < 0.15 }));
  }
  for (let k = 0; k < 3; k++) {
    const a = Math.PI * 1.5 + (k - 1) * 0.75;
    at(mb, Math.cos(a) * 0.17, 0, Math.sin(a) * 0.15, -a - Math.PI / 2, () => trilithon(mb, r, 0.15, 0.21 + (k === 1 ? 0.04 : 0), st(20 + k)));
  }
  at(mb, 0, 0, 0.02, 0.3, () => sblock(mb, 0.1, 0.05, -0.03, 0.022, st(40), 0.9));
}

recipe("lm.stones", { variants: 2, build: (mb, r, { variant }) => buildStandingStones(mb, r, variant) });

/** The Dragon Bones: a dragon's skeleton sprawled across the plot -- a horned skull, a spine
 *  arching through ribs taller than a man, the finger-bones of a wing spread like a broken fan
 *  and a tail coiling away, all half sunk in the turf. */
function buildDragonBones(mb, r, variant) {
  mb.scale(1.25);
  const m0 = mb.mark();
  mb.color = C(T.bone);
  const spine = [];
  for (let i = 0; i <= 14; i++) {
    const t = i / 14;
    spine.push([-0.38 + 0.68 * t, 0.012 + 0.07 * Math.sin(Math.PI * clamp(t * 1.25, 0, 1)), 0.08 * Math.sin(t * 3.5)]);
  }
  mb.tube(spine, spine.map((_, i) => 0.012 + 0.008 * Math.sin((i / 14) * Math.PI)), 5);
  for (let i = 1; i < spine.length - 1; i++) at(mb, ...vadd(spine[i], [0, 0.012, 0]), 0, () => mb.box(0.014, 0.026, 0.01, { top: 0.4 }));
  // Ribs: tall arches, thick at the spine.
  for (let i = 5; i < 12; i++) {
    const p = spine[i];
    const L = 0.15 * Math.sin(((i - 4) / 8) * Math.PI) + 0.04;
    for (const sz of [-1, 1]) {
      mb.tube([p, [p[0] - 0.01, p[1] + 0.07, p[2] + sz * L * 0.45], [p[0] - 0.02, p[1] + 0.04, p[2] + sz * L * 0.9], [p[0] - 0.025, -0.02, p[2] + sz * L]], [0.009, 0.0075, 0.006, 0.005], 4);
    }
  }
  // The skull.
  const sp = spine[spine.length - 1];
  at(mb, sp[0] + 0.07, 0.03, sp[2] + 0.01, Math.PI / 2 - 0.4, () => {
    mb.rotateX(0.25);
    mb.color = C(T.bone);
    mb.part(() => {
      mb.scale(1, 0.8, 1.4);
      mb.sphere(0.045, 1);
    });
    mb.tube([[0, -0.005, 0.03], [0, -0.01, 0.09], [0, -0.016, 0.12]], [0.032, 0.022, 0.012], 6, { caps: true });
    mb.tube([[0, -0.03, 0.02], [0, -0.045, 0.08], [0, -0.05, 0.11]], [0.016, 0.012, 0.007], 5, { caps: true });
    mb.color = C("#1a1412");
    for (const x of [-0.022, 0.022]) mb.quad([x - 0.01, 0.0, 0.05], [x + 0.01, 0.0, 0.05], [x + 0.008, 0.016, 0.035], [x - 0.008, 0.016, 0.035]);
    mb.color = C("#5a5048");
    for (const sx of [-1, 1]) mb.tube([[sx * 0.025, 0.025, -0.02], [sx * 0.05, 0.05, -0.07], [sx * 0.05, 0.09, -0.11], [sx * 0.04, 0.12, -0.12]], [0.011, 0.008, 0.005, 0.001], 5);
  });
  // A wing: finger bones fanning from the shoulder, one raised like a mast.
  mb.color = C(T.bone);
  const sh = spine[9];
  const elbow = [sh[0] - 0.05, 0.05, sh[2] - 0.2];
  mb.tube([sh, [sh[0] - 0.02, 0.07, sh[2] - 0.1], elbow], [0.009, 0.008, 0.007], 5);
  for (let k = 0; k < 4; k++) {
    const a = Math.PI * 1.1 + k * 0.32;
    const L = 0.22 - k * 0.025;
    const tip = [elbow[0] + Math.cos(a) * L, k === 1 ? 0.2 : 0.0, elbow[2] + Math.sin(a) * L * 0.8];
    mb.tube([elbow, vadd(vscale(elbow, 0.5), vscale(tip, 0.5)).map((v, i) => (i === 1 ? v + (k === 1 ? 0.03 : 0.02) : v)), tip], [0.0055, 0.0045, 0.002], 4);
  }
  // A few loose bones and a heap of earth under the ribs.
  bone(mb, [0.12, 0.004, 0.22], [0.25, 0.004, 0.18], 0.007);
  bone(mb, [-0.2, 0.004, 0.2], [-0.12, 0.004, 0.26], 0.006);
  mb.tint(m0, (x, y, z) => 0.62 + 0.45 * clamp(y / 0.08, 0, 1));
  mound(mb, r, 0.2, 0.12, 0.03, { grass: "#55602c", y: -0.02 });
}

recipe("lm.dragonbones", { variants: 1, build: (mb, r, { variant }) => buildDragonBones(mb, r, variant) });

/** The Maelstrom: the sea wound into a spiral -- dark water drawn round a black eye, crests of
 *  foam spiralling in, jagged rocks at the rim and a ship's broken mast caught in the pull. */
function buildMaelstrom(mb, r) {
  const prof = [];
  for (let i = 0; i <= 9; i++) {
    const t = i / 9;
    prof.push([0.46 * (1 - t) + 0.01, 0.003 + 0.004 * t]);
  }
  mb.color = C("#1e4650");
  const m0 = mb.mark();
  mb.lathe(prof, 28, { caps: false });
  mb.tint(m0, (x, y, z) => {
    const d = Math.hypot(x, z);
    const a = Math.atan2(z, x);
    const sp = Math.sin(a * 2 + Math.log(d + 0.02) * 9);
    const deep = clamp(1 - d / 0.25, 0, 1);
    const foam = sp > 0.55 ? 1.6 : 1.0;
    return [(1 - deep * 0.75) * foam, (1 - deep * 0.72) * foam, (1 - deep * 0.6) * foam];
  });
  // Foam crests spiralling inward.
  mb.color = C("#c8d4d0");
  for (const ph of [0, Math.PI]) {
    const pts = [], radii = [];
    for (let i = 0; i <= 24; i++) {
      const t = i / 24;
      const d = 0.43 * Math.pow(1 - t, 1.2) + 0.03;
      const a = ph + t * TAU * 1.4;
      pts.push([Math.cos(a) * d, 0.008 + 0.006 * (1 - t), Math.sin(a) * d]);
      radii.push(0.012 * (1 - t * 0.7));
    }
    const s0 = mb.mark();
    mb.tube(pts, radii, 4);
    mb.displace(s0, (x, y, z) => [0, Math.min(0, 0.006 - y) * 0.5, 0]);
  }
  // The eye.
  mb.color = C("#04080a");
  mb.lathe([[0.04, 0.009], [0, 0.009]], 10, { caps: false });
  // Rocks at the rim, a broken mast and planks in the pull.
  const sh = rockShader(rockPaint("basalt", r), 3);
  for (let k = 0; k < 4; k++) {
    const a = r.range(0, TAU), d = r.range(0.36, 0.44);
    spire(mb, r, { x: Math.cos(a) * d, z: Math.sin(a) * d, h: r.range(0.05, 0.1), rad: 0.025, sides: 5, rings: 1, base: -0.05, taper: 0.6, shade: sh });
  }
  mb.color = C("#3a2e24");
  stick(mb, [0.15, 0.0, 0.12], [0.1, 0.12, 0.04], 0.006, 5);
  stick(mb, [0.13, 0.08, 0.1], [0.07, 0.07, 0.12], 0.003, 4);
  for (let k = 0; k < 3; k++) at(mb, Math.cos(k * 2) * 0.2, 0.008, Math.sin(k * 2) * 0.2, k * 2 + 1.2, () => mb.box(0.045, 0.004, 0.01));
}

recipe("lm.maelstrom", { variants: 1, build: (mb, r) => buildMaelstrom(mb, r) });

/** Yggdrasil: the World Tree -- a trunk of twisting strands rising from roots that sprawl over the
 *  plot, a vast tiered crown of silver-green leaf masses, and golden fruit glowing in its shade. */
function buildYggdrasil(mb, r) {
  // The World Tree: an ash of pale, silvered bark whose roots sprawl over the plot, a bole of
  // twisting strands, and great limbs spreading level to hold its crown as separate tiers of
  // foliage -- a broad, layered, umbrella silhouette no ordinary tree has -- hung with golden fruit.
  const bark = J("#8e887a", r, 0.04);
  // Roots arching over the ground.
  mb.color = mulc(bark, 0.8);
  for (let k = 0; k < 7; k++) {
    const a = (k / 7) * TAU + r.range(-0.2, 0.2);
    const L = r.range(0.3, 0.42);
    const pts = [];
    for (let i = 0; i <= 5; i++) {
      const t = i / 5;
      pts.push([Math.cos(a + t * 0.3) * (0.06 + L * t), 0.06 * (1 - t) + 0.025 * Math.sin(t * Math.PI * 2) - 0.014 * t, Math.sin(a + t * 0.3) * (0.06 + L * t)]);
    }
    mb.tube(pts, pts.map((_, i) => 0.03 * (1 - i / 6) + 0.003), 4);
  }
  // The trunk: five strands twisting about each other into one great bole.
  const H = 0.42;
  const tr = mb.mark();
  mb.color = bark;
  for (let k = 0; k < 5; k++) {
    const pts = [], radii = [];
    for (let i = 0; i <= 7; i++) {
      const t = i / 7;
      const a = (k / 5) * TAU + t * 1.9;
      const d = 0.072 * (1 - t * 0.5) + (t > 0.78 ? (t - 0.78) * 0.6 : 0);
      pts.push([Math.cos(a) * d, -0.03 + (H + 0.04) * t, Math.sin(a) * d]);
      radii.push(0.052 * (1 - t * 0.5));
    }
    mb.tube(pts, radii, 5);
  }
  // Great limbs, each reaching out level to a tier of the crown.
  const ends = [];
  const n = 6, a0 = r.range(0, TAU);
  for (let k = 0; k < n; k++) {
    const a = a0 + (k / n) * TAU + r.range(-0.2, 0.2);
    const reach = k % 2 ? r.range(0.3, 0.37) : r.range(0.22, 0.28);
    const top = H + (k % 2 ? r.range(-0.02, 0.04) : r.range(0.08, 0.13));
    const mid = [Math.cos(a) * reach * 0.5, top - 0.035, Math.sin(a) * reach * 0.5];
    const end = [Math.cos(a) * reach, top, Math.sin(a) * reach];
    mb.tube([[0, H * 0.72 + k * 0.012, 0], mid, end], [0.034, 0.022, 0.012], 4);
    ends.push(end);
  }
  ends.push([0, H + 0.24, 0]);
  mb.tint(tr, (x, y, z) => (0.66 + 0.45 * clamp(y / H, 0, 1)) * (0.85 + 0.3 * clamp(0.5 - z * 4 - x * 2, 0, 1)));
  // The crown: a flattened cloud of leaf at each limb's end (and one over the bole): deep
  // blue-green beneath, lit gold-green on top.
  mb.mat = MAT.FOLIAGE;
  const cr = mb.mark();
  ends.forEach((c, k) => {
    const m = mb.mark();
    const top = k === ends.length - 1;
    const parts = top || k % 2 ? 3 : 2;
    for (let j = 0; j < parts; j++) {
      const b = r.range(0, TAU), d = j ? r.range(0.045, 0.07) : 0;
      const seed = r.range(0, 9);
      mb.color = J(r.pick(["#3c6250", "#466c56", "#527458"]), r, 0.05);
      at(mb, c[0] + Math.cos(b) * d, c[1] + 0.02 + r.range(-0.012, 0.012), c[2] + Math.sin(b) * d, b, () => {
        mb.scale(1.3, 1, 1.15);
        mb.blob((top ? 0.1 : 0.088) * r.range(0.9, 1.08), { detail: 1, amp: 0.24, squash: 0.46, noise: (v) => noise3(v[0] * 2.6 + seed, v[1] * 2.6, v[2] * 2.6) });
      });
    }
    mb.sphereNormals(m, [c[0], c[1] - 0.03, c[2]], 0.65);
    mb.tint(m, (x, y) => {
      const t = clamp((y - c[1] + 0.01) / 0.07, 0, 1);
      return [0.55 + 0.75 * t * t, 0.6 + 0.62 * t * t, 0.7 + 0.25 * t * t];
    });
  });
  mb.heightSway(cr, H, H + 0.36, 0.18);
  mb.mat = MAT.MATTE;
  // Golden fruit hanging under the tiers.
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#ffc850");
    for (let k = 0; k < 9; k++) {
      const c = ends[k % (ends.length - 1)];
      const b = r.range(0, TAU), d = r.range(0.03, 0.08);
      at(mb, c[0] + Math.cos(b) * d, c[1] - 0.025 + r.range(-0.01, 0.01), c[2] + Math.sin(b) * d, 0, () => mb.sphere(0.01, 0));
    }
  });
}

recipe("lm.yggdrasil", { variants: 1, build: (mb, r) => buildYggdrasil(mb, r) });

/** The Pyre of the Seraphic: a white stepped altar under four gilded pillars, a great pyre of
 *  logs in a golden bowl burning with pale holy fire. */
function buildPyre(mb, r) {
  const marble = stoneShade("#a49c8c", { moss: 0.08, seed: 8 });
  sblock(mb, 0.42, 0.38, -0.05, 0.02, marble);
  sblock(mb, 0.32, 0.28, 0.0, 0.045, marble);
  sblock(mb, 0.2, 0.18, 0.03, 0.07, marble);
  for (let k = 0; k < 6; k++) at(mb, 0, 0.0 + k * 0.012, 0.2 - k * 0.01, 0, () => sblock(mb, 0.1, 0.02, 0, 0.012, marble));
  for (const [x, z] of [[-0.13, -0.11], [0.13, -0.11], [-0.13, 0.11], [0.13, 0.11]]) {
    at(mb, x, 0.045, z, 0, () => {
      column(mb, r, 0.2, 0.013, marble, false);
      mb.color = C("gold");
      mb.mat = MAT.METAL;
      at(mb, 0, 0.21, 0, 0, () => mb.lathe([[0.016, 0], [0.012, 0.02], [0, 0.03]], 6));
      mb.mat = MAT.MATTE;
    });
  }
  // The golden bowl and the pyre.
  at(mb, 0, 0.07, 0, 0, () => {
    mb.color = C("gold");
    mb.mat = MAT.METAL;
    mb.lathe([[0.03, 0], [0.065, 0.025], [0.07, 0.03], [0.06, 0.03]], 10, { caps: false });
    mb.mat = MAT.MATTE;
    mb.color = C("#4a3424");
    for (let k = 0; k < 6; k++) {
      const a = (k / 6) * TAU;
      stick(mb, [Math.cos(a) * 0.05, 0.02, Math.sin(a) * 0.05], [-Math.cos(a) * 0.01, 0.06, -Math.sin(a) * 0.01], 0.006, 4);
    }
    mb.mat = MAT.EMISSIVE;
    for (let k = 0; k < 6; k++) {
      const a = (k / 6) * TAU, d = k ? 0.028 : 0;
      flameTongue(mb, r, Math.cos(a) * d, Math.sin(a) * d, k ? 0.2 : 0.33, k ? 0.03 : 0.042, [Math.cos(a) * d * 0.5, Math.sin(a) * d * 0.5], 1, HOLYFIRE);
    }
    mb.mat = MAT.MATTE;
  });
}

recipe("lm.pyre", { variants: 1, build: (mb, r) => buildPyre(mb, r) });

/** The Mirror of Heaven: a great round mirror of polished silver in a gold frame, raised upright on
 *  a white dais and tilted back to hold the sky (it faces the viewer and the heavens at once), a
 *  star of light at its heart; slender pillars crowned with crystals keep watch around it. */
function buildMirror(mb, r) {
  const marble = stoneShade("#b4ac9c", { moss: 0.05, seed: 2 });
  mb.color = C("#8e887c");
  const d0 = mb.mark();
  mb.lathe([[0.26, -0.05], [0.255, 0.012], [0.215, 0.016], [0.21, 0.034], [0.17, 0.038], [0, 0.038]], 16);
  mb.tint(d0, (x, y) => (y > 0.03 ? 1.15 : 0.85));
  const R = 0.12, tilt = 0.5, cy = 0.215, cz = -0.03;
  // The pedestal under it and two marble posts holding the frame at its sides.
  at(mb, 0, 0.038, cz - 0.01, 0, () => sblock(mb, 0.09, 0.06, -0.01, 0.07, marble, 0.8));
  for (const sx of [-1, 1]) at(mb, sx * (R + 0.03), 0.038, cz, 0, () => column(mb, r, cy - 0.03, 0.014, marble, false));
  mb.part(() => {
    mb.translate(0, cy, cz);
    mb.rotateX(Math.PI / 2 - tilt);
    // The gold frame (a deep ring round the glass) ...
    mb.mat = MAT.METAL;
    mb.color = C("#c8a048");
    const f0 = mb.mark();
    mb.lathe([[R * 0.85, -0.016], [R + 0.02, -0.01], [R + 0.022, 0.004], [R + 0.008, 0.01], [R, 0.005]], 20);
    mb.tint(f0, (x, y, z) => 0.8 + 0.3 * clamp(y + 0.5, 0, 1));
    // ... and the glass: the sky in it, pale at the rim, a clear deep blue at its heart.
    mb.color = C("#7c9ec4");
    const m1 = mb.mark();
    mb.lathe([[R, 0.005], [R * 0.55, 0.006], [0, 0.0065]], 20, { caps: false });
    mb.tint(m1, (x, y, z) => 0.55 + 0.6 * clamp(Math.hypot(x, z) / R, 0, 1) ** 2);
    // The star of heaven at its centre (small: the one bright thing).
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#fff2c8");
    for (const a of [0, Math.PI / 2]) {
      const c = Math.cos(a), sn = Math.sin(a);
      mb.quad([-0.032 * c, 0.008, -0.032 * sn], [0.006 * sn, 0.008, -0.006 * c], [0.032 * c, 0.008, 0.032 * sn], [-0.006 * sn, 0.008, 0.006 * c]);
    }
    mb.color = C("#c8dcf0");
    mb.lathe([[0.016, 0.0085], [0, 0.009]], 8, { caps: false });
  });
  mb.mat = MAT.MATTE;
  for (let k = 0; k < 4; k++) {
    const a = Math.PI / 4 + (k / 4) * TAU;
    at(mb, Math.cos(a) * 0.215, 0.016, Math.sin(a) * 0.215, 0, () => {
      column(mb, r, k < 2 ? 0.2 : 0.26, 0.012, marble, false);
      at(mb, 0, k < 2 ? 0.215 : 0.275, 0, 0, () => manaCrystal(mb, [0, 0.015, 0], 0.03, 0.008, "#c8e0ff", { sides: 4 }));
      mb.mat = MAT.MATTE;
    });
  }
}

recipe("lm.mirror", { variants: 1, build: (mb, r) => buildMirror(mb, r) });

/** A weeping figure: a hooded, kneeling statue bowed toward `+z`. */
function weeper(mb, shade) {
  slab(mb, [[-0.02, -0.022], [0.02, -0.022], [0.024, 0.016], [-0.024, 0.016]], -0.01, 0.045, { top: 0.6, shade });
  mb.part(() => {
    mb.translate(0, 0.045, 0.008);
    mb.rotateX(0.5);
    slab(mb, [[-0.012, -0.012], [0.012, -0.012], [0.012, 0.012], [-0.012, 0.012]], 0, 0.026, { top: 0.45, shade });
  });
}

/** The Pool of Tears: a still pool whose water glows faintly blue, ringed by hooded stone mourners
 *  kneeling at its rim; a willow weeps over it (a separate part). */
function buildPoolOfTears(mb, r) {
  pool(mb, r, { rad: 0.19, outer: "#4e5e2c", bank: "#5a5650", water: "#0e2028" });
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#1e4258");
    mb.lathe([[0.07, 0.0065], [0, 0.007]], 12, { caps: false, wobble: wobbler(r, 0.15) });
  });
  const st = stoneShade("#b4b0a8", { moss: 0.35, seed: 3 });
  for (let k = 0; k < 5; k++) {
    const a = (k / 5) * TAU + 0.6;
    at(mb, Math.cos(a) * 0.26, 0, Math.sin(a) * 0.23, -a - Math.PI / 2, () => {
      mb.scale(1.6);
      weeper(mb, st);
    });
  }
}

recipe("lm.pooloftears", { variants: 1, build: (mb, r) => buildPoolOfTears(mb, r) });

/** Bradeline's Well: an ancient well, its stones cracked, a crooked timber hood and a chain into a
 *  shaft that glows a deep, wrong violet; dead brambles and a ring of broken flagstones. */
function buildBradelinesWell(mb, r) {
  const st = stoneShade("#5e5a58", { moss: 0.5, seed: 4 });
  // Broken flagstones.
  for (let k = 0; k < 10; k++) {
    const a = (k / 10) * TAU + r.range(-0.1, 0.1), d = r.range(0.12, 0.2);
    at(mb, Math.cos(a) * d, 0, Math.sin(a) * d, a, () => slab(mb, jagPoly(r, 5, 0.03, 0.25), -0.01, 0.006 + r.range(0, 0.004), { top: 0.95, shade: st }));
  }
  // The well drum.
  const sides = 10;
  for (let k = 0; k < sides; k++) {
    const a = (k / sides) * TAU;
    at(mb, Math.cos(a) * 0.068, 0, Math.sin(a) * 0.068, -a + Math.PI / 2, () => sblock(mb, 0.044, 0.024, -0.04, 0.05 + (k % 3 === 0 ? -0.012 : r.range(0, 0.006)), st, 0.95));
  }
  mb.color = C("#0a060c");
  mb.lathe([[0.058, 0.04], [0.058, -0.02], [0, -0.02]], 10, { caps: false });
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#5a2a8a");
    mb.lathe([[0.05, 0.02], [0, 0.02]], 10, { caps: false });
  });
  // The crooked hood.
  mb.color = C("#3a2e26");
  stick(mb, [-0.07, -0.02, 0], [-0.062, 0.15, 0.004], 0.005, 4);
  stick(mb, [0.07, -0.02, 0], [0.066, 0.14, -0.004], 0.005, 4);
  stick(mb, [-0.07, 0.12, 0.0], [0.07, 0.115, 0.0], 0.0045, 5, true);
  at(mb, 0, 0.14, 0, 0.08, () => {
    mb.rotateZ(-0.08);
    roofGable(mb, 0.15, 0.07, 0.045, { col: "#3e3430", ends: null, over: 0.01, sag: 0.006 });
  });
  mb.color = C("#4a4a50");
  stick(mb, [0.005, 0.115, 0], [0.006, 0.03, 0.004], 0.0012, 3);
  // Dead brambles.
  mb.color = C("#2a2220");
  for (let k = 0; k < 6; k++) {
    const a = r.range(0, TAU), d = r.range(0.2, 0.3);
    const b = [Math.cos(a) * d, -0.01, Math.sin(a) * d];
    stick(mb, b, vadd(b, [r.range(-0.03, 0.03), r.range(0.04, 0.07), r.range(-0.03, 0.03)]), 0.002, 3);
  }
}

recipe("lm.bradeline", { variants: 1, build: (mb, r) => buildBradelinesWell(mb, r) });

/** The Broken Sepulcher: a shattered tomb on its stepped platform -- two walls left standing, the
 *  roof slab fallen against them, a sarcophagus split open and a sick green light within. */
function buildSepulcher(mb, r) {
  const st = stoneShade("#55524e", { moss: 0.45, seed: 7 });
  sblock(mb, 0.4, 0.34, -0.05, 0.02, st);
  sblock(mb, 0.3, 0.26, 0.0, 0.04, st);
  for (let k = 0; k < 4; k++) at(mb, 0, k * 0.01, 0.17 - k * 0.012, 0, () => sblock(mb, 0.12, 0.02, 0, 0.01, st));
  at(mb, -0.12, 0.04, -0.02, Math.PI / 2, () => brokenWall(mb, r, 0.22, 0.17, 0.026, st, { arch: true }));
  at(mb, 0.0, 0.04, -0.12, 0, () => brokenWall(mb, r, 0.24, 0.2, 0.026, st, { arch: true }));
  // The fallen roof slab.
  mb.part(() => {
    mb.translate(0.1, 0.04, -0.03);
    mb.rotateZ(-0.9);
    sblock(mb, 0.2, 0.16, 0, 0.018, st);
  });
  // The sarcophagus, its lid askew.
  at(mb, -0.02, 0.04, 0.03, 0.2, () => {
    sblock(mb, 0.13, 0.06, 0, 0.045, stoneShade("#6a6660", { moss: 0.1 }));
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      mb.color = C("#4a8a4a");
      mb.quad([-0.055, 0.0455, -0.022], [-0.055, 0.0455, 0.022], [0.055, 0.0455, 0.022], [0.055, 0.0455, -0.022]);
    });
    mb.part(() => {
      mb.translate(0.04, 0.05, 0.02);
      mb.rotateY(0.5);
      mb.rotateZ(0.2);
      sblock(mb, 0.12, 0.065, 0, 0.012, stoneShade("#6e6a64", { moss: 0.1 }));
    });
  });
  mb.color = C(T.bone);
  bone(mb, [0.06, 0.042, 0.1], [0.1, 0.042, 0.08], 0.003);
  at(mb, 0.09, 0.046, 0.12, -0.4, () => skull(mb, 0.01));
}

recipe("lm.sepulcher", { variants: 1, build: (mb, r) => buildSepulcher(mb, r) });

/** The Tomb of Sucellus: a domed mausoleum of pale stone behind a portico of columns, its bronze
 *  roof gone green, ivy and flowering briars about it and a warm green-gold light at the door. */
function buildSucellus(mb, r) {
  const st = stoneShade("#b0a894", { moss: 0.5, seed: 5 });
  sblock(mb, 0.36, 0.32, -0.05, 0.03, st);
  for (let k = 0; k < 3; k++) at(mb, 0, k * 0.01, 0.17 - k * 0.012, 0, () => sblock(mb, 0.16, 0.02, 0, 0.01, st));
  at(mb, 0, 0.03, -0.04, 0, () => {
    sblock(mb, 0.2, 0.16, 0, 0.13, st);
    mb.color = C("#4b7a68");
    mb.mat = MAT.METAL;
    const d0 = mb.mark();
    mb.lathe([[0.09, 0.13], [0.088, 0.17], [0.06, 0.21], [0.02, 0.228], [0, 0.23]], 10);
    mb.tint(d0, (x, y) => 0.7 + 0.4 * clamp((y - 0.13) / 0.1, 0, 1));
    mb.mat = MAT.MATTE;
    plate(mb, -0.022, 0.0, 0.022, 0.075, 0.081, "#1e1a14");
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      plate(mb, -0.016, 0.0, 0.016, 0.06, 0.082, "#a8c860");
    });
  });
  // Portico: four columns and a pediment.
  for (const x of [-0.075, -0.025, 0.025, 0.075]) at(mb, x, 0.03, 0.08, 0, () => column(mb, r, 0.11, 0.009, st, false));
  at(mb, 0, 0.15, 0.06, 0, () => {
    sblock(mb, 0.19, 0.06, -0.008, 0.008, st);
    roofGable(mb, 0.19, 0.06, 0.035, { col: "#4b7a68", ends: "#b0a894", over: 0.006 });
  });
  // Flowering briars.
  mb.mat = MAT.FOLIAGE;
  for (let k = 0; k < 5; k++) {
    const a = r.range(0, TAU), d = r.range(0.2, 0.28);
    const seed = r.range(0, 9);
    mb.color = J("#3e5a2e", r, 0.08);
    const b0 = mb.mark();
    at(mb, Math.cos(a) * d, 0.0, Math.sin(a) * d, 0, () => mb.blob(0.035, { detail: 0, amp: 0.25, squash: 0.7, noise: (v) => noise3(v[0] * 3 + seed, v[1] * 3, v[2] * 3) }));
    mb.sphereNormals(b0, [Math.cos(a) * d, -0.02, Math.sin(a) * d], 0.6);
    mb.mat = MAT.MATTE;
    mb.color = C(k % 2 ? "#e8d8b0" : "#c86a7a");
    for (let l = 0; l < 3; l++) at(mb, Math.cos(a) * d + r.range(-0.02, 0.02), 0.022, Math.sin(a) * d + r.range(-0.02, 0.02), 0, () => mb.sphere(0.005, 0));
    mb.mat = MAT.FOLIAGE;
  }
  mb.mat = MAT.MATTE;
}

recipe("lm.sucellus", { variants: 1, build: (mb, r) => buildSucellus(mb, r) });

/** Letum Frigus: a place the cold has taken -- a crown of glassy ice spires around a frozen pool,
 *  drifts of snow and a ruined arch locked in the ice. */
function buildLetumFrigus(mb, r) {
  // Snow drifts.
  mb.color = C(T.snow);
  for (let k = 0; k < 4; k++) {
    const a = (k / 4) * TAU + r.range(-0.3, 0.3), d = r.range(0.22, 0.32);
    const seed = r.range(0, 9);
    const s0 = mb.mark();
    at(mb, Math.cos(a) * d, -0.012, Math.sin(a) * d, a + Math.PI / 2, () => { mb.scale(2.2, 1, 1.2); mb.blob(0.07, { detail: 1, amp: 0.3, squash: 0.22, noise: (v) => noise3(v[0] * 2 + seed, v[1] * 2, v[2] * 2) }); });
    mb.tint(s0, (x, y) => 0.8 + 0.25 * clamp(y / 0.02, 0, 1));
  }
  pool(mb, r, { rad: 0.12, outer: "#d8e2ea", bank: "#a8bccc", water: "#9cc4d8" });
  // Ice spires.
  const ice = (n, y, part) => {
    const s2 = n[0] * -0.78 + n[1] * 0.46 + n[2] * 0.42;
    return mixl(s2 > 0.2 ? "#d8f0fa" : "#6aa0c0", "#ffffff", part === 1 ? 0.3 : 0);
  };
  mb.mat = MAT.METAL;
  for (let k = 0; k < 9; k++) {
    const a = (k / 9) * TAU + r.range(-0.2, 0.2), d = r.range(0.14, 0.2);
    const len = r.range(0.12, 0.26) * (Math.sin(a) < 0 ? 1.2 : 0.7);
    crystal(mb, [Math.cos(a) * d, -0.03, Math.sin(a) * d], [Math.cos(a) * 0.25, 1, Math.sin(a) * 0.25], len, r.range(0.018, 0.03), 5, { tip: 0.45, foot: 0, phase: r.range(0, TAU), shade: ice });
  }
  mb.mat = MAT.EMISSIVE;
  mb.color = C("#7ab8e0");
  crystal(mb, [0, 0.0, -0.02], [0, 1, 0], 0.3, 0.03, 6, { tip: 0.4, foot: 0, shade: (n) => (n[0] * -0.78 + n[2] * 0.42 > 0 ? C("#a8dcf8") : C("#4a88b8")) });
  mb.mat = MAT.MATTE;
  // A ruined arch in the ice.
  const st = stoneShade("#8a8e96", { moss: 0, seed: 1 });
  at(mb, 0.18, 0, 0.14, -0.6, () => {
    for (const x of [-0.035, 0.035]) at(mb, x, 0, 0, 0, () => sblock(mb, 0.016, 0.016, -0.03, 0.08, st));
    at(mb, -0.01, 0.08, 0, 0.0, () => sblock(mb, 0.07, 0.018, 0, 0.014, st));
  });
}

recipe("lm.letum", { variants: 1, build: (mb, r) => buildLetumFrigus(mb, r) });

/** The Remnants of Patria: the bones of a great city -- a colonnade with its architrave, a
 *  triumphal arch, a colossal crowned head toppled in the grass and a broken stair. */
function buildPatria(mb, r) {
  const st = stoneShade("#968a72", { moss: 0.45, seed: 6 });
  sblock(mb, 0.5, 0.12, -0.05, 0.02, st);
  for (let k = 0; k < 6; k++) {
    const x = -0.22 + k * 0.088;
    const broken = k === 1 || k === 4 || k === 5;
    at(mb, x, 0.02, -0.12, 0, () => column(mb, r, broken ? r.range(0.06, 0.12) : 0.2, 0.014, st, broken));
  }
  at(mb, -0.13, 0.22, -0.12, 0, () => sblock(mb, 0.21, 0.04, 0, 0.022, st));
  // The arch.
  at(mb, 0.2, 0, 0.1, -0.5, () => {
    for (const x of [-0.05, 0.05]) at(mb, x, 0, 0, 0, () => sblock(mb, 0.03, 0.04, -0.04, 0.13, st));
    const pts = [];
    for (let i = 0; i <= 8; i++) {
      const a = Math.PI * (i / 8);
      pts.push([-Math.cos(a) * 0.05, 0.13 + Math.sin(a) * 0.045, 0]);
    }
    mb.color = C("#8a7e66");
    mb.tube(pts, 0.016, 4);
    at(mb, 0, 0.17, 0, 0, () => sblock(mb, 0.14, 0.045, 0, 0.03, st));
  });
  // The fallen head, crowned, half sunk.
  at(mb, -0.14, 0.03, 0.12, 0.9, () => {
    mb.rotateZ(1.1);
    mb.color = C("#8e8270");
    mb.part(() => {
      mb.scale(0.85, 1.0, 0.9);
      mb.sphere(0.07, 1);
    });
    mb.tube([[0, 0.0, 0.06], [0, -0.02, 0.078]], [0.016, 0.01], 4, { caps: true });
    mb.color = C("#5a5040");
    for (const x of [-0.024, 0.024]) mb.quad([x - 0.012, 0.012, 0.062], [x + 0.012, 0.012, 0.062], [x + 0.012, 0.022, 0.058], [x - 0.012, 0.022, 0.058]);
    mb.color = C("bronze");
    mb.mat = MAT.METAL;
    for (let k = 0; k < 7; k++) {
      const a = (k / 7) * Math.PI - Math.PI;
      mb.tri([Math.cos(a) * 0.06, 0.05, Math.sin(a) * 0.06], [Math.cos(a + 0.45) * 0.06, 0.05, Math.sin(a + 0.45) * 0.06], [Math.cos(a + 0.22) * 0.065, 0.085, Math.sin(a + 0.22) * 0.065]);
    }
    mb.mat = MAT.MATTE;
  });
  // A broken stair and scattered drums.
  for (let k = 0; k < 4; k++) at(mb, 0.0, k * 0.012, 0.08 - k * 0.016, 0, () => sblock(mb, 0.12 - k * 0.02, 0.02, -0.02, 0.012, st));
  for (let k = 0; k < 3; k++) at(mb, r.range(-0.2, 0.2), 0.012, r.range(0.12, 0.2), r.range(0, TAU), () => { mb.color = C("#a89c80"); mb.rotateZ(Math.PI / 2); mb.translate(0, -0.02, 0); mb.cylinder(0.014, 0.014, 0.04, 7); });
}

recipe("lm.patria", { variants: 1, build: (mb, r) => buildPatria(mb, r) });

/** Aifon Isle: a rocky islet crowned with grass and a round white temple of columns under a
 *  shallow dome. */
function buildAifon(mb, r) {
  const sh = rockShader(rockPaint("moss", r), 5);
  rockMass(mb, r, { rad: 0.26, sx: 1.2, sy: 0.38, sz: 1.0, amp: 0.15, cuts: 4, cut: 0.7, c: [0, -0.02, 0], floor: -0.12, shade: sh });
  for (let k = 0; k < 4; k++) {
    const a = r.range(0, TAU);
    rockMass(mb, r, { rad: r.range(0.04, 0.07), detail: 0, sy: 0.8, cuts: 3, cut: 0.6, c: [Math.cos(a) * 0.3, -0.01, Math.sin(a) * 0.25], floor: -0.1, shade: sh });
  }
  const marble = stoneShade("#d8d0c0", { moss: 0.1, seed: 3 });
  at(mb, 0, 0.06, -0.02, 0, () => {
    mb.color = C("#c8c0b0");
    mb.lathe([[0.1, -0.03], [0.1, 0.012], [0.09, 0.02], [0, 0.02]], 10);
    for (let k = 0; k < 8; k++) {
      const a = (k / 8) * TAU;
      at(mb, Math.cos(a) * 0.075, 0.02, Math.sin(a) * 0.075, 0, () => column(mb, r, 0.1, 0.008, marble, false));
    }
    mb.color = C("#c8c0b0");
    mb.lathe([[0.088, 0.125], [0.088, 0.135], [0.07, 0.15], [0.03, 0.165], [0, 0.168]], 10);
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      mb.color = C("#ffd890");
      at(mb, 0, 0.02, 0, 0, () => mb.lathe([[0.012, 0], [0.008, 0.03], [0, 0.034]], 6));
    });
  });
}

recipe("lm.aifon", { variants: 1, build: (mb, r) => buildAifon(mb, r) });

/** Odio's Prison: a black monolith carved with burning runes, chained to iron stakes driven in a
 *  ring around it, on scorched ground. */
function buildOdiosPrison(mb, r, variant, carcer = false) {
  mb.color = C("#2a2422");
  mb.lathe([[0.36, -0.03], [0.3, 0.004], [0, 0.006]], 14, { caps: false, wobble: wobbler(r, 0.1) });
  const obs = (n) => faceShade("#16121a", n, { lit: 0.6 });
  mb.mat = MAT.METAL;
  if (!carcer) {
    slab(mb, [[-0.065, -0.05], [0.065, -0.05], [0.072, 0.05], [-0.072, 0.05]], -0.05, 0.42, { top: 0.6, shade: obs });
    at(mb, 0, 0.42, 0, 0, () => slab(mb, [[-0.039, -0.03], [0.039, -0.03], [0.043, 0.03], [-0.043, 0.03]], 0, 0.06, { top: 0.1, shade: obs }));
  }
  mb.mat = MAT.MATTE;
  // Runes.
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#c8301a");
    if (!carcer) {
      for (let k = 0; k < 5; k++) {
        const y = 0.07 + k * 0.065, w = 0.04 - k * 0.005, z = 0.0505 - (k * 0.065 + 0.08) * 0.0465;
        mb.quad([-w * 0.5, y, z + 0.0035], [w * 0.5, y + 0.005, z + 0.0035], [w * 0.5, y + 0.02, z + 0.0028], [-w * 0.5, y + 0.015, z + 0.0028]);
      }
    } else {
      // A seal of runes round a smouldering pit (a ring, not a glowing plate).
      mb.color = C("#a02a14");
      mb.lathe([[0.105, 0.008], [0.088, 0.0085]], 14, { caps: false });
      mb.color = C("#3e0e08");
      mb.lathe([[0.088, 0.0085], [0, 0.009]], 14, { caps: false });
    }
  });
  // The ring of stakes (iron) or of black pillars (Carcer), chained to the heart.
  const n = carcer ? 8 : 6;
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + 0.2;
    const R = carcer ? 0.28 : 0.24;
    const p = [Math.cos(a) * R, 0, Math.sin(a) * R * 0.9];
    if (carcer) {
      mb.mat = MAT.METAL;
      at(mb, p[0], 0, p[2], -a, () => slab(mb, [[-0.02, -0.02], [0.02, -0.02], [0.02, 0.02], [-0.02, 0.02]], -0.05, 0.2, { top: 0.7, shade: obs }));
      mb.mat = MAT.MATTE;
      mb.color = C("#4a4648");
      at(mb, p[0], 0.06, p[2], 0, () => mb.box(0.05, 0.012, 0.05));
      at(mb, p[0], 0.15, p[2], 0, () => mb.box(0.046, 0.012, 0.046));
    } else {
      mb.color = C("#3a3638");
      stick(mb, vadd(p, [0, -0.03, 0]), vadd(p, [0, 0.07, 0]), 0.006, 4);
    }
    mb.color = C("#4e4a4e");
    mb.mat = MAT.METAL;
    const top = carcer ? 0.17 : 0.06;
    const q = carcer ? [Math.cos(a + TAU / n) * R, 0.17, Math.sin(a + TAU / n) * R * 0.9] : [Math.cos(a) * 0.045, 0.3, Math.sin(a) * 0.035];
    const mid = vadd(vscale(vadd(vadd(p, [0, top, 0]), q), 0.5), [0, carcer ? -0.04 : -0.03, 0]);
    mb.tube([vadd(p, [0, top, 0]), mid, q], 0.0025, 3);
    mb.mat = MAT.MATTE;
  }
}

recipe("lm.odio", { variants: 1, build: (mb, r, { variant }) => buildOdiosPrison(mb, r, variant) });
recipe("lm.carcer", { variants: 1, build: (mb, r, { variant }) => buildOdiosPrison(mb, r, variant, true) });

/** The altar of the Seven Pines: a mossy table stone, a green rune glowing faintly on its face. */
function buildPineAltar(mb, r) {
  const st = stoneShade("#6a6a5e", { moss: 0.6, seed: 2 });
  for (const x of [-0.04, 0.04]) at(mb, x, 0, 0, 0, () => sblock(mb, 0.024, 0.04, -0.04, 0.04, st));
  at(mb, 0, 0.04, 0, 0.05, () => sblock(mb, 0.13, 0.07, 0, 0.02, st, 0.95));
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#5a9a50");
    plate(mb, -0.01, 0.045, 0.01, 0.055, 0.0362, "#5a9a50");
  });
  mb.mat = MAT.FOLIAGE;
  tufts(mb, r, 5, 0.12, 0.04, [T.grass, T.moss]);
  mb.mat = MAT.MATTE;
}

recipe("lm.pinealtar", { variants: 1, build: (mb, r) => buildPineAltar(mb, r) });

/** A dignified generic monument: an obelisk on a stepped plinth (0), a robed statue (1) or a
 *  carved stele between two braziers (2). */
function buildMonument(mb, r, variant) {
  const st = stoneShade(J(variant === 1 ? "#a09a8c" : "#7a766e", r, 0.04), { moss: 0.4, seed: variant });
  sblock(mb, 0.2, 0.2, -0.05, 0.02, st);
  sblock(mb, 0.14, 0.14, 0.0, 0.045, st);
  if (variant === 0) {
    at(mb, 0, 0.045, 0, 0, () => {
      sblock(mb, 0.05, 0.05, 0, 0.26, st, 0.62);
      at(mb, 0, 0.26, 0, 0, () => slab(mb, [[-0.0155, -0.0155], [0.0155, -0.0155], [0.0155, 0.0155], [-0.0155, 0.0155]], 0, 0.03, { top: 0.05, shade: st }));
      mb.part(() => {
        mb.mat = MAT.EMISSIVE;
        plate(mb, -0.006, 0.12, 0.006, 0.17, 0.0222, "#d8c890");
      });
    });
  } else if (variant === 1) {
    at(mb, 0, 0.045, 0, 0, () => {
      slab(mb, [[-0.035, -0.025], [0.035, -0.025], [0.04, 0.025], [-0.04, 0.025]], 0, 0.18, { top: 0.62, shade: st });
      at(mb, 0, 0.18, 0, 0, () => sblock(mb, 0.06, 0.035, 0, 0.018, st, 0.8));
      at(mb, 0, 0.198, 0.0, 0, () => { mb.color = C("#a09a8c"); mb.sphere(0.018, 1, { squash: 1.15 }); });
      mb.color = C("#8a8478");
      stick(mb, [0.04, 0.0, 0.03], [0.045, 0.25, 0.03], 0.005, 4);
    });
  } else {
    at(mb, 0, 0.045, 0, 0, () => {
      sblock(mb, 0.09, 0.03, 0, 0.2, st, 0.9);
      at(mb, 0, 0.2, 0, 0, () => slab(mb, [[-0.04, -0.014], [0.04, -0.014], [0.04, 0.014], [-0.04, 0.014]], 0, 0.03, { top: 0.3, shade: st }));
      mb.part(() => {
        mb.mat = MAT.EMISSIVE;
        plate(mb, -0.02, 0.1, 0.02, 0.105, 0.0152, "#c8b080");
        plate(mb, -0.015, 0.12, 0.015, 0.125, 0.0152, "#c8b080");
      });
    });
    for (const x of [-0.08, 0.08]) {
      at(mb, x, 0.045, 0.05, 0, () => {
        mb.color = C("#3a3634");
        mb.lathe([[0.006, 0], [0.004, 0.035], [0.012, 0.045], [0.01, 0.05]], 6);
        mb.mat = MAT.EMISSIVE;
        flameTongue(mb, r, 0, 0, 0.03, 0.007, [0, 0], 1);
        mb.mat = MAT.MATTE;
      });
    }
  }
}

recipe("lm.monument", { variants: 3, build: (mb, r, { variant }) => buildMonument(mb, r, variant) });

// ================================================================================================
// Resource markers (~0.12-0.2 of a plot, at the front-left of the plot)
// ================================================================================================

/** Ore: a broken boulder shot through with veins of the metal (param), a few bright nuggets or
 *  crystals breaking out of it. */
function buildOre(mb, r, variant, metal) {
  const o = ORES[metal] ?? ORES.iron;
  const vein = C(o[1]);
  const base = J(o[0], r, 0.04);
  const seed = r.range(0, 9);
  const metallic = o[2];
  // Lodes: broad bands of the metal across the broken faces, so the split rock shows its ore from
  // afar; the bands (and the nuggets) take the metal's sheen, the rock stays matte.
  const sh = (n, y, a, b, c) => {
    const cx = (a[0] + b[0] + c[0]) / 3, cz = (a[2] + b[2] + c[2]) / 3;
    const band = Math.sin((cx * 0.8 + y * 1.5 + cz * 0.5) * 105 + seed + 2.2 * noise3(cx * 22 + seed, y * 22, cz * 22));
    const lode = band > 0.15 - 0.5 * clamp(n[1] - 0.4, 0, 1) && n[1] > -0.35;
    mb.mat = lode && metallic ? MAT.METAL : MAT.MATTE;
    return lode ? faceShade(vein, n, { top: mixl(vein, "#ffffff", 0.06), lit: 0.3 }) : faceShade(base, n, { top: mixl(base, "#6a6460", 0.3), lit: 0.3 });
  };
  rockMass(mb, r, { rad: 0.062, sx: 1.25, sy: 0.95, sz: 1.0, cuts: 5, cut: 0.6, c: [0, 0.016, 0], floor: -0.045, shade: sh });
  rockMass(mb, r, { rad: 0.036, detail: 0, sx: 1.1, sy: 0.9, cuts: 3, cut: 0.6, c: [0.07, 0.004, 0.03], floor: -0.045, shade: sh });
  // Nuggets / crystals of the metal broken out at the foot and on the shoulders.
  mb.mat = metallic ? MAT.METAL : MAT.MATTE;
  const shine = mixl(vein, "#ffffff", 0.25);
  for (let k = 0; k < 4; k++) {
    const a = r.range(-1.3, 1.3) + Math.PI / 2, d = r.range(0.045, 0.075);
    const p0 = [Math.cos(a) * d, 0.012 + r.range(0, 0.035), Math.sin(a) * d * 0.8];
    if (metal === "mithril" || metal === "silver") crystal(mb, p0, [Math.cos(a) * 0.5, 1, Math.sin(a) * 0.5], r.range(0.035, 0.055), 0.01, 5, { tip: 0.4, foot: 0, shade: (n) => faceShade(shine, n, { lit: 0.4 }) });
    else rockMass(mb, r, { rad: r.range(0.012, 0.017), detail: 0, sy: 0.8, cuts: 2, cut: 0.6, c: p0, floor: -0.1, shade: (n) => faceShade(shine, n, { lit: 0.4 }) });
  }
  mb.mat = MAT.MATTE;
  if (metal === "mithril") {
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      mb.color = C("#5a8aa0");
      crystal(mb, [0.0, 0.05, 0.03], [0.1, 1, 0.3], 0.05, 0.008, 4, { tip: 0.4, foot: 0 });
    });
  }
  groundAO(mb, 0, -0.02, 0.06, 0.6);
}

markerRecipe("res.ore", 1.4, { variants: 4, build: (mb, r, { variant, param }) => buildOre(mb, r, variant, param || "iron") });

/** Gems: a cluster of coloured crystals breaking out of dark rock. */
function buildGems(mb, r, variant, kind) {
  const cols = kind === "jewels" ? ["#c03a6a", "#3a9a6a", "#3a5ac0", "#e0b040"] : [["#b8304a", "#d85070"], ["#2e8a5a", "#50b07a"], ["#3050b0", "#5a7ad8"], ["#8a40b0", "#b070d8"]][variant % 4];
  rockMass(mb, r, { rad: 0.055, detail: 0, sx: 1.3, sy: 0.55, sz: 1.1, cuts: 3, cut: 0.6, c: [0, 0.0, 0], shade: rockShader(rockPaint("basalt", r), 1) });
  mb.mat = MAT.METAL;
  const n = 6;
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + r.range(-0.3, 0.3), d = k ? r.range(0.015, 0.035) : 0;
    const tilt = k ? r.range(0.3, 0.7) : 0.1;
    const c = Array.isArray(cols[0]) ? cols : cols;
    const col = kind === "jewels" ? cols[k % cols.length] : cols[k % 2];
    const lit = mixl(col, "#ffffff", 0.15), dk = mixl(col, "#000000", 0.45);
    crystal(mb, [Math.cos(a) * d, 0.0, Math.sin(a) * d], [Math.cos(a) * Math.sin(tilt), Math.cos(tilt), Math.sin(a) * Math.sin(tilt)], k ? r.range(0.055, 0.08) : 0.115, k ? 0.013 : 0.019, 6, { tip: 0.32, foot: 0, phase: r.range(0, TAU), shade: (nn) => { const s2 = nn[0] * -0.78 + nn[1] * 0.46 + nn[2] * 0.42; return s2 > 0.3 ? lit : s2 > -0.1 ? C(col) : dk; } });
    void c;
  }
  mb.mat = MAT.MATTE;
}

markerRecipe("res.gems", 1.5, { variants: 4, build: (mb, r, { variant, param }) => buildGems(mb, r, variant, param) });

/** Marble: cut white blocks stacked by a raw, veined outcrop. */
function buildMarble(mb, r, variant) {
  const white = J("#cfc8ba", r, 0.03);
  const sh = (n, y, a) => {
    const v = Math.sin(a[0] * 120 + a[2] * 80 + y * 60) > 0.92 ? 0.75 : 1;
    const k = faceShade(white, n, { top: "#e0dacc", lit: 0.3 });
    return [k[0] * v, k[1] * v, k[2] * v];
  };
  rockMass(mb, r, { rad: 0.055, sx: 1.2, sy: 0.9, sz: 1.0, cuts: 6, cut: 0.55, c: [-0.03, 0.012, -0.02], shade: sh });
  for (const [x, z, y, yaw] of [[0.05, 0.03, 0, 0.2], [0.035, 0.065, 0, -0.1], [0.045, 0.045, 0.032, 0.5]]) at(mb, x, y, z, yaw, () => sblock(mb, 0.04, 0.03, y ? 0 : -0.01, 0.032, sh));
}

markerRecipe("res.marble", 1.4, { variants: 2, build: (mb, r, { variant }) => buildMarble(mb, r, variant) });

/** A stook of wheat: sheaves leaned together in a little tent, their ears flaring golden at the
 *  top, and a sheaf lying in the stubble. */
function buildWheat(mb, r, variant) {
  const sheaf = (base, top, w) => {
    const d = vsub(top, base), L = Math.hypot(...d);
    const pts = [base, vadd(base, vscale(d, 0.45)), vadd(base, vscale(d, 0.6)), vadd(base, vscale(d, 0.85)), top];
    const m0 = mb.mark();
    mb.color = J("#b4964a", r, 0.05);
    mb.tube(pts, [w, w * 0.75, w * 0.7, w * 1.25, w * 0.5], 5, { caps: true, colors: ["#8a7238", "#9a8040", "#b89a4a", "#d0b45c"] });
    void L;
    void m0;
  };
  const n = 4 + (variant % 2);
  const apex = [0, 0.075, 0];
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + r.range(-0.2, 0.2);
    sheaf([Math.cos(a) * 0.03, -0.006, Math.sin(a) * 0.03], vadd(apex, [Math.cos(a) * 0.008, r.range(0, 0.01), Math.sin(a) * 0.008]), 0.011);
  }
  const a = r.range(0, TAU);
  sheaf([0.04 + Math.cos(a) * 0.03, 0.006, 0.03 + Math.sin(a) * 0.03], [0.04 - Math.cos(a) * 0.03, 0.008, 0.03 - Math.sin(a) * 0.03], 0.009);
  mb.mat = MAT.FOLIAGE;
  tufts(mb, r, 3, 0.06, 0.025, ["#a89448", "#8a7a3e"]);
  mb.mat = MAT.MATTE;
}

markerRecipe("res.wheat", 1.8, { variants: 2, build: (mb, r, { variant }) => buildWheat(mb, r, variant) });

/** Corn: a few tall stalks with drooping leaves and yellow cobs. */
function buildCorn(mb, r) {
  // Stalks, tassels and leaves are blades (drawn as lines, not massed like a crown).
  mb.mat = MAT.BLADE;
  for (let k = 0; k < 5; k++) {
    const a = (k / 5) * TAU + r.range(-0.3, 0.3), d = k ? r.range(0.025, 0.04) : 0;
    const x = Math.cos(a) * d, z = Math.sin(a) * d, h = r.range(0.11, 0.14);
    const m0 = mb.mark();
    mb.color = J("#7e9a3c", r, 0.06);
    stick(mb, [x, -0.01, z], [x + r.range(-0.006, 0.006), h, z], 0.0035, 3);
    // A pale tassel at the top.
    mb.color = J("#c8b060", r, 0.06);
    mb.tri([x - 0.004, h, z], [x + 0.004, h, z], [x + r.range(-0.006, 0.006), h + 0.022, z + r.range(-0.006, 0.006)]);
    mb.color = J("#7e9a3c", r, 0.06);
    for (let l = 0; l < 3; l++) {
      const b = r.range(0, TAU), y = h * (0.35 + l * 0.2);
      mb.tri([x, y, z], [x + Math.cos(b) * 0.04, y - 0.02, z + Math.sin(b) * 0.04], [x + Math.cos(b + 0.3) * 0.02, y + 0.008, z + Math.sin(b + 0.3) * 0.02]);
    }
    mb.heightSway(m0, 0.03, h, 0.5);
    mb.mat = MAT.MATTE;
    mb.color = C("#ecc040");
    at(mb, x + 0.006, h * 0.5, z + 0.004, 0, () => { mb.rotateZ(-0.45); mb.lathe([[0.0075, 0], [0.009, 0.016], [0, 0.034]], 5); });
    mb.mat = MAT.BLADE;
  }
  mb.mat = MAT.MATTE;
}

markerRecipe("res.corn", 1.45, { variants: 2, build: (mb, r) => buildCorn(mb, r) });

/** Rice: a little flooded paddy in a mud bank, green shoots in rows. */
function buildRice(mb, r) {
  // A flooded paddy in its earth bank (the sky in the water), shoots of vivid young green in rows.
  pool(mb, r, { rad: 0.062, water: "#3e6a6e", outer: "#6a6a36", bank: "#5a4834" });
  mb.mat = MAT.BLADE;
  const m0 = mb.mark();
  for (let j = 0; j < 4; j++) {
    for (let k = 0; k < 5; k++) {
      const x = -0.044 + k * 0.022 + r.range(-0.003, 0.003), z = -0.033 + j * 0.022;
      if (Math.hypot(x / 1.0, z / 0.9) > 0.058) continue;
      mb.color = J(j % 2 ? "#86a83c" : "#7a9e36", r, 0.08);
      for (let l = 0; l < 3; l++) {
        const b = r.range(0, TAU);
        mb.tri([x - 0.003, 0.004, z], [x + 0.003, 0.004, z], [x + Math.cos(b) * 0.009, 0.034, z + Math.sin(b) * 0.009]);
      }
    }
  }
  mb.heightSway(m0, 0.006, 0.034, 0.5);
  mb.mat = MAT.MATTE;
}

markerRecipe("res.rice", 1.35, { variants: 1, build: (mb, r) => buildRice(mb, r) });

/** A banana plant with a heavy yellow bunch. */
function buildBanana(mb, r) {
  mb.color = C("#6e7040");
  mb.tube([[0, -0.01, 0], [0.004, 0.06, 0], [0.002, 0.1, 0.002]], [0.011, 0.009, 0.007], 5);
  mb.mat = MAT.FOLIAGE;
  const m0 = mb.mark();
  for (let k = 0; k < 6; k++) {
    const a = (k / 6) * TAU + r.range(-0.2, 0.2);
    const tip = [Math.cos(a) * 0.08, 0.06, Math.sin(a) * 0.08];
    const mid = [Math.cos(a) * 0.04, 0.115, Math.sin(a) * 0.04];
    const side = [-Math.sin(a) * 0.016, 0, Math.cos(a) * 0.016];
    mb.color = J(k % 2 ? "#5a7a2e" : "#6e8e36", r, 0.06);
    mb.quad([0, 0.1, 0], vadd(mid, side), tip, vsub(mid, side));
  }
  mb.heightSway(m0, 0.06, 0.12, 0.5);
  mb.mat = MAT.MATTE;
  mb.color = C("#d8b830");
  at(mb, 0.02, 0.06, 0.018, 0, () => mb.blob(0.016, { detail: 0, amp: 0.25, squash: 1.4, noise: (v) => Math.sin(v[1] * 9) }));
}

markerRecipe("res.banana", 1.45, { variants: 2, build: (mb, r) => buildBanana(mb, r) });

/** Sugar cane: a stand of tall jointed canes, cut stalks bundled at its foot. */
function buildSugar(mb, r) {
  mb.mat = MAT.BLADE;
  for (let k = 0; k < 8; k++) {
    const x = r.range(-0.035, 0.035), z = r.range(-0.03, 0.03), h = r.range(0.11, 0.15);
    const m0 = mb.mark();
    mb.color = J("#9aa04a", r, 0.08);
    stick(mb, [x, -0.01, z], [x + r.range(-0.01, 0.01), h, z + r.range(-0.01, 0.01)], 0.0028, 3);
    mb.color = J("#5e7a30", r, 0.08);
    const b = r.range(0, TAU);
    mb.tri([x, h, z], [x + Math.cos(b) * 0.03, h - 0.03, z + Math.sin(b) * 0.03], [x + Math.cos(b + 0.4) * 0.012, h - 0.01, z + Math.sin(b + 0.4) * 0.012]);
    mb.heightSway(m0, 0.03, h, 0.5);
  }
  mb.mat = MAT.MATTE;
  mb.color = C("#a8a050");
  for (let k = 0; k < 4; k++) stick(mb, [0.03 + k * 0.006, 0.006, 0.045], [-0.03 + k * 0.006, 0.006, 0.065], 0.003, 3);
}

markerRecipe("res.sugar", 1.45, { variants: 2, build: (mb, r) => buildSugar(mb, r) });

/** Cotton: bushes starred with white bolls, a bale beside them. */
function buildCotton(mb, r) {
  mb.mat = MAT.FOLIAGE;
  for (let k = 0; k < 3; k++) {
    const a = (k / 3) * TAU, d = k ? 0.04 : 0;
    const seed = r.range(0, 9);
    const m0 = mb.mark();
    mb.color = J("#4a5a30", r, 0.06);
    at(mb, Math.cos(a) * d, 0.02, Math.sin(a) * d, 0, () => mb.blob(0.028, { detail: 0, amp: 0.25, squash: 0.85, noise: (v) => noise3(v[0] * 3 + seed, v[1] * 3, v[2] * 3) }));
    mb.sphereNormals(m0, [Math.cos(a) * d, 0, Math.sin(a) * d], 0.6);
    mb.mat = MAT.MATTE;
    mb.color = C("#eee8dc");
    for (let l = 0; l < 3; l++) at(mb, Math.cos(a) * d + r.range(-0.02, 0.02), 0.03 + r.range(0, 0.016), Math.sin(a) * d + r.range(-0.018, 0.022), 0, () => mb.sphere(0.0075, 0));
    mb.mat = MAT.FOLIAGE;
  }
  mb.mat = MAT.MATTE;
  at(mb, 0.06, 0, 0.05, 0.3, () => bale(mb, "#e0d8c8"));
}

/** A corded bale of cloth or fibre. */
function bale(mb, col, s = 1) {
  mb.part(() => {
    mb.scale(s);
    block(mb, 0.04, 0.03, -0.002, 0.026, col);
    mb.color = C("#6a5034");
    for (const x of [-0.01, 0.01]) at(mb, x, -0.002, 0, 0, () => mb.box(0.003, 0.0285, 0.0315));
  });
}

markerRecipe("res.cotton", 1.6, { variants: 2, build: (mb, r) => buildCotton(mb, r) });

// Livestock markers: one beast, larger than in a pasture, on a tuft of grass.
for (const k of ["cow", "horse", "sheep", "pig", "deer"]) {
  markerRecipe(`res.${k}`, 1.2, {
    variants: 2,
    build: (mb, r, { variant }) => {
      mb.part(() => {
        mb.scale(k === "horse" || k === "deer" ? 1.15 : 1.3);
        mb.rotateY(Math.PI / 2 - 0.45 + (variant ? Math.PI : 0));
        animal(mb, r, k, { pose: variant });
      });
      mb.mat = MAT.FOLIAGE;
      tufts(mb, r, 2, 0.06, 0.025, [T.grass, "#7a8238"]);
      mb.mat = MAT.MATTE;
    },
  });
}

/** A fish leaping from a ring of ripples. */
function buildFish(mb, r, variant) {
  mb.color = C("#c8d8d8");
  for (const [rad, y] of [[0.05, 0.003], [0.075, 0.002]]) {
    const pts = [];
    for (let i = 0; i <= 10; i++) pts.push([Math.cos((i / 10) * TAU) * rad, y, Math.sin((i / 10) * TAU) * rad * 0.8]);
    mb.tube(pts, 0.0035, 3);
  }
  // The fish arcs out of the water.
  const pts = [], radii = [];
  for (let i = 0; i <= 6; i++) {
    const t = i / 6, a = Math.PI * (0.15 + t * 0.7);
    pts.push([-Math.cos(a) * 0.05, 0.01 + Math.sin(a) * 0.06, 0.0]);
    radii.push(0.013 * Math.sin(Math.PI * (0.12 + t * 0.8)) + 0.002);
  }
  mb.mat = MAT.METAL;
  mb.color = C(variant % 2 ? "#a4b2b8" : "#b4aa8c");
  const m0 = mb.mark();
  mb.tube(pts, radii, 5, { caps: true });
  mb.tint(m0, (x, y, z) => (y > 0.045 ? 0.7 : 1.15));
  mb.mat = MAT.MATTE;
  mb.color = C("#5a6a70");
  const tail = pts[0];
  mb.card(tail, [tail[0] - 0.012, tail[1] - 0.016, 0.012], [tail[0] - 0.006, tail[1] - 0.008, 0], [tail[0] - 0.012, tail[1] - 0.016, -0.012]);
  // Spray.
  mb.color = C("#e0eaea");
  for (let k = 0; k < 4; k++) at(mb, 0.05 + r.range(-0.01, 0.01), 0.01 + k * 0.006, r.range(-0.01, 0.01), 0, () => mb.sphere(0.004, 0));
}

markerRecipe("res.fish", 1.45, { variants: 2, build: (mb, r, { variant }) => buildFish(mb, r, variant) });

/** Clams: a cluster of ribbed shells on a sandbar just breaking the water. */
function buildClams(mb, r, pearl = false) {
  // A pale sandbar breaking the water (wet and dark at the waterline), the shells heaped on it.
  mb.color = C("#c4b08a");
  const s0 = mb.mark();
  mb.part(() => {
    mb.translate(0, -0.008, 0);
    mb.scale(1.4, 1, 1);
    mb.blob(0.045, { detail: 1, amp: 0.15, squash: 0.5, noise: (v) => noise3(v[0] * 3, v[1] * 3, v[2] * 3) });
  });
  mb.tint(s0, (x, y) => 0.62 + 0.45 * clamp((y + 0.002) / 0.016, 0, 1));
  const n = pearl ? 1 : 4;
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + 0.4, d = pearl ? 0 : 0.035;
    at(mb, Math.cos(a) * d, pearl ? 0.012 : 0.01, Math.sin(a) * d, r.range(0, TAU), () => {
      const s = pearl ? 1.5 : r.range(0.8, 1.1);
      mb.scale(s);
      mb.color = J("#d8c8b0", r, 0.06);
      const m0 = mb.mark();
      mb.lathe([[0.018, 0], [0.014, 0.006], [0, 0.008]], 8, { caps: true, wobble: (aa) => 0.08 * Math.cos(aa * 9) });
      mb.tint(m0, (x, y, z) => 0.85 + 0.25 * Math.cos(Math.atan2(z, x) * 9));
      if (pearl) {
        // An open shell: the upper valve raised, a pearl inside.
        mb.part(() => {
          mb.translate(0, 0.002, -0.016);
          mb.rotateX(-0.9);
          mb.translate(0, 0, 0.016);
          mb.lathe([[0.018, 0], [0.014, 0.005], [0, 0.007]], 8, { caps: true, wobble: (aa) => 0.08 * Math.cos(aa * 9) });
        });
        mb.mat = MAT.METAL;
        mb.color = C("#f0ece4");
        at(mb, 0, 0.012, 0.0, 0, () => mb.sphere(0.008, 1));
        mb.mat = MAT.MATTE;
      }
    });
  }
}

markerRecipe("res.clam", 1.6, { variants: 2, build: (mb, r) => buildClams(mb, r, false) });
markerRecipe("res.pearl", 1.7, { variants: 1, build: (mb, r) => buildClams(mb, r, true) });

/** A crab on a wet rock, claws raised. */
function buildCrab(mb, r) {
  rockMass(mb, r, { rad: 0.05, detail: 0, sx: 1.4, sy: 0.4, sz: 1.1, cuts: 3, cut: 0.6, c: [0, -0.005, 0], shade: rockShader(rockPaint("basalt", r), 2) });
  at(mb, 0, 0.018, 0.01, r.range(-0.4, 0.4), () => {
    mb.color = C("#b8482a");
    mb.part(() => {
      mb.scale(1.3, 0.55, 1);
      mb.sphere(0.02, 1);
    });
    mb.color = C("#a03a22");
    for (const sx of [-1, 1]) {
      for (let l = 0; l < 3; l++) {
        const z = -0.01 + l * 0.009;
        mb.tube([[sx * 0.02, 0, z], [sx * 0.035, 0.008, z], [sx * 0.042, -0.012, z + 0.004]], 0.0022, 3);
      }
      mb.tube([[sx * 0.016, 0.004, 0.014], [sx * 0.026, 0.018, 0.026], [sx * 0.02, 0.026, 0.034]], [0.003, 0.0035, 0.002], 3);
      at(mb, sx * 0.02, 0.028, 0.036, 0, () => mb.sphere(0.0065, 0, { squash: 0.8 }));
    }
    mb.color = C("#1a1412");
    for (const x of [-0.006, 0.006]) stick(mb, [x, 0.008, 0.016], [x, 0.016, 0.02], 0.0018, 3);
  });
}

markerRecipe("res.crab", 1.6, { variants: 1, build: (mb, r) => buildCrab(mb, r) });

/** A whale's back breaking the sea, its tail fluke raised behind and a spout of spray. */
function buildWhale(mb, r) {
  mb.color = C("#3e4650");
  const m0 = mb.mark();
  mb.part(() => {
    mb.translate(0.0, -0.012, 0);
    mb.scale(2.6, 0.75, 1);
    mb.sphere(0.04, 1);
  });
  mb.tint(m0, (x, y) => (y > 0.012 ? 1.15 : 0.75));
  mb.color = C("#363e48");
  mb.tube([[-0.1, -0.005, 0], [-0.13, 0.02, 0.0], [-0.145, 0.05, 0.0]], [0.012, 0.008, 0.005], 5);
  mb.card([-0.145, 0.05, 0], [-0.16, 0.07, 0.035], [-0.15, 0.058, 0], [-0.16, 0.07, -0.035]);
  mb.card([-0.145, 0.05, 0], [-0.16, 0.07, -0.035], [-0.135, 0.065, 0], [-0.16, 0.07, 0.035]);
  // Spray from the blowhole.
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#5a6a70");
    mb.lathe([[0.003, 0.025], [0.01, 0.06], [0.022, 0.08], [0.012, 0.09], [0, 0.088]], 6, { caps: false, wobble: (a, y) => 0.15 * Math.sin(a * 3 + y * 60) });
  });
  mb.color = C("#c8d4d8");
  const pts = [];
  for (let i = 0; i <= 14; i++) pts.push([Math.cos((i / 14) * TAU) * 0.13, 0.002, Math.sin((i / 14) * TAU) * 0.055]);
  mb.tube(pts, 0.004, 3);
}

markerRecipe("res.whale", 1.2, { variants: 1, build: (mb, r) => buildWhale(mb, r) });

/** Trade goods by kind (param): bolts of silk, dye vats, incense jars, wine, furs, ivory tusks,
 *  reagents, gunpowder kegs, ale, fine clothes. */
function buildGoods(mb, r, variant, kind) {
  switch (kind) {
    case "silk":
    case "clothes": {
      const cols = kind === "silk" ? ["#a83a4a", "#3a6a9a", "#c8a040", "#6a3a8a"] : ["#7a2a3a", "#2a4a6a", "#5a6a2a"];
      for (let k = 0; k < 4; k++) {
        at(mb, (k % 2) * 0.034 - 0.017, 0.012 + Math.floor(k / 2) * 0.022, 0, 0, () => {
          mb.color = C(cols[k % cols.length]);
          mb.rotateZ(Math.PI / 2);
          mb.translate(0, -0.03, 0);
          mb.cylinder(0.011, 0.011, 0.06, 7);
        });
      }
      crate(mb, 0.05, 0.04, 0.03, 0.4, "#7a5a3c");
      if (kind === "clothes") {
        mb.color = C("#4a3826");
        stick(mb, [-0.05, -0.01, -0.03], [-0.05, 0.09, -0.03], 0.003, 4);
        mb.color = C("#8a2a3a");
        mb.lathe([[0.022, 0.03], [0.016, 0.06], [0.02, 0.08], [0.008, 0.088]], 6);
      }
      break;
    }
    case "dye": {
      const cols = ["#2a3a8a", "#9a2a2a", "#c8a030"];
      for (let k = 0; k < 3; k++) {
        at(mb, (k - 1) * 0.04, 0, (k % 2) * 0.02, 0, () => {
          mb.color = C("#5a4430");
          mb.lathe([[0.017, -0.004], [0.019, 0.03], [0.017, 0.03]], 8, { caps: false });
          mb.color = C(cols[k]);
          mb.lathe([[0.0172, 0.026], [0, 0.026]], 8, { caps: false });
        });
      }
      mb.color = C(cols[0]);
      mb.card([-0.04, 0.0, 0.04], [0.02, 0.0, 0.05], [0.02, 0.0, 0.07], [-0.04, 0.0, 0.06]);
      break;
    }
    case "incense": {
      for (let k = 0; k < 3; k++) {
        at(mb, (k - 1) * 0.03, 0, (k % 2) * 0.02, 0, () => {
          mb.color = C(k === 1 ? "#a8704a" : "#8a5a3a");
          mb.lathe([[0.008, 0], [0.016, 0.02], [0.012, 0.04], [0.007, 0.045], [0.009, 0.05]], 7);
        });
      }
      mb.color = C("gold");
      mb.mat = MAT.METAL;
      at(mb, 0.0, 0, 0.05, 0, () => mb.lathe([[0.006, 0], [0.004, 0.012], [0.014, 0.02], [0.012, 0.026]], 7));
      mb.mat = MAT.EMISSIVE;
      mb.color = C("#ff9a40");
      at(mb, 0.0, 0.024, 0.05, 0, () => mb.lathe([[0.011, 0], [0, 0.002]], 6, { caps: false }));
      mb.mat = MAT.MATTE;
      break;
    }
    case "wine": {
      for (let k = 0; k < 2; k++) at(mb, -0.02 + k * 0.034, 0.016, 0, 0, () => { mb.rotateX(Math.PI / 2); mb.translate(0, -0.015, 0); barrel(mb, 0, 0, 1.1, "#5a3a26"); });
      at(mb, 0.0, 0.034, 0, 0, () => { mb.rotateX(Math.PI / 2); mb.translate(0, -0.015, 0); barrel(mb, 0, 0, 1.0, "#5a3a26"); });
      mb.color = C("#3a1a3a");
      for (let k = 0; k < 5; k++) at(mb, 0.05 + r.range(-0.006, 0.006), 0.006 + k * 0.004, 0.04 + r.range(-0.006, 0.006), 0, () => mb.sphere(0.0065, 0));
      break;
    }
    case "fur": {
      const cols = ["#6a5a48", "#8a7a64", "#4a3a2c", "#d8d0c0"];
      for (let k = 0; k < 4; k++) at(mb, r.range(-0.01, 0.01), 0.004 + k * 0.007, r.range(-0.01, 0.01), r.range(-0.4, 0.4), () => {
        mb.color = C(cols[k]);
        mb.part(() => {
          mb.scale(1.5, 0.18, 1);
          mb.sphere(0.035, 0);
        });
      });
      mb.color = C("#e8e0d0");
      stick(mb, [0.03, 0.03, 0.02], [0.05, 0.034, 0.03], 0.003, 3);
      break;
    }
    case "ivory": {
      mb.color = C("#e8dcc0");
      for (const s of [-1, 1]) mb.tube([[s * 0.04, 0.0, -0.02], [s * 0.02, 0.04, 0.0], [-s * 0.005, 0.07, 0.015], [-s * 0.02, 0.085, 0.02]], [0.01, 0.009, 0.006, 0.002], 6);
      crate(mb, 0.0, 0.04, 0.03, 0.2, "#6a4a30");
      break;
    }
    case "reagents": {
      const cols = ["#3a8a5a", "#8a3a8a", "#c8a030", "#3a5aa8"];
      for (let k = 0; k < 4; k++) {
        at(mb, (k % 2) * 0.03 - 0.015, 0, Math.floor(k / 2) * 0.03 - 0.015, 0, () => {
          mb.color = C("#4a4038");
          mb.lathe([[0.012, 0], [0.013, 0.02], [0.005, 0.03], [0.004, 0.038], [0, 0.038]], 6);
          mb.mat = MAT.EMISSIVE;
          mb.color = mixl(cols[k], "#000000", 0.4);
          mb.lathe([[0.0132, 0.006], [0.0132, 0.016]], 6, { caps: false });
          mb.mat = MAT.MATTE;
        });
      }
      at(mb, 0.05, 0.004, 0.03, -0.5, () => skull(mb, 0.01));
      break;
    }
    case "gunpowder":
    case "ale":
    default: {
      for (let k = 0; k < 3; k++) barrel(mb, (k - 1) * 0.033, (k % 2) * 0.02, 1.15, kind === "gunpowder" ? "#3a3430" : "#6a4a30");
      crate(mb, 0.0, 0.05, 0.032, 0.3);
      if (kind === "gunpowder") {
        mb.color = C("#1a1614");
        at(mb, 0.0, 0.035, 0.02, 0, () => mb.lathe([[0.008, 0], [0, 0.004]], 6));
      }
    }
  }
}

markerRecipe("res.goods", 1.9, { variants: 2, build: (mb, r, { variant, param }) => buildGoods(mb, r, variant, param) });

/** A giant toad squatting on a lily pad of the infernal marshes (warty, sickly). */
function buildToad(mb, r) {
  mb.color = C("#3a4a2a");
  mb.lathe([[0.06, 0.0], [0, 0.002]], 10, { caps: false, wobble: (a) => (a < 0.5 ? -0.5 : 0) });
  const seed = r.range(0, 9);
  mb.color = C("#5a6a34");
  const m0 = mb.mark();
  at(mb, 0, 0.026, 0, 0, () => { mb.scale(1.0, 0.75, 1.15); mb.blob(0.036, { detail: 1, amp: 0.1, squash: 1, noise: (v) => noise3(v[0] * 9 + seed, v[1] * 9, v[2] * 9) }); });
  mb.tint(m0, (x, y) => (y < 0.018 ? 1.4 : 0.85));
  for (const s of [-1, 1]) {
    mb.color = C("#4e5e2e");
    mb.tube([[s * 0.025, 0.015, -0.02], [s * 0.045, 0.03, -0.015], [s * 0.05, 0.004, 0.0]], 0.008, 4);
    mb.tube([[s * 0.02, 0.012, 0.025], [s * 0.03, 0.004, 0.038]], 0.005, 4);
    mb.color = C("#d8b030");
    at(mb, s * 0.015, 0.05, 0.03, 0, () => mb.sphere(0.008, 0));
    mb.color = C("#120e0a");
    at(mb, s * 0.016, 0.051, 0.037, 0, () => mb.sphere(0.0035, 0));
  }
}

markerRecipe("res.toad", 1.6, { variants: 1, build: (mb, r) => buildToad(mb, r) });

/** A nightmare: a black horse of smoke and shadow, its mane and eyes burning red. */
function buildNightmare(mb, r, variant) {
  mb.part(() => {
    mb.scale(1.2);
    mb.rotateY(Math.PI / 2 - 0.45);
    const saved = ANIMALS.horse;
    ANIMALS.horse = { ...saved, coat: "#1a1618", patch: "#100c10", mane: "#1a1618" };
    animal(mb, r, "horse", { pose: 1 });
    ANIMALS.horse = saved;
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#d03a1a");
    const A = saved;
    const yb = A.leg + A.girth * 0.85, hy = yb + A.girth * 1.9, hz = A.len / 2 + 0.012;
    mb.tube([[0, hy + 0.006, hz - 0.004], [0, yb + A.girth * 1.2, A.len / 2 - A.girth * 0.7]], [0.003, 0.004], 3);
    for (const sx of [-1, 1]) at(mb, sx * 0.008, hy - 0.004, hz + 0.01, 0, () => mb.sphere(0.0028, 0));
    mb.tube([[0, yb + A.girth * 0.5, -A.len / 2], [0, yb, -A.len / 2 - 0.03]], [0.004, 0.001], 3);
  });
}

markerRecipe("res.nightmare", 1.2, { variants: 1, build: (mb, r, { variant }) => buildNightmare(mb, r, variant) });

/** Razorweed: a clump of black, blade-edged leaves with red-hot edges. */
function buildRazorweed(mb, r) {
  mb.mat = MAT.METAL;
  for (let k = 0; k < 9; k++) {
    const a = (k / 9) * TAU + r.range(-0.2, 0.2), tilt = r.range(0.2, 0.7);
    crystal(mb, [Math.cos(a) * 0.01, -0.005, Math.sin(a) * 0.01], [Math.cos(a) * Math.sin(tilt), Math.cos(tilt), Math.sin(a) * Math.sin(tilt)], r.range(0.06, 0.11), 0.006, 3, { tip: 0.7, foot: 0, shade: (n, y, part) => (part === 1 ? C("#8a2a1a") : faceShade("#1e1418", n, { lit: 0.5 })) });
  }
  mb.mat = MAT.MATTE;
}

markerRecipe("res.razorweed", 1.7, { variants: 2, build: (mb, r) => buildRazorweed(mb, r) });

/** Gulagarm: a fleshy, spined pod of the hells, veined with a dull inner glow. */
function buildGulagarm(mb, r) {
  const seed = r.range(0, 9);
  mb.color = C("#5a2630");
  const m0 = mb.mark();
  at(mb, 0, 0.03, 0, 0, () => mb.blob(0.04, { detail: 1, amp: 0.25, squash: 0.9, noise: (v) => noise3(v[0] * 4 + seed, v[1] * 4, v[2] * 4) }));
  mb.tint(m0, (x, y) => 0.7 + 0.5 * clamp(y / 0.06, 0, 1));
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#a03a1a");
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * TAU;
      mb.tube([[Math.cos(a) * 0.036, 0.012, Math.sin(a) * 0.036], [Math.cos(a) * 0.04, 0.03, Math.sin(a) * 0.04], [Math.cos(a) * 0.026, 0.058, Math.sin(a) * 0.026]], 0.002, 3);
    }
  });
  mb.mat = MAT.METAL;
  for (let k = 0; k < 6; k++) {
    const a = r.range(0, TAU), el = r.range(0.2, 1.2);
    const d = [Math.cos(a) * Math.cos(el), Math.sin(el), Math.sin(a) * Math.cos(el)];
    crystal(mb, vadd([0, 0.03, 0], vscale(d, 0.036)), d, 0.03, 0.004, 3, { tip: 0.8, foot: 0, shade: (n) => faceShade("#2a1a1a", n, { lit: 0.5 }) });
  }
  mb.mat = MAT.MATTE;
}

markerRecipe("res.gulagarm", 1.7, { variants: 1, build: (mb, r) => buildGulagarm(mb, r) });

/** The Fruit of Yggdrasil: a silver bough bent under glowing golden fruit. */
function buildYggFruit(mb, r) {
  mb.color = C("#8a8478");
  mb.tube([[0, -0.01, 0], [0.01, 0.06, 0], [0.05, 0.1, 0.01], [0.08, 0.09, 0.02]], [0.008, 0.006, 0.004, 0.002], 5);
  mb.mat = MAT.FOLIAGE;
  for (let k = 0; k < 4; k++) {
    mb.color = J("#6a8a5a", r, 0.06);
    const p = [0.02 + k * 0.018, 0.085 + r.range(0, 0.02), 0.01];
    mb.card(p, vadd(p, [0.01, 0.02, 0.012]), vadd(p, [0.024, 0.012, 0.006]), vadd(p, [0.012, -0.004, -0.004]));
  }
  mb.mat = MAT.EMISSIVE;
  mb.color = C("#ffc850");
  for (const [x, y] of [[0.035, 0.07], [0.06, 0.065], [0.08, 0.075]]) at(mb, x, y, 0.016, 0, () => mb.sphere(0.011, 1));
  mb.mat = MAT.MATTE;
}

markerRecipe("res.yggfruit", 1.6, { variants: 1, build: (mb, r) => buildYggFruit(mb, r) });

/** A Sheut stone: a black, egg-smooth stone veined with violet light. */
function buildSheut(mb, r) {
  mb.color = C("#1e1a24");
  mb.mat = MAT.METAL;
  at(mb, 0, 0.03, 0, 0, () => mb.sphere(0.042, 1, { squash: 1.25 }));
  mb.mat = MAT.EMISSIVE;
  mb.color = C("#7a3ac8");
  for (let k = 0; k < 3; k++) {
    const a = k * 2.1;
    const pts = [];
    for (let i = 0; i <= 6; i++) {
      const t = i / 6, b = a + t * 1.2;
      const y = 0.005 + t * 0.07;
      const rr = Math.sqrt(Math.max(0, 1 - ((y - 0.03) / 0.0525) ** 2)) * 0.0425;
      pts.push([Math.cos(b) * rr, y, Math.sin(b) * rr]);
    }
    mb.tube(pts, 0.0022, 3);
  }
  mb.mat = MAT.MATTE;
}

markerRecipe("res.sheut", 1.7, { variants: 1, build: (mb, r) => buildSheut(mb, r) });

// ------------------------------------------------------------------------------------------------
// Catalog: rocks

function rocksOf(ctx) {
  const { kind, ground, feat, r } = ctx;
  if (kind === KIND.VOID || kind === KIND.OCEAN || kind === KIND.COAST || kind === KIND.ICE) return [];
  const hell = isHell(ground);
  const cold = ground === GROUND.TUNDRA || ground === GROUND.SNOW;
  const sand = ground === GROUND.DESERT;
  const wooded = feat === FEAT.FOREST || feat === FEAT.JUNGLE || feat === FEAT.ANCIENT || feat === FEAT.YOUNG;
  if (kind === KIND.PEAK) {
    // Crags break the mountain's silhouette; scree and boulders on the flanks.
    if (feat === FEAT.VOLCANO) return [{ asset: "rock.basalt", scale: 1.2, count: 3 }, { asset: "rock.shard", scale: 1.1, count: 1 }];
    if (hell) return [{ asset: "rock.crag", scale: 1.0, count: 2, tint: [0.62, 0.55, 0.58], prefer: "ridge" }, { asset: "rock.thorn", scale: 1.3, count: 1 }, { asset: "rock.shard", scale: 1.2, count: 1 }];
    return [
      { asset: cold ? "rock.crag.snow" : "rock.crag", scale: 1.0, count: 2 + (r() < 0.4 ? 1 : 0), prefer: "ridge" },
      { asset: cold ? "rock.boulder.snow" : "rock.boulder", scale: 0.9, count: r() < 0.6 ? 1 : 0 },
    ];
  }
  if (kind === KIND.HILLS) {
    if (hell) return [{ asset: "rock.thorn", scale: 1.25, count: 1 }, { asset: "rock.shard", scale: 1.1, count: r() < 0.5 ? 1 : 0 }, { asset: "rock.basalt", scale: 1.1, count: 1 }];
    if (sand) return [{ asset: r() < 0.6 ? "rock.outcrop.sand" : "rock.sandstone", scale: 1.0, count: 1 }, { asset: "rock.sandstone", scale: 0.8, count: r() < 0.5 ? 1 : 0 }];
    if (cold) return [{ asset: "rock.outcrop", scale: 0.95, count: 1 }, { asset: "rock.boulder.lichen", scale: 1.0, count: 1 + (r() < 0.4 ? 1 : 0) }];
    if (wooded) return [{ asset: "rock.boulder.moss", scale: 0.9, count: r() < 0.6 ? 1 : 0 }];
    return [{ asset: r() < 0.7 ? "rock.outcrop.moss" : "rock.outcrop", scale: 1.0, count: 1 + (r() < 0.35 ? 1 : 0) }, { asset: "rock.boulder.moss", scale: 0.85, count: r() < 0.5 ? 1 : 0 }, { asset: "rock.boulder.moss", scale: 0.42, count: ctx.improvement ? 0 : r.int(0, 3) }];
  }
  // Flat land: sparse.
  if (hell) {
    if (feat === FEAT.OBSIDIAN) return [];
    const k = r();
    return [{ asset: k < 0.45 ? "rock.thorn" : k < 0.7 ? "rock.shard" : "rock.basalt", scale: k < 0.45 ? 1.35 : 1.0, count: r() < 0.75 ? 1 + (r() < 0.25 ? 1 : 0) : 0 }];
  }
  // Small stones in the grass (not in an improvement's fields), now and then a boulder.
  const stones = (asset, n, param) => ({ asset, param, scale: 0.42, count: ctx.improvement || wooded ? 0 : r.int(0, n) });
  if (ground === GROUND.TUNDRA) return [{ asset: "rock.boulder.lichen", scale: 0.95, count: r() < 0.55 ? 1 + (r() < 0.3 ? 1 : 0) : 0 }, stones("rock.boulder.lichen", 3)];
  if (ground === GROUND.SNOW) return [{ asset: "rock.boulder.snow", scale: 0.9, count: r() < 0.4 ? 1 : 0 }];
  // (The desert's stones are sandstone, not the grasslands' grey.)
  if (sand && feat === FEAT.NONE) return [{ asset: "rock.sandstone", scale: 0.85, count: r() < 0.3 ? 1 : 0 }, stones("rock.boulder", 2, "sand")];
  if (ground === GROUND.PLAINS || ground === GROUND.GRASS) return [{ asset: "rock.boulder", scale: 0.75, count: r() < 0.18 ? 1 : 0 }, stones("rock.boulder", 2)];
  return [];
}

// ------------------------------------------------------------------------------------------------
// Catalog: improvements

/** Settlement-like improvements belong to architecture.js (and the "worked" markers and smoke have
 *  no structure). */
const NOT_OURS = /^(COTTAGE|HAMLET|VILLAGE|TOWN|ENCLAVE|FORT|CASTLE|CITADEL|TOWER|WARNING_POST|PIRATE_PORT|GOBLIN_FORT|LAND_WORKED|WATER_WORKED|SMOKE)$/;

/** The ore a mine digs (from the plot's resource). */
function oreOf(bonus) {
  const b = (bonus ?? "").replace("BONUS_", "");
  return { COPPER: "copper", IRON: "iron", GOLD: "gold", SILVER: "silver", MITHRIL: "mithril", GEMS: "gems", JEWELS: "gems" }[b] ?? "";
}

/** A beast for a pasture (from the plot's resource). */
function herdOf(bonus) {
  const b = (bonus ?? "").replace("BONUS_", "");
  return { HORSE: "horse", COW: "cow", SHEEP: "sheep", PIG: "pig", DEER: "deer", NIGHTMARE: "horse" }[b] ?? null;
}

const glowOf = (school) => MANA[school] ?? MANA[""];

/** Whether any of the plot-local points [[x, z], ...] lies on a road through the plot, or within
 *  `pad` of the strip the fields leave it (fields.js onRoad). */
function onRoadOf(ctx) {
  const links = ctx.world ? roadLinks(ctx.world, ctx.world.map, ctx.x, ctx.y) : [];
  const routed = !!ctx.world && ctx.world.map.route[ctx.i] >= 0;
  return (pts, pad = 0) => pts.some(([x, z]) => onRoad(links, routed, x, z, ROAD_CLEAR + pad));
}

/** The plot's field layout (fields.js), drawn back from the rivers round it (dress.js gives the
 *  context its riversAround; the lab's grid has none). */
const layoutOf = (ctx) => fieldLayout(ctx.i, ctx.improvement, ctx.bonus, ctx.rivers ?? 0);

/** A farm on its fields (fields.js): the farmstead (a barn or a granary) in its yard with its
 *  haystacks and, on some farms, a cart or its tools; fence runs along the field edges, stooks on
 *  the grain and a scarecrow in the beds.
 *  The plot's centre keeps only low crops (a unit standing there stays readable). */
function farmOf(ctx, part, yj) {
  const r = ctx.r;
  const lay = layoutOf(ctx);
  const { yard } = lay, s = yard.side;
  // Nothing of the farm but its barn stands on a road through it (the decal keeps the fields off
  // the road too).
  const road = onRoadOf(ctx);
  const hz = yard.z + r.range(-0.05, 0.03);
  const out = [
    part("imp.barn", yard.x, yard.z, { variant: r.int(0, 3), yaw: yj(0.3), scale: 1.2 }),
    part("imp.haystacks", yard.x - s * 0.2, hz, { variant: r.int(0, 3), yaw: r.range(0, TAU), scale: r.range(1.3, 1.5), tint: [1, 0.96, 0.9] }),
  ];
  // Now and then a cart or the farm's tools in the yard: the first spot round the barn that is off
  // the fields, the road and the water and clear of the barn and the ricks.
  const kit = r(), ka = r.range(0, TAU);
  if (kit < 0.6) {
    const ok = (x, z) =>
      Math.abs(x) < 0.42 && Math.abs(z) < 0.42 && Math.hypot(x - yard.x, z - yard.z) > 0.17 && Math.hypot(x - (yard.x - s * 0.2), z - hz) > 0.12 &&
      !inField(ctx.i, ctx.improvement, ctx.bonus, ctx.rivers ?? 0, x, z, 0.03) && !road([[x, z]], 0.04) && (!ctx.unit || Math.hypot(x, z) > 0.22) &&
      (!ctx.world || ctx.world.riverDist(ctx.x + 0.5 + x, ctx.y + 0.5 - z) > RIVER_HALF + RIVER_WANDER + 0.06);
    for (let k = 0; k < 10; k++) {
      const a = ka + k * 2.4, d = 0.19 + 0.02 * (k % 3);
      const x = yard.x + Math.cos(a) * d, z = yard.z + Math.sin(a) * d;
      if (!ok(x, z)) continue;
      out.push(part(kit < 0.35 ? "imp.cart" : "imp.tools", x, z, { variant: r.int(0, 1), yaw: r.range(0, TAU), scale: 1.35 }));
      break;
    }
  }
  for (const [[ax, az], [bx, bz]] of lay.fences) {
    const L = Math.hypot(bx - ax, bz - az), n = Math.max(1, Math.round(L / 0.2));
    const yaw = Math.atan2(-(bz - az), bx - ax);
    for (let k = 0; k < n; k++) {
      const t = (k + 0.5) / n;
      if (road([0, 0.5, 1].map((u) => [ax + ((bx - ax) * (k + u)) / n, az + ((bz - az) * (k + u)) / n]))) continue;
      out.push({ asset: "imp.fence", dx: ax + (bx - ax) * t, dz: az + (bz - az) * t, yaw: yaw + yj(0.03), variant: r.int(0, 3), scale: 1, sx: L / n / 0.2 });
    }
  }
  // The crops in the round: beds and standing grain in patches over their fields, each patch on its
  // own ground, its rows on the rows the decal paints (fields.js ROWS).
  for (const f of lay.fields) {
    if (f.crop === CROP.SOIL) continue;
    const green = f.crop === CROP.GREEN;
    const N = green ? ROWS.green : ROWS.grain, per = green ? 3 : 4;
    // Row positions across the field (z for beds running along x, x for grain running along z).
    const lo = green ? f.z0 : f.x0, hi = green ? f.z1 : f.x1;
    const rows = [];
    for (let k = 0; k < N; k++) {
      const c = green ? 0.5 - (k + 0.5) / N : (k + 0.5) / N - 0.5;
      if (c > lo + 0.022 && c < hi - 0.022) rows.push(c);
    }
    rows.sort((a, b) => a - b);
    const a0 = green ? f.x0 + 0.015 : f.z0 + 0.015, a1 = green ? f.x1 - 0.015 : f.z1 - 0.015;
    const m = Math.max(1, Math.round((a1 - a0) / 0.2)), len = (a1 - a0) / m;
    for (let k = 0; k < rows.length; k += per) {
      const chunk = rows.slice(k, k + per);
      const c = (chunk[0] + chunk[chunk.length - 1]) / 2;
      const half = (chunk[chunk.length - 1] - chunk[0]) / 2 + 0.03;
      for (let q = 0; q < m; q++) {
        const along = a0 + len * (q + 0.5);
        // A patch stands level: where the ground under it is too steep, the painted rows suffice.
        if (ctx.world) {
          const hs = [[-1, -1], [1, -1], [1, 1], [-1, 1]].map(([u, v]) => {
            const lx = green ? along + (u * len) / 2 : c + u * half, lz = green ? c + v * half : along + (v * len) / 2;
            return ctx.world.heightAt(ctx.x + 0.5 + lx, ctx.y + 0.5 - lz);
          });
          if (Math.max(...hs) - Math.min(...hs) > 0.024) continue;
        }
        const ext = green ? [len / 2, half] : [half, len / 2];
        const cx = green ? along : c, cz = green ? c : along;
        if (road([-1, 0, 1].flatMap((u) => [-1, 0, 1].map((v) => [cx + u * ext[0], cz + v * ext[1]])))) continue;
        out.push(green
          ? { asset: "imp.beds", dx: along, dz: c, yaw: 0, variant: chunk.length - 1, scale: 1, sx: len / 0.2 }
          : { asset: "imp.grain", dx: c, dz: along, yaw: 0, variant: chunk.length - 1, scale: 1, sz: (len / 0.2) * 1.05 });
      }
    }
  }
  for (const [x, z] of lay.stooks) {
    if ((ctx.unit && Math.abs(x) < 0.16) || road([[x, z]], 0.03)) continue;
    out.push(part("imp.stook", x, z, { variant: r.int(0, 2), yaw: r.range(0, TAU), scale: r.range(1.4, 1.7) }));
  }
  const beds = lay.fields.find((f) => f.crop === CROP.GREEN);
  if (beds && r() < 0.6) {
    const x = beds.x0 + (beds.x1 - beds.x0) * r.range(0.3, 0.7), z = beds.z0 + (beds.z1 - beds.z0) * r.range(0.3, 0.7);
    if ((!ctx.unit || Math.hypot(x, z) > 0.2) && !road([[x, z]], 0.03)) out.push(part("imp.scarecrow", x, z, { yaw: yj(0.5), scale: 1.35 }));
  }
  return out;
}

/** Where a pair of row props (a plantation's crop rows, a winery's vines) stands on field `f`: at
 *  fractions `us` across it and `v` down it, each shrunk (x, z) as the field is narrower than `w`
 *  or shallower than `d` (a field drawn back from a river, fields.js). [[x, z, sx, sz], ...]. */
function rowsOn(f, w, d, us, v) {
  const sx = Math.min(1, (f.x1 - f.x0) / w), sz = Math.min(1, (f.z1 - f.z0) / d);
  return us.map((u) => [f.x0 + (f.x1 - f.x0) * u, f.z0 + (f.z1 - f.z0) * v, sx, sz]);
}

function improvementOf(type, ctx) {
  const t = type.replace("IMPROVEMENT_", "");
  if (NOT_OURS.test(t)) return null;
  const r = ctx.r;
  const unit = !!ctx.unit;
  const yj = (a = 0.25) => r.range(-a, a);
  const part = (asset, dx, dz, o = {}) => ({ asset, dx, dz, yaw: o.yaw ?? yj(), scale: o.scale ?? 1, ...(o.param !== undefined ? { param: o.param } : {}), ...(o.variant !== undefined ? { variant: o.variant } : {}), ...(o.float ? { float: true } : {}), ...(o.draft ? { draft: o.draft } : {}), ...(o.emit ? { emit: o.emit } : {}), ...(o.tint ? { tint: o.tint } : {}), ...(o.sx !== undefined ? { sx: o.sx } : {}), ...(o.sz !== undefined ? { sz: o.sz } : {}) });
  // Old stone in the hells is scorched dark (pale ruins would glare on the black ground).
  const scorch = isHell(ctx.ground) ? [0.5, 0.44, 0.42] : undefined;
  // Mirror the layout now and then (never onto the resource marker's front-left corner).
  const flip = !ctx.bonus && r() < 0.5 ? -1 : 1;
  const water = ctx.world?.isWater?.(ctx.i) ?? false;
  // Landmarks stand at the centre; with a unit there they step back and shrink a little.
  const lm = (asset, o = {}) => part(asset, 0, unit ? -0.14 : -0.02, { ...o, scale: (o.scale ?? 1) * (unit ? 0.85 : 1), yaw: o.yaw ?? yj(0.2) });
  if (t.startsWith("MANA_")) {
    const school = t.slice(5);
    return part("imp.mananode", 0, unit ? -0.17 : -0.02, { param: school, scale: unit ? 0.95 : 1.3, yaw: r.range(0, TAU), emit: [{ color: glowOf(school), size: 0.016, rise: 0.2, count: 5, spread: 0.1, dy: 0.2 }] });
  }
  switch (t) {
    case "FARM":
      return farmOf(ctx, part, yj);
    case "PLANTATION": {
      // The rows fill the green field the decal lays (fields.js), shrunk with it off a river; the
      // drying shed in the yard.
      const b = (ctx.bonus ?? "").replace("BONUS_", "");
      const crop = { BANANA: "banana", SUGAR: "sugar", COTTON: "cotton", DYE: "dye", SILK: "tea", INCENSE: "tea", SPICES: "tea" }[b] ?? "tea";
      const lay = layoutOf(ctx);
      const road = onRoadOf(ctx);
      return rowsOn(lay.fields[0], 0.84, 0.48, [0.26, 0.74], 0.54).map(([x, z, sx, sz], k) => part("imp.croprows", x, z, { param: crop, ...(k ? { variant: 1 } : {}), yaw: yj(0.08), scale: 1.4, sx: 1.4 * sx, sz: 1.4 * sz }))
        .filter((p) => !road([[p.dx, p.dz]], 0.06)).concat([part("imp.shed", lay.yard.x, lay.yard.z, { scale: 1.45 })]);
    }
    case "WINERY": {
      const lay = layoutOf(ctx);
      const road = onRoadOf(ctx);
      const vines = rowsOn(lay.fields[0], 0.78, 0.44, [0.27, 0.75], 0.5).map(([x, z, sx, sz]) => part("imp.vines", x, z, { yaw: yj(0.1), scale: 1.4, sx: 1.4 * sx, sz: 1.4 * sz }));
      return [...vines.filter((p) => !road([[p.dx, p.dz]], 0.06)), part("imp.press", lay.yard.x, lay.yard.z, { scale: 1.4 })];
    }
    case "PASTURE": {
      // The paddock (fields.js) where its ground is trodden, the herd inside its rails.
      const p = layoutOf(ctx).paddock;
      const herd = herdOf(ctx.bonus);
      const kinds = herd ? [herd, herd] : ["sheep", "cow"];
      const spots = unit ? [[-0.22, -0.1], [0.22, -0.04], [0.06, -0.22]] : [[-0.17, -0.04], [0.15, -0.12], [0.06, 0.1]];
      // Side-on (their readable profile), a few turned the other way.
      return [part("imp.paddock", p.x, p.z, { yaw: yj(0.1), variant: p.variant, sx: p.sx, sz: p.sz }), ...kinds.map((k, i) => part(`anim.${k}`, p.x + spots[i][0] * p.sx, p.z + spots[i][1] * p.sz, { variant: i, yaw: yj(0.5), scale: 1.15 }))];
    }
    case "CAMP":
      return [
        // A ring of hide tents round the fire, the drying rack at the edge.
        part("imp.tent", -0.15 * flip, -0.16, { variant: 0, scale: 1.75 }),
        part("imp.tent", 0.11 * flip, -0.2, { variant: 1, scale: 1.55 }),
        part("imp.tent", -0.25 * flip, -0.01, { variant: 2, scale: 1.45, yaw: 0.9 * flip + yj(0.3) }),
        part("imp.hiderack", 0.21 * flip, 0.02, { yaw: -0.4 * flip, scale: 1.45 }),
        part("imp.firepit", unit ? 0.18 * flip : -0.03, unit ? -0.06 : -0.04, { variant: 0, scale: 1.4, emit: [{ color: "#ff9a40", size: 0.008, rise: 0.5, count: 3, spread: 0.02, dy: 0.03 }, { color: "#8a8078", size: 0.03, rise: 0.12, count: 3, spread: 0.01, dy: 0.06, smoke: true }] }),
      ];
    case "MINE":
      return part("imp.mine", 0, unit ? -0.18 : -0.06, { param: oreOf(ctx.bonus), scale: 1.2, yaw: yj(0.3) });
    case "QUARRY":
      return part("imp.quarry", 0, unit ? -0.18 : -0.06, { scale: 1.15, yaw: yj(0.3) });
    case "LUMBERMILL":
      return [part("imp.sawmill", -0.1 * flip, -0.17, { scale: 1.4 }), part("imp.logpile", 0.2 * flip, 0.04, { yaw: yj(0.3) + 0.3 * flip, scale: 1.4 })];
    case "WINDMILL":
      return part("imp.windmill", 0.12 * flip, unit ? -0.22 : -0.12, { yaw: yj(0.2), scale: 1.15 });
    case "WORKSHOP":
      return [part("imp.workshop", -0.08 * flip, unit ? -0.22 : -0.14, { scale: 1.4, emit: [{ dx: 0.07, dy: 0.18, dz: -0.01, color: "#ff8a30", size: 0.007, rise: 0.6, count: 3, spread: 0.01 }, { dx: 0.07, dy: 0.19, dz: -0.01, color: "#8a8480", size: 0.035, rise: 0.1, count: 4, spread: 0.01, smoke: true }] })];
    case "FISHING_BOATS":
      return [part("imp.boat", -0.16, -0.12, { variant: r.int(0, 1), yaw: r.range(-0.5, 0.5), float: true, draft: DRAFT, scale: 1.25 }), part("imp.boat", 0.2, 0.08, { variant: r.int(0, 1), yaw: r.range(-0.5, 0.5) + Math.PI * (r() < 0.5 ? 1 : 0), float: true, draft: DRAFT, scale: 1.1 })];
    case "WHALING_BOATS":
      return [part("imp.boat", -0.15, -0.1, { variant: 2, yaw: r.range(-0.5, 0.5), float: true, draft: DRAFT, scale: 1.25 }), part("imp.boat", 0.2, 0.08, { variant: 0, yaw: r.range(-0.5, 0.5) + Math.PI, float: true, draft: DRAFT, scale: 1.05 })];
    case "DUNGEON":
      return part("imp.dungeon", 0, unit ? -0.2 : -0.08, { yaw: yj(0.2), scale: 1.25, emit: [{ dy: 0.05, dz: 0.12, color: "#7ad870", size: 0.01, rise: 0.15, count: 3, spread: 0.05 }] });
    case "BARROW":
      return part("imp.barrow", 0, unit ? -0.2 : -0.08, { yaw: yj(0.2), scale: 1.3, tint: scorch });
    case "GRAVEYARD":
      return [part("imp.graveyard", 0, unit ? -0.14 : -0.04, { yaw: yj(0.15), scale: 1.3, emit: [{ dy: 0.04, color: "#a8c8c0", size: 0.01, rise: 0.12, count: 3, spread: 0.18 }] }), part(hasRecipe("tree.dead") ? "tree.dead" : "rock.boulder", 0.26 * flip, -0.22, { scale: 0.8, yaw: r.range(0, TAU) })];
    case "RUINS":
      return part("imp.ruins", 0, unit ? -0.17 : -0.05, { yaw: yj(0.3), scale: 1.5, tint: scorch });
    case "CITY_RUINS":
      return part("imp.cityruins", 0, unit ? -0.15 : -0.03, { yaw: yj(0.2), scale: 1.1, tint: scorch });
    case "MONSTER_SKELETON":
      return part("imp.bones", 0, unit ? -0.2 : -0.04, { yaw: r.range(0, TAU), scale: 1.35, tint: scorch });
    case "SHIP_WRECK":
      return part("imp.wreck", 0.05, -0.08, { yaw: r.range(0, TAU), float: true, draft: DRAFT, scale: 1.2 });
    case "BEAR_DEN":
    case "LION_DEN":
      return part("imp.den", 0, unit ? -0.2 : -0.08, { param: t === "LION_DEN" ? "lion" : "bear", yaw: yj(0.3), scale: 1.35 });
    case "GOODY_HUT":
      return part("imp.goodyhut", 0.02, unit ? -0.2 : -0.06, { yaw: yj(0.4), scale: 1.45 });
    case "CAGE":
      return part("imp.cage", 0.12, unit ? -0.2 : -0.06, { yaw: yj(0.5), scale: 1.3 });
    case "NECROTOTEM":
      return part("imp.necrototem", 0, unit ? -0.2 : -0.06, { yaw: yj(0.2), scale: 1.7, emit: [{ dy: 0.18, color: "#7ae05a", size: 0.01, rise: 0.2, count: 3, spread: 0.04 }] });
    case "SNAKE_PILLAR":
      return part("imp.snakepillar", 0, unit ? -0.2 : -0.06, { yaw: yj(0.3), scale: 1.6 });
    case "JUNGLE_ALTAR":
      return part("imp.junglealtar", 0, unit ? -0.18 : -0.06, { yaw: yj(0.2), scale: 1.25 });
    case "GUARDIAN":
      return part("imp.guardian", 0, unit ? -0.2 : -0.06, { yaw: yj(0.15), scale: unit ? 1.4 : 1.7 });
    case "PORTAL":
      return part("imp.portal", 0, unit ? -0.2 : -0.06, { yaw: yj(0.15), scale: 1.5, emit: [{ dy: 0.14, color: "#b08aff", size: 0.012, rise: 0.25, count: 4, spread: 0.06 }] });
    case "HELLFIRE":
      return part("imp.hellfire", 0, unit ? -0.18 : -0.04, { yaw: r.range(0, TAU), scale: 1.3, emit: [{ dy: 0.1, color: "#ff5a1a", size: 0.012, rise: 0.4, count: 6, spread: 0.08 }] });
    case "MUSHROOMS": {
      // On their patch of trodden earth (fields.js), at its back when a unit stands on the plot.
      const p = layoutOf(ctx).paddock;
      return part("imp.mushrooms", p.x, p.z + (unit ? -0.06 : 0.06), { yaw: r.range(0, TAU), scale: 1.25 });
    }
    case "PENGUINS":
      return part("imp.penguins", 0.06, unit ? -0.2 : -0.06, { yaw: r.range(0, TAU), scale: 1.3 });
    case "PIRATE_COVE":
    case "PIRATE_HARBOR":
      return part("imp.piratecove", 0, unit ? -0.18 : -0.06, { yaw: yj(0.3), scale: 1.2, float: water });
    // Unique features (landmarks).
    case "MAELSTROM":
      return part("lm.maelstrom", 0, 0, { yaw: r.range(0, TAU), float: true, scale: 1.05 });
    case "SEVEN_PINES": {
      const pine = hasRecipe("tree.pine") ? "tree.pine" : "basic.pine";
      const out = [lm("lm.pinealtar", { emit: [{ dy: 0.08, color: "#9affb0", size: 0.01, rise: 0.15, count: 3, spread: 0.08 }] })];
      for (let k = 0; k < 7; k++) {
        const a = Math.PI / 2 + 0.45 + (k / 7) * (TAU - 0.9);
        out.push(part(pine, Math.cos(a) * 0.34, Math.sin(a) * 0.3 - 0.02, { variant: k, scale: r.range(1.25, 1.5), yaw: r.range(0, TAU) }));
      }
      return out;
    }
    case "DRAGON_BONES":
      return lm("lm.dragonbones", { yaw: r.range(-0.5, 0.5) });
    case "POOL_OF_TEARS":
      return [lm("lm.pooloftears", { emit: [{ dy: 0.03, color: "#8ac8ff", size: 0.012, rise: 0.15, count: 4, spread: 0.12 }] }), part(hasRecipe("tree.willow") ? "tree.willow" : "basic.tree", 0.3, -0.3, { scale: 1.1, yaw: r.range(0, TAU) })];
    case "BRADELINES_WELL":
      return lm("lm.bradeline", { scale: 1.35, emit: [{ dy: 0.06, color: "#9a5aff", size: 0.012, rise: 0.18, count: 4, spread: 0.04 }] });
    case "BROKEN_SEPULCHER":
      return lm("lm.sepulcher", { scale: 1.3, emit: [{ dy: 0.1, color: "#7ad870", size: 0.012, rise: 0.15, count: 3, spread: 0.06 }] });
    case "LETUM_FRIGUS":
      return lm("lm.letum", { emit: [{ dy: 0.1, color: "#c8ecff", size: 0.012, rise: 0.12, count: 4, spread: 0.2 }] });
    case "REMNANTS_OF_PATRIA":
      return lm("lm.patria", { scale: 1.3 });
    case "AIFON_ISLE":
      return { ...lm("lm.aifon", { scale: 1.2 }), float: water };
    case "PYRE_OF_THE_SERAPHIC":
      return lm("lm.pyre", { scale: 1.2, emit: [{ dy: 0.3, color: "#ffd890", size: 0.014, rise: 0.35, count: 6, spread: 0.05 }] });
    case "MIRROR_OF_HEAVEN":
      return lm("lm.mirror", { scale: 1.2, emit: [{ dy: 0.08, color: "#d8ecff", size: 0.012, rise: 0.12, count: 5, spread: 0.15 }] });
    case "ODIOS_PRISON":
      return lm("lm.odio", { emit: [{ dy: 0.1, color: "#ff4a20", size: 0.012, rise: 0.3, count: 4, spread: 0.08 }] });
    case "RING_OF_CARCER":
      return lm("lm.carcer", { emit: [{ dy: 0.03, color: "#ff4a20", size: 0.012, rise: 0.25, count: 4, spread: 0.08 }] });
    case "STANDING_STONES":
      return lm("lm.stones", { scale: 1.1 });
    case "TOMB_OF_SUCELLUS":
      return lm("lm.sucellus", { scale: 1.25, emit: [{ dy: 0.08, dz: 0.06, color: "#c8e890", size: 0.012, rise: 0.15, count: 3, spread: 0.08 }] });
    case "YGGDRASIL":
      return part("lm.yggdrasil", 0, unit ? -0.2 : -0.04, { scale: unit ? 1.0 : 1.22, yaw: r.range(0, TAU), emit: [{ dy: 0.4, color: "#ffe08a", size: 0.014, rise: 0.12, count: 6, spread: 0.3 }] });
    default:
      return lm("lm.monument", { variant: r.int(0, 2), scale: 1.2 });
  }
}

// ------------------------------------------------------------------------------------------------
// Catalog: resource markers

const GOODS = { DYE: "dye", SILK: "silk", INCENSE: "incense", WINE: "wine", FUR: "fur", IVORY: "ivory", REAGENTS: "reagents", GUNPOWDER: "gunpowder", ALE: "ale", FINE_CLOTHES: "clothes" };

function bonusOf(type, ctx) {
  const t = type.replace("BONUS_", "");
  if (t === "MANA" || t.startsWith("MANA_")) {
    // A built node shows the school's crystal itself.
    if (ctx.improvement && /MANA/.test(ctx.improvement)) return null;
    const school = t === "MANA" ? "" : t.slice(5);
    return { asset: "res.mana", param: school, emit: [{ color: glowOf(school), size: 0.012, rise: 0.25, count: 2, spread: 0.04, dy: 0.08 }] };
  }
  const ore = oreOf(type);
  if (ore && ore !== "gems") return { asset: "res.ore", param: ore };
  if (t === "GEMS") return { asset: "res.gems" };
  if (t === "JEWELS") return { asset: "res.gems", param: "jewels" };
  if (GOODS[t]) return { asset: "res.goods", param: GOODS[t] };
  switch (t) {
    case "MARBLE": return { asset: "res.marble" };
    case "WHEAT": return { asset: "res.wheat" };
    case "CORN": return { asset: "res.corn" };
    case "RICE": return { asset: "res.rice" };
    case "BANANA": return { asset: "res.banana" };
    case "SUGAR": return { asset: "res.sugar" };
    case "COTTON": return { asset: "res.cotton" };
    case "COW": case "HORSE": case "SHEEP": case "PIG": case "DEER":
      return { asset: `res.${t.toLowerCase()}` };
    case "FISH": return { asset: "res.fish" };
    case "CLAM": return { asset: "res.clam" };
    case "PEARL": return { asset: "res.pearl" };
    case "CRAB": return { asset: "res.crab" };
    case "WHALE": return { asset: "res.whale" };
    case "TOAD": return { asset: "res.toad" };
    case "NIGHTMARE": return { asset: "res.nightmare", emit: [{ color: "#ff4a20", size: 0.008, rise: 0.3, count: 2, spread: 0.04, dy: 0.1 }] };
    case "RAZORWEED": return { asset: "res.razorweed" };
    case "GULAGARM": return { asset: "res.gulagarm" };
    case "FRUIT_OF_YGGDRASIL": return { asset: "res.yggfruit", emit: [{ color: "#ffd060", size: 0.01, rise: 0.15, count: 2, spread: 0.04, dy: 0.12 }] };
    case "SHEUT_STONE": return { asset: "res.sheut" };
    default: return { asset: "res.goods", param: "", scale: 0.8 };
  }
}

// ================================================================================================
// Catalog
// ================================================================================================

extendCatalog({
  rocks: (ctx) => rocksOf(ctx),
  feature: (feat, ctx) => featureOf(feat, ctx),
  improvement: (type, ctx) => improvementOf(type, ctx),
  bonus: (type, ctx) => bonusOf(type, ctx),
  // A stone bridge where a road crosses a river, by the crossing: an arch reads only from the side,
  // so mostly arches where the road runs east-west (its side toward the camera), mostly flat slab
  // bridges where it runs north-south.
  bridge: (ctx) => {
    const h = hashf(ctx.i, ctx.crossing ?? 0, 0x5b1d), v = hashf(ctx.i, ctx.crossing ?? 0, 0x5b1e);
    return { asset: (ctx.alongX ? h < 0.75 : h < 0.25) ? "bridge.arch" : "bridge.slab", variant: v < 0.65 ? 0 : 1 };
  },
});

void TEAM;
void TEAM_SHADE;
