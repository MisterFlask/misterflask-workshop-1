// Terrain meshes and the map data textures of the 3D renderer (docs/R3D.md "Terrain").
//
// The ground is one heightfield surface cut into CHUNK x CHUNK-plot meshes of RES vertices per plot
// (built once, rebuilt only where the map changed: a revealed plot, a terraformed one). Everything
// that varies per plot (ground type, feature, kind, visibility, owner) goes to the shaders as small
// textures, one texel per plot, so the ground's painting reads the logical grid directly and stays
// world-anchored however the camera moves.

import { GROUND, KIND } from "./world.js";
import { texture, upload, Mesh } from "./gl.js";

export const CHUNK = 16;
/** Vertices per plot along each axis at full detail; distant zooms use LOW_RES. */
export const RES = 6;
export const LOW_RES = 2;
/** Height texture samples per plot (water depth, shore foam). */
export const HRES = 4;

export const TERRAIN_LAYOUT = { aPos: 3, aNormal: 3 };

/** The heights of a chunk's grid (with a one-sample border for normals): Float32Array of
 *  (n + 3)^2, n = CHUNK * res. */
function chunkHeights(world, cx, cy, res) {
  const n = CHUNK * res;
  const N = n + 3;
  const hs = new Float32Array(N * N);
  const x0 = cx * CHUNK;
  const y0 = cy * CHUNK;
  for (let j = 0; j < N; j++) {
    const my = y0 + (j - 1) / res;
    for (let i = 0; i < N; i++) {
      const mx = x0 + (i - 1) / res;
      hs[j * N + i] = world.heightAt(mx, my);
    }
  }
  return hs;
}

/** The mesh data of chunk (cx, cy): interleaved position + normal, and indices. */
export function buildChunk(world, cx, cy, res = RES) {
  const n = CHUNK * res;
  const N = n + 3;
  const hs = chunkHeights(world, cx, cy, res);
  // Chunks at the map's east end may hold fewer plots (no parchment chunk is drawn past the east
  // edge, so there is no gap there).
  const pw = Math.min(CHUNK, world.w - cx * CHUNK);
  // Every chunk is a full CHUNK of rows tall (F15). The last in-map chunk used to stop at row
  // world.h while the north parchment chunk began at ceil(h/CHUNK)*CHUNK, leaving an unmeshed band
  // (12 rows when h = 52). A full-height chunk runs through to the next chunk boundary, meeting the
  // parchment with no gap; its off-map rows read as parchment (heightAt and the textures default
  // there), and chunkVisible already culls each chunk as CHUNK rows tall.
  const ph = CHUNK;
  const vw = pw * res + 1;
  const vh = ph * res + 1;
  const data = new Float32Array(vw * vh * 6);
  const step = 1 / res;
  let lo = Infinity, hi = -Infinity;
  let o = 0;
  for (let j = 0; j < vh; j++) {
    for (let i = 0; i < vw; i++) {
      const h = hs[(j + 1) * N + i + 1];
      const hx = hs[(j + 1) * N + i + 2] - hs[(j + 1) * N + i];
      const hy = hs[(j + 2) * N + i + 1] - hs[j * N + i + 1];
      const nx = -hx / (2 * step);
      const nz = hy / (2 * step);
      const l = Math.hypot(nx, 1, nz);
      data[o++] = cx * CHUNK + i * step;
      data[o++] = h;
      data[o++] = -(cy * CHUNK + j * step);
      data[o++] = nx / l;
      data[o++] = 1 / l;
      data[o++] = nz / l;
      lo = Math.min(lo, h);
      hi = Math.max(hi, h);
    }
  }
  const idx = new Uint32Array((vw - 1) * (vh - 1) * 6);
  let k = 0;
  for (let j = 0; j + 1 < vh; j++) {
    for (let i = 0; i + 1 < vw; i++) {
      const a = j * vw + i, b = a + 1, c = a + vw, d = c + 1;
      // Split each quad along the diagonal that follows the ground's fold (fewer sawteeth).
      const ha = data[a * 6 + 1], hb = data[b * 6 + 1], hc = data[c * 6 + 1], hd = data[d * 6 + 1];
      if (Math.abs(ha - hd) < Math.abs(hb - hc)) {
        idx[k++] = a; idx[k++] = d; idx[k++] = b;
        idx[k++] = a; idx[k++] = c; idx[k++] = d;
      } else {
        idx[k++] = a; idx[k++] = c; idx[k++] = b;
        idx[k++] = b; idx[k++] = c; idx[k++] = d;
      }
    }
  }
  return { data, idx, x0: cx * CHUNK, y0: cy * CHUNK, pw, ph, lo, hi };
}

/** The terrain's chunks on the GPU, built lazily and rebuilt when the plots under them change. */
export class TerrainMeshes {
  constructor(gl) {
    this.gl = gl;
    this.chunks = new Map(); // "cx,cy,res" -> {mesh, x0, y0, pw, ph, lo, hi}
    this.world = null;
  }

  setWorld(world, dirtyPlots = null) {
    const same = this.world && world && this.world.w === world.w && this.world.h === world.h;
    this.world = world;
    if (!same || !dirtyPlots) {
      this.clear();
      return;
    }
    // A plot's change reaches one plot around it (the height blend); drop the chunks it touches.
    const drop = new Set();
    for (const i of dirtyPlots) {
      const x = i % world.w, y = Math.floor(i / world.w);
      for (let dy = -2; dy <= 2; dy++) {
        for (let dx = -2; dx <= 2; dx++) {
          let xx = x + dx;
          if (world.wrapX) xx = ((xx % world.w) + world.w) % world.w;
          const yy = y + dy;
          if (xx < 0 || yy < 0 || xx >= world.w || yy >= world.h) continue;
          drop.add(`${Math.floor(xx / CHUNK)},${Math.floor(yy / CHUNK)}`);
        }
      }
    }
    for (const [k, c] of [...this.chunks]) {
      if (drop.has(k.split(",").slice(0, 2).join(","))) {
        c.mesh.dispose();
        this.chunks.delete(k);
      }
    }
  }

  clear() {
    for (const c of this.chunks.values()) c.mesh.dispose();
    this.chunks.clear();
  }

  /** Chunk (cx, cy) at `res`, building it on first use. `budget.n` limits builds per frame. */
  get(cx, cy, res, budget) {
    const k = `${cx},${cy},${res}`;
    let c = this.chunks.get(k);
    if (c) return c;
    if (budget && budget.n <= 0) return this.chunks.get(`${cx},${cy},${res === RES ? LOW_RES : RES}`) ?? null;
    if (budget) budget.n--;
    const b = buildChunk(this.world, cx, cy, res);
    c = { mesh: new Mesh(this.gl, b.data, TERRAIN_LAYOUT, { indices: b.idx }), x0: b.x0, y0: b.y0, pw: b.pw, ph: b.ph, lo: b.lo, hi: b.hi };
    this.chunks.set(k, c);
    return c;
  }

  dispose() {
    this.clear();
  }
}

/** The per-plot data textures (one texel per plot, NEAREST) and the soft visibility and height
 *  textures (LINEAR). */
export class MapTextures {
  constructor(gl) {
    this.gl = gl;
    this.terrain = null;
    this.vis = null;
    this.owner = null;
    this.height = null;
    this.w = 0;
    this.h = 0;
  }

  /** Rebuilds the textures from the world and the map reply; `dirty` (plot indices) limits the
   *  height samples recomputed to the plots around them. */
  update(world, map, dirty = null) {
    const gl = this.gl;
    const w = world.w, h = world.h, n = w * h;
    if (w !== this.w || h !== this.h) {
      this.dispose();
      dirty = null;
      this.w = w;
      this.h = h;
      const nearest = { filter: gl.NEAREST, wrapS: world.wrapX ? gl.REPEAT : gl.CLAMP_TO_EDGE };
      this.terrain = texture(gl, w, h, nearest);
      this.owner = texture(gl, w, h, nearest);
      this.vis = texture(gl, w, h, { internal: gl.R8, format: gl.RED, filter: gl.LINEAR, wrapS: world.wrapX ? gl.REPEAT : gl.CLAMP_TO_EDGE });
      this.height = texture(gl, w * HRES, h * HRES, { internal: gl.R16F, format: gl.RED, type: gl.FLOAT, filter: gl.LINEAR, wrapS: world.wrapX ? gl.REPEAT : gl.CLAMP_TO_EDGE });
    }
    const t = new Uint8Array(n * 4);
    const o = new Uint8Array(n * 4);
    const v = new Uint8Array(n);
    for (let i = 0; i < n; i++) {
      t[i * 4] = world.ground[i];
      t[i * 4 + 1] = world.feat[i];
      t[i * 4 + 2] = world.kind[i];
      t[i * 4 + 3] = world.vis[i] * 127;
      v[i] = world.vis[i] === 2 ? 255 : world.vis[i] === 1 ? 128 : 0;
      const ow = map.owner[i];
      o[i * 4] = ow >= 0 ? ow + 1 : 0;
      o[i * 4 + 1] = map.city[i] >= 0 ? 255 : 0;
      o[i * 4 + 2] = map.improvement[i] >= 0 ? 255 : 0;
      o[i * 4 + 3] = map.route[i] >= 0 ? 255 : 0;
    }
    upload(gl, this.terrain, t);
    upload(gl, this.owner, o);
    upload(gl, this.vis, v);
    const W = w * HRES;
    if (!dirty || !this.H) {
      this.H = new Float32Array(W * h * HRES);
      for (let j = 0; j < h * HRES; j++) {
        for (let i = 0; i < W; i++) this.H[j * W + i] = world.heightAt((i + 0.5) / HRES, (j + 0.5) / HRES);
      }
    } else {
      // A plot's change reaches the height one plot (plus the warp) around it.
      const redo = new Set();
      for (const p of dirty) {
        const x = p % w, y = Math.floor(p / w);
        for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) {
          const k = world.index(x + dx, y + dy);
          if (k >= 0) redo.add(k);
        }
      }
      for (const k of redo) {
        const x = k % w, y = Math.floor(k / w);
        for (let j = y * HRES; j < (y + 1) * HRES; j++) {
          for (let i = x * HRES; i < (x + 1) * HRES; i++) this.H[j * W + i] = world.heightAt((i + 0.5) / HRES, (j + 0.5) / HRES);
        }
      }
    }
    upload(gl, this.height, this.H);
  }

  dispose() {
    const gl = this.gl;
    for (const k of ["terrain", "vis", "owner", "height"]) if (this[k]) gl.deleteTexture(this[k]);
    this.terrain = this.vis = this.owner = this.height = null;
    this.w = this.h = 0;
  }
}

export { GROUND, KIND };
