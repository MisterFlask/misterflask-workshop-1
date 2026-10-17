// units.js: the figures of the 3D map (docs/R3D.md "Asset recipes"): one humanoid generator that
// composes a body, head, armour, clothing, weapons and gear from a parameter set (races by the
// recipe param), the archetype recipes built on it (unit.warrior ... unit.hero), the creatures
// (wolf, bear, spider, dragon, demon, golem, elemental, skeleton, serpent, beast, angel, bird), the
// mounts and vehicles (cavalry, settler's wagon), the ships and engines; and the catalog entry that
// maps the game's ~325 unit types to them.
//
// Conventions (mesh.js, registry.js): y up, one unit = one plot, a figure stands on y = 0 facing +z
// (toward the camera). Its right hand is on -x (screen left, the sun's side), its left on +x. A
// human's head top is ~0.48 (helmets ~0.5). Owner colour is MAT.TEAM with light-grey albedo: every
// figure carries a large team-coloured area (tabard, shield face, hood, cape, sail, caparison,
// collar or war paint). Parts carry their BONE so the vertex shader breathes and sways them.
//
// Params: the race, "human" | "elf" | "dwarf" | "orc" | "goblin" | "lizard" | "undead" | "demon",
// optionally with "-dark" (darker garb: Sheaim, Calabim, Svartalfar, the dark faiths) -- e.g.
// "elf-dark" -- or a mount override for cavalry ("orc-boar", "human-wolf"). Creatures take their
// kind as the param (unit.beast "lion", unit.golem "iron", unit.elemental "water", ...).

import { recipe } from "./registry.js";
import { extendCatalog } from "./catalog.js";
import { MeshBuilder, MAT, BONE } from "../mesh.js";
import { pal, jitter, mixc } from "./palette.js";
import { noise3 } from "../noise.js";
import { vadd, vsub, vscale, vdot, vcross, vlen, vnorm, vlerp, mat4 } from "../math.js";

// ---------------------------------------------------------------------------------------------
// Small helpers

/** This module's recipes (so one recipe can compose another's mesh). */
const RECIPE_DEFS = new Map();
function unitRecipe(name, def) {
  RECIPE_DEFS.set(name, def);
  recipe(name, def);
}

/** Builds another recipe's mesh into `sub` (composition without the registry cache). */
function buildAsset_(name, sub, r, ctx) {
  const def = RECIPE_DEFS.get(name);
  def?.build(sub, r, { name, ...ctx });
}

const TAU = Math.PI * 2;
const R_SIDE = -1; // the figure's right hand (-x)
const L_SIDE = 1;

/** A linear colour from a palette name, "#hex" or a linear triple. */
function C(c) {
  return Array.isArray(c) ? c : pal(c);
}

/** Team-coloured cloth: `v` 0..1 is how much of the owner's colour shows (light grey = full). */
function teamCol(v = 1) {
  const g = Math.round(0x50 + (0xdc - 0x50) * v * 0.92).toString(16).padStart(2, "0");
  return `#${g}${g}${g}`;
}

function setTeam(mb, v = 1) {
  mb.mat = MAT.TEAM;
  mb.color = teamCol(v);
}

function setMat(mb, color, mat = MAT.MATTE) {
  mb.mat = mat;
  mb.color = C(color);
}

/** Sets the transform so local +y runs along `ydir` and local +z toward `zhint`, origin `o`. */
function orient(mb, o, ydir, zhint = [0, 0, 1]) {
  const Y = vnorm(ydir);
  let Z = vsub(zhint, vscale(Y, vdot(zhint, Y)));
  if (vlen(Z) < 1e-5) {
    const alt = Math.abs(Y[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0];
    Z = vsub(alt, vscale(Y, vdot(alt, Y)));
  }
  Z = vnorm(Z);
  const X = vcross(Y, Z);
  const t = mat4();
  t[0] = X[0]; t[1] = X[1]; t[2] = X[2];
  t[4] = Y[0]; t[5] = Y[1]; t[6] = Y[2];
  t[8] = Z[0]; t[9] = Z[1]; t[10] = Z[2];
  t[12] = o[0]; t[13] = o[1]; t[14] = o[2];
  return mb.apply(t);
}

/** Two-bone IK: the joint between `a` and the target `t` (bone lengths l1, l2), bent toward
 *  `pole`. Returns {mid, end} (end pulled in when out of reach). */
function ik(a, t, l1, l2, pole) {
  let d = vsub(t, a);
  let dl = vlen(d);
  const max = (l1 + l2) * 0.995;
  if (dl > max) {
    t = vadd(a, vscale(d, max / dl));
    d = vsub(t, a);
    dl = max;
  }
  dl = Math.max(dl, 1e-4);
  const dir = vscale(d, 1 / dl);
  const x = (l1 * l1 - l2 * l2 + dl * dl) / (2 * dl);
  const h = Math.sqrt(Math.max(0, l1 * l1 - x * x));
  let p = vsub(pole, vscale(dir, vdot(pole, dir)));
  if (vlen(p) < 1e-5) p = Math.abs(dir[1]) < 0.9 ? [0, 1, 0] : [1, 0, 0];
  p = vnorm(p);
  return { mid: vadd(vadd(a, vscale(dir, x)), vscale(p, h)), end: t };
}

/** A flat shape: polygon `poly` ([[x, y], ...], star-shaped about its centroid) in the local xy
 *  plane, `thick` deep along z (centred), with its rim. */
function slab(mb, poly, thick, { bulge = 0 } = {}) {
  const n = poly.length;
  const cx = poly.reduce((a, p) => a + p[0], 0) / n;
  const cy = poly.reduce((a, p) => a + p[1], 0) / n;
  const z = thick / 2;
  const cf = [cx, cy, z + bulge], cb = [cx, cy, -z - bulge];
  for (let i = 0; i < n; i++) {
    const a = poly[i], b = poly[(i + 1) % n];
    mb.tri(cf, [a[0], a[1], z], [b[0], b[1], z]);
    mb.tri(cb, [b[0], b[1], -z], [a[0], a[1], -z]);
    mb.quad([a[0], a[1], -z], [b[0], b[1], -z], [b[0], b[1], z], [a[0], a[1], z]);
  }
  return mb;
}

/** A closed loop tube (collars, rims, belts) of radius `rad` through points of an ellipse. */
function ringTube(mb, rx, rz, rad, seg = 12, sides = 5, { y = 0, wob = 0, seed = 0 } = {}) {
  const pts = [];
  for (let i = 0; i <= seg; i++) {
    const a = (i / seg) * TAU;
    const w = wob ? 1 + wob * noise3(Math.cos(a) * 2 + seed, Math.sin(a) * 2, seed) : 1;
    pts.push([Math.cos(a) * rx * w, y + (wob ? wob * 0.3 * Math.sin(a * 3 + seed) * rad : 0), Math.sin(a) * rz * w]);
  }
  pts[seg] = pts[0];
  return mb.tube(pts, rad, sides);
}

/** A small sphere at `p`. */
function ball(mb, p, rad, detail = 0, sq = [1, 1, 1]) {
  return mb.part(() => {
    mb.translate(p[0], p[1], p[2]);
    mb.scale(sq[0], sq[1], sq[2]);
    mb.sphere(rad, detail);
  });
}

/** A horn or spike: a curved tapering tube from `p0` along `dir`, curling by `curl` (vector added
 *  progressively), `n` points. */
function horn(mb, p0, dir, len, r0, curl = [0, 0, 0], n = 5, sides = 5) {
  const pts = [], rad = [];
  let p = p0, d = vnorm(dir);
  for (let i = 0; i < n; i++) {
    pts.push(p);
    rad.push(r0 * (1 - i / (n - 1)) + 0.0006);
    d = vnorm(vadd(d, vscale(curl, 1 / (n - 1))));
    p = vadd(p, vscale(d, len / (n - 1)));
  }
  return mb.tube(pts, rad, sides, { caps: true });
}

/** Runs `f` on a fresh builder (carrying `mb`'s material state) and appends the result through
 *  `mb`'s transform: per-part edits (displace, sway) then work in the part's own coordinates. */
function local(mb, f) {
  const sub = new MeshBuilder();
  sub._color = mb._color;
  sub.mat = mb.mat;
  sub.sway = mb.sway;
  sub.bone = mb.bone;
  sub.ao = mb.ao;
  f(sub);
  mb.append(sub);
  return mb;
}

/** Variant-dependent pick without consuming randomness. */
function vpick(variant, list) {
  return list[((variant % list.length) + list.length) % list.length];
}

/** Splits a recipe param "race-mod" -> {race, dark, mod}. */
function raceOf(param) {
  const [race, mod] = String(param || "human").split("-");
  return { race: RACES[race] ? race : "human", dark: mod === "dark", mod: mod ?? "" };
}

/** Appends `sub` (a MeshBuilder) at offset `p`, rotated `yaw`, scaled `s`. */
function place(mb, sub, p = [0, 0, 0], yaw = 0, s = 1) {
  return mb.part(() => {
    mb.translate(p[0], p[1], p[2]);
    if (yaw) mb.rotateY(yaw);
    if (s !== 1) mb.scale(s);
    mb.append(sub);
  });
}

// ---------------------------------------------------------------------------------------------
// Races: proportions of a figure standing straight (head top ~ neckY + 2.1 headR), skin, garb

const RACES = {
  human: { hipY: 0.225, shY: 0.37, neckY: 0.386, headR: 0.044, shW: 0.07, hipW: 0.03, chestR: 0.056, waistR: 0.045, depth: 0.74, armR: 0.0155, legR: 0.0195, handR: 0.0175, foot: 0.04, lean: 0.03, arm: 1, skin: "skinHuman" },
  elf: { hipY: 0.258, shY: 0.405, neckY: 0.423, headR: 0.041, shW: 0.06, hipW: 0.026, chestR: 0.046, waistR: 0.035, depth: 0.72, armR: 0.0122, legR: 0.0152, handR: 0.0145, foot: 0.036, lean: 0, arm: 1, skin: "skinElf" },
  dwarf: { hipY: 0.13, shY: 0.262, neckY: 0.272, headR: 0.047, shW: 0.078, hipW: 0.035, chestR: 0.069, waistR: 0.065, depth: 0.86, armR: 0.021, legR: 0.025, handR: 0.0205, foot: 0.042, lean: 0.03, arm: 1.12, skin: "skinDwarf" },
  orc: { hipY: 0.235, shY: 0.392, neckY: 0.386, headR: 0.045, shW: 0.094, hipW: 0.036, chestR: 0.074, waistR: 0.056, depth: 0.8, armR: 0.024, legR: 0.026, handR: 0.024, foot: 0.045, lean: 0.2, arm: 1.15, skin: "skinOrc" },
  goblin: { hipY: 0.112, shY: 0.205, neckY: 0.212, headR: 0.05, shW: 0.045, hipW: 0.022, chestR: 0.037, waistR: 0.034, depth: 0.82, armR: 0.0105, legR: 0.0125, handR: 0.0145, foot: 0.032, lean: 0.16, arm: 1.2, skin: "#6f7b3e" },
  lizard: { hipY: 0.232, shY: 0.375, neckY: 0.39, headR: 0.039, shW: 0.066, hipW: 0.03, chestR: 0.054, waistR: 0.043, depth: 0.84, armR: 0.0165, legR: 0.0215, handR: 0.0165, foot: 0.046, lean: 0.2, arm: 1.08, skin: "#557050" },
  undead: { hipY: 0.225, shY: 0.37, neckY: 0.386, headR: 0.04, shW: 0.064, hipW: 0.027, chestR: 0.047, waistR: 0.016, depth: 0.76, armR: 0.0078, legR: 0.0092, handR: 0.0125, foot: 0.032, lean: 0.07, arm: 1.02, skin: "bone" },
  demon: { hipY: 0.248, shY: 0.405, neckY: 0.414, headR: 0.043, shW: 0.09, hipW: 0.032, chestR: 0.07, waistR: 0.046, depth: 0.78, armR: 0.022, legR: 0.0235, handR: 0.0215, foot: 0.044, lean: 0.12, arm: 1.12, skin: "skinDemon" },
};

/** Each race's materials: cloth, leather, metal, trim, wood, legs and boots; hair colours. */
const GARB = {
  human: { cloth: "#b8a888", leather: "leather", leather2: "leatherDark", metal: "steel", metal2: "iron", trim: "gold", wood: "timber", legs: "#4e4234", boots: "#3a2a1e", fur: "fur", hair: ["#3a2a1e", "#5a3a22", "#2a2220", "#7a5a32", "#4a3a2a"] },
  dark: { cloth: "#4a3e4a", leather: "#3a2c30", leather2: "#241c20", metal: "#5e6068", metal2: "#3c3c44", trim: "#8a7aa0", wood: "#3e2e28", legs: "#2c2428", boots: "#1e1618", fur: "#3a3234", hair: ["#1a1618", "#2a2024", "#c8c0b8"] },
  elf: { cloth: "#b8b08a", leather: "#5e6038", leather2: "#3c4228", metal: "#b09a62", metal2: "#8a7a50", trim: "gold", wood: "#7a5e3a", legs: "#3e4a30", boots: "#4a3a26", fur: "#8a7a5a", hair: ["#d8c088", "#e0d0a8", "#a87a42", "#c8b890"] },
  elfdark: { cloth: "#4a4458", leather: "#2e2a38", leather2: "#1e1c26", metal: "#6a6a80", metal2: "#4a4a5a", trim: "#a8a0c8", wood: "#3a3036", legs: "#262430", boots: "#1a1820", fur: "#3a3640", hair: ["#e8e4f0", "#c8c4d8", "#2a2830"] },
  dwarf: { cloth: "#7a6248", leather: "#5a3a26", leather2: "#3a2618", metal: "#767a82", metal2: "#54585e", trim: "bronze", wood: "#5a3e28", legs: "#4a3a2e", boots: "#2e2018", fur: "#6a5640", hair: ["#9a4a22", "#5a3820", "#8a8a88", "#c8a050", "#2e2420"] },
  orc: { cloth: "#6a5a40", leather: "#4c3626", leather2: "#2e2018", metal: "#55504a", metal2: "#3e3a36", trim: "#8a4a2a", wood: "#4e3a28", legs: "#3e3226", boots: "#2a1e16", fur: "#5a4a3a", hair: ["#1a1614", "#2a2420"] },
  lizard: { cloth: "#7a6a44", leather: "#5a4630", leather2: "#3a2c1e", metal: "bronze", metal2: "#6a5034", trim: "#d0b070", wood: "#6a5034", legs: "#4a3c2a", boots: "#3a2c1e", fur: "#6a5a40", hair: ["#2a3a2a"] },
  undead: { cloth: "#4a4640", leather: "#3a3430", leather2: "#262220", metal: "#6a625a", metal2: "#4a443e", trim: "#7a6a4a", wood: "#3e342c", legs: "#2e2a26", boots: "#221e1c", fur: "#4a4440", hair: ["#8a8478"] },
  demon: { cloth: "#3a1c1c", leather: "#2a1a18", leather2: "#1a1010", metal: "#3a3436", metal2: "#26222a", trim: "#9a3a1a", wood: "#2e2220", legs: "#221616", boots: "#160e0e", fur: "#2a1e1e", hair: ["#140c0c"] },
};

// Undyed outdoor wool (hunters' kilts, scouts' cloaks) and the deep dye of a mage's hat, per garb:
// the owner's colour goes on one or two big readable zones, the rest of a figure stays natural
// (leather, wool, steel, skin) so it reads as a character, not a coloured pawn.
const FIELD = { human: ["#5a5236", "#2e3044"], dark: ["#2e2a30", "#1e1a26"], elf: ["#4a5434", "#2c3a2c"], elfdark: ["#2a2834", "#1a1824"], dwarf: ["#5e4630", "#3a2c24"], orc: ["#4a3e2c", "#2e2620"], lizard: ["#5a4c30", "#2e3a2a"], undead: ["#3a3632", "#24202a"], demon: ["#2a1616", "#1a0e10"] };
for (const [k, [field, hat]] of Object.entries(FIELD)) Object.assign(GARB[k], { field, hat });

function garbOf(race, dark) {
  if (dark) return race === "elf" ? GARB.elfdark : race === "human" || race === "dwarf" ? GARB.dark : GARB[race] ?? GARB.dark;
  return GARB[race === "goblin" ? "orc" : race] ?? GARB.human;
}

/** Glow colours (staff tips, symbols) by race. */
const GLOW = { human: "#9ad0ff", elf: "#b8ffb0", dwarf: "#ffc070", orc: "#ff7a3a", goblin: "#c0ff60", lizard: "#7affd0", undead: "#9a7aff", demon: "#ff5020" };

// ---------------------------------------------------------------------------------------------
// Equipment, drawn in a local frame: origin at the grip, +y toward the business end

function bladeShape(len, w, tip = 0.3) {
  return [[-w / 2, 0], [w / 2, 0], [w / 2, len * (1 - tip)], [0, len], [-w / 2, len * (1 - tip)]];
}

const WEAPONS = {
  sword(mb, g, o = {}) {
    const len = o.len ?? 0.15, w = o.w ?? 0.016;
    setMat(mb, g.leather2);
    mb.part(() => {
      mb.translate(0, -0.026, 0);
      mb.cylinder(0.0055, 0.0055, 0.04, 5);
    });
    setMat(mb, g.trim, MAT.METAL);
    ball(mb, [0, -0.03, 0], 0.008);
    mb.part(() => {
      mb.translate(0, 0.012, 0);
      mb.box(0.048, 0.007, 0.01);
    });
    setMat(mb, o.metal ?? g.metal, MAT.METAL);
    mb.part(() => {
      mb.translate(0, 0.016, 0);
      slab(mb, bladeShape(len, w, 0.18), 0.0045, { bulge: 0.0015 });
    });
  },
  axe(mb, g, o = {}) {
    const len = o.len ?? 0.2;
    setMat(mb, g.wood);
    mb.part(() => {
      mb.translate(0, -0.05, 0);
      mb.cylinder(0.0065, 0.006, len + 0.05, 5);
    });
    setMat(mb, o.metal ?? g.metal, MAT.METAL);
    const s = o.head ?? 1;
    mb.part(() => {
      mb.translate(0, len - 0.035 * s, 0);
      mb.scale(s);
      // A bearded crescent blade on one side, a spike or second blade on the other.
      slab(mb, [[0, -0.012], [0.03, -0.03], [0.058, -0.04], [0.064, 0.0], [0.06, 0.04], [0.03, 0.03], [0, 0.016]], 0.006, { bulge: 0.002 });
      if (o.double) slab(mb, [[0, 0.016], [-0.03, 0.03], [-0.058, 0.04], [-0.064, 0.0], [-0.058, -0.04], [-0.03, -0.03], [0, -0.012]], 0.006, { bulge: 0.002 });
      else slab(mb, [[0, -0.01], [-0.03, -0.004], [-0.03, 0.004], [0, 0.012]], 0.007);
    });
  },
  club(mb, g, o = {}) {
    setMat(mb, o.color ?? g.wood);
    const len = o.len ?? 0.17;
    mb.part(() => {
      mb.translate(0, -0.035, 0);
      mb.lathe([[0.007, 0], [0.008, 0.06], [0.016, len * 0.75], [0.022, len], [0.012, len + 0.02], [0, len + 0.024]], 6, { wobble: (a, y) => 0.15 * Math.sin(a * 3 + y * 60) });
    });
    if (o.spikes !== false) {
      setMat(mb, g.metal2, MAT.METAL);
      for (let i = 0; i < 4; i++) {
        const a = i * 1.7 + 0.4, y = len * (0.72 + 0.07 * i) - 0.035;
        horn(mb, [Math.cos(a) * 0.016, y, Math.sin(a) * 0.016], [Math.cos(a), 0.3, Math.sin(a)], 0.018, 0.004, [0, 0, 0], 2, 4);
      }
    }
  },
  mace(mb, g, o = {}) {
    setMat(mb, g.wood);
    mb.part(() => {
      mb.translate(0, -0.035, 0);
      mb.cylinder(0.006, 0.006, 0.17, 5);
    });
    setMat(mb, o.metal ?? g.metal, MAT.METAL);
    mb.part(() => {
      mb.translate(0, 0.14, 0);
      mb.lathe([[0.004, -0.02], [0.022, -0.004], [0.026, 0.012], [0.016, 0.03], [0, 0.036]], 6);
    });
  },
  hammer(mb, g, o = {}) {
    setMat(mb, g.wood);
    const len = o.len ?? 0.2;
    mb.part(() => {
      mb.translate(0, -0.04, 0);
      mb.cylinder(0.0065, 0.0065, len + 0.04, 5);
    });
    setMat(mb, o.metal ?? g.metal, MAT.METAL);
    mb.part(() => {
      mb.translate(0, len - 0.03, 0);
      mb.box(0.075, 0.042, 0.042, { top: 0.92 });
    });
    setMat(mb, g.trim, MAT.METAL);
    mb.part(() => {
      mb.translate(0, len - 0.014, 0);
      mb.box(0.078, 0.008, 0.045);
    });
  },
  spear(mb, g, o = {}) {
    const len = o.len ?? 0.4, back = o.back ?? 0.12;
    setMat(mb, g.wood);
    mb.part(() => {
      mb.translate(0, -back, 0);
      mb.cylinder(0.0055, 0.005, len + back, 5);
    });
    setMat(mb, o.metal ?? g.metal, MAT.METAL);
    mb.part(() => {
      mb.translate(0, len - 0.004, 0);
      slab(mb, [[-0.003, 0], [0.003, 0], [0.013, 0.025], [0, 0.07], [-0.013, 0.025]], 0.005, { bulge: 0.002 });
    });
    if (o.pennon) {
      setTeam(mb, 0.95);
      mb.sway = 0.8;
      mb.card([0, len - 0.03, 0], [0, len - 0.075, 0], [-0.075, len - 0.065, 0.004], [-0.09, len - 0.045, 0.006]);
      mb.sway = 0;
    }
  },
  lance(mb, g, o = {}) {
    const len = o.len ?? 0.4;
    setTeam(mb, 0.85);
    mb.part(() => {
      mb.translate(0, -0.08, 0);
      mb.lathe([[0.006, 0], [0.007, 0.06], [0.014, 0.075], [0.011, 0.12], [0.004, len], [0, len + 0.03]], 6);
    });
    setMat(mb, o.metal ?? g.metal, MAT.METAL);
    mb.part(() => {
      mb.translate(0, -0.012, 0);
      mb.cone(0.028, 0.035, 8);
    });
    setTeam(mb, 0.95);
    mb.sway = 0.9;
    mb.card([0, len - 0.12, 0], [0, len - 0.17, 0], [-0.085, len - 0.16, 0.006], [-0.1, len - 0.13, 0.008]);
    mb.sway = 0;
  },
  staff(mb, g, o = {}) {
    const len = o.len ?? 0.3, back = o.back ?? 0.16;
    setMat(mb, o.wood ?? g.wood);
    const pts = [[0, -back, 0], [0.002, 0, 0], [-0.002, len * 0.6, 0.002], [0.003, len, 0]];
    mb.tube(pts, [0.0065, 0.007, 0.007, 0.0085], 5, { caps: true });
    if (o.plain) return;
    // The head: a crook of three prongs holding a crystal (the staff's light).
    setMat(mb, o.prong ?? g.metal2, MAT.METAL);
    for (let i = 0; i < 3; i++) {
      const a = (i / 3) * TAU + 0.5;
      horn(mb, [0.003, len - 0.004, 0], [Math.cos(a) * 0.6, 1, Math.sin(a) * 0.6], 0.04, 0.0045, [-Math.cos(a) * 1.2, 0.2, -Math.sin(a) * 1.2], 4, 4);
    }
    mb.mat = MAT.EMISSIVE;
    mb.color = C(o.glow ?? "#9ad0ff");
    mb.part(() => {
      mb.translate(0.003, len + 0.026, 0);
      mb.scale(0.8, 1.35, 0.8);
      mb.sphere(o.gem ?? 0.015, 1);
    });
  },
  bow(mb, g, o = {}) {
    // A vertical arc in the local yz plane: the grip at the origin, the limbs curving toward +z (the
    // archer's side), the string drawn back to `draw` behind the grip, the arrow toward -z.
    const h = o.len ?? 0.34, depth = o.depth ?? 0.045, draw = o.draw ?? 0.13;
    const pts = [], rad = [];
    const n = 9;
    for (let i = 0; i < n; i++) {
      const t = i / (n - 1) - 0.5;
      const rc = o.recurve && Math.abs(t) > 0.36 ? -(Math.abs(t) - 0.36) * 0.35 : 0;
      pts.push([0, t * h, depth * (1 - Math.cos(t * Math.PI)) + rc]);
      rad.push(0.0062 * (1 - Math.abs(t) * 1.2) + 0.0018);
    }
    setMat(mb, o.wood ?? g.wood);
    mb.tube(pts, rad, 5, { caps: true });
    setMat(mb, "#d8d0b8");
    const top = pts[n - 1], bot = pts[0];
    const nock = [0, 0, draw];
    mb.tube([top, nock], 0.0013, 3);
    mb.tube([nock, bot], 0.0013, 3);
    if (o.arrow !== false) {
      setMat(mb, "#a08a6a");
      mb.tube([nock, [0, 0, -0.035]], 0.0022, 3);
      setMat(mb, "#e8e0d0");
      mb.part(() => {
        mb.translate(0, 0, draw - 0.012);
        mb.card([0, 0, -0.018], [0, 0.007, -0.006], [0, 0.007, 0.012], [0, 0, 0.012]);
      });
      setMat(mb, g.metal2, MAT.METAL);
      mb.part(() => {
        mb.translate(0, 0, -0.035);
        mb.rotateX(-Math.PI / 2);
        mb.cone(0.0055, 0.018, 4);
      });
    }
  },
  pick(mb, g) {
    setMat(mb, g.wood);
    mb.part(() => {
      mb.translate(0, -0.04, 0);
      mb.cylinder(0.006, 0.0055, 0.22, 5);
    });
    setMat(mb, g.metal2, MAT.METAL);
    mb.tube([[-0.06, 0.15, 0], [-0.03, 0.172, 0], [0, 0.178, 0], [0.03, 0.172, 0], [0.06, 0.15, 0]], [0.0015, 0.0065, 0.009, 0.0065, 0.0015], 5, { caps: true });
  },
  hoe(mb, g) {
    setMat(mb, g.wood);
    mb.part(() => {
      mb.translate(0, -0.1, 0);
      mb.cylinder(0.0055, 0.005, 0.32, 5);
    });
    setMat(mb, g.metal2, MAT.METAL);
    mb.part(() => {
      mb.translate(0, 0.215, 0.005);
      mb.rotateX(1.25);
      mb.box(0.04, 0.045, 0.004, { top: 0.85 });
    });
  },
  scythe(mb, g) {
    setMat(mb, g.wood);
    mb.tube([[0, -0.12, 0], [0.004, 0.08, 0], [0, 0.24, 0.004]], 0.0055, 5, { caps: true });
    setMat(mb, g.metal2, MAT.METAL);
    mb.part(() => {
      mb.translate(0, 0.235, 0);
      slab(mb, [[0, -0.006], [0.05, -0.01], [0.1, -0.03], [0.04, 0.008], [0, 0.008]], 0.003);
    });
  },
  dagger(mb, g) {
    WEAPONS.sword(mb, g, { len: 0.06, w: 0.012 });
  },
  torch(mb, g) {
    setMat(mb, g.wood);
    mb.part(() => {
      mb.translate(0, -0.04, 0);
      mb.cylinder(0.005, 0.007, 0.12, 5);
    });
    mb.mat = MAT.EMISSIVE;
    mb.color = C("fire");
    mb.part(() => {
      mb.translate(0, 0.085, 0);
      mb.lathe([[0.012, 0], [0.014, 0.012], [0.006, 0.03], [0, 0.045]], 5);
    });
  },
  censer(mb, g, o = {}) {
    // Hangs from the hand on a chain (local -y is down when the hand is level).
    setMat(mb, g.metal2, MAT.METAL);
    mb.tube([[0, 0, 0], [0.002, -0.035, 0.004], [0.004, -0.065, 0.01]], 0.0015, 3);
    setMat(mb, g.trim, MAT.METAL);
    mb.part(() => {
      mb.translate(0.004, -0.09, 0.01);
      mb.lathe([[0, -0.012], [0.016, -0.004], [0.017, 0.006], [0.01, 0.014], [0.003, 0.024]], 7);
    });
    mb.mat = MAT.EMISSIVE;
    mb.color = C(o.glow ?? "holy");
    ball(mb, [0.004, -0.082, 0.01], 0.0075, 0);
  },
  symbol(mb, g, o = {}) {
    // A holy standard: a short pole topped by a sun disc with rays (soft glow at its heart).
    setMat(mb, g.wood);
    mb.part(() => {
      mb.translate(0, -0.06, 0);
      mb.cylinder(0.005, 0.005, 0.2, 5);
    });
    setMat(mb, o.metal ?? g.trim, MAT.METAL);
    mb.part(() => {
      mb.translate(0, 0.175, 0);
      const poly = [];
      for (let i = 0; i < 12; i++) {
        const a = (i / 12) * TAU + Math.PI / 2;
        const rr = i % 2 ? 0.024 : 0.04;
        poly.push([Math.cos(a) * rr, Math.sin(a) * rr]);
      }
      slab(mb, poly, 0.005);
    });
    mb.mat = MAT.EMISSIVE;
    mb.color = C(o.glow ?? "holy");
    mb.part(() => {
      mb.translate(0, 0.175, 0);
      mb.scale(1, 1, 0.45);
      mb.sphere(0.0125, 0);
    });
  },
  banner(mb, g, o = {}) {
    const len = o.len ?? 0.42;
    setMat(mb, g.wood);
    mb.part(() => {
      mb.translate(0, -0.12, 0);
      mb.cylinder(0.0055, 0.005, len + 0.12, 5);
    });
    setMat(mb, g.trim, MAT.METAL);
    ball(mb, [0, len + 0.008, 0], 0.009);
    // The flag: a hanging cloth in the owner's colours with a darker band, rippling in the wind
    // (sway grows away from the pole).
    const W = o.w ?? 0.085, Hh = o.h ?? 0.13, nu = 4, nv = 4;
    local(mb, (k) => {
      for (let i = 0; i < nu; i++) {
        for (let j = 0; j < nv; j++) {
          const P = (u, v) => [-u * W, len - 0.012 - v * Hh * (1 - 0.25 * u * v), Math.sin(u * 3.2 + v) * 0.008];
          const u0 = i / nu, u1 = (i + 1) / nu, v0 = j / nv, v1 = (j + 1) / nv;
          k.mat = MAT.TEAM;
          k.color = teamCol(j === 1 ? 0.5 : 0.95);
          k.quad(P(u0, v0), P(u1, v0), P(u1, v1), P(u0, v1));
        }
      }
      const v = k.v;
      for (let i = 0; i < v.length; i += 13) v[i + 10] = Math.min(1, 0.15 + 8 * Math.abs(v[i]));
    });
  },
  sling(mb, g) {
    setMat(mb, g.leather);
    mb.tube([[0, 0, 0], [0.01, -0.05, 0.01], [0.004, -0.09, 0.02]], 0.0018, 3);
    setMat(mb, "stone");
    ball(mb, [0.004, -0.095, 0.02], 0.009);
  },
  crossbow(mb, g) {
    setMat(mb, g.wood);
    mb.part(() => {
      mb.translate(0, -0.04, 0);
      mb.box(0.012, 0.15, 0.016);
    });
    setMat(mb, g.metal2, MAT.METAL);
    mb.tube([[-0.06, 0.09, -0.02], [0, 0.1, 0.004], [0.06, 0.09, -0.02]], [0.002, 0.0045, 0.002], 4, { caps: true });
  },
};

/** Shields, in a frame whose +z is the face's outward normal and +y its top. */
function shield(mb, g, shape, o = {}) {
  const face = o.face ?? 0.98;
  if (shape === "round" || shape === "buckler") {
    const rr = o.r ?? (shape === "buckler" ? 0.036 : 0.06);
    setTeam(mb, face);
    mb.part(() => {
      mb.rotateX(Math.PI / 2);
      mb.lathe([[rr, -0.004], [rr, 0.002], [rr * 0.6, 0.008], [0, 0.011]], 10);
    });
    // A painted device: a darker band across, then the rim and the boss.
    if (o.band !== false) {
      setTeam(mb, 0.45);
      mb.part(() => {
        mb.translate(0, 0, 0.0088);
        mb.rotateZ(o.bandAngle ?? 0.7);
        mb.box(rr * 1.7, 0.0025, rr * 0.36);
      });
    }
    setMat(mb, o.rim ?? g.metal2, MAT.METAL);
    mb.part(() => ringTube(mb, rr, rr, 0.0045, 10, 3, {}));
    setMat(mb, o.boss ?? g.metal, MAT.METAL);
    ball(mb, [0, 0, 0.01], rr * 0.24, 0, [1, 1, 0.7]);
    return;
  }
  if (shape === "kite" || shape === "heater") {
    const w = o.w ?? 0.072, h = o.h ?? 0.13;
    const poly = shape === "kite"
      ? [[0, -h * 0.62], [w * 0.42, -h * 0.22], [w * 0.5, h * 0.12], [w * 0.38, h * 0.34], [0, h * 0.38], [-w * 0.38, h * 0.34], [-w * 0.5, h * 0.12], [-w * 0.42, -h * 0.22]]
      : [[0, -h * 0.5], [w * 0.36, -h * 0.28], [w * 0.5, h * 0.05], [w * 0.5, h * 0.4], [0, h * 0.44], [-w * 0.5, h * 0.4], [-w * 0.5, h * 0.05], [-w * 0.36, -h * 0.28]];
    local(mb, (k) => {
      setTeam(k, face);
      slab(k, poly, 0.007, { bulge: 0.005 });
      // A pale cross device.
      if (o.device !== "none") {
        setTeam(k, 0.4);
        k.part(() => {
          k.translate(0, -h * 0.3, 0.0095);
          k.box(w * 0.2, h * 0.62, 0.002);
          k.translate(0, h * 0.42, 0);
          k.rotateZ(Math.PI / 2);
          k.translate(-w * 0.09, 0, 0);
          k.box(w * 0.18, w * 0.78, 0.002);
        });
      }
      // Curved across: bend the face around a vertical axis.
      k.displace(0, (x) => [0, 0, -x * x * 5]);
    });
    setMat(mb, o.rim ?? g.trim, MAT.METAL);
    const rim = poly.map(([x, y]) => [x, y, 0.004 - x * x * 5]);
    rim.push(rim[0]);
    mb.tube(rim, 0.0028, 3);
    return;
  }
  if (shape === "tower") {
    const w = o.w ?? 0.09, h = o.h ?? 0.18;
    const n = 4, R0 = 0.09;
    const span = w / R0;
    setTeam(mb, face);
    for (let i = 0; i < n; i++) {
      const a0 = -span / 2 + (span * i) / n, a1 = -span / 2 + (span * (i + 1)) / n;
      const P = (a, y) => [Math.sin(a) * R0, y, Math.cos(a) * R0 - R0];
      const N = (a) => [Math.sin(a), 0, Math.cos(a)];
      mb.quadN(P(a0, -h / 2), P(a1, -h / 2), P(a1, h / 2), P(a0, h / 2), N(a0), N(a1), N(a1), N(a0));
    }
    // Owner's band and rim.
    setTeam(mb, 0.42);
    for (let i = 0; i < n; i++) {
      const a0 = -span / 2 + (span * i) / n, a1 = -span / 2 + (span * (i + 1)) / n;
      const P = (a, y) => [Math.sin(a) * (R0 + 0.002), y, Math.cos(a) * (R0 + 0.002) - R0];
      mb.quad(P(a0, -0.012), P(a1, -0.012), P(a1, 0.012), P(a0, 0.012));
    }
    setMat(mb, o.rim ?? g.metal2, MAT.METAL);
    const edge = [];
    for (let i = 0; i <= n; i++) {
      const a = -span / 2 + (span * i) / n;
      edge.push([Math.sin(a) * R0, h / 2, Math.cos(a) * R0 - R0]);
    }
    const bottom = edge.map(([x, , z]) => [x, -h / 2, z]).reverse();
    mb.tube([...edge, ...bottom, edge[0]], 0.003, 3);
    ball(mb, [0, 0.03, 0.004], 0.011, 0, [1, 1, 0.6]);
  }
}

// ---------------------------------------------------------------------------------------------
// The humanoid generator

const STANCES = {
  stand: { fl: [0.034, 0.012], fr: [-0.034, -0.01], drop: 0.003 },
  stride: { fl: [0.036, 0.055], fr: [-0.032, -0.05], drop: 0.013 },
  brace: { fl: [0.05, 0.06], fr: [-0.048, -0.045], drop: 0.026 },
  wide: { fl: [0.052, 0.004], fr: [-0.052, -0.004], drop: 0.018 },
  walk: { fl: [0.03, -0.045], fr: [-0.03, 0.05], drop: 0.011 },
};

/**
 * Builds a humanoid figure into a fresh MeshBuilder (figure space: feet on y = 0, facing +z).
 *
 * o: {r, race, dark, variant, garb, skin, lite (fewer sides), stance, shift, lean, twist, headYaw,
 *     headTilt, headScale, bulk,
 *     body: "tunic" | "armour" | "plate" | "robe" | "fur" | "vest" | "bones" | "rags" | "bare",
 *     tunicTeam, robeTeam, robeShade, robeColor, hemColor, flare, stole, chasuble, tabard,
 *     tabardLen, skirt (length), skirtColor, skirtMat, ragged, sleeves ("team" | "cloth" |
 *     "metal" | "body" | "bare"), longSleeves, bracers, gloves, greaves, belt, pauldrons, mantle,
 *     legs, boots, bareShin, barefoot, claws, tail, metal,
 *     head: "bare" | "helm" | "kettle" | "great" | "horned" | "winged" | "coif" | "hood" |
 *           "deephood" | "hat" | "mitre" | "crown" | "circlet" | "straw" | "skull" | "bandana",
 *     hair, hairStyle ("long" | "topknot" | "mohawk" | "none"), beard ("short" | "full" |
 *     "braided"), beardColor, horns, eyeGlow, plume, beastHead ("goat" | "bull" | "wolf" |
 *     "boar" | "ape"), cape, capeLen, capeShade, wings ({span, lift, sweep, feathers, ...}),
 *     back: ["quiver" | "sack" | "bedroll" | "shield" | "banner"], seat: {w} (riding),
 *     right / left: {hand: [out, dy, fwd] from the shoulder, or handAt(hands, shoulder),
 *                    elbow: pole [out, dy, fwd], weapon (WEAPONS key), dir, face, at, opts
 *                    (values or functions of the hands), scale, shield, shieldFace, shieldUp,
 *                    shieldOff, shieldOpts}}
 * Returns {mb, hands, headC, top}.
 */
function figure(o) {
  const mb = new MeshBuilder();
  const r = o.r;
  const race = o.race ?? "human";
  const R0 = RACES[race] ?? RACES.human;
  // Heroic stylisation: limbs a little stouter and hands bigger than life, so arms, weapons and
  // stances still read when the figure is 35-45 pixels tall.
  const R = { ...R0, armR: R0.armR * 1.12, legR: R0.legR * 1.08, handR: R0.handR * 1.18 };
  const g = o.garb ?? garbOf(race, o.dark);
  const skinC = jitter(C(o.skin ?? R.skin), r, 0.04);
  const skinMat = race === "undead" ? MAT.MATTE : MAT.SKIN;
  const bony = o.body === "bones";
  const robe = o.body === "robe";
  const W = R.shW / 0.07; // width factor
  const LS = o.lite ? 4 : 5; // limb sides

  // --- the pelvis and legs ------------------------------------------------------------------
  const seated = !!o.seat;
  const st = seated ? null : STANCES[o.stance ?? "stand"] ?? STANCES.stand;
  const drop = seated ? 0 : st.drop * (R.hipY / 0.225);
  const pelvis = seated ? [0, R.hipY, 0] : [o.shift ?? 0, R.hipY - drop, 0];
  const ankleY = 0.02 * (R.legR / 0.0195);
  const legLen = ((R.hipY - ankleY) / 2) * 1.035;
  const hipJ = (s) => vadd(pelvis, [s * R.hipW, -0.012, 0]);
  const feet = {};
  for (const s of [L_SIDE, R_SIDE]) {
    let ankle, pole;
    if (seated) {
      ankle = [s * (o.seat.w + 0.012), R.hipY - legLen * 1.55, 0.02];
      pole = [s * 0.3, 0, 1];
    } else {
      const f = s === L_SIDE ? st.fl : st.fr;
      ankle = [f[0] * W, ankleY, f[1] * (R.hipY / 0.225)];
      pole = [s * 0.12, 0, 1];
    }
    const hip = hipJ(s);
    const { mid: knee, end } = ik(hip, ankle, legLen, legLen, pole);
    feet[s] = { hip, knee, ankle: end };
  }

  // --- the torso frame ------------------------------------------------------------------------
  const lean = R.lean + (o.lean ?? 0);
  const twist = o.twist ?? 0;
  const TF = new MeshBuilder();
  TF.translate(pelvis[0], pelvis[1], pelvis[2]).rotateY(twist).rotateX(lean);
  const T = (p) => TF.xp(p);
  const ts = R.shY - R.hipY; // shoulder height above the pelvis
  const tn = R.neckY - R.hipY;
  const shoulder = (s) => T([s * R.shW, ts - R.armR * 0.4, -0.004]);

  // --- legs -------------------------------------------------------------------------------------
  for (const s of [L_SIDE, R_SIDE]) {
    const { hip, knee, ankle } = feet[s];
    mb.bone = s === L_SIDE ? BONE.LEG_L : BONE.LEG_R;
    if (!robe) {
      if (bony) {
        setMat(mb, skinC);
        mb.tube([hip, knee, ankle], [R.legR * 1.2, R.legR, R.legR * 0.8], 4, { caps: true });
        ball(mb, knee, R.legR * 1.6, 0);
      } else {
        const legC = o.legs ? C(o.legs) : C(g.legs);
        const bootC = o.boots ? C(o.boots) : C(g.boots);
        const mid = vlerp(knee, ankle, 0.35);
        mb.mat = MAT.MATTE;
        mb.tube([hip, vlerp(hip, knee, 0.5), knee, mid, ankle], [R.legR * 1.15, R.legR * 1.08, R.legR * 0.92, R.legR * 1.02, R.legR * 0.9], LS, { caps: true, colors: [legC, legC, o.bareShin ? skinC : legC, bootC] });
        // Boot cuff (under greaves, or on a demon's hoof-like legs, not needed).
        if (!o.greaves && race !== "demon") {
          mb.color = bootC;
          mb.mat = MAT.MATTE;
          mb.tube([vlerp(knee, ankle, 0.3), vlerp(knee, ankle, 0.42)], R.legR * 1.18, LS);
        }
        if (o.greaves) {
          setMat(mb, g.metal, MAT.METAL);
          mb.tube([vlerp(knee, ankle, 0.05), vlerp(knee, ankle, 0.6)], [R.legR * 1.12, R.legR * 1.05], LS);
          ball(mb, knee, R.legR * 1.25, 0);
        }
      }
    }
    // The foot (its sole below the ground, for slopes).
    const fdir = Math.atan2(s * 0.12, 1);
    mb.color = bony ? skinC : C(o.boots ?? g.boots);
    mb.mat = MAT.MATTE;
    if (o.barefoot && !bony) {
      mb.color = skinC;
      mb.mat = skinMat;
    }
    mb.part(() => {
      mb.translate(ankle[0], seated ? ankle[1] - ankleY * 0.8 : -0.014, ankle[2] + R.foot * 0.25);
      mb.rotateY(fdir);
      mb.box(R.legR * 1.9, ankleY + 0.02, R.foot * (seated ? 0.9 : 1.2), { top: 0.75, skew: [0, -R.foot * 0.2] });
    });
    if (o.claws) {
      setMat(mb, "bone");
      for (const dx of [-1, 1]) {
        mb.part(() => {
          orient(mb, [ankle[0] + dx * R.legR * 0.6, 0.004, ankle[2] + R.foot * 0.7], [dx * 0.3, -0.25, 1], [0, 1, 0]);
          mb.cone(0.0035, 0.016, 3);
        });
      }
    }
  }

  // --- a tail (lizardmen, demons, werewolves) ------------------------------------------------
  if (o.tail ?? (race === "lizard" || race === "demon")) {
    mb.bone = BONE.TAIL;
    const thin = race === "demon";
    const b = vadd(pelvis, [0, -0.01, -R.waistR * 0.7]);
    const pts = [b, vadd(b, [0, -0.05, -0.06]), vadd(b, [0.02, -0.12, -0.13]), vadd(b, [0.055, -b[1] + 0.03, -0.19]), vadd(b, [0.1, -b[1] + 0.02, -0.22])];
    setMat(mb, skinC, skinMat);
    mb.tube(pts, thin ? [0.009, 0.007, 0.005, 0.004, 0.002] : [R.waistR * 0.75, R.waistR * 0.55, R.waistR * 0.36, R.waistR * 0.18, 0.002], 5, { caps: true });
    if (thin) {
      mb.part(() => {
        orient(mb, pts[4], vsub(pts[4], pts[3]), [0, 1, 0]);
        slab(mb, [[0, -0.004], [0.012, -0.002], [0, 0.022], [-0.012, -0.002]], 0.003);
      });
    }
  }

  // --- the body ---------------------------------------------------------------------------------
  mb.bone = BONE.BODY;
  const bodyCol = () => {
    switch (o.body) {
      case "armour":
      case "plate":
        return [C(o.metal ?? g.metal), MAT.METAL];
      case "robe":
        return o.robeTeam === false ? [C(o.robeColor ?? g.cloth), MAT.MATTE] : [C(teamCol(o.robeShade ?? 0.9)), MAT.TEAM];
      case "fur":
      case "vest":
        return [C(g.leather), MAT.MATTE];
      case "bare":
        return [skinC, skinMat];
      case "rags":
        return [C(g.cloth), MAT.MATTE];
      case "tunic":
      default:
        return o.tunicTeam === false ? [C(g.cloth), MAT.MATTE] : [C(teamCol(o.tunicShade ?? 0.9)), MAT.TEAM];
    }
  };
  const [bc, bm] = bodyCol();
  const bulk = o.bulk ?? 1;
  if (bony) {
    // A ribcage, a spine and a pelvis of bone.
    setMat(mb, skinC);
    mb.tube([pelvis, T([0, ts * 0.5, -0.01]), T([0, tn, 0])], [R.waistR * 0.8, R.waistR * 0.7, R.waistR * 0.6], 4);
    mb.part(() => {
      mb.apply(TF.m);
      mb.scale(1, 1, R.depth);
      mb.lathe([[R.chestR * 0.55, ts * 0.38], [R.chestR * 0.95, ts * 0.55], [R.chestR, ts * 0.78], [R.chestR * 0.7, ts * 0.98], [R.chestR * 0.3, tn]], 7, { caps: false, wobble: (a, y) => 0.12 * Math.sin(y * 260) });
    });
    mb.part(() => {
      mb.translate(pelvis[0], pelvis[1] - 0.02, pelvis[2]);
      mb.lathe([[R.hipW * 1.1, 0], [R.hipW * 1.5, 0.022], [R.hipW * 0.9, 0.03]], 6);
    });
  } else {
    mb.color = bc;
    mb.mat = bm;
    mb.part(() => {
      mb.apply(TF.m);
      mb.scale(bulk, 1, R.depth * bulk);
      mb.lathe([
        [R.waistR * 1.04, -0.032],
        [R.waistR * 1.12, 0.0],
        [R.waistR * 1.0, ts * 0.34],
        [R.chestR * 0.97, ts * 0.64],
        [R.chestR * 1.0, ts * 0.82],
        [R.chestR * 0.88, ts * 0.97],
        [R.chestR * 0.55, tn - 0.002],
        [R.chestR * 0.3, tn + 0.006],
      ], o.lite ? 7 : 8);
    });
    if (o.body === "fur") {
      // The chest bare under the open fur.
      setMat(mb, skinC, skinMat);
      mb.part(() => {
        mb.apply(TF.m);
        mb.translate(0, ts * 0.7, R.chestR * R.depth * 0.55);
        mb.scale(1.1, 0.8, 0.6);
        mb.sphere(R.chestR * 0.62, 1);
      });
    }
  }

  // Skirt / tunic hem / robe.
  const skirtLen = o.skirt ?? 0;
  // A robe hangs from the chest: never narrower than the body's frame (skeletal liches included).
  const RW = Math.max(R.waistR, R.chestR * 0.8);
  if (robe) {
    mb.color = bc;
    mb.mat = bm;
    const m0 = mb.mark();
    mb.part(() => {
      mb.translate(pelvis[0], 0, pelvis[2]);
      mb.scale(1, 1, 0.85);
      const top = pelvis[1] + 0.02;
      const flare = o.flare ?? 1;
      const folds = (a, y) => (0.07 * Math.sin(a * 7 + 0.5) + 0.04 * Math.sin(a * 3)) * Math.max(0, 1 - y / top);
      mb.lathe([[RW * 0.6, -0.008], [RW * 1.75 * flare, -0.008], [RW * 1.6 * flare, top * 0.25], [RW * 1.25, top * 0.7], [RW * 1.1, top]], o.lite ? 9 : 10, { caps: true, wobble: folds });
      // A hem band of darker cloth.
      mb.color = o.hemColor ? C(o.hemColor) : bm === MAT.TEAM ? C(teamCol(0.35)) : mixc(bc, "#000000", 0.45);
      mb.lathe([[RW * 1.79 * flare, -0.008], [RW * 1.76 * flare, 0.022], [RW * 1.7 * flare, 0.026]], o.lite ? 9 : 10, { caps: false, wobble: folds });
    });
    mb.heightSway(m0, R.hipY * 0.6, 0.0, 0.25);
    // Painted folds and a hem in shadow: the dye varies around and down the robe.
    mb.tint(m0, (x, y, z) => (0.84 + 0.16 * Math.min(1, y / (R.hipY * 0.9))) * (1 + 0.1 * Math.sin(Math.atan2(z - pelvis[2], x - pelvis[0]) * 7 + 0.5)));
    // A front panel (stole / scapular) in the contrasting colour.
    if (o.stole !== false) {
      const sc = o.stole ?? (bm === MAT.TEAM ? [C(g.trim), MAT.METAL] : [C(teamCol(0.95)), MAT.TEAM]);
      mb.color = sc[0];
      mb.mat = sc[1] ?? MAT.MATTE;
      mb.part(() => {
        const top = T([0, ts * 0.95, R.chestR * R.depth * 0.92]);
        const mid = T([0, 0.0, RW * R.depth * 1.12]);
        const bot = [pelvis[0], 0.004, pelvis[2] + RW * 1.95 * (o.flare ?? 1) * 0.85];
        const wv = R.chestR * 0.36;
        mb.quad(vadd(top, [-wv, 0, 0.003]), vadd(top, [wv, 0, 0.003]), vadd(mid, [wv * 1.05, 0, 0.004]), vadd(mid, [-wv * 1.05, 0, 0.004]));
        mb.quad(vadd(mid, [-wv * 1.05, 0, 0.004]), vadd(mid, [wv * 1.05, 0, 0.004]), vadd(bot, [wv * 1.3, 0, 0.004]), vadd(bot, [-wv * 1.3, 0, 0.004]));
      });
    }
  } else if (skirtLen > 0) {
    const sk = o.skirtColor ? [C(o.skirtColor), o.skirtMat ?? MAT.MATTE] : o.body === "armour" || o.body === "plate" ? [C(g.leather), MAT.MATTE] : [bc, bm];
    mb.color = sk[0];
    mb.mat = sk[1];
    const m0 = mb.mark();
    mb.part(() => {
      mb.translate(pelvis[0], pelvis[1], pelvis[2]);
      mb.rotateY(twist * 0.5);
      mb.scale(1, 1, R.depth * 1.05);
      mb.lathe([[R.waistR * 1.13, 0.012], [R.waistR * 1.3, -0.03], [R.waistR * 1.55, -skirtLen]], o.lite ? 7 : 8, { caps: false, wobble: o.ragged ? (a) => 0.1 * Math.sin(a * 7) : null });
    });
    mb.heightSway(m0, pelvis[1], pelvis[1] - skirtLen, 0.3);
  }

  // Tabard: team panels front and back over armour.
  if (o.tabard) {
    setTeam(mb, 0.95);
    const m0 = mb.mark();
    mb.part(() => {
      mb.apply(TF.m);
      const zf = R.chestR * R.depth * 1.02 * bulk;
      const top = ts * 0.92, bot = -(o.tabardLen ?? R.hipY * 0.5);
      const w0 = R.chestR * 0.55, w1 = R.waistR * 0.8;
      for (const s of [1, -1]) {
        const z0 = s * zf, z1 = s * R.waistR * R.depth * 1.25;
        mb.quad([-w0, top, z0], [w0, top, z0], [w1, 0, z1 + s * 0.003], [-w1, 0, z1 + s * 0.003]);
        mb.quad([-w1, 0, z1 + s * 0.003], [w1, 0, z1 + s * 0.003], [w1 * 1.05, bot, z1 + s * 0.012], [-w1 * 1.05, bot, z1 + s * 0.012]);
      }
      // The device: a pale band.
      setTeam(mb, 0.4);
      mb.quad([-w1 * 0.3, ts * 0.6, zf + 0.002], [w1 * 0.3, ts * 0.6, zf + 0.002], [w1 * 0.3, bot * 0.7, R.waistR * R.depth * 1.25 + 0.012], [-w1 * 0.3, bot * 0.7, R.waistR * R.depth * 1.25 + 0.012]);
    });
    mb.heightSway(m0, pelvis[1], pelvis[1] - R.hipY * 0.5, 0.35);
  }

  // Chasuble: a bell of the owner's colour over the robe (priests).
  if (o.chasuble) {
    setTeam(mb, 0.92);
    const m0 = mb.mark();
    mb.part(() => {
      mb.apply(TF.m);
      mb.scale(1, 1, R.depth * 1.05);
      mb.lathe([[R.chestR * 0.45, tn + 0.004], [R.chestR * 1.05, ts * 0.92], [R.chestR * 1.12, ts * 0.55], [R.waistR * 1.55, 0.0], [R.waistR * 1.75, -R.hipY * 0.38]], 10, { caps: false });
    });
    mb.heightSway(m0, pelvis[1], pelvis[1] - R.hipY * 0.38, 0.25);
    setMat(mb, g.trim, MAT.METAL);
    mb.part(() => {
      mb.apply(TF.m);
      mb.translate(0, ts * 0.45, R.chestR * R.depth * 1.18);
      mb.box(R.chestR * 0.2, ts * 0.7, 0.004);
      mb.translate(0, ts * 0.42, 0);
      mb.box(R.chestR * 0.9, R.chestR * 0.2, 0.004);
    });
  }

  // Belt.
  if (o.belt !== false && !bony) {
    setMat(mb, g.leather2);
    mb.part(() => {
      mb.apply(TF.m);
      mb.translate(0, 0.004, 0);
      mb.scale(1, 1, R.depth * bulk);
      mb.lathe([[R.waistR * 1.14 * bulk, -0.006], [R.waistR * 1.16 * bulk, 0.008]], 8, { caps: false });
    });
    setMat(mb, g.trim, MAT.METAL);
    mb.part(() => {
      mb.apply(TF.m);
      mb.translate(0, -0.004, R.waistR * R.depth * 1.15 * bulk);
      mb.box(0.014, 0.014, 0.004);
    });
  }

  // Fur mantle / collar about the shoulders.
  if (o.mantle) {
    setMat(mb, o.mantle === true ? g.fur : o.mantle);
    const seed = r.range(0, 50);
    mb.part(() => {
      mb.apply(TF.m);
      mb.translate(0, ts - 0.004, -0.004);
      ringTube(mb, R.shW * 0.92, R.chestR * R.depth * 1.05, R.armR * 1.15, 8, 4, { wob: 0.18, seed });
    });
  }

  // --- arms and hands -------------------------------------------------------------------------
  const hands = {};
  const armL = (R.shY - R.hipY) * 1.22 * R.arm;
  for (const s of [R_SIDE, L_SIDE]) {
    const spec = (s === L_SIDE ? o.left : o.right) ?? {};
    const S = shoulder(s);
    const h = spec.hand ?? [0.012, -armL * 0.92, 0.02];
    const target = spec.handAt ? spec.handAt(hands, S) : vadd(S, [s * h[0], h[1], h[2]]);
    const pole = spec.elbow ? [s * spec.elbow[0], spec.elbow[1], spec.elbow[2]] : [s * 0.8, -0.2, -0.6];
    const { mid: E, end: Hd } = ik(S, target, armL * 0.52, armL * 0.48, pole);
    const dirW = vnorm(vsub(Hd, E));
    const wrist = vsub(Hd, vscale(dirW, R.handR * 0.7));
    hands[s] = { pos: Hd, dir: dirW, elbow: E, shoulder: S };
    mb.bone = s === L_SIDE ? BONE.ARM_L : BONE.ARM_R;
    if (bony) {
      setMat(mb, skinC);
      mb.tube([S, E, wrist], [R.armR * 1.2, R.armR, R.armR * 0.8], 4, { caps: true });
      ball(mb, E, R.armR * 1.6, 0);
    } else {
      const sleeve = o.sleeves ?? (o.body === "armour" || o.body === "plate" ? "metal" : o.body === "fur" || o.body === "bare" || o.body === "vest" ? "bare" : "team");
      const upC = sleeve === "metal" ? [C(o.metal ?? g.metal), MAT.METAL] : sleeve === "team" ? (bm === MAT.TEAM ? [bc, bm] : [C(teamCol(0.85)), MAT.TEAM]) : sleeve === "cloth" ? [C(g.cloth), MAT.MATTE] : sleeve === "body" ? [bc, bm] : [skinC, skinMat];
      const loC = o.bracers ? [C(o.bracers === true ? g.leather : o.bracers), MAT.MATTE] : sleeve === "metal" ? upC : robe || sleeve === "body" ? upC : sleeve === "team" && o.longSleeves ? upC : [skinC, skinMat];
      mb.color = upC[0];
      mb.mat = upC[1];
      mb.tube([S, vlerp(S, E, 0.5), E], [R.armR * 1.18, R.armR * 1.1, R.armR * 0.98], LS, { caps: true });
      mb.color = loC[0];
      mb.mat = loC[1];
      const cuff = robe ? 1.6 : 1;
      mb.tube([E, vlerp(E, wrist, 0.5), wrist], [R.armR * 0.98, R.armR * (0.95 + 0.25 * (cuff - 1)), R.armR * 0.85 * cuff], LS, { caps: true });
      if (o.bracers) mb.tube([vlerp(E, wrist, 0.45), wrist], [R.armR * 1.08, R.armR * 1.05], LS);
      // Shoulder: a deltoid, or a pauldron.
      if (o.pauldrons) {
        const pc = o.pauldrons === true ? [C(o.metal ?? g.metal), MAT.METAL] : o.pauldrons === "team" ? [C(teamCol(0.8)), MAT.TEAM] : [C(o.pauldrons), MAT.MATTE];
        mb.color = pc[0];
        mb.mat = pc[1];
        mb.part(() => {
          orient(mb, vadd(S, [0, R.armR * 0.3, 0]), vnorm(vadd(vsub(S, E), [0, 0.04, 0])), [0, 0, 1]);
          mb.lathe([[R.armR * 2.1, -R.armR * 1.2], [R.armR * 2.0, 0], [R.armR * 1.4, R.armR * 1.2], [0, R.armR * 1.6]], 7);
        });
      } else {
        mb.color = upC[0];
        mb.mat = upC[1];
        ball(mb, S, R.armR * 1.38, 0);
      }
    }
    // The hand.
    mb.color = o.gloves ? C(o.gloves === true ? g.leather2 : o.gloves) : skinC;
    mb.mat = o.gloves ? MAT.MATTE : skinMat;
    ball(mb, Hd, R.handR, 0, [1, 0.9, 1]);
  }

  // --- the neck and head ----------------------------------------------------------------------
  mb.bone = BONE.HEAD;
  const neckTop = T([0, tn, 0.004]);
  const HR = R.headR * (o.headScale ?? 1.16); // heroic: heads a little large
  const hdOff = race === "orc" || race === "goblin" ? [0, HR * 0.68, HR * 0.42] : race === "lizard" ? [0, HR * 0.85, HR * 0.3] : [0, HR * 0.9, HR * 0.1];
  const headC = vadd(neckTop, hdOff);
  if (!bony) {
    setMat(mb, skinC, skinMat);
    mb.tube([T([0, tn - 0.012, 0]), vlerp(neckTop, headC, 0.5)], HR * (race === "orc" ? 0.55 : 0.4), 5);
  } else {
    setMat(mb, skinC);
    mb.tube([T([0, tn - 0.012, 0]), headC], R.armR, 4);
  }
  const HF = new MeshBuilder();
  HF.translate(headC[0], headC[1], headC[2]).rotateY(o.headYaw ?? twist * 0.4).rotateX(-lean * 0.75 + (o.headTilt ?? 0));
  const headPart = (f) => mb.part(() => {
    mb.apply(HF.m);
    f();
  });
  head(mb, headPart, { o, race, hr: HR, skinC, skinMat, g, r });

  // --- cape -------------------------------------------------------------------------------------
  if (o.cape) {
    mb.bone = BONE.CAPE;
    const m0 = mb.mark();
    const nu = 5, nv = 5;
    const top = (u) => {
      const a = (u - 0.5) * 2.6;
      return T([Math.sin(a) * R.shW * 0.95, ts + 0.002, -Math.cos(a) * R.chestR * R.depth * 1.02 - 0.004]);
    };
    const len = o.capeLen ?? 1;
    const bot = (u) => {
      const a = (u - 0.5) * 2.3;
      const base = [pelvis[0], 0.02 + (1 - len) * R.hipY * 1.5, pelvis[2] - 0.035];
      return vadd(base, [Math.sin(a) * R.shW * 1.45, Math.abs(u - 0.5) * 0.02, -Math.cos(a) * 0.05 - 0.012 * Math.sin(u * 9)]);
    };
    const P = (u, v) => {
      const p = vlerp(top(u), bot(u), v);
      // Bulge outward from the back mid-way (hangs off the shoulders and the buttocks).
      return vadd(p, [0, 0, -Math.sin(v * Math.PI) * 0.02]);
    };
    for (let i = 0; i < nu; i++) {
      for (let j = 0; j < nv; j++) {
        const u0 = i / nu, u1 = (i + 1) / nu, v0 = j / nv, v1 = (j + 1) / nv;
        if (o.capeColor) setMat(mb, o.capeColor);
        else setTeam(mb, o.capeShade ?? 0.82);
        mb.quad(P(u0, v0), P(u1, v0), P(u1, v1), P(u0, v1));
      }
    }
    // Soft normals (round about the body), flutter growing toward the hem.
    mb.sphereNormals(m0, [pelvis[0], R.hipY, pelvis[2] + 0.02], 0.55);
    const vv = mb.v;
    for (let i = m0 * 13; i < vv.length; i += 13) {
      vv[i + 10] = Math.max(0, Math.min(1, (ts + R.hipY - vv[i + 1]) / (ts + R.hipY))) * 0.85;
      vv[i + 12] *= 0.8 + 0.2 * Math.min(1, vv[i + 1] / 0.2);
    }
    mb.bone = BONE.BODY;
  }

  // --- wings (imps, demon lords, angels) ---------------------------------------------------------
  if (o.wings) {
    for (const s of [L_SIDE, R_SIDE]) {
      const root = T([s * R.shW * 0.45, ts * 0.86, -R.chestR * R.depth * 0.7]);
      wing(mb, s, root, o.wings);
    }
  }

  // --- things carried on the back -------------------------------------------------------------
  mb.bone = BONE.BODY;
  for (const item of o.back ?? []) {
    const backP = (y, x = 0) => T([x, y, -R.chestR * R.depth - 0.01]);
    if (item === "quiver") {
      setMat(mb, g.leather);
      const a = backP(ts * 0.3, R.shW * 0.4), b = backP(ts * 1.05, -R.shW * 0.5);
      mb.tube([a, b], [0.014, 0.016], 6, { caps: true });
      setMat(mb, "#e0d8c8");
      for (let i = 0; i < 4; i++) {
        const tip = vadd(b, [(i - 1.5) * 0.006 - 0.012, 0.03 + (i % 2) * 0.006, -0.004 + (i % 2) * 0.004]);
        mb.tube([vadd(b, [(i - 1.5) * 0.005, -0.005, 0]), tip], 0.0035, 3);
      }
    } else if (item === "sack") {
      setMat(mb, "linen");
      const p = backP(ts * 0.55);
      mb.part(() => {
        mb.translate(p[0], p[1], p[2] - 0.012);
        mb.blob(0.042, { detail: 1, amp: 0.12, squash: 1.2, noise: (v) => noise3(v[0] * 3, v[1] * 3, v[2] * 3) });
      });
      setMat(mb, "rope");
      mb.tube([T([R.shW * 0.6, ts, R.chestR * R.depth * 0.6]), T([R.shW * 0.4, ts * 0.4, R.chestR * R.depth * 1.05]), T([-R.shW * 0.3, ts * 0.1, R.chestR * R.depth * 1.05])], 0.003, 3);
    } else if (item === "bedroll") {
      setMat(mb, g.cloth);
      const p = backP(ts * 1.0);
      mb.part(() => {
        mb.translate(p[0], p[1] + 0.006, p[2]);
        mb.rotateZ(Math.PI / 2);
        mb.translate(0, -0.05, 0);
        mb.cylinder(0.016, 0.016, 0.1, 7);
      });
    } else if (item === "shield") {
      const p = backP(ts * 0.6);
      mb.part(() => {
        orient(mb, vadd(p, [0, 0, -0.006]), [0, 1, 0.1], [0, -0.1, -1]);
        shield(mb, g, "round", { r: 0.055 });
      });
    } else if (item === "banner") {
      // A small owner's banner on a pole strapped to the back.
      const a = backP(ts * 0.2, 0.0);
      mb.part(() => {
        orient(mb, a, [0.06, 1, -0.12], [0, 0, 1]);
        mb.bone = BONE.CAPE;
        WEAPONS.banner(mb, g, { len: 0.3 });
      });
    }
  }

  // --- weapons and shields in the hands ---------------------------------------------------------
  for (const s of [R_SIDE, L_SIDE]) {
    const spec = (s === L_SIDE ? o.left : o.right) ?? {};
    const hd = hands[s];
    if (spec.weapon) {
      mb.bone = BONE.WEAPON;
      mb.part(() => {
        const fn = (v, d) => (typeof v === "function" ? v(hands) : v ?? d);
        orient(mb, fn(spec.at, hd.pos), fn(spec.dir, [0, 1, 0]), fn(spec.face, [0, 0, 1]));
        if (spec.scale) mb.scale(spec.scale);
        WEAPONS[spec.weapon](mb, g, fn(spec.opts, {}));
      });
    }
    if (spec.shield) {
      mb.bone = s === L_SIDE ? BONE.ARM_L : BONE.ARM_R;
      mb.part(() => {
        const face = vnorm(spec.shieldFace ?? [s * 0.35, 0.15, 1]);
        const at = vadd(hd.pos, vadd(vscale(face, 0.016), spec.shieldOff ?? [0, 0.012, 0]));
        orient(mb, at, spec.shieldUp ?? [s * 0.05, 1, -0.1], face);
        shield(mb, g, spec.shield, spec.shieldOpts ?? {});
      });
    }
  }
  mb.bone = BONE.BODY;
  // Baked occlusion: darker toward the ground (legs, hems), so the figure sits in its shadow.
  if (!seated) mb.heightAO(0, 0.0, R.hipY * 1.15, 0.62);
  // Dyed cloth is never one flat colour: the owner's colour deepens toward the hems and in soft
  // blotches (folds, wear), so it reads as painted wool rather than plastic.
  const tv = mb.v, nseed = (o.variant ?? 0) * 3.7;
  for (let i = 0; i < tv.length; i += 13) {
    if (tv[i + 9] !== MAT.TEAM) continue;
    const k = (0.86 + 0.14 * Math.min(1, Math.max(0, tv[i + 1] / R.shY))) * (1 + 0.11 * noise3(tv[i] * 42 + nseed, tv[i + 1] * 30, tv[i + 2] * 42));
    tv[i + 6] *= k;
    tv[i + 7] *= k;
    tv[i + 8] *= k;
  }
  return { mb, hands, headC, top: headC[1] + HR };
}

/** The head: skull, face features, hair, beard and headgear (in the head frame). */
function head(mb, part, { o, race, hr, skinC, skinMat, g, r }) {
  const hair = C(o.hair ?? vpick(o.variant ?? 0, g.hair));
  if (o.beastHead) {
    part(() => beastHead(mb, o.beastHead, hr, skinC, o));
    return;
  }
  // The skull.
  part(() => {
    mb.color = skinC;
    mb.mat = skinMat;
    if (race === "lizard") {
      mb.part(() => {
        mb.scale(0.85, 0.85, 1);
        mb.sphere(hr, 1);
      });
      // A long snout and a jaw.
      mb.tube([[0, -hr * 0.1, hr * 0.3], [0, -hr * 0.25, hr * 1.2], [0, -hr * 0.35, hr * 1.85]], [hr * 0.66, hr * 0.5, hr * 0.3], 6, { caps: true });
      mb.tube([[0, -hr * 0.45, hr * 0.2], [0, -hr * 0.6, hr * 1.5]], [hr * 0.45, hr * 0.22], 5, { caps: true });
      // A crest (frill) in war paint.
      setTeam(mb, 0.8);
      mb.part(() => {
        mb.rotateY(Math.PI / 2);
        slab(mb, [[hr * 0.9, hr * 0.6], [hr * 0.2, hr * 1.25], [-hr * 0.6, hr * 1.35], [-hr * 1.4, hr * 0.95], [-hr * 1.2, hr * 0.2], [0, hr * 0.5]], 0.004);
      });
      mb.mat = MAT.EMISSIVE;
      mb.color = C("#e8c040");
      for (const s of [-1, 1]) ball(mb, [s * hr * 0.42, hr * 0.15, hr * 0.62], hr * 0.12, 0);
      return;
    }
    if (race === "undead") {
      mb.part(() => {
        mb.scale(0.88, 1, 0.96);
        mb.sphere(hr, 1);
      });
      // The jaw and dark sockets.
      mb.part(() => {
        mb.translate(0, -hr * 0.62, hr * 0.28);
        mb.scale(0.7, 0.45, 0.7);
        mb.sphere(hr * 0.8, 0);
      });
      mb.color = C("#140e12");
      for (const s of [-1, 1]) ball(mb, [s * hr * 0.36, -hr * 0.02, hr * 0.78], hr * 0.24, 0);
      mb.mat = MAT.EMISSIVE;
      mb.color = C(o.eyeGlow ?? "#7ad8ff");
      for (const s of [-1, 1]) ball(mb, [s * hr * 0.36, -hr * 0.02, hr * 0.86], hr * 0.09, 0);
      return;
    }
    mb.part(() => {
      mb.scale(race === "orc" ? 1.08 : race === "goblin" ? 1.1 : 0.94, race === "goblin" ? 0.92 : 1.03, 1);
      mb.sphere(hr, 1);
    });
    if (race === "orc" || race === "goblin") {
      // A heavy jaw and brow; tusks for orcs, a long nose for goblins.
      mb.part(() => {
        mb.translate(0, -hr * 0.45, hr * 0.42);
        mb.scale(race === "goblin" ? 0.7 : 1.05, 0.55, 0.65);
        mb.sphere(hr * 0.8, 0);
      });
      mb.part(() => {
        mb.translate(0, hr * 0.26, hr * 0.62);
        mb.scale(1.0, 0.32, 0.5);
        mb.sphere(hr * 0.8, 0);
      });
      if (race === "orc") {
        setMat(mb, "bone");
        for (const s of [-1, 1]) horn(mb, [s * hr * 0.38, -hr * 0.55, hr * 0.78], [s * 0.2, 1, 0.3], hr * 0.45, hr * 0.12, [0, 0, -0.5], 3, 4);
      } else {
        mb.part(() => {
          mb.translate(0, -hr * 0.05, hr * 0.85);
          mb.rotateX(1.25);
          mb.cone(hr * 0.2, hr * 0.55, 5);
        });
      }
    }
    if (race === "demon") {
      mb.part(() => {
        mb.translate(0, -hr * 0.5, hr * 0.35);
        mb.scale(0.85, 0.5, 0.7);
        mb.sphere(hr * 0.85, 0);
      });
    }
  });
  // A painter's face: the eyes as two dabs of shadow under the brow (a face turned toward the
  // viewer reads as a face, not a bald egg). Demons' eyes glow instead (below).
  if (race !== "demon" && !o.eyeGlow && !o.lite) {
    part(() => {
      mb.color = mixc(skinC, "#140a08", 0.68);
      mb.mat = MAT.MATTE;
      const ey = race === "goblin" ? hr * 0.12 : hr * 0.06;
      for (const s of [-1, 1]) {
        mb.part(() => {
          const ex = s * hr * (race === "orc" || race === "goblin" ? 0.4 : 0.34);
          mb.translate(ex, ey, Math.sqrt(Math.max(0, 1 - (ex / hr) ** 2 - (ey / hr) ** 2)) * hr * 0.97 - hr * 0.06);
          mb.rotateY(s * 0.35);
          mb.box(hr * 0.26, hr * 0.16, hr * 0.12, { top: 0.8 });
        });
      }
    });
  }
  // Ears.
  part(() => {
    mb.color = skinC;
    mb.mat = skinMat;
    if (race === "elf") {
      for (const s of [-1, 1]) horn(mb, [s * hr * 0.86, 0, -hr * 0.05], [s * 0.85, 0.75, -0.35], hr * 0.95, hr * 0.2, [0, 0.3, 0], 3, 4);
    } else if (race === "goblin") {
      for (const s of [-1, 1]) {
        mb.part(() => {
          mb.translate(s * hr * 0.85, hr * 0.05, -hr * 0.05);
          mb.rotateZ(-s * (Math.PI / 2 - 0.25));
          mb.scale(1, 1, 0.35);
          mb.cone(hr * 0.38, hr * 1.25, 5);
        });
      }
    } else if (race === "orc") {
      for (const s of [-1, 1]) horn(mb, [s * hr * 0.98, hr * 0.05, -hr * 0.1], [s, 0.4, -0.3], hr * 0.45, hr * 0.17, [0, 0, 0], 3, 4);
    } else if (race === "demon") {
      for (const s of [-1, 1]) horn(mb, [s * hr * 0.9, 0, -hr * 0.05], [s, 0.3, -0.4], hr * 0.55, hr * 0.15, [0, 0.3, 0], 3, 4);
    }
  });
  // Eyes: glowing for demons (and the possessed); too small to model for the rest.
  if (race === "demon" || o.eyeGlow) {
    part(() => {
      mb.mat = MAT.EMISSIVE;
      mb.color = C(o.eyeGlow ?? "#ffb020");
      for (const s of [-1, 1]) ball(mb, [s * hr * 0.36, hr * 0.08, hr * 0.9], hr * 0.11, 0);
    });
  }
  // Horns.
  if (race === "demon" || o.horns) {
    part(() => {
      setMat(mb, o.hornColor ?? "#2a2020");
      const big = o.horns === "big" ? 1.5 : 1;
      for (const s of [-1, 1]) horn(mb, [s * hr * 0.55, hr * 0.62, hr * 0.15], [s * 0.6, 1, 0.1], hr * 1.5 * big, hr * 0.28 * big, [s * 0.4, -0.8, -1.2], 5, 4);
    });
  }
  // Hair.
  const hg = o.head ?? "bare";
  const covered = ["helm", "great", "horned", "hood", "deephood", "coif", "skull", "winged", "kettle"].includes(hg);
  if (!covered && race !== "undead" && race !== "lizard" && o.hairStyle !== "none") {
    part(() => {
      setMat(mb, hair);
      const seed = r.range(0, 40);
      mb.part(() => {
        mb.translate(0, hr * 0.12, -hr * 0.12);
        mb.blob(hr * 1.04, { detail: 1, amp: 0.1, squash: 0.95, noise: (v) => (v[2] > 0.3 && v[1] < 0.6 ? -1.6 : 0) + noise3(v[0] * 3 + seed, v[1] * 3, v[2] * 3) * 0.6 });
      });
      if (o.hairStyle === "long" || race === "elf") {
        mb.tube([[0, hr * 0.2, -hr * 0.7], [0, -hr * 0.8, -hr * 0.95], [0, -hr * 1.9, -hr * 0.9]], [hr * 0.75, hr * 0.62, hr * 0.3], 6, { caps: true });
      }
      if (o.hairStyle === "topknot" || o.hairStyle === "mohawk") {
        mb.tube([[0, hr * 0.95, hr * 0.4], [0, hr * 1.18, -hr * 0.2], [0, hr * 0.9, -hr * 0.9]], [hr * 0.18, hr * 0.26, hr * 0.12], 5, { caps: true });
      }
    });
  }
  // Beard.
  if (o.beard && race !== "undead" && race !== "lizard") {
    part(() => {
      setMat(mb, o.beardColor ?? hair);
      const big = o.beard === "full" ? 1.35 : o.beard === "braided" ? 1.25 : 0.8;
      const seed = r.range(0, 40);
      mb.part(() => {
        mb.translate(0, -hr * 0.6, hr * 0.42);
        mb.rotateX(0.35);
        mb.scale(1, 1.25 * big, 0.62);
        mb.blob(hr * 0.75, { detail: 1, amp: 0.14, noise: (v) => noise3(v[0] * 4 + seed, v[1] * 4, v[2] * 4) });
      });
      if (o.beard === "braided" || o.beard === "full") {
        // The beard flows down onto the chest.
        mb.tube([[0, -hr * 1.1, hr * 0.7], [0, -hr * 1.65, hr * 0.8], [0, -hr * 2.15, hr * 0.7]], [hr * 0.55, hr * 0.44, hr * 0.15], 6, { caps: true });
        if (o.beard === "braided") {
          setMat(mb, g.trim, MAT.METAL);
          ball(mb, [0, -hr * 1.75, hr * 0.82], hr * 0.2, 0);
        }
      }
      // Moustache.
      setMat(mb, o.beardColor ?? hair);
      mb.tube([[-hr * 0.55, -hr * 0.55, hr * 0.7], [0, -hr * 0.28, hr * 0.98], [hr * 0.55, -hr * 0.55, hr * 0.7]], [hr * 0.1, hr * 0.15, hr * 0.1], 3, { caps: true });
    });
  }
  // Headgear.
  part(() => headgear(mb, hg, { o, race, hr, g, r }));
}

/** Animal heads on humanoid bodies (beastmen, minotaurs, werewolves, apes). */
function beastHead(mb, kind, hr, skinC, o) {
  const fur = C(o.furColor ?? skinC);
  const horn_ = C(o.hornColor ?? "#d8cfb6");
  mb.mat = MAT.MATTE;
  mb.color = fur;
  if (kind === "goat" || kind === "bull") {
    const bull = kind === "bull";
    mb.part(() => {
      mb.scale(bull ? 1.15 : 0.95, 1, 1.05);
      mb.sphere(hr, 1);
    });
    mb.tube([[0, -hr * 0.1, hr * 0.4], [0, -hr * 0.45, hr * 1.15], [0, -hr * 0.62, hr * 1.5]], [hr * (bull ? 0.75 : 0.6), hr * (bull ? 0.62 : 0.42), hr * (bull ? 0.55 : 0.28)], 6, { caps: true });
    mb.color = C("#2a2220");
    ball(mb, [0, -hr * 0.6, hr * 1.55], hr * (bull ? 0.45 : 0.25), 0, [1.2, 0.8, 0.6]);
    mb.color = fur;
    for (const s of [-1, 1]) {
      mb.part(() => {
        mb.translate(s * hr * 0.95, hr * 0.15, -hr * 0.05);
        mb.rotateZ(-s * 1.2);
        mb.scale(1, 1, 0.4);
        mb.cone(hr * 0.28, hr * 0.8, 4);
      });
    }
    mb.color = horn_;
    for (const s of [-1, 1]) {
      if (bull) horn(mb, [s * hr * 0.8, hr * 0.55, hr * 0.1], [s, 0.25, 0.1], hr * 2.0, hr * 0.32, [0, 1.6, 0.9], 5, 5);
      else horn(mb, [s * hr * 0.45, hr * 0.8, hr * 0.15], [s * 0.3, 0.8, -0.6], hr * 2.2, hr * 0.3, [s * 1.2, -2.2, 0.6], 6, 5);
    }
    if (!bull) {
      mb.color = C(o.beardColor ?? "#3a3028");
      horn(mb, [0, -hr * 0.85, hr * 1.1], [0, -1, 0.2], hr * 0.7, hr * 0.22, [0, 0, -0.3], 3, 4);
    } else {
      setMat(mb, "gold", MAT.METAL);
      mb.part(() => {
        mb.translate(0, -hr * 0.85, hr * 1.6);
        ringTube(mb, hr * 0.22, hr * 0.22, hr * 0.06, 8, 3);
      });
    }
  } else if (kind === "wolf") {
    ball(mb, [0, 0, 0], hr, 1);
    mb.tube([[0, -hr * 0.05, hr * 0.5], [0, -hr * 0.3, hr * 1.35], [0, -hr * 0.42, hr * 1.95]], [hr * 0.62, hr * 0.42, hr * 0.22], 6, { caps: true });
    mb.color = C("#161212");
    ball(mb, [0, -hr * 0.36, hr * 2.0], hr * 0.18, 0);
    mb.color = fur;
    for (const s of [-1, 1]) horn(mb, [s * hr * 0.55, hr * 0.7, -hr * 0.1], [s * 0.3, 1, -0.2], hr * 0.9, hr * 0.3, [0, 0, -0.3], 3, 4);
    mb.color = C("#e8e0cc");
    for (const s of [-1, 1]) horn(mb, [s * hr * 0.22, -hr * 0.55, hr * 1.55], [0, -1, 0.2], hr * 0.4, hr * 0.09, [0, 0, 0], 2, 3);
    mb.mat = MAT.EMISSIVE;
    mb.color = C(o.eyeGlow ?? "#ffd040");
    for (const s of [-1, 1]) ball(mb, [s * hr * 0.4, hr * 0.15, hr * 0.85], hr * 0.11, 0);
  } else if (kind === "boar") {
    ball(mb, [0, 0, 0], hr * 1.05, 1);
    mb.tube([[0, -hr * 0.1, hr * 0.4], [0, -hr * 0.35, hr * 1.3]], [hr * 0.75, hr * 0.48], 6, { caps: true });
    mb.color = C("#2a2220");
    ball(mb, [0, -hr * 0.36, hr * 1.35], hr * 0.42, 0, [1.1, 0.9, 0.4]);
    mb.color = C("#e8e0cc");
    for (const s of [-1, 1]) horn(mb, [s * hr * 0.4, -hr * 0.55, hr * 1.0], [s * 0.4, 1, 0.3], hr * 0.8, hr * 0.15, [0, 0, -0.8], 3, 3);
  } else {
    // ape
    mb.part(() => {
      mb.scale(1.05, 0.95, 1);
      mb.sphere(hr, 1);
    });
    mb.color = mixc(fur, "#2a2220", 0.5);
    mb.part(() => {
      mb.translate(0, -hr * 0.35, hr * 0.62);
      mb.scale(1, 0.7, 0.7);
      mb.sphere(hr * 0.62, 1);
    });
    mb.color = fur;
    mb.part(() => {
      mb.translate(0, hr * 0.3, hr * 0.6);
      mb.scale(1.2, 0.35, 0.5);
      mb.sphere(hr * 0.7, 0);
    });
  }
}

function headgear(mb, hg, { o, race, hr, g }) {
  const metal = () => setMat(mb, o.helmMetal ?? o.metal ?? g.metal, MAT.METAL);
  const sc = race === "orc" || race === "goblin" ? 1.08 : 1;
  const domeR = hr * 1.1 * sc;
  if (hg === "helm" || hg === "horned" || hg === "winged" || hg === "kettle" || hg === "coif") {
    if (hg === "coif") {
      setMat(mb, g.metal2, MAT.METAL);
      mb.part(() => {
        mb.translate(0, hr * 0.05, -hr * 0.12);
        mb.sphere(hr * 1.12, 1, { squash: 1.02 });
      });
      return;
    }
    metal();
    mb.part(() => {
      mb.translate(0, hr * 0.1, -hr * 0.02);
      mb.lathe([[domeR * 1.02, -hr * 0.2], [domeR * 1.04, hr * 0.1], [domeR * 0.82, hr * 0.62], [domeR * 0.36, hr * 0.98], [0, hr * (hg === "helm" ? 1.18 : 1.02)]], 9);
    });
    if (hg === "kettle") {
      mb.part(() => {
        mb.translate(0, hr * 0.0, -hr * 0.02);
        mb.lathe([[domeR * 1.0, -hr * 0.06], [domeR * 1.6, -hr * 0.26], [domeR * 1.62, -hr * 0.2], [domeR * 1.0, hr * 0.02]], 10);
      });
    } else {
      // Nasal and rim.
      mb.part(() => {
        mb.translate(0, -hr * 0.42, hr * 1.02 * sc);
        mb.box(hr * 0.2, hr * 0.62, hr * 0.12);
      });
      setMat(mb, o.helmTrim ?? g.trim, MAT.METAL);
      mb.part(() => {
        mb.translate(0, -hr * 0.08, -hr * 0.02);
        ringTube(mb, domeR * 1.05, domeR * 1.05, hr * 0.09, 10, 4);
      });
    }
    if (hg === "horned") {
      setMat(mb, o.hornColor ?? "bone");
      for (const s of [-1, 1]) horn(mb, [s * domeR * 0.92, hr * 0.32, -hr * 0.05], [s, 0.45, 0.1], hr * 1.3, hr * 0.26, [0, 1.4, 0.2], 5, 5);
    }
    if (hg === "winged") {
      setMat(mb, o.wingColor ?? "#e8e0d0");
      for (const s of [-1, 1]) {
        mb.part(() => {
          mb.translate(s * domeR * 0.95, hr * 0.35, -hr * 0.1);
          mb.rotateY(s * 0.3);
          mb.rotateZ(-s * 0.35);
          slab(mb, [[0, 0], [s * hr * 0.5, hr * 0.5], [s * hr * 0.6, hr * 1.25], [s * hr * 0.25, hr * 0.8], [0, hr * 0.45]], 0.004);
        });
      }
    }
    if (o.plume) {
      setTeam(mb, 0.95);
      mb.sway = 0.5;
      mb.tube([[0, hr * 1.15, hr * 0.2], [0, hr * 1.45, -hr * 0.2], [0, hr * 1.25, -hr * 0.9], [0, hr * 0.7, -hr * 1.3]], [hr * 0.18, hr * 0.32, hr * 0.28, hr * 0.12], 5, { caps: true });
      mb.sway = 0;
    }
    return;
  }
  if (hg === "great") {
    metal();
    mb.part(() => {
      mb.translate(0, -hr * 0.85, 0);
      mb.lathe([[hr * 1.06, 0], [hr * 1.12, hr * 0.9], [hr * 1.08, hr * 1.6], [hr * 0.7, hr * 2.0], [0, hr * 2.06]], 9);
    });
    setMat(mb, "#0e0c0c");
    mb.part(() => {
      mb.translate(0, hr * 0.05, hr * 1.04);
      mb.box(hr * 1.4, hr * 0.14, hr * 0.12);
    });
    setMat(mb, o.helmTrim ?? g.trim, MAT.METAL);
    mb.part(() => {
      mb.translate(0, -hr * 0.6, hr * 1.06);
      mb.box(hr * 0.16, hr * 0.85, hr * 0.1);
    });
    if (o.plume !== false) {
      setTeam(mb, 0.95);
      mb.sway = 0.5;
      mb.tube([[0, hr * 1.15, hr * 0.1], [0, hr * 1.5, -hr * 0.3], [0, hr * 1.3, -hr * 1.0], [0, hr * 0.8, -hr * 1.35]], [hr * 0.2, hr * 0.36, hr * 0.3, hr * 0.12], 5, { caps: true });
      mb.sway = 0;
    }
    return;
  }
  if (hg === "hood" || hg === "deephood") {
    // A cowl: a shell around the head, open at the face, its point falling back; and a short
    // mantle (capelet) over the shoulders, both in the owner's colour.
    const deep = hg === "deephood";
    setTeam(mb, o.hoodShade ?? (deep ? 0.78 : 0.88));
    if (o.hoodColor) setMat(mb, o.hoodColor);
    mb.part(() => {
      mb.translate(0, hr * 0.1, -hr * (deep ? 0.12 : 0.3));
      mb.sphere(hr * (deep ? 1.26 : 1.15), 1, { deform: (v) => 1 + (v[1] > 0.1 && v[2] < 0 ? 0.3 * v[1] * -v[2] : 0) + (v[2] > 0.5 ? -0.06 : 0) });
    });
    mb.part(() => {
      mb.translate(0, hr * 0.62, -hr * 0.95);
      horn(mb, [0, 0, 0], [0, 0.5, -1], hr * 1.1, hr * 0.42, [0, -1.8, 0], 4, 6);
    });
    if (deep) {
      // The face in shadow.
      setMat(mb, "#1a1418");
      mb.part(() => {
        mb.translate(0, -hr * 0.05, hr * 0.62);
        mb.scale(0.8, 0.95, 0.5);
        mb.sphere(hr * 0.88, 1);
      });
      if (o.hoodGlow) {
        mb.mat = MAT.EMISSIVE;
        mb.color = C(o.hoodGlow);
        for (const s of [-1, 1]) ball(mb, [s * hr * 0.3, 0, hr * 1.0], hr * 0.09, 0);
      }
    }
    // The mantle.
    if (o.lite) return;
    setTeam(mb, (o.hoodShade ?? 0.85) - 0.08);
    if (o.hoodColor) setMat(mb, o.hoodColor);
    mb.part(() => {
      mb.translate(0, -hr * 1.05, -hr * 0.15);
      mb.rotateX(o.mantleTilt ?? 0);
      mb.scale(1, 1, 0.82);
      mb.lathe([[hr * 0.62, hr * 0.42], [hr * 1.3, -hr * 0.2], [hr * 1.62, -hr * 0.82], [hr * 1.55, -hr * 0.95]], 10, { caps: false });
    });
    return;
  }
  if (hg === "hat") {
    // The wizard's hat: a broad brim and a tall cone whose tip bends back.
    if (o.hatColor) setMat(mb, o.hatColor);
    else setTeam(mb, 0.86);
    mb.part(() => {
      mb.translate(0, hr * 0.45, -hr * 0.05);
      mb.rotateX(-0.12);
      local(mb, (h) => {
        h.lathe([[hr * 0.4, 0], [hr * 1.9, -hr * 0.06], [hr * 2.0, -hr * 0.02], [hr * 1.5, hr * 0.1], [hr * 0.98, hr * 0.2]], 12);
        h.lathe([[hr * 0.98, hr * 0.15], [hr * 0.86, hr * 0.7], [hr * 0.55, hr * 1.5], [hr * 0.28, hr * 2.2], [0, hr * 2.9]], 9);
        // Bend the tip back and to one side.
        h.displace(0, (x, y) => {
          const t = Math.max(0, (y - hr * 0.9) / (hr * 2.0));
          return [t * t * hr * 0.4, -t * t * hr * 0.25, -t * t * hr * 1.1];
        });
      });
    });
    if (o.hatBand === "team") setTeam(mb, 0.86);
    else setMat(mb, o.hatBand ?? g.trim, o.hatBand ? MAT.MATTE : MAT.METAL);
    const bandH = o.hatBand === "team" ? 0.26 : 0.15;
    mb.part(() => {
      mb.translate(0, hr * 0.62, -hr * 0.07);
      mb.rotateX(-0.12);
      mb.cylinder(hr * 1.0, hr * 0.94, hr * bandH, 10, { caps: false });
    });
    return;
  }
  if (hg === "straw") {
    setMat(mb, "#b09858");
    mb.part(() => {
      mb.translate(0, hr * 0.42, 0);
      mb.lathe([[0, hr * 0.62], [hr * 0.8, hr * 0.42], [hr * 1.0, hr * 0.1], [hr * 1.85, -hr * 0.12], [hr * 1.9, -hr * 0.16]], 10);
    });
    return;
  }
  if (hg === "mitre") {
    setMat(mb, "#e4dccb");
    mb.part(() => {
      mb.translate(0, hr * 0.42, -hr * 0.04);
      mb.scale(1, 1, 0.7);
      mb.lathe([[hr * 0.95, 0], [hr * 1.0, hr * 0.7], [hr * 0.7, hr * 1.5], [0, hr * 1.95]], 8);
    });
    setTeam(mb, 0.9);
    mb.part(() => {
      mb.translate(0, hr * 0.42, hr * 0.66);
      mb.box(hr * 0.22, hr * 1.55, hr * 0.06, { top: 0.6 });
    });
    setMat(mb, g.trim, MAT.METAL);
    mb.part(() => {
      mb.translate(0, hr * 0.44, -hr * 0.04);
      mb.scale(1, 1, 0.72);
      mb.cylinder(hr * 0.99, hr * 0.99, hr * 0.15, 8, { caps: false });
    });
    return;
  }
  if (hg === "crown" || hg === "circlet") {
    setMat(mb, o.crownColor ?? "gold", MAT.METAL);
    const big = hg === "crown";
    mb.part(() => {
      mb.translate(0, hr * (big ? 0.5 : 0.38), -hr * 0.03);
      mb.cylinder(hr * 0.98, hr * (big ? 1.08 : 0.98), hr * (big ? 0.25 : 0.1), 10, { caps: false });
      if (big) {
        for (let i = 0; i < 7; i++) {
          const a = (i / 7) * TAU + Math.PI / 2;
          mb.part(() => {
            mb.translate(Math.cos(a) * hr * 1.06, hr * 0.22, Math.sin(a) * hr * 1.06);
            mb.cone(hr * 0.13, hr * 0.42, 4);
          });
        }
      } else {
        // A leaf at the brow.
        mb.part(() => {
          mb.translate(0, hr * 0.06, hr * 0.98);
          slab(mb, [[0, -hr * 0.1], [hr * 0.2, hr * 0.1], [0, hr * 0.5], [-hr * 0.2, hr * 0.1]], 0.003);
        });
      }
    });
    return;
  }
  if (hg === "bandana") {
    setTeam(mb, 0.9);
    mb.part(() => {
      mb.translate(0, hr * 0.28, -hr * 0.05);
      mb.cylinder(hr * 1.02, hr * 1.04, hr * 0.28, 9, { caps: false });
    });
    mb.sway = 0.7;
    mb.tube([[0, hr * 0.4, -hr * 1.0], [hr * 0.25, hr * 0.1, -hr * 1.4], [hr * 0.35, -hr * 0.3, -hr * 1.55]], [hr * 0.12, hr * 0.1, hr * 0.05], 4, { caps: true });
    mb.sway = 0;
    return;
  }
  if (hg === "skull") {
    // A beast's skull worn as a helm (orc shamans, barbarians).
    setMat(mb, "bone");
    mb.part(() => {
      mb.translate(0, hr * 0.4, hr * 0.05);
      mb.scale(1.05, 0.7, 1.2);
      mb.sphere(hr * 1.05, 1);
    });
    for (const s of [-1, 1]) horn(mb, [s * hr * 0.8, hr * 0.62, 0], [s, 0.5, -0.2], hr * 1.2, hr * 0.22, [0, 1.2, -0.6], 5, 5);
  }
}

// ---------------------------------------------------------------------------------------------
// Archetypes: parameter sets for the generator (race, variant -> figure spec)

/** Per-race touches every archetype shares (beards, hair, claws). */
function kit(race, dark, variant) {
  const k = {};
  if (race === "dwarf") {
    k.beard = vpick(variant, ["full", "braided", "full", "braided", "full"]);
    k.greaves = true;
  }
  if (race === "human") k.beard = vpick(variant, [null, "short", null, "short", "full"]);
  if (race === "elf") k.hairStyle = "long";
  if (race === "orc") k.hairStyle = vpick(variant, ["topknot", "mohawk", "none"]);
  if (race === "goblin") k.hairStyle = "none";
  if (race === "undead") {
    k.body = "bones";
    k.eyeGlow = dark ? "#b07aff" : "#7ad8ff";
  }
  if (race === "demon") k.claws = true;
  if (race === "lizard") {
    k.claws = true;
    k.barefoot = true;
  }
  return k;
}

const isBig = (race) => race === "orc" || race === "demon";

/** unit.warrior: fur and leather, an axe or club raised, a round shield forward. */
function warriorSpec({ race, dark, variant }) {
  const k = kit(race, dark, variant);
  const R = RACES[race];
  const weapon = vpick(variant, ["axe", "club", "axe", "hammer"]);
  return {
    ...k,
    stance: "brace",
    lean: 0.1,
    twist: -0.18,
    headYaw: 0.1,
    body: k.body ?? (race === "demon" || race === "lizard" ? "bare" : "fur"),
    mantle: race !== "undead" && race !== "lizard",
    skirt: R.hipY * 0.4,
    skirtColor: teamCol(0.88),
    skirtMat: MAT.TEAM,
    ragged: true,
    bracers: true,
    head: race === "undead" ? "helm" : vpick(variant, ["bare", "bare", "horned", "skull"]),
    hairStyle: k.hairStyle ?? vpick(variant, [undefined, "topknot", undefined, "mohawk"]),
    right: { hand: [0.03, 0.08, -0.005], elbow: [1, -0.15, -0.25], weapon, dir: [-0.55, 0.8, 0.12], opts: { len: 0.19, double: race === "dwarf", head: isBig(race) ? 1.25 : 1.05 } },
    left: { hand: [-0.005, -0.09, 0.1], elbow: [0.9, -0.4, -0.1], shield: "round", shieldFace: [0.32, 0.12, 1], shieldOpts: { r: race === "dwarf" ? 0.062 : 0.064 } },
  };
}

/** unit.soldier: helmet, mail and a tabard, a sword (or axe, mace) and a kite shield. */
function soldierSpec({ race, dark, variant }) {
  const k = kit(race, dark, variant);
  const R = RACES[race];
  const weapon = vpick(variant, ["sword", "axe", "mace", "sword"]);
  const sh = race === "elf" ? "heater" : race === "dwarf" ? "round" : race === "orc" || race === "lizard" ? vpick(variant, ["round", "heater"]) : vpick(variant, ["kite", "heater", "kite", "heater"]);
  return {
    ...k,
    stance: "stride",
    lean: 0.06,
    twist: 0.12,
    body: k.body ?? "armour",
    metal: race === "elf" || race === "lizard" ? undefined : garbOf(race, dark).metal2,
    tabard: race !== "undead",
    skirt: R.hipY * 0.36,
    pauldrons: race === "elf" ? "team" : true,
    gloves: true,
    greaves: race !== "undead",
    head: race === "elf" ? "helm" : race === "dwarf" ? vpick(variant, ["horned", "helm"]) : race === "orc" ? "horned" : race === "lizard" ? "bare" : vpick(variant, ["helm", "kettle", "great", "helm"]),
    plume: race === "elf" || (race === "human" && variant === 3),
    right: { hand: [0.02, -0.03, 0.085], elbow: [0.7, -0.6, -0.4], weapon, dir: [-0.5, 0.85, 0.18], opts: { len: race === "dwarf" ? 0.16 : 0.2, w: 0.024, head: 1.2 } },
    left: { hand: [-0.02, -0.1, 0.1], elbow: [0.9, -0.3, -0.2], shield: sh, shieldFace: [0.22, 0.1, 1], shieldOpts: { r: 0.072, w: 0.1, h: race === "dwarf" ? 0.15 : 0.17 } },
  };
}

/** unit.spearman: a spear upright and a tall shield. */
function spearmanSpec({ race, dark, variant }) {
  const k = kit(race, dark, variant);
  const R = RACES[race];
  const sh = race === "orc" || race === "goblin" ? "round" : race === "elf" ? "heater" : vpick(variant, ["tower", "round", "tower", "kite"]);
  const short = race === "dwarf" || race === "goblin";
  return {
    ...k,
    stance: "brace",
    lean: 0.08,
    twist: 0.08,
    body: k.body ?? (race === "demon" ? "armour" : "vest"),
    sleeves: "cloth",
    skirt: R.hipY * 0.42,
    skirtColor: teamCol(0.8),
    skirtMat: MAT.TEAM,
    pauldrons: race === "dwarf" ? true : race === "elf" ? "team" : false,
    bracers: true,
    head: race === "elf" ? "helm" : race === "lizard" ? "bare" : race === "goblin" ? "bandana" : vpick(variant, ["kettle", "helm", "helm", "kettle"]),
    plume: race === "human" && variant === 1,
    right: { hand: [0.035, -0.04, 0.06], elbow: [0.8, -0.5, -0.4], weapon: "spear", dir: [-0.1, 1, 0.2], opts: { len: short ? 0.2 : 0.23, back: short ? 0.15 : 0.22, pennon: variant % 2 === 0 } },
    left: { hand: [-0.01, -0.11, 0.1], elbow: [0.9, -0.4, -0.2], shield: sh, shieldFace: [0.16, 0.06, 1], shieldOpts: { w: 0.09, h: short ? 0.15 : 0.18, r: 0.066 } },
  };
}

/** unit.archer: a hood, a drawn longbow (side-on, arrow nocked), a quiver. */
function archerSpec({ race, dark, variant }) {
  const k = kit(race, dark, variant);
  const R = RACES[race];
  const reach = (R.shY - R.hipY) * 1.22 * R.arm * 0.95;
  const bowLen = race === "dwarf" || race === "goblin" ? 0.25 : race === "elf" ? 0.38 : 0.34;
  return {
    ...k,
    stance: "wide",
    lean: -0.02,
    twist: 0.05,
    headYaw: 0.95,
    body: k.body ?? "vest",
    sleeves: "cloth",
    skirt: R.hipY * 0.38,
    skirtColor: garbOf(race, dark).field,
    bracers: true,
    head: race === "lizard" ? "bare" : race === "undead" ? "hood" : race === "elf" ? vpick(variant, ["circlet", "hood", "circlet", "hood"]) : vpick(variant, ["hood", "hood", "deephood", "hood"]),
    cape: race === "elf",
    capeLen: 0.8,
    back: ["quiver"],
    left: { hand: [reach, 0.025, 0.0], elbow: [0.1, -1, -0.2], weapon: "bow", dir: [0.06, 1, 0.02], face: [-1, 0, 0], opts: (h) => ({ len: bowLen, recurve: race === "elf" || race === "lizard", draw: Math.max(0.08, vlen(vsub(h[L_SIDE].pos, h[R_SIDE].pos)) - 0.01) }) },
    right: { handAt: (h, S) => vadd(S, [R.shW * 1.0, 0.03, 0.025]), elbow: [1, 0.15, -0.4] },
  };
}

/** unit.mage: robe, pointed hat (or deep hood), a staff with a glowing tip. */
function mageSpec({ race, dark, variant }) {
  const k = kit(race, dark, variant);
  const R = RACES[race];
  const hood = variant === 1 || race === "undead" || race === "lizard";
  return {
    ...k,
    stance: "stand",
    lean: 0.02,
    twist: -0.15,
    body: "robe",
    robeShade: dark ? 0.6 : 0.76,
    // Hatted mages wear the owner's robe under a hat of deep dye; hooded ones (and the dead) a robe
    // of deep dye under the owner's hood, a stole of the owner's colour down the front.
    robeTeam: !hood,
    robeColor: garbOf(race, dark).hat,
    sleeves: hood ? "body" : undefined,
    stole: hood ? [C(teamCol(0.85)), MAT.TEAM] : [C(garbOf(race, dark).hat), MAT.MATTE],
    hatColor: garbOf(race, dark).hat,
    hatBand: "team",
    head: hood ? "deephood" : race === "goblin" || race === "orc" ? "hood" : "hat",
    horns: race === "goblin" || race === "orc",
    hornColor: "bone",
    hoodGlow: hood ? GLOW[race] : null,
    beard: race === "human" || race === "dwarf" ? (variant === 2 ? "short" : "full") : null,
    beardColor: race === "human" ? "#cfc8b8" : undefined,
    mantle: race === "orc" || race === "goblin",
    right: { hand: [0.05, -0.05, 0.075], elbow: [0.8, -0.7, -0.2], weapon: "staff", dir: [-0.15, 1, 0.07], opts: { len: 0.25 + R.headR * 1.5, back: R.shY - 0.05, glow: GLOW[race] } },
    left: { hand: [0.01, -0.05, 0.13], elbow: [0.5, -1, -0.1] },
  };
}

/** unit.priest: vestments (pale robe under a chasuble in the owner's colour), a mitre or hood,
 *  a holy standard raised and a censer. */
function priestSpec({ race, dark, variant }) {
  const k = kit(race, dark, variant);
  const evil = dark || race === "undead" || race === "demon";
  return {
    ...k,
    stance: "stand",
    lean: -0.02,
    twist: 0.1,
    body: "robe",
    robeTeam: false,
    robeColor: evil ? "#2a2228" : race === "elf" ? "#c8c4a0" : "#d4ccb8",
    stole: false,
    chasuble: true,
    sleeves: "body",
    hoodColor: evil ? "#2a2228" : undefined,
    head: evil ? "deephood" : race === "dwarf" ? "helm" : race === "elf" ? "circlet" : vpick(variant, ["mitre", "hood", "mitre", "bare"]),
    hoodGlow: evil ? (race === "demon" ? "#ff5020" : "#b07aff") : null,
    hoodShade: 0.7,
    beard: race === "dwarf" ? "full" : race === "human" ? vpick(variant, ["short", null, "full"]) : null,
    beardColor: race === "human" ? "#9a9488" : undefined,
    left: { hand: [0.015, 0.09, 0.04], elbow: [0.9, -0.2, -0.3], weapon: "symbol", dir: [0.04, 1, 0.04], opts: { glow: evil ? (race === "demon" ? "#ff6a2a" : "#a080ff") : "holy", metal: evil ? "#4a3a4a" : undefined } },
    right: { hand: [0.035, -0.11, 0.085], elbow: [0.8, -0.5, -0.3], weapon: "censer", dir: [0, 1, 0], opts: { glow: evil ? "#c060ff" : "holy" } },
  };
}

/** unit.worker: a tunic, a straw hat, a pick over the shoulder or a hoe, a sack. */
function workerSpec({ race, dark, variant }) {
  const k = kit(race, dark, variant);
  const R = RACES[race];
  const hoe = variant % 2 === 1;
  return {
    ...k,
    stance: hoe ? "brace" : "walk",
    lean: hoe ? 0.16 : 0.06,
    twist: hoe ? -0.2 : 0,
    body: k.body ?? "tunic",
    tunicShade: 0.78,
    skirt: R.hipY * 0.4,
    sleeves: "cloth",
    head: race === "dwarf" ? "helm" : race === "elf" ? "bare" : race === "orc" || race === "goblin" ? "bandana" : "straw",
    back: hoe ? [] : ["sack"],
    right: hoe ? { hand: [-0.01, -0.09, 0.11], elbow: [0.6, -0.6, -0.3], weapon: "hoe", dir: [0.3, 0.75, -0.6] } : { hand: [0.0, 0.015, 0.055], elbow: [0.5, -0.9, 0.4], weapon: "pick", dir: [-0.28, 0.55, -0.8] },
    left: hoe ? { handAt: (h) => vadd(h[R_SIDE].pos, vscale(vnorm([0.3, 0.75, -0.6]), 0.1)), elbow: [0.9, -0.4, 0] } : { hand: [0.015, -0.15, 0.03] },
  };
}

/** unit.scout's walker: hood and cloak in the owner's colour, a spear as a walking staff (or a
 *  bow slung). */
function scoutSpec({ race, dark, variant }) {
  const k = kit(race, dark, variant);
  const R = RACES[race];
  const bow = variant % 2 === 1;
  return {
    ...k,
    stance: "walk",
    lean: 0.05,
    body: k.body ?? "vest",
    sleeves: "cloth",
    skirt: R.hipY * 0.3,
    head: race === "lizard" ? "bare" : "hood",
    hoodShade: 0.9,
    cape: true,
    capeColor: garbOf(race, dark).field,
    capeLen: 0.85,
    back: bow ? ["quiver"] : ["bedroll"],
    right: { hand: [0.03, -0.07, 0.07], elbow: [0.8, -0.5, -0.3], weapon: "spear", dir: [-0.06, 1, 0.12], opts: { len: 0.2, back: R.shY - 0.05 } },
    left: bow ? { hand: [0.02, -0.14, 0.02], weapon: "bow", dir: [0.2, 0.1, 1], face: [0, 1, 0], opts: { len: 0.24, arrow: false, draw: 0.04 } } : { hand: [0.02, -0.15, 0.02] },
  };
}

/** unit.hero: larger, a cape, gilded armour, a raised blade and a crown or winged helm. */
function heroSpec({ race, dark, variant }) {
  const k = kit(race, dark, variant);
  const R = RACES[race];
  const g = garbOf(race, dark);
  const kind = variant % 4;
  const weapon = kind === 1 ? "axe" : kind === 2 ? "spear" : kind === 3 ? "staff" : "sword";
  const right =
    kind === 0
      ? { hand: [0.02, 0.07, 0.04], elbow: [1, -0.2, -0.1], weapon, dir: [-0.3, 0.95, 0.1], opts: { len: 0.2, w: 0.022, metal: "#c8ccd4" } }
      : kind === 1
        ? { hand: [0.03, 0.08, 0.0], elbow: [1, -0.15, -0.25], weapon, dir: [-0.5, 0.82, 0.1], opts: { len: 0.22, double: true, head: 1.3 } }
        : kind === 2
          ? { hand: [0.03, -0.03, 0.07], elbow: [0.8, -0.5, -0.4], weapon, dir: [-0.1, 1, 0.22], opts: { len: 0.24, back: 0.24, pennon: true } }
          : { hand: [0.045, -0.05, 0.075], elbow: [0.8, -0.7, -0.2], weapon, dir: [-0.08, 1, 0.06], opts: { len: 0.24, back: R.shY - 0.05, glow: GLOW[race], gem: 0.017 } };
  return {
    ...k,
    stance: kind === 3 ? "stand" : "stride",
    lean: 0.05,
    twist: 0.1,
    body: race === "undead" ? "armour" : kind === 3 ? "robe" : "plate",
    metal: race === "demon" ? "#3a3036" : race === "undead" ? "#5a5660" : "#a8acb4",
    tabard: kind !== 3,
    tabardLen: R.hipY * 0.6,
    skirt: R.hipY * 0.36,
    pauldrons: true,
    gloves: true,
    greaves: kind !== 3,
    cape: true,
    capeShade: 0.8,
    head: race === "elf" ? "circlet" : race === "dwarf" ? "horned" : race === "orc" ? "horned" : race === "demon" ? "crown" : vpick(variant, ["winged", "great", "crown", "crown"]),
    crownColor: race === "demon" ? "#3a2a2a" : "gold",
    helmTrim: "gold",
    plume: kind === 1,
    beard: race === "human" ? "short" : k.beard,
    right,
    left: kind === 0 || kind === 2 ? { hand: [-0.01, -0.1, 0.1], elbow: [0.9, -0.3, -0.2], shield: race === "dwarf" || race === "orc" ? "round" : "heater", shieldFace: [0.25, 0.1, 1], shieldOpts: { rim: "gold", w: 0.085, h: 0.14, r: 0.068 } } : kind === 1 ? { handAt: (h) => vadd(h[R_SIDE].pos, vscale(vnorm([-0.5, 0.82, 0.1]), -0.06)), elbow: [0.9, -0.5, -0.2] } : { hand: [0.06, 0.02, 0.1], elbow: [0.8, -0.8, -0.1] },
    garb: { ...g, trim: "gold" },
  };
}

const SPECS = {
  "unit.warrior": [4, warriorSpec],
  "unit.soldier": [4, soldierSpec],
  "unit.spearman": [4, spearmanSpec],
  "unit.archer": [4, archerSpec],
  "unit.mage": [4, mageSpec],
  "unit.priest": [4, priestSpec],
  "unit.worker": [2, workerSpec],
  "unit.hero": [4, heroSpec],
};

/** Heroes stand taller than their peers. */
const HERO_SCALE = 1.14;

for (const [name, [variants, make]] of Object.entries(SPECS)) {
  unitRecipe(name, {
    variants,
    build(mb, r, { variant, param }) {
      const { race, dark } = raceOf(param);
      const spec = make({ race, dark, variant, r });
      const f = figure({ r, race, dark, variant, ...spec });
      place(mb, f.mb, [0, 0, 0], 0, name === "unit.hero" ? HERO_SCALE : 1);
    },
  });
}

// ---------------------------------------------------------------------------------------------
// Quadrupeds: one generator for horses, oxen, wolves, hounds, bears, boars, great cats, elephants

/** Rotates point p about the x axis through `piv` by `a` (a leg's swing). */
function rotXAbout(p, piv, a) {
  const c = Math.cos(a), s = Math.sin(a);
  const y = p[1] - piv[1], z = p[2] - piv[2];
  return [p[0], piv[1] + y * c - z * s, piv[2] + y * s + z * c];
}

/** Body plans: len (rump to chest), h (withers), girth (body radius), chest/rump/mid radius
 *  factors, neck [len, angle, r0, r1], head [kind, len, r], leg [r, foot], tail [kind, len, r],
 *  coat/belly/mane colours. */
const BEASTS = {
  horse: { len: 0.38, h: 0.28, girth: 0.07, chest: 1.04, rump: 1.04, mid: 0.94, neck: [0.17, 1.0, 0.052, 0.03], head: ["horse", 0.12, 0.03], leg: [0.0155, "hoof"], tail: ["horse", 0.12, 0.012], coat: "#6a4630", belly: "#5a3a26", mane: "#241a14" },
  ox: { len: 0.3, h: 0.19, girth: 0.068, chest: 1.12, rump: 0.95, mid: 1.05, neck: [0.06, 0.25, 0.045, 0.036], head: ["bovine", 0.075, 0.03], leg: [0.015, "hoof"], tail: ["rope", 0.1, 0.006], coat: "#7a6048", belly: "#9a8264", mane: "#4a3a2c", horns: true },
  wolf: { len: 0.24, h: 0.17, girth: 0.05, chest: 1.3, rump: 0.86, mid: 0.82, neck: [0.06, 0.38, 0.044, 0.036], head: ["canine", 0.07, 0.035], leg: [0.0125, "paw"], tail: ["brush", 0.13, 0.025], coat: "#5e5a54", belly: "#a8a294", mane: "#2a2624", saddleBack: 0.6, ruff: 1.3 },
  hound: { len: 0.17, h: 0.125, girth: 0.033, chest: 1.18, rump: 0.86, mid: 0.76, neck: [0.05, 0.5, 0.025, 0.021], head: ["canine", 0.055, 0.024], leg: [0.0078, "paw"], tail: ["rope", 0.08, 0.006], coat: "#7a5a3a", belly: "#b09a78", mane: "#4a3624", saddleBack: 0.35 },
  bear: { len: 0.3, h: 0.21, girth: 0.072, chest: 1.08, rump: 0.98, mid: 1.0, neck: [0.05, 0.1, 0.05, 0.042], head: ["bear", 0.065, 0.04], leg: [0.02, "bear"], tail: ["stub", 0.03, 0.012], coat: "#4e3828", belly: "#3a2a1e", mane: "#6a503a", hump: 1 },
  boar: { len: 0.24, h: 0.15, girth: 0.055, chest: 1.15, rump: 0.85, mid: 0.95, neck: [0.04, 0.0, 0.045, 0.04], head: ["boar", 0.08, 0.034], leg: [0.01, "hoof"], tail: ["rope", 0.05, 0.004], coat: "#4a3a30", belly: "#3a2e26", mane: "#241c18", hump: 0.6 },
  cat: { len: 0.22, h: 0.175, girth: 0.042, chest: 1.2, rump: 0.95, mid: 0.85, neck: [0.045, 0.45, 0.034, 0.03], head: ["cat", 0.05, 0.04], leg: [0.0135, "paw"], tail: ["rope", 0.16, 0.007], coat: "#b08a52", belly: "#d8c098", mane: "#6a4a2a" },
  elephant: { len: 0.4, h: 0.4, girth: 0.12, chest: 1.05, rump: 1.0, mid: 1.0, neck: [0.04, 0.2, 0.09, 0.08], head: ["elephant", 0.1, 0.085], leg: [0.034, "pillar"], tail: ["rope", 0.1, 0.006], coat: "#6e6a66", belly: "#5a5652", mane: "#4a4644" },
};

/**
 * Builds a quadruped into `mb` (standing on y = 0, facing +z). `b`: a BEASTS entry (overridable),
 * o: {r, gait: "stand" | "walk" | "stalk" | "charge", headDown, collar, warpaint, caparison,
 *     caparisonHem, caparisonSpan, saddle, saddleColor, barding, bone, snarl, lite}. Returns anchors {saddle,
 *     withers, head, girth, yb, ...}.
 */
function quadruped(mb, b, o = {}) {
  const r = o.r;
  const L = b.len, G = b.girth;
  const yb = b.h - G; // the body's axis height
  const coat = jitter(C(b.coat), r, 0.05);
  const bellyC = C(b.belly);
  const mane = C(b.mane);
  const gait = o.gait ?? "stand";
  const bodyBone = o.bone ?? BONE.BODY;
  const lean = gait === "stalk" ? -0.08 : gait === "charge" ? 0.05 : 0;

  // --- the body: rump to chest along z (narrower than deep), shaded dark on the back -----------
  mb.bone = bodyBone;
  const rumpZ = -L / 2, chestZ = L / 2;
  const hump = b.hump ?? 0;
  const pts = [
    [0, yb + G * 0.15, rumpZ - G * 0.32],
    [0, yb + G * 0.05, rumpZ + G * 0.25],
    [0, yb - G * 0.05, 0],
    [0, yb + G * (0.08 + hump * 0.25), chestZ - G * 0.45],
    [0, yb + G * 0.02, chestZ + G * 0.25],
  ];
  const m0 = mb.mark();
  mb.color = coat;
  mb.mat = MAT.MATTE;
  mb.part(() => {
    mb.translate(0, yb, 0);
    mb.rotateX(lean);
    mb.translate(0, -yb, 0);
    mb.scale(0.82, 1, 1);
    mb.tube(pts, [G * b.rump * 0.55, G * b.rump, G * b.mid, G * b.chest, G * b.chest * 0.6], o.lite ? 7 : 9, { caps: true });
    if (hump) {
      mb.translate(0, yb + G * 0.55, chestZ - G * 0.9);
      mb.scale(1.05, 0.7, 1.4);
      mb.sphere(G * 0.75 * hump, 1);
    }
  });
  // Pale belly, darker back (painted countershading).
  const sb = b.saddleBack ?? 0;
  mb.tint(m0, (x, y) => {
    const t = Math.max(0, Math.min(1, (yb - y) / G + 0.2));
    const back = Math.max(0, Math.min(1, (y - yb) / G));
    const dk = Math.max(0, Math.min(1, (y - yb) / G + 0.35)) * sb;
    return [0, 1, 2].map((k) => (1 - t) * (1 - back * 0.25) * (1 - dk + (dk * mane[k]) / Math.max(coat[k], 1e-3)) + (t * bellyC[k]) / Math.max(coat[k], 1e-3));
  });
  if (b.stripes) {
    const sc = C(b.stripes);
    mb.tint(m0, (x, y, z) => (Math.sin(z * 95 + Math.sin(y * 60) * 1.5) > 0.45 && y > yb - G * 0.6 ? [0, 1, 2].map((k) => sc[k] / Math.max(coat[k], 1e-3)) : 1));
  }
  // Soft fur: a ruff over the shoulders of wolves and bears.
  if (b.head[0] === "canine" || b.head[0] === "bear" || b.ruff) {
    mb.color = mixc(coat, mane, 0.35);
    const seed = r.range(0, 30);
    const ru = typeof b.ruff === "number" ? b.ruff : 1;
    mb.part(() => {
      mb.translate(0, yb + G * 0.18 * ru, chestZ - G * 0.5);
      mb.scale(0.86, 1, 1);
      mb.blob(G * 1.08 * ru, { detail: 1, amp: 0.2, squash: 1.0, noise: (v) => noise3(v[0] * 3 + seed, v[1] * 3, v[2] * 3) });
    });
  }

  // --- legs -----------------------------------------------------------------------------------
  const [legR, foot] = b.leg;
  const legs = [];
  const lift = { stand: [0, 0, 0, 0], walk: [0.5, -0.25, -0.35, 0.3], stalk: [0.65, 0, 0, 0.2], charge: [0.9, 0.6, -0.5, -0.7] }[gait] ?? [0, 0, 0, 0];
  // order: front-left, front-right, hind-left, hind-right
  const defs = [[1, 1, lift[0]], [-1, 1, lift[1]], [1, -1, lift[2]], [-1, -1, lift[3]]];
  for (const [s, front, sw] of defs) {
    const x = s * G * 0.5;
    const z = front > 0 ? chestZ - G * 0.55 : rumpZ + G * 0.5;
    const top = [x, yb + G * 0.1, z];
    let P;
    if (foot === "hoof") {
      P = front > 0
        ? [top, [x * 1.08, yb - G * 0.85, z + G * 0.12], [x * 1.05, yb * 0.42, z + G * 0.1], [x, 0.03, z + G * 0.08], [x, 0.0, z + G * 0.18]]
        : [[x, yb + G * 0.2, z], [x * 1.1, yb - G * 0.6, z + G * 0.4], [x * 1.05, yb * 0.48, z - G * 0.55], [x, 0.03, z - G * 0.3], [x, 0.0, z - G * 0.2]];
    } else if (foot === "paw" || foot === "bear") {
      P = front > 0
        ? [top, [x * 1.05, yb - G * 0.9, z - G * 0.05], [x, yb * 0.22, z + G * 0.02], [x, 0.012, z + G * 0.18]]
        : [[x, yb + G * 0.15, z], [x * 1.1, yb - G * 0.55, z + G * 0.45], [x * 1.05, yb * 0.36, z - G * 0.45], [x, 0.012, z - G * 0.15]];
    } else {
      // pillar legs (elephants)
      P = [top, [x * 1.05, yb - G * 0.8, z], [x, yb * 0.3, z], [x, 0.0, z]];
    }
    // Swing the leg (sw > 0 forward and lifted, < 0 back) about its top.
    if (sw) {
      const a = -sw * 0.45;
      P = P.map((p, i) => (i === 0 ? p : rotXAbout(p, top, a)));
      if (sw > 0) {
        // Fold the lower leg back under (a lifted hoof).
        const knee = P[2];
        const fold = (front > 0 ? 1.1 : -0.9) * sw;
        P = P.map((p, i) => (i <= 2 ? p : rotXAbout(p, knee, fold)));
      }
      const lowest = Math.min(...P.map((p) => p[1]));
      if (sw < 0 && lowest > 0.0) P = P.map((p, i) => (i === 0 ? p : [p[0], p[1] - lowest * 0.8, p[2]]));
    }
    legs.push(P);
    mb.bone = bodyBone;
    const n = P.length;
    const radii = P.map((_, i) => legR * (foot === "pillar" ? 1.0 - i * 0.05 : foot === "bear" ? 1.5 - i * 0.18 : i === 0 ? 1.9 : i === 1 ? 1.25 : 1.0 - (i / n) * 0.35));
    mb.color = coat;
    mb.mat = MAT.MATTE;
    const legM = mb.mark();
    mb.tube(P, radii, o.lite ? 4 : 6, { caps: true });
    mb.tint(legM, (x, y) => (y < yb * 0.45 ? 0.82 : 1));
    const end = P[n - 1];
    if (foot === "hoof") {
      mb.color = C("#2a2220");
      mb.part(() => {
        mb.translate(end[0], end[1] - 0.012, end[2]);
        mb.cylinder(legR * 1.15, legR * 0.95, 0.024, o.lite ? 4 : 6);
      });
      if (b.feathers) {
        mb.color = C(b.feathers);
        mb.part(() => {
          mb.translate(end[0], end[1] + 0.008, end[2]);
          mb.cylinder(legR * 1.6, legR * 1.1, 0.03, 6);
        });
      }
    } else if (foot === "paw" || foot === "bear") {
      mb.color = mixc(coat, mane, 0.4);
      ball(mb, [end[0], Math.max(0.0, end[1]) - 0.004, end[2] + legR * 0.4], legR * (foot === "bear" ? 1.5 : 1.35), 0, [1, 0.6, 1.4]);
    } else {
      mb.color = C("#4a4642");
      mb.part(() => {
        mb.translate(end[0], -0.012, end[2]);
        mb.cylinder(legR * 1.15, legR * 1.1, 0.02, 7);
      });
    }
  }

  // --- tail -----------------------------------------------------------------------------------
  mb.bone = BONE.TAIL;
  const [tk, tl, tr] = b.tail;
  const tb = [0, yb + G * 0.25, rumpZ - G * 0.15];
  if (tk === "horse") {
    mb.color = mane;
    mb.tube([tb, vadd(tb, [0, -0.02, -0.035]), vadd(tb, [0.01, -tl * 0.6, -0.055]), vadd(tb, [0.015, -tl, -0.05])], [tr * 0.8, tr * 1.3, tr * 1.1, tr * 0.4], 5, { caps: true });
  } else if (tk === "brush") {
    mb.color = mixc(coat, mane, 0.3);
    const up = gait === "charge" ? 0.4 : gait === "stalk" ? -0.2 : 0;
    mb.tube([tb, vadd(tb, [0, -0.01 + up * 0.04, -tl * 0.4]), vadd(tb, [0.01, -0.045 + up * 0.08, -tl * 0.8]), vadd(tb, [0.016, -0.07 + up * 0.1, -tl])], [tr * 0.6, tr * 1.2, tr * 1.0, tr * 0.3], 6, { caps: true });
    mb.color = mixc(coat, "#e0dcd0", 0.5);
    ball(mb, vadd(tb, [0.016, -0.07 + up * 0.1, -tl]), tr * 0.45, 0);
  } else if (tk === "rope") {
    mb.color = coat;
    const up = gait === "stalk" ? 0.5 : 0;
    mb.tube([tb, vadd(tb, [0, -tl * 0.3 + up * 0.03, -tl * 0.4]), vadd(tb, [0.01, -tl * 0.6 + up * 0.06, -tl * 0.75]), vadd(tb, [0.02, -tl * 0.65 + up * 0.11, -tl])], [tr, tr * 0.9, tr * 0.8, tr * 0.6], 4, { caps: true });
    mb.color = mane;
    ball(mb, vadd(tb, [0.02, -tl * 0.65 + up * 0.11, -tl]), tr * 1.6, 0);
  } else {
    mb.color = coat;
    ball(mb, vadd(tb, [0, -0.005, -0.008]), tr, 0);
  }

  // --- neck and head --------------------------------------------------------------------------
  const headM = mb.mark();
  mb.bone = BONE.HEAD;
  const [nl, na, nr0, nr1] = b.neck;
  const nb = [0, yb + G * 0.3, chestZ - G * 0.15];
  const headDown = (o.headDown ?? 0) + (gait === "stalk" ? 0.3 : gait === "charge" ? 0.15 : 0);
  const ang = na - headDown;
  const nd = [0, Math.sin(ang), Math.cos(ang)];
  const ne = vadd(nb, vscale(nd, nl));
  mb.color = coat;
  const nm = mb.mark();
  if (b.head[0] === "horse") {
    // A horse's neck: a deep blade, flat from side to side, arched along its crest.
    local(mb, (q) => {
      q.tube([vadd(nb, vscale(nd, -0.03)), vadd(vadd(nb, vscale(nd, nl * 0.45)), [0, 0.006, 0]), ne], [nr0 * 1.1, (nr0 + nr1) * 0.52, nr1], o.lite ? 6 : 8, { caps: true });
      q.displace(0, (x) => [-x * 0.3, 0, 0]);
    });
  } else mb.tube([vadd(nb, vscale(nd, -0.02)), vadd(nb, vscale(nd, nl * 0.5)), ne], [nr0 * 1.1, (nr0 + nr1) / 2, nr1], o.lite ? 5 : 7, { caps: true });
  mb.sphereNormals(nm, vadd(nb, [0, -0.02, 0]), 0.2);
  const [hk, hl, hr] = b.head;
  const hang = (hk === "horse" ? -1.0 : hk === "bovine" ? -0.9 : hk === "elephant" ? -0.6 : -0.25) - headDown * 0.6;
  const hd = [0, Math.sin(hang), Math.cos(hang)];
  const hs = vadd(ne, vscale(nd, hr * 0.3)); // the skull's back
  const muzzle = vadd(hs, vscale(hd, hl));
  mb.color = coat;
  if (hk === "horse") {
    mb.tube([vadd(hs, vscale(hd, -0.01)), vadd(hs, vscale(hd, hl * 0.35)), vadd(hs, vscale(hd, hl * 0.8)), muzzle], [hr * 1.0, hr * 0.92, hr * 0.68, hr * 0.6], 7, { caps: true });
    // Jowls: the broad cheek under the eye.
    ball(mb, vadd(vadd(hs, vscale(hd, hl * 0.16)), [0, -hr * 0.45, -hr * 0.2]), hr * 1.08, 1, [0.78, 1, 1.1]);
    mb.color = mixc(coat, "#1a1412", 0.5);
    ball(mb, muzzle, hr * 0.62, 1);
    // Mane along the neck's crest.
    mb.color = mane;
    const crest = (t) => vadd(vlerp(nb, ne, t), [0, nr0 * (1 - t) + nr1 * t, -0.004]);
    mb.tube([crest(0.05), crest(0.4), crest(0.75), crest(1.02)], [0.009, 0.012, 0.011, 0.008], 4, { caps: true });
    // Ears.
    mb.color = coat;
    for (const s of [-1, 1]) horn(mb, vadd(hs, [s * hr * 0.5, hr * 0.7, -0.004]), [s * 0.25, 1, 0.1], 0.024, 0.007, [0, 0, 0.3], 3, 4);
  } else if (hk === "canine" || hk === "cat") {
    ball(mb, vadd(hs, vscale(hd, hr * 0.5)), hr * 1.12, 1, [1, 0.92, 1.05]);
    const snout = hk === "cat" ? 0.45 : 1;
    mb.tube([vadd(hs, vscale(hd, hr * 0.8)), vadd(hs, vscale(hd, hr * 0.8 + hl * 0.5 * snout)), vadd(vadd(hs, vscale(hd, hr * 0.8 + hl * snout)), [0, -hr * 0.2, 0])], [hr * 0.7, hr * 0.5, hr * 0.32], 6, { caps: true });
    mb.color = C("#161212");
    ball(mb, vadd(vadd(hs, vscale(hd, hr * 0.8 + hl * snout)), [0, -hr * 0.16, 0.002]), hr * 0.22, 0);
    mb.color = coat;
    for (const s of [-1, 1]) horn(mb, vadd(hs, [s * hr * 0.55, hr * 0.75, hr * 0.1]), [s * 0.35, 1, -0.15], hr * (hk === "cat" ? 0.6 : 1.05), hr * 0.32, [0, 0, -0.2], 3, 4);
    if (hk === "cat" && b.maneful) {
      mb.color = mane;
      const seed = r.range(0, 20);
      mb.part(() => {
        mb.translate(hs[0], hs[1] - hr * 0.25, hs[2] - hr * 0.55);
        mb.scale(1, 1.05, 0.9);
        mb.blob(hr * 1.62, { detail: 1, amp: 0.24, noise: (v) => noise3(v[0] * 3 + seed, v[1] * 3, v[2] * 3) });
      });
    }
    if (o.snarl) {
      mb.color = C("#e8e0cc");
      for (const s of [-1, 1]) horn(mb, vadd(vadd(hs, vscale(hd, hr * 0.8 + hl * 0.8)), [s * hr * 0.25, -hr * 0.3, 0]), [0, -1, 0.2], hr * 0.35, hr * 0.08, [0, 0, 0], 2, 3);
    }
  } else if (hk === "bear") {
    ball(mb, vadd(hs, vscale(hd, hr * 0.4)), hr * 1.1, 1);
    mb.tube([vadd(hs, vscale(hd, hr * 0.9)), vadd(vadd(hs, vscale(hd, hr * 0.9 + hl * 0.55)), [0, -hr * 0.12, 0])], [hr * 0.62, hr * 0.42], 6, { caps: true });
    mb.color = C("#161212");
    ball(mb, vadd(vadd(hs, vscale(hd, hr * 0.9 + hl * 0.55)), [0, -hr * 0.1, 0.003]), hr * 0.22, 0);
    mb.color = mixc(coat, mane, 0.4);
    for (const s of [-1, 1]) ball(mb, vadd(hs, [s * hr * 0.75, hr * 0.85, 0]), hr * 0.32, 0, [1, 1, 0.6]);
  } else if (hk === "bovine") {
    ball(mb, vadd(hs, vscale(hd, hr * 0.4)), hr * 1.05, o.lite ? 0 : 1, [1.15, 1, 1]);
    mb.tube([vadd(hs, vscale(hd, hr * 0.6)), vadd(hs, vscale(hd, hl))], [hr * 0.9, hr * 0.75], 6, { caps: true });
    mb.color = mixc(coat, "#2a2220", 0.5);
    ball(mb, vadd(hs, vscale(hd, hl)), hr * 0.72, o.lite ? 0 : 1, [1.1, 0.8, 0.8]);
    if (b.horns) {
      mb.color = C("#d8ccb0");
      for (const s of [-1, 1]) horn(mb, vadd(hs, [s * hr * 0.85, hr * 0.55, 0]), [s, 0.15, 0.2], hr * 1.9, hr * 0.28, [0, 1.2, 0.6], o.lite ? 3 : 4, o.lite ? 4 : 5);
    }
  } else if (hk === "boar") {
    mb.tube([vadd(hs, vscale(hd, -0.005)), vadd(hs, vscale(hd, hl * 0.5)), vadd(hs, vscale(hd, hl))], [hr * 1.05, hr * 0.8, hr * 0.45], 7, { caps: true });
    mb.color = C("#2a2220");
    ball(mb, vadd(hs, vscale(hd, hl)), hr * 0.42, 0, [1.1, 1, 0.5]);
    mb.color = C("#e8e0cc");
    for (const s of [-1, 1]) horn(mb, vadd(hs, vadd(vscale(hd, hl * 0.82), [s * hr * 0.4, -hr * 0.2, 0])), [s * 0.4, 1, 0.3], hr * 0.7, hr * 0.12, [0, 0, -0.8], 3, 3);
    mb.color = coat;
    for (const s of [-1, 1]) horn(mb, vadd(hs, [s * hr * 0.55, hr * 0.7, 0]), [s * 0.4, 1, -0.3], hr * 0.6, hr * 0.25, [0, 0, 0], 2, 4);
    // A bristling crest.
    mb.color = mane;
    mb.tube([vadd(nb, [0, nr0 * 1.1, -0.02]), vadd(ne, [0, nr1 * 1.2, 0]), vadd(hs, [0, hr * 0.95, 0])], [0.008, 0.01, 0.006], 4, { caps: true });
  } else if (hk === "elephant") {
    ball(mb, vadd(hs, vscale(hd, hr * 0.3)), hr * 1.0, 1, [1, 1.05, 1]);
    // Trunk curling down, tusks, great ears.
    mb.tube([vadd(hs, vscale(hd, hr * 0.85)), vadd(hs, [0, -hr * 0.9, hr * 1.3]), vadd(hs, [0, -hr * 2.0, hr * 1.35]), vadd(hs, [0, -hr * 2.7, hr * 1.6])], [hr * 0.38, hr * 0.28, hr * 0.2, hr * 0.15], 6, { caps: true });
    mb.color = C("#e8e0cc");
    for (const s of [-1, 1]) horn(mb, vadd(hs, [s * hr * 0.4, -hr * 0.6, hr * 0.8]), [s * 0.2, -0.4, 1], hr * 1.3, hr * 0.14, [0, 1.2, 0], 4, 4);
    mb.color = mixc(coat, "#a07a70", 0.15);
    for (const s of [-1, 1]) {
      mb.part(() => {
        mb.translate(hs[0] + s * hr * 0.9, hs[1] + hr * 0.1, hs[2] - hr * 0.1);
        mb.rotateY(s * 0.5);
        slab(mb, [[0, hr * 0.6], [s * hr * 0.9, hr * 0.7], [s * hr * 1.2, -hr * 0.1], [s * hr * 0.7, -hr * 1.0], [0, -hr * 0.6]], 0.006);
      });
    }
  }

  // A centaur's horse body has no neck or head of its own (the rider's torso rises there).
  if (o.noHead) mb.v.length = headM * 13;

  // --- trappings: collar, war paint, caparison, saddle, barding --------------------------------
  const anchors = { saddle: [0, yb + G * 0.95, -G * 0.15], withers: [0, b.h, chestZ - G * 0.5], head: hs, neckBase: nb, neckEnd: ne, girth: G, yb, chestZ, rumpZ, legs, muzzle };
  if (o.collar) {
    mb.bone = BONE.HEAD;
    setTeam(mb, 0.92);
    mb.part(() => {
      const c = vlerp(nb, ne, 0.55);
      orient(mb, c, nd, [0, 0, 1]);
      ringTube(mb, (nr0 + nr1) * 0.55, (nr0 + nr1) * 0.55, 0.0065, 10, 4);
    });
    setMat(mb, "gold", MAT.METAL);
    mb.part(() => {
      const c = vadd(vlerp(nb, ne, 0.55), [0, -(nr0 + nr1) * 0.5, 0.004]);
      mb.translate(c[0], c[1], c[2]);
      mb.sphere(0.006, 0);
    });
  }
  if (o.warpaint) {
    // A slash of the owner's colour across each flank (beasts with no harness).
    mb.bone = bodyBone;
    setTeam(mb, 0.95);
    for (const s of [-1, 1]) {
      mb.part(() => {
        mb.translate(s * G * 0.8, yb + G * 0.15, 0);
        mb.rotateY((s * Math.PI) / 2);
        mb.rotateZ(0.5);
        mb.box(L * 0.42, G * 0.32, 0.004);
      });
    }
  }
  if (o.caparison) {
    // A cloth over the back falling to the knees, in the owner's colour with a dark hem.
    mb.bone = bodyBone;
    const hem = o.caparisonHem ?? yb * 0.45;
    const nz = 6, na2 = 7;
    // `caparisonSpan` < 1: a saddle cloth over the middle of the back instead of a full caparison.
    const span = o.caparisonSpan ?? 1;
    const zc = (rumpZ - G * 0.1 + chestZ + G * 0.05) * 0.5, zh = (chestZ + G * 0.05 - (rumpZ - G * 0.1)) * 0.5 * span;
    const z0 = zc - zh, z1 = zc + zh;
    const prof = (z) => {
      const t = (z - z0) / (z1 - z0);
      return G * (0.92 + 0.12 * Math.sin(t * Math.PI)) * 1.06;
    };
    const P = (i, j) => {
      const z = z0 + ((z1 - z0) * i) / nz;
      const rr = prof(z);
      const v = j / na2; // 0 = right hem, 0.5 = spine, 1 = left hem
      const a = (v - 0.5) * Math.PI * 1.1; // around the back
      const side = Math.sign(a) || 1;
      if (Math.abs(a) <= Math.PI / 2) return [Math.sin(a) * rr * 0.86, yb + Math.cos(a) * rr + 0.004, z];
      const k = (Math.abs(a) - Math.PI / 2) / (Math.PI * 0.05);
      return [side * rr * (0.9 + 0.22 * k), yb - k * (yb - hem) + (k > 0.5 ? 0.014 * Math.sin(i * 2.6 + 0.5) : 0), z];
    };
    for (let i = 0; i < nz; i++) {
      for (let j = 0; j < na2; j++) {
        const edge = j === 0 || j === na2 - 1;
        mb.mat = MAT.TEAM;
        mb.color = teamCol(edge ? 0.5 : 0.92);
        mb.quad(P(i, j), P(i + 1, j), P(i + 1, j + 1), P(i, j + 1));
      }
    }
  }
  if (o.saddle) {
    mb.bone = bodyBone;
    setMat(mb, o.saddleColor ?? "leatherDark");
    mb.part(() => {
      mb.translate(anchors.saddle[0], anchors.saddle[1] - 0.012, anchors.saddle[2]);
      mb.scale(1, 0.5, 1.3);
      mb.lathe([[G * 0.62, 0], [G * 0.66, 0.02], [G * 0.4, 0.035], [0, 0.04]], 8);
    });
  }
  if (o.barding) {
    // A metal chanfron on the face.
    mb.bone = BONE.HEAD;
    setMat(mb, o.barding === true ? "steel" : o.barding, MAT.METAL);
    mb.part(() => {
      orient(mb, vadd(hs, vscale(hd, hl * 0.4)), hd, [0, 1, 0]);
      mb.translate(0, -hl * 0.35, hr * 0.55);
      mb.box(hr * 1.3, hl * 0.75, hr * 0.5, { top: 0.6 });
    });
  }
  mb.bone = BONE.BODY;
  mb.heightAO(0, 0.0, yb, 0.6);
  return anchors;
}

/** A beast recipe on the quadruped generator: variants recolour and re-pose it; seen
 *  three-quarters on (`o.yaw`), scaled up to the map's monster scale (`o.scale`). */
function beastRecipe(name, kind, variants, tweak) {
  unitRecipe(name, {
    variants,
    build(mb, r, { variant, param }) {
      const b = { ...BEASTS[kind] };
      const o = { r, gait: "stand", collar: true };
      tweak?.(b, o, variant, param, r);
      mb.part(() => {
        mb.rotateY(o.yaw ?? 0.7);
        if (o.scale) mb.scale(o.scale);
        local(mb, (k) => {
          quadruped(k, b, o);
          o.after?.(k);
        });
      });
    },
  });
}

/** unit.wolf: a grey wolf stalking, head low, collared in the owner's colour; colours by
 *  variant (grey, dark, pale, winter), param "winter" | "hell" (a hellhound, ember eyes). */
beastRecipe("unit.wolf", "wolf", 4, (b, o, v, param) => {
  const coats = [["#5e5a54", "#aaa494", "#24201e"], ["#3a3634", "#7a746a", "#141210"], ["#7a6c58", "#c0b49c", "#3a3026"], ["#aeaeaa", "#e0ded8", "#6a6a68"]];
  const [c, bl, m] = param === "winter" ? coats[3] : coats[v % 4];
  b.coat = c;
  b.belly = bl;
  b.mane = m;
  o.gait = v % 2 ? "walk" : "stalk";
  o.snarl = true;
  o.scale = 1.5;
  if (param === "hell") {
    b.coat = "#3a1a14";
    b.belly = "#6a2a18";
    b.mane = "#120a08";
    o.after = (mb) => {
      mb.mat = MAT.EMISSIVE;
      mb.color = C("#ff6a1a");
      for (const s of [-1, 1]) ball(mb, [s * 0.013, 0.14, 0.205], 0.0045, 0);
    };
  }
});

/** unit.bear: a heavy brown bear on all fours (param "polar"). */
beastRecipe("unit.bear", "bear", 3, (b, o, v, param) => {
  if (param === "polar") {
    b.coat = "#d8d4c8";
    b.belly = "#c0baa8";
    b.mane = "#e8e4d8";
  } else {
    [b.coat, b.belly, b.mane] = [["#4e3828", "#3a2a1e", "#6a503a"], ["#3a2c22", "#2a1e16", "#56402e"], ["#5e4a36", "#4a3a2a", "#7a6248"]][v % 3];
  }
  o.gait = v === 1 ? "walk" : "stand";
  o.headDown = 0.15;
  o.scale = 1.45;
});

// ---------------------------------------------------------------------------------------------
// Riders, mounts and wagons

/** Mounts by race: horses for most, wolves for goblins and orcs, boars for dwarves, a nightmare
 *  for demons and the dead (`mod` "boar" | "wolf" overrides). */
function mountOf(race, variant, dark, mod = "") {
  if (mod === "boar") race = "dwarf";
  if (mod === "wolf") race = "goblin";
  const horseCoats = [["#6a4630", "#5a3a26", "#241a14"], ["#2e2622", "#26201c", "#141010"], ["#9a9890", "#8a8880", "#d8d4cc"], ["#8a5a32", "#7a4a28", "#3a2416"]];
  if (race === "goblin" || race === "orc") {
    const b = { ...BEASTS.wolf, len: 0.3, h: 0.2, girth: 0.052, neck: [0.08, 0.45, 0.032, 0.027], head: ["canine", 0.085, 0.032], leg: [0.011, "paw"], tail: ["brush", 0.14, 0.016], coat: "#4a4440", belly: "#8a8274", mane: "#24201e" };
    if (race === "orc") Object.assign(b, { len: 0.34, h: 0.23, girth: 0.06, neck: [0.09, 0.45, 0.036, 0.03], head: ["canine", 0.095, 0.036], leg: [0.013, "paw"] });
    return { b, wolf: true };
  }
  if (race === "dwarf") return { b: { ...BEASTS.boar, len: 0.3, h: 0.19, girth: 0.066, neck: [0.04, 0.0, 0.055, 0.05], head: ["boar", 0.09, 0.04], leg: [0.013, "hoof"] }, boar: true };
  const b = { ...BEASTS.horse };
  const [c, bl, m] = horseCoats[variant % 4];
  b.coat = c;
  b.belly = bl;
  b.mane = m;
  if (race === "undead" || race === "demon" || dark) {
    b.coat = race === "undead" ? "#3a3634" : "#221818";
    b.belly = "#1a1414";
    b.mane = race === "demon" ? "#3a1410" : "#121010";
  }
  if (race === "elf") {
    b.coat = "#d8d4c8";
    b.belly = "#c8c4b8";
    b.mane = "#f0ece4";
  }
  if (race === "human" && variant % 4 === 3) b.feathers = "#e8e4dc";
  return { b };
}

/** unit.cavalry: a horse (wolf, boar, nightmare) in a caparison of the owner's colour and its
 *  rider: variant 0 a lance upright with a pennon, 1 a sword raised at the charge, 2 a drawn bow
 *  (horse archer), 3 a heavy knight in a great helm. Seen three-quarters on. */
unitRecipe("unit.cavalry", {
  variants: 4,
  build(mb, r, { variant, param }) {
    const { race, dark, mod } = raceOf(param);
    const kind = variant % 4;
    const { b, wolf } = mountOf(race, variant, dark, mod);
    const o = { r, gait: kind === 1 ? "charge" : "walk", caparison: !wolf && kind !== 2, caparisonHem: kind === 3 ? b.h * 0.42 : (b.h - b.girth) - b.girth * 0.85, caparisonSpan: kind === 3 ? 0.9 : 0.5, saddle: true, barding: kind === 0 || kind === 3 ? (race === "demon" ? "#3a3036" : "steel") : false, collar: !!wolf, bone: BONE.MOUNT };
    const R = RACES[race];
    const k = kit(race, dark, variant);
    const spec =
      kind === 2
        ? {
            ...k,
            body: "vest",
            sleeves: "team",
            skirt: R.hipY * 0.3,
            skirtColor: teamCol(0.85),
            skirtMat: MAT.TEAM,
            head: race === "elf" ? "circlet" : "hood",
            twist: 0.2,
            headYaw: 0.9,
            back: ["quiver"],
            cape: true,
            capeLen: 0.6,
            left: { hand: [(R.shY - R.hipY) * 1.15, 0.03, 0.02], elbow: [0.1, -1, -0.2], weapon: "bow", dir: [0.06, 1, 0.02], face: [-1, 0, 0], opts: (h) => ({ len: 0.28, recurve: true, draw: Math.max(0.08, vlen(vsub(h[L_SIDE].pos, h[R_SIDE].pos)) - 0.01) }) },
            right: { handAt: (h, S) => vadd(S, [R.shW * 1.0, 0.03, 0.03]), elbow: [1, 0.15, -0.4] },
          }
        : {
            ...k,
            body: "armour",
            tabard: race !== "undead" && kind !== 3,
            tabardLen: R.hipY * 0.35,
            pauldrons: true,
            gloves: true,
            cape: kind === 1,
            capeLen: 0.55,
            head: kind === 3 ? "great" : race === "elf" ? "helm" : race === "orc" || race === "dwarf" ? "horned" : "helm",
            plume: kind !== 1,
            twist: kind === 1 ? -0.15 : 0.05,
            right: kind === 1 ? { hand: [0.03, 0.1, 0.02], elbow: [1, -0.2, -0.1], weapon: race === "orc" || race === "dwarf" ? "axe" : "sword", dir: [-0.4, 0.88, 0.2], opts: { len: 0.19, w: 0.02, head: 1.2 } } : { hand: [0.035, -0.05, 0.06], elbow: [0.8, -0.5, -0.4], weapon: "lance", dir: [-0.12, 1, 0.3], opts: { len: 0.36 } },
            left: { hand: [-0.01, -0.08, 0.09], elbow: [0.9, -0.3, -0.2], shield: race === "orc" || race === "dwarf" ? "round" : "heater", shieldFace: [0.6, 0.1, 0.8], shieldOpts: { w: 0.075, h: 0.12, r: 0.058 } },
          };
    mb.part(() => {
      mb.rotateY(0.75);
      mb.translate(0, 0, 0.02);
      local(mb, (k2) => {
        const a = quadruped(k2, b, o);
        const f = figure({ r, race, dark, variant, ...spec, seat: { w: a.girth * 0.78 } });
        place(k2, f.mb, [0, a.saddle[1] + 0.006 - R.hipY, a.saddle[2] + 0.005]);
      });
    });
  },
});

/** unit.scout: a hooded, cloaked walker with a spear (or bow) and a hound at its side. */
unitRecipe("unit.scout", {
  variants: 4,
  build(mb, r, { variant, param }) {
    const { race, dark } = raceOf(param);
    const spec = scoutSpec({ race, dark, variant, r });
    const f = figure({ r, race, dark, variant, ...spec });
    place(mb, f.mb, [-0.035, 0, -0.02], -0.15);
    const hb = { ...BEASTS.hound };
    if (race === "goblin" || race === "orc") Object.assign(hb, { coat: "#3a3634", belly: "#6a645c", mane: "#1a1816" });
    else hb.coat = ["#7a5a3a", "#4a4440", "#b0a088", "#5a3a26"][variant % 4];
    mb.part(() => {
      mb.translate(0.11, 0, 0.06);
      mb.rotateY(0.6);
      mb.scale(1.35);
      local(mb, (k) => quadruped(k, hb, { r, gait: "walk", collar: true, lite: true }));
    });
  },
});

/** A covered wagon (box bed, four spoked wheels, canvas on hoops with the owner's stripes). */
function wagon(mb, r, g, { len = 0.22, w = 0.12, team = true } = {}) {
  const bedY = 0.07;
  setMat(mb, "plank");
  mb.part(() => {
    mb.translate(0, bedY, 0);
    mb.box(w, 0.036, len);
  });
  setMat(mb, "timber");
  for (const s of [-1, 1]) {
    mb.part(() => {
      mb.translate(s * w * 0.5, bedY + 0.004, 0);
      mb.box(0.008, 0.04, len * 1.02);
    });
  }
  // Wheels.
  for (const [sx, sz] of [[-1, -1], [1, -1], [-1, 1], [1, 1]]) {
    const rad = sz > 0 ? 0.04 : 0.05;
    mb.part(() => {
      mb.translate(sx * (w * 0.5 + 0.012), rad - 0.006, sz * len * 0.33);
      mb.rotateZ(Math.PI / 2);
      setMat(mb, "#3a2a1e");
      mb.translate(0, -0.006, 0);
      mb.lathe([[rad * 0.8, 0], [rad, 0.0], [rad, 0.012], [rad * 0.8, 0.012]], 7, { caps: false });
      mb.translate(0, 0.006, 0);
      setMat(mb, "timber");
      mb.cylinder(0.008, 0.008, 0.016, 5);
      for (let i = 0; i < 2; i++) {
        mb.part(() => {
          mb.rotateY((i / 2) * Math.PI + 0.4);
          mb.translate(0, 0.006, 0);
          mb.rotateX(Math.PI / 2);
          mb.translate(0, -rad * 0.85, 0);
          mb.cylinder(0.0028, 0.0028, rad * 1.7, 3, { caps: false });
        });
      }
    });
  }
  // The canvas: hoops over the bed, pale cloth with the owner's colour in bands.
  const hoopR = w * 0.56, z0 = -len * 0.48, z1 = len * 0.36, nz = 4, na = 8;
  const P = (i, j) => {
    const z = z0 + ((z1 - z0) * i) / nz;
    const a = Math.PI * (j / na);
    return [Math.cos(a) * hoopR, bedY + 0.03 + Math.sin(a) * hoopR * 1.15, z];
  };
  const N = (j) => [Math.cos(Math.PI * (j / na)), Math.sin(Math.PI * (j / na)), 0];
  for (let i = 0; i < nz; i++) {
    for (let j = 0; j < na; j++) {
      // One narrow stripe of the owner's colour along each side: the canvas stays canvas (a solid
      // team-coloured cover read as a giant barrel at the gameplay zoom).
      const band = team && (j === 2 || j === na - 3) && i > 0;
      if (band) setTeam(mb, 0.9);
      else setMat(mb, "#cfc3a4");
      // (wound so the explicit outward normals face the same way as the triangles)
      mb.quadN(P(i, j), P(i, j + 1), P(i + 1, j + 1), P(i + 1, j), N(j), N(j + 1), N(j + 1), N(j));
    }
  }
  // The back is closed (the open front shows the canvas's shadowed inside).
  setMat(mb, "#cfc3a4");
  mb.part(() => {
    mb.translate(0, bedY + 0.03, z0);
    mb.scale(1, 1.15, 1);
    mb.rotateX(Math.PI / 2);
    mb.cylinder(hoopR, hoopR * 0.9, 0.01, 6, { caps: true });
  });
  setMat(mb, "timber");
  for (const zz of [z0, z1]) {
    mb.part(() => {
      mb.translate(0, bedY + 0.03, zz);
      mb.scale(1, 1.15, 1);
      ringTube(mb, hoopR + 0.002, 0.0001, 0.004, 6, 3);
    });
  }
  // The shaft to the yoke.
  setMat(mb, "timber");
  for (const s of [-1, 1]) mb.tube([[s * w * 0.3, bedY + 0.01, len * 0.5], [s * 0.035, bedY + 0.02, len * 0.5 + 0.12]], 0.004, 3);
  return { front: len * 0.5 + 0.12, bedY };
}

/** unit.settler: a covered wagon drawn by an ox, a settler walking beside it with a staff. */
unitRecipe("unit.settler", {
  variants: 2,
  build(mb, r, { variant, param }) {
    const { race, dark } = raceOf(param);
    const g = garbOf(race, dark);
    mb.part(() => {
      mb.rotateY(1.0);
      mb.translate(0, 0, -0.08);
      // The wagon and its ox a little under life size against the walker (the unit's mass should
      // match the other figures', not tower over them), but big enough to read as the settlers'
      // covered wagon, the unit's emblem.
      mb.scale(0.9);
      local(mb, (k) => {
        const wg = wagon(k, r, g, {});
        const ox = { ...BEASTS.ox, len: 0.22, h: 0.16, girth: 0.055 };
        if (race === "orc" || race === "goblin") Object.assign(ox, { coat: "#4a3a30", belly: "#6a5848" });
        k.part(() => {
          k.translate(0, 0, wg.front + 0.1);
          local(k, (q) => quadruped(q, ox, { r, gait: "walk", headDown: 0.2, lite: true }));
        });
        // The yoke.
        setMat(k, "timber");
        k.part(() => {
          k.translate(0, 0.15, wg.front + 0.175);
          k.rotateZ(Math.PI / 2);
          k.translate(0, -0.05, 0);
          k.cylinder(0.006, 0.006, 0.1, 4);
        });
      });
    });
    // The settler walks ahead on the near side, staff in hand, a bundle on the back.
    const R = RACES[race];
    const k2 = kit(race, dark, variant);
    const f = figure({
      r, race, dark, variant, ...k2,
      lite: true,
      claws: false,
      greaves: false,
      stance: "walk",
      lean: 0.06,
      body: k2.body ?? "tunic",
      skirt: R.hipY * 0.45,
      head: race === "elf" ? "bare" : race === "dwarf" ? "helm" : "hood",
      hoodShade: 0.85,
      back: ["bedroll"],
      right: { hand: [0.03, -0.06, 0.07], weapon: "staff", dir: [-0.05, 1, 0.1], opts: { len: 0.14, back: R.shY - 0.05, plain: true } },
    });
    place(mb, f.mb, [0.17, 0, 0.13], 0.6, 0.92);
  },
});

// ---------------------------------------------------------------------------------------------
// Wings (dragons, demons, imps, angels, birds)

/**
 * A wing on side `s` (+1 left / +x, -1 right / -x) from `root`: `span` its reach, `lift` how high
 * the wrist rises, `sweep` how far back the tips trail. Bat wings (membrane between finger spars,
 * scalloped trailing edge) or feathered (fanned cards). Drawn with BONE.WING_L / WING_R so the
 * shader beats it (the beat grows with |x|, so the root stays nearly put).
 */
function wing(mb, s, root, o = {}) {
  const span = o.span ?? 0.5, lift = o.lift ?? 0.2, sweep = o.sweep ?? 0.15;
  const fingers = o.fingers ?? 4;
  const boneC = C(o.bone ?? "#3a1a16"), memC = C(o.membrane ?? "#5a1a14");
  const prev = mb.bone;
  mb.bone = s > 0 ? BONE.WING_L : BONE.WING_R;
  const w0 = mb.mark();
  // The beat's weight per vertex (the shader reads the sway channel of wing bones): 0 at the root,
  // growing with the distance from it (absolute, so small wings beat less far than great ones).
  const flapWeights = () => {
    const v = mb.v;
    for (let i = w0 * 13; i < v.length; i += 13) v[i + 10] = Math.max(0.02, Math.min(1, Math.hypot(v[i] - root[0], v[i + 1] - root[1], v[i + 2] - root[2]) / 0.6));
  };
  const P = (x, y, z) => [root[0] + s * x * span, root[1] + y, root[2] + z];
  const elbow = P(0.42, lift * 0.75, -sweep * 0.05);
  const wrist = P(0.78, lift, -sweep * 0.25);
  const tips = [];
  for (let i = 0; i < fingers; i++) {
    const t = i / Math.max(1, fingers - 1);
    // Fingers fan from forward-out to back-down.
    tips.push(P(1.0 - t * 0.38, lift * (0.9 - t * 1.25) - 0.02, -sweep * (0.45 + t * 1.15)));
  }
  const body0 = P(0.04, -0.02, -sweep * 0.2 - (o.chord ?? 0.25) * span); // where the membrane meets the flank
  if (o.feathers) {
    // Feathered: an arm and rows of feather cards fanning back from it.
    mb.color = boneC;
    mb.mat = MAT.MATTE;
    mb.tube([root, elbow, wrist, tips[0]], [0.016, 0.013, 0.01, 0.004], 5, { caps: true });
    // Two layers: long overlapping flight feathers with rounded tips, and short broad coverts over
    // their roots (a solid wing mass at the gameplay zoom, not a fan of sticks).
    const rows = o.rows ?? 10;
    for (const layer of [0, 1]) {
      const n = layer ? rows - 2 : rows;
      for (let i = 0; i < n; i++) {
        const t = i / (n - 1);
        const base = vadd(t < 0.5 ? vlerp(root, wrist, t * 2) : vlerp(wrist, tips[0], (t - 0.5) * 2), [0, layer * 0.006, 0]);
        const len = (layer ? 0.42 : 1) * (0.2 + 0.34 * Math.sin(t * Math.PI * 0.9)) * span;
        // Feathers hang down and back from the arm, so the wing shows its face to the viewer
        // (fanned straight back they were seen edge-on: a stick with a tuft at the end).
        const hang = o.hang ?? 0.65; // 0: spread flat (a bird in flight)
        const dir = vnorm([s * (0.15 + t * 0.85), (-0.25 - t * 0.2) * (1 - hang) + (-0.8 + t * 0.35) * hang, (-1.0 + t * 0.35) * (1 - hang) + (-0.55 + t * 0.2) * hang]);
        const tip = vadd(base, vscale(dir, len));
        const w = (layer ? 0.2 : 0.15) * span;
        // The vane lies along the arm (neighbours overlap like shingles), across the feather.
        const arm = vnorm(t < 0.5 ? vsub(wrist, root) : vsub(tips[0], wrist));
        const side = vnorm(vsub(arm, vscale(dir, vdot(arm, dir))));
        mb.color = layer ? mixc(boneC, memC, 0.25) : mixc(boneC, memC, 0.45 + t * 0.55);
        const a = vadd(base, vscale(side, -w * 0.5)), b = vadd(base, vscale(side, w * 0.5));
        const c = vadd(vlerp(base, tip, 0.8), vscale(side, w * 0.42)), d = vadd(vlerp(base, tip, 0.8), vscale(side, -w * 0.42));
        mb.quad(a, b, c, d);
        mb.tri(d, c, tip);
      }
    }
    flapWeights();
    mb.bone = prev;
    return { wrist, tips };
  }
  // The membrane: fans between consecutive spars, each panel's trailing edge sagging toward the
  // wrist, and a last panel back to the flank.
  // A membrane is thin skin: the foliage material lets the low sun glow through it from behind
  // (a lit underside reads warm, not as a cold blue shadow).
  mb.mat = MAT.FOLIAGE;
  mb.color = memC;
  const m0 = mb.mark();
  for (let i = 0; i + 1 < fingers; i++) {
    const b = tips[i], c = tips[i + 1];
    const mid = vlerp(vlerp(b, c, 0.5), wrist, 0.18 + 0.04 * i);
    mb.tri(wrist, b, mid);
    mb.tri(wrist, mid, c);
  }
  const lastTip = tips[fingers - 1];
  const midIn = vlerp(vlerp(lastTip, body0, 0.5), elbow, 0.22);
  mb.tri(elbow, wrist, lastTip);
  mb.tri(elbow, lastTip, midIn);
  mb.tri(elbow, midIn, body0);
  mb.tri(root, elbow, body0);
  // Membrane normals: face up (a little curl), darker toward the body. The triangles are first
  // wound so their front faces look up (the mirrored wing's came out wound the other way, and the
  // shader's back-face flip then turned its stored normals down: a cold rim-lit wing).
  const v = mb.v;
  const want = [s * 0.12, 1, 0.15];
  for (let i = m0 * 13; i < v.length; i += 39) {
    const a = [v[i], v[i + 1], v[i + 2]], b = [v[i + 13], v[i + 14], v[i + 15]], c = [v[i + 26], v[i + 27], v[i + 28]];
    if (vdot(vcross(vsub(b, a), vsub(c, a)), want) < 0) {
      for (let j = 0; j < 13; j++) {
        const t = v[i + 13 + j];
        v[i + 13 + j] = v[i + 26 + j];
        v[i + 26 + j] = t;
      }
    }
  }
  // Each panel keeps its own facing (eased a little toward up for a soft, painted membrane): a
  // panel seen from below then shades as an underside instead of catching the rim light.
  for (let i = m0 * 13; i < v.length; i += 39) {
    const a = [v[i], v[i + 1], v[i + 2]], b = [v[i + 13], v[i + 14], v[i + 15]], c = [v[i + 26], v[i + 27], v[i + 28]];
    const n = vnorm(vadd(vnorm(vcross(vsub(b, a), vsub(c, a))), vscale(vnorm(want), 0.3)));
    for (let j = 0; j < 39; j += 13) {
      v[i + j + 3] = n[0];
      v[i + j + 4] = n[1];
      v[i + j + 5] = n[2];
    }
  }
  for (let i = m0 * 13; i < v.length; i += 13) {
    const d = Math.min(1, Math.abs(v[i] - root[0]) / span);
    const k = 0.65 + 0.45 * d;
    // Occlusion damps the sky's rim light, which otherwise paints a thin dark skin seen edge-on
    // (the far panels of a raised wing) cold blue.
    v[i + 12] *= o.feathers ? 1 : 0.42;
    v[i + 6] *= k;
    v[i + 7] *= k;
    v[i + 8] *= k;
  }
  // The arm and the finger spars.
  mb.mat = MAT.MATTE;
  mb.color = boneC;
  const ar = o.armR ?? 0.022;
  mb.tube([root, elbow, wrist], [ar, ar * 0.75, ar * 0.55], 6, { caps: true });
  for (const t of tips) mb.tube([wrist, vlerp(wrist, t, 0.55), t], [ar * 0.45, ar * 0.3, 0.0015], 4, { caps: true });
  // A thumb claw at the wrist.
  mb.color = C(o.claw ?? "bone");
  horn(mb, wrist, [s * 0.3, 1, 0.5], 0.035 * span * 2, 0.008 * span * 2, [0, -0.5, 0.6], 3, 4);
  flapWeights();
  mb.bone = prev;
  return { wrist, tips, elbow };
}

// ---------------------------------------------------------------------------------------------
// The giant spider

/** unit.spider: a giant spider: a low body slung between eight long legs with high knees, an
 *  abdomen marked in the owner's colour, fangs and a cluster of ember eyes (param "pale"). */
unitRecipe("unit.spider", {
  variants: 3,
  build(mb, r, { variant, param }) {
    const palettes = [["#2a221e", "#5e4630", "#151110"], ["#33302c", "#7a6a50", "#1a1714"], ["#3a2620", "#8a4a2a", "#1a0e0c"]];
    const [bodyC, bandC, darkC] = param === "pale" ? ["#8a8478", "#c8c0b0", "#4a463e"] : palettes[variant % 3];
    mb.part(() => {
      mb.rotateY(0.45);
      mb.scale(1.12);
      local(mb, (k) => {
        const head = [0, 0.1, 0.06];
        const abd = [0, 0.15, -0.13];
        // Cephalothorax.
        k.bone = BONE.BODY;
        setMat(k, bodyC);
        k.part(() => {
          k.translate(head[0], head[1], head[2]);
          k.scale(1, 0.72, 1.15);
          k.sphere(0.055, 1);
        });
        // Abdomen: tilted up at the back, hairy, the owner's mark on top.
        k.bone = BONE.HEAD;
        const seed = r.range(0, 30);
        const am = k.mark();
        setMat(k, bodyC);
        k.part(() => {
          k.translate(abd[0], abd[1], abd[2]);
          k.rotateX(-0.35);
          k.scale(0.92, 0.78, 1.18);
          k.blob(0.115, { detail: 2, amp: 0.06, noise: (v) => noise3(v[0] * 6 + seed, v[1] * 6, v[2] * 6) });
        });
        k.tint(am, (x, y) => 0.75 + 0.5 * Math.max(0, Math.min(1, (y - 0.1) / 0.12)));
        setTeam(k, 0.95);
        k.part(() => {
          k.translate(abd[0], abd[1] + 0.058, abd[2] - 0.008);
          k.rotateX(-0.35);
          k.scale(0.62, 0.32, 1.05);
          k.sphere(0.095, 1);
        });
        setMat(k, darkC);
        for (const dz of [-0.03, 0.02]) {
          k.part(() => {
            k.translate(abd[0], abd[1] + 0.075 + dz * 0.4, abd[2] + dz);
            k.rotateX(-0.35);
            k.scale(0.62, 0.2, 0.18);
            k.sphere(0.09, 0);
          });
        }
        // Legs: four a side, knees high above the body, feet splayed far out.
        k.bone = BONE.BODY;
        const angs = [0.42, 1.05, 1.75, 2.45];
        for (const s of [-1, 1]) {
          angs.forEach((a, i) => {
            const dir = [s * Math.sin(a), 0, Math.cos(a)];
            const at = (d, y) => [head[0] + dir[0] * d, y, head[2] + dir[2] * d * (i < 2 ? 1.1 : 0.9)];
            const kneeH = 0.25 - Math.abs(i - 1.5) * 0.012 + (i === 0 ? 0.02 : 0);
            const reach = 0.3 + (i === 0 || i === 3 ? 0.03 : 0);
            const lift = variant === 1 && s > 0 && i === 0 ? 0.08 : 0;
            const pts = [at(0.035, head[1] + 0.005), at(0.1, kneeH - 0.04), at(0.15, kneeH + lift), at(0.24, 0.1 + lift), at(reach, -0.004 + lift * 1.6)];
            k.mat = MAT.MATTE;
            k.tube(pts, [0.012, 0.011, 0.009, 0.006, 0.0022], 5, { caps: true, colors: [bodyC, bandC, bodyC, bandC].map(C) });
            // Joint knobs.
            k.color = C(bandC);
            ball(k, pts[2], 0.01, 0);
          });
        }
        // Fangs (chelicerae) and palps.
        for (const s of [-1, 1]) {
          setMat(k, darkC);
          k.tube([[s * 0.014, 0.095, 0.11], [s * 0.016, 0.07, 0.125], [s * 0.012, 0.05, 0.12]], [0.011, 0.009, 0.006], 5, { caps: true });
          setMat(k, "#d8cfb6");
          horn(k, [s * 0.012, 0.05, 0.12], [-s * 0.3, -1, -0.2], 0.03, 0.004, [s * 0.6, 0, -0.8], 3, 4);
          setMat(k, bodyC);
          k.tube([[s * 0.03, 0.095, 0.1], [s * 0.05, 0.08, 0.14], [s * 0.045, 0.03, 0.16]], [0.006, 0.005, 0.003], 4, { caps: true });
        }
        // Eyes: two big, four small, glowing.
        k.mat = MAT.EMISSIVE;
        k.color = C(param === "pale" ? "#9affd8" : "#ff3a1a");
        for (const s of [-1, 1]) {
          ball(k, [s * 0.014, 0.128, 0.112], 0.0075, 0);
          ball(k, [s * 0.03, 0.124, 0.1], 0.0045, 0);
          ball(k, [s * 0.008, 0.138, 0.098], 0.004, 0);
        }
      });
    });
  },
});

// ---------------------------------------------------------------------------------------------
// The dragon

/** unit.dragon: a great dragon (Acheron the Red by default; param "black" | "white" | "gold" |
 *  "green" for the others): crouched on four legs, the neck raised in an S, horned head with a
 *  furnace glow between the jaws, wings spread wide (beating: BONE.WING_L/R), the tail curling
 *  behind (BONE.TAIL), dorsal spines in the owner's colour. ~1.0 long. Variant 1 raises the wings. */
unitRecipe("unit.dragon", {
  variants: 2,
  build(mb, r, { variant, param }) {
    const SCHEMES = {
      red: { body: "#5c271e", dark: "#220e0b", belly: "#9a7a4c", wing: "#4a1a15", horn: "#d8cfb6", glow: "#ff7a20", eye: "#ffd040" },
      black: { body: "#2a2626", dark: "#141212", belly: "#5a5248", wing: "#2a1e22", horn: "#a8a090", glow: "#9a60ff", eye: "#c0ff60" },
      white: { body: "#c8ccd0", dark: "#7a8490", belly: "#e8e4dc", wing: "#8aa0b4", horn: "#e8e8f0", glow: "#9ae6ff", eye: "#80d0ff" },
      gold: { body: "#b08a3a", dark: "#6a4a1a", belly: "#e0c080", wing: "#8a5a22", horn: "#f0e8d0", glow: "#ffd060", eye: "#ffffff" },
      green: { body: "#3a5a34", dark: "#1a2a18", belly: "#9a9a5a", wing: "#2e4a2a", horn: "#d8cfb6", glow: "#a0ff60", eye: "#ffe040" },
    };
    const sc = SCHEMES[param] ?? SCHEMES.red;
    const body = C(sc.body), dark = C(sc.dark), belly = C(sc.belly);
    mb.part(() => {
      mb.rotateY(0.55);
      local(mb, (k) => {
        // Torso: hips to chest, deep-chested and low.
        k.bone = BONE.BODY;
        const tm = k.mark();
        k.color = body;
        k.part(() => {
          k.scale(0.85, 1, 1);
          k.tube([[0, 0.19, -0.38], [0, 0.2, -0.25], [0, 0.21, -0.06], [0, 0.24, 0.1], [0, 0.265, 0.21]], [0.045, 0.115, 0.135, 0.13, 0.08], 10, { caps: true });
        });
        // Belly plates (ochre below, banded) and a darker spine.
        k.tint(tm, (x, y, z) => {
          const t = Math.max(0, Math.min(1, (0.19 - y) / 0.07));
          const band = 0.9 + 0.1 * Math.sin(z * 80);
          const sp = Math.max(0, Math.min(1, (y - 0.27) / 0.09)) * 0.75;
          return [0, 1, 2].map((i) => (1 - t) * (1 - sp + (sp * dark[i]) / body[i]) + (t * belly[i] * band) / body[i]);
        });
        // Tail: sweeping back and curling to the side, ending in a spade.
        k.bone = BONE.TAIL;
        const tail = [[0, 0.2, -0.3], [0.01, 0.15, -0.45], [0.06, 0.08, -0.58], [0.16, 0.04, -0.68], [0.28, 0.035, -0.71], [0.38, 0.05, -0.66]];
        k.color = body;
        const tlm = k.mark();
        k.tube(tail, [0.08, 0.06, 0.042, 0.028, 0.016, 0.005], 7, { caps: true });
        k.tint(tlm, (x, y) => (y < 0.05 ? 0.85 : 1));
        k.color = dark;
        k.part(() => {
          orient(k, tail[5], vsub(tail[5], tail[4]), [0, 1, 0]);
          slab(k, [[0, -0.012], [0.04, 0.012], [0, 0.07], [-0.04, 0.012]], 0.007);
        });
        // Neck: an S reaching forward and up from the chest; the head looks ahead and down.
        k.bone = BONE.HEAD;
        const neck = [[0, 0.28, 0.16], [0, 0.35, 0.28], [0, 0.42, 0.34], [0, 0.48, 0.4], [0, 0.505, 0.46]];
        k.color = body;
        k.tube(neck, [0.08, 0.065, 0.056, 0.05, 0.048], 8, { caps: true });
        // Throat plates.
        k.color = belly;
        k.tube([[0, 0.26, 0.24], [0, 0.33, 0.33], [0, 0.4, 0.39], [0, 0.46, 0.46]], [0.045, 0.04, 0.036, 0.032], 5, { caps: true });
        // Head.
        const hc = [0, 0.515, 0.5];
        k.color = body;
        k.part(() => {
          k.translate(hc[0], hc[1], hc[2]);
          k.scale(0.85, 0.75, 1.2);
          k.sphere(0.058, 1);
        });
        // Snout and upper jaw, the lower jaw open, fire in the throat.
        k.tube([[0, 0.52, 0.53], [0, 0.505, 0.6], [0, 0.485, 0.665]], [0.043, 0.034, 0.022], 7, { caps: true });
        k.color = dark;
        k.tube([[0, 0.475, 0.51], [0, 0.445, 0.58], [0, 0.428, 0.632]], [0.03, 0.022, 0.012], 6, { caps: true });
        k.mat = MAT.EMISSIVE;
        k.color = C(sc.glow);
        ball(k, [0, 0.468, 0.585], 0.017, 1, [0.9, 0.7, 1.4]);
        k.mat = MAT.MATTE;
        // Teeth.
        k.color = C(sc.horn);
        for (const s of [-1, 1]) for (const z of [0.6, 0.64]) horn(k, [s * 0.016, 0.488, z], [0, -1, 0.1], 0.016, 0.004, [0, 0, 0], 2, 3);
        // Brow ridges, eyes and the crown of horns sweeping back.
        k.color = dark;
        for (const s of [-1, 1]) k.tube([[s * 0.027, 0.545, 0.55], [s * 0.042, 0.552, 0.51], [s * 0.044, 0.545, 0.47]], [0.01, 0.014, 0.008], 4, { caps: true });
        k.mat = MAT.EMISSIVE;
        k.color = C(sc.eye);
        for (const s of [-1, 1]) ball(k, [s * 0.037, 0.534, 0.545], 0.0068, 0);
        k.mat = MAT.MATTE;
        k.color = C(sc.horn);
        for (const s of [-1, 1]) {
          horn(k, [s * 0.032, 0.555, 0.48], [s * 0.35, 0.55, -1], 0.18, 0.018, [s * 0.2, 0.9, 0.2], 6, 5);
          horn(k, [s * 0.05, 0.525, 0.47], [s * 0.9, 0.1, -0.8], 0.09, 0.01, [0, 0.5, -0.2], 4, 4);
          horn(k, [s * 0.032, 0.48, 0.56], [s * 0.8, -0.4, -0.2], 0.045, 0.006, [0, 0, -0.5], 3, 3);
        }
        horn(k, [0, 0.515, 0.655], [0, 1, 0.3], 0.028, 0.007, [0, 0, -0.6], 3, 3);
        // Dorsal spines along the neck, back and tail, in the owner's colour.
        k.bone = BONE.BODY;
        setTeam(k, 0.72);
        const spine = [...neck.slice(1).reverse().map((p) => [p[0], p[1] + 0.045, p[2] - 0.012]), [0, 0.38, 0.12], [0, 0.365, 0.03], [0, 0.35, -0.06], [0, 0.325, -0.15], [0, 0.3, -0.24], ...tail.slice(1, 5).map((p, i) => [p[0], p[1] + 0.05 - i * 0.01, p[2]])];
        spine.forEach((p, i) => {
          const sz = i < 4 ? 0.035 : i < 9 ? 0.06 : 0.04 - (i - 9) * 0.007;
          k.part(() => {
            k.translate(p[0], p[1] - 0.008, p[2]);
            k.rotateX(-0.6);
            k.scale(0.35, 1, 1);
            k.cone(sz * 0.55, sz, 4);
          });
        });
        // Legs: crouched, splayed, clawed; and the wings from the shoulders.
        for (const s of [-1, 1]) {
          k.bone = BONE.BODY;
          k.color = body;
          const fr = [[s * 0.09, 0.23, 0.13], [s * 0.16, 0.11, 0.05], [s * 0.16, 0.04, 0.16], [s * 0.16, 0.014, 0.22]];
          k.tube(fr, [0.062, 0.046, 0.033, 0.03], 6, { caps: true });
          const hd = [[s * 0.08, 0.2, -0.2], [s * 0.17, 0.14, -0.06], [s * 0.16, 0.06, -0.27], [s * 0.16, 0.014, -0.2]];
          k.tube(hd, [0.07, 0.055, 0.034, 0.028], 6, { caps: true });
          k.part(() => {
            k.translate(s * 0.115, 0.17, -0.15);
            k.scale(0.72, 1, 1.15);
            k.sphere(0.095, 1);
          });
          k.color = C(sc.horn);
          for (const foot of [fr[3], hd[3]]) {
            for (const dx of [-1, 0, 1]) horn(k, [foot[0] + dx * 0.016, 0.01, foot[2] + 0.016], [dx * 0.4, -0.5, 1], 0.034, 0.008, [0, -1, 0], 3, 3);
          }
          wing(k, s, [s * 0.06, 0.32, 0.08], { span: 0.68, lift: variant ? 0.4 : 0.24, sweep: variant ? 0.3 : 0.16, fingers: 4, bone: sc.dark, membrane: sc.wing, chord: 0.45, armR: 0.03, claw: sc.horn });
        }
      });
    });
  },
});

// ---------------------------------------------------------------------------------------------
// Demons, golems, elementals, the dead, serpents and beasts

/** unit.demon: variant 0 an imp (small, horned, bat-winged, a flame in its claw); 1 a hulking
 *  demon (huge, horned, a great cleaver); 2 a winged demon lord (balor). Loincloths in the owner's
 *  colour. param "dark" | "pale" recolours the skin. */
unitRecipe("unit.demon", {
  variants: 3,
  build(mb, r, { variant, param }) {
    const kind = variant % 3;
    const skin = param === "dark" ? "#2a1a1e" : param === "pale" ? "#9a7a6a" : undefined;
    const R = RACES.demon;
    const wingsOf = (span, lift) => ({ span, lift, sweep: 0.12, fingers: 3, bone: "#1e1012", membrane: "#4a1a16", chord: 0.4, armR: 0.012 });
    const base = { r, race: "demon", variant, skin, body: "bare", claws: true, eyeGlow: "#ffb020", skirt: R.hipY * 0.36, skirtColor: teamCol(0.9), skirtMat: MAT.TEAM, ragged: true };
    let f, scale;
    if (kind === 0) {
      f = figure({
        ...base,
        stance: "brace",
        lean: 0.28,
        bulk: 0.85,
        horns: true,
        wings: wingsOf(0.26, 0.16),
        right: { hand: [0.06, -0.02, 0.1], elbow: [0.8, -0.6, -0.2], weapon: "torch", dir: [-0.1, 1, 0.2] },
        left: { hand: [0.08, -0.05, 0.08], elbow: [0.9, -0.5, -0.1] },
      });
      scale = 0.66;
    } else if (kind === 1) {
      f = figure({
        ...base,
        stance: "brace",
        lean: 0.18,
        bulk: 1.22,
        horns: "big",
        mantle: "#2a1a18",
        pauldrons: "#2a2226",
        bracers: "#2a2226",
        right: { hand: [0.03, 0.08, -0.01], elbow: [1, -0.15, -0.25], weapon: "axe", dir: [-0.55, 0.8, 0.12], opts: { len: 0.2, double: true, head: 1.5, metal: "#3a3436" } },
        left: { hand: [0.06, -0.12, 0.1], elbow: [0.9, -0.4, -0.2] },
      });
      scale = 1.5;
    } else {
      f = figure({
        ...base,
        stance: "stride",
        lean: 0.12,
        bulk: 1.1,
        horns: "big",
        wings: wingsOf(0.42, 0.24),
        body: "armour",
        metal: "#3a3036",
        pauldrons: true,
        right: { hand: [0.02, 0.06, 0.04], elbow: [1, -0.2, -0.2], weapon: "sword", dir: [-0.4, 0.85, 0.2], opts: { len: 0.22, w: 0.024, metal: "#4a3c3a" } },
        left: { hand: [0.08, -0.04, 0.1], elbow: [0.9, -0.5, -0.1], weapon: "torch", dir: [0.2, 1, 0.3] },
      });
      scale = 1.32;
    }
    place(mb, f.mb, [0, 0, 0], 0, scale);
  },
});

/** unit.golem: a hulking construct, broad-shouldered and hunched, one fist hanging and one raised,
 *  a rune burning in its chest in the owner's colour and a sash of the owner's cloth. param:
 *  "stone" | "clay" | "iron" | "wood" | "bone" | "flesh" | "mithril" (default by variant: stone,
 *  clay, iron). Stone, metal and bone are hewn blocks; clay, wood and flesh are lumps. */
unitRecipe("unit.golem", {
  variants: 3,
  build(mb, r, { variant, param }) {
    const KINDS = {
      stone: { c: "#8a8378", c2: "#625c54", flat: true, amp: 0.2, eye: "#9ad0ff" },
      clay: { c: "#8a6440", c2: "#6a4a2e", eye: "#ffb060", amp: 0.12 },
      iron: { c: "#4a4a4e", c2: "#2c2a2c", hewn: true, metal: true, eye: "#ff8a30" },
      wood: { c: "#5e4632", c2: "#3e2c1e", eye: "#a0ff70", moss: "#46612e", amp: 0.16 },
      bone: { c: "#d9cfb6", c2: "#a89a80", flat: true, amp: 0.12, eye: "#7ad8ff" },
      flesh: { c: "#a87a6a", c2: "#7a4a44", eye: "#ffd040", amp: 0.14 },
      mithril: { c: "#b8c4d0", c2: "#8a96a4", hewn: true, metal: true, eye: "#b0e0ff" },
    };
    const kname = KINDS[param] ? param : ["stone", "clay", "iron"][variant % 3];
    const K = KINDS[kname];
    const mat = K.metal ? MAT.METAL : MAT.MATTE;
    const seed = r.range(0, 50);
    /** One mass of the body: a hewn block or a lump, `size` [w, h, d] centred at `p`. */
    const mass = (k, p, size, { col = K.c, rx = 0, rz = 0, taper = 0.85 } = {}) => {
      k.color = jitter(C(col), r, 0.07);
      k.mat = mat;
      k.part(() => {
        k.translate(p[0], p[1], p[2]);
        if (rx) k.rotateX(rx);
        if (rz) k.rotateZ(rz);
        if (K.hewn) {
          // Forged plate: a bellied, eight-sided shell with a riveted rim band (not a box).
          const w = size[0] / 2, h = size[1];
          k.translate(0, -h / 2, 0);
          k.scale(1, 1, size[2] / size[0]);
          k.lathe([[w * 0.82, 0], [w * 1.0, h * 0.28], [w * 0.98 * Math.max(taper, 0.9), h * 0.7], [w * 0.78 * taper, h * 0.96], [w * 0.4 * taper, h]], 8, { phase: Math.PI / 8 });
          k.color = jitter(C(K.c2), r, 0.05);
          k.lathe([[w * 1.01, h * 0.26], [w * 1.04, h * 0.32], [w * 1.01, h * 0.38]], 8, { caps: false, phase: Math.PI / 8 });
        } else {
          k.scale(size[0] / 2, size[1] / 2, size[2] / 2);
          k.blob(1, { detail: 1, amp: K.amp ?? 0.15, flat: K.flat, noise: (v) => noise3(v[0] * 2.5 + seed + p[1] * 9, v[1] * 2.5 + p[0] * 7, v[2] * 2.5) });
        }
      });
    };
    /** A limb segment from a to b, `w` thick: a hewn block or a lumpy tube with a knot at b. */
    const seg = (k, a, b, w, col = K.c) => {
      k.color = jitter(C(col), r, 0.07);
      k.mat = mat;
      if (K.hewn) {
        // A forged limb: a tapering plated tube and a ball joint at its end.
        k.tube([vsub(a, vscale(vnorm(vsub(b, a)), w * 0.2)), vlerp(a, b, 0.5), b], [w * 0.5, w * 0.47, w * 0.4], 6, { caps: true });
        k.color = jitter(C(K.c2), r, 0.05);
        ball(k, b, w * 0.5, 0);
      } else {
        k.tube([a, vlerp(a, b, 0.5), b], [w * 0.5, w * 0.44, w * 0.42], 6, { caps: true });
        k.part(() => {
          k.translate(b[0], b[1], b[2]);
          k.blob(w * 0.55, { detail: 1, amp: K.amp ?? 0.15, flat: K.flat, noise: (v) => noise3(v[0] * 2.5 + seed + b[1] * 9, v[1] * 2.5 + b[0] * 7, v[2] * 2.5) });
        });
      }
    };
    mb.part(() => {
      mb.rotateY(-0.2);
      local(mb, (k) => {
        // Legs: thick, wide apart, one forward.
        for (const s of [-1, 1]) {
          k.bone = s > 0 ? BONE.LEG_L : BONE.LEG_R;
          const fz = s > 0 ? 0.04 : -0.03;
          const hip = [s * 0.07, 0.2, 0], knee = [s * 0.09, 0.11, fz + 0.015], foot = [s * 0.09, 0.025, fz];
          seg(k, hip, knee, 0.08, K.c2);
          seg(k, knee, foot, 0.075);
          mass(k, [foot[0], 0.03, foot[2] + 0.015], [0.09, 0.05, 0.11], { col: K.c2, taper: 0.9 });
        }
        k.bone = BONE.BODY;
        mass(k, [0, 0.235, -0.005], [0.16, 0.08, 0.11], { col: K.c2 });
        // The torso: broad at the shoulders, narrow at the waist, hunched forward.
        mass(k, [0, 0.415, 0.02], [0.24, 0.17, 0.15], { rx: 0.25, taper: K.hewn ? 1.15 : 1 });
        mass(k, [0, 0.3, 0.03], [0.15, 0.1, 0.12], { col: K.c2, rx: 0.15 });
        // Arms: the left hangs heavy, the right is raised in a fist.
        const arms = [
          [1, [0.165, 0.465, 0.03], [0.22, 0.32, 0.07], [0.205, 0.18, 0.12]],
          [-1, [-0.165, 0.465, 0.03], [-0.25, 0.37, 0.1], [-0.215, 0.5, 0.17]],
        ];
        for (const [s, sh, el, fist] of arms) {
          k.bone = s > 0 ? BONE.ARM_L : BONE.ARM_R;
          mass(k, vadd(sh, [s * 0.015, 0.045, 0]), [0.12, 0.1, 0.12], { rz: -s * 0.3 });
          seg(k, sh, el, 0.075, K.c2);
          seg(k, el, fist, 0.068);
          mass(k, vadd(fist, [0, K.hewn ? 0.045 : 0, 0]), [0.095, 0.085, 0.095], { col: K.c2 });
        }
        // Head set forward between the shoulders, eyes burning.
        k.bone = BONE.HEAD;
        mass(k, [0, 0.575, 0.1], [0.08, 0.075, 0.075], { taper: 0.8 });
        k.mat = MAT.EMISSIVE;
        k.color = C(K.eye);
        for (const s of [-1, 1]) ball(k, [s * 0.018, 0.545, 0.139], 0.0078, 0, [1.4, 0.55, 1]);
        // The owner's rune burning in the chest, and a sash of the owner's cloth.
        k.bone = BONE.BODY;
        k.mat = MAT.GLOW_TEAM;
        k.color = C("#ffffff");
        k.part(() => {
          k.translate(0, 0.42, 0.115);
          k.rotateX(0.25);
          slab(k, [[0, -0.032], [0.022, 0], [0, 0.034], [-0.022, 0]], 0.012);
        });
        setTeam(k, 0.9);
        k.part(() => {
          k.translate(0, 0.36, 0.02);
          k.rotateX(0.25);
          k.rotateZ(0.6);
          ringTube(k, 0.15, 0.11, 0.014, 12, 4);
        });
        if (K.moss) {
          k.color = C(K.moss);
          k.mat = MAT.FOLIAGE;
          for (const p of [[0.17, 0.53, 0.02], [-0.15, 0.53, -0.01], [0.02, 0.52, -0.06]]) ball(k, p, 0.04, 1, [1.2, 0.4, 1.2]);
        }
      });
    });
  },
});

/** unit.elemental: param "fire" (default) | "water" | "air" | "earth" | "ice": a figure of its
 *  element rising from a base. Fire: layered tongues of flame (dark red to yellow), cinders
 *  orbiting, an owner-coloured heart. Earth is a golem of raw rock. */
unitRecipe("unit.elemental", {
  variants: 2,
  build(mb, r, { variant, param }) {
    const el = ["water", "air", "earth", "ice"].includes(param) ? param : "fire";
    if (el === "earth") {
      const sub = new MeshBuilder();
      buildAsset_("unit.golem", sub, r, { variant: 0, param: "stone" });
      place(mb, sub, [0, 0, 0], 0, 1.15);
      return;
    }
    const S = {
      fire: { mat: MAT.EMISSIVE, cols: ["#7a1a06", "#b8380c", "#e8701a", "#ffc048"], rock: "#1e1618" },
      water: { mat: MAT.METAL, cols: ["#1a3a5a", "#2a5a7a", "#3a7a9a", "#a8d0e0"], rock: "#2a3a44" },
      air: { mat: MAT.MATTE, cols: ["#7a8494", "#9aa4b4", "#c0c8d4", "#e8eef4"], rock: "#6a7080" },
      ice: { mat: MAT.METAL, cols: ["#4a7a9a", "#7aa8c4", "#a8d0e4", "#e0f4ff"], rock: "#3a5a6a" },
    }[el];
    const col = (t) => C(S.cols[Math.min(3, Math.max(0, Math.floor(t * 3.999)))]);
    mb.part(() => {
      mb.rotateY(-0.2);
      local(mb, (k) => {
        k.bone = BONE.BODY;
        const twist = (y) => Math.sin(y * 9 + variant) * 0.025;
        // The column: a base swirl rising into a torso (layers, darkest outside).
        const layers = el === "fire" ? 3 : 2;
        for (let li = 0; li < layers; li++) {
          const sc = 1 - li * 0.22;
          k.mat = S.mat;
          k.color = col(li / 3);
          const prof = el === "air"
            ? [[0.012 * sc, 0.0], [0.028 * sc, 0.07], [0.05 * sc, 0.17], [0.08 * sc, 0.28], [0.1 * sc, 0.37], [0.08 * sc, 0.44], [0.03 * sc, 0.48]]
            : [[0.075 * sc, 0.0], [0.06 * sc, 0.06], [0.045 * sc, 0.15], [0.07 * sc, 0.26], [0.095 * sc, 0.35], [0.08 * sc, 0.42], [0.03 * sc, 0.47]];
          local(k, (q) => {
            q.lathe(prof, el === "ice" ? 6 : 9, { caps: true, wobble: el === "ice" ? null : (a, y) => (el === "air" ? 0.1 : 0.18) * Math.sin(a * 5 + y * 30 + li) });
            q.displace(0, (x, y) => [twist(y) * (el === "air" ? 1.8 : 1), 0, twist(y + 1) * 0.6]);
          });
        }
        // Tongues of flame / spray / gusts rising from the shoulders and head.
        /** A flat-faceted crystal (a bipyramid of four sides) from `p` along `dir`. */
        const shard = (p, dir, len, rad) => k.part(() => {
          orient(k, p, dir, [0.3, 0.2, 1]);
          const ring = [0, 1, 2, 3].map((j) => [Math.cos((j * TAU) / 4) * rad, len * 0.22, Math.sin((j * TAU) / 4) * rad]);
          for (let j = 0; j < 4; j++) {
            const a = ring[j], b = ring[(j + 1) % 4];
            k.tri([0, len, 0], b, a);
            k.tri([0, -len * 0.05, 0], a, b);
          }
        });
        if (el === "air") {
          // Wisps spiralling up around the funnel.
          for (let i = 0; i < 4; i++) {
            const pts = [], rads = [];
            for (let j = 0; j <= 8; j++) {
              const t = j / 8, a = (i / 4) * TAU + t * 5.5;
              const rr = 0.04 + t * 0.11;
              pts.push([Math.cos(a) * rr + twist(t * 0.45) * 1.8, 0.02 + t * 0.44, Math.sin(a) * rr * 0.85]);
              rads.push(0.004 + 0.008 * Math.sin(t * Math.PI));
            }
            k.mat = S.mat;
            k.color = col(0.6 + i * 0.1);
            k.sway = 0.4;
            k.tube(pts, rads, 4, { caps: true });
            k.sway = 0;
          }
        } else {
          const nT = el === "ice" ? 9 : 12;
          for (let i = 0; i < nT; i++) {
            const a = (i / nT) * TAU + r.range(-0.3, 0.3);
            const y0 = r.range(0.18, 0.44);
            const rad = 0.06 + (0.44 - y0) * 0.2;
            const p0 = [Math.cos(a) * rad, y0, Math.sin(a) * rad * 0.8];
            k.mat = S.mat;
            k.color = col(r.range(0.2, 1));
            if (el === "ice") {
              shard(p0, [Math.cos(a) * 0.9, r.range(0.3, 0.9), Math.sin(a) * 0.8], r.range(0.07, 0.13), r.range(0.012, 0.02));
              continue;
            }
            k.sway = 0.5;
            if (el === "water") horn(k, p0, [Math.cos(a) * 0.4, 1, Math.sin(a) * 0.35], r.range(0.08, 0.13), r.range(0.014, 0.022), [Math.cos(a) * 1.6, -2.2, Math.sin(a) * 1.4], 5, 5);
            else horn(k, p0, [Math.cos(a) * 0.5, 1, Math.sin(a) * 0.4], r.range(0.08, 0.16), r.range(0.014, 0.024), [r.range(-0.6, 0.6), 0.2, r.range(-0.6, 0.6)], 4, 5);
            k.sway = 0;
          }
        }
        // Arms of the element, reaching out.
        for (const s of [-1, 1]) {
          k.bone = s > 0 ? BONE.ARM_L : BONE.ARM_R;
          k.mat = S.mat;
          k.color = col(0.4);
          const sh = [s * 0.08, 0.37, 0.0];
          const el2 = [s * 0.16, 0.3, 0.05];
          const hand = [s * 0.2, 0.36 + (s > 0 ? 0.08 : 0), 0.1];
          k.tube([sh, el2, hand], [0.034, 0.026, 0.02], 6, { caps: true });
          k.color = col(0.9);
          for (let j = 0; j < 3; j++) horn(k, hand, [s * 0.3 + (j - 1) * 0.3, 1, 0.3], 0.06, 0.012, [0, 0.3, 0], 3, 4);
        }
        // The head: a crown of the element; dark eye hollows.
        k.bone = BONE.HEAD;
        k.mat = S.mat;
        k.color = col(0.75);
        ball(k, [0, 0.5, 0.02], 0.045, 1, [0.9, 1.15, 0.9]);
        k.color = col(1);
        if (el === "ice") for (let i = 0; i < 3; i++) shard([(i - 1) * 0.02, 0.53, -0.01], [(i - 1) * 0.35, 1, -0.25], 0.09 + (i === 1 ? 0.04 : 0), 0.014);
        else if (el === "water") horn(k, [0, 0.53, 0.02], [0, 1, -0.6], 0.11, 0.024, [0, -1.2, 2.2], 6, 6);
        else if (el === "fire") for (let i = 0; i < 5; i++) horn(k, [(i - 2) * 0.012, 0.53, -0.01], [(i - 2) * 0.25, 1, -0.3], 0.07 + (i % 2) * 0.03, 0.012, [0, 0, -0.4], 4, 4);
        k.mat = MAT.MATTE;
        k.color = C("#120a08");
        for (const s of [-1, 1]) ball(k, [s * 0.016, 0.505, 0.06], 0.008, 0, [1.3, 0.7, 0.6]);
        // The heart: owner-coloured light.
        k.bone = BONE.BODY;
        k.mat = MAT.GLOW_TEAM;
        k.color = C("#ffffff");
        ball(k, [0, 0.34, 0.07], 0.022, 1);
        // Cinders / stones orbiting, anchoring the glow.
        k.mat = MAT.MATTE;
        k.color = C(S.rock);
        for (let i = 0; i < 5; i++) {
          const a = (i / 5) * TAU + 0.4;
          const p = [Math.cos(a) * 0.16, 0.08 + (i % 3) * 0.12, Math.sin(a) * 0.13];
          k.part(() => {
            k.translate(p[0], p[1], p[2]);
            k.blob(0.02 + (i % 2) * 0.01, { detail: 0, amp: 0.3, flat: true, noise: (v) => noise3(v[0] * 3 + i, v[1] * 3, v[2] * 3) });
          });
        }
        // The base: a scorched ring of embers (or a pool, a whirl of dust).
        k.mat = MAT.MATTE;
        k.color = C(el === "fire" ? "#2a1612" : S.cols[0]);
        k.part(() => {
          k.translate(0, -0.012, 0);
          k.lathe([[0, 0.0], [0.15, 0.0], [0.17, 0.016], [0.1, 0.022], [0, 0.02]], 12, { wobble: (a) => 0.15 * Math.sin(a * 6) });
        });
        if (el === "fire") {
          // Embers smouldering in the scorch.
          k.mat = MAT.EMISSIVE;
          k.color = C("#c8400c");
          k.part(() => {
            k.translate(0, 0.012, 0);
            ringTube(k, 0.1, 0.09, 0.008, 12, 3, { wob: 0.25, seed: variant });
          });
        }
      });
    });
  },
});

/** unit.skeleton: the risen dead: a skeleton with rusted arms and a tattered rag of the owner's
 *  colour; variant 0 sword and shield, 1 spear, 2 bow, 3 axe. param "dark" for wraith light. */
unitRecipe("unit.skeleton", {
  variants: 4,
  build(mb, r, { variant, param }) {
    const dark = param === "dark";
    const kind = variant % 4;
    const make = [soldierSpec, spearmanSpec, archerSpec, warriorSpec][kind];
    const spec = make({ race: "undead", dark, variant: kind === 0 ? 0 : 1, r });
    const f = figure({
      r, race: "undead", dark, variant, ...spec,
      body: "bones",
      tabard: kind === 0,
      skirt: RACES.undead.hipY * 0.38,
      skirtColor: teamCol(0.75),
      skirtMat: MAT.TEAM,
      ragged: true,
      greaves: false,
      head: kind === 0 ? "helm" : kind === 3 ? "horned" : "bare",
      helmMetal: "#6a625a",
      metal: "#6a625a",
      mantle: false,
      back: kind === 2 ? ["quiver"] : [],
    });
    place(mb, f.mb);
  },
});

/** unit.serpent: a sea serpent: coils arching out of the water, the neck rearing, a frill in
 *  the owner's colour. Placed at sea level (float); parts below y = 0 are under water. */
unitRecipe("unit.serpent", {
  variants: 2,
  build(mb, r, { variant, param }) {
    const body = C(param === "dark" ? "#2a3a3a" : "#2e5a50"), belly = C("#b0a878"), fin = C("#1a3a34");
    mb.part(() => {
      mb.rotateY(0.9 + variant * 0.3);
      local(mb, (k) => {
        k.bone = BONE.BODY;
        // Coils: arches along -z, smaller toward the tail.
        const arches = [[-0.02, 0.2, 0.075], [-0.24, 0.15, 0.06], [-0.42, 0.1, 0.045]];
        arches.forEach(([zc, hgt, rad], i) => {
          const pts = [];
          const n = 7;
          for (let j = 0; j < n; j++) {
            const t = j / (n - 1);
            pts.push([Math.sin(t * 3) * 0.015, -0.06 + Math.sin(Math.PI * t) * hgt, zc + 0.09 * (1 - i * 0.25) - t * 0.18 * (1 - i * 0.25)]);
          }
          k.color = body;
          const m0 = k.mark();
          k.tube(pts, pts.map((_, j) => rad * (1 - 0.15 * Math.abs(j / (n - 1) - 0.5))), 8, { caps: true });
          // The pale belly on the arch's underside (by the normal), dark scales above.
          k.tint(m0, (x, y, z, vi) => {
            const ny = k.v[vi * 13 + 4];
            const t = Math.max(0, Math.min(1, (-ny - 0.1) * 2.5));
            return [0, 1, 2].map((c) => (1 - t) * (0.85 + 0.15 * Math.max(0, ny)) + (t * belly[c]) / body[c]);
          });
          // Dorsal fin along the arch.
          k.color = fin;
          for (let j = 1; j < n - 1; j++) {
            const p = pts[j];
            k.part(() => {
              k.translate(p[0], p[1] + rad * 0.8, p[2]);
              k.rotateX(-0.4 + (j / n) * 0.8);
              k.scale(0.3, 1, 1);
              k.cone(rad * 0.4, rad * 0.9, 4);
            });
          }
          // Foam where the coil meets the water.
          k.color = C("#d8e4e4");
          for (const e of [pts[0], pts[n - 1]]) {
            k.part(() => {
              k.translate(e[0], 0.002, e[2]);
              ringTube(k, rad * 1.25, rad * 1.25, 0.008, 10, 3);
            });
          }
        });
        // The neck rears up and forward; the head looks down at its prey.
        k.bone = BONE.HEAD;
        const neck = [[0, -0.06, 0.12], [0, 0.12, 0.17], [0, 0.28, 0.15], [0, 0.38, 0.19], [0, 0.42, 0.26]];
        k.color = body;
        const nm = k.mark();
        k.tube(neck, [0.085, 0.075, 0.065, 0.058, 0.05], 8, { caps: true });
        k.tint(nm, (x, y, z) => (z > 0.17 && y < 0.36 && Math.abs(x) < 0.04 ? 1.4 : 1));
        k.color = C("#d8e4e4");
        k.part(() => {
          k.translate(0, 0.002, 0.13);
          ringTube(k, 0.1, 0.1, 0.01, 12, 3);
        });
        // Head: a long wedge, jaws open, fangs, eyes.
        k.color = body;
        k.tube([[0, 0.43, 0.25], [0, 0.42, 0.32], [0, 0.4, 0.38]], [0.05, 0.04, 0.022], 7, { caps: true });
        k.color = C("#3a2a2a");
        k.tube([[0, 0.39, 0.26], [0, 0.355, 0.32], [0, 0.34, 0.36]], [0.032, 0.022, 0.012], 6, { caps: true });
        k.color = C("#e8e0cc");
        for (const s of [-1, 1]) horn(k, [s * 0.016, 0.395, 0.34], [0, -1, 0.1], 0.025, 0.005, [0, 0, 0], 2, 3);
        k.mat = MAT.EMISSIVE;
        k.color = C("#d0ff60");
        for (const s of [-1, 1]) ball(k, [s * 0.03, 0.44, 0.3], 0.007, 0);
        // The frill: owner-coloured membranes on spines behind the head.
        for (const s of [-1, 1]) {
          setTeam(k, 0.9);
          const base = [s * 0.03, 0.43, 0.23];
          const pts = [base, vadd(base, [s * 0.1, 0.07, -0.05]), vadd(base, [s * 0.11, -0.02, -0.08]), vadd(base, [s * 0.08, -0.09, -0.06]), vadd(base, [s * 0.01, -0.06, -0.02])];
          for (let i = 1; i + 1 < pts.length; i++) k.tri(pts[0], pts[i], pts[i + 1]);
          k.mat = MAT.MATTE;
          k.color = fin;
          for (let i = 1; i < pts.length - 1; i++) k.tube([base, pts[i]], [0.005, 0.002], 3);
        }
        setTeam(k, 0.9);
        k.part(() => {
          k.translate(0, 0.46, 0.22);
          k.rotateX(-0.6);
          k.scale(0.25, 1, 1);
          k.cone(0.04, 0.09, 4);
        });
      });
    });
  },
});

/** unit.beast: param picks the creature: "beastman" (goat-headed, default), "minotaur",
 *  "werewolf", "ogre", "giant", "ape", "boar", "lion", "tiger", "sandlion", "elephant", "war-elephant", "treant",
 *  "centaur", "lizard" (a giant lizard), "scorpion". Variants recolour and re-pose. */
unitRecipe("unit.beast", {
  variants: 3,
  build(mb, r, { variant, param }) {
    const kind = param || "beastman";
    const quad = (b, o, yaw = 0.7, s = 1) => {
      mb.part(() => {
        mb.rotateY(yaw);
        mb.scale(s);
        local(mb, (k) => quadruped(k, b, { r, ...o }));
      });
    };
    const fig = (spec, s = 1) => place(mb, figure({ r, variant, ...spec }).mb, [0, 0, 0], 0, s);
    if (kind === "boar") return quad({ ...BEASTS.boar, coat: ["#4a3a30", "#5a4636", "#2e2622"][variant % 3] }, { gait: variant ? "charge" : "stand", collar: true }, 0.7, 1.5);
    if (kind === "lion" || kind === "tiger" || kind === "sandlion") {
      const b = { ...BEASTS.cat };
      if (kind === "lion") Object.assign(b, { maneful: true, coat: "#a8824e", belly: "#d0b890", mane: "#5a3c22", girth: 0.05, len: 0.25 });
      if (kind === "tiger") Object.assign(b, { coat: "#c07a32", belly: "#e8dcc0", mane: "#2a1a10", stripes: "#2a1a12" });
      if (kind === "sandlion") Object.assign(b, { coat: "#c0a06a", belly: "#e0d0a8", mane: "#7a5a36", maneful: true, girth: 0.05, len: 0.25 });
      return quad(b, { gait: variant % 2 ? "stalk" : "walk", collar: true, snarl: true }, 0.7, 1.45);
    }
    // A wild elephant is grey hide; only the war elephant wears the owner's cloth, a saddle cloth
    // over the back (a full caparison turned the whole beast into a block of team colour).
    if (kind === "elephant") return quad({ ...BEASTS.elephant }, { gait: "walk" }, 0.75, 1.0);
    if (kind === "war-elephant") return quad({ ...BEASTS.elephant }, { gait: "walk", caparison: true, caparisonHem: 0.27, caparisonSpan: 0.42 }, 0.75, 1.0);
    if (kind === "lizard") return quad({ ...BEASTS.cat, coat: "#4a5a3a", belly: "#9a9a6a", mane: "#2a3a22", head: ["boar", 0.08, 0.03], tail: ["rope", 0.24, 0.016], h: 0.12, leg: [0.012, "paw"] }, { gait: "stalk", warpaint: true }, 0.7, 1.4);
    if (kind === "scorpion") {
      const sub = new MeshBuilder();
      buildAsset_("unit.spider", sub, r, { variant: 2, param: "" });
      return place(mb, sub, [0, 0, 0], 0, 0.85);
    }
    if (kind === "centaur") {
      quad({ ...BEASTS.horse, coat: ["#6a4630", "#8a5a32", "#4a3a30"][variant % 3] }, { gait: "walk", noHead: true }, 0.75);
      // The human torso rising from the horse's chest (the rider's legs dropped).
      const R = RACES.human;
      const f = figure({ r, race: "human", variant, body: "vest", sleeves: "bare", seat: { w: 0.03 }, hairStyle: "long", mantle: true, left: { hand: [0.16, 0.02, 0.0], weapon: "bow", dir: [0.06, 1, 0.02], face: [-1, 0, 0], opts: (h) => ({ len: 0.3, recurve: true, draw: Math.max(0.08, vlen(vsub(h[L_SIDE].pos, h[R_SIDE].pos)) - 0.01) }) }, right: { handAt: (h, S) => vadd(S, [R.shW, 0.03, 0.025]) }, headYaw: 0.9 });
      const top = new MeshBuilder();
      const v = f.mb.v;
      for (let i = 0; i < v.length; i += 39) {
        if (v[i + 1] > R.hipY - 0.02 && v[i + 14] > R.hipY - 0.02 && v[i + 27] > R.hipY - 0.02) for (let j = 0; j < 39; j++) top.v.push(v[i + j]);
      }
      mb.part(() => {
        mb.rotateY(0.75);
        mb.translate(0, 0.21 - R.hipY + 0.06, 0.16);
        mb.append(top);
      });
      setTeam(mb, 0.9);
      mb.part(() => {
        mb.rotateY(0.75);
        mb.translate(0, 0.25, 0.16);
        ringTube(mb, 0.06, 0.05, 0.012, 10, 4);
      });
      return;
    }
    if (kind === "treant") {
      // A walking tree: root-footed stumps, a gnarled trunk leaning forward with a face in the
      // bark, branch arms, and a ragged crown of leaf clumps on its upper boughs (asymmetric,
      // with sky between them: never a ball on a stick).
      mb.part(() => {
        mb.rotateY(-0.2);
        local(mb, (k) => {
          const bark = C(variant === 2 ? "#5a4e44" : "#4a392b");
          const seed = r.range(0, 40);
          const grain = (m0) => k.tint(m0, (x, y, z) => 0.78 + 0.3 * Math.max(0, Math.sin(Math.atan2(z, x) * 7 + y * 14 + seed)));
          // Legs and roots.
          for (const s2 of [-1, 1]) {
            k.bone = s2 > 0 ? BONE.LEG_L : BONE.LEG_R;
            k.color = bark;
            k.mat = MAT.MATTE;
            const m0 = k.mark();
            const foot = [s2 * 0.08, -0.03, s2 * 0.02 + 0.01];
            k.tube([foot, [s2 * 0.07, 0.08, 0.01], [s2 * 0.035, 0.2, 0]], [0.05, 0.04, 0.05], 6, { caps: true });
            for (let j = 0; j < 3; j++) {
              const a = (s2 > 0 ? 0 : Math.PI) + (j - 1) * 0.9 + 0.3;
              horn(k, vadd(foot, [0, 0.04, 0]), [Math.cos(a), -0.35, Math.sin(a) + 0.3], 0.075, 0.016, [0, -0.4, 0], 3, 4);
            }
            grain(m0);
          }
          // The trunk, bending forward, splitting into boughs at the top.
          k.bone = BONE.BODY;
          k.color = bark;
          const tm = k.mark();
          k.tube([[0, 0.16, 0], [0.012, 0.29, 0.03], [-0.01, 0.41, 0.045], [0.005, 0.5, 0.02]], [0.082, 0.078, 0.07, 0.052], 8, { caps: true });
          const boughs = [[[0.01, 0.48, 0.02], [0.09, 0.58, -0.02], [0.15, 0.66, -0.04]], [[-0.01, 0.48, 0.02], [-0.07, 0.6, 0.0], [-0.12, 0.7, -0.03]], [[0, 0.5, 0.0], [0.02, 0.63, -0.07], [-0.01, 0.76, -0.09]]];
          for (const b of boughs) k.tube(b, [0.03, 0.022, 0.012], 5, { caps: true });
          grain(tm);
          // A face in the bark: a heavy brow, deep sockets with a green light, a hollow mouth.
          k.bone = BONE.HEAD;
          k.color = mixc(bark, "#000000", 0.25);
          k.tube([[-0.04, 0.455, 0.075], [0, 0.445, 0.088], [0.04, 0.455, 0.075]], [0.012, 0.016, 0.012], 5, { caps: true });
          k.color = C("#120c08");
          ball(k, [0, 0.39, 0.083], 0.016, 0, [1.3, 0.8, 0.5]);
          k.mat = MAT.EMISSIVE;
          k.color = C("#b8f090");
          for (const s2 of [-1, 1]) ball(k, [s2 * 0.022, 0.43, 0.084], 0.0075, 0);
          // Arms: one bough raised in threat, one hanging to a clawed hand of twigs.
          const arms = [[1, [0.06, 0.42, 0.01], [0.15, 0.4, 0.05], [0.2, 0.28, 0.09], [0.21, 0.17, 0.11]], [-1, [-0.06, 0.43, 0.01], [-0.15, 0.47, 0.04], [-0.2, 0.58, 0.06], [-0.2, 0.66, 0.1]]];
          for (const [s2, a0, a1, a2, a3] of arms) {
            k.bone = s2 > 0 ? BONE.ARM_L : BONE.ARM_R;
            k.mat = MAT.MATTE;
            k.color = bark;
            const am = k.mark();
            k.tube([a0, a1, a2, a3], [0.036, 0.028, 0.021, 0.013], 5, { caps: true });
            grain(am);
            for (let j = 0; j < 3; j++) horn(k, a3, vadd(vnorm(vsub(a3, a2)), [(j - 1) * 0.5, 0, (j - 1) * 0.2]), 0.05, 0.008, [0, s2 > 0 ? -0.3 : 0.3, 0.2], 3, 3);
          }
          // The crown: leaf clumps on the bough tips and the raised arm, sizes and heights varied.
          k.bone = BONE.BODY;
          k.mat = MAT.FOLIAGE;
          const leaf = variant === 1 ? ["ancientDark", "ancient", "ancientLight"] : variant === 2 ? ["#3a3a22", "#5a5228", "#8a7a3a"] : ["oakDark", "oak", "oakLight"];
          const clumps = [[0.16, 0.69, -0.05, 0.105], [-0.13, 0.73, -0.04, 0.098], [-0.01, 0.8, -0.1, 0.1], [0.04, 0.68, 0.04, 0.08], [0.0, 0.7, -0.17, 0.085], [-0.21, 0.69, 0.1, 0.06]];
          clumps.forEach(([x, y, z, rad], i) => {
            const m0 = k.mark();
            k.color = jitter(mixc(leaf[0], leaf[1], r.range(0.3, 0.9)), r, 0.06);
            k.part(() => {
              k.translate(x, y, z);
              k.scale(1, 0.78, 1);
              k.blob(rad, { detail: 1, amp: 0.3, noise: (v) => noise3(v[0] * 2.4 + seed + i * 3, v[1] * 2.4, v[2] * 2.4) });
            });
            k.sphereNormals(m0, [x, y - rad * 0.4, z], 0.75);
            k.tint(m0, (px, py) => mixc([1, 1, 1], [0.82, 0.86, 0.74], Math.max(0, Math.min(1, (y + rad * 0.2 - py) / (rad * 1.2)))));
            k.heightSway(m0, y - rad, y + rad, 0.45);
          });
          // A light top on the sunward clumps.
          // The owner's colour: a band of cloth knotted about the trunk, its ends hanging.
          k.mat = MAT.TEAM;
          setTeam(k, 0.85);
          k.part(() => {
            k.translate(0.004, 0.3, 0.028);
            k.rotateZ(0.12);
            ringTube(k, 0.083, 0.078, 0.015, 10, 4);
          });
          k.sway = 0.6;
          k.tube([[0.07, 0.3, 0.06], [0.085, 0.24, 0.075], [0.08, 0.18, 0.07]], [0.012, 0.01, 0.006], 4, { caps: true });
          k.sway = 0;
        });
      });
      return;
    }
    // Humanoid beasts.
    const fur = ["#5a4a3a", "#3e3430", "#7a6a50"][variant % 3];
    if (kind === "minotaur") {
      return fig({ race: "orc", skin: "#6a4a3a", beastHead: "bull", furColor: "#4a3428", body: "bare", bulk: 1.15, stance: "brace", lean: 0.12, skirt: 0.09, skirtColor: teamCol(0.9), skirtMat: MAT.TEAM, ragged: true, bracers: "#3a2a1e", barefoot: true, right: { hand: [0.03, 0.08, -0.01], elbow: [1, -0.15, -0.25], weapon: "axe", dir: [-0.5, 0.82, 0.1], opts: { len: 0.22, double: true, head: 1.45 } }, left: { hand: [0.06, -0.1, 0.1] } }, 1.3);
    }
    if (kind === "werewolf") {
      return fig({ race: "orc", skin: fur, beastHead: "wolf", body: "bare", mantle: fur, stance: "brace", lean: 0.35, skirt: 0.07, skirtColor: teamCol(0.85), skirtMat: MAT.TEAM, ragged: true, claws: true, barefoot: true, tail: true, right: { hand: [0.07, -0.02, 0.12], elbow: [0.8, -0.6, -0.2] }, left: { hand: [0.08, 0.02, 0.1], elbow: [0.8, -0.6, -0.2] } }, 1.2);
    }
    if (kind === "ape") {
      return fig({ race: "orc", skin: "#3a3430", beastHead: "ape", furColor: "#2e2a28", body: "bare", bulk: 1.25, stance: "wide", lean: 0.32, belt: false, skirt: 0.06, skirtColor: teamCol(0.88), skirtMat: MAT.TEAM, ragged: true, barefoot: true, right: { hand: [0.05, -0.2, 0.08], elbow: [1, 0, -0.4] }, left: { hand: [0.05, -0.12, 0.12], elbow: [1, -0.3, -0.4] } }, 1.25);
    }
    if (kind === "ogre" || kind === "giant") {
      const giant = kind === "giant";
      return fig({ race: "orc", skin: giant ? "#a88a6a" : "#8a8a5a", hairStyle: giant ? "long" : "none", beard: giant ? "full" : null, hair: "#4a3a2a", body: "fur", mantle: true, bulk: 1.2, stance: "brace", lean: 0.12, skirt: 0.1, skirtColor: teamCol(0.88), skirtMat: MAT.TEAM, ragged: true, barefoot: true, right: { hand: [0.03, 0.08, -0.01], elbow: [1, -0.15, -0.25], weapon: "club", dir: [-0.5, 0.82, 0.1], opts: { len: 0.24 } }, left: { hand: [0.06, -0.12, 0.1] } }, giant ? 1.55 : 1.35);
    }
    // Beastman: goat-headed, fur-legged, a crude axe and a round shield.
    return fig({ race: "human", skin: fur, beastHead: "goat", furColor: fur, body: "fur", mantle: true, legs: fur, boots: "#2a2220", stance: "brace", lean: 0.12, skirt: 0.08, skirtColor: teamCol(0.88), skirtMat: MAT.TEAM, ragged: true, bracers: true, right: { hand: [0.03, 0.08, -0.005], elbow: [1, -0.15, -0.25], weapon: variant % 2 ? "club" : "axe", dir: [-0.55, 0.8, 0.12], opts: { len: 0.19 } }, left: { hand: [-0.005, -0.09, 0.1], shield: "round", shieldFace: [0.32, 0.12, 1] } }, 1.05);
  },
});

// ---------------------------------------------------------------------------------------------
// Ships and engines

/**
 * A planked hull along z (bow at +z): `L` long, `B` beam, `F` freeboard at midships (deck height
 * above the water at y = 0), `D` draught below it, `sheer` how much bow and stern rise, `stern`
 * extra height of the stern post. The owner's colour runs in a band below the gunwale. Returns
 * {gunwale(t, side), width(t), top(t), Z(t)} (t 0 stern .. 1 bow).
 */
function hull(mb, { L, B, F, D, sheer = 0.03, stern = 0, hullC = "#4a3424", deckC = "plank", band = true, nz = 10, na = 5 }) {
  const width = (t) => {
    const u = t * 2 - 1; // -1 stern .. 1 bow
    const bow = u > 0 ? Math.pow(1 - u, 0.75) : Math.pow(1 + u, 0.42);
    return (B / 2) * Math.max(0.02, bow);
  };
  const top = (t) => F + sheer * Math.pow(Math.abs(t * 2 - 1), 2) + (t < 0.15 ? stern * ((0.15 - t) / 0.15) ** 1.5 : 0);
  const keel = (t) => -D * Math.min(1, Math.sin(Math.PI * Math.min(1, Math.max(0, t))) * 1.4 + 0.15);
  const Z = (t) => -L / 2 + L * t;
  const P = (i, j) => {
    const t = i / nz;
    const phi = Math.PI * (j / na); // 0 port gunwale .. pi starboard gunwale
    const w = width(t), yt = top(t), yk = keel(t);
    return [-w * Math.cos(phi), yt - (yt - yk) * Math.pow(Math.sin(phi), 0.65), Z(t)];
  };
  const hc = C(hullC);
  for (let i = 0; i < nz; i++) {
    for (let j = 0; j < na; j++) {
      if (band && (j === 0 || j === na - 1)) setTeam(mb, 0.88);
      else {
        mb.mat = MAT.MATTE;
        mb.color = mixc(hc, "#1a120c", (Math.abs(j - na / 2) < 1 ? 0.35 : 0) + ((i + j) % 2) * 0.06);
      }
      mb.quad(P(i, j), P(i + 1, j), P(i + 1, j + 1), P(i, j + 1));
    }
  }
  // The deck between the gunwales (a little below them).
  setMat(mb, deckC);
  for (let i = 0; i < nz; i++) {
    const a = P(i, 0), b = P(i + 1, 0), c = P(i + 1, na), d = P(i, na);
    const dy = -0.006;
    mb.quad([a[0], a[1] + dy, a[2]], [d[0], d[1] + dy, d[2]], [c[0], c[1] + dy, c[2]], [b[0], b[1] + dy, b[2]]);
  }
  // The rail.
  setMat(mb, "#2e2018");
  for (const side of [0, na]) {
    const pts = [];
    for (let i = 0; i <= nz; i++) pts.push(vadd(P(i, side), [0, 0.003, 0]));
    mb.tube(pts, 0.0035, 3);
  }
  return { gunwale: (t, s) => [s * width(t), top(t), Z(t)], width, top, Z };
}

/** A square sail hanging from a yard at height `y` on a mast at `z`: `w` wide, `h` deep,
 *  bellying forward, braced round the mast by `brace` (turned to the wind, and toward the
 *  camera); the owner's colour in vertical stripes. */
function squareSail(mb, z, y, w, h, { belly = 0.03, stripes = true, shade = 0.92, brace = 0 } = {}) {
  mb.part(() => {
    mb.translate(0, 0, z);
    mb.rotateY(brace);
    mb.translate(0, 0, -z);
    setMat(mb, "#3a2a1e");
    mb.part(() => {
      mb.translate(-w / 2 - 0.01, y, z);
      mb.rotateZ(-Math.PI / 2);
      mb.cylinder(0.0035, 0.0035, w + 0.02, 4, { caps: false });
    });
    const nu = 6, nv = 4;
    local(mb, (k) => {
      for (let i = 0; i < nu; i++) {
        for (let j = 0; j < nv; j++) {
          const P = (u, v) => [-w / 2 + w * u, y - h * v, z + 0.004 + belly * Math.sin(Math.PI * u) * Math.sin(Math.PI * Math.min(1, v * 0.9 + 0.1))];
          k.mat = MAT.TEAM;
          k.color = teamCol(stripes && i % 2 === 1 ? shade - 0.45 : shade);
          k.quad(P(i / nu, j / nv), P((i + 1) / nu, j / nv), P((i + 1) / nu, (j + 1) / nv), P(i / nu, (j + 1) / nv));
        }
      }
      k.sphereNormals(0, [0, y - h / 2, z - 0.2], 0.4);
      const v = k.v;
      for (let i = 0; i < v.length; i += 13) v[i + 10] = Math.max(0, Math.min(1, (y - v[i + 1]) / h)) * 0.35;
    });
  });
}

/** A pennant at a masthead. */
function pennant(mb, p, len = 0.07) {
  local(mb, (k) => {
    k.mat = MAT.TEAM;
    k.color = teamCol(0.95);
    k.card([p[0], p[1], p[2]], [p[0], p[1] - 0.018, p[2]], [p[0] - len * 0.6, p[1] - 0.012, p[2] - len * 0.8], [p[0] - len * 0.4, p[1] - 0.004, p[2] - len * 0.6]);
    const v = k.v;
    for (let i = 0; i < v.length; i += 13) v[i + 10] = Math.min(1, Math.hypot(v[i] - p[0], v[i + 2] - p[2]) * 14);
  });
}

/** unit.galley: a long low oared ship (trireme, galley): a bronze ram, a curling stern post, a
 *  row of shields and oars on each side, one mast with a square sail in the owner's colours. */
unitRecipe("unit.galley", {
  variants: 2,
  build(mb, r, { variant, param }) {
    const dark = param === "dark";
    mb.part(() => {
      mb.rotateY(1.15);
      local(mb, (k) => {
        const L = 0.62, B = 0.13;
        const H = hull(k, { L, B, F: 0.035, D: 0.035, sheer: 0.03, stern: 0.05, hullC: dark ? "#2a2224" : "#4e3624" });
        // Bronze ram at the waterline, eyes on the bow.
        setMat(k, "bronze", MAT.METAL);
        k.part(() => {
          k.translate(0, 0.0, L / 2 - 0.01);
          k.rotateX(Math.PI / 2);
          k.cone(0.016, 0.07, 5);
        });
        for (const s of [-1, 1]) {
          setMat(k, "#e8e0d0");
          const e = H.gunwale(0.9, s);
          ball(k, [e[0] * 1.02, e[1] - 0.014, e[2]], 0.008, 0, [0.4, 1, 1.4]);
          setMat(k, "#141010");
          ball(k, [e[0] * 1.06, e[1] - 0.014, e[2] + 0.002], 0.004, 0, [0.4, 1, 1]);
        }
        // The stern post curling up and over.
        setMat(k, dark ? "#2a2224" : "#4e3624");
        const st = H.gunwale(0.0, 0);
        horn(k, [0, st[1] - 0.02, -L / 2 + 0.005], [0, 1, -0.6], 0.12, 0.012, [0, -0.4, 2.2], 6, 5);
        // Oars: a bank on each side dipping into the water; shields along the rail.
        const n = 7;
        for (const s of [-1, 1]) {
          for (let i = 0; i < n; i++) {
            const t = 0.24 + (i / (n - 1)) * 0.52;
            const g = H.gunwale(t, s);
            const out = [g[0] + s * 0.13, -0.012, g[2] - 0.03 - (variant ? 0.02 : 0)];
            setMat(k, "timberLight");
            k.tube([[g[0] - s * 0.01, g[1] + 0.006, g[2]], out], 0.0028, 3);
            k.part(() => {
              orient(k, out, vsub(out, g), [0, 1, 0]);
              k.box(0.012, 0.03, 0.002);
            });
            if (i < n - 1) {
              const sp = H.gunwale(t + 0.04, s);
              k.part(() => {
                orient(k, [sp[0] + s * 0.004, sp[1] - 0.004, sp[2]], [s, 0.2, 0], [0, 1, 0]);
                setTeam(k, i % 2 ? 0.95 : 0.55);
                k.cylinder(0.017, 0.012, 0.004, 8);
              });
            }
          }
        }
        // Mast, yard and sail.
        setMat(k, "timber");
        k.part(() => {
          k.translate(0, 0.02, 0.02);
          k.cylinder(0.006, 0.0045, 0.33, 5);
        });
        squareSail(k, 0.026, 0.31, 0.2, 0.15, { belly: 0.035, brace: -0.7 });
        pennant(k, [0, 0.35, 0.02]);
        // Rigging.
        setMat(k, "rope");
        for (const s of [-1, 1]) k.tube([[0, 0.33, 0.02], [s * B * 0.45, 0.04, 0.02]], 0.0012, 3);
        k.tube([[0, 0.34, 0.02], [0, 0.07, L / 2 - 0.02]], 0.0012, 3);
        k.tube([[0, 0.34, 0.02], [0, 0.1, -L / 2 + 0.04]], 0.0012, 3);
        // A steering oar, and heads at the oars.
        setMat(k, "timber");
        k.tube([[0.05, 0.06, -L / 2 + 0.08], [0.08, -0.02, -L / 2 + 0.02]], 0.004, 4);
        for (let i = 0; i < 4; i++) {
          setMat(k, "skinHuman", MAT.SKIN);
          ball(k, [(i % 2 ? 1 : -1) * 0.03, 0.06, -0.12 + i * 0.07], 0.011, 0);
          setTeam(k, 0.7);
          ball(k, [(i % 2 ? 1 : -1) * 0.03, 0.04, -0.12 + i * 0.07], 0.015, 0, [1, 0.8, 1]);
        }
      });
    });
  },
});

/** unit.ship: a sailing ship (caravel to galleon by variant): high castles fore and aft, two or
 *  three masts of square sails in the owner's colours, a bowsprit and pennants. */
unitRecipe("unit.ship", {
  variants: 3,
  build(mb, r, { variant, param }) {
    const big = variant % 3;
    const dark = param === "dark";
    mb.part(() => {
      mb.rotateY(1.1);
      local(mb, (k) => {
        const L = 0.56 + big * 0.06, B = 0.16 + big * 0.02;
        const hc = dark ? "#2a2224" : "#5a3e28";
        const H = hull(k, { L, B, F: 0.055, D: 0.04, sheer: 0.035, stern: 0.0, hullC: hc });
        // Sterncastle and forecastle, with an owner's band under their rails.
        const castle = (t0, t1, h, inset) => {
          const z0 = H.Z(t0), z1 = H.Z(t1);
          const w0 = H.width(t0) * inset, w1 = H.width(t1) * inset;
          const y0 = Math.min(H.top(t0), H.top(t1)) - 0.008;
          const poly = (d) => [[-w0 - d, z0], [-w1 - d, z1], [w1 + d, z1], [w0 + d, z0]];
          k.part(() => {
            k.translate(0, y0, 0);
            setMat(k, hc);
            k.prism(poly(0), h);
            setTeam(k, 0.85);
            k.translate(0, h - 0.014, 0);
            k.prism(poly(0.002), 0.01, { cap: false });
            setMat(k, "#2e2018");
            k.translate(0, 0.014, 0);
            k.prism(poly(0.001), 0.006, { top: 0.96 });
          });
          return y0 + h;
        };
        const sternTop = castle(0.0, 0.24, 0.05 + big * 0.012, 0.98);
        castle(0.8, 0.96, 0.03 + big * 0.008, 0.95);
        k.mat = MAT.EMISSIVE;
        k.color = C("window");
        for (const s of [-1, 1]) ball(k, [s * 0.02, sternTop - 0.025, -L / 2 + 0.002], 0.006, 0, [1, 0.8, 0.3]);
        // Bowsprit.
        setMat(k, "timber");
        k.tube([[0, 0.07, L / 2 - 0.06], [0, 0.13, L / 2 + 0.1]], [0.005, 0.003], 4);
        // Masts and sails ([z, height]).
        const masts = big === 2 ? [[0.05, 0.5], [0.2, 0.42], [-0.15, 0.36]] : big === 1 ? [[0.04, 0.48], [0.19, 0.4]] : [[0.03, 0.46], [0.18, 0.38]];
        masts.forEach(([z, h], i) => {
          setMat(k, "timber");
          k.part(() => {
            k.translate(0, 0.03, z);
            k.cylinder(0.0065, 0.004, h, 5);
          });
          const w = (i === 0 ? 0.2 : 0.16) + big * 0.015;
          if (i === 2) {
            // Lateen mizzen: a triangle in the owner's colour.
            setTeam(k, 0.8);
            k.card([0, h * 0.95, z + 0.04], [0, 0.12, z + 0.08], [0, 0.11, z - 0.12], [0, 0.13, z - 0.11]);
            return;
          }
          squareSail(k, z + 0.006, h * 0.62, w, h * 0.27, { belly: 0.03, brace: -0.65 });
          squareSail(k, z + 0.006, h * 0.95, w * 0.75, h * 0.24, { belly: 0.025, shade: 0.85, brace: -0.65 });
          pennant(k, [0, h + 0.035, z], 0.08);
          if (i === 0) {
            // Crow's nest.
            setMat(k, "timber");
            k.part(() => {
              k.translate(0, h * 0.7, z);
              k.cylinder(0.014, 0.016, 0.014, 6);
            });
          }
        });
        // Rigging from the mastheads to the rails.
        setMat(k, "rope");
        for (const [z, h] of masts) {
          for (const s of [-1, 1]) k.tube([[0, h, z], [s * B * 0.46, 0.065, z - 0.06]], 0.0011, 3);
        }
        k.tube([[0, masts[0][1], masts[0][0]], [0, 0.13, L / 2 + 0.1]], 0.0011, 3);
      });
    });
  },
});

/** unit.catapult: a wheeled mangonel: a timber frame, a throwing arm with a stone in its cup
 *  resting against the padded crossbar (the owner's colour), a torsion bundle and a winch; a
 *  crewman beside it and the owner's banner on the frame. */
unitRecipe("unit.catapult", {
  variants: 2,
  build(mb, r, { variant, param }) {
    const { race, dark } = raceOf(param);
    const g = garbOf(race, dark);
    mb.part(() => {
      mb.translate(-0.03, 0, -0.03);
      mb.rotateY(0.9);
      mb.scale(1.35);
      local(mb, (k) => {
        const wood = C(g.wood === "timber" ? "#6a4a30" : g.wood);
        // Frame: two side beams and crossbeams.
        for (const s of [-1, 1]) {
          k.color = wood;
          k.mat = MAT.MATTE;
          k.part(() => {
            k.translate(s * 0.065, 0.045, 0);
            k.box(0.022, 0.026, 0.3);
          });
        }
        for (const z of [-0.12, 0.0, 0.12]) {
          k.part(() => {
            k.translate(0, 0.05, z);
            k.box(0.15, 0.018, 0.02);
          });
        }
        // Wheels.
        for (const [sx, sz] of [[-1, -1], [1, -1], [-1, 1], [1, 1]]) {
          k.part(() => {
            k.translate(sx * 0.085, 0.036, sz * 0.1);
            k.rotateZ(Math.PI / 2);
            k.translate(0, -0.008, 0);
            setMat(k, "#3a2a1e");
            k.cylinder(0.04, 0.04, 0.016, 10);
            setMat(k, g.metal2, MAT.METAL);
            k.translate(0, 0.016, 0);
            k.cylinder(0.012, 0.01, 0.006, 6);
          });
        }
        // Uprights and the padded crossbar (owner's colour).
        k.color = wood;
        k.mat = MAT.MATTE;
        for (const s of [-1, 1]) k.tube([[s * 0.065, 0.055, 0.0], [s * 0.06, 0.2, 0.05]], 0.011, 4, { caps: true });
        for (const s of [-1, 1]) k.tube([[s * 0.065, 0.055, 0.1], [s * 0.06, 0.2, 0.055]], 0.009, 4, { caps: true });
        k.part(() => {
          k.translate(-0.075, 0.205, 0.052);
          k.rotateZ(-Math.PI / 2);
          k.cylinder(0.012, 0.012, 0.15, 6);
        });
        setTeam(k, 0.92);
        k.part(() => {
          k.translate(-0.04, 0.206, 0.05);
          k.rotateZ(-Math.PI / 2);
          k.cylinder(0.02, 0.02, 0.08, 7);
        });
        // The torsion bundle and the arm (raised against the crossbar at the moment of throwing).
        setMat(k, "rope");
        k.part(() => {
          k.translate(-0.06, 0.07, -0.06);
          k.rotateZ(-Math.PI / 2);
          k.cylinder(0.022, 0.022, 0.12, 8);
        });
        k.color = wood;
        k.mat = MAT.MATTE;
        const armEnd = variant ? [0, 0.3, 0.12] : [0, 0.24, 0.1];
        k.tube([[0, 0.07, -0.06], vlerp([0, 0.07, -0.06], armEnd, 0.5), armEnd], [0.012, 0.01, 0.008], 5, { caps: true });
        // The cup and its stone.
        k.part(() => {
          orient(k, armEnd, vsub(armEnd, [0, 0.07, -0.06]), [0, 0, 1]);
          k.translate(0, 0.01, 0);
          setMat(k, wood);
          k.lathe([[0, -0.01], [0.026, 0.0], [0.03, 0.016]], 7, { caps: false });
          setMat(k, "stone");
          k.translate(0, 0.012, 0);
          k.blob(0.022, { detail: 1, amp: 0.2, flat: true, noise: (v) => noise3(v[0] * 3, v[1] * 3, v[2] * 3) });
        });
        // The winch at the back.
        k.color = wood;
        k.part(() => {
          k.translate(-0.08, 0.075, -0.135);
          k.rotateZ(-Math.PI / 2);
          k.cylinder(0.012, 0.012, 0.16, 6);
        });
        setMat(k, g.metal2, MAT.METAL);
        k.tube([[0.08, 0.075, -0.135], [0.095, 0.075, -0.135], [0.095, 0.1, -0.15]], 0.003, 3);
        // A pile of stones.
        setMat(k, "stoneDark");
        for (let i = 0; i < 3; i++) {
          k.part(() => {
            k.translate(0.11 + (i % 2) * 0.03, 0.014, -0.16 + i * 0.03);
            k.blob(0.02, { detail: 0, amp: 0.25, flat: true, noise: (v) => noise3(v[0] * 3 + i, v[1] * 3, v[2] * 3) });
          });
        }
      });
    });
    // The crewman.
    const R = RACES[race];
    const k2 = kit(race, dark, variant);
    const f = figure({ r, race, dark, variant, ...k2, stance: "brace", lean: 0.3, body: k2.body ?? "tunic", skirt: R.hipY * 0.38, head: race === "dwarf" ? "helm" : "kettle", right: { hand: [-0.02, -0.06, 0.13], elbow: [0.6, -0.6, -0.3] }, left: { hand: [-0.02, -0.07, 0.13], elbow: [0.6, -0.6, -0.3] } });
    place(mb, f.mb, [0.2, 0, 0.1], 2.2, 0.85);
    // The owner's banner planted on the frame.
    mb.part(() => {
      mb.translate(-0.17, 0.05, -0.12);
      mb.bone = BONE.CAPE;
      WEAPONS.banner(mb, g, { len: 0.36, w: 0.08, h: 0.11 });
    });
  },
});

// ---------------------------------------------------------------------------------------------
// Angels, birds and relics

/** unit.angel: a winged warrior in gilded white plate, feathered wings raised, a halo; variant 0
 *  sword and shield, 1 spear (valkyrie), 2 a fallen one (dark wings, a scythe). */
unitRecipe("unit.angel", {
  variants: 3,
  build(mb, r, { variant, param }) {
    const kind = variant % 3;
    const fallen = kind === 2 || param === "dark";
    const g = { ...GARB.human, metal: fallen ? "#4a4650" : "#d8d4cc", metal2: fallen ? "#2e2c34" : "#b8b0a0", trim: "gold" };
    const right =
      kind === 1
        ? { hand: [0.035, -0.04, 0.06], elbow: [0.8, -0.5, -0.4], weapon: "spear", dir: [-0.1, 1, 0.22], opts: { len: 0.24, back: 0.2, pennon: true } }
        : kind === 2
          ? { hand: [0.035, -0.04, 0.07], elbow: [0.8, -0.5, -0.4], weapon: "scythe", dir: [-0.1, 1, 0.15] }
          : { hand: [0.02, 0.08, 0.04], elbow: [1, -0.2, -0.1], weapon: "sword", dir: [-0.25, 0.95, 0.15], opts: { len: 0.2, w: 0.02, metal: "#e8e8f0" } };
    const f = figure({
      r, race: "elf", variant, garb: g,
      skin: fallen ? "#b8b0b8" : "#e8ccb0",
      hair: fallen ? "#1a1418" : "#e8d8a8",
      hairStyle: "long",
      body: "plate",
      tabard: true,
      tabardLen: 0.16,
      skirt: 0.1,
      pauldrons: true,
      gloves: true,
      greaves: true,
      stance: "stride",
      lean: 0.04,
      head: kind === 1 ? "winged" : "circlet",
      wings: { feathers: true, span: 0.32, lift: 0.3, sweep: 0.12, rows: 9, bone: fallen ? "#2a2830" : "#efeae0", membrane: fallen ? "#3a3644" : "#d8d0c0" },
      right,
      left: kind === 0 ? { hand: [-0.01, -0.1, 0.1], elbow: [0.9, -0.3, -0.2], shield: "heater", shieldFace: [0.25, 0.1, 1], shieldOpts: { rim: "gold", w: 0.075, h: 0.12 } } : { hand: [0.06, -0.06, 0.08] },
    });
    place(mb, f.mb, [0, 0, 0], 0, 1.08);
    // The halo.
    mb.mat = MAT.EMISSIVE;
    mb.color = C(fallen ? "#7a5aff" : "holy");
    mb.bone = BONE.HEAD;
    mb.part(() => {
      mb.translate(f.headC[0] * 1.08, (f.headC[1] + 0.06) * 1.08, (f.headC[2] - 0.02) * 1.08);
      mb.rotateX(-0.35);
      ringTube(mb, 0.034, 0.034, 0.0035, 14, 3);
    });
  },
});

/** unit.bird: a hawk on the wing (feathered wings spread, a fanned tail barred in the owner's
 *  colour). Placed with `fly`. */
unitRecipe("unit.bird", {
  variants: 1,
  build(mb, r) {
    mb.part(() => {
      mb.rotateY(0.6);
      local(mb, (k) => {
        k.color = C("#6a5038");
        k.part(() => {
          k.scale(0.8, 0.75, 1.5);
          k.sphere(0.035, 1);
        });
        k.color = C("#d8ccb0");
        k.part(() => {
          k.translate(0, -0.008, 0.01);
          k.scale(0.7, 0.6, 1.2);
          k.sphere(0.03, 1);
        });
        k.color = C("#5a4430");
        ball(k, [0, 0.018, 0.055], 0.022, 1);
        setMat(k, "#e0b040");
        k.part(() => {
          k.translate(0, 0.014, 0.074);
          k.rotateX(1.9);
          k.cone(0.007, 0.018, 4);
        });
        k.mat = MAT.TEAM;
        for (let i = 0; i < 5; i++) {
          const a = (i - 2) * 0.22;
          k.color = C(teamCol(i % 2 ? 0.5 : 0.9));
          k.quad([0, 0, -0.04], [Math.sin(a - 0.1) * 0.08, 0.004, -0.04 - Math.cos(a - 0.1) * 0.08], [Math.sin(a + 0.1) * 0.08, 0.004, -0.04 - Math.cos(a + 0.1) * 0.08], [0, 0, -0.04]);
        }
        for (const s of [-1, 1]) wing(k, s, [s * 0.02, 0.012, 0.01], { feathers: true, hang: 0, span: 0.17, lift: 0.04, sweep: 0.05, rows: 8, bone: "#5a4430", membrane: "#3e3024" });
      });
    });
  },
});

/** unit.chest: a treasure chest or relic (equipment lying on the map): iron-bound, gold
 *  glinting at the lid, the owner's pennant beside it. */
unitRecipe("unit.chest", {
  variants: 1,
  build(mb) {
    setMat(mb, "#5a3a22");
    mb.part(() => {
      mb.translate(0, -0.01, 0);
      mb.rotateY(0.4);
      mb.box(0.12, 0.07, 0.08);
      setMat(mb, "#6a4428");
      mb.translate(0, 0.07, 0);
      mb.scale(1, 0.6, 1);
      mb.rotateZ(Math.PI / 2);
      mb.translate(0, -0.06, 0);
      mb.cylinder(0.04, 0.04, 0.12, 8, { caps: true });
    });
    setMat(mb, "gold", MAT.METAL);
    mb.part(() => {
      mb.rotateY(0.4);
      for (const x of [-0.04, 0.04]) {
        mb.part(() => {
          mb.translate(x, 0.0, 0);
          mb.box(0.01, 0.09, 0.084);
        });
      }
      mb.translate(0, 0.04, 0.042);
      mb.box(0.018, 0.022, 0.004);
    });
    mb.mat = MAT.EMISSIVE;
    mb.color = C("#ffd060");
    ball(mb, [0.0, 0.075, 0.03], 0.008, 0);
    mb.part(() => {
      mb.translate(0.1, 0, -0.04);
      mb.bone = BONE.CAPE;
      WEAPONS.banner(mb, GARB.human, { len: 0.22, w: 0.06, h: 0.08 });
    });
  },
});

// ---------------------------------------------------------------------------------------------
// The catalog: the game's unit types -> figures

/** The race of a civilization's people. */
const CIV_RACE = {
  CIVILIZATION_LJOSALFAR: "elf",
  CIVILIZATION_SVARTALFAR: "elf-dark",
  CIVILIZATION_KHAZAD: "dwarf",
  CIVILIZATION_LUCHUIRP: "dwarf",
  CIVILIZATION_CLAN_OF_EMBERS: "orc",
  CIVILIZATION_INFERNAL: "demon",
  CIVILIZATION_SHEAIM: "human-dark",
  CIVILIZATION_CALABIM: "human-dark",
  CIVILIZATION_BARBARIAN: "orc",
};

/** A race implied by the unit's own name (overrides the owner's). */
function raceByName(n) {
  if (/GOBLIN|FROSTLING|SCORPION_CLAN/.test(n)) return "goblin";
  if (/LIZARDMAN/.test(n)) return "lizard";
  if (/SKELETON|LICH|DRAUGR|WRAITH|SPECTRE|GHOST|ZOMBIE|DISEASED_CORPSE|BONE_GOLEM|DEATH_KNIGHT|^UNIT_ARS$|BUBOES|STEPHANOS|YERSINIA|BLACK_WIND|SAILORS_DIRGE|PYRE_ZOMBIE/.test(n)) return "undead";
  if (/DWARVEN|HORNGUARD|BAMBUR|MARDERO|RUNES/.test(n)) return "dwarf";
  if (/^UNIT_(MOROI|VAMPIRE|BRUJAH|NYXKIN|SHADOW|ALAZKAN|MARY|SAVEROUS|LOSHA|DIVIDED_SOUL|EATER_OF_DREAMS)/.test(n)) return "human-dark";
  if (/OGRE|ORTHUS|CHAOS_MARAUDER|BOAR_RIDER|WOLF_RIDER/.test(n)) return "orc";
  if (/IMP|BALOR|MANES|SUCCUBUS|HYBOREM|MESHABBER|SON_OF_THE_INFERNO|PIT_BEAST|TAR_DEMON|ROSIER|HELLHOUND/.test(n)) return "demon";
  if (/GILDEN|ARTHENDAIN/.test(n)) return "elf";
  return null;
}

/** Head-top heights of the recipes (for the HUD badge), before race and scale. */
const HEAD_TOP = {
  "unit.warrior": 0.48, "unit.soldier": 0.5, "unit.spearman": 0.5, "unit.archer": 0.5, "unit.mage": 0.56, "unit.priest": 0.56, "unit.worker": 0.5,
  "unit.hero": 0.56, "unit.scout": 0.5, "unit.cavalry": 0.66, "unit.settler": 0.46, "unit.wolf": 0.3, "unit.bear": 0.35, "unit.spider": 0.32,
  "unit.dragon": 0.62, "unit.demon": 0.6, "unit.golem": 0.56, "unit.elemental": 0.6, "unit.skeleton": 0.48, "unit.serpent": 0.48, "unit.beast": 0.55,
  "unit.galley": 0.36, "unit.ship": 0.52, "unit.catapult": 0.42, "unit.angel": 0.6, "unit.bird": 0.08, "unit.chest": 0.24,
};
const RACE_H = { human: 1, elf: 1.06, dwarf: 0.78, orc: 1.02, goblin: 0.7, lizard: 1, undead: 1, demon: 1.06 };
const HUMANOIDS = new Set(["unit.warrior", "unit.soldier", "unit.spearman", "unit.archer", "unit.mage", "unit.priest", "unit.worker", "unit.hero", "unit.scout"]);

/** Specific unit types (most specific first): name pattern -> look (param "@race" = the
 *  owner's race, or the race the name implies). */
const UNIT_RULES = [
  [/EQUIPMENT_|TREASURE/, { asset: "unit.chest" }],
  // Dragons and the great beasts of the sea.
  [/^UNIT_ACHERON$/, { asset: "unit.dragon", param: "red" }],
  [/ABASHI/, { asset: "unit.dragon", param: "black" }],
  [/DRIFA/, { asset: "unit.dragon", param: "white" }],
  [/EURABATRES/, { asset: "unit.dragon", param: "gold" }],
  [/KRAKEN|LEVIATHAN/, { asset: "unit.serpent", param: "dark", scale: 1.3, float: true }],
  [/SEA_SERPENT/, { asset: "unit.serpent", float: true }],
  [/GIANT_TORTOISE/, { asset: "unit.serpent", variant: 1, scale: 0.85, float: true }],
  // Animals.
  [/POLAR_BEAR/, { asset: "unit.bear", param: "polar" }],
  [/^UNIT_BEAR$/, { asset: "unit.bear" }],
  [/WOLF_PACK/, { asset: "unit.wolf", variant: 1, scale: 1.12 }],
  [/HELLHOUND/, { asset: "unit.wolf", param: "hell", scale: 1.05 }],
  [/^UNIT_WOLF$/, { asset: "unit.wolf" }],
  [/BABY_SPIDER/, { asset: "unit.spider", scale: 0.55 }],
  [/SPIDERKIN/, { asset: "unit.spider", variant: 2, scale: 0.8 }],
  [/SPIDER/, { asset: "unit.spider" }],
  [/^UNIT_SCORPION$/, { asset: "unit.beast", param: "scorpion" }],
  [/LION_PRIDE/, { asset: "unit.beast", param: "lion", variant: 1, scale: 1.1 }],
  [/SAND_LION/, { asset: "unit.beast", param: "sandlion" }],
  [/LION|GRIFFON|MANTICORE/, { asset: "unit.beast", param: "lion" }],
  [/TIGER/, { asset: "unit.beast", param: "tiger" }],
  [/WAR_ELEPHANT/, { asset: "unit.beast", param: "war-elephant", scale: 1.1 }],
  [/ELEPHANT/, { asset: "unit.beast", param: "elephant" }],
  [/GORILLA/, { asset: "unit.beast", param: "ape" }],
  [/HAWK/, { asset: "unit.bird", fly: 0.3, height: 0.1 }],
  [/^UNIT_EYE$/, { asset: "unit.elemental", param: "air", scale: 0.55, fly: 0.15 }],
  [/BEASTMAN|SATYR|FAWN|REVELERS/, { asset: "unit.beast", param: "beastman" }],
  [/MINOTAUR/, { asset: "unit.beast", param: "minotaur" }],
  [/WEREWOLF|DUIN|SONS_OF_ASENA/, { asset: "unit.beast", param: "werewolf" }],
  [/STONESKIN_OGRE|OGRE_WARCHIEF/, { asset: "unit.beast", param: "ogre", scale: 1.1 }],
  [/OGRE|BEAST_OF_AGARES/, { asset: "unit.beast", param: "ogre" }],
  [/HILL_GIANT|GURID|MARGALARD/, { asset: "unit.beast", param: "giant" }],
  [/TREANT|GUARDIAN_VINES|MYCONID/, { asset: "unit.beast", param: "treant" }],
  [/CENTAUR/, { asset: "unit.beast", param: "centaur" }],
  // Constructs and elementals.
  [/MUD_GOLEM/, { asset: "unit.golem", param: "clay", scale: 0.85 }],
  [/IRON_GOLEM|CLOCKWORK_GOLEM|WAR_MACHINE/, { asset: "unit.golem", param: "iron" }],
  [/WOOD_GOLEM/, { asset: "unit.golem", param: "wood" }],
  [/FLESH_GOLEM|FREAK/, { asset: "unit.golem", param: "flesh" }],
  [/BONE_GOLEM/, { asset: "unit.golem", param: "bone" }],
  [/MITHRIL_GOLEM|BARNAXUS/, { asset: "unit.golem", param: "mithril", scale: 1.1 }],
  [/NULLSTONE_GOLEM|GARGOYLE|GOLEM/, { asset: "unit.golem", param: "stone" }],
  [/TAR_DEMON/, { asset: "unit.golem", param: "flesh", scale: 0.9 }],
  [/FIRE_ELEMENTAL|AZER|FIREBALL|METEOR/, { asset: "unit.elemental", param: "fire" }],
  [/WATER_ELEMENTAL|DROWN/, { asset: "unit.elemental", param: "water" }],
  [/AIR_ELEMENTAL|LIGHTNING_ELEMENTAL|DJINN|MISTFORM/, { asset: "unit.elemental", param: "air", fly: 0.04 }],
  [/EARTH_ELEMENTAL/, { asset: "unit.elemental", param: "earth" }],
  [/ICE_ELEMENTAL|AUREALIS/, { asset: "unit.elemental", param: "ice" }],
  [/^UNIT_GOBLIN/, { asset: "unit.warrior", param: "goblin", variant: 1 }],
  [/^UNIT_LIZARDMAN$/, { asset: "unit.spearman", param: "lizard", variant: 2 }],
  // Demons and the dead.
  [/^UNIT_IMP$/, { asset: "unit.demon", variant: 0 }],
  [/BALOR|AVATAR|WRATH|MESHABBER/, { asset: "unit.demon", variant: 2 }],
  [/PIT_BEAST/, { asset: "unit.demon", variant: 1 }],
  [/MANES/, { asset: "unit.demon", variant: 0, scale: 1.25 }],
  [/SUCCUBUS/, { asset: "unit.demon", variant: 2, scale: 0.85 }],
  [/HYBOREM|ROSIER/, { asset: "unit.hero", param: "demon" }],
  [/SKELETON/, { asset: "unit.skeleton" }],
  [/LICH/, { asset: "unit.mage", param: "undead", variant: 1 }],
  [/WRAITH|SPECTRE|GHOST|SHADE|SEVERED_SOUL|DIVIDED_SOUL/, { asset: "unit.mage", param: "undead", variant: 1, fly: 0.04 }],
  [/DRAUGR|PYRE_ZOMBIE|DISEASED_CORPSE|ZOMBIE/, { asset: "unit.warrior", param: "undead" }],
  [/DEATH_KNIGHT|^UNIT_ARS$|BUBOES|STEPHANOS|YERSINIA/, { asset: "unit.cavalry", param: "undead", variant: 3 }],
  [/ANGEL_OF_DEATH|REPENTANT_ANGEL/, { asset: "unit.angel", variant: 2, fly: 0.05 }],
  [/VALKYRIE|EINHERJAR/, { asset: "unit.angel", variant: 1, fly: 0.04 }],
  [/ANGEL|SERAPH|OPHANIM|LIGHTBRINGER|EIDOLON/, { asset: "unit.angel", variant: 0, fly: 0.04 }],
  // Ships, siege, civilians.
  [/TRIREME/, { asset: "unit.galley", variant: 0, float: true }],
  [/GALLEY|WORKBOAT/, { asset: "unit.galley", variant: 1, float: true }],
  [/CARAVEL|ARCANE_BARGE/, { asset: "unit.ship", variant: 0, float: true }],
  [/GALLEON|PRIVATEER|PIRATE|FRIGATE/, { asset: "unit.ship", variant: 1, float: true }],
  [/QUEEN_OF_THE_LINE|MAN_O_WAR|BLACK_WIND|SAILORS_DIRGE/, { asset: "unit.ship", variant: 2, float: true }],
  [/AIRSHIP/, { asset: "unit.ship", variant: 0, fly: 0.3, scale: 0.8 }],
  [/CATAPULT|TREBUCHET|BATTERING_RAM|CANNON/, { asset: "unit.catapult", param: "@race" }],
  [/SETTLER|SUPPLIES/, { asset: "unit.settler", param: "@race" }],
  [/WORKER|SLAVE|ENGINEER|MERCHANT/, { asset: "unit.worker", param: "@race" }],
  [/PROPHET|ARTIST/, { asset: "unit.priest", param: "@race" }],
  [/SCIENTIST/, { asset: "unit.mage", param: "@race", variant: 2 }],
  [/COMMANDER|ADVENTURER/, { asset: "unit.hero", param: "@race" }],
  // Mounted.
  [/HORSE_ARCHER|CAMEL_ARCHER|FYRDWELL|NYXKIN/, { asset: "unit.cavalry", param: "@race", variant: 2 }],
  [/WOLF_RIDER/, { asset: "unit.cavalry", param: "goblin", variant: 1 }],
  [/BOAR_RIDER|BISON_RIDER/, { asset: "unit.cavalry", param: "orc-boar", variant: 1 }],
  [/HORNGUARD/, { asset: "unit.cavalry", param: "dwarf", variant: 0 }],
  [/KNIGHT|ROYAL_GUARD|MAGNADINE|VALIN|KITHRA|SHADOWRIDER|WAR_TORTOISE|HERNE/, { asset: "unit.cavalry", param: "@race", variant: 3 }],
  [/CHARIOT|HORSEMAN|MERCENARY_MOUNTED|RATHA|TUMTUM/, { asset: "unit.cavalry", param: "@race", variant: 1 }],
  // Foot.
  [/LONGBOW|ARCHER|NIGHTWATCH|FIREBOW|MARKSMAN|ARTHENDAIN|GILDEN|TEUTORIX|FLURRY|CROSSBOW|ARQUEBUS|SLINGER/, { asset: "unit.archer", param: "@race" }],
  [/JAVELIN|PHALANX|SPEAR|STYGIAN|RADIANT_GUARD/, { asset: "unit.spearman", param: "@race" }],
  [/HUNTER|RANGER|BEASTMASTER/, { asset: "unit.scout", param: "@race", variant: 1 }],
  [/SCOUT|ASSASSIN|SHADOW|COURTESAN|HARLEQUIN|CHANTER|DEVOUT|TASKMASTER|HERALD|ALAZKAN|RATHUS/, { asset: "unit.scout", param: "@race", variant: 0 }],
  [/ARCHMAGE|WIZARD|MAGE|ILLUSIONIST|ADEPT|SHAMAN|EATER_OF_DREAMS|MOBIUS_WITCH|SON_OF_THE_INFERNO|DISCIPLE_OF_ACHERON/, { asset: "unit.mage", param: "@race" }],
  [/DISCIPLE|PRIEST|DRUID|MONK|LUONNOTAR|SPHENER|YVAIN|CHALID|PARAMANDER/, { asset: "unit.priest", param: "@race" }],
  [/CRUSADER|PALADIN/, { asset: "unit.hero", param: "@race", variant: 0 }],
  [/^UNIT_WARRIOR$|BERSERKER|BLOODPET|CHAOS_MARAUDER|LUNATIC|MUIRIN/, { asset: "unit.warrior", param: "@race" }],
  [/AXEMAN|DWARVEN_SOLDIER/, { asset: "unit.soldier", param: "@race", variant: 1 }],
  [/IMMORTAL|MOROI|VAMPIRE|BRUJAH/, { asset: "unit.soldier", param: "@race", variant: 2 }],
  [/CHAMPION|BATTLEMASTER|DRAGON_SLAYER/, { asset: "unit.soldier", param: "@race", variant: 3 }],
  [/FLAGBEARER|BOARDING|MERCENARY|SWORDSMAN|GRIGORI_MEDIC|DEMAGOG/, { asset: "unit.soldier", param: "@race", variant: 0 }],
];

/** Per-name variant tweaks the rule table does not express. */
function nameVariant(n, look) {
  if (/LONGBOW|MARKSMAN|NIGHTWATCH/.test(n) && look.asset === "unit.archer") look.variant = 2;
  if (/RANGER/.test(n) && look.asset === "unit.scout") look.variant = 3;
  if (/ARCHMAGE|WIZARD/.test(n) && look.asset === "unit.mage") look.variant = 3;
  if (/HIGH_PRIEST/.test(n) && look.asset === "unit.priest") look.variant = 2;
  if (/BERSERKER/.test(n)) look.variant = 2;
  return look;
}

/** The fallback by combat class and domain (no unit goes without a figure). */
function byCombat(combat, domain, tier) {
  if (domain === "DOMAIN_SEA") return { asset: tier >= 3 ? "unit.ship" : "unit.galley", float: true };
  if (domain === "DOMAIN_AIR") return { asset: "unit.bird", fly: 0.3 };
  switch (combat) {
    case "UNITCOMBAT_MELEE":
      return tier >= 4 ? { asset: "unit.hero", param: "@race" } : tier <= 1 ? { asset: "unit.warrior", param: "@race" } : { asset: "unit.soldier", param: "@race", variant: tier >= 3 ? 3 : 0 };
    case "UNITCOMBAT_ARCHER":
      return { asset: "unit.archer", param: "@race" };
    case "UNITCOMBAT_MOUNTED":
      return { asset: "unit.cavalry", param: "@race", variant: tier >= 4 ? 3 : 1 };
    case "UNITCOMBAT_ADEPT":
      return { asset: "unit.mage", param: "@race" };
    case "UNITCOMBAT_DISCIPLE":
      return { asset: "unit.priest", param: "@race" };
    case "UNITCOMBAT_RECON":
      return { asset: "unit.scout", param: "@race" };
    case "UNITCOMBAT_SIEGE":
      return { asset: "unit.catapult", param: "@race" };
    case "UNITCOMBAT_NAVAL":
      return { asset: "unit.ship", float: true };
    case "UNITCOMBAT_ANIMAL":
      return { asset: "unit.beast", param: "boar" };
    case "UNITCOMBAT_BEAST":
      return { asset: "unit.beast", param: "ogre" };
    default:
      return tier >= 3 ? { asset: "unit.hero", param: "@race" } : { asset: "unit.soldier", param: "@race" };
  }
}

/** A unit type's figure: {asset, param, variant, scale, height, float, fly} (ctx as dress.js
 *  passes it: info, civ, domain, combat). */
export function unitLook(name, ctx = {}) {
  const n = String(name || "");
  const tier = ctx.info?.tier ?? 1;
  const ownRace = raceByName(n) ?? CIV_RACE[ctx.civ] ?? "human";
  let look = null;
  for (const [re, l] of UNIT_RULES) {
    if (re.test(n)) {
      look = { ...l };
      break;
    }
  }
  if (!look) look = byCombat(ctx.combat, ctx.domain, tier);
  if (look.param === "@race") {
    look.param = ownRace;
    // The dark faiths' clergy wear black, whoever they serve.
    if (/VEIL|OVERLORDS/.test(n) && !look.param.includes("-")) look.param += "-dark";
  }
  look = nameVariant(n, look);
  if (ctx.domain === "DOMAIN_SEA" && !look.float && !look.fly) look.float = true;
  const race = (look.param ?? "").split("-")[0];
  const rh = HUMANOIDS.has(look.asset) ? RACE_H[race] ?? 1 : 1;
  look.height = look.height ?? (HEAD_TOP[look.asset] ?? 0.5) * rh * (look.scale ?? 1) + (look.fly ?? 0);
  return look;
}

extendCatalog({
  unit: (name, ctx) => unitLook(name, ctx),
});

// The generators, for other modules that want a figure (a guard at a gate, a farmer in a field).
export { figure, quadruped, RACES, BEASTS };

