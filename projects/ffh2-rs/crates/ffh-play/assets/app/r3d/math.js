// Small vector and matrix helpers for the 3D renderer (docs/R3D.md). Matrices are column-major
// Float32Array(16), as WebGL takes them; vectors are plain [x, y, z] arrays.

export function vadd(a, b) {
  return [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
}

export function vsub(a, b) {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}

export function vscale(a, s) {
  return [a[0] * s, a[1] * s, a[2] * s];
}

export function vdot(a, b) {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

export function vcross(a, b) {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}

export function vlen(a) {
  return Math.hypot(a[0], a[1], a[2]);
}

export function vnorm(a) {
  const l = vlen(a) || 1;
  return [a[0] / l, a[1] / l, a[2] / l];
}

export function vlerp(a, b, t) {
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
}

export function clamp(x, lo, hi) {
  return x < lo ? lo : x > hi ? hi : x;
}

export function smoothstep(e0, e1, x) {
  const t = clamp((x - e0) / (e1 - e0), 0, 1);
  return t * t * (3 - 2 * t);
}

export function lerp(a, b, t) {
  return a + (b - a) * t;
}

export function mat4() {
  const m = new Float32Array(16);
  m[0] = m[5] = m[10] = m[15] = 1;
  return m;
}

export function mul(a, b) {
  const o = new Float32Array(16);
  for (let c = 0; c < 4; c++) {
    for (let r = 0; r < 4; r++) {
      o[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
    }
  }
  return o;
}

export function perspective(fovy, aspect, near, far) {
  const f = 1 / Math.tan(fovy / 2);
  const m = new Float32Array(16);
  m[0] = f / aspect;
  m[5] = f;
  m[10] = (far + near) / (near - far);
  m[11] = -1;
  m[14] = (2 * far * near) / (near - far);
  return m;
}

export function ortho(l, r, b, t, n, f) {
  const m = mat4();
  m[0] = 2 / (r - l);
  m[5] = 2 / (t - b);
  m[10] = -2 / (f - n);
  m[12] = -(r + l) / (r - l);
  m[13] = -(t + b) / (t - b);
  m[14] = -(f + n) / (f - n);
  return m;
}

export function lookAt(eye, target, up) {
  const z = vnorm(vsub(eye, target));
  const x = vnorm(vcross(up, z));
  const y = vcross(z, x);
  const m = mat4();
  m[0] = x[0];
  m[4] = x[1];
  m[8] = x[2];
  m[1] = y[0];
  m[5] = y[1];
  m[9] = y[2];
  m[2] = z[0];
  m[6] = z[1];
  m[10] = z[2];
  m[12] = -vdot(x, eye);
  m[13] = -vdot(y, eye);
  m[14] = -vdot(z, eye);
  return m;
}

export function invert(m) {
  const a = m;
  const o = new Float32Array(16);
  const b00 = a[0] * a[5] - a[1] * a[4];
  const b01 = a[0] * a[6] - a[2] * a[4];
  const b02 = a[0] * a[7] - a[3] * a[4];
  const b03 = a[1] * a[6] - a[2] * a[5];
  const b04 = a[1] * a[7] - a[3] * a[5];
  const b05 = a[2] * a[7] - a[3] * a[6];
  const b06 = a[8] * a[13] - a[9] * a[12];
  const b07 = a[8] * a[14] - a[10] * a[12];
  const b08 = a[8] * a[15] - a[11] * a[12];
  const b09 = a[9] * a[14] - a[10] * a[13];
  const b10 = a[9] * a[15] - a[11] * a[13];
  const b11 = a[10] * a[15] - a[11] * a[14];
  let det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;
  if (!det) return mat4();
  det = 1 / det;
  o[0] = (a[5] * b11 - a[6] * b10 + a[7] * b09) * det;
  o[1] = (a[2] * b10 - a[1] * b11 - a[3] * b09) * det;
  o[2] = (a[13] * b05 - a[14] * b04 + a[15] * b03) * det;
  o[3] = (a[10] * b04 - a[9] * b05 - a[11] * b03) * det;
  o[4] = (a[6] * b08 - a[4] * b11 - a[7] * b07) * det;
  o[5] = (a[0] * b11 - a[2] * b08 + a[3] * b07) * det;
  o[6] = (a[14] * b02 - a[12] * b05 - a[15] * b01) * det;
  o[7] = (a[8] * b05 - a[10] * b02 + a[11] * b01) * det;
  o[8] = (a[4] * b10 - a[5] * b08 + a[7] * b06) * det;
  o[9] = (a[1] * b08 - a[0] * b10 - a[3] * b06) * det;
  o[10] = (a[12] * b04 - a[13] * b02 + a[15] * b00) * det;
  o[11] = (a[9] * b02 - a[8] * b04 - a[11] * b00) * det;
  o[12] = (a[5] * b07 - a[4] * b09 - a[6] * b06) * det;
  o[13] = (a[0] * b09 - a[1] * b07 + a[2] * b06) * det;
  o[14] = (a[13] * b01 - a[12] * b03 - a[14] * b00) * det;
  o[15] = (a[8] * b03 - a[9] * b01 + a[10] * b00) * det;
  return o;
}

/** m · (x, y, z, w): the 4-vector. */
export function xform4(m, x, y, z, w = 1) {
  return [
    m[0] * x + m[4] * y + m[8] * z + m[12] * w,
    m[1] * x + m[5] * y + m[9] * z + m[13] * w,
    m[2] * x + m[6] * y + m[10] * z + m[14] * w,
    m[3] * x + m[7] * y + m[11] * z + m[15] * w,
  ];
}

/** A point through m with the perspective divide. */
export function xformPoint(m, p) {
  const r = xform4(m, p[0], p[1], p[2], 1);
  return [r[0] / r[3], r[1] / r[3], r[2] / r[3]];
}

/** "#rrggbb" (sRGB) -> linear [r, g, b] in 0..1. */
export function hexLinear(hex) {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!m) return [0.5, 0.5, 0.5];
  return [1, 2, 3].map((i) => srgbToLinear(parseInt(m[i], 16) / 255));
}

export function srgbToLinear(c) {
  return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

/** Linear colour a mixed toward b by t. */
export function cmix(a, b, t) {
  return vlerp(a, b, t);
}
