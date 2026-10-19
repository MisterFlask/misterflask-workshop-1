// The mesh builder of the 3D renderer's asset recipes (docs/R3D.md "Asset recipes"): a transform
// stack and a set of modelling primitives (boxes, lathes, tubes along paths, deformed blobs, roofs)
// that append triangles to one vertex list in the PROP_LAYOUT format:
//
//   aPos (3)    position in the asset's space: y up, base on y = 0, one unit = one map plot
//   aNormal (3)
//   aColor (3)  linear albedo
//   aMat (4)    x material (MAT), y sway (0 rigid .. 1 tips of leaves and cloth), z bone (units'
//               animation groups, BONE), w baked occlusion (1 open .. 0 buried)
//
// A recipe sets `mb.color`, `mb.mat`, `mb.sway`, `mb.bone`, `mb.ao` and calls primitives; the
// current values stick to every vertex added. Normals are smooth where the primitive is curved;
// `sphereNormals` bends a part's normals out from a centre (soft, cloud-like canopies).

import { mat4, mul, hexLinear, vnorm, vsub, vcross, vlen, vadd, vscale } from "./math.js";

/** Material kinds, read by the shaders (shaders.js `shadeProp`). */
export const MAT = {
  MATTE: 0,
  FOLIAGE: 1,
  METAL: 2,
  TEAM: 3, // cloth/paint tinted by the owner's colour
  EMISSIVE: 4,
  SKIN: 5,
  GLOW_TEAM: 6, // emissive in the owner's colour (banners' lights, runes)
  WATER: 7,
  BLADE: 8, // grass and reed blades: lit as leaves, drawn as lines (not massed like a crown)
  LEAF: 9, // a crown's leaf clusters: foliage whose rims the depth prepass cuts into leaves
};

/** Animation groups of a unit's body (units.js poses them in the vertex shader). */
export const BONE = {
  BODY: 0,
  HEAD: 1,
  ARM_L: 2,
  ARM_R: 3,
  LEG_L: 4,
  LEG_R: 5,
  CAPE: 6,
  WEAPON: 7,
  WING_L: 8,
  WING_R: 9,
  TAIL: 10,
  MOUNT: 11,
};

export const FLOATS = 13;

/** A colour value: "#rrggbb" or a linear [r, g, b]. */
export function col(c) {
  return typeof c === "string" ? hexLinear(c) : c;
}

export class MeshBuilder {
  constructor() {
    this.v = [];
    this.m = mat4();
    this.stack = [];
    this._color = [0.5, 0.5, 0.5];
    this.mat = MAT.MATTE;
    this.sway = 0;
    this.bone = 0;
    this.ao = 1;
  }

  set color(c) {
    this._color = col(c);
  }

  get color() {
    return this._color;
  }

  /** The number of vertices so far (a mark for the per-part edits below). */
  mark() {
    return this.v.length / FLOATS;
  }

  // --- transforms -------------------------------------------------------------------------------

  push() {
    this.stack.push(this.m);
    return this;
  }

  pop() {
    this.m = this.stack.pop() ?? mat4();
    return this;
  }

  apply(t) {
    this.m = mul(this.m, t);
    return this;
  }

  translate(x, y, z) {
    const t = mat4();
    t[12] = x;
    t[13] = y;
    t[14] = z;
    return this.apply(t);
  }

  scale(x, y = x, z = x) {
    const t = mat4();
    t[0] = x;
    t[5] = y;
    t[10] = z;
    return this.apply(t);
  }

  rotateY(a) {
    const t = mat4();
    const c = Math.cos(a);
    const s = Math.sin(a);
    t[0] = c;
    t[2] = -s;
    t[8] = s;
    t[10] = c;
    return this.apply(t);
  }

  rotateX(a) {
    const t = mat4();
    const c = Math.cos(a);
    const s = Math.sin(a);
    t[5] = c;
    t[6] = s;
    t[9] = -s;
    t[10] = c;
    return this.apply(t);
  }

  rotateZ(a) {
    const t = mat4();
    const c = Math.cos(a);
    const s = Math.sin(a);
    t[0] = c;
    t[1] = s;
    t[4] = -s;
    t[5] = c;
    return this.apply(t);
  }

  /** Runs `f` with the transform and the material state saved and restored around it. */
  part(f) {
    const saved = [this.m, this._color, this.mat, this.sway, this.bone, this.ao];
    this.stack.push(this.m);
    try {
      f(this);
    } finally {
      this.stack.pop();
      [this.m, this._color, this.mat, this.sway, this.bone, this.ao] = saved;
    }
    return this;
  }

  // --- raw vertices -----------------------------------------------------------------------------

  xp(p) {
    const m = this.m;
    return [m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12], m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13], m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14]];
  }

  /** A normal through the current transform (inverse transpose of its 3x3). */
  xn(n) {
    const m = this.m;
    const a = m[0], b = m[4], c = m[8], d = m[1], e = m[5], f = m[9], g = m[2], h = m[6], i = m[10];
    // Cofactor matrix = det * inverse transpose; the scale drops out on normalising.
    const x = (e * i - f * h) * n[0] + (f * g - d * i) * n[1] + (d * h - e * g) * n[2];
    const y = (c * h - b * i) * n[0] + (a * i - c * g) * n[1] + (b * g - a * h) * n[2];
    const z = (b * f - c * e) * n[0] + (c * d - a * f) * n[1] + (a * e - b * d) * n[2];
    const det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    const s = det < 0 ? -1 : 1;
    const l = Math.hypot(x, y, z) || 1;
    return [(s * x) / l, (s * y) / l, (s * z) / l];
  }

  /** One vertex in local space (transformed here). `c` overrides the colour, `ao` the occlusion. */
  vert(p, n, c = null, ao = null) {
    const w = this.xp(p);
    const wn = this.xn(n);
    const k = c ?? this._color;
    this.v.push(w[0], w[1], w[2], wn[0], wn[1], wn[2], k[0], k[1], k[2], this.mat, this.sway, this.bone, ao ?? this.ao);
  }

  /** A triangle with a flat normal (counter-clockwise seen from outside). */
  tri(a, b, c) {
    const n = vnorm(vcross(vsub(b, a), vsub(c, a)));
    this.vert(a, n);
    this.vert(b, n);
    this.vert(c, n);
    return this;
  }

  /** A triangle with given vertex normals. */
  triN(a, b, c, na, nb, nc) {
    this.vert(a, na);
    this.vert(b, nb);
    this.vert(c, nc);
    return this;
  }

  quad(a, b, c, d) {
    this.tri(a, b, c);
    this.tri(a, c, d);
    return this;
  }

  quadN(a, b, c, d, na, nb, nc, nd) {
    this.triN(a, b, c, na, nb, nc);
    this.triN(a, c, d, na, nc, nd);
    return this;
  }

  // --- primitives -------------------------------------------------------------------------------

  /** A box `w` x `h` x `d`, base centred on the origin; `top` scales the top face (a taper), `skew`
   *  shifts it [x, z]. */
  box(w, h, d, { top = 1, skew = [0, 0] } = {}) {
    const x = w / 2, z = d / 2, tx = x * top, tz = z * top;
    const b = [[-x, 0, -z], [x, 0, -z], [x, 0, z], [-x, 0, z]];
    const t = [[-tx + skew[0], h, -tz + skew[1]], [tx + skew[0], h, -tz + skew[1]], [tx + skew[0], h, tz + skew[1]], [-tx + skew[0], h, tz + skew[1]]];
    this.quad(t[0], t[3], t[2], t[1]);
    this.quad(b[0], b[1], b[2], b[3]);
    for (let i = 0; i < 4; i++) {
      const j = (i + 1) % 4;
      this.quad(b[i], t[i], t[j], b[j]);
    }
    return this;
  }

  /** A surface of revolution about y: `profile` [[r, y], ...] bottom to top, `seg` sides. Smooth
   *  around, creased along the profile only where `creases` says (else smooth). Closed with a flat
   *  cap where the profile starts or ends off the axis (`caps`). */
  lathe(profile, seg = 10, { caps = true, phase = 0, wobble = null } = {}) {
    const ring = (i) => {
      const [r, y] = profile[i];
      const out = [];
      for (let s = 0; s <= seg; s++) {
        const a = phase + (s / seg) * Math.PI * 2;
        const rr = wobble ? r * (1 + wobble(a, y, i)) : r;
        out.push([Math.cos(a) * rr, y, Math.sin(a) * rr]);
      }
      return out;
    };
    const rings = profile.map((_, i) => ring(i));
    // Normals: from the profile's slope, per ring (averaged between neighbouring segments).
    const slope = (i) => {
      const a = profile[Math.max(0, i - 1)];
      const b = profile[Math.min(profile.length - 1, i + 1)];
      const dr = b[0] - a[0];
      const dy = b[1] - a[1];
      return [dy, -dr]; // outward (radial, up) normal of the profile tangent (dr, dy)
    };
    const prof = (i, s) => {
      const sl = slope(i), a = phase + (s / seg) * Math.PI * 2;
      return vnorm([Math.cos(a) * sl[0], sl[1], Math.sin(a) * sl[0]]);
    };
    // With a wobble the surface is lobed: its normals come from the wobbled grid itself (central
    // differences around and along it, turned outward, a little of the profile's mixed in), so
    // the lobes shade as lobes.
    const grid = (i, s) => {
      const nr = profile.length;
      const around = vsub(rings[i][(s + 1) % seg], rings[i][(s + seg - 1) % seg]);
      const along = vsub(rings[Math.min(nr - 1, i + 1)][s % seg], rings[Math.max(0, i - 1)][s % seg]);
      const p = prof(i, s);
      let n = vcross(along, around);
      if (vlen(n) < 1e-9) return p;
      n = vnorm(n);
      if (n[0] * p[0] + n[1] * p[1] + n[2] * p[2] < 0) n = vscale(n, -1);
      return vnorm(vadd(vscale(n, 0.75), vscale(p, 0.25)));
    };
    const nrm = wobble ? grid : prof;
    for (let i = 0; i + 1 < profile.length; i++) {
      for (let s = 0; s < seg; s++) {
        const p00 = rings[i][s], p01 = rings[i][s + 1], p10 = rings[i + 1][s], p11 = rings[i + 1][s + 1];
        this.quadN(p00, p10, p11, p01, nrm(i, s), nrm(i + 1, s), nrm(i + 1, s + 1), nrm(i, s + 1));
      }
    }
    if (caps) {
      const capAt = (i, up) => {
        const [r, y] = profile[i];
        if (r < 1e-4) return;
        const c = [0, y, 0];
        const nn = [0, up ? 1 : -1, 0];
        for (let s = 0; s < seg; s++) {
          const a = rings[i][s], b = rings[i][s + 1];
          if (up) this.triN(c, b, a, nn, nn, nn);
          else this.triN(c, a, b, nn, nn, nn);
        }
      };
      capAt(0, false);
      capAt(profile.length - 1, true);
    }
    return this;
  }

  cylinder(r0, r1, h, seg = 8, opts = {}) {
    return this.lathe([[r0, 0], [r1, h]], seg, opts);
  }

  cone(r, h, seg = 8, opts = {}) {
    return this.lathe([[r, 0], [0, h]], seg, opts);
  }

  /** A tube along `points` ([x, y, z] ...) with `radii` (one per point, or one number), `seg`
   *  sides; smooth, open-ended unless `caps`. Frames by parallel transport, so it does not twist. */
  tube(points, radii, seg = 6, { caps = false, colors = null } = {}) {
    const R = (i) => (Array.isArray(radii) ? radii[i] : radii);
    const n = points.length;
    if (n < 2) return this;
    const tangents = points.map((_, i) => vnorm(vsub(points[Math.min(n - 1, i + 1)], points[Math.max(0, i - 1)])));
    let u = Math.abs(tangents[0][1]) < 0.9 ? [0, 1, 0] : [1, 0, 0];
    let nrm = vnorm(vcross(vcross(tangents[0], u), tangents[0]));
    const frames = [];
    for (let i = 0; i < n; i++) {
      const t = tangents[i];
      // Remove the tangent component (parallel transport).
      nrm = vnorm(vsub(nrm, vscale(t, nrm[0] * t[0] + nrm[1] * t[1] + nrm[2] * t[2])));
      const bin = vcross(t, nrm);
      frames.push([nrm, bin]);
    }
    const ringAt = (i) => {
      const [a, b] = frames[i];
      const out = [];
      for (let s = 0; s <= seg; s++) {
        const ang = (s / seg) * Math.PI * 2;
        const d = vadd(vscale(a, Math.cos(ang)), vscale(b, Math.sin(ang)));
        out.push([vadd(points[i], vscale(d, R(i))), d]);
      }
      return out;
    };
    const rings = points.map((_, i) => ringAt(i));
    const saved = this._color;
    for (let i = 0; i + 1 < n; i++) {
      if (colors) this._color = col(colors[i]);
      for (let s = 0; s < seg; s++) {
        const [p00, n00] = rings[i][s], [p01, n01] = rings[i][s + 1], [p10, n10] = rings[i + 1][s], [p11, n11] = rings[i + 1][s + 1];
        this.quadN(p00, p01, p11, p10, n00, n01, n11, n10);
      }
    }
    this._color = saved;
    if (caps) {
      for (const [i, sign] of [[0, -1], [n - 1, 1]]) {
        if (R(i) < 1e-4) continue;
        const c = points[i];
        const nn = vscale(tangents[i], sign);
        for (let s = 0; s < seg; s++) {
          const a = rings[i][s][0], b = rings[i][s + 1][0];
          if (sign > 0) this.triN(c, a, b, nn, nn, nn);
          else this.triN(c, b, a, nn, nn, nn);
        }
      }
    }
    return this;
  }

  /** An icosphere of radius `r` (detail 0..3), smooth; `deform(p) -> scale` displaces it. */
  sphere(r, detail = 1, { deform = null, squash = 1, flat = false } = {}) {
    const { verts, faces } = icosphere(detail);
    const P = verts.map((v) => {
      const k = deform ? deform(v) : 1;
      return [v[0] * r * k, v[1] * r * k * squash, v[2] * r * k];
    });
    // Smooth normals by face accumulation (deformed shapes need real normals).
    const N = P.map(() => [0, 0, 0]);
    for (const [a, b, c] of faces) {
      const fn = vcross(vsub(P[b], P[a]), vsub(P[c], P[a]));
      for (const i of [a, b, c]) N[i] = vadd(N[i], fn);
    }
    for (const [a, b, c] of faces) {
      if (flat) this.tri(P[a], P[b], P[c]);
      else this.triN(P[a], P[b], P[c], vnorm(N[a]), vnorm(N[b]), vnorm(N[c]));
    }
    return this;
  }

  /** A lumpy blob (canopies, rocks, clouds of leaves): a sphere displaced by noise `f(p)` in
   *  -1..1 with amplitude `amp`. */
  blob(r, { detail = 2, amp = 0.25, noise, squash = 1, flat = false } = {}) {
    return this.sphere(r, detail, { deform: (v) => 1 + amp * noise(v), squash, flat });
  }

  /** A prism: the polygon `poly` ([[x, z], ...] counter-clockwise seen from above) from y = 0 to
   *  `h`, with a top `inset` scale. */
  prism(poly, h, { top = 1, cap = true } = {}) {
    const n = poly.length;
    const cx = poly.reduce((a, p) => a + p[0], 0) / n;
    const cz = poly.reduce((a, p) => a + p[1], 0) / n;
    const T = poly.map(([x, z]) => [cx + (x - cx) * top, h, cz + (z - cz) * top]);
    const B = poly.map(([x, z]) => [x, 0, z]);
    for (let i = 0; i < n; i++) {
      const j = (i + 1) % n;
      this.quad(B[i], T[i], T[j], B[j]);
    }
    if (cap) for (let i = 1; i + 1 < n; i++) this.tri(T[0], T[i + 1], T[i]);
    return this;
  }

  /** A gable roof over a `w` (x) by `d` (z) rectangle, ridge along x at height `h`, eaves
   *  overhanging by `over`; `sag` bows the ridge (old roofs), `thick` the slab's edge. */
  gable(w, d, h, { over = 0.1, sag = 0, thick = 0.02 } = {}) {
    const x = w / 2 + over, z = d / 2 + over;
    const segs = sag ? 4 : 1;
    for (let i = 0; i < segs; i++) {
      const xa = -x + (2 * x * i) / segs, xb = -x + (2 * x * (i + 1)) / segs;
      const ya = h - sag * Math.sin((Math.PI * i) / segs), yb = h - sag * Math.sin((Math.PI * (i + 1)) / segs);
      this.quad([xa, ya, 0], [xb, yb, 0], [xb, -thick * 0, -z], [xa, 0, -z]);
      this.quad([xa, ya, 0], [xa, 0, z], [xb, 0, z], [xb, yb, 0]);
      if (thick) {
        this.quad([xa, 0, -z], [xb, 0, -z], [xb, -thick, -z], [xa, -thick, -z]);
        this.quad([xa, -thick, z], [xb, -thick, z], [xb, 0, z], [xa, 0, z]);
      }
    }
    // Gable ends (the triangles under the ridge at both x ends).
    const y0 = h - (sag ? 0 : 0);
    this.tri([-x + over, 0, -z + over], [-x + over, 0, z - over], [-x + over, y0 * (1 - over / z), 0]);
    this.tri([x - over, 0, z - over], [x - over, 0, -z + over], [x - over, y0 * (1 - over / z), 0]);
    return this;
  }

  /** A hipped/pyramid roof: a box's top `w` x `d` rising to a ridge of length `ridge` (0 = a
   *  pyramid) at `h`, overhang `over`. */
  hip(w, d, h, { over = 0.08, ridge = 0 } = {}) {
    const x = w / 2 + over, z = d / 2 + over, r = ridge / 2;
    const a = [-x, 0, -z], b = [x, 0, -z], c = [x, 0, z], e = [-x, 0, z];
    const t0 = [-r, h, 0], t1 = [r, h, 0];
    this.quad(a, t0, t1, b);
    this.quad(c, t1, t0, e);
    this.tri(e, t0, a);
    this.tri(b, t1, c);
    this.quad(a, b, c, e); // underside
    return this;
  }

  /** A double-sided flat card (leaves, fronds, sails, banners): the quad a, b, c, d. */
  card(a, b, c, d) {
    this.quad(a, b, c, d);
    this.quad(a, d, c, b);
    return this;
  }

  /** Appends another builder's vertices through the current transform. */
  append(other) {
    const v = other.v;
    for (let i = 0; i < v.length; i += FLOATS) {
      const p = this.xp([v[i], v[i + 1], v[i + 2]]);
      const n = this.xn([v[i + 3], v[i + 4], v[i + 5]]);
      this.v.push(p[0], p[1], p[2], n[0], n[1], n[2], v[i + 6], v[i + 7], v[i + 8], v[i + 9], v[i + 10], v[i + 11], v[i + 12]);
    }
    return this;
  }

  // --- per-part edits (on the vertices from `mark` on) ----------------------------------------

  /** Bends normals toward the direction from `center` (asset space): 1 = fully spherical. */
  sphereNormals(from, center, blend = 1) {
    const v = this.v;
    for (let i = from * FLOATS; i < v.length; i += FLOATS) {
      const d = vnorm([v[i] - center[0], v[i + 1] - center[1], v[i + 2] - center[2]]);
      const n = vnorm([v[i + 3] * (1 - blend) + d[0] * blend, v[i + 4] * (1 - blend) + d[1] * blend, v[i + 5] * (1 - blend) + d[2] * blend]);
      v[i + 3] = n[0];
      v[i + 4] = n[1];
      v[i + 5] = n[2];
    }
    return this;
  }

  /** Occlusion from height: vertices at y0 get `min`, at y1 and above 1 (multiplied in). */
  heightAO(from, y0, y1, min = 0.45) {
    const v = this.v;
    for (let i = from * FLOATS; i < v.length; i += FLOATS) {
      const t = Math.max(0, Math.min(1, (v[i + 1] - y0) / (y1 - y0)));
      v[i + 12] *= min + (1 - min) * t * t * (3 - 2 * t);
    }
    return this;
  }

  /** Sway from height: 0 at y0 rising to `max` at y1 (wind on canopies, cloth). */
  heightSway(from, y0, y1, max = 1) {
    const v = this.v;
    for (let i = from * FLOATS; i < v.length; i += FLOATS) {
      const t = Math.max(0, Math.min(1, (v[i + 1] - y0) / (y1 - y0)));
      v[i + 10] = Math.max(v[i + 10], max * t);
    }
    return this;
  }

  /** Multiplies the colour of each vertex from `from` on by `f(x, y, z, i) -> [r, g, b]` or a
   *  scalar (painted variation baked into the vertices). */
  tint(from, f) {
    const v = this.v;
    for (let i = from * FLOATS; i < v.length; i += FLOATS) {
      const k = f(v[i], v[i + 1], v[i + 2], i / FLOATS);
      if (typeof k === "number") {
        v[i + 6] *= k;
        v[i + 7] *= k;
        v[i + 8] *= k;
      } else {
        v[i + 6] *= k[0];
        v[i + 7] *= k[1];
        v[i + 8] *= k[2];
      }
    }
    return this;
  }

  /** Moves each vertex from `from` on by `f(x, y, z) -> [dx, dy, dz]` (bends, droops). */
  displace(from, f) {
    const v = this.v;
    for (let i = from * FLOATS; i < v.length; i += FLOATS) {
      const d = f(v[i], v[i + 1], v[i + 2]);
      v[i] += d[0];
      v[i + 1] += d[1];
      v[i + 2] += d[2];
    }
    return this;
  }

  /** Smooth normals for the vertices from `from` on, recomputed from their triangles (area-
   *  weighted, shared between vertices at the same position): after `displace` has bent a part,
   *  so it shades as its new shape. Hard edges within the part are smoothed too. */
  smoothNormals(from) {
    const v = this.v;
    const acc = new Map();
    const key = (i) => `${Math.round(v[i] * 1e5)},${Math.round(v[i + 1] * 1e5)},${Math.round(v[i + 2] * 1e5)}`;
    const n0 = from * FLOATS, nv = (v.length - n0) / FLOATS;
    for (let t = 0; t + 2 < nv; t += 3) {
      const a = n0 + t * FLOATS, b = a + FLOATS, c = b + FLOATS;
      const e1 = [v[b] - v[a], v[b + 1] - v[a + 1], v[b + 2] - v[a + 2]];
      const e2 = [v[c] - v[a], v[c + 1] - v[a + 1], v[c + 2] - v[a + 2]];
      const fn = vcross(e1, e2);
      for (const i of [a, b, c]) {
        const k = key(i);
        const s = acc.get(k) ?? [0, 0, 0];
        s[0] += fn[0];
        s[1] += fn[1];
        s[2] += fn[2];
        acc.set(k, s);
      }
    }
    for (let i = n0; i < v.length; i += FLOATS) {
      const s = acc.get(key(i));
      if (!s || vlen(s) < 1e-12) continue;
      const n = vnorm(s);
      v[i + 3] = n[0];
      v[i + 4] = n[1];
      v[i + 5] = n[2];
    }
    return this;
  }

  /** The vertex data (Float32Array, FLOATS per vertex) and bounds. */
  build() {
    const data = new Float32Array(this.v);
    let lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
    for (let i = 0; i < data.length; i += FLOATS) {
      for (let k = 0; k < 3; k++) {
        lo[k] = Math.min(lo[k], data[i + k]);
        hi[k] = Math.max(hi[k], data[i + k]);
      }
    }
    if (!data.length) lo = hi = [0, 0, 0];
    return { data, count: data.length / FLOATS, lo, hi, radius: Math.max(vlen(lo), vlen(hi)) };
  }
}

const ICO = new Map();

/** An icosphere's unit vertices and faces at `detail` subdivisions (cached). */
export function icosphere(detail) {
  if (ICO.has(detail)) return ICO.get(detail);
  const t = (1 + Math.sqrt(5)) / 2;
  let verts = [[-1, t, 0], [1, t, 0], [-1, -t, 0], [1, -t, 0], [0, -1, t], [0, 1, t], [0, -1, -t], [0, 1, -t], [t, 0, -1], [t, 0, 1], [-t, 0, -1], [-t, 0, 1]].map(vnorm);
  let faces = [[0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8], [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1]];
  for (let d = 0; d < detail; d++) {
    const mid = new Map();
    const m = (a, b) => {
      const k = a < b ? `${a},${b}` : `${b},${a}`;
      if (!mid.has(k)) {
        verts.push(vnorm(vscale(vadd(verts[a], verts[b]), 0.5)));
        mid.set(k, verts.length - 1);
      }
      return mid.get(k);
    };
    const next = [];
    for (const [a, b, c] of faces) {
      const ab = m(a, b), bc = m(b, c), ca = m(c, a);
      next.push([a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]);
    }
    faces = next;
  }
  const out = { verts, faces };
  ICO.set(detail, out);
  return out;
}
