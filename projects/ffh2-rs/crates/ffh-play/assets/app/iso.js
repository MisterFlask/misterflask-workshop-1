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

/** FeatureType name -> feature sprite file (under app/iso/features), drawn as an overlay on top of
 *  the terrain tile. Unmapped features draw nothing (the tile alone shows). */
const FEATURE_SPRITE = {
  FEATURE_FOREST: "forest",
  FEATURE_FOREST_NEW: "forest",
  FEATURE_JUNGLE: "jungle",
  FEATURE_FOREST_ANCIENT: "ancient",
  FEATURE_FOREST_BURNT: "burnt",
};

/** Per-sprite on-screen width as a fraction of the tile; the ancient grove reads bigger. */
const FEATURE_SCALE = { forest: 0.92, jungle: 0.94, ancient: 1.04, burnt: 0.9 };

const FEATURE_FILES = ["forest", "jungle", "ancient", "burnt"];

/** Relief sprites (under app/iso/relief), drawn as tall overlays for a plot's relief: a peak plot
 *  gets the mountain. */
const RELIEF_FILES = ["peak"];

/** A UnitType name -> unit sprite file (under app/iso/units), by keyword; else a drawn token.
 *  Civilian keywords are matched first so e.g. a scout never falls through to a combat archetype. */
function unitSpriteKey(type) {
  const t = type ?? "";
  if (/SETTLER/.test(t)) return "settler";
  if (/WORKER|SLAVE|LABORER/.test(t)) return "worker";
  if (/SCOUT|EXPLORER/.test(t)) return "scout";
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
    for (const name of FEATURE_FILES) load(`f_${name}`, `./iso/features/${name}.png`);
    for (const name of RELIEF_FILES) load(`r_${name}`, `./iso/relief/${name}.png`);
    load("c_town", "./iso/cities/town.png");
    for (const name of ["knight", "lich", "archer", "spider", "settler", "scout", "worker"]) load(`u_${name}`, `./iso/units/${name}.png`);
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
    // Features (forests, ...) overlay their tile, in the same back-to-front order so a nearer
    // clump occludes a farther one.
    this.drawFeatures(g, cells);
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
    const terrain = tname("TerrainType", map.terrain[c.k]);
    const tileKey = TERRAIN_TILE[terrain];
    const img = tileKey && this.imgs[`t_${tileKey}`];
    if (img) {
      // A 1px bleed closes the thin seams between adjacent diamonds (a resized tile's
      // anti-aliased edge does not quite cover otherwise). A peak draws its terrain base here
      // and the mountain relief on top in drawFeatures.
      g.drawImage(img, Math.round(c.sx - TW / 2) - 1, Math.round(c.sy - TH / 2) - 1, TW + 2, TH + 2);
    } else {
      this.diamond(g, c.sx, c.sy);
      g.fillStyle = T.terrainColor(terrain);
      g.fill();
    }
    // Hills: a soft dome of light-over-dark implies raised ground, over any terrain colour,
    // without moving the tile (units and overlays stay anchored at the flat centre). Peaks get
    // their mountain in drawFeatures instead.
    if (map.plot_type[c.k] === this.plotTypes().hills) {
      g.save();
      this.diamond(g, c.sx, c.sy);
      g.clip();
      // A bright, tight crown of light high on the tile fading to a dark lower rim reads as a
      // convex, sunlit mound.
      const grad = g.createRadialGradient(c.sx, c.sy - TH * 0.4, TW * 0.08, c.sx, c.sy + TH * 0.35, TW * 0.66);
      grad.addColorStop(0, "rgba(255,246,222,0.32)");
      grad.addColorStop(0.34, "rgba(255,246,222,0.07)");
      grad.addColorStop(0.62, "rgba(0,0,0,0)");
      grad.addColorStop(1, "rgba(0,0,0,0.36)");
      g.fillStyle = grad;
      g.fillRect(c.sx - TW / 2, c.sy - TH / 2, TW, TH);
      g.restore();
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

  /** Relief and feature overlays drawn on top of the terrain tiles. A peak plot gets the mountain
   *  sprite; otherwise a mapped feature (forest, ...) is drawn. Anchored so the sprite's base sits
   *  just below the tile centre and it rises upward, like a unit sprite; iterated in the same
   *  back-to-front cell order so a nearer overlay occludes a farther one. */
  drawFeatures(g, cells) {
    const v = this.view;
    const map = v.map;
    const TW = v.cam.ts;
    const TH = TW / 2;
    const PLOT = this.plotTypes();
    for (const c of cells) {
      const vis = map.vis[c.k];
      if (vis === 0) continue; // unrevealed
      let img = null;
      let scale = 0.92;
      let lift = 0.18;
      if (map.plot_type[c.k] === PLOT.peak) {
        img = this.imgs.r_peak;
        scale = 1.08; // mountains overhang the tile and rise highest
        lift = 0.12;
      } else {
        const feat = tname("FeatureType", map.feature[c.k]);
        const key = feat && FEATURE_SPRITE[feat];
        img = key ? this.imgs[`f_${key}`] : null;
        if (img) scale = FEATURE_SCALE[key] ?? 0.92;
      }
      if (!img) continue;
      const fw = TW * scale;
      const fh = fw * (img.height / img.width);
      const bx = Math.round(c.sx - fw / 2);
      const by = Math.round(c.sy + TH * lift - fh);
      if (vis === 1) g.globalAlpha = 0.5; // remembered-but-fogged: muted
      g.drawImage(img, bx, by, Math.round(fw), Math.round(fh));
      g.globalAlpha = 1;
    }
  }

  drawCities(g, cells) {
    const v = this.view;
    const TW = v.cam.ts;
    const TH = TW / 2;
    const mine = me()?.player;
    const byPlot = new Map(cells.map((c) => [`${c.x},${c.y}`, c]));
    // Visible cities, back to front so a nearer settlement occludes a farther one.
    const list = [];
    for (const city of S.cities.values()) {
      const c = byPlot.get(`${city.x},${city.y}`);
      if (c) list.push({ city, c });
    }
    list.sort((a, b) => a.c.sy - b.c.sy);
    const img = this.imgs.c_town;
    for (const { city, c } of list) {
      const col = ownerColor(city.owner);
      // Team ring (iso ellipse) under the settlement; gold for a capital.
      g.save();
      g.beginPath();
      g.ellipse(c.sx, c.sy, TW * 0.42, TH * 0.42, 0, 0, Math.PI * 2);
      g.fillStyle = col;
      g.globalAlpha = 0.9;
      g.fill();
      g.globalAlpha = 1;
      g.lineWidth = Math.max(2, TW * 0.055);
      g.strokeStyle = city.capital ? "#ffe9a8" : "rgba(10,10,12,0.5)";
      g.stroke();
      g.restore();
      // The settlement sprite, rising from just below the tile centre; else a labelled box.
      if (img) {
        const cw = TW * 1.02;
        const ch = cw * (img.height / img.width);
        g.drawImage(img, Math.round(c.sx - cw / 2), Math.round(c.sy + TH * 0.12 - ch), Math.round(cw), Math.round(ch));
      } else {
        const s = TW * 0.5;
        g.fillStyle = "rgba(12,14,20,0.82)";
        roundRect(g, c.sx - s / 2, c.sy - s * 0.3, s, s * 0.5, 3);
        g.fill();
        g.strokeStyle = col;
        g.lineWidth = 2;
        g.stroke();
      }
      // Capital crown pip above the settlement.
      if (city.capital) {
        g.fillStyle = "#ffdd55";
        g.strokeStyle = "rgba(10,10,12,0.6)";
        g.lineWidth = 1;
        star(g, c.sx, c.sy - TW * 0.92, 5, Math.max(3, TW * 0.08), Math.max(1.5, TW * 0.035));
        g.fill();
        g.stroke();
      }
      // Size badge at the lower-left of the base.
      if (city.size != null) {
        const r = Math.max(7, TW * 0.15);
        const bx = c.sx - TW * 0.34;
        const by = c.sy + TH * 0.02;
        g.beginPath();
        g.arc(bx, by, r, 0, Math.PI * 2);
        g.fillStyle = "rgba(16,13,12,0.86)";
        g.fill();
        g.lineWidth = Math.max(1.5, TW * 0.025);
        g.strokeStyle = col;
        g.stroke();
        g.fillStyle = city.owner === mine ? "#ffe9a8" : "#f4f0e6";
        g.font = `bold ${Math.round(r * 1.1)}px system-ui, sans-serif`;
        g.textAlign = "center";
        g.textBaseline = "middle";
        g.fillText(String(city.size), bx, by + 0.5);
      }
      // City name chip below the ring, only when there is room.
      if (city.name && TW >= 52) {
        const fs = Math.max(10, Math.round(TW * 0.17));
        g.font = `600 ${fs}px system-ui, sans-serif`;
        const tw2 = g.measureText(city.name).width;
        const pad = fs * 0.5;
        const bw = tw2 + pad * 2;
        const bh = fs + 6;
        const bx = c.sx - bw / 2;
        const by = c.sy + TH * 0.42;
        g.fillStyle = "rgba(12,14,20,0.8)";
        roundRect(g, bx, by, bw, bh, 3);
        g.fill();
        g.fillStyle = city.owner === mine ? "#ffe9a8" : "#e8e4da";
        g.textAlign = "center";
        g.textBaseline = "middle";
        g.fillText(city.name, c.sx, by + bh / 2 + 0.5);
      }
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

/** A filled star path with `points` spikes (outer radius `ro`, inner `ri`), centred at (cx, cy). */
function star(g, cx, cy, points, ro, ri) {
  g.beginPath();
  for (let i = 0; i < points * 2; i++) {
    const r = i % 2 === 0 ? ro : ri;
    const a = (Math.PI / points) * i - Math.PI / 2;
    const x = cx + Math.cos(a) * r;
    const y = cy + Math.sin(a) * r;
    i === 0 ? g.moveTo(x, y) : g.lineTo(x, y);
  }
  g.closePath();
}
