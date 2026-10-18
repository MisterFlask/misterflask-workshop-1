// The render pipeline of the 3D map (docs/R3D.md "Pipeline"):
//
//   shadow   sun's depth map over the ground in view (texel-snapped: no shimmer when panning)
//   scene    terrain, props -> colour (linear HDR, a = keep sharp), view normal + linear depth,
//            emission (+ object id; negated on leaves)
//   water    the scene grabbed, then the sea and rivers over it: seen through to the floor,
//            mirroring the sky and what stands round them, writing depth
//   ssao     half resolution, depth-aware blur
//   glow     emission, blurred at half resolution
//   compose  occlusion, glow, exposure, filmic curve -> display colour
//   paint    structure tensor -> anisotropic Kuwahara (units kept sharp)
//   finish   ink outlines, grade, world-anchored paper grain, vignette
//   overlay  borders, political wash, grid, marks, rings, particles -- over the finished frame,
//            depth-tested against the scene and ghosted through what stands in front
//   blit     to the canvas
//
// Every stage after `scene` is optional (config.js); the timings of each are measured with GPU
// timer queries when the browser offers them (`stats`).

import { Program, texture, framebuffer, depthBuffer, depthTexture, fullscreen, FULLSCREEN_VS } from "./gl.js";
import * as SH from "./shaders.js";

const SHADOW_SIZE = 2048;
// The terrain casts into a map of its own, coarser (its shadows are broad and soft) and applied at
// part strength (cfg.terrainShadow): a hill's far side is shaded by the light model, its cast
// shadow on the plain behind only darkens it a little, as a painter would keep it.
const TERRAIN_SHADOW_SIZE = 1024;

export class Pipeline {
  constructor(gl) {
    this.gl = gl;
    this.hdr = !!gl.getExtension("EXT_color_buffer_float") || !!gl.getExtension("EXT_color_buffer_half_float");
    gl.getExtension("OES_texture_float_linear");
    this.timer = gl.getExtension("EXT_disjoint_timer_query_webgl2");
    this.fs = fullscreen(gl);
    const P = (name, vs, fs, defs) => new Program(gl, name, vs, fs, defs);
    const C = SH.COMMON;
    this.p = {
      terrain: P("terrain", C + SH.TERRAIN_VS, C + SH.TERRAIN_FS),
      prop: P("prop", C + SH.PROP_VS, C + SH.PROP_FS),
      water: P("water", C + SH.WATER_VS, C + SH.WATER_FS),
      shadowTerrain: P("shadow-terrain", C + SH.SHADOW_TERRAIN_VS, SH.SHADOW_FS),
      shadowProp: P("shadow-prop", C + SH.PROP_VS, SH.SHADOW_FS),
      ssao: P("ssao", FULLSCREEN_VS, C + SH.SSAO_FS),
      aoBlur: P("ao-blur", FULLSCREEN_VS, SH.AO_BLUR_FS),
      blur: P("blur", FULLSCREEN_VS, SH.BLUR_FS),
      composite: P("composite", FULLSCREEN_VS, SH.COMPOSITE_FS),
      tensor: P("tensor", FULLSCREEN_VS, SH.TENSOR_FS),
      kuwahara: P("kuwahara", FULLSCREEN_VS, SH.KUWAHARA_FS),
      finish: P("finish", FULLSCREEN_VS, C + SH.FINISH_FS),
      fxaa: P("fxaa", FULLSCREEN_VS, SH.FXAA_FS),
      overlay: P("overlay", C + SH.OVERLAY_VS, C + SH.OVERLAY_FS),
      marker: P("marker", C + SH.MARKER_VS, C + SH.MARKER_FS),
      ghost: P("ghost", C + SH.PROP_VS, C + SH.GHOST_FS),
      particle: P("particle", C + SH.PARTICLE_VS, SH.PARTICLE_FS),
      blit: P("blit", FULLSCREEN_VS, SH.BLIT_FS),
    };
    this.shadowMap = depthTexture(gl, SHADOW_SIZE, SHADOW_SIZE);
    this.shadowFbo = framebuffer(gl, [], this.shadowMap);
    this.shadowMapT = depthTexture(gl, TERRAIN_SHADOW_SIZE, TERRAIN_SHADOW_SIZE);
    this.shadowFboT = framebuffer(gl, [], this.shadowMapT);
    this.w = 0;
    this.h = 0;
    this.stats = { frames: 0, ms: {}, cpu: 0 };
    this.queries = [];
  }

  /** (Re)allocates the targets for an internal size of w x h pixels. */
  resize(w, h) {
    if (w === this.w && h === this.h) return;
    this.disposeTargets();
    const gl = this.gl;
    this.w = w;
    this.h = h;
    const F = this.hdr ? { internal: gl.RGBA16F, format: gl.RGBA, type: gl.HALF_FLOAT } : {};
    const hw = Math.max(1, w >> 1), hh = Math.max(1, h >> 1);
    this.t = {
      color: texture(gl, w, h, F),
      nd: texture(gl, w, h, { ...F, filter: gl.NEAREST }),
      emit: texture(gl, w, h, F),
      ao: texture(gl, hw, hh, F),
      ao2: texture(gl, hw, hh, F),
      glow: texture(gl, hw, hh, F),
      glow2: texture(gl, hw, hh, F),
      ldrA: texture(gl, w, h),
      ldrB: texture(gl, w, h),
      tensorA: texture(gl, w, h, F),
      tensorB: texture(gl, w, h, F),
      aa: texture(gl, w, h),
      final: texture(gl, w, h),
      // The scene before the water (grab): what the water is seen through and what it mirrors.
      sceneCopy: texture(gl, w, h, F),
      ndCopy: texture(gl, w, h, { ...F, filter: gl.NEAREST }),
    };
    this.depth = depthBuffer(gl, w, h);
    const t = this.t;
    this.f = {
      scene: framebuffer(gl, [t.color, t.nd, t.emit], this.depth),
      ao: framebuffer(gl, [t.ao]),
      ao2: framebuffer(gl, [t.ao2]),
      glow: framebuffer(gl, [t.glow]),
      glow2: framebuffer(gl, [t.glow2]),
      ldrA: framebuffer(gl, [t.ldrA]),
      ldrB: framebuffer(gl, [t.ldrB]),
      tensorA: framebuffer(gl, [t.tensorA]),
      tensorB: framebuffer(gl, [t.tensorB]),
      aa: framebuffer(gl, [t.aa]),
      final: framebuffer(gl, [t.final], this.depth),
      sceneCopy: framebuffer(gl, [t.sceneCopy]),
      ndCopy: framebuffer(gl, [t.ndCopy]),
    };
  }

  /** Copies the scene's colour and normal/depth targets (the water reads them: it may not sample
   *  the targets it draws into). */
  grab() {
    const gl = this.gl, W = this.w, H = this.h;
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, this.f.scene);
    for (const [i, dst] of [[0, this.f.sceneCopy], [1, this.f.ndCopy]]) {
      gl.readBuffer(gl.COLOR_ATTACHMENT0 + i);
      gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, dst);
      gl.blitFramebuffer(0, 0, W, H, 0, 0, W, H, gl.COLOR_BUFFER_BIT, gl.NEAREST);
    }
    gl.readBuffer(gl.COLOR_ATTACHMENT0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.f.scene);
  }

  disposeTargets() {
    const gl = this.gl;
    if (this.t) for (const x of Object.values(this.t)) gl.deleteTexture(x);
    if (this.f) for (const x of Object.values(this.f)) gl.deleteFramebuffer(x);
    if (this.depth) gl.deleteRenderbuffer(this.depth);
    this.t = this.f = this.depth = null;
  }

  dispose() {
    this.disposeTargets();
    const gl = this.gl;
    gl.deleteTexture(this.shadowMap);
    gl.deleteFramebuffer(this.shadowFbo);
    gl.deleteTexture(this.shadowMapT);
    gl.deleteFramebuffer(this.shadowFboT);
    for (const p of Object.values(this.p)) p.dispose();
  }

  /** A full-screen pass of program `prog` into framebuffer `fbo` (null = the canvas). */
  pass(prog, fbo, w, h, uniforms) {
    const gl = this.gl;
    gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
    gl.viewport(0, 0, w, h);
    prog.use().set(uniforms);
    this.fs();
  }

  /** GPU time of a stage while measuring: wraps `f`. With timer queries, the GPU's own count;
   *  without (software rendering), wall time with the pipeline drained before and after (a
   *  one-pixel read waits for all work in flight), so each stage is timed alone. */
  timed(name, f) {
    const gl = this.gl;
    if (!this.measure) return f();
    if (!this.timer) {
      this.drain();
      const t0 = performance.now();
      f();
      this.drain();
      const s = this.stats.ms[name] ?? { sum: 0, n: 0 };
      s.sum += performance.now() - t0;
      s.n++;
      this.stats.ms[name] = s;
      this.stats.method = "wall";
      return;
    }
    this.stats.method = "timer";
    const q = gl.createQuery();
    gl.beginQuery(this.timer.TIME_ELAPSED_EXT, q);
    f();
    gl.endQuery(this.timer.TIME_ELAPSED_EXT);
    this.queries.push([name, q]);
  }

  drain() {
    const gl = this.gl;
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
    gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, (this.px1 ??= new Uint8Array(4)));
  }

  collectTimings() {
    const gl = this.gl;
    if (!this.timer) return;
    const left = [];
    for (const [name, q] of this.queries) {
      if (gl.getQueryParameter(q, gl.QUERY_RESULT_AVAILABLE)) {
        const ns = gl.getQueryParameter(q, gl.QUERY_RESULT);
        const s = this.stats.ms[name] ?? { sum: 0, n: 0 };
        s.sum += ns / 1e6;
        s.n++;
        this.stats.ms[name] = s;
        gl.deleteQuery(q);
      } else left.push([name, q]);
    }
    this.queries = left;
  }

  /** Renders one frame. `fr`: the frame's camera and light (map3d.js `frameState`); `scene`: the
   *  draw callbacks (scene.js); `cfg`: config.js; `canvas`: [w, h] in device pixels. */
  render(fr, scene, cfg, canvasW, canvasH) {
    const gl = this.gl;
    const W = this.w, H = this.h;
    this.collectTimings();
    const U = fr.uniforms;
    gl.disable(gl.CULL_FACE);
    gl.disable(gl.BLEND);
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.depthMask(true);

    // --- shadow map
    if (cfg.shadows) {
      this.timed("shadow", () => {
        gl.enable(gl.POLYGON_OFFSET_FILL);
        gl.polygonOffset(1.5, 3);
        const SU = { ...U, uViewProj: fr.lightViewProj };
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.shadowFbo);
        gl.viewport(0, 0, SHADOW_SIZE, SHADOW_SIZE);
        gl.clear(gl.DEPTH_BUFFER_BIT);
        scene.drawProps(this.p.shadowProp, SU, true);
        gl.bindFramebuffer(gl.FRAMEBUFFER, this.shadowFboT);
        gl.viewport(0, 0, TERRAIN_SHADOW_SIZE, TERRAIN_SHADOW_SIZE);
        gl.clear(gl.DEPTH_BUFFER_BIT);
        scene.drawTerrain(this.p.shadowTerrain, SU, true);
        gl.disable(gl.POLYGON_OFFSET_FILL);
      });
    }

    // --- scene, then the water over it
    const SU = { ...U, uShadowMap: this.shadowMap, uShadowMapT: this.shadowMapT, uShadowOn: cfg.shadows ? 1 : 0 };
    this.timed("scene", () => {
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.f.scene);
      gl.viewport(0, 0, W, H);
      gl.clearBufferfv(gl.COLOR, 0, [...fr.background, 0]);
      gl.clearBufferfv(gl.COLOR, 1, [0, 0, 1, 0]);
      gl.clearBufferfv(gl.COLOR, 2, [0, 0, 0, 0]);
      gl.clear(gl.DEPTH_BUFFER_BIT);
      // Depth first (cheap vertex-only passes), then each pixel is shaded once: the ground under
      // a forest is never painted only to be covered. The water is not in it: the sea floor and
      // the river beds are painted (cheaply where too deep to see), grabbed, and the water is
      // drawn last, seen through to them and mirroring what stands around it.
      gl.colorMask(false, false, false, false);
      scene.drawTerrain(this.p.shadowTerrain, U, false);
      scene.drawProps(this.p.shadowProp, U, false);
      gl.colorMask(true, true, true, true);
      gl.depthMask(false);
      gl.depthFunc(gl.LEQUAL);
      scene.drawTerrain(this.p.terrain, SU, false);
      scene.drawProps(this.p.prop, SU, false);
    });
    this.timed("water", () => {
      this.grab();
      // The water writes its depth (the overlays and markers sit on its surface).
      gl.viewport(0, 0, W, H);
      gl.depthMask(true);
      // Seeing into the water and the mirror read the grabbed view depth, which only a float target
      // holds (an 8-bit one clamps it to 1): without one the water is drawn opaque, unmirrored.
      const hdr = this.hdr ? 1 : 0;
      scene.drawWater(this.p.water, { ...SU, uSceneColor: this.t.sceneCopy, uSceneND: this.t.ndCopy, uReflect: (cfg.reflections ?? 1) * hdr, uRefract: (cfg.refraction ?? 1) * hdr });
    });
    gl.disable(gl.DEPTH_TEST);
    gl.depthMask(false);
    const hw = Math.max(1, W >> 1), hh = Math.max(1, H >> 1);

    // --- ambient occlusion
    let ao = null;
    if (cfg.ssao > 0) {
      this.timed("ssao", () => {
        this.pass(this.p.ssao, this.f.ao, hw, hh, { ...U, uND: this.t.nd, uTan: fr.tan, uRadius: cfg.aoRadius, uTexel: [1 / hw, 1 / hh] });
        this.pass(this.p.aoBlur, this.f.ao2, hw, hh, { uSrc: this.t.ao, uDir: [1 / hw, 0] });
        this.pass(this.p.aoBlur, this.f.ao, hw, hh, { uSrc: this.t.ao2, uDir: [0, 1 / hh] });
      });
      ao = this.t.ao;
    }

    // --- glow
    let glow = null;
    if (cfg.glow > 0) {
      this.timed("glow", () => {
        this.pass(this.p.blur, this.f.glow, hw, hh, { uSrc: this.t.emit, uDir: [2 / W, 0], uSigma: 4 });
        this.pass(this.p.blur, this.f.glow2, hw, hh, { uSrc: this.t.glow, uDir: [0, 2 / H], uSigma: 4 });
      });
      glow = this.t.glow2;
    }

    // --- compose
    this.timed("compose", () => {
      this.pass(this.p.composite, this.f.ldrA, W, H, {
        uColor: this.t.color,
        uAO: ao ?? this.t.color,
        uGlow: glow ?? this.t.emit,
        uAOAmount: ao ? cfg.ssao : 0,
        uGlowAmount: glow ? cfg.glow : 0,
        uExposure: fr.exposure,
      });
    });

    // --- paint
    let painted = this.t.ldrA;
    if (cfg.kuwahara > 0) {
      this.timed("kuwahara", () => {
        this.pass(this.p.tensor, this.f.tensorA, W, H, { uSrc: this.t.ldrA, uTexel: [1 / W, 1 / H] });
        this.pass(this.p.blur, this.f.tensorB, W, H, { uSrc: this.t.tensorA, uDir: [1 / W, 0], uSigma: 2 });
        this.pass(this.p.blur, this.f.tensorA, W, H, { uSrc: this.t.tensorB, uDir: [0, 1 / H], uSigma: 2 });
        this.pass(this.p.kuwahara, this.f.ldrB, W, H, {
          uSrc: this.t.ldrA,
          uTensor: this.t.tensorA,
          uRadius: Math.min(8, cfg.kuwaharaRadius * (fr.scale ?? 1) * (fr.paintZoom ?? 1)),
          uSharp: cfg.sharp,
          uAniso: cfg.aniso,
          uStrength: cfg.kuwahara * (fr.paintStrength ?? 1),
        });
      });
      painted = this.t.ldrB;
    }

    // --- finish (into the AA buffer when FXAA is on, else straight to final)
    const useFxaa = (cfg.fxaa ?? 1) > 0;
    this.timed("finish", () => {
      this.pass(this.p.finish, useFxaa ? this.f.aa : this.f.final, W, H, {
        ...U,
        uSrc: painted,
        uND: this.t.nd,
        uEmitId: this.t.emit,
        uTexel: [1 / W, 1 / H],
        uOutline: cfg.outline,
        uOutlineWidth: cfg.outlineWidth * (fr.scale ?? 1),
        uInk: [0.13, 0.09, 0.08],
        uGrade: cfg.grade,
        uSaturation: cfg.saturation,
        uContrast: cfg.contrast,
        uShadowGrade: [0.82, 0.84, 1.08],
        uLightGrade: [1.06, 1.0, 0.9],
        uGrain: cfg.grain,
        uVignette: cfg.vignette,
        uInkLost: cfg.inkLost ?? 1,
        uInkColour: cfg.inkColour ?? 0.5,
        uMapSize: [scene.world.w, scene.world.h],
        uWrapX: scene.world.wrapX ? 1 : 0,
        uTan: fr.tan,
        uInvView: fr.invView,
      });
    });

    // --- anti-aliasing: FXAA on the finished colour into final, before the overlays (F17). The
    // HUD is a separate 2D canvas, so it is unaffected.
    if (useFxaa) this.timed("fxaa", () => this.pass(this.p.fxaa, this.f.final, W, H, { uSrc: this.t.aa, uTexel: [1 / W, 1 / H] }));

    // --- overlays, depth-tested against the scene's depth (shared by the final target)
    this.timed("overlay", () => {
      gl.bindFramebuffer(gl.FRAMEBUFFER, this.f.final);
      gl.viewport(0, 0, W, H);
      gl.enable(gl.DEPTH_TEST);
      gl.depthMask(false);
      gl.enable(gl.BLEND);
      gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.enable(gl.POLYGON_OFFSET_FILL);
      gl.polygonOffset(-1, -2);
      // The ground marks where the ground is the front surface, then ghosted where something
      // stands over it (a border under a forest still shows, faintly).
      for (const ghost of [0, 1]) {
        gl.depthFunc(ghost ? gl.GREATER : gl.LEQUAL);
        scene.drawOverlay(this.p.overlay, { ...U, uGhost: ghost });
        scene.drawMarkers(this.p.marker, { ...U, uGhost: ghost });
      }
      gl.disable(gl.POLYGON_OFFSET_FILL);
      // Units behind scenery: their silhouettes, where something stands in front of them.
      gl.depthFunc(gl.GREATER);
      scene.drawUnitGhosts(this.p.ghost, { ...U, uND: this.t.nd, uEmitId: this.t.emit });
      gl.depthFunc(gl.LEQUAL);
      if (cfg.particles) {
        gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
        scene.drawParticles(this.p.particle, U);
      }
      gl.disable(gl.BLEND);
      gl.disable(gl.DEPTH_TEST);
      gl.depthMask(true);
    });

    // --- to the canvas
    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, this.f.final);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);
    gl.blitFramebuffer(0, 0, W, H, 0, 0, canvasW, canvasH, gl.COLOR_BUFFER_BIT, W === canvasW && H === canvasH ? gl.NEAREST : gl.LINEAR);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    this.stats.frames++;
  }
}
