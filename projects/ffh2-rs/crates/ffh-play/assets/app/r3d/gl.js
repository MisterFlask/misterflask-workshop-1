// WebGL2 plumbing for the 3D renderer (docs/R3D.md): programs with typed uniform setters, textures,
// framebuffers, vertex-array meshes (optionally instanced) and the full-screen triangle the post
// passes draw with. No library: the client has no build step and no dependencies.

/** Fixed attribute locations shared by every program, so one vertex layout fits them all. */
export const ATTR = {
  aPos: 0,
  aNormal: 1,
  aColor: 2,
  aMat: 3,
  iPosRot: 4,
  iScale: 5,
  iTint: 6,
  iTeam: 7,
  aExtra: 8,
  aFlow: 9,
};

const SAMPLERS = new Set();

export class Program {
  /** `defines`: `{NAME: value}` prepended as `#define NAME value`. */
  constructor(gl, name, vs, fs, defines = {}) {
    this.gl = gl;
    this.name = name;
    const head = "#version 300 es\nprecision highp float;\nprecision highp int;\nprecision highp sampler2D;\n" + Object.entries(defines).map(([k, v]) => `#define ${k} ${v}\n`).join("");
    const v = compileShader(gl, gl.VERTEX_SHADER, head + "#define VERTEX 1\n" + vs, name);
    const f = compileShader(gl, gl.FRAGMENT_SHADER, head + "#define FRAGMENT 1\n" + fs, name);
    const p = gl.createProgram();
    gl.attachShader(p, v);
    gl.attachShader(p, f);
    for (const [k, loc] of Object.entries(ATTR)) gl.bindAttribLocation(p, loc, k);
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(`r3d program ${name}: ${gl.getProgramInfoLog(p)}`);
    gl.deleteShader(v);
    gl.deleteShader(f);
    this.p = p;
    this.u = {};
    this.units = {};
    let unit = 0;
    const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
    for (let i = 0; i < n; i++) {
      const info = gl.getActiveUniform(p, i);
      const key = info.name.replace(/\[0\]$/, "");
      const loc = gl.getUniformLocation(p, info.name);
      const isSampler = info.type === gl.SAMPLER_2D || info.type === gl.SAMPLER_3D || info.type === gl.SAMPLER_2D_ARRAY || info.type === gl.UNSIGNED_INT_SAMPLER_2D || info.type === gl.INT_SAMPLER_2D;
      if (isSampler) this.units[key] = unit++;
      this.u[key] = { loc, type: info.type, size: info.size };
    }
    SAMPLERS.add(this);
  }

  use() {
    this.gl.useProgram(this.p);
    return this;
  }

  /** Sets uniforms by name (unknown names are ignored: a define may have compiled them out).
   *  Textures are WebGLTexture values; numbers, arrays and typed arrays otherwise. */
  set(values) {
    const gl = this.gl;
    for (const [k, v] of Object.entries(values)) {
      const u = this.u[k];
      if (!u || v === undefined || v === null) continue;
      switch (u.type) {
        case gl.FLOAT:
          if (u.size > 1) gl.uniform1fv(u.loc, v);
          else gl.uniform1f(u.loc, v);
          break;
        case gl.FLOAT_VEC2:
          gl.uniform2fv(u.loc, v);
          break;
        case gl.FLOAT_VEC3:
          gl.uniform3fv(u.loc, v);
          break;
        case gl.FLOAT_VEC4:
          gl.uniform4fv(u.loc, v);
          break;
        case gl.INT:
        case gl.BOOL:
          if (u.size > 1) gl.uniform1iv(u.loc, v);
          else gl.uniform1i(u.loc, v | 0);
          break;
        case gl.INT_VEC2:
          gl.uniform2iv(u.loc, v);
          break;
        case gl.FLOAT_MAT4:
          gl.uniformMatrix4fv(u.loc, false, v);
          break;
        case gl.FLOAT_MAT3:
          gl.uniformMatrix3fv(u.loc, false, v);
          break;
        default: {
          const unit = this.units[k];
          if (unit === undefined) break;
          gl.activeTexture(gl.TEXTURE0 + unit);
          gl.bindTexture(u.type === gl.SAMPLER_3D ? gl.TEXTURE_3D : u.type === gl.SAMPLER_2D_ARRAY ? gl.TEXTURE_2D_ARRAY : gl.TEXTURE_2D, v);
          gl.uniform1i(u.loc, unit);
        }
      }
    }
    return this;
  }

  dispose() {
    this.gl.deleteProgram(this.p);
    SAMPLERS.delete(this);
  }
}

function compileShader(gl, type, src, name) {
  const s = gl.createShader(type);
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(s);
    const lines = src.split("\n");
    const m = /ERROR: \d+:(\d+)/.exec(log);
    const at = m ? Number(m[1]) : 0;
    const ctx = lines.slice(Math.max(0, at - 4), at + 2).map((l, i) => `${Math.max(1, at - 3) + i}: ${l}`).join("\n");
    throw new Error(`r3d shader ${name} (${type === gl.VERTEX_SHADER ? "vs" : "fs"}): ${log}\n${ctx}`);
  }
  return s;
}

/** A 2D texture. `opts`: `internal` (gl.RGBA8 ...), `format`, `type`, `filter` (gl.LINEAR),
 *  `wrap` (gl.CLAMP_TO_EDGE), `data` (typed array or null), `mips`. */
export function texture(gl, w, h, opts = {}) {
  const t = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, t);
  const internal = opts.internal ?? gl.RGBA8;
  const format = opts.format ?? gl.RGBA;
  const type = opts.type ?? gl.UNSIGNED_BYTE;
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  gl.texImage2D(gl.TEXTURE_2D, 0, internal, w, h, 0, format, type, opts.data ?? null);
  const filter = opts.filter ?? gl.LINEAR;
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, opts.mips ? gl.LINEAR_MIPMAP_LINEAR : filter);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, opts.wrapS ?? opts.wrap ?? gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, opts.wrapT ?? opts.wrap ?? gl.CLAMP_TO_EDGE);
  if (opts.mips) gl.generateMipmap(gl.TEXTURE_2D);
  t.w = w;
  t.h = h;
  t.opts = { internal, format, type };
  return t;
}

/** Replaces a texture's whole contents. */
export function upload(gl, t, data) {
  gl.bindTexture(gl.TEXTURE_2D, t);
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, t.w, t.h, t.opts.format, t.opts.type, data);
}

/** A framebuffer over `colors` (textures) and an optional `depth` (renderbuffer or texture). */
export function framebuffer(gl, colors, depth = null) {
  const f = gl.createFramebuffer();
  gl.bindFramebuffer(gl.FRAMEBUFFER, f);
  colors.forEach((t, i) => gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0 + i, gl.TEXTURE_2D, t, 0));
  if (depth) {
    if (depth instanceof WebGLRenderbuffer) gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, depth);
    else gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.TEXTURE_2D, depth, 0);
  }
  gl.drawBuffers(colors.length ? colors.map((_, i) => gl.COLOR_ATTACHMENT0 + i) : [gl.NONE]);
  const st = gl.checkFramebufferStatus(gl.FRAMEBUFFER);
  if (st !== gl.FRAMEBUFFER_COMPLETE) throw new Error(`r3d framebuffer incomplete: 0x${st.toString(16)}`);
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  f.w = colors[0]?.w ?? depth?.w;
  f.h = colors[0]?.h ?? depth?.h;
  return f;
}

export function depthBuffer(gl, w, h) {
  const r = gl.createRenderbuffer();
  gl.bindRenderbuffer(gl.RENDERBUFFER, r);
  gl.renderbufferStorage(gl.RENDERBUFFER, gl.DEPTH_COMPONENT24, w, h);
  r.w = w;
  r.h = h;
  return r;
}

/** A depth texture (the shadow map). */
export function depthTexture(gl, w, h) {
  const t = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, t);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.DEPTH_COMPONENT24, w, h, 0, gl.DEPTH_COMPONENT, gl.UNSIGNED_INT, null);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  t.w = w;
  t.h = h;
  return t;
}

/** Vertex layouts: attribute name -> components. Float32 throughout. */
export const PROP_LAYOUT = { aPos: 3, aNormal: 3, aColor: 3, aMat: 4 };
export const INSTANCE_LAYOUT = { iPosRot: 4, iScale: 4, iTint: 4, iTeam: 4 };
export const INSTANCE_FLOATS = 16;

/** A mesh on the GPU: one interleaved vertex buffer (`layout`), optional indices, optional
 *  per-instance buffer (INSTANCE_LAYOUT) set with `setInstances`. */
export class Mesh {
  constructor(gl, data, layout, { indices = null, instanced = false, mode = null, usage = null } = {}) {
    this.gl = gl;
    this.layout = layout;
    this.mode = mode ?? gl.TRIANGLES;
    this.vao = gl.createVertexArray();
    gl.bindVertexArray(this.vao);
    this.vbo = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.vbo);
    gl.bufferData(gl.ARRAY_BUFFER, data, usage ?? gl.STATIC_DRAW);
    const stride = Object.values(layout).reduce((a, b) => a + b, 0);
    this.stride = stride;
    let off = 0;
    for (const [name, n] of Object.entries(layout)) {
      const loc = ATTR[name];
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, n, gl.FLOAT, false, stride * 4, off * 4);
      off += n;
    }
    this.count = data.length / stride;
    if (indices) {
      this.ibo = gl.createBuffer();
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.ibo);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.STATIC_DRAW);
      this.indexType = indices instanceof Uint32Array ? gl.UNSIGNED_INT : gl.UNSIGNED_SHORT;
      this.count = indices.length;
    }
    this.instances = 0;
    if (instanced) {
      this.ibuf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, this.ibuf);
      let o = 0;
      for (const [name, n] of Object.entries(INSTANCE_LAYOUT)) {
        const loc = ATTR[name];
        gl.enableVertexAttribArray(loc);
        gl.vertexAttribPointer(loc, n, gl.FLOAT, false, INSTANCE_FLOATS * 4, o * 4);
        gl.vertexAttribDivisor(loc, 1);
        o += n;
      }
    } else {
      // Programs that read instance attributes still work on a plain mesh: identity values.
      gl.vertexAttrib4f(ATTR.iPosRot, 0, 0, 0, 0);
      gl.vertexAttrib4f(ATTR.iScale, 1, 1, 1, 0);
      gl.vertexAttrib4f(ATTR.iTint, 1, 1, 1, 1);
      gl.vertexAttrib4f(ATTR.iTeam, 1, 1, 1, 1);
    }
    gl.bindVertexArray(null);
  }

  /** Replaces the instances: a Float32Array of INSTANCE_FLOATS per instance. */
  setInstances(arr, n = arr.length / INSTANCE_FLOATS) {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.ibuf);
    gl.bufferData(gl.ARRAY_BUFFER, arr, gl.DYNAMIC_DRAW);
    this.instances = n;
  }

  draw() {
    const gl = this.gl;
    if (this.ibuf && this.instances === 0) return;
    gl.bindVertexArray(this.vao);
    if (!this.ibuf) {
      gl.vertexAttrib4f(ATTR.iPosRot, 0, 0, 0, 0);
      gl.vertexAttrib4f(ATTR.iScale, 1, 1, 1, 0);
      gl.vertexAttrib4f(ATTR.iTint, 1, 1, 1, 1);
      gl.vertexAttrib4f(ATTR.iTeam, 1, 1, 1, 1);
    }
    if (this.ibo) {
      if (this.ibuf) gl.drawElementsInstanced(this.mode, this.count, this.indexType, 0, this.instances);
      else gl.drawElements(this.mode, this.count, this.indexType, 0);
    } else if (this.ibuf) gl.drawArraysInstanced(this.mode, 0, this.count, this.instances);
    else gl.drawArrays(this.mode, 0, this.count);
    gl.bindVertexArray(null);
  }

  dispose() {
    const gl = this.gl;
    gl.deleteBuffer(this.vbo);
    if (this.ibo) gl.deleteBuffer(this.ibo);
    if (this.ibuf) gl.deleteBuffer(this.ibuf);
    gl.deleteVertexArray(this.vao);
  }
}

/** The full-screen triangle (no attributes: the vertex shader makes it from gl_VertexID). */
export function fullscreen(gl) {
  const vao = gl.createVertexArray();
  return () => {
    gl.bindVertexArray(vao);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.bindVertexArray(null);
  };
}

export const FULLSCREEN_VS = `
out vec2 vUv;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  vUv = p;
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;
