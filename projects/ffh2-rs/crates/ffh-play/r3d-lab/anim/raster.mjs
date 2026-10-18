// raster.mjs: a small software renderer for baked animation frames (docs/ANIMATION.md
// "Validation"): the figure seen from the game's camera (the map camera's pitch at a zoom, the
// stack's yaw), flat-shaded under the daylight sun, with a z-buffer, an object mask (the
// silhouette) and screen-space marks (trails, points). Deterministic and node-only (no browser,
// no GPU): fast enough to render every frame of every clip, so the validator can measure
// silhouettes and the review sheets can show every frame. It is not the real renderer: no
// shadows, paint filter or ink; tools/r3d-anim.mjs --lab renders filmstrips with the real one.

import zlib from "node:zlib";

/** The map camera's pitch (radians) at `ts` pixels per plot (map3d.js camera()). */
export function pitchAt(ts) {
  const Z0 = 28, Z1 = 220;
  const zt = Math.min(1, Math.max(0, Math.log(ts / Z0) / Math.log(Z1 / Z0)));
  return ((50 + 12 - zt * 24) * Math.PI) / 180;
}

/**
 * A view of figure space: `ts` pixels per plot, `yaw` the instance's turn (the head of a stack
 * stands at 0.25), `w` x `h` pixels with the figure's feet at (`ox`, `oy`) (pixels from the
 * top-left), `ss` supersampling. Returns {project(p) -> [sx, sy, depth], ...}.
 */
export function camera({ ts = 96, yaw = 0.25, w = 64, h = 64, ox = null, oy = null, pitch = null } = {}) {
  const th = pitch ?? pitchAt(ts);
  const cy = Math.cos(yaw), sy = Math.sin(yaw), cp = Math.cos(th), sp = Math.sin(th);
  const fx = ox ?? w / 2, fy = oy ?? h * 0.86;
  return {
    ts, yaw, w, h, pitch: th,
    /** Figure space -> [screen x, screen y (down), depth (larger = nearer the camera)]. */
    project(p) {
      // The instance's yaw (shaders.js rotY), then the camera looking north, pitched down.
      const x = cy * p[0] + sy * p[2], y = p[1], z = -sy * p[0] + cy * p[2];
      return [fx + x * ts, fy - (y * cp - z * sp) * ts, y * sp + z * cp];
    },
    /** The direction a figure-space vector points on screen ([dx, dy], y down; not normalised). */
    screenDir(v) {
      const x = cy * v[0] + sy * v[2], y = v[1], z = -sy * v[0] + cy * v[2];
      return [x, -(y * cp - z * sp)];
    },
  };
}

const SUN = norm([-0.65, 0.67, 0.35]);

function norm(v) {
  const l = Math.hypot(v[0], v[1], v[2]) || 1;
  return [v[0] / l, v[1] / l, v[2] / l];
}

const toSrgb = (c) => {
  const x = Math.max(0, Math.min(1, c));
  return x <= 0.0031308 ? x * 12.92 : 1.055 * Math.pow(x, 1 / 2.4) - 0.055;
};

/** An RGBA image with a depth buffer and an id plane. */
export class Image {
  constructor(w, h, bg = [0, 0, 0, 0]) {
    this.w = w;
    this.h = h;
    this.px = new Uint8ClampedArray(w * h * 4);
    this.depth = new Float32Array(w * h).fill(-Infinity);
    this.id = new Int16Array(w * h).fill(-1);
    for (let i = 0; i < w * h; i++) this.px.set(bg, i * 4);
  }

  set(x, y, c, a = 1) {
    x = Math.round(x);
    y = Math.round(y);
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    const o = (y * this.w + x) * 4;
    for (let k = 0; k < 3; k++) this.px[o + k] = this.px[o + k] * (1 - a) + c[k] * a;
    this.px[o + 3] = Math.max(this.px[o + 3], 255 * a);
  }

  /** A line from a to b (screen pixels), `c` [r, g, b] 0..255. */
  line(a, b, c, a1 = 1) {
    const n = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1])));
    for (let i = 0; i <= n; i++) this.set(a[0] + ((b[0] - a[0]) * i) / n, a[1] + ((b[1] - a[1]) * i) / n, c, a1);
  }

  dot(p, r, c, a1 = 1) {
    for (let dy = -r; dy <= r; dy++) for (let dx = -r; dx <= r; dx++) if (dx * dx + dy * dy <= r * r + 0.5) this.set(p[0] + dx, p[1] + dy, c, a1);
  }

  fill(x0, y0, w, h, c) {
    for (let y = y0; y < y0 + h; y++) for (let x = x0; x < x0 + w; x++) this.set(x, y, c);
  }

  /** Pastes image `im` at (x, y), scaled by integer `k` (nearest), over this one. */
  paste(im, x, y, k = 1) {
    for (let j = 0; j < im.h * k; j++) {
      for (let i = 0; i < im.w * k; i++) {
        const o = (Math.floor(j / k) * im.w + Math.floor(i / k)) * 4;
        const a = im.px[o + 3] / 255;
        if (a > 0) this.set(x + i, y + j, [im.px[o], im.px[o + 1], im.px[o + 2]], a);
      }
    }
  }

  png() {
    return encodePNG(this.w, this.h, this.px);
  }
}

/**
 * Renders frame `f` of a bake (anim.js bakeClip) with camera `cam` into a new Image (or `into`).
 * opts: {team [r, g, b] linear, ss (supersampling, default 3), ghost (alpha: an onion skin),
 *        flat (one colour: a silhouette), tint [r, g, b] 0..255}.
 * The returned image's `mask` (Uint8Array w*h, 0..255 coverage) is the silhouette.
 */
export function renderFrame(bake, f, cam, opts = {}) {
  const ss = opts.ss ?? 3;
  const W = cam.w * ss, H = cam.h * ss;
  const hi = new Image(W, H);
  const team = opts.team ?? [0.62, 0.12, 0.09];
  const rest = bake.rest.data;
  const pos = bake.pos, nrm = bake.nrm, count = bake.count;
  const base = f * count * 3;
  const P = new Float32Array(count * 3);
  const C = { ...cam, project: (p) => { const q = cam.project(p); return [q[0] * ss, q[1] * ss, q[2]]; } };
  for (let v = 0; v < count; v++) {
    const q = C.project([pos[base + v * 3], pos[base + v * 3 + 1], pos[base + v * 3 + 2]]);
    P[v * 3] = q[0];
    P[v * 3 + 1] = q[1];
    P[v * 3 + 2] = q[2];
  }
  const cy = Math.cos(cam.yaw), sy = Math.sin(cam.yaw);
  for (let t = 0; t < count; t += 3) {
    const ax = P[t * 3], ay = P[t * 3 + 1], az = P[t * 3 + 2];
    const bx = P[t * 3 + 3], by = P[t * 3 + 4], bz = P[t * 3 + 5];
    const cx = P[t * 3 + 6], cyy = P[t * 3 + 7], cz = P[t * 3 + 8];
    const area = (bx - ax) * (cyy - ay) - (by - ay) * (cx - ax);
    if (Math.abs(area) < 1e-9) continue;
    // Shade: the triangle's mean normal under the sun (two-sided, as the prop shader lights).
    let nx = 0, ny = 0, nz = 0;
    for (let k = 0; k < 3; k++) {
      const o = base + (t + k) * 3;
      nx += nrm[o];
      ny += nrm[o + 1];
      nz += nrm[o + 2];
    }
    const wn = norm([cy * nx + sy * nz, ny, -sy * nx + cy * nz]);
    const d = wn[0] * SUN[0] + wn[1] * SUN[1] + wn[2] * SUN[2];
    const light = 0.38 + 0.62 * Math.max(0, Math.abs(d) * Math.sign(d + 0.3));
    let col;
    if (opts.flat) col = opts.flat;
    else {
      const o = t * 13;
      const mat = rest[o + 9];
      let alb = [rest[o + 6], rest[o + 7], rest[o + 8]];
      if (mat === 3) {
        const l = 0.2126 * alb[0] + 0.7152 * alb[1] + 0.0722 * alb[2];
        alb = team.map((c) => c * (0.38 + 1.25 * l));
      }
      const ao = rest[o + 12];
      col = alb.map((c) => Math.round(255 * toSrgb(c * light * 1.9 * (0.55 + 0.45 * ao))));
    }
    const x0 = Math.max(0, Math.floor(Math.min(ax, bx, cx))), x1 = Math.min(W - 1, Math.ceil(Math.max(ax, bx, cx)));
    const y0 = Math.max(0, Math.floor(Math.min(ay, by, cyy))), y1 = Math.min(H - 1, Math.ceil(Math.max(ay, by, cyy)));
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        const px = x + 0.5, py = y + 0.5;
        const w0 = ((bx - px) * (cyy - py) - (by - py) * (cx - px)) / area;
        const w1 = ((cx - px) * (ay - py) - (cyy - py) * (ax - px)) / area;
        const w2 = 1 - w0 - w1;
        if (w0 < 0 || w1 < 0 || w2 < 0) continue;
        const z = w0 * az + w1 * bz + w2 * cz;
        const i = y * W + x;
        if (z <= hi.depth[i]) continue;
        hi.depth[i] = z;
        hi.id[i] = bake.bone[t];
        const o = i * 4;
        hi.px[o] = col[0];
        hi.px[o + 1] = col[1];
        hi.px[o + 2] = col[2];
        hi.px[o + 3] = 255;
      }
    }
  }
  // Down-sample (box filter) to the camera's size; the mask is the coverage.
  const out = opts.into ?? new Image(cam.w, cam.h);
  const mask = new Uint8Array(cam.w * cam.h);
  const ga = opts.ghost ?? 1;
  for (let y = 0; y < cam.h; y++) {
    for (let x = 0; x < cam.w; x++) {
      let r = 0, g = 0, b = 0, a = 0;
      for (let j = 0; j < ss; j++) {
        for (let i = 0; i < ss; i++) {
          const o = ((y * ss + j) * W + x * ss + i) * 4;
          const al = hi.px[o + 3] / 255;
          r += hi.px[o] * al;
          g += hi.px[o + 1] * al;
          b += hi.px[o + 2] * al;
          a += al;
        }
      }
      const cov = a / (ss * ss);
      mask[y * cam.w + x] = Math.round(cov * 255);
      if (a > 0) {
        let c = [r / a, g / a, b / a];
        if (opts.tint) c = c.map((v, k) => v * 0.45 + opts.tint[k] * 0.55);
        out.set(x, y, c, cov * ga);
      }
    }
  }
  out.mask = mask;
  // `split`: a bone id; the coverage of that bone's front-most samples and of everything else.
  if (opts.split !== undefined) {
    const sw = new Uint8Array(cam.w * cam.h), so = new Uint8Array(cam.w * cam.h);
    for (let y = 0; y < cam.h; y++) {
      for (let x = 0; x < cam.w; x++) {
        let a = 0, o = 0;
        for (let j = 0; j < ss; j++) {
          for (let i = 0; i < ss; i++) {
            const id = hi.id[(y * ss + j) * W + x * ss + i];
            if (id === opts.split) a++;
            else if (id >= 0) o++;
          }
        }
        sw[y * cam.w + x] = Math.round((a / (ss * ss)) * 255);
        so[y * cam.w + x] = Math.round((o / (ss * ss)) * 255);
      }
    }
    out.split = { weapon: sw, other: so };
  }
  return out;
}

/** A silhouette (coverage 0..255, w*h) of frame f: renderFrame without colour. */
export function silhouette(bake, f, cam, ss = 3) {
  return renderFrame(bake, f, cam, { ss, flat: [255, 255, 255] }).mask;
}

// --- text: a 3x5 pixel font for the sheets' labels ------------------------------------------------

const GLYPHS = {
  A: "111101111101101", B: "110101110101110", C: "111100100100111", D: "110101101101110", E: "111100110100111",
  F: "111100110100100", G: "111100101101111", H: "101101111101101", I: "111010010010111", J: "001001001101111",
  K: "101101110101101", L: "100100100100111", M: "101111111101101", N: "111101101101101", O: "111101101101111",
  P: "111101111100100", Q: "111101101111001", R: "111101110101101", S: "011100010001110", T: "111010010010010",
  U: "101101101101111", V: "101101101101010", W: "101101111111101", X: "101101010101101", Y: "101101010010010",
  Z: "111001010100111", 0: "111101101101111", 1: "010110010010111", 2: "111001111100111", 3: "111001111001111",
  4: "101101111001001", 5: "111100111001111", 6: "111100111101111", 7: "111001001001001", 8: "111101111101111",
  9: "111101111001111", ".": "000000000000010", "-": "000000111000000", ":": "000010000010000", "/": "001001010100100",
  "%": "101001010100101", "=": "000111000111000", "+": "000010111010000", "(": "010100100100010", ")": "010001001001010",
  ">": "100010001010100", "<": "001010100010001", "_": "000000000000111", "#": "101111101111101", " ": "000000000000000",
};

/** Writes `text` at (x, y) in the 3x5 font scaled by k. */
export function text(im, x, y, str, c = [255, 255, 255], k = 1) {
  let cx = x;
  for (const ch of String(str).toUpperCase()) {
    const g = GLYPHS[ch] ?? GLYPHS[" "];
    for (let j = 0; j < 5; j++) for (let i = 0; i < 3; i++) if (g[j * 3 + i] === "1") im.fill(cx + i * k, y + j * k, k, k, c);
    cx += 4 * k;
  }
  return cx;
}

// --- PNG ------------------------------------------------------------------------------------------

const CRC = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}

/** RGBA pixels -> a PNG file (Buffer). */
export function encodePNG(w, h, px) {
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (w * 4 + 1)] = 0;
    Buffer.from(px.buffer, px.byteOffset + y * w * 4, w * 4).copy(raw, y * (w * 4 + 1) + 1);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", zlib.deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}
