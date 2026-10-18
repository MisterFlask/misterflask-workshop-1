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

// Strokes running along the map's x -- left to right on the screen, the camera looking north --
// 'f' across and 'f / stretch' along, periodic like mnoise (water seen at an angle breaks its
// reflections into such level strokes).
float mstrokeH(vec2 m, float f, float stretch) {
  float fa = f / stretch;
  if (uWrapX > 0.5) {
    float P = max(1.0, floor(uMapSize.x * fa + 0.5));
    return pnoise(vec2(m.x * P / uMapSize.x, m.y * f), vec2(P, 4096.0));
  }
  return pnoise(vec2(m.x * fa, m.y * f), vec2(4096.0));
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
  float sh0 = sh;
  sh = mix(sh, mix(0.38, 1.0, sh), uStylize);
  sun = d * sh;
  vec3 amb = mix(uGroundColor, uSkyColor, n.y * 0.5 + 0.5);
  // The floor is the light of the open sky, not a dimmed sun: cooler than the sun, so a shadow
  // reads as a saturated cool shade (teal on grass, violet-brown on sand) rather than as the same
  // warm colour turned grey.
  vec3 skyFill = mix(uSunColor, lum(uSunColor) * vec3(0.78, 0.92, 1.2), 0.5 * uStylize);
  vec3 L = uSunColor * (d * sh0) + skyFill * (d * (sh - sh0)) + amb * ao;
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

// Plots per pixel at this fragment, set in main before any branch (the ground's marks fade by it).
float gPpx = 0.0;
// How much of the paint filter the ground just painted is spared (its marks are the paint there).
float gKeep = 0.0;

// Ground marks: one flat-toned mark per cell of a jittered lattice (about f per plot), periodic like
// mnoise, each turned to 'ang' (radians from the map's x) give or take 'jit', drawn out 'elong'
// times, 'lobe' three-lobed like dab2 (0: a plain tapered stroke), with an edge about a pixel wide
// at any zoom ('aa': cells per pixel). Returns (pick, tone, cover, along): two hashes of the mark
// that covers the point, how firmly, and where along the mark the point lies (-1 its tail, 1 the end
// it heads to: a blade's tip).
vec4 gmarks(vec2 m, float f, float ang, float jit, float elong, float rad, float lobe, float aa) {
  float P = uWrapX > 0.5 ? max(1.0, floor(uMapSize.x * f + 0.5)) : 4096.0;
  vec2 p = uWrapX > 0.5 ? vec2(m.x * P / uMapSize.x, m.y * f) : m * f;
  vec2 i = floor(p);
  vec2 fr = fract(p);
  float soft = 0.12 + aa * 1.2;
  float best = 0.0, along = 0.0;
  vec2 pick = vec2(0.5);
  // Per call, not per cell: the heading's rotation (each cell turns it by its own small jitter,
  // rotated by polynomials -- the terrain is the frame's costliest shading, and a software
  // rasteriser's sine is dear), and how far a mark can reach (cells beyond it are skipped).
  float cb = cos(ang), sb = sin(ang);
  bool wide = jit > 1.3;
  float reach = 1.46 * rad * sqrt(elong) / (1.0 - lobe) + 0.02;
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 g = vec2(float(x), float(y));
      vec2 cell = mod(i + g, vec2(P, 4096.0));
      vec2 o = g + hash22(cell) - fr;
      if (dot(o, o) > reach * reach) continue;
      float hs = hash12(cell + 1.3);
      float jj = (hash12(cell + 3.7) - 0.5) * jit;
      float ca, sa;
      if (wide) { ca = cos(ang + jj); sa = sin(ang + jj); }
      else {
        float j2 = jj * jj;
        float cj = 1.0 - j2 * 0.5 * (1.0 - j2 / 12.0), sj = jj * (1.0 - j2 / 6.0 * (1.0 - j2 / 20.0));
        ca = cb * cj - sb * sj;
        sa = sb * cj + cb * sj;
      }
      vec2 r = vec2(ca * o.x + sa * o.y, -sa * o.x + ca * o.y);
      float se = sqrt(elong * (0.75 + 0.5 * hash12(cell + 6.1)));
      vec2 q = r * vec2(1.0 / se, se) / (rad * (0.7 + 0.6 * hs));
      float len = length(q);
      float d = len;
      if (lobe > 0.0) { float c = q.x / max(len, 1e-4); d *= 1.0 + lobe * (4.0 * c * c * c - 3.0 * c); }
      float w = (1.0 - smoothstep(1.0 - soft, 1.0, d)) * (0.55 + 0.45 * hs);
      if (w > best) { best = w; pick = vec2(hash12(cell + 9.1), hash12(cell + 5.3)); along = -q.x; }
    }
  }
  return vec4(pick, best, along);
}

// --- Laid marks on the open ground. Each ground: its broad masses (as masses(), without the soft
// dabs) -- the meadow's green and golden grass, desert and tundra with a slow drift between a warm
// and a cool colour of their own -- then marks laid over them at up
// to three sizes (4.5, 11 and 26 a plot) -- each fading in once its marks are a few pixels and out
// before they would swell into blots, so at the gameplay zoom the two larger are laid and close up
// the finest joins them -- each mark one of four related colours, so the ground reads as strokes of
// broken colour while its value stays quiet round roads, rings and figures. Every ground its own
// hand: upright blades on the meadow's green and in reed beds, long strokes swept along the wind
// on its golden grass and on snow, round clumps on tundra and in perdition, strokes along the
// dunes' crests in the deserts, each basalt plate its own direction. A ground holding only a
// sliver of a blend lays none (gMarkW: no muddy double layer at borders).
float gMarkW = 1.0;
float gK() { return uStylize * min(uDabs / 1.6, 1.5); }
// Three tones by the broad shapes ('big') and a finer noise ('mid'): dark where the broad shapes
// rise highest, light on their shoulders where the finer one rises too.
vec3 mass3(vec3 dark, vec3 base, vec3 light, float big, float mid) {
  vec3 c = mix(base, dark, smoothstep(0.52, 0.8, big) * 0.6);
  return mix(c, light, smoothstep(0.38, 0.78, big) * smoothstep(0.45, 0.8, mid) * 0.5);
}
vec3 broad(vec2 m, vec3 dark, vec3 base, vec3 light, float scale, out float big) {
  big = mfbm(m, 0.45 * scale);
  return mass3(dark, base, light, big, mnoise(m + 11.0, 1.6 * scale));
}
vec3 drift(vec2 m, vec3 c, vec3 ca, vec3 cb, float a) {
  return mix(c, mix(cb, ca, smoothstep(0.3, 0.7, mnoise(m - 23.0, 0.7))), a * gK());
}
// The wind over open country (radians from the map's x), turning slowly.
float gWind(vec2 m) { return 0.35 + (mnoise(m + 41.0, 0.3) - 0.5) * 1.6; }
// A palette: four colours and the cumulative pick thresholds of the first three.
mat4 P4(vec3 c0, vec3 c1, vec3 c2, vec3 c3, vec3 cuts) { return mat4(vec4(c0, cuts.x), vec4(c1, cuts.y), vec4(c2, cuts.z), vec4(c3, 1.0)); }
// The meadow's second palette (golden dry grass) and how much of the point lies golden (0 on every
// other ground): a mark takes the second palette where gSplit passes a threshold of its own, so the
// edge of a golden patch is a band of interleaved green and golden strokes, not an olive mix.
mat4 gPalB;
float gSplit = 0.0;
// How much of the meadow at this fragment is prairie rather than grassland (set in main).
float gDry = 0.0;
// One size of marks laid over c: 'str' how firmly, 'tone' the value spread of the marks (+-),
// 'hue' how much of each mark's value comes from c (1: only its hue from the palette), 'tip' how
// much lighter a mark is at its head than at its tail (a blade of grass: light tip, dark root),
// 'seed' decorrelates sizes.
vec3 lay(vec2 m, vec3 c, mat4 pal, float f, float str, float ang, float jit, float elong, float rad, float lobe, float tone, float hue, float tip, float seed) {
  float aa = gPpx * f;
  float s = str * (1.0 - smoothstep(0.22, 0.42, aa)) * smoothstep(0.02, 0.03, aa) * gK() * gMarkW;
  // A size that would add next to nothing is not laid (the finest at the gameplay zoom, a blend's
  // minor ground): faded to zero continuously, then skipped.
  s *= smoothstep(0.015, 0.05, s);
  if (s <= 0.0) return c;
  vec4 mk = gmarks(m + seed, f, ang, jit, elong, rad, lobe, aa);
  if (gSplit > hash12(mk.xy * 61.0)) pal = gPalB;
  vec3 pc = mk.x < pal[0].w ? pal[0].rgb : mk.x < pal[1].w ? pal[1].rgb : mk.x < pal[2].w ? pal[2].rgb : pal[3].rgb;
  pc = mix(pc, pc * lum(c) / max(lum(pc), 1e-4), hue);
  pc *= 1.0 + tip * mk.w;
  return mix(c, pc * (1.0 - tone + 2.0 * tone * mk.y), mk.z * s);
}
// The meadow's hatching, finer than its marks: value noise drawn out into short streaks -- upright
// on the green (f across, a third of that along the map's y), along the wind on the golden (mstroke:
// the map's diagonal, near enough the wind's mean heading) -- at two sizes, each fading out before
// it would shimmer, so the sward reads as grass even where the marks are too small to lay.
vec3 hatch(vec2 m, vec3 c, float dry) {
  float k = 0.0;
  for (int o = 0; o < 2; o++) {
    float f = o == 0 ? 17.0 : 38.0;
    float a = (1.0 - smoothstep(0.2, 0.4, gPpx * f)) * gK() * gMarkW;
    if (a <= 0.0) continue;
    float h = 0.0;
    if (dry < 0.99) {
      float P = uWrapX > 0.5 ? max(1.0, floor(uMapSize.x * f + 0.5)) : 4096.0;
      h = pnoise(vec2(uWrapX > 0.5 ? m.x * P / uMapSize.x : m.x * f, m.y * f / 3.0) + float(o) * 7.7, vec2(P, 4096.0)) * (1.0 - dry);
    }
    if (dry > 0.01) h += mstroke(m + float(o) * 5.3, f, 4.0) * dry;
    // The green carries more of it than the straw (its marks vary less in value).
    k += (h - 0.5) * a * (o == 0 ? 0.32 : 0.26) * mix(1.4, 1.0, dry);
  }
  return c * (1.0 + k);
}
// The meadow's flowers: on a fine lattice (17 a plot) a few cells, in loose drifts, hold a fleck --
// white, butter yellow or a cool violet -- a pixel or two across at the gameplay zoom, gone before
// the flecks would shimmer. 'amt' scales how many. Periodic like mnoise.
vec3 flowers(vec2 m, vec3 c, float amt) {
  const float f = 17.0;
  float aa = gPpx * f;
  float k = amt * (1.0 - smoothstep(0.13, 0.24, aa)) * min(gK(), 1.0) * gMarkW;
  float drift = smoothstep(0.5, 0.78, mnoise(m + 19.0, 1.1));
  if (k * drift <= 0.0) return c;
  float P = uWrapX > 0.5 ? max(1.0, floor(uMapSize.x * f + 0.5)) : 4096.0;
  vec2 p = uWrapX > 0.5 ? vec2(m.x * P / uMapSize.x, m.y * f) : m * f;
  vec2 cell = mod(floor(p), vec2(P, 4096.0));
  vec2 h = hash22(cell + 0.7);
  if (h.x > 0.15 * drift) return c;
  vec2 o = fract(p) - 0.27 - 0.46 * hash22(cell + 4.1);
  float r = 0.12 + 0.08 * h.y;
  float fl = 1.0 - smoothstep(r - aa * 0.5, r + aa * 0.5, length(o));
  float pk = hash12(cell + 8.3);
  vec3 fc = pk < 0.5 ? vec3(0.93, 0.92, 0.84) : pk < 0.8 ? vec3(0.95, 0.82, 0.34) : vec3(0.68, 0.60, 0.86);
  return mix(c, fc, fl * k);
}
// Dunes: crests across the map's diagonal, sinuous; dunes(phase) = (the lit back slope rising to the
// crest, the shadow dropping at once behind it), the crest's edge about two pixels wide. The phase
// runs a whole number of cycles over a wrapping map's width (no seam).
float dunePh(vec2 m) {
  float fx = uWrapX > 0.5 ? floor(uMapSize.x * 1.146 + 0.5) / uMapSize.x : 1.146;
  return m.x * fx + m.y * 1.432 + 1.432 * mfbm(m, 0.8);
}
vec2 dunes(float ph) {
  float t = fract(ph - 0.25), e = 2.2 * gPpx;
  return vec2(smoothstep(0.25, 1.0, t) * (1.0 - smoothstep(1.0 - e, 1.0, t)),
              (1.0 - smoothstep(0.0, 0.3, t)) * smoothstep(0.0, e, t));
}
// mcells with the nearest cell's id (a hash of its wrapped cell, 0..1).
vec3 mcellsId(vec2 m, float f) {
  float P = uWrapX > 0.5 ? max(1.0, floor(uMapSize.x * f + 0.5)) : 4096.0;
  vec2 p = uWrapX > 0.5 ? vec2(m.x * P / uMapSize.x, m.y * f) : m * f;
  vec2 i = floor(p);
  vec2 fr = fract(p);
  float d1 = 8.0, d2 = 8.0, id = 0.0;
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 g = vec2(float(x), float(y));
      vec2 cell = mod(i + g, vec2(P, 4096.0));
      vec2 o = hash22(cell);
      float d = length(g + o - fr);
      if (d < d1) { d2 = d1; d1 = d; id = hash12(cell + 2.9); } else if (d < d2) { d2 = d; }
    }
  }
  return vec3(d1, d2, id);
}

// The unexplored: dark parchment with a faint ink hatching. Its own function: the frontier's call
// would otherwise inline a second copy of every ground's painting.
vec3 blankAlbedo(vec2 m) {
  float hatch = smoothstep(0.55, 0.9, mstroke(m, 14.0, 8.0));
  vec3 c = mix(vec3(0.15, 0.12, 0.10), vec3(0.25, 0.20, 0.15), mfbm(m, 0.5));
  c *= 0.9 + 0.2 * mdabs(m, 1.5);
  return srgb(c * (1.0 - hatch * 0.22));
}

vec3 groundAlbedo(int g, vec2 m, float h, inout vec3 emit) {
  if (g == G_SHALLOWS || g == G_COAST) return srgb(masses(m, vec3(0.45, 0.40, 0.30), vec3(0.62, 0.55, 0.40), vec3(0.72, 0.66, 0.50), 1.0));
  if (g == G_OCEAN) return srgb(vec3(0.22, 0.26, 0.28));
  bool hell = g == G_BURNING || g == G_BROKEN || g == G_PERDITION;
  if (g != G_GRASS && g != G_PLAINS && g != G_DESERT && g != G_TUNDRA && g != G_SNOW && g != G_MARSH && !hell) return blankAlbedo(m);
  // Each open ground: its masses (c), then the hand its marks are laid in -- a palette per size
  // (P0..P2), the sizes' strengths, heading, angle spread, length, width, lobe, value spread and
  // tip, and how much of a mark's value comes from the masses (hue) -- all laid by the one loop
  // below (one inlined copy of the mark lattice per size, not one per ground: the terrain shader
  // stays small enough to compile quickly and run lean).
  float big;
  vec3 c;
  mat4 P0, P1, P2;
  vec3 str = vec3(0.0), jit = vec3(0.35), elong = vec3(2.0), rad = vec3(0.5), lobe = vec3(0.2), tone = vec3(0.12);
  float ang = 0.0;
  float hue = 0.0, dry = 0.0;
  vec3 tip = vec3(0.0);
  float rimW = 0.0, poolW = 0.0, seam = 0.0;
  vec3 hot = vec3(1.0, 0.4, 0.1);
  gSplit = 0.0;
  if (g == G_GRASS) {
    // The meadow: grassland and prairie painted as one ground (main folds the prairie in), green
    // grass broken by golden dry grass. Where it lies golden: organic patches (about 1, 0.5 and 0.3
    // plots, the finest drawn out along the map's diagonal) that ignore the plot grid, the prairie's
    // share of the point (gDry) tipping the balance -- about a sixth of a grassland lies golden, five
    // sixths of a prairie, and no broader noise than a plot, so a whole region never flips -- so a
    // plot's centre still reads as its own ground while the grid no longer shows in the grass.
    float dn = mnoise(m + 61.0, 0.85) * 0.35 + mnoise(m - 27.0, 1.9) * 0.4 + mstroke(m + 13.0, 3.6, 2.5) * 0.25;
    float t = mix(0.63, 0.37, gDry);
    // A ragged edge: finer fingers combed along the wind where a patch meets the green (only there).
    if (abs(dn - t) < 0.12) dn += (mstroke(m - 7.0, 11.0, 3.0) - 0.5) * 0.14;
    dry = smoothstep(t - 0.06, t + 0.06, dn);
    gSplit = dry;
    big = mfbm(m, 0.45);
    float mid = mnoise(m + 11.0, 1.6), mot = mnoise(m + 5.0, 2.4);
    // Green: yellow-green with darker blue-green mottling at two sizes (the broad shapes and
    // half-plot blotches), drifting slowly between a yellower and a bluer green.
    vec3 cg = mass3(vec3(0.18, 0.27, 0.17), vec3(0.27, 0.35, 0.22), vec3(0.35, 0.415, 0.24), big, mid);
    cg = mix(cg, vec3(0.17, 0.26, 0.17), smoothstep(0.55, 0.8, mot) * 0.42);
    vec3 dr = mix(vec3(0.20, 0.31, 0.21), vec3(0.33, 0.40, 0.20), smoothstep(0.3, 0.7, mnoise(m - 23.0, 0.7)));
    cg = mix(cg, dr * lum(cg) / max(lum(dr), 1e-4), 0.22 * gK());
    // Golden: straw over ochre, combed by the wind into lighter and darker streaks; its shade an
    // olive ochre (not russet: the warm sun and the grade's saturation would turn that orange).
    vec3 cd = mass3(vec3(0.37, 0.30, 0.185), vec3(0.50, 0.42, 0.265), vec3(0.62, 0.54, 0.355), big, mid);
    cd = mix(cd, vec3(0.38, 0.31, 0.185), smoothstep(0.55, 0.8, mstroke(m - 3.0, 5.0, 4.0)) * 0.5);
    cd = mix(cd, vec3(0.64, 0.56, 0.385), smoothstep(0.6, 0.85, mstroke(m + 9.0, 3.5, 5.0)) * 0.3);
    cd = mix(cd, vec3(0.43, 0.40, 0.255), smoothstep(0.55, 0.8, mot) * 0.4);
    c = mix(cg, cd, dry);
    // Marks: short upright blades on the green (each mark's value from the masses under it, only its
    // hue from the palette -- yellow-green, blue-green, olive, sap -- lighter at the tip), long strokes
    // swept along the wind on the golden (light straw, ochre, gold and a green fleck); a mark between
    // the two is bent between their headings, as grass bends.
    P0 = P4(vec3(0.33, 0.41, 0.21), vec3(0.22, 0.32, 0.21), vec3(0.30, 0.36, 0.20), vec3(0.25, 0.34, 0.18), vec3(0.3, 0.55, 0.8)); P1 = P0; P2 = P0;
    gPalB = P4(vec3(0.60, 0.53, 0.365), vec3(0.46, 0.37, 0.235), vec3(0.53, 0.46, 0.295), vec3(0.34, 0.38, 0.23), vec3(0.35, 0.62, 0.88));
    // The finest blades thin and thicken (smoother sward between thicker tufts), so the grass is not
    // one even pile -- but never vanish: the target's grass is textured everywhere.
    str = mix(vec3(0.35, 0.6, mix(0.45, 0.75, smoothstep(0.25, 0.6, mid))), vec3(0.45, 0.6, 0.7), dry);
    ang = mix(1.5708 + (mnoise(m + 41.0, 0.5) - 0.5) * 0.6, gWind(m), dry);
    jit = vec3(mix(0.8, 0.35, dry)); elong = mix(vec3(2.0, 4.5, 3.5), vec3(4.0, 5.0, 5.5), dry); rad = vec3(0.55, 0.55, 0.6); lobe = vec3(0.0);
    tone = mix(vec3(0.18), vec3(0.16, 0.2, 0.24), dry); hue = mix(0.5, 0.3, dry); tip = mix(vec3(0.1, 0.22, 0.3), vec3(0.0, 0.1, 0.2), dry);
  } else if (g == G_DESERT) {
    // Desert: pale sand cut by sinuous dune crests -- the back slope brightening to the crest, a cool
    // rose shadow behind it (the sun is from the south-west) -- with long low sheets of sand laid
    // along the crests, and close up ripples on the lit back slopes and finer grains.
    c = broad(m, vec3(0.58, 0.46, 0.32), vec3(0.71, 0.59, 0.42), vec3(0.82, 0.72, 0.55), 0.8, big);
    vec3 d0 = vec3(0.78, 0.68, 0.50), d1 = vec3(0.60, 0.48, 0.40), d2 = vec3(0.74, 0.58, 0.36), d3 = vec3(0.68, 0.60, 0.47);
    c = drift(m, c, d0, d1, 0.15);
    float ph = dunePh(m);
    vec2 dn = dunes(ph);
    c *= 0.91 + 0.12 * dn.x;
    c = mix(c, vec3(0.58, 0.46, 0.40), 0.35 * dn.y);
    float rs = fract(ph * 6.0), er = 13.2 * gPpx;
    float band = smoothstep(0.5, 0.5 + er, rs) * (1.0 - smoothstep(0.75, 0.75 + er, rs));
    band *= smoothstep(0.25, 0.3, fract(ph - 0.25)); // on the lit back slope only, not the slip face
    float rlod = 1.0 - smoothstep(0.12, 0.2, 11.0 * gPpx);
    P0 = P4(d0, d3, d2, d3, vec3(0.35, 0.6, 0.8)); P1 = P4(d0, d1, d1, d3, vec3(0.35, 0.6, 0.85)); P2 = P4(d0, d1, d2, d3, vec3(0.2, 0.5, 0.8));
    str = vec3(0.35, 0.7 * band * rlod, 0.35); ang = 2.46;
    jit = vec3(0.35, 0.2, 0.8); elong = vec3(3.0, 5.0, 1.8); rad = vec3(0.55, 0.5, 0.6); lobe = vec3(0.2, 0.2, 0.0); tone = vec3(0.08, 0.08, 0.10);
  } else if (g == G_TUNDRA) {
    // Tundra: lichen and moss mottling in round clumps -- moss, rust dwarf shrub, pale lichen, slate --
    // all near one value; late snow only in the hollows, as the larger patches.
    c = broad(m, vec3(0.33, 0.35, 0.30), vec3(0.45, 0.46, 0.38), vec3(0.58, 0.57, 0.48), 1.2, big);
    vec3 t0 = vec3(0.41, 0.45, 0.36), t1 = vec3(0.49, 0.42, 0.33), t2 = vec3(0.54, 0.55, 0.48), t3 = vec3(0.36, 0.37, 0.38);
    c = drift(m, c, t0, t1, 0.15);
    float hollow = smoothstep(0.55, 0.75, big);
    vec3 cuts = vec3(0.40, 0.62, 0.85);
    P0 = P4(t0, t1, mix(t2, vec3(0.78, 0.82, 0.86), 0.6 * hollow), t3, cuts); P1 = P4(t0, t1, t2, t3, cuts); P2 = P1;
    str = vec3(0.34, 0.40, 0.32); ang = 0.6;
    jit = vec3(1.2); elong = vec3(1.6); lobe = vec3(0.0);
  } else if (g == G_SNOW) {
    // Snow: carved by the wind -- blue and lilac shadow streaks on warm white, laid along the same
    // wind as the prairie's, so it runs on across a border.
    c = broad(m, vec3(0.70, 0.76, 0.84), vec3(0.85, 0.88, 0.92), vec3(0.95, 0.96, 0.97), 0.7, big);
    P0 = P4(vec3(0.96, 0.94, 0.90), vec3(0.60, 0.68, 0.84), vec3(0.68, 0.66, 0.82), vec3(0.84, 0.88, 0.94), vec3(0.30, 0.52, 0.68)); P1 = P0; P2 = P0;
    str = vec3(0.60, 0.65, 0.40); ang = gWind(m);
    jit = vec3(0.3); elong = vec3(5.0); tone = vec3(0.06);
  } else if (g == G_MARSH) {
    c = broad(m, vec3(0.20, 0.24, 0.17), vec3(0.29, 0.34, 0.21), vec3(0.40, 0.42, 0.26), 1.3, big);
    // Shallow pools among the reeds, catching the sky; a darker muddy rim around each.
    float pn = mfbm(m + 7.0, 2.6) + 0.03;
    rimW = smoothstep(0.56, 0.62, pn) * (1.0 - smoothstep(0.62, 0.66, pn));
    poolW = smoothstep(0.63, 0.67, pn);
    // Reed beds: stiff upright strokes in straw-olive, dark sedge, bronze and wet peat (none in the
    // water), darker and browner than grass.
    P0 = P4(vec3(0.42, 0.40, 0.24), vec3(0.22, 0.28, 0.20), vec3(0.36, 0.31, 0.19), vec3(0.20, 0.21, 0.15), vec3(0.30, 0.60, 0.80)); P1 = P0; P2 = P0;
    str = vec3(0.25, 0.32, 0.30) * (1.0 - poolW); ang = 1.5708 + (mnoise(m + 41.0, 0.5) - 0.5) * 0.3;
    jit = vec3(0.35); elong = vec3(3.6);
  } else if (g == G_BURNING) {
    // Corrupted ground, each hell terrain its own material (F3). Burning sands: the desert's hell
    // twin -- the same dunes, lower, with char-dark lee shadows -- and broken flakes of ash, burnt
    // orange, rust and char laid along the crests.
    c = broad(m, vec3(0.26, 0.15, 0.09), vec3(0.40, 0.25, 0.16), vec3(0.56, 0.38, 0.22), 0.9, big);
    vec2 dn = dunes(dunePh(m));
    c *= 0.94 + 0.08 * dn.x;
    c = mix(c, vec3(0.24, 0.14, 0.10), 0.15 * dn.y);
    P0 = P4(vec3(0.44, 0.39, 0.36), vec3(0.52, 0.29, 0.14), vec3(0.36, 0.21, 0.13), vec3(0.24, 0.15, 0.11), vec3(0.18, 0.52, 0.82)); P1 = P0; P2 = P0;
    str = vec3(0.25, 0.30, 0.25); ang = 2.46;
    jit = vec3(0.8); elong = vec3(2.5); rad = vec3(0.45);
    hot = vec3(1.0, 0.42, 0.10);
  } else if (g == G_BROKEN) {
    // Broken lands: grey basalt cut into plates by cell edges (the F_ICE idiom), dark seams between
    // them; each plate painted as its own facet -- its own tone and stroke direction, the strokes
    // stopping at the seams. Only the two finer sizes (the seams already make it the busiest ground).
    c = broad(m, vec3(0.20, 0.18, 0.18), vec3(0.30, 0.27, 0.26), vec3(0.42, 0.39, 0.37), 1.1, big);
    vec3 cl = mcellsId(m, 3.2);
    c *= 0.9 + 0.2 * cl.z;
    seam = 1.0 - smoothstep(0.015, 0.06, cl.y - cl.x);
    P0 = P4(vec3(0.30, 0.30, 0.33), vec3(0.36, 0.29, 0.24), vec3(0.44, 0.41, 0.39), vec3(0.19, 0.18, 0.19), vec3(0.35, 0.65, 0.82)); P1 = P0; P2 = P0;
    str = vec3(0.0, 0.35, 0.30); ang = cl.z * 3.1416;
    jit = vec3(0.3); elong = vec3(2.4); tone = vec3(0.10);
    hot = vec3(1.0, 0.36, 0.08);
  } else {
    // Fields of perdition: wine-red with dark festering pools (the marsh pool/rim idiom), the ground
    // between them clotted in round dabs of crimson, bruise, sick flesh and clot.
    c = broad(m, vec3(0.22, 0.10, 0.13), vec3(0.34, 0.15, 0.19), vec3(0.48, 0.24, 0.27), 1.2, big);
    float pn = mfbm(m + 7.0, 2.4) + 0.03;
    rimW = smoothstep(0.56, 0.62, pn) * (1.0 - smoothstep(0.62, 0.66, pn));
    poolW = smoothstep(0.63, 0.67, pn);
    P0 = P4(vec3(0.46, 0.16, 0.18), vec3(0.30, 0.14, 0.24), vec3(0.50, 0.28, 0.26), vec3(0.18, 0.06, 0.08), vec3(0.35, 0.65, 0.85)); P1 = P0; P2 = P0;
    str = vec3(0.25, 0.30, 0.28) * (1.0 - poolW); ang = 0.0;
    jit = vec3(6.283); elong = vec3(1.3); rad = vec3(0.55); lobe = vec3(0.0);
    hot = vec3(0.95, 0.18, 0.12);
  }
  // The meadow spares more of the paint filter: its blades and flowers are finer than the filter's
  // radius, which would smear them back into a smooth wash.
  gKeep = (g == G_GRASS ? 0.8 : 0.5) * min(gK(), 1.0);
  for (int o = 0; o < 3; o++) {
    float f = o == 0 ? 4.5 : o == 1 ? 11.0 : 26.0;
    c = lay(m, c, o == 0 ? P0 : o == 1 ? P1 : P2, f, str[o], ang, jit[o], elong[o], rad[o], lobe[o], tone[o], hue, tip[o], float(o) * 13.1);
  }
  if (g == G_GRASS) return srgb(flowers(m, hatch(m, c, dry), 1.0 - dry * 0.7));
  if (g == G_MARSH) {
    c = mix(c, c * 0.7, rimW);
    // The pools are peaty, darker and greener than open water (marsh is land), with a faint
    // drifting sheen of the sky.
    float sheen = smoothstep(0.35, 0.8, mstroke(m + vec2(uTime * 0.02, 0.0), 5.0, 4.0));
    vec3 water = mix(vec3(0.15, 0.23, 0.24), vec3(0.34, 0.40, 0.42), sheen * 0.3);
    return srgb(mix(c, water, poolW * 0.85));
  }
  if (!hell) return srgb(c);
  // The hell grounds: mid-value bases so daylight and the marks register; a sparse smouldering crack
  // line (not iso-line rings); brightest where the corruption runs deepest; a slow pulse; anchored.
  c = mix(c, c * 0.5, seam * 0.8);
  c = mix(c, c * 0.6, rimW);
  c = mix(c, vec3(0.10, 0.03, 0.05), poolW * 0.8);
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
// bumpD takes the derivatives already taken (where every pixel of the quad still runs).
vec3 bumpD(vec3 n, vec3 dpdx, vec3 dpdy, float dhdx, float dhdy, float k) {
  vec3 r1 = cross(dpdy, n), r2 = cross(n, dpdx);
  float det = dot(dpdx, r1);
  vec3 grad = sign(det) * (dhdx * r1 + dhdy * r2);
  return normalize(abs(det) * n - k * grad);
}
vec3 bump(vec3 n, vec3 p, float hgt, float k) {
  return bumpD(n, dFdx(p), dFdy(p), dFdx(hgt), dFdy(hgt), k);
}

void main() {
  vec3 n = normalize(vNormal);
  vec2 m = mapOf(vWorld);
  float h = vWorld.y;
  vec3 emit = vec3(0.0);
  // Impasto: the dabs raise the paint a little (close up only: it fades out with distance). Its
  // derivatives are taken here, before the deep floor's early return below.
  float near = 1.0 - smoothstep(14.0, 30.0, length(vWorld - uEye));
  float imp = 0.0;
  if (near > 0.0 && uStylize > 0.0 && h > -0.32) imp = ((mdabs(m + 7.3, 6.5) - 0.5) * 0.6 + (mdabs(m, 2.4) - 0.5) * 0.4) * 0.012 * near * uStylize;
  vec3 dpx = dFdx(vWorld), dpy = dFdy(vWorld);
  float dix = dFdx(imp), diy = dFdy(imp);
  gPpx = length(fwidth(m));
  // The deep sea floor is deeper than the water lets anyone see (the water, drawn over it, shows at
  // most a dark trace of it), so it is shaded cheaply -- but with its true depth, which the water
  // reads for its thickness. Not where unexplored: no water is drawn there, and the blank
  // parchment below must show.
  if (h < -0.3) {
    float v0 = visAt(vWorld);
    if (v0 >= 0.05) {
      vec3 c0 = srgb(vec3(0.22, 0.26, 0.28)) * (uSunColor * max(dot(n, uSunDir), 0.0) * 0.6 + uSkyColor);
      oColor = vec4(atmosphere(fogOfWar(c0, v0), vWorld), 0.0);
      oND = vec4(normalize((uView * vec4(n, 0.0)).xyz), -(uView * vec4(vWorld, 1.0)).z);
      oEmit = vec4(0.0);
      return;
    }
  }
  if (near > 0.0 && uStylize > 0.0) n = bumpD(n, dpx, dpy, dix, diy, 1.0);

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
  int gs[4] = int[](p00.x, p10.x, p01.x, p11.x);
  // Grassland and prairie are one ground, the meadow, painted once: gDry, the prairie's share of it
  // by the smooth weights (before they are sharpened), only tips the balance between its green and
  // golden grass, so the golden patches follow their own organic shapes, not the plot grid.
  float meadW = 0.0, dryW = 0.0;
  for (int k = 0; k < 4; k++) {
    if (gs[k] != G_GRASS && gs[k] != G_PLAINS) continue;
    meadW += w[k];
    if (gs[k] == G_PLAINS) { dryW += w[k]; gs[k] = G_GRASS; }
  }
  gDry = meadW > 0.0 ? dryW / meadW : 0.0;
  // Painted edges: the weights are pushed by stroke noise, so a border between two grounds is a
  // band of interleaved brush strokes rather than a gradient.
  float brk = mdabs(m - 2.1, 4.5);
  w = pow(w, vec4(2.2 + 4.0 * brk));
  w /= dot(w, vec4(1.0));
  // Each distinct ground among the four is painted once, with the summed weight of its plots.
  float ws[4] = float[](w.x, w.y, w.z, w.w);
  // How much of the point is hell ground: its rock is basalt and it takes no snow.
  float hellW = 0.0;
  for (int k = 0; k < 4; k++) if (gs[k] == G_BURNING || gs[k] == G_BROKEN || gs[k] == G_PERDITION) hellW += ws[k];
  vec3 alb = vec3(0.0);
  float keepG = 0.0;
  for (int k = 0; k < 4; k++) {
    if (ws[k] < 0.0) continue;
    float wk = ws[k];
    for (int j = k + 1; j < 4; j++) {
      if (gs[j] == gs[k] && ws[j] >= 0.0) { wk += ws[j]; ws[j] = -1.0; }
    }
    if (wk < 0.004) continue;
    vec3 e = vec3(0.0);
    gKeep = 0.0;
    gMarkW = smoothstep(0.08, 0.2, wk);
    alb += groundAlbedo(gs[k], m, h, e) * wk;
    keepG += gKeep * wk;
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
    beach *= 1.0 - smoothstep(0.3, 0.7, texture(uDecal, mapUV(m)).b); // a river's banks: wet earth
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
    // Wet banks along the rivers (decals.js b: full under the water and on the cut bank, fading
    // over the bank tops): lush grass by the river, and the bank itself -- its cut face and the
    // strip at the water's edge -- dark wet earth and moss under the overhanging grass.
    vec4 dec = texture(uDecal, mapUV(m));
    alb = mix(alb, alb * vec3(0.86, 1.0, 0.76), smoothstep(0.05, 0.6, dec.b) * 0.6);
    float mudN = mnoise(m, 9.0);
    float mud = smoothstep(0.8 + (mudN - 0.5) * 0.16, 0.95, dec.b);
    alb = mix(alb, srgb(mix(vec3(0.25, 0.2, 0.13), vec3(0.19, 0.23, 0.12), mudN)), mud * 0.9);
    // Settlement ground, fields and roads (rasterised per plot, see decals.js).
    vec3 earth = srgb(mix(vec3(0.36, 0.29, 0.21), vec3(0.47, 0.39, 0.29), mnoise(m, 8.0)));
    alb = mix(alb, earth, dec.a * 0.8);
    // Fields (decals.js, fields.js): g holds the crop -- ~0.33 vegetable beds (green rows on tilled
    // soil, 14 a plot, running east-west), ~0.67 bare tilled soil, 1 golden grain (18 rows a plot,
    // running north-south). The filtered value fades the field in; the crop is the highest of the
    // four texels around (a field's soft edge would pass through the levels below its own). The
    // rows fade to their mean colour once they are finer than the pixels, and raise a bump.
    float fieldW = smoothstep(0.08, 0.2, dec.g);
    float rowH = 0.0;
    if (fieldW > 0.0) {
      ivec2 dsz = textureSize(uDecal, 0);
      ivec2 t0 = ivec2(floor(mapUV(m) * vec2(dsz) - 0.5));
      float gk = 0.0;
      for (int k = 0; k < 4; k++) {
        ivec2 tk = t0 + ivec2(k & 1, k >> 1);
        tk = ivec2(uWrapX > 0.5 ? (tk.x % dsz.x + dsz.x) % dsz.x : clamp(tk.x, 0, dsz.x - 1), clamp(tk.y, 0, dsz.y - 1));
        gk = max(gk, texelFetch(uDecal, tk, 0).g);
      }
      float soilK = smoothstep(0.42, 0.56, gk), grainK = smoothstep(0.78, 0.9, gk);
      // Each pattern fades once its period is under a few pixels (gPpx: map units a pixel).
      float fineF = 1.0 - smoothstep(0.01, 0.018, gPpx), fineB = 1.0 - smoothstep(0.022, 0.04, gPpx), fineG = 1.0 - smoothstep(0.018, 0.03, gPpx);
      float clod = mnoise(m, 30.0);
      float furrow = 0.5 + 0.5 * sin(m.y * 175.9 + mnoise(m, 4.0) * 3.0 + (clod - 0.5) * 1.6);
      vec3 soil = mix(vec3(0.30, 0.25, 0.185), vec3(0.41, 0.34, 0.25), mix(0.5, furrow * (0.6 + 0.6 * clod), fineF)) * (0.86 + 0.28 * mnoise(m, 9.0));
      float leaf = mnoise(m, 55.0);
      float bed = 1.0 - abs(fract(m.y * 14.0) - 0.5) * 2.0;
      float bedK = smoothstep(0.32, 0.56, bed + (leaf - 0.5) * 0.35);
      vec3 green = mix(vec3(0.24, 0.38, 0.14), vec3(0.42, 0.56, 0.22), leaf * 0.6 + bed * 0.4);
      vec3 veg = mix(soil, green, mix(0.6, bedK, fineB));
      float row = 1.0 - abs(fract(m.x * 18.0) - 0.5) * 2.0;
      float ears = mnoise(vec2(m.x, m.y * 0.25), 40.0);
      float ridge = smoothstep(0.05, 0.75, row) * (0.7 + 0.6 * ears);
      vec3 grain = mix(vec3(0.62, 0.42, 0.15), vec3(0.88, 0.69, 0.33), mix(0.62, ridge, fineG));
      vec3 crop = mix(mix(veg, soil, soilK), grain, grainK);
      alb = mix(alb, srgb(crop), fieldW);
      rowH = fieldW * mix(mix(bedK * fineB, furrow * 0.4 * fineF, soilK), ridge * 0.8 * fineG, grainK);
      keepG = max(keepG, fieldW * 0.45);
    }
    n = bump(n, vWorld, rowH * 0.007, 1.0);
    // Roads: r is a ramp falling from 1 on a track's centre line to 0 at 0.3 plots (decals.js
    // ROAD_RAMP), cut here into a packed-dirt track ~0.14 plots wide: a crisp edge, a lighter crown,
    // darker margins and wheel ruts, and a verge of worn, dry grass beside it.
    float rd = (1.0 - dec.r) * 0.3;
    if (rd < 0.15) {
      float hw = 0.07 + (mnoise(m, 7.0) - 0.5) * 0.024 + (mnoise(m + 2.3, 19.0) - 0.5) * 0.012 + (mnoise(m + 4.4, 35.0) - 0.5) * 0.04;
      float aa = max(gPpx * 0.75, 0.002);
      float on = 1.0 - smoothstep(hw - aa, hw + aa, rd);
      float t = rd / hw;
      // Mottled packed dirt (kept below the tone curve's shoulder, where mottling would wash out),
      // its margins and ruts broken along the road rather than drawn as unbroken parallel stripes.
      vec3 dirt = mix(vec3(0.515, 0.41, 0.29), vec3(0.58, 0.475, 0.34), mnoise(m, 10.0)) * (0.8 + 0.4 * mnoise(m + 5.1, 30.0));
      float broken = smoothstep(0.25, 0.65, mnoise(m + 9.7, 6.0));
      dirt = mix(dirt, vec3(0.47, 0.35, 0.23), smoothstep(0.5, 0.95, t) * mix(0.15, 0.4, broken));
      dirt = mix(dirt, vec3(0.36, 0.26, 0.17), smoothstep(0.86, 1.0, t) * mix(0.1, 0.3, broken));
      float rut = (1.0 - smoothstep(0.05, 0.13, abs(t - 0.58))) * (1.0 - smoothstep(0.006, 0.013, gPpx)) * broken;
      dirt = mix(dirt, vec3(0.45, 0.34, 0.22), rut * 0.55);
      float cob = smoothstep(0.2, 0.5, mcells(m, 26.0).x);
      dirt = mix(dirt, dirt * (0.8 + 0.3 * cob), dec.a);
      float verge = (1.0 - smoothstep(hw, hw + 0.05, rd)) * smoothstep(0.3, 0.7, mnoise(m, 30.0) + 0.2);
      alb = mix(alb, alb * vec3(1.12, 1.0, 0.7), verge * (1.0 - on) * 0.7);
      alb = mix(alb, srgb(dirt), on);
      keepG = max(keepG, on * 0.6);
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
    vec3 blank = blankAlbedo(m) * 1.5;
    float rim = smoothstep(0.0, 0.5, blankT) * (1.0 - smoothstep(0.5, 1.0, blankT));
    col = mix(col, blank, blankT);
    col *= 1.0 - rim * 0.55;
    emit *= 1.0 - blankT;
  }
  col = fogOfWar(col, v);
  col = atmosphere(col, vWorld);
  // Alpha: the paint filter's keep mask -- open ground spares part of its own marks, not the blank.
  oColor = vec4(col, keepG * (1.0 - blankT));
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
  // Foliage (1) and blades of grass and reed (8) are lit as leaves, but only a prop's foliage is
  // painted as a crown (massed leaf marks, value steps, its lobes merged by the ink): not a unit's
  // wing membrane or moss (a unit must read at a glance), nor a blade (a line, not a mass).
  bool blade = kind == 8;
  bool leafy = kind == 1 || blade;
  bool crown = kind == 1 && vTeam.a < 0.5;
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
  // Brushwork: leaf clusters on crowns, strokes on everything else, each mark its own value
  // (tone) and pushing the light/shade boundary (jit). Units take a lighter hand: they must read
  // at a glance, and the paint filter keeps them sharp too. Matte and painted props (rock, timber,
  // plaster) take the strokes strongest: flat facets are where the brush must show.
  float tone = 0.0, jit = 0.0;
  float matte = (kind == 0 || kind == 3) ? 1.0 - vTeam.a : 0.0;
  // How squarely the plane faces the sun (before the shadows): where a mark may go dark or light.
  float d0 = clamp((dot(n, uSunDir) + uWrapLight) / (1.0 + uWrapLight), 0.0, 1.0);
  if (uBrush > 0.0 && kind != 4 && kind != 6) {
    if (crown) {
      tone = leafMarks(po, nO, dpo).x * uBrush;
      // A dark mark fades toward the lit planes and a light one toward the turned-away ones: the
      // marks turn the form at its turning, rather than punch holes in the light mass.
      tone *= tone < 0.0 ? mix(0.25, 1.0, 1.0 - smoothstep(0.4, 0.85, d0)) : mix(0.4, 1.0, smoothstep(0.05, 0.45, d0));
      jit = tone * 0.75;
      alb *= 1.0 + tone * 0.6;
    } else {
      tone = strokeMarks(po, nO, dpo) * uBrush * (1.0 - 0.6 * vTeam.a) * (1.0 + matte);
      // On a matte prop a dark mark fades where the plane already turns from the sun, and no mark
      // goes past a bound: the doubled strokes would otherwise sink a tower's shaded flank into a
      // near-black streak.
      if (tone < 0.0) tone *= mix(1.0, mix(0.4, 1.0, smoothstep(0.05, 0.4, d0)), matte);
      tone = mix(tone, clamp(tone, -0.3, 0.6), matte);
      jit = tone * mix(0.5, 0.7, matte) * (1.0 - vTeam.a);
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
    float bc = min(1.0, abs(tone) * 1.6) * uStylize * (crown ? 1.0 : 0.6);
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
    if (crown) rim *= mix(1.0, 0.3, uBrush);
    col += uSkyColor * rim * 0.6 * ao;
    // Gouache: the value is laid in flat steps (foliage half a stop apart), each mark shifting
    // where the steps fall, so a crown reads as a few values of overlapping marks, not a shaded
    // ball -- the baked colour ramp, the ambient and the occlusion gradients are all flattened
    // alike, while the hues (warm lights, cool shade, each mass's own) stay broken. Other props
    // get a gentler version (flat-painted planes); units none, so they keep their modelling.
    // Matte props keep more of the marks' variation inside a step (a lower amount).
    float amt = (crown ? 0.85 : mix(0.55, 0.45, matte) * (1.0 - vTeam.a)) * uBrush * uStylize;
    if (amt > 0.0) {
      float S = crown ? 2.0 : 1.5;
      float Lc = max(lum(col), 1e-5);
      // The mark already pushed its own value (alb, jit): here it only moves where the steps fall
      // -- light marks most on the flat-faced props, where the steps would otherwise follow the
      // facets.
      float q = (log2(Lc) + tone * (crown ? 0.6 : tone > 0.0 ? mix(1.2, 1.8, matte) : 1.2)) * S + 32.0;
      float qf = floor(q) + smoothstep(0.3, 0.7, fract(q));
      float gm = mix(1.0, exp2((qf - 32.0) / S) / Lc, amt);
      // A prop's step never darkens it by more than about half a stop (a crown's steps may).
      col *= crown ? gm : max(gm, 0.7);
    }
  }
  col = fogOfWar(col, vVis);
  col = atmosphere(col, vWorld);
  // Alpha is the paint filter's keep mask: units stay sharp (1), and blades (the filter would
  // smear a line of grass away); crowns keep part of their own leaf marks (they are the paint
  // there, world-anchored, and the screen-space filter would smear them back into soft balls).
  oColor = vec4(col, blade ? 1.0 : max(vTeam.a, crown ? 0.45 * uBrush : 0.0));
  vec3 vn = normalize((uView * vec4(n, 0.0)).xyz);
  oND = vec4(vn, -(uView * vec4(vWorld, 1.0)).z);
  // Crowns write their object id negated: the ink pass (FINISH_FS) merges one crown's lobes into
  // a single mass by it and keeps crown against crown in a wood a light line. (Units keep theirs:
  // the x-ray ghost (GHOST_FS) knows a figure's own surface by it.)
  oEmit = vec4(emit * smoothstep(0.5, 0.9, vVis), crown ? -max(vTint.a, 0.003) : vTint.a);
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
in vec2 aFlow;
out vec3 vWorld;
out vec4 vExtra;
out vec2 vFlow;
uniform float uRiverLift;
invariant gl_Position;
void main() {
  // uRiverLift raises the river ribbon at the far zoom, where the coarse (LOW_RES) terrain does not
  // hold the carved channel and would otherwise bury it (F18); 0 for the sea and at near zoom.
  vec3 w = aPos + vec3(uCopyX, uRiverLift, 0.0);
  vWorld = w;
  vExtra = aExtra;
  vFlow = aFlow;
  gl_Position = uViewProj * vec4(w, 1.0);
}`;

export const WATER_FS = `
in vec3 vWorld;
in vec4 vExtra;
in vec2 vFlow;
uniform sampler2D uHeight;
uniform sampler2D uSceneColor;
uniform sampler2D uSceneND;
uniform float uRiver;
uniform float uReflect;
uniform float uRefract;
uniform float uCoarse;
uniform float uCaustics;
layout(location = 0) out vec4 oColor;
layout(location = 1) out vec4 oND;
layout(location = 2) out vec4 oEmit;

// The swell's crests run along the map's (1, 0.6) diagonal (mstroke's grain) and travel across it,
// toward the north-north-west.
const vec2 SWELL = vec2(-0.514, 0.857);

// The sea's surface height (arbitrary units) at map point p: a slow swell of long crests under two
// drifting layers of ripples. Map-anchored (periodic over a wrapping map's width), so the waves
// belong to the world and do not swim under a moving camera.
float seaH(vec2 p, float t) {
  float swell = mstroke(p - SWELL * t * 0.08, 2.6, 4.5);
  float r1 = mnoise(p + vec2(t * 0.05, t * 0.035), 3.2);
  float r2 = mnoise(p + vec2(-t * 0.04, t * 0.06), 7.0);
  return swell * 0.9 + r1 * 0.55 + r2 * 0.3;
}

// A river's surface in its own frame -- across the channel (u, -1 left bank .. 1 right bank,
// looking downstream) and down it (s, plots): ripples drawn out along the current and carried
// down it, fastest mid-channel. Each layer moves at one speed everywhere (a speed that varied
// across the channel would shear the pattern further every second, into speckle within minutes);
// the fast layer weighs most mid-channel, the slow one along the banks.
float riverH(float u, float s, float t) {
  float mid = 1.0 - u * u;
  return pnoise(vec2(u * 2.2, s * 3.2 - t * 1.6), vec2(4096.0)) * (0.3 + 0.5 * mid)
       + pnoise(vec2(u * 4.5 + 7.0, s * 7.5 - t * 1.1), vec2(4096.0)) * 0.35;
}

// How deep a river's water reads (0 its banks' light turquoise, 1 the blue of its channel's
// middle), in its own frame (bend: how it turns, + to the left looking downstream): deepening
// toward the middle in long bands drawn out along the current and carried down it, shallow over
// the inside of a bend.
float riverDeep(float u, float s, float bend, float t) {
  float b = pnoise(vec2(u * 1.7 + 3.0, s * 0.55 - t * 0.22), vec2(4096.0));
  float inner = clamp(-u * bend * 0.25, 0.0, 0.5);
  float u2 = u * u;
  // Over that, the ripples' strokes: short marks drawn out along the current, light where they
  // catch the sky, a few dark.
  float k = pnoise(vec2(u * 6.0 + 1.0, s * 4.5 - t * 0.35), vec2(4096.0));
  float marks = -0.45 * smoothstep(0.65, 0.75, k) + 0.25 * (1.0 - smoothstep(0.26, 0.34, k));
  return clamp(1.0 - u2 * u2 + (b - 0.5) * 0.8 - inner + marks, 0.0, 1.0);
}

// A thin line wherever the phase p crosses a half (w its half-width, dp the phase across a pixel):
// thinner than a pixel, it fades rather than breaking into dots.
float phaseLine(float p, float w, float dp) {
  float ww = max(w, dp);
  return (1.0 - smoothstep(ww - dp, ww + dp, abs(fract(p) - 0.5))) * min(1.0, w / dp);
}

// A river's foam at time t, in its own frame: streaks drawn out along the current -- lines running
// down the channel at a few to its width, warped into swirls (more on the outside of a bend) and
// broken along their length -- and a lace of white water on the outside of a bend. 'aa' is the
// channel's width (in u) across a pixel. Each layer moves at one speed (the warps too: a bounded
// wobble, never a shear).
float riverFoam(float u, float s, float bend, float t, float aa) {
  vec2 P = vec2(4096.0);
  float outer = smoothstep(0.15, 1.1, u * bend * 0.5);
  // The warps vary slowly across the channel (a line never folds back into a blot) and quickly
  // down it (the lines swing from side to side: swirls).
  float wv = pnoise(vec2(u * 0.6 + 5.0, s * 1.5 - t * 0.2), P) - 0.5;
  float wv2 = pnoise(vec2(u * 1.1 - 3.0, s * 3.2 - t * 0.35), P) - 0.5;
  // Each streak swells and tapers along its length like a stroke of the brush (its width from a
  // noise moving with it), and is lost in between.
  float ma = smoothstep(0.5 - 0.25 * outer, 0.85 - 0.25 * outer, pnoise(vec2(u * 2.2 + 2.0, s * 2.0 - t * 0.35), P));
  float mb = smoothstep(0.55 - 0.25 * outer, 0.88 - 0.25 * outer, pnoise(vec2(u * 3.1 - 6.0, s * 3.0 - t * 0.2), P));
  float la = phaseLine(u * 2.6 + wv * (2.2 + 1.2 * outer) + wv2 * 0.7, 0.09 * ma, aa * 2.6);
  float lb = phaseLine(u * 4.4 + 0.37 + wv * 1.6 - wv2 * 1.1, 0.065 * mb, aa * 4.4) * (0.4 + 0.6 * outer);
  // White water: churned foam in broken cells, packed against the outer bank of a bend.
  vec2 c = mcells(vec2(u * 1.7, s * 2.0 - t * 0.35 * 2.0), 2.4);
  float churn = smoothstep(0.3, 0.7, pnoise(vec2(u * 2.0, s * 2.5 - t * 0.35), P) + outer * 0.35);
  float lace = (1.0 - smoothstep(0.04, 0.24 + 0.3 * outer, c.y - c.x)) * churn * smoothstep(0.1, 0.8, u * sign(bend));
  return clamp(max(la, lb * 0.7) * (0.75 + 0.5 * outer) + lace * outer, 0.0, 1.0);
}

// Where a river's water ends: its edge (in RIVER_HALFs from its line, world.js) wanders with the
// map, never more than RIVER_WANDER either way, so the banks are drawn here, finer than the
// terrain's mesh could carve them.
float riverEdge(vec2 m) {
  return 1.0 + (mnoise(m, 2.6) - 0.5) * 0.2 + (mnoise(m + 7.7, 7.0) - 0.5) * 0.1;
}

// The painted sky the water mirrors: a pale glow low down deepening to the sky's blue overhead,
// with the clouds that drift over the land (the same field that shadows it in lightAt), seen where
// the reflected ray meets their height -- so their reflections drift too.
vec3 skyRefl(vec3 P, vec3 R) {
  float up = clamp(R.y, 0.0, 1.0);
  vec3 c = mix(uHazeColor * 1.1, uSkyColor * 1.5, smoothstep(0.05, 0.75, up));
  float tc = (2.4 - P.y) / max(R.y, 0.1);
  vec2 cm = mapOf(P + R * tc) + vec2(uTime * 0.03, uTime * 0.012);
  float cn = mnoise(cm + 31.0, 0.11) * 0.7 + mnoise(cm - 13.0, 0.23) * 0.3;
  float cloud = smoothstep(0.46, 0.7, cn) * min(1.0, uClouds * 2.0);
  return mix(c, mix(vec3(1.0, 0.98, 0.94), uSunColor * 0.4, 0.3), cloud * 0.45);
}

// The surface's slow drifts at time t -- the mirror's break-up strokes (level on the sea, along
// the current on a river), the foam's lace, the body's mottle -- so remembered water can hold them
// at rest (t = 0) and the frontier mix the two.
float brkAt(vec2 m, vec2 bw, bool river, vec2 us, float t) {
  return river ? pnoise(vec2(us.x * 2.0, us.y * 5.0 - t * 1.5), vec2(4096.0))
               : mstrokeH(m + bw + vec2(t * 0.04, -t * 0.05), 9.0, 6.0) * 0.65 + mstrokeH(m - bw + vec2(-t * 0.03, t * 0.04) + 2.7, 17.0, 5.0) * 0.35;
}
float laceAt(vec2 m, float t) {
  vec2 cl = mcells(m + vec2(t * 0.02, -t * 0.014), 10.0);
  return 1.0 - smoothstep(0.03, 0.12, cl.y - cl.x);
}
float bodyAt(vec2 m, float t) { return 0.9 + 0.2 * mfbm(m + t * 0.01, 0.5); }

void main() {
  vec2 m = mapOf(vWorld);
  float v = visAt(vWorld);
  bool river = uRiver > 0.5;
  if (!river && v < 0.02) discard;
  float t = uTime;
  // Water in sight moves; remembered water calms: its surface lies flat, without glitter or the
  // surf's breathing (only the slow drift of what it mirrors goes on, as cloud shadows drift over
  // remembered land). Across the frontier the moving and the calm values are mixed, never the time
  // itself (a time scaled by the fog would shear the waves further every second).
  float live = smoothstep(0.62, 0.97, v);
  float tl = live > 0.0 ? t : 0.0;    // the drifts' time: at rest where remembered
  bool band = live > 0.0 && live < 1.0; // the frontier: the moving and the resting values mixed
  // The sea floor's depth, read through a warp and averaged over a small footprint, so the
  // colour bands of the shelf follow soft, irregular contours instead of the plot grid.
  // Only samples under water count: around a small lake most of the footprint is dry land, which
  // read as shallows everywhere (a pale salt pan instead of a lake).
  vec2 wq = m + (vec2(mnoise(m, 0.8), mnoise(m + 9.0, 0.8)) - 0.5) * 0.7;
  float near = texture(uHeight, mapUV(m)).r;
  float gsum = 0.0, gw = 0.0;
  float gt[4];
  for (int k = 0; k < 4; k++) {
    vec2 o = vec2(k == 1 ? 0.35 : k == 3 ? -0.35 : 0.0, k == 0 ? 0.35 : k == 2 ? -0.35 : 0.0);
    float g = texture(uHeight, mapUV(wq + o)).r;
    gt[k] = g;
    // Fade each tap's weight continuously across the waterline (F20): the hard step() made a tap
    // snap in or out as it crossed the shore, so depth jumped into hard-edged polygonal facets in
    // lakes and on the shelf. The fallback to the centre sample blends by the missing weight too.
    float wet = smoothstep(0.0, 0.03, vWorld.y - g);
    gsum += g * wet;
    gw += wet;
  }
  float rest = max(0.0, 1.0 - gw);
  float ground = (gsum + near * rest) / (gw + rest);
  float depth = max(0.0, vWorld.y - mix(near, ground, smoothstep(0.0, 0.08, vWorld.y - near)));
  // How steeply the floor shelves (height per plot, over the same footprint): the rollers break
  // only where it rises toward a shore.
  float shelf = length(vec2(gt[1] - gt[3], gt[0] - gt[2])) / 0.7;

  // The current's direction (rivers): from how the downstream distance changes over the screen,
  // solved back into map space. Derivatives first, outside any branch.
  vec2 dmx = dFdx(m), dmy = dFdy(m);
  float dax = dFdx(vExtra.y), day = dFdy(vExtra.y);
  float det = dmx.x * dmy.y - dmx.y * dmy.x;
  vec2 flow = abs(det) > 1e-12 ? vec2(dmy.y * dax - dmx.y * day, dmx.x * day - dmy.x * dax) / det : vec2(0.0, 1.0);
  flow = normalize(flow + vec2(1e-6, 0.0));
  vec2 side = vec2(flow.y, -flow.x);
  float gaa = length(fwidth(m * 8.0));
  float ppx = max(length(dmx), length(dmy)); // plots per pixel
  // A river's water ends at its wandering edge (riverEdge), rounded past the ribbon's ends (a
  // source, a confluence); beyond it the bank under the ribbon shows. 'across' is in RIVER_HALFs.
  float across = length(vec2(vExtra.x, vFlow.y));
  float edgeAA = max(fwidth(across), 1e-4);
  float inEdge = river ? riverEdge(m) - across : 1.0;
  float inWater = smoothstep(-edgeAA, edgeAA, inEdge);
  // A river along the edge of the unexplored: the blank's parchment covers its far half (no water
  // is drawn on an unrevealed plot).
  if (river) inWater *= smoothstep(0.18, 0.32, v);
  if (inWater <= 0.0) discard;

  // The surface normal: the sea's swell and ripples, or a river's current (map y runs along -z).
  vec3 n;
  if (river) {
    float u = vExtra.x, s = vExtra.y, e = 0.02;
    float h0 = riverH(u, s, t);
    vec2 g = flow * (riverH(u, s + e, t) - h0) / e + side * (riverH(u + e * 8.0, s, t) - h0) / e;
    n = normalize(vec3(-g.x * 0.03, 1.0, g.y * 0.03));
  } else {
    float e = 0.025;
    float h0 = seaH(m, t);
    n = normalize(vec3(-(seaH(m + vec2(e, 0.0), t) - h0) / e * 0.035, 1.0, (seaH(m + vec2(0.0, e), t) - h0) / e * 0.035));
  }
  n = normalize(mix(vec3(0.0, 1.0, 0.0), n, live));
  vec3 V = normalize(uEye - vWorld);
  float zW = -(uView * vec4(vWorld, 1.0)).z;
  vec2 res = vec2(textureSize(uSceneColor, 0));
  vec2 uv = gl_FragCoord.xy / res;

  // --- Seen through the water: the floor (and what is under the surface: a hull, a pier's legs),
  // bent by the waves and dimmed and tinted by the water it is seen through -- clear over a sandy
  // shelf, then turquoise, then lost in the deep's own colour. The path through the water is the
  // depth behind it less the surface's (the scene grabbed before the water). Only the waves' tilt
  // bends it (a flat surface is seen straight through), and less in the shallows, so the waterline
  // does not swim.
  vec3 nv = (uView * vec4(n.x, 0.0, n.z, 0.0)).xyz;
  vec2 ruv = uv + nv.xy * (0.9 / max(zW, 1.0)) * smoothstep(0.0, 0.1, depth) * uRefract;
  float fz = texture(uSceneND, ruv).w;
  // Never bend something standing in front of the water into it (a bank, a ship's side).
  if (fz < zW) { ruv = uv; fz = texture(uSceneND, uv).w; }
  float path = fz > 0.0 ? max(0.0, fz - zW) : 4.0;
  // The far zooms draw the ground coarse (LOW_RES), which flattens a lake's bowl into a shallow
  // pyramid: there, never let the water read clearer than its true bed (uHeight) allows.
  path = max(path, uCoarse * depth / max(V.y, 0.3));
  vec3 floorCol = texture(uSceneColor, ruv).rgb;
  // Sand under the sea is wet, darker from the very waterline: the shore keeps a crisp edge.
  if (!river) floorCol *= mix(1.0, 0.65, smoothstep(0.0, 0.008, depth));
  // A river carries silt: it closes over its bed sooner than the clear sea over the shelf. Even at
  // the waterline the water tints what it covers.
  vec3 T = exp(-vec3(22.0, 10.0, 7.5) * (path + (river ? 0.012 : 0.06)) * (river ? 4.0 : 1.0)) * uRefract;
  // The water's own colour where it is deep (scattered light): teal over the shelf to indigo.
  float dcol = river ? 0.2 + path * 1.5 : depth;
  vec3 body = mix(srgb(vec3(0.15, 0.45, 0.45)), srgb(vec3(0.07, 0.3, 0.4)), smoothstep(0.0, 0.12, dcol));
  body = mix(body, srgb(vec3(0.05, 0.17, 0.34)), smoothstep(0.12, 0.4, dcol));
  float bm = bodyAt(m, tl);
  if (band) bm = mix(bodyAt(m, 0.0), bm, live);
  body *= bm;
  float sun;
  vec3 Lb = lightAt(vec3(0.0, 1.0, 0.0), vWorld, 1.0, sun);
  body *= Lb * 0.7 + 0.1;
  if (river) {
    // A river is painted, not seen into: bright turquoise by its banks, through cerulean, to a deep
    // blue down the middle of its channel (riverDeep), lit by the light's value only (the warm sun
    // would turn it green) -- in shade it darkens without losing its blue.
    float rd = riverDeep(vExtra.x, vExtra.y, vFlow.x, tl);
    if (band) rd = mix(riverDeep(vExtra.x, vExtra.y, vFlow.x, 0.0), rd, live);
    vec3 rc = mix(srgb(vec3(0.34, 0.64, 0.72)), srgb(vec3(0.2, 0.48, 0.62)), smoothstep(0.1, 0.5, rd));
    rc = mix(rc, srgb(vec3(0.11, 0.3, 0.47)), smoothstep(0.55, 0.95, rd));
    body = rc * (lum(Lb) * 0.7 + 0.1) * bm;
    T *= 0.5;
  }

  // --- The mirror: what stands round the water (the reflected ray marched through the grabbed
  // scene's depth), else the sky and its drifting clouds. Painted, not mirror-sharp: three taps
  // down the screen, and the ripples break it into strokes.
  vec3 R = reflect(-V, n);
  vec3 sky = skyRefl(vWorld, R);
  vec3 hitCol = vec3(0.0);
  float hit = 0.0;
  if (uReflect > 0.0 && R.y > 0.0) {
    float s = 0.0;
    float st = 0.05 + zW * 0.0035;
    for (int i = 0; i < 20; i++) {
      s += st * (1.0 + float(i) * 0.15);
      vec3 Q = vWorld + R * s;
      vec4 cq = uViewProj * vec4(Q, 1.0);
      vec2 q = cq.xy / cq.w * 0.5 + 0.5;
      if (q.x < 0.0 || q.x > 1.0 || q.y < 0.0 || q.y > 1.0) break;
      float zq = -(uView * vec4(Q, 1.0)).z;
      float zs = texture(uSceneND, q).w;
      if (zs > 0.0 && zq > zs + 0.01 && zq - zs < 0.3 + s * 0.4) {
        vec2 dy = vec2(0.0, 2.0 / res.y);
        hitCol = texture(uSceneColor, q).rgb * 0.5 + (texture(uSceneColor, q - dy).rgb + texture(uSceneColor, q + dy).rgb) * 0.25;
        hit = smoothstep(0.0, 0.06, min(min(q.x, 1.0 - q.x), min(q.y, 1.0 - q.y))) * (1.0 - smoothstep(1.5, 4.0, s)) * uReflect;
        break;
      }
    }
  }
  // How much the surface mirrors. Fresnel would make it all but nothing from up here; painted
  // water mirrors its sky and banks plainly, more toward the horizon, broken by the ripples into
  // level strokes (warped so they do not run in ruled lines, drifting) -- along the current on a
  // river.
  float fres = pow(1.0 - max(dot(n, V), 0.0), 3.0);
  vec2 bw = (vec2(mnoise(m, 1.1), mnoise(m + 5.3, 1.1)) - 0.5) * 0.5;
  float brk = brkAt(m, bw, river, vExtra.xy, tl);
  if (band) brk = mix(brkAt(m, bw, river, vExtra.xy, 0.0), brk, live);
  // The deep mirrors less, and its own blue tints what it mirrors: ocean must stay darker than the
  // shelf, coast against open sea read by value. A river a little too: it must read darker than
  // its banks.
  float dk = river ? 0.3 : smoothstep(0.12, 0.4, dcol);
  sky *= mix(vec3(1.0), vec3(0.45, 0.7, 1.1), dk);
  float rw = clamp(mix(mix(river ? 0.08 : 0.15, 0.04, dk), 0.85, fres) * mix(0.3, 1.45, smoothstep(0.25, 0.8, brk)), 0.0, 0.85);
  rw *= mix(1.0, 0.6, dk);

  // --- Glitter: the ripples' small steep facets catch the sun and flash -- short bright marks
  // along the swell, twinkling; in the sun only (not in a cast or a cloud's shadow), in sight only.
  // As the zoom shrinks the cells, each glint keeps about a pixel's width (it widens across its
  // cell) and fades out only once even its length would be a few pixels.
  vec3 H = normalize(V + uSunDir);
  float glit = 0.0;
  float thin = max(1.0, gaa / 0.1);
  float glod = 1.0 - smoothstep(0.3, 0.45, gaa);
  float share = mix(0.35, 0.5, smoothstep(0.1, 0.25, gaa));
  // Only along the swell's crests, in broad drifting patches.
  float crest = river ? 0.6 : smoothstep(0.5, 0.78, mstroke(m - SWELL * t * 0.08, 2.6, 4.5)) * smoothstep(0.3, 0.6, mnoise(m - SWELL * t * 0.03, 0.35));
  if (glod > 0.0 && sun > 0.05 && live > 0.0 && crest > 0.0) {
    vec2 gp = m * 8.0;
    vec2 gi = floor(gp), gfr = fract(gp);
    // Periodic over a wrapping map's width, like every pattern on the map (no seam).
    vec2 gP = vec2(uWrapX > 0.5 ? max(1.0, floor(uMapSize.x * 8.0 + 0.5)) : 1e6, 1e6);
    for (int y = -1; y <= 1; y++) {
      for (int x = -1; x <= 1; x++) {
        vec2 g = vec2(float(x), float(y));
        vec2 cell = mod(gi + g, gP);
        vec2 o = g + hash22(cell) - gfr;
        float ph = t * 1.3 + hash12(cell + 2.9) * 9.0;
        if (hash12(cell + 5.5) > share) continue;
        vec2 tilt = (hash22(cell + floor(ph) * 7.13) - 0.5) * 1.7;
        float spec = pow(max(dot(normalize(vec3(tilt.x, 1.0, -tilt.y)), H), 0.0), 140.0);
        // Each glint its own length, width and slant: not a row of stamped lozenges.
        float sz = mix(0.7, 1.4, hash12(cell + 8.1));
        float wd = thin * mix(0.7, 1.3, hash12(cell + 11.3));
        float ang = (hash12(cell + 4.7) - 0.5) * 0.5;
        vec2 orot = vec2(cos(ang) * o.x - sin(ang) * o.y, sin(ang) * o.x + cos(ang) * o.y);
        float shape = 1.0 - smoothstep(0.1, 0.22, length(orot * vec2(0.38 / sz, 1.7 / wd)));
        glit = max(glit, spec * shape * sin(fract(ph) * PI));
      }
    }
    glit *= sun * live * glod * crest;
  }

  // --- Surf: the swash runs up the beach and draws back (a slow breathing), its foam a lace;
  // rollers come in from offshore and break into it. A river: lace along its banks and flecks
  // carried down mid-channel.
  float lace = laceAt(m, tl);
  if (band) lace = mix(laceAt(m, 0.0), lace, live);
  float sph = mnoise(m, 1.3) * 6.2831;
  float swash = 0.03 + 0.016 * mix(sin(sph), sin(t * 0.8 + sph), live);
  float shore = 1.0 - smoothstep(swash * 0.35, swash, depth);
  // Rollers follow the depth's contours, so only where the floor shelves toward a shore (over a flat
  // or bumpy floor the contours close into rings offshore), and not once they would be hairlines.
  float rph = depth * 70.0 + mnoise(m, 2.0) * 3.0;
  float roll = mix(smoothstep(0.72, 1.0, sin(rph)), smoothstep(0.72, 1.0, sin(rph + t * 1.5)), live);
  roll *= smoothstep(0.012, 0.03, depth) * (1.0 - smoothstep(0.05, 0.11, depth));
  roll *= smoothstep(0.08, 0.2, shelf) * (1.0 - smoothstep(0.8, 1.6, fwidth(rph)));
  // ... and only where the floor keeps rising toward land across the pixel's neighbourhood (a
  // bump's flank rises one way and falls the other: its contours would close into a lens).
  vec2 up = normalize(vec2(gt[1] - gt[3], gt[0] - gt[2]) + vec2(1e-6, 0.0));
  float hA = texture(uHeight, mapUV(m + up * 0.15)).r, hB = texture(uHeight, mapUV(m - up * 0.15)).r;
  roll *= smoothstep(0.0, 0.006, hA - near) * smoothstep(0.0, 0.006, near - hB);
  // The edge of the unexplored is no coast: the blank's floor stands above the sea (world.js BASE),
  // so the water shallows against it, but surf there would say land. The height's lookup is warped
  // up to ~0.45 plot, so the blank is looked for half a plot round.
  float vk = v;
  for (int k = 0; k < 4; k++) {
    vec2 o = vec2(k == 1 ? 0.5 : k == 3 ? -0.5 : 0.0, k == 0 ? 0.5 : k == 2 ? -0.5 : 0.0);
    vk = min(vk, texture(uVis, mapUV(m + o)).r);
  }
  float knownC = smoothstep(0.15, 0.4, vk);
  float foam = clamp(shore * mix(0.6, 1.0, lace) + roll * mix(0.35, 0.8, lace) * 0.6, 0.0, 1.0) * knownC;
  // Where something breaks the surface out on the water (a hull, a pier, a rock offshore): a collar
  // of foam lapping at its waterline. Read from the grabbed scene in a small ring round the pixel
  // (a few pixels, constant in the world): a surface standing at the water's own depth there is
  // something crossing the waterline -- which the floor's depth alone does not show. Not at the
  // beach (the surf has it).
  // On a river, a rock or a pier in the stream (not its own bank: the edge has its lace below).
  float contact = 0.0;
  {
    float rr = (river ? 0.045 : 0.06) / max(zW, 1.0);
    for (int k = 0; k < 6; k++) {
      float a = float(k) * 1.0472;
      float zs = texture(uSceneND, uv + vec2(cos(a) * res.y / res.x, sin(a)) * rr).w;
      contact = max(contact, 1.0 - smoothstep(0.006, 0.03, abs(zs - zW)));
    }
    contact *= (river ? smoothstep(0.08, 0.3, inEdge) : smoothstep(0.03, 0.08, depth)) * knownC;
  }
  float lph = mnoise(m, 4.0) * 6.2831;
  float lap = 0.75 + 0.25 * mix(sin(lph), sin(t * 2.1 + lph), live);
  foam = max(foam, contact * mix(0.5, 1.0, lace) * 0.85 * lap);
  if (river) {
    // Streaks and swirls carried down the current, white water on the outside of the bends
    // (riverFoam); a broken lace along the banks; a collar round what stands in the stream.
    float rfa = ppx / 0.19; // u across a pixel (u counts RIVER_HALFs, world.js)
    float rf = riverFoam(vExtra.x, vExtra.y, vFlow.x, tl, rfa);
    if (band) rf = mix(riverFoam(vExtra.x, vExtra.y, vFlow.x, 0.0, rfa), rf, live);
    float tr = live > 0.0 ? t : 0.0;
    float bk = pnoise(vec2(vExtra.x * 3.0, vExtra.y * 5.0 - tr * 0.3), vec2(4096.0));
    if (band) bk = mix(pnoise(vec2(vExtra.x * 3.0, vExtra.y * 5.0), vec2(4096.0)), bk, live);
    float edgeLace = (1.0 - smoothstep(0.0, 0.1 + 0.08 * bk, inEdge)) * smoothstep(0.35, 0.75, bk) * mix(0.5, 1.0, lace);
    // White water trailing downstream of what stands in the stream (a rock, a reed bed, a pier):
    // looking upstream, a surface at the water's level or just above it, not the bank.
    float wake = 0.0;
    vec3 up3 = vec3(-flow.x, 0.0, flow.y);
    for (int k = 1; k <= 4; k++) {
      vec3 Q = vWorld + up3 * (0.035 * float(k));
      vec4 cq = uViewProj * vec4(Q, 1.0);
      float zq = -(uView * vec4(Q, 1.0)).z;
      float zs = texture(uSceneND, cq.xy / cq.w * 0.5 + 0.5).w;
      float d = zq - zs;
      wake = max(wake, smoothstep(-0.006, 0.0, d) * (1.0 - smoothstep(0.05, 0.1, d)) * (1.1 - 0.2 * float(k)));
    }
    wake *= smoothstep(0.0, 0.2, inEdge) * smoothstep(0.15, 0.55, lace + 0.4 * rf);
    foam = clamp(max(max(rf, edgeLace * 0.75), max(contact * smoothstep(0.2, 0.6, lace) * lap, wake * 0.85)), 0.0, 1.0) * knownC;
  }

  // --- Caustics: the waves gather the sun into a drifting net of light on the shallow floor, seen
  // through the water (so only where the floor is seen: T), found where the ray meets the floor.
  // The cells' edges are warped (light gathered by waves runs in curves, not a crazed glaze), and
  // the net dims once its lines would be hairlines: a close-up detail.
  float cLod = mix(0.3, 1.0, 1.0 - smoothstep(0.006, 0.011, ppx));
  float fadeC = smoothstep(0.004, 0.025, depth) * (1.0 - smoothstep(0.1, 0.25, depth));
  if (!river && uCaustics > 0.0 && live > 0.0 && sun > 0.05 && fadeC > 0.0) {
    vec2 fm = mapOf(vWorld - V * min(path, 0.6));
    fm += (vec2(mnoise(fm, 1.7), mnoise(fm + 4.1, 1.7)) - 0.5) * 0.3;
    vec2 ca = mcells(fm + vec2(t * 0.05, t * 0.031), 4.2);
    vec2 cb = mcells(fm + vec2(-t * 0.036, t * 0.047) + 3.7, 5.6);
    float web = (1.0 - smoothstep(0.0, 0.1, ca.y - ca.x)) * 0.6 + (1.0 - smoothstep(0.0, 0.08, cb.y - cb.x)) * 0.5;
    floorCol *= 1.0 + web * fadeC * sun * live * cLod * 1.1 * uCaustics;
  }
  vec3 foamCol = srgb(vec3(0.9, 0.89, 0.83)) * (Lb * 0.32 + 0.08);
  // A river's foam is a cool white, lit by the light's value (it reads against bright turquoise).
  if (river) foamCol = srgb(vec3(0.9, 0.96, 1.0)) * (lum(Lb) * 0.45 + 0.08);

  // --- Together. What the water adds of its own (its body, the sky, foam, glitter) takes the
  // fog of war and the haze; what is seen in it (the floor, a reflected bank) was grabbed with
  // both already applied.
  vec3 own = body * (1.0 - T) * (1.0 - rw) + sky * (1.0 - hit) * rw;
  vec3 seen = floorCol * T * (1.0 - rw) + hitCol * hit * rw;
  own = mix(own, foamCol, foam) + uSunColor * glit * 0.7;
  seen *= 1.0 - foam;
  vec3 col = atmosphere(fogOfWar(own, v), vWorld) + seen;
  // A river's edge: a dark, wet line where the bank's grass hangs over the water, then the bank
  // (the scene under the ribbon) past the wandering edge, antialiased.
  // vExtra.z is 1 where the ribbon runs out over the sea (Scene.buildRivers): there the sea already
  // drawn shows instead, its colours and its surf, along a broken edge.
  float overSea = vExtra.z + 0.3 * (mnoise(m, 6.0) - 0.5);
  if (river) {
    col *= mix(0.62, 1.0, clamp(smoothstep(0.0, 0.07, inEdge) + foam * 0.6, 0.0, 1.0));
    col = mix(texture(uSceneColor, uv).rgb, col, inWater);
  }
  if (river && overSea > 0.5) discard;
  // The paint filter spares the glitter (it would erase so small a mark); it blooms a little.
  oColor = vec4(col, clamp(glit * 3.0, 0.0, 0.9));
  oND = vec4(normalize((uView * vec4(0.0, 1.0, 0.0, 0.0)).xyz), zW);
  oEmit = vec4(uSunColor * glit * 0.35, 0.0);
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
    float keep = cid <= 0.0 ? 0.0 : clamp(src.a, 0.0, 1.0);
    vec3 wpI = (uInvView * vec4((vUv * 2.0 - 1.0) * uTan * nd.w, -nd.w, 1.0)).xyz;
    // Periodic over a wrapping map's width (x enters at 7 cells a plot), so the camera's wrap across
    // the seam (a jump of the map's width in the world) does not change it.
    float wob = pnoise(vec2(wpI.x * 7.0 + wpI.y * 4.0, wpI.z * 7.0 - wpI.y * 3.0), vec2(uWrapX > 0.5 ? 7.0 * uMapSize.x : 4096.0, 4096.0));
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
      // Periodic over a wrapping map's width, like the ink's wobble.
      bool wr = uWrapX > 0.5;
      vec2 gc = floor(vec2(wp.x, wp.z) * 90.0);
      if (wr) gc.x = mod(gc.x, 90.0 * uMapSize.x);
      float gr = hash12(gc) - 0.5;
      float fib = pnoise(vec2(wp.x * 20.0 + wp.z * 7.0, wp.z * 60.0 - wp.x * 11.0), wr ? vec2(20.0, 11.0) * uMapSize.x : vec2(4096.0)) - 0.5;
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
  // Not under the sea's surface (world.js SEA = 0): the water shows a hull there, or hides it.
  if (vWorld.y < 0.0) discard;
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
