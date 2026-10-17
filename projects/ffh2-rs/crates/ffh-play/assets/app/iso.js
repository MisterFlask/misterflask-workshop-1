// An isometric renderer for the map (the "Songs of Conquest" direction, docs/art/SOC_DIRECTION.md).
//
// It is a third renderer beside the flat one (map.js) and the art one (artmap.js): selected with
// `?renderer=iso`, it reads the same `/api/map` + `/api/view` data the others do and draws the map
// as a 2:1 isometric board with PNG terrain tiles and unit sprites on team-coloured rings. The
// projection and hit-testing live here; MapView delegates draw(), screenToMap() and the drag-pan
// to this class when `mapView.iso` is set, and keeps its own camera, overlays and state.
//
// Geometry: a plot (x, y) has its centre at map point (x + 0.5, y + 0.5); the camera centre
// (cam.cx, cam.cy) is a map point and cam.ts is the tile WIDTH in CSS pixels (tile height = ts/2).
// North is up-and-right, east is down-and-right (Civ4's isometric convention).

import { S, me, tname, dname, isBarbarian } from "./state.js";
import * as T from "./tiles.js";

const ISO_ZOOMS = [32, 48, 64, 96, 128];

/** TerrainType name -> terrain tile file (under app/iso/terrain). Unmapped terrains fall back to a
 *  flat diamond in the terrain colour. */
const TERRAIN_TILE = {
  TERRAIN_GRASS: "grass",
  TERRAIN_PLAINS: "plains",
  TERRAIN_DESERT: "desert",
  TERRAIN_SNOW: "snow",
  TERRAIN_TUNDRA: "tundra",
  TERRAIN_MARSH: "marsh",
  TERRAIN_COAST: "coast",
  TERRAIN_OCEAN: "ocean",
  TERRAIN_BROKEN_LANDS: "hell",
  TERRAIN_FIELDS_OF_PERDITION: "perdition",
  TERRAIN_BURNING_SANDS: "burning",
  TERRAIN_SHALLOWS: "coast",
};

const TERRAIN_FILES = ["grass", "plains", "desert", "snow", "tundra", "marsh", "coast", "ocean", "hell", "burning", "perdition"];

/** A UnitType name -> unit sprite file (under app/iso/units), by keyword; else a drawn token. */
function unitSpriteKey(type) {
  const t = type ?? "";
  if (/ARCHER|HUNTER|RANGER|MARKSMAN|LONGBOW|CROSSBOW/.test(t)) return "archer";
  if (/MAGE|ADEPT|LICH|WIZARD|SORCER|ARCHMAGE|PRIEST|SUMMONER|DRUID|CONJUR|NECROMANCER/.test(t)) return "lich";
  if (/SPIDER|BEAST|WOLF|LION|BEAR|TIGER|GORILLA|SCORPION|BROOD|ANIMAL/.test(t)) return "spider";
  if (/WARRIOR|SWORD|AXE|MACE|KNIGHT|CHAMPION|SOLDIER|MELEE|HORSE|HORSEMAN|LANCE|PHALANX|SPEAR|DISCIPLE|PALADIN|GUARD/.test(t)) return "knight";
  return null;
}

function ownerColor(p) {
  return T.playerColor(p, isBarbarian(p));
}

export class IsoMap {
  constructor(view) {
    this.view = view;
    this.imgs = {};
    this.ready = false;
    const load = (key, url) => {
      const im = new Image();
      im.onload = () => {
        this.imgs[key] = im;
        this.ready = true;
        this.view.request();
      };
      im.src = new URL(url, import.meta.url).href;
    };
    for (const name of TERRAIN_FILES) load(`t_${name}`, `./iso/terrain/${name}.png`);
    for (const name of ["knight", "lich", "archer", "spider"]) load(`u_${name}`, `./iso/units/${name}.png`);
  }

  dispose() {}

  zooms() {
    return ISO_ZOOMS;
  }

  // --- geometry -----------------------------------------------------------------------------

  tw() {
    return this.view.cam.ts;
  }

  /** The screen centre (CSS px) of plot (x, y), taking the copy of a wrapping map nearest the
   *  camera. */
  center(x, y) {
    const { cam, vw, vh, map } = this.view;
    const TW = cam.ts;
    const TH = cam.ts / 2;
    let u = x + 0.5;
    if (map?.wrap_x) u += Math.round((cam.cx - u) / map.w) * map.w;
    const v = y + 0.5;
    const ex = u - cam.cx;
    const ny = v - cam.cy;
    return { sx: vw / 2 + (ex + ny) * (TW / 2), sy: vh / 2 + (ex - ny) * (TH / 2) };
  }

  /** The plot under screen point (sx, sy), or null off the map. */
  screenToMap(sx, sy) {
    const { cam, vw, vh, map } = this.view;
    if (!map) return null;
    const TW = cam.ts;
    const TH = cam.ts / 2;
    const A = (sx - vw / 2) / (TW / 2);
    const B = (sy - vh / 2) / (TH / 2);
    const u = cam.cx + (A + B) / 2;
    const v = cam.cy + (A - B) / 2;
    const x = T.wrapCoord(Math.floor(u), map.w, map.wrap_x);
    const y = T.wrapCoord(Math.floor(v), map.h, map.wrap_y);
    return x === null || y === null ? null : { x, y };
  }

  /** The camera centre after a drag of (dx, dy) screen pixels from (down.cx, down.cy). */
  panDelta(startCx, startCy, dx, dy) {
    const TW = this.view.cam.ts;
    const TH = TW / 2;
    return { cx: startCx - dx / TW - dy / TH, cy: startCy - dx / TW + dy / TH };
  }

  screenOf(x, y) {
    const c = this.center(x, y);
    return { x: c.sx, y: c.sy, ts: this.view.cam.ts };
  }

  // --- drawing ------------------------------------------------------------------------------

  diamond(g, sx, sy) {
    const hw = this.view.cam.ts / 2;
    const hh = this.view.cam.ts / 4;
    g.beginPath();
    g.moveTo(sx, sy - hh);
    g.lineTo(sx + hw, sy);
    g.lineTo(sx, sy + hh);
    g.lineTo(sx - hw, sy);
    g.closePath();
  }

  draw() {
    const v = this.view;
    const g = v.ctx;
    const map = v.map;
    g.setTransform(v.dpr, 0, 0, v.dpr, 0, 0);
    g.fillStyle = "#0b1017";
    g.fillRect(0, 0, v.vw, v.vh);
    if (!map) return;
    g.imageSmoothingEnabled = false;

    const TW = v.cam.ts;
    const TH = TW / 2;
    // Collect the plots in view (one copy each, nearest the camera), back to front by screen y.
    const cells = [];
    for (let y = 0; y < map.h; y++) {
      for (let x = 0; x < map.w; x++) {
        const c = this.center(x, y);
        if (c.sx < -TW || c.sx > v.vw + TW || c.sy < -TH * 2 || c.sy > v.vh + TH * 3) continue;
        cells.push({ x, y, sx: c.sx, sy: c.sy, k: y * map.w + x });
      }
    }
    cells.sort((a, b) => a.sy - b.sy);

    for (const c of cells) this.drawTile(g, c);
    // Cities then units, back to front (units already sorted by their plot's sy via a fresh pass).
    this.drawCities(g, cells);
    this.drawUnits(g);
    this.drawOverlays(g);
    v.drawFlashes?.(g);
  }

  drawTile(g, c) {
    const v = this.view;
    const map = v.map;
    const vis = map.vis[c.k];
    if (vis === 0) return; // unrevealed: the dark background shows
    const TW = v.cam.ts;
    const TH = TW / 2;
    const PLOT = this.plotTypes();
    const pt = map.plot_type[c.k];
    const terrain = tname("TerrainType", map.terrain[c.k]);
    const tileKey = TERRAIN_TILE[terrain];
    const img = pt === PLOT.peak ? null : tileKey && this.imgs[`t_${tileKey}`];
    if (img) {
      // A 1px bleed closes the thin seams between adjacent diamonds (a resized tile's
      // anti-aliased edge does not quite cover otherwise).
      g.drawImage(img, Math.round(c.sx - TW / 2) - 1, Math.round(c.sy - TH / 2) - 1, TW + 2, TH + 2);
    } else {
      this.diamond(g, c.sx, c.sy);
      g.fillStyle = pt === PLOT.peak ? "#6c6863" : T.terrainColor(terrain);
      g.fill();
      if (pt === PLOT.peak) {
        g.fillStyle = "#cfc9c0";
        g.beginPath();
        g.moveTo(c.sx, c.sy - TH * 0.5);
        g.lineTo(c.sx + TW * 0.22, c.sy + TH * 0.1);
        g.lineTo(c.sx - TW * 0.22, c.sy + TH * 0.1);
        g.closePath();
        g.fill();
      }
    }
    // A faint tile outline keeps the grid legible without reading as a hard lattice.
    this.diamond(g, c.sx, c.sy);
    g.strokeStyle = "rgba(10,13,18,0.16)";
    g.lineWidth = 1;
    g.stroke();
    if (vis === 1) {
      this.diamond(g, c.sx, c.sy);
      g.fillStyle = "rgba(6,10,16,0.45)";
      g.fill();
    }
  }

  drawCities(g, cells) {
    const v = this.view;
    const TW = v.cam.ts;
    const mine = me()?.player;
    const byPlot = new Map(cells.map((c) => [`${c.x},${c.y}`, c]));
    for (const city of S.cities.values()) {
      const c = byPlot.get(`${city.x},${city.y}`);
      if (!c) continue;
      const col = ownerColor(city.owner);
      const w = TW * 0.5;
      const hh = Math.max(9, TW * 0.26);
      const bx = c.sx - w / 2;
      const by = c.sy - hh * 0.5 - TW * 0.08;
      g.fillStyle = "rgba(12,14,20,0.82)";
      roundRect(g, bx, by, w, hh, 3);
      g.fill();
      g.strokeStyle = col;
      g.lineWidth = 2;
      g.stroke();
      g.fillStyle = city.owner === mine ? "#ffe9a8" : col;
      g.font = `bold ${Math.round(hh * 0.74)}px system-ui, sans-serif`;
      g.textAlign = "center";
      g.textBaseline = "middle";
      g.fillText(city.size == null ? "?" : String(city.size), c.sx, by + hh / 2 + 0.5);
    }
  }

  drawUnits(g) {
    const v = this.view;
    const TW = v.cam.ts;
    const TH = TW / 2;
    const mine = me()?.player;
    const sel = v.overlay.units ?? [];
    const list = [];
    for (const units of S.unitsAt.values()) {
      const u0 = units[0];
      list.push({ units, c: this.center(u0.x, u0.y) });
    }
    list.sort((a, b) => a.c.sy - b.c.sy);
    for (const { units, c } of list) {
      if (c.sx < -TW || c.sx > v.vw + TW || c.sy < -TH * 3 || c.sy > v.vh + TH * 2) continue;
      const top = T.topUnit(units, mine, sel);
      const col = ownerColor(top.owner);
      // Team ring (iso ellipse) under the feet.
      g.save();
      g.beginPath();
      g.ellipse(c.sx, c.sy, TW * 0.36, TH * 0.36, 0, 0, Math.PI * 2);
      g.fillStyle = col;
      g.globalAlpha = 0.9;
      g.fill();
      g.globalAlpha = 1;
      g.lineWidth = Math.max(2, TW * 0.05);
      g.strokeStyle = sel.includes(top.id) ? "#fff7a0" : "rgba(10,10,12,0.5)";
      g.stroke();
      g.restore();
      // The figure: a sprite when the type maps to one, else a token.
      const key = unitSpriteKey(tname("UnitType", top.type));
      const img = key && this.imgs[`u_${key}`];
      if (img) {
        const uh = TW * 0.98;
        const uw = uh * (img.width / img.height);
        g.drawImage(img, Math.round(c.sx - uw / 2), Math.round(c.sy + TH * 0.15 - uh), Math.round(uw), Math.round(uh));
      } else {
        const s = TW * 0.42;
        g.fillStyle = col;
        g.strokeStyle = sel.includes(top.id) ? "#fff7a0" : "rgba(0,0,0,0.85)";
        g.lineWidth = 1.5;
        roundRect(g, c.sx - s / 2, c.sy - TH * 0.2 - s, s, s, Math.max(2, s / 6));
        g.fill();
        g.stroke();
        g.fillStyle = "#101010";
        g.font = `bold ${Math.round(s * 0.42)}px system-ui, sans-serif`;
        g.textAlign = "center";
        g.textBaseline = "middle";
        g.fillText(T.abbrev(dname("UnitType", top.type)), c.sx, c.sy - TH * 0.2 - s / 2 + 0.5);
      }
      // Count badge.
      if (units.length > 1) {
        const t = String(units.length);
        g.font = `bold ${Math.max(10, Math.round(TW * 0.2))}px system-ui, sans-serif`;
        const wch = g.measureText(t).width + 6;
        const bx = c.sx - TW * 0.34 - wch;
        const by = c.sy - TW * 0.7;
        g.fillStyle = "rgba(16,13,12,0.85)";
        roundRect(g, bx, by, wch, Math.max(13, TW * 0.22), 3);
        g.fill();
        g.fillStyle = "#f4f0e6";
        g.textAlign = "center";
        g.textBaseline = "middle";
        g.fillText(t, bx + wch / 2, by + Math.max(13, TW * 0.22) / 2 + 0.5);
      }
      // Moves pip for own units.
      if (top.owner === mine) {
        const r = Math.max(2.5, TW * 0.05);
        g.fillStyle = top.moves > 0 ? "#3fdc5a" : "#d03a2a";
        g.beginPath();
        g.arc(c.sx + TW * 0.28, c.sy - TW * 0.62, r, 0, Math.PI * 2);
        g.fill();
      }
    }
  }

  drawOverlays(g) {
    const o = this.view.overlay;
    const ring = (x, y, color, width, dash = []) => {
      const c = this.center(x, y);
      this.diamond(g, c.sx, c.sy);
      g.strokeStyle = color;
      g.lineWidth = width;
      g.setLineDash(dash);
      g.stroke();
      g.setLineDash([]);
    };
    if (Array.isArray(o.targets)) for (const [x, y] of o.targets) ring(x, y, "#4fe3ff", 2, [4, 3]);
    if (o.sel) ring(o.sel.x, o.sel.y, "#ffe14d", 2.5);
    if (o.hover) ring(o.hover.x, o.hover.y, o.targets ? "#4fe3ff" : "rgba(255,255,255,0.7)", 1.5);
  }

  plotTypes() {
    if (this._plot) return this._plot;
    const t = S.data?.tables?.PlotType ?? [];
    const at = (n, d) => {
      const i = t.findIndex((e) => e.type === n);
      return i < 0 ? d : i;
    };
    this._plot = { peak: at("PLOT_PEAK", 0), hills: at("PLOT_HILLS", 1), land: at("PLOT_LAND", 2), ocean: at("PLOT_OCEAN", 3) };
    return this._plot;
  }

  minimap() {
    return null; // MapView falls back to its own top-down minimap.
  }
}

function roundRect(g, x, y, w, h, r) {
  g.beginPath();
  g.moveTo(x + r, y);
  g.arcTo(x + w, y, x + w, y + h, r);
  g.arcTo(x + w, y + h, x, y + h, r);
  g.arcTo(x, y + h, x, y, r);
  g.arcTo(x, y, x + w, y, r);
  g.closePath();
}
