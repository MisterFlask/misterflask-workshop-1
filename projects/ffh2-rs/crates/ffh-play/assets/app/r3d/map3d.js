// The experimental 3D renderer of the map (docs/R3D.md): a fourth renderer beside flat, art and
// iso, chosen with `?renderer=3d` or Tools -> Options. Like the isometric one, MapView delegates
// draw(), screenToMap(), screenOf() and the drag-pan to it when `mapView.r3d` is set and keeps its
// own camera (cam.cx, cam.cy: the map point looked at; cam.ts: pixels per plot there), overlays
// and input; the game state comes from the same `S.map` / `S.view` the others draw.
//
// It draws into its own WebGL2 canvas under the map's 2D canvas; the 2D canvas stays on top for
// the input and the interface marks that must stay crisp (city names, stack badges, yields).
// Nothing here changes the game: it only reads the state.

import { S, me, tname } from "../state.js";
import * as T from "../tiles.js";
import { mul, perspective, lookAt, invert, xform4, ortho, vnorm, clamp, lerp, smoothstep } from "./math.js";
import { World, SEA } from "./world.js";
import { Pipeline } from "./pipeline.js";
import { Scene, bucketOf } from "./scene.js";
import { dressStatic, dressUnits, staticEnv, teamColour, standHeight } from "./dress.js";
import { loadConfig, saveConfig, withPreset, LIGHTING, PANEL } from "./config.js";
import { drawHud } from "./hud.js";
import "./assets/index.js";

const ZOOMS = [28, 40, 56, 72, 96, 128, 168, 220];
export const DEFAULT_ZOOM = 96;

export class Map3D {
  constructor(view) {
    this.view = view;
    this.cfg = loadConfig();
    this.canvas = document.createElement("canvas");
    this.canvas.className = "map3d";
    Object.assign(this.canvas.style, { position: "absolute", inset: "0", width: "100%", height: "100%", display: "block", pointerEvents: "none" });
    view.canvas.before(this.canvas);
    // The 2D canvas on top now only carries marks: transparent.
    view.canvas.style.background = "transparent";
    const gl = this.canvas.getContext("webgl2", { antialias: false, alpha: false, depth: false, stencil: false, preserveDrawingBuffer: true, powerPreference: "high-performance" });
    if (!gl) {
      this.canvas.remove();
      throw new Error("WebGL2 is not available");
    }
    this.gl = gl;
    this.pipeline = new Pipeline(gl);
    this.scene = new Scene(gl);
    this.world = null;
    this.mapSeen = null;
    this.viewSeen = null;
    this.selSeen = null;
    this.overlaySeen = "";
    this.ts = view.cam.ts;
    this.t0 = performance.now();
    this.time = 0;
    this.dirty = true;
    this.running = true;
    this.frameMs = [];
    // Quality ladder (docs/R3D.md §11): when the measured frame time stays high, step down (0 full
    // -> 1 paint filter off -> 2 + render scale 0.75) to keep the map responsive, and step back up
    // when it is fast again. Off with cfg.autoQuality = 0.
    this.autoLevel = 0;
    this.slowN = 0;
    this.fastN = 0;
    /** The last sync's costs by stage (ms): world, textures, dress, instances, paint, units. */
    this.syncMs = {};
    this.raf = requestAnimationFrame(() => this.loop());
    this.panel = null;
    this.addPanelButton();
    // A lost context (driver reset, too many contexts) drops back to the 2D renderer: main.js
    // listens for "ffh:r3d-lost" (the stored choice stays 3D, so a reload tries again).
    this.canvas.addEventListener("webglcontextlost", (ev) => {
      ev.preventDefault();
      console.warn("r3d: WebGL context lost");
      this.lost = true;
      setTimeout(() => globalThis.dispatchEvent?.(new CustomEvent("ffh:r3d-lost")), 0);
    });
  }

  dispose() {
    this.running = false;
    cancelAnimationFrame(this.raf);
    try {
      this.scene.dispose();
      this.pipeline.dispose();
    } catch {
      /* the context may be gone */
    }
    this.canvas.remove();
    this.panelBtn?.remove();
    this.panel?.remove();
    this.view.canvas.style.background = "";
    this.gl.getExtension("WEBGL_lose_context")?.loseContext();
  }

  zooms() {
    return ZOOMS;
  }

  // --- camera -----------------------------------------------------------------------------------

  /** The camera for the current view: eye, matrices, pitch, the plot rect seen. */
  camera(ts = this.ts) {
    const v = this.view;
    const vw = Math.max(1, v.vw), vh = Math.max(1, v.vh);
    const zt = clamp(Math.log(ts / ZOOMS[0]) / Math.log(ZOOMS[ZOOMS.length - 1] / ZOOMS[0]), 0, 1);
    // Looking down on the map when far, more over the shoulder when close.
    const pitch = ((this.cfg.pitch + 12 - zt * 24) * Math.PI) / 180;
    const fov = (this.cfg.fov * Math.PI) / 180;
    const dist = vh / (2 * ts * Math.tan(fov / 2));
    const target = [v.cam.cx, 0.08, -v.cam.cy];
    const eye = [target[0], target[1] + dist * Math.sin(pitch), target[2] + dist * Math.cos(pitch)];
    const viewM = lookAt(eye, target, [0, 1, 0]);
    const proj = perspective(fov, vw / vh, Math.max(0.05, dist * 0.25), dist * 2.5 + 12);
    const viewProj = mul(proj, viewM);
    return { eye, target, view: viewM, proj, viewProj, inv: invert(viewProj), invView: invert(viewM), dist, pitch, fov, vw, vh, tan: [Math.tan(fov / 2) * (vw / vh), Math.tan(fov / 2)] };
  }

  /** The ray (origin, direction) through CSS pixel (sx, sy). */
  ray(cam, sx, sy) {
    const nx = (sx / cam.vw) * 2 - 1;
    const ny = 1 - (sy / cam.vh) * 2;
    const a = xform4(cam.inv, nx, ny, -1, 1);
    const b = xform4(cam.inv, nx, ny, 1, 1);
    const p0 = [a[0] / a[3], a[1] / a[3], a[2] / a[3]];
    const p1 = [b[0] / b[3], b[1] / b[3], b[2] / b[3]];
    return [p0, vnorm([p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]])];
  }

  /** Where the ray through (sx, sy) meets the plane y = h: [X, Z]. */
  planeHit(cam, sx, sy, h = 0) {
    const [o, d] = this.ray(cam, sx, sy);
    const t = (h - o[1]) / (d[1] || -1e-6);
    return [o[0] + d[0] * t, o[2] + d[2] * t];
  }

  /** The ground point under CSS pixel (sx, sy) (marching the ray over the heightfield), as map
   *  coordinates [mx, my], or null. */
  groundAt(sx, sy) {
    const world = this.world;
    if (!world) return null;
    const cam = this.camera();
    const [o, d] = this.ray(cam, sx, sy);
    if (d[1] >= 0) return null;
    const ground = (p) => Math.max(SEA, world.heightAt(p[0], -p[2]));
    // From the height of the tallest peaks down to the sea floor.
    let t = (1.7 - o[1]) / d[1];
    const tEnd = (-0.5 - o[1]) / d[1];
    const step = 0.04;
    let prev = t;
    for (; t < tEnd; t += step) {
      const p = [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
      if (p[1] <= ground(p)) break;
      prev = t;
    }
    let lo = prev, hi = Math.min(t, tEnd);
    for (let k = 0; k < 8; k++) {
      const mid = (lo + hi) / 2;
      const p = [o[0] + d[0] * mid, o[1] + d[1] * mid, o[2] + d[2] * mid];
      if (p[1] <= ground(p)) hi = mid;
      else lo = mid;
    }
    const p = [o[0] + d[0] * hi, o[1] + d[1] * hi, o[2] + d[2] * hi];
    return [p[0], -p[2]];
  }

  screenToMap(sx, sy) {
    const map = this.view.map;
    if (!map) return null;
    const g = this.groundAt(sx, sy);
    if (!g) return null;
    const x = T.wrapCoord(Math.floor(g[0]), map.w, map.wrap_x);
    const y = T.wrapCoord(Math.floor(g[1]), map.h, map.wrap_y);
    return x === null || y === null ? null : { x, y };
  }

  /** The CSS-pixel position of world point p under camera cam (null behind the eye). */
  project(cam, p) {
    const c = xform4(cam.viewProj, p[0], p[1], p[2], 1);
    if (c[3] <= 0) return null;
    return { x: ((c[0] / c[3]) * 0.5 + 0.5) * cam.vw, y: (1 - ((c[1] / c[3]) * 0.5 + 0.5)) * cam.vh, depth: c[3] };
  }

  /** The world x of plot column x nearest the camera (a wrapping map's copies). */
  nearX(mx) {
    const map = this.view.map;
    if (!map?.wrap_x) return mx;
    return mx + Math.round((this.view.cam.cx - mx) / map.w) * map.w;
  }

  screenOf(x, y) {
    const cam = this.camera();
    const h = this.world ? this.world.plotHeight(x, y) : 0;
    const p = this.project(cam, [this.nearX(x + 0.5), h, -(y + 0.5)]);
    return p ? { x: p.x, y: p.y, ts: this.ts } : { x: -1e4, y: -1e4, ts: this.ts };
  }

  /** The camera centre after a drag of (dx, dy) CSS pixels from (startCx, startCy): the ground
   *  under the pointer follows it. */
  panDelta(startCx, startCy, dx, dy) {
    const cam = this.camera();
    return { cx: startCx - dx / this.ts, cy: startCy + dy / (this.ts * Math.sin(cam.pitch)) };
  }

  minimap() {
    return null;
  }

  // --- state ------------------------------------------------------------------------------------

  /** Brings the scene up to date with the game state (cheap when nothing changed). */
  sync() {
    const map = this.view.map;
    if (!map) return false;
    let mapChanged = false;
    let mapDirty = null;
    const T0 = performance.now();
    if (map !== this.mapSeen) {
      const prev = this.mapSeen;
      const dirty = prev && prev.w === map.w && prev.h === map.h ? changedPlots(prev, map) : null;
      mapDirty = dirty;
      if (!prev || !dirty || dirty.length) {
        const before = this.world;
        this.world = new World(map);
        // A lake is decided by its whole body (it touches no unrevealed plot): revealing one plot
        // can change the floor of water several plots away.
        if (dirty && before?.lake) for (let i = 0; i < map.w * map.h; i++) if (before.lake[i] !== this.world.lake[i]) dirty.push(i);
        const T1 = performance.now();
        this.scene.setWorld(this.world, map, dirty);
        mapChanged = true;
        this.syncMs.world = T1 - T0;
        this.syncMs.textures = performance.now() - T1;
      }
      this.mapSeen = map;
    }
    const viewChanged = S.view !== this.viewSeen;
    if (mapChanged || viewChanged) {
      const colors = [];
      for (const p of S.players.values()) colors[p.id] = teamColour(p.id);
      for (let i = 0; i < 64; i++) if (!colors[i]) colors[i] = T.playerColor(i);
      this.scene.setOwnerColors(colors);
      const T2 = performance.now();
      // The static dressing, redressed where something changed: a plot's dressing depends on the
      // plot and its neighbours (glades where units stand, a city's wood opening in front of it),
      // and the ground's height on the plots around (two plots after a map change). A unit's
      // move then rebuilds a few instance buckets instead of the whole map's.
      const env = staticEnv(this.world);
      const n = map.w * map.h;
      const cityKey = new Map();
      for (const [i, c] of env.cityAt) cityKey.set(i, `${c.id}|${c.owner}|${c.size}|${c.walls}|${c.capital}|${c.name}`);
      const civKey = [...S.players.values()].map((p) => `${p.id}:${p.civ}`).join(",");
      const seen = this.dressSeen;
      let plots = null;
      if (seen && seen.n === n && seen.civKey === civKey && this.byPlot && (!mapChanged || mapDirty)) {
        const changed = new Set(mapChanged ? mapDirty : []);
        for (const i of env.unitAt) if (!seen.unitAt.has(i)) changed.add(i);
        for (const i of seen.unitAt) if (!env.unitAt.has(i)) changed.add(i);
        for (const [i, k] of cityKey) if (seen.cityKey.get(i) !== k) changed.add(i);
        for (const i of seen.cityKey.keys()) if (!cityKey.has(i)) changed.add(i);
        plots = this.around(changed, mapChanged ? 2 : 1);
      }
      const st = dressStatic(this.world, map, plots, env);
      const T3 = performance.now();
      if (!plots) {
        this.byPlot = new Array(n);
        for (const [i, d] of st.byPlot) this.byPlot[i] = d;
        this.scene.setInstances("static", st.list);
      } else if (plots.length) {
        const chunks = new Set();
        for (const i of plots) for (const it of this.byPlot[i]?.list ?? []) chunks.add(bucketOf(it));
        for (const [i, d] of st.byPlot) {
          this.byPlot[i] = d;
          for (const it of d.list) chunks.add(bucketOf(it));
        }
        const items = [];
        for (const d of this.byPlot) if (d) for (const it of d.list) if (chunks.has(bucketOf(it))) items.push(it);
        this.scene.setInstances("static", items, chunks);
      }
      const emitters = [], cities = [];
      let count = 0;
      for (const d of this.byPlot) {
        if (!d) continue;
        for (const e of d.emitters) emitters.push(e);
        if (d.city) cities.push(d.city);
        count += d.list.length;
      }
      this.cities = cities;
      this.emitters = emitters;
      this.scene.setParticles(emitters);
      const T4 = performance.now();
      // The paint layers (roads, fields, settlement ground, light pools) change with the map, the
      // cities and the light.
      const paintKey = `${this.cfg.lighting};${[...cityKey].join(";")}`;
      if (mapChanged || paintKey !== this.paintSeen) {
        this.scene.setPaint(this.world, map, cities);
        this.paintSeen = paintKey;
      }
      this.dressSeen = { n, civKey, unitAt: env.unitAt, cityKey };
      this.syncMs.dress = T3 - T2;
      this.syncMs.instances = T4 - T3;
      this.syncMs.paint = performance.now() - T4;
      this.syncMs.redressed = plots ? plots.length : n;
      this.staticCount = count;
      this.viewSeen = S.view;
      this.selSeen = null;
    }
    const selKey = JSON.stringify(S.sel?.units ?? []) + JSON.stringify(this.view.overlay.units ?? []);
    if (selKey !== this.selSeen || viewChanged || mapChanged) {
      const T5 = performance.now();
      const u = dressUnits(this.world, map);
      this.stacks = u.stacks;
      this.startTweens(u.list);
      this.unitList = u.list;
      this.scene.setInstances("units", this.tweened(performance.now()));
      this.syncMs.units = performance.now() - T5;
      this.unitCount = u.list.length;
      this.selSeen = selKey;
      this.overlaySeen = "";
    }
    // Overlays: hover, selection, targets, path.
    const o = this.view.overlay;
    const key = JSON.stringify([o.hover, o.sel, o.targets, o.path?.nodes?.length, o.path?.nodes?.[o.path.nodes.length - 1], o.waypoints, this.view.layer]);
    if (key !== this.overlaySeen) {
      this.overlaySeen = key;
      this.scene.hover = o.hover ? [o.hover.x, o.hover.y, 0, 1] : [0, 0, 0, 0];
      this.scene.sel = o.sel ? [o.sel.x, o.sel.y, 0, 1] : [0, 0, 0, 0];
      const targets = Array.isArray(o.targets) ? o.targets : [];
      const nodes = o.path?.ok ? o.path.nodes ?? [] : [];
      this.scene.setMarks(targets, nodes.map((n) => [n.x, n.y]));
      const rings = (this.stacks ?? []).map((s) => ({
        x: this.nearXMap(s.x + 0.5 + s.spot[0]),
        y: s.y + 0.5 - s.spot[1],
        r: 0.17,
        color: s.picked ? "#fff2a8" : s.team,
        rim: s.picked ? "#3a2a08" : "#140e0a",
        width: s.picked ? 0.05 : 0.04,
        fill: s.picked ? 0.18 : 0.1,
      }));
      let path = null;
      if (nodes.length > 1) path = nodes.map((n) => [n.x + 0.5, n.y + 0.5]);
      const danger = o.path?.attack ? "#ff5a40" : o.path?.declare_war_team != null ? "#ffa040" : "#ffe9a0";
      this.scene.setMarkers(rings, unwrapPath(path, this.world), danger);
      this.scene.overlayOpts = {
        borders: this.cfg.borders,
        political: this.view.layer === "political" ? 1 : 0,
        grid: this.cfg.grid,
      };
    }
    return true;
  }

  nearXMap(mx) {
    return mx;
  }

  /** The plots within `r` (Chebyshev) of the plots in `set`, as an array of plot indices. */
  around(set, r) {
    const out = new Set();
    const w = this.world.w;
    for (const i of set) {
      const x = i % w, y = (i - x) / w;
      for (let dy = -r; dy <= r; dy++) {
        for (let dx = -r; dx <= r; dx++) {
          const j = this.world.index(x + dx, y + dy);
          if (j >= 0) out.add(j);
        }
      }
    }
    return [...out];
  }

  // --- unit movement --------------------------------------------------------------------------

  /** Units that changed plot since the last dressing walk there (a short glide with a step bob)
   *  and keep facing their heading -- turned at most 100 degrees from the camera, so a figure
   *  never shows only its back. A jump of more than three plots (a teleport, a load) is not
   *  animated; nor is anything under prefers-reduced-motion. */
  startTweens(list) {
    const now = performance.now();
    this.tweens ??= new Map();
    this.facing ??= new Map();
    this.lastPos ??= new Map();
    const motion = !this.view.reduceMotion();
    const w = this.world?.w ?? 1;
    for (const it of list) {
      if (it.uid === undefined) continue;
      const prev = this.lastPos.get(it.uid);
      if (prev) {
        let dx = it.x - prev.x;
        if (this.world?.wrapX) dx -= Math.round(dx / w) * w;
        const dz = it.z - prev.z;
        const d = Math.hypot(dx, dz);
        if (d > 0.3 && d < 3.2) {
          const heading = Math.atan2(dx, dz);
          const lim = (100 * Math.PI) / 180;
          this.facing.set(it.uid, Math.max(-lim, Math.min(lim, heading)));
          if (motion) this.tweens.set(it.uid, { fx: it.x - dx, fz: it.z - dz, fy: prev.y, t0: now, dur: 380 + d * 120 });
        }
      }
      this.lastPos.set(it.uid, { x: it.x, y: it.y, z: it.z });
    }
  }

  /** The unit instances at time `now`, the walking ones partway along. */
  tweened(now) {
    const out = [];
    for (const it of this.unitList ?? []) {
      const f = it.uid !== undefined ? this.facing?.get(it.uid) : undefined;
      let o = f !== undefined ? { ...it, yaw: f } : it;
      const tw = it.uid !== undefined ? this.tweens?.get(it.uid) : undefined;
      if (tw) {
        const t = Math.min(1, (now - tw.t0) / tw.dur);
        if (t >= 1) this.tweens.delete(it.uid);
        else {
          const e = t * t * (3 - 2 * t);
          const x = tw.fx + (it.x - tw.fx) * e;
          const z = tw.fz + (it.z - tw.fz) * e;
          // On foot over the ground's relief; ships and fliers straight between their heights.
          const y = it.ground && this.world ? standHeight(this.world, x, -z, it.foot) : tw.fy + (it.y - tw.fy) * e;
          o = { ...o, x, z, y: y + (it.ground ? Math.abs(Math.sin(t * Math.PI * 3)) * 0.012 : 0) };
        }
      }
      out.push(o);
    }
    return out;
  }

  // --- frames -----------------------------------------------------------------------------------

  /** MapView's draw: the next frame will render (the loop below draws). */
  draw() {
    this.dirty = true;
  }

  loop() {
    if (!this.running) return;
    this.raf = requestAnimationFrame(() => this.loop());
    if (this.lost) return;
    const target = this.view.cam.ts;
    if (Math.abs(this.ts - target) > 0.05) {
      this.ts = this.view.reduceMotion() ? target : lerp(this.ts, target, 0.25);
      this.dirty = true;
    } else this.ts = target;
    const animate = this.cfg.animate && !this.view.reduceMotion();
    if (this.tweens?.size) {
      this.scene.setInstances("units", this.tweened(performance.now()));
      this.dirty = true;
    }
    if (!this.dirty && !animate) return;
    // Animation at ~30 frames per second is enough for wind and water.
    const now = performance.now();
    if (!this.dirty && now - (this.lastFrame ?? 0) < 32) return;
    this.lastFrame = now;
    this.dirty = false;
    try {
      this.render(animate ? (now - this.t0) / 1000 : this.time);
    } catch (e) {
      console.error("r3d frame:", e);
    }
  }

  /** The frame's uniforms and light, for camera `cam`. */
  frameState(cam, time) {
    const cfg = this.cfg;
    const L = LIGHTING[cfg.lighting] ?? LIGHTING.daylight;
    const sunDir = vnorm(L.sunDir);
    const sun = L.sun[0].map((c) => c * L.sun[1]);
    const sky = L.sky[0].map((c) => c * L.sky[1]);
    const ground = L.ground[0].map((c) => c * L.ground[1]);
    const plain = cfg.stylize <= 0;
    // The sun's shadow frustum: the ground in view (plus a margin for what casts into it),
    // seen along the sun, snapped to whole shadow texels so panning does not make it crawl.
    const r = this.visibleRect(cam);
    const cx = (r.x0 + r.x1) / 2, cy = (r.y0 + r.y1) / 2;
    const center = [cx, 0, -cy];
    const lv = lookAt([center[0] + sunDir[0] * 20, sunDir[1] * 20, center[2] + sunDir[2] * 20], center, [0, 1, 0]);
    let lx0 = Infinity, lx1 = -Infinity, ly0 = Infinity, ly1 = -Infinity, lz0 = Infinity, lz1 = -Infinity;
    for (const X of [r.x0 - 1.5, r.x1 + 1.5]) {
      for (const Y of [r.y0 - 1.5, r.y1 + 1.5]) {
        for (const h of [-0.5, 1.6]) {
          const p = xform4(lv, X, h, -Y, 1);
          lx0 = Math.min(lx0, p[0]);
          lx1 = Math.max(lx1, p[0]);
          ly0 = Math.min(ly0, p[1]);
          ly1 = Math.max(ly1, p[1]);
          lz0 = Math.min(lz0, p[2]);
          lz1 = Math.max(lz1, p[2]);
        }
      }
    }
    const size = Math.max(lx1 - lx0, ly1 - ly0);
    const texel = size / 2048;
    const sx = Math.floor((lx0 + lx1) / 2 / texel) * texel, sy = Math.floor((ly0 + ly1) / 2 / texel) * texel;
    const lp = ortho(sx - size / 2, sx + size / 2, sy - size / 2, sy + size / 2, -lz1 - 5, -lz0 + 5);
    const lightViewProj = mul(lp, lv);
    const U = {
      uViewProj: cam.viewProj,
      uView: cam.view,
      uEye: cam.eye,
      uTime: time,
      uSunDir: sunDir,
      uSunColor: plain ? [2.3, 2.25, 2.1] : sun,
      uSkyColor: plain ? [0.42, 0.45, 0.5] : sky,
      uGroundColor: plain ? [0.3, 0.3, 0.3] : ground,
      uHazeColor: L.haze,
      uShadowTint: L.shadowTint,
      uWrapLight: cfg.wrapLight,
      uBands: cfg.bands,
      uRim: cfg.rim,
      uPaint: cfg.paint,
      uHaze: cfg.haze,
      uHazeRange: [cam.dist * 1.02, cam.dist * 1.9],
      uMist: cfg.mist,
      uFogWar: cfg.fogWar,
      uStylize: cfg.stylize,
      uClouds: cfg.clouds,
      uTerrainShadow: cfg.terrainShadow ?? 1,
      uDabs: cfg.dabs ?? 1,
      uShadowMat: lightViewProj,
      uShadowTexel: [1 / 2048, 1 / 2048],
      uWind: cfg.wind,
      uEmissive: cfg.emissive,
      uPx: this.ts,
    };
    return { uniforms: U, lightViewProj, tan: cam.tan, invView: cam.invView, exposure: L.exposure, background: [0.05, 0.045, 0.04], scale: cfg.renderScale };
  }

  /** The map rect the camera sees (map units; x unwrapped around the camera). */
  visibleRect(cam) {
    let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
    for (const [sx, sy] of [[0, 0], [cam.vw, 0], [0, cam.vh], [cam.vw, cam.vh]]) {
      for (const h of [-0.2, 0.6]) {
        const [X, Z] = this.planeHit(cam, sx, sy, h);
        x0 = Math.min(x0, X);
        x1 = Math.max(x1, X);
        y0 = Math.min(y0, -Z);
        y1 = Math.max(y1, -Z);
      }
    }
    return { x0, x1, y0, y1 };
  }

  render(time) {
    const gl = this.gl;
    const v = this.view;
    if (!this.sync()) {
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.clearColor(0.05, 0.045, 0.04, 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
      v.ctx.setTransform(1, 0, 0, 1, 0, 0);
      v.ctx.clearRect(0, 0, v.canvas.width, v.canvas.height);
      return;
    }
    const t0 = performance.now();
    this.time = time;
    const cw = Math.max(1, Math.round(v.vw * v.dpr)), ch = Math.max(1, Math.round(v.vh * v.dpr));
    if (this.canvas.width !== cw || this.canvas.height !== ch) {
      this.canvas.width = cw;
      this.canvas.height = ch;
    }
    // The quality ladder lowers the paint filter and the render scale when frames are slow.
    const qc = this.qualityCfg();
    const scale = qc.renderScale;
    this.pipeline.resize(Math.max(1, Math.round(cw * scale)), Math.max(1, Math.round(ch * scale)));
    const cam = this.camera();
    const fr = this.frameState(cam, time);
    fr.scale = v.dpr * scale;
    // The brush grows with the zoom: the painted marks belong to the world, not to the screen.
    fr.paintZoom = clamp(this.ts / 96, 0.6, 1.7);
    // The filter erases what is narrower than its radius: restrained at the gameplay zooms (where
    // a road or a pool's rim is a few pixels), fuller close up (where the same things are large
    // enough to become painted patches).
    fr.paintStrength = 0.35 + 0.65 * smoothstep(100, 190, this.ts);
    const rect = this.visibleRect(cam);
    const map = v.map;
    const copies = [];
    if (map.wrap_x) {
      for (let k = Math.floor((rect.x0 - 2) / map.w); k <= Math.floor((rect.x1 + 2) / map.w); k++) copies.push(k * map.w);
    } else copies.push(0);
    this.scene.setView(rect, copies, this.ts < 48, this.ts, cam.viewProj);
    this.pipeline.measure = !!this.measuring;
    this.pipeline.render(fr, this.scene, qc, cw, ch);
    this.cam = cam;
    drawHud(this, cam);
    const ms = performance.now() - t0;
    this.frameMs.push(ms);
    if (this.frameMs.length > 120) this.frameMs.shift();
    this.tuneQuality(ms);
  }

  /** The config with the quality ladder applied: at level 1 the Kuwahara paint filter is off, at
   *  level 2 the render scale drops to 0.75 as well. */
  qualityCfg() {
    if (this.autoLevel <= 0) return this.cfg;
    return {
      ...this.cfg,
      kuwahara: 0,
      renderScale: this.cfg.renderScale * (this.autoLevel >= 2 ? 0.75 : 1),
    };
  }

  /** Steps the quality ladder from the measured frame time (hysteresis: several sustained slow or
   *  fast frames before a change, so a single hitch does not flip it; a single lab render never
   *  accumulates enough frames to change it). */
  tuneQuality(ms) {
    if (!(this.cfg.autoQuality ?? 1)) {
      this.autoLevel = 0;
      return;
    }
    if (ms > 42) {
      this.slowN++;
      this.fastN = 0;
    } else if (ms < 24) {
      this.fastN++;
      this.slowN = 0;
    } else {
      this.slowN = 0;
      this.fastN = 0;
    }
    if (this.slowN >= 6 && this.autoLevel < 2) {
      this.autoLevel++;
      this.slowN = 0;
    } else if (this.fastN >= 24 && this.autoLevel > 0) {
      this.autoLevel--;
      this.fastN = 0;
    }
  }

  // --- the tuning panel -------------------------------------------------------------------------

  addPanelButton() {
    const b = document.createElement("button");
    b.textContent = "3D";
    b.title = "The 3D renderer's look: presets and every stage of the painterly pipeline";
    b.className = "r3d-btn";
    Object.assign(b.style, { position: "absolute", left: "10px", top: "10px", zIndex: 5, padding: "2px 8px" });
    b.addEventListener("click", () => this.togglePanel());
    this.view.wrap.append(b);
    this.panelBtn = b;
  }

  togglePanel() {
    if (this.panel) {
      this.panel.remove();
      this.panel = null;
      return;
    }
    const p = document.createElement("div");
    p.className = "r3d-panel";
    Object.assign(p.style, { position: "absolute", left: "10px", top: "40px", zIndex: 6, background: "rgba(14,16,24,0.92)", color: "#e8e0cc", padding: "8px 10px", font: "12px system-ui, sans-serif", maxHeight: "70%", overflowY: "auto", border: "1px solid #6a5a3a", borderRadius: "4px", width: "250px" });
    const row = (label, el) => {
      const r = document.createElement("label");
      Object.assign(r.style, { display: "flex", justifyContent: "space-between", alignItems: "center", gap: "6px", margin: "2px 0", whiteSpace: "pre" });
      r.append(label, el);
      p.append(r);
    };
    const presets = document.createElement("div");
    for (const name of ["painterly", "plain"]) {
      const b = document.createElement("button");
      b.textContent = name;
      b.style.marginRight = "4px";
      b.addEventListener("click", () => {
        this.setConfig(withPreset(this.cfg, name));
        this.togglePanel();
        this.togglePanel();
      });
      presets.append(b);
    }
    p.append(presets);
    const light = document.createElement("select");
    for (const k of Object.keys(LIGHTING)) light.append(new Option(k, k, false, this.cfg.lighting === k));
    light.addEventListener("change", () => this.setConfig({ ...this.cfg, lighting: light.value }));
    row("Light", light);
    for (const [key, label, kind, min, max, step] of PANEL) {
      const el = document.createElement("input");
      if (kind === "bool") {
        el.type = "checkbox";
        el.checked = !!this.cfg[key];
        el.addEventListener("change", () => this.setConfig({ ...this.cfg, [key]: el.checked }));
      } else {
        el.type = "range";
        Object.assign(el, { min, max, step, value: this.cfg[key] });
        el.style.width = "90px";
        el.addEventListener("input", () => this.setConfig({ ...this.cfg, [key]: Number(el.value) }));
      }
      row(label, el);
    }
    const stats = document.createElement("pre");
    stats.style.margin = "6px 0 0";
    p.append(stats);
    const tick = () => {
      if (!this.panel) return;
      stats.textContent = this.statsText();
      setTimeout(tick, 1000);
    };
    this.view.wrap.append(p);
    this.panel = p;
    tick();
  }

  setConfig(cfg) {
    const relight = cfg.lighting !== this.cfg.lighting;
    this.cfg = cfg;
    saveConfig(cfg);
    if (relight) this.viewSeen = null;
    this.overlaySeen = "";
    this.dirty = true;
  }

  statsText() {
    const f = this.frameMs;
    const avg = f.length ? f.reduce((a, b) => a + b, 0) / f.length : 0;
    const gpu = Object.entries(this.pipeline.stats.ms).map(([k, s]) => `${k} ${(s.sum / Math.max(1, s.n)).toFixed(2)}`).join("\n");
    const q = this.autoLevel > 0 ? `  quality -${this.autoLevel}` : "";
    return `cpu ${avg.toFixed(1)} ms/frame${q}\nprops ${this.staticCount ?? 0} + units ${this.unitCount ?? 0}\n${gpu}`;
  }
}

/** The plots whose terrain-shaping layers differ between two maps of the same size. */
function changedPlots(a, b) {
  const out = [];
  for (let i = 0; i < a.vis.length; i++) {
    if (a.vis[i] !== b.vis[i] || a.plot_type[i] !== b.plot_type[i] || a.terrain[i] !== b.terrain[i] || a.feature[i] !== b.feature[i] || a.river[i] !== b.river[i] || a.improvement[i] !== b.improvement[i] || a.route[i] !== b.route[i] || a.owner[i] !== b.owner[i] || a.city[i] !== b.city[i] || a.bonus[i] !== b.bonus[i]) out.push(i);
  }
  return out;
}

/** A path across a wrapping map's seam, kept continuous (each node next to the one before). */
function unwrapPath(path, world) {
  if (!path || !world?.wrapX) return path;
  const out = [path[0]];
  for (let k = 1; k < path.length; k++) {
    const prev = out[k - 1];
    let x = path[k][0];
    x += Math.round((prev[0] - x) / world.w) * world.w;
    out.push([x, path[k][1]]);
  }
  return out;
}

export { tname, me };
