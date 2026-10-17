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
 *  name for far zooms). */
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
