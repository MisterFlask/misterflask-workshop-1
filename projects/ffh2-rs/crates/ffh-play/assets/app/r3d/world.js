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
import { noise2, noise2p, fbm2p, ridged2p, hashf } from "./noise.js";
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

/** How deep a hull sits in the water: its keel is under the surface, so the water is seen over it
 *  (refracted, tinted) and laps at its waterline in a collar of foam (shaders.js WATER_FS). Only
 *  hulls (ships, boats, wrecks, swimmers) take it; what lies on the surface (a resource's ripples, a
 *  whirlpool, a floe) floats at the sea's level. */
export const DRAFT = 0.012;

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
    this.findLakes();
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

  /** Lakes: bodies of water of at most 12 plots (8-connected, as the game's areas) that touch no
   *  unrevealed plot. Their floors are sunk into bowls (heightAt): the blend with the land around
   *  would leave a small lake all shelf, so it painted as surf and pale shallows, like a salt pan. */
  findLakes() {
    const n = this.w * this.h;
    this.lake = new Uint8Array(n);
    this.hasLakes = false;
    const seen = new Uint8Array(n);
    const water = (k) => k === KIND.OCEAN || k === KIND.COAST;
    for (let s = 0; s < n; s++) {
      if (seen[s] || !water(this.kind[s])) continue;
      const comp = [s];
      seen[s] = 1;
      let open = false;
      for (let q = 0; q < comp.length && comp.length <= 12; q++) {
        const x = comp[q] % this.w, y = (comp[q] - x) / this.w;
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            const j = this.index(x + dx, y + dy);
            if (j < 0 || j === comp[q]) continue;
            if (this.kind[j] === KIND.VOID) open = true;
            else if (!seen[j] && water(this.kind[j])) {
              seen[j] = 1;
              comp.push(j);
            }
          }
        }
      }
      if (comp.length <= 12 && !open) {
        for (const i of comp) this.lake[i] = 1;
        this.hasLakes = true;
      }
    }
  }

  /** The round bowls of the lake plots near (mx, my), summed (0 at a bowl's rim, 1 at a centre). */
  lakeBowl(mx, my) {
    let s = 0;
    const cx = Math.floor(mx);
    const cy = Math.floor(my);
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const i = this.index(cx + dx, cy + dy);
        if (i < 0 || !this.lake[i]) continue;
        const t = clamp(1 - Math.hypot(mx - (cx + dx + 0.5), my - (cy + dy + 0.5)) / 0.8, 0, 1);
        s += t * t * (3 - 2 * t);
      }
    }
    return s;
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
  heightAt(mx, my, carve = true) {
    // Warp the blend lookup so coastlines and hill feet wander off the grid lines.
    // Two octaves: broad bays and headlands, then a ragged edge (plot centres stay inside their
    // plot: the warp never reaches half a plot).
    const N = (f, ox, oy) => this.n2(mx, my, f, ox, oy);
    const wx = mx + 0.34 * N(0.7, 3.1, -1.7) + 0.1 * N(2.6, -4.4, 9.1);
    const wy = my + 0.34 * N(0.7, -7.3, 5.5) + 0.1 * N(2.6, 6.2, -2.3);
    const u = wx - 0.5;
    const v = wy - 0.5;
    const x0 = Math.floor(u);
    const y0 = Math.floor(v);
    const fx = smoother(u - x0);
    const fy = smoother(v - y0);
    let h = 0, hill = 0, peak = 0, water = 0, ice = 0, voidW = 0, sand = 0, lakeW = 0;
    for (let j = 0; j < 2; j++) {
      for (let i = 0; i < 2; i++) {
        const k = this.kindAt(x0 + i, y0 + j);
        const wgt = (i ? fx : 1 - fx) * (j ? fy : 1 - fy);
        h += wgt * BASE[k];
        const gi = this.index(x0 + i, y0 + j);
        if (gi >= 0 && (this.ground[gi] === GROUND.DESERT || this.ground[gi] === GROUND.BURNING_SANDS) && k === KIND.LAND) sand += wgt;
        if (gi >= 0 && this.lake[gi]) lakeW += wgt;
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
    h += land * 0.05 * this.fbm(mx, my, 1.1, 40, -20, 3);
    h += water * 0.06 * this.fbm(mx, my, 0.9, -11, 4, 3);
    // A lake's floor falls away just inside its shore (the shore is where the blend crosses sea
    // level, at about 0.45 of the lake's weight), so a lake is a bowl with a narrow rim of surf;
    // round bowls about its plots' centres keep the deep water from following the plot grid.
    if (lakeW > 0.4) h -= 0.2 * smoothstep(0.42, 0.95, lakeW) * Math.min(1, this.lakeBowl(mx, my) * 1.4);
    if (sand > 0.05) {
      // Dunes: wind-combed ridges, sharp crest and long back slope, wandering with the noise.
      const ph = 2 * Math.PI * (mx * this.freq(1.25) + my * 0.55) + 2.6 * this.n2(mx, my, 0.6, 12.5, -4.1);
      const crest = Math.pow(0.5 + 0.5 * Math.sin(ph), 3);
      h += smoothstep(0.05, 0.8, sand) * 0.045 * crest * (0.6 + 0.4 * this.n2(mx, my, 1.4, -2.2, 7.7));
    }
    // Hills and mountains do not spill into a lake (their domes reach over the plot's edge).
    const dry = lakeW > 0.3 ? 1 - smoothstep(0.3, 0.6, lakeW) : 1;
    if (hill > 0 && dry > 0) {
      // Rolling hills: a low shoulder under the hill country, the rounded domes of the hill plots
      // (they overlap into ridges where hills neighbour), crests of ridged noise on top.
      const r = this.ridged(mx, my, 1.6, 5.2, -3.4, 3);
      h += dry * smoothstep(0, 1, hill) * (0.06 + 0.07 * r);
      h += dry * this.domes(mx, my) * (0.8 + 0.35 * r);
    }
    if (peak > 0 && dry > 0) {
      // Mountains: ridged massif, plus a summit near each peak plot's centre.
      // Sharp crests (ridged noise raised to a power: narrow ridges, deep gullies), a second,
      // finer set of spurs, and the summit of each peak plot.
      const r = this.ridged(mx, my, 1.1, -2.7, 8.1, 4);
      const r2 = this.ridged(mx, my, 2.6, 5.3, -1.9, 3);
      const pw = Math.pow(peak, 1.4);
      h += dry * pw * (0.18 + 0.62 * Math.pow(r, 1.7) + 0.16 * r2 * r);
      h += dry * this.summits(mx, my) * (0.75 + 0.35 * r2);
    }
    if (voidW > 0) h = h * (1 - voidW) + BASE[KIND.VOID] * voidW;
    // Rivers carve their channel.
    // Rivers carve their channel (`carve` false: the ground the river runs over, uncut).
    if (carve && this.riverCells.size) {
      const d = this.riverDist(mx, my);
      if (d < 0.32) {
        // A channel with sloping banks: full depth across the bed, easing out to the bank tops.
        const cv = 1 - smoothstep(0.05, 0.24, d);
        h = Math.min(h, h - 0.08 * cv * cv * (3 - 2 * cv));
        // A river's bed never sinks deep under sea level -- on land: over the sea the floor stays
        // the sea's (raised, it would show through the clear water as a sandbar along the river).
        if (cv > 0.5) h = Math.max(h, -0.06 - 0.5 * smoothstep(0.3, 0.7, water));
      }
    }
    return h;
  }

  // Noise of the height, at about f cycles a plot, periodic over the map's width when it wraps
  // (so the ground meets itself at the seam, and every copy of the map is the same ground).

  period(f) {
    return this.wrapX ? Math.max(1, Math.round(this.w * f)) : 1 << 20;
  }

  freq(f) {
    return this.wrapX ? this.period(f) / this.w : f;
  }

  n2(mx, my, f, ox, oy) {
    return noise2p(mx * this.freq(f) + ox, my * f + oy, this.period(f));
  }

  fbm(mx, my, f, ox, oy, oct) {
    return fbm2p(mx * this.freq(f) + ox, my * f + oy, this.period(f), oct);
  }

  ridged(mx, my, f, ox, oy, oct) {
    return ridged2p(mx * this.freq(f) + ox, my * f + oy, this.period(f), oct);
  }

  /** The rounded domes of the hill plots near (mx, my) (soft sum: overlapping domes merge). */
  domes(mx, my) {
    let s = 0;
    const cx = Math.floor(mx);
    const cy = Math.floor(my);
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const i = this.index(cx + dx, cy + dy);
        if (i < 0 || (this.kind[i] !== KIND.HILLS && this.kind[i] !== KIND.PEAK)) continue;
        const px = cx + dx + 0.5 + this.peakJitter[i * 2] * 1.6;
        const py = cy + dy + 0.5 + this.peakJitter[i * 2 + 1] * 1.6;
        const t = clamp(1 - Math.hypot(mx - px, my - py) / 0.78, 0, 1);
        s += 0.34 * t * t * (3 - 2 * t);
      }
    }
    return s / (1 + s * 1.1);
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
        if (d > 0.9) continue;
        // Not a cone: the reach of the summit's flanks varies around it (two low harmonics, their
        // phases per plot), so spurs run out from the top with gullies between them.
        const a = Math.atan2(my - py, mx - px);
        const p1 = hashf(i, 71) * 6.2832, p2 = hashf(i, 72) * 6.2832;
        const reach = 0.62 * (1 + 0.3 * Math.sin(3 * a + p1) + 0.14 * Math.sin(5 * a + p2));
        const t = clamp(1 - d / reach, 0, 1);
        s = Math.max(s, 0.55 * Math.pow(t, 1.25));
        // A lower second summit on one shoulder: a lone mountain is a lopsided massif.
        const b = hashf(i, 73) * 6.2832;
        const d2 = Math.hypot(mx - (px + Math.cos(b) * 0.3), my - (py + Math.sin(b) * 0.3));
        const t2 = clamp(1 - d2 / 0.36, 0, 1);
        s = Math.max(s, 0.36 * Math.pow(t2, 1.1));
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
    // Subdivide before smoothing so even a one-edge reach (2 points, a straight bar otherwise) gets
    // interior points that chaikin and meander can bend (F18).
    this.rivers = lines.map((l) => meander(chaikin(chaikin(subdivide(l, 0.3)))));
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

/** Insert interior points so each segment is about `step` long (keeps the end points), so short
 *  reaches have points to bend. */
function subdivide(pts, step) {
  if (pts.length < 2) return pts;
  const out = [pts[0]];
  for (let i = 0; i + 1 < pts.length; i++) {
    const a = pts[i], b = pts[i + 1];
    const n = Math.max(1, Math.round(Math.hypot(b[0] - a[0], b[1] - a[1]) / step));
    for (let k = 1; k <= n; k++) out.push([a[0] + ((b[0] - a[0]) * k) / n, a[1] + ((b[1] - a[1]) * k) / n]);
  }
  return out;
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
    const o = 0.13 * noise2(p[0] * 1.7 + 0.5, p[1] * 1.7 - 0.5) + 0.04 * noise2(p[0] * 5.1 - 2.5, p[1] * 5.1 + 1.5);
    return [p[0] - ty * o, p[1] + tx * o];
  });
}

/** The world built from the current map (S.map), or null. */
export function worldOf(map = S.map) {
  return map ? new World(map) : null;
}
