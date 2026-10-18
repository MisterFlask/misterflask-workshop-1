// The asset registry of the 3D renderer (docs/R3D.md "Asset recipes"). A recipe is a function that
// models one kind of thing with the mesh builder; the registry builds each (name, variant, param)
// once, on first use, and caches the vertex data. Recipes are deterministic: the variant number
// seeds their randomness, so variant 3 of "tree.oak" is the same tree on every machine.
//
//   recipe("tree.oak", { variants: 6, build(mb, r, { variant, param }) { ... } })
//
// `r` is a seeded generator (noise.js `rng`), `param` a recipe-specific string (a unit's race, a
// house's architectural family, a mana node's school). Scale: one unit is one map plot; things
// stand on y = 0, centred on x = z = 0, facing +z (toward the camera at rotation 0).

import { MeshBuilder } from "../mesh.js";
import { rng, hash32 } from "../noise.js";

const RECIPES = new Map();
const BUILT = new Map();

/** Registers recipe `name`: `{variants, build(mb, r, ctx)}`; optional `lod` (a simpler recipe
 *  name for far zooms) and `shadow: false` (ground clutter too small to cast into the shadow map). */
export function recipe(name, def) {
  RECIPES.set(name, { variants: 1, ...def });
}

export function hasRecipe(name) {
  return RECIPES.has(name);
}

export function variantsOf(name) {
  return RECIPES.get(name)?.variants ?? 0;
}

export function recipeNames() {
  return [...RECIPES.keys()].sort();
}

/** The built mesh data of (name, variant, param): `{data, count, lo, hi, radius}`. */
export function buildAsset(name, variant = 0, param = "") {
  const def = RECIPES.get(name);
  if (!def) throw new Error(`r3d: no recipe "${name}"`);
  const v = ((variant % def.variants) + def.variants) % def.variants;
  const key = `${name}#${v}#${param}`;
  let out = BUILT.get(key);
  if (!out) {
    const mb = new MeshBuilder();
    const seed = hash32(hash32(strHash(name)) ^ (v * 7919 + 1) ^ strHash(param));
    def.build(mb, rng(seed), { variant: v, param, name });
    out = mb.build();
    out.shadow = def.shadow !== false;
    BUILT.set(key, out);
  }
  return out;
}

/** Forgets the built meshes (after a recipe module is reloaded in the lab). */
export function clearBuilt() {
  BUILT.clear();
}

export function strHash(s) {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 0x01000193);
  return h >>> 0;
}

const LODS = new Map();

/** A simplified copy of (name, variant, param) for distant drawing and shadows: vertex clustering
 *  on a grid of about a tenth of the asset's size (vertices of one material and bone that fall in
 *  one cell merge into their average; triangles that collapse go). Generic, so every recipe gets a
 *  level of detail without modelling one. Null when it would not save a third of the triangles. */
export function buildAssetLOD(name, variant = 0, param = "") {
  const key = `${name}#${variant}#${param}`;
  if (LODS.has(key)) return LODS.get(key);
  const full = buildAsset(name, variant, param);
  // A tenth of the size first; a coarser grid when that keeps too much.
  let out = simplify(full, 11);
  if (out.count >= full.count * 0.67) out = simplify(full, 7);
  const lod = out.count < full.count * 0.67 ? out : null;
  LODS.set(key, lod);
  return lod;
}

function simplify(full, cells = 11) {
  const F = 13;
  const d = full.data;
  const ext = Math.max(full.hi[0] - full.lo[0], full.hi[1] - full.lo[1], full.hi[2] - full.lo[2], 1e-3);
  const cell = Math.max(0.008, ext / cells);
  const ids = new Map();
  const acc = [];
  const vid = new Int32Array(full.count);
  for (let v = 0; v < full.count; v++) {
    const o = v * F;
    const k = `${Math.floor(d[o] / cell)},${Math.floor(d[o + 1] / cell)},${Math.floor(d[o + 2] / cell)},${d[o + 9]},${d[o + 11]}`;
    let c = ids.get(k);
    if (c === undefined) {
      c = acc.length;
      ids.set(k, c);
      acc.push(new Float64Array(F + 1));
    }
    const a = acc[c];
    for (let j = 0; j < F; j++) a[j] += d[o + j];
    a[F]++;
    vid[v] = c;
  }
  const seen = new Set();
  const outv = [];
  const avg = (c) => {
    const a = acc[c];
    const n = a[F];
    const r = Array.from({ length: F }, (_, j) => a[j] / n);
    const l = Math.hypot(r[3], r[4], r[5]);
    if (l > 1e-4) {
      r[3] /= l;
      r[4] /= l;
      r[5] /= l;
    } else {
      r[3] = 0;
      r[4] = 1;
      r[5] = 0;
    }
    return r;
  };
  const cache = new Map();
  const vert = (c) => {
    let r = cache.get(c);
    if (!r) cache.set(c, (r = avg(c)));
    return r;
  };
  for (let t = 0; t < full.count; t += 3) {
    const a = vid[t], b = vid[t + 1], c = vid[t + 2];
    if (a === b || b === c || a === c) continue;
    const s = [a, b, c].sort((x, y) => x - y).join(",");
    if (seen.has(s)) continue;
    seen.add(s);
    for (const k of [a, b, c]) outv.push(...vert(k));
  }
  const data = new Float32Array(outv);
  return { data, count: data.length / F, lo: full.lo, hi: full.hi, radius: full.radius };
}
