// GLSL of the 3D renderer (docs/R3D.md "Shaders"). The scene passes (terrain, props, water,
// rivers, shadows) write three targets -- lit colour (linear HDR, alpha = "keep sharp"), view
// normal + linear depth (+ object id), emission -- and the post passes turn them into the painted
// frame: ambient occlusion, glow, tone mapping, the anisotropic Kuwahara filter, ink outlines and
// the grade. The strategic overlays (borders, rings, selection, paths) draw after the post passes,
// so no filter smears them.
//
// World-anchored detail: every procedural pattern on the ground is a function of the map point
// (plot units), periodic over the map's width when it wraps (no seam where the map joins), so the
// painting is fixed to the world and never swims under a moving camera.

// --------------------------------------------------------------------------------------------
// Shared pieces

export const COMMON = `
uniform mat4 uViewProj;
uniform mat4 uView;
uniform vec3 uEye;
uniform float uTime;
uniform vec3 uSunDir;
uniform vec3 uSunColor;
uniform vec3 uSkyColor;
uniform vec3 uGroundColor;
uniform vec3 uHazeColor;
uniform vec3 uShadowTint;
uniform float uWrapLight;
uniform float uBands;
uniform float uRim;
uniform float uPaint;
uniform float uHaze;
uniform vec2 uHazeRange;
uniform float uMist;
uniform float uFogWar;
uniform float uStylize;
uniform float uClouds;
uniform float uDabs;
uniform float uBrush;
uniform sampler2D uShadowMap;
uniform sampler2D uShadowMapT;
uniform float uTerrainShadow;
uniform mat4 uShadowMat;
uniform float uShadowOn;
uniform vec2 uShadowTexel;
uniform sampler2D uVis;
uniform sampler2D uLight;
uniform vec2 uMapSize;
uniform float uWrapX;
uniform float uCopyX;

const float PI = 3.14159265;

vec3 srgb(vec3 c) { return pow(c, vec3(2.2)); }
float lum(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }

float hash12(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.x + p3.y) * p3.z);
}
vec2 hash22(vec2 p) {
  vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
  p3 += dot(p3, p3.yzx + 33.33);
  return fract((p3.xx + p3.yz) * p3.zy);
}
float hash13(vec3 p3) {
  p3 = fract(p3 * 0.1031);
  p3 += dot(p3, p3.zyx + 31.32);
  return fract((p3.x + p3.y) * p3.z);
}

// Value noise on a lattice periodic over P (cells).
float pnoise(vec2 p, vec2 P) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  float a = hash12(mod(i, P));
  float b = hash12(mod(i + vec2(1.0, 0.0), P));
  float c = hash12(mod(i + vec2(0.0, 1.0), P));
  float d = hash12(mod(i + vec2(1.0, 1.0), P));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Map-anchored noise at about f cycles per plot (0..1), periodic over the map's width on a
// wrapping map.
float mnoise(vec2 m, float f) {
  if (uWrapX > 0.5) {
    float P = max(1.0, floor(uMapSize.x * f + 0.5));
    return pnoise(vec2(m.x * P / uMapSize.x, m.y * f), vec2(P, 4096.0));
  }
  return pnoise(m * f, vec2(4096.0));
}

float mfbm(vec2 m, float f) {
  return (mnoise(m, f) * 0.5 + mnoise(m + 3.7, f * 2.0) * 0.25 + mnoise(m - 5.1, f * 4.0) * 0.125 + mnoise(m + 9.3, f * 8.0) * 0.0625) / 0.9375;
}

// Brush-stroke noise: streaks running along the map's diagonal (f across, f / stretch along),
// periodic like mnoise.
float mstroke(vec2 m, float f, float stretch) {
  vec2 d = vec2(m.x + m.y * 0.6, m.y - m.x * 0.6);
  float fa = f / stretch;
  float fb = f;
  if (uWrapX > 0.5) {
    float Pa = max(1.0, floor(uMapSize.x * fa + 0.5));
    float Pb = max(1.0, floor(uMapSize.x * 0.6 * fb + 0.5));
    return pnoise(vec2(d.x * Pa / uMapSize.x, d.y * Pb / (uMapSize.x * 0.6)), vec2(Pa, Pb));
  }
  return pnoise(vec2(d.x * fa, d.y * fb), vec2(4096.0));
}

// Cellular noise: (distance to nearest, to second nearest) feature point, periodic like mnoise.
vec2 mcells(vec2 m, float f) {
  float P = uWrapX > 0.5 ? max(1.0, floor(uMapSize.x * f + 0.5)) : 4096.0;
  vec2 p = uWrapX > 0.5 ? vec2(m.x * P / uMapSize.x, m.y * f) : m * f;
  vec2 i = floor(p);
  vec2 fr = fract(p);
  float d1 = 8.0, d2 = 8.0;
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 g = vec2(float(x), float(y));
      vec2 o = hash22(mod(i + g, vec2(P, 4096.0)));
      float d = length(g + o - fr);
      if (d < d1) { d2 = d1; d1 = d; } else if (d < d2) { d2 = d; }
    }
  }
  return vec2(d1, d2);
}

// Impressionist brush dabs: oriented elliptical marks on a jittered lattice (about f per plot),
// each its own tone, laid in a consistent hand (orientations within a 60-degree fan). Returns the
// tone of the dab covering the point, 0..1, and 0.5 between dabs; periodic like mnoise.
float mdabs(vec2 m, float f) {
  float P = uWrapX > 0.5 ? max(1.0, floor(uMapSize.x * f + 0.5)) : 4096.0;
  vec2 p = uWrapX > 0.5 ? vec2(m.x * P / uMapSize.x, m.y * f) : m * f;
  vec2 i = floor(p);
  vec2 fr = fract(p);
  float best = 0.0, tone = 0.5;
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 g = vec2(float(x), float(y));
      vec2 cell = mod(i + g, vec2(P, 4096.0));
      vec2 o = g + hash22(cell) - fr;
      float a = 0.35 + hash12(cell + 3.7) * 1.05;
      vec2 r = vec2(cos(a) * o.x + sin(a) * o.y, -sin(a) * o.x + cos(a) * o.y);
      float d = length(r / vec2(0.78, 0.3));
      float w = (1.0 - smoothstep(0.5, 1.0, d)) * (0.6 + 0.4 * hash12(cell + 1.3));
      if (w > best) { best = w; tone = hash12(cell + 9.1); }
    }
  }
  return mix(0.5, tone, best);
}

// 3D value noise (props' object-space pigment).
float vnoise3(vec3 p) {
  vec3 i = floor(p);
  vec3 f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(hash13(i), hash13(i + vec3(1, 0, 0)), f.x), mix(hash13(i + vec3(0, 1, 0)), hash13(i + vec3(1, 1, 0)), f.x), f.y),
             mix(mix(hash13(i + vec3(0, 0, 1)), hash13(i + vec3(1, 0, 1)), f.x), mix(hash13(i + vec3(0, 1, 1)), hash13(i + vec3(1, 1, 1)), f.x), f.y), f.z);
}

vec2 mapOf(vec3 w) { return vec2(w.x - uCopyX, -w.z); }
vec2 mapUV(vec2 m) { return m / uMapSize; }

const vec2 POISSON[12] = vec2[](
  vec2(-0.326, -0.406), vec2(-0.840, -0.074), vec2(-0.696, 0.457), vec2(-0.203, 0.621),
  vec2(0.962, -0.195), vec2(0.473, -0.480), vec2(0.519, 0.767), vec2(0.185, -0.893),
  vec2(0.507, 0.064), vec2(0.896, 0.412), vec2(-0.322, -0.933), vec2(-0.792, -0.598));

#ifdef FRAGMENT
float shadowAt(vec3 wp, vec3 n) {
  if (uShadowOn < 0.5) return 1.0;
  vec4 sp = uShadowMat * vec4(wp + n * 0.015, 1.0);
  vec3 c = sp.xyz / sp.w * 0.5 + 0.5;
  if (c.x < 0.0 || c.x > 1.0 || c.y < 0.0 || c.y > 1.0 || c.z > 1.0) return 1.0;
  float a = hash12(gl_FragCoord.xy) * 6.2831;
  mat2 r = mat2(cos(a), sin(a), -sin(a), cos(a));
  float s = 0.0;
  for (int i = 0; i < 12; i++) {
    float d = texture(uShadowMap, c.xy + r * POISSON[i] * uShadowTexel * 1.6).r;
    s += c.z - 0.0012 > d ? 0.0 : 1.0;
  }
  // The terrain's own map: wider, fewer taps (soft), at part strength.
  float t = 0.0;
  for (int i = 0; i < 6; i++) {
    float d = texture(uShadowMapT, c.xy + r * POISSON[i * 2 + 1] * uShadowTexel * 5.0).r;
    t += c.z - 0.002 > d ? 0.0 : 1.0;
  }
  return s / 12.0 * mix(1.0, t / 6.0, uTerrainShadow);
}

// The painterly light model: a wrapped, softly stepped sun term (broad light masses), coloured
// shadows from a cool sky ambient, the local light pools (cities, lava, mana) and the occlusion.
// Returns the light to multiply the albedo by; 'sun' is the sun term (for highlights).
// 'jit' is a brush mark's tone (about -0.5..0.5, 0 for none): it pushes the sun term before the
// band, so where light turns to shade the boundary is laid in dabs and strokes, each one light or
// dark, instead of a smooth gradient -- the place a painter's brushwork shows most.
vec3 lightAtJ(vec3 n, vec3 wp, float ao, float jit, out float sun) {
  float ndl = dot(n, uSunDir);
  float d = clamp((ndl + uWrapLight) / (1.0 + uWrapLight) + jit, 0.0, 1.0);
  // The light-mass band: a lower threshold so a slope turned from the (now higher) sun keeps a
  // readable value instead of being crushed near-black (F1).
  float stepped = smoothstep(0.16, 0.56, d) * 0.82 + d * 0.18;
  d = mix(d, stepped, uBands);
  float sh = shadowAt(wp, n);
  // The same marks break a cast shadow's soft edge (flecks of sun in the shade, shade in the sun);
  // fully lit and fully shaded ground are untouched.
  sh = clamp(sh + jit * 0.9 * (1.0 - abs(sh * 2.0 - 1.0)), 0.0, 1.0);
  // Cloud shadows drifting over the land: broad, soft masses of light and shade (fixed to the
  // world, slowly moving), the painter's way of composing a wide landscape.
  // Two octaves only: a cloud's shadow spans several plots and has no small blots in it.
  vec2 cm = mapOf(wp) + vec2(uTime * 0.03, uTime * 0.012);
  float cn = mnoise(cm + 31.0, 0.11) * 0.7 + mnoise(cm - 13.0, 0.23) * 0.3;
  float cloud = smoothstep(0.5, 0.72, cn);
  sh *= 1.0 - cloud * uClouds * 0.75;
  // Floor the cast/terrain shadow for the stylized look, so shaded ground keeps its colour and
  // never reads as a near-black hole (F1). Plain (uStylize 0) keeps full-strength shadows.
  sh = mix(sh, mix(0.38, 1.0, sh), uStylize);
  sun = d * sh;
  vec3 amb = mix(uGroundColor, uSkyColor, n.y * 0.5 + 0.5);
  vec3 L = uSunColor * sun + amb * ao;
  // A painter's fill from the side away from the sun: a roof or a cliff turned from it stays a
  // readable cool shade, never a black slab (the stylized look only).
  vec3 fd = normalize(vec3(-uSunDir.x, 0.4, -uSunDir.z));
  L += uSkyColor * 0.24 * max(0.0, dot(n, fd)) * ao * uStylize;
  vec3 pool = texture(uLight, mapUV(mapOf(wp))).rgb;
  L += pool * (0.55 + 0.45 * ao);
  return L;
}

vec3 lightAt(vec3 n, vec3 wp, float ao, out float sun) { return lightAtJ(n, wp, ao, 0.0, sun); }

// Coloured shadows: where the sun does not reach, colours lean to the shadow tint (painters' cool
// shadows), more so for the stylized look.
vec3 shadowColour(vec3 c, float sun) {
  return mix(c, c * uShadowTint, (1.0 - sun) * 0.55 * uStylize);
}

// Brush marks on an object, laid in the object's own space so they stay on it as it sways and
// never swim under the camera. dab2: marks on a jittered lattice (one per cell), laid in one hand
// (orientations within a fan), each its own size, length ('elong' on average: about 1 for leaf
// clusters, 2+ for strokes) and tone, with a three-lobed rather than elliptical outline (a loaded
// brush's mark, not a stamped spot); 'rad' is the mean half-size in cells. Where marks overlap the
// one pressed hardest (the larger) wins, so the surface reads as stroke laid over stroke. Tones
// are drawn from a triangular distribution: most marks near the middle, few strong accents (no
// polka dots). Returns (tone 0..1, 0.5 between marks; cover 0..1). 'aa' is the lattice cells per
// pixel: it softens a mark's edge as it shrinks on screen (no sparkle).
vec2 dab2(vec2 p, float elong, float rad, float aa) {
  vec2 i = floor(p);
  vec2 fr = fract(p);
  float soft = 0.1 + aa * 0.9;
  float best = 0.0, tone = 0.5;
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 g = vec2(float(x), float(y));
      vec2 cell = i + g;
      vec2 o = g + hash22(cell) - fr;
      float hs = hash12(cell + 1.3);
      float a = 0.3 + hash12(cell + 3.7) * 0.9;
      vec2 r = vec2(cos(a) * o.x + sin(a) * o.y, -sin(a) * o.x + cos(a) * o.y);
      float el = elong * (0.8 + 0.5 * hash12(cell + 6.1));
      vec2 q = r / (rad * (0.75 + 0.5 * hs) * vec2(sqrt(el), 1.0 / sqrt(el)));
      float len = length(q);
      float c = q.x / max(len, 1e-4);
      float d = len * (1.0 + 0.2 * (4.0 * c * c * c - 3.0 * c));
      float w = (1.0 - smoothstep(1.0 - soft, 1.0, d)) * (0.55 + 0.45 * hs);
      if (w > best) { best = w; tone = 0.5 * (hash12(cell + 9.1) + hash12(cell + 5.3)); }
    }
  }
  return vec2(mix(0.5, tone, best), best);
}

// dab2 projected onto a surface from the three axis planes, by the object-space normal (a sharp
// blend: one plane carries each face). 'f' marks per plot.
vec2 objDabs(vec3 p, vec3 n, float f, float elong, float rad, float aa) {
  vec3 w = abs(n);
  w = w * w;
  w = w * w;
  w /= dot(w, vec3(1.0)) + 1e-5;
  vec2 acc = vec2(0.0);
  float ws = 1e-4;
  if (w.x > 0.04) { acc += w.x * dab2(p.zy * f, elong, rad, aa); ws += w.x; }
  if (w.y > 0.04) { acc += w.y * dab2(p.xz * f + 17.0, elong, rad, aa); ws += w.y; }
  if (w.z > 0.04) { acc += w.z * dab2(p.xy * f + 31.0, elong, rad, aa); ws += w.z; }
  return acc / ws;
}

// Foliage marks: big leaf clusters (a few to a crown at the gameplay zoom) and smaller ones (close
// up). Each fades out as its marks shrink below a few pixels. 'dpo' = fwidth(po), taken by the
// caller outside any branch. Returns the signed tone (-0.5..0.5) and the clusters' cover.
const float LEAF_F = 13.0, LEAF_F2 = 31.0;
vec2 leafMarks(vec3 po, vec3 nO, vec3 dpo) {
  float aC = length(dpo * LEAF_F), aF = length(dpo * LEAF_F2);
  vec2 dC = objDabs(po, nO, LEAF_F, 1.15, 0.66, aC);
  float lC = 1.0 - smoothstep(0.22, 0.5, aC);
  float lF = 1.0 - smoothstep(0.22, 0.5, aF);
  float t = (dC.x - 0.5) * 1.1 * lC;
  if (lF > 0.0) t += (objDabs(po + 7.0, nO, LEAF_F2, 1.3, 0.66, aF).x - 0.5) * 0.65 * lF;
  return vec2(t, mix(1.0, dC.y, lC));
}

// Strokes on everything else (walls, roofs, rock, hulls, cloth): longer marks, a broad and a fine
// size, the same hand.
float strokeMarks(vec3 po, vec3 nO, vec3 dpo) {
  float aC = length(dpo * 9.0), aF = length(dpo * 22.0);
  float lC = 1.0 - smoothstep(0.22, 0.5, aC);
  float lF = 1.0 - smoothstep(0.22, 0.5, aF);
  float t = 0.0;
  if (lC > 0.0) t += (objDabs(po, nO, 9.0, 2.3, 0.66, aC).x - 0.5) * 0.8 * lC;
  if (lF > 0.0) t += (objDabs(po + 3.0, nO, 22.0, 2.0, 0.66, aF).x - 0.5) * 0.45 * lF;
  return t;
}

#endif

// The fog of war: 1 visible, 0.5 remembered (fogged), 0 unrevealed.
float visAt(vec3 wp) {
  return texture(uVis, mapUV(mapOf(wp))).r;
}

vec3 fogOfWar(vec3 c, float v) {
  float fog = (1.0 - smoothstep(0.62, 0.97, v)) * uFogWar;
  vec3 g = vec3(lum(c));
  // Remembered, not seen: dimmed, half the colour gone, cooled (a memory of the land).
  vec3 mem = mix(g, c, 0.5) * vec3(0.48, 0.52, 0.62);
  return mix(c, mem, fog);
}

// Aerial perspective: haze with distance from the eye (the far edge of the view reads as far)
// and a low mist settling in the valleys.
vec3 atmosphere(vec3 c, vec3 wp) {
  float dist = length(wp - uEye);
  float f = smoothstep(uHazeRange.x, uHazeRange.y, dist) * uHaze;
  float low = (1.0 - smoothstep(-0.02, 0.32, wp.y)) * uMist;
  return mix(c, uHazeColor, clamp(f + low * 0.22, 0.0, 0.85));
}
`;

// --------------------------------------------------------------------------------------------
// Terrain

export const TERRAIN_VS = `
in vec3 aPos;
in vec3 aNormal;
out vec3 vWorld;
out vec3 vNormal;
invariant gl_Position;
void main() {
  vec3 w = aPos + vec3(uCopyX, 0.0, 0.0);
  vWorld = w;
  vNormal = aNormal;
  gl_Position = uViewProj * vec4(w, 1.0);
}`;

// The ground overlays (borders, grid, selection/hover/target frames) draw on the terrain chunk
// meshes; under water those meshes are the sea floor, so the marks sank below the water and only
// the faded ghost pass showed them, shifted down the screen from the ships and from picking (F7).
// This VS lifts a submerged vertex to the water surface (SEA = 0); land vertices are unchanged, and
// the overlay pass's polygonOffset keeps them in front of the terrain on land.
export const OVERLAY_VS = `
in vec3 aPos;
in vec3 aNormal;
out vec3 vWorld;
out vec3 vNormal;
void main() {
  vec3 w = aPos + vec3(uCopyX, 0.0, 0.0);
  w.y = max(w.y, 0.004);
  vWorld = w;
  vNormal = aNormal;
  gl_Position = uViewProj * vec4(w, 1.0);
}`;

export const TERRAIN_FS = `
in vec3 vWorld;
in vec3 vNormal;
uniform sampler2D uTerrain;
uniform sampler2D uDecal;
uniform sampler2D uOwner;
layout(location = 0) out vec4 oColor;
layout(location = 1) out vec4 oND;
layout(location = 2) out vec4 oEmit;

// Ground ids (world.js GROUND) and feature ids (FEAT).
const int G_VOID = 0, G_GRASS = 1, G_PLAINS = 2, G_DESERT = 3, G_TUNDRA = 4, G_SNOW = 5, G_MARSH = 6,
  G_BURNING = 7, G_BROKEN = 8, G_PERDITION = 9, G_SHALLOWS = 10, G_COAST = 11, G_OCEAN = 12;
const int F_NONE = 0, F_FOREST = 1, F_JUNGLE = 2, F_ANCIENT = 3, F_BURNT = 4, F_YOUNG = 5, F_ICE = 6,
  F_FLOOD = 7, F_OASIS = 8, F_SCRUB = 9, F_FLAMES = 10, F_VOLCANO = 11, F_OBSIDIAN = 12,
  F_BLIZZARD = 13, F_SOULS = 14, F_FALLOUT = 15;

ivec4 plotAt(vec2 cell) {
  int w = int(uMapSize.x);
  int h = int(uMapSize.y);
  ivec2 c = ivec2(floor(cell));
  if (uWrapX > 0.5) c.x = ((c.x % w) + w) % w;
  if (c.x < 0 || c.x >= w || c.y < 0 || c.y >= h) return ivec4(0);
  return ivec4(texelFetch(uTerrain, c, 0) * 255.0 + 0.5);
}

// A three-tone painted mass: broad, soft shapes of base and dark, the lighter tone where the
// broad shapes rise, then brush dabs at two sizes lightening or darkening the paint.
vec3 masses(vec2 m, vec3 dark, vec3 base, vec3 light, float scale) {
  float big = mfbm(m, 0.45 * scale);
  float mid = mnoise(m + 11.0, 1.6 * scale);
  vec3 c = mix(base, dark, smoothstep(0.52, 0.8, big) * 0.6);
  c = mix(c, light, smoothstep(0.38, 0.78, big) * smoothstep(0.45, 0.8, mid) * 0.5);
  float d1 = mdabs(m, 2.4 * scale) - 0.5;
  float d2 = mdabs(m + 7.3, 6.5 * scale) - 0.5;
  float k = (d1 * 0.42 + d2 * 0.22) * uStylize * uDabs;
  c = k > 0.0 ? mix(c, light, k) : mix(c, dark, -k);
  return c;
}

vec3 groundAlbedo(int g, vec2 m, float h, inout vec3 emit) {
  if (g == G_GRASS) return srgb(masses(m, vec3(0.16, 0.24, 0.17), vec3(0.27, 0.36, 0.21), vec3(0.43, 0.49, 0.28), 1.0));
  if (g == G_PLAINS) return srgb(masses(m, vec3(0.34, 0.31, 0.21), vec3(0.48, 0.42, 0.28), vec3(0.62, 0.54, 0.37), 1.0));
  if (g == G_DESERT) {
    vec3 c = masses(m, vec3(0.58, 0.46, 0.32), vec3(0.71, 0.59, 0.42), vec3(0.82, 0.72, 0.55), 0.8);
    float dune = sin((m.x * 0.8 + m.y) * 9.0 + mfbm(m, 0.8) * 9.0);
    c *= 0.92 + 0.1 * smoothstep(-0.2, 0.9, dune);
    return srgb(c);
  }
  if (g == G_TUNDRA) {
    vec3 c = masses(m, vec3(0.33, 0.35, 0.30), vec3(0.45, 0.46, 0.38), vec3(0.58, 0.57, 0.48), 1.2);
    float st = smoothstep(0.78, 0.86, mnoise(m, 7.0));
    return srgb(mix(c, vec3(0.30, 0.30, 0.31), st * 0.7));
  }
  if (g == G_SNOW) return srgb(masses(m, vec3(0.70, 0.76, 0.84), vec3(0.85, 0.88, 0.92), vec3(0.95, 0.96, 0.97), 0.7));
  if (g == G_MARSH) {
    vec3 c = masses(m, vec3(0.20, 0.24, 0.17), vec3(0.29, 0.34, 0.21), vec3(0.40, 0.42, 0.26), 1.3);
    // Shallow pools among the reeds, catching the sky; a darker muddy rim around each.
    float pn = mfbm(m + 7.0, 2.6) + 0.03;
    float rimM = smoothstep(0.56, 0.62, pn) * (1.0 - smoothstep(0.62, 0.66, pn));
    float pool = smoothstep(0.63, 0.67, pn);
    c = mix(c, c * 0.7, rimM);
    return srgb(mix(c, vec3(0.16, 0.24, 0.26), pool * 0.85));
  }
  if (g == G_BURNING || g == G_BROKEN || g == G_PERDITION) {
    // Corrupted ground, each hell terrain its own material (F3): burning sands (ash-orange dunes),
    // broken lands (grey basalt split into plates), fields of perdition (wine-red with festering
    // pools). Mid-value bases so daylight and the dabs register; a sparse smouldering crack line
    // (not iso-line rings); brightest where the corruption runs deepest; a slow pulse; anchored.
    vec3 c = vec3(0.0);
    vec3 hot = vec3(1.0, 0.4, 0.1);
    if (g == G_BURNING) {
      // Ash-orange sand with the desert dune ripple.
      c = masses(m, vec3(0.26, 0.15, 0.09), vec3(0.40, 0.25, 0.16), vec3(0.56, 0.38, 0.22), 0.9);
      float dune = sin((m.x * 0.8 + m.y) * 9.0 + mfbm(m, 0.8) * 9.0);
      c *= 0.9 + 0.12 * smoothstep(-0.2, 0.9, dune);
      hot = vec3(1.0, 0.42, 0.10);
    } else if (g == G_BROKEN) {
      // Grey basalt cut into plates by cell edges (the F_ICE idiom), dark seams between them.
      c = masses(m, vec3(0.20, 0.18, 0.18), vec3(0.30, 0.27, 0.26), vec3(0.42, 0.39, 0.37), 1.1);
      vec2 cl = mcells(m, 3.2);
      float seam = 1.0 - smoothstep(0.015, 0.06, cl.y - cl.x);
      c = mix(c, c * 0.5, seam * 0.8);
      hot = vec3(1.0, 0.36, 0.08);
    } else {
      // Fields of perdition: wine-red with dark festering pools (the marsh pool/rim idiom).
      c = masses(m, vec3(0.22, 0.10, 0.13), vec3(0.34, 0.15, 0.19), vec3(0.48, 0.24, 0.27), 1.2);
      float pn = mfbm(m + 7.0, 2.4) + 0.03;
      float rimP = smoothstep(0.56, 0.62, pn) * (1.0 - smoothstep(0.62, 0.66, pn));
      float poolP = smoothstep(0.63, 0.67, pn);
      c = mix(c, c * 0.6, rimP);
      c = mix(c, vec3(0.10, 0.03, 0.05), poolP * 0.8);
      hot = vec3(0.95, 0.18, 0.12);
    }
    // Blight: scorch masses keep the dark contrast without a uniformly low base.
    float scorch = smoothstep(0.5, 0.82, mstroke(m, 3.0, 4.0));
    c = mix(c, c * 0.6, scorch * 0.4);
    // A sparse smouldering network: one low-frequency crack line (the small iso-rings came from a
    // second high-frequency term, now dropped), live where the corruption runs deepest.
    float live = smoothstep(0.42, 0.72, mfbm(m + 3.3, 0.5));
    float r1 = 1.0 - abs(mfbm(m + 7.7, 1.2) - 0.5) * 2.0;
    float crackLine = smoothstep(0.93, 0.985, r1) * live;
    float fis = smoothstep(0.986, 0.998, r1) * live;
    c = mix(c, vec3(0.04, 0.02, 0.02), crackLine * 0.6);
    float pulse = 0.72 + 0.28 * sin(uTime * 1.1 + mnoise(m, 0.7) * 9.0);
    emit += srgb(hot) * fis * pulse * 1.1;
    c += srgb(hot) * crackLine * 0.03;
    emit += srgb(hot) * smoothstep(0.965, 0.99, mnoise(m, 14.0)) * live * 0.22;
    return srgb(c);
  }
  if (g == G_SHALLOWS || g == G_COAST) return srgb(masses(m, vec3(0.45, 0.40, 0.30), vec3(0.62, 0.55, 0.40), vec3(0.72, 0.66, 0.50), 1.0));
  if (g == G_OCEAN) return srgb(vec3(0.22, 0.26, 0.28));
  // The unexplored: dark parchment with a faint ink hatching.
  float hatch = smoothstep(0.55, 0.9, mstroke(m, 14.0, 8.0));
  vec3 c = mix(vec3(0.15, 0.12, 0.10), vec3(0.25, 0.20, 0.15), mfbm(m, 0.5));
  c *= 0.9 + 0.2 * mdabs(m, 1.5);
  return srgb(c * (1.0 - hatch * 0.22));
}

// The floor a feature lays over the ground (forest litter, ice, flood silt, ...).
vec3 featureFloor(int f, vec3 c, vec2 m, inout vec3 emit) {
  if (f == F_FOREST || f == F_YOUNG) return mix(c, srgb(mix(vec3(0.17, 0.20, 0.12), vec3(0.30, 0.25, 0.16), mnoise(m, 5.0))), 0.6);
  if (f == F_JUNGLE) return mix(c, srgb(vec3(0.12, 0.22, 0.12)), 0.6);
  if (f == F_ANCIENT) return mix(c, srgb(mix(vec3(0.09, 0.20, 0.17), vec3(0.18, 0.32, 0.24), mnoise(m, 3.0))), 0.7);
  if (f == F_BURNT) {
    float ember = smoothstep(0.86, 0.95, mnoise(m, 11.0));
    emit += srgb(vec3(0.9, 0.3, 0.05)) * ember * (0.6 + 0.4 * sin(uTime * 2.0 + m.x * 7.0));
    return mix(c, srgb(mix(vec3(0.10, 0.09, 0.08), vec3(0.24, 0.21, 0.19), mnoise(m, 4.0))), 0.75);
  }
  if (f == F_FLOOD) {
    // Fertile river land, not dry desert scrub (F13): broken patches of fresh green over the silt,
    // vivid where present (about half the tile) with sandy gaps between, and thin dark damp streaks
    // -- no pale silt wash pulling the whole tile back to khaki.
    vec3 green = srgb(mix(vec3(0.30, 0.46, 0.17), vec3(0.42, 0.54, 0.22), mnoise(m + 5.0, 4.0)));
    float grow = smoothstep(0.36, 0.56, mnoise(m, 2.6));
    vec3 col = mix(c, green, grow * 0.92);
    float streak = smoothstep(0.6, 0.82, mstroke(m, 6.0, 5.0));
    col = mix(col, srgb(vec3(0.20, 0.25, 0.14)), streak * 0.3);
    return col;
  }
  if (f == F_SCRUB) return mix(c, srgb(vec3(0.55, 0.48, 0.30)), smoothstep(0.6, 0.8, mnoise(m, 9.0)) * 0.5);
  if (f == F_ICE) {
    vec2 cl = mcells(m, 1.8);
    float crack = 1.0 - smoothstep(0.01, 0.05, cl.y - cl.x);
    return srgb(mix(mix(vec3(0.78, 0.86, 0.92), vec3(0.92, 0.95, 0.97), mnoise(m, 2.0)), vec3(0.55, 0.68, 0.78), crack * 0.7));
  }
  if (f == F_FLAMES) {
    // Burning land: charred ground with embers glowing in it (the flames themselves are props),
    // not pools of light.
    float fl = smoothstep(0.78, 0.95, mnoise(m + vec2(0.0, uTime * 0.25), 7.0)) * smoothstep(0.35, 0.7, mnoise(m - 4.0, 1.3));
    emit += srgb(vec3(1.0, 0.42, 0.1)) * fl * 0.7;
    return mix(c, srgb(vec3(0.1, 0.07, 0.06)), 0.7);
  }
  if (f == F_OBSIDIAN) return mix(c, srgb(vec3(0.07, 0.06, 0.09)), 0.85);
  if (f == F_SOULS) {
    // Tormented souls: a faint, cold shimmer in grey ground (the wisps rising are particles).
    emit += srgb(vec3(0.35, 0.7, 0.55)) * smoothstep(0.88, 0.98, mnoise(m + uTime * 0.05, 6.0)) * 0.22;
    return mix(c, srgb(vec3(0.20, 0.22, 0.24)), 0.5);
  }
  if (f == F_FALLOUT) return mix(c, srgb(vec3(0.35, 0.40, 0.18)), 0.45);
  return c;
}

// Bump from a height field given per pixel (derivative bump mapping, Mikkelsen 2010): the brush
// strokes' impasto catching the light. The field is world-anchored; only its derivatives are taken
// on screen, so it does not swim.
vec3 bump(vec3 n, vec3 p, float hgt, float k) {
  vec3 dpdx = dFdx(p), dpdy = dFdy(p);
  float dhdx = dFdx(hgt), dhdy = dFdy(hgt);
  vec3 r1 = cross(dpdy, n), r2 = cross(n, dpdx);
  float det = dot(dpdx, r1);
  vec3 grad = sign(det) * (dhdx * r1 + dhdy * r2);
  return normalize(abs(det) * n - k * grad);
}

void main() {
  vec3 n = normalize(vNormal);
  vec2 m = mapOf(vWorld);
  float h = vWorld.y;
  vec3 emit = vec3(0.0);
  // Impasto: the dabs raise the paint a little (close up only: it fades out with distance).
  float near = 1.0 - smoothstep(14.0, 30.0, length(vWorld - uEye));
  if (near > 0.0 && uStylize > 0.0) {
    float imp = (mdabs(m + 7.3, 6.5) - 0.5) * 0.6 + (mdabs(m, 2.4) - 0.5) * 0.4;
    n = bump(n, vWorld, imp * 0.012 * near * uStylize, 1.0);
  }

  // The ground of the four plots around the point, blended by bilinear weights sharpened and
  // broken up by noise (irregular, painted borders between terrains; the plot centre is pure).
  // The lookup is warped most between plots and not at all at their centres, so a plot's centre
  // always shows its own ground while the borders wander well off the plot edges (organic regions
  // instead of a board of squares).
  vec2 fc = abs(fract(m) - 0.5);
  float freeW = smoothstep(0.1, 0.42, max(fc.x, fc.y));
  vec2 warp = (vec2(mnoise(m, 0.45), mnoise(m + 17.3, 0.45)) - 0.5) * 1.3 + (vec2(mnoise(m + 3.3, 1.1), mnoise(m - 4.4, 1.1)) - 0.5) * 0.5;
  vec2 q = m + warp * freeW + (vec2(mnoise(m + 5.1, 3.2), mnoise(m - 8.7, 3.2)) - 0.5) * 0.2;
  vec2 gq = q - 0.5;
  vec2 c0 = floor(gq);
  vec2 f = fract(gq);
  ivec4 p00 = plotAt(c0), p10 = plotAt(c0 + vec2(1, 0)), p01 = plotAt(c0 + vec2(0, 1)), p11 = plotAt(c0 + vec2(1, 1));
  vec4 w = vec4((1.0 - f.x) * (1.0 - f.y), f.x * (1.0 - f.y), (1.0 - f.x) * f.y, f.x * f.y);
  // Painted edges: the weights are pushed by stroke noise, so a border between two grounds is a
  // band of interleaved brush strokes rather than a gradient.
  float brk = mdabs(m - 2.1, 4.5);
  w = pow(w, vec4(2.2 + 4.0 * brk));
  w /= dot(w, vec4(1.0));
  // Each distinct ground among the four is painted once, with the summed weight of its plots.
  int gs[4] = int[](p00.x, p10.x, p01.x, p11.x);
  float ws[4] = float[](w.x, w.y, w.z, w.w);
  // How much of the point is hell ground: its rock is basalt and it takes no snow.
  float hellW = 0.0;
  for (int k = 0; k < 4; k++) if (gs[k] == G_BURNING || gs[k] == G_BROKEN || gs[k] == G_PERDITION) hellW += ws[k];
  vec3 alb = vec3(0.0);
  for (int k = 0; k < 4; k++) {
    if (ws[k] < 0.0) continue;
    float wk = ws[k];
    for (int j = k + 1; j < 4; j++) {
      if (gs[j] == gs[k] && ws[j] >= 0.0) { wk += ws[j]; ws[j] = -1.0; }
    }
    if (wk < 0.004) continue;
    vec3 e = vec3(0.0);
    alb += groundAlbedo(gs[k], m, h, e) * wk;
    emit += e * wk;
  }
  // Features' floors, by the same weights.
  int fs[4] = int[](p00.y, p10.y, p01.y, p11.y);
  float wf[4] = float[](w.x, w.y, w.z, w.w);
  vec3 fa = vec3(0.0);
  for (int k = 0; k < 4; k++) {
    if (wf[k] < 0.0) continue;
    float wk = wf[k];
    for (int j = k + 1; j < 4; j++) {
      if (fs[j] == fs[k] && wf[j] >= 0.0) { wk += wf[j]; wf[j] = -1.0; }
    }
    vec3 e = vec3(0.0);
    fa += (fs[k] == F_NONE ? alb : featureFloor(fs[k], alb, m, e)) * wk;
    emit += e * wk;
  }
  alb = fa;
  int here = plotAt(m).z;
  bool known = here != 0;

  if (known) {
    // Beaches: the land's lowest rim by the water.
    // Polar ice keeps its own blue-white floor at the waterline instead of being painted as beach
    // sand (F16). The ice weight uses the original bilinear weights (w.*); wf[] was mutated above.
    float iceW = (fs[0] == F_ICE ? max(0.0, w.x) : 0.0) + (fs[1] == F_ICE ? max(0.0, w.y) : 0.0)
               + (fs[2] == F_ICE ? max(0.0, w.z) : 0.0) + (fs[3] == F_ICE ? max(0.0, w.w) : 0.0);
    float beach = (1.0 - smoothstep(0.02, 0.075, h)) * smoothstep(-0.05, 0.0, h);
    beach *= 1.0 - smoothstep(0.15, 0.5, iceW);
    vec3 sand = srgb(mix(vec3(0.66, 0.57, 0.40), vec3(0.78, 0.70, 0.52), mnoise(m, 6.0)));
    alb = mix(alb, sand, beach * 0.85);
    // Rock on steep ground: cliffs and the flanks of the mountains, with strata.
    float slope = 1.0 - n.y;
    float rockMask = smoothstep(0.24, 0.4, slope + (mnoise(m, 4.0) - 0.5) * 0.14);
    rockMask = max(rockMask, smoothstep(0.42, 0.62, h) * 0.85);
    if (rockMask > 0.0) {
      // Crags: creases of ridged noise across the rock faces catch the light (a bump on the
      // shading normal, world-anchored, close up mostly), finer than the terrain mesh can carry.
      float crease = 1.0 - abs(mnoise(m + 1.7, 2.2) * 2.0 - 1.0);
      float crease2 = 1.0 - abs(mnoise(m - 6.1, 5.3) * 2.0 - 1.0);
      float crag = (crease * crease * 0.7 + crease2 * crease2 * 0.3) * rockMask;
      n = bump(n, vWorld, crag * 0.1 * (0.35 + 0.65 * near), 1.0);
      float strata = 0.5 + 0.5 * sin(h * 30.0 + (m.x - m.y) * 1.3 + mnoise(m, 2.5) * 7.0);
      strata *= smoothstep(0.3, 0.7, mnoise(m + 2.0, 1.2));
      vec3 rock = mix(vec3(0.22, 0.21, 0.24), vec3(0.42, 0.40, 0.38), mfbm(m, 1.7));
      rock = mix(rock, mix(vec3(0.09, 0.07, 0.08), vec3(0.2, 0.15, 0.14), mfbm(m, 1.7)), hellW);
      rock = mix(rock, vec3(0.16, 0.15, 0.19), strata * 0.22);
      // Lit faces of the rock read warm, the shaded ones cool (painted massing of a mountain).
      rock = mix(rock, rock * vec3(1.12, 1.02, 0.9), clamp(dot(n, uSunDir), 0.0, 1.0) * 0.5);
      float rd = mdabs(m + 4.0, 7.0) - 0.5;
      rock *= 1.0 + rd * 0.35 * uStylize;
      alb = mix(alb, srgb(rock), rockMask);
    }
    // Snow on the high peaks (and their sunlit ledges).
    // Snow on the summits and caught on the gentler ledges below them, streaked by the wind.
    float sn = mnoise(m, 3.0) - 0.5 + (mdabs(m, 5.0) - 0.5) * 0.6;
    float snow = smoothstep(0.92, 1.08, h + sn * 0.22) * smoothstep(0.3, 0.75, n.y);
    snow = max(snow, smoothstep(1.15, 1.3, h + sn * 0.1));
    snow *= 1.0 - hellW;
    alb = mix(alb, srgb(vec3(0.88, 0.91, 0.95)), snow);
    // Wet ground along rivers and the shore line.
    vec4 dec = texture(uDecal, mapUV(m));
    alb *= mix(1.0, 0.72, dec.b);
    alb = mix(alb, alb * vec3(0.85, 1.0, 0.85), dec.b * 0.4);
    // Settlement ground, fields and roads (rasterised per plot, see decals.js).
    vec3 earth = srgb(mix(vec3(0.36, 0.29, 0.21), vec3(0.47, 0.39, 0.29), mnoise(m, 8.0)));
    alb = mix(alb, earth, dec.a * 0.8);
    if (dec.g > 0.01) {
      vec2 cell = floor(m * 3.0 + vec2(mnoise(m, 1.0)) * 0.6);
      float kind = hash12(cell);
      float ang = hash12(cell + 7.0) > 0.5 ? m.x : m.y;
      float furrow = 0.5 + 0.5 * sin(ang * 70.0);
      vec3 crop = kind < 0.35 ? vec3(0.66, 0.55, 0.26) : kind < 0.7 ? vec3(0.38, 0.45, 0.20) : vec3(0.42, 0.32, 0.22);
      crop *= 0.85 + 0.2 * furrow;
      alb = mix(alb, srgb(crop), dec.g * 0.85);
    }
    if (dec.r > 0.01) {
      vec3 road = srgb(mix(vec3(0.40, 0.33, 0.25), vec3(0.55, 0.47, 0.37), mnoise(m, 10.0)));
      float cob = smoothstep(0.2, 0.5, mcells(m, 26.0).x);
      road = mix(road, road * (0.8 + 0.3 * cob), dec.a);
      alb = mix(alb, road, smoothstep(0.2, 0.6, dec.r));
    }
  }

  // Painter's pigment: a broad hue/value drift that keeps large areas from reading flat.
  float pig = mnoise(m, 0.35) - 0.5;
  alb *= 1.0 + uPaint * pig * 0.35;
  alb = mix(alb, alb * vec3(1.05, 0.97, 0.9), uPaint * smoothstep(0.3, 0.8, mnoise(m + 50.0, 0.25)) * 0.6);

  float ao = 1.0;
  float sun;
  vec3 L = lightAt(n, vWorld, ao, sun);
  vec3 col = alb * L;
  col = shadowColour(col, sun);
  col += emit;

  float v = visAt(vWorld);
  // The unexplored: the map's blank, dark parchment. The frontier is a brushy edge (the smooth
  // visibility field pushed by brush dabs), with a scorched rim like the edge of burnt paper.
  float edgeN = (mdabs(m + 1.7, 3.0) - 0.5) * 0.35 + (mnoise(m, 2.0) - 0.5) * 0.15;
  float blankT = 1.0 - smoothstep(0.12, 0.4, v + edgeN);
  if (!known) blankT = max(blankT, 1.0 - smoothstep(0.08, 0.3, v + edgeN));
  if (blankT > 0.0) {
    vec3 e0 = vec3(0.0);
    vec3 blank = groundAlbedo(G_VOID, m, h, e0) * 1.5;
    float rim = smoothstep(0.0, 0.5, blankT) * (1.0 - smoothstep(0.5, 1.0, blankT));
    col = mix(col, blank, blankT);
    col *= 1.0 - rim * 0.55;
    emit *= 1.0 - blankT;
  }
  col = fogOfWar(col, v);
  col = atmosphere(col, vWorld);
  oColor = vec4(col, 0.0);
  vec3 vn = normalize((uView * vec4(n, 0.0)).xyz);
  oND = vec4(vn, -(uView * vec4(vWorld, 1.0)).z);
  oEmit = vec4(emit * smoothstep(0.5, 0.9, v), 0.0);
}`;

// --------------------------------------------------------------------------------------------
// Props (trees, buildings, units, rocks): instanced meshes in the PROP_LAYOUT

export const PROP_VS = `
in vec3 aPos;
in vec3 aNormal;
in vec3 aColor;
in vec4 aMat;
in vec4 iPosRot;
in vec4 iScale;
in vec4 iTint;
in vec4 iTeam;
uniform float uWind;
out vec3 vWorld;
out vec3 vNormal;
out vec3 vColor;
out vec3 vLocal;
out vec4 vTint;
out vec4 vTeam;
flat out vec4 vMat;
out float vVis;
// The object's own space, scaled to the map (plots) but not posed or swayed: brush marks laid in
// it stay on the surface as leaves sway and figures breathe. vObjN is the normal there.
out vec3 vObj;
out vec3 vObjN;
invariant gl_Position;

vec3 rotY(vec3 p, float a) {
  float c = cos(a), s = sin(a);
  return vec3(c * p.x + s * p.z, p.y, -s * p.x + c * p.z);
}

void main() {
  vec3 p = aPos * iScale.xyz;
  vObj = p;
  vObjN = aNormal;
  float phase = iScale.w;
  // Wind in the leaves and cloth: a slow sway and a faster flutter, stronger toward the tips.
  int bone0 = int(aMat.z + 0.5);
  if (aMat.y > 0.0 && bone0 != 8 && bone0 != 9) {
    float ph = phase * 6.2831 + iPosRot.x * 0.37 + iPosRot.z * 0.21;
    float s = sin(uTime * 1.3 + ph) * 0.7 + sin(uTime * 2.9 + ph * 1.7) * 0.3;
    p.xz += vec2(0.8, 0.5) * s * aMat.y * uWind * 0.025 * max(iScale.y, 0.5);
    p.y += sin(uTime * 3.7 + ph + aPos.x * 9.0) * aMat.y * uWind * 0.004;
  }
  // Units breathe and shift their weight; wings beat (bones, mesh.js BONE).
  int bone = int(aMat.z + 0.5);
  if (bone > 0 || iTeam.a > 0.5) {
    float t = uTime * 1.6 + phase * 6.2831;
    if (bone == 1) p.y += sin(t) * 0.004;
    // A weapon moves with the hand that holds it (the right arm).
    else if (bone == 2 || bone == 3 || bone == 7) p.y += sin(t + (bone == 2 ? 0.0 : 1.5)) * 0.004;
    // Cloth hanging from the shoulders: the hem moves most.
    else if (bone == 6) p.xz += vec2(0.0, 1.0) * sin(t * 1.7 + aPos.y * 9.0) * 0.018 * clamp((0.42 - aPos.y) * 3.0, 0.0, 1.0);
    else if (bone == 8 || bone == 9) {
      // Wings beat about their root: the sway channel weighs each vertex (0 at the root, 1 at the
      // tips) when the model sets it, else the distance from the body's axis does.
      float flap = sin(uTime * 3.2 + phase * 6.2831);
      float wgt = aMat.y > 0.0 ? aMat.y : clamp(abs(aPos.x) * 2.5, 0.0, 1.0);
      p.y += wgt * flap * 0.07 * iScale.y;
    } else if (bone == 10) p.x += sin(t * 1.3 + aPos.z * 8.0) * 0.01 * abs(aPos.z) * 4.0;
    // A mount breathes and shifts too.
    else if (bone == 11) p.y += sin(t * 0.8) * 0.0025 * clamp(aPos.y * 4.0, 0.0, 1.0);
    if (bone == 0 || bone == 1 || bone == 2 || bone == 3 || bone == 7) p.y *= 1.0 + sin(t) * 0.006;
  }
  vec3 w = rotY(p, iPosRot.w) + iPosRot.xyz + vec3(uCopyX, 0.0, 0.0);
  vWorld = w;
  vNormal = rotY(aNormal / iScale.xyz, iPosRot.w);
  vColor = aColor;
  vLocal = aPos;
  vTint = iTint;
  vTeam = iTeam;
  vMat = aMat;
  vVis = texture(uVis, (vec2(iPosRot.x, -iPosRot.z)) / uMapSize).r;
  gl_Position = uViewProj * vec4(w, 1.0);
}`;

export const PROP_FS = `
in vec3 vWorld;
in vec3 vNormal;
in vec3 vColor;
in vec3 vLocal;
in vec4 vTint;
in vec4 vTeam;
flat in vec4 vMat;
in float vVis;
in vec3 vObj;
in vec3 vObjN;
uniform float uEmissive;
layout(location = 0) out vec4 oColor;
layout(location = 1) out vec4 oND;
layout(location = 2) out vec4 oEmit;

void main() {
  int kind = int(vMat.x + 0.5);
  bool leafy = kind == 1;
  vec3 n = normalize(vNormal);
  if (!gl_FrontFacing) n = -n;
  // The brush marks' space, offset per instance so neighbours differ; its derivatives are taken
  // here, outside any branch.
  vec3 po = vObj + vTint.a * 61.0;
  vec3 dpo = fwidth(po);
  vec3 nO = normalize(vObjN);
  vec3 alb = vColor * vTint.rgb;
  if (kind == 3) alb = srgb(vTeam.rgb) * (0.38 + 1.25 * lum(vColor)) * vTint.rgb;
  // Pigment variation in the object's own space (stays on the object); the brush marks below
  // carry the finer variation, so the fine pigment steps back as they come in.
  float pg = vnoise3(vLocal * 22.0 + vTint.a * 50.0);
  float pg2 = vnoise3(vLocal * 6.0 + vTint.a * 31.0);
  alb *= 1.0 + uPaint * ((pg - 0.5) * 0.22 * (1.0 - 0.6 * uBrush) + (pg2 - 0.5) * 0.3);
  // Brushwork: leaf clusters on foliage, strokes on everything else, each mark its own value
  // (tone) and pushing the light/shade boundary (jit). Units take a lighter hand: they must read
  // at a glance, and the paint filter keeps them sharp too.
  float tone = 0.0, jit = 0.0;
  if (uBrush > 0.0 && kind != 4 && kind != 6) {
    if (leafy) {
      tone = leafMarks(po, nO, dpo).x * uBrush;
      jit = tone * 0.75;
      alb *= 1.0 + tone * 0.6;
    } else {
      tone = strokeMarks(po, nO, dpo) * uBrush * (1.0 - 0.6 * vTeam.a);
      jit = tone * 0.5 * (1.0 - vTeam.a);
      alb *= 1.0 + tone * 0.6;
    }
  }
  float ao = vMat.w;
  vec3 emit = vec3(0.0);
  vec3 col;
  if (kind == 4 || kind == 6) {
    vec3 e = kind == 6 ? srgb(vTeam.rgb) * 1.5 : alb;
    float flick = 0.85 + 0.15 * sin(uTime * 7.0 + vTint.a * 40.0 + vLocal.y * 20.0);
    emit = e * uEmissive * flick;
    col = e * 1.2;
  } else {
    float sun;
    vec3 L = lightAtJ(n, vWorld, ao, jit * uStylize, sun);
    col = alb * L;
    // Broken colour: a light mark leans warm, a dark one cool (sky in the shade) -- strongly in
    // foliage (sunlit leaves against blue-green depths), gently elsewhere.
    float bc = min(1.0, abs(tone) * 1.6) * uStylize * (leafy ? 1.0 : 0.6);
    col *= mix(vec3(1.0), tone > 0.0 ? vec3(1.07, 1.03, 0.84) : vec3(0.88, 0.97, 1.12), bc);
    if (leafy) {
      // Leaves: light through the canopy from behind, and a warm, sunlit crown.
      float back = max(0.0, -dot(n, uSunDir)) * 0.35;
      col += alb * uSunColor * back * 0.6;
      col *= mix(1.0, 0.82 + 0.3 * smoothstep(0.0, 1.0, sun), uStylize);
    } else if (kind == 2) {
      vec3 V = normalize(uEye - vWorld);
      vec3 H = normalize(V + uSunDir);
      float sp = pow(max(dot(n, H), 0.0), 24.0) * sun;
      col += uSunColor * sp * 0.45 + uSkyColor * 0.15 * pow(1.0 - max(dot(n, V), 0.0), 2.0);
    } else if (kind == 5) {
      col += alb * uSunColor * 0.12 * max(0.0, dot(n, uSunDir) + 0.4);
    }
    col = shadowColour(col, sun);
    // A rim of sky light lifts silhouettes off the ground (units most: alpha protect). It is light
    // on the surface, so it follows the surface's brightness: a dark cloak or a wing membrane seen
    // edge-on catches a little of it, not a blue sheen; the back of a thin card less still.
    vec3 V = normalize(uEye - vWorld);
    float rim = pow(1.0 - max(dot(n, V), 0.0), 3.0) * uRim * (0.5 + vTeam.a);
    rim *= mix(0.35, 1.0, clamp(lum(alb) * 2.5, 0.0, 1.0)) * (gl_FrontFacing ? 1.0 : 0.5);
    // Leaves take little of it: a sky rim round every crown models it as a ball, the thing the
    // painted crown must not be.
    if (leafy) rim *= mix(1.0, 0.3, uBrush);
    col += uSkyColor * rim * 0.6 * ao;
    // Gouache: the value is laid in flat steps (foliage half a stop apart), each mark shifting
    // where the steps fall, so a crown reads as a few values of overlapping marks, not a shaded
    // ball -- the baked colour ramp, the ambient and the occlusion gradients are all flattened
    // alike, while the hues (warm lights, cool shade, each mass's own) stay broken. Other props
    // get a gentler version (flat-painted planes); units none, so they keep their modelling.
    float amt = (leafy ? 0.85 : 0.55 * (1.0 - vTeam.a)) * uBrush * uStylize;
    if (amt > 0.0) {
      float S = leafy ? 2.0 : 1.5;
      float Lc = max(lum(col), 1e-5);
      float q = (log2(Lc) + tone * 1.2) * S + 32.0;
      float qf = floor(q) + smoothstep(0.3, 0.7, fract(q));
      col *= mix(1.0, exp2((qf - 32.0) / S) / Lc, amt);
    }
  }
  col = fogOfWar(col, vVis);
  col = atmosphere(col, vWorld);
  // Alpha is the paint filter's keep mask: units stay sharp (1); foliage keeps part of its own
  // leaf marks (they are the paint there, world-anchored, and the screen-space filter would smear
  // them back into soft balls).
  oColor = vec4(col, max(vTeam.a, leafy ? 0.45 * uBrush : 0.0));
  vec3 vn = normalize((uView * vec4(n, 0.0)).xyz);
  oND = vec4(vn, -(uView * vec4(vWorld, 1.0)).z);
  // Leaves write their object id negated: the ink pass (FINISH_FS) merges one crown's lobes into
  // a single mass by it and keeps crown against crown in a wood a light line.
  oEmit = vec4(emit * smoothstep(0.5, 0.9, vVis), leafy ? -max(vTint.a, 0.003) : vTint.a);
}`;


// --------------------------------------------------------------------------------------------
// Shadows (depth only): the terrain and props through their own vertex transforms

export const SHADOW_TERRAIN_VS = `
in vec3 aPos;
invariant gl_Position;
void main() { gl_Position = uViewProj * vec4(aPos + vec3(uCopyX, 0.0, 0.0), 1.0); }`;

export const SHADOW_FS = `
out vec4 o;
void main() { o = vec4(1.0); }`;

// --------------------------------------------------------------------------------------------
// Water: the sea (a plane at sea level) and the rivers (ribbons along their beds)

export const WATER_VS = `
in vec3 aPos;
in vec4 aExtra;
out vec3 vWorld;
out vec4 vExtra;
uniform float uRiverLift;
invariant gl_Position;
void main() {
  // uRiverLift raises the river ribbon at the far zoom, where the coarse (LOW_RES) terrain does not
  // hold the carved channel and would otherwise bury it (F18); 0 for the sea and at near zoom.
  vec3 w = aPos + vec3(uCopyX, uRiverLift, 0.0);
  vWorld = w;
  vExtra = aExtra;
  gl_Position = uViewProj * vec4(w, 1.0);
}`;

export const WATER_FS = `
in vec3 vWorld;
in vec4 vExtra;
uniform sampler2D uHeight;
uniform float uRiver;
layout(location = 0) out vec4 oColor;
layout(location = 1) out vec4 oND;
layout(location = 2) out vec4 oEmit;

void main() {
  vec2 m = mapOf(vWorld);
  // The sea floor's depth, read through a warp and averaged over a small footprint, so the
  // colour bands of the shelf follow soft, irregular contours instead of the plot grid.
  // Only samples under water count: around a small lake most of the footprint is dry land, which
  // read as shallows everywhere (a pale salt pan instead of a lake).
  vec2 wq = m + (vec2(mnoise(m, 0.8), mnoise(m + 9.0, 0.8)) - 0.5) * 0.7;
  float near = texture(uHeight, mapUV(m)).r;
  float gsum = 0.0, gw = 0.0;
  for (int k = 0; k < 4; k++) {
    vec2 o = vec2(k == 1 ? 0.35 : k == 3 ? -0.35 : 0.0, k == 0 ? 0.35 : k == 2 ? -0.35 : 0.0);
    float g = texture(uHeight, mapUV(wq + o)).r;
    // Fade each tap's weight continuously across the waterline (F20): the hard step() made a tap
    // snap in or out as it crossed the shore, so depth jumped into hard-edged polygonal facets in
    // lakes and on the shelf. The fallback to the centre sample blends by the missing weight too.
    float wet = smoothstep(0.0, 0.03, vWorld.y - g);
    gsum += g * wet;
    gw += wet;
  }
  float rest = max(0.0, 1.0 - gw);
  float ground = (gsum + near * rest) / (gw + rest);
  float depth = max(0.0, vWorld.y - mix(ground, near, smoothstep(0.08, 0.0, vWorld.y - near)));
  float v = visAt(vWorld);
  if (uRiver < 0.5 && v < 0.02) discard;
  // Waves: two drifting layers of noise give the surface normal (gentle, painted ripples).
  vec2 flow = uRiver > 0.5 ? vec2(0.0, -vExtra.y * 0.0) : vec2(0.0);
  float t = uTime;
  vec2 m1 = m + vec2(t * 0.03, t * 0.02);
  vec2 m2 = m + vec2(-t * 0.025, t * 0.035);
  float e = 0.03;
  float hA = mnoise(m1, 3.0) + mnoise(m2, 5.0) * 0.6;
  float hB = mnoise(m1 + vec2(e, 0.0), 3.0) + mnoise(m2 + vec2(e, 0.0), 5.0) * 0.6;
  float hC = mnoise(m1 + vec2(0.0, e), 3.0) + mnoise(m2 + vec2(0.0, e), 5.0) * 0.6;
  vec3 n = normalize(vec3(-(hB - hA) * 0.5, 1.0, (hC - hA) * 0.5));
  float along = 0.0;
  if (uRiver > 0.5) {
    // A river: streaks running downstream, foam at its banks.
    along = vExtra.y;
    float streak = pnoise(vec2(vExtra.x * 3.0, along * 2.5 - t * 1.2), vec2(4096.0));
    n = normalize(n + vec3(0.0, 0.0, (streak - 0.5) * 0.3));
    depth = 0.11 * (1.0 - vExtra.x * vExtra.x);
  }
  // Colour by depth: clear teal over the shelf, deepening to indigo over the ocean.
  vec3 shallow = srgb(vec3(0.20, 0.42, 0.40));
  vec3 mid = srgb(vec3(0.10, 0.27, 0.34));
  vec3 deep = srgb(vec3(0.07, 0.16, 0.27));
  vec3 c = mix(shallow, mid, smoothstep(0.0, 0.1, depth));
  c = mix(c, deep, smoothstep(0.1, 0.38, depth));
  // Painted variation of the water's body.
  c *= 0.9 + 0.2 * mfbm(m + t * 0.01, 0.5);
  float sun;
  vec3 L = lightAt(n, vWorld, 1.0, sun);
  vec3 col = c * (L * 0.85 + 0.15);
  vec3 V = normalize(uEye - vWorld);
  float fres = pow(1.0 - max(dot(n, V), 0.0), 4.0);
  col = mix(col, uSkyColor * 0.55, fres * 0.35);
  vec3 H = normalize(V + uSunDir);
  float glint = pow(max(dot(n, H), 0.0), 120.0) * sun;
  col += uSunColor * glint * 0.7;
  // Foam: the surf line and a second, slower line offshore.
  float shore = 1.0 - smoothstep(0.0, 0.035 + 0.02 * mnoise(m, 6.0), depth);
  float wave = smoothstep(0.75, 1.0, sin(depth * 70.0 - t * 1.4 + mnoise(m, 2.0) * 6.0)) * (1.0 - smoothstep(0.02, 0.09, depth));
  float foam = clamp(shore * 0.9 + wave * 0.45, 0.0, 1.0);
  // A river: no surf; light flecks drifting downstream in the current.
  if (uRiver > 0.5) foam = smoothstep(0.8, 0.95, pnoise(vec2(vExtra.x * 5.0, along * 7.0 - t * 2.2), vec2(4096.0))) * 0.3 * (1.0 - abs(vExtra.x));
  col = mix(col, srgb(vec3(0.86, 0.85, 0.78)) * (0.6 + 0.4 * sun), foam * 0.85);
  col = fogOfWar(col, v);
  col = atmosphere(col, vWorld);
  oColor = vec4(col, 0.0);
  vec3 vn = normalize((uView * vec4(0.0, 1.0, 0.0, 0.0)).xyz);
  oND = vec4(vn, -(uView * vec4(vWorld, 1.0)).z);
  oEmit = vec4(0.0);
}`;

// --------------------------------------------------------------------------------------------
// Post passes (full-screen triangle, gl.js FULLSCREEN_VS)

/** Scalable ambient obscurance from view normals and linear depth (half resolution). */
export const SSAO_FS = `
in vec2 vUv;
uniform sampler2D uND;
uniform vec2 uTan; // tan(fov/2) * aspect, tan(fov/2)
uniform float uRadius;
uniform vec2 uTexel;
out vec4 o;
vec3 viewPos(vec2 uv, float d) { return vec3((uv * 2.0 - 1.0) * uTan * d, -d); }
void main() {
  vec4 nd = texture(uND, vUv);
  float d = nd.w;
  if (d <= 0.0) { o = vec4(1.0); return; }
  vec3 n = normalize(nd.xyz);
  vec3 P = viewPos(vUv, d);
  float rPix = uRadius / (d * uTan.y) * 0.5; // radius in uv units (vertical)
  float a0 = hash12(gl_FragCoord.xy) * 6.2831;
  float occ = 0.0;
  const int N = 12;
  for (int i = 0; i < N; i++) {
    float t = (float(i) + 0.5) / float(N);
    float ang = a0 + float(i) * 2.399963;
    vec2 off = vec2(cos(ang), sin(ang)) * rPix * t * vec2(uTan.y / uTan.x, 1.0);
    vec2 uv = vUv + off;
    float sd = texture(uND, uv).w;
    if (sd <= 0.0) continue;
    vec3 S = viewPos(uv, sd);
    vec3 v = S - P;
    float vv = dot(v, v);
    occ += max(0.0, dot(v, n) - 0.004 * d) / (vv + 0.002) * (1.0 - smoothstep(uRadius * uRadius, uRadius * uRadius * 4.0, vv));
  }
  float ao = max(0.0, 1.0 - 2.2 * occ * uRadius / float(N) * 0.5);
  o = vec4(pow(ao, 1.4), d, 0.0, 1.0);
}`;

/** Depth-aware separable blur of the occlusion. */
export const AO_BLUR_FS = `
in vec2 vUv;
uniform sampler2D uSrc;
uniform vec2 uDir;
out vec4 o;
void main() {
  vec4 c = texture(uSrc, vUv);
  float s = c.x, ws = 1.0;
  for (int i = -4; i <= 4; i++) {
    if (i == 0) continue;
    vec4 t = texture(uSrc, vUv + uDir * float(i));
    float w = exp(-float(i * i) / 8.0) * (1.0 - smoothstep(0.0, 0.03 * c.y, abs(t.y - c.y)));
    s += t.x * w;
    ws += w;
  }
  o = vec4(s / ws, c.y, 0.0, 1.0);
}`;

/** Separable gaussian (glow, structure tensor smoothing). */
export const BLUR_FS = `
in vec2 vUv;
uniform sampler2D uSrc;
uniform vec2 uDir;
uniform float uSigma;
out vec4 o;
void main() {
  vec4 s = vec4(0.0);
  float ws = 0.0;
  int r = int(ceil(uSigma * 2.5));
  for (int i = -12; i <= 12; i++) {
    if (abs(i) > r) continue;
    float w = exp(-float(i * i) / (2.0 * uSigma * uSigma));
    s += texture(uSrc, vUv + uDir * float(i)) * w;
    ws += w;
  }
  o = s / ws;
}`;

/** The scene's light into display colour: occlusion, glow, exposure, filmic tone curve. Alpha
 *  carries the keep-sharp mask. */
export const COMPOSITE_FS = `
in vec2 vUv;
uniform sampler2D uColor;
uniform sampler2D uAO;
uniform sampler2D uGlow;
uniform float uAOAmount;
uniform float uGlowAmount;
uniform float uExposure;
out vec4 o;
vec3 filmic(vec3 x) {
  // ACES fitted (Narkowicz), gentle toe.
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}
void main() {
  vec4 c = texture(uColor, vUv);
  float ao = mix(1.0, texture(uAO, vUv).x, uAOAmount);
  vec3 col = c.rgb * ao;
  col += texture(uGlow, vUv).rgb * uGlowAmount;
  col = filmic(col * uExposure);
  o = vec4(pow(col, vec3(1.0 / 2.2)), c.a);
}`;

/** Structure tensor of the image (for the anisotropic Kuwahara filter). */
export const TENSOR_FS = `
in vec2 vUv;
uniform sampler2D uSrc;
uniform vec2 uTexel;
out vec4 o;
void main() {
  vec2 d = uTexel;
  vec3 tl = texture(uSrc, vUv + vec2(-d.x, d.y)).rgb, t = texture(uSrc, vUv + vec2(0.0, d.y)).rgb, tr = texture(uSrc, vUv + d).rgb;
  vec3 l = texture(uSrc, vUv - vec2(d.x, 0.0)).rgb, r = texture(uSrc, vUv + vec2(d.x, 0.0)).rgb;
  vec3 bl = texture(uSrc, vUv - d).rgb, b = texture(uSrc, vUv - vec2(0.0, d.y)).rgb, br = texture(uSrc, vUv + vec2(d.x, -d.y)).rgb;
  vec3 gx = (tr + 2.0 * r + br - tl - 2.0 * l - bl) / 4.0;
  vec3 gy = (tl + 2.0 * t + tr - bl - 2.0 * b - br) / 4.0;
  o = vec4(dot(gx, gx), dot(gx, gy), dot(gy, gy), 1.0);
}`;

/** The anisotropic Kuwahara filter (Kyprianidis, Kang & Döllner 2009, with polynomial sector
 *  weights): flat, oriented strokes of colour following the image's structure. 'keep' (alpha of
 *  the source) restores the original where it must stay sharp (units). */
export const KUWAHARA_FS = `
in vec2 vUv;
uniform sampler2D uSrc;
uniform sampler2D uTensor;
uniform float uRadius;
uniform float uSharp;
uniform float uAniso;
uniform float uStrength;
out vec4 o;
void main() {
  vec2 size = vec2(textureSize(uSrc, 0));
  vec4 src = texture(uSrc, vUv);
  vec4 tt = texture(uTensor, vUv);
  float E = tt.x, F = tt.y, G = tt.z;
  float D = sqrt(max(0.0, (E - G) * (E - G) + 4.0 * F * F));
  float l1 = 0.5 * (E + G + D);
  float l2 = 0.5 * (E + G - D);
  vec2 v = vec2(l1 - E, -F);
  vec2 dir = length(v) > 1e-8 ? normalize(v) : vec2(0.0, 1.0);
  float phi = -atan(dir.y, dir.x);
  float A = (l1 + l2 > 1e-8) ? (l1 - l2) / (l1 + l2) : 0.0;
  float a = uRadius * clamp((uAniso + A) / uAniso, 0.1, 2.0);
  float b = uRadius * clamp(uAniso / (uAniso + A), 0.1, 2.0);
  float cp = cos(phi), sp = sin(phi);
  mat2 SR = mat2(0.5 / a, 0.0, 0.0, 0.5 / b) * mat2(cp, -sp, sp, cp);
  int mx = int(sqrt(a * a * cp * cp + b * b * sp * sp));
  int my = int(sqrt(a * a * sp * sp + b * b * cp * cp));
  float zeta = 2.0 / uRadius;
  float zc = 0.39269908; // pi / 8
  float sz = sin(zc);
  float eta = (zeta + cos(zc)) / (sz * sz);
  vec4 M[8];
  vec3 Sq[8];
  for (int k = 0; k < 8; k++) { M[k] = vec4(0.0); Sq[k] = vec3(0.0); }
  for (int j = -8; j <= 8; j++) {
    if (j < -my || j > my) continue;
    for (int i = -8; i <= 8; i++) {
      if (i < -mx || i > mx) continue;
      vec2 q = SR * vec2(float(i), float(j));
      if (dot(q, q) > 0.25) continue;
      vec3 c = clamp(texture(uSrc, vUv + vec2(float(i), float(j)) / size).rgb, 0.0, 1.0);
      float w[8];
      float sum = 0.0;
      float vxx = zeta - eta * q.x * q.x;
      float vyy = zeta - eta * q.y * q.y;
      float z;
      z = max(0.0, q.y + vxx); w[0] = z * z; sum += w[0];
      z = max(0.0, -q.x + vyy); w[2] = z * z; sum += w[2];
      z = max(0.0, -q.y + vxx); w[4] = z * z; sum += w[4];
      z = max(0.0, q.x + vyy); w[6] = z * z; sum += w[6];
      vec2 r = 0.70710678 * vec2(q.x - q.y, q.x + q.y);
      vxx = zeta - eta * r.x * r.x;
      vyy = zeta - eta * r.y * r.y;
      z = max(0.0, r.y + vxx); w[1] = z * z; sum += w[1];
      z = max(0.0, -r.x + vyy); w[3] = z * z; sum += w[3];
      z = max(0.0, -r.y + vxx); w[5] = z * z; sum += w[5];
      z = max(0.0, r.x + vyy); w[7] = z * z; sum += w[7];
      float g = exp(-3.125 * dot(q, q)) / max(sum, 1e-6);
      for (int k = 0; k < 8; k++) {
        float wk = w[k] * g;
        M[k] += vec4(c * wk, wk);
        Sq[k] += c * c * wk;
      }
    }
  }
  vec4 acc = vec4(0.0);
  for (int k = 0; k < 8; k++) {
    if (M[k].w <= 0.0) continue;
    vec3 mean = M[k].rgb / M[k].w;
    vec3 var = abs(Sq[k] / M[k].w - mean * mean);
    float s2 = var.r + var.g + var.b;
    float wgt = 1.0 / (1.0 + pow(255.0 * s2, 0.5 * uSharp));
    acc += vec4(mean * wgt, wgt);
  }
  vec3 painted = acc.w > 0.0 ? acc.rgb / acc.w : src.rgb;
  float keep = src.a;
  o = vec4(mix(src.rgb, painted, uStrength * (1.0 - keep * 0.85)), src.a);
}`;

/** Ink outlines (depth, normal and object-id edges, fading with distance), then the grade:
 *  colour balance, saturation, a world-anchored paper grain and a soft vignette. */
export const FINISH_FS = `
in vec2 vUv;
uniform sampler2D uSrc;
uniform sampler2D uND;
uniform sampler2D uEmitId;
uniform vec2 uTexel;
uniform float uOutline;
uniform float uOutlineWidth;
uniform vec3 uInk;
uniform float uGrade;
uniform float uSaturation;
uniform float uContrast;
uniform vec3 uShadowGrade;
uniform vec3 uLightGrade;
uniform float uGrain;
uniform float uVignette;
uniform float uInkLost;
uniform float uInkColour;
uniform vec2 uTan;
uniform mat4 uInvView;
out vec4 o;

float edgeAt(vec2 uv, vec4 c, float cid) {
  vec2 t = uTexel * uOutlineWidth;
  float e = 0.0;
  vec2 offs[4] = vec2[](vec2(t.x, 0.0), vec2(-t.x, 0.0), vec2(0.0, t.y), vec2(0.0, -t.y));
  for (int i = 0; i < 4; i++) {
    vec4 s = texture(uND, uv + offs[i]);
    float sid = texture(uEmitId, uv + offs[i]).a;
    if (s.w <= 0.0) continue;
    // Depth: only the far side of a jump draws (a single line on the near object's silhouette).
    float dd = (s.w - c.w) / c.w;
    float de = smoothstep(0.012, 0.03, dd);
    // Leaves against leaves (PROP_FS writes leaves' ids negated): one crown's lobes merge into a
    // single painted mass -- no line where one lobe overlaps another -- and in a wood one crown
    // against the next is a light line, so the canopy reads as a mass, as a painter masses it.
    if (cid < 0.0 && sid < 0.0) de *= abs(sid - cid) < 0.002 ? 0.0 : 0.3;
    // A crown against the ground: a lighter line too -- its leafy rim reads by value, and inking
    // every leaf cluster along it would draw a dotted fringe.
    else if (cid < 0.0) de *= 0.6;
    // Creases and object boundaries on props (not on the ground's gentle folds).
    float ne = (cid > 0.0) ? smoothstep(0.55, 0.85, 1.0 - dot(normalize(s.xyz), normalize(c.xyz))) * 0.6 : 0.0;
    float ie = (cid > 0.0 && abs(sid - cid) > 0.002 && s.w >= c.w - 0.02) ? 0.8 : 0.0;
    // A trunk or limb where it runs into its own crown: a light touch, not a hard line.
    if (cid > 0.0 && sid < 0.0 && abs(sid + cid) < 0.002) ie *= 0.3;
    e = max(e, max(de, max(ne, ie)));
  }
  return e;
}

void main() {
  vec4 src = texture(uSrc, vUv);
  vec3 col = src.rgb;
  vec4 nd = texture(uND, vUv);
  float cid = texture(uEmitId, vUv).a;
  if (uOutline > 0.0 && nd.w > 0.0) {
    float e = edgeAt(vUv, nd, cid);
    float fade = 1.0 - smoothstep(30.0, 70.0, nd.w);
    // Lost and found: a painted contour's weight wanders along it (world-anchored, so it does not
    // crawl as the camera moves) and lightens where the paint is light -- an edge in the sun is
    // half lost, one in the shade found. Units keep their whole line (src.a, the keep mask), so a
    // figure always reads; foliage (negative id) only borrows the keep mask for the paint filter.
    float keep = cid < 0.0 ? 0.0 : clamp(src.a, 0.0, 1.0);
    vec3 wpI = (uInvView * vec4((vUv * 2.0 - 1.0) * uTan * nd.w, -nd.w, 1.0)).xyz;
    float wob = pnoise(vec2(wpI.x * 7.0 + wpI.y * 4.0, wpI.z * 7.0 - wpI.y * 3.0), vec2(4096.0));
    // Foliage loses the most; buildings, rock and the land's own edges keep a firmer drawing.
    float lost = mix(cid < 0.0 ? 0.35 : 0.62, 1.05, smoothstep(0.28, 0.72, wob)) * mix(1.0, cid < 0.0 ? 0.6 : 0.78, smoothstep(0.5, 0.85, lum(col)));
    e *= mix(1.0, lost, uInkLost * (1.0 - keep));
    // A coloured line: a deep version of the local colour rather than one uniform sepia.
    vec3 ink = mix(uInk * (0.35 + 0.4 * col), col * col * 0.5 + uInk * 0.12, uInkColour * (1.0 - keep));
    col = mix(col, ink, e * uOutline * fade);
  }
  if (uGrade > 0.0) {
    float L = dot(col, vec3(0.299, 0.587, 0.114));
    vec3 g = col;
    // Split toning: cool, violet shadows; warm light.
    g = mix(g, g * uShadowGrade + uShadowGrade * 0.02, (1.0 - smoothstep(0.0, 0.45, L)) * 0.35);
    g = mix(g, g * uLightGrade, smoothstep(0.45, 1.0, L) * 0.4);
    float L2 = dot(g, vec3(0.299, 0.587, 0.114));
    g = mix(vec3(L2), g, uSaturation);
    g = (g - 0.5) * uContrast + 0.5;
    // Paper grain fixed to the world: from the depth, the point on the ground under the pixel.
    if (nd.w > 0.0) {
      vec3 vp = vec3((vUv * 2.0 - 1.0) * uTan * nd.w, -nd.w);
      vec3 wp = (uInvView * vec4(vp, 1.0)).xyz;
      float gr = hash12(floor(vec2(wp.x, wp.z) * 90.0)) - 0.5;
      float fib = pnoise(vec2(wp.x * 20.0 + wp.z * 7.0, wp.z * 60.0 - wp.x * 11.0), vec2(4096.0)) - 0.5;
      g *= 1.0 + (gr * 0.6 + fib) * uGrain;
    }
    vec2 vc = vUv - 0.5;
    g *= 1.0 - uVignette * smoothstep(0.35, 0.85, length(vc * vec2(1.1, 1.0)));
    col = mix(col, g, uGrade);
  }
  o = vec4(clamp(col, 0.0, 1.0), 1.0);
}`;

// --------------------------------------------------------------------------------------------
// Overlays: drawn after the post passes, over the finished frame, depth-tested against the scene

/** The ground overlay: territory borders, the political wash, the grid, hovered / selected /
 *  target plots and the path, re-drawn on the terrain mesh (depth equal) or ghosted through what
 *  stands in front (uGhost). */
export const OVERLAY_FS = `
in vec3 vWorld;
in vec3 vNormal;
uniform sampler2D uOwner;
uniform sampler2D uMarks;
uniform vec3 uOwnerColors[64];
uniform float uBorders;
uniform float uPolitical;
uniform float uGrid;
uniform float uGhost;
uniform vec4 uHover;
uniform vec4 uSel;
out vec4 o;

int ownerAt(vec2 cell) {
  int w = int(uMapSize.x);
  int h = int(uMapSize.y);
  ivec2 c = ivec2(floor(cell));
  if (uWrapX > 0.5) c.x = ((c.x % w) + w) % w;
  if (c.x < 0 || c.x >= w || c.y < 0 || c.y >= h) return 0;
  return int(texelFetch(uOwner, c, 0).r * 255.0 + 0.5);
}

vec4 marksAt(vec2 cell) {
  int w = int(uMapSize.x);
  int h = int(uMapSize.y);
  ivec2 c = ivec2(floor(cell));
  if (uWrapX > 0.5) c.x = ((c.x % w) + w) % w;
  if (c.x < 0 || c.x >= w || c.y < 0 || c.y >= h) return vec4(0.0);
  return texelFetch(uMarks, c, 0);
}

// Distance from point f (in the cell, 0..1) to the cell's edge toward neighbour (dx, dy).
float edgeDist(vec2 f, int dx, int dy) {
  if (dx == 1) return 1.0 - f.x;
  if (dx == -1) return f.x;
  if (dy == 1) return 1.0 - f.y;
  return f.y;
}

void main() {
  vec2 m = mapOf(vWorld);
  vec2 cell = floor(m);
  vec2 f = m - cell;
  // The last terrain chunk is a full CHUNK tall, so the overlay mesh reaches past the map's north
  // edge; those off-map fragments must draw nothing (F15). ownerAt/marksAt already return 0 off the
  // map, but the grid branch has no bounds guard, so without this the grid bleeds over the parchment
  // band when the top row is revealed (uVis clamps to it). Guard once here, mirroring ownerAt.
  int mw = int(uMapSize.x);
  int mh = int(uMapSize.y);
  ivec2 ci = ivec2(floor(cell));
  if (uWrapX > 0.5) ci.x = ((ci.x % mw) + mw) % mw;
  if (ci.x < 0 || ci.x >= mw || ci.y < 0 || ci.y >= mh) discard;
  vec4 acc = vec4(0.0);
  float v = visAt(vWorld);
  if (v < 0.2) discard;
  int me = ownerAt(cell);
  // Borders: a crisp ink line on the edge where the owner changes, and a soft band of the
  // owner's colour inside it.
  if (uBorders > 0.0 && me > 0) {
    vec3 oc = uOwnerColors[me];
    float dmin = 1.0;
    ivec2 dirs[4] = ivec2[](ivec2(1, 0), ivec2(-1, 0), ivec2(0, 1), ivec2(0, -1));
    for (int i = 0; i < 4; i++) {
      if (ownerAt(cell + vec2(dirs[i])) != me) dmin = min(dmin, edgeDist(f, dirs[i].x, dirs[i].y));
    }
    // Diagonal neighbours close the corners.
    ivec2 diag[4] = ivec2[](ivec2(1, 1), ivec2(-1, 1), ivec2(1, -1), ivec2(-1, -1));
    for (int i = 0; i < 4; i++) {
      if (ownerAt(cell + vec2(diag[i])) != me) dmin = min(dmin, max(edgeDist(f, diag[i].x, 0), edgeDist(f, 0, diag[i].y)));
    }
    float wob = (mnoise(m, 6.0) - 0.5) * 0.02;
    float line = 1.0 - smoothstep(0.035, 0.06, dmin + wob);
    float band = (1.0 - smoothstep(0.05, 0.24, dmin + wob)) * 0.55;
    acc = mix(acc, vec4(oc, 1.0), band * uBorders);
    acc = mix(acc, vec4(oc * 0.55 + 0.45 * vec3(1.0, 0.95, 0.85) * lum(oc), 1.0), line * uBorders);
  }
  if (uPolitical > 0.0 && me > 0) acc = mix(acc, vec4(uOwnerColors[me], 1.0), 0.42 * uPolitical);
  // The grid: faint lines on the plot edges (the logical board under the painted world).
  if (uGrid > 0.0) {
    vec2 d = min(f, 1.0 - f);
    float g = 1.0 - smoothstep(0.0, 0.018, min(d.x, d.y));
    acc = mix(acc, vec4(0.06, 0.05, 0.04, 1.0), g * uGrid * 0.5);
  }
  // Marks (marks texture: r targets, g path, b reachable this turn).
  vec4 mk = marksAt(cell);
  vec2 dd = min(f, 1.0 - f);
  float edge = min(dd.x, dd.y);
  if (mk.r > 0.0) {
    float dash = step(0.5, fract((f.x + f.y) * 6.0));
    float t = 1.0 - smoothstep(0.03, 0.07, edge);
    acc = mix(acc, vec4(0.31, 0.89, 1.0, 1.0), t * (0.55 + 0.45 * dash));
    acc = mix(acc, vec4(0.31, 0.89, 1.0, 1.0), 0.12);
  }
  if (mk.g > 0.0) acc = mix(acc, vec4(1.0, 0.92, 0.55, 1.0), 0.22 * mk.g);
  // Hovered and selected plots: a rounded square frame.
  for (int k = 0; k < 2; k++) {
    vec4 p = k == 0 ? uHover : uSel;
    if (p.w <= 0.0) continue;
    vec2 cp = p.xy;
    if (uWrapX > 0.5) cp.x += floor((cell.x - cp.x) / uMapSize.x + 0.5) * uMapSize.x;
    if (cell != cp) continue;
    vec2 q = abs(f - 0.5);
    float r = length(max(q - 0.36, 0.0)) + min(max(q.x, q.y) - 0.36, 0.0) - 0.08;
    float ring = 1.0 - smoothstep(0.0, 0.03, abs(r + 0.015) - (k == 0 ? 0.008 : 0.016));
    vec3 c = k == 0 ? vec3(1.0, 1.0, 0.95) : vec3(1.0, 0.86, 0.3);
    acc = mix(acc, vec4(c, 1.0), ring * (k == 0 ? 0.7 : 1.0));
  }
  acc.a *= mix(1.0, 0.35, uGhost) * smoothstep(0.2, 0.5, v);
  if (acc.a < 0.003) discard;
  o = vec4(acc.rgb, acc.a);
}`;

/** Units behind scenery (a forest canopy, a house, a hill): their silhouettes in the owner's
 *  colour, drawn where the depth test fails (GREATER), so a unit is never lost from sight. */
export const GHOST_FS = `
in vec3 vWorld;
in vec3 vNormal;
in vec4 vTeam;
in vec4 vTint;
flat in vec4 vMat;
uniform sampler2D uND;
uniform sampler2D uEmitId;
out vec4 o;
void main() {
  // Only where something else stands in front: not the figure's own far side (its object id is
  // the frontmost one here), nor a surface barely nearer.
  ivec2 px = ivec2(gl_FragCoord.xy);
  float frontId = texelFetch(uEmitId, px, 0).a;
  float frontDepth = texelFetch(uND, px, 0).w;
  float depth = -(uView * vec4(vWorld, 1.0)).z;
  if (abs(frontId - vTint.a) < 1.5e-3 || frontDepth > depth - 0.03) discard;
  vec3 n = normalize(vNormal);
  if (!gl_FrontFacing) n = -n;
  vec3 V = normalize(uEye - vWorld);
  float rim = pow(1.0 - abs(dot(n, V)), 2.0);
  vec3 c = mix(vTeam.rgb * 0.75, vec3(1.0, 0.97, 0.9), rim * 0.6);
  // A fine diagonal hatching: it reads as "seen through", not as a solid painted figure.
  float hatch = step(0.5, fract((gl_FragCoord.x + gl_FragCoord.y) * 0.25));
  o = vec4(c, (0.38 + 0.4 * rim) * (0.75 + 0.25 * hatch));
}`;

/** Flat overlay meshes laid on the ground (team rings, path lines): colour from the vertex,
 *  ghosted through occluders by the second pass. */
export const MARKER_VS = `
in vec3 aPos;
in vec4 aExtra;
out vec4 vCol;
out vec3 vWorld;
void main() {
  vec3 w = aPos + vec3(uCopyX, 0.0, 0.0);
  vWorld = w;
  vCol = aExtra;
  gl_Position = uViewProj * vec4(w, 1.0);
}`;

export const MARKER_FS = `
in vec4 vCol;
in vec3 vWorld;
uniform float uGhost;
out vec4 o;
void main() {
  o = vec4(vCol.rgb, vCol.a * mix(1.0, 0.45, uGhost));
}`;

/** Particles (embers, mana motes, fireflies, smoke): camera-facing soft points. */
export const PARTICLE_VS = `
in vec3 aPos;
in vec4 aExtra; // r, g, b, size
in vec3 aNormal; // velocity-ish: x drift phase, y rise speed, z life offset
out vec4 vCol;
out float vLife;
uniform float uPx; // pixels per world unit at the target distance
void main() {
  float life = fract(uTime * aNormal.y + aNormal.z);
  // Smoke (negative size): rises higher, leans with the wind, swells and thins as it goes.
  bool smoke = aExtra.a < 0.0;
  float size = abs(aExtra.a);
  vec3 p = smoke
    ? aPos + vec3(life * 0.12 + sin(uTime * 0.4 + aNormal.x * 6.28) * 0.03, life * 0.6, -life * 0.05)
    : aPos + vec3(sin(uTime * 0.7 + aNormal.x * 6.28) * 0.06, life * 0.35, cos(uTime * 0.5 + aNormal.x * 4.0) * 0.06);
  vec3 w = p + vec3(uCopyX, 0.0, 0.0);
  vec4 c = uViewProj * vec4(w, 1.0);
  gl_Position = c;
  if (smoke) size *= 0.6 + 1.6 * life;
  gl_PointSize = clamp(size * uPx / max(c.w, 0.1) * 30.0, 1.0, smoke ? 64.0 : 24.0);
  vCol = vec4(aExtra.rgb, smoke ? 0.7 * (1.0 - life) : 1.0);
  vLife = life;
  float v = texture(uVis, vec2(aPos.x, -aPos.z) / uMapSize).r;
  vCol.a *= smoothstep(0.6, 0.95, v);
}`;

export const PARTICLE_FS = `
in vec4 vCol;
in float vLife;
out vec4 o;
void main() {
  vec2 d = gl_PointCoord - 0.5;
  float r = length(d);
  float a = (1.0 - smoothstep(0.15, 0.5, r)) * sin(vLife * 3.14159) * vCol.a;
  if (a < 0.01) discard;
  o = vec4(vCol.rgb * a, a);
}`;

export const BLIT_FS = `
in vec2 vUv;
uniform sampler2D uSrc;
out vec4 o;
void main() { o = texture(uSrc, vUv); }`;

// FXAA (Timothy Lottes' console-quality variant): cheap edge anti-aliasing on the finished colour,
// to take the stair-step off silhouettes and the ink outlines and stop them crawling when panning
// (F17). Run after the finish pass, before the overlays; the HUD is a separate 2D canvas.
export const FXAA_FS = `
in vec2 vUv;
uniform sampler2D uSrc;
uniform vec2 uTexel;
out vec4 o;
#define FXAA_SPAN_MAX 8.0
#define FXAA_REDUCE_MUL (1.0 / 8.0)
#define FXAA_REDUCE_MIN (1.0 / 128.0)
float flum(vec3 c) { return dot(c, vec3(0.299, 0.587, 0.114)); }
void main() {
  vec2 t = uTexel;
  vec3 rgbNW = texture(uSrc, vUv + vec2(-t.x, -t.y)).rgb;
  vec3 rgbNE = texture(uSrc, vUv + vec2(t.x, -t.y)).rgb;
  vec3 rgbSW = texture(uSrc, vUv + vec2(-t.x, t.y)).rgb;
  vec3 rgbSE = texture(uSrc, vUv + vec2(t.x, t.y)).rgb;
  vec3 rgbM = texture(uSrc, vUv).rgb;
  float lNW = flum(rgbNW), lNE = flum(rgbNE), lSW = flum(rgbSW), lSE = flum(rgbSE), lM = flum(rgbM);
  float lMin = min(lM, min(min(lNW, lNE), min(lSW, lSE)));
  float lMax = max(lM, max(max(lNW, lNE), max(lSW, lSE)));
  vec2 dir = vec2(-((lNW + lNE) - (lSW + lSE)), ((lNW + lSW) - (lNE + lSE)));
  float dirReduce = max((lNW + lNE + lSW + lSE) * 0.25 * FXAA_REDUCE_MUL, FXAA_REDUCE_MIN);
  float rcpDirMin = 1.0 / (min(abs(dir.x), abs(dir.y)) + dirReduce);
  dir = clamp(dir * rcpDirMin, vec2(-FXAA_SPAN_MAX), vec2(FXAA_SPAN_MAX)) * t;
  vec3 rgbA = 0.5 * (texture(uSrc, vUv + dir * (1.0 / 3.0 - 0.5)).rgb + texture(uSrc, vUv + dir * (2.0 / 3.0 - 0.5)).rgb);
  vec3 rgbB = rgbA * 0.5 + 0.25 * (texture(uSrc, vUv + dir * -0.5).rgb + texture(uSrc, vUv + dir * 0.5).rgb);
  float lB = flum(rgbB);
  o = vec4((lB < lMin || lB > lMax) ? rgbA : rgbB, 1.0);
}`;
