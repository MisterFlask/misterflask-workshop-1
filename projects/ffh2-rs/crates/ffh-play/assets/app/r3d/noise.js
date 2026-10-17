// Deterministic randomness for the 3D renderer (docs/R3D.md): integer hashes, a seeded generator
// and gradient noise. Everything the map's look depends on (terrain relief, tree placement, house
// layouts) is a function of the plot index and these, so a plot looks the same every time it is
// drawn, on every machine, and nothing is drawn from Math.random().

/** A well-mixed 32-bit hash of an integer. */
export function hash32(n) {
  let h = Math.imul((n | 0) ^ 0x9e3779b9, 0x85ebca6b);
  h ^= h >>> 13;
  h = Math.imul(h, 0xc2b2ae35);
  h ^= h >>> 16;
  return h >>> 0;
}

/** A hash of several integers, in 0..1. */
export function hashf(...ns) {
  let h = 0x811c9dc5;
  for (const n of ns) h = hash32(h ^ Math.imul(n | 0, 0x27d4eb2d));
  return h / 4294967296;
}

/** A seeded generator (mulberry32): `r()` in 0..1, plus helpers. */
export function rng(seed) {
  let a = hash32(seed) || 1;
  const r = () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  r.range = (lo, hi) => lo + (hi - lo) * r();
  r.int = (lo, hi) => lo + Math.floor(r() * (hi - lo + 1));
  r.pick = (list) => list[Math.floor(r() * list.length)];
  r.sign = () => (r() < 0.5 ? -1 : 1);
  /** Roughly normal (sum of three), mean 0, spread ~`s`. */
  r.gauss = (s = 1) => (r() + r() + r() - 1.5) * s * 1.15;
  return r;
}

// Gradient noise (Perlin's improved noise), fixed permutation so it is the same everywhere.
const PERM = new Uint8Array(512);
{
  const r = rng(1337);
  const p = Array.from({ length: 256 }, (_, i) => i);
  for (let i = 255; i > 0; i--) {
    const j = Math.floor(r() * (i + 1));
    [p[i], p[j]] = [p[j], p[i]];
  }
  for (let i = 0; i < 512; i++) PERM[i] = p[i & 255];
}

function fade(t) {
  return t * t * t * (t * (t * 6 - 15) + 10);
}

function grad2(h, x, y) {
  switch (h & 7) {
    case 0: return x + y;
    case 1: return -x + y;
    case 2: return x - y;
    case 3: return -x - y;
    case 4: return x;
    case 5: return -x;
    case 6: return y;
    default: return -y;
  }
}

/** 2D gradient noise, about -1..1. */
export function noise2(x, y) {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const X = xi & 255;
  const Y = yi & 255;
  const xf = x - xi;
  const yf = y - yi;
  const u = fade(xf);
  const v = fade(yf);
  const aa = PERM[PERM[X] + Y];
  const ab = PERM[PERM[X] + Y + 1];
  const ba = PERM[PERM[X + 1] + Y];
  const bb = PERM[PERM[X + 1] + Y + 1];
  const x1 = grad2(aa, xf, yf) + u * (grad2(ba, xf - 1, yf) - grad2(aa, xf, yf));
  const x2 = grad2(ab, xf, yf - 1) + u * (grad2(bb, xf - 1, yf - 1) - grad2(ab, xf, yf - 1));
  return (x1 + v * (x2 - x1)) * 0.7;
}

function grad3(h, x, y, z) {
  const hh = h & 15;
  const u = hh < 8 ? x : y;
  const v = hh < 4 ? y : hh === 12 || hh === 14 ? x : z;
  return ((hh & 1) === 0 ? u : -u) + ((hh & 2) === 0 ? v : -v);
}

/** 3D gradient noise, about -1..1. */
export function noise3(x, y, z) {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const zi = Math.floor(z);
  const X = xi & 255;
  const Y = yi & 255;
  const Z = zi & 255;
  const xf = x - xi;
  const yf = y - yi;
  const zf = z - zi;
  const u = fade(xf);
  const v = fade(yf);
  const w = fade(zf);
  const A = PERM[X] + Y;
  const AA = PERM[A] + Z;
  const AB = PERM[A + 1] + Z;
  const B = PERM[X + 1] + Y;
  const BA = PERM[B] + Z;
  const BB = PERM[B + 1] + Z;
  const l = (a, b, t) => a + t * (b - a);
  return l(
    l(l(grad3(PERM[AA], xf, yf, zf), grad3(PERM[BA], xf - 1, yf, zf), u), l(grad3(PERM[AB], xf, yf - 1, zf), grad3(PERM[BB], xf - 1, yf - 1, zf), u), v),
    l(l(grad3(PERM[AA + 1], xf, yf, zf - 1), grad3(PERM[BA + 1], xf - 1, yf, zf - 1), u), l(grad3(PERM[AB + 1], xf, yf - 1, zf - 1), grad3(PERM[BB + 1], xf - 1, yf - 1, zf - 1), u), v),
    w,
  );
}

/** Fractal sum of 2D noise, about -1..1. */
export function fbm2(x, y, oct = 4, lac = 2.03, gain = 0.5) {
  let a = 1;
  let s = 0;
  let n = 0;
  for (let i = 0; i < oct; i++) {
    s += a * noise2(x, y);
    n += a;
    x = x * lac + 17.3;
    y = y * lac - 9.1;
    a *= gain;
  }
  return s / n;
}

/** Ridged fractal noise in 0..1: sharp crests where the noise crosses zero (mountain ridges). */
export function ridged2(x, y, oct = 4, lac = 2.1, gain = 0.5) {
  let a = 1;
  let s = 0;
  let n = 0;
  let w = 1;
  for (let i = 0; i < oct; i++) {
    let r = 1 - Math.abs(noise2(x, y));
    r *= r;
    s += a * r * w;
    n += a;
    w = Math.min(1, r * 1.6);
    x = x * lac + 31.7;
    y = y * lac + 11.3;
    a *= gain;
  }
  return s / n;
}

export function fbm3(x, y, z, oct = 3) {
  let a = 1;
  let s = 0;
  let n = 0;
  for (let i = 0; i < oct; i++) {
    s += a * noise3(x, y, z);
    n += a;
    x *= 2.02;
    y *= 2.02;
    z *= 2.02;
    a *= 0.5;
  }
  return s / n;
}

/** 2D gradient noise periodic in x with period P (whole lattice cells): noise2p(x + P, y, P) ===
 *  noise2p(x, y, P). For maps that wrap east-west. */
export function noise2p(x, y, P) {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const xf = x - xi;
  const yf = y - yi;
  const X0 = (((xi % P) + P) % P) & 255;
  const X1 = ((((xi + 1) % P) + P) % P) & 255;
  const Y = yi & 255;
  const u = fade(xf);
  const v = fade(yf);
  const aa = PERM[PERM[X0] + Y];
  const ab = PERM[PERM[X0] + Y + 1];
  const ba = PERM[PERM[X1] + Y];
  const bb = PERM[PERM[X1] + Y + 1];
  const x1 = grad2(aa, xf, yf) + u * (grad2(ba, xf - 1, yf) - grad2(aa, xf, yf));
  const x2 = grad2(ab, xf, yf - 1) + u * (grad2(bb, xf - 1, yf - 1) - grad2(ab, xf, yf - 1));
  return (x1 + v * (x2 - x1)) * 0.7;
}

/** fbm2 with a period P in x (lacunarity 2, so every octave stays periodic). */
export function fbm2p(x, y, P, oct = 4, gain = 0.5) {
  let a = 1, s = 0, n = 0, p = P;
  for (let i = 0; i < oct; i++) {
    s += a * noise2p(x, y, p);
    n += a;
    x = x * 2 + 17.3;
    y = y * 2 - 9.1;
    p *= 2;
    a *= gain;
  }
  return s / n;
}

/** ridged2 with a period P in x (lacunarity 2). */
export function ridged2p(x, y, P, oct = 4, gain = 0.5) {
  let a = 1, s = 0, n = 0, w = 1, p = P;
  for (let i = 0; i < oct; i++) {
    let r = 1 - Math.abs(noise2p(x, y, p));
    r *= r;
    s += a * r * w;
    n += a;
    w = Math.min(1, r * 1.6);
    x = x * 2 + 31.7;
    y = y * 2 + 11.3;
    p *= 2;
    a *= gain;
  }
  return s / n;
}
