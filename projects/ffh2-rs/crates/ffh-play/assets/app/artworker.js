// The art renderer's worker: draws the terrain cache's chunks (artmap.js) off the page's thread,
// so that panning and hovering never wait for the art. It holds its own copy of the art (the same
// sprite bundle), of the map and of what the chunks show of the view (owners' heraldry, cities),
// all sent by the page in order before the chunks that depend on them; each chunk comes back as
// an ImageBitmap at 1x.
//
// Messages from the page: {t: "init", bundle, names}; {t: "map", map}; {t: "view", owners, cities};
// {t: "render", key, cx, cr, layer, gen}. To the page: {t: "ready"} | {t: "error", error};
// {t: "chunk", key, gen, bitmap, ms}.

import { loadArt } from "../art/art.js";
import { renderMap, makePlots, fromApiMap } from "../art/compose.js";
import { OwnerStyles } from "./stacks.js";
import { drawChunkLayers, CHUNK } from "./chunks.js";

let art = null;
let ready = null;
let map = null;
let cmap = null;
let plots = null;
let owners = null;
let cities = [];

self.onmessage = async (e) => {
  const m = e.data;
  try {
    if (m.t === "init") {
      ready = (async () => {
        art = await loadArt({ bundle: m.bundle, lenient: true });
        for (const [kind, names] of Object.entries(m.names ?? {})) art.setIndexNames(kind, names);
        owners = new OwnerStyles(art, []);
      })();
      await ready;
      self.postMessage({ t: "ready" });
      return;
    }
    await ready;
    if (m.t === "map") {
      map = m.map;
      cmap = map ? fromApiMap(map, map.w, map.h, !!map.wrap_x) : null;
      plots = cmap ? makePlots(cmap) : null;
    } else if (m.t === "view") {
      owners = new OwnerStyles(art, m.owners);
      cities = m.cities;
    } else if (m.t === "render") {
      if (!plots) return;
      const t0 = performance.now();
      const cols = Math.min(CHUNK, map.w - m.cx * CHUNK);
      const rows = Math.min(CHUNK, map.h - m.cr * CHUNK);
      const canvas = new OffscreenCanvas(cols * 32, rows * 32);
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      art.scale = 1;
      const tx0 = m.cx * CHUNK;
      const ty0 = m.cr * CHUNK;
      renderMap(art, ctx, cmap, { plots, owners, cities, only: "terrain", x0: 0, y0: 0, tx0, ty0, cols, rows });
      drawChunkLayers(art, ctx, m.layer, { map, plots, owners, tx0, ty0, cols, rows });
      const bitmap = canvas.transferToImageBitmap();
      self.postMessage({ t: "chunk", key: m.key, gen: m.gen, bitmap, ms: performance.now() - t0 }, [bitmap]);
    }
  } catch (err) {
    self.postMessage({ t: "error", key: m.key, gen: m.gen, error: String(err?.stack ?? err) });
  }
};
