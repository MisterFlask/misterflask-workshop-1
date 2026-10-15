// The map drawn with the original pixel art (docs/ART.md; the toolkit in web/art, served at
// /art/): the renderer MapView uses unless the flat one was asked for (`?renderer=flat`).
//
// The terrain (ground, rivers, roads, improvements, relief, features, city sprites and their
// light, resources, borders, fog: compose.js `renderMap(.., {only: 'terrain'})`) is drawn into
// offscreen chunks of CHUNK x CHUNK plots at the art's own scale (1x) and blown up to the zoom's
// integer scale when the frame is drawn, so every zoom shares one cache, a zoom costs nothing,
// and panning blits. A chunk is redrawn only when a tile it depends on changed (map deltas) or a
// city near it did (size tier, owner, capital, light, walls). Chunks not drawn yet are drawn a
// few per frame within a time budget (FRAME_MS) and shown meanwhile as their chart (flat
// palette colours); once the view is complete the chunks around it are drawn while the page is
// idle. Each frame then draws the vignette, the units (sprites row by row, the city banners and
// plaques, the stack standards), the tile marks, the path preview with the odds, and the
// minimap. The two smallest zoom steps are the chart (docs/ART.md 3.2: flat colours, owner
// washes, city dots; no sprites).

import { S, tname, info, me, isBarbarian } from "./state.js";
import * as T from "./tiles.js";
import { look, UI, ownerStyles, unknownOwner } from "./look.js";
import { renderMap, makePlots, fromApiMap } from "../art/compose.js";
import { TILE } from "../art/art.js";
import { CHUNK, artScale, rowOf, rowChunksOfTiles, changedTiles, stackState, citySignature, updateFacing, tierOf } from "./stacks.js";

/** The terrain cache's budget, in art pixels (least recently used chunks go first). */
const PIXEL_BUDGET = 24e6;
/** The time a frame may spend drawing chunks (at least one is drawn per frame). */
const FRAME_MS = 24;
/** The time an idle slice may spend drawing the chunks around the view. */
const IDLE_MS = 12;
/** Chart pixels per tile (the chart zoom steps and the placeholders of chunks not drawn yet). */
const CHART = 4;

const LAYERS = T.MAP_LAYERS;

export class ArtMap {
  /** `view`: the MapView (camera, viewport, canvas, overlay). */
  constructor(view) {
    this.v = view;
    this.art = look.art;
    this.chunks = new Map();
    this.charts = new Map();
    this.pixels = 0;
    this.clock = 0;
    this.map = null;
    this.cmap = null;
    this.plots = null;
    this.ccities = [];
    this.cityById = new Map();
    this.sigs = new Map();
    this.lastSize = new Map();
    this.facing = new Map();
    this.owners = new Map();
    this.viewSeen = null;
    this.miniCanvas = null;
    this.idleTimer = null;
    this.vig = null;
    this.fogRgb = new Map();
    /** Frame statistics (`window.ffh.mapView.art.stats`): the last frame's milliseconds by
     *  phase, chunks drawn, the slowest chunk. */
    this.stats = { frames: 0, frame: 0, phases: {}, chunksDrawn: 0, chunkMs: 0, chunkMax: 0, placeholders: 0 };
  }

  /** Every cached chunk is dropped (another layer of the same map keeps its own). */
  clear() {
    this.chunks.clear();
    this.charts.clear();
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
      return;
    }
    this.cmap = fromApiMap(map, map.w, map.h, !!map.wrap_x);
    this.plots = makePlots(this.cmap);
    const tiles = changed === "all" || !prev ? "all" : Array.isArray(changed) ? changed : changedTiles(prev, map, LAYERS);
    if (tiles === "all" || prev.w !== map.w || prev.h !== map.h) {
      this.clear();
      this.sigs.clear();
      this.facing.clear();
      this.lastSize.clear();
    } else {
      this.drop(rowChunksOfTiles(map, tiles));
    }
    this.miniCanvas = null;
  }

  /** Drops the chunks (every layer) and charts with keys in `keys` ("cx,cr"). */
  drop(keys) {
    if (!keys.size) return;
    for (const [key, c] of [...this.chunks]) {
      if (keys.has(key.slice(key.indexOf("|") + 1))) {
        this.pixels -= c.canvas.width * c.canvas.height;
        this.chunks.delete(key);
      }
    }
    for (const k of keys) this.charts.delete(k);
  }

  /** The view changed (units, cities, players): owners, facing, the cities the art draws, and
   *  the chunks around cities that look different now. */
  syncView() {
    const view = S.view;
    if (view === this.viewSeen || !this.map) return;
    this.viewSeen = view;
    const map = this.map;
    // Owners: you, then the players you met, in slot order; others fly the lozenge.
    const players = [];
    const m = me();
    const civ = (i) => (i === null || i === undefined ? null : tname("CivilizationType", i));
    if (m) players.push({ id: m.player, civ: civ(m.civ), barbarian: false });
    for (const p of [...S.players.values()].sort((a, b) => a.id - b.id)) players.push({ id: p.id, civ: civ(p.civ), barbarian: !!p.barbarian });
    const styles = ownerStyles(players, S.status?.game?.journal ?? S.status?.game?.dir ?? "memory");
    this.owners = new OwnerMap(styles);
    // Facing from the moves since the last view.
    this.facing = updateFacing(this.facing, view?.units ?? [], map.w, !!map.wrap_x);
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
    }
    this.sigs = sigs;
    this.ccities = cities;
    this.cityById = new Map(cities.map((c) => [c.id, c]));
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

  /** The chunks in view: `{cx, cr, cols, rows, X, Y}` (X, Y: device position). */
  visibleChunks(f) {
    const map = this.map;
    const out = [];
    const ncx = Math.ceil(map.w / CHUNK);
    const k0 = map.wrap_x ? Math.floor(f.c0 / map.w) : 0;
    const k1 = map.wrap_x ? Math.floor(f.c1 / map.w) : 0;
    for (let cr = Math.floor(f.r0 / CHUNK); cr <= Math.floor(f.r1 / CHUNK); cr++) {
      const rows = Math.min(CHUNK, map.h - cr * CHUNK);
      for (let k = k0; k <= k1; k++) {
        for (let cx = 0; cx < ncx; cx++) {
          const x0 = cx * CHUNK + k * map.w;
          const cols = Math.min(CHUNK, map.w - cx * CHUNK);
          if (x0 + cols - 1 < f.c0 || x0 > f.c1) continue;
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
    let incomplete = false;
    this.stats.chunksDrawn = 0;
    this.stats.placeholders = 0;
    const deadline = t0 + FRAME_MS;
    for (const c of this.visibleChunks(f)) {
      if (!f.s) {
        const chart = this.chart(c.cx, c.cr, c.cols, c.rows);
        g.drawImage(chart, 0, 0, c.cols * CHART, c.rows * CHART, c.X, c.Y, c.cols * f.tp, c.rows * f.tp);
        continue;
      }
      let ch = this.chunks.get(`${layer}|${c.cx},${c.cr}`);
      if (!ch && (this.stats.chunksDrawn === 0 || performance.now() < deadline)) ch = this.renderChunk(c.cx, c.cr, layer);
      if (ch) {
        ch.used = this.clock;
        g.drawImage(ch.canvas, 0, 0, c.cols * TILE, c.rows * TILE, c.X, c.Y, c.cols * f.tp, c.rows * f.tp);
      } else {
        incomplete = true;
        this.stats.placeholders++;
        const chart = this.chart(c.cx, c.cr, c.cols, c.rows);
        g.drawImage(chart, 0, 0, c.cols * CHART, c.rows * CHART, c.X, c.Y, c.cols * f.tp, c.rows * f.tp);
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
      lap("overlays");
    } else {
      this.drawChartUnits(g, f);
      this.drawChartOverlays(g, f);
      lap("units");
    }
    this.stats.frames++;
    this.stats.frame = Math.round((performance.now() - t0) * 10) / 10;
    this.stats.phases = ph;
    if (incomplete) v.request();
    else this.idle(f);
    // The path's dots march (docs/ART.md 4.5: 1 px every 150 ms) while a path is shown.
    if (f.s && v.overlay.path?.ok && !this.marching) {
      this.marching = setTimeout(() => {
        this.marching = null;
        if (v.overlay.path?.ok) v.request();
      }, 150);
    }
  }

  /** Draws chunk (cx, cr) of `layer` at 1x into the cache. */
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
      if (layer === "political") this.politicalWash(ctx, tx0, ty0, cols, rows);
      if (layer === "yield") this.yieldLayer(ctx, tx0, ty0, cols, rows);
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
      this.chunks.delete(key);
      this.pixels -= c.canvas.width * c.canvas.height;
    }
  }

  /** Once the view is complete: the chunks around it (one chunk's margin), drawn while the page
   *  is idle, a few milliseconds at a time, so that panning finds them ready. */
  idle(f) {
    if (this.idleTimer || !f.s) return;
    const layer = this.v.layer;
    const map = this.map;
    const ncx = Math.ceil(map.w / CHUNK);
    const ncr = Math.ceil(map.h / CHUNK);
    const want = [];
    const cx0 = Math.floor(f.c0 / CHUNK) - 1;
    const cx1 = Math.floor(f.c1 / CHUNK) + 1;
    for (let cr = Math.max(0, Math.floor(f.r0 / CHUNK) - 1); cr <= Math.min(ncr - 1, Math.floor(f.r1 / CHUNK) + 1); cr++) {
      for (let ux = cx0; ux <= cx1; ux++) {
        const cx = map.wrap_x ? ((ux % ncx) + ncx) % ncx : ux;
        if (cx < 0 || cx >= ncx) continue;
        if (!this.chunks.has(`${layer}|${cx},${cr}`) && !want.some(([a, b]) => a === cx && b === cr)) want.push([cx, cr]);
      }
    }
    if (!want.length) return;
    const step = (deadline) => {
      this.idleTimer = null;
      if (this.map !== map || this.v.layer !== layer) return;
      const until = performance.now() + Math.min(IDLE_MS, deadline?.timeRemaining?.() ?? IDLE_MS);
      while (want.length && performance.now() < until) {
        const [cx, cr] = want.shift();
        if (!this.chunks.has(`${layer}|${cx},${cr}`)) this.renderChunk(cx, cr, layer).used = this.clock;
      }
      if (want.length) this.idleTimer = schedule(step);
    };
    this.idleTimer = schedule(step);
  }

  /** The political layer (docs/ART.md 8.1): every owned pixel of revealed ground through a
   *  25% grid of the owner's shade. */
  politicalWash(ctx, tx0, ty0, cols, rows) {
    const art = this.art;
    for (let j = 0; j < rows; j++) {
      for (let i = 0; i < cols; i++) {
        const p = this.plots.get(tx0 + i, ty0 + j);
        if (!p || !p.vis || p.owner === null || p.owner === undefined) continue;
        const o = this.owners.get(p.owner);
        if (!o) continue;
        const shade = o.tincture.shade;
        const X0 = p.x * TILE;
        const Y0 = p.y * TILE;
        art.remapRegion(ctx, i * TILE, j * TILE, TILE, TILE, (a, b, c) => (((X0 + a) & 1) === 0 && ((Y0 + b) & 1) === 0 ? shade : c));
      }
    }
  }

  /** The yield layer: the ground through FOG, and each visible plot's food, production and
   *  commerce as their 8x8 icons and Tally digits (ui.js drawTileYields). */
  yieldLayer(ctx, tx0, ty0, cols, rows) {
    const art = this.art;
    const FOG = art.tables.FOG;
    art.remapRegion(ctx, 0, 0, cols * TILE, rows * TILE, (a, b, c) => FOG[c]);
    const map = this.map;
    for (let j = 0; j < rows; j++) {
      for (let i = 0; i < cols; i++) {
        const row = ty0 + j;
        const y = map.h - 1 - row;
        const x = tx0 + i;
        const yl = T.unpackYield(map.yield[y * map.w + x]);
        if (yl) UI.drawTileYields(art, ctx, i * TILE, j * TILE, yl);
      }
    }
  }

  /** The viewport vignette (docs/ART.md 8.3): beyond 75% of the half-diagonal, pixels pass
   *  through FOG with a Bayer threshold rising to full at the corners. Only the strips at the
   *  sides that hold such pixels are read back. */
  vignette(g, f) {
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

  /** Units (docs/ART.md 3.1, compose.js's unit layers): the sprites row by row, the city
   *  banners and plaques, then the stacks' standards, counts, health and marks. */
  drawUnits(g, f) {
    const art = this.art;
    const stacks = this.stacksInView(f);
    const cityAt = new Set(this.ccities.map((c) => `${c.x},${c.y}`));
    const owner = (p) => this.owners.get(p);
    const zoom = f.s / f.dpr;
    for (const st of stacks) {
      const X = f.ox + st.ux * f.tp;
      const Y = f.oy + st.row * f.tp;
      art.drawUnit(g, X, Y, st.head.type, owner(st.head.owner), { ...st.state, inCity: cityAt.has(`${st.x},${st.row}`), layer: "sprite" });
    }
    for (const c of this.ccities) {
      if (c.y < f.r0 - 1 || c.y > f.r1 + 1) continue;
      const p = this.plots.get(c.x, c.y);
      if (!p || !p.vis) continue;
      for (const ux of this.copies(f, c.x)) {
        art.drawCity(g, f.ox + ux * f.tp, f.oy + c.y * f.tp, { ...c, visible: p.vis === 2, layer: "overlay", plaque: zoom >= 2 ? "full" : "chip" }, owner(c.owner));
      }
    }
    for (const st of stacks) {
      const X = f.ox + st.ux * f.tp;
      const Y = f.oy + st.row * f.tp;
      art.drawUnit(g, X, Y, st.head.type, owner(st.head.owner), { ...st.state, inCity: cityAt.has(`${st.x},${st.row}`), layer: "overlay" });
    }
  }

  /** The tile marks (targets, selection, hover), the waypoints and the path preview. */
  drawOverlays(g, f) {
    const art = this.art;
    const o = this.v.overlay;
    const at = (x, y, fn) => {
      const row = rowOf(this.map, y);
      if (row < f.r0 - 1 || row > f.r1 + 1) return;
      for (const ux of this.copies(f, x)) fn(f.ox + ux * f.tp, f.oy + row * f.tp, ux, row);
    };
    if (Array.isArray(o.targets)) for (const [x, y] of o.targets) at(x, y, (X, Y, ux, row) => UI.drawTileMark(art, g, X, Y, "target", { mx: ux, my: row }));
    if (o.sel) at(o.sel.x, o.sel.y, (X, Y) => UI.drawTileMark(art, g, X, Y, "select"));
    if (o.hover && !(o.sel && o.sel.x === o.hover.x && o.sel.y === o.hover.y)) at(o.hover.x, o.hover.y, (X, Y) => UI.drawTileMark(art, g, X, Y, o.targets ? "route" : "hover"));
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

  /** A palette key's colour as [r, g, b]. */
  rgbOf(index) {
    const a = this.art.rgb;
    return [a[index * 3], a[index * 3 + 1], a[index * 3 + 2]];
  }

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

  /** Chunk (cx, cr) as a chart: CHART px a tile, terrain colours, owned land washed with a 50%
   *  checker of the owner's field, cities as vellum dots, fog through FOG, unrevealed void. */
  chart(cx, cr, cols, rows) {
    const key = `${cx},${cr}`;
    let c = this.charts.get(key);
    if (c) return c;
    const art = this.art;
    const FOG = art.tables.FOG;
    const img = new ImageData(cols * CHART, rows * CHART);
    const d = img.data;
    const v8 = art.K("8");
    const v0 = art.K("0");
    for (let j = 0; j < rows; j++) {
      for (let i = 0; i < cols; i++) {
        const p = this.plots.get(cx * CHUNK + i, cr * CHUNK + j);
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
    c = document.createElement("canvas");
    c.width = cols * CHART;
    c.height = rows * CHART;
    c.getContext("2d").putImageData(img, 0, 0);
    this.charts.set(key, c);
    return c;
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

/** Owner styles by player id; a player the client does not know flies the lozenge (and a
 *  barbarian the skull on Sable). */
class OwnerMap extends Map {
  get(id) {
    if (id === null || id === undefined || id < 0) return undefined;
    if (!super.has(id)) {
      const st = isBarbarian(id) ? look.art.ownerStyle("CIVILIZATION_BARBARIAN", { id, barbarian: true }) : unknownOwner(id);
      super.set(id, st);
    }
    return super.get(id);
  }
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

export { tierOf };
