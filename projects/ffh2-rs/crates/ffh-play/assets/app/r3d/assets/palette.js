// The 3D world's palette (docs/R3D.md "Art direction"): every recipe takes its colours from here, so
// trees, towns and armies of different authors share one painted world. sRGB hex; `pal(name)` gives
// the linear colour the mesh builder stores. Late-afternoon key: warm light, earthy mid-tones,
// cool deep shadows; saturation is spent on owner colours, magic and fire, not on the ground.

import { hexLinear, vlerp } from "../math.js";

export const PAL = {
  // Foliage
  oakDark: "#2c4424",
  oak: "#46612e",
  oakLight: "#76873a",
  oakAutumn: "#94652a",
  pineDark: "#1b3329",
  pine: "#2a4a3a",
  pineLight: "#456447",
  ancientDark: "#173d37",
  ancient: "#2a6152",
  ancientLight: "#86ad6e",
  ancientGlow: "#c8f0a4",
  jungleDark: "#1a3c22",
  jungle: "#2c5a2a",
  jungleLight: "#5c8a34",
  bush: "#3a5228",
  reed: "#7d7a45",
  dry: "#8a7a48",
  // Bark and wood
  barkOak: "#4a392b",
  barkPine: "#4b3328",
  barkBirch: "#c9c1b2",
  barkAncient: "#6a5d52",
  barkDead: "#3b3230",
  char: "#1d1918",
  timber: "#5a3e2a",
  timberLight: "#80603f",
  plank: "#7a5a3c",
  // Stone and earth
  stoneLight: "#a39a8e",
  stone: "#7d766e",
  stoneDark: "#4e4a48",
  sandstone: "#b0956e",
  sandstoneDark: "#86704f",
  basalt: "#3a363a",
  obsidian: "#1a161e",
  marble: "#d8d2c6",
  earth: "#5e4834",
  mud: "#4a3a2c",
  // Walls and roofs
  plaster: "#d8ccb0",
  plasterShade: "#bfae8c",
  roofTile: "#8a3d2b",
  roofTileDark: "#6a2c22",
  roofSlate: "#4a5260",
  roofThatch: "#a08a50",
  roofCopper: "#4b7a68",
  roofDwarf: "#3b3b42",
  roofInfernal: "#2b1516",
  roofHide: "#8a6a4a",
  // Metal
  iron: "#5a5e66",
  steel: "#8f97a0",
  gold: "#c9a043",
  bronze: "#9a6a3a",
  // Cloth, leather, fur
  linen: "#c8b898",
  leather: "#6a4a30",
  leatherDark: "#3e2c20",
  fur: "#7a6a58",
  rope: "#9a8460",
  // Skin
  skinHuman: "#c99b79",
  skinElf: "#dcbca2",
  skinDwarf: "#b98a69",
  skinOrc: "#6b7b49",
  skinUndead: "#aaa898",
  skinDemon: "#7a2a22",
  bone: "#d9cfb6",
  // Light and magic
  fire: "#ff8a30",
  ember: "#ff5a1a",
  window: "#ffbf6a",
  lava: "#ff4a10",
  hellGlow: "#ff3010",
  holy: "#ffe9a8",
  arcane: "#9a7aff",
  water: "#3f7f8f",
};

/** Mana schools -> glow colour (FfH2's sixteen spheres and the plain node). */
export const MANA = {
  AIR: "#c4e8ff",
  BODY: "#ff8a7a",
  CHAOS: "#ff4aa8",
  DEATH: "#7a4aff",
  EARTH: "#c08a3a",
  ENCHANTMENT: "#b08aff",
  ENTROPY: "#a020c8",
  FIRE: "#ff5a1a",
  ICE: "#9ee6ff",
  LAW: "#ffe9a0",
  LIFE: "#8aff9a",
  METAMAGIC: "#40ffd8",
  MIND: "#ffa0e8",
  NATURE: "#60d860",
  SHADOW: "#6a4aa8",
  SPIRIT: "#e8d0ff",
  SUN: "#ffd040",
  WATER: "#3a9aff",
  DIMENSIONAL: "#8a6aff",
  CREATION: "#fff0c0",
  FORCE: "#ff6aff",
  "": "#c0b0ff",
};

/** The linear colour of palette entry `name` (or a hex string). */
export function pal(name) {
  return hexLinear(PAL[name] ?? name);
}

/** A colour nudged by the generator: value and slight hue drift (`amt` ~0.06). */
export function jitter(c, r, amt = 0.06) {
  const k = 1 + (r() - 0.5) * 2 * amt;
  const t = (r() - 0.5) * amt;
  return [c[0] * k * (1 + t), c[1] * k, c[2] * k * (1 - t)];
}

/** a toward b. */
export function mixc(a, b, t) {
  return vlerp(typeof a === "string" ? pal(a) : a, typeof b === "string" ? pal(b) : b, t);
}
