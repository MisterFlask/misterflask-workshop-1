// The 3D scene on the GPU (docs/R3D.md "Scene"): terrain chunks, the sea and the rivers, the
// dressing (instanced props, bucketed by asset and map chunk so the frustum test skips whole
// buckets), team rings and path ribbons, particles, and the map textures the shaders read. The
// pipeline (pipeline.js) calls the draw* methods with its programs; each draws every copy of the
// map the camera sees (a wrapping map repeats east and west).

import { ATTR, Mesh, texture, upload, INSTANCE_FLOATS, PROP_LAYOUT } from "./gl.js";
import { TerrainMeshes, MapTextures, CHUNK, RES, LOW_RES } from "./terrain.js";
import { buildAsset, buildAssetLOD } from "./assets/registry.js";
import { rasterDecals, rasterLight } from "./decals.js";
import { SEA, RIVER_HALF, RIVER_WANDER } from "./world.js";
import { hexLinear } from "./math.js";
import { rng } from "./noise.js";

const RIVER_LAYOUT = { aPos: 3, aExtra: 4 };
/** The sea and the rivers: aExtra (across the river, down it, over the sea), aFlow (the river's
 *  bend, how far past its end) -- the sea's are zero. */
const WATER_LAYOUT = { aPos: 3, aExtra: 4, aFlow: 2 };
/** Below this size on screen (bounding radius in pixels) a prop draws its simplified copy. */
const LOD_PX = 22;
/** Props are bucketed by squares of this many plots (finer than the terrain chunks: the frustum
 *  test then skips more of what is off screen, for a few more draw calls). */
const BUCKET = 8;
const PARTICLE_LAYOUT = { aPos: 3, aNormal: 3, aExtra: 4 };

/** One asset's vertex buffer, shared by its per-chunk instance buckets. */
class AssetBatch {
  constructor(gl, built) {
    this.gl = gl;
    this.vbo = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.vbo);
    gl.bufferData(gl.ARRAY_BUFFER, built.data, gl.STATIC_DRAW);
    this.count = built.count;
    this.radius = built.radius;
    this.hi = built.hi[1];
    this.shadow = built.shadow !== false;
  }

  /** A VAO over this asset's vertices and a new instance buffer (or `shared`, another bucket's,
   *  for the level of detail drawing the same instances). */
  bucket(instances, shared = null) {
    const gl = this.gl;
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.vbo);
    const stride = Object.values(PROP_LAYOUT).reduce((a, b) => a + b, 0) * 4;
    let off = 0;
    for (const [name, n] of Object.entries(PROP_LAYOUT)) {
      gl.enableVertexAttribArray(ATTR[name]);
      gl.vertexAttribPointer(ATTR[name], n, gl.FLOAT, false, stride, off);
      off += n * 4;
    }
    const ibuf = shared ?? gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, ibuf);
    if (!shared) gl.bufferData(gl.ARRAY_BUFFER, instances, gl.STATIC_DRAW);
    let o = 0;
    for (const name of ["iPosRot", "iScale", "iTint", "iTeam"]) {
      gl.enableVertexAttribArray(ATTR[name]);
      gl.vertexAttribPointer(ATTR[name], 4, gl.FLOAT, false, INSTANCE_FLOATS * 4, o);
      gl.vertexAttribDivisor(ATTR[name], 1);
      o += 16;
    }
    gl.bindVertexArray(null);
    return { vao, ibuf, n: instances ? instances.length / INSTANCE_FLOATS : 0, shared: !!shared };
  }

  dispose() {
    this.gl.deleteBuffer(this.vbo);
  }
}

/** The bucket key ("cx,cy") of an instance record. */
export function bucketOf(it) {
  return `${Math.floor(it.x / BUCKET)},${Math.floor(-it.z / BUCKET)}`;
}

/** Instance records (dress.js) -> buckets by asset and chunk. */
function packBuckets(list) {
  const by = new Map();
  for (const it of list) {
    const ck = bucketOf(it);
    const key = `${it.asset}#${it.variant}#${it.param}`;
    let m = by.get(key);
    if (!m) by.set(key, (m = new Map()));
    let a = m.get(ck);
    if (!a) m.set(ck, (a = []));
    a.push(it);
  }
  return by;
}

export class Scene {
  constructor(gl) {
    this.gl = gl;
    this.terrain = new TerrainMeshes(gl);
    this.tex = new MapTextures(gl);
    this.assets = new Map(); // asset key -> AssetBatch
    this.layers = { static: [], units: [] }; // [{batch, chunk: [cx, cy], top, bucket}]
    this.world = null;
    this.water = null;
    this.rivers = null;
    this.markers = null;
    this.particles = null;
    this.decal = null;
    this.light = null;
    this.marks = null;
    this.visible = null; // {x0, x1, y0, y1} map rect, set per frame
    this.copies = [0];
    this.budget = { n: 4 };
    this.ownerColors = new Float32Array(64 * 3);
    this.hover = [0, 0, 0, 0];
    this.sel = [0, 0, 0, 0];
    this.overlayOpts = { borders: 1, political: 0, grid: 0.2 };
  }

  // --- data -------------------------------------------------------------------------------------

  setWorld(world, map, dirty) {
    const sizeChanged = !this.world || this.world.w !== world.w || this.world.h !== world.h;
    this.world = world;
    this.terrain.setWorld(world, sizeChanged ? null : dirty);
    this.tex.update(world, map, sizeChanged ? null : dirty);
    if (sizeChanged || !this.water) this.buildWater();
    this.buildRivers();
  }

  /** The decal and light textures (after the cities are known). */
  setPaint(world, map, cities, lightStrength) {
    const gl = this.gl;
    const d = rasterDecals(world, map, cities);
    const l = rasterLight(world, map, cities, lightStrength);
    const wrap = world.wrapX ? gl.REPEAT : gl.CLAMP_TO_EDGE;
    if (!this.decal || this.decal.w !== d.w || this.decal.h !== d.h) {
      if (this.decal) gl.deleteTexture(this.decal);
      this.decal = texture(gl, d.w, d.h, { wrapS: wrap, mips: false });
    }
    upload(gl, this.decal, d.data);
    if (!this.light || this.light.w !== l.w || this.light.h !== l.h) {
      if (this.light) gl.deleteTexture(this.light);
      this.light = texture(gl, l.w, l.h, { wrapS: wrap });
    }
    upload(gl, this.light, l.data);
  }

  setOwnerColors(colors) {
    this.ownerColors.fill(0);
    colors.forEach((hex, i) => {
      if (i + 1 >= 64 || !hex) return;
      const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
      if (!m) return;
      this.ownerColors.set([parseInt(m[1], 16) / 255, parseInt(m[2], 16) / 255, parseInt(m[3], 16) / 255], (i + 1) * 3);
    });
  }

  /** The plot marks (r targets, g path) for the overlay. */
  setMarks(targets, path) {
    const gl = this.gl;
    const w = this.world?.w ?? 1, h = this.world?.h ?? 1;
    if (!this.marks || this.marks.w !== w || this.marks.h !== h) {
      if (this.marks) gl.deleteTexture(this.marks);
      this.marks = texture(gl, w, h, { filter: gl.NEAREST });
    }
    const a = new Uint8Array(w * h * 4);
    for (const [x, y] of targets ?? []) if (x >= 0 && y >= 0 && x < w && y < h) a[(y * w + x) * 4] = 255;
    for (const [x, y] of path ?? []) if (x >= 0 && y >= 0 && x < w && y < h) a[(y * w + x) * 4 + 1] = 255;
    upload(gl, this.marks, a);
  }

  /** Replaces a layer's instances ("static" dressing or "units"). */
  setInstances(layer, list, chunks = null) {
    const gl = this.gl;
    // `chunks` (a Set of "cx,cy" bucket keys): only those chunks are replaced, by `list` (which
    // then holds exactly their instances); the rest of the layer stays as it is.
    const out = [];
    for (const b of this.layers[layer]) {
      if (chunks && !chunks.has(`${b.cx},${b.cy}`)) {
        out.push(b);
        continue;
      }
      gl.deleteVertexArray(b.bucket.vao);
      gl.deleteBuffer(b.bucket.ibuf);
      if (b.lodBucket) gl.deleteVertexArray(b.lodBucket.vao);
    }
    for (const [key, chunks] of packBuckets(list)) {
      let batch = this.assets.get(key);
      if (!batch) {
        const [asset, variant, param] = key.split("#");
        let built;
        try {
          built = buildAsset(asset, Number(variant), param);
        } catch (e) {
          console.warn(`r3d asset ${key}:`, e.message);
          continue;
        }
        batch = new AssetBatch(gl, built);
        // The simplified copy, for far zooms and the shadow map (not for units: they stay whole).
        if (layer === "static") {
          const [asset, variant, param] = key.split("#");
          const lod = buildAssetLOD(asset, Number(variant), param);
          if (lod) batch.lod = new AssetBatch(gl, lod);
        }
        this.assets.set(key, batch);
      }
      for (const [ck, items] of chunks) {
        const arr = new Float32Array(items.length * INSTANCE_FLOATS);
        let top = 0;
        items.forEach((it, k) => {
          arr.set([it.x, it.y, it.z, it.yaw, it.sx, it.sy, it.sz, it.phase, it.tint[0], it.tint[1], it.tint[2], it.id, it.team[0], it.team[1], it.team[2], it.protect], k * INSTANCE_FLOATS);
          top = Math.max(top, it.y + batch.hi * it.sy);
        });
        const [cx, cy] = ck.split(",").map(Number);
        const bucket = batch.bucket(arr);
        const lodBucket = batch.lod ? batch.lod.bucket(null, bucket.ibuf) : null;
        if (lodBucket) lodBucket.n = bucket.n;
        out.push({ batch, cx, cy, top, bucket, lodBucket });
      }
    }
    this.layers[layer] = out;
  }

  /** The sea: a grid over the map at sea level (the water shader paints depth, foam and waves). */
  buildWater() {
    const gl = this.gl;
    this.water?.dispose();
    const w = this.world.w, h = this.world.h;
    const step = 4;
    const v = [];
    const idx = [];
    const nx = Math.ceil(w / step) + 1, ny = Math.ceil(h / step) + 1;
    for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) v.push(Math.min(w, i * step), SEA, -Math.min(h, j * step), 0, 0, 0, 0, 0, 0);
    for (let j = 0; j + 1 < ny; j++) {
      for (let i = 0; i + 1 < nx; i++) {
        const a = j * nx + i;
        idx.push(a, a + nx, a + 1, a + 1, a + nx, a + nx + 1);
      }
    }
    this.water = new Mesh(gl, new Float32Array(v), WATER_LAYOUT, { indices: new Uint32Array(idx) });
  }

  /** The rivers: ribbons along the smoothed river lines, their surface a little under the banks,
   *  a little wider than the water (the water shader draws its wandering edge, WATER_FS) and
   *  capped at both ends (a source, a confluence). Per vertex: aExtra = across the channel (1 at
   *  the water's mean edge, RIVER_HALF), the distance downstream, 1 over the sea; aFlow = how the
   *  river bends (radians a plot, + to the left looking downstream), how far past its end (in
   *  RIVER_HALFs, as across). */
  buildRivers() {
    const gl = this.gl;
    this.rivers?.dispose();
    this.rivers = null;
    const world = this.world;
    const v = [];
    // Half the ribbon's width: the widest the water's edge may wander, and a pixel or two more.
    const half = RIVER_HALF + RIVER_WANDER + 0.02;
    for (const line of world.rivers) {
      // Resample to an even spacing.
      const pts = [];
      for (let k = 0; k + 1 < line.length; k++) {
        const a = line[k], b = line[k + 1];
        const n = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1]) / 0.08));
        for (let s = 0; s < n; s++) pts.push([a[0] + ((b[0] - a[0]) * s) / n, a[1] + ((b[1] - a[1]) * s) / n]);
      }
      pts.push(line[line.length - 1]);
      if (pts.length < 2) continue;
      // Downstream is the lower end.
      const h0 = world.heightAt(pts[0][0], pts[0][1]);
      const h1 = world.heightAt(pts[pts.length - 1][0], pts[pts.length - 1][1]);
      if (h0 < h1) pts.reverse();
      const N = pts.length;
      // Tangents, the distance downstream, and the bend: the heading's change a plot, averaged over
      // about a third of a plot each way (white water gathers on the outside of a whole bend).
      const tan = pts.map((p, k) => {
        const a = pts[Math.max(0, k - 1)], b = pts[Math.min(N - 1, k + 1)];
        const l = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
        return [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
      });
      const along = [0];
      for (let k = 1; k < N; k++) along.push(along[k - 1] + Math.hypot(pts[k][0] - pts[k - 1][0], pts[k][1] - pts[k - 1][1]));
      const turn = tan.map((t, k) => {
        if (k === 0 || k === N - 1) return 0;
        const a = tan[k - 1], b = tan[k + 1];
        return Math.atan2(a[0] * b[1] - a[1] * b[0], a[0] * b[0] + a[1] * b[1]) / Math.max(1e-6, along[k + 1] - along[k - 1]);
      });
      const bend = turn.map((_, k) => {
        let s = 0, n = 0;
        for (let j = Math.max(0, k - 4); j <= Math.min(N - 1, k + 4); j++, n++) s += turn[j];
        return s / n;
      });
      const level = (p) => Math.max(SEA + 0.004, world.heightAt(p[0], p[1]) + 0.045);
      const rows = [];
      const row = (p, t, y, s, b, past) => rows.push([[p[0] - t[1] * half, y, -(p[1] + t[0] * half)], [p[0] + t[1] * half, y, -(p[1] - t[0] * half)], s, b, past]);
      // The river and a cap past each end (the shader rounds it). A cap sits a hair lower, more the
      // further out: where it overlaps another river's water (a confluence), the ribbon nearest its
      // own line is the one drawn.
      const CAPS = 4;
      const capRow = (k, dir, j) => {
        const d = (half * j) / CAPS;
        const p = [pts[k][0] + tan[k][0] * d * dir, pts[k][1] + tan[k][1] * d * dir];
        row(p, tan[k], level(pts[k]) - 0.002 - 0.01 * d, along[k] + d * dir, 0, d);
      };
      for (let j = CAPS; j > 0; j--) capRow(0, -1, j);
      for (let k = 0; k < N; k++) row(pts[k], tan[k], level(pts[k]), along[k], bend[k], 0);
      for (let j = 1; j <= CAPS; j++) capRow(N - 1, 1, j);
      // aExtra.z: 1 where the ribbon runs out over the sea (the ground under it, uncut by the
      // channel, is below sea level): the water shader leaves the sea there (WATER_FS).
      const sea = (p) => (world.heightAt(p[0], -p[2], false) < SEA - 0.005 ? 1 : 0);
      const u = half / RIVER_HALF;
      for (let k = 0; k + 1 < rows.length; k++) {
        const [a0, a1, s0, b0, p0] = rows[k];
        const [c0, c1, s1, b1, p1] = rows[k + 1];
        const V = (p, uu, s, b, past) => v.push(p[0], p[1], p[2], uu, s, sea(p), 0, b, past / RIVER_HALF);
        V(a0, -u, s0, b0, p0); V(c0, -u, s1, b1, p1); V(a1, u, s0, b0, p0);
        V(a1, u, s0, b0, p0); V(c0, -u, s1, b1, p1); V(c1, u, s1, b1, p1);
      }
    }
    if (v.length) this.rivers = new Mesh(gl, new Float32Array(v), WATER_LAYOUT);
  }

  /** Team rings (unit stacks, cities) and the path ribbon, draped on the ground. `rings`:
   *  [{x, y (map plot centre), r, color "#hex", rim "#hex", width}], `path`: [[mx, my], ...]. */
  setMarkers(rings, path, pathColor) {
    const gl = this.gl;
    this.markers?.dispose();
    this.markers = null;
    const world = this.world;
    if (!world) return;
    // The rings change with the stacks and the selection, the path with every hover: the rings
    // keep their own mesh, rebuilt only when they (or the ground) change.
    const ringKey = rings.map((g) => `${g.x},${g.y},${g.r},${g.color},${g.rim},${g.width},${g.fill}`).join(";");
    const rebuildRings = ringKey !== this.ringKey || world !== this.ringWorld;
    if (rebuildRings) {
      this.ringMesh?.dispose();
      this.ringMesh = null;
      this.ringKey = ringKey;
      this.ringWorld = world;
    }
    const v = [];
    // Draped on the surface the player sees: on a sea plot heightAt is the sea floor, so clamp to
    // the water level so rings and the path ribbon sit on the water, not under it (F7).
    const ground = (mx, my) => Math.max(SEA, world.heightAt(mx, my));
    const push = (mx, my, lift, c, a, H = ground) => v.push(mx, H(mx, my) + lift, -my, c[0], c[1], c[2], a);
    // A ring is draped on a 5 x 5 patch of the ground's height under it (bilinear), not on a
    // height lookup per vertex: hundreds of stacks re-ring on every move.
    const patch = (cx, cy, R) => {
      const N = 4;
      const hs = new Float64Array((N + 1) * (N + 1));
      for (let j = 0; j <= N; j++) for (let i = 0; i <= N; i++) hs[j * (N + 1) + i] = Math.max(SEA, world.heightAt(cx - R + (2 * R * i) / N, cy - R + (2 * R * j) / N));
      return (mx, my) => {
        const u = Math.min(N - 1e-6, Math.max(0, ((mx - cx + R) / (2 * R)) * N));
        const t = Math.min(N - 1e-6, Math.max(0, ((my - cy + R) / (2 * R)) * N));
        const i = Math.floor(u), j = Math.floor(t), fu = u - i, ft = t - j;
        const h = (a, b) => hs[b * (N + 1) + a];
        return (h(i, j) * (1 - fu) + h(i + 1, j) * fu) * (1 - ft) + (h(i, j + 1) * (1 - fu) + h(i + 1, j + 1) * fu) * ft;
      };
    };
    const N = 28;
    const cs = Array.from({ length: N + 1 }, (_, k) => [Math.cos((k / N) * Math.PI * 2), Math.sin((k / N) * Math.PI * 2) * 0.92]);
    const annulus = (cx, cy, r0, r1, c, a, lift, H) => {
      const corner = (r, k) => push(cx + cs[k][0] * r, cy + cs[k][1] * r, lift, c, a, H);
      for (let k = 0; k < N; k++) {
        corner(r0, k);
        corner(r1, k);
        corner(r1, k + 1);
        corner(r0, k);
        corner(r1, k + 1);
        corner(r0, k + 1);
      }
    };
    for (const g of rebuildRings ? rings : []) {
      const c = hexRgbArr(g.color);
      const rim = hexRgbArr(g.rim ?? "#1a1410");
      const r = g.r ?? 0.3;
      const wd = g.width ?? 0.045;
      const H = patch(g.x, g.y, r + wd);
      annulus(g.x, g.y, r - wd * 0.5, r + wd * 0.5, rim, 0.75, 0.014, H);
      annulus(g.x, g.y, r - wd * 0.32, r + wd * 0.32, c, 0.95, 0.016, H);
      if (g.fill) annulus(g.x, g.y, 0, r - wd * 0.5, c, g.fill, 0.012, H);
    }
    if (rebuildRings && v.length) this.ringMesh = new Mesh(gl, new Float32Array(v), RIVER_LAYOUT);
    v.length = 0;
    if (path && path.length > 1) {
      const c = hexRgbArr(pathColor ?? "#ffe9a0");
      const wd = 0.035;
      for (let k = 0; k + 1 < path.length; k++) {
        const [ax, ay] = path[k], [bx, by] = path[k + 1];
        const n = Math.max(2, Math.ceil(Math.hypot(bx - ax, by - ay) / 0.1));
        let tx = bx - ax, ty = by - ay;
        const l = Math.hypot(tx, ty) || 1;
        tx /= l;
        ty /= l;
        for (let s = 0; s < n; s++) {
          const t0 = s / n, t1 = (s + 1) / n;
          if (s % 2 === 1) continue; // dashed
          const p0 = [ax + (bx - ax) * t0, ay + (by - ay) * t0], p1 = [ax + (bx - ax) * t1, ay + (by - ay) * t1];
          const q = [[p0[0] - ty * wd, p0[1] + tx * wd], [p0[0] + ty * wd, p0[1] - tx * wd], [p1[0] + ty * wd, p1[1] - tx * wd], [p1[0] - ty * wd, p1[1] + tx * wd]];
          for (const [mx, my] of [q[0], q[1], q[2], q[0], q[2], q[3]]) push(mx, my, 0.02, c, 0.95);
        }
      }
    }
    if (v.length) this.markers = new Mesh(gl, new Float32Array(v), RIVER_LAYOUT);
  }

  /** Particles from the emitters (dress.js): [{x, y, z, color, size, rise, count, spread, smoke}]
   *  (`smoke`: soft puffs that swell, rise higher and drift, composited over the frame instead of
   *  glowing; the shader tells them by a negative size). */
  setParticles(emitters) {
    const gl = this.gl;
    this.particles?.dispose();
    this.particles = null;
    const v = [];
    let k = 0;
    for (const e of emitters) {
      const c = Array.isArray(e.color) ? e.color : hexRgbArr(e.color ?? "#ff8a30");
      const r = rng(k++ * 131 + 7);
      for (let i = 0; i < (e.count ?? 6); i++) {
        const sp = e.spread ?? 0.3;
        const size = (e.size ?? 0.02) * r.range(0.7, 1.3);
        v.push(e.x + r.range(-sp, sp), e.y + r.range(0, 0.05), e.z + r.range(-sp, sp), r(), (e.rise ?? 0.25) * r.range(0.6, 1.4), r(), c[0], c[1], c[2], e.smoke ? -size : size);
      }
    }
    if (v.length) this.particles = new Mesh(gl, new Float32Array(v), PARTICLE_LAYOUT, { mode: gl.POINTS });
  }

  // --- drawing ----------------------------------------------------------------------------------

  /** Sets the frame's view: the map rect seen (x0..x1, y0..y1 in map units, x unwrapped), the
   *  copies of the map in view, and whether distant chunks may use the coarse terrain. */
  setView(rect, copies, far, px = 96, viewProj = null) {
    this.px = px;
    this.visible = rect;
    this.planes = viewProj ? frustumPlanes(viewProj) : null;
    this.copies = copies;
    this.far = far;
    this.budget.n = 6;
    /** What the last frame's shaded prop pass drew: draw calls, instances, triangles. */
    this.drawn = { draws: 0, instances: 0, tris: 0, terrainTris: 0 };
  }

  chunkVisible(cx, cy, copy, margin = 1.5, size = CHUNK) {
    const r = this.visible;
    if (!r) return true;
    const x0 = cx * size + copy, y0 = cy * size;
    return x0 + size >= r.x0 - margin && x0 <= r.x1 + margin && y0 + size >= r.y0 - margin && y0 <= r.y1 + margin;
  }

  /** Whether the box over map rect (x0, y0, size) from the sea floor to height `top` meets the
   *  camera's frustum (the planes of the frame's view-projection). */
  boxVisible(x0, y0, size, top) {
    const P = this.planes;
    if (!P) return true;
    const lo = [x0, -0.5, -(y0 + size)], hi = [x0 + size, Math.max(top, 0.2), -y0];
    for (const p of P) {
      // The box corner furthest along the plane's normal.
      const x = p[0] >= 0 ? hi[0] : lo[0], y = p[1] >= 0 ? hi[1] : lo[1], z = p[2] >= 0 ? hi[2] : lo[2];
      if (p[0] * x + p[1] * y + p[2] * z + p[3] < 0) return false;
    }
    return true;
  }

  mapUniforms() {
    const w = this.world;
    return {
      uMapSize: [w.w, w.h],
      uWrapX: w.wrapX ? 1 : 0,
      uVis: this.tex.vis,
      uLight: this.light,
      uTerrain: this.tex.terrain,
      uDecal: this.decal,
      uOwner: this.tex.owner,
      uHeight: this.tex.height,
    };
  }

  drawTerrain(prog, U, shadow) {
    const world = this.world;
    if (!world) return;
    prog.use().set(U).set(this.mapUniforms());
    const ncx = Math.ceil(world.w / CHUNK);
    const ncy = Math.ceil(world.h / CHUNK);
    for (const copy of this.copies) {
      prog.set({ uCopyX: copy });
      for (let cy = -1; cy <= ncy; cy++) {
        for (let cx = 0; cx < ncx; cx++) {
          if (!this.chunkVisible(cx, cy, copy, shadow ? 3 : 1)) continue;
          const res = this.far || shadow || cy < 0 || cy >= ncy ? LOW_RES : RES;
          const c = this.terrain.get(cx, cy, res, this.budget);
          if (c) {
            c.mesh.draw();
            if (!shadow && this.drawn && prog.name === "terrain") this.drawn.terrainTris += c.mesh.count / 3;
          }
        }
      }
    }
  }

  drawWater(prog, U, depthOnly = false) {
    if (!this.water) return;
    prog.use().set(U);
    // uCoarse: the far zoom's ground is coarse, so the water reads its depth from the height field.
    if (!depthOnly) prog.set({ ...this.mapUniforms(), uCoarse: this.far ? 1 : 0 });
    for (const copy of this.copies) {
      prog.set({ uCopyX: copy, uRiver: 0 });
      if (!depthOnly) prog.set({ uRiverLift: 0 });
      this.water.draw();
      if (this.rivers) {
        prog.set({ uRiver: 1 });
        // Lift the ribbon only in the colour pass, and only at the far zoom, so it rides over the
        // coarse terrain instead of sinking under it (F18). The depth prepass (shadowTerrain) does
        // not use this uniform, so no reset is needed there.
        if (!depthOnly) prog.set({ uRiverLift: this.far ? 0.05 : 0 });
        this.rivers.draw();
      }
    }
    if (!depthOnly) prog.set({ uRiverLift: 0 });
  }

  drawProps(prog, U, shadow) {
    const gl = this.gl;
    prog.use().set(U).set(this.mapUniforms());
    for (const copy of this.copies) {
      prog.set({ uCopyX: copy });
      for (const layer of ["static", "units"]) {
        // Detail culling: decoration a few pixels across at this zoom is not drawn, and the
        // smallest things and ground clutter (recipes with `shadow: false`) cast no shadow
        // (units always draw).
        const minPx = layer === "units" ? 0 : shadow ? 6 : 2.5;
        for (const b of this.layers[layer]) {
          const px = b.batch.radius * this.px;
          if (px < minPx || (shadow && !b.batch.shadow)) continue;
          if (!this.chunkVisible(b.cx, b.cy, copy, shadow ? 3 : 1.5, BUCKET)) continue;
          if (!shadow && !this.boxVisible(b.cx * BUCKET + copy - 0.6, b.cy * BUCKET - 0.6, BUCKET + 1.2, b.top + 0.05)) continue;
          // The simplified copy when the thing is small on screen, and always for the shadows.
          const lod = b.lodBucket && (shadow || px < LOD_PX);
          const bk = lod ? b.lodBucket : b.bucket;
          const count = lod ? b.batch.lod.count : b.batch.count;
          gl.bindVertexArray(bk.vao);
          gl.drawArraysInstanced(gl.TRIANGLES, 0, count, bk.n);
          if (!shadow && this.drawn && prog.name === "prop") {
            this.drawn.draws++;
            this.drawn.instances += bk.n;
            this.drawn.tris += (count / 3) * bk.n;
          }
        }
      }
    }
    gl.bindVertexArray(null);
  }

  /** The unit figures again (the units layer), for their x-ray silhouettes. */
  drawUnitGhosts(prog, U) {
    const gl = this.gl;
    prog.use().set(U).set(this.mapUniforms());
    for (const copy of this.copies) {
      prog.set({ uCopyX: copy });
      for (const b of this.layers.units) {
        if (!this.chunkVisible(b.cx, b.cy, copy, 1.5, BUCKET)) continue;
        gl.bindVertexArray(b.bucket.vao);
        gl.drawArraysInstanced(gl.TRIANGLES, 0, b.batch.count, b.bucket.n);
      }
    }
    gl.bindVertexArray(null);
  }

  drawOverlay(prog, U) {
    const world = this.world;
    if (!world || !this.marks) return;
    prog.use().set(U).set(this.mapUniforms()).set({
      uMarks: this.marks,
      uOwnerColors: this.ownerColors,
      uBorders: this.overlayOpts.borders,
      uPolitical: this.overlayOpts.political,
      uGrid: this.overlayOpts.grid,
      uHover: this.hover,
      uSel: this.sel,
    });
    const ncx = Math.ceil(world.w / CHUNK);
    const ncy = Math.ceil(world.h / CHUNK);
    for (const copy of this.copies) {
      prog.set({ uCopyX: copy });
      for (let cy = 0; cy < ncy; cy++) {
        for (let cx = 0; cx < ncx; cx++) {
          if (!this.chunkVisible(cx, cy, copy)) continue;
          const c = this.terrain.get(cx, cy, this.far ? LOW_RES : RES, null);
          if (c) c.mesh.draw();
        }
      }
    }
  }

  drawMarkers(prog, U) {
    if (!this.markers && !this.ringMesh) return;
    prog.use().set(U).set(this.mapUniforms());
    for (const copy of this.copies) {
      prog.set({ uCopyX: copy });
      this.ringMesh?.draw();
      this.markers?.draw();
    }
  }

  drawParticles(prog, U) {
    if (!this.particles) return;
    prog.use().set(U).set(this.mapUniforms());
    for (const copy of this.copies) {
      prog.set({ uCopyX: copy });
      this.particles.draw();
    }
  }

  dispose() {
    const gl = this.gl;
    this.terrain.dispose();
    this.tex.dispose();
    for (const layer of Object.keys(this.layers)) this.setInstances(layer, []);
    for (const a of this.assets.values()) {
      a.dispose();
      a.lod?.dispose();
    }
    this.assets.clear();
    for (const m of [this.water, this.rivers, this.markers, this.ringMesh, this.particles]) m?.dispose();
    for (const t of [this.decal, this.light, this.marks]) if (t) gl.deleteTexture(t);
  }
}

/** The six planes (a, b, c, d: inside when ax + by + cz + d >= 0) of view-projection m. */
function frustumPlanes(m) {
  const row = (i) => [m[i], m[4 + i], m[8 + i], m[12 + i]];
  const r0 = row(0), r1 = row(1), r2 = row(2), r3 = row(3);
  const add = (a, b, s) => a.map((v, k) => v + s * b[k]);
  return [add(r3, r0, 1), add(r3, r0, -1), add(r3, r1, 1), add(r3, r1, -1), add(r3, r2, 1), add(r3, r2, -1)];
}

function hexRgbArr(hex) {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex ?? "");
  return m ? [parseInt(m[1], 16) / 255, parseInt(m[2], 16) / 255, parseInt(m[3], 16) / 255] : [1, 1, 1];
}

export { hexLinear };
