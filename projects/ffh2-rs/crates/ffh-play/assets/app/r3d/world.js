// The 3D world's ground model (docs/R3D.md "Terrain"): what each plot is, from the `map` reply the
// other renderers draw, and the continuous height of the land over it.
//
// Coordinates: map point (mx, my) -- plot (x, y) covers [x, x+1) x [y, y+1), north is +y -- sits at
// world (X, Z) = (mx, -my), Y up; one world unit is one plot. The plot centre (x + 0.5, y + 0.5) is
// where units, cities and markers stand, so the logical grid survives however the ground is shaped.
//
// The height is a blend of the plot kinds around a point (ocean, coast, land, hills, peak), with
// the blend's lookup warped by low noise so coasts and hill feet do not follow the grid; hills add
// rolling noise, peaks a ridged mountain with its summit on the plot centre; rivers (plot edges)
// carve a channel. Unrevealed plots are "void": flat, unknown ground (no hidden plot shapes it).

import { S, tname } from "../state.js";
import { noise2, fbm2, ridged2, hashf } from "./noise.js";
import { smoothstep, clamp } from "./math.js";

/** Plot kinds (the shaders read them too, in the terrain texture's B channel). */
export const KIND = { VOID: 0, OCEAN: 1, COAST: 2, LAND: 3, HILLS: 4, PEAK: 5, ICE: 6 };

/** Ground materials (terrain texture R): one per terrain, as the terrain shader paints them. */
export const GROUND = {
  VOID: 0,
  GRASS: 1,
  PLAINS: 2,
  DESERT: 3,
  TUNDRA: 4,
  SNOW: 5,
  MARSH: 6,
  BURNING_SANDS: 7,
  BROKEN_LANDS: 8,
  PERDITION: 9,
  SHALLOWS: 10,
  COAST: 11,
  OCEAN: 12,
};

const GROUND_OF = {
  TERRAIN_GRASS: GROUND.GRASS,
  TERRAIN_PLAINS: GROUND.PLAINS,
  TERRAIN_DESERT: GROUND.DESERT,
  TERRAIN_TUNDRA: GROUND.TUNDRA,
  TERRAIN_SNOW: GROUND.SNOW,
  TERRAIN_MARSH: GROUND.MARSH,
  TERRAIN_BURNING_SANDS: GROUND.BURNING_SANDS,
  TERRAIN_BROKEN_LANDS: GROUND.BROKEN_LANDS,
  TERRAIN_FIELDS_OF_PERDITION: GROUND.PERDITION,
  TERRAIN_SHALLOWS: GROUND.SHALLOWS,
  TERRAIN_COAST: GROUND.COAST,
  TERRAIN_OCEAN: GROUND.OCEAN,
};

/** Feature classes (terrain texture G). */
export const FEAT = {
  NONE: 0,
  FOREST: 1,
  JUNGLE: 2,
  ANCIENT: 3,
  BURNT: 4,
  YOUNG: 5,
  ICE: 6,
  FLOOD: 7,
  OASIS: 8,
  SCRUB: 9,
  FLAMES: 10,
  VOLCANO: 11,
  OBSIDIAN: 12,
  BLIZZARD: 13,
  SOULS: 14,
  FALLOUT: 15,
};

const FEAT_OF = {
  FEATURE_FOREST: FEAT.FOREST,
  FEATURE_JUNGLE: FEAT.JUNGLE,
  FEATURE_FOREST_ANCIENT: FEAT.ANCIENT,
  FEATURE_FOREST_BURNT: FEAT.BURNT,
  FEATURE_FOREST_NEW: FEAT.YOUNG,
  FEATURE_ICE: FEAT.ICE,
  FEATURE_FLOOD_PLAINS: FEAT.FLOOD,
  FEATURE_OASIS: FEAT.OASIS,
  FEATURE_SCRUB: FEAT.SCRUB,
  FEATURE_FLAMES: FEAT.FLAMES,
  FEATURE_VOLCANO: FEAT.VOLCANO,
  FEATURE_OBSIDIAN_PLAINS: FEAT.OBSIDIAN,
  FEATURE_BLIZZARD: FEAT.BLIZZARD,
  FEATURE_TORMENTED_SOULS: FEAT.SOULS,
  FEATURE_FALLOUT: FEAT.FALLOUT,
};

/** Base height of each kind at its plot centre. */
const BASE = [0.035, -0.42, -0.12, 0.1, 0.1, 0.1, 0.025];

export const SEA = 0;

/** Where the hell terrains begin (corrupted ground glows, ash falls). */
export function isHell(g) {
  return g === GROUND.BURNING_SANDS || g === GROUND.BROKEN_LANDS || g === GROUND.PERDITION;
}

export class World {
  /** `map`: the `map` reply (S.map shape). */
  constructor(map) {
    this.w = map.w;
    this.h = map.h;
    this.wrapX = !!map.wrap_x;
    this.wrapY = !!map.wrap_y;
    this.map = map;
    const n = map.w * map.h;
    this.kind = new Uint8Array(n);
    this.ground = new Uint8Array(n);
    this.feat = new Uint8Array(n);
    this.vis = new Uint8Array(n);
    this.peakJitter = new Float32Array(n * 2);
    this.classify();
    this.buildRivers();
    this.centerH = new Float32Array(n);
    for (let y = 0; y < this.h; y++) for (let x = 0; x < this.w; x++) this.centerH[y * this.w + x] = this.heightAt(x + 0.5, y + 0.5);
  }

  classify() {
    const m = this.map;
    const ptype = (i) => tname("PlotType", m.plot_type[i]);
    for (let i = 0; i < this.w * this.h; i++) {
      this.vis[i] = m.vis[i];
      this.peakJitter[i * 2] = (hashf(i, 7) - 0.5) * 0.18;
      this.peakJitter[i * 2 + 1] = (hashf(i, 8) - 0.5) * 0.18;
      if (m.vis[i] === 0) {
        this.kind[i] = KIND.VOID;
        this.ground[i] = GROUND.VOID;
        this.feat[i] = FEAT.NONE;
        continue;
      }
      const t = tname("TerrainType", m.terrain[i]);
      const f = tname("FeatureType", m.feature[i]);
      this.ground[i] = GROUND_OF[t] ?? GROUND.GRASS;
      this.feat[i] = FEAT_OF[f] ?? FEAT.NONE;
      const pt = ptype(i);
      let k = KIND.LAND;
      if (pt === "PLOT_OCEAN") k = t === "TERRAIN_OCEAN" ? KIND.OCEAN : KIND.COAST;
      else if (pt === "PLOT_HILLS") k = KIND.HILLS;
      else if (pt === "PLOT_PEAK") k = KIND.PEAK;
      if (this.feat[i] === FEAT.ICE && (k === KIND.OCEAN || k === KIND.COAST)) k = KIND.ICE;
      this.kind[i] = k;
    }
  }

  /** The plot index of (x, y) after wrapping, or -1 off the map. */
  index(x, y) {
    if (this.wrapX) x = ((x % this.w) + this.w) % this.w;
    else if (x < 0 || x >= this.w) return -1;
    if (this.wrapY) y = ((y % this.h) + this.h) % this.h;
    else if (y < 0 || y >= this.h) return -1;
    return y * this.w + x;
  }

  kindAt(x, y) {
    const i = this.index(x, y);
    return i < 0 ? KIND.VOID : this.kind[i];
  }

  isWater(i) {
    const k = this.kind[i];
    return k === KIND.OCEAN || k === KIND.COAST;
  }

  /** The ground height at map point (mx, my). */
  heightAt(mx, my) {
    // Warp the blend lookup so coastlines and hill feet wander off the grid lines.
    const wx = mx + 0.2 * noise2(mx * 0.85 + 3.1, my * 0.85 - 1.7);
    const wy = my + 0.2 * noise2(mx * 0.85 - 7.3, my * 0.85 + 5.5);
    const u = wx - 0.5;
    const v = wy - 0.5;
    const x0 = Math.floor(u);
    const y0 = Math.floor(v);
    const fx = smoother(u - x0);
    const fy = smoother(v - y0);
    let h = 0, hill = 0, peak = 0, water = 0, ice = 0, voidW = 0;
    for (let j = 0; j < 2; j++) {
      for (let i = 0; i < 2; i++) {
        const k = this.kindAt(x0 + i, y0 + j);
        const wgt = (i ? fx : 1 - fx) * (j ? fy : 1 - fy);
        h += wgt * BASE[k];
        if (k === KIND.HILLS) hill += wgt;
        else if (k === KIND.PEAK) {
          peak += wgt;
          hill += wgt * 0.6;
        } else if (k === KIND.OCEAN || k === KIND.COAST) water += wgt;
        else if (k === KIND.ICE) ice += wgt;
        else if (k === KIND.VOID) voidW += wgt;
      }
    }
    const land = 1 - water - ice - voidW;
    // Gentle undulation of the lowlands; rough sea floor.
    h += land * 0.05 * fbm2(mx * 1.1 + 40, my * 1.1 - 20, 3);
    h += water * 0.06 * fbm2(mx * 0.9 - 11, my * 0.9 + 4, 3);
    if (hill > 0) {
      // Rolling hills: a raised shoulder with crests of ridged noise.
      const r = ridged2(mx * 1.6 + 5.2, my * 1.6 - 3.4, 3);
      h += smoothstep(0, 1, hill) * (0.19 + 0.2 * r);
    }
    if (peak > 0) {
      // Mountains: ridged massif, plus a summit near each peak plot's centre.
      const r = ridged2(mx * 1.15 - 2.7, my * 1.15 + 8.1, 4);
      h += Math.pow(peak, 1.3) * (0.4 + 0.45 * r);
      h += this.summits(mx, my);
    }
    if (voidW > 0) h = h * (1 - voidW) + BASE[KIND.VOID] * voidW;
    // Rivers carve their channel.
    if (this.riverCells.size) {
      const d = this.riverDist(mx, my);
      if (d < 0.3) {
        const carve = 1 - smoothstep(0.035, 0.2, d);
        h = Math.min(h, h - 0.075 * carve);
        // A river's bed never rises over its banks' sea level side.
        if (carve > 0.5) h = Math.max(h, -0.06);
      }
    }
    return h;
  }

  /** The summit cones of the peak plots near (mx, my). */
  summits(mx, my) {
    let s = 0;
    const cx = Math.floor(mx);
    const cy = Math.floor(my);
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const i = this.index(cx + dx, cy + dy);
        if (i < 0 || this.kind[i] !== KIND.PEAK) continue;
        const px = cx + dx + 0.5 + this.peakJitter[i * 2];
        const py = cy + dy + 0.5 + this.peakJitter[i * 2 + 1];
        const d = Math.hypot(mx - px, my - py);
        const t = clamp(1 - d / 0.72, 0, 1);
        s = Math.max(s, 0.45 * Math.pow(t, 1.5));
      }
    }
    return s;
  }

  /** The height at the centre of plot (x, y): where units and buildings stand. */
  plotHeight(x, y) {
    const i = this.index(x, y);
    if (i < 0) return BASE[KIND.VOID];
    return Math.max(this.centerH[i], SEA);
  }

  /** The normal of the ground at (mx, my) (world space: x east, y up, z south). */
  normalAt(mx, my, e = 0.05) {
    const hx = this.heightAt(mx + e, my) - this.heightAt(mx - e, my);
    const hy = this.heightAt(mx, my + e) - this.heightAt(mx, my - e);
    // World z = -my, so d/dz = -d/dmy.
    const n = [-hx / (2 * e), 1, hy / (2 * e)];
    const l = Math.hypot(n[0], n[1], n[2]);
    return [n[0] / l, n[1] / l, n[2] / l];
  }

  // --- rivers -----------------------------------------------------------------------------------

  /** River polylines in map coordinates, from the plot-edge river bits: chained through shared
   *  corners, smoothed (Chaikin) and given a slight meander. `riverCells` indexes their segments by
   *  integer cell for distance queries. */
  buildRivers() {
    const m = this.map;
    const w = this.w;
    const segs = [];
    for (let y = 0; y < this.h; y++) {
      for (let x = 0; x < w; x++) {
        const i = y * w + x;
        if (m.vis[i] === 0) continue;
        const r = m.river[i];
        if (r <= 0) continue;
        if (r & 1) segs.push([[x, y], [x + 1, y]]); // the plot's south edge
        if (r & 2) segs.push([[x + 1, y], [x + 1, y + 1]]); // its east edge
      }
    }
    // Chain the segments through shared corners.
    const key = ([x, y]) => `${this.wrapX ? ((x % w) + w) % w : x},${y}`;
    const at = new Map();
    segs.forEach((s, k) => {
      for (const p of s) {
        const kk = key(p);
        if (!at.has(kk)) at.set(kk, []);
        at.get(kk).push(k);
      }
    });
    const used = new Uint8Array(segs.length);
    const lines = [];
    // Walks from corner `from` along segment `start`, through corners shared by exactly two
    // segments, until an end or a junction.
    const walk = (start, from) => {
      const line = [from.slice()];
      let cur = start;
      let p = from;
      for (;;) {
        used[cur] = 1;
        const s = segs[cur];
        const other = key(s[0]) === key(p) ? s[1] : s[0];
        // Keep the line continuous across the wrap seam.
        const last = line[line.length - 1];
        let qx = other[0];
        if (this.wrapX) qx += Math.round((last[0] - qx) / w) * w;
        line.push([qx, other[1]]);
        p = other;
        const all = at.get(key(p)) ?? [];
        const next = all.filter((k) => !used[k]);
        if (all.length !== 2 || next.length !== 1) break;
        cur = next[0];
      }
      return line;
    };
    // Start from ends and junctions (corners not shared by exactly two segments), then any loops.
    for (const [kk, ks] of at) {
      if (ks.length === 2) continue;
      for (const k of ks) {
        if (used[k]) continue;
        const s = segs[k];
        lines.push(walk(k, key(s[0]) === kk ? s[0] : s[1]));
      }
    }
    segs.forEach((s, k) => {
      if (!used[k]) lines.push(walk(k, s[0]));
    });
    this.rivers = lines.map((l) => meander(chaikin(chaikin(l))));
    // Segment index by cell.
    this.riverCells = new Map();
    this.riverSegs = [];
    for (const line of this.rivers) {
      for (let i = 0; i + 1 < line.length; i++) {
        const a = line[i], b = line[i + 1];
        const k = this.riverSegs.length;
        this.riverSegs.push([a[0], a[1], b[0], b[1]]);
        const x0 = Math.floor(Math.min(a[0], b[0]) - 0.35), x1 = Math.floor(Math.max(a[0], b[0]) + 0.35);
        const y0 = Math.floor(Math.min(a[1], b[1]) - 0.35), y1 = Math.floor(Math.max(a[1], b[1]) + 0.35);
        for (let y = y0; y <= y1; y++) {
          for (let x = x0; x <= x1; x++) {
            const ck = `${this.wrapX ? ((x % w) + w) % w : x},${y}`;
            if (!this.riverCells.has(ck)) this.riverCells.set(ck, []);
            this.riverCells.get(ck).push(k);
          }
        }
      }
    }
  }

  /** Distance from map point (mx, my) to the nearest river (Infinity when none is near). */
  riverDist(mx, my) {
    const w = this.w;
    const cx = Math.floor(mx);
    const ck = `${this.wrapX ? ((cx % w) + w) % w : cx},${Math.floor(my)}`;
    const list = this.riverCells.get(ck);
    if (!list) return Infinity;
    let best = Infinity;
    for (const k of list) {
      const [ax0, ay, bx0, by] = this.riverSegs[k];
      // Bring the segment next to the point across the wrap seam.
      let ax = ax0, bx = bx0;
      if (this.wrapX) {
        const s = Math.round((mx - ax) / w) * w;
        ax += s;
        bx += s;
      }
      best = Math.min(best, segDist(mx, my, ax, ay, bx, by));
    }
    return best;
  }
}

function smoother(t) {
  return t * t * t * (t * (t * 6 - 15) + 10);
}

function segDist(px, py, ax, ay, bx, by) {
  const dx = bx - ax, dy = by - ay;
  const l2 = dx * dx + dy * dy || 1e-9;
  const t = clamp(((px - ax) * dx + (py - ay) * dy) / l2, 0, 1);
  return Math.hypot(px - ax - dx * t, py - ay - dy * t);
}

/** One round of Chaikin corner cutting (keeps the end points). */
function chaikin(pts) {
  if (pts.length < 3) return pts;
  const out = [pts[0]];
  for (let i = 0; i + 1 < pts.length; i++) {
    const a = pts[i], b = pts[i + 1];
    out.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25], [a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75]);
  }
  out.push(pts[pts.length - 1]);
  return out;
}

/** A gentle sideways wander along a polyline (deterministic by position). */
function meander(pts) {
  return pts.map((p, i) => {
    if (i === 0 || i === pts.length - 1) return p;
    const a = pts[i - 1], b = pts[i + 1];
    let tx = b[0] - a[0], ty = b[1] - a[1];
    const l = Math.hypot(tx, ty) || 1;
    tx /= l;
    ty /= l;
    const o = 0.07 * noise2(p[0] * 2.3 + 0.5, p[1] * 2.3 - 0.5);
    return [p[0] - ty * o, p[1] + tx * o];
  });
}

/** The world built from the current map (S.map), or null. */
export function worldOf(map = S.map) {
  return map ? new World(map) : null;
}
