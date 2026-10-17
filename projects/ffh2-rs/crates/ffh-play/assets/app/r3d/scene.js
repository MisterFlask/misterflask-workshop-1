// The 3D scene on the GPU (docs/R3D.md "Scene"): terrain chunks, the sea and the rivers, the
// dressing (instanced props, bucketed by asset and map chunk so the frustum test skips whole
// buckets), team rings and path ribbons, particles, and the map textures the shaders read. The
// pipeline (pipeline.js) calls the draw* methods with its programs; each draws every copy of the
// map the camera sees (a wrapping map repeats east and west).

import { ATTR, Mesh, texture, upload, INSTANCE_FLOATS, PROP_LAYOUT } from "./gl.js";
import { TerrainMeshes, MapTextures, CHUNK, RES, LOW_RES } from "./terrain.js";
import { buildAsset } from "./assets/registry.js";
import { rasterDecals, rasterLight } from "./decals.js";
import { SEA } from "./world.js";
import { hexLinear } from "./math.js";
import { rng } from "./noise.js";

const RIVER_LAYOUT = { aPos: 3, aExtra: 4 };
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
  }

  /** A VAO over this asset's vertices and a new instance buffer. */
  bucket(instances) {
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
    const ibuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, ibuf);
    gl.bufferData(gl.ARRAY_BUFFER, instances, gl.STATIC_DRAW);
    let o = 0;
    for (const name of ["iPosRot", "iScale", "iTint", "iTeam"]) {
      gl.enableVertexAttribArray(ATTR[name]);
      gl.vertexAttribPointer(ATTR[name], 4, gl.FLOAT, false, INSTANCE_FLOATS * 4, o);
      gl.vertexAttribDivisor(ATTR[name], 1);
      o += 16;
    }
    gl.bindVertexArray(null);
    return { vao, ibuf, n: instances.length / INSTANCE_FLOATS };
  }

  dispose() {
    this.gl.deleteBuffer(this.vbo);
  }
}

/** Instance records (dress.js) -> buckets by asset and chunk. */
function packBuckets(list) {
  const by = new Map();
  for (const it of list) {
    const ck = `${Math.floor(it.x / CHUNK)},${Math.floor(-it.z / CHUNK)}`;
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
    this.tex.update(world, map);
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
  setInstances(layer, list) {
    const gl = this.gl;
    for (const b of this.layers[layer]) {
      gl.deleteVertexArray(b.bucket.vao);
      gl.deleteBuffer(b.bucket.ibuf);
    }
    const out = [];
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
        out.push({ batch, cx, cy, top, bucket: batch.bucket(arr) });
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
    for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) v.push(Math.min(w, i * step), SEA, -Math.min(h, j * step), 0, 0, 0, 0);
    for (let j = 0; j + 1 < ny; j++) {
      for (let i = 0; i + 1 < nx; i++) {
        const a = j * nx + i;
        idx.push(a, a + nx, a + 1, a + 1, a + nx, a + nx + 1);
      }
    }
    this.water = new Mesh(gl, new Float32Array(v), RIVER_LAYOUT, { indices: new Uint32Array(idx) });
  }

  /** The rivers: ribbons along the smoothed river lines, their surface a little under the banks. */
  buildRivers() {
    const gl = this.gl;
    this.rivers?.dispose();
    this.rivers = null;
    const world = this.world;
    const v = [];
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
      let along = 0;
      // Wider than the bed: the banks cut the water's edge (an organic shore line).
      const width = 0.15;
      const ring = [];
      for (let k = 0; k < pts.length; k++) {
        const p = pts[k];
        const a = pts[Math.max(0, k - 1)], b = pts[Math.min(pts.length - 1, k + 1)];
        let tx = b[0] - a[0], ty = b[1] - a[1];
        const l = Math.hypot(tx, ty) || 1;
        tx /= l;
        ty /= l;
        if (k > 0) along += Math.hypot(p[0] - pts[k - 1][0], p[1] - pts[k - 1][1]);
        const y = Math.max(SEA + 0.004, world.heightAt(p[0], p[1]) + 0.045);
        ring.push([[p[0] - ty * width, y, -(p[1] + tx * width)], [p[0] + ty * width, y, -(p[1] - tx * width)], along]);
      }
      for (let k = 0; k + 1 < ring.length; k++) {
        const [a0, a1, s0] = ring[k];
        const [b0, b1, s1] = ring[k + 1];
        const V = (p, u, s) => v.push(p[0], p[1], p[2], u, s, 0, 0);
        V(a0, -1, s0); V(b0, -1, s1); V(a1, 1, s0);
        V(a1, 1, s0); V(b0, -1, s1); V(b1, 1, s1);
      }
    }
    if (v.length) this.rivers = new Mesh(gl, new Float32Array(v), RIVER_LAYOUT);
  }

  /** Team rings (unit stacks, cities) and the path ribbon, draped on the ground. `rings`:
   *  [{x, y (map plot centre), r, color "#hex", rim "#hex", width}], `path`: [[mx, my], ...]. */
  setMarkers(rings, path, pathColor) {
    const gl = this.gl;
    this.markers?.dispose();
    this.markers = null;
    const world = this.world;
    if (!world) return;
    const v = [];
    const push = (mx, my, lift, c, a) => v.push(mx, world.heightAt(mx, my) + lift, -my, c[0], c[1], c[2], a);
    const annulus = (cx, cy, r0, r1, c, a, lift) => {
      const n = 40;
      for (let k = 0; k < n; k++) {
        const a0 = (k / n) * Math.PI * 2, a1 = ((k + 1) / n) * Math.PI * 2;
        const p = (r, t) => [cx + Math.cos(t) * r, cy + Math.sin(t) * r * 0.92];
        const q = [p(r0, a0), p(r1, a0), p(r1, a1), p(r0, a1)];
        const tri = [q[0], q[1], q[2], q[0], q[2], q[3]];
        for (const [mx, my] of tri) push(mx, my, lift, c, a);
      }
    };
    for (const g of rings) {
      const c = hexRgbArr(g.color);
      const rim = hexRgbArr(g.rim ?? "#1a1410");
      const r = g.r ?? 0.3;
      const wd = g.width ?? 0.045;
      annulus(g.x, g.y, r - wd * 0.5, r + wd * 0.5, rim, 0.75, 0.014);
      annulus(g.x, g.y, r - wd * 0.32, r + wd * 0.32, c, 0.95, 0.016);
      if (g.fill) annulus(g.x, g.y, 0, r - wd * 0.5, c, g.fill, 0.012);
    }
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

  /** Particles from the emitters (dress.js): [{x, y, z, color, size, rise, count, spread}]. */
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
        v.push(e.x + r.range(-sp, sp), e.y + r.range(0, 0.05), e.z + r.range(-sp, sp), r(), (e.rise ?? 0.25) * r.range(0.6, 1.4), r(), c[0], c[1], c[2], (e.size ?? 0.02) * r.range(0.7, 1.3));
      }
    }
    if (v.length) this.particles = new Mesh(gl, new Float32Array(v), PARTICLE_LAYOUT, { mode: gl.POINTS });
  }

  // --- drawing ----------------------------------------------------------------------------------

  /** Sets the frame's view: the map rect seen (x0..x1, y0..y1 in map units, x unwrapped), the
   *  copies of the map in view, and whether distant chunks may use the coarse terrain. */
  setView(rect, copies, far) {
    this.visible = rect;
    this.copies = copies;
    this.far = far;
    this.budget.n = 6;
    /** What the last frame's shaded prop pass drew: draw calls, instances, triangles. */
    this.drawn = { draws: 0, instances: 0, tris: 0, terrainTris: 0 };
  }

  chunkVisible(cx, cy, copy, margin = 1.5) {
    const r = this.visible;
    if (!r) return true;
    const x0 = cx * CHUNK + copy, y0 = cy * CHUNK;
    return x0 + CHUNK >= r.x0 - margin && x0 <= r.x1 + margin && y0 + CHUNK >= r.y0 - margin && y0 <= r.y1 + margin;
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
    if (!depthOnly) prog.set(this.mapUniforms());
    for (const copy of this.copies) {
      prog.set({ uCopyX: copy, uRiver: 0 });
      this.water.draw();
      if (this.rivers) {
        prog.set({ uRiver: 1 });
        this.rivers.draw();
      }
    }
  }

  drawProps(prog, U, shadow) {
    const gl = this.gl;
    prog.use().set(U).set(this.mapUniforms());
    for (const copy of this.copies) {
      prog.set({ uCopyX: copy });
      for (const layer of ["static", "units"]) {
        for (const b of this.layers[layer]) {
          if (!this.chunkVisible(b.cx, b.cy, copy, shadow ? 3 : 1.5)) continue;
          gl.bindVertexArray(b.bucket.vao);
          gl.drawArraysInstanced(gl.TRIANGLES, 0, b.batch.count, b.bucket.n);
          if (!shadow && this.drawn && prog.name === "prop") {
            this.drawn.draws++;
            this.drawn.instances += b.bucket.n;
            this.drawn.tris += (b.batch.count / 3) * b.bucket.n;
          }
        }
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
    if (!this.markers) return;
    prog.use().set(U).set(this.mapUniforms());
    for (const copy of this.copies) {
      prog.set({ uCopyX: copy });
      this.markers.draw();
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
    for (const a of this.assets.values()) a.dispose();
    this.assets.clear();
    for (const m of [this.water, this.rivers, this.markers, this.particles]) m?.dispose();
    for (const t of [this.decal, this.light, this.marks]) if (t) gl.deleteTexture(t);
  }
}

function hexRgbArr(hex) {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex ?? "");
  return m ? [parseInt(m[1], 16) / 255, parseInt(m[2], 16) / 255, parseInt(m[3], 16) / 255] : [1, 1, 1];
}

export { hexLinear };
