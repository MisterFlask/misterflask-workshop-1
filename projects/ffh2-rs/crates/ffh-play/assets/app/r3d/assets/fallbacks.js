// The fallback catalog (docs/R3D.md "Asset recipes"): everything maps to the basic recipes until a
// recipe module claims it. Imported before the recipe modules, so theirs win.

import { extendCatalog } from "./catalog.js";
import { FEAT } from "../world.js";

// Fallback catalog: everything maps to the basic recipes.
extendCatalog({
  forest: (feat) => {
    if (feat === FEAT.BURNT) return [{ asset: "basic.tree", scale: 0.9, tint: [0.35, 0.3, 0.28] }];
    if (feat === FEAT.YOUNG) return [{ asset: "basic.tree", scale: 0.6 }];
    return [{ asset: "basic.tree", scale: 1 }, { asset: "basic.pine", scale: 1 }];
  },
  improvement: (type) => ({ asset: /MINE|QUARRY/.test(type) ? "basic.rock" : "basic.house", scale: 1 }),
  bonus: () => ({ asset: "basic.rock", scale: 0.5, tint: [1.2, 1.0, 0.6] }),
  unit: () => ({ asset: "basic.unit" }),
});
