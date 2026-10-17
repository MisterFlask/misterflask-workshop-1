// The map drawn with the original pixel art (docs/ART.md; the toolkit in web/art, served at
// /art/): the renderer MapView uses unless the flat one was asked for (`?renderer=flat`).
//
// The terrain (ground, rivers, roads, improvements, relief, features, city sprites and their
// light, resources, borders, fog: compose.js `renderMap(.., {only: 'terrain'})`) is drawn into
// chunks of CHUNK x CHUNK plots at the art's own scale (1x) and blown up to the zoom's integer
// scale when the frame is drawn, so every zoom shares one cache, a zoom costs nothing, and
// panning blits. The chunks are drawn by a worker (artworker.js), a few at a time, the ones in
// view first and then those around it, so a frame never waits for the art; a chunk not drawn
// yet shows its chart (flat palette colours) meanwhile. Without a worker (or until it is ready)
// the page draws them itself within a time budget per frame. A chunk is drawn again only when a
// tile it depends on changed (map deltas) or a city near it did (size tier, owner, capital,
// light, walls).
//
// Each frame then draws the vignette (none by day), the units (sprites row by row, the tile
// marks, the city banners and plaques, the stack marks), the path preview with the odds, and the
// minimap. The two smallest zoom steps are the chart (docs/ART.md 3.2: flat colours, owner
// washes, city dots; no sprites).

import { S, tname, info, me } from "./state.js";
import * as T from "./tiles.js";
import { look, UI, heraldry, KIND_TABLES } from "./look.js";
import { renderMap, makePlots, fromApiMap } from "../art/compose.js";
import { TILE } from "../art/art.js";
import { CHUNK, artScale, rowOf, rowChunksOfTiles, changedTiles, stackState, citySignature, updateFacing, unitMoves, OwnerStyles } from "./stacks.js";
import { drawChunkLayers } from "./chunks.js";

/** The terrain cache's budget, in art pixels (least recently used chunks go first). */
const PIXEL_BUDGET = 24e6;
/** Without the worker: the time a frame may spend drawing chunks (at least one per frame), and
 *  an idle slice drawing the chunks around the view. */
const FRAME_MS = 24;
const IDLE_MS = 12;
/** Chunks the worker may hold at once (the page asks for more as they come back, the ones in view
 *  first). */
const IN_FLIGHT = 3;
/** Chart pixels per tile (the chart zoom steps and the placeholders of chunks not drawn yet). */
const CHART = 4;

const LAYERS = T.MAP_LAYERS;

/** The move slide (docs/ART.md 4.5): about 8 tiles a second (125 ms a tile), capped so a longer
 *  net hop is still a quick sweep; a move of more than SLIDE_MAX_TILES jumps (a teleport, or a
 *  long multi-tile move that would read as flying in a straight line across the terrain). */
const SLIDE_MS_PER_TILE = 125;
const SLIDE_MAX_MS = 400;
const SLIDE_MAX_TILES = 4;
const ZERO_OFF = { ox: 0, oy: 0 };

/** The 4x4 ordered-dither threshold the art's light pools use (docs/ART.md 4.4), so the combat
 *  flash dissolves at its edge in crisp pixels rather than a smooth gradient. */
const BAYER4 = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
const bayer4 = (x, y) => (BAYER4[(((y % 4) + 4) % 4) * 4 + (((x % 4) + 4) % 4)] + 0.5) / 16;

export class ArtMap {
  /** `view`: the MapView (camera, viewport, canvas, overlay). */
  constructor(view) {
    this.v = view;
    this.art = look.art;
    this.chunks = new Map();
    /** The chart of the whole map (CHART px a tile, rows in the art's order), painted again in
     *  full (`chartAll`) or by chunk (`chartDirty`, keys "cx,cr") when it is next needed. */
    this.chartC = null;
    this.chartAll = true;
    this.chartDirty = new Set();
    /** A chunk's generation (key "cx,cr" -> n): bumped when it is dropped, so a chunk the worker
     *  drew before the drop is not taken. */
    this.gens = new Map();
    this.inflight = new Map();
    this.pixels = 0;
    this.clock = 0;
    this.map = null;
    this.cmap = null;
    this.plots = null;
    this.ccities = [];
    this.sigs = new Map();
    this.lastSize = new Map();
    this.facing = new Map();
    /** The move slide (docs/ART.md 4.5): the last view's unit positions (id -> {x, y}), the units
     *  now sliding (id -> {dx, dy, t0, dur}), and the turn of the last view processed (slides are
     *  made only for the player's own in-turn moves, not the AI moves seen on turn rollover). */
    this.unitPos = new Map();
    this.slides = new Map();
    this.slideTurn = null;
    this.owners = new OwnerStyles(this.art, []);
    this.ownerSpecs = [];
    this.ownersSig = "";
    this.viewSeen = null;
    this.miniCanvas = null;
    this.idleTimer = null;
    this.vig = null;
    this.fogRgb = new Map();
    this.worker = null;
    this.workerReady = false;
    /** Frame statistics (`window.ffh.mapView.art.stats`): the last frame's milliseconds by
     *  phase, chunks drawn on the page in that frame and by the worker in all, the slowest chunk,
     *  the placeholders shown. */
    this.stats = { frames: 0, frame: 0, phases: {}, chunksDrawn: 0, workerChunks: 0, chunkMs: 0, chunkMax: 0, placeholders: 0, worker: false };
    this.startWorker();
  }

  /** The worker that draws the chunks (none where module workers or OffscreenCanvas are
   *  missing: the page draws them then). */
  startWorker() {
    if (typeof Worker === "undefined" || typeof OffscreenCanvas === "undefined" || !look.bundle) return;
    try {
      this.worker = new Worker(new URL("./artworker.js", import.meta.url), { type: "module" });
    } catch (e) {
      console.warn("art worker:", e);
      this.worker = null;
      return;
    }
    this.worker.onmessage = (e) => this.fromWorker(e.data);
    this.worker.onerror = (e) => {
      console.warn("art worker failed, the page draws the map:", e.message ?? e);
      this.stopWorker();
    };
    const names = {};
    for (const [kind, table] of Object.entries(KIND_TABLES)) {
      const list = look.data?.tables?.[table];
      if (list) names[kind] = list.map((e) => e.type);
    }
    this.worker.postMessage({ t: "init", bundle: look.bundle, names });
    if (this.map) this.worker.postMessage({ t: "map", map: plainMap(this.map) });
  }

  stopWorker() {
    this.worker?.terminate();
    this.worker = null;
    this.workerReady = false;
    this.stats.worker = false;
    this.inflight.clear();
    this.v.request();
  }

  /** The renderer is turned off (useArt(false)): stop the worker and cancel the idle prefetch and
   *  path-march timers so their callbacks don't fire against this detached ArtMap. */
  dispose() {
    if (this.idleTimer != null) { cancelSchedule(this.idleTimer); this.idleTimer = null; }
    if (this.marching != null) { clearTimeout(this.marching); this.marching = null; }
    this.stopWorker();
    this.clear();
  }

  fromWorker(m) {
    if (m.t === "ready") {
      this.workerReady = true;
      this.stats.worker = true;
      this.v.request();
      return;
    }
    if (m.t === "error") {
      console.error("art worker:", m.error);
      if (m.key) {
        this.inflight.delete(m.key);
        // Fall back to drawing this chunk on the page so the view can still settle (and the
        // tile is shown) when the worker fails on one chunk; a page failure is just logged.
        const i = m.key.indexOf("|");
        const [cx, cr] = m.key.slice(i + 1).split(",").map(Number);
        try {
          this.renderChunk(cx, cr, m.key.slice(0, i));
        } catch (e) {
          console.error("art page fallback:", e);
        }
        this.v.request();
      }
      return;
    }
    if (m.t !== "chunk") return;
    const want = this.inflight.get(m.key);
    this.inflight.delete(m.key);
    const xy = m.key.slice(m.key.indexOf("|") + 1);
    // Dropped since it was asked for (a tile or a city near it changed): drawn again later.
    if (want !== m.gen || (this.gens.get(xy) ?? 0) !== m.gen) {
      m.bitmap.close?.();
    } else {
      const old = this.chunks.get(m.key);
      if (old) this.forget(m.key, old);
      this.chunks.set(m.key, { canvas: m.bitmap, used: this.clock });
      this.pixels += m.bitmap.width * m.bitmap.height;
      this.stats.workerChunks++;
      this.stats.chunkMax = Math.max(this.stats.chunkMax, m.ms);
      this.evict();
    }
    this.v.request();
  }

  forget(key, c) {
    this.pixels -= c.canvas.width * c.canvas.height;
    c.canvas.close?.();
    this.chunks.delete(key);
  }

  /** Every cached chunk is dropped. */
  clear() {
    for (const [key, c] of [...this.chunks]) this.forget(key, c);
    for (const k of new Set([...this.gens.keys(), ...[...this.inflight.keys()].map((x) => x.slice(x.indexOf("|") + 1))])) this.gens.set(k, (this.gens.get(k) ?? 0) + 1);
    this.chartAll = true;
    this.chartDirty.clear();
    this.pixels = 0;
    this.miniCanvas = null;
  }

  // ------------------------------------------------------------------------------------------
  // Data

  /** A new map (a full reply or a delta applied): the plots the art draws, and the chunks the
   *  changed tiles touch are dropped (`changed`: the tile indices, "all", or null to compare). */
  setMap(prev, map, changed) {
    this.map = map;
    if (!map) {
      this.cmap = null;
      this.plots = null;
      this.clear();
      this.worker?.postMessage({ t: "map", map: null });
      return;
    }
    this.cmap = fromApiMap(map, map.w, map.h, !!map.wrap_x);
    this.plots = makePlots(this.cmap);
    this.worker?.postMessage({ t: "map", map: plainMap(map) });
    const tiles = changed === "all" || !prev ? "all" : Array.isArray(changed) ? changed : changedTiles(prev, map, LAYERS);
    if (tiles === "all" || prev.w !== map.w || prev.h !== map.h) {
      this.clear();
      this.sigs.clear();
      this.facing.clear();
      this.unitPos.clear();
      this.slides.clear();
      this.slideTurn = null;
      this.lastSize.clear();
      this.viewSeen = null;
    } else {
      this.drop(rowChunksOfTiles(map, tiles));
    }
    this.miniCanvas = null;
  }

  /** Drops the chunks (every layer) and charts with keys in `keys` ("cx,cr"). */
  drop(keys) {
    if (!keys.size) return;
    for (const [key, c] of [...this.chunks]) if (keys.has(key.slice(key.indexOf("|") + 1))) this.forget(key, c);
    for (const k of keys) {
      this.chartDirty.add(k);
      this.gens.set(k, (this.gens.get(k) ?? 0) + 1);
    }
  }

  /** The view changed (units, cities, players): owners, facing, the cities the art draws, and
   *  the chunks around cities that look different now. */
  syncView() {
    const view = S.view;
    if (view === this.viewSeen || !this.map) return;
    this.viewSeen = view;
    const map = this.map;
    // Heraldry: you, then the players you met, in slot order; others fly the lozenge.
    const players = [];
    const m = me();
    const civ = (i) => (i === null || i === undefined ? null : tname("CivilizationType", i));
    if (m) players.push({ id: m.player, civ: civ(m.civ), barbarian: false });
    for (const p of [...S.players.values()].sort((a, b) => a.id - b.id)) players.push({ id: p.id, civ: civ(p.civ), barbarian: !!p.barbarian });
    const specs = heraldry(players, S.status?.game?.journal ?? S.status?.game?.dir ?? "memory");
    const ownersSig = JSON.stringify(specs);
    let viewChanged = false;
    if (ownersSig !== this.ownersSig) {
      // Borders and city banners change with the owners: everything is drawn again.
      if (this.ownersSig) this.clear();
      this.ownersSig = ownersSig;
      this.ownerSpecs = specs;
      this.owners = new OwnerStyles(this.art, specs);
      viewChanged = true;
    }
    // Facing from the moves since the last view.
    this.facing = updateFacing(this.facing, view?.units ?? [], map.w, !!map.wrap_x);
    // Move slide (docs/ART.md 4.5): the units that changed tile glide from their old tile to the
    // new one. Only the player's own in-turn moves (same turn as the last view) slide — on turn
    // rollover every AI move would otherwise slide at once; and only short hops, so a unit never
    // flies in a straight line across the map. prefers-reduced-motion jumps.
    const { positions, moved } = unitMoves(this.unitPos, view?.units ?? [], map.w, !!map.wrap_x);
    this.unitPos = positions;
    const turn = S.status?.turn ?? S.view?.turn ?? 0;
    // Slide only the player's OWN moves made while they have control, and only within the same
    // turn: an AI move seen on turn rollover, or when a decision refetches the view mid-AI-turn
    // (same turn number, phase "decision"), must not slide a swarm of foreign units.
    const mep = me()?.player;
    if (turn === this.slideTurn && S.status?.phase === "human" && mep !== null && mep !== undefined && !this.v.reduceMotion()) {
      const own = new Set((view?.units ?? []).filter((u) => u.owner === mep).map((u) => u.id));
      for (const { id, dx, dy } of moved) {
        if (!own.has(id)) continue;
        const steps = Math.max(Math.abs(dx), Math.abs(dy));
        if (steps > SLIDE_MAX_TILES) continue; // a teleport or a long multi-tile move: jump
        this.slides.set(id, { dx, dy, t0: performance.now(), dur: Math.min(SLIDE_MAX_MS, steps * SLIDE_MS_PER_TILE) });
      }
    }
    this.slideTurn = turn;
    // Cities as the art draws them; a city looks like the last size seen while it is fogged.
    const cities = [];
    const sigs = new Map();
    for (const c of S.cities.values()) {
      if (c.size !== null && c.size !== undefined) this.lastSize.set(c.id, c.size);
      const own = c.owner === m?.player;
      const cc = {
        id: c.id,
        x: c.x,
        y: rowOf(map, c.y),
        owner: c.owner,
        size: this.lastSize.get(c.id) ?? 1,
        name: c.name,
        capital: !!c.capital,
        own,
        turns: own && c.production?.turns !== null && c.production?.turns !== undefined ? c.production.turns : null,
        disorder: !!c.disorder,
        walls: !!c.walls,
        visible: !!c.visible,
      };
      cities.push(cc);
      sigs.set(c.id, `${c.x},${c.y}|${citySignature(cc)}`);
    }
    const dirty = [];
    for (const [id, sig] of sigs) if (this.sigs.get(id) !== sig) dirty.push(sig);
    for (const [id, sig] of this.sigs) if (!sigs.has(id)) dirty.push(sig);
    if (dirty.length) {
      const tiles = dirty.map((s) => {
        const [x, y] = s.slice(0, s.indexOf("|")).split(",").map(Number);
        return y * map.w + x;
      });
      this.drop(rowChunksOfTiles(map, tiles));
      this.miniCanvas = null;
      viewChanged = true;
    }
    this.sigs = sigs;
    this.ccities = cities;
    if (viewChanged) this.worker?.postMessage({ t: "view", owners: this.ownerSpecs, cities: cities.map(terrainCity) });
  }

  // ------------------------------------------------------------------------------------------
  // Geometry

  /** The frame's geometry in device pixels: scale `s` (device px per art px; 0 at a chart
   *  step), tile size `tp`, the origin of column 0 / row 0, the canvas size, and the visible
   *  unwrapped columns and rows. */
  frame() {
    const v = this.v;
    const dpr = v.dpr;
    const ts = v.cam.ts;
    const s = artScale(ts, dpr);
    const tp = s ? 32 * s : ts * dpr;
    const W = v.canvas.width;
    const H = v.canvas.height;
    const map = this.map;
    const ox = Math.round(dpr * (v.vw / 2 - v.cam.cx * ts));
    const oy = Math.round(dpr * (v.vh / 2 - (map.h - v.cam.cy) * ts));
    const c0 = Math.floor(-ox / tp);
    const c1 = Math.floor((W - 1 - ox) / tp);
    const r0 = Math.max(0, Math.floor(-oy / tp));
    const r1 = Math.min(map.h - 1, Math.floor((H - 1 - oy) / tp));
    return { s, tp, ox, oy, W, H, c0: map.wrap_x ? c0 : Math.max(0, c0), c1: map.wrap_x ? c1 : Math.min(map.w - 1, c1), r0, r1, dpr };
  }

  /** The unwrapped columns of map column `x` within [c0 - m, c1 + m]. */
  copies(f, x, m = 1) {
    const w = this.map.w;
    if (!this.map.wrap_x) return x >= f.c0 - m && x <= f.c1 + m ? [x] : [];
    const out = [];
    for (let k = Math.floor((f.c0 - m - x) / w); x + k * w <= f.c1 + m; k++) if (x + k * w >= f.c0 - m) out.push(x + k * w);
    return out;
  }

  /** The chunks in view, by unwrapped chunk columns `ux` from `cx0` to `cx1` (a margin of
   *  chunks around the view with `margin`): `{cx, cr, cols, rows, X, Y}` (X, Y: device
   *  position). */
  visibleChunks(f, margin = 0) {
    const map = this.map;
    const out = [];
    const ncx = Math.ceil(map.w / CHUNK);
    const ncr = Math.ceil(map.h / CHUNK);
    const k0 = map.wrap_x ? Math.floor((f.c0 - margin * CHUNK) / map.w) : 0;
    const k1 = map.wrap_x ? Math.floor((f.c1 + margin * CHUNK) / map.w) : 0;
    const cr0 = Math.max(0, Math.floor(f.r0 / CHUNK) - margin);
    const cr1 = Math.min(ncr - 1, Math.floor(f.r1 / CHUNK) + margin);
    for (let cr = cr0; cr <= cr1; cr++) {
      const rows = Math.min(CHUNK, map.h - cr * CHUNK);
      for (let k = k0; k <= k1; k++) {
        for (let cx = 0; cx < ncx; cx++) {
          const x0 = cx * CHUNK + k * map.w;
          const cols = Math.min(CHUNK, map.w - cx * CHUNK);
          if (x0 + cols - 1 < f.c0 - margin * CHUNK || x0 > f.c1 + margin * CHUNK) continue;
          out.push({ cx, cr, cols, rows, X: f.ox + x0 * f.tp, Y: f.oy + cr * CHUNK * f.tp });
        }
      }
    }
    return out;
  }

  // ------------------------------------------------------------------------------------------
  // Drawing

  draw() {
    const v = this.v;
    const g = v.ctx;
    const t0 = performance.now();
    const ph = {};
    let tl = t0;
    const lap = (name) => {
      const t = performance.now();
      ph[name] = Math.round((t - tl) * 10) / 10;
      tl = t;
    };
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.imageSmoothingEnabled = false;
    g.fillStyle = this.art.css("0");
    g.fillRect(0, 0, v.canvas.width, v.canvas.height);
    if (!this.map || !this.plots) return;
    this.syncView();
    this.clock++;
    const f = this.frame();
    const layer = v.layer;
    const art = this.art;
    art.time = performance.now();
    let missing = 0;
    this.stats.chunksDrawn = 0;
    this.stats.placeholders = 0;
    const deadline = t0 + FRAME_MS;
    const worker = this.worker && this.workerReady;
    let chart = null;
    const drawChart = (c) => {
      chart ??= this.chart();
      g.drawImage(chart, c.cx * CHUNK * CHART, c.cr * CHUNK * CHART, c.cols * CHART, c.rows * CHART, c.X, c.Y, c.cols * f.tp, c.rows * f.tp);
    };
    for (const c of this.visibleChunks(f)) {
      if (!f.s) {
        drawChart(c);
        continue;
      }
      const key = `${layer}|${c.cx},${c.cr}`;
      let ch = this.chunks.get(key);
      if (!ch && !worker && !this.worker && (this.stats.chunksDrawn === 0 || performance.now() < deadline)) ch = this.renderChunk(c.cx, c.cr, layer);
      if (ch) {
        ch.used = this.clock;
        g.drawImage(ch.canvas, 0, 0, c.cols * TILE, c.rows * TILE, c.X, c.Y, c.cols * f.tp, c.rows * f.tp);
      } else {
        missing++;
        this.stats.placeholders++;
        if (worker) this.ask(c, layer);
        drawChart(c);
      }
    }
    this.evict();
    lap("terrain");
    if (f.s) {
      art.scale = f.s;
      this.vignette(g, f);
      lap("vignette");
      this.drawUnits(g, f);
      lap("units");
      this.drawOverlays(g, f);
      this.drawFlashes(g, f);
      lap("overlays");
    } else {
      this.drawChartUnits(g, f);
      this.drawChartOverlays(g, f);
      lap("units");
    }
    this.stats.frames++;
    this.stats.frame = Math.round((performance.now() - t0) * 10) / 10;
    this.stats.phases = ph;
    if (missing && !worker) v.request();
    else if (!missing && f.s) this.prefetch(f);
    // Keep frames coming while a unit is sliding. Expire by time here (not only in drawUnits,
    // which runs just for in-view stacks) so a unit that slid off-screen cannot leave a slide
    // behind and spin this loop forever.
    if (this.slides.size) {
      const now = performance.now();
      for (const [id, s] of this.slides) if (now - s.t0 >= s.dur) this.slides.delete(id);
      if (f.s && this.slides.size) v.request();
    }
    // The path's dots march (docs/ART.md 4.5: 1 px every 150 ms) while a path is shown.
    if (f.s && v.overlay.path?.ok && !this.marching) {
      this.marching = setTimeout(() => {
        this.marching = null;
        if (v.overlay.path?.ok) v.request();
      }, 150);
    }
  }

  /** Asks the worker for chunk `c` of `layer` (at most IN_FLIGHT at once). */
  ask(c, layer) {
    const xy = `${c.cx},${c.cr}`;
    const key = `${layer}|${xy}`;
    if (this.inflight.has(key) || this.inflight.size >= IN_FLIGHT) return false;
    const gen = this.gens.get(xy) ?? 0;
    this.inflight.set(key, gen);
    this.worker.postMessage({ t: "render", key, cx: c.cx, cr: c.cr, layer, gen });
    return true;
  }

  /** Once the view is complete: the chunks around it (one chunk's margin), by the worker, or by
   *  the page while it is idle a few milliseconds at a time, so that panning finds them ready. */
  prefetch(f) {
    const layer = this.v.layer;
    const want = this.visibleChunks(f, 1).filter((c) => !this.chunks.has(`${layer}|${c.cx},${c.cr}`));
    if (!want.length) return;
    if (this.worker) {
      if (this.workerReady) for (const c of want) if (!this.ask(c, layer) && this.inflight.size >= IN_FLIGHT) break;
      return;
    }
    if (this.idleTimer) return;
    const map = this.map;
    const step = (deadline) => {
      this.idleTimer = null;
      if (this.map !== map || this.v.layer !== layer) return;
      const until = performance.now() + Math.min(IDLE_MS, deadline?.timeRemaining?.() ?? IDLE_MS);
      while (want.length && performance.now() < until) {
        const c = want.shift();
        if (!this.chunks.has(`${layer}|${c.cx},${c.cr}`)) this.renderChunk(c.cx, c.cr, layer).used = this.clock;
      }
      if (want.length) this.idleTimer = schedule(step);
    };
    this.idleTimer = schedule(step);
  }

  /** Whether the view is drawn (no chunk in view or around it waits; for the tests). */
  settled() {
    if (!this.map) return true;
    const f = this.frame();
    if (!f.s) return true;
    const layer = this.v.layer;
    return this.visibleChunks(f).every((c) => this.chunks.has(`${layer}|${c.cx},${c.cr}`)) && !this.idleTimer && this.inflight.size === 0;
  }

  /** Draws chunk (cx, cr) of `layer` at 1x into the cache on the page (without the worker). */
  renderChunk(cx, cr, layer) {
    const t0 = performance.now();
    const map = this.map;
    const cols = Math.min(CHUNK, map.w - cx * CHUNK);
    const rows = Math.min(CHUNK, map.h - cr * CHUNK);
    const canvas = document.createElement("canvas");
    canvas.width = cols * TILE;
    canvas.height = rows * TILE;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    ctx.imageSmoothingEnabled = false;
    const art = this.art;
    const old = art.scale;
    art.scale = 1;
    try {
      const tx0 = cx * CHUNK;
      const ty0 = cr * CHUNK;
      renderMap(art, ctx, this.cmap, { plots: this.plots, owners: this.owners, cities: this.ccities, only: "terrain", x0: 0, y0: 0, tx0, ty0, cols, rows });
      drawChunkLayers(art, ctx, layer, { map, plots: this.plots, owners: this.owners, tx0, ty0, cols, rows });
    } catch (e) {
      console.error("art: chunk", cx, cr, e);
    } finally {
      art.scale = old;
    }
    const c = { canvas, used: this.clock };
    this.chunks.set(`${layer}|${cx},${cr}`, c);
    this.pixels += canvas.width * canvas.height;
    const ms = performance.now() - t0;
    this.stats.chunksDrawn++;
    this.stats.chunkMs += ms;
    this.stats.chunkMax = Math.max(this.stats.chunkMax, ms);
    return c;
  }

  evict() {
    if (this.pixels <= PIXEL_BUDGET) return;
    const old = [...this.chunks].filter(([, c]) => c.used < this.clock).sort((a, b) => a[1].used - b[1].used);
    for (const [key, c] of old) {
      if (this.pixels <= PIXEL_BUDGET * 0.75) break;
      this.forget(key, c);
    }
  }

  /** The viewport vignette (docs/ART.md 8.3): beyond 75% of the half-diagonal, pixels pass
   *  through FOG with a Bayer threshold rising to full at the corners. Only the strips at the
   *  sides that hold such pixels are read back. Off when the ground pass says so (by day). */
  vignette(g, f) {
    if (this.art.vignetteOn === false) return;
    const s = f.s;
    const Wa = Math.floor(f.W / s);
    const Ha = Math.floor(f.H / s);
    if (!this.vig || this.vig.Wa !== Wa || this.vig.Ha !== Ha) this.vig = vignetteMask(Wa, Ha);
    const { spans, strip } = this.vig;
    if (strip <= 0) return;
    const art = this.art;
    const fogRgb = (r, gg, b) => {
      const key = (r << 16) | (gg << 8) | b;
      let v = this.fogRgb.get(key);
      if (v === undefined) {
        const n = art.tables.FOG[art._nearest(key)];
        v = (art.rgb[n * 3] << 16) | (art.rgb[n * 3 + 1] << 8) | art.rgb[n * 3 + 2];
        this.fogRgb.set(key, v);
      }
      return v;
    };
    for (const side of [0, 1]) {
      const X = side ? (Wa - strip) * s : 0;
      const img = g.getImageData(X, 0, strip * s, Ha * s);
      const d = img.data;
      const rowLen = strip * s * 4;
      for (let j = 0; j < Ha; j++) {
        const list = spans[j][side];
        for (let n = 0; n < list.length; n++) {
          const i = list[n] - (side ? Wa - strip : 0);
          const p0 = j * s * rowLen + i * s * 4;
          const v = fogRgb(d[p0], d[p0 + 1], d[p0 + 2]);
          const r = v >> 16;
          const gg = (v >> 8) & 255;
          const b = v & 255;
          for (let yy = 0; yy < s; yy++) {
            let p = p0 + yy * rowLen;
            for (let xx = 0; xx < s; xx++, p += 4) {
              d[p] = r;
              d[p + 1] = gg;
              d[p + 2] = b;
            }
          }
        }
      }
      g.putImageData(img, X, 0);
    }
  }

  /** The stacks of the view, with the facts the art draws (head unit and its state). */
  stacksInView(f) {
    const mine = me()?.player;
    const sel = this.v.overlay.units ?? [];
    const ctx = {
      me: mine,
      selected: sel,
      activity: (i) => tname("ActivityType", i),
      unitInfo: (t) => {
        const name = tname("UnitType", t);
        const e = name ? this.art.mapper.entry("unit", name) : null;
        return { hero: !!e?.hero, combat: e?.combat ?? "NONE", strength: info("UnitType", t)?.combat ?? e?.strength ?? 0 };
      },
      automated: (g) => {
        const grp = S.groups.get(g);
        return !!grp && grp.automated !== null && grp.automated !== undefined;
      },
      facing: (id) => this.facing.get(id)?.facing ?? "right",
    };
    const out = [];
    for (const units of S.unitsAt.values()) {
      const u0 = units[0];
      const row = rowOf(this.map, u0.y);
      if (row < f.r0 - 1 || row > f.r1 + 1) continue;
      const cols = this.copies(f, u0.x);
      if (!cols.length) continue;
      const st = stackState(units, ctx);
      if (!st) continue;
      for (const ux of cols) out.push({ ux, row, x: u0.x, y: u0.y, head: st.head, state: st.state });
    }
    out.sort((a, b) => a.row - b.row || a.ux - b.ux);
    return out;
  }

  /** The ease-out pixel offset of unit `id`'s slide (from its new tile back toward the old one),
   *  snapped to whole art pixels; ZERO_OFF when it is not sliding. Pure — expiry is handled by the
   *  time prune in draw() — so the sprite (drawUnits) and the selection mark (drawTileMarks) read
   *  the same value and stay locked together. */
  slideOffset(id, f, now) {
    const s = this.slides.get(id);
    if (!s) return ZERO_OFF;
    const t = (now - s.t0) / s.dur;
    if (t >= 1) return ZERO_OFF;
    const e = 1 - Math.pow(1 - t, 3);
    const snap = (v) => Math.round(v / f.s) * f.s;
    return { ox: snap(-s.dx * f.tp * (1 - e)), oy: snap(s.dy * f.tp * (1 - e)) };
  }

  /** Units (docs/ART.md 3.1, compose.js's unit layers): the sprites row by row, the tile marks
   *  (drawTileMarks), the city banners and plaques, then the stacks' count chips, health and marks. */
  drawUnits(g, f) {
    const art = this.art;
    const stacks = this.stacksInView(f);
    const cityAt = new Set(this.ccities.map((c) => `${c.x},${c.y}`));
    const owner = (p) => this.owners.get(p);
    const zoom = f.s / f.dpr;
    // The slide offset of each moving stack (docs/ART.md 4.5): from its old tile to its new one,
    // used for both the sprite and the overlay pass so the count, health and marks ride with
    // the figure (and, in drawTileMarks, the selection mark rides with it too).
    const now = performance.now();
    const offsets = new Map();
    const offOf = (id) => {
      if (!offsets.has(id)) offsets.set(id, this.slideOffset(id, f, now));
      return offsets.get(id);
    };
    for (const st of stacks) {
      const off = offOf(st.head.id);
      const inCity = cityAt.has(`${st.x},${st.row}`);
      // A garrison would hide its city (units fill the tile): in a city the figure shows only while
      // it is selected or sliding; the count chip (overlay pass) still marks the garrison.
      if (inCity && !st.state.selected && !off.ox && !off.oy) continue;
      const X = f.ox + st.ux * f.tp + off.ox;
      const Y = f.oy + st.row * f.tp + off.oy;
      art.drawUnit(g, X, Y, st.head.type, owner(st.head.owner), { ...st.state, inCity, layer: "sprite" });
    }
    this.drawTileMarks(g, f);
    for (const c of this.ccities) {
      if (c.y < f.r0 - 1 || c.y > f.r1 + 1) continue;
      const p = this.plots.get(c.x, c.y);
      if (!p || !p.vis) continue;
      for (const ux of this.copies(f, c.x)) {
        art.drawCity(g, f.ox + ux * f.tp, f.oy + c.y * f.tp, { ...c, visible: p.vis === 2, layer: "overlay", plaque: zoom >= 2 ? "full" : "chip" }, owner(c.owner));
      }
    }
    for (const st of stacks) {
      const off = offOf(st.head.id);
      const X = f.ox + st.ux * f.tp + off.ox;
      const Y = f.oy + st.row * f.tp + off.oy;
      art.drawUnit(g, X, Y, st.head.type, owner(st.head.owner), { ...st.state, inCity: cityAt.has(`${st.x},${st.row}`), layer: "overlay" });
    }
  }

  /** Call `fn(X, Y, ux, row)` at each on-screen copy of map tile (x, y) (device px of its top-left). */
  tileAt(f, x, y, fn) {
    const row = rowOf(this.map, y);
    if (row < f.r0 - 1 || row > f.r1 + 1) return;
    for (const ux of this.copies(f, x)) fn(f.ox + ux * f.tp, f.oy + row * f.tp, ux, row);
  }

  /** The tile marks (targets, selection, hover). drawUnits draws them between the sprites and the
   *  city plaques, banners and stack chips, so a bracket never writes over a town's name. */
  drawTileMarks(g, f) {
    const art = this.art;
    const o = this.v.overlay;
    const at = (x, y, fn) => this.tileAt(f, x, y, fn);
    if (Array.isArray(o.targets)) for (const [x, y] of o.targets) at(x, y, (X, Y, ux, row) => UI.drawTileMark(art, g, X, Y, "target", { mx: ux, my: row }));
    // The selection mark rides with the selected unit while it slides, so the bracket does not sit
    // at the destination waiting for the sprite to catch up.
    if (o.sel) {
      const selId = (o.units ?? [])[0];
      const off = selId !== undefined && selId !== null ? this.slideOffset(selId, f, performance.now()) : ZERO_OFF;
      at(o.sel.x, o.sel.y, (X, Y) => UI.drawTileMark(art, g, X + off.ox, Y + off.oy, "select"));
    }
    if (o.hover && !(o.sel && o.sel.x === o.hover.x && o.sel.y === o.hover.y)) at(o.hover.x, o.hover.y, (X, Y) => UI.drawTileMark(art, g, X, Y, o.targets ? "route" : "hover"));
  }

  /** The waypoints and the path preview (over everything, after the tile marks and the units). */
  drawOverlays(g, f) {
    const art = this.art;
    const o = this.v.overlay;
    const at = (x, y, fn) => this.tileAt(f, x, y, fn);
    if (Array.isArray(o.waypoints)) {
      o.waypoints.forEach(([x, y], i) => at(x, y, (X, Y) => {
        const s = f.s;
        art.drawSprite(g, X + 16 * s, Y + 18 * s, "ui/path_turn");
        const text = String(i + 1);
        const tw = art.measureText(text, "tally");
        art.drawText(g, X + (16 - Math.floor(tw / 2)) * s, Y + 20 * s, text, { font: "tally", color: "y" });
      }));
    }
    const path = o.path;
    if (path?.ok && path.nodes?.length > 1) {
      const steps = this.pathSteps(f, path.nodes);
      const kind = path.attack ? "attack" : o.routeTo ? "route" : "move";
      const odds = path.attack && o.odds !== null && o.odds !== undefined ? o.odds / 10 : null;
      UI.drawPath(art, g, f.ox, f.oy, steps, { kind, odds, time: art.time });
    }
  }

  /** The combat flashes (docs/ART.md 4.5), in device pixels; both renderers read `v.flashes`,
   *  the flat one draws them in `MapView.drawFlashes`. A brief crimson glow on the struck tile, a
   *  crimson puff rising as a unit is lost. */
  drawFlashes(g, f) {
    const flashes = this.v.flashes;
    if (!flashes || !flashes.length || !f.s) return;
    const s = f.s; // device px per art px
    const now = performance.now();
    for (const fl of flashes) {
      const row = rowOf(this.map, fl.y);
      if (row < f.r0 - 1 || row > f.r1 + 1) continue;
      const t = Math.min(1, (now - fl.t0) / fl.dur);
      const a = (1 - t) * (fl.kind === "death" ? 0.5 : 0.58);
      if (a <= 0.01) continue;
      // Radius in whole art pixels; a death pool is smaller and rises as it fades.
      const R = Math.round(fl.kind === "death" ? 9 + t * 5 : 14);
      const rise = fl.kind === "death" ? Math.round(t * 8) : 0;
      g.fillStyle = `rgba(119,37,43,${a})`; // crimson #77252b (ART.md's war/damage hue)
      for (const ux of this.copies(f, fl.x)) {
        // Centre in art-pixel coordinates (16,16 is the tile centre of a 32 px tile).
        const aox = Math.round((f.ox + ux * f.tp) / s) + 16;
        const aoy = Math.round((f.oy + row * f.tp) / s) + 16 - rise;
        g.beginPath();
        for (let dy = -R; dy <= R; dy++) {
          for (let dx = -R; dx <= R; dx++) {
            const d = Math.sqrt((dx / R) ** 2 + ((dy * 1.4) / R) ** 2); // elliptical, as the light pools
            if (d < 1 && 1 - d > bayer4(aox + dx, aoy + dy)) g.rect((aox + dx) * s, (aoy + dy) * s, s, s);
          }
        }
        g.fill();
      }
    }
  }

  /** Path nodes in tile coordinates of the frame (unwrapped from the selection's copy nearest
   *  the centre, rows from the north). */
  pathSteps(f, nodes) {
    const map = this.map;
    const n0 = nodes[0];
    let ux = n0.x;
    if (map.wrap_x) {
      const centre = (f.W / 2 - f.ox) / f.tp;
      ux = n0.x + Math.round((centre - (n0.x + 0.5)) / map.w) * map.w;
    }
    let prev = n0.x;
    const out = [{ x: ux, y: rowOf(map, n0.y), turn: n0.turn }];
    for (const n of nodes.slice(1)) {
      let dx = n.x - prev;
      if (map.wrap_x) {
        if (dx > map.w / 2) dx -= map.w;
        if (dx < -map.w / 2) dx += map.w;
      }
      ux += dx;
      prev = n.x;
      out.push({ x: ux, y: rowOf(map, n.y), turn: n.turn });
    }
    return out;
  }

  // ------------------------------------------------------------------------------------------
  // The chart (the two smallest zoom steps, the minimap, the placeholders)

  /** The chart colour (a palette index) of a plot (docs/ART.md 8.4, terrain/manifest.txt
   *  `mini`): the feature's, else the peak's or hill's, else the terrain's; null unrevealed. */
  miniIndex(p) {
    if (!p || !p.vis) return null;
    const art = this.art;
    const key = (kind, type) => art.resolve(kind, type)?.opts?.mini;
    const t = art.typeName("terrain", p.terrain);
    const pt = p.plotType;
    let k = p.feature !== null && p.feature !== undefined ? key("feature", p.feature) : null;
    if (!k && pt === 0) k = key("peak", t);
    if (!k && pt === 1) k = key("hill", t);
    if (!k) k = key("terrain", t) ?? "4";
    try {
      return art.K(k);
    } catch {
      return art.K("4");
    }
  }

  /** The map as a chart (one canvas, CHART px a tile, rows in the art's order): terrain
   *  colours, owned land washed with a 50% checker of the owner's field, cities as vellum dots,
   *  fog through FOG, unrevealed void. Painted in full after a new map or new owners, else only
   *  the chunks dropped since (`drop`). */
  chart() {
    const map = this.map;
    const w = map.w * CHART;
    const hgt = map.h * CHART;
    if (!this.chartC || this.chartC.width !== w || this.chartC.height !== hgt) {
      this.chartC = document.createElement("canvas");
      this.chartC.width = w;
      this.chartC.height = hgt;
      this.chartAll = true;
    }
    const g = this.chartC.getContext("2d");
    if (this.chartAll) {
      this.paintChart(g, 0, 0, map.w, map.h);
      this.chartAll = false;
    } else {
      for (const k of this.chartDirty) {
        const [cx, cr] = k.split(",").map(Number);
        const x0 = cx * CHUNK;
        const y0 = cr * CHUNK;
        if (x0 < map.w && y0 < map.h) this.paintChart(g, x0, y0, Math.min(CHUNK, map.w - x0), Math.min(CHUNK, map.h - y0));
      }
    }
    this.chartDirty.clear();
    return this.chartC;
  }

  /** Paints the chart of columns x0.. and art rows y0.. (`cols` x `rows` plots). */
  paintChart(g, x0, y0, cols, rows) {
    const art = this.art;
    const FOG = art.tables.FOG;
    const img = new ImageData(cols * CHART, rows * CHART);
    const d = img.data;
    const v8 = art.K("8");
    const v0 = art.K("0");
    for (let j = 0; j < rows; j++) {
      for (let i = 0; i < cols; i++) {
        const p = this.plots.get(x0 + i, y0 + j);
        const base = this.miniIndex(p);
        const o = p && p.owner !== null && p.owner !== undefined && p.plotType !== 3 ? this.owners.get(p.owner) : null;
        for (let b = 0; b < CHART; b++) {
          for (let a = 0; a < CHART; a++) {
            let k = base === null ? v0 : base;
            if (base !== null && o && ((a + b) & 1) === 0) k = o.tincture.field;
            if (base !== null && p.city && a >= 1 && a <= 2 && b >= 1 && b <= 2) k = v8;
            if (base !== null && p.vis === 1) k = FOG[k];
            const q = ((j * CHART + b) * cols * CHART + i * CHART + a) * 4;
            d[q] = art.rgb[k * 3];
            d[q + 1] = art.rgb[k * 3 + 1];
            d[q + 2] = art.rgb[k * 3 + 2];
            d[q + 3] = 255;
          }
        }
      }
    }
    g.putImageData(img, x0 * CHART, y0 * CHART);
  }

  /** Units at a chart step: a square of the owner's field in a void frame (vellum when
   *  selected). */
  drawChartUnits(g, f) {
    const art = this.art;
    const sel = this.v.overlay.units ?? [];
    const k = Math.max(2, Math.round(f.tp * 0.4));
    for (const st of this.stacksInView(f)) {
      const X = f.ox + st.ux * f.tp + Math.round((f.tp - k) / 2);
      const Y = f.oy + st.row * f.tp + Math.round((f.tp - k) / 2);
      const o = this.owners.get(st.head.owner);
      g.fillStyle = art.css(sel.includes(st.head.id) ? "8" : "0");
      g.fillRect(X - 1, Y - 1, k + 2, k + 2);
      g.fillStyle = o ? art.palette.colors[o.tincture.field].hex : art.css("7");
      g.fillRect(X, Y, k, k);
    }
  }

  drawChartOverlays(g, f) {
    const art = this.art;
    const o = this.v.overlay;
    const box = (x, y, key) => {
      const row = rowOf(this.map, y);
      for (const ux of this.copies(f, x)) {
        g.strokeStyle = art.css(key);
        g.lineWidth = 1;
        g.strokeRect(f.ox + ux * f.tp + 0.5, f.oy + row * f.tp + 0.5, f.tp - 1, f.tp - 1);
      }
    };
    if (Array.isArray(o.targets)) for (const [x, y] of o.targets) box(x, y, "P");
    if (o.sel) box(o.sel.x, o.sel.y, "8");
    if (o.hover) box(o.hover.x, o.hover.y, "6");
    const path = o.path;
    if (path?.ok && path.nodes?.length > 1) {
      const steps = this.pathSteps(f, path.nodes);
      g.strokeStyle = art.css(path.attack ? "R" : "8");
      g.lineWidth = Math.max(1, f.dpr);
      g.setLineDash([2 * f.dpr, 2 * f.dpr]);
      g.beginPath();
      steps.forEach((st, i) => {
        const X = f.ox + (st.x + 0.5) * f.tp;
        const Y = f.oy + (st.y + 0.5) * f.tp;
        if (i) g.lineTo(X, Y);
        else g.moveTo(X, Y);
      });
      g.stroke();
      g.setLineDash([]);
    }
  }

  /** The minimap's picture (docs/ART.md 8.4): a pixel a tile, terrain colours, owned land in the
   *  owner's field colour, cities vellum, fog through FOG, unrevealed void. */
  minimap() {
    if (this.miniCanvas) return this.miniCanvas;
    if (!this.plots) return null;
    this.syncView();
    const art = this.art;
    const map = this.map;
    const FOG = art.tables.FOG;
    const img = new ImageData(map.w, map.h);
    const d = img.data;
    for (let row = 0; row < map.h; row++) {
      for (let x = 0; x < map.w; x++) {
        const p = this.plots.get(x, row);
        let k = this.miniIndex(p);
        if (k === null) k = art.K("0");
        else {
          const o = p.owner !== null && p.owner !== undefined && p.plotType !== 3 ? this.owners.get(p.owner) : null;
          if (o) k = o.tincture.field;
          if (p.city) k = art.K("8");
          if (p.vis === 1) k = FOG[k];
        }
        const q = (row * map.w + x) * 4;
        d[q] = art.rgb[k * 3];
        d[q + 1] = art.rgb[k * 3 + 1];
        d[q + 2] = art.rgb[k * 3 + 2];
        d[q + 3] = 255;
      }
    }
    const c = document.createElement("canvas");
    c.width = map.w;
    c.height = map.h;
    c.getContext("2d").putImageData(img, 0, 0);
    this.miniCanvas = c;
    return c;
  }
}

/** What the terrain cache draws of a city (the worker's copy). */
function terrainCity(c) {
  return { x: c.x, y: c.y, owner: c.owner, size: c.size, capital: c.capital, own: c.own, disorder: c.disorder, walls: c.walls, visible: c.visible, name: c.name };
}

/** The map's arrays and size for the worker (the reply's other fields stay on the page). */
function plainMap(map) {
  const out = { w: map.w, h: map.h, wrap_x: !!map.wrap_x, wrap_y: !!map.wrap_y };
  for (const k of LAYERS) out[k] = map[k];
  return out;
}

/** The vignette's pixels for a view of Wa x Ha art px: per row, the columns of the left and the
 *  right strip that pass through FOG (beyond 75% of the half-diagonal, against a 4x4 Bayer
 *  threshold rising to full at the corners); `strip`: the strips' width. */
export function vignetteMask(Wa, Ha) {
  const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map((v) => (v + 0.5) / 16);
  const cx = Wa / 2;
  const cy = Ha / 2;
  const R = Math.hypot(cx, cy);
  const spans = [];
  let strip = 0;
  for (let j = 0; j < Ha; j++) {
    const left = [];
    const right = [];
    for (let i = 0; i < Wa; i++) {
      const dd = Math.hypot(i - cx, j - cy) / R;
      if (dd <= 0.75) continue;
      const t = (dd - 0.75) / 0.25;
      if (t > BAYER[(j & 3) * 4 + (i & 3)]) (i < cx ? left : right).push(i);
    }
    if (left.length) strip = Math.max(strip, left[left.length - 1] + 1);
    if (right.length) strip = Math.max(strip, Wa - right[0]);
    spans.push([left, right]);
  }
  // A view so wide that the strips meet: one strip each, half the view.
  strip = Math.min(strip, Math.ceil(Wa / 2));
  for (const [l, r] of spans) {
    for (let n = l.length - 1; n >= 0; n--) if (l[n] >= strip) l.splice(n, 1);
    for (let n = r.length - 1; n >= 0; n--) if (r[n] < Wa - strip) r.splice(n, 1);
  }
  return { Wa, Ha, spans, strip };
}

function schedule(fn) {
  return typeof requestIdleCallback === "function" ? requestIdleCallback(fn, { timeout: 200 }) : setTimeout(fn, 30);
}

function cancelSchedule(id) {
  if (typeof cancelIdleCallback === "function") cancelIdleCallback(id);
  else clearTimeout(id);
}
