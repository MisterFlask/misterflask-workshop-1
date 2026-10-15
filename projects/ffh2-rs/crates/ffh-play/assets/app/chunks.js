// What a chunk of the art renderer's terrain cache draws over compose.js's terrain for the map's
// layers (design §5.1 "Layers"), shared by the page (artmap.js) and the art worker
// (artworker.js): no DOM, only the art.

import * as UI from "../art/ui/ui.js";
import { unpackYield } from "./tiles.js";

export { CHUNK } from "./stacks.js";

/** The layer over a chunk drawn at 1x on `ctx` (tiles tx0.., ty0.. in the art's rows, row 0
 *  north): "political" or "yield"; "terrain" adds nothing. */
export function drawChunkLayers(art, ctx, layer, { map, plots, owners, tx0, ty0, cols, rows }) {
  if (layer === "political") politicalWash(art, ctx, plots, owners, tx0, ty0, cols, rows);
  else if (layer === "yield") yieldLayer(art, ctx, map, tx0, ty0, cols, rows);
}

/** The political layer (docs/ART.md 8.1): every owned pixel of revealed ground through a 25%
 *  grid of the owner's shade. */
function politicalWash(art, ctx, plots, owners, tx0, ty0, cols, rows) {
  for (let j = 0; j < rows; j++) {
    for (let i = 0; i < cols; i++) {
      const p = plots.get(tx0 + i, ty0 + j);
      if (!p || !p.vis || p.owner === null || p.owner === undefined) continue;
      const o = owners.get(p.owner);
      if (!o) continue;
      const shade = o.tincture.shade;
      const X0 = p.x * 32;
      const Y0 = p.y * 32;
      art.remapRegion(ctx, i * 32, j * 32, 32, 32, (a, b, c) => (((X0 + a) & 1) === 0 && ((Y0 + b) & 1) === 0 ? shade : c));
    }
  }
}

/** The yield layer: the ground through FOG, and each visible plot's food, production and
 *  commerce as their 8x8 icons and Tally digits (ui.js drawTileYields). */
function yieldLayer(art, ctx, map, tx0, ty0, cols, rows) {
  const FOG = art.tables.FOG;
  art.remapRegion(ctx, 0, 0, cols * 32, rows * 32, (a, b, c) => FOG[c]);
  for (let j = 0; j < rows; j++) {
    for (let i = 0; i < cols; i++) {
      const y = map.h - 1 - (ty0 + j);
      const x = tx0 + i;
      const yl = unpackYield(map.yield[y * map.w + x]);
      if (yl) UI.drawTileYields(art, ctx, i * 32, j * 32, yl);
    }
  }
}
