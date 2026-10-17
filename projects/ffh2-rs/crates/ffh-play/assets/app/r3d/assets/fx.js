// fx.js: the particles of the 3D map -- the catalog's `emitters` entry (docs/R3D.md "Asset
// recipes"). Particles are soft glowing points that rise and drift (scene.js setParticles,
// shaders.js PARTICLE_*): embers, sparks, motes, wisps, fireflies; an emitter with `smoke: true`
// gives soft grey puffs instead that swell and lean with the wind. They are spent sparingly, a few points per plot at most, where they
// tell something: the hells smoulder, flames and volcanoes throw sparks, the ancient forests and
// marshes hold a few drifting lights, tormented souls rise as pale wisps, and cities glow by their
// nature (warm sparks for most, red embers for the infernal, arcane motes for the Amurites).
// Mana nodes, lairs and landmarks carry their own emitters (props.js, the improvement's `emit`).
//
// An emitter: {dx, dy, dz (offsets from the plot or city centre; x east, z south, y up), color,
// size (world units), rise (cycles per second), count, spread (radius), smoke?}.

import { extendCatalog } from "./catalog.js";
import { KIND, FEAT, GROUND, isHell } from "../world.js";

/** The particles of one plot. */
function plotEmitters(ctx) {
  const { r, kind, feat, ground } = ctx;
  if (kind === KIND.VOID) return [];
  const out = [];
  const water = kind === KIND.OCEAN || kind === KIND.COAST || kind === KIND.ICE;
  if (feat === FEAT.VOLCANO) {
    // Sparks thrown up from the crater (the summit stands ~0.18 over the plot centre).
    out.push({ dy: 0.2, color: "#ff7a2a", size: 0.014, rise: 0.32, count: 6, spread: 0.06 });
    out.push({ dy: 0.24, color: "#ffb060", size: 0.009, rise: 0.5, count: 3, spread: 0.04 });
    // A slow plume of ash over the crater.
    out.push({ dy: 0.26, color: "#6e666c", size: 0.05, rise: 0.09, count: 6, spread: 0.05, smoke: true });
    return out;
  }
  if (feat === FEAT.FLAMES) {
    out.push({ dy: 0.04, color: "#ff8a30", size: 0.012, rise: 0.45, count: 5, spread: 0.3 });
    // A thin pall of smoke drifting off the burning ground.
    out.push({ dy: 0.14, color: "#5e5654", size: 0.04, rise: 0.1, count: 3, spread: 0.22, smoke: true });
    return out;
  }
  if (!water && isHell(ground)) {
    // The hells smoulder: a few embers drift up from the cracks (not on every plot).
    if (r() < 0.45) out.push({ dy: 0.01, color: r() < 0.5 ? "#ff5a1a" : "#ff7a30", size: 0.009, rise: r.range(0.14, 0.22), count: 2 + (r() < 0.4 ? 1 : 0), spread: 0.4 });
  }
  if (feat === FEAT.SOULS) out.push({ dy: 0.02, color: "#a8e0d8", size: 0.016, rise: 0.11, count: 3, spread: 0.3 });
  else if (feat === FEAT.FALLOUT) out.push({ dy: 0.02, color: "#a0d040", size: 0.01, rise: 0.1, count: 2, spread: 0.25 });
  else if (feat === FEAT.ANCIENT) {
    // Pale motes among the old trees (few: the light pools already glow there).
    if (r() < 0.5) out.push({ dy: 0.12, color: "#c8f0a8", size: 0.01, rise: 0.07, count: 2, spread: 0.35 });
  } else if (ground === GROUND.MARSH && !water && feat === FEAT.NONE) {
    // A will-o'-the-wisp now and then.
    if (r() < 0.25) out.push({ dy: 0.03, color: "#d0f0a0", size: 0.008, rise: 0.06, count: 1, spread: 0.3 });
  }
  return out;
}

/** The particles of a city, by its family (architecture.js adds its braziers and towers). */
function cityEmitters(c) {
  const fam = String(c.family ?? "human");
  const civ = String(c.civ ?? "");
  const n = Math.min(3, 1 + Math.floor((c.size ?? 1) / 4));
  if (fam.startsWith("infernal")) return [{ dy: 0.08, color: "#ff4a1a", size: 0.012, rise: 0.28, count: 3 + n, spread: 0.36 }];
  if (civ === "CIVILIZATION_AMURITES" || fam.includes("arcane")) return [{ dy: 0.16, color: "#b49aff", size: 0.011, rise: 0.14, count: 2 + n, spread: 0.32 }];
  if (fam.startsWith("elven")) return [{ dy: 0.14, color: fam.includes("dark") ? "#b0b8ff" : "#e0f0a0", size: 0.008, rise: 0.06, count: 2 + n, spread: 0.4 }];
  if (fam.startsWith("dwarven")) return [{ dy: 0.12, color: "#ff9a40", size: 0.008, rise: 0.5, count: 2 + n, spread: 0.2 }];
  return [{ dy: 0.14, color: "#ffc070", size: 0.008, rise: 0.25, count: 1 + n, spread: 0.3 }];
}

extendCatalog({
  emitters: (kind, ctx) => (kind === "city" ? cityEmitters(ctx) : kind === "plot" ? plotEmitters(ctx) : []),
});
