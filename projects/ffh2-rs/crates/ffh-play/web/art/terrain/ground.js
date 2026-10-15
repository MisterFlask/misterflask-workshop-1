// terrain/ground.js: the terrain set's ground pass (docs/ART.md 5.1, 5.3, 5.5, 5.6, 8.2), drawn over
// art.js's seed versions of the same functions. loadArt() installs it (art.js PASSES); by hand:
//
//   import { installTerrain } from './art/terrain/ground.js';
//   const art = await loadArt({ passes: false });
//   installTerrain(art);                      // once; idempotent. art.drawTerrain etc. then use it.
//
// What it changes, against the seed drawing (the art director's terrain review, round 2):
//
//  classes  a pixel no longer takes the tile its warped position falls in (which keeps every tile a
//           warped square with four corners); it takes the class (terrain type and water) with the
//           largest sum of Gaussian weights (sigma 13 px) of the 3x3 tile centres around it, sampled
//           at the warped position. A lone tile of one terrain amid another becomes a blob (a disc
//           of about 15 px radius, displaced by the warp), runs of tiles merge, junctions round off.
//  warp     a third, low-frequency octave (cell 44 px) under the 22 px and 7 px ones, |w| <= 10 px,
//           so long runs of tile edges wander. It is evaluated exactly on a 4 px map lattice and
//           bilinearly in between: the same value for a map pixel whichever tile asks.
//  feather  between two land textures each pixel's lookup is offset by an ordered-dither vector of
//           up to 1.5 px (4x4 Bayer, a second phase for y), so a 3 px band of every edge is
//           dithered and edges feather like an ink wash. Shorelines and the depth contour stay crisp.
//  fog      seen and remembered are classified the same way among revealed tiles (same warp, same
//           feather), so the fog edge is organic and follows the land's edges; the unrevealed edge
//           has its own warp, a 4 px feather, and is ragged both ways (unrevealed tiles show their
//           revealed neighbours' ground where the edge gives it to them).
//  verges   rivers meander inside the wider warp (11 px pad), and rivers and roads get a pale verge:
//           a 50% checker one step lighter on the ground beside the ink (the board's pale paths).
//  hills    humps are never mirrored (the moon stays in the north-west). On a wooded hill, trees
//           standing on a hump's lit side are lifted onto its crest (three at most, 6 px apart),
//           and no other tree may hide a hump's body or dark flank: the mound reads under the wood.
//  woods    the north-west rim of every wood mass gets a 1 px moonlit edge (lichen O) where open
//           ground lies above or to the left; the mass is the trees of this tile and the tiles drawn
//           before it, closed over gaps up to about 6 px, so tile rows leave no seams.
import { TILE, PLOT, hash3 } from '../art.js';
import { TRANSPARENT } from '../spritefmt.js';

const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map((v) => (v + 0.5) / 16);
const bayer = (x, y) => BAYER[(y & 3) * 4 + (x & 3)];
const DIRS8 = [[0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1]];

/** Tunables of the ground pass (art px). */
export const GROUND = {
  octaves: [[44, 4.5, 5, 6], [22, 5.5, 1, 3], [7, 1.5, 2, 4]], // [cell, amplitude, seed offset x, seed offset y]
  maxWarp: 10,
  sigma: 13, // the class kernel (Gaussian) around each tile centre
  feather: 1.5, // half-width of the dithered band between two land textures
  fogFeather: 1.5,
  voidFeather: 2,
  riverPad: 11,
  verge: 1.6, // road verge half-width
  rimKey: 'O', // the moonlit rim of woods
  massGap: 3, // gaps narrower than about twice this between trees still count as one wood
  crestTrees: 3, // trees standing on one hump's crest (wooded hills)
  crestGap: 6, // their least spacing
};

// one step lighter within the family, for ground colours only (water, ink and lights unchanged)
const VERGE = 'q:m m:o o:O k:t t:T u:Q Q:y x:y y:Y Y:7 1:2 2:3 3:4 4:5 5:6 6:7 g:f v:p';
// scattered features whose wood masses take the moonlit rim
const RIMMED = new Set(['feature/conifer', 'feature/ancient', 'feature/jungle', 'feature/sapling']);

export function installTerrain(art, opts = {}) {
  if (art._terrainPass) return art;
  const G = { ...GROUND, ...opts };
  const orig = { drawRoad: art.drawRoad.bind(art), drawFeature: art.drawFeature.bind(art), drawRelief: art.drawRelief.bind(art) };
  const verge = new Uint8Array(64).map((_, i) => i);
  for (const p of VERGE.split(' ')) { const [a, b] = p.split(':'); verge[art.K(a)] = art.K(b); }
  const kO = art.K(G.rimKey), k0 = art.K('0'), ku = art.K('u'), k2 = art.K('2');
  const spotCache = new Map();
  const rimCache = new Map();
  const fogCache = new Map();
  const vdCache = new Map();
  art._terrainCaches = () => { spotCache.clear(); rimCache.clear(); fogCache.clear(); vdCache.clear(); };
  const clip = (v, m) => Math.max(-m, Math.min(m, v));

  // ---- the warp: value noise as art.js's _noise (same lattice hash), three octaves, no allocation
  const W2 = new Float64Array(2);
  const octs = G.octaves.map(([cell, amp, sx, sy]) => ({ cell, amp, sx, sy }));
  const lat = (i, j, z) => hash3(i, j, z) / 4294967296;
  function warpInto(X, Y, seed) {
    const wpx = art.world.wrapX && art.world.width ? art.world.width * TILE : 0;
    let wx = 0, wy = 0;
    for (let o = 0; o < octs.length; o++) {
      const { cell, amp, sx, sy } = octs[o];
      let c = cell, per = 0;
      if (wpx) { per = Math.max(1, Math.round(wpx / cell)); c = wpx / per; }
      const fx = X / c, fy = Y / c;
      let ix = Math.floor(fx);
      const iy = Math.floor(fy);
      const tx = fx - ix, ty = fy - iy;
      const ux = tx * tx * (3 - 2 * tx), uy = ty * ty * (3 - 2 * ty);
      let ix1 = ix + 1;
      if (per) { ix = ((ix % per) + per) % per; ix1 = ((ix1 % per) + per) % per; }
      const z1 = seed + sx, z2 = seed + sy;
      let a = lat(ix, iy, z1), b = lat(ix1, iy, z1), cc = lat(ix, iy + 1, z1), d = lat(ix1, iy + 1, z1);
      wx += amp * ((a + (b - a) * ux + (cc - a) * uy + (a - b - cc + d) * ux * uy) * 2 - 1);
      a = lat(ix, iy, z2); b = lat(ix1, iy, z2); cc = lat(ix, iy + 1, z2); d = lat(ix1, iy + 1, z2);
      wy += amp * ((a + (b - a) * ux + (cc - a) * uy + (a - b - cc + d) * ux * uy) * 2 - 1);
    }
    W2[0] = clip(wx, G.maxWarp); W2[1] = clip(wy, G.maxWarp);
  }

  // The warp is evaluated exactly on a 4 px map lattice and bilinearly in between (the finest
  // octave has a 7 px cell): the same value for a map pixel whichever tile asks, at a 16th the cost.
  const STEP = 4;
  function warpRegion(X0, Y0, N, seed) {
    const gx0 = Math.floor(X0 / STEP), gy0 = Math.floor(Y0 / STEP);
    const gw = Math.floor((X0 + N - 1) / STEP) - gx0 + 2, gh = Math.floor((Y0 + N - 1) / STEP) - gy0 + 2;
    const lx = new Float64Array(gw * gh), ly = new Float64Array(gw * gh);
    for (let b = 0; b < gh; b++) for (let a = 0; a < gw; a++) {
      warpInto((gx0 + a) * STEP, (gy0 + b) * STEP, seed);
      lx[b * gw + a] = W2[0]; ly[b * gw + a] = W2[1];
    }
    const wx = new Float64Array(N * N), wy = new Float64Array(N * N);
    for (let j = 0; j < N; j++) {
      const Y = Y0 + j, b = Math.floor(Y / STEP) - gy0, ty = (Y - (gy0 + b) * STEP) / STEP;
      for (let i = 0; i < N; i++) {
        const X = X0 + i, a = Math.floor(X / STEP) - gx0, tx = (X - (gx0 + a) * STEP) / STEP;
        const p = b * gw + a;
        const k = j * N + i;
        wx[k] = (lx[p] * (1 - tx) + lx[p + 1] * tx) * (1 - ty) + (lx[p + gw] * (1 - tx) + lx[p + gw + 1] * tx) * ty;
        wy[k] = (ly[p] * (1 - tx) + ly[p + 1] * tx) * (1 - ty) + (ly[p + gw] * (1 - tx) + ly[p + gw + 1] * tx) * ty;
      }
    }
    return { wx, wy };
  }

  /** The domain warp at map art pixel (X, Y): three octaves of value noise, |w| <= maxWarp. */
  art._warpRaw = function (X, Y, seed) { const r = warpRegion(X, Y, 1, seed); return [r.wx[0], r.wy[0]]; };

  // ---- the class kernel, separable: exp(-(dx^2 + dy^2) / 2s^2) = E(dx) E(dy), E from a table at 1/8 px
  const LUT_RES = 8, LUT_N = 160 * LUT_RES;
  const LUT = new Float64Array(LUT_N);
  for (let i = 0; i < LUT_N; i++) { const t = i / LUT_RES; LUT[i] = Math.exp(-(t * t) / (2 * G.sigma * G.sigma)); }
  const E = (t) => { const i = (Math.abs(t) * LUT_RES) | 0; return i < LUT_N ? LUT[i] : 0; };

  /**
   * Per-pixel source cells (0..8 of the 3x3 neighbourhood) of a padded tile region. `cls` gives
   * each neighbour's class (terrain type, visibility...; null: takes no part). A pixel takes the
   * class with the largest sum of Gaussian weights of its tiles' centres, sampled through the warp:
   * one tile of a class amid another becomes a blob rather than a square, and junctions round off.
   * `crisp` is the class through the warp alone, `soft` with an ordered-dither offset of half-width
   * `f` (the feather). Both hold the index of a neighbour of the winning class.
   */
  function cells(plot, pad, seed, cls, f, phase = 0) {
    const N = TILE + 2 * pad;
    const crisp = new Uint8Array(N * N), soft = new Uint8Array(N * N);
    const uniq = [...new Set(cls.filter((c) => c !== null))];
    if (uniq.length === 1 && cls[4] !== null) { crisp.fill(4); soft.fill(4); return { crisp, soft }; }
    const ci = Int8Array.from(cls.map((c) => (c === null ? -1 : uniq.indexOf(c))));
    const rep = uniq.map((c) => (cls[4] === c ? 4 : cls.indexOf(c)));
    const nc = uniq.length;
    const sums = new Float64Array(nc);
    const best = (u, v) => {
      const x0 = E(u + 16), x1 = E(u - 16), x2 = E(u - 48);
      const y0 = E(v + 16), y1 = E(v - 16), y2 = E(v - 48);
      sums.fill(0);
      if (ci[0] >= 0) sums[ci[0]] += x0 * y0;
      if (ci[1] >= 0) sums[ci[1]] += x1 * y0;
      if (ci[2] >= 0) sums[ci[2]] += x2 * y0;
      if (ci[3] >= 0) sums[ci[3]] += x0 * y1;
      if (ci[4] >= 0) sums[ci[4]] += x1 * y1;
      if (ci[5] >= 0) sums[ci[5]] += x2 * y1;
      if (ci[6] >= 0) sums[ci[6]] += x0 * y2;
      if (ci[7] >= 0) sums[ci[7]] += x1 * y2;
      if (ci[8] >= 0) sums[ci[8]] += x2 * y2;
      let b = 0;
      for (let c = 1; c < nc; c++) if (sums[c] > sums[b]) b = c;
      return rep[b];
    };
    const X0 = plot.x * TILE - pad, Y0 = plot.y * TILE - pad;
    const { wx, wy } = warpRegion(X0, Y0, N, seed);
    for (let j = 0; j < N; j++) for (let i = 0; i < N; i++) {
      const X = X0 + i, Y = Y0 + j;
      const k = j * N + i;
      const u = i - pad + wx[k], v = j - pad + wy[k];
      const c = best(u, v);
      crisp[k] = c;
      if (!f) { soft[k] = c; continue; }
      const du = f * (2 * bayer(X + phase, Y) - 1), dv = f * (2 * bayer(Y + 1, X + 2 + phase) - 1);
      soft[k] = best(u + du, v + dv);
    }
    return { crisp, soft };
  }

  /** Ground indices of a tile (32x32): feathered textures, shoreline ink, foam, depth contour. */
  art.groundIndices = function (plot) {
    const nb = this._nbhd(plot, true);
    const infos = nb.map((p) => this._terrain(p.terrain));
    const water = nb.map((p) => this._isWater(p));
    const key = `${plot.x},${plot.y}|` + nb.map((p, k) => infos[k].type + (water[k] ? 'w' : '')).join(',');
    let out = this._ground.get(key);
    if (out) return out;
    if (this._ground.size > 8192) this._ground.clear();
    const texel = (t, X, Y) => t.data[(((Y % t.h) + t.h) % t.h) * t.w + (((X % t.w) + t.w) % t.w)];
    out = new Uint8Array(TILE * TILE);
    if (infos.every((t, k) => t === infos[4] && water[k] === water[4])) {
      const t = infos[4].tex;
      for (let j = 0; j < TILE; j++) for (let i = 0; i < TILE; i++) out[j * TILE + i] = texel(t, plot.x * TILE + i, plot.y * TILE + j);
      this._ground.set(key, out);
      return out;
    }
    const shore = water.some((w) => w);
    const PAD = 3, N = TILE + 2 * PAD;
    const { crisp, soft } = cells(plot, PAD, 100, nb.map((p, k) => infos[k].type + (water[k] ? 'w' : '')), G.feather);
    const cls = new Uint8Array(N * N); // 0 land, 1 shallow water, 2 deep water
    for (let k = 0; k < N * N; k++) { const c = crisp[k]; cls[k] = water[c] ? (infos[c].deep ? 2 : 1) : 0; }
    const C = (i, j) => cls[(j + PAD) * N + i + PAD];
    const k7 = this.K('7'), k6 = this.K('6'), kN = this.K('N'), kg = this.K('g');
    for (let j = 0; j < TILE; j++) for (let i = 0; i < TILE; i++) {
      const X = plot.x * TILE + i, Y = plot.y * TILE + j;
      const k = (j + PAD) * N + i + PAD;
      let c = crisp[k];
      // feather only between two land textures: water edges keep their ink
      if (soft[k] !== c && !water[c] && !water[soft[k]]) c = soft[k];
      let v = texel(infos[c].tex, X, Y);
      const me = C(i, j);
      if (!shore) { /* no water near: texture only */ } else if (me === 0) {
        if (C(i - 1, j) || C(i + 1, j) || C(i, j - 1) || C(i, j + 1)) v = (X + Y) % 5 === 0 ? k6 : k7;
        else {
          let near = false;
          for (let dj = -2; dj <= 2 && !near; dj++) for (let di = -2; di <= 2; di++) if (C(i + di, j + dj)) { near = true; break; }
          if (near) v = k2;
        }
      } else {
        let land = false;
        for (let dj = -2; dj <= 2 && !land; dj++) for (let di = -2; di <= 2; di++) if (!C(i + di, j + dj)) { land = true; break; }
        if (land) { if (((X + Y) & 1) === 0) v = kN; }
        else if (me === 2 && (C(i - 1, j) === 1 || C(i + 1, j) === 1 || C(i, j - 1) === 1 || C(i, j + 1) === 1)) {
          if ((X + Y) % 4 === 0) v = kg;
        }
      }
      out[j * TILE + i] = v;
    }
    this._ground.set(key, out);
    return out;
  };

  /** Fog pass: FOG with ink hatching on fogged pixels, void on unrevealed; both edges feathered. */
  art.drawFog = function (ctx, x, y, plot) {
    const nb = this._nbhd(plot);
    const vis = nb.map((p) => (p.vis === undefined ? 2 : p.vis));
    if (vis.every((v) => v === 2)) return;
    if (vis.every((v) => v === 0)) { ctx.fillStyle = this.css('0'); ctx.fillRect(x, y, TILE * this.scale, TILE * this.scale); return; }
    const ck = `${plot.x},${plot.y},${this.world.width},${this.world.wrapX}|${vis.join('')}`;
    let fc = fogCache.get(ck);
    if (!fc) {
      if (fogCache.size > 4096) fogCache.clear();
      // seen and remembered are judged among revealed tiles only; the void edge separately
      fc = { fog: cells(plot, 0, 100, vis.map((v) => (v ? v : null)), G.fogFeather).soft, rev: revealCells(plot, vis) };
      fogCache.set(ck, fc);
    }
    const FOG = this.tables.FOG;
    const X0 = plot.x * TILE, Y0 = plot.y * TILE;
    this.remapRegion(ctx, x, y, TILE, TILE, (i, j, c) => {
      const k = j * TILE + i;
      if (vis[fc.rev[k]] === 0) return k0;
      if (vis[fc.fog[k]] === 1) {
        const v = FOG[c];
        return (X0 + i + Y0 + j) % 4 === 0 ? FOG[v] : v;
      }
      return c;
    });
  };

  function revealCells(plot, vis) { return cells(plot, 0, 300, vis.map((v) => (v ? 1 : 0)), G.voidFeather, 1).soft; }

  /**
   * Ground of a tile. An unrevealed tile next to revealed ones shows their ground where the
   * feathered void edge (seed 300) gives the pixel to them, so the edge is ragged both ways.
   */
  art.drawTerrain = function (ctx, x, y, plot) {
    if (plot.vis !== 0) { this.blit(ctx, x, y, this.groundIndices(plot), TILE, TILE); return; }
    const nb = this._nbhd(plot);
    const vis = nb.map((p) => (p.vis === undefined ? 2 : p.vis));
    if (vis.every((v) => v === 0)) { ctx.fillStyle = this.css('0'); ctx.fillRect(x, y, TILE * this.scale, TILE * this.scale); return; }
    const rev = revealCells(plot, vis);
    const infos = nb.map((p) => this._terrain(p.terrain));
    const water = nb.map((p) => this._isWater(p));
    const { soft } = cells(plot, 0, 100, nb.map((p, k) => (vis[k] ? infos[k].type + (water[k] ? 'w' : '') : null)), G.feather);
    const out = new Uint8Array(TILE * TILE);
    for (let j = 0; j < TILE; j++) for (let i = 0; i < TILE; i++) {
      const k = j * TILE + i;
      if (!vis[rev[k]] || !vis[soft[k]]) { out[k] = k0; continue; }
      const t = infos[soft[k]].tex, X = plot.x * TILE + i, Y = plot.y * TILE + j;
      out[k] = t.data[(((Y % t.h) + t.h) % t.h) * t.w + (((X % t.w) + t.w) % t.w)];
    }
    this.blit(ctx, x, y, out, TILE, TILE);
  };

  /** River ink on the south and east edges (as art.js), inside the wider warp, with a pale verge. */
  art.drawRiver = function (ctx, x, y, plot, edges = null) {
    const bits = plot.river | 0;
    const S = edges ? !!edges.s : !!(bits & 1), E = edges ? !!edges.e : !!(bits & 2);
    if (!S && !E) return;
    const PAD = G.riverPad, N = TILE + 2 * PAD, s = this.scale;
    const nb = this._nbhd(plot, true);
    const water = nb.map((p) => this._isWater(p));
    const src = cells(plot, PAD, 100, water, 0).crisp;
    const rw = warpRegion(plot.x * TILE - PAD, plot.y * TILE - PAD, N, 100);
    const out = new Uint8Array(N * N).fill(TRANSPARENT);
    const bank = new Uint8Array(N * N);
    const kn = this.K('n'), kg = this.K('g'), k1 = this.K('1');
    const X0 = plot.x * TILE, Y0 = plot.y * TILE;
    let banks = false;
    for (let j = 0; j < N; j++) for (let i = 0; i < N; i++) {
      if (water[src[j * N + i]]) continue;
      const X = X0 + i - PAD, Y = Y0 + j - PAD;
      const wx = rw.wx[j * N + i], wy = rw.wy[j * N + i];
      const px = X + 0.5 + 0.8 * wx - X0, py = Y + 0.5 + 0.8 * wy - Y0;
      let best = null;
      if (S && px >= -1 && px <= TILE + 1) { const d = py - TILE; best = { d, t: px }; }
      if (E && py >= -1 && py <= TILE + 1) { const d = px - TILE; if (!best || Math.abs(d) < Math.abs(best.d)) best = { d, t: py }; }
      if (!best) continue;
      const ad = Math.abs(best.d);
      if (ad < 1) out[j * N + i] = Math.floor(best.t + 600) % 6 === 0 && best.d < 0 ? kg : kn;
      else if (ad < 2) out[j * N + i] = k1;
      else if (ad < 3.2 && ((X + Y) & 1) === 0) { bank[j * N + i] = 1; banks = true; }
    }
    if (banks) this.remapRegion(ctx, x - PAD * s, y - PAD * s, N, N, (i, j, c) => (bank[j * N + i] ? verge[c] : c));
    this.blit(ctx, x - PAD * s, y - PAD * s, out, N, N);
  };

  /** Road: a pale verge (50% checker one step lighter) under art.js's dotted ink. */
  art.drawRoad = function (ctx, x, y, plot, dirs = null) {
    const type = this.typeName('route', plot.route);
    if (!type && !plot.city) return;
    const has = dirs || DIRS8.map(([dx, dy]) => { const n = plot.at?.(dx, dy); return !!n && n.vis !== 0 && (!!this.typeName('route', n.route) || !!n.city); });
    const segs = [];
    has.forEach((on, k) => { if (on) segs.push(DIRS8[k]); });
    const ax = 16, ay = 18, R = G.verge;
    const X0 = plot.x * TILE, Y0 = plot.y * TILE;
    this.remapRegion(ctx, x, y, TILE, TILE, (i, j, c) => {
      if (((X0 + i + Y0 + j) & 1) !== 0) return c;
      let d = Math.hypot(i - ax, j - ay);
      for (const [dx, dy] of segs) {
        const ex = dx * 16, ey = dy * 16, L2 = ex * ex + ey * ey;
        const t = Math.max(0, Math.min(1, ((i - ax) * ex + (j - ay) * ey) / L2));
        d = Math.min(d, Math.hypot(i - ax - t * ex, j - ay - t * ey));
      }
      return d <= R ? verge[c] : c;
    });
    return orig.drawRoad(ctx, x, y, plot, dirs);
  };

  // ---------------------------------------------------------------- woods

  /** Hump outlines of a hill plot (as art.js drawRelief places them): [{x0, y0, w, h, top[]}] in tile px. */
  function humps(plot) {
    const terrain = art.typeName('terrain', plot.terrain);
    const r = art.mapper.resolve('hill', terrain);
    if (!r || r.sprite === 'none') return [];
    const vs = art.variants(r.sprite);
    const h = hash3(plot.x, plot.y, 7);
    const spots = [[10 + (h % 3), 19 + ((h >>> 3) % 3), 0], [21 + ((h >>> 5) % 3), 27 + ((h >>> 7) % 2), 1]];
    return spots.map(([px, py, k]) => {
      const spr = vs[(k + (h >>> 9)) % vs.length];
      const top = [];
      for (let c = 0; c < spr.w; c++) {
        let t = -1;
        for (let rr = 0; rr < spr.h; rr++) if (spr.data[rr * spr.w + c] !== TRANSPARENT) { t = rr; break; }
        top.push(t);
      }
      return { x0: px - spr.ax, y0: py - spr.ay, w: spr.w, h: spr.h, top, spr };
    });
  }

  /** Hills (two humps, never mirrored: the moon is in the north-west) and peaks, as art.js. */
  art.drawRelief = function (ctx, x, y, plot) {
    const pt = this._plotType(plot);
    if (pt !== PLOT.HILLS) return orig.drawRelief(ctx, x, y, plot);
    const r = this.mapper.resolve('hill', this.typeName('terrain', plot.terrain));
    if (!r || r.sprite === 'none') return;
    const s = this.scale;
    for (const hm of humps(plot)) this.drawSprite(ctx, x + (hm.x0 + hm.spr.ax) * s, y + (hm.y0 + hm.spr.ay) * s, hm.spr.name, { remap: r.opts.remap });
  };

  /**
   * Tree placement of a scattered feature (the jittered grid of art.js), then, on hills, trees
   * standing on a hump lifted onto its crest line and thinned to two per hump. Cached per plot.
   */
  art._woodSpots = function (plot, type = plot.feature) {
    const r = this.resolve('feature', type);
    if (!r || r.sprite === 'none' || !r.opts.scatter) return null;
    const pt = this._plotType(plot);
    const key = `${plot.x},${plot.y},${r.sprite},${pt},${pt === PLOT.HILLS ? this.typeName('terrain', plot.terrain) : ''}`;
    let spots = spotCache.get(key);
    if (spots) return spots;
    if (spotCache.size > 8192) spotCache.clear();
    const [a, b] = String(r.opts.scatter).split('-').map(Number);
    const h = hash3(plot.x, plot.y, 11);
    const n = a + (h % ((b || a) - a + 1));
    const order = [...Array(12).keys()].sort((p, q) => hash3(plot.x, plot.y, 50 + p) - hash3(plot.x, plot.y, 50 + q));
    spots = [];
    for (let k = 0; k < n; k++) {
      const cell = order[k % 12];
      const cx = cell % 4, cy = Math.floor(cell / 4);
      const jx = hash3(plot.x, plot.y, 70 + k) % 7, jy = hash3(plot.x, plot.y, 90 + k) % 6;
      const spr = this.pick(r.sprite, hash3(plot.x, plot.y, 30 + k));
      if (!spr) continue;
      spots.push({ px: 2 + cx * 7 + jx, py: 11 + cy * 7 + jy, spr, mirror: (hash3(plot.x, plot.y, 110 + k) & 1) === 1 });
    }
    if (pt === PLOT.HILLS) {
      // a wooded hill: the mounds stay readable. Trees standing on a hump's lit side are lifted onto
      // its crest (at most G.crestTrees per hump, G.crestGap px apart); others standing on a hump,
      // or whose crown would hide a hump's body (below the crest, flank included), are left out.
      const hs = humps(plot);
      const kept = [];
      const perHump = hs.map(() => []);
      const box = (sp) => {
        const x0 = sp.mirror ? sp.px - (sp.spr.w - 1 - sp.spr.ax) : sp.px - sp.spr.ax;
        return [x0, sp.py - sp.spr.ay, x0 + sp.spr.w - 1, sp.py - sp.spr.ay + sp.spr.h - 1];
      };
      for (const sp of spots) {
        let on = -1;
        hs.forEach((hm, i) => {
          const c = sp.px - hm.x0;
          if (on < 0 && c >= 2 && c < hm.w - 2 && hm.top[c] >= 0 && sp.py >= hm.y0 + hm.top[c] - 3 && sp.py <= hm.y0 + hm.h + 1) on = i;
        });
        if (on >= 0) {
          const hm = hs[on];
          const c = sp.px - hm.x0;
          if (c > hm.w * 0.62) continue; // the dark flank stays clear
          const lifted = { ...sp, py: hm.y0 + hm.top[c] - 1 };
          if (perHump[on].length < G.crestTrees && perHump[on].every((o) => Math.abs(o.px - lifted.px) >= G.crestGap)) perHump[on].push(lifted);
          continue;
        }
        const [bx0, by0, bx1, by1] = box(sp);
        const hides = hs.some((hm) => bx1 >= hm.x0 + 1 && bx0 <= hm.x0 + hm.w - 2 && by1 >= hm.y0 + 3 && by0 <= hm.y0 + hm.h - 1);
        if (!hides) kept.push(sp);
      }
      spots = kept.concat(...perHump);
    }
    spots.sort((p, q) => p.py - q.py);
    spotCache.set(key, spots);
    return spots;
  };

  /**
   * The moonlit rim of a wood: lichen pixels on the north-west edge of the tile's trees where open
   * ground lies above or to the left, judged over the trees of the tiles drawn before this one
   * (the row to the north, the tile to the west). Returns { x0, y0, w, h, data } or null.
   */
  art._woodRim = function (plot) {
    const rr = this.resolve('feature', plot.feature);
    if (!rr || !RIMMED.has(rr.sprite)) return null;
    const sig = [];
    const tiles = [[-1, -1], [0, -1], [1, -1], [-1, 0], [0, 0]];
    for (const [dx, dy] of tiles) {
      const n = dx || dy ? plot.at?.(dx, dy) : plot;
      sig.push(n && n.vis && n.feature != null ? `${this.typeName('feature', n.feature)}/${this._plotType(n)}/${this.typeName('terrain', n.terrain)}` : '-');
    }
    const key = `${plot.x},${plot.y}|${sig.join(',')}`;
    if (rimCache.has(key)) return rimCache.get(key);
    if (rimCache.size > 4096) rimCache.clear();
    const x0 = -16, y0 = -18, W = TILE + 32, H = TILE + 24;
    const idx = new Uint8Array(W * H).fill(TRANSPARENT);
    const mine = new Uint8Array(W * H);
    tiles.forEach(([dx, dy]) => {
      const n = dx || dy ? plot.at?.(dx, dy) : plot;
      if (!n || !n.vis || n.feature == null) return;
      const spots = this._woodSpots(n);
      if (!spots) return;
      const r = this.resolve('feature', n.feature);
      const own = !dx && !dy;
      for (const sp of spots) {
        const vk = `${sp.spr.name}|${r.opts.remap || ''}`;
        let vd = vdCache.get(vk);
        if (!vd) { vd = this.variantData(sp.spr, { remap: r.opts.remap }); vdCache.set(vk, vd); }
        for (let j = 0; j < vd.h; j++) {
          const ty = dy * TILE + sp.py + j - vd.ay - y0;
          if (ty < 0 || ty >= H) continue;
          for (let i = 0; i < vd.w; i++) {
            const c = vd.data[j * vd.w + i];
            if (c === TRANSPARENT) continue;
            const tx = dx * TILE + sp.px + (sp.mirror ? vd.ax - i : i - vd.ax) - x0;
            if (tx < 0 || tx >= W) continue;
            idx[ty * W + tx] = c;
            mine[ty * W + tx] = own ? 1 : 0;
          }
        }
      }
    });
    let out = null;
    const leaf = (c) => c !== TRANSPARENT && c !== k0 && c !== ku && c !== k2;
    // the wood as a mass: the tree mask closed (dilate, then erode, by a (2R+1) square, separably)
    // over gaps up to about 2R px, so only the mass's outer edge is lit
    const R = G.massGap;
    const pass = (src, horiz, any) => {
      const dst = new Uint8Array(W * H);
      for (let j = 0; j < H; j++) for (let i = 0; i < W; i++) {
        let v = any ? 0 : 1;
        for (let d = -R; d <= R; d++) {
          const ii = horiz ? i + d : i, jj = horiz ? j : j + d;
          const inb = ii >= 0 && jj >= 0 && ii < W && jj < H;
          const b = inb ? src[jj * W + ii] : 0;
          if (any && b) { v = 1; break; }
          if (!any && !b) { v = 0; break; }
        }
        dst[j * W + i] = v;
      }
      return dst;
    };
    const mask = new Uint8Array(W * H);
    for (let p = 0; p < W * H; p++) mask[p] = idx[p] !== TRANSPARENT ? 1 : 0;
    const closed = pass(pass(pass(pass(mask, true, true), false, true), true, false), false, false);
    for (let j = 2; j < H; j++) for (let i = 2; i < W; i++) {
      const p = j * W + i;
      if (!mine[p] || !leaf(idx[p])) continue;
      const up = idx[p - W] === k0 && !closed[p - 2 * W];
      const left = idx[p - 1] === k0 && !closed[p - 2];
      if (up || left) {
        if (!out) out = new Uint8Array(W * H).fill(TRANSPARENT);
        out[p] = kO;
      }
    }
    const res = out ? { x0, y0, w: W, h: H, data: out } : null;
    rimCache.set(key, res);
    return res;
  };

  /** Features: scattered woods through _woodSpots (hills) with the moonlit rim; others as art.js. */
  art.drawFeature = function (ctx, x, y, plot, type = plot.feature) {
    const r = this.resolve('feature', type);
    if (!r || r.sprite === 'none' || !r.opts.scatter || type !== plot.feature) return orig.drawFeature(ctx, x, y, plot, type);
    const s = this.scale;
    const v = { remap: r.opts.remap, table: r.opts.table };
    for (const sp of this._woodSpots(plot, type)) this.drawSprite(ctx, x + sp.px * s, y + sp.py * s, sp.spr.name, { ...v, mirror: sp.mirror });
    const rim = this._woodRim(plot);
    if (rim) this.blit(ctx, x + rim.x0 * s, y + rim.y0 * s, rim.data, rim.w, rim.h);
  };

  art._terrainPass = true;
  art._ground.clear();
  return art;
}
