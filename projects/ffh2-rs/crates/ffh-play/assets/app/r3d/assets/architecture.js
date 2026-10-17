// architecture.js: the buildings of the 3D map and the catalog's `family`, `city` and `settlement`
// entries (docs/R3D.md "Asset recipes").
//
// Visual language: every building is a readable silhouette first -- a plinth sunk below the ground
// (to y = -0.05), walls in two colour masses (a stone or dark lower course under plaster, timber,
// hide or living wood), and a big, steep, overhanging roof that is the dominant colour mass from
// the oblique camera (terracotta, slate, thatch, leaf-green and bronze, obsidian, turf). Roofs have
// bell-cast eaves (so the walls stay in view), a darker slab edge, and are painted darker toward the
// eaves. Detail is spent where it reads at gameplay zoom: chimneys, gables turned to the street,
// towers with conical and spire roofs, a few lit windows, banners in the owner's colour; fine
// detail (timber studs, merlons, slits) only adds texture up close. Each family has its own palette
// and shape grammar: human (half-timber, steep tile and slate, spires), elven (pods with petal-bud
// roofs held in living wood, a golden-crowned great tree), dwarven (battered stone, earth berms,
// heavy low slate, forge glow), infernal (faceted obsidian shards, bone, chains, lava-lit slits, a
// planar rift), tribal (hide tents, round huts, horned longhouses, palisades, totems, skulls),
// desert (sandstone cubes, domes, minarets) and coastal (planks on stilts, sails, crow's nests).
//
// Settlements are composed, not scattered (`cityLayout`, with the `Plan` packer below): a focal
// landmark at the back (north, so it never hides the square), an open square in front where the
// garrison stands (plot-local (0, 0.3) and the way in kept clear), a ring street with buildings
// lined up wall to wall along both sides and turned to face it (oriented footprints from the
// meshes' own bounds, so roofs do not run into each other), halls on the square, towers at the
// back, and a wall ring with towers and a gate toward the camera (+z). Families differ in gaps,
// how freely buildings turn, the square's furniture (stalls and a well; a fire and totems; braziers;
// trees from vegetation.js among elven pods) and particles (embers, forge sparks, lantern motes,
// arcane motes).
//
// Families (the recipes' param, from `family(civType)`; a ".sub" suffix picks a sub-style that
// `city` and `settlement` derive from the civilization): human ("human.arcane" for the Amurites and
// Sidar), elven ("elven.dark" for the Svartalfar), dwarven, infernal ("infernal.gothic" for the
// Calabim: the human builders in a dark, steep key), tribal ("tribal.north" for the Doviello and
// Illians: turf roofs), desert, coastal.
//
// Recipes (param = family[.sub]): bld.house (6 variants), bld.hall (3), bld.tower (3), bld.keep
// (2: keep / landmark, capital palace), bld.wall (2: a segment along x of length 0.2, outer face +z;
// a half segment), bld.gate (width 0.2, outer face +z), bld.walltower (2), and the common pieces
// bld.well, bld.stall (3), bld.banner (2: hanging banner, swallow-tailed standard), bld.fence
// (along x, length 0.2), bld.totem, bld.brazier.

import { recipe, buildAsset, hasRecipe } from "./registry.js";
import { extendCatalog } from "./catalog.js";
import { MAT, FLOATS } from "../mesh.js";
import { pal, jitter } from "./palette.js";
import { noise3 } from "../noise.js";

const TAU = Math.PI * 2;
const QT = Math.PI / 2;

// --- colour ---------------------------------------------------------------------------------------

const C = (c) => (typeof c === "string" ? pal(c) : c);
const mulc = (c, k) => {
  const v = C(c);
  return [v[0] * k, v[1] * k, v[2] * k];
};
const J = (c, r, a = 0.05) => jitter(C(c), r, a);
const TEAM = "#d8d8d8";
const TEAM_SHADE = "#9a9a9a";

/** A weighted pick from [[item, weight], ...]. */
function wpick(r, list) {
  let t = r() * list.reduce((a, e) => a + e[1], 0);
  for (const e of list) if ((t -= e[1]) <= 0) return e[0];
  return list[list.length - 1][0];
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

/** The four walls of a w x d block from y0 to y1, no top or bottom (8 triangles); `top` tapers. */
function shell(mb, w, d, y0, y1, top = 1) {
  const x = w / 2, z = d / 2, tx = x * top, tz = z * top;
  const b = [[-x, y0, -z], [x, y0, -z], [x, y0, z], [-x, y0, z]];
  const t = [[-tx, y1, -tz], [tx, y1, -tz], [tx, y1, tz], [-tx, y1, tz]];
  for (let i = 0; i < 4; i++) {
    const j = (i + 1) % 4;
    mb.quad(b[i], t[i], t[j], b[j]);
  }
}

/** A block without its bottom face (10 triangles), from y0 to y1. */
function blk(mb, w, d, y0, y1, top = 1) {
  shell(mb, w, d, y0, y1, top);
  const tx = (w / 2) * top, tz = (d / 2) * top;
  mb.quad([-tx, y1, -tz], [-tx, y1, tz], [tx, y1, tz], [tx, y1, -tz]);
}

/** A flat rectangle in the xy plane at z (doors, windows, bands, banners on a face). */
function rect(mb, x0, y0, x1, y1, z = 0) {
  mb.quad([x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]);
}

/** Runs f(span) on wall face `side` (0 front +z, 1 east +x, 2 back -z, 3 west -x) of a w x d
 *  block: local x runs along the face, y up, z = 0 just proud of it. */
function face(mb, side, w, d, f, proud = 0.0012) {
  const half = side % 2 ? w / 2 : d / 2;
  mb.part(() => {
    mb.rotateY(side * QT);
    mb.translate(0, 0, half + proud);
    f(side % 2 ? d : w);
  });
}

/** A vertical cylinder wall (open), r0 at y0 to r1 at y1. */
function drum(mb, r0, r1, y0, y1, seg = 8, phase = 0) {
  mb.lathe([[r0, y0], [r1, y1]], seg, { caps: false, phase });
}

/** A window (lit: emissive warm light; else dark glass); `arch` adds a pointed top. */
function win(mb, S, x, y, w, h, lit, arch = 0) {
  mb.part(() => {
    if (lit) {
      mb.mat = MAT.EMISSIVE;
      mb.color = C(S.lit);
    } else mb.color = C(S.glass);
    rect(mb, x - w / 2, y, x + w / 2, y + h);
    if (arch) mb.tri([x - w / 2, y + h, 0], [x + w / 2, y + h, 0], [x, y + h + arch, 0]);
  });
}

/** A door (dark wood), optionally with a pointed or round head. */
function door(mb, S, x, w, h, arch = 0, y = 0.004, c = null) {
  mb.part(() => {
    mb.color = C(c ?? S.door);
    rect(mb, x - w / 2, y, x + w / 2, y + h);
    if (arch) mb.tri([x - w / 2, y + h, 0], [x + w / 2, y + h, 0], [x, y + h + arch, 0]);
  });
}

/** A gable roof over a w (x) by d (z) wall top at y = 0, ridge along x at height h. Past the wall
 *  line the eaves overhang by `over` at a flatter pitch (`kick`: a bell-cast eave that keeps the
 *  walls in view), the verges by `overX`; `thick` gives the slab a darker edge, `sag` bows the
 *  ridge, `sweep` lifts the eaves' ends (elven), `ends` fills the gable triangles with a colour.
 *  The slopes are painted darker toward the eaves. */
function gableRoof(mb, w, d, h, o = {}) {
  const { over = 0.014, overX = 0.012, thick = 0.007, sag = 0, sweep = 0, ends = null, edge = null, n: nseg = 0, kick = 0.42, shade = 0.24 } = o;
  const hz = d / 2, X = w / 2 + overX;
  const n = nseg || (sag || sweep ? 4 : 1);
  const roofC = mb.color, edgeC = edge ? C(edge) : mulc(roofC, 0.55);
  const m0 = mb.mark();
  const R = (t) => h - sag * Math.sin(Math.PI * t);
  const sw = (t) => sweep * (2 * t - 1) ** 2;
  const W = (t) => sw(t) * 0.35;
  const E = (t) => -((h * over) / hz) * kick + sw(t);
  const ZZ = (t) => hz + over + sw(t) * 0.5;
  const ZW = (t) => hz + sw(t) * 0.12;
  for (let i = 0; i < n; i++) {
    const t0 = i / n, t1 = (i + 1) / n;
    const xa = -X + 2 * X * t0, xb = -X + 2 * X * t1;
    for (const s of [-1, 1]) {
      mb.quad([xa, R(t0), 0], [xb, R(t1), 0], [xb, W(t1), s * ZW(t1)], [xa, W(t0), s * ZW(t0)]);
      mb.quad([xa, W(t0), s * ZW(t0)], [xb, W(t1), s * ZW(t1)], [xb, E(t1), s * ZZ(t1)], [xa, E(t0), s * ZZ(t0)]);
      if (thick) {
        mb.color = edgeC;
        mb.quad([xa, E(t0), s * ZZ(t0)], [xb, E(t1), s * ZZ(t1)], [xb, E(t1) - thick, s * ZZ(t1)], [xa, E(t0) - thick, s * ZZ(t0)]);
        mb.color = roofC;
      }
    }
  }
  if (thick) {
    mb.color = edgeC;
    for (const e of [-1, 1]) {
      const t = e < 0 ? 0 : 1;
      for (const s of [-1, 1]) {
        mb.quad([e * X, R(t), 0], [e * X, W(t), s * ZW(t)], [e * X, W(t) - thick, s * ZW(t)], [e * X, R(t) - thick, 0]);
        mb.quad([e * X, W(t), s * ZW(t)], [e * X, E(t), s * ZZ(t)], [e * X, E(t) - thick, s * ZZ(t)], [e * X, W(t) - thick, s * ZW(t)]);
      }
    }
    mb.color = roofC;
  }
  if (shade) roofShade(mb, m0, mb.xp([0, E(0.5) - thick, 0])[1], mb.xp([0, h, 0])[1], shade);
  if (ends) {
    mb.color = C(ends);
    const top = R(overX / (2 * X)) - 0.001;
    for (const e of [-1, 1]) mb.tri([(e * w) / 2, 0, -hz], [(e * w) / 2, 0, hz], [(e * w) / 2, top, 0]);
    mb.color = roofC;
  }
}

/** Paints a roof from m0 on darker toward its eaves (asset-space heights y0 eaves .. y1 ridge). */
function roofShade(mb, m0, y0, y1, amt = 0.24) {
  mb.tint(m0, (x, y) => 1 - amt + amt * 1.25 * Math.max(0, Math.min(1, (y - y0) / (y1 - y0 || 1))));
}

/** A hipped roof (a pyramid when `ridge` is 0) over a w x d wall top at y = 0, ridge along x. */
function hipRoof(mb, w, d, h, o = {}) {
  const { over = 0.012, ridge = 0, thick = 0.006, edge = null } = o;
  const hx = w / 2, hz = d / 2;
  const run = Math.min(hx, hz);
  const ye = -((h * over) / run) * 0.5;
  const m0 = mb.mark();
  const X = hx + over, Z = hz + over, rr = Math.min(ridge / 2, Math.max(0, hx - hz * 0.6));
  const a = [-X, ye, -Z], b = [X, ye, -Z], c = [X, ye, Z], e = [-X, ye, Z];
  const roofC = mb.color;
  if (rr > 1e-4) {
    const t0 = [-rr, h, 0], t1 = [rr, h, 0];
    mb.quad(a, t0, t1, b);
    mb.quad(e, c, t1, t0);
    mb.tri(e, t0, a);
    mb.tri(b, t1, c);
  } else {
    const t = [0, h, 0];
    mb.tri(a, t, b);
    mb.tri(b, t, c);
    mb.tri(c, t, e);
    mb.tri(e, t, a);
  }
  if (thick) {
    mb.color = edge ? C(edge) : mulc(roofC, 0.55);
    const P = [a, b, c, e];
    for (let i = 0; i < 4; i++) {
      const p = P[i], q = P[(i + 1) % 4];
      mb.quad(p, q, [q[0], q[1] - thick, q[2]], [p[0], p[1] - thick, p[2]]);
    }
    mb.color = roofC;
  }
  roofShade(mb, m0, mb.xp([0, ye - thick, 0])[1], mb.xp([0, h, 0])[1]);
}

/** A conical roof of radius R (eaves) rising h above y = 0; `curve` hollows the profile (witch's
 *  hat), `drop` lowers the eaves, `kick` flares them. */
function coneRoof(mb, R, h, o = {}) {
  const { seg = 8, curve = 0.3, drop = 0.008, thick = 0.006, kick = 0.1, phase = 0, rings = 3 } = o;
  const prof = [];
  if (thick) prof.push([R * 0.97, -drop - thick]);
  prof.push([R, -drop]);
  for (let i = 1; i <= rings; i++) {
    const t = i / (rings + 1);
    const lin = 1 - t;
    const k = 1 - curve * Math.sin(Math.PI * Math.min(1, t * 1.15)) * 0.55 - kick * (1 - Math.exp(-t * 6)) * 0.5;
    prof.push([R * lin * k, -drop + (h + drop) * t]);
  }
  prof.push([0, h]);
  mb.lathe(prof, seg, { caps: false, phase });
}

/** An onion dome over radius r at y = 0, height h. */
function onion(mb, r, h, seg = 8) {
  mb.lathe([[r * 0.92, -0.002], [r * 1.18, h * 0.22], [r * 1.12, h * 0.45], [r * 0.6, h * 0.72], [r * 0.16, h * 0.9], [0, h]], seg, { caps: false });
}

/** A dome (half sphere-ish) of radius r, height h. */
function dome(mb, r, h, seg = 10) {
  mb.lathe([[r, -0.002], [r * 0.97, h * 0.3], [r * 0.82, h * 0.62], [r * 0.5, h * 0.9], [0, h]], seg, { caps: false });
}

/** Merlons along a line on x from x0 to x1 at height y, depth md, centred at z. */
function merlonsLine(mb, x0, x1, y, z, o = {}) {
  const { mw = 0.016, mh = 0.014, md = 0.009, n = 0 } = o;
  const L = x1 - x0;
  const k = n || Math.max(2, Math.round(L / (mw * 2)));
  for (let i = 0; i < k; i++) {
    const x = x0 + (L * (i + 0.5)) / k;
    at(mb, x, 0, z, 0, () => blk(mb, mw, md, y, y + mh));
  }
}

/** Merlons on the four edges of a w x d top at height y. */
function merlonsRect(mb, w, d, y, o = {}) {
  const md = o.md ?? 0.009;
  for (let s = 0; s < 4; s++) {
    const span = s % 2 ? d : w, half = s % 2 ? w / 2 : d / 2;
    mb.part(() => {
      mb.rotateY(s * QT);
      merlonsLine(mb, -span / 2 + md, span / 2 - md, y, half - md / 2, o);
    });
  }
}

/** Merlons around a circle of radius rad at height y. */
function merlonsRing(mb, rad, y, n, o = {}) {
  const { mw = 0.014, mh = 0.013, md = 0.008, phase = 0 } = o;
  for (let k = 0; k < n; k++) {
    const a = phase + (k / n) * TAU;
    at(mb, Math.sin(a) * rad, 0, Math.cos(a) * rad, a, () => blk(mb, mw, md, y, y + mh));
  }
}

/** A chimney stack from y0 to y1 at (x, z) with a cap; `glow` lights its mouth (forges). */
function chimney(mb, c, x, z, y0, y1, s = 0.016, glow = null) {
  mb.part(() => {
    mb.color = C(c);
    at(mb, x, 0, z, 0, () => {
      blk(mb, s, s, y0, y1);
      mb.color = mulc(c, 0.8);
      blk(mb, s * 1.35, s * 1.35, y1, y1 + 0.006);
      if (glow) {
        mb.mat = MAT.EMISSIVE;
        mb.color = C(glow);
        const g = s * 0.42;
        mb.quad([-g, y1 + 0.0065, -g], [-g, y1 + 0.0065, g], [g, y1 + 0.0065, g], [g, y1 + 0.0065, -g]);
      }
    });
  });
}

/** Wind on cloth: vertices from m0 on sway by their distance from the fixed asset-space point p,
 *  reaching `max` at distance `len`. */
function clothSway(mb, m0, p, len, max = 0.6) {
  const v = mb.v;
  for (let i = m0 * FLOATS; i < v.length; i += FLOATS) {
    const d = Math.hypot(v[i] - p[0], v[i + 1] - p[1], v[i + 2] - p[2]);
    v[i + 10] = Math.max(v[i + 10], max * Math.min(1, d / len));
  }
}

/** Hanging cloth: sway grows with the drop below asset-space height `top`. */
function hangSway(mb, m0, top, len, max = 0.35) {
  const v = mb.v;
  for (let i = m0 * FLOATS; i < v.length; i += FLOATS) v[i + 10] = Math.max(v[i + 10], max * Math.max(0, Math.min(1, (top - v[i + 1]) / len)));
}

/** A banner in the owner's colour hanging on a face (face coordinates): width w from yTop down by
 *  h, with a pointed tail and a dark rod. */
function wallBanner(mb, x, yTop, w, h, z = 0.002) {
  const m0 = mb.mark();
  const top = mb.xp([x, yTop, z])[1];
  mb.part(() => {
    mb.mat = MAT.TEAM;
    mb.color = C(TEAM);
    const yb = yTop - h;
    mb.quad([x - w / 2, yb, z], [x + w / 2, yb, z], [x + w / 2, yTop, z], [x - w / 2, yTop, z]);
    mb.tri([x - w / 2, yb, z], [x, yb - w * 0.5, z], [x + w / 2, yb, z]);
    mb.color = C(TEAM_SHADE);
    rect(mb, x - w * 0.3, yb + h * 0.18, x + w * 0.3, yb + h * 0.3, z + 0.0005);
  });
  hangSway(mb, m0, top, h * 1.5, 0.22);
  mb.part(() => {
    mb.color = C("#3a2c22");
    rect(mb, x - w * 0.62, yTop - 0.002, x + w * 0.62, yTop + 0.003, z + 0.0008);
  });
}

/** A flag on a pole top: a team-coloured pennant at (0, y, 0) flying toward +x. */
function pennant(mb, y, len = 0.045, h = 0.016) {
  const m0 = mb.mark();
  const p = mb.xp([0, y, 0]);
  mb.part(() => {
    mb.mat = MAT.TEAM;
    mb.color = C(TEAM);
    mb.quad([0, y, 0], [0, y - h, 0], [len * 0.55, y - h * 0.82, 0.003], [len * 0.55, y - h * 0.08, 0.003]);
    mb.tri([len * 0.55, y - h * 0.08, 0.003], [len * 0.55, y - h * 0.82, 0.003], [len, y - h * 0.62, 0.006]);
  });
  clothSway(mb, m0, p, len, 0.9);
}

/** A thin pole (finial) with a ball. */
function finial(mb, y0, h, c = "#c9a043", r = 0.003) {
  mb.part(() => {
    mb.mat = MAT.METAL;
    mb.color = C(c);
    mb.translate(0, y0, 0);
    mb.cylinder(r, r * 0.6, h, 4, { caps: false });
    at(mb, 0, h, 0, 0, () => mb.sphere(r * 1.8, 0));
  });
}

/** Contact shadow and grime toward the ground for everything from m0 on. */
function ground(mb, m0, h = 0.05, min = 0.55) {
  mb.heightAO(m0, -0.03, h, min);
}

/** A low-poly diamond (lantern, crystal): an octahedron of radius r, height h. */
function gem(mb, r, h = r * 1.6) {
  const P = [[r, 0, 0], [0, 0, r], [-r, 0, 0], [0, 0, -r]];
  for (let i = 0; i < 4; i++) {
    const a = P[i], b = P[(i + 1) % 4];
    mb.tri(a, b, [0, h, 0]);
    mb.tri(b, a, [0, -h, 0]);
  }
}

// --- styles -----------------------------------------------------------------------------------------
// Each family's palette (sRGB hex; resolved per building with a little jitter).

const STY = {
  human: {
    wall: ["#d8cbac", "#dccfb4", "#cfc0a0", "#d4cab2", "#d8c8a2"],
    stone: ["#958c80", "#8a8278", "#9c9282"],
    stoneLight: ["#b4ab9a", "#aea594", "#b8ae9a"],
    timber: "#3e2c20",
    roof: [["#7a4636", 3.2], ["#6a3c2e", 2], ["#7e5a40", 1.2], ["#566070", 1.4], ["#847250", 1.3]],
    slate: "#5c6676",
    slateDark: "#4a5464",
    thatch: "#8a7650",
    glass: "#2a282c",
    lit: "#ffbf6a",
    door: "#4a3020",
    chimney: "#857c72",
    spire: "#505a6a",
    pitch: 1,
    litP: 0.32,
  },
  gothic: {
    wall: ["#6e6468", "#625a60", "#74686a"],
    stone: ["#4a4448", "#423c42", "#504a4c"],
    stoneLight: ["#6e6668", "#625a5e"],
    timber: "#24181c",
    roof: [["#2c2c38", 3], ["#34262e", 2], ["#3a3440", 1.5]],
    slate: "#2c2c38",
    slateDark: "#22222c",
    thatch: "#3a3440",
    glass: "#1c1418",
    lit: "#d8704e",
    door: "#2a1416",
    chimney: "#3e383c",
    spire: "#26242e",
    pitch: 1.5,
    litP: 0.2,
  },
};

const FAMILIES = ["human", "elven", "dwarven", "infernal", "tribal", "desert", "coastal"];

/** The family and sub-style of a recipe param ("elven.dark" -> elven, dark). */
function famOf(param) {
  const [f, sub = ""] = String(param || "human").split(".");
  return { f: FAMILIES.includes(f) ? f : "human", sub };
}

// --- human: half-timbered towns, stone keeps, slate spires --------------------------------------------

/** The palette of one human building (gothic for the Calabim). */
function humanStyle(r, gothic) {
  const B = gothic ? STY.gothic : STY.human;
  return {
    ...B,
    wallC: J(r.pick(B.wall), r, 0.04),
    stoneC: J(r.pick(B.stone), r, 0.05),
    lightC: J(r.pick(B.stoneLight), r, 0.04),
    roofC: J(wpick(r, B.roof), r, 0.06),
    gothic,
  };
}

/** Windows along a face of span `span` between x0..x1 for a storey at y (h tall). */
function windowRow(mb, r, S, span, y, h, { n = 0, w = 0.012, skip = null, arch = 0 } = {}) {
  const k = n || Math.max(1, Math.floor(span / 0.042));
  for (let i = 0; i < k; i++) {
    const x = -span / 2 + (span * (i + 0.5)) / k;
    if (skip !== null && Math.abs(x - skip) < 0.02) continue;
    win(mb, S, x, y, w, h, r() < S.litP, arch);
  }
}

/** Timber framing on a face: corner posts, studs between the windows, a sill. */
function framing(mb, S, span, y0, y1, k) {
  mb.part(() => {
    mb.color = C(S.timber);
    const pw = 0.0055;
    rect(mb, -span / 2, y0, -span / 2 + pw, y1, 0.0004);
    rect(mb, span / 2 - pw, y0, span / 2, y1, 0.0004);
    for (let i = 1; i < k; i++) {
      const x = -span / 2 + (span * i) / k;
      rect(mb, x - pw / 2, y0, x + pw / 2, y1, 0.0004);
    }
    rect(mb, -span / 2, y0, span / 2, y0 + 0.004, 0.0004);
  });
}

/** A rectangular human house body: plinth, ground storey, an optional jettied, timber-framed upper
 *  storey, windows, a door on the front. Returns {w, d, y} of the wall top (the roof's base). */
function humanBody(mb, r, S, w, d, o) {
  const { h1, h2 = 0, jetty = 0, lower = "wall", frame = true, doorX = 0, doorSide = 0, gableZ = false } = o;
  // Plinth.
  mb.color = mulc(S.stoneC, 0.85);
  blk(mb, w + 0.008, d + 0.008, -0.05, 0.01);
  // Ground storey.
  const lowC = lower === "stone" ? S.stoneC : lower === "light" ? S.lightC : S.wallC;
  mb.color = lowC;
  shell(mb, w, d, 0.0, h1);
  const w2 = w + jetty * 0.8, d2 = d + jetty * 2;
  if (h2) {
    mb.color = C(S.timber);
    shell(mb, w2 + 0.003, d2 + 0.003, h1 - 0.003, h1 + 0.004);
    mb.color = S.wallC;
    shell(mb, w2, d2, h1, h1 + h2);
  }
  const top = h1 + h2;
  for (let s = 0; s < 4; s++) {
    const isEnd = gableZ ? s % 2 === 0 : s % 2 === 1;
    face(mb, s, w, d, (span) => {
      const k = Math.max(1, Math.floor(span / 0.042));
      if (frame && lower === "wall" && !h2) framing(mb, S, span, 0.004, h1, k);
      const dx = s === doorSide ? doorX : null;
      windowRow(mb, r, S, span, h1 * 0.42, Math.min(0.016, h1 * 0.3), { n: isEnd ? 1 : k, skip: dx });
      if (s === doorSide) door(mb, S, doorX, 0.017, Math.min(0.032, h1 * 0.62), S.gothic ? 0.008 : 0);
    });
    if (h2)
      face(mb, s, w2, d2, (span) => {
        const k = Math.max(1, Math.floor(span / 0.042));
        if (frame) framing(mb, S, span, h1, top, k);
        windowRow(mb, r, S, span, h1 + h2 * 0.3, Math.min(0.016, h2 * 0.36), { n: isEnd ? 1 : k, arch: S.gothic ? 0.008 : 0 });
      });
  }
  return { w: h2 ? w2 : w, d: h2 ? d2 : d, y: top };
}

/** A roof on a body {w, d, y}: kind "x" (gable, ridge along x), "z" (gable facing front), "hip",
 *  "pyr", "thatch" (soft gable). */
function humanRoof(mb, r, S, b, kind, pitch, o = {}) {
  const P = pitch * S.pitch;
  mb.part(() => {
    mb.translate(0, b.y, 0);
    mb.color = o.color ?? S.roofC;
    const ends = o.ends ?? S.wallC;
    if (kind === "x" || kind === "thatch") {
      const th = kind === "thatch";
      gableRoof(mb, b.w, b.d, P, { over: th ? 0.018 : 0.014, overX: th ? 0.01 : 0.012, thick: th ? 0.012 : 0.006, sag: o.sag ?? (th ? 0.006 : r.range(0, 0.004)), ends });
      if (th) {
        mb.color = mulc(S.roofC, 0.7);
        mb.tube([[-b.w / 2 - 0.006, P - 0.002, 0], [0, P - 0.006, 0], [b.w / 2 + 0.006, P - 0.002, 0]], 0.0065, 4);
      }
    } else if (kind === "z") {
      mb.rotateY(QT);
      gableRoof(mb, b.d, b.w, P, { over: 0.013, overX: 0.012, thick: 0.006, sag: o.sag ?? r.range(0, 0.003), ends });
    } else if (kind === "hip") {
      hipRoof(mb, b.w, b.d, P, { over: 0.013, ridge: o.ridge ?? b.w * 0.5 });
    } else {
      hipRoof(mb, b.w, b.d, P, { over: 0.012, ridge: 0 });
    }
  });
}

function humanHouse(mb, r, v, sub) {
  const S = humanStyle(r, sub === "gothic");
  const chim = (x, z, y0, y1) => chimney(mb, S.chimney, x, z, y0, y1, 0.016);
  switch (v) {
    case 0: {
      // A cottage: low plastered walls under a deep thatch (or tile) roof, a stone end chimney.
      const w = r.range(0.11, 0.125), d = r.range(0.078, 0.088);
      const b = humanBody(mb, r, S, w, d, { h1: r.range(0.056, 0.064), doorX: r.range(-0.02, 0.02) });
      const thatch = !S.gothic && r() < 0.65;
      if (thatch) S.roofC = J(S.thatch, r, 0.06);
      humanRoof(mb, r, S, b, thatch ? "thatch" : "x", 0.085);
      mb.color = S.stoneC;
      at(mb, (r.sign() * w) / 2 + 0.0, 0, r.range(-0.01, 0.01), 0, () => blk(mb, 0.022, 0.024, -0.04, b.y + 0.085 * S.pitch + 0.012, 0.85));
      break;
    }
    case 1: {
      // A gable-fronted townhouse: stone ground floor, jettied timber upper floor, steep roof.
      const w = r.range(0.085, 0.095), d = r.range(0.1, 0.112);
      const b = humanBody(mb, r, S, w, d, { h1: 0.045, h2: 0.045, jetty: 0.007, lower: "stone", gableZ: true, doorX: r.range(-0.015, 0.015) });
      humanRoof(mb, r, S, b, "z", 0.09);
      chim(r.range(-0.02, 0.02), -b.d * 0.25, b.y, b.y + 0.09 * S.pitch * 0.6 + 0.02);
      break;
    }
    case 2: {
      // An L-shaped house: a main range and a gable-fronted wing toward the street.
      const w = r.range(0.12, 0.135), d = r.range(0.075, 0.085);
      const s = r.sign();
      mb.part(() => {
        mb.translate(0, 0, -0.012);
        const b = humanBody(mb, r, S, w, d, { h1: 0.07, doorX: -s * 0.03, lower: r() < 0.5 ? "stone" : "wall" });
        humanRoof(mb, r, S, b, "x", 0.08);
        chim(-s * w * 0.3, 0, b.y, b.y + 0.08 * S.pitch + 0.012);
      });
      mb.part(() => {
        mb.translate(s * w * 0.27, 0, 0.035);
        const b = humanBody(mb, r, S, 0.06, 0.07, { h1: 0.062, gableZ: true, doorX: 0, doorSide: 1, frame: false });
        humanRoof(mb, r, S, b, "z", 0.065);
      });
      break;
    }
    case 3: {
      // A gable-fronted stone house under slate, a heavy chimney stack on its back gable.
      const w = r.range(0.09, 0.1), d = r.range(0.115, 0.125);
      S.roofC = J(r() < 0.6 ? S.slate : S.roofC, r, 0.05);
      const b = humanBody(mb, r, S, w, d, { h1: 0.075, lower: "light", frame: false, gableZ: true, doorX: r.range(-0.015, 0.015) });
      humanRoof(mb, r, S, b, "z", 0.08, { ends: S.lightC });
      mb.color = S.lightC;
      at(mb, r.range(-0.02, 0.02), 0, -d / 2 - 0.006, 0, () => blk(mb, 0.026, 0.02, -0.05, b.y + 0.08 * S.pitch + 0.016, 0.9));
      break;
    }
    case 4: {
      // A long house with a dormer gable over the door.
      const w = r.range(0.145, 0.16), d = r.range(0.078, 0.086);
      const dx = r.range(-0.02, 0.02);
      const b = humanBody(mb, r, S, w, d, { h1: 0.064, doorX: dx, lower: r() < 0.4 ? "stone" : "wall" });
      humanRoof(mb, r, S, b, "x", 0.08);
      // The dormer: a small front wall and its own gable, set into the front slope.
      mb.part(() => {
        mb.translate(dx, b.y - 0.01, d / 2 - 0.012);
        mb.color = S.wallC;
        shell(mb, 0.036, 0.028, 0, 0.038);
        face(mb, 0, 0.036, 0.028, () => win(mb, S, 0, 0.012, 0.012, 0.014, r() < 0.6));
        mb.translate(0, 0.038, 0);
        mb.rotateY(QT);
        mb.color = S.roofC;
        gableRoof(mb, 0.028, 0.036, 0.03 * S.pitch, { over: 0.006, overX: 0.008, thick: 0.004, ends: S.wallC });
      });
      chim(-dx * 0.5 + r.sign() * 0.05, -0.012, b.y, b.y + 0.08 * S.pitch + 0.01);
      break;
    }
    default: {
      // A narrow tower-house: two storeys under a steep pyramid of slate.
      const w = r.range(0.082, 0.09), d = r.range(0.082, 0.09);
      S.roofC = J(r() < 0.55 ? S.slate : S.roofC, r, 0.05);
      const b = humanBody(mb, r, S, w, d, { h1: 0.05, h2: 0.05, jetty: 0.006, lower: "stone", doorX: 0 });
      humanRoof(mb, r, S, b, "pyr", 0.085);
      mb.part(() => {
        mb.translate(0, b.y + 0.085 * S.pitch, 0);
        finial(mb, 0, 0.018);
      });
      chim(b.w * 0.22, -b.d * 0.2, b.y, b.y + 0.06 * S.pitch + 0.02);
      break;
    }
  }
  ground(mb, 0);
}

/** A round stone tower: drum walls, a corbelled parapet, slit windows, a cone or spire roof. */
function roundTower(mb, r, S, o) {
  const { rad, h, roofH, roofC = S.roofC, seg = 10, curve = 0.35, crenel = false, flag = true, slits = 3, wallC = S.lightC, taper = 0.9 } = o;
  mb.color = mulc(wallC, 0.86);
  drum(mb, rad * 1.08, rad * 1.02, -0.05, 0.012, seg);
  mb.color = wallC;
  drum(mb, rad, rad * taper, 0.012, h, seg);
  // Stone courses: a slightly darker band.
  mb.color = mulc(wallC, 0.9);
  drum(mb, rad * (1 - (1 - taper) * 0.45) + 0.0008, rad * (1 - (1 - taper) * 0.55) + 0.0008, h * 0.45, h * 0.55, seg);
  const top = rad * taper;
  // Corbelled ring.
  mb.color = mulc(wallC, 0.95);
  mb.lathe([[top, h - 0.004], [top * 1.16, h + 0.01], [top * 1.16, h + 0.022]], seg, { caps: false });
  if (crenel) {
    mb.color = wallC;
    mb.lathe([[top * 1.16, h + 0.022], [top * 0.96, h + 0.022]], seg, { caps: false });
    merlonsRing(mb, top * 1.1, h + 0.022, Math.round(seg * 0.8), { mw: top * 0.5, mh: 0.014, md: 0.008 });
  }
  // Slit windows and a door, on the front half.
  for (let k = 0; k < slits; k++) {
    const a = r.range(-1.2, 1.2) + (k - (slits - 1) / 2) * 0.9;
    const y = h * (0.3 + 0.55 * (k / Math.max(1, slits - 1)));
    const rr = rad * (1 - (1 - taper) * (y / h)) + 0.001;
    at(mb, Math.sin(a) * rr, 0, Math.cos(a) * rr, a, () => win(mb, S, 0, y, 0.008, 0.02, r() < S.litP * 1.2, S.gothic ? 0.006 : 0.004));
  }
  at(mb, 0, 0, rad + 0.001, 0, () => door(mb, S, 0, 0.018, 0.03, 0.009));
  if (roofH > 0) {
    mb.part(() => {
      mb.translate(0, h + (crenel ? 0.012 : 0.022), 0);
      mb.color = roofC;
      coneRoof(mb, top * (crenel ? 1.0 : 1.3), roofH, { seg, curve, drop: crenel ? 0 : 0.01 });
      mb.translate(0, roofH - 0.004, 0);
      finial(mb, 0, 0.022);
      if (flag) pennant(mb, 0.024);
    });
  }
}

function humanTower(mb, r, v, sub) {
  const S = humanStyle(r, sub === "gothic");
  if (sub === "arcane" && v === 0) v = 2;
  if (v === 0) {
    // A round watchtower under a steep conical slate roof.
    roundTower(mb, r, S, { rad: 0.046, h: 0.3, roofH: 0.15 * S.pitch, roofC: J(S.slate, r, 0.05), seg: 10 });
  } else if (v === 1) {
    // A square keep-tower: battlements around a small pyramid roof, a banner on its face.
    const w = 0.088, h = 0.3;
    mb.color = mulc(S.lightC, 0.85);
    blk(mb, w + 0.012, w + 0.012, -0.05, 0.016, 0.96);
    mb.color = S.lightC;
    shell(mb, w, w, 0.016, h, 0.93);
    mb.color = mulc(S.lightC, 0.92);
    shell(mb, w * 0.95 + 0.002, w * 0.95 + 0.002, h * 0.5, h * 0.56);
    const tw = w * 0.93;
    mb.color = mulc(S.lightC, 0.96);
    blk(mb, tw, tw, h, h + 0.012, 1.18);
    mb.color = S.lightC;
    merlonsRect(mb, tw * 1.18, tw * 1.18, h + 0.012, { mw: 0.016, mh: 0.015, md: 0.008 });
    for (let s = 0; s < 4; s++)
      face(mb, s, w * 0.95, w * 0.95, () => {
        win(mb, S, 0, h * 0.7, 0.01, 0.02, r() < 0.5, 0.005);
        if (s === 0) {
          wallBanner(mb, 0, h * 0.58, 0.03, 0.07);
          door(mb, S, 0, 0.02, 0.034, 0.01, 0.016);
        } else win(mb, S, 0, h * 0.35, 0.008, 0.018, r() < 0.3, 0.004);
      });
    mb.part(() => {
      mb.translate(0, h + 0.012, 0);
      mb.color = J(S.slate, r, 0.05);
      hipRoof(mb, tw * 0.9, tw * 0.9, 0.1 * S.pitch, { over: 0.002 });
      mb.translate(0, 0.1 * S.pitch, 0);
      finial(mb, 0, 0.02);
      pennant(mb, 0.02);
    });
  } else {
    // A wizard's tower: slender, with a swelling upper chamber ringed by lit windows and a tall
    // concave spire of blue slate.
    const arc = sub === "arcane";
    const wallC = J(arc ? "#b8b0c4" : "#a8a092", r, 0.04);
    mb.color = mulc(wallC, 0.85);
    drum(mb, 0.05, 0.046, -0.05, 0.02, 9);
    mb.color = wallC;
    drum(mb, 0.04, 0.032, 0.02, 0.3, 9);
    mb.lathe([[0.032, 0.3], [0.046, 0.33], [0.048, 0.38], [0.042, 0.395]], 9, { caps: false });
    for (let k = 0; k < 5; k++) {
      const a = (k / 5) * TAU + 0.3;
      at(mb, Math.sin(a) * 0.0485, 0, Math.cos(a) * 0.0485, a, () => {
        mb.mat = MAT.EMISSIVE;
        mb.color = C(arc ? "#c4a8ff" : S.lit);
        rect(mb, -0.006, 0.342, 0.006, 0.37);
        mb.tri([-0.006, 0.37, 0], [0.006, 0.37, 0], [0, 0.378, 0]);
      });
    }
    for (let k = 0; k < 3; k++) {
      const a = r.range(-0.8, 0.8), y = 0.09 + k * 0.07;
      const rr = 0.04 - 0.008 * (y / 0.3) + 0.001;
      at(mb, Math.sin(a) * rr, 0, Math.cos(a) * rr, a, () => win(mb, S, 0, y, 0.007, 0.016, r() < 0.5, 0.004));
    }
    at(mb, 0, 0, 0.041, 0, () => door(mb, S, 0, 0.016, 0.028, 0.008, 0.02));
    mb.part(() => {
      mb.translate(0, 0.395, 0);
      mb.color = J(arc ? "#3c3a66" : "#3a4458", r, 0.05);
      coneRoof(mb, 0.058, 0.17, { seg: 9, curve: 0.75, drop: 0.012, kick: 0.25 });
      mb.translate(0, 0.17, 0);
      finial(mb, 0, 0.02);
      if (arc) {
        mb.mat = MAT.EMISSIVE;
        mb.color = C("#c8b0ff");
        at(mb, 0, 0.026, 0, 0, () => gem(mb, 0.006, 0.01));
      } else pennant(mb, 0.02);
    });
  }
  ground(mb, 0);
}

function humanHall(mb, r, v, sub) {
  const S = humanStyle(r, sub === "gothic");
  if (v === 0) {
    // A manor: a two-storey range under a hipped roof between two gable-fronted wings.
    const w = 0.13, d = 0.085;
    const b = humanBody(mb, r, S, w, d, { h1: 0.05, h2: 0.045, lower: "stone", doorX: 0 });
    humanRoof(mb, r, S, b, "hip", 0.075, { ridge: 0.08 });
    for (const s of [-1, 1]) {
      mb.part(() => {
        mb.translate(s * 0.088, 0, 0.012);
        const bw = humanBody(mb, r, S, 0.06, 0.12, { h1: 0.05, h2: 0.05, lower: "stone", gableZ: true, doorSide: 2 });
        humanRoof(mb, r, S, bw, "z", 0.08);
      });
      chimney(mb, S.chimney, s * 0.04, -0.02, 0.09, 0.19 * Math.min(1.3, S.pitch), 0.016);
    }
  } else if (v === 1) {
    // A guild hall: an arcaded stone ground floor, a framed upper hall, a steep roof and a little
    // clock turret with a spire on the ridge.
    const w = r.range(0.17, 0.185), d = 0.1;
    mb.color = mulc(S.stoneC, 0.85);
    blk(mb, w + 0.008, d + 0.008, -0.05, 0.008);
    mb.color = C("#2e2824");
    shell(mb, w - 0.012, d - 0.012, 0.008, 0.05);
    mb.color = S.lightC;
    const cols = 6;
    for (let i = 0; i <= cols; i++) {
      for (const z of [-1, 1]) at(mb, -w / 2 + 0.006 + ((w - 0.012) * i) / cols, 0, (z * (d - 0.012)) / 2, 0, () => blk(mb, 0.009, 0.009, 0.008, 0.05));
    }
    for (const x of [-1, 1]) at(mb, (x * (w - 0.012)) / 2, 0, 0, 0, () => blk(mb, 0.009, 0.03, 0.008, 0.05));
    mb.part(() => {
      mb.translate(0, 0.05, 0);
      mb.color = C(S.timber);
      shell(mb, w + 0.004, d + 0.004, -0.004, 0.004);
      mb.color = S.wallC;
      shell(mb, w, d, 0, 0.05);
      for (let s = 0; s < 4; s++)
        face(mb, s, w, d, (span) => {
          const k = s % 2 ? 2 : 5;
          framing(mb, S, span, 0, 0.05, k);
          windowRow(mb, r, S, span, 0.016, 0.018, { n: k });
        });
      humanRoof(mb, r, S, { w, d, y: 0.05 }, "x", 0.1);
      mb.part(() => {
        mb.translate(0, 0.05 + 0.1 * S.pitch - 0.035, 0);
        mb.color = S.wallC;
        blk(mb, 0.03, 0.03, 0, 0.05);
        face(mb, 0, 0.03, 0.03, () => {
          mb.color = C("#e8dcc0");
          rect(mb, -0.009, 0.026, 0.009, 0.044);
        });
        mb.translate(0, 0.05, 0);
        mb.color = J(S.slate, r, 0.05);
        hipRoof(mb, 0.03, 0.03, 0.07 * S.pitch, { over: 0.006 });
        mb.translate(0, 0.07 * S.pitch, 0);
        finial(mb, 0, 0.016);
        pennant(mb, 0.016, 0.035, 0.012);
      });
    });
  } else {
    // A chapel: a stone nave along z under a steep slate roof, a bell tower and spire at the front
    // (toward the camera), a lit rose window and lancets.
    const w = 0.085, d = 0.17;
    S.roofC = J(S.slate, r, 0.05);
    mb.part(() => {
      mb.translate(0, 0, -0.02);
      mb.color = mulc(S.lightC, 0.85);
      blk(mb, w + 0.01, d + 0.01, -0.05, 0.01);
      mb.color = S.lightC;
      shell(mb, w, d, 0.01, 0.085);
      for (const s of [1, 3])
        face(mb, s, w, d, (span) => {
          for (let i = 0; i < 4; i++) {
            const x = -span / 2 + (span * (i + 0.5)) / 4;
            win(mb, S, x, 0.03, 0.009, 0.03, r() < 0.55, 0.008);
            mb.color = mulc(S.lightC, 0.8);
            if (i) rect(mb, x - span / 8 - 0.004, 0.01, x - span / 8 + 0.004, 0.08, 0.002);
          }
        });
      humanRoof(mb, r, S, { w, d, y: 0.085 }, "z", 0.08, { ends: S.lightC });
      // The apse.
      mb.color = S.lightC;
      at(mb, 0, 0, -d / 2, 0, () => {
        mb.lathe([[0.034, -0.04], [0.034, 0.07]], 8, { caps: false, phase: 0 });
        mb.translate(0, 0.07, 0);
        mb.color = S.roofC;
        coneRoof(mb, 0.04, 0.05, { seg: 8, curve: 0.1, drop: 0.004 });
      });
    });
    // The bell tower and its spire.
    mb.part(() => {
      mb.translate(0, 0, 0.07);
      const tw = 0.05, th = 0.2;
      mb.color = mulc(S.lightC, 0.85);
      blk(mb, tw + 0.008, tw + 0.008, -0.05, 0.012);
      mb.color = S.lightC;
      shell(mb, tw, tw, 0.012, th, 0.94);
      face(mb, 0, tw, tw, () => {
        door(mb, S, 0, 0.02, 0.034, 0.012, 0.012);
        win(mb, S, 0, 0.075, 0.016, 0.0, true, 0);
        mb.part(() => {
          mb.mat = MAT.EMISSIVE;
          mb.color = C(S.lit);
          at(mb, 0, 0.088, 0, 0, () => {
            const R = 0.0095;
            for (let k = 0; k < 8; k++) {
              const a0 = (k / 8) * TAU, a1 = ((k + 1) / 8) * TAU;
              mb.tri([0, 0, 0], [Math.cos(a0) * R, Math.sin(a0) * R, 0], [Math.cos(a1) * R, Math.sin(a1) * R, 0]);
            }
          });
        });
      });
      for (let s = 0; s < 4; s++) face(mb, s, tw * 0.94, tw * 0.94, () => win(mb, S, 0, th - 0.05, 0.016, 0.026, false, 0.008));
      mb.translate(0, th, 0);
      mb.color = mulc(S.lightC, 0.95);
      blk(mb, tw * 0.94 + 0.006, tw * 0.94 + 0.006, -0.006, 0.006);
      mb.color = J(S.spire, r, 0.04);
      mb.lathe([[0.034, -0.002], [0.0, 0.13 * S.pitch]], 8, { caps: false, phase: TAU / 16 });
      mb.translate(0, 0.13 * S.pitch, 0);
      finial(mb, 0, 0.02);
      mb.part(() => {
        mb.mat = MAT.METAL;
        mb.color = C("#c9a043");
        at(mb, 0, 0.012, 0, 0, () => rect(mb, -0.007, -0.0015, 0.007, 0.0015));
      });
    });
  }
  ground(mb, 0);
}

function humanKeep(mb, r, v, sub) {
  const S = humanStyle(r, sub === "gothic");
  const slate = J(S.slate, r, 0.04);
  if (v === 0) {
    // A castle keep: a tall square donjon with a hipped roof inside its battlements, four corner
    // turrets with conical roofs, a forebuilding with the door, banners down its face.
    const w = 0.15, d = 0.13, h = 0.23;
    mb.color = mulc(S.lightC, 0.82);
    blk(mb, w + 0.016, d + 0.016, -0.05, 0.02, 0.97);
    mb.color = S.lightC;
    shell(mb, w, d, 0.02, h);
    mb.color = mulc(S.lightC, 0.92);
    shell(mb, w + 0.002, d + 0.002, h * 0.36, h * 0.4);
    mb.color = mulc(S.lightC, 0.97);
    blk(mb, w, d, h, h + 0.014, 1.06);
    mb.color = S.lightC;
    merlonsRect(mb, w * 1.06, d * 1.06, h + 0.014, { mw: 0.016, mh: 0.016, md: 0.009 });
    for (let s = 0; s < 4; s++)
      face(mb, s, w, d, (span) => {
        for (let i = 0; i < 3; i++) {
          const x = -span / 2 + (span * (i + 0.5)) / 3;
          win(mb, S, x, h * 0.62, 0.011, 0.024, r() < 0.45, 0.006);
          if (s !== 0 || i !== 1) win(mb, S, x, h * 0.25, 0.008, 0.018, r() < 0.2, 0.004);
        }
        if (s === 0) {
          wallBanner(mb, -0.042, h * 0.9, 0.03, 0.085);
          wallBanner(mb, 0.042, h * 0.9, 0.03, 0.085);
        }
      });
    // The roof inside the battlements.
    mb.part(() => {
      mb.translate(0, h + 0.014, 0);
      mb.color = slate;
      hipRoof(mb, w * 0.86, d * 0.82, 0.075 * S.pitch, { over: 0.0, ridge: 0.05 });
    });
    // Corner turrets.
    for (const [sx, sz] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
      mb.part(() => {
        mb.translate((sx * w) / 2, 0, (sz * d) / 2);
        mb.color = S.lightC;
        drum(mb, 0.027, 0.027, 0.06, h + 0.04, 8);
        mb.lathe([[0.02, 0.04], [0.027, 0.06]], 8, { caps: false });
        mb.translate(0, h + 0.04, 0);
        mb.color = slate;
        coneRoof(mb, 0.034, 0.09 * S.pitch, { seg: 8, curve: 0.3, drop: 0.006 });
        if (sz > 0) {
          mb.translate(0, 0.09 * S.pitch, 0);
          finial(mb, 0, 0.014);
          pennant(mb, 0.014, 0.035, 0.012);
        }
      });
    }
    // Forebuilding and steps.
    mb.part(() => {
      mb.translate(0, 0, d / 2 + 0.02);
      mb.color = mulc(S.lightC, 0.9);
      blk(mb, 0.05, 0.04, -0.05, 0.07);
      merlonsRect(mb, 0.05, 0.04, 0.07, { mw: 0.012, mh: 0.012, md: 0.007 });
      face(mb, 0, 0.05, 0.04, () => door(mb, S, 0, 0.022, 0.036, 0.011));
    });
  } else {
    // A palace-cathedral (capitals): a lofty nave under a steep roof, twin spired towers on its
    // front, a crossing tower, buttresses, a great lit rose window.
    const w = 0.1, d = 0.22, h = 0.13;
    const L = S.lightC;
    mb.part(() => {
      mb.translate(0, 0, -0.03);
      mb.color = mulc(L, 0.84);
      blk(mb, w + 0.012, d + 0.012, -0.05, 0.014);
      mb.color = L;
      shell(mb, w, d, 0.014, h);
      // Aisles: lower lean-to ranges along both sides.
      for (const s of [-1, 1]) {
        mb.part(() => {
          mb.translate((s * (w + 0.04)) / 2, 0, 0);
          mb.color = mulc(L, 0.95);
          blk(mb, 0.04, d * 0.92, -0.04, 0.07);
          face(mb, s > 0 ? 1 : 3, 0.04, d * 0.92, (span) => {
            for (let i = 0; i < 5; i++) win(mb, S, -span / 2 + (span * (i + 0.5)) / 5, 0.022, 0.009, 0.028, r() < 0.5, 0.007);
          });
          mb.translate(0, 0.07, 0);
          mb.color = slate;
          mb.quad([(-s * 0.02), 0.035, (-d * 0.46)], [(-s * 0.02), 0.035, d * 0.46], [s * 0.026, -0.004, d * 0.46], [s * 0.026, -0.004, -d * 0.46]);
        });
      }
      for (const s of [1, 3])
        face(mb, s, w, d, (span) => {
          for (let i = 0; i < 5; i++) win(mb, S, -span / 2 + (span * (i + 0.5)) / 5, h * 0.66, 0.01, 0.028, r() < 0.6, 0.008);
        });
      humanRoof(mb, r, S, { w, d, y: h }, "z", 0.1, { color: slate, ends: L });
      // Crossing tower with a spire.
      mb.part(() => {
        mb.translate(0, h + 0.05, -0.03);
        mb.color = L;
        blk(mb, 0.05, 0.05, -0.03, 0.06);
        for (let s = 0; s < 4; s++) face(mb, s, 0.05, 0.05, () => win(mb, S, 0, 0.012, 0.012, 0.026, r() < 0.5, 0.008));
        mb.translate(0, 0.06, 0);
        mb.color = slate;
        mb.lathe([[0.036, -0.002], [0, 0.14 * S.pitch]], 8, { caps: false, phase: TAU / 16 });
        mb.translate(0, 0.14 * S.pitch, 0);
        finial(mb, 0, 0.02);
      });
      // The apse.
      at(mb, 0, 0, -d / 2, 0, () => {
        mb.color = L;
        mb.lathe([[0.045, -0.04], [0.045, h * 0.85]], 9, { caps: false });
        mb.translate(0, h * 0.85, 0);
        mb.color = slate;
        coneRoof(mb, 0.05, 0.06, { seg: 9, curve: 0.1, drop: 0.004 });
      });
    });
    // Twin west towers framing the great door and rose window.
    const fz = d / 2 - 0.03 + 0.02;
    for (const s of [-1, 1]) {
      mb.part(() => {
        mb.translate(s * 0.058, 0, fz);
        const tw = 0.052, th = 0.25;
        mb.color = mulc(L, 0.84);
        blk(mb, tw + 0.01, tw + 0.01, -0.05, 0.016);
        mb.color = L;
        shell(mb, tw, tw, 0.016, th, 0.94);
        for (let q = 0; q < 4; q++)
          face(mb, q, tw * 0.95, tw * 0.95, () => {
            win(mb, S, 0, th - 0.06, 0.012, 0.03, false, 0.008);
            if (q === 0) win(mb, S, 0, th * 0.42, 0.009, 0.024, r() < 0.5, 0.006);
          });
        face(mb, 0, tw, tw, () => wallBanner(mb, 0, th * 0.66, 0.028, 0.06));
        mb.translate(0, th, 0);
        mb.color = mulc(L, 0.95);
        blk(mb, tw + 0.004, tw + 0.004, -0.008, 0.006);
        mb.color = slate;
        mb.lathe([[0.036, 0], [0.0, 0.15 * S.pitch]], 8, { caps: false, phase: TAU / 16 });
        mb.translate(0, 0.15 * S.pitch, 0);
        finial(mb, 0, 0.018);
        pennant(mb, 0.018, 0.04, 0.014);
      });
    }
    // The west front between the towers.
    mb.part(() => {
      mb.translate(0, 0, fz - 0.005);
      mb.color = L;
      blk(mb, 0.07, 0.03, -0.05, h + 0.03);
      face(mb, 0, 0.07, 0.03, () => {
        door(mb, S, 0, 0.026, 0.05, 0.014, 0.012);
        mb.part(() => {
          mb.mat = MAT.EMISSIVE;
          mb.color = C(S.lit);
          const R = 0.016, cy = h * 0.72;
          for (let k = 0; k < 10; k++) {
            const a0 = (k / 10) * TAU, a1 = ((k + 1) / 10) * TAU;
            mb.tri([0, cy, 0], [Math.cos(a0) * R, cy + Math.sin(a0) * R, 0], [Math.cos(a1) * R, cy + Math.sin(a1) * R, 0]);
          }
        });
      });
      mb.translate(0, h + 0.03, 0);
      mb.color = L;
      mb.tri([-0.035, 0, 0.015], [0.035, 0, 0.015], [0, 0.05, 0.015]);
    });
  }
  ground(mb, 0);
}

/** A curtain wall segment along x (length 0.2, outer face +z): battered plinth, walls, a walkway
 *  and merlons on the outer edge. */
function humanWall(mb, r, sub) {
  const S = humanStyle(r, sub === "gothic");
  const L = 0.205, t = 0.034, h = 0.07;
  mb.color = mulc(S.lightC, 0.8);
  blk(mb, L, t + 0.014, -0.05, 0.016, 0.98);
  mb.color = S.lightC;
  shell(mb, L, t, 0.016, h);
  mb.color = mulc(S.lightC, 0.9);
  shell(mb, L + 0.0005, t + 0.0015, 0.04, 0.045);
  mb.color = mulc(S.lightC, 0.78);
  mb.quad([-L / 2, h, -t / 2], [-L / 2, h, t / 2], [L / 2, h, t / 2], [L / 2, h, -t / 2]);
  mb.color = S.lightC;
  merlonsLine(mb, -L / 2 + 0.01, L / 2 - 0.01, h, t / 2 - 0.004, { mw: 0.02, mh: 0.016, md: 0.008, n: 5 });
  at(mb, 0, 0, -t / 2 + 0.003, 0, () => blk(mb, L, 0.006, h, h + 0.007));
  ground(mb, 0, 0.04, 0.6);
}

/** A gatehouse (width 0.2 along x, outer face +z): two towers flanking an arched passage, banners. */
function humanGate(mb, r, sub) {
  const S = humanStyle(r, sub === "gothic");
  const L = S.lightC, slate = J(S.slate, r, 0.04);
  const t = 0.05;
  // The passage block.
  mb.color = mulc(L, 0.82);
  blk(mb, 0.1, t + 0.01, -0.05, 0.014);
  mb.color = L;
  shell(mb, 0.1, t, 0.014, 0.11);
  mb.color = mulc(L, 0.8);
  mb.quad([-0.05, 0.11, -t / 2], [-0.05, 0.11, t / 2], [0.05, 0.11, t / 2], [0.05, 0.11, -t / 2]);
  mb.color = L;
  merlonsLine(mb, -0.045, 0.045, 0.11, t / 2 - 0.004, { mw: 0.016, mh: 0.015, md: 0.008, n: 3 });
  for (const side of [0, 2])
    face(mb, side, 0.1, t, () => {
      mb.color = mulc(L, 1.08);
      rect(mb, -0.026, 0.0, 0.026, 0.066, -0.0004);
      door(mb, S, 0, 0.04, 0.045, 0.02, 0.0, "#1c1814");
      mb.color = C("#3a3634");
      for (let i = -1; i <= 1; i++) rect(mb, i * 0.011 - 0.0015, 0.005, i * 0.011 + 0.0015, 0.06, 0.0006);
    });
  // Flanking towers.
  for (const s of [-1, 1]) {
    mb.part(() => {
      mb.translate(s * 0.072, 0, 0.004);
      const tw = 0.06, th = 0.16;
      mb.color = mulc(L, 0.82);
      blk(mb, tw + 0.012, tw + 0.012, -0.05, 0.016, 0.96);
      mb.color = L;
      shell(mb, tw, tw, 0.016, th, 0.94);
      face(mb, 0, tw * 0.97, tw * 0.97, () => {
        wallBanner(mb, 0, th * 0.86, 0.026, 0.06);
        win(mb, S, 0, th * 0.25, 0.007, 0.016, r() < 0.4, 0.004);
      });
      const top = tw * 0.94;
      mb.color = mulc(L, 0.96);
      blk(mb, top, top, th, th + 0.01, 1.15);
      mb.color = L;
      merlonsRect(mb, top * 1.15, top * 1.15, th + 0.01, { mw: 0.013, mh: 0.013, md: 0.007 });
      mb.translate(0, th + 0.01, 0);
      mb.color = slate;
      hipRoof(mb, top * 0.9, top * 0.9, 0.07 * S.pitch, { over: 0.002 });
      mb.translate(0, 0.07 * S.pitch, 0);
      finial(mb, 0, 0.016);
      pennant(mb, 0.016, 0.036, 0.013);
    });
  }
  ground(mb, 0, 0.04, 0.6);
}

/** A round wall tower (v0 battlements, v1 a conical roof). */
function humanWallTower(mb, r, v, sub) {
  const S = humanStyle(r, sub === "gothic");
  roundTower(mb, r, S, { rad: 0.038, h: 0.11, roofH: v ? 0.08 * S.pitch : 0, roofC: J(S.slate, r, 0.04), crenel: !v, seg: 9, flag: v === 1, slits: 1, taper: 0.95 });
  ground(mb, 0, 0.04, 0.6);
}

// --- organic and faceted helpers ----------------------------------------------------------------------

/** A faceted, tapering prism of n sides (flat-shaded): radius rad at y0 to rad * top at y1, its top
 *  shifted by `lean` [x, z] and turned by `twist`; top 0 makes a spike. */
function shard(mb, n, rad, y0, y1, { top = 0, lean = [0, 0], phase = 0, twist = 0, cap = true, sx = 1 } = {}) {
  const B = [], T = [];
  for (let k = 0; k < n; k++) {
    const a = phase + (k / n) * TAU;
    B.push([Math.sin(a) * rad * sx, y0, Math.cos(a) * rad]);
    const b = a + twist;
    T.push([Math.sin(b) * rad * top * sx + lean[0], y1, Math.cos(b) * rad * top + lean[1]]);
  }
  if (top < 1e-4) {
    const apex = [lean[0], y1, lean[1]];
    for (let k = 0; k < n; k++) mb.tri(B[k], B[(k + 1) % n], apex);
  } else {
    for (let k = 0; k < n; k++) {
      const j = (k + 1) % n;
      mb.quad(B[k], B[j], T[j], T[k]);
    }
    if (cap) for (let k = 1; k + 1 < n; k++) mb.tri(T[0], T[k], T[k + 1]);
  }
}

/** A tube between two points (logs, poles, branches, bones). */
function pole(mb, a, b, r0, r1 = r0, seg = 5) {
  mb.tube([a, b], [r0, r1], seg);
}

/** A clump of foliage around c: n lumpy, flat-bellied masses with soft normals bent out from
 *  `centre` (default c), painted from the deep colour below to the light one on top of the range
 *  y0..y1 (default the clump's), swaying toward the top. */
function leaves(mb, r, c, rad, n, ramp, { detail = 0, spread = 1, squash = 0.8, sway = 0.5, flat = 0.45, centre = null, range = null, blend = 0.75 } = {}) {
  const m0 = mb.mark();
  const ctr = mb.xp(centre ?? c);
  mb.part(() => {
    mb.mat = MAT.FOLIAGE;
    mb.color = C(ramp[Math.min(ramp.length - 1, 1)]);
    for (let i = 0; i < n; i++) {
      const a = (i / n) * TAU + r.range(-0.5, 0.5);
      const d = i === 0 ? 0 : rad * r.range(0.6, 0.95) * spread;
      const p = [c[0] + Math.sin(a) * d, c[1] + r.range(-0.2, 0.3) * rad - (i ? d * 0.3 : 0), c[2] + Math.cos(a) * d];
      const s = rad * (i === 0 ? 1 : r.range(0.62, 0.85));
      const seed = r.range(0, 50);
      at(mb, p[0], p[1], p[2], 0, () => mb.sphere(s, detail, { squash, deform: (v) => 1 + 0.28 * noise3(v[0] * 2 + seed, v[1] * 2, v[2] * 2) - (v[1] < 0 ? flat * -v[1] : 0) }));
    }
  });
  mb.sphereNormals(m0, ctr, blend);
  const [lo, hi] = range ?? [mb.xp(c)[1] - rad, mb.xp(c)[1] + rad * 1.1];
  const cols = ramp.map(C);
  const v = mb.v;
  for (let i = m0 * FLOATS; i < v.length; i += FLOATS) {
    const t = Math.max(0, Math.min(0.999, (v[i + 1] - lo) / (hi - lo))) * (cols.length - 1);
    const k = Math.floor(t), f = t - k;
    const a = cols[k], b = cols[Math.min(cols.length - 1, k + 1)];
    v[i + 6] = a[0] + (b[0] - a[0]) * f;
    v[i + 7] = a[1] + (b[1] - a[1]) * f;
    v[i + 8] = a[2] + (b[2] - a[2]) * f;
  }
  mb.heightSway(m0, lo, hi, sway);
}

/** A small hanging lantern (emissive). */
function lantern(mb, x, y, z, c, s = 0.0055) {
  mb.part(() => {
    mb.translate(x, y, z);
    mb.mat = MAT.EMISSIVE;
    mb.color = C(c);
    gem(mb, s, s * 1.5);
  });
}

/** A roof like a closing flower bud: flared, petal-scalloped eaves, an ogee curve to a long tip
 *  (`open` leaves a hole of that radius for a trunk). */
function petalRoof(mb, R, h, { seg = 8, petals = 6, lip = 0.1, open = 0, curl = true, rings = 3 } = {}) {
  const tip = Math.max(open, R * 0.02), top = open ? h * 0.8 : h;
  const prof = rings > 3
    ? [[R, 0.012], [R * 0.9, 0], [R * 0.7, h * 0.16], [R * 0.42, h * 0.38], [R * 0.17, h * 0.66], [tip, top]]
    : [[R, 0.012], [R * 0.9, 0], [R * 0.56, h * 0.28], [R * 0.2, h * 0.64], [tip, top]];
  mb.lathe(prof, seg, { caps: false, wobble: petals ? (a, y, i) => (i === 0 ? lip * Math.cos(a * petals) : i === 1 ? lip * 0.6 * Math.cos(a * petals) : i === 2 ? lip * 0.25 * Math.cos(a * petals) : 0) : null });
  if (curl && !open) mb.tube([[0, h - 0.004, 0], [0.002, h + 0.012, 0], [0.012, h + 0.022, 0], [0.02, h + 0.018, 0.002]], [R * 0.06, R * 0.045, R * 0.03, 0.0008], 3);
}

/** Roots spreading from a trunk base of radius rad into the ground. */
function roots(mb, r, n, rad, len, phase = 0) {
  for (let k = 0; k < n; k++) {
    const a = phase + (k / n) * TAU + r.range(-0.3, 0.3);
    const ca = Math.sin(a), sa = Math.cos(a);
    const l = len * r.range(0.8, 1.2);
    mb.tube([[ca * rad * 0.5, rad * 1.4, sa * rad * 0.5], [ca * (rad + l * 0.4), rad * 0.25, sa * (rad + l * 0.4)], [ca * (rad + l), -0.03, sa * (rad + l)]], [rad * 0.42, rad * 0.3, rad * 0.12], 4);
  }
}

/** Thorns (the Svartalfar's): small dark spikes from the asset-space points given. */
function thorns(mb, r, pts, len = 0.016) {
  mb.part(() => {
    mb.color = C("#1e1a1e");
    for (const [x, y, z, ax, az] of pts) {
      const a = Math.atan2(ax, az);
      at(mb, x, y, z, a, () => {
        mb.rotateX(r.range(0.7, 1.3));
        shard(mb, 3, len * 0.18, 0, len * r.range(0.8, 1.3), { phase: r.range(0, 2) });
      });
    }
  });
}

// --- elven: houses grown into living trees, bud roofs, lantern light ------------------------------

const ELF = {
  light: {
    bark: "#7c7564", barkDark: "#5e5848", wall: ["#cfc0a0", "#c6b694", "#d4c7a6"],
    roof: [["#3f5f52", 3], ["#4c6648", 2.2], ["#7e6a40", 1.4], ["#5a6e5a", 1]],
    gold: "#76643c",
    leaf: ["#173d37", "#2a5a48", "#4a7a4a", "#8aab6a"],
    mallorn: ["#2e3a22", "#56562a", "#8e803a", "#bca452"],
    lit: "#ffd68a", glass: "#2a3028", door: "#5a4630", moss: "#4a5a38",
  },
  dark: {
    bark: "#3e3638", barkDark: "#2a2428", wall: ["#5e5460", "#544a56", "#665a68"],
    roof: [["#2c2836", 3], ["#382c42", 2], ["#46283a", 1]],
    gold: "#4a3a5a",
    leaf: ["#121418", "#1e2228", "#2e3440", "#4a5262"],
    mallorn: ["#120e16", "#221a28", "#362a40", "#54465e"],
    lit: "#a8b4ff", glass: "#141218", door: "#201a1e", moss: "#2a3030",
  },
};

function elfStyle(r, dark) {
  const B = dark ? ELF.dark : ELF.light;
  return { ...B, dark, barkC: J(B.bark, r, 0.04), wallC: J(r.pick(B.wall), r, 0.04), roofC: J(wpick(r, B.roof), r, 0.05), gold: J(B.gold, r, 0.05) };
}

/** An elven pod house: a low root plinth (or a bowl when raised), rounded walls with tall oval lit
 *  windows and a round door, a bud roof. */
function elfPod(mb, r, S, o) {
  const { rad = 0.05, h = 0.055, roofH = 0.08, y = 0, petals = 6, windows = 3, roofC = S.roofC, open = 0, door: hasDoor = true, seg = 8, curl = true, rings = 3 } = o;
  mb.part(() => {
    mb.translate(0, y, 0);
    mb.color = mulc(S.barkC, 0.9);
    if (y > 0.002) mb.lathe([[0.004, -0.03], [rad * 0.7, -0.016], [rad * 1.04, 0.0], [rad * 1.0, 0.008]], seg, { caps: false });
    else mb.lathe([[rad * 1.1, -0.045], [rad * 1.0, 0.01]], seg, { caps: false });
    mb.color = S.wallC;
    mb.lathe([[rad * 0.99, 0.008], [rad * 1.05, h * 0.55], [rad * 0.96, h + 0.002]], seg, { caps: false });
    for (let k = 0; k < windows; k++) {
      const a = (k + 0.5) * (TAU / windows) + r.range(-0.3, 0.3);
      at(mb, Math.sin(a) * rad * 1.045, 0, Math.cos(a) * rad * 1.045, a, () => win(mb, S, 0, h * 0.28, 0.011, h * 0.34, r() < 0.55, 0.008));
    }
    if (hasDoor) at(mb, 0, 0, rad * 1.04, 0, () => door(mb, S, 0, 0.017, h * 0.5, 0.009, 0.008));
    mb.translate(0, h, 0);
    mb.color = roofC;
    petalRoof(mb, rad * 1.24, roofH, { petals, open, curl, seg, rings, lip: 0.12 });
    if (S.dark && rad > 0.039) {
      const pts = [];
      for (let k = 0; k < 3; k++) {
        const a = (k / 3) * TAU + 0.3;
        pts.push([Math.sin(a) * rad * 0.9, roofH * 0.25, Math.cos(a) * rad * 0.9, Math.sin(a), Math.cos(a)]);
      }
      thorns(mb, r, pts, 0.02);
    }
  });
}

function elvenHouse(mb, r, v, sub) {
  const S = elfStyle(r, sub === "dark");
  switch (v) {
    case 0: {
      // A pod held in the hand of a living tree: great roots rise from the ground, climb its walls
      // and curl over its eaves toward the tip of its roof.
      const rad = 0.046, h = 0.066, roofH = 0.1;
      elfPod(mb, r, S, { rad, h, roofH, seg: 7, windows: 2 });
      mb.color = S.barkC;
      for (const a0 of [1.0, 3.1, 5.2]) {
        const a = a0 + r.range(-0.25, 0.25);
        const tw = r.range(0.25, 0.5) * r.sign();
        const p = (rr, y, da = 0) => [Math.sin(a + da) * rr, y, Math.cos(a + da) * rr];
        mb.tube([p(rad * 1.9, -0.03), p(rad * 1.2, 0.02, tw * 0.3), p(rad * 1.12, h * 0.85, tw * 0.6), p(rad * 0.85, h + roofH * 0.3, tw), p(rad * 0.25, h + roofH * 0.62, tw * 1.3)], [0.011, 0.008, 0.0065, 0.004, 0.0015], 4);
      }
      if (!S.dark) lantern(mb, 0.03, 0.05, 0.052, S.lit);
      break;
    }
    case 1: {
      // A pod raised on three root legs, a little stair to its door.
      const y = r.range(0.045, 0.06);
      mb.color = S.barkC;
      for (let k = 0; k < 3; k++) {
        const a = (k / 3) * TAU + 0.5;
        mb.tube([[Math.sin(a) * 0.02, y, Math.cos(a) * 0.02], [Math.sin(a) * 0.04, y * 0.45, Math.cos(a) * 0.04], [Math.sin(a) * 0.058, -0.03, Math.cos(a) * 0.058]], [0.009, 0.008, 0.007], 4);
      }
      elfPod(mb, r, S, { rad: 0.04, h: 0.062, roofH: 0.11, y, seg: 7 });
      mb.color = mulc(S.barkC, 0.85);
      for (let k = 0; k < 3; k++) at(mb, 0, 0, 0.06 + k * 0.009, 0, () => blk(mb, 0.016, 0.01, -0.02, y - k * (y / 3)));
      break;
    }
    case 2: {
      // A long leaf-shaped house: oval walls under an elongated bud roof with a ridge vein.
      mb.part(() => {
        mb.scale(1.55, 1, 1);
        elfPod(mb, r, S, { rad: 0.042, h: 0.06, roofH: 0.085, petals: 0, windows: 4, door: false, curl: false, seg: 9 });
      });
      at(mb, 0, 0, 0.044, 0, () => door(mb, S, 0, 0.018, 0.03, 0.009, 0.008));
      mb.color = mulc(S.roofC, 0.8);
      mb.tube([[-0.075, 0.064, 0], [-0.03, 0.12, 0], [0.03, 0.12, 0], [0.075, 0.064, 0]], [0.002, 0.004, 0.004, 0.002], 4);
      if (!S.dark) lantern(mb, 0.075, 0.04, 0.045, S.lit);
      break;
    }
    case 3: {
      // Twin pods: one on the ground, one raised on a trunk behind it, a plank bridge between.
      mb.part(() => {
        mb.translate(-0.03, 0, 0.022);
        elfPod(mb, r, S, { rad: 0.034, h: 0.058, roofH: 0.085, seg: 6, windows: 1 });
      });
      mb.part(() => {
        mb.translate(0.035, 0, -0.03);
        mb.color = S.barkC;
        mb.tube([[0, -0.04, 0], [0.002, 0.05, 0], [0, 0.085, 0]], [0.016, 0.012, 0.02], 4);
        elfPod(mb, r, S, { rad: 0.03, h: 0.05, roofH: 0.09, y: 0.085, seg: 5, windows: 1, door: false });
      });
      break;
    }
    case 4: {
      // A low domed house nestled under great arching roots, a lantern at its door.
      elfPod(mb, r, S, { rad: 0.05, h: 0.04, roofH: 0.045, petals: 5, lip: 0.04, curl: false, seg: 8, rings: 3, roofC: J(S.dark ? S.moss : "#4f6a3c", r, 0.06) });
      mb.color = S.barkC;
      for (let k = 0; k < 2; k++) {
        const a = r.range(0, TAU);
        const ca = Math.sin(a), sa = Math.cos(a);
        mb.tube([[ca * 0.085, -0.03, sa * 0.085], [ca * 0.06, 0.06, sa * 0.06], [ca * 0.01, 0.1, sa * 0.01], [-ca * 0.05, 0.07, -sa * 0.05], [-ca * 0.075, -0.03, -sa * 0.075]], [0.014, 0.011, 0.009, 0.008, 0.007], 5);
      }
      lantern(mb, 0.024, 0.045, 0.06, S.lit, 0.006);
      break;
    }
    default: {
      // A pod under a tall slender bud spire, lanterns hung from its petals.
      elfPod(mb, r, S, { rad: 0.04, h: 0.072, roofH: 0.16, petals: 7, seg: 8, roofC: S.dark ? S.roofC : J(S.gold, r, 0.05) });
      for (const a of [0.8, -0.9]) lantern(mb, Math.sin(a) * 0.058, 0.05, Math.cos(a) * 0.058, S.lit);
      break;
    }
  }
  ground(mb, 0, 0.04, 0.6);
}

/** A great tree: a flared trunk with roots, boughs each ending in a broad, flat-bellied mass of
 *  leaves, a dome over the leader; one soft crown in light and shade. Returns the boughs' ends. */
function greatTree(mb, r, S, { h = 0.7, rad = 0.07, limbs = 4, crown = 0.13, detail = 1, masses = 1, dome = masses, seg = 9, nroots = -1, ramp = S.leaf } = {}) {
  mb.color = S.barkC;
  const fork = h * 0.4;
  mb.lathe([[rad * 1.6, -0.05], [rad * 1.25, 0.02], [rad, 0.07], [rad * 0.82, fork * 0.6], [rad * 0.72, fork], [rad * 0.4, h * 0.8]], seg, { caps: false, wobble: (a, y) => 0.1 * Math.sin(a * 5 + y * 30) * Math.max(0, 1 - y / 0.1) });
  roots(mb, r, nroots >= 0 ? nroots : h > 0.5 ? 4 : 3, rad * 1.1, rad * 1.4, r.range(0, TAU));
  const tips = [];
  const ph = r.range(0, TAU);
  for (let k = 0; k < limbs; k++) {
    const a = ph + (k / limbs) * TAU + r.range(-0.3, 0.3);
    const ca = Math.sin(a), sa = Math.cos(a);
    const out = r.range(0.13, 0.17) * (h / 0.7);
    const tip = [ca * out, fork + h * r.range(0.12, 0.3), sa * out];
    mb.color = S.barkC;
    mb.tube([[0, fork - 0.03, 0], [ca * out * 0.45, fork + (tip[1] - fork) * 0.55, sa * out * 0.45], tip], [rad * 0.5, rad * 0.34, rad * 0.18], 5);
    tips.push(tip);
  }
  const C0 = [0, fork + h * 0.28, 0];
  const range = [fork + h * 0.05, h * 1.08];
  for (const t of tips) leaves(mb, r, [t[0], t[1] + 0.02, t[2]], crown * r.range(1.1, 1.3), masses, ramp, { detail, spread: 0.8, squash: 0.42, flat: 0.55, sway: 0.3, centre: C0, range, blend: 0.5 });
  for (let k = 0; k < dome; k++) leaves(mb, r, [r.range(-0.03, 0.03), h * (0.8 + 0.12 * k), r.range(-0.03, 0.03)], crown * (1.05 - 0.25 * k), 1, ramp, { detail, spread: 0.6, squash: 0.5, flat: 0.5, sway: 0.35, centre: C0, range, blend: 0.55 });
  return tips;
}

function elvenHall(mb, r, v, sub) {
  const S = elfStyle(r, sub === "dark");
  if (v === 0) {
    // A moot hall: a long oval hall under an elongated bud roof, a small pod at one end.
    mb.part(() => {
      mb.scale(1.8, 1, 1.05);
      elfPod(mb, r, S, { rad: 0.052, h: 0.06, roofH: 0.1, petals: 0, windows: 6, door: false, curl: false, seg: 10 });
    });
    at(mb, 0, 0, 0.056, 0, () => door(mb, S, 0, 0.022, 0.036, 0.011, 0.008));
    mb.color = mulc(S.roofC, 0.8);
    mb.tube([[-0.12, 0.062, 0], [-0.05, 0.14, 0], [0.05, 0.14, 0], [0.12, 0.062, 0]], [0.002, 0.005, 0.005, 0.002], 4);
    const s = r.sign();
    at(mb, s * 0.11, 0, -0.02, 0, () => elfPod(mb, r, S, { rad: 0.034, h: 0.05, roofH: 0.09, seg: 7, windows: 2, door: false, roofC: J(S.gold, r, 0.05) }));
    lantern(mb, -s * 0.06, 0.05, 0.06, S.lit);
    lantern(mb, s * 0.03, 0.05, 0.062, S.lit);
  } else if (v === 1) {
    // A grove temple: slender living pillars around a lit heart, carrying a golden bud dome.
    mb.color = mulc(S.barkC, 0.9);
    mb.lathe([[0.1, -0.04], [0.098, 0.006], [0.085, 0.012], [0.083, 0.02]], 10, { caps: true });
    const n = 7;
    mb.color = S.barkC;
    for (let k = 0; k < n; k++) {
      const a = (k / n) * TAU;
      const ca = Math.sin(a), sa = Math.cos(a);
      mb.tube([[ca * 0.07, 0.015, sa * 0.07], [Math.sin(a + 0.25) * 0.068, 0.06, Math.cos(a + 0.25) * 0.068], [Math.sin(a + 0.4) * 0.074, 0.105, Math.cos(a + 0.4) * 0.074]], [0.006, 0.005, 0.0065], 4);
    }
    mb.lathe([[0.08, 0.1], [0.086, 0.108], [0.078, 0.114]], 10, { caps: false });
    mb.part(() => {
      mb.translate(0, 0.112, 0);
      mb.color = S.dark ? S.roofC : J(S.gold, r, 0.04);
      petalRoof(mb, 0.1, 0.13, { petals: 7, seg: 10, lip: 0.06 });
    });
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      mb.color = C(S.dark ? "#8a90ff" : "#fff0b0");
      at(mb, 0, 0.05, 0, 0, () => gem(mb, 0.012, 0.024));
    });
    mb.color = mulc(S.barkC, 0.8);
    at(mb, 0, 0.02, 0, 0, () => mb.cylinder(0.014, 0.01, 0.012, 6));
  } else {
    // A hall of lanterns: three pods gathered round a silver living pillar whose bare branches
    // arch out over them, a lantern hung from each.
    mb.color = S.barkC;
    mb.lathe([[0.04, -0.04], [0.03, 0.02], [0.02, 0.15], [0.012, 0.28], [0.004, 0.33]], 6, { caps: false });
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * TAU + 0.4;
      const ca = Math.sin(a), sa = Math.cos(a);
      const y0 = 0.17 + k * 0.025;
      mb.color = S.barkC;
      mb.tube([[ca * 0.01, y0, sa * 0.01], [ca * 0.05, y0 + 0.06, sa * 0.05], [ca * 0.09, y0 + 0.05, sa * 0.09], [ca * 0.11, y0 + 0.01, sa * 0.11]], [0.007, 0.005, 0.003, 0.0012], 4);
      lantern(mb, ca * 0.11, y0 - 0.006, sa * 0.11, S.lit, 0.0065);
    }
    for (let k = 0; k < 3; k++) {
      const a = (k / 3) * TAU + Math.PI / 3;
      at(mb, Math.sin(a) * 0.07, 0, Math.cos(a) * 0.07, a, () => elfPod(mb, r, S, { rad: 0.038, h: 0.055, roofH: 0.085, seg: 6, windows: 1, door: k === 1, roofC: k === 1 ? J(S.gold, r, 0.05) : S.roofC }));
    }
  }
  ground(mb, 0, 0.04, 0.6);
}

function elvenTower(mb, r, v, sub) {
  const S = elfStyle(r, sub === "dark");
  if (v === 0) {
    // A tree spire: a slender twisting trunk carrying a pod high above the canopy.
    const pts = [], rad = [];
    const n = 6;
    for (let k = 0; k <= n; k++) {
      const t = k / n;
      const a = t * 4 + r.range(0, 0.3);
      pts.push([Math.sin(a) * 0.012 * t, -0.04 + t * 0.38, Math.cos(a) * 0.012 * t]);
      rad.push(0.032 - t * 0.02);
    }
    mb.color = S.barkC;
    mb.tube(pts, rad, 6);
    roots(mb, r, 3, 0.03, 0.05, r.range(0, TAU));
    const top = pts[n];
    at(mb, top[0], 0, top[2], 0, () => elfPod(mb, r, S, { rad: 0.04, h: 0.045, roofH: 0.11, y: 0.335, seg: 8, windows: 3, door: false, roofC: J(S.gold, r, 0.05) }));
    mb.color = S.barkC;
    for (const [s, y] of [[1, 0.17], [-1, 0.24]]) {
      mb.tube([[0, y, 0], [s * 0.03, y + 0.03, 0.01], [s * 0.05, y + 0.02, 0.02], [s * 0.052, y + 0.005, 0.024]], [0.006, 0.004, 0.0025, 0.001], 4);
      lantern(mb, s * 0.05, y - 0.008, 0.024, S.lit);
    }
  } else if (v === 1) {
    // Stacked pods on a central trunk, smaller as they rise, like a budding stem.
    mb.color = S.barkC;
    mb.tube([[0, -0.04, 0], [0, 0.2, 0], [0, 0.36, 0]], [0.026, 0.018, 0.012], 6);
    roots(mb, r, 3, 0.025, 0.05, r.range(0, TAU));
    elfPod(mb, r, S, { rad: 0.05, h: 0.05, roofH: 0.06, open: 0.022, seg: 8, windows: 3 });
    elfPod(mb, r, S, { rad: 0.04, h: 0.045, roofH: 0.055, y: 0.12, open: 0.017, seg: 8, windows: 2, door: false, roofC: J(S.gold, r, 0.05) });
    elfPod(mb, r, S, { rad: 0.032, h: 0.04, roofH: 0.11, y: 0.235, seg: 7, windows: 2, door: false });
    lantern(mb, 0.05, 0.1, 0.03, S.lit);
  } else {
    // A white tower of the old elves: tapering, a balcony ring, a lit upper chamber, a bud cap.
    const wallC = J(S.dark ? "#4a4250" : "#d8d0bc", r, 0.03);
    mb.color = mulc(wallC, 0.85);
    mb.lathe([[0.05, -0.045], [0.046, 0.02]], 9, { caps: false });
    mb.color = wallC;
    mb.lathe([[0.042, 0.02], [0.036, 0.18], [0.03, 0.3]], 9, { caps: false });
    mb.lathe([[0.03, 0.3], [0.05, 0.31], [0.052, 0.318], [0.04, 0.32]], 9, { caps: false });
    mb.lathe([[0.036, 0.32], [0.038, 0.37], [0.034, 0.38]], 9, { caps: false });
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * TAU + 0.4;
      at(mb, Math.sin(a) * 0.0385, 0, Math.cos(a) * 0.0385, a, () => win(mb, S, 0, 0.33, 0.01, 0.03, true, 0.008));
    }
    for (let k = 0; k < 2; k++) {
      const a = r.range(-0.7, 0.7);
      at(mb, Math.sin(a) * 0.04, 0, Math.cos(a) * 0.04, a, () => win(mb, S, 0, 0.1 + k * 0.1, 0.008, 0.02, r() < 0.5, 0.006));
    }
    at(mb, 0, 0, 0.043, 0, () => door(mb, S, 0, 0.018, 0.03, 0.009, 0.02));
    mb.part(() => {
      mb.translate(0, 0.38, 0);
      mb.color = S.dark ? S.roofC : J(r() < 0.5 ? S.gold : "#4c7458", r, 0.04);
      petalRoof(mb, 0.055, 0.14, { petals: 8, seg: 9, lip: 0.05 });
    });
  }
  ground(mb, 0, 0.04, 0.6);
}

function elvenKeep(mb, r, v, sub) {
  const S = elfStyle(r, sub === "dark");
  // The great tree hall: a giant tree, a hall wrapped around its foot, platforms and pods in its
  // limbs, lanterns everywhere; the capital's tree is taller and crowned with a golden bud.
  const big = v === 1;
  const H = big ? 1.12 : 1.0;
  const tips = greatTree(mb, r, S, { h: H, rad: big ? 0.09 : 0.082, limbs: 5, crown: big ? 0.13 : 0.12, detail: 1, masses: 1, dome: 2, ramp: S.mallorn });
  elfPod(mb, r, S, { rad: 0.11, h: 0.07, roofH: 0.085, open: 0.06, seg: 11, windows: 5, petals: 9 });
  // Two wings: smaller pods against the trunk hall.
  for (const s of [-1, 1]) at(mb, s * 0.13, 0, 0.03, 0, () => elfPod(mb, r, S, { rad: 0.04, h: 0.05, roofH: 0.08, seg: 7, windows: 2, door: false, roofC: J(S.gold, r, 0.05) }));
  // Platforms around the trunk, pods in the limbs.
  for (const [y, rr] of [[0.2, 0.075], [0.31, 0.06]]) {
    mb.part(() => {
      mb.translate(0, y, 0);
      mb.color = mulc(S.barkC, 0.85);
      mb.lathe([[0.03, -0.014], [rr, -0.002], [rr * 1.04, 0.004], [rr, 0.006]], 10, { caps: false });
      mb.color = mulc(S.barkC, 1.05);
      mb.lathe([[rr * 1.02, 0.006], [rr * 1.02, 0.018]], 10, { caps: false });
    });
  }
  {
    const t = tips[0];
    at(mb, t[0] * 0.7, t[1] - 0.06, t[2] * 0.7, r.range(0, TAU), () => elfPod(mb, r, S, { rad: 0.03, h: 0.035, roofH: 0.06, seg: 6, windows: 1, door: false, roofC: J(S.gold, r, 0.05) }));
  }
  at(mb, 0.05, 0.2, 0.04, 0.8, () => elfPod(mb, r, S, { rad: 0.028, h: 0.034, roofH: 0.07, y: 0.006, seg: 6, windows: 1, door: false }));
  if (big)
    mb.part(() => {
      mb.translate(0, H + 0.1, 0);
      mb.color = S.dark ? S.roofC : C("#c8a048");
      mb.mat = MAT.METAL;
      petalRoof(mb, 0.04, 0.09, { petals: 6, seg: 8 });
    });
  // Lanterns hung in the limbs and at the hall.
  for (let k = 0; k < 6; k++) {
    const a = (k / 6) * TAU + r.range(-0.3, 0.3);
    const d = r.range(0.08, 0.15), y = r.range(0.16, 0.42);
    lantern(mb, Math.sin(a) * d, y, Math.cos(a) * d, S.lit, 0.0065);
  }
  // A team banner on the trunk's front.
  at(mb, 0, 0, 0.075, 0, () => wallBanner(mb, 0, 0.19, 0.03, 0.07));
  ground(mb, 0, 0.04, 0.6);
}

/** A hedge wall of woven living wood with a leafy crest (along x, length 0.2). */
function elvenWall(mb, r, sub) {
  const S = elfStyle(r, sub === "dark");
  mb.color = mulc(S.barkC, 0.9);
  for (let k = 0; k < 3; k++) {
    const pts = [];
    for (let i = 0; i <= 6; i++) {
      const x = -0.105 + (0.21 * i) / 6;
      pts.push([x, 0.012 + k * 0.022 + 0.008 * Math.sin(i * 1.9 + k * 2.1), 0.006 * Math.cos(i * 2.3 + k)]);
    }
    mb.tube(pts, 0.008, 4);
  }
  mb.color = S.barkC;
  for (let i = 0; i < 5; i++) {
    const x = -0.09 + i * 0.045;
    mb.tube([[x, -0.04, 0], [x + 0.004, 0.04, 0.003], [x - 0.003, 0.085, -0.002]], [0.008, 0.006, 0.003], 4);
  }
  for (let i = 0; i < 3; i++) leaves(mb, r, [-0.07 + i * 0.07, 0.08, 0], 0.03, 2, S.leaf, { squash: 0.6, spread: 1.2, sway: 0.3 });
  if (S.dark) thorns(mb, r, [[-0.06, 0.06, 0.008, 0, 1], [0.02, 0.05, 0.008, 0, 1], [0.07, 0.065, 0.008, 0, 1]], 0.02);
  ground(mb, 0, 0.03, 0.6);
}

/** A gate of two trees arching to meet over the way, a lantern hung at the apex, banners. */
function elvenGate(mb, r, sub) {
  const S = elfStyle(r, sub === "dark");
  mb.color = S.barkC;
  for (const s of [-1, 1]) {
    mb.tube([[s * 0.07, -0.04, 0], [s * 0.068, 0.08, 0], [s * 0.055, 0.15, 0], [s * 0.02, 0.2, 0], [0, 0.205, 0]], [0.022, 0.018, 0.013, 0.009, 0.007], 6);
    roots(mb, r, 2, 0.02, 0.04, s > 0 ? 1 : 4);
    leaves(mb, r, [s * 0.06, 0.2, 0], 0.045, 2, S.leaf, { sway: 0.3 });
    at(mb, s * 0.07, 0, 0.021, 0, () => wallBanner(mb, 0, 0.14, 0.024, 0.055));
  }
  leaves(mb, r, [0, 0.235, 0], 0.04, 2, S.leaf, { sway: 0.3 });
  lantern(mb, 0, 0.17, 0, S.lit, 0.008);
  ground(mb, 0, 0.03, 0.6);
}

/** A wall tree: a trunk with a small lookout pod (v1 taller). */
function elvenWallTower(mb, r, v, sub) {
  const S = elfStyle(r, sub === "dark");
  const y = v ? 0.12 : 0.09;
  mb.color = S.barkC;
  mb.tube([[0, -0.04, 0], [0.004, y * 0.6, 0], [0, y, 0]], [0.024, 0.018, 0.022], 6);
  roots(mb, r, 3, 0.022, 0.04, r.range(0, TAU));
  elfPod(mb, r, S, { rad: 0.035, h: 0.035, roofH: 0.07, y, seg: 7, windows: 2, door: false, roofC: v ? J(S.gold, r, 0.05) : S.roofC });
  ground(mb, 0, 0.03, 0.6);
}

// --- dwarven: heavy stone halls set into the ground, dark slate, forge glow ------------------------

const DWARF = {
  stone: ["#7a746e", "#706a64", "#837c74", "#766f66"],
  dark: "#4a4644",
  roof: [["#3b3b42", 3], ["#34343c", 2], ["#46403c", 1]],
  turf: "#4e5a36",
  earth: "#5a4632",
  bronze: "#9a6a3a",
  gold: "#b8923e",
  glow: "#ff8a30",
  lit: "#ffae58",
  glass: "#1e1a18",
  door: "#3a2a20",
  chimney: "#5e5852",
};

function dwarfStyle(r) {
  return { ...DWARF, stoneC: J(r.pick(DWARF.stone), r, 0.04), roofC: J(wpick(r, DWARF.roof), r, 0.04), lit: DWARF.lit, glass: DWARF.glass, door: DWARF.door };
}

/** A heavy dwarven door: a dark round-headed opening in a bronze frame. */
function dwarfDoor(mb, S, x, w, h, y = 0.004, glow = false) {
  mb.part(() => {
    mb.mat = MAT.METAL;
    mb.color = C(S.bronze);
    rect(mb, x - w / 2 - 0.004, y, x + w / 2 + 0.004, y + h + 0.002, -0.0004);
    mb.tri([x - w / 2 - 0.004, y + h + 0.002, -0.0004], [x + w / 2 + 0.004, y + h + 0.002, -0.0004], [x, y + h + w * 0.55 + 0.004, -0.0004]);
    mb.mat = glow ? MAT.EMISSIVE : MAT.MATTE;
    mb.color = C(glow ? "#d0581c" : S.door);
    rect(mb, x - w / 2, y, x + w / 2, y + h);
    mb.tri([x - w / 2, y + h, 0], [x + w / 2, y + h, 0], [x, y + h + w * 0.5, 0]);
  });
}

/** A dwarven stone block: battered walls, a darker footing, a heavy door, a few lit slits. */
function dwarfBlock(mb, r, S, w, d, h, { doorX = 0, doorSide = 0, door = true, wins = true, top = 0.93, glowDoor = false, dw = 0.02 } = {}) {
  mb.color = mulc(S.stoneC, 0.78);
  blk(mb, w + 0.012, d + 0.012, -0.05, 0.012, 0.97);
  mb.color = S.stoneC;
  shell(mb, w, d, 0.012, h, top);
  mb.color = mulc(S.stoneC, 0.88);
  shell(mb, w * (1 - (1 - top) * 0.55) + 0.002, d * (1 - (1 - top) * 0.55) + 0.002, h * 0.5, h * 0.56);
  for (let s = 0; s < 4; s++)
    face(mb, s, w * (1 - (1 - top) * 0.3), d * (1 - (1 - top) * 0.3), (span) => {
      if (door && s === doorSide) dwarfDoor(mb, S, doorX, dw, Math.min(0.032, h * 0.55), 0.012, glowDoor);
      else if (wins && span > 0.06) {
        const k = span > 0.12 ? 2 : 1;
        for (let i = 0; i < k; i++) win(mb, S, -span / 2 + (span * (i + 0.5)) / k, h * 0.42, 0.009, 0.014, r() < 0.4, 0.004);
      }
    });
  return { w: w * top, d: d * top, y: h };
}

/** A heavy slate roof on a block {w, d, y}: low pitch, thick slabs ("x" ridge along x, "z", "hip"). */
function dwarfRoof(mb, r, S, b, kind, pitch, o = {}) {
  mb.part(() => {
    mb.translate(0, b.y, 0);
    mb.color = o.color ?? S.roofC;
    if (kind === "hip") hipRoof(mb, b.w, b.d, pitch, { over: 0.014, ridge: o.ridge ?? b.w * 0.4, thick: 0.012 });
    else {
      if (kind === "z") mb.rotateY(QT);
      const [w, d] = kind === "z" ? [b.d, b.w] : [b.w, b.d];
      gableRoof(mb, w, d, pitch, { over: 0.016, overX: 0.01, thick: 0.013, kick: 0.6, ends: o.ends ?? S.stoneC });
      if (o.ridge !== false) {
        mb.mat = MAT.METAL;
        mb.color = C(S.bronze);
        mb.tube([[-w / 2 - 0.01, pitch + 0.002, 0], [w / 2 + 0.01, pitch + 0.002, 0]], 0.004, 4);
      }
    }
  });
}

/** An earth berm hugging a w x d footprint up to height h (turf on top). */
function berm(mb, r, S, w, d, h) {
  mb.part(() => {
    mb.scale((w / 2 + 0.004) / 0.05 / Math.cos(TAU / 16), 1, (d / 2 + 0.004) / 0.05 / Math.cos(TAU / 16));
    mb.color = J(S.earth, r, 0.05);
    mb.lathe([[0.068, -0.04], [0.064, h * 0.3]], 8, { caps: false, phase: TAU / 16 });
    mb.color = J(S.turf, r, 0.06);
    mb.lathe([[0.064, h * 0.3], [0.056, h * 0.8], [0.05, h]], 8, { caps: false, phase: TAU / 16 });
  });
}

function dwarvenHouse(mb, r, v) {
  const S = dwarfStyle(r);
  const glow = DWARF.glow;
  switch (v) {
    case 0: {
      // A stone hall half sunk in a turf berm, a low heavy roof, a massive glowing chimney.
      const w = r.range(0.11, 0.125), d = r.range(0.085, 0.095);
      const b = dwarfBlock(mb, r, S, w, d, 0.06, { doorX: r.range(-0.02, 0.02) });
      berm(mb, r, S, w, d, 0.03);
      dwarfRoof(mb, r, S, b, "x", 0.05);
      chimney(mb, S.chimney, r.sign() * w * 0.3, -d * 0.15, 0.05, 0.14, 0.024, glow);
      break;
    }
    case 1: {
      // A round stone hut: thick battered walls, a low cone of slate.
      const R = r.range(0.05, 0.056);
      mb.color = mulc(S.stoneC, 0.78);
      drum(mb, R * 1.12, R * 1.08, -0.05, 0.012, 9);
      mb.color = S.stoneC;
      drum(mb, R, R * 0.9, 0.012, 0.06, 9);
      at(mb, 0, 0, R * 0.98, 0.0, () => dwarfDoor(mb, S, 0, 0.018, 0.03, 0.012));
      for (const a of [1.4, -1.6, 3.0]) at(mb, Math.sin(a) * R * 0.97, 0, Math.cos(a) * R * 0.97, a, () => win(mb, S, 0, 0.032, 0.008, 0.013, r() < 0.45, 0.004));
      mb.part(() => {
        mb.translate(0, 0.06, 0);
        mb.color = S.roofC;
        coneRoof(mb, R * 1.15, 0.06, { seg: 9, curve: 0.0, kick: 0.0, drop: 0.008, thick: 0.012, rings: 1 });
      });
      chimney(mb, S.chimney, R * 0.35, -R * 0.3, 0.04, 0.12, 0.02, glow);
      break;
    }
    case 2: {
      // A house cut into a boulder: a dressed stone front, a slate porch, a chimney through the rock.
      mb.color = J(DWARF.dark, r, 0.05);
      const seed = r.range(0, 50);
      at(mb, 0, 0.02, -0.02, 0, () => mb.blob(0.075, { detail: 1, amp: 0.28, squash: 0.72, noise: (v) => noise3(v[0] * 1.7 + seed, v[1] * 1.7, v[2] * 1.7), flat: true }));
      mb.part(() => {
        mb.translate(0, 0, 0.035);
        const b = dwarfBlock(mb, r, S, 0.085, 0.04, 0.055, { doorX: 0 });
        mb.translate(0, b.y, 0.008);
        mb.color = S.roofC;
        mb.quad([-0.055, 0.016, -0.02], [0.055, 0.016, -0.02], [0.055, -0.004, 0.03], [-0.055, -0.004, 0.03]);
      });
      chimney(mb, S.chimney, -0.025, -0.03, 0.06, 0.14, 0.02, glow);
      break;
    }
    case 3: {
      // A stepped house: a broad lower storey, a smaller upper one set back, two hipped roofs.
      const b = dwarfBlock(mb, r, S, 0.125, 0.1, 0.05, { doorX: -0.03 });
      dwarfRoof(mb, r, S, b, "hip", 0.03, { ridge: 0.08 });
      mb.part(() => {
        mb.translate(0.015, 0.05, -0.012);
        mb.color = S.stoneC;
        shell(mb, 0.07, 0.06, 0, 0.045, 0.95);
        face(mb, 0, 0.07, 0.06, () => win(mb, S, 0, 0.018, 0.012, 0.014, r() < 0.6, 0.005));
        dwarfRoof(mb, r, S, { w: 0.066, d: 0.057, y: 0.045 }, "hip", 0.045, { ridge: 0.02 });
      });
      chimney(mb, S.chimney, -0.04, -0.025, 0.04, 0.15, 0.02, glow);
      break;
    }
    case 4: {
      // A forge: an open, glowing front under a slate roof, a great chimney, an anvil.
      const b = dwarfBlock(mb, r, S, 0.11, 0.09, 0.055, { door: false });
      face(mb, 0, b.w / 0.93, 0.09, () => {
        mb.part(() => {
          mb.color = C("#1a1210");
          rect(mb, -0.035, 0.012, 0.035, 0.045, 0.0003);
          mb.mat = MAT.EMISSIVE;
          mb.color = C("#e0601e");
          rect(mb, -0.022, 0.014, 0.022, 0.03, 0.0006);
        });
      });
      dwarfRoof(mb, r, S, b, "x", 0.05);
      chimney(mb, S.chimney, 0.035, -0.02, 0.04, 0.17, 0.03, glow);
      mb.part(() => {
        mb.mat = MAT.METAL;
        mb.color = C("#3a3a40");
        at(mb, -0.02, 0, 0.075, 0, () => {
          blk(mb, 0.01, 0.01, -0.01, 0.012);
          blk(mb, 0.022, 0.011, 0.012, 0.02);
        });
      });
      break;
    }
    default: {
      // A long hall with buttresses and a bronze-ridged roof.
      const w = r.range(0.15, 0.16), d = 0.082;
      const b = dwarfBlock(mb, r, S, w, d, 0.055, { doorX: r.range(-0.03, 0.03) });
      mb.color = mulc(S.stoneC, 0.9);
      for (const z of [-1, 1]) for (let i = 0; i < 3; i++) at(mb, -w * 0.33 + i * w * 0.33, 0, (z * d) / 2 + z * 0.006, 0, () => blk(mb, 0.014, 0.014, -0.04, 0.05, 0.7));
      dwarfRoof(mb, r, S, b, "x", 0.065);
      chimney(mb, S.chimney, -w * 0.3, 0, 0.05, 0.15, 0.022, glow);
      break;
    }
  }
  ground(mb, 0, 0.04, 0.55);
}

/** A stout round dwarven tower: battered drum, machicolations, parapet, a low cone or a brazier. */
function dwarfTower(mb, r, S, R, h, { roof = true, brazier = false, crenel = true } = {}) {
  mb.color = mulc(S.stoneC, 0.78);
  drum(mb, R * 1.12, R * 1.06, -0.05, 0.016, 10);
  mb.color = S.stoneC;
  drum(mb, R, R * 0.88, 0.016, h, 10);
  mb.color = mulc(S.stoneC, 0.88);
  drum(mb, R * 0.95 + 0.001, R * 0.93 + 0.001, h * 0.45, h * 0.5, 10);
  const t = R * 0.88;
  mb.color = mulc(S.stoneC, 0.95);
  mb.lathe([[t, h - 0.006], [t * 1.18, h + 0.012], [t * 1.18, h + 0.03], [t * 1.0, h + 0.03]], 10, { caps: false });
  if (crenel) {
    mb.color = S.stoneC;
    merlonsRing(mb, t * 1.12, h + 0.03, 7, { mw: t * 0.55, mh: 0.016, md: 0.012 });
  }
  for (const a of [0.3, -0.9, 2.2]) at(mb, Math.sin(a) * (R * 0.92 + 0.001), 0, Math.cos(a) * (R * 0.92 + 0.001), a, () => win(mb, S, 0, h * 0.6, 0.008, 0.016, r() < 0.5, 0.004));
  at(mb, 0, 0, R * 0.99, 0, () => dwarfDoor(mb, S, 0, 0.02, 0.03, 0.016));
  if (roof)
    mb.part(() => {
      mb.translate(0, h + 0.03, 0);
      mb.color = S.roofC;
      coneRoof(mb, t * 1.05, h * 0.3, { seg: 10, curve: 0.05, kick: 0, drop: 0.0, thick: 0.01, rings: 1 });
      mb.translate(0, h * 0.3, 0);
      finial(mb, 0, 0.016, S.gold);
    });
  if (brazier)
    mb.part(() => {
      mb.translate(0, h + 0.03, 0);
      mb.mat = MAT.METAL;
      mb.color = C("#3a3634");
      mb.lathe([[0.006, 0], [0.02, 0.012], [0.026, 0.024]], 8, { caps: false });
      mb.mat = MAT.EMISSIVE;
      mb.color = C(DWARF.glow);
      at(mb, 0, 0.02, 0, 0, () => mb.sphere(0.02, 0, { squash: 0.5 }));
    });
}

function dwarvenTower(mb, r, v) {
  const S = dwarfStyle(r);
  if (v === 0) dwarfTower(mb, r, S, 0.065, 0.24);
  else if (v === 1) {
    // A square stout tower with a great brazier burning on its roof.
    const b = dwarfBlock(mb, r, S, 0.1, 0.1, 0.26, { top: 0.88, dw: 0.022 });
    mb.color = mulc(S.stoneC, 0.95);
    blk(mb, b.w, b.d, b.y, b.y + 0.014, 1.16);
    mb.color = S.stoneC;
    merlonsRect(mb, b.w * 1.16, b.d * 1.16, b.y + 0.014, { mw: 0.022, mh: 0.018, md: 0.012, n: 2 });
    face(mb, 0, b.w, b.d, () => wallBanner(mb, 0, 0.23, 0.03, 0.065));
    mb.part(() => {
      mb.translate(0, b.y + 0.014, 0);
      mb.mat = MAT.METAL;
      mb.color = C("#3a3634");
      mb.lathe([[0.006, 0], [0.022, 0.012], [0.028, 0.026]], 8, { caps: false });
      mb.mat = MAT.EMISSIVE;
      mb.color = C(DWARF.glow);
      at(mb, 0, 0.022, 0, 0, () => mb.sphere(0.022, 0, { squash: 0.5 }));
    });
  } else {
    // A workshop tower (the Luchuirp's): octagonal, a bronze gear-ring, a tall glowing smokestack.
    mb.color = mulc(S.stoneC, 0.78);
    shard(mb, 8, 0.066, -0.05, 0.016, { top: 0.98, phase: TAU / 16 });
    mb.color = S.stoneC;
    shard(mb, 8, 0.06, 0.016, 0.26, { top: 0.84, phase: TAU / 16 });
    mb.mat = MAT.METAL;
    mb.color = C(S.bronze);
    mb.lathe([[0.056, 0.18], [0.062, 0.185], [0.062, 0.2], [0.054, 0.205]], 12, { caps: false, wobble: (a, y, i) => (i === 1 || i === 2 ? 0.08 * (Math.cos(a * 12) > 0 ? 1 : -1) : 0) });
    mb.mat = MAT.MATTE;
    for (const a of [0, 2.1, -2.1]) at(mb, Math.sin(a) * 0.057, 0, Math.cos(a) * 0.057, a, () => win(mb, S, 0, 0.1, 0.01, 0.018, r() < 0.6, 0.005));
    at(mb, 0, 0, 0.061, 0, () => dwarfDoor(mb, S, 0, 0.02, 0.03, 0.016));
    mb.part(() => {
      mb.translate(0, 0.26, 0);
      mb.color = S.roofC;
      coneRoof(mb, 0.058, 0.04, { seg: 8, curve: 0, kick: 0, drop: 0, thick: 0.008, rings: 1, phase: TAU / 16 });
    });
    chimney(mb, S.chimney, 0.012, 0.0, 0.25, 0.4, 0.026, DWARF.glow);
  }
  ground(mb, 0, 0.04, 0.55);
}

function dwarvenHall(mb, r, v) {
  const S = dwarfStyle(r);
  if (v === 0) {
    // A forge hall: a broad stone hall under a stepped roof, three glowing chimneys, a great
    // glowing doorway.
    const b = dwarfBlock(mb, r, S, 0.2, 0.13, 0.06, { dw: 0.034, glowDoor: true });
    dwarfRoof(mb, r, S, b, "hip", 0.03, { ridge: 0.16 });
    mb.part(() => {
      mb.translate(0, 0.06, -0.01);
      mb.color = S.stoneC;
      shell(mb, 0.13, 0.07, 0, 0.04, 0.96);
      face(mb, 0, 0.13, 0.07, (span) => windowRow(mb, r, S, span, 0.012, 0.014, { n: 3 }));
      dwarfRoof(mb, r, S, { w: 0.125, d: 0.067, y: 0.04 }, "x", 0.04);
    });
    for (const x of [-0.07, 0.0, 0.07]) chimney(mb, S.chimney, x, -0.05, 0.05, 0.19 + r.range(-0.01, 0.02), 0.024, DWARF.glow);
  } else if (v === 1) {
    // A mead hall: a long stone hall under a tall slate roof with a bronze ridge and gilded gables.
    const b = dwarfBlock(mb, r, S, 0.19, 0.11, 0.065, { dw: 0.026 });
    dwarfRoof(mb, r, S, b, "x", 0.085);
    for (const s of [-1, 1])
      mb.part(() => {
        mb.mat = MAT.METAL;
        mb.color = C(S.gold);
        mb.translate((s * b.w) / 2 + s * 0.001, b.y, 0);
        mb.tri([0, 0.015, -0.025], [0, 0.015, 0.025], [0, 0.06, 0]);
      });
    chimney(mb, S.chimney, -0.05, -0.025, 0.06, 0.18, 0.022, DWARF.glow);
    chimney(mb, S.chimney, 0.06, -0.025, 0.06, 0.17, 0.022);
  } else {
    // A domed hall: a stone drum under a low stone dome, a pillared porch.
    mb.color = mulc(S.stoneC, 0.78);
    drum(mb, 0.09, 0.088, -0.05, 0.014, 12);
    mb.color = S.stoneC;
    drum(mb, 0.08, 0.077, 0.014, 0.07, 12);
    for (let k = 0; k < 5; k++) {
      const a = (k / 5) * TAU + 0.6;
      at(mb, Math.sin(a) * 0.079, 0, Math.cos(a) * 0.079, a, () => win(mb, S, 0, 0.035, 0.009, 0.016, r() < 0.5, 0.005));
    }
    mb.part(() => {
      mb.translate(0, 0.07, 0);
      mb.color = mulc(S.stoneC, 0.85);
      mb.lathe([[0.084, -0.002], [0.088, 0.008]], 12, { caps: false });
      mb.translate(0, 0.008, 0);
      mb.color = J("#5a5650", r, 0.04);
      dome(mb, 0.086, 0.06, 12);
      mb.translate(0, 0.06, 0);
      finial(mb, 0, 0.02, S.gold, 0.004);
    });
    mb.part(() => {
      mb.translate(0, 0, 0.09);
      mb.color = mulc(S.stoneC, 1.05);
      for (const s of [-1, 1]) at(mb, s * 0.026, 0, 0.01, 0, () => blk(mb, 0.012, 0.012, 0.0, 0.05));
      blk(mb, 0.07, 0.03, 0.05, 0.06);
      blk(mb, 0.06, 0.03, -0.04, 0.008);
      mb.translate(0, 0.06, 0);
      mb.rotateY(QT);
      mb.color = S.roofC;
      gableRoof(mb, 0.03, 0.068, 0.025, { over: 0.006, overX: 0.004, thick: 0.006, ends: S.stoneC });
    });
    at(mb, 0, 0, 0.08, 0, () => dwarfDoor(mb, S, 0, 0.022, 0.032, 0.012));
  }
  ground(mb, 0, 0.04, 0.55);
}

function dwarvenKeep(mb, r, v) {
  const S = dwarfStyle(r);
  // A mountain hall: stepped stone terraces, a great gilded gate, stout towers at its front
  // corners, a heavy roofed hall on top, chimneys glowing (the capital's crowned by a dome).
  const b1 = dwarfBlock(mb, r, S, 0.25, 0.19, 0.085, { door: false, top: 0.95 });
  mb.color = mulc(S.stoneC, 0.95);
  blk(mb, b1.w + 0.004, b1.d + 0.004, 0.085, 0.095);
  mb.part(() => {
    mb.translate(0, 0.095, -0.015);
    mb.color = S.stoneC;
    shell(mb, 0.18, 0.13, 0, 0.07, 0.95);
    face(mb, 0, 0.18, 0.13, (span) => windowRow(mb, r, S, span, 0.024, 0.02, { n: 4, w: 0.01, arch: 0.005 }));
    mb.color = mulc(S.stoneC, 0.92);
    blk(mb, 0.18 * 0.95 + 0.004, 0.13 * 0.95 + 0.004, 0.07, 0.078);
    if (v === 1) {
      mb.translate(0, 0.078, 0);
      mb.color = mulc(S.stoneC, 0.9);
      drum(mb, 0.07, 0.068, 0, 0.04, 12);
      mb.translate(0, 0.04, 0);
      mb.mat = MAT.METAL;
      mb.color = C("#4e6a5c");
      dome(mb, 0.074, 0.075, 12);
      mb.translate(0, 0.075, 0);
      finial(mb, 0, 0.03, S.gold, 0.005);
    } else dwarfRoof(mb, r, S, { w: 0.171, d: 0.12, y: 0.078 }, "x", 0.09);
  });
  // The great gate.
  face(mb, 0, b1.w / 0.95, 0.19, () => {
    mb.part(() => {
      mb.mat = MAT.METAL;
      mb.color = C(S.gold);
      rect(mb, -0.042, 0.062, 0.042, 0.074, 0.0008);
      mb.color = C(S.bronze);
      rect(mb, -0.034, 0.012, 0.034, 0.064, 0.0004);
      mb.mat = MAT.MATTE;
      mb.color = C("#120e0c");
      rect(mb, -0.027, 0.012, 0.027, 0.058, 0.0008);
      mb.mat = MAT.EMISSIVE;
      mb.color = C("#c0501a");
      rect(mb, -0.018, 0.012, 0.018, 0.022, 0.0012);
    });
    wallBanner(mb, -0.075, 0.08, 0.026, 0.055);
    wallBanner(mb, 0.075, 0.08, 0.026, 0.055);
  });
  for (const s of [-1, 1]) at(mb, s * 0.12, 0, 0.085, 0, () => dwarfTower(mb, r, S, 0.042, 0.19, { crenel: v === 1, roof: v === 0 }));
  for (const x of [-0.06, 0.06]) chimney(mb, S.chimney, x, -0.07, 0.15, 0.3, 0.026, DWARF.glow);
  ground(mb, 0, 0.05, 0.55);
}

/** A massive dwarven wall segment (along x, length 0.2). */
function dwarvenWall(mb, r) {
  const S = dwarfStyle(r);
  mb.color = mulc(S.stoneC, 0.78);
  blk(mb, 0.205, 0.056, -0.05, 0.016, 0.96);
  mb.color = S.stoneC;
  shell(mb, 0.205, 0.046, 0.016, 0.075, 0.92);
  mb.color = mulc(S.stoneC, 0.8);
  mb.quad([-0.1025, 0.075, -0.021], [-0.1025, 0.075, 0.021], [0.1025, 0.075, 0.021], [0.1025, 0.075, -0.021]);
  mb.color = S.stoneC;
  merlonsLine(mb, -0.09, 0.09, 0.075, 0.014, { mw: 0.034, mh: 0.02, md: 0.014, n: 3 });
  ground(mb, 0, 0.04, 0.6);
}

function dwarvenGate(mb, r) {
  const S = dwarfStyle(r);
  const b = dwarfBlock(mb, r, S, 0.12, 0.07, 0.12, { door: false, top: 0.95 });
  mb.color = S.stoneC;
  merlonsLine(mb, -0.05, 0.05, 0.12, b.d / 2 - 0.006, { mw: 0.026, mh: 0.018, md: 0.012, n: 3 });
  mb.color = mulc(S.stoneC, 0.8);
  mb.quad([-b.w / 2, 0.12, -b.d / 2], [-b.w / 2, 0.12, b.d / 2], [b.w / 2, 0.12, b.d / 2], [b.w / 2, 0.12, -b.d / 2]);
  for (const side of [0, 2])
    face(mb, side, 0.12 * 0.97, 0.07 * 0.97, () => {
      mb.part(() => {
        mb.mat = MAT.METAL;
        mb.color = C(S.gold);
        rect(mb, -0.038, 0.07, 0.038, 0.082, 0.0008);
        mb.mat = MAT.MATTE;
        mb.color = C("#14100e");
        rect(mb, -0.028, 0.0, 0.028, 0.068, 0.0006);
      });
    });
  for (const s of [-1, 1]) at(mb, s * 0.075, 0, 0.004, 0, () => dwarfTower(mb, r, S, 0.042, 0.13, { roof: true, crenel: false }));
  at(mb, -0.075, 0, 0.047, 0, () => wallBanner(mb, 0, 0.11, 0.024, 0.05));
  at(mb, 0.075, 0, 0.047, 0, () => wallBanner(mb, 0, 0.11, 0.024, 0.05));
  ground(mb, 0, 0.04, 0.6);
}

function dwarvenWallTower(mb, r, v) {
  const S = dwarfStyle(r);
  dwarfTower(mb, r, S, 0.046, 0.1, { roof: v === 1, crenel: v === 0 });
  ground(mb, 0, 0.04, 0.6);
}

// --- infernal: obsidian shards, bone, chains, lava-lit slits, the planar gate -------------------------

const HELL = {
  obs: ["#1e1a22", "#24202a", "#1a161c"],
  basalt: ["#3a343a", "#423a40", "#36302e"],
  hide: "#3a2622",
  bone: "#a89c82",
  iron: "#46484e",
  glow: "#ff4a12",
  lit: "#ff5a1e",
  glass: "#120c0e",
  door: "#0c0809",
  portal: "#2a0a20",
  rim: "#ff3a3a",
};

function hellStyle(r) {
  return { ...HELL, obsC: J(r.pick(HELL.obs), r, 0.04), basaltC: J(r.pick(HELL.basalt), r, 0.04), boneC: J(HELL.bone, r, 0.05) };
}

/** A glowing slit (lava light through a crack) on a face. */
function slit(mb, x, y, w, h, c = HELL.glow) {
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C(c);
    mb.quad([x - w / 2, y, 0], [x + w / 2, y, 0], [x + w * 0.2, y + h, 0], [x - w * 0.2, y + h, 0]);
  });
}

/** A horn or tusk: a tapering curved tube from p rising and curling toward `dir`. */
function horn(mb, p, dir, len, rad, curl = 0.6) {
  const [dx, dz] = dir;
  const pts = [], rr = [];
  for (let k = 0; k <= 4; k++) {
    const t = k / 4;
    const out = Math.sin(t * curl * 2) * len * 0.6;
    pts.push([p[0] + dx * out, p[1] + t * len * (1 - t * 0.35 * curl), p[2] + dz * out]);
    rr.push(rad * (1 - t * 0.9));
  }
  mb.tube(pts, rr, 4);
}

/** A sagging chain between two points (dark iron). */
function chain(mb, a, b, sag = 0.03) {
  mb.part(() => {
    mb.mat = MAT.METAL;
    mb.color = C(HELL.iron);
    const pts = [];
    for (let k = 0; k <= 4; k++) {
      const t = k / 4;
      pts.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t - sag * Math.sin(Math.PI * t), a[2] + (b[2] - a[2]) * t]);
    }
    mb.tube(pts, 0.0022, 3);
  });
}

/** A cluster of leaning obsidian spikes on a footing, around (0, y0, 0). */
function spikes(mb, r, S, n, rad, h, y0 = 0, spread = 0.03) {
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + r.range(-0.4, 0.4);
    const d = k === 0 ? 0 : spread * r.range(0.7, 1.1);
    const hh = h * (k === 0 ? 1 : r.range(0.45, 0.7));
    mb.color = k % 2 ? S.obsC : mulc(S.obsC, 1.25);
    at(mb, Math.sin(a) * d, 0, Math.cos(a) * d, 0, () => shard(mb, 4, rad * (k === 0 ? 1 : 0.7), y0 - 0.01, y0 + hh, { lean: [Math.sin(a) * hh * 0.25, Math.cos(a) * hh * 0.25], phase: r.range(0, 2) }));
  }
}

function infernalHouse(mb, r, v) {
  const S = hellStyle(r);
  switch (v) {
    case 0: {
      // A shard hut: a leaning five-sided basalt body, a crown of obsidian spikes, glowing slits.
      mb.color = S.basaltC;
      shard(mb, 5, 0.052, -0.05, 0.07, { top: 0.78, lean: [r.range(-0.006, 0.006), -0.004], phase: -TAU / 10 });
      const fz = 0.052 * Math.cos(TAU / 10);
      at(mb, 0, 0, fz * 0.97 + 0.002, 0, () => {
        door(mb, S, 0, 0.018, 0.03, 0.012, 0.0, HELL.door);
        slit(mb, 0.024, 0.035, 0.006, 0.02);
      });
      spikes(mb, r, S, 3, 0.03, 0.12, 0.06, 0.025);
      break;
    }
    case 1: {
      // A bone tent: dark hide stretched over great ribs, a skull over the door.
      mb.color = J(S.hide, r, 0.05);
      mb.lathe([[0.058, -0.03], [0.055, 0.01], [0.034, 0.06], [0.012, 0.1], [0.004, 0.12]], 7, { caps: false });
      mb.color = S.boneC;
      for (let k = 0; k < 4; k++) {
        const a = (k / 4) * TAU + 0.4;
        horn(mb, [Math.sin(a) * 0.066, -0.02, Math.cos(a) * 0.066], [-Math.sin(a), -Math.cos(a)], 0.14, 0.007, 0.55);
      }
      at(mb, 0, 0.06, 0.04, 0, () => mb.sphere(0.012, 0, { squash: 0.9 }));
      at(mb, 0, 0, 0.056, 0, () => slit(mb, 0, 0.0, 0.016, 0.03, "#c83a14"));
      break;
    }
    case 2: {
      // A black stone hovel under a steep pyramid whose corners rise into horns.
      const w = 0.09, d = 0.08;
      mb.color = mulc(S.basaltC, 0.85);
      blk(mb, w + 0.01, d + 0.01, -0.05, 0.01);
      mb.color = S.basaltC;
      shell(mb, w, d, 0.01, 0.06, 0.94);
      face(mb, 0, w * 0.97, d * 0.97, () => {
        door(mb, S, 0, 0.018, 0.03, 0.01, 0.01, HELL.door);
        slit(mb, -0.028, 0.03, 0.006, 0.018);
      });
      face(mb, 1, w * 0.97, d * 0.97, () => slit(mb, 0, 0.03, 0.006, 0.018));
      mb.part(() => {
        mb.translate(0, 0.06, 0);
        mb.color = S.obsC;
        hipRoof(mb, w * 0.94, d * 0.94, 0.1, { over: 0.012 });
        mb.color = S.boneC;
        for (const [x, z] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) horn(mb, [x * 0.05, -0.006, z * 0.045], [x * 0.7, z * 0.7], 0.06, 0.006, 0.9);
      });
      break;
    }
    case 3: {
      // A twisted tower-house: a tall turning four-sided shard, a horned cap, slits up its faces.
      mb.color = S.basaltC;
      shard(mb, 4, 0.05, -0.05, 0.16, { top: 0.62, twist: 0.5, phase: TAU / 8 });
      mb.color = S.obsC;
      shard(mb, 4, 0.034, 0.158, 0.26, { top: 0, twist: 0.3, lean: [0.01, -0.006], phase: TAU / 8 + 0.5 });
      at(mb, 0, 0, 0.037, 0, () => {
        door(mb, S, 0, 0.016, 0.028, 0.008, 0.0, HELL.door);
        slit(mb, 0.0, 0.07, 0.005, 0.024);
        slit(mb, 0.0, 0.11, 0.004, 0.018);
      });
      mb.color = S.boneC;
      for (const s of [-1, 1]) horn(mb, [s * 0.022, 0.15, 0.0], [s, 0], 0.05, 0.005, 0.9);
      break;
    }
    case 4: {
      // A lava pit: a jagged ring of leaning shards around a glowing pool, chained bone stakes.
      for (let k = 0; k < 7; k++) {
        const a = (k / 7) * TAU + r.range(-0.2, 0.2);
        mb.color = k % 2 ? S.obsC : S.basaltC;
        at(mb, Math.sin(a) * 0.05, 0, Math.cos(a) * 0.05, 0, () => shard(mb, 4, 0.016, -0.04, r.range(0.04, 0.08), { lean: [Math.sin(a) * 0.02, Math.cos(a) * 0.02], phase: a }));
      }
      mb.part(() => {
        mb.mat = MAT.EMISSIVE;
        mb.color = C("#e0400e");
        mb.lathe([[0.042, 0.004], [0, 0.008]], 8, { caps: false });
      });
      mb.color = S.boneC;
      const p0 = [0.07, 0.07, 0.03], p1 = [-0.065, 0.06, 0.04];
      pole(mb, [0.07, -0.03, 0.03], p0, 0.005, 0.003, 4);
      pole(mb, [-0.065, -0.03, 0.04], p1, 0.005, 0.003, 4);
      chain(mb, p0, p1, 0.035);
      break;
    }
    default: {
      // A shrine of the veil: a narrow dark chapel with a jagged front and a glowing round eye.
      const w = 0.07, d = 0.11;
      mb.color = mulc(S.basaltC, 0.85);
      blk(mb, w + 0.01, d + 0.01, -0.05, 0.01);
      mb.color = S.basaltC;
      shell(mb, w, d, 0.01, 0.07);
      mb.part(() => {
        mb.translate(0, 0.07, 0);
        mb.rotateY(QT);
        mb.color = S.obsC;
        gableRoof(mb, d, w, 0.08, { over: 0.008, overX: 0.006, thick: 0.006, ends: S.basaltC });
      });
      face(mb, 0, w, d, () => {
        mb.color = S.obsC;
        mb.tri([-0.04, 0.06, 0], [0.04, 0.06, 0], [0, 0.2, 0]);
        door(mb, S, 0, 0.018, 0.034, 0.012, 0.01, HELL.door);
        mb.part(() => {
          mb.mat = MAT.EMISSIVE;
          mb.color = C(HELL.glow);
          const R = 0.008, cy = 0.09;
          for (let k = 0; k < 6; k++) {
            const a0 = (k / 6) * TAU, a1 = ((k + 1) / 6) * TAU;
            mb.tri([0, cy, 0.001], [Math.cos(a0) * R, cy + Math.sin(a0) * R, 0.001], [Math.cos(a1) * R, cy + Math.sin(a1) * R, 0.001]);
          }
        });
      });
      for (const s of [-1, 1]) at(mb, s * 0.04, 0, 0.055, 0, () => { mb.color = S.obsC; shard(mb, 4, 0.01, -0.03, 0.12, { lean: [s * 0.01, 0.005] }); });
      break;
    }
  }
  ground(mb, 0, 0.04, 0.6);
}

/** A planar gate: a ring of jagged shards on a stepped dais around a dark rift edged in light. */
function planarGate(mb, r, S, R, y0) {
  mb.color = mulc(S.basaltC, 0.9);
  shard(mb, 8, R * 1.5, -0.05, y0 * 0.5, { top: 0.92, phase: TAU / 16 });
  shard(mb, 8, R * 1.3, y0 * 0.5, y0, { top: 0.94, phase: TAU / 16 });
  mb.part(() => {
    mb.translate(0, y0 + R + 0.004, 0);
    const n = 9;
    for (let k = 0; k < n; k++) {
      const a = (k / n) * TAU;
      mb.color = k % 2 ? S.obsC : mulc(S.obsC, 1.3);
      at(mb, Math.sin(a) * R, Math.cos(a) * R, 0, 0, () => {
        mb.rotateZ(-a);
        shard(mb, 4, 0.011, -0.006, 0.03 + (k % 3) * 0.012, { phase: 0.4 });
      });
    }
    // The rift: near-black with a burning rim.
    mb.color = C(HELL.portal);
    const m = 12;
    for (let k = 0; k < m; k++) {
      const a0 = (k / m) * TAU, a1 = ((k + 1) / m) * TAU;
      mb.tri([0, 0, 0], [Math.sin(a0) * R * 0.92, Math.cos(a0) * R * 0.92, 0], [Math.sin(a1) * R * 0.92, Math.cos(a1) * R * 0.92, 0]);
    }
    mb.mat = MAT.EMISSIVE;
    mb.color = C(HELL.rim);
    for (let k = 0; k < m; k++) {
      const a0 = (k / m) * TAU, a1 = ((k + 1) / m) * TAU;
      const p = (a, s) => [Math.sin(a) * R * s, Math.cos(a) * R * s, 0.001];
      mb.quad(p(a0, 0.88), p(a1, 0.88), p(a1, 0.97), p(a0, 0.97));
    }
  });
}

function infernalHall(mb, r, v) {
  const S = hellStyle(r);
  if (v === 0) {
    // A temple of the veil: a dark nave with a spined ridge, a jagged front between two spikes.
    const w = 0.1, d = 0.18;
    mb.color = mulc(S.basaltC, 0.85);
    blk(mb, w + 0.012, d + 0.012, -0.05, 0.012);
    mb.color = S.basaltC;
    shell(mb, w, d, 0.012, 0.085);
    for (const s of [1, 3]) face(mb, s, w, d, (span) => { for (let i = 0; i < 3; i++) slit(mb, -span / 2 + (span * (i + 0.5)) / 3, 0.03, 0.006, 0.035); });
    mb.part(() => {
      mb.translate(0, 0.085, 0);
      mb.rotateY(QT);
      mb.color = S.obsC;
      gableRoof(mb, d, w, 0.09, { over: 0.01, overX: 0.008, thick: 0.007, ends: S.basaltC });
      for (let i = 0; i < 5; i++) at(mb, -d / 2 + 0.02 + (i * (d - 0.04)) / 4, 0.085, 0, 0, () => shard(mb, 3, 0.008, 0, 0.03 + (i % 2) * 0.015, { phase: 0.3 }));
    });
    face(mb, 0, w, d, () => {
      mb.color = S.obsC;
      mb.tri([-0.05, 0.08, 0], [0.05, 0.08, 0], [0, 0.24, 0]);
      door(mb, S, 0, 0.026, 0.045, 0.016, 0.012, HELL.door);
      slit(mb, 0, 0.1, 0.008, 0.04);
    });
    for (const s of [-1, 1]) at(mb, s * 0.065, 0, d / 2, 0, () => { mb.color = S.obsC; shard(mb, 5, 0.02, -0.04, 0.22, { lean: [s * 0.015, 0.008], phase: 0.2 }); });
  } else if (v === 1) {
    // A bone hall: a long hide-roofed hall ribbed with giant bones, a great skull at its door.
    const w = 0.2, d = 0.1;
    mb.color = mulc(S.basaltC, 0.85);
    blk(mb, w + 0.01, d + 0.01, -0.05, 0.03);
    mb.part(() => {
      mb.translate(0, 0.03, 0);
      mb.color = J(S.hide, r, 0.05);
      gableRoof(mb, w, d, 0.1, { over: 0.012, overX: 0.008, thick: 0.006, sag: 0.012, ends: mulc(S.hide, 0.8) });
    });
    mb.color = S.boneC;
    for (let i = 0; i < 5; i++) {
      const x = -w / 2 + 0.02 + (i * (w - 0.04)) / 4;
      for (const s of [-1, 1]) mb.tube([[x, -0.03, s * 0.07], [x, 0.06, s * 0.06], [x, 0.12, s * 0.025], [x, 0.15, -s * 0.01]], [0.007, 0.006, 0.005, 0.002], 4);
    }
    mb.part(() => {
      mb.translate(w / 2 + 0.01, 0.05, 0);
      mb.color = S.boneC;
      mb.sphere(0.03, 1, { squash: 0.85 });
      mb.color = C("#120a0a");
      for (const z of [-1, 1]) at(mb, 0.025, 0.008, z * 0.011, QT, () => rect(mb, -0.006, -0.005, 0.006, 0.005));
      mb.color = S.boneC;
      for (const z of [-1, 1]) horn(mb, [0, 0.015, z * 0.02], [0, z], 0.08, 0.008, 0.9);
    });
    at(mb, 0, 0, d / 2 + 0.016, 0, () => slit(mb, 0, 0.0, 0.02, 0.035, "#c83a14"));
  } else {
    // The planar gate (the Sheaim's): a rift held in a ring of shards on a stepped dais, chained
    // pillars beside it.
    planarGate(mb, r, S, 0.065, 0.03);
    for (const s of [-1, 1]) {
      mb.color = S.obsC;
      at(mb, s * 0.12, 0, 0.0, 0, () => shard(mb, 4, 0.016, -0.04, 0.17, { lean: [s * 0.01, 0], phase: 0.4 }));
      chain(mb, [s * 0.115, 0.15, 0], [s * 0.06, 0.13, 0], 0.02);
    }
  }
  ground(mb, 0, 0.04, 0.6);
}

function infernalTower(mb, r, v) {
  const S = hellStyle(r);
  if (v === 0) {
    // A jagged spire: one great leaning obsidian shard and its lesser kin, a fissure of lava light.
    mb.color = S.basaltC;
    shard(mb, 6, 0.064, -0.05, 0.03, { top: 0.88, phase: 0.2 });
    const lean = [r.range(-0.02, 0.02), -0.015];
    mb.color = S.obsC;
    shard(mb, 5, 0.044, 0.02, 0.5, { lean, phase: 0.3 });
    spikes(mb, r, S, 3, 0.022, 0.26, 0.02, 0.04);
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      mb.color = C(HELL.glow);
      const z = 0.044 * Math.cos(TAU / 10) + 0.002;
      mb.quad([-0.003, 0.05, z], [0.003, 0.05, z], [lean[0] * 0.5 + 0.0015, 0.28, z * 0.5 + lean[1] * 0.5], [lean[0] * 0.5 - 0.0015, 0.28, z * 0.5 + lean[1] * 0.5]);
    });
  } else if (v === 1) {
    // A horned tower: a turning four-sided shaft crowned with great horns around a brazier.
    mb.color = S.basaltC;
    shard(mb, 4, 0.06, -0.05, 0.3, { top: 0.72, twist: 0.35, phase: TAU / 8 });
    at(mb, 0, 0, 0.044, 0, () => {
      door(mb, S, 0, 0.018, 0.03, 0.01, 0.0, HELL.door);
      for (let k = 0; k < 3; k++) slit(mb, r.range(-0.01, 0.01), 0.08 + k * 0.06, 0.005, 0.024);
    });
    mb.color = S.boneC;
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * TAU + TAU / 8 + 0.35;
      horn(mb, [Math.sin(a) * 0.04, 0.29, Math.cos(a) * 0.04], [Math.sin(a), Math.cos(a)], 0.12, 0.009, 0.8);
    }
    mb.part(() => {
      mb.translate(0, 0.3, 0);
      mb.mat = MAT.EMISSIVE;
      mb.color = C(HELL.glow);
      mb.sphere(0.02, 0, { squash: 0.6 });
    });
  } else {
    // A bone tower: a spine of stacked vertebrae, ribs, a skull with burning eyes.
    mb.color = mulc(S.basaltC, 0.9);
    shard(mb, 6, 0.05, -0.05, 0.02, { top: 0.9 });
    let y = 0.02;
    for (let k = 0; k < 7; k++) {
      const s = 1 - k * 0.07;
      mb.color = k % 2 ? S.boneC : mulc(S.boneC, 0.82);
      mb.lathe([[0.022 * s, y], [0.034 * s, y + 0.012], [0.03 * s, y + 0.036], [0.02 * s, y + 0.044]], 6, { caps: false });
      y += 0.044;
    }
    mb.color = S.boneC;
    for (let k = 0; k < 3; k++) for (const s of [-1, 1]) mb.tube([[0, 0.1 + k * 0.06, 0], [s * 0.05, 0.11 + k * 0.06, 0.01], [s * 0.06, 0.07 + k * 0.06, 0.03]], [0.005, 0.004, 0.0015], 3);
    mb.part(() => {
      mb.translate(0, y + 0.02, 0);
      mb.color = S.boneC;
      mb.sphere(0.03, 1, { squash: 0.9 });
      mb.mat = MAT.EMISSIVE;
      mb.color = C(HELL.glow);
      for (const s of [-1, 1]) at(mb, s * 0.011, 0.004, 0.027, 0, () => rect(mb, -0.005, -0.004, 0.005, 0.004));
    });
  }
  ground(mb, 0, 0.04, 0.6);
}

function infernalKeep(mb, r, v) {
  const S = hellStyle(r);
  // The obsidian citadel: a great central spire on a stepped octagon, lesser spires leaning out,
  // fissures of lava light, a planar rift at its front (the capital: twin spires and a crown of
  // horns).
  mb.color = mulc(S.basaltC, 0.85);
  shard(mb, 8, 0.17, -0.05, 0.03, { top: 0.93, phase: TAU / 16 });
  mb.color = S.basaltC;
  shard(mb, 8, 0.15, 0.03, 0.08, { top: 0.85, phase: TAU / 16 });
  const lean = [0.006, -0.012];
  mb.color = S.obsC;
  if (v === 1) {
    shard(mb, 6, 0.06, 0.07, 0.46, { lean: [-0.03, -0.01], phase: 0.2 });
    shard(mb, 6, 0.055, 0.07, 0.42, { lean: [0.035, -0.015], phase: 0.5 });
    at(mb, 0, 0, -0.02, 0, () => { mb.color = mulc(S.obsC, 1.2); shard(mb, 6, 0.05, 0.07, 0.36, { phase: 0.1 }); });
  } else shard(mb, 6, 0.075, 0.07, 0.52, { lean, phase: 0.2 });
  const n = v === 1 ? 7 : 5;
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + 0.6;
    const ca = Math.sin(a), sa = Math.cos(a);
    if (sa > 0.75) continue;
    const h = r.range(0.16, 0.26);
    mb.color = k % 2 ? S.obsC : mulc(S.obsC, 1.25);
    at(mb, ca * 0.11, 0, sa * 0.1, 0, () => shard(mb, 4, 0.03, 0.04, h, { lean: [ca * h * 0.3, sa * h * 0.3], phase: a }));
  }
  // Fissures and slits.
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C(HELL.glow);
    for (const s of [-1, 1]) {
      const a = s * 0.5;
      const z = 0.07;
      mb.quad([Math.sin(a) * z - 0.003, 0.1, Math.cos(a) * z], [Math.sin(a) * z + 0.003, 0.1, Math.cos(a) * z], [Math.sin(a) * 0.03 + 0.0015, 0.3, Math.cos(a) * 0.03], [Math.sin(a) * 0.03 - 0.0015, 0.3, Math.cos(a) * 0.03]);
    }
  });
  if (v === 1) {
    mb.color = S.boneC;
    for (let k = 0; k < 6; k++) {
      const a = (k / 6) * TAU;
      horn(mb, [Math.sin(a) * 0.13, 0.07, Math.cos(a) * 0.12], [Math.sin(a), Math.cos(a)], 0.1, 0.009, 0.8);
    }
  }
  // The rift at the front, chains from the spire to its pillars.
  at(mb, 0, 0.05, 0.13, 0, () => planarGate(mb, r, S, 0.045, 0.012));
  for (const s of [-1, 1]) {
    mb.color = S.obsC;
    at(mb, s * 0.17, 0, 0.08, 0, () => shard(mb, 4, 0.014, -0.04, 0.12, { lean: [s * 0.01, 0.006], phase: 0.3 }));
    chain(mb, [s * 0.165, 0.11, 0.08], [s * 0.03, 0.28, 0.02], 0.04);
  }
  for (const s of [-1, 1]) at(mb, s * 0.075, 0, 0.128, 0, () => wallBanner(mb, 0, 0.075, 0.024, 0.05));
  ground(mb, 0, 0.05, 0.6);
}

/** A basalt wall crowned with leaning obsidian spikes and hung with a chain (along x, 0.2). */
function infernalWall(mb, r) {
  const S = hellStyle(r);
  mb.color = mulc(S.basaltC, 0.85);
  blk(mb, 0.205, 0.042, -0.05, 0.045, 0.9);
  for (let i = 0; i < 5; i++) {
    const x = -0.08 + i * 0.04 + r.range(-0.006, 0.006);
    mb.color = i % 2 ? S.obsC : mulc(S.obsC, 1.25);
    at(mb, x, 0, 0.004, 0, () => shard(mb, 4, 0.014, 0.03, 0.045 + r.range(0.04, 0.075), { lean: [r.range(-0.01, 0.01), 0.018], phase: r.range(0, 1) }));
  }
  chain(mb, [-0.08, 0.075, 0.014], [0.0, 0.08, 0.014], 0.02);
  ground(mb, 0, 0.04, 0.6);
}

/** A gate like jaws: two great curved horns meeting over a burning door, spikes on its flanks. */
function infernalGate(mb, r) {
  const S = hellStyle(r);
  mb.color = mulc(S.basaltC, 0.85);
  blk(mb, 0.2, 0.05, -0.05, 0.03, 0.95);
  for (const s of [-1, 1]) {
    mb.color = S.basaltC;
    at(mb, s * 0.07, 0, 0, 0, () => shard(mb, 5, 0.035, 0.0, 0.12, { top: 0.6, phase: 0.3 }));
    mb.color = S.obsC;
    at(mb, s * 0.07, 0, 0, 0, () => shard(mb, 4, 0.025, 0.115, 0.2, { lean: [s * 0.02, 0.0], phase: 0.2 }));
    mb.color = S.boneC;
    mb.tube([[s * 0.05, 0.02, 0.02], [s * 0.052, 0.09, 0.03], [s * 0.03, 0.15, 0.03], [s * 0.004, 0.17, 0.028]], [0.013, 0.011, 0.008, 0.003], 5);
  }
  mb.part(() => {
    mb.translate(0, 0, 0.026);
    mb.color = C(HELL.door);
    mb.quad([-0.032, 0.0, 0], [0.032, 0.0, 0], [0.026, 0.09, 0], [-0.026, 0.09, 0]);
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#c8381a");
    mb.quad([-0.026, 0.0, 0.001], [0.026, 0.0, 0.001], [0.02, 0.025, 0.001], [-0.02, 0.025, 0.001]);
  });
  at(mb, 0.07, 0, 0.036, 0, () => wallBanner(mb, 0, 0.1, 0.022, 0.05));
  at(mb, -0.07, 0, 0.036, 0, () => wallBanner(mb, 0, 0.1, 0.022, 0.05));
  ground(mb, 0, 0.04, 0.6);
}

function infernalWallTower(mb, r, v) {
  const S = hellStyle(r);
  mb.color = S.basaltC;
  shard(mb, 5, 0.045, -0.05, 0.08, { top: 0.75, phase: 0.2 });
  at(mb, 0, 0, 0.037, 0, () => slit(mb, 0, 0.04, 0.006, 0.022));
  spikes(mb, r, S, v ? 3 : 2, 0.026, v ? 0.13 : 0.1, 0.075, 0.02);
  ground(mb, 0, 0.04, 0.6);
}

// --- tribal: hide tents, round huts, longhouses, palisades, totems and skulls ---------------------

const TRIBE = {
  hide: ["#7e6a52", "#74604a", "#86705a"],
  hideDark: "#5e4634",
  fur: "#6e6052",
  wood: "#5a4232",
  woodLight: "#7a5e44",
  thatch: ["#6e6242", "#665a3c", "#74684a"],
  turf: "#566036",
  bone: "#cabd9e",
  ochre: "#8a3a22",
  door: "#2a1c14",
  lit: "#ffa04a",
  glass: "#1e1612",
};

function tribeStyle(r, north) {
  return { ...TRIBE, north, hideC: J(r.pick(TRIBE.hide), r, 0.05), woodC: J(TRIBE.wood, r, 0.05), thatchC: J(r.pick(TRIBE.thatch), r, 0.05), boneC: J(TRIBE.bone, r, 0.04) };
}

/** Poles crossing out of a tent's top (n of them, from radius rr at y0 to the apex and beyond). */
function tentPoles(mb, S, n, rr, y0, top, ext = 0.03) {
  mb.color = mulc(S.woodC, 0.9);
  for (let k = 0; k < n; k++) {
    const a = (k / n) * TAU + 0.3;
    const p0 = [Math.sin(a) * rr, y0, Math.cos(a) * rr];
    const d = [-p0[0], top - y0, -p0[2]];
    const L = Math.hypot(d[0], d[1], d[2]);
    const e = 1 + ext / L;
    pole(mb, p0, [p0[0] + d[0] * e, y0 + d[1] * e, p0[2] + d[2] * e], 0.0028, 0.002, 3);
  }
}

/** A skull (a squashed ball with dark eyes), facing +z. */
function skull(mb, S, x, y, z, s = 0.012, yaw = 0) {
  at(mb, x, y, z, yaw, () => {
    mb.color = S.boneC;
    mb.sphere(s, 0, { squash: 0.9 });
    mb.color = C("#1a1210");
    for (const e of [-1, 1]) rect(mb, e * s * 0.42 - s * 0.22, -s * 0.05, e * s * 0.42 + s * 0.22, s * 0.32, s * 0.86);
  });
}

/** Crossed gable horns (carved timbers) at a ridge end (x, y, z), spreading along z. */
function gableHorns(mb, S, x, y, len = 0.035) {
  mb.color = mulc(S.woodC, 1.1);
  for (const s of [-1, 1]) mb.tube([[x, y - 0.012, -s * 0.012], [x, y + len * 0.5, s * len * 0.25], [x, y + len * 0.8, s * len * 0.55]], [0.004, 0.003, 0.0015], 3);
}

function tribalHouse(mb, r, v, sub) {
  const S = tribeStyle(r, sub === "north");
  switch (v) {
    case 0: {
      // A hide tent: a tall sagging cone of hide with a painted band and its poles crossing out
      // of the smoke hole.
      const R = r.range(0.05, 0.056), H = r.range(0.11, 0.13);
      mb.color = S.hideC;
      mb.lathe([[R * 1.06, -0.04], [R, 0.004], [R * 0.66, H * 0.36], [R * 0.3, H * 0.7], [R * 0.08, H * 0.94]], 7, { caps: false, wobble: (a, y, i) => (i === 2 ? 0.05 * Math.cos(a * 7) : 0) });
      mb.part(() => {
        mb.mat = MAT.TEAM;
        mb.color = C(TEAM_SHADE);
        mb.lathe([[R * 0.86, H * 0.2], [R * 0.74, H * 0.28]], 7, { caps: false });
      });
      tentPoles(mb, S, 4, R * 0.1, H * 0.86, H * 1.02, 0.03);
      at(mb, 0, 0, R * 0.98, 0, () => {
        mb.color = C(S.door);
        mb.tri([-0.014, 0.002, 0], [0.014, 0.002, 0], [0, 0.04, -0.012]);
      });
      break;
    }
    case 1: {
      // A round hut: a low wattle-and-daub wall under a domed thatch roof.
      const R = r.range(0.05, 0.056);
      mb.color = J("#7a6650", r, 0.05);
      drum(mb, R * 1.04, R, -0.04, 0.045, 8);
      at(mb, 0, 0, R + 0.001, 0, () => door(mb, S, 0, 0.018, 0.032, 0.0, 0.002));
      mb.part(() => {
        mb.translate(0, 0.045, 0);
        mb.color = S.thatchC;
        mb.lathe([[R * 1.32, -0.014], [R * 1.22, 0.0], [R * 0.95, 0.04], [R * 0.5, 0.072], [R * 0.12, 0.082]], 8, { caps: false });
        mb.color = mulc(S.thatchC, 0.6);
        mb.lathe([[R * 0.13, 0.08], [R * 0.1, 0.092], [0, 0.094]], 8, { caps: false });
      });
      break;
    }
    case 2: {
      // A small longhouse: low timber walls, a deep sagging thatch (turf in the north) roof,
      // carved horns crossing at its gable ends.
      const w = r.range(0.14, 0.155), d = 0.07;
      mb.color = mulc(S.woodC, 0.9);
      blk(mb, w + 0.006, d + 0.006, -0.04, 0.008);
      mb.color = S.woodC;
      shell(mb, w, d, 0.008, 0.035);
      face(mb, 1, w, d, () => door(mb, S, 0, 0.02, 0.028, 0.0, 0.008));
      mb.part(() => {
        mb.translate(0, 0.035, 0);
        mb.color = S.north ? J(S.turf, r, 0.06) : S.thatchC;
        gableRoof(mb, w, d, 0.075, { over: 0.02, overX: 0.008, thick: 0.01, sag: 0.012, kick: 0.25, ends: S.woodC });
        for (const s of [-1, 1]) gableHorns(mb, S, s * (w / 2 + 0.008), 0.075, 0.04);
      });
      break;
    }
    case 3: {
      // An A-frame hut: a thatch roof down to the ground, a hide-hung front, poles crossed at the
      // ridge.
      const w = 0.1, d = 0.085;
      mb.part(() => {
        mb.rotateY(QT);
        mb.color = S.thatchC;
        mb.translate(0, -0.01, 0);
        gableRoof(mb, w, d, 0.11, { over: 0.008, overX: 0.01, thick: 0.01, sag: 0.006, kick: 0.2, ends: S.hideC });
        for (const s of [-1, 1]) gableHorns(mb, S, s * (w / 2 + 0.01), 0.11, 0.03);
      });
      at(mb, 0, 0, w / 2 + 0.001, 0, () => {
        mb.color = C(S.door);
        mb.tri([-0.016, 0.0, 0], [0.016, 0.0, 0], [0, 0.045, 0]);
      });
      break;
    }
    case 4: {
      // A yurt: a felt drum with a dark band, a low cone of fur and hide, a smoke hole.
      const R = r.range(0.055, 0.06);
      mb.color = J("#a08a6a", r, 0.05);
      drum(mb, R, R * 0.98, -0.03, 0.045, 9);
      mb.color = J(S.hideDark, r, 0.05);
      drum(mb, R + 0.0008, R * 0.98 + 0.0008, 0.024, 0.032, 9);
      at(mb, 0, 0, R + 0.001, 0, () => door(mb, S, 0, 0.018, 0.03, 0.0, 0.0, "#6a2a1a"));
      mb.part(() => {
        mb.translate(0, 0.045, 0);
        mb.color = J(S.fur, r, 0.05);
        mb.lathe([[R * 1.1, -0.006], [R * 0.6, 0.03], [R * 0.16, 0.048]], 9, { caps: false });
        mb.color = mulc(S.woodC, 0.8);
        mb.lathe([[R * 0.16, 0.048], [R * 0.13, 0.054]], 9, { caps: false });
      });
      break;
    }
    default: {
      // A hut on short stilts, its posts hung with skulls.
      const y = 0.03;
      mb.color = S.woodC;
      for (const [x, z] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) pole(mb, [x * 0.034, -0.04, z * 0.03], [x * 0.032, y + 0.002, z * 0.028], 0.005, 0.004, 4);
      mb.part(() => {
        mb.translate(0, y, 0);
        mb.color = mulc(S.woodC, 1.1);
        blk(mb, 0.085, 0.075, -0.004, 0.004);
        mb.color = J("#7a6650", r, 0.05);
        shell(mb, 0.07, 0.06, 0.004, 0.045);
        face(mb, 0, 0.07, 0.06, () => door(mb, S, 0, 0.016, 0.03, 0.0, 0.004));
        mb.translate(0, 0.045, 0);
        mb.color = S.thatchC;
        hipRoof(mb, 0.07, 0.06, 0.06, { over: 0.016, ridge: 0.02, thick: 0.008 });
      });
      for (const s of [-1, 1]) {
        mb.color = S.woodC;
        pole(mb, [s * 0.06, -0.03, 0.05], [s * 0.06, 0.09, 0.05], 0.003, 0.0025, 3);
        skull(mb, S, s * 0.06, 0.095, 0.05, 0.009);
      }
      break;
    }
  }
  ground(mb, 0, 0.04, 0.6);
}

function tribalHall(mb, r, v, sub) {
  const S = tribeStyle(r, sub === "north");
  if (v === 0) {
    // A great longhouse, its horned gable to the front: a long sagging roof nearly to the ground,
    // a skull over the door.
    const w = 0.1, d = 0.24;
    mb.color = mulc(S.woodC, 0.9);
    blk(mb, w + 0.008, d + 0.008, -0.04, 0.008);
    mb.color = S.woodC;
    shell(mb, w, d, 0.008, 0.04);
    face(mb, 0, w, d, () => door(mb, S, 0, 0.024, 0.032, 0.0, 0.008));
    for (const sd of [1, 3]) face(mb, sd, w, d, (span) => { for (let i = 0; i < 3; i++) win(mb, S, -span / 3 + (i * span) / 3, 0.018, 0.008, 0.008, r() < 0.4); });
    mb.part(() => {
      mb.translate(0, 0.04, 0);
      mb.rotateY(QT);
      mb.color = S.north ? J(S.turf, r, 0.06) : S.thatchC;
      gableRoof(mb, d, w, 0.11, { over: 0.022, overX: 0.012, thick: 0.012, sag: 0.018, kick: 0.25, ends: S.woodC });
      for (const s of [-1, 1]) gableHorns(mb, S, s * (d / 2 + 0.012), 0.11, 0.055);
    });
    skull(mb, S, 0.0, 0.085, d / 2 + 0.014, 0.011);
  } else if (v === 1) {
    // A chieftain's roundhouse: a great drum under a tall cone of thatch, a totem at its door.
    const R = 0.085;
    mb.color = J("#7a6650", r, 0.05);
    drum(mb, R * 1.04, R, -0.04, 0.05, 11);
    at(mb, 0, 0, R + 0.001, 0, () => door(mb, S, 0, 0.024, 0.036, 0.0, 0.002));
    mb.part(() => {
      mb.translate(0, 0.05, 0);
      mb.color = S.thatchC;
      mb.lathe([[R * 1.3, -0.016], [R * 1.2, 0.0], [R * 0.8, 0.05], [R * 0.36, 0.11], [R * 0.08, 0.15]], 11, { caps: false });
      tentPoles(mb, S, 4, R * 0.08, 0.135, 0.16, 0.02);
    });
    at(mb, 0.06, 0, 0.11, 0, () => totem(mb, r, 0.75));
  } else {
    // A shaman's lodge: a hide tent hung with bones before a great skull on a frame.
    mb.color = S.hideC;
    mb.lathe([[0.07, -0.02], [0.066, 0.004], [0.04, 0.06], [0.016, 0.12], [0.004, 0.14]], 8, { caps: false });
    tentPoles(mb, S, 5, 0.008, 0.12, 0.16, 0.03);
    mb.color = S.boneC;
    for (let k = 0; k < 5; k++) {
      const a = (k / 5) * TAU + 0.2;
      pole(mb, [Math.sin(a) * 0.05, 0.035, Math.cos(a) * 0.05], [Math.sin(a) * 0.052, 0.008, Math.cos(a) * 0.052], 0.0025, 0.0015, 3);
    }
    mb.part(() => {
      mb.translate(0.0, 0, 0.1);
      mb.color = S.woodC;
      for (const s of [-1, 1]) pole(mb, [s * 0.03, -0.03, 0], [s * 0.026, 0.1, 0], 0.004, 0.003, 4);
      pole(mb, [-0.04, 0.095, 0], [0.04, 0.095, 0], 0.003, 0.003, 4);
      mb.color = S.boneC;
      at(mb, 0, 0.07, 0, 0, () => mb.sphere(0.022, 1, { squash: 0.85 }));
      mb.color = C("#1a1210");
      for (const e of [-1, 1]) at(mb, e * 0.008, 0.074, 0.019, 0, () => rect(mb, -0.004, -0.003, 0.004, 0.004));
      mb.color = S.boneC;
      for (const s of [-1, 1]) horn(mb, [s * 0.016, 0.08, 0.0], [s, 0], 0.07, 0.006, 0.9);
    });
  }
  ground(mb, 0, 0.04, 0.6);
}

function tribalTower(mb, r, v, sub) {
  const S = tribeStyle(r, sub === "north");
  if (v === 0) {
    // A log watchtower: four splayed legs, cross-braces, a railed platform under a hide cone.
    const H = 0.27;
    mb.color = S.woodC;
    for (const [x, z] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) pole(mb, [x * 0.05, -0.04, z * 0.05], [x * 0.032, H, z * 0.032], 0.006, 0.005, 4);
    mb.color = mulc(S.woodC, 0.85);
    for (const [y0, y1] of [[0.03, 0.14], [0.14, 0.24]]) for (const s of [-1, 1]) pole(mb, [-0.045, y0, s * 0.044], [0.04, y1, s * 0.037], 0.0025, 0.0025, 3);
    mb.part(() => {
      mb.translate(0, H, 0);
      mb.color = mulc(S.woodC, 1.1);
      blk(mb, 0.085, 0.085, -0.006, 0.002);
      mb.color = S.woodC;
      shell(mb, 0.084, 0.084, 0.002, 0.03);
      face(mb, 0, 0.084, 0.084, () => {
        mb.mat = MAT.TEAM;
        mb.color = C(TEAM_SHADE);
        rect(mb, -0.03, 0.008, 0.03, 0.022);
      });
      mb.color = S.hideC;
      for (const [x, z] of [[-1, -1], [1, 1]]) pole(mb, [x * 0.04, 0.03, z * 0.04], [x * 0.04, 0.07, z * 0.04], 0.003, 0.003, 3);
      mb.translate(0, 0.07, 0);
      hipRoof(mb, 0.08, 0.08, 0.06, { over: 0.012, thick: 0.004 });
      mb.translate(0, 0.06, 0);
      pennant(mb, 0.0, 0.04, 0.016);
    });
  } else if (v === 1) {
    // A totem tower: a great carved pole of stacked faces, horns and skulls.
    mb.part(() => {
      mb.scale(1.7, 2.1, 1.7);
      totem(mb, r, 1);
    });
  } else {
    // A tusk arch-tower: four great tusks curving up around a pole crowned with a skull.
    mb.color = S.woodC;
    pole(mb, [0, -0.04, 0], [0, 0.3, 0], 0.012, 0.008, 6);
    mb.color = S.boneC;
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * TAU + TAU / 8;
      horn(mb, [Math.sin(a) * 0.06, -0.02, Math.cos(a) * 0.06], [-Math.sin(a), -Math.cos(a)], 0.24, 0.012, 0.45);
    }
    skull(mb, S, 0, 0.31, 0.0, 0.02);
    mb.color = S.boneC;
    for (const s of [-1, 1]) horn(mb, [s * 0.016, 0.32, 0], [s, 0], 0.06, 0.006, 0.9);
  }
  ground(mb, 0, 0.04, 0.6);
}

function tribalKeep(mb, r, v, sub) {
  const S = tribeStyle(r, sub === "north");
  // The chieftain's great hall: a huge longhouse on a low earth platform, its roof sweeping to the
  // ground, a horned skull over the door, totems and a war-pole; the capital's on a palisaded mound.
  const w = 0.16, d = 0.3;
  mb.color = J("#5a4632", r, 0.05);
  blk(mb, w + 0.05, d + 0.04, -0.05, 0.012, 0.95);
  mb.color = S.woodC;
  shell(mb, w, d, 0.012, 0.06);
  for (const sd of [1, 3])
    face(mb, sd, w, d, (span) => {
      mb.color = mulc(S.woodC, 0.75);
      for (let i = 0; i < 9; i++) rect(mb, -span / 2 + 0.01 + (i * (span - 0.02)) / 8 - 0.002, 0.012, -span / 2 + 0.01 + (i * (span - 0.02)) / 8 + 0.002, 0.06, 0.0004);
    });
  face(mb, 0, w, d, () => door(mb, S, 0, 0.03, 0.045, 0.0, 0.012));
  mb.part(() => {
    mb.translate(0, 0.06, 0);
    mb.rotateY(QT);
    mb.color = S.north ? J(S.turf, r, 0.06) : S.thatchC;
    gableRoof(mb, d, w, 0.17, { over: 0.032, overX: 0.014, thick: 0.014, sag: 0.025, kick: 0.25, ends: S.woodC });
    for (const s of [-1, 1]) gableHorns(mb, S, s * (d / 2 + 0.014), 0.17, 0.075);
    // Smoke-hole hoods along the ridge.
    mb.color = mulc(S.thatchC, 0.7);
    for (const x of [-0.06, 0.06]) at(mb, x, 0.145, 0, 0, () => blk(mb, 0.03, 0.03, 0, 0.012, 0.6));
  });
  // Porch with the horned skull.
  mb.part(() => {
    mb.translate(0, 0, d / 2 + 0.035);
    mb.color = S.woodC;
    for (const s of [-1, 1]) pole(mb, [s * 0.03, -0.03, 0], [s * 0.028, 0.08, 0], 0.005, 0.004, 4);
    mb.color = mulc(S.thatchC, 0.9);
    mb.quad([-0.045, 0.085, -0.04], [0.045, 0.085, -0.04], [0.045, 0.07, 0.012], [-0.045, 0.07, 0.012]);
    mb.color = S.boneC;
    at(mb, 0, 0.1, -0.03, 0, () => mb.sphere(0.024, 1, { squash: 0.85 }));
    for (const s of [-1, 1]) horn(mb, [s * 0.02, 0.11, -0.03], [s, 0], 0.09, 0.008, 0.9);
    mb.color = C("#1a1210");
    for (const e of [-1, 1]) at(mb, e * 0.009, 0.103, -0.008, 0, () => rect(mb, -0.004, -0.003, 0.004, 0.004));
  });
  for (const s of [-1, 1]) at(mb, s * 0.085, 0, d / 2 + 0.03, 0, () => totem(mb, r, 0.85));
  // A war-pole with a banner and smoke-hole poles on the ridge.
  at(mb, -w / 2 - 0.03, 0, d / 2 - 0.05, 0, () => bannerPole(mb, r, 1, "tribal"));
  if (v === 1) {
    mb.color = S.woodC;
    for (let k = 0; k < 14; k++) {
      const a = Math.PI * 0.22 + (k / 13) * Math.PI * 1.56;
      at(mb, Math.sin(a) * 0.21, 0, Math.cos(a) * 0.17 - 0.01, 0, () => mb.lathe([[0.008, -0.04], [0.007, 0.07 + (k % 2) * 0.01], [0, 0.09 + (k % 2) * 0.01]], 4, { caps: false }));
    }
  }
  ground(mb, 0, 0.05, 0.6);
}

/** A palisade of sharpened logs bound with a rail (along x, length 0.2). */
function tribalWall(mb, r) {
  const S = tribeStyle(r);
  const n = 9;
  for (let i = 0; i < n; i++) {
    const x = -0.1 + (0.2 * (i + 0.5)) / n;
    const h = 0.075 + r.range(-0.01, 0.012);
    mb.color = J(i % 2 ? "#5a4232" : "#664c38", r, 0.06);
    at(mb, x, 0, r.range(-0.003, 0.003), r.range(0, 1), () => mb.lathe([[0.0115, -0.05], [0.011, h], [0, h + 0.022]], 5, { caps: false }));
  }
  mb.color = mulc(S.woodC, 0.75);
  for (const z of [-1, 1]) at(mb, 0, 0.042, z * 0.0105, 0, () => blk(mb, 0.205, 0.004, 0, 0.007));
  ground(mb, 0, 0.04, 0.6);
}

/** A palisade gate: two tall log watch-posts with platforms, a lintel with horns and skulls. */
function tribalGate(mb, r) {
  const S = tribeStyle(r);
  for (const s of [-1, 1]) {
    for (let i = 0; i < 3; i++) {
      const x = s * (0.05 + i * 0.021);
      const h = 0.11 - i * 0.012;
      mb.color = J(i % 2 ? "#5a4232" : "#664c38", r, 0.06);
      at(mb, x, 0, 0, 0, () => mb.lathe([[0.012, -0.05], [0.011, h], [0, h + 0.022]], 5, { caps: false }));
    }
    mb.part(() => {
      mb.translate(s * 0.07, 0.11, 0);
      mb.color = mulc(S.woodC, 1.1);
      blk(mb, 0.06, 0.04, 0, 0.006);
      mb.color = S.hideC;
      mb.translate(0, 0.04, 0);
      hipRoof(mb, 0.05, 0.04, 0.035, { over: 0.01, thick: 0.004 });
    });
    at(mb, s * 0.07, 0, 0.022, 0, () => wallBanner(mb, 0, 0.1, 0.024, 0.05));
  }
  mb.color = mulc(S.woodC, 0.85);
  pole(mb, [-0.06, 0.1, 0.014], [0.06, 0.1, 0.014], 0.006, 0.006, 4);
  skull(mb, S, 0, 0.115, 0.016, 0.014);
  mb.color = S.boneC;
  for (const s of [-1, 1]) horn(mb, [s * 0.012, 0.12, 0.016], [s, 0], 0.05, 0.006, 0.9);
  ground(mb, 0, 0.04, 0.6);
}

function tribalWallTower(mb, r, v) {
  const S = tribeStyle(r);
  const H = v ? 0.13 : 0.1;
  mb.color = S.woodC;
  for (const [x, z] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) pole(mb, [x * 0.03, -0.04, z * 0.03], [x * 0.024, H, z * 0.024], 0.005, 0.004, 4);
  mb.part(() => {
    mb.translate(0, H, 0);
    mb.color = mulc(S.woodC, 1.1);
    blk(mb, 0.06, 0.06, -0.004, 0.02);
    mb.color = S.hideC;
    mb.translate(0, 0.045, 0);
    hipRoof(mb, 0.055, 0.055, 0.04, { over: 0.008, thick: 0.004 });
  });
  mb.color = S.hideC;
  for (const [x, z] of [[-1, -1], [1, 1]]) pole(mb, [x * 0.026, H + 0.02, z * 0.026], [x * 0.026, H + 0.045, z * 0.026], 0.0025, 0.0025, 3);
  if (v) skull(mb, S, 0, H - 0.015, 0.03, 0.01);
  ground(mb, 0, 0.04, 0.6);
}

// --- desert: sandstone cubes, flat roofs, domes and minarets (the Malakim) ------------------------

const SAND = {
  wall: ["#c9b58e", "#d2c09c", "#bfa982", "#d6c6a4"],
  stone: "#a88e68",
  dark: "#86704f",
  dome: [["#d8ccb4", 2], ["#3e7a72", 1.5], ["#b8923e", 1]],
  wood: "#6a4a30",
  lit: "#ffc070",
  glass: "#2a2018",
  door: "#4a3020",
};

function sandStyle(r) {
  return { ...SAND, wallC: J(r.pick(SAND.wall), r, 0.035), stoneC: J(SAND.stone, r, 0.04), domeC: J(wpick(r, SAND.dome), r, 0.04) };
}

/** A flat-roofed sandstone cube with a parapet, arched dark windows and a door; returns its top. */
function sandCube(mb, r, S, w, d, h, { door: hasDoor = true, doorSide = 0, y0 = 0, base = true } = {}) {
  mb.part(() => {
    mb.translate(0, y0, 0);
    if (base) {
      mb.color = mulc(S.stoneC, 0.9);
      blk(mb, w + 0.006, d + 0.006, -0.05, 0.008);
    }
    mb.color = S.wallC;
    shell(mb, w, d, y0 ? 0 : 0.008, h);
    mb.color = mulc(S.wallC, 0.93);
    mb.quad([-w / 2, h, -d / 2], [-w / 2, h, d / 2], [w / 2, h, d / 2], [w / 2, h, -d / 2]);
    mb.color = mulc(S.wallC, 1.04);
    shell(mb, w + 0.004, d + 0.004, h, h + 0.009);
    for (let s = 0; s < 4; s++)
      face(mb, s, w, d, (span) => {
        const k = Math.max(1, Math.floor(span / 0.045));
        for (let i = 0; i < k; i++) {
          const x = -span / 2 + (span * (i + 0.5)) / k;
          if (hasDoor && s === doorSide && i === Math.floor(k / 2)) door(mb, S, x, 0.018, Math.min(0.032, h * 0.55), 0.009, 0.008);
          else win(mb, S, x, h * 0.45, 0.01, Math.min(0.016, h * 0.25), r() < 0.3, 0.006);
        }
      });
  });
  return y0 + h;
}

/** A dome on a short drum at (x, y, z). */
function sandDome(mb, r, S, x, y, z, R, H, c = S.domeC, seg = 10) {
  mb.part(() => {
    mb.translate(x, y, z);
    mb.color = S.wallC;
    mb.lathe([[R * 1.02, -0.002], [R, R * 0.35]], seg, { caps: false });
    mb.translate(0, R * 0.35, 0);
    mb.color = c;
    if (c === SAND.dome[2][0] || C(c)[0] > 0.3) mb.mat = MAT.METAL;
    mb.lathe([[R * 1.04, -0.002], [R * 1.02, H * 0.3], [R * 0.82, H * 0.62], [R * 0.45, H * 0.88], [0, H]], seg, { caps: false });
    mb.translate(0, H, 0);
    finial(mb, 0, 0.014, "#c9a043", 0.0025);
  });
}

/** A striped awning in the owner's colour on poles, over a roof or a door. */
function awning(mb, x, y, z, w, d, yaw = 0) {
  mb.part(() => {
    mb.translate(x, y, z);
    if (yaw) mb.rotateY(yaw);
    mb.color = C("#5a4030");
    for (const [px, pz] of [[-1, 1], [1, 1]]) pole(mb, [(px * w) / 2, 0, (pz * d) / 2], [(px * w) / 2, 0.03, (pz * d) / 2], 0.0018, 0.0018, 3);
    const m0 = mb.mark();
    const n = 4;
    for (let i = 0; i < n; i++) {
      const x0 = -w / 2 + (w * i) / n, x1 = -w / 2 + (w * (i + 1)) / n;
      mb.mat = i % 2 ? MAT.MATTE : MAT.TEAM;
      mb.color = C(i % 2 ? "#d8ccb0" : TEAM);
      mb.quad([x0, 0.036, -d / 2], [x1, 0.036, -d / 2], [x1, 0.03, d / 2], [x0, 0.03, d / 2]);
    }
    mb.mat = MAT.MATTE;
    clothSway(mb, m0, mb.xp([0, 0.036, -d / 2]), d * 1.5, 0.12);
  });
}

function desertHouse(mb, r, v) {
  const S = sandStyle(r);
  switch (v) {
    case 0: {
      const w = r.range(0.095, 0.11), d = r.range(0.085, 0.095);
      const h = sandCube(mb, r, S, w, d, r.range(0.065, 0.075));
      awning(mb, -w * 0.15, h, -d * 0.1, w * 0.55, d * 0.55);
      break;
    }
    case 1: {
      const h = sandCube(mb, r, S, 0.115, 0.1, 0.05);
      mb.part(() => {
        mb.translate(0.015, 0, -0.015);
        sandCube(mb, r, S, 0.07, 0.065, 0.045, { y0: h, door: false, base: false });
      });
      break;
    }
    case 2: {
      const h = sandCube(mb, r, S, 0.1, 0.1, 0.065);
      sandDome(mb, r, S, 0, h, 0, 0.032, 0.04);
      break;
    }
    case 3: {
      // A house with a wind tower.
      const h = sandCube(mb, r, S, 0.11, 0.09, 0.06);
      mb.part(() => {
        mb.translate(-0.03, h, -0.02);
        mb.color = S.wallC;
        blk(mb, 0.028, 0.028, 0, 0.06);
        for (let s = 0; s < 4; s++) face(mb, s, 0.028, 0.028, () => { mb.color = C("#3a2c20"); rect(mb, -0.008, 0.035, 0.008, 0.054); });
      });
      break;
    }
    case 4: {
      // An L of two cubes, the taller with an outside stair.
      sandCube(mb, r, S, 0.08, 0.08, 0.085);
      mb.part(() => {
        mb.translate(0.062, 0, 0.02);
        sandCube(mb, r, S, 0.06, 0.07, 0.05, { doorSide: 1 });
      });
      mb.color = mulc(S.wallC, 0.95);
      for (let k = 0; k < 4; k++) at(mb, -0.02 + k * 0.012, 0, 0.047, 0, () => blk(mb, 0.012, 0.012, -0.02, 0.015 + k * 0.016));
      break;
    }
    default: {
      // A house with an arcaded front loggia.
      const w = 0.12, d = 0.085;
      const h = sandCube(mb, r, S, w, d, 0.07, { door: false });
      face(mb, 0, w, d, (span) => {
        mb.color = C("#3a2c20");
        for (let i = 0; i < 3; i++) {
          const x = -span / 3 + (span * i) / 3;
          rect(mb, x - 0.012, 0.008, x + 0.012, 0.035, 0.0006);
          mb.tri([x - 0.012, 0.035, 0.0006], [x + 0.012, 0.035, 0.0006], [x, 0.047, 0.0006]);
        }
      });
      awning(mb, 0.02, h, 0.0, w * 0.5, d * 0.6);
      break;
    }
  }
  ground(mb, 0, 0.04, 0.6);
}

/** A minaret: a slender shaft, a balcony, a lantern stage and an onion cap. */
function minaret(mb, r, S, H, R = 0.022) {
  mb.color = mulc(S.stoneC, 0.9);
  shard(mb, 8, R * 1.5, -0.05, 0.02, { top: 0.9, phase: TAU / 16 });
  mb.color = S.wallC;
  mb.lathe([[R * 1.2, 0.02], [R, H * 0.7]], 8, { caps: false });
  mb.color = mulc(S.wallC, 1.05);
  mb.lathe([[R, H * 0.7], [R * 1.7, H * 0.73], [R * 1.7, H * 0.75], [R * 0.9, H * 0.76]], 8, { caps: false });
  mb.color = S.wallC;
  mb.lathe([[R * 0.9, H * 0.76], [R * 0.85, H * 0.88]], 8, { caps: false });
  mb.part(() => {
    mb.mat = MAT.EMISSIVE;
    mb.color = C(S.lit);
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * TAU + 0.4;
      at(mb, Math.sin(a) * R * 0.87, 0, Math.cos(a) * R * 0.87, a, () => rect(mb, -0.003, H * 0.79, 0.003, H * 0.85));
    }
  });
  mb.part(() => {
    mb.translate(0, H * 0.88, 0);
    mb.color = S.domeC;
    onion(mb, R * 0.95, H * 0.14, 8);
    mb.translate(0, H * 0.14, 0);
    finial(mb, 0, 0.016, "#c9a043", 0.0025);
  });
}

function desertTower(mb, r, v) {
  const S = sandStyle(r);
  if (v === 0) minaret(mb, r, S, 0.46);
  else if (v === 1) {
    const h = sandCube(mb, r, S, 0.085, 0.085, 0.28, { door: true });
    mb.color = S.wallC;
    merlonsRect(mb, 0.09, 0.09, h + 0.009, { mw: 0.016, mh: 0.016, md: 0.01 });
    face(mb, 0, 0.085, 0.085, () => wallBanner(mb, 0, 0.25, 0.026, 0.06));
  } else {
    mb.color = mulc(S.stoneC, 0.9);
    drum(mb, 0.052, 0.05, -0.05, 0.012, 10);
    mb.color = S.wallC;
    drum(mb, 0.046, 0.042, 0.012, 0.26, 10);
    for (let k = 0; k < 3; k++) {
      const a = r.range(-0.8, 0.8);
      at(mb, Math.sin(a) * 0.045, 0, Math.cos(a) * 0.045, a, () => win(mb, S, 0, 0.08 + k * 0.06, 0.008, 0.016, r() < 0.4, 0.006));
    }
    sandDome(mb, r, S, 0, 0.26, 0, 0.044, 0.06);
  }
  ground(mb, 0, 0.04, 0.6);
}

function desertHall(mb, r, v) {
  const S = sandStyle(r);
  if (v === 0) {
    // A domed hall with corner turrets.
    const h = sandCube(mb, r, S, 0.15, 0.15, 0.08);
    sandDome(mb, r, S, 0, h, 0, 0.055, 0.07);
    for (const [x, z] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) sandDome(mb, r, S, x * 0.065, h, z * 0.065, 0.013, 0.016, S.wallC, 4);
  } else if (v === 1) {
    // A caravanserai: four low ranges round an open court, an arched gate.
    for (let s = 0; s < 4; s++)
      mb.part(() => {
        mb.rotateY(s * QT);
        mb.translate(0, 0, 0.07);
        sandCube(mb, r, S, 0.18, 0.04, 0.055, { door: s === 0 });
      });
    awning(mb, 0, 0.0, 0.0, 0.05, 0.04);
  } else {
    // A temple: a portico of columns before a domed sanctuary, a minaret at its side.
    const h = sandCube(mb, r, S, 0.12, 0.11, 0.085, { door: false });
    sandDome(mb, r, S, 0, h, 0, 0.045, 0.06);
    mb.part(() => {
      mb.translate(0, 0, 0.075);
      mb.color = mulc(S.wallC, 1.05);
      for (let i = 0; i < 4; i++) at(mb, -0.045 + i * 0.03, 0, 0, 0, () => mb.cylinder(0.006, 0.005, 0.07, 6, { caps: false }));
      blk(mb, 0.11, 0.035, 0.07, 0.082);
      blk(mb, 0.11, 0.04, -0.04, 0.008);
    });
    at(mb, 0.085, 0, -0.03, 0, () => minaret(mb, r, S, 0.28, 0.016));
  }
  ground(mb, 0, 0.04, 0.6);
}

function desertKeep(mb, r, v) {
  const S = sandStyle(r);
  if (v === 0) {
    // A palace: a broad terraced block, a great dome on a high drum, minarets at the corners.
    const h = sandCube(mb, r, S, 0.24, 0.2, 0.1, { door: false });
    face(mb, 0, 0.24, 0.2, () => {
      mb.color = C("#2a2018");
      rect(mb, -0.022, 0.008, 0.022, 0.06, 0.0006);
      mb.tri([-0.022, 0.06, 0.0006], [0.022, 0.06, 0.0006], [0, 0.085, 0.0006]);
      wallBanner(mb, -0.06, 0.092, 0.026, 0.06);
      wallBanner(mb, 0.06, 0.092, 0.026, 0.06);
    });
    mb.part(() => {
      mb.translate(0, h, -0.01);
      sandCube(mb, r, S, 0.14, 0.12, 0.05, { door: false, base: false });
    });
    sandDome(mb, r, S, 0, h + 0.059, -0.01, 0.06, 0.1, J("#b8923e", r, 0.04), 12);
    for (const [x, z] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) at(mb, x * 0.13, 0, z * 0.11, 0, () => minaret(mb, r, S, 0.34, 0.016));
  } else {
    // A sun temple: a stepped pyramid with a gilded sun disc on its summit.
    for (let k = 0; k < 4; k++) {
      mb.color = k % 2 ? S.wallC : mulc(S.wallC, 0.94);
      blk(mb, 0.26 - k * 0.055, 0.24 - k * 0.05, k ? 0.05 * k : -0.05, 0.05 * (k + 1));
    }
    mb.color = mulc(S.wallC, 0.9);
    at(mb, 0, 0, 0.12, 0, () => {
      for (let k = 0; k < 8; k++) blk(mb, 0.04, 0.03, -0.04, 0.025 * k * 0.8 + 0.02, 1);
    });
    mb.part(() => {
      mb.translate(0, 0.2, 0);
      mb.color = S.wallC;
      blk(mb, 0.05, 0.05, 0, 0.04);
      mb.translate(0, 0.075, 0);
      mb.mat = MAT.METAL;
      mb.color = C("#c9a043");
      const R = 0.032;
      for (let k = 0; k < 12; k++) {
        const a0 = (k / 12) * TAU, a1 = ((k + 1) / 12) * TAU;
        mb.tri([0, 0, 0], [Math.cos(a0) * R, Math.sin(a0) * R, 0], [Math.cos(a1) * R, Math.sin(a1) * R, 0]);
        const am = (a0 + a1) / 2;
        if (k % 2 === 0) mb.tri([Math.cos(a0) * R, Math.sin(a0) * R, 0], [Math.cos(am) * R * 1.45, Math.sin(am) * R * 1.45, 0], [Math.cos(a1) * R, Math.sin(a1) * R, 0]);
      }
    });
  }
  ground(mb, 0, 0.05, 0.6);
}

function desertWall(mb, r) {
  const S = sandStyle(r);
  mb.color = mulc(S.stoneC, 0.9);
  blk(mb, 0.205, 0.042, -0.05, 0.012, 0.97);
  mb.color = S.wallC;
  shell(mb, 0.205, 0.034, 0.012, 0.07);
  mb.color = mulc(S.wallC, 0.85);
  mb.quad([-0.1025, 0.07, -0.017], [-0.1025, 0.07, 0.017], [0.1025, 0.07, 0.017], [0.1025, 0.07, -0.017]);
  mb.color = S.wallC;
  for (let i = 0; i < 5; i++) at(mb, -0.08 + i * 0.04, 0, 0.013, 0, () => mb.lathe([[0.009, 0.07], [0.009, 0.08], [0.004, 0.088], [0, 0.09]], 4, { caps: false, phase: TAU / 8 }));
  ground(mb, 0, 0.04, 0.6);
}

function desertGate(mb, r) {
  const S = sandStyle(r);
  sandCube(mb, r, S, 0.1, 0.05, 0.11, { door: false });
  for (const side of [0, 2])
    face(mb, side, 0.1, 0.05, () => {
      mb.color = C("#241a12");
      rect(mb, -0.022, 0.008, 0.022, 0.06, 0.0006);
      for (let k = 0; k < 6; k++) {
        const a0 = (k / 6) * Math.PI, a1 = ((k + 1) / 6) * Math.PI;
        mb.tri([0, 0.06, 0.0006], [Math.cos(a0) * 0.026, 0.06 + Math.sin(a0) * 0.026, 0.0006], [Math.cos(a1) * 0.026, 0.06 + Math.sin(a1) * 0.026, 0.0006]);
      }
    });
  for (const s of [-1, 1]) {
    mb.part(() => {
      mb.translate(s * 0.072, 0, 0.004);
      mb.color = S.wallC;
      drum(mb, 0.032, 0.03, -0.05, 0.15, 9);
      face(mb, 0, 0.0, 0.062, () => wallBanner(mb, 0, 0.13, 0.022, 0.05));
    });
    sandDome(mb, r, S, s * 0.072, 0.15, 0.004, 0.03, 0.04);
  }
  ground(mb, 0, 0.04, 0.6);
}

function desertWallTower(mb, r, v) {
  const S = sandStyle(r);
  mb.color = S.wallC;
  drum(mb, 0.042, 0.04, -0.05, 0.11, 9);
  if (v) sandDome(mb, r, S, 0, 0.11, 0, 0.04, 0.045);
  else {
    mb.color = mulc(S.wallC, 0.85);
    mb.lathe([[0.04, 0.11], [0, 0.1101]], 9, { caps: false });
    mb.color = S.wallC;
    for (let k = 0; k < 6; k++) {
      const a = (k / 6) * TAU;
      at(mb, Math.sin(a) * 0.034, 0, Math.cos(a) * 0.034, a, () => mb.lathe([[0.007, 0.11], [0.007, 0.12], [0, 0.128]], 4, { caps: false, phase: TAU / 8 }));
    }
  }
  ground(mb, 0, 0.04, 0.6);
}

// --- coastal: timber and planks on stilts, ropes, sails (the Lanun) -----------------------------------

const COAST = {
  plank: ["#7a5a3c", "#6e5236", "#84664a"],
  tar: "#3e2c20",
  weather: "#8a7a68",
  roof: [["#5e4a38", 2], ["#6a3a2e", 1.5], ["#6a6050", 1.2]],
  sail: "#c8b898",
  stone: "#7d766e",
  lit: "#ffb860",
  glass: "#1e1814",
  door: "#2e2018",
};

function coastStyle(r) {
  return { ...COAST, plankC: J(r.pick(COAST.plank), r, 0.05), roofC: J(wpick(r, COAST.roof), r, 0.05) };
}

/** Stilts under a w x d floor at height y (posts reach into the mud or water). */
function stilts(mb, S, w, d, y) {
  mb.color = C(S.tar);
  for (const [x, z] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) pole(mb, [(x * w) / 2, -0.07, (z * d) / 2], [(x * w) / 2, y, (z * d) / 2], 0.0045, 0.004, 4);
}

/** A plank house body: boards (darker seams), a door, small windows; returns the wall top. */
function plankBody(mb, r, S, w, d, h, y0 = 0, { door: hasDoor = true } = {}) {
  mb.part(() => {
    mb.translate(0, y0, 0);
    mb.color = mulc(S.plankC, 0.85);
    blk(mb, w + 0.012, d + 0.012, y0 > 0.001 ? -0.004 : -0.05, 0.004);
    mb.color = S.plankC;
    shell(mb, w, d, 0.004, h);
    for (let s = 0; s < 4; s++)
      face(mb, s, w, d, (span) => {
        mb.color = mulc(S.plankC, 0.7);
        for (let i = 1; i < 4; i++) rect(mb, -span / 2 + (span * i) / 4 - 0.001, 0.004, -span / 2 + (span * i) / 4 + 0.001, h, 0.0003);
        if (hasDoor && s === 0) door(mb, S, 0, 0.017, Math.min(0.032, h * 0.6), 0, 0.004);
        else win(mb, S, 0, h * 0.45, 0.011, 0.012, r() < 0.35);
      });
  });
  return y0 + h;
}

function coastalHouse(mb, r, v) {
  const S = coastStyle(r);
  switch (v) {
    case 0:
    case 3: {
      // A plank house on stilts under a plank gable (v3 two storeys with a gallery).
      const w = r.range(0.1, 0.115), d = 0.08, y = 0.03;
      stilts(mb, S, w, d, y);
      let top = plankBody(mb, r, S, w, d, 0.05, y);
      if (v === 3) {
        mb.color = mulc(S.plankC, 0.9);
        at(mb, 0, top, 0.012, 0, () => blk(mb, w + 0.02, d + 0.03, -0.004, 0.003));
        top = plankBody(mb, r, S, w, d, 0.04, top, { door: false });
      }
      mb.part(() => {
        mb.translate(0, top, 0);
        mb.color = S.roofC;
        gableRoof(mb, w, d, 0.06, { over: 0.014, overX: 0.01, thick: 0.006, sag: 0.006, ends: S.plankC });
      });
      break;
    }
    case 1: {
      // A shack with a sail stretched as an awning, barrels by the door.
      const top = plankBody(mb, r, S, 0.09, 0.075, 0.055);
      mb.part(() => {
        mb.translate(0, top, 0);
        mb.color = S.roofC;
        mb.quad([-0.05, 0.03, -0.045], [0.05, 0.03, -0.045], [0.05, -0.004, 0.045], [-0.05, -0.004, 0.045]);
      });
      const m0 = mb.mark();
      mb.color = C(S.sail);
      mb.quad([-0.045, 0.05, 0.04], [0.045, 0.05, 0.04], [0.05, 0.04, 0.085], [-0.04, 0.038, 0.085]);
      clothSway(mb, m0, [0, 0.05, 0.04], 0.05, 0.3);
      mb.color = C(S.tar);
      for (const x of [-0.04, 0.045]) pole(mb, [x, -0.02, 0.085], [x, 0.04, 0.085], 0.0025, 0.0025, 3);
      mb.color = J("#6a4a30", r, 0.06);
      for (const [x, z] of [[0.06, 0.03], [0.065, 0.05]]) at(mb, x, 0, z, 0, () => mb.cylinder(0.01, 0.01, 0.022, 6));
      break;
    }
    case 2: {
      // A boathouse roofed with an upturned hull.
      const top = plankBody(mb, r, S, 0.13, 0.07, 0.035);
      mb.part(() => {
        mb.translate(0, top, 0);
        mb.color = J(COAST.tar, r, 0.08);
        const n = 6;
        for (let i = 0; i < n; i++) {
          const t0 = i / n, t1 = (i + 1) / n;
          const hull = (t) => [-0.08 + t * 0.16, Math.sin(Math.PI * t) * 0.012];
          const [x0, l0] = hull(t0), [x1, l1] = hull(t1);
          const half = (t) => 0.048 * Math.sin(Math.PI * Math.min(1, 0.15 + t * 0.85));
          for (const s of [-1, 1]) mb.quad([x0, 0.05 + l0, 0], [x1, 0.05 + l1, 0], [x1, 0.0, s * half(t1)], [x0, 0.0, s * half(t0)]);
        }
        mb.color = C(S.weather);
        mb.tube([[-0.085, 0.05, 0], [0, 0.064, 0], [0.085, 0.05, 0]], 0.003, 3);
      });
      break;
    }
    case 4: {
      // A hut on a little pier with a crane boom.
      const y = 0.025;
      stilts(mb, S, 0.12, 0.09, y);
      mb.color = mulc(S.plankC, 0.9);
      at(mb, 0, y, 0, 0, () => blk(mb, 0.13, 0.1, -0.005, 0.0));
      mb.part(() => {
        mb.translate(-0.02, 0, -0.01);
        const top = plankBody(mb, r, S, 0.07, 0.065, 0.045, y);
        mb.translate(0, top, 0);
        mb.color = S.roofC;
        hipRoof(mb, 0.07, 0.065, 0.045, { over: 0.012, ridge: 0.01 });
      });
      mb.color = C(S.tar);
      pole(mb, [0.045, y, 0.03], [0.045, y + 0.11, 0.03], 0.003, 0.0025, 4);
      pole(mb, [0.045, y + 0.1, 0.03], [0.085, y + 0.08, 0.06], 0.0022, 0.002, 3);
      break;
    }
    default:
      tavern(mb, r, S);
  }
  ground(mb, 0, 0.04, 0.6);
}

/** A tavern-house: a plank upper floor over stone, a red roof and a sign in the owner's colour. */
function tavern(mb, r, S) {
  const w = 0.11, d = 0.085;
  mb.color = J(S.stone, r, 0.04);
  blk(mb, w + 0.006, d + 0.006, -0.05, 0.04);
  face(mb, 0, w + 0.006, d + 0.006, () => door(mb, S, 0, 0.018, 0.03, 0, 0.004));
  const top = plankBody(mb, r, S, w, d, 0.045, 0.04, { door: false });
  mb.part(() => {
    mb.translate(0, top, 0);
    mb.rotateY(QT);
    mb.color = J("#6a3a2e", r, 0.05);
    gableRoof(mb, d, w, 0.065, { over: 0.012, overX: 0.01, thick: 0.006, ends: S.plankC });
  });
  mb.color = C(S.tar);
  pole(mb, [0.04, 0.06, d / 2], [0.04, 0.06, d / 2 + 0.025], 0.0018, 0.0018, 3);
  mb.part(() => {
    mb.mat = MAT.TEAM;
    mb.color = C(TEAM_SHADE);
    at(mb, 0.04, 0.045, d / 2 + 0.022, QT, () => rect(mb, -0.01, 0, 0.01, 0.013));
  });
}

/** A mast with a crow's nest barrel, rope ladders and a team flag. */
function crowsNest(mb, r, S, H) {
  mb.color = C(S.tar);
  pole(mb, [0, -0.05, 0], [0, H, 0], 0.008, 0.005, 5);
  mb.color = J("#6a4a30", r, 0.05);
  at(mb, 0, H * 0.78, 0, 0, () => mb.lathe([[0.022, 0], [0.026, 0.01], [0.024, 0.024]], 8, { caps: false }));
  mb.color = C(COAST.weather);
  for (const s of [-1, 1]) pole(mb, [s * 0.05, -0.02, 0.02], [s * 0.01, H * 0.78, 0.0], 0.0012, 0.0012, 3);
  mb.part(() => {
    mb.translate(0, H, 0);
    pennant(mb, 0.0, 0.06, 0.022);
  });
}

function coastalTower(mb, r, v) {
  const S = coastStyle(r);
  if (v === 0) {
    // A lighthouse: a tapering stone shaft, a timber lantern stage with a burning fire.
    mb.color = J(S.stone, r, 0.04);
    drum(mb, 0.05, 0.036, -0.05, 0.3, 9);
    mb.color = J("#d8d0c0", r, 0.03);
    drum(mb, 0.046 - 0.0, 0.043, 0.1, 0.13, 9);
    mb.color = S.plankC;
    mb.lathe([[0.036, 0.3], [0.05, 0.305], [0.05, 0.312]], 9, { caps: false });
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * TAU + TAU / 8;
      pole(mb, [Math.sin(a) * 0.03, 0.31, Math.cos(a) * 0.03], [Math.sin(a) * 0.026, 0.36, Math.cos(a) * 0.026], 0.0025, 0.0025, 3);
    }
    mb.part(() => {
      mb.mat = MAT.EMISSIVE;
      mb.color = C("#ffb050");
      at(mb, 0, 0.33, 0, 0, () => gem(mb, 0.014, 0.018));
    });
    mb.part(() => {
      mb.translate(0, 0.36, 0);
      mb.color = S.roofC;
      coneRoof(mb, 0.04, 0.05, { seg: 8, curve: 0.2, drop: 0.004, rings: 2 });
    });
  } else if (v === 1) crowsNest(mb, r, S, 0.42);
  else {
    // A plank watchtower, tapering, a gallery under a pyramid roof.
    mb.color = S.plankC;
    shell(mb, 0.08, 0.08, -0.05, 0.26, 0.75);
    face(mb, 0, 0.07, 0.07, () => door(mb, S, 0, 0.018, 0.03, 0, 0.0));
    mb.part(() => {
      mb.translate(0, 0.26, 0);
      mb.color = mulc(S.plankC, 0.9);
      blk(mb, 0.085, 0.085, -0.004, 0.004);
      mb.color = S.plankC;
      shell(mb, 0.08, 0.08, 0.004, 0.025);
      for (const [x, z] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) pole(mb, [x * 0.036, 0.0, z * 0.036], [x * 0.036, 0.06, z * 0.036], 0.0025, 0.0025, 3);
      mb.translate(0, 0.06, 0);
      mb.color = S.roofC;
      hipRoof(mb, 0.075, 0.075, 0.05, { over: 0.012 });
      mb.translate(0, 0.05, 0);
      pennant(mb, 0.0, 0.04, 0.016);
    });
  }
  ground(mb, 0, 0.04, 0.6);
}

function coastalHall(mb, r, v) {
  const S = coastStyle(r);
  if (v === 0) {
    tavern(mb, r, S);
    mb.part(() => {
      mb.translate(0.11, 0, 0.0);
      const top = plankBody(mb, r, S, 0.08, 0.08, 0.06);
      mb.translate(0, top, 0);
      mb.color = S.roofC;
      gableRoof(mb, 0.08, 0.08, 0.05, { over: 0.012, overX: 0.008, ends: S.plankC });
    });
  } else if (v === 1) {
    // A warehouse with a hoist and barrels.
    const top = plankBody(mb, r, S, 0.18, 0.1, 0.07);
    mb.part(() => {
      mb.translate(0, top, 0);
      mb.color = S.roofC;
      gableRoof(mb, 0.18, 0.1, 0.07, { over: 0.014, overX: 0.01, thick: 0.006, sag: 0.008, ends: S.plankC });
    });
    mb.color = C(S.tar);
    pole(mb, [0.07, top + 0.03, 0.05], [0.07, top + 0.03, 0.08], 0.003, 0.003, 3);
    mb.color = J("#6a4a30", r, 0.06);
    for (const [x, z] of [[-0.06, 0.07], [-0.04, 0.075], [-0.05, 0.09]]) at(mb, x, 0, z, 0, () => mb.cylinder(0.01, 0.01, 0.022, 6));
  } else {
    // A shipwright's slip: the ribs of a hull on the stocks beside a shed.
    mb.color = J("#8a6a4a", r, 0.05);
    pole(mb, [-0.11, 0.012, 0.02], [0.1, 0.02, 0.02], 0.005, 0.005, 4);
    for (let i = 0; i < 6; i++) {
      const x = -0.08 + i * 0.03;
      const s = Math.sin(((i + 0.5) / 6) * Math.PI);
      for (const z of [-1, 1]) mb.tube([[x, 0.012, 0.02], [x, 0.03 * s + 0.02, 0.02 + z * 0.035 * s], [x, 0.07 * s + 0.02, 0.02 + z * 0.042 * s]], 0.0025, 3);
    }
    mb.part(() => {
      mb.translate(0, 0, -0.07);
      const top = plankBody(mb, r, S, 0.12, 0.05, 0.045);
      mb.translate(0, top, 0);
      mb.color = S.roofC;
      mb.quad([-0.07, 0.03, -0.035], [0.07, 0.03, -0.035], [0.07, -0.004, 0.035], [-0.07, -0.004, 0.035]);
    });
  }
  ground(mb, 0, 0.04, 0.6);
}

function coastalKeep(mb, r, v) {
  const S = coastStyle(r);
  if (v === 0) {
    // The hulk hall: a beached galleon, a captain's house built on its deck, masts with flags.
    mb.part(() => {
      mb.translate(0, 0.02, 0);
      mb.color = J(COAST.tar, r, 0.06);
      const n = 8, L = 0.3;
      const hw = (t) => 0.07 * Math.pow(Math.sin(Math.PI * Math.min(1, 0.08 + t * 0.92)), 0.6);
      for (let i = 0; i < n; i++) {
        const t0 = i / n, t1 = (i + 1) / n;
        const x0 = -L / 2 + L * t0, x1 = -L / 2 + L * t1;
        for (const s of [-1, 1]) {
          mb.quad([x0, 0.07, s * hw(t0)], [x1, 0.07, s * hw(t1)], [x1, -0.05, s * hw(t1) * 0.3], [x0, -0.05, s * hw(t0) * 0.3]);
        }
        mb.color = mulc(S.plankC, 0.9);
        mb.quad([x0, 0.07, -hw(t0)], [x1, 0.07, -hw(t1)], [x1, 0.07, hw(t1)], [x0, 0.07, hw(t0)]);
        mb.color = J(COAST.tar, r, 0.06);
      }
      mb.color = J("#6a3a2e", r, 0.05);
      for (const s of [-1, 1]) mb.quad([-L / 2 + 0.02, 0.06, s * hw(0.06)], [L / 2 - 0.02, 0.06, s * hw(0.94)], [L / 2 - 0.02, 0.07, s * hw(0.94)], [-L / 2 + 0.02, 0.07, s * hw(0.06)]);
      mb.part(() => {
        mb.translate(-0.07, 0.07, 0);
        const top = plankBody(mb, r, S, 0.1, 0.09, 0.06);
        mb.translate(0, top - 0.07, 0);
        mb.color = J("#6a3a2e", r, 0.05);
        gableRoof(mb, 0.1, 0.09, 0.05, { over: 0.01, overX: 0.008, ends: S.plankC });
      });
      for (const [x, H] of [[0.03, 0.34], [0.1, 0.26]]) at(mb, x, 0.07, 0, 0, () => crowsNest(mb, r, S, H));
      const m0 = mb.mark();
      mb.color = mulc(C(S.sail), 0.85);
      mb.quad([0.0, 0.3, 0.004], [0.06, 0.3, 0.004], [0.065, 0.2, 0.012], [-0.004, 0.19, 0.012]);
      clothSway(mb, m0, [0.03, 0.32, 0], 0.12, 0.3);
    });
  } else {
    // A pirate fortress: a stone bastion, cannon ports, a timber upper house and a great flag.
    mb.color = J(S.stone, r, 0.04);
    shard(mb, 6, 0.16, -0.05, 0.09, { top: 0.92, phase: TAU / 12 });
    mb.color = mulc(J(S.stone, r, 0.04), 0.9);
    for (let k = 0; k < 5; k++) {
      const a = -1 + k * 0.5;
      mb.mat = MAT.METAL;
      mb.color = C("#2a2a2e");
      at(mb, Math.sin(a) * 0.14, 0.065, Math.cos(a) * 0.14, a, () => {
        mb.rotateX(QT * 0.92);
        mb.cylinder(0.006, 0.005, 0.03, 5);
      });
    }
    mb.mat = MAT.MATTE;
    mb.part(() => {
      mb.translate(0, 0.09, -0.02);
      const top = plankBody(mb, r, S, 0.14, 0.1, 0.07, 0, { door: true });
      mb.translate(0, top, 0);
      mb.color = J("#6a3a2e", r, 0.05);
      gableRoof(mb, 0.14, 0.1, 0.07, { over: 0.014, overX: 0.01, ends: S.plankC });
    });
    at(mb, 0.1, 0.09, 0.05, 0, () => crowsNest(mb, r, S, 0.3));
  }
  ground(mb, 0, 0.05, 0.6);
}

function coastalWall(mb, r) {
  const S = coastStyle(r);
  for (let i = 0; i < 10; i++) {
    mb.color = J(i % 2 ? "#6e5236" : "#7a5a3c", r, 0.06);
    at(mb, -0.095 + i * 0.021, 0, 0, 0, () => blk(mb, 0.019, 0.01, -0.05, 0.07 + (i % 3) * 0.006));
  }
  mb.color = C(S.tar);
  for (const y of [0.02, 0.055]) at(mb, 0, y, 0.007, 0, () => blk(mb, 0.205, 0.005, 0, 0.008));
  ground(mb, 0, 0.04, 0.6);
}

function coastalGate(mb, r) {
  const S = coastStyle(r);
  for (const s of [-1, 1]) at(mb, s * 0.07, 0, 0, 0, () => crowsNest(mb, r, S, 0.18));
  mb.color = C(S.tar);
  pole(mb, [-0.075, 0.12, 0.0], [0.075, 0.12, 0.0], 0.006, 0.006, 4);
  for (let i = 0; i < 4; i++) {
    mb.color = J(i % 2 ? "#6e5236" : "#7a5a3c", r, 0.06);
    at(mb, -0.045 + i * 0.03, 0, -0.01, 0, () => blk(mb, 0.026, 0.008, -0.03, 0.09));
  }
  at(mb, 0, 0, 0.0, 0, () => wallBanner(mb, 0, 0.115, 0.05, 0.05, 0.008));
  ground(mb, 0, 0.04, 0.6);
}

function coastalWallTower(mb, r, v) {
  const S = coastStyle(r);
  if (v) crowsNest(mb, r, S, 0.16);
  else {
    mb.color = S.plankC;
    shell(mb, 0.06, 0.06, -0.05, 0.1, 0.85);
    mb.translate(0, 0.1, 0);
    mb.color = S.roofC;
    hipRoof(mb, 0.052, 0.052, 0.04, { over: 0.01 });
  }
  ground(mb, 0, 0.04, 0.6);
}

// --- common pieces ----------------------------------------------------------------------------------

/** A well: a stone ring, two posts, a small roof and its winch. */
function well(mb, r, f) {
  const stone = J(f === "dwarven" ? "#6a6560" : f === "desert" ? "#b8a07a" : f === "infernal" ? "#2a2428" : "#8c847a", r, 0.04);
  mb.color = stone;
  mb.lathe([[0.024, -0.03], [0.024, 0.02], [0.019, 0.022], [0.017, 0.012]], 9, { caps: false });
  mb.color = C(f === "infernal" ? "#ff4a10" : "#22303a");
  if (f === "infernal") mb.mat = MAT.EMISSIVE;
  mb.part(() => {
    mb.translate(0, 0.012, 0);
    mb.lathe([[0.017, 0], [0, 0.0001]], 9, { caps: false });
  });
  mb.mat = MAT.MATTE;
  if (f === "infernal") {
    mb.color = C("#d9cfb6");
    for (const s of [-1, 1]) mb.tube([[s * 0.022, 0.015, 0], [s * 0.026, 0.045, 0.004], [s * 0.016, 0.07, 0.006]], [0.004, 0.003, 0.001], 4);
  } else {
    mb.color = C("#5a3e2a");
    for (const s of [-1, 1]) at(mb, s * 0.02, 0, 0, 0, () => blk(mb, 0.005, 0.005, 0.015, 0.062));
    at(mb, 0, 0.048, 0, 0, () => {
      mb.rotateZ(QT);
      mb.cylinder(0.003, 0.003, 0.04, 4, { caps: false });
    });
    mb.part(() => {
      mb.translate(-0.0, 0.062, 0);
      mb.color = J(f === "desert" ? "#c8b898" : f === "dwarven" ? "#3b3b42" : "#8a3d2b", r, 0.06);
      gableRoof(mb, 0.044, 0.03, 0.022, { over: 0.008, overX: 0.004, thick: 0.003 });
    });
  }
  ground(mb, 0, 0.03);
}

/** A market stall: a counter, four posts, a striped awning in the owner's colour, wares. */
function stall(mb, r, f) {
  const wood = J(f === "infernal" ? "#2a1c1c" : "#6a4a30", r, 0.06);
  mb.color = wood;
  blk(mb, 0.06, 0.03, -0.02, 0.022);
  for (const [x, z] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) at(mb, x * 0.028, 0, z * 0.016, 0, () => blk(mb, 0.004, 0.004, 0.0, z > 0 ? 0.05 : 0.06));
  // Awning: stripes alternating team colour and linen, sloping toward the front.
  const m0 = mb.mark();
  const n = 4;
  for (let i = 0; i < n; i++) {
    const x0 = -0.034 + (0.068 * i) / n, x1 = -0.034 + (0.068 * (i + 1)) / n;
    if (i % 2 === 0) {
      mb.mat = MAT.TEAM;
      mb.color = C(TEAM);
    } else {
      mb.mat = MAT.MATTE;
      mb.color = C(f === "infernal" ? "#3a2a2a" : "#d6c8a8");
    }
    mb.quad([x0, 0.062, -0.024], [x1, 0.062, -0.024], [x1, 0.048, 0.03], [x0, 0.048, 0.03]);
    mb.quad([x0, 0.048, 0.03], [x1, 0.048, 0.03], [x1, 0.038, 0.032], [x0, 0.038, 0.032]);
  }
  mb.mat = MAT.MATTE;
  clothSway(mb, m0, [0, 0.062, -0.024], 0.07, 0.15);
  // Wares: sacks, baskets and crates.
  const wares = ["#9a7a4a", "#8a3a2a", "#b0903a", "#5a6a3a", "#7a5a3a"];
  for (let i = 0; i < 3; i++) {
    mb.color = J(r.pick(wares), r, 0.08);
    at(mb, -0.02 + i * 0.02, 0.022, r.range(-0.006, 0.006), r.range(0, 1), () => (r() < 0.5 ? blk(mb, 0.012, 0.012, 0, 0.008) : mb.sphere(0.0075, 0, { squash: 0.7 })));
  }
  ground(mb, 0, 0.02);
}

/** A banner pole: v0 a hanging banner on a crossbar, v1 a tall swallow-tailed standard. */
function bannerPole(mb, r, v, f) {
  const poleC = f === "infernal" ? "#1e1a1e" : f === "tribal" ? "#5a4030" : f === "elven" ? "#8a8676" : "#3e2c20";
  mb.color = C(poleC);
  const H = v ? 0.24 : 0.19;
  mb.cylinder(0.0045, 0.0035, H, 5, { caps: false });
  at(mb, 0, -0.03, 0, 0, () => mb.cylinder(0.006, 0.005, 0.04, 5, { caps: false }));
  const m0 = mb.mark();
  const top = H - 0.012;
  if (v === 0) {
    at(mb, 0, top, 0, 0, () => {
      mb.rotateZ(QT);
      mb.translate(0, -0.024, 0);
      mb.cylinder(0.0025, 0.0025, 0.048, 4, { caps: false });
    });
    mb.part(() => {
      mb.mat = MAT.TEAM;
      mb.color = C(TEAM);
      const w = 0.04, h = 0.075, z = 0.003;
      mb.quad([-w / 2, top - h, z], [w / 2, top - h, z], [w / 2, top, z], [-w / 2, top, z]);
      mb.tri([-w / 2, top - h, z], [0, top - h - 0.018, z], [w / 2, top - h, z]);
      mb.color = C(TEAM_SHADE);
      rect(mb, -w * 0.32, top - h * 0.55, w * 0.32, top - h * 0.25, z + 0.0006);
    });
    hangSway(mb, m0, top, 0.09, 0.4);
  } else {
    mb.part(() => {
      mb.mat = MAT.TEAM;
      mb.color = C(TEAM);
      const h = 0.05, L = 0.08;
      mb.quad([0, top, 0], [0, top - h, 0], [L * 0.6, top - h * 0.9, 0.004], [L * 0.6, top - h * 0.1, 0.004]);
      mb.tri([L * 0.6, top - h * 0.1, 0.004], [L * 0.6, top - h * 0.5, 0.004], [L, top - h * 0.12, 0.008]);
      mb.tri([L * 0.6, top - h * 0.5, 0.004], [L * 0.6, top - h * 0.9, 0.004], [L, top - h * 0.95, 0.008]);
    });
    clothSway(mb, m0, [0, top, 0], 0.08, 0.9);
  }
  mb.part(() => {
    mb.mat = MAT.METAL;
    mb.color = C(f === "infernal" ? "#5a5e66" : "#c9a043");
    at(mb, 0, H, 0, 0, () => (f === "infernal" ? mb.cone(0.006, 0.02, 4) : mb.sphere(0.006, 0)));
  });
}

/** A fence along x (length 0.2): posts and rails (human), wattle (tribal), a dry-stone dyke
 *  (dwarven), woven roots (elven), bone stakes (infernal). */
function fence(mb, r, f) {
  const L = 0.2;
  if (f === "dwarven" || f === "desert") {
    mb.color = J(f === "desert" ? "#b0956e" : "#6e6964", r, 0.05);
    blk(mb, L, 0.016, -0.03, 0.022, 0.85);
    return ground(mb, 0, 0.02, 0.6);
  }
  if (f === "elven") {
    mb.color = J("#8a8270", r, 0.05);
    for (let k = 0; k < 2; k++) {
      const pts = [];
      for (let i = 0; i <= 6; i++) {
        const x = -L / 2 + (L * i) / 6;
        pts.push([x, 0.012 + 0.012 * Math.sin(i * 1.6 + k * 2.2) + k * 0.006, 0.003 * Math.cos(i * 2 + k)]);
      }
      mb.tube(pts, 0.0035, 4);
    }
    return ground(mb, 0, 0.02, 0.6);
  }
  const post = J(f === "infernal" ? "#d9cfb6" : f === "tribal" ? "#5a4030" : "#5e4630", r, 0.06);
  const n = 5;
  for (let i = 0; i < n; i++) {
    const x = -L / 2 + 0.01 + ((L - 0.02) * i) / (n - 1);
    mb.color = post;
    const h = 0.032 + r.range(-0.004, 0.004);
    at(mb, x, 0, r.range(-0.002, 0.002), r.range(-0.2, 0.2), () => {
      if (f === "tribal" || f === "infernal") mb.lathe([[0.0045, -0.03], [0.004, h], [0, h + 0.012]], 4, { caps: false });
      else blk(mb, 0.005, 0.005, -0.03, h);
    });
  }
  mb.color = J(f === "tribal" ? "#6a5238" : f === "infernal" ? "#2a2426" : "#6a5034", r, 0.06);
  if (f === "tribal") {
    blk(mb, L, 0.006, 0.0, 0.024);
  } else {
    for (const y of [0.012, 0.026]) at(mb, 0, y, 0, 0, () => blk(mb, L, 0.004, 0, 0.004));
  }
  ground(mb, 0, 0.02, 0.6);
}

/** A totem: a carved pole with stacked heads, horns and a skull (tribal camps). */
function totem(mb, r, sc = 1) {
  const m0 = mb.mark();
  mb.part(() => {
  mb.scale(sc);
  mb.color = J("#5a4030", r, 0.05);
  mb.cylinder(0.011, 0.009, 0.16, 6, { caps: false });
  at(mb, 0, -0.03, 0, 0, () => mb.cylinder(0.013, 0.012, 0.04, 6, { caps: false }));
  const paint = ["#6e3424", "#38404a", "#8a7044"];
  for (let i = 0; i < 3; i++) {
    mb.color = J(paint[i], r, 0.05);
    at(mb, 0, 0.04 + i * 0.04, 0, 0, () => mb.lathe([[0.012, 0], [0.016, 0.01], [0.015, 0.024], [0.011, 0.032]], 6, { caps: false }));
  }
  mb.color = J("#d9cfb6", r, 0.04);
  at(mb, 0, 0.165, 0.003, 0, () => mb.sphere(0.013, 0, { squash: 0.9 }));
  for (const s of [-1, 1]) mb.tube([[s * 0.01, 0.17, 0], [s * 0.026, 0.18, 0], [s * 0.032, 0.2, -0.004]], [0.004, 0.003, 0.0008], 4);
  mb.color = C("#1a1414");
  at(mb, 0, 0.165, 0.0155, 0, () => {
    rect(mb, -0.007, 0.0, -0.002, 0.004);
    rect(mb, 0.002, 0.0, 0.007, 0.004);
  });
  });
  ground(mb, m0, 0.04);
}

/** A brazier: a tripod bowl with burning coals (emissive); pair with an ember emitter. */
function brazier(mb, r, f) {
  const metal = f === "infernal" ? "#2a2226" : "#4a4440";
  mb.color = C(metal);
  mb.mat = MAT.METAL;
  for (let k = 0; k < 3; k++) {
    const a = (k / 3) * TAU;
    mb.tube([[Math.sin(a) * 0.016, -0.01, Math.cos(a) * 0.016], [Math.sin(a) * 0.01, 0.04, Math.cos(a) * 0.01]], 0.0025, 4);
  }
  at(mb, 0, 0.04, 0, 0, () => mb.lathe([[0.006, 0], [0.018, 0.008], [0.022, 0.018]], 7, { caps: false }));
  mb.mat = MAT.EMISSIVE;
  mb.color = C(f === "infernal" ? "#ff4a12" : "#ff8a30");
  at(mb, 0, 0.054, 0, 0, () => mb.sphere(0.017, 0, { squash: 0.45 }));
  void r;
}

// --- recipes ------------------------------------------------------------------------------------------

const BUILD = {
  human: { house: humanHouse, hall: humanHall, tower: humanTower, keep: humanKeep, wall: humanWall, gate: humanGate, walltower: humanWallTower },
  elven: { house: elvenHouse, hall: elvenHall, tower: elvenTower, keep: elvenKeep, wall: elvenWall, gate: elvenGate, walltower: elvenWallTower },
  dwarven: { house: dwarvenHouse, hall: dwarvenHall, tower: dwarvenTower, keep: dwarvenKeep, wall: dwarvenWall, gate: dwarvenGate, walltower: dwarvenWallTower },
  infernal: { house: infernalHouse, hall: infernalHall, tower: infernalTower, keep: infernalKeep, wall: infernalWall, gate: infernalGate, walltower: infernalWallTower },
  tribal: { house: tribalHouse, hall: tribalHall, tower: tribalTower, keep: tribalKeep, wall: tribalWall, gate: tribalGate, walltower: tribalWallTower },
  desert: { house: desertHouse, hall: desertHall, tower: desertTower, keep: desertKeep, wall: desertWall, gate: desertGate, walltower: desertWallTower },
  coastal: { house: coastalHouse, hall: coastalHall, tower: coastalTower, keep: coastalKeep, wall: coastalWall, gate: coastalGate, walltower: coastalWallTower },
};

/** The builder of `kind` for a param (families not yet modelled fall back to human). */
function builder(kind, param) {
  let { f, sub } = famOf(param);
  // The Calabim's gothic towns are the human builders in a darker, steeper key.
  if (f === "infernal" && sub === "gothic") f = "human";
  const fam = BUILD[f] ?? BUILD.human;
  return { fn: fam[kind] ?? BUILD.human[kind], sub, f };
}

recipe("bld.house", { variants: 6, build: (mb, r, { variant, param }) => { const b = builder("house", param); b.fn(mb, r, variant, b.sub); } });
recipe("bld.hall", { variants: 3, build: (mb, r, { variant, param }) => { const b = builder("hall", param); b.fn(mb, r, variant, b.sub); } });
recipe("bld.tower", { variants: 3, build: (mb, r, { variant, param }) => { const b = builder("tower", param); b.fn(mb, r, variant, b.sub); } });
recipe("bld.keep", { variants: 2, build: (mb, r, { variant, param }) => { const b = builder("keep", param); b.fn(mb, r, variant, b.sub); } });
recipe("bld.wall", {
  variants: 2,
  build: (mb, r, { variant, param }) => {
    const b = builder("wall", param);
    // Variant 1 is a half-length piece (forts' fronts beside their gates).
    if (variant === 1) mb.scale(0.5, 1, 1);
    b.fn(mb, r, b.sub);
  },
});
recipe("bld.gate", { variants: 1, build: (mb, r, { param }) => { const b = builder("gate", param); b.fn(mb, r, b.sub); } });
recipe("bld.walltower", { variants: 2, build: (mb, r, { variant, param }) => { const b = builder("walltower", param); b.fn(mb, r, variant, b.sub); } });
recipe("bld.well", { variants: 1, build: (mb, r, { param }) => well(mb, r, famOf(param).f) });
recipe("bld.stall", { variants: 3, build: (mb, r, { param }) => stall(mb, r, famOf(param).f) });
recipe("bld.banner", { variants: 2, build: (mb, r, { variant, param }) => bannerPole(mb, r, variant, famOf(param).f) });
recipe("bld.fence", { variants: 2, build: (mb, r, { param }) => fence(mb, r, famOf(param).f) });
recipe("bld.totem", { variants: 2, build: (mb, r) => totem(mb, r) });
recipe("bld.brazier", { variants: 1, build: (mb, r, { param }) => brazier(mb, r, famOf(param).f) });

// --- the catalog: families, cities, settlements ---------------------------------------------------------

const CIV_FAMILY = {
  CIVILIZATION_BANNOR: "human",
  CIVILIZATION_MALAKIM: "desert",
  CIVILIZATION_ELOHIM: "human",
  CIVILIZATION_MERCURIANS: "human",
  CIVILIZATION_LUCHUIRP: "dwarven",
  CIVILIZATION_KURIOTATES: "human",
  CIVILIZATION_LJOSALFAR: "elven",
  CIVILIZATION_KHAZAD: "dwarven",
  CIVILIZATION_SIDAR: "human",
  CIVILIZATION_LANUN: "coastal",
  CIVILIZATION_GRIGORI: "human",
  CIVILIZATION_HIPPUS: "human",
  CIVILIZATION_AMURITES: "human",
  CIVILIZATION_DOVIELLO: "tribal",
  CIVILIZATION_BALSERAPHS: "human",
  CIVILIZATION_CLAN_OF_EMBERS: "tribal",
  CIVILIZATION_SVARTALFAR: "elven",
  CIVILIZATION_CALABIM: "infernal",
  CIVILIZATION_SHEAIM: "infernal",
  CIVILIZATION_INFERNAL: "infernal",
  CIVILIZATION_ILLIANS: "human",
  CIVILIZATION_BARBARIAN: "tribal",
};

/** The sub-style of a civilization within its family (the recipes' param suffix). */
const CIV_SUB = {
  CIVILIZATION_SVARTALFAR: "dark",
  CIVILIZATION_CALABIM: "gothic",
  CIVILIZATION_AMURITES: "arcane",
  CIVILIZATION_SIDAR: "arcane",
  CIVILIZATION_DOVIELLO: "north",
  CIVILIZATION_ILLIANS: "north",
};

function familyOf(civ) {
  return CIV_FAMILY[civ] ?? "human";
}

/** The recipe param for a family and civilization. */
function paramFor(family, civ) {
  const f = FAMILIES.includes(family) ? family : "human";
  const sub = CIV_SUB[civ];
  return sub ? `${f}.${sub}` : f;
}

// --- layout -----------------------------------------------------------------------------------------
// Settlements are planned with oriented footprints (each part's mesh bounds, turned and scaled), so
// buildings line streets wall to wall without their roofs running into each other.

/** The oriented footprint of a part: centre, half extents along its own x and z, and its turn. */
function obb(asset, variant, param, x, z, yaw, scale = 1, pad = 0) {
  const b = buildAsset(asset, variant, param);
  const lx = ((b.lo[0] + b.hi[0]) / 2) * scale, lz = ((b.lo[2] + b.hi[2]) / 2) * scale;
  const c = Math.cos(yaw), s = Math.sin(yaw);
  return { x: x + c * lx + s * lz, z: z - s * lx + c * lz, hw: ((b.hi[0] - b.lo[0]) / 2) * scale + pad, hd: ((b.hi[2] - b.lo[2]) / 2) * scale + pad, c, s };
}

/** Whether two footprints overlap by more than `tol` (separating axes). */
function overlap(A, B, tol = 0.004) {
  const dx = B.x - A.x, dz = B.z - A.z;
  for (const [ax, az] of [[A.c, -A.s], [A.s, A.c], [B.c, -B.s], [B.s, B.c]]) {
    const ra = A.hw * Math.abs(A.c * ax - A.s * az) + A.hd * Math.abs(A.s * ax + A.c * az);
    const rb = B.hw * Math.abs(B.c * ax - B.s * az) + B.hd * Math.abs(B.s * ax + B.c * az);
    if (Math.abs(dx * ax + dz * az) > ra + rb - tol) return false;
  }
  return true;
}

/** Whether a footprint comes within `rad` of the point (x, z). */
function nearPoint(A, x, z, rad) {
  const dx = x - A.x, dz = z - A.z;
  const u = dx * A.c - dz * A.s, v = dx * A.s + dz * A.c;
  return Math.hypot(Math.max(0, Math.abs(u) - A.hw), Math.max(0, Math.abs(v) - A.hd)) < rad;
}

function corners(A) {
  const out = [];
  for (const [u, v] of [[-A.hw, -A.hd], [A.hw, -A.hd], [A.hw, A.hd], [-A.hw, A.hd]]) out.push([A.x + A.c * u + A.s * v, A.z - A.s * u + A.c * v]);
  return out;
}

const yawTo = (dx, dz) => Math.atan2(dx, dz);

/** A settlement being planned: the parts placed so far, their footprints, the open spaces and
 *  streets that must stay clear, and the radius everything stays within. */
class Plan {
  constructor(r, P, Rmax, height = null) {
    this.r = r;
    this.P = P;
    this.Rmax = Rmax;
    this.height = height;
    this.out = [];
    this.out.emit = [];
    this.boxes = [];
    this.zones = [];
    this.lanes = [];
  }

  /** Keeps a disc clear (squares, the garrison's place). */
  zone(x, z, rad) {
    this.zones.push([x, z, rad]);
  }

  /** Keeps a street clear: a polyline of half-width `half`. */
  lane(pts, half = 0.016) {
    for (let i = 0; i + 1 < pts.length; i++) {
      const [ax, az] = pts[i], [bx, bz] = pts[i + 1];
      const n = Math.max(1, Math.ceil(Math.hypot(bx - ax, bz - az) / 0.02));
      for (let k = 0; k <= n; k++) this.lanes.push([ax + ((bx - ax) * k) / n, az + ((bz - az) * k) / n, half]);
    }
  }

  fits(A, tol = 0.004, rmax = this.Rmax) {
    for (const [x, z] of corners(A)) if (Math.hypot(x, z) > rmax) return false;
    for (const [x, z, rad] of this.zones) if (nearPoint(A, x, z, rad)) return false;
    for (const [x, z, rad] of this.lanes) if (nearPoint(A, x, z, rad)) return false;
    for (const B of this.boxes) if (overlap(A, B, tol)) return false;
    // Dry, fairly level ground: no footing in the sea or across a river bank.
    if (this.height) {
      let lo = Infinity, hi = -Infinity;
      for (const [x, z] of [...corners(A), [A.x, A.z]]) {
        const h = this.height(x, z);
        lo = Math.min(lo, h);
        hi = Math.max(hi, h);
      }
      if (lo < 0.015 || hi - lo > 0.065) return false;
    }
    return true;
  }

  add(asset, variant, x, z, yaw, scale = 1, o = {}) {
    const p = { asset, param: o.param ?? this.P, variant, x, z, yaw, scale };
    if (o.sx) p.sx = o.sx;
    this.out.push(p);
    if (!o.ghost) this.boxes.push(obb(asset, variant, p.param, x, z, yaw, scale, o.pad ?? 0));
    return p;
  }

  /** Places the part if its footprint is free. */
  try(asset, variant, x, z, yaw, scale = 1, o = {}) {
    const A = obb(asset, variant, o.param ?? this.P, x, z, yaw, scale, o.pad ?? 0);
    if (!this.fits(A, o.tol, o.rmax)) return null;
    return this.add(asset, variant, x, z, yaw, scale, o);
  }

  /** Tries random spots (polar, radius rmin..rmax, angles a0..a1 from +z) facing `face(x, z)`. */
  scatter(item, { rmin = 0.1, rmax = this.Rmax, a0 = 0, a1 = TAU, tries = 80, face = null, back = null } = {}) {
    const r = this.r;
    for (let t = 0; t < tries; t++) {
      const a = r.range(a0, a1), d = r.range(rmin, rmax);
      const x = Math.sin(a) * d, z = Math.cos(a) * d;
      if (back !== null && z > back) continue;
      const yaw = face ? face(x, z) : r.range(0, TAU);
      const p = this.try(item.asset, item.variant, x, z, yaw, item.scale ?? 1, item);
      if (p) return p;
    }
    return null;
  }

  /** Lines one side (+1 left, -1 right) of a street with items (consumed from the front of the
   *  list), fronts on the street edge, wall to wall with small gaps; at most `quota` items. */
  row(pts, side, items, { half = 0.022, start = 0.02, quota = 99, gap = 0.008, setback = 0.006, jitter = 0.06 } = {}) {
    const r = this.r;
    const segs = [];
    let L = 0;
    for (let i = 0; i + 1 < pts.length; i++) {
      const l = Math.hypot(pts[i + 1][0] - pts[i][0], pts[i + 1][1] - pts[i][1]);
      segs.push([pts[i], pts[i + 1], L, l]);
      L += l;
    }
    const at = (s) => {
      for (const [a, b, s0, l] of segs) {
        if (s <= s0 + l || b === pts[pts.length - 1]) {
          const t = Math.min(1, Math.max(0, (s - s0) / l));
          return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, (b[0] - a[0]) / l, (b[1] - a[1]) / l];
        }
      }
      return null;
    };
    let s = start, n = 0;
    while (s < L && items.length && n < quota) {
      // The first of the next few items that fits here (a big hall waits for a big gap).
      let done = 0;
      for (let k = 0; k < Math.min(4, items.length) && !done; k++) {
        const it = items[k];
        const b = buildAsset(it.asset, it.variant, it.param ?? this.P);
        const sc = it.scale ?? 1;
        const hw = ((b.hi[0] - b.lo[0]) / 2) * sc;
        const q = at(Math.min(L, s + hw));
        if (!q) break;
        const [px, pz, tx, tz] = q;
        const nx = -tz * side, nz = tx * side;
        const off = half + b.hi[2] * sc + 0.002 + r.range(0, setback);
        const x = px + nx * off, z = pz + nz * off;
        const yaw = yawTo(-nx, -nz) + r.range(-jitter, jitter);
        if (this.try(it.asset, it.variant, x, z, yaw, sc, it)) {
          items.splice(k, 1);
          n++;
          done = 2 * hw + r.range(0, gap) + (r() < 0.1 ? r.range(0.03, 0.06) : 0);
        }
      }
      s += done || 0.016;
    }
  }
}

/** A street from (x, z) heading `a` (from +z), drifting by `turn` per step, until radius `R`. */
function street(r, x, z, a, turn, R, step = 0.05) {
  const pts = [[x, z]];
  for (let s = 0; s < 14; s++) {
    x += Math.sin(a) * step;
    z += Math.cos(a) * step;
    pts.push([x, z]);
    a += turn;
    if (Math.hypot(x, z) > R) break;
  }
  return pts;
}

/** A ring street around (cx, cz) of radius R (wobbling), from angle a0 to a1 (from +z, clockwise
 *  seen from above). */
function ringStreet(r, cx, cz, R, a0, a1) {
  const ph = r.range(0, TAU), wob = r.range(0.02, 0.05);
  const n = Math.max(8, Math.round(((a1 - a0) * R) / 0.04));
  const pts = [];
  for (let k = 0; k <= n; k++) {
    const a = a0 + ((a1 - a0) * k) / n;
    const rr = R * (1 + wob * Math.sin(2 * a + ph));
    pts.push([cx + Math.sin(a) * rr, cz + Math.cos(a) * rr]);
  }
  return pts;
}

/** Shuffles a list in place with the generator. */
function shuffle(r, list) {
  for (let i = list.length - 1; i > 0; i--) {
    const j = Math.floor(r() * (i + 1));
    [list[i], list[j]] = [list[j], list[i]];
  }
  return list;
}

/** How many of each kind a settlement of `size` has. */
function cityCounts(size) {
  return {
    houses: Math.min(32, Math.round(4 + size * 2.2)),
    halls: size >= 9 ? 2 : size >= 4 ? 1 : 0,
    towers: size >= 11 ? 2 : size >= 5 ? 1 : 0,
    stalls: Math.min(4, 1 + Math.floor(size / 3)),
  };
}

/** How each family lays out a settlement: gaps between buildings on a street front, how freely
 *  they turn, the square's furniture, trees among the houses, and its particles. */
const LAY = {
  human: { gap: 0.008, jitter: 0.06, stalls: true, well: true, banner: 0 },
  dwarven: { gap: 0.004, jitter: 0.0, stalls: true, well: true, banner: 0, sparks: true },
  desert: { gap: 0.004, jitter: 0.04, stalls: true, well: true, banner: 0 },
  coastal: { gap: 0.02, jitter: 0.2, stalls: true, well: false, banner: 1 },
  elven: { gap: 0.022, jitter: 0.25, stalls: false, well: false, banner: 1, trees: true, motes: true },
  tribal: { gap: 0.03, jitter: 0.5, stalls: false, well: false, banner: 1, fire: true },
  infernal: { gap: 0.012, jitter: 0.25, stalls: false, well: true, banner: 0, braziers: true },
};

/** A settlement's layout around its plot centre (the catalog's `city`). */
function cityLayout(ctx) {
  const r = ctx.r;
  const size = Math.max(1, ctx.size ?? 1);
  const P = paramFor(ctx.family, ctx.civ);
  const { f, sub } = famOf(P);
  const L = sub === "gothic" ? { ...LAY.human, stalls: false, braziers: true } : LAY[f] ?? LAY.human;
  const walls = !!ctx.walls;
  const Rw = walls ? Math.min(0.445, 0.37 + size * 0.01) : 0;
  const Rmax = walls ? Rw - 0.03 : Math.min(0.46, 0.38 + size * 0.008);
  const plan = new Plan(r, P, Rmax, typeof ctx.height === "function" ? (x, z) => ctx.height(x, z) : null);
  const out = plan.out;
  const N = cityCounts(size);
  // The garrison's place by the gate, and the way in.
  plan.zone(0, 0.3, 0.1);
  plan.lane([[0, 0.12], [0, 0.6]], 0.05);

  // The landmark at the back of the square (a great tree's crown may spread over the houses).
  const lmScale = ctx.capital ? 1 : Math.min(0.92, 0.7 + size * 0.025);
  const lm = { x: r.range(-0.05, 0.05), z: ctx.capital ? r.range(-0.12, -0.09) : r.range(-0.14, -0.1) };
  if (size <= 3 && !ctx.capital && f !== "elven") plan.add("bld.hall", r.int(0, 2), lm.x, lm.z + 0.03, r.range(-0.15, 0.15), 1);
  else {
    const p = plan.add("bld.keep", ctx.capital ? 1 : 0, lm.x, lm.z, r.range(-0.1, 0.1), f === "elven" ? Math.max(0.9, lmScale) : lmScale, { ghost: f === "elven" });
    if (f === "elven") plan.boxes.push({ x: lm.x, z: lm.z, hw: 0.14 * p.scale, hd: 0.12 * p.scale, c: 1, s: 0 });
  }
  const towerV = sub === "arcane" ? [2, 0] : f === "elven" ? [2, 0, 1] : [0, 1];
  const cityTower = (k) => ({ asset: "bld.tower", variant: towerV[k % towerV.length], scale: f === "elven" ? 0.9 : 0.82 });

  // The square: a well, stalls, banners flanking the way to the landmark; a fire for a camp,
  // braziers for an infernal court.
  const sq = { x: lm.x * 0.5, z: 0.12 };
  plan.zone(sq.x, sq.z, 0.075);
  for (const s of [-1, 1]) plan.add("bld.banner", L.banner, lm.x + s * 0.09, lm.z + 0.16, L.banner ? -0.4 : 0, 1, { ghost: true });
  if (L.well) plan.try("bld.well", 0, sq.x + r.sign() * r.range(0.05, 0.08), sq.z - r.range(0.02, 0.05), r.range(0, TAU), 1, { tol: 0 });
  if (L.stalls)
    for (let k = 0; k < (f === "dwarven" ? 1 : N.stalls); k++) {
      const s = k % 2 ? 1 : -1;
      const x = sq.x + s * r.range(0.09, 0.13), z = sq.z + r.range(-0.04, 0.08);
      plan.try("bld.stall", k % 3, x, z, yawTo(-s, 0) + r.range(-0.3, 0.3), 1, { tol: 0 });
    }
  if (L.fire) {
    plan.add("bld.brazier", 0, sq.x, sq.z, 0, 1.2, { ghost: true });
    out.emit.push({ dx: sq.x, dz: sq.z, dy: 0.07, color: "#ff9a40", size: 0.008, rise: 0.3, count: 6, spread: 0.02 });
    for (const s of [-1, 1]) plan.try("bld.totem", 0, sq.x + s * 0.085, sq.z - 0.02, 0, 0.8, { tol: 0 });
  }
  if (L.braziers)
    for (const s of [-1, 1])
      for (const z of [0.2, 0.42]) {
        if (!walls && z > 0.4) continue;
        plan.add("bld.brazier", 0, s * 0.075, z, 0, 1.1, { ghost: true });
        out.emit.push({ dx: s * 0.075, dz: z, dy: 0.08, color: "#ff5a1a", size: 0.007, rise: 0.3, count: 4, spread: 0.015 });
      }

  // A ring street around the heart of the town (wobbling, a little off-centre), lanes crossing it;
  // buildings line both sides of the ring, turned to face it.
  const hsc = Math.max(0.76, Math.min(0.95, 1.0 - size * 0.02));
  const Rs = Math.max(0.19, Math.min(0.29, Rmax - 0.135 * hsc - 0.035));
  const ring = ringStreet(r, lm.x * 0.3, 0.0, Rs, 0.42, TAU - 0.42);
  plan.lane(ring, 0.014);
  const dirs = [QT, -QT, Math.PI * 0.8 * r.sign()].slice(0, size < 8 ? 2 : 3);
  for (const a of dirs) {
    const a1 = a + r.range(-0.25, 0.25);
    plan.lane(street(r, Math.sin(a1) * (Rs - 0.06), Math.cos(a1) * (Rs - 0.06), a1, r.range(-0.08, 0.08), Rmax + 0.06), 0.018);
  }
  // Halls face the square from its sides; towers stand toward the back.
  const halls = [];
  for (let k = 0; k < N.halls; k++) halls.push({ asset: "bld.hall", variant: (k + r.int(0, 2)) % 3, scale: 0.86 });
  const hallSide = r.sign();
  halls.forEach((h, k) => {
    const s = k % 2 ? -hallSide : hallSide;
    for (let t = 0; t < 12; t++) {
      const x = sq.x + s * r.range(0.17, 0.26), z = sq.z + r.range(-0.08, 0.06);
      if (plan.try(h.asset, h.variant, x, z, yawTo(sq.x - x, 0) + r.range(-0.15, 0.15), h.scale)) {
        h.done = true;
        break;
      }
    }
  });
  for (let k = 0; k < N.towers; k++) plan.scatter(cityTower(k), { rmin: Rs, rmax: Rmax - 0.03, back: -0.05, face: (x, z) => yawTo(-x, -z) });
  const items = halls.filter((h) => !h.done);
  const hv = shuffle(r, [0, 1, 2, 3, 4, 5]);
  for (let k = 0; k < N.houses; k++) items.push({ asset: "bld.house", variant: hv[k % 6], scale: r.range(0.92, 1.04) * hsc });
  const rowOpts = { gap: L.gap, jitter: L.jitter };
  plan.row(ring, 1, items, { ...rowOpts, quota: Math.ceil(items.length * 0.6) });
  plan.row(ring, -1, items, rowOpts);
  plan.row(ring, 1, items, rowOpts);
  // Back rows behind the street fronts, then whatever gaps are left, turned to the ring.
  const back = 0.022 + 0.13 * hsc + 0.012;
  plan.row(ring, 1, items, { ...rowOpts, half: back });
  plan.row(ring, -1, items, { ...rowOpts, half: back });
  const ringFace = (x, z) => {
    let best = ring[0], bd = Infinity;
    for (const p of ring) {
      const d = Math.hypot(p[0] - x, p[1] - z);
      if (d < bd) [bd, best] = [d, p];
    }
    return yawTo(best[0] - x, best[1] - z) + r.range(-0.2, 0.2);
  };
  if (f === "elven") items.splice(Math.ceil(N.houses * 0.75));
  while (items.length) {
      const it = items.shift();
      plan.scatter(it.asset === "bld.house" ? { ...it, scale: it.scale * 0.92 } : it, { rmin: 0.12, face: ringFace, tries: 150 });
    }
  // Trees among an elven town's houses (the vegetation's, when it is loaded).
  if (L.trees) {
    const dark = sub === "dark";
    const kinds = (dark ? ["tree.pine", "tree.dead"] : ["tree.ancient", "tree.oak", "tree.birch"]).filter((k) => hasRecipe(k));
    for (let k = 0; kinds.length && k < 4 + Math.floor(size / 3); k++) {
      const kind = kinds[k % kinds.length];
      const sc = kind === "tree.ancient" ? r.range(0.32, 0.4) : r.range(0.75, 0.95);
      plan.scatter({ asset: kind, variant: r.int(0, 7), scale: sc, pad: -0.04, param: "" }, { rmin: 0.12, rmax: Rmax, tries: 30 });
    }
  }
  if (L.motes) out.emit.push({ dx: lm.x, dz: lm.z, dy: 0.22, color: sub === "dark" ? "#a8b0ff" : "#ffe2a0", size: 0.007, rise: 0.06, count: 12, spread: 0.24 });
  if (L.sparks)
    for (const p of out)
      if (p.asset === "bld.keep") for (const s of [-1, 1]) out.emit.push({ dx: p.x + s * 0.06 * p.scale, dz: p.z - 0.07 * p.scale, dy: 0.31 * p.scale, color: "#ffb050", size: 0.006, rise: 0.35, count: 4, spread: 0.01 });
  if (sub === "arcane") for (const p of out) if (p.asset === "bld.tower" && p.variant === 2) out.emit.push({ dx: p.x, dz: p.z, dy: 0.46, color: "#b89aff", size: 0.009, rise: 0.12, count: 8, spread: 0.05 });
  if (walls) wallRing(out, r, P, Rw, plan.height);
  return out;
}

/** A ring of wall segments of radius R with towers and a gate at +z (none standing in water). */
function wallRing(out, r, P, R, height = null) {
  const wet = (x, z) => !!height && height(x, z) < 0.005;
  const N = Math.max(8, Math.round((TAU * R) / 0.2));
  const D = TAU / N;
  const V = [];
  for (let k = 0; k < N; k++) {
    const a = (k + 0.5) * D;
    const rr = R * (1 + (k === 0 || k === N - 1 ? 0 : r.range(-0.035, 0.02)));
    V.push([Math.sin(a) * rr, Math.cos(a) * rr]);
  }
  for (let k = 0; k < N; k++) {
    const p = V[(k + N - 1) % N], q = V[k];
    const cx = (p[0] + q[0]) / 2, cz = (p[1] + q[1]) / 2;
    const L = Math.hypot(q[0] - p[0], q[1] - p[1]);
    const yaw = Math.atan2(-(q[1] - p[1]), q[0] - p[0]);
    if (wet(cx, cz)) continue;
    if (k === 0) out.push({ asset: "bld.gate", param: P, x: cx, z: cz, yaw, scale: 1, sx: L / 0.2 });
    else out.push({ asset: "bld.wall", param: P, x: cx, z: cz, yaw, scale: 1, sx: (L / 0.2) * 1.02 });
  }
  for (let k = 0; k < N; k++) {
    if (k === 0 || k === N - 1) continue;
    if (k % 2 === 1 && k !== N - 2) continue;
    const [x, z] = V[k];
    if (wet(x, z)) continue;
    out.push({ asset: "bld.walltower", param: P, variant: k % 4 === 0 ? 1 : 0, x, z, yaw: Math.atan2(x, z), scale: 1 });
  }
}

/** Settlement-like improvements (cottage to town, forts, towers, pirate coves, goblin forts) in
 *  the plot owner's family: parts within the plot (radius 0.45), the centre left clear when a unit
 *  stands there. Null for anything else (props.js models farms, mines, lairs, nodes). */
function settlementOf(type, ctx) {
  const t = type.replace("IMPROVEMENT_", "");
  const r = ctx.r;
  let P = paramFor(ctx.family, ctx.civ);
  if (/^(PIRATE_COVE|PIRATE_HARBOR|PIRATE_PORT)$/.test(t)) P = "coastal";
  if (/^(GOBLIN_FORT|GOODY_HUT)$/.test(t)) P = "tribal";
  const { f } = famOf(P);
  const land = !/^PIRATE_/.test(t) && ctx.world?.heightAt;
  const plan = new Plan(r, P, 0.45, land ? (x, z) => ctx.world.heightAt(ctx.x + 0.5 + x, ctx.y + 0.5 - z) : null);
  if (ctx.unit) plan.zone(0, 0.04, 0.14);
  const face = (x, z) => yawTo(-x, -z) + r.range(-0.5, 0.5);
  const houses = (n, sc = 0.92) => {
    const vs = shuffle(r, [0, 1, 2, 3, 4, 5]);
    return Array.from({ length: n }, (_, k) => ({ asset: "bld.house", variant: vs[k % 6], scale: sc * r.range(0.92, 1.05) }));
  };
  const scatter = (items, rmin, rmax) => {
    for (const it of items) plan.scatter(it, { rmin, rmax, face, tries: 80 });
  };
  const ringRow = (items, R) => {
    const ring = ringStreet(r, 0, -0.02, R, 0.5, TAU - 0.5);
    plan.lane(ring, 0.012);
    plan.row(ring, 1, items, { gap: f === "elven" || f === "tribal" ? 0.03 : 0.012, jitter: LAY[f]?.jitter ?? 0.1 });
    plan.row(ring, -1, items, { gap: 0.02, jitter: LAY[f]?.jitter ?? 0.1 });
  };
  switch (t) {
    case "COTTAGE": {
      // One small house and its garden fence.
      const z = ctx.unit ? -0.2 : -0.05, x = r.range(-0.1, 0.1);
      const v = f === "human" ? r.pick([0, 0, 2, 3, 4]) : r.int(0, 5);
      plan.add("bld.house", v, x, z, r.range(-0.35, 0.35), 0.95);
      const s = r.sign();
      plan.add("bld.fence", 0, x + s * 0.06, z + 0.12, r.range(-0.15, 0.15), 0.7, { ghost: true });
      plan.add("bld.fence", 0, x + s * 0.13, z + 0.05, QT + r.range(-0.15, 0.15), 0.6, { ghost: true });
      break;
    }
    case "HAMLET":
      scatter(houses(r.int(2, 3)), 0.1, 0.32);
      plan.add("bld.fence", 0, r.range(-0.15, 0.15), 0.3, r.range(-0.3, 0.3), 0.8, { ghost: true });
      break;
    case "VILLAGE":
    case "ENCLAVE":
      if (!ctx.unit) plan.try("bld.well", 0, r.range(-0.03, 0.03), r.range(-0.02, 0.03), r.range(0, TAU), 1);
      ringRow(houses(r.int(4, 6), 0.88), 0.2);
      if (ctx.unit) plan.try("bld.well", 0, r.range(-0.2, 0.2), 0.3, 0, 1);
      break;
    case "TOWN": {
      if (!ctx.unit) plan.try("bld.well", 0, r.range(-0.03, 0.03), 0.02, r.range(0, TAU), 1);
      const items = [{ asset: "bld.hall", variant: r.int(0, 2), scale: 0.8 }, ...houses(r.int(8, 10), 0.8)];
      ringRow(items, 0.22);
      scatter(items.splice(0), 0.12, 0.42);
      break;
    }
    case "TOWER":
    case "WARNING_POST":
      plan.add("bld.tower", t === "TOWER" ? 0 : 1, r.range(-0.06, 0.06), ctx.unit ? -0.2 : -0.04, r.range(-0.3, 0.3), t === "TOWER" ? 0.9 : 0.75);
      if (t === "TOWER") plan.add("bld.fence", 0, r.range(-0.1, 0.1), 0.18, r.range(-0.3, 0.3), 0.6, { ghost: true });
      break;
    case "FORT":
    case "CASTLE":
    case "CITADEL": {
      // A square of walls with corner towers and a gate at the front; a tower (fort) or a keep in
      // it, at the back when a unit holds the plot.
      const sc = t === "FORT" ? 0.8 : t === "CASTLE" ? 0.9 : 1.0;
      const L = 0.2 * sc, h = L;
      for (let s = 0; s < 4; s++) {
        const yaw = s * QT;
        const nx = Math.sin(yaw), nz = Math.cos(yaw), tx = Math.cos(yaw), tz = -Math.sin(yaw);
        const at2 = (u) => [nx * h + tx * u, nz * h + tz * u];
        if (s === 0) {
          plan.add("bld.gate", 0, ...at2(0), yaw, sc, { ghost: true });
          for (const u of [-0.75 * L, 0.75 * L]) plan.add("bld.wall", 1, ...at2(u), yaw, sc, { ghost: true });
        } else for (const u of [-L / 2, L / 2]) plan.add("bld.wall", 0, ...at2(u), yaw, sc, { ghost: true });
        const [cx, cz] = [nx * h + tx * h, nz * h + tz * h];
        plan.add("bld.walltower", t === "FORT" ? 0 : 1, cx, cz, Math.atan2(cx, cz), sc, { ghost: true });
      }
      const z = ctx.unit ? -0.08 : -0.03;
      if (t === "FORT") plan.add("bld.tower", 1, 0, z - 0.02, 0, 0.7);
      else plan.add("bld.keep", 0, 0, z - 0.03, 0, t === "CASTLE" ? 0.5 : 0.6);
      break;
    }
    case "PIRATE_COVE":
    case "PIRATE_HARBOR":
    case "PIRATE_PORT": {
      const n = t === "PIRATE_COVE" ? 2 : t === "PIRATE_HARBOR" ? 3 : 5;
      const items = [{ asset: "bld.tower", variant: t === "PIRATE_PORT" ? 0 : 1, scale: 0.8 }, ...Array.from({ length: n }, (_, k) => ({ asset: "bld.house", variant: [0, 4, 3, 1, 2][k], scale: 0.9 }))];
      scatter(items, 0.08, 0.4);
      break;
    }
    case "GOBLIN_FORT": {
      // A ring of palisade around crude huts and a totem.
      const R = 0.3, n = 10;
      for (let k = 0; k < n; k++) {
        const a = (k / n) * TAU;
        plan.add(k === 0 ? "bld.gate" : "bld.wall", 0, Math.sin(a) * R, Math.cos(a) * R, a, 0.92, { ghost: true });
      }
      plan.add("bld.totem", 0, r.range(-0.05, 0.05), ctx.unit ? -0.15 : 0.0, 0, 1);
      scatter(houses(3, 0.85), 0.1, 0.24);
      break;
    }
    case "GOODY_HUT":
      scatter(houses(2, 0.9), 0.08, 0.28);
      plan.scatter({ asset: "bld.totem", variant: 0, scale: 0.9 }, { rmin: 0.05, rmax: 0.25, tries: 30 });
      break;
    default:
      return null;
  }
  const water = /^PIRATE_/.test(t);
  return plan.out.map((p) => ({ asset: p.asset, param: p.param, variant: p.variant, dx: p.x, dz: p.z, yaw: p.yaw, scale: p.scale, float: water }));
}

extendCatalog({
  family: (civ) => familyOf(civ),
  city: (ctx) => cityLayout(ctx),
  settlement: (type, ctx) => settlementOf(type, ctx),
});

