// The isometric map (docs/ISO.md): Fall from Heaven as a bright "HD pixel art" diorama, in the M7
// art of tools/m7 (sprites drawn by an image model and brought to one contract by code).
//
// It is one of MapView's renderers (map.js), chosen in Tools → Options or with `?renderer=iso`.
// MapView keeps its camera, input, overlays and state, and delegates draw(), screenToMap(),
// screenOf(), panDelta() and zooms() to this class; the data is the same `/api/map` + `/api/view`
// the other renderers read.
//
// Geometry: a plot (x, y) has its centre at map point (x + 0.5, y + 0.5); the camera centre
// (cam.cx, cam.cy) is a map point and cam.ts the tile width in CSS px (its height is ts / 2). North
// is up-and-right, east down-and-right (Civ4's isometric convention). The art is made for a tile
// NATIVE px wide (the M7 contract, tools/m7/README.md): sprites are drawn at ts / NATIVE, by nearest
// neighbour at whole device-pixel factors, else smoothed once per zoom and cached.
//
// Draw order (the contract): the tiles back to front, water ones WATER_DROP art px lower so the
// land's earth skirts show as banks, each with its overlay (hills, farm ...); the ground marks
// (rivers, roads, borders, the selection); then everything that stands up -- features, buildings
// and cities, resources, units -- sorted by their plot's depth and, on one plot, in that order. A
// unit mostly hidden by something in front of it is drawn again, faded, over it. Banners, contact
// shadows and the selection highlight are drawn here, not baked into sprites, so the owner's colour
// stays a runtime choice.

import { S, me, tname, isBarbarian } from "./state.js";
import * as T from "./tiles.js";
import { unitMoves } from "./stacks.js";
import { SPRITES } from "./iso/sprites.js";

/** A tile's width in art pixels: the M7 contract (tiles 192 x 128, the diamond in rows 0-95). */
const NATIVE = 192;
const ISO_ZOOMS = [48, 64, 96, 128, 192, 384];
/** The zoom the renderer opens at: half the art's size, about ten tiles across a laptop screen. */
export const ISO_DEFAULT_ZOOM = 96;
/** Water tiles are drawn this many art px lower, so the land's earth skirts show as banks. */
const WATER_DROP = 8;
/** A unit's canvas bottom-centre stands this many art px below its tile's centre; a garrison's at
 *  (GATE_DX, GATE_DY) from it, at the city's front-right gate. */
const UNIT_DY = 6;
const GATE_DX = 40;
const GATE_DY = 26;

/** The move slide (as artmap.js, docs/ART.md 4.5): about 8 tiles a second, capped; a longer move
 *  jumps. */
const SLIDE_MS_PER_TILE = 125;
const SLIDE_MAX_MS = 400;
const SLIDE_MAX_TILES = 4;

/** TerrainType name -> tile sprite. Each base terrain may have a `_b` variant, picked per plot. */
const TERRAIN_TILE = {
  TERRAIN_GRASS: "tile_grass",
  TERRAIN_PLAINS: "tile_plains",
  TERRAIN_DESERT: "tile_desert",
  TERRAIN_TUNDRA: "tile_tundra",
  TERRAIN_SNOW: "tile_snow",
  TERRAIN_MARSH: "tile_marsh",
  TERRAIN_COAST: "tile_water",
  TERRAIN_SHALLOWS: "tile_water",
  TERRAIN_OCEAN: "tile_ocean",
  TERRAIN_BURNING_SANDS: "tile_burning_sands",
  TERRAIN_BROKEN_LANDS: "tile_broken_lands",
  TERRAIN_FIELDS_OF_PERDITION: "tile_fields_of_perdition",
};

/** Where a tile sprite is missing, the nearest one that exists. */
const TILE_FALLBACK = { tile_ocean: "tile_water", tile_marsh: "tile_grass", tile_tundra: "tile_plains" };

/** The terrains whose hills are dry, snowy or hellish (the hills overlay); the rest are green. */
const DRY = new Set(["TERRAIN_PLAINS", "TERRAIN_DESERT", "TERRAIN_BURNING_SANDS", "TERRAIN_BROKEN_LANDS", "TERRAIN_FIELDS_OF_PERDITION"]);
const COLD = new Set(["TERRAIN_TUNDRA", "TERRAIN_SNOW"]);
const HELL = new Set(["TERRAIN_BURNING_SANDS", "TERRAIN_BROKEN_LANDS", "TERRAIN_FIELDS_OF_PERDITION"]);

/** FeatureType name -> sprite: an overlay inside the tile (`ovl_`) or a feature standing on it
 *  (`feat_`), with fallbacks. Unlisted features (blizzards, fallout, the scenario walls) draw
 *  nothing: the tile alone shows. */
const FEATURE_SPRITES = {
  FEATURE_FOREST: ["feat_forest"],
  FEATURE_FOREST_NEW: ["feat_forest_new", "feat_forest"],
  FEATURE_FOREST_ANCIENT: ["feat_forest_ancient", "feat_forest"],
  FEATURE_FOREST_BURNT: ["feat_forest_burnt"],
  FEATURE_JUNGLE: ["feat_jungle", "feat_forest"],
  FEATURE_SCRUB: ["feat_scrub"],
  FEATURE_FLAMES: ["feat_flames"],
  FEATURE_TORMENTED_SOULS: ["feat_tormented_souls"],
  FEATURE_OASIS: ["ovl_oasis"],
  FEATURE_FLOOD_PLAINS: ["ovl_flood_plains"],
  FEATURE_ICE: ["ovl_ice"],
};

/** An ImprovementType name -> its sprites (first that exists), by kind; the exact improvement
 *  shows on hover. Flat works (fields, pastures, quarries) are overlays inside the tile; the rest
 *  stand on it. */
function improvementSprites(t) {
  if (/MANA_|STANDING_STONES|RING_OF_CARCER|PORTAL|MIRROR_OF_HEAVEN|JUNGLE_ALTAR|PYRE_OF|POOL_OF_TEARS|BRADELINES_WELL/.test(t)) return ["feat_mana_node", "bld_ruins"];
  if (/YGGDRASIL|SEVEN_PINES/.test(t)) return ["feat_forest_ancient", "feat_forest"];
  if (/FARM|MUSHROOMS/.test(t)) return ["ovl_farm"];
  if (/PASTURE/.test(t)) return ["ovl_pasture", "ovl_farm"];
  if (/PLANTATION/.test(t)) return ["ovl_plantation", "ovl_farm"];
  if (/WINERY/.test(t)) return ["ovl_winery", "ovl_farm"];
  if (/QUARRY/.test(t)) return ["ovl_quarry", "bld_mine"];
  if (/MINE/.test(t)) return ["bld_mine"];
  if (/LUMBERMILL/.test(t)) return ["bld_lumbermill", "bld_workshop"];
  if (/WINDMILL/.test(t)) return ["bld_windmill", "bld_workshop"];
  if (/WORKSHOP/.test(t)) return ["bld_workshop", "bld_hamlet"];
  if (/CAMP/.test(t)) return ["bld_camp"];
  if (/COTTAGE|HAMLET/.test(t)) return ["bld_cottage", "bld_hamlet"];
  if (/VILLAGE|TOWN|GOODY_HUT/.test(t)) return ["bld_hamlet", "bld_cottage"];
  if (/GOBLIN_FORT/.test(t)) return ["bld_goblin_fort", "bld_fort"];
  if (/FORT|CASTLE|CITADEL|ENCLAVE|GUARDIAN/.test(t)) return ["bld_fort"];
  if (/TOWER|WARNING_POST/.test(t)) return ["bld_watchtower", "bld_fort"];
  if (/WHALING/.test(t)) return ["feat_whaling_boats", "feat_fishing_boats"];
  if (/FISHING/.test(t)) return ["feat_fishing_boats"];
  if (/PIRATE|SHIP_WRECK/.test(t)) return ["bld_harbor", "bld_ruins"];
  if (/GRAVEYARD|NECROTOTEM|BONES|SKELETON/.test(t)) return ["bld_graveyard", "bld_ruins"];
  if (/BARROW|DUNGEON|DEN$|CAGE|SNAKE_PILLAR|HELLFIRE|ODIOS/.test(t)) return ["bld_lair", "bld_ruins"];
  if (/WORKED|SMOKE/.test(t)) return [];
  return ["bld_ruins", "bld_lair"]; // ruins, sepulchers, tombs and the other unique landmarks
}

/** A BonusType name -> its resource prop (the specific kind shows on hover). */
function resourceSprite(t) {
  if (/MANA/.test(t)) return "res_mana";
  if (/WHEAT|CORN|RICE/.test(t)) return "res_wheat";
  if (/COW|PIG/.test(t)) return "res_cow";
  if (/SHEEP/.test(t)) return "res_sheep";
  if (/HORSE|NIGHTMARE/.test(t)) return "res_horse";
  if (/DEER|FUR|IVORY|GULAGARM/.test(t)) return "res_deer";
  if (/WHALE/.test(t)) return "res_whale";
  if (/FISH|CLAM|CRAB|PEARL/.test(t)) return "res_fish";
  if (/IRON|COPPER|GUNPOWDER/.test(t)) return "res_iron";
  if (/GOLD/.test(t)) return "res_gold";
  if (/GEMS|JEWELS/.test(t)) return "res_gems";
  if (/MARBLE/.test(t)) return "res_marble";
  if (/MITHRIL|SHEUT/.test(t)) return "res_mithril";
  if (/WINE/.test(t)) return "res_wine";
  if (/BANANA|SUGAR|FRUIT/.test(t)) return "res_fruit";
  return "res_goods"; // silk, dye, incense, cotton, ale, fine clothes, reagents, toads, razorweed
}

/** A city's settlement sprite by size, then the others; the 2x2 city (`bld_city`) last, drawn at
 *  half size since a city holds one plot. */
function citySprites(size) {
  const tiers = ["bld_city_village", "bld_city_town", "bld_city_castle"];
  const i = size <= 3 ? 0 : size <= 8 ? 1 : 2;
  return [tiers[i], ...tiers.filter((_, j) => j !== i).sort((a, b) => Math.abs(tiers.indexOf(a) - i) - Math.abs(tiers.indexOf(b) - i)), "bld_city"];
}

/** A unit type or class (its name without UNIT_ / UNITCLASS_, lower case) -> the sprites that
 *  stand in for it, before its combat type's (UNIT_COMBAT) and domain's. A sprite named after the
 *  exact type, its class, or `<civilization>_<type>` (unit_ljosalfar_archer) needs no entry here. */
const UNIT_ALIASES = {
  beastman: ["warrior"], bloodpet: ["warrior"], berserker: ["axeman"], immortal: ["champion"],
  dwarven_soldier_runes: ["axeman"], longbowman: ["archer"], firebow: ["archer"], arquebus: ["crossbowman", "archer"],
  marksman: ["crossbowman", "archer"], nightwatch: ["crossbowman", "archer"], crossbowman: ["archer"],
  horse_archer: ["horseman"], chariot: ["horseman"], knight: ["horseman"], frostling_wolf_rider: ["horseman"],
  shadowrider: ["knight"], royal_guard: ["knight"], war_elephant: ["knight"], war_machine: ["catapult"],
  cannon: ["catapult"], battering_ram: ["catapult"], galley: ["trireme"], arcane_barge: ["trireme"],
  caravel: ["frigate", "trireme"], galleon: ["frigate", "trireme"], man_o_war: ["frigate", "trireme"],
  queen_of_the_line: ["frigate", "trireme"], privateer: ["frigate", "trireme"], black_wind: ["frigate", "trireme"],
  sailors_dirge: ["frigate", "trireme"], frigate: ["trireme"], workboat: ["trireme"],
  ranger: ["hunter"], beastmaster: ["hunter"], fawn: ["hunter"], satyr: ["hunter"], adventurer: ["scout"],
  shadow: ["assassin"], revelers: ["assassin"], frostling: ["goblin"], goblin_scorpion_clan: ["goblin"],
  lizardman_assassin: ["lizardman"], lizardman_beastmaster: ["lizardman"], lizardman_druid: ["lizardman"],
  lizardman_ranger: ["lizardman"], imp: ["manes"], archmage: ["mage"], adept: ["mage"],
  monk: ["disciple"], crusader: ["paladin"], lightbringer: ["paladin"], einherjar: ["paladin"],
  slave: ["worker"], mud_golem: ["golem"], prophet: ["great_person"], artist: ["great_person"],
  merchant: ["great_person"], engineer: ["great_person"], scientist: ["great_person"], commander: ["great_person"],
  wolf_pack: ["wolf"], hellhound: ["wolf"], polar_bear: ["bear"], gorilla: ["bear"], lion_pride: ["lion"],
  tiger: ["lion"], sand_lion: ["lion"], baby_spider: ["giant_spider"], scorpion: ["giant_spider"],
  spiderkin: ["giant_spider"], hawk: ["griffon"], eye: ["griffon"], elephant: ["war_elephant"],
  greater_werewolf: ["werewolf"], ravenous_werewolf: ["werewolf"], balor: ["demon"], chaos_marauder: ["demon"],
  succubus: ["demon"], pit_beast: ["demon"], tar_demon: ["demon"], meshabber: ["demon"], manes: ["demon"],
  lightning_elemental: ["air_elemental"], ice_elemental: ["water_elemental"], fireball: ["fire_elemental"],
  meteor: ["fire_elemental"], iron_golem: ["golem"], clockwork_golem: ["golem"], bone_golem: ["golem"],
  flesh_golem: ["golem"], mithril_golem: ["golem"], nullstone_golem: ["golem"], wood_golem: ["golem"],
  seraph: ["angel"], valkyrie: ["angel"], repentant_angel: ["angel"], angel_of_death: ["angel"], ophanim: ["angel"],
  guardian_vines: ["treant"], kraken: ["sea_serpent"], leviathan: ["sea_serpent"], giant_tortoise: ["sea_serpent"],
  wraith: ["spectre"], shade: ["spectre"], ghost: ["spectre"], mistform: ["spectre"], severed_soul: ["spectre"],
  divided_soul: ["spectre"], eidolon: ["spectre"], diseased_corpse: ["skeleton"], pyre_zombie: ["skeleton"],
  drown: ["skeleton"], ogre: ["ogre_embers"], ogre_warchief: ["ogre_embers"], stoneskin_ogre: ["ogre_embers"],
  minotaur: ["ogre_embers"],
};

/** The last resorts by combat type and domain. */
const COMBAT_SPRITES = {
  UNITCOMBAT_MELEE: "swordsman", UNITCOMBAT_ARCHER: "archer", UNITCOMBAT_MOUNTED: "horseman", UNITCOMBAT_RECON: "scout",
  UNITCOMBAT_SIEGE: "catapult", UNITCOMBAT_ADEPT: "mage", UNITCOMBAT_DISCIPLE: "disciple", UNITCOMBAT_ANIMAL: "wolf",
  UNITCOMBAT_BEAST: "werewolf", UNITCOMBAT_NAVAL: "trireme",
};
const DOMAIN_SPRITES = { DOMAIN_SEA: "trireme", DOMAIN_AIR: "griffon" };

/** The owner banner (compose.py draw_banner): a pole 96 art px tall with a swallow-tailed flag in
 *  the owner's colour and a gold charge, its base at the canvas's (4, 95). */
const BANNER_W = 40;
const BANNER_H = 96;

function ownerColor(p) {
  return T.playerColor(p, isBarbarian(p));
}

function shortName(type, prefix) {
  return type?.startsWith(prefix) ? type.slice(prefix.length).toLowerCase() : null;
}

function canvas(w, h) {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  return c;
}

/** `src` scaled by `k` with smoothing, halving first so a big reduction keeps its detail and
 *  enlarging by nearest neighbour first (to the next whole factor) so a fractional enlargement
 *  stays crisp; `solid` makes the edge alpha all-or-nothing (a tile's diamond must cover its
 *  neighbours' skirts). */
function scaled(src, k, solid) {
  const w = Math.max(1, Math.round(src.width * k));
  const h = Math.max(1, Math.round(src.height * k));
  let cur = src;
  if (k > 1) {
    const n = Math.ceil(k);
    cur = canvas(src.width * n, src.height * n);
    const ug = cur.getContext("2d");
    ug.imageSmoothingEnabled = false;
    ug.drawImage(src, 0, 0, cur.width, cur.height);
  }
  while (cur.width / 2 >= w && cur.height / 2 >= h) {
    const half = canvas(Math.ceil(cur.width / 2), Math.ceil(cur.height / 2));
    const hg = half.getContext("2d");
    hg.imageSmoothingQuality = "high";
    hg.drawImage(cur, 0, 0, half.width, half.height);
    cur = half;
  }
  const out = canvas(w, h);
  const g = out.getContext("2d");
  g.imageSmoothingQuality = "high";
  g.drawImage(cur, 0, 0, w, h);
  if (solid) {
    const d = g.getImageData(0, 0, w, h);
    for (let i = 3; i < d.data.length; i += 4) d.data[i] = d.data[i] >= 100 ? 255 : 0;
    g.putImageData(d, 0, 0);
  }
  return out;
}

/** `src` mirrored and/or in the fog's tones (a remembered plot out of sight): colours move 45%
 *  toward their grey and darken, with a slight cool tint, so remembered grassland still reads as
 *  grassland, not plains. */
function variantOf(src, flip, fog) {
  const c = canvas(src.width, src.height);
  const g = c.getContext("2d", fog ? { willReadFrequently: true } : undefined);
  if (flip) {
    g.translate(src.width, 0);
    g.scale(-1, 1);
  }
  g.drawImage(src, 0, 0);
  if (fog) {
    const d = g.getImageData(0, 0, c.width, c.height);
    const p = d.data;
    for (let i = 0; i < p.length; i += 4) {
      const l = 0.45 * (0.3 * p[i] + 0.59 * p[i + 1] + 0.11 * p[i + 2]);
      p[i] = (p[i] * 0.55 + l) * 0.62;
      p[i + 1] = (p[i + 1] * 0.55 + l) * 0.62 + 3;
      p[i + 2] = (p[i + 2] * 0.55 + l) * 0.62 + 10;
    }
    g.putImageData(d, 0, 0);
  }
  return c;
}

/** A CSS colour as [r, g, b] (memoised). */
const RGB = new Map();
function rgbOf(css) {
  let v = RGB.get(css);
  if (!v) {
    const g = canvas(1, 1).getContext("2d", { willReadFrequently: true });
    g.fillStyle = css;
    g.fillRect(0, 0, 1, 1);
    v = [...g.getImageData(0, 0, 1, 1).data.slice(0, 3)];
    RGB.set(css, v);
  }
  return v;
}

function toHsl(r, g, b) {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const h = max === r ? (g - b) / d + (g < b ? 6 : 0) : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [h * 60, s, l];
}

function fromHsl(h, s, l) {
  const k = (n) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n) => l - a * Math.max(-1, Math.min(k(n) - 3, 9 - k(n), 1));
  return [Math.round(f(0) * 255), Math.round(f(8) * 255), Math.round(f(4) * 255)];
}

/** The sprites: the PNGs of `iso/` (listed in iso/sprites.js, loaded on first use) and the ones
 *  drawn here (banners), each at the device scales of the frame. A scale that is a whole number
 *  draws the native image (or its variant) by nearest neighbour; others are scaled once and cached,
 *  and the cache is dropped when the zoom changes (`frame`). */
class SpriteStore {
  constructor(onload) {
    this.onload = onload;
    this.images = new Map();
    this.drawn = new Map();
    this.cache = new Map();
    this.cacheScale = null;
    this.alpha = new Map();
    this.base = new Map();
  }

  /** A new frame at device scale `k`: a new zoom drops the cache. */
  frame(k) {
    if (k === this.cacheScale) return;
    this.cache.clear();
    this.cacheScale = k;
  }

  has(key) {
    return Object.hasOwn(SPRITES, key) || this.drawn.has(key);
  }

  /** The sprite's native width and height, and its opaque box [x0, y0, x1, y1] (art px). */
  info(key) {
    const s = SPRITES[this.base.get(key) ?? key];
    if (s) return s;
    const c = this.drawn.get(key);
    return c ? { w: c.width, h: c.height, box: [0, 0, c.width, c.height] } : null;
  }

  put(key, c) {
    this.drawn.set(key, c);
  }

  /** `key` with the art's stock blue (the settlements' roofs, spires and flags) turned to the
   *  owner's colour `css`, as the drawn sprite `key@css`; the plain key until it has loaded. */
  owned(key, css) {
    const id = `${key}@${css}`;
    if (this.drawn.has(id)) return id;
    const src = Object.hasOwn(SPRITES, key) ? this.source(key) : null;
    if (!src) return key;
    const c = canvas(src.width, src.height);
    const g = c.getContext("2d", { willReadFrequently: true });
    g.drawImage(src, 0, 0);
    const [oh, os, ol] = toHsl(...rgbOf(css));
    const grey = os < 0.2;
    const d = g.getImageData(0, 0, c.width, c.height);
    const p = d.data;
    for (let i = 0; i < p.length; i += 4) {
      if (p[i + 3] === 0) continue;
      const [h, s, l] = toHsl(p[i], p[i + 1], p[i + 2]);
      if (h < 195 || h > 252 || s < 0.35) continue;
      const rgb = fromHsl(oh, grey ? os : s * 0.4 + os * 0.6, grey ? l * 0.5 + ol * 0.5 : l);
      p[i] = rgb[0];
      p[i + 1] = rgb[1];
      p[i + 2] = rgb[2];
    }
    g.putImageData(d, 0, 0);
    this.drawn.set(id, c);
    this.base.set(id, key);
    return id;
  }

  /** Whether art pixel (u, v) of sprite `key` is opaque (picking); its alpha is read once. */
  opaque(key, u, v) {
    const info = this.info(key);
    if (!info || u < 0 || v < 0 || u >= info.w || v >= info.h) return false;
    let a = this.alpha.get(key);
    if (!a) {
      const src = this.source(key);
      if (!src) return false;
      const c = canvas(info.w, info.h);
      const g = c.getContext("2d", { willReadFrequently: true });
      g.drawImage(src, 0, 0);
      const d = g.getImageData(0, 0, info.w, info.h).data;
      a = new Uint8Array(info.w * info.h);
      for (let i = 0; i < a.length; i++) a[i] = d[i * 4 + 3];
      this.alpha.set(key, a);
    }
    return a[v * info.w + u] > 40;
  }

  /** The native image, or null while it loads (or when there is none). */
  source(key) {
    const d = this.drawn.get(key);
    if (d) return d;
    if (!Object.hasOwn(SPRITES, key)) return null;
    const e = this.images.get(key);
    if (e) return e.ok ? e.im : null;
    const im = new Image();
    const entry = { im, ok: false };
    im.onload = () => {
      entry.ok = true;
      this.onload();
    };
    im.src = new URL(`./iso/${key}.png`, import.meta.url).href;
    this.images.set(key, entry);
    return null;
  }

  /** `key` ready to draw at device scale `k`: `{ c, w, h }` (the canvas and the device size to
   *  draw it at), or null while it loads. */
  get(key, k, flip = false, fog = false, solid = false) {
    const id = `${key}|${k}|${flip ? 1 : 0}${fog ? 1 : 0}${solid ? 1 : 0}`;
    let e = this.cache.get(id);
    if (!e) {
      const src = this.source(key);
      if (!src) return null;
      if (Number.isInteger(k)) {
        const c = flip || fog ? variantOf(src, flip, fog) : src;
        e = { c, w: src.width * k, h: src.height * k };
      } else {
        let c = scaled(src, k, solid);
        if (flip || fog) c = variantOf(c, flip, fog);
        e = { c, w: c.width, h: c.height };
      }
      this.cache.set(id, e);
    }
    return e;
  }
}

/** The banner (compose.py draw_banner) in colour `fg` with its dark `dk`, at 1x. */
function drawBanner(fg, dk) {
  const c = canvas(BANNER_W, BANNER_H);
  const g = c.getContext("2d");
  g.fillStyle = "#5c3a1e";
  g.fillRect(3, 4, 4, 92);
  g.fillStyle = "#8c6034";
  g.fillRect(3, 4, 1, 92);
  g.fillStyle = "#ecc448";
  g.strokeStyle = "#785014";
  g.beginPath();
  g.ellipse(4.5, 3.5, 3.5, 3.5, 0, 0, Math.PI * 2);
  g.fill();
  g.stroke();
  g.beginPath();
  for (const [i, [x, y]] of [[7, 8], [36, 8], [36, 40], [21, 48], [7, 40]].entries()) i ? g.lineTo(x, y) : g.moveTo(x, y);
  g.closePath();
  g.fillStyle = fg;
  g.fill();
  g.strokeStyle = dk;
  g.lineWidth = 1.5;
  g.stroke();
  g.fillStyle = "#ecc448";
  g.fillRect(9, 10, 26, 2);
  g.fillStyle = "#f0c846";
  for (const [x, y, w, h] of [[17, 18, 8, 3], [15, 21, 10, 6], [14, 27, 4, 6], [22, 27, 4, 6], [24, 15, 4, 4]]) g.fillRect(x, y, w, h);
  return c;
}

/** A colour (#rgb, #rrggbb, rgb(...) or hsl(...), as tiles.js gives) darkened to `k`. */
function darken(css, k) {
  const c = canvas(1, 1).getContext("2d");
  c.fillStyle = css;
  c.fillRect(0, 0, 1, 1);
  const [r, g, b] = c.getImageData(0, 0, 1, 1).data;
  return `rgb(${Math.round(r * k)},${Math.round(g * k)},${Math.round(b * k)})`;
}

export class IsoMap {
  constructor(view) {
    this.view = view;
    this.sprites = new SpriteStore(() => this.view.request());
    /** The unit and city boxes of the last frame, front-most last, for picking (screenToMap). */
    this.hits = [];
    this.unitKeys = new Map();
    this.unitPos = new Map();
    this.facing = new Map();
    this.slides = new Map();
    this.slideTurn = null;
    this.seenView = null;
    this.epoch = null;
    /** The last size seen of each city (city id -> size): a fogged city keeps its look. */
    this.lastSize = new Map();
  }

  dispose() {}

  zooms() {
    return ISO_ZOOMS;
  }

  // --- geometry -----------------------------------------------------------------------------

  /** The frame's scales: tile width and height in CSS px, art px to CSS px (`s`), art px to
   *  device px (`k`). */
  frame() {
    const v = this.view;
    const TW = v.cam.ts;
    return { TW, TH: TW / 2, s: TW / NATIVE, k: (TW * v.dpr) / NATIVE, dpr: v.dpr };
  }

  /** The screen centre (CSS px) of plot (x, y), taking the copy of a wrapping map nearest the
   *  camera. */
  center(x, y) {
    const { cam, vw, vh, map } = this.view;
    const TW = cam.ts;
    const TH = cam.ts / 2;
    let u = x + 0.5;
    if (map?.wrap_x) u += Math.round((cam.cx - u) / map.w) * map.w;
    const ex = u - cam.cx;
    const ny = y + 0.5 - cam.cy;
    return { sx: vw / 2 + (ex + ny) * (TW / 2), sy: vh / 2 + (ex - ny) * (TH / 2) };
  }

  /** The plot under screen point (sx, sy): a unit or city drawn there (front-most first), else the
   *  ground; null off the map. */
  screenToMap(sx, sy) {
    const { cam, vw, vh, map } = this.view;
    if (!map) return null;
    for (let i = this.hits.length - 1; i >= 0; i--) {
      const h = this.hits[i];
      if (sx < h.x0 || sx >= h.x1 || sy < h.y0 || sy >= h.y1) continue;
      for (const p of h.parts) {
        const w = this.sprites.info(p.key).w;
        const u = Math.floor((sx - p.x) / p.s);
        if (this.sprites.opaque(p.key, p.flip ? w - 1 - u : u, Math.floor((sy - p.y) / p.s))) return { x: h.x, y: h.y };
      }
    }
    const A = (sx - vw / 2) / (cam.ts / 2);
    const B = (sy - vh / 2) / (cam.ts / 4);
    const x = T.wrapCoord(Math.floor(cam.cx + (A + B) / 2), map.w, map.wrap_x);
    const y = T.wrapCoord(Math.floor(cam.cy + (A - B) / 2), map.h, map.wrap_y);
    return x === null || y === null ? null : { x, y };
  }

  /** The camera centre after a drag of (dx, dy) screen pixels from (startCx, startCy). */
  panDelta(startCx, startCy, dx, dy) {
    const TW = this.view.cam.ts;
    const TH = TW / 2;
    return { cx: startCx - dx / TW - dy / TH, cy: startCy - dx / TW + dy / TH };
  }

  screenOf(x, y) {
    const c = this.center(x, y);
    return { x: c.sx, y: c.sy + this.dropAt(x, y), ts: this.view.cam.ts };
  }

  /** How much lower plot (x, y) is drawn (CSS px): water plots WATER_DROP art px. */
  dropAt(x, y) {
    const map = this.view.map;
    const k = T.plotIndex(map, x, y);
    return k >= 0 && map.plot_type[k] === this.plotTypes().ocean ? (WATER_DROP * this.view.cam.ts) / NATIVE : 0;
  }

  /** A map step (dx east, dy north) as a screen step (CSS px). */
  step(dx, dy) {
    const TW = this.view.cam.ts;
    return { x: ((dx + dy) * TW) / 2, y: ((dx - dy) * TW) / 4 };
  }

  plotTypes() {
    if (this._plot) return this._plot;
    const t = S.data?.tables?.PlotType ?? [];
    const at = (n, d) => {
      const i = t.findIndex((e) => e.type === n);
      return i < 0 ? d : i;
    };
    this._plot = { peak: at("PLOT_PEAK", 0), hills: at("PLOT_HILLS", 1), land: at("PLOT_LAND", 2), ocean: at("PLOT_OCEAN", 3) };
    return this._plot;
  }

  // --- sprites ------------------------------------------------------------------------------

  /** The first of `keys` there is a sprite for, or null. */
  pick(keys) {
    for (const k of keys) if (this.sprites.has(k)) return k;
    return null;
  }

  /** Draws sprite `key` with its art point (ox, oy) at screen point (ax, ay) (CSS px), `mul`
   *  times the zoom's size; returns its opaque box on screen (CSS px) with `part` (where the
   *  sprite went, for picking), or null while it loads. */
  blit(g, f, key, ax, ay, ox, oy, { flip = false, fog = false, solid = false, mul = 1, alpha = 1 } = {}) {
    const info = this.sprites.info(key);
    const sp = info && this.sprites.get(key, f.k * mul, flip, fog, solid);
    if (!sp) return null;
    const s = f.s * mul;
    const left = flip ? info.w - ox : ox;
    // Both edges rounded from their exact positions, so neighbouring tiles share their edges at any
    // device-pixel ratio (a browser zoom of 110% leaves no pinholes); at whole scales w = sp.w.
    const ex = ax - left * s;
    const ey = ay - oy * s;
    const x = Math.round(ex * f.dpr);
    const y = Math.round(ey * f.dpr);
    const w = Math.round((ex + info.w * s) * f.dpr) - x;
    const h = Math.round((ey + info.h * s) * f.dpr) - y;
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.imageSmoothingEnabled = false;
    if (alpha !== 1) g.globalAlpha = alpha;
    g.drawImage(sp.c, x, y, w, h);
    if (alpha !== 1) g.globalAlpha = 1;
    g.setTransform(f.dpr, 0, 0, f.dpr, 0, 0);
    const [bx0, by0, bx1, by1] = info.box;
    const x0 = flip ? info.w - bx1 : bx0;
    const x1 = flip ? info.w - bx0 : bx1;
    const part = { key, x: x / f.dpr, y: y / f.dpr, s: w / f.dpr / info.w, flip };
    return { x0: part.x + x0 * s, y0: part.y + by0 * s, x1: part.x + x1 * s, y1: part.y + by1 * s, part };
  }

  /** The tile sprite of plot k: its terrain's, or the `_b` variant on about half the plots. */
  tileKey(terrain, k) {
    let key = TERRAIN_TILE[terrain] ?? null;
    if (key && !this.sprites.has(key)) key = TILE_FALLBACK[key] ?? null;
    if (!key || !this.sprites.has(key)) return null;
    const alt = `${key}_b`;
    return hash32(k) & 1 && this.sprites.has(alt) ? alt : key;
  }

  /** The sprite standing in for unit type `type` owned by `owner` (memoised). */
  unitKey(type, owner) {
    const civIdx = owner === me()?.player ? me()?.civ : S.players.get(owner)?.civ;
    const civ = shortName(tname("CivilizationType", civIdx), "CIVILIZATION_");
    const memo = `${type}|${civ}`;
    if (this.unitKeys.has(memo)) return this.unitKeys.get(memo);
    const info = S.data?.tables?.UnitType?.[type] ?? {};
    const t = shortName(info.type, "UNIT_");
    const c = shortName(tname("UnitClassType", info.class), "UNITCLASS_");
    const names = [];
    if (civ && t) names.push(`${civ}_${t}`);
    if (civ && c) names.push(`${civ}_${c}`);
    for (const n of [t, c]) if (n) names.push(n, ...(UNIT_ALIASES[n] ?? []));
    if (info.found) names.push("settler");
    if (info.worker) names.push("worker");
    const combat = COMBAT_SPRITES[tname("UnitCombatType", info.unit_combat)];
    const domain = DOMAIN_SPRITES[tname("DomainType", info.domain)];
    for (const n of [combat, domain, "swordsman"]) if (n) names.push(n, ...(UNIT_ALIASES[n] ?? []));
    const key = this.pick(names.map((n) => `unit_${n}`));
    this.unitKeys.set(memo, key);
    return key;
  }

  /** The banner sprite of `owner`, drawn on first use. */
  bannerKey(owner) {
    // Keyed by the colour, not the player: a player's colour can change (a barbarian seen before
    // it is met, a new game in the same page).
    const fg = ownerColor(owner);
    const key = `banner:${fg}`;
    if (!this.sprites.has(key)) this.sprites.put(key, drawBanner(fg, darken(fg, 0.45)));
    return key;
  }

  // --- unit moves ---------------------------------------------------------------------------

  /** Facing and the move slide from the units' moves since the last view: units face right and
   *  are mirrored after a move to the left of the screen; the player's own moves within their turn
   *  slide (as artmap.js). */
  syncView() {
    if (S.view === this.seenView) return;
    this.seenView = S.view;
    const map = this.view.map;
    if (map.epoch !== this.epoch) {
      // Another game (a new map tracker): unit ids restart, so last positions mean nothing.
      this.epoch = map.epoch;
      this.unitPos = new Map();
      this.facing = new Map();
      this.slides.clear();
      this.slideTurn = null;
      this.lastSize.clear();
    }
    const units = S.view?.units ?? [];
    const { positions, moved } = unitMoves(this.unitPos, units, map.w, !!map.wrap_x);
    this.unitPos = positions;
    const facing = new Map();
    for (const u of units) facing.set(u.id, this.facing.get(u.id) ?? false);
    for (const { id, dx, dy } of moved) if (dx + dy !== 0) facing.set(id, dx + dy < 0);
    this.facing = facing;
    const turn = S.status?.turn ?? S.view?.turn ?? 0;
    const mep = me()?.player;
    if (turn === this.slideTurn && S.status?.phase === "human" && mep !== null && mep !== undefined && !this.view.reduceMotion()) {
      const own = new Set(units.filter((u) => u.owner === mep).map((u) => u.id));
      for (const { id, dx, dy } of moved) {
        const steps = Math.max(Math.abs(dx), Math.abs(dy));
        if (own.has(id) && steps <= SLIDE_MAX_TILES) this.slides.set(id, { dx, dy, t0: performance.now(), dur: Math.min(SLIDE_MAX_MS, steps * SLIDE_MS_PER_TILE) });
      }
    }
    this.slideTurn = turn;
  }

  /** Unit `id`'s slide offset (CSS px, ease-out from its old tile), or null. */
  slideOffset(id, now) {
    const sl = this.slides.get(id);
    if (!sl) return null;
    const t = (now - sl.t0) / sl.dur;
    if (t >= 1) {
      this.slides.delete(id);
      return null;
    }
    const e = 1 - Math.pow(1 - t, 3);
    return this.step(-sl.dx * (1 - e), -sl.dy * (1 - e));
  }

  // --- drawing ------------------------------------------------------------------------------

  draw() {
    const v = this.view;
    const g = v.ctx;
    const map = v.map;
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.fillStyle = "#0b1220";
    g.fillRect(0, 0, v.canvas.width, v.canvas.height);
    this.hits = [];
    if (!map) return;
    const f = this.frame();
    this.sprites.frame(f.k);
    g.setTransform(f.dpr, 0, 0, f.dpr, 0, 0);
    this.syncView();
    const PLOT = this.plotTypes();

    // The plots in view (one copy each, nearest the camera), back to front. Sprites rise up to
    // about 270 art px above their plot, so plots below the view still count.
    // On a map that wraps east-west, a small map or a far zoom shows a plot more than once: each
    // copy in view is a cell of its own.
    const cells = [];
    const below = 300 * f.s;
    const copies = map.wrap_x ? [-2, -1, 0, 1, 2] : [0];
    const wrap = this.step(map.w, 0);
    for (let y = 0; y < map.h; y++) {
      for (let x = 0; x < map.w; x++) {
        const c = this.center(x, y);
        const k = y * map.w + x;
        const water = map.plot_type[k] === PLOT.ocean;
        for (const m of copies) {
          const sx = c.sx + m * wrap.x;
          const sy = c.sy + m * wrap.y;
          if (sx < -f.TW || sx > v.vw + f.TW || sy < -f.TH * 2 || sy > v.vh + below) continue;
          cells.push({ x, y, k, sx, sy, water, drop: water ? WATER_DROP * f.s : 0, fog: map.vis[k] === 1 });
        }
      }
    }
    cells.sort((a, b) => a.sy - b.sy || a.sx - b.sx);
    for (const c of cells) this.drawTile(g, f, c, PLOT);

    const layer = v.layer ?? "terrain";
    if (layer === "political") this.drawPolitical(g, f, cells, PLOT);
    else if (layer === "yield") this.drawYields(g, f, cells);
    else {
      this.drawShores(g, f, cells, PLOT);
      this.drawRivers(g, f, cells);
      this.drawRoutes(g, f, cells);
      this.drawBorders(g, f, cells, false);
    }
    this.drawGroundMarks(g, f);

    const byPlot = new Map();
    for (const c of cells) byPlot.set(c.k, [...(byPlot.get(c.k) ?? []), c]);
    const cities = [];
    for (const city of S.cities.values()) for (const c of byPlot.get(city.y * map.w + city.x) ?? []) cities.push({ city, c });
    const stacks = this.unitStacks(byPlot);
    // Everything that stands up, as items {d, o, f}: drawn by f in one pass sorted by d (the plot's
    // depth) then o (on one plot: feature, building or city, resource, unit).
    const items = [];
    const cityAt = new Set(cities.map((x) => x.c.k));
    if (layer === "terrain") this.collectTerrain(f, cells, cityAt, items, PLOT);
    this.collectCities(f, cities, items);
    this.collectUnits(f, stacks, cityAt, items);
    items.sort((a, b) => a.d - b.d || a.o - b.o || a.x - b.x);
    for (const it of items) {
      it.box = it.f(g);
      if (it.stack) it.fig = it.stack.figure;
    }
    this.ghostBuried(g, f, items);
    this.hits = items.filter((it) => it.hit && it.box).map((it) => ({ ...it.box, x: it.hit.x, y: it.hit.y, fig: it.fig ?? null }));
    // Borders again over what stands up, faintly, so who owns a forest still shows.
    if (layer === "terrain") this.drawBorders(g, f, cells, false, true);
    this.drawClouds(g, f, cells);

    this.drawCityLabels(g, f, cities);
    this.drawUnitBadges(g, f, items.filter((it) => it.stack));
    this.drawPath(g, f);
    this.drawFlashes(g, f);
    if (this.slides.size) {
      const now = performance.now();
      for (const [id, sl] of this.slides) if (now - sl.t0 >= sl.dur) this.slides.delete(id);
      if (this.slides.size) v.request();
    }
  }

  /** A plot's tile, its overlays (hills, flat features, flat improvements) and its fog. */
  drawTile(g, f, c, PLOT) {
    const map = this.view.map;
    if (map.vis[c.k] === 0) return; // unrevealed: the dark background shows
    const terrain = tname("TerrainType", map.terrain[c.k]);
    const x0 = c.sx - f.TW / 2;
    const y0 = c.sy - f.TH / 2 + c.drop;
    const key = this.tileKey(terrain, c.k);
    if (!key || !this.blit(g, f, key, x0, y0, 0, 0, { fog: c.fog, solid: true })) {
      this.diamond(g, c.sx, c.sy + c.drop, f.TW);
      g.fillStyle = T.terrainColor(terrain);
      g.fill();
      if (c.fog) {
        g.fillStyle = "rgba(10,16,32,0.4)";
        g.fill();
      }
    }
    const over = (keys) => {
      const k = this.pick(keys);
      if (k) this.blit(g, f, k, x0, y0, 0, 0, { fog: c.fog });
    };
    if (map.plot_type[c.k] === PLOT.hills) {
      over(DRY.has(terrain) ? ["ovl_hills_dry", "ovl_hills"] : COLD.has(terrain) ? ["ovl_hills_snow", "ovl_hills"] : ["ovl_hills"]);
    }
    if (map.city[c.k] >= 0 && map.city[c.k] !== null && map.city[c.k] !== undefined) return;
    const feat = FEATURE_SPRITES[tname("FeatureType", map.feature[c.k])];
    if (feat?.[0].startsWith("ovl_")) over(feat);
    const imp = map.improvement[c.k];
    if (imp !== null && imp !== undefined && imp >= 0) {
      const keys = improvementSprites(tname("ImprovementType", imp) ?? "");
      if (keys[0]?.startsWith("ovl_")) over(keys);
    }
  }

  diamond(g, sx, sy, TW, inset = 0) {
    const hw = TW / 2 - inset * 2;
    const hh = TW / 4 - inset;
    g.beginPath();
    g.moveTo(sx, sy - hh);
    g.lineTo(sx + hw, sy);
    g.lineTo(sx, sy + hh);
    g.lineTo(sx - hw, sy);
    g.closePath();
  }

  /** The features, buildings (improvements standing on a plot) and resource props, as items. A
   *  city's plot shows only the city. Remembered-but-fogged plots draw them darkened. */
  collectTerrain(f, cells, cityAt, items, PLOT) {
    const map = this.view.map;
    for (const c of cells) {
      if (map.vis[c.k] === 0 || cityAt.has(c.k)) continue;
      const terrain = tname("TerrainType", map.terrain[c.k]);
      const feature = tname("FeatureType", map.feature[c.k]);
      const bottom = c.sy + f.TH / 2 + c.drop; // the tile's bottom vertex: features and buildings stand on it
      const stand = (key, o) => {
        const info = this.sprites.info(key);
        items.push({ d: c.sy, o, x: c.sx, f: (g) => this.blit(g, f, key, c.sx, bottom, info.w / 2, info.h, { fog: c.fog }), solid: true });
      };
      let up = null;
      if (feature === "FEATURE_VOLCANO") up = this.pick(["feat_volcano", "feat_mountain"]);
      else if (map.plot_type[c.k] === PLOT.peak) up = this.pick(HELL.has(terrain) ? ["feat_mountain_hell", "feat_mountain"] : ["feat_mountain"]);
      else {
        const keys = FEATURE_SPRITES[feature];
        if (keys && !keys[0].startsWith("ovl_")) up = this.pick(keys);
      }
      if (up) stand(up, 0);
      const imp = map.improvement[c.k];
      if (imp !== null && imp !== undefined && imp >= 0) {
        const keys = improvementSprites(tname("ImprovementType", imp) ?? "");
        const key = keys[0]?.startsWith("ovl_") ? null : this.pick(keys);
        if (key) stand(key, 1);
      }
      const bonus = map.bonus[c.k];
      if (bonus !== null && bonus !== undefined && bonus >= 0) {
        // A prop on the plot's front-right quarter, in front of what stands on the plot.
        const key = this.pick([resourceSprite(tname("BonusType", bonus) ?? "")]);
        if (key) {
          const info = this.sprites.info(key);
          items.push({ d: c.sy, o: 2, x: c.sx, f: (g) => this.blit(g, f, key, c.sx + 40 * f.s, c.sy + 30 * f.s + c.drop, info.w / 2, info.h, { fog: c.fog }) });
        }
      }
    }
  }

  /** The cities: the settlement for their size in its owner's colour (the art's blue roofs and
   *  flags recoloured), standing on the plot's bottom vertex, with the owner's banner planted at
   *  its front-left; the banner also carries the garrison's count (drawUnitBadges). */
  collectCities(f, cities, items) {
    this.cityFlags = new Map();
    for (const { city, c } of cities) {
      if (city.size !== null && city.size !== undefined) this.lastSize.set(city.id, city.size);
      const base = this.pick(citySprites(city.size ?? this.lastSize.get(city.id) ?? 1));
      const banner = this.bannerKey(city.owner);
      const bottom = c.sy + f.TH / 2;
      items.push({
        d: c.sy, o: 1, x: c.sx, solid: true, ghost: 0.35, hit: { x: city.x, y: city.y },
        f: (g, alpha = 1) => {
          let box = null;
          if (base) {
            const key = this.sprites.owned(base, ownerColor(city.owner));
            const info = this.sprites.info(key);
            box = this.blit(g, f, key, c.sx, bottom, info.w / 2, info.h, { fog: c.fog, alpha, mul: info.w > NATIVE ? NATIVE / info.w : 1 });
          }
          const fl = this.blit(g, f, banner, c.sx - 62 * f.s, bottom - 16 * f.s, 4, 95, { alpha });
          if (fl) this.cityFlags.set(c, fl);
          return union(box, fl);
        },
      });
    }
  }

  /** The unit stacks in view: their units, the one shown, its plot cell, and whether it is
   *  selected. */
  unitStacks(byPlot) {
    const v = this.view;
    const mine = me()?.player;
    const sel = v.overlay.units ?? [];
    const out = [];
    for (const units of S.unitsAt.values()) {
      const u0 = units[0];
      const top = T.topUnit(units, mine, sel);
      for (const c of byPlot.get(u0.y * v.map.w + u0.x) ?? []) out.push({ units, top, c, picked: sel.includes(top.id) });
    }
    return out;
  }

  /** Each stack's figure (its top unit's sprite) standing at the plot centre, with its contact
   *  shadow and its owner's banner just left of the figure's opaque box (compose.py). A garrison
   *  would hide its city: as in the 2D renderer it shows only while selected or sliding, at the
   *  city's gate, front right, and the city's banner carries its count. */
  collectUnits(f, stacks, cityAt, items) {
    const now = performance.now();
    for (const st of stacks) {
      const { top, c } = st;
      const key = this.unitKey(top.type, top.owner);
      const off = this.slideOffset(top.id, now);
      const gate = cityAt.has(c.k);
      st.flag = null;
      st.figure = null;
      if (gate && !st.picked && !off) {
        items.push({ d: c.sy, o: 3, x: c.sx, stack: st, ax: c.sx, ay: c.sy + UNIT_DY * f.s, f: () => null });
        continue;
      }
      const ax = c.sx + (gate ? GATE_DX * f.s : 0) + (off?.x ?? 0);
      const ay = c.sy + (gate ? GATE_DY : UNIT_DY) * f.s + c.drop + (off?.y ?? 0);
      const flip = this.facing.get(top.id) ?? false;
      const banner = gate ? null : this.bannerKey(top.owner);
      items.push({
        d: c.sy, o: 3, x: c.sx, stack: st, ax, ay, ghost: 0.25, hit: { x: c.x, y: c.y },
        f: (g, alpha = 1) => {
          const info = key && this.sprites.info(key);
          if (!info) return null;
          const left = flip ? info.w - info.box[2] : info.box[0];
          const bx = Math.min(-34, left - info.w / 2 - 10);
          if (alpha === 1) this.shadow(g, f, ax, ay - 2 * f.s, Math.min(70, Math.max(40, info.w * 0.5)));
          const fl = banner ? this.blit(g, f, banner, ax + bx * f.s, ay - 6 * f.s, 4, 95, { alpha }) : null;
          const box = this.blit(g, f, key, ax, ay, info.w / 2, info.h, { flip, alpha });
          st.flag = fl;
          st.figure = box;
          return union(box, fl);
        },
      });
    }
  }

  /** A soft contact shadow `w` art px wide under a unit's feet (compose.py draw_shadow). */
  shadow(g, f, cx, cy, w) {
    const rx = (w / 2 + 3) * f.s;
    const ry = 10 * f.s;
    g.save();
    g.translate(cx, cy);
    g.scale(1, ry / rx);
    const grd = g.createRadialGradient(0, 0, 0, 0, 0, rx);
    grd.addColorStop(0, "rgba(10,20,10,0.42)");
    grd.addColorStop(0.6, "rgba(10,20,10,0.3)");
    grd.addColorStop(1, "rgba(10,20,10,0)");
    g.fillStyle = grd;
    g.beginPath();
    g.arc(0, 0, rx, 0, Math.PI * 2);
    g.fill();
    g.restore();
  }

  /** Units (and cities) that something drawn after them hides by more than a quarter (a third)
   *  are drawn again, faded, over it, so a unit behind a mountain or a city behind a forest still
   *  shows. Items are in depth order and no sprite rises more than about 300 art px above its plot,
   *  so the scan stops there. */
  ghostBuried(g, f, items) {
    const reach = 300 * f.s;
    for (let i = 0; i < items.length; i++) {
      const u = items[i];
      const a = u.stack ? u.stack.figure : u.ghost ? u.box : null;
      if (!a) continue;
      const area = (a.x1 - a.x0) * (a.y1 - a.y0);
      let hidden = 0;
      for (let j = i + 1; j < items.length && items[j].d <= u.d + reach; j++) {
        const b = items[j].box;
        if (!items[j].solid || !b) continue;
        const w = Math.min(a.x1, b.x1) - Math.max(a.x0, b.x0);
        const h = Math.min(a.y1, b.y1) - Math.max(a.y0, b.y0);
        if (w > 0 && h > 0) hidden += w * h;
      }
      if (hidden > area * u.ghost) u.f(g, 0.55);
    }
  }

  /** Rivers along plot edges: bit 0 is the plot's south edge (the diamond's lower-left), bit 1 its
   *  east edge (lower-right); a royal-blue channel between earth banks, like the water tiles. An
   *  edge whose far side is unexplored is left out (it would hang over the void). */
  drawRivers(g, f, cells) {
    const map = this.view.map;
    const hw = f.TW / 2;
    const hh = f.TW / 4;
    const seen = (x, y) => {
      const n = T.plotIndex(map, x, y);
      return n < 0 || map.vis[n] > 0;
    };
    const path = new Path2D();
    let any = false;
    for (const c of cells) {
      if (map.vis[c.k] === 0) continue;
      const rv = map.river[c.k];
      if (!(rv > 0)) continue;
      if (rv & 1 && seen(c.x, c.y - 1)) {
        path.moveTo(c.sx - hw, c.sy);
        path.lineTo(c.sx, c.sy + hh);
        any = true;
      }
      if (rv & 2 && seen(c.x + 1, c.y)) {
        path.moveTo(c.sx, c.sy + hh);
        path.lineTo(c.sx + hw, c.sy);
        any = true;
      }
    }
    if (!any) return;
    g.lineCap = "round";
    g.lineJoin = "round";
    g.strokeStyle = "#6a4a26";
    g.lineWidth = Math.max(4, 24 * f.s);
    g.stroke(path);
    g.strokeStyle = "#1f56b8";
    g.lineWidth = Math.max(3, 17 * f.s);
    g.stroke(path);
    g.strokeStyle = "#3f86e4";
    g.lineWidth = Math.max(1.5, 9 * f.s);
    g.stroke(path);
  }

  /** Surf where land meets water: white along the land's front edges at the water's level (the foot
   *  of the earth bank) and along its back edges at its own. */
  drawShores(g, f, cells, PLOT) {
    const map = this.view.map;
    const hw = f.TW / 2;
    const hh = f.TW / 4;
    const dz = WATER_DROP * f.s;
    const wet = (x, y) => {
      const n = T.plotIndex(map, x, y);
      return n >= 0 && map.vis[n] > 0 && map.plot_type[n] === PLOT.ocean;
    };
    const p = new Path2D();
    let any = false;
    for (const c of cells) {
      if (c.water || map.vis[c.k] === 0) continue;
      if (wet(c.x + 1, c.y)) (p.moveTo(c.sx + hw, c.sy + dz), p.lineTo(c.sx, c.sy + hh + dz), (any = true)); // E: right -> bottom
      if (wet(c.x, c.y - 1)) (p.moveTo(c.sx, c.sy + hh + dz), p.lineTo(c.sx - hw, c.sy + dz), (any = true)); // S: bottom -> left
      if (wet(c.x, c.y + 1)) (p.moveTo(c.sx, c.sy - hh), p.lineTo(c.sx + hw, c.sy), (any = true)); // N: top -> right
      if (wet(c.x - 1, c.y)) (p.moveTo(c.sx - hw, c.sy), p.lineTo(c.sx, c.sy - hh), (any = true)); // W: left -> top
    }
    if (!any) return;
    g.lineCap = "round";
    g.strokeStyle = "rgba(255,255,255,0.3)";
    g.lineWidth = Math.max(2, 10 * f.s);
    g.stroke(p);
    g.strokeStyle = "rgba(255,255,255,0.85)";
    g.lineWidth = Math.max(1, 3 * f.s);
    g.setLineDash([12 * f.s, 7 * f.s]);
    g.stroke(p);
    g.setLineDash([]);
  }

  /** Clouds over the unexplored plots that touch explored ones (as the reference's), so the known
   *  world ends in a bank of cloud rather than a cliff into the void: the shaded undersides of
   *  every puff first, then the lit tops, so neighbouring puffs merge. */
  drawClouds(g, f, cells) {
    const map = this.view.map;
    const edge = cells.filter((c) => map.vis[c.k] === 0 && T.DIRS8.some(([dx, dy]) => {
      const n = T.plotIndex(map, c.x + dx, c.y + dy);
      return n >= 0 && map.vis[n] > 0;
    }));
    if (!edge.length) return;
    for (const lit of [false, true]) {
      g.fillStyle = lit ? "#f7f9ff" : "#b9cbe6";
      g.beginPath();
      for (const c of edge) {
        for (let i = 0; i < 3; i++) {
          const h = hash32(c.k * 3 + i);
          const r = f.TW * (0.24 + ((h >>> 16) & 255) / 1800);
          const x = c.sx + (((h & 255) / 255) - 0.5) * f.TW * 0.5;
          const y = c.sy + ((((h >>> 8) & 255) / 255) - 0.5) * f.TH * 0.5 + (lit ? -r * 0.12 : r * 0.08);
          g.moveTo(x + r, y);
          g.ellipse(x, y, lit ? r * 0.92 : r, (lit ? 0.52 : 0.6) * r, 0, 0, Math.PI * 2);
        }
      }
      g.fill();
    }
  }

  /** Roads and railroads from each routed plot's centre to the shared midpoint with every routed,
   *  revealed neighbour, so a network joins up: packed tan earth with darker verges. */
  drawRoutes(g, f, cells) {
    const map = this.view.map;
    const road = new Path2D();
    const rail = new Path2D();
    let anyRoad = false;
    let anyRail = false;
    for (const c of cells) {
      if (map.vis[c.k] === 0) continue;
      const rt = map.route[c.k];
      if (!(rt >= 0)) continue;
      const isRail = tname("RouteType", rt) === "ROUTE_RAILROAD";
      const path = isRail ? rail : road;
      let joined = false;
      for (const [dx, dy] of T.DIRS8) {
        const n = T.plotIndex(map, c.x + dx, c.y + dy);
        if (n >= 0 && map.route[n] >= 0 && map.vis[n] > 0) {
          const s = this.step(dx, dy);
          path.moveTo(c.sx, c.sy);
          path.lineTo(c.sx + s.x / 2, c.sy + s.y / 2);
          joined = true;
        }
      }
      if (!joined) {
        path.moveTo(c.sx - 12 * f.s, c.sy);
        path.lineTo(c.sx + 12 * f.s, c.sy);
      }
      if (isRail) anyRail = true;
      else anyRoad = true;
    }
    g.lineCap = "round";
    g.lineJoin = "round";
    if (anyRoad) {
      g.strokeStyle = "rgba(92,64,30,0.85)";
      g.lineWidth = Math.max(2.5, 14 * f.s);
      g.stroke(road);
      g.strokeStyle = "#d2b27a";
      g.lineWidth = Math.max(1.5, 9 * f.s);
      g.stroke(road);
    }
    if (anyRail) {
      g.strokeStyle = "#3a2a1c";
      g.lineWidth = Math.max(2.5, 14 * f.s);
      g.stroke(rail);
      g.strokeStyle = "#9aa0a8";
      g.lineWidth = Math.max(1, 3 * f.s);
      g.setLineDash([4 * f.s, 4 * f.s]);
      g.stroke(rail);
      g.setLineDash([]);
    }
  }

  /** Each owner's border along every plot edge to a differently-owned plot: a line in the owner's
   *  colour just inside the edge, on a dark rule. `wash` also tints the owned plots (Political);
   *  `over` draws the line alone, faint, over what stands up. */
  drawBorders(g, f, cells, wash, over = false) {
    const map = this.view.map;
    const hw = f.TW / 2;
    const hh = f.TW / 4;
    const byOwner = new Map();
    for (const c of cells) {
      if (map.vis[c.k] === 0) continue;
      const owner = map.owner[c.k];
      if (!(owner >= 0)) continue;
      const sy = c.sy + c.drop;
      if (wash) {
        this.diamond(g, c.sx, sy, f.TW);
        g.fillStyle = ownerColor(owner);
        g.globalAlpha = 0.45;
        g.fill();
        g.globalAlpha = 1;
      }
      const nb = (dx, dy) => {
        const n = T.plotIndex(map, c.x + dx, c.y + dy);
        return n >= 0 ? map.owner[n] : -2;
      };
      let p = byOwner.get(owner);
      if (!p) byOwner.set(owner, (p = new Path2D()));
      const i = 3 * f.s; // inset, so two neighbours' borders run side by side
      if (nb(0, 1) !== owner) (p.moveTo(c.sx, sy - hh + i), p.lineTo(c.sx + hw - 2 * i, sy)); // N: top -> right
      if (nb(1, 0) !== owner) (p.moveTo(c.sx + hw - 2 * i, sy), p.lineTo(c.sx, sy + hh - i)); // E: right -> bottom
      if (nb(0, -1) !== owner) (p.moveTo(c.sx, sy + hh - i), p.lineTo(c.sx - hw + 2 * i, sy)); // S: bottom -> left
      if (nb(-1, 0) !== owner) (p.moveTo(c.sx - hw + 2 * i, sy), p.lineTo(c.sx, sy - hh + i)); // W: left -> top
    }
    g.lineCap = "round";
    for (const [owner, p] of byOwner) {
      if (over) {
        g.globalAlpha = 0.5;
        g.strokeStyle = ownerColor(owner);
        g.lineWidth = Math.max(1, 3 * f.s);
        g.stroke(p);
        g.globalAlpha = 1;
        continue;
      }
      g.strokeStyle = "rgba(12,14,24,0.55)";
      g.lineWidth = Math.max(3, 10 * f.s);
      g.stroke(p);
      g.strokeStyle = ownerColor(owner);
      g.lineWidth = Math.max(2, 6 * f.s);
      g.stroke(p);
    }
  }

  /** The Political view: owned plots washed in their owner's colour (unowned land faintly grey),
   *  with the borders. */
  drawPolitical(g, f, cells, PLOT) {
    const map = this.view.map;
    for (const c of cells) {
      if (map.vis[c.k] === 0 || map.owner[c.k] >= 0 || map.plot_type[c.k] === PLOT.ocean) continue;
      this.diamond(g, c.sx, c.sy + c.drop, f.TW);
      g.fillStyle = "rgba(150,150,150,0.3)";
      g.fill();
    }
    this.drawBorders(g, f, cells, true);
  }

  /** The Yields view: the board darkened, with the food, production and commerce of every plot in
   *  sight drawn in their colours. */
  drawYields(g, f, cells) {
    const map = this.view.map;
    for (const c of cells) {
      if (map.vis[c.k] === 0) continue;
      this.diamond(g, c.sx, c.sy + c.drop, f.TW);
      g.fillStyle = "rgba(0,0,0,0.5)";
      g.fill();
    }
    if (f.TW < 48) return; // too small for legible numbers
    g.font = `bold ${Math.max(10, Math.round(f.TW * 0.16))}px system-ui, sans-serif`;
    g.textAlign = "center";
    g.textBaseline = "middle";
    for (const c of cells) {
      if (map.vis[c.k] !== 2) continue;
      const y = T.unpackYield(map.yield[c.k]);
      if (!y) continue;
      const parts = [];
      y.forEach((val, j) => {
        if (val > 0) parts.push([String(val), T.YIELD_COLORS[j]]);
      });
      parts.forEach(([val, col], i) => {
        const ox = (i - (parts.length - 1) / 2) * f.TW * 0.2;
        g.fillStyle = "rgba(0,0,0,0.65)";
        g.fillText(val, c.sx + ox + 1, c.sy + c.drop + 1);
        g.fillStyle = col;
        g.fillText(val, c.sx + ox, c.sy + c.drop);
      });
    }
  }

  /** The ground marks: the selected plot's highlight (compose.py draw_highlight), the targets of
   *  an interface mode, the hovered plot and the waypoints not sent yet. */
  drawGroundMarks(g, f) {
    const o = this.view.overlay;
    const at = (x, y) => {
      const c = this.center(x, y);
      const k = y * this.view.map.w + x;
      return { sx: c.sx, sy: c.sy + (this.view.map.plot_type[k] === this.plotTypes().ocean ? WATER_DROP * f.s : 0) };
    };
    const ring = (x, y, color, width, dash = []) => {
      const c = at(x, y);
      this.diamond(g, c.sx, c.sy, f.TW, 2 * f.s);
      g.strokeStyle = color;
      g.lineWidth = width;
      g.setLineDash(dash);
      g.stroke();
      g.setLineDash([]);
    };
    if (Array.isArray(o.targets)) for (const [x, y] of o.targets) ring(x, y, "#4fe3ff", 2, [4, 3]);
    if (o.sel) {
      const c = at(o.sel.x, o.sel.y);
      this.diamond(g, c.sx, c.sy, f.TW, 8 * f.s);
      g.fillStyle = "rgba(250,230,90,0.24)";
      g.fill();
      g.strokeStyle = "rgba(255,236,110,0.9)";
      g.lineWidth = Math.max(1.5, 3 * f.s);
      g.stroke();
    }
    if (o.hover) ring(o.hover.x, o.hover.y, o.targets ? "#4fe3ff" : "rgba(255,255,255,0.75)", 1.5);
    if (Array.isArray(o.waypoints)) {
      o.waypoints.forEach(([x, y], i) => {
        ring(x, y, "#ffe14d", 2, [3, 3]);
        const c = at(x, y);
        this.roundel(g, f, c.sx, c.sy, String(i + 1), "#ffe14d");
      });
    }
  }

  /** A numbered disc (path turns, waypoints). */
  roundel(g, f, x, y, text, fill) {
    const r = Math.max(7, 18 * f.s);
    g.fillStyle = "rgba(0,0,0,0.55)";
    g.beginPath();
    g.arc(x + 1, y + 1.5, r, 0, Math.PI * 2);
    g.fill();
    g.fillStyle = fill;
    g.beginPath();
    g.arc(x, y, r, 0, Math.PI * 2);
    g.fill();
    g.fillStyle = "#111";
    g.font = `bold ${Math.round(r * 1.1)}px system-ui, sans-serif`;
    g.textAlign = "center";
    g.textBaseline = "middle";
    g.fillText(text, x, y + 0.5);
  }

  /** The planned path of the selection (overlay.path): a line from plot to plot, unwrapped from
   *  the selection's copy, with a roundel at each turn's end; red to an attack, orange when it
   *  declares war. Drawn over everything, so trees and walls never hide it. */
  drawPath(g, f) {
    const v = this.view;
    const path = v.overlay.path;
    if (!path?.ok || !(path.nodes?.length > 1)) return;
    const map = v.map;
    const n0 = path.nodes[0];
    const c0 = this.center(n0.x, n0.y);
    let ux = n0.x;
    let px = n0.x;
    const pts = [{ sx: c0.sx, sy: c0.sy }];
    for (const n of path.nodes.slice(1)) {
      let dx = n.x - px;
      if (map.wrap_x) {
        if (dx > map.w / 2) dx -= map.w;
        if (dx < -map.w / 2) dx += map.w;
      }
      ux += dx;
      px = n.x;
      const s = this.step(ux - n0.x, n.y - n0.y);
      pts.push({ sx: c0.sx + s.x, sy: c0.sy + s.y + this.dropAt(n.x, n.y) });
    }
    pts[0].sy += this.dropAt(n0.x, n0.y);
    const danger = path.attack ? "#ff4a3a" : path.declare_war_team !== null && path.declare_war_team !== undefined ? "#ff9d2e" : null;
    g.lineJoin = "round";
    g.lineCap = "round";
    g.beginPath();
    pts.forEach((p, i) => (i ? g.lineTo(p.sx, p.sy) : g.moveTo(p.sx, p.sy)));
    g.strokeStyle = "rgba(0,0,0,0.6)";
    g.lineWidth = Math.max(4, 10 * f.s);
    g.stroke();
    g.strokeStyle = danger ?? "#f5f1e0";
    g.lineWidth = Math.max(2, 5 * f.s);
    g.stroke();
    for (const m of T.pathTurnMarks(path.nodes)) {
      const p = pts[m.i];
      this.roundel(g, f, p.sx, p.sy, String(m.turn), m.last ? (danger ?? "#ffe14d") : "#f5f1e0");
    }
  }

  /** The combat flashes (map.js flashTile): a brief crimson light on the struck plot. */
  drawFlashes(g, f) {
    const v = this.view;
    if (!v.flashes?.length) return;
    const now = performance.now();
    for (const fl of v.flashes) {
      const t = Math.min(1, (now - fl.t0) / fl.dur);
      const a = (1 - t) * (fl.kind === "death" ? 0.5 : 0.58);
      const c = this.center(fl.x, fl.y);
      const cy = c.sy + this.dropAt(fl.x, fl.y) - 30 * f.s - (fl.kind === "death" ? t * f.TH * 0.5 : 0);
      const r = f.TW * (fl.kind === "death" ? 0.3 + t * 0.22 : 0.42);
      const grd = g.createRadialGradient(c.sx, cy, 0, c.sx, cy, r);
      grd.addColorStop(0, `rgba(119,37,43,${a})`);
      grd.addColorStop(1, "rgba(119,37,43,0)");
      g.fillStyle = grd;
      g.beginPath();
      g.arc(c.sx, cy, r, 0, Math.PI * 2);
      g.fill();
    }
  }

  /** Each city's plaque over the front of its base: the size on a disc in the owner's colour, the name, and
   *  for the player's own cities the turns left on what it builds; a gold star over a capital. */
  drawCityLabels(g, f, cities) {
    const mine = me()?.player;
    for (const { city, c } of cities) {
      const col = ownerColor(city.owner);
      const fs = Math.max(10, Math.round(f.TW * 0.12));
      const y = c.sy + f.TH / 4;
      const size = city.size === null || city.size === undefined ? "?" : String(city.size);
      const turns = city.owner === mine ? city.production?.turns : null;
      const label = f.TW >= 64 ? `${city.name ?? ""}${turns !== null && turns !== undefined ? `  ${turns}` : ""}` : "";
      g.font = `600 ${fs}px Georgia, "Times New Roman", serif`;
      const tw = label ? g.measureText(label).width : 0;
      const r = fs * 0.75;
      const h = fs + 8;
      const w = tw + (label ? fs * 1.2 : 0) + r * 2 + 6;
      const x0 = c.sx - w / 2;
      g.fillStyle = "rgba(17,27,52,0.9)";
      roundRect(g, x0, y, w, h, 4);
      g.fill();
      g.strokeStyle = "#d4a845";
      g.lineWidth = 1.5;
      g.stroke();
      g.beginPath();
      g.arc(x0 + 3 + r, y + h / 2, r, 0, Math.PI * 2);
      g.fillStyle = col;
      g.fill();
      g.strokeStyle = "rgba(0,0,0,0.6)";
      g.lineWidth = 1;
      g.stroke();
      g.fillStyle = "#fff";
      g.font = `bold ${Math.round(fs * 0.9)}px system-ui, sans-serif`;
      g.textAlign = "center";
      g.textBaseline = "middle";
      g.fillText(size, x0 + 3 + r, y + h / 2 + 0.5);
      if (label) {
        g.font = `600 ${fs}px Georgia, "Times New Roman", serif`;
        g.textAlign = "left";
        g.fillStyle = city.owner === mine ? "#ffe9a8" : "#f2ecdc";
        g.fillText(label, x0 + 6 + r * 2 + fs * 0.4, y + h / 2 + 0.5);
      }
      if (city.capital) {
        g.fillStyle = "#ffdd55";
        g.strokeStyle = "rgba(10,10,12,0.7)";
        g.lineWidth = 1;
        star(g, x0 - 2, y + 1, 5, Math.max(4, fs * 0.55), Math.max(2, fs * 0.24));
        g.fill();
        g.stroke();
      }
    }
  }

  /** Each stack's count (on its flag; a hidden garrison's on its city's, even when alone), its
   *  health bar when hurt, and for the player's own stacks a pip on the finial: green while it can
   *  move. Drawn over everything. */
  drawUnitBadges(g, f, items) {
    const mine = me()?.player;
    for (const it of items) {
      const { units, top, figure } = it.stack;
      const flag = it.stack.flag ?? this.cityFlags?.get(it.stack.c);
      if (!flag) continue;
      const fx = flag.x0;
      const fy = flag.y0;
      if (units.length > 1 || !figure) {
        const fs = Math.max(9, Math.round(20 * f.s));
        g.font = `bold ${fs}px system-ui, sans-serif`;
        g.textAlign = "center";
        g.textBaseline = "middle";
        const cx = fx + 22 * f.s;
        const cy = fy + 30 * f.s;
        g.lineWidth = 3;
        g.strokeStyle = "rgba(0,0,0,0.75)";
        g.strokeText(String(units.length), cx, cy);
        g.fillStyle = "#fff6d8";
        g.fillText(String(units.length), cx, cy);
      }
      if (top.owner === mine) {
        g.beginPath();
        g.arc(fx + 4.5 * f.s, fy + 3.5 * f.s, Math.max(2.5, 5 * f.s), 0, Math.PI * 2);
        g.fillStyle = (top.moves ?? 0) > 0 ? "#45e05c" : "#d03a2a";
        g.fill();
        g.strokeStyle = "rgba(0,0,0,0.6)";
        g.lineWidth = 1;
        g.stroke();
      }
      if (figure && top.max_hp > 0 && top.hp < top.max_hp) {
        const w = Math.max(16, 40 * f.s);
        const h = Math.max(3, 5 * f.s);
        const x = it.ax - w / 2;
        const y = it.ay + 2;
        const p = Math.max(0, top.hp / top.max_hp);
        g.fillStyle = "rgba(0,0,0,0.7)";
        g.fillRect(x - 1, y - 1, w + 2, h + 2);
        g.fillStyle = p > 0.6 ? "#45e05c" : p > 0.3 ? "#f2c230" : "#e04030";
        g.fillRect(x, y, w * p, h);
      }
    }
  }

  minimap() {
    return null; // MapView falls back to its own top-down minimap.
  }
}

/** The box around blits `a` and `b` (either may be null) with both their parts, for picking. */
function union(a, b) {
  if (!a || !b) return a || b ? { ...(a ?? b), parts: [(a ?? b).part] } : null;
  return { x0: Math.min(a.x0, b.x0), y0: Math.min(a.y0, b.y0), x1: Math.max(a.x1, b.x1), y1: Math.max(a.y1, b.y1), parts: [a.part, b.part] };
}

function roundRect(g, x, y, w, h, r) {
  g.beginPath();
  g.moveTo(x + r, y);
  g.arcTo(x + w, y, x + w, y + h, r);
  g.arcTo(x + w, y + h, x, y + h, r);
  g.arcTo(x, y + h, x, y, r);
  g.arcTo(x, y, x + w, y, r);
  g.closePath();
}

/** A well-mixed 32-bit hash of an integer: stable per-plot choices (tile variants). */
function hash32(n) {
  let h = Math.imul(n ^ 0x9e3779b9, 0x85ebca6b);
  h ^= h >>> 13;
  h = Math.imul(h, 0xc2b2ae35);
  h ^= h >>> 16;
  return h >>> 0;
}

/** A filled star path with `points` spikes (outer radius `ro`, inner `ri`), centred at (cx, cy). */
function star(g, cx, cy, points, ro, ri) {
  g.beginPath();
  for (let i = 0; i < points * 2; i++) {
    const r = i % 2 === 0 ? ro : ri;
    const a = (Math.PI / points) * i - Math.PI / 2;
    const x = cx + Math.cos(a) * r;
    const y = cy + Math.sin(a) * r;
    i === 0 ? g.moveTo(x, y) : g.lineTo(x, y);
  }
  g.closePath();
}

/** The lookup tables, for the tests (assets/test/iso.test.mjs). */
export const ISO_TABLES = { TERRAIN_TILE, FEATURE_SPRITES, UNIT_ALIASES, COMBAT_SPRITES, DOMAIN_SPRITES, improvementSprites, resourceSprite, citySprites };
