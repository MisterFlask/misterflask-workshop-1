// Tunables of the 3D renderer (docs/R3D.md "Configuration"): every stage of the painterly pipeline
// can be switched off or scaled, alone or through a preset ("painterly", the art-directed look;
// "plain", an ordinary real-time render of the same scene, for comparison). Remembered by this
// browser; `?r3d=plain` (or a JSON object of overrides) sets them for one page.

import { hexLinear } from "./math.js";

const KEY = "ffh.r3d.config";

/** The light of the scene: sun direction (toward the sun) and colours (linear).
 * The map is always in daylight. There is one light, `daylight`: a warm, high sun (about 42
 * degrees up) so shadows stay short (about 1.2x object height) and the shaded side keeps its
 * colour. No dusk, night or time-of-day cycle (owner decision); the old `afternoon`/`day` pair,
 * whose low (27 degrees) sun cast the long near-black shadows F1 reported, is merged into this. */
export const LIGHTING = {
  daylight: {
    sunDir: [-0.65, 0.67, 0.35],
    sun: [hexLinear("#ffe2bc"), 2.8],
    sky: [hexLinear("#7e93bf"), 0.68],
    ground: [hexLinear("#6c5a44"), 0.26],
    haze: hexLinear("#b7b3bc"),
    shadowTint: [0.72, 0.78, 1.02],
    exposure: 1.0,
  },
};

// The painterly preset was moved toward the owner's look target (docs/R3D.md §16: a bright,
// saturated hand-painted strategy map): more saturation, little haze or cloud shadow, a lighter
// paint filter and ink so painted detail stays crisp.
export const PRESETS = {
  painterly: {
    lighting: "daylight",
    shadows: true,
    ssao: 0.75,
    glow: 0.55,
    kuwahara: 0.45,
    kuwaharaRadius: 4,
    sharp: 8,
    aniso: 1,
    outline: 0.4,
    outlineWidth: 1,
    grade: 1,
    saturation: 1.25,
    contrast: 1.08,
    grain: 0.02,
    vignette: 0.1,
    paint: 1,
    bands: 0.55,
    wrapLight: 0.45,
    rim: 0.35,
    stylize: 1,
    clouds: 0.1,
    terrainShadow: 0.45,
    dabs: 1.6,
    brush: 1,
    inkLost: 1,
    inkColour: 0.5,
    reflections: 1,
    refraction: 1,
    caustics: 1,
    haze: 0.12,
    mist: 0.12,
  },
  plain: {
    lighting: "daylight",
    shadows: true,
    ssao: 0,
    glow: 0,
    kuwahara: 0,
    outline: 0,
    grade: 0,
    grain: 0,
    vignette: 0,
    paint: 0,
    bands: 0,
    wrapLight: 0,
    rim: 0,
    stylize: 0,
    clouds: 0,
    terrainShadow: 1,
    dabs: 0,
    brush: 0,
    inkLost: 0,
    inkColour: 0,
    // Seeing into and mirrored in water is ordinary real-time rendering, not stylization.
    reflections: 1,
    refraction: 1,
    caustics: 1,
    haze: 0,
    mist: 0,
  },
};

export const DEFAULTS = {
  preset: "painterly",
  ...PRESETS.painterly,
  fogWar: 1,
  borders: 1,
  grid: 0.12,
  renderScale: 1,
  fxaa: 1,
  autoQuality: 1,
  animate: true,
  wind: 1,
  particles: true,
  emissive: 2.2,
  aoRadius: 0.22,
  pitch: 50,
  fov: 24,
};

/** The switches the debug panel shows: [key, label, kind, min, max, step]. */
export const PANEL = [
  ["shadows", "Shadows", "bool"],
  ["terrainShadow", "  shadows cast by the land", "range", 0, 1, 0.05],
  ["ssao", "Ambient occlusion", "range", 0, 1, 0.05],
  ["glow", "Glow (emissive)", "range", 0, 1.5, 0.05],
  ["kuwahara", "Painterly filter (Kuwahara)", "range", 0, 1, 0.05],
  ["kuwaharaRadius", "  filter radius (px)", "range", 2, 8, 1],
  ["outline", "Ink outlines", "range", 0, 1, 0.05],
  ["outlineWidth", "  outline width (px)", "range", 0.5, 2.5, 0.25],
  ["paint", "Pigment variation", "range", 0, 1.5, 0.05],
  ["dabs", "Brush dabs", "range", 0, 2.5, 0.05],
  ["brush", "Brushwork on objects (leaves, strokes)", "range", 0, 1.5, 0.05],
  ["inkLost", "  lost-and-found ink", "range", 0, 1, 0.05],
  ["inkColour", "  coloured ink", "range", 0, 1, 0.05],
  ["bands", "Light banding", "range", 0, 1, 0.05],
  ["stylize", "Coloured shadows", "range", 0, 1, 0.05],
  ["rim", "Rim light", "range", 0, 1, 0.05],
  ["clouds", "Cloud shadows", "range", 0, 1, 0.05],
  ["reflections", "Water reflections (banks, ships)", "range", 0, 1, 0.05],
  ["refraction", "Water transparency (shallows)", "range", 0, 1, 0.05],
  ["caustics", "  caustics on the shallow floor", "range", 0, 1, 0.05],
  ["haze", "Aerial haze", "range", 0, 1, 0.05],
  ["mist", "Valley mist", "range", 0, 1, 0.05],
  ["grade", "Colour grade", "range", 0, 1, 0.05],
  ["grain", "Paper grain", "range", 0, 0.1, 0.005],
  ["vignette", "Vignette", "range", 0, 0.5, 0.02],
  ["borders", "Borders", "range", 0, 1, 0.05],
  ["grid", "Grid", "range", 0, 1, 0.05],
  ["fogWar", "Fog of war shading", "range", 0, 1, 0.05],
  ["particles", "Particles", "bool"],
  ["animate", "Animate (wind, water, fire)", "bool"],
  ["renderScale", "Render scale", "range", 0.5, 1.5, 0.25],
  ["fxaa", "Anti-aliasing (FXAA)", "bool"],
  ["autoQuality", "Auto quality (drop paint/scale when slow)", "bool"],
];

export function loadConfig() {
  let saved = {};
  try {
    saved = JSON.parse(globalThis.localStorage?.getItem(KEY) ?? "{}") ?? {};
  } catch {
    saved = {};
  }
  // Keys added since the config was saved come from the preset it was saved under, not from the
  // defaults (a plain look saved before the brushwork existed must not gain brush marks).
  const base = PRESETS[saved.preset] ? { ...DEFAULTS, ...PRESETS[saved.preset] } : DEFAULTS;
  const cfg = { ...base, ...saved };
  // The map is always in daylight; a light no longer offered (a saved "dusk", "afternoon" or
  // "day") falls back to the single "daylight" look.
  if (!LIGHTING[cfg.lighting]) cfg.lighting = DEFAULTS.lighting;
  // A page-only override: ?r3d=plain|painterly or ?r3d={"kuwahara":0}.
  try {
    const q = new URLSearchParams(globalThis.location?.search ?? "").get("r3d");
    if (q) {
      if (PRESETS[q]) Object.assign(cfg, PRESETS[q], { preset: q });
      else Object.assign(cfg, JSON.parse(q));
    }
  } catch {
    /* a malformed override is ignored */
  }
  return cfg;
}

export function saveConfig(cfg) {
  try {
    globalThis.localStorage?.setItem(KEY, JSON.stringify(cfg));
  } catch {
    /* storage may be unavailable */
  }
}

/** The config with preset `name` applied. */
export function withPreset(cfg, name) {
  return { ...cfg, ...PRESETS[name], preset: name };
}
