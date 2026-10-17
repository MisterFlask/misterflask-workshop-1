// terrain/ground.js: the terrain set's ground pass (docs/ART.md 5.1, 5.3, 5.5, 5.6, 8.2), drawn over
// art.js's seed versions of the same functions. loadArt() installs it (art.js PASSES); by hand:
//
//   import { installTerrain } from './art/terrain/ground.js';
//   const art = await loadArt({ passes: false });
//   installTerrain(art);                      // once; idempotent. art.drawTerrain etc. then use it.
//
// What it changes, against the seed drawing (the art director's terrain review, round 2, and the
// light and ground review, round 3):
//
//  classes  a pixel no longer takes the tile its warped position falls in (which keeps every tile a
//           warped square with four corners); it takes the class (terrain type and water) with the
//           largest sum of Gaussian weights (sigma 13 px) of the 3x3 tile centres around it, sampled
//           at the warped position. A lone tile of one terrain amid another becomes a blob (a disc
//           of about 15 px radius, displaced by the warp), runs of tiles merge, junctions round off.
//           Each tile's weight is scaled by e^(+-G.bias) from a hash of its position, so a run of
//           edges does not follow the tile lattice exactly.
//  warp     a low-frequency octave (cell 46 px, amplitude 10, shaped toward its extremes so most
//           edges really wander) under the 22 px and 7 px ones, |w| <= 14 px. It is evaluated
//           exactly on a 4 px map lattice and bilinearly in between: the same value for a map pixel
//           whichever tile asks.
//  bands    where two land textures meet, the winner's share rises from 50% on the edge to 100%
//           G.band (4) px inside it, thresholded by the 4x4 Bayer matrix in map space: a 25/50/75%
//           ordered transition band rather than a line (round 3; round 2 offset each lookup by a
//           1.5 px ordered-dither vector, which left the squares readable). Shorelines and the depth
//           contour stay crisp.
//  moon     moonlit clearings and cloud shade: a low-frequency field (cells 88 and 30 px) lifts the
//           land one step (MOON) by a Bayer share up to 60% where it is high, adds a warm grain
//           (GRAIN, up to 30%) at its brightest, and lowers it one step (SHADE, up to 25%) where it
//           is low: the board's lighter open ground between dark woods, instead of flat mid-value
//           fields. Water, shoreline ink and unrevealed void are left alone.
//  fog      seen and remembered are classified the same way among revealed tiles (same warp, a 4 px
//           band), so the fog edge is organic and follows the land's edges; the unrevealed edge has
//           its own warp, a 3 px band, and is ragged both ways (unrevealed tiles show their revealed
//           neighbours' ground where the edge gives it to them). Fogged pixels go through REMEMBER:
//           docs/ART.md 2.4 FOG with the frost family sent to the neutrals, so remembered snow is
//           grey, not steel-blue. The vignette uses REMEMBER too.
//  paths    roads are pale paths: dashes of the route sprite (2 on, 2 off) along a line that sways
//           with a slow warp of its own (a function of the map position, so two tiles' halves meet),
//           over a worn verge one step lighter (50% within 1.6 px, 25% out to 2.2 px).
//  verges   rivers meander inside the wider warp (14 px pad) with a pale verge: a 50% checker one
//           step lighter on the ground beside the ink.
//  hills    humps are never mirrored (the moon stays in the north-west). On a wooded hill, trees
//           standing on a hump's lit side are lifted onto its crest (three at most, 6 px apart),
//           and no other tree may hide a hump's body or dark flank: the mound reads under the wood.
//  woods    the north-west rim of every wood mass gets a 1 px moonlit edge (lichen O) where open
//           ground lies above or to the left; the mass is the trees of this tile and the tiles drawn
//           before it, closed over gaps up to about 6 px, so tile rows leave no seams.
//
// Its tables (VERGE, MOON, GRAIN, SHADE, REMEMBER) are palette.txt remap columns (docs/ART.md 2.4).
import { TILE, PLOT, hash3 } from '../art.js';
import { TRANSPARENT } from '../spritefmt.js';

const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map((v) => (v + 0.5) / 16);
const bayer = (x, y) => BAYER[(y & 3) * 4 + (x & 3)];
const DIRS8 = [[0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1]];

/** Tunables of the ground pass (art px). */
export const GROUND = {
  // [cell, amplitude, seed offset x, seed offset y, gamma]: the 46 px octave bends long runs of tile
  // edges; its gamma (< 1) pushes the noise toward its extremes, so most edges really wander
  octaves: [[46, 10, 5, 6, 0.6], [22, 4.5, 1, 3, 1], [7, 1.5, 2, 4, 1]],
  maxWarp: 14,
  bias: 0.5, // each tile's class weight times exp(bias x a hash in [-1, 1]): edges shift +-2.6 px per tile
  sigma: 13, // the class kernel (Gaussian) around each tile centre
  band: 4, // half-width of the Bayer-dithered transition band between two land textures
  fogBand: 4, // ... between seen and remembered
  voidBand: 3, // ... between revealed and unrevealed
  riverPad: 14,
  verge: 2.2, // half-width of a path's pale verge
  path: { dash: 2, gap: 2, sway: 0.55 }, // pale paths: dash and gap lengths, sway (x the warp)
  // moonlit clearings and cloud shade: low-frequency noise (cells in px, weights) lifting the ground
  // one step where it is above `lit`, lowering it below `shade`, by a Bayer share rising over `ramp`
  // to `max` (`shadeMax`); `grain` above `lit` a share up to `grainMax` takes the pale mark colour
  moon: { cells: [[88, 0.65], [30, 0.35]], lit: -0.06, shade: -0.38, ramp: 0.35, max: 0.4, shadeMax: 0.15, grain: 0.16, grainMax: 0.15 },
  rimKey: 'O', // the sunlit rim of woods
  unseenKey: '1', // unrevealed ground: the interface's navy, not a black hole
  vignette: false, // daylight: no candle-lit page edge
  massGap: 3, // gaps narrower than about twice this between trees still count as one wood
  crestTrees: 3, // trees standing on one hump's crest (wooded hills)
  crestGap: 6, // their least spacing
};

// scattered features whose wood masses take the moonlit rim
const RIMMED = new Set(['feature/conifer', 'feature/ancient', 'feature/jungle', 'feature/sapling']);

export function installTerrain(art, opts = {}) {
  if (art._terrainPass) return art;
  const G = { ...GROUND, ...opts };
  const orig = { drawRoad: art.drawRoad.bind(art), drawFeature: art.drawFeature.bind(art), drawRelief: art.drawRelief.bind(art) };
  // the pass's tables are palette.txt remap columns (docs/ART.md 2.4)
  const { VERGE: verge, MOON: moonT, GRAIN: grainT, SHADE: shadeT, REMEMBER: remember } = art.tables;
  for (const n of ['VERGE', 'MOON', 'GRAIN', 'SHADE', 'REMEMBER']) if (!art.tables[n]) throw new Error(`terrain: palette.txt lacks remap table ${n}`);
  const kO = art.K(G.rimKey), k0 = art.K('0'), ku = art.K('u'), k2 = art.K('2'), kUnseen = art.K(G.unseenKey);
  const spotCache = new Map();
  const rimCache = new Map();
  const fogCache = new Map();
  const vdCache = new Map();
  art._terrainCaches = () => { spotCache.clear(); rimCache.clear(); fogCache.clear(); vdCache.clear(); };
  const clip = (v, m) => Math.max(-m, Math.min(m, v));

  // ---- the warp: value noise as art.js's _noise (same lattice hash), three octaves, no allocation
  const W2 = new Float64Array(2);
  const octs = G.octaves.map(([cell, amp, sx, sy, gamma = 1]) => ({ cell, amp, sx, sy, gamma }));
  const shape = (n, g) => (g === 1 ? n : Math.sign(n) * Math.abs(n) ** g);
  const lat = (i, j, z) => hash3(i, j, z) / 4294967296;
  function warpInto(X, Y, seed) {
    const wpx = art.world.wrapX && art.world.width ? art.world.width * TILE : 0;
    let wx = 0, wy = 0;
    for (let o = 0; o < octs.length; o++) {
      const { cell, amp, sx, sy, gamma } = octs[o];
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
      wx += amp * shape((a + (b - a) * ux + (cc - a) * uy + (a - b - cc + d) * ux * uy) * 2 - 1, gamma);
      a = lat(ix, iy, z2); b = lat(ix1, iy, z2); cc = lat(ix, iy + 1, z2); d = lat(ix1, iy + 1, z2);
      wy += amp * shape((a + (b - a) * ux + (cc - a) * uy + (a - b - cc + d) * ux * uy) * 2 - 1, gamma);
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
   * Each tile's weight is scaled by a hash of its position (G.bias), so a run of edges does not
   * follow the tile lattice exactly. `crisp` is that class; `soft` dithers it with the runner-up across a band of half-width `band`
   * px: the share of the winner rises from 50% on the edge to 100% `band` px inside, thresholded by
   * the 4x4 Bayer matrix in map space, so an edge is a 25/50/75% ordered transition, not a line.
   * Both hold the index of a neighbour of the class.
   */
  // the log ratio of two adjacent tiles' weights changes by 32 / sigma^2 per px across their edge
  const SLOPE = 32 / (G.sigma * G.sigma);
  function cells(plot, pad, seed, cls, band, phase = 0) {
    const N = TILE + 2 * pad;
    const nb = art._nbhd(plot);
    const bias = nb.map((p) => Math.exp(G.bias * (2 * lat(p.x, p.y, seed + 41) - 1)));
    const crisp = new Uint8Array(N * N), soft = new Uint8Array(N * N);
    const uniq = [...new Set(cls.filter((c) => c !== null))];
    if (uniq.length === 1 && cls[4] !== null) { crisp.fill(4); soft.fill(4); return { crisp, soft }; }
    const ci = Int8Array.from(cls.map((c) => (c === null ? -1 : uniq.indexOf(c))));
    const rep = uniq.map((c) => (cls[4] === c ? 4 : cls.indexOf(c)));
    const nc = uniq.length;
    const sums = new Float64Array(nc);
    const L2 = band ? 2 * SLOPE * band : 0;
    const X0 = plot.x * TILE - pad, Y0 = plot.y * TILE - pad;
    const { wx, wy } = warpRegion(X0, Y0, N, seed);
    for (let j = 0; j < N; j++) for (let i = 0; i < N; i++) {
      const k = j * N + i;
      const u = i - pad + wx[k], v = j - pad + wy[k];
      const x0 = E(u + 16), x1 = E(u - 16), x2 = E(u - 48);
      const y0 = E(v + 16), y1 = E(v - 16), y2 = E(v - 48);
      sums.fill(0);
      if (ci[0] >= 0) sums[ci[0]] += x0 * y0 * bias[0];
      if (ci[1] >= 0) sums[ci[1]] += x1 * y0 * bias[1];
      if (ci[2] >= 0) sums[ci[2]] += x2 * y0 * bias[2];
      if (ci[3] >= 0) sums[ci[3]] += x0 * y1 * bias[3];
      if (ci[4] >= 0) sums[ci[4]] += x1 * y1 * bias[4];
      if (ci[5] >= 0) sums[ci[5]] += x2 * y1 * bias[5];
      if (ci[6] >= 0) sums[ci[6]] += x0 * y2 * bias[6];
      if (ci[7] >= 0) sums[ci[7]] += x1 * y2 * bias[7];
      if (ci[8] >= 0) sums[ci[8]] += x2 * y2 * bias[8];
      let b1 = 0, b2 = -1;
      for (let c = 1; c < nc; c++) {
        if (sums[c] > sums[b1]) { b2 = b1; b1 = c; } else if (b2 < 0 || sums[c] > sums[b2]) b2 = c;
      }
      crisp[k] = rep[b1];
      if (!L2 || b2 < 0) { soft[k] = rep[b1]; continue; }
      const t = 0.5 + Math.log(sums[b1] / Math.max(sums[b2], 1e-300)) / L2; // the winner's share
      soft[k] = t >= 1 || bayer(X0 + i + phase, Y0 + j) < t ? rep[b1] : rep[b2];
    }
    return { crisp, soft };
  }

  // ---- moonlit clearings and cloud shade: a low-frequency field over the land, on the 4 px lattice
  const moonLat = (X, Y) => {
    const wpx = art.world.wrapX && art.world.width ? art.world.width * TILE : 0;
    let n = 0;
    for (const [cell, w] of G.moon.cells) n += w * art._noise(X, Y, cell, 700 + cell, wpx);
    return n;
  };
  function moonRegion(X0, Y0, N) {
    const gx0 = Math.floor(X0 / STEP), gy0 = Math.floor(Y0 / STEP);
    const gw = Math.floor((X0 + N - 1) / STEP) - gx0 + 2, gh = Math.floor((Y0 + N - 1) / STEP) - gy0 + 2;
    const lat = new Float64Array(gw * gh);
    for (let b = 0; b < gh; b++) for (let a = 0; a < gw; a++) lat[b * gw + a] = moonLat((gx0 + a) * STEP, (gy0 + b) * STEP);
    const out = new Float64Array(N * N);
    for (let j = 0; j < N; j++) {
      const Y = Y0 + j, b = Math.floor(Y / STEP) - gy0, ty = (Y - (gy0 + b) * STEP) / STEP;
      for (let i = 0; i < N; i++) {
        const X = X0 + i, a = Math.floor(X / STEP) - gx0, tx = (X - (gx0 + a) * STEP) / STEP;
        const p = b * gw + a;
        out[j * N + i] = (lat[p] * (1 - tx) + lat[p + 1] * tx) * (1 - ty) + (lat[p + gw] * (1 - tx) + lat[p + gw + 1] * tx) * ty;
      }
    }
    return out;
  }
  /** Moonlight over a tile's ground indices (in place); `skip(k)` marks pixels left alone (water, ink). */
  function moonlight(plot, out, skip) {
    const M = G.moon;
    const X0 = plot.x * TILE, Y0 = plot.y * TILE;
    const n = moonRegion(X0, Y0, TILE);
    for (let j = 0; j < TILE; j++) for (let i = 0; i < TILE; i++) {
      const k = j * TILE + i;
      const v = n[k];
      if (v < M.lit && v > M.shade) continue;
      if (skip && skip(k)) continue;
      const b = bayer(X0 + i + 2, Y0 + j + 1);
      if (v >= M.lit) {
        if (b < Math.min(1, (v - M.lit - M.grain) / M.ramp) * M.grainMax) out[k] = grainT[out[k]];
        else if (b < Math.min(1, (v - M.lit) / M.ramp) * M.max) out[k] = moonT[out[k]];
      }
      else if (b < Math.min(1, (M.shade - v) / M.ramp) * M.shadeMax) out[k] = shadeT[out[k]];
    }
  }
  art._moon = (X, Y) => moonLat(X, Y);

  /** Ground indices of a tile (32x32): banded textures, moonlight, shoreline ink, foam, depth contour. */
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
      if (!water[4]) moonlight(plot, out, null);
      this._ground.set(key, out);
      return out;
    }
    const shore = water.some((w) => w);
    const PAD = 3, N = TILE + 2 * PAD;
    const { crisp, soft } = cells(plot, PAD, 100, nb.map((p, k) => infos[k].type + (water[k] ? 'w' : '')), G.band);
    const still = new Uint8Array(TILE * TILE); // water and shoreline ink: no moonlight
    const cls = new Uint8Array(N * N); // 0 land, 1 shallow water, 2 deep water
    for (let k = 0; k < N * N; k++) { const c = crisp[k]; cls[k] = water[c] ? (infos[c].deep ? 2 : 1) : 0; }
    const C = (i, j) => cls[(j + PAD) * N + i + PAD];
    const k7 = this.K('7'), k6 = this.K('6'), kN = this.K('N'), kg = this.K('g');
    for (let j = 0; j < TILE; j++) for (let i = 0; i < TILE; i++) {
      const X = plot.x * TILE + i, Y = plot.y * TILE + j;
      const k = (j + PAD) * N + i + PAD;
      let c = crisp[k];
      // the dithered band only between two land textures: water edges keep their ink
      if (soft[k] !== c && !water[c] && !water[soft[k]]) c = soft[k];
      let v = texel(infos[c].tex, X, Y);
      const me = C(i, j);
      if (me) still[j * TILE + i] = 1;
      if (!shore) { /* no water near: texture only */ } else if (me === 0) {
        if (C(i - 1, j) || C(i + 1, j) || C(i, j - 1) || C(i, j + 1)) { v = (X + Y) % 5 === 0 ? k6 : k7; still[j * TILE + i] = 1; } else {
          let near = false;
          for (let dj = -2; dj <= 2 && !near; dj++) for (let di = -2; di <= 2; di++) if (C(i + di, j + dj)) { near = true; break; }
          if (near) { v = k2; still[j * TILE + i] = 1; }
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
    moonlight(plot, out, (k) => still[k]);
    this._ground.set(key, out);
    return out;
  };

  /**
   * Fog pass: REMEMBER (FOG with grey frost) with ink hatching on fogged pixels, void on unrevealed;
   * both edges are Bayer-dithered bands that follow the warp.
   */
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
      fc = { fog: cells(plot, 0, 100, vis.map((v) => (v ? v : null)), G.fogBand).soft, rev: revealCells(plot, vis) };
      fogCache.set(ck, fc);
    }
    const FOG = remember;
    const X0 = plot.x * TILE, Y0 = plot.y * TILE;
    this.remapRegion(ctx, x, y, TILE, TILE, (i, j, c) => {
      const k = j * TILE + i;
      if (vis[fc.rev[k]] === 0) return kUnseen;
      if (vis[fc.fog[k]] === 1) {
        const v = FOG[c];
        return (X0 + i + Y0 + j) % 4 === 0 ? FOG[v] : v;
      }
      return c;
    });
  };

  function revealCells(plot, vis) { return cells(plot, 0, 300, vis.map((v) => (v ? 1 : 0)), G.voidBand, 1).soft; }

  /**
   * Ground of a tile. An unrevealed tile next to revealed ones shows their ground where the
   * banded void edge (seed 300) gives the pixel to them, so the edge is ragged both ways.
   */
  art.drawTerrain = function (ctx, x, y, plot) {
    if (plot.vis !== 0) { this.blit(ctx, x, y, this.groundIndices(plot), TILE, TILE); return; }
    const nb = this._nbhd(plot);
    const vis = nb.map((p) => (p.vis === undefined ? 2 : p.vis));
    if (vis.every((v) => v === 0)) { ctx.fillStyle = this.css('0'); ctx.fillRect(x, y, TILE * this.scale, TILE * this.scale); return; }
    const rev = revealCells(plot, vis);
    const infos = nb.map((p) => this._terrain(p.terrain));
    const water = nb.map((p) => this._isWater(p));
    const { soft } = cells(plot, 0, 100, nb.map((p, k) => (vis[k] ? infos[k].type + (water[k] ? 'w' : '') : null)), G.band);
    const out = new Uint8Array(TILE * TILE);
    const still = new Uint8Array(TILE * TILE);
    for (let j = 0; j < TILE; j++) for (let i = 0; i < TILE; i++) {
      const k = j * TILE + i;
      if (!vis[rev[k]] || !vis[soft[k]]) { out[k] = kUnseen; still[k] = 1; continue; }
      if (water[soft[k]]) still[k] = 1;
      const t = infos[soft[k]].tex, X = plot.x * TILE + i, Y = plot.y * TILE + j;
      out[k] = t.data[(((Y % t.h) + t.h) % t.h) * t.w + (((X % t.w) + t.w) % t.w)];
    }
    moonlight(plot, out, (k) => still[k]);
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

  /**
   * Road: a pale path. From the tile anchor (16, 18) half-way toward each connected neighbour, the
   * line sways with a slow warp (seed 500, x G.path.sway; a function of the map position, so the
   * halves drawn by two tiles meet), drawn as dashes of the route sprite (bone ink with its coal
   * shadow) G.path.dash px on and G.path.gap off along the arc, over a worn verge: the ground one
   * step lighter, a 50% checker within 1.6 px of the line and 25% out to G.verge. Railroads keep
   * art.js's two straight ruled lines (over the same verge).
   */
  art.drawRoad = function (ctx, x, y, plot, dirs = null) {
    const type = this.typeName('route', plot.route);
    if (!type && !plot.city) return;
    const r = this.mapper.resolve('route', type || 'ROUTE_ROAD');
    if (!r || r.sprite === 'none') return;
    const has = dirs || DIRS8.map(([dx, dy]) => { const n = plot.at?.(dx, dy); return !!n && n.vis !== 0 && (!!this.typeName('route', n.route) || !!n.city); });
    const segs = [];
    has.forEach((on, k) => { if (on) segs.push(DIRS8[k]); });
    const ax = 16, ay = 18, s = this.scale;
    const X0 = plot.x * TILE, Y0 = plot.y * TILE;
    const sway = r.opts.double ? 0 : G.path.sway;
    const PAD = 10, N = TILE + 2 * PAD;
    const w = sway ? warpRegion(X0 - PAD, Y0 - PAD, N, 500) : null;
    const dot = this.pick(r.sprite, 0);
    const ink = new Uint8Array(N * N).fill(TRANSPARENT);
    const wear = new Uint8Array(N * N); // 2: inner verge, 1: outer
    const R = G.verge;
    const stamp = (qx, qy) => {
      if (!dot) return;
      for (let j = 0; j < dot.h; j++) for (let i = 0; i < dot.w; i++) {
        const c = dot.data[j * dot.w + i];
        if (c === TRANSPARENT) continue;
        const X = Math.round(qx) + i - dot.ax + PAD, Y = Math.round(qy) + j - dot.ay + PAD;
        if (X < 0 || Y < 0 || X >= N || Y >= N) continue;
        const p = Y * N + X;
        // ink over shadow: a later dot's shadow never covers an earlier dot
        if (ink[p] === TRANSPARENT || (i === dot.ax && j === dot.ay)) ink[p] = c;
      }
    };
    const worn = (qx, qy) => {
      for (let j = Math.floor(qy - R); j <= Math.ceil(qy + R); j++) for (let i = Math.floor(qx - R); i <= Math.ceil(qx + R); i++) {
        const X = i + PAD, Y = j + PAD;
        if (X < 0 || Y < 0 || X >= N || Y >= N) continue;
        const d = Math.hypot(i - qx, j - qy);
        const v = d <= 1.6 ? 2 : d <= R ? 1 : 0;
        if (v > wear[Y * N + X]) wear[Y * N + X] = v;
      }
    };
    const at = (px, py) => {
      if (!w) return [px, py];
      const i = Math.max(0, Math.min(N - 1, Math.round(px) + PAD)), j = Math.max(0, Math.min(N - 1, Math.round(py) + PAD));
      return [px + sway * w.wx[j * N + i], py + sway * w.wy[j * N + i]];
    };
    const period = G.path.dash + G.path.gap;
    const lines = segs.length ? segs : [[0, 0]];
    for (const [dx, dy] of lines) {
      const len = 16 * Math.hypot(dx, dy);
      const steps = Math.max(1, Math.ceil(len * 4));
      let arc = 0, prev = null;
      for (let k = 0; k <= steps; k++) {
        const t = k / steps;
        const [qx, qy] = at(ax + dx * 16 * t, ay + dy * 16 * t);
        if (prev) arc += Math.hypot(qx - prev[0], qy - prev[1]);
        prev = [qx, qy];
        worn(qx, qy);
        if (r.opts.double) continue;
        if (arc % period < G.path.dash) stamp(qx, qy);
      }
    }
    this.remapRegion(ctx, x - PAD * s, y - PAD * s, N, N, (i, j, c) => {
      const v = wear[j * N + i];
      if (!v) return c;
      const X = X0 - PAD + i, Y = Y0 - PAD + j;
      if (v === 2 ? ((X + Y) & 1) !== 0 : bayer(X, Y) >= 0.25) return c;
      return verge[c];
    });
    if (r.opts.double) return orig.drawRoad(ctx, x, y, plot, dirs);
    this.blit(ctx, x - PAD * s, y - PAD * s, ink, N, N);
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

  /** Viewport vignette (docs/ART.md 8.3) through REMEMBER, as the fog: grey snow at the corners. */
  art.drawVignette = function (ctx, x, y, w, h) {
    if (!G.vignette) return;
    const s = this.scale;
    const W = Math.floor(w / s), H = Math.floor(h / s);
    const cx = W / 2, cy = H / 2, R = Math.hypot(cx, cy);
    this.remapRegion(ctx, x, y, W, H, (i, j, c) => {
      const d = Math.hypot(i - cx, j - cy) / R;
      if (d <= 0.75) return c;
      return (d - 0.75) / 0.25 > bayer(i, j) ? remember[c] : c;
    });
  };

  // the live client's viewport vignette (assets/app/artmap.js) reads this too: none by day
  art.vignetteOn = !!G.vignette;
  art._terrainPass = true;
  art._ground.clear();
  return art;
}
