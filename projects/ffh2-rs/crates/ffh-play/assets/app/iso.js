// An isometric renderer for the map (the "Songs of Conquest" direction, docs/art/SOC_DIRECTION.md).
//
// It is a third renderer beside the flat one (map.js) and the art one (artmap.js): chosen in
// Tools → Options (or with `?renderer=iso`), it reads the same `/api/map` + `/api/view` data the
// others do and draws the map as a 2:1 isometric board with PNG terrain tiles and unit sprites on
// team-coloured rings. The projection and hit-testing live here; MapView delegates draw(),
// screenToMap() and the drag-pan to this class when `mapView.iso` is set, and keeps its own camera,
// overlays and state.
//
// Geometry: a plot (x, y) has its centre at map point (x + 0.5, y + 0.5); the camera centre
// (cam.cx, cam.cy) is a map point and cam.ts is the tile WIDTH in CSS pixels (tile height = ts/2).
// North is up-and-right, east is down-and-right (Civ4's isometric convention).
//
// Depth: flat things (tiles, rivers, roads, rings) are painted first; everything that stands up is
// then drawn in one pass sorted by the screen y where it touches the ground, so what stands nearer
// the camera occludes what is behind it, across plots and within one (see draw).

import { S, me, tname, dname, isBarbarian } from "./state.js";
import * as T from "./tiles.js";

const ISO_ZOOMS = [32, 48, 64, 96, 128];

/** TerrainType name -> terrain tile file (under app/iso/terrain). Unmapped terrains fall back to a
 *  flat diamond in the terrain colour. */
const TERRAIN_TILE = {
  TERRAIN_GRASS: "grass",
  TERRAIN_PLAINS: "plains",
  TERRAIN_DESERT: "desert",
  TERRAIN_SNOW: "snow",
  TERRAIN_TUNDRA: "tundra",
  TERRAIN_MARSH: "marsh",
  TERRAIN_COAST: "coast",
  TERRAIN_OCEAN: "ocean",
  TERRAIN_BROKEN_LANDS: "hell",
  TERRAIN_FIELDS_OF_PERDITION: "perdition",
  TERRAIN_BURNING_SANDS: "burning",
  TERRAIN_SHALLOWS: "coast",
};

const TERRAIN_FILES = ["grass", "plains", "desert", "snow", "tundra", "marsh", "coast", "ocean", "hell", "burning", "perdition"];

/** FeatureType name -> feature sprite file (under app/iso/features) for the features drawn as one
 *  sprite on their plot. The wooded features are scattered trees instead (TREE_FEATURES); unmapped
 *  features draw nothing (the tile alone shows). */
const FEATURE_SPRITE = {
  FEATURE_OASIS: "oasis",
  FEATURE_ICE: "ice",
  FEATURE_SCRUB: "scrub",
  FEATURE_FLOOD_PLAINS: "flood",
  FEATURE_VOLCANO: "volcano",
  FEATURE_FLAMES: "flames",
};

/** Per-sprite on-screen width as a fraction of the tile; the volcano reads big, the low ground
 *  features (scrub, flood, flames) smaller. */
const FEATURE_SCALE = { oasis: 0.95, ice: 0.95, scrub: 0.8, flood: 0.82, volcano: 1.05, flames: 0.78 };

const FEATURE_FILES = ["oasis", "ice", "scrub", "flood", "volcano", "flames"];

/** The wooded features, drawn as individual trees (under app/iso/trees, two variants per kind)
 *  scattered over the plot rather than as one clump, so each tree sorts on its own depth and a unit
 *  in the forest stands among them. `scale` sizes the kind's trees (young forest is saplings);
 *  `sparse` takes the few-great-trees layout. */
const TREE_FEATURES = {
  FEATURE_FOREST: { kind: "pine", scale: 1 },
  FEATURE_FOREST_NEW: { kind: "pine", scale: 0.7 },
  FEATURE_JUNGLE: { kind: "palm", scale: 1 },
  FEATURE_FOREST_ANCIENT: { kind: "ancient", scale: 1, sparse: true },
  FEATURE_FOREST_BURNT: { kind: "dead", scale: 0.95 },
};

/** A full-size tree's height per kind, as a fraction of the tile width: about three quarters of a
 *  unit's, so even the trees of the plots in front of a unit stop below its shoulders -- it stands
 *  among the trunks with its head and weapon above the canopy, rather than lost behind it. */
const TREE_HEIGHT = { pine: 0.66, palm: 0.62, ancient: 0.7, dead: 0.62 };

const TREE_FILES = ["pine_a", "pine_b", "palm_a", "palm_b", "ancient_a", "ancient_b", "dead_a", "dead_b"];

/** Where the trees of a wooded plot stand, back to front: (u, v) in the plot's diamond (u across, v
 *  down, both in [-1, 1]); `s` scales the tree. A unit's feet are at v = 0.3, so every slot above
 *  that stands behind it and the front pair (v = 0.42) before it, pushed out to the corners to frame
 *  its sides rather than cover its middle. `clear` slots (the centre and the spot just before it)
 *  give way to a unit or structure holding the plot. `core` slots are the ones kept when zoomed far
 *  out. */
const TREE_SLOTS = [
  { u: -0.28, v: -0.66, s: 1, core: true },
  { u: 0.32, v: -0.62, s: 1 },
  { u: -0.6, v: -0.24, s: 1, core: true },
  { u: 0.62, v: -0.2, s: 1, core: true },
  { u: 0.02, v: -0.1, s: 1, clear: true },
  { u: -0.8, v: 0.1, s: 0.92 },
  { u: 0.8, v: 0.12, s: 0.92 },
  { u: -0.55, v: 0.42, s: 0.75, core: true },
  { u: 0.56, v: 0.42, s: 0.75, core: true },
  { u: 0, v: 0.66, s: 0.62, clear: true },
];

/** The ancient forest's few great trees: wide enough that one squarely in front would bury a unit,
 *  so only a smaller one stands at the front corner. */
const ANCIENT_SLOTS = [
  { u: -0.34, v: -0.5, s: 1, core: true },
  { u: 0.42, v: -0.28, s: 0.92, core: true },
  { u: -0.64, v: 0.14, s: 0.78 },
  { u: 0.08, v: 0.02, s: 0.85, clear: true },
  { u: 0.58, v: 0.4, s: 0.66, core: true },
];

/** The plots whose structure or resource icon a tree or mountain on plot (x, y) can stand in front
 *  of, as (dx, dy): its own, and the three behind it on screen (straight behind, behind-left and
 *  behind-right). */
const SIGHT = [[0, 0], [-1, 1], [-1, 0], [0, 1]];

/** Where on its plot each upright thing touches the ground, as a fraction of the tile height below
 *  the plot centre: the depth the upright pass sorts it by (trees carry their own, TREE_SLOTS). A
 *  unit stands nearest the camera, so on its own plot it is drawn over the structure, resource or
 *  city there. */
const DEPTH = { ground: 0, structure: 0.08, resource: 0.1, city: 0.12, unit: 0.15 };

/** Relief sprites (under app/iso/relief), drawn as tall overlays for a plot's relief: a peak plot
 *  gets the mountain. */
const RELIEF_FILES = ["peak"];

/** A BonusType name -> resource icon file (under app/iso/resources), by category. With ~58
 *  bonuses (over 20 of them mana nodes), one icon per visual class keeps the set small while
 *  covering every resource; the specific identity still shows on tile hover/inspect. */
function resourceSprite(type) {
  const t = type ?? "";
  if (/MANA/.test(t)) return "mana";
  if (/CLAM|CRAB|FISH|WHALE|PEARL/.test(t)) return "fish";
  if (/COW|PIG|SHEEP|DEER|HORSE|TOAD|NIGHTMARE|GULAGARM/.test(t)) return "animal";
  if (/COPPER|IRON|GOLD|GEM|JEWEL|MITHRIL|MARBLE|SHEUT|GUNPOWDER/.test(t)) return "ore";
  if (/BANANA|CORN|RICE|WHEAT|SUGAR|COTTON|YGGDRASIL|FRUIT|REAGENT|RAZORWEED/.test(t)) return "crop";
  return "goods"; // DYE, FUR, INCENSE, IVORY, SILK, WINE, ALE, FINE_CLOTHES, ...
}

const RESOURCE_FILES = ["mana", "crop", "animal", "fish", "ore", "goods"];

/** An ImprovementType name -> structure sprite (under app/iso/improvements), by category. FFH has
 *  ~81 improvements (18 of them mana nodes); one sprite per class covers them all, the exact kind
 *  showing on hover/inspect. The fallback 'lair' fits the dungeons/dens/ruins that spawn on the
 *  map. */
function improvementSprite(type) {
  const t = type ?? "";
  if (/MANA_/.test(t)) return "mananode";
  if (/FORT|CASTLE|CITADEL|TOWER|ENCLAVE|GUARDIAN|PIRATE/.test(t)) return "fort";
  if (/COTTAGE|HAMLET|VILLAGE|TOWN|CAMP|GOODY_HUT/.test(t)) return "cottage";
  if (/FARM|PASTURE|PLANTATION|FISHING_BOATS|MUSHROOM|LAND_WORKED|WINERY|VINEYARD/.test(t)) return "farm";
  if (/MINE|QUARRY|LUMBERMILL|WORKSHOP|WINDMILL|WATERMILL|SMOKEHOUSE/.test(t)) return "mine";
  return "lair"; // dungeon, barrow, dens, sepulcher, graveyard, ruins, bones, altars, ...
}

const IMPROVEMENT_FILES = ["lair", "fort", "mananode", "cottage", "farm", "mine"];

/** Per-class on-screen height as a fraction of the tile width (a farm lies flat, an obelisk rises). */
const IMPROVEMENT_SCALE = { lair: 0.78, fort: 0.72, mananode: 0.92, cottage: 0.72, farm: 0.42, mine: 0.66 };

/** A UnitType name -> unit sprite file (under app/iso/units), by keyword; else a drawn token.
 *  Civilian keywords are matched first so e.g. a scout never falls through to a combat archetype. */
function unitSpriteKey(type) {
  const t = type ?? "";
  if (/SETTLER/.test(t)) return "settler";
  if (/WORKER|SLAVE|LABORER/.test(t)) return "worker";
  if (/SCOUT|EXPLORER/.test(t)) return "scout";
  if (/CATAPULT|BALLISTA|CANNON|TREBUCHET|SIEGE|BATTERING|ONAGER/.test(t)) return "siege";
  if (/GALLEY|GALLEON|FRIGATE|WARSHIP|TRIREME|CARAVEL|PRIVATEER|LONGSHIP|MAN_O_WAR|SHIP|BOAT|BARGE/.test(t)) return "ship";
  if (/HORSEMAN|HORSE|CHARIOT|RIDER|LANCE|CAVALR|KNIGHT|CRUSADER|PALADIN/.test(t)) return "cavalry";
  if (/DEMON|MANES|\bIMP|BALOR|SUCCUBUS|HELLHOUND|REVELER|MISTRESS|INFERNAL|CHAOS_MARAUDER|PIT_BEAST|FIEND|TAR_/.test(t)) return "demon";
  if (/ARCHER|HUNTER|RANGER|MARKSMAN|LONGBOW|CROSSBOW/.test(t)) return "archer";
  if (/MAGE|ADEPT|LICH|WIZARD|SORCER|ARCHMAGE|PRIEST|SUMMONER|DRUID|CONJUR|NECROMANCER/.test(t)) return "lich";
  if (/SPIDER|BEAST|WOLF|LION|BEAR|TIGER|GORILLA|SCORPION|BROOD|ANIMAL/.test(t)) return "spider";
  if (/WARRIOR|SWORD|AXE|MACE|CHAMPION|SOLDIER|MELEE|PHALANX|SPEAR|DISCIPLE|GUARD/.test(t)) return "knight";
  return null;
}

function ownerColor(p) {
  return T.playerColor(p, isBarbarian(p));
}

export class IsoMap {
  constructor(view) {
    this.view = view;
    this.imgs = {};
    this.variants = new Map(); // sprite -> its mirrored and fog-darkened copies (see variant)
    this.ready = false;
    const load = (key, url) => {
      const im = new Image();
      im.onload = () => {
        this.imgs[key] = im;
        this.ready = true;
        this.view.request();
      };
      im.src = new URL(url, import.meta.url).href;
    };
    for (const name of TERRAIN_FILES) load(`t_${name}`, `./iso/terrain/${name}.png`);
    for (const name of FEATURE_FILES) load(`f_${name}`, `./iso/features/${name}.png`);
    for (const name of TREE_FILES) load(`w_${name}`, `./iso/trees/${name}.png`);
    for (const name of RELIEF_FILES) load(`r_${name}`, `./iso/relief/${name}.png`);
    for (const name of RESOURCE_FILES) load(`b_${name}`, `./iso/resources/${name}.png`);
    for (const name of IMPROVEMENT_FILES) load(`i_${name}`, `./iso/improvements/${name}.png`);
    load("c_town", "./iso/cities/town.png");
    for (const name of ["knight", "lich", "archer", "spider", "settler", "scout", "worker", "cavalry", "siege", "ship", "demon"]) load(`u_${name}`, `./iso/units/${name}.png`);
  }

  dispose() {}

  zooms() {
    return ISO_ZOOMS;
  }

  // --- geometry -----------------------------------------------------------------------------

  tw() {
    return this.view.cam.ts;
  }

  /** The screen centre (CSS px) of plot (x, y), taking the copy of a wrapping map nearest the
   *  camera. */
  center(x, y) {
    const { cam, vw, vh, map } = this.view;
    const TW = cam.ts;
    const TH = cam.ts / 2;
    let u = x + 0.5;
    if (map?.wrap_x) u += Math.round((cam.cx - u) / map.w) * map.w;
    const v = y + 0.5;
    const ex = u - cam.cx;
    const ny = v - cam.cy;
    return { sx: vw / 2 + (ex + ny) * (TW / 2), sy: vh / 2 + (ex - ny) * (TH / 2) };
  }

  /** The plot under screen point (sx, sy), or null off the map. */
  screenToMap(sx, sy) {
    const { cam, vw, vh, map } = this.view;
    if (!map) return null;
    const TW = cam.ts;
    const TH = cam.ts / 2;
    const A = (sx - vw / 2) / (TW / 2);
    const B = (sy - vh / 2) / (TH / 2);
    const u = cam.cx + (A + B) / 2;
    const v = cam.cy + (A - B) / 2;
    const x = T.wrapCoord(Math.floor(u), map.w, map.wrap_x);
    const y = T.wrapCoord(Math.floor(v), map.h, map.wrap_y);
    return x === null || y === null ? null : { x, y };
  }

  /** The camera centre after a drag of (dx, dy) screen pixels from (down.cx, down.cy). */
  panDelta(startCx, startCy, dx, dy) {
    const TW = this.view.cam.ts;
    const TH = TW / 2;
    return { cx: startCx - dx / TW - dy / TH, cy: startCy - dx / TW + dy / TH };
  }

  screenOf(x, y) {
    const c = this.center(x, y);
    return { x: c.sx, y: c.sy, ts: this.view.cam.ts };
  }

  // --- drawing ------------------------------------------------------------------------------

  diamond(g, sx, sy) {
    const hw = this.view.cam.ts / 2;
    const hh = this.view.cam.ts / 4;
    g.beginPath();
    g.moveTo(sx, sy - hh);
    g.lineTo(sx + hw, sy);
    g.lineTo(sx, sy + hh);
    g.lineTo(sx - hw, sy);
    g.closePath();
  }

  draw() {
    const v = this.view;
    const g = v.ctx;
    const map = v.map;
    g.setTransform(v.dpr, 0, 0, v.dpr, 0, 0);
    g.fillStyle = "#0b1017";
    g.fillRect(0, 0, v.vw, v.vh);
    if (!map) return;
    g.imageSmoothingEnabled = false;

    const TW = v.cam.ts;
    const TH = TW / 2;
    // Collect the plots in view (one copy each, nearest the camera), back to front by screen y.
    const cells = [];
    for (let y = 0; y < map.h; y++) {
      for (let x = 0; x < map.w; x++) {
        const c = this.center(x, y);
        if (c.sx < -TW || c.sx > v.vw + TW || c.sy < -TH * 2 || c.sy > v.vh + TH * 3) continue;
        cells.push({ x, y, sx: c.sx, sy: c.sy, k: y * map.w + x });
      }
    }
    cells.sort((a, b) => a.sy - b.sy);

    for (const c of cells) this.drawTile(g, c);
    const stacks = this.unitStacks();
    const cities = this.cityList(cells);
    // Everything that stands up (trees, mountains, structures, resources, cities, units) becomes an
    // item {d, o, f}: drawn by f, in one pass sorted by d, the screen y where it touches the ground
    // (o breaks ties). Whatever stands nearer the camera then occludes what is behind it -- across
    // plots, and within one: a forest plot's trees stand at their own depths, so a unit there is
    // drawn after the trees behind it and before the ones in front, standing among them.
    const items = [];
    // The map's layer (Terrain / Political / Yields, like the flat renderer): the terrain view
    // shows the full detail; the political and yield views replace it with an owner or yield
    // overlay, keeping cities and units on top.
    const layer = v.layer ?? "terrain";
    if (layer === "political") {
      this.drawPolitical(g, cells);
    } else if (layer === "yield") {
      this.drawYields(g, cells);
    } else {
      // Rivers run along the lower (south/east) edges of their plot; draw them over all tiles so a
      // nearer tile does not paint over a segment.
      this.drawRivers(g, cells);
      // Roads connect plot centres to their routed neighbours.
      this.drawRoutes(g, cells);
      this.collectTerrain(cells, stacks, cities, items);
    }
    // Team rings lie flat on the ground, so they go down before anything upright stands on them
    // (a tree rooted inside a ring is drawn over it, as it should be).
    this.drawRings(g, stacks, cities);
    this.collectCities(cities, items);
    this.collectUnits(stacks, items);
    items.sort((a, b) => a.d - b.d || a.o - b.o);
    for (const it of items) it.f(g);
    // Labels and badges last: no tree or wall may hide a city's name or a stack's count.
    this.drawCityLabels(g, cities);
    this.drawUnitBadges(g, stacks);
    this.drawOverlays(g);
    v.drawFlashes?.(g);
  }

  drawTile(g, c) {
    const v = this.view;
    const map = v.map;
    const vis = map.vis[c.k];
    if (vis === 0) return; // unrevealed: the dark background shows
    const TW = v.cam.ts;
    const TH = TW / 2;
    const terrain = tname("TerrainType", map.terrain[c.k]);
    const tileKey = TERRAIN_TILE[terrain];
    const img = tileKey && this.imgs[`t_${tileKey}`];
    if (img) {
      // A 1px bleed closes the thin seams between adjacent diamonds (a resized tile's
      // anti-aliased edge does not quite cover otherwise). A peak draws its terrain base here
      // and its mountain in the upright pass (collectTerrain).
      g.drawImage(img, Math.round(c.sx - TW / 2) - 1, Math.round(c.sy - TH / 2) - 1, TW + 2, TH + 2);
    } else {
      this.diamond(g, c.sx, c.sy);
      g.fillStyle = T.terrainColor(terrain);
      g.fill();
    }
    // Hills: a soft dome of light-over-dark implies raised ground, over any terrain colour,
    // without moving the tile (units and overlays stay anchored at the flat centre). Peaks get
    // their mountain sprite instead.
    if (map.plot_type[c.k] === this.plotTypes().hills) {
      g.save();
      this.diamond(g, c.sx, c.sy);
      g.clip();
      // A bright, tight crown of light high on the tile fading to a dark lower rim reads as a
      // convex, sunlit mound.
      const grad = g.createRadialGradient(c.sx, c.sy - TH * 0.4, TW * 0.08, c.sx, c.sy + TH * 0.35, TW * 0.66);
      grad.addColorStop(0, "rgba(255,246,222,0.32)");
      grad.addColorStop(0.34, "rgba(255,246,222,0.07)");
      grad.addColorStop(0.62, "rgba(0,0,0,0)");
      grad.addColorStop(1, "rgba(0,0,0,0.36)");
      g.fillStyle = grad;
      g.fillRect(c.sx - TW / 2, c.sy - TH / 2, TW, TH);
      g.restore();
    }
    // A faint tile outline keeps the grid legible without reading as a hard lattice.
    this.diamond(g, c.sx, c.sy);
    g.strokeStyle = "rgba(10,13,18,0.16)";
    g.lineWidth = 1;
    g.stroke();
    if (vis === 1) {
      this.diamond(g, c.sx, c.sy);
      g.fillStyle = "rgba(6,10,16,0.45)";
      g.fill();
    }
  }

  /** Rivers along plot edges. The server packs bit 0 = a river on the plot's south edge and bit 1
   *  = its east edge; in the iso projection those are the diamond's lower-left (left→bottom vertex)
   *  and lower-right (bottom→right vertex) edges. Drawn as a darker channel with a brighter core. */
  drawRivers(g, cells) {
    const v = this.view;
    const map = v.map;
    const TW = v.cam.ts;
    const hw = TW / 2;
    const hh = TW / 4;
    const path = new Path2D();
    let any = false;
    for (const c of cells) {
      if (map.vis[c.k] === 0) continue;
      const rv = map.river[c.k];
      if (rv <= 0) continue;
      if (rv & 1) {
        path.moveTo(c.sx - hw, c.sy);
        path.lineTo(c.sx, c.sy + hh);
        any = true;
      }
      if (rv & 2) {
        path.moveTo(c.sx, c.sy + hh);
        path.lineTo(c.sx + hw, c.sy);
        any = true;
      }
    }
    if (!any) return;
    g.lineCap = "round";
    g.lineJoin = "round";
    g.strokeStyle = "#235c99";
    g.lineWidth = Math.max(2.5, TW / 7);
    g.stroke(path);
    g.strokeStyle = "#6fb8ec";
    g.lineWidth = Math.max(1, TW / 14);
    g.stroke(path);
  }

  /** Roads/railroads: a line from each routed plot's centre toward every routed, revealed
   *  neighbour, meeting at the shared midpoint so a network joins up. Neighbour directions use the
   *  same iso projection as the tiles (a map step (dx, dy) is a screen step of
   *  ((dx+dy)·TW/2, (dx-dy)·TW/4)); half of that reaches the midpoint. */
  drawRoutes(g, cells) {
    const v = this.view;
    const map = v.map;
    const TW = v.cam.ts;
    const roadPath = new Path2D();
    const railPath = new Path2D();
    let anyRoad = false;
    let anyRail = false;
    for (const c of cells) {
      if (map.vis[c.k] === 0) continue;
      const rt = map.route[c.k];
      if (rt < 0) continue;
      const rail = tname("RouteType", rt) === "ROUTE_RAILROAD";
      const path = rail ? railPath : roadPath;
      let connected = false;
      for (const [dx, dy] of T.DIRS8) {
        const n = T.plotIndex(map, c.x + dx, c.y + dy);
        if (n >= 0 && map.route[n] >= 0 && map.vis[n] > 0) {
          path.moveTo(c.sx, c.sy);
          path.lineTo(c.sx + ((dx + dy) * TW) / 4, c.sy + ((dx - dy) * TW) / 8);
          connected = true;
        }
      }
      if (!connected) {
        path.moveTo(c.sx - TW * 0.1, c.sy);
        path.lineTo(c.sx + TW * 0.1, c.sy);
      }
      if (rail) anyRail = true;
      else anyRoad = true;
    }
    g.lineCap = "round";
    g.lineJoin = "round";
    if (anyRoad) {
      g.strokeStyle = "#6f5024";
      g.lineWidth = Math.max(2, TW / 10);
      g.stroke(roadPath);
      g.strokeStyle = "#a9814a";
      g.lineWidth = Math.max(1, TW / 20);
      g.stroke(roadPath);
    }
    if (anyRail) {
      g.strokeStyle = "#2a2a2a";
      g.lineWidth = Math.max(2, TW / 8);
      g.stroke(railPath);
      g.strokeStyle = "#6b6b6b";
      g.lineWidth = Math.max(1, TW / 24);
      g.stroke(railPath);
    }
  }

  /** The terrain view's upright things, as depth-sorted items (see draw): a peak's mountain, the
   *  plot's feature (scattered trees for a wooded one, else one sprite), its improvement and its
   *  resource icon. A city's plot shows no feature: the settlement stands there. Remembered-but-
   *  fogged plots draw them darkened like the tile. */
  collectTerrain(cells, stacks, cities, items) {
    const v = this.view;
    const map = v.map;
    const TW = v.cam.ts;
    const TH = TW / 2;
    const PLOT = this.plotTypes();
    const cityAt = new Set(cities.map((x) => x.c.k));
    const unitAt = new Set(stacks.map((s) => s.k));
    // The structures and resource icons by plot, and the wooded plots: their trees are placed last,
    // once everything they could bury is known.
    const keep = new Map();
    const hold = (k, it) => {
      items.push(it);
      if (keep.has(k)) keep.get(k).push(it);
      else keep.set(k, [it]);
    };
    // A mountain or one-sprite feature cannot give way like a tree: a structure or resource icon
    // behind it that it would bury is drawn just after it instead, over it. (Cells run back to
    // front, so what stands behind is already kept.)
    const rise = (c, it) => {
      items.push(it);
      for (const [dx, dy] of SIGHT) {
        for (const b of keep.get(T.plotIndex(map, c.x + dx, c.y + dy)) ?? []) if (buries(it, b)) b.d = it.d;
      }
    };
    const woods = [];
    for (const c of cells) {
      const vis = map.vis[c.k];
      if (vis === 0) continue; // unrevealed
      const fog = vis === 1;
      const imp = map.improvement[c.k];
      const bonus = map.bonus[c.k];
      const built = imp != null && imp >= 0;
      const marked = bonus != null && bonus >= 0;
      if (map.plot_type[c.k] === PLOT.peak) {
        // Mountains overhang the tile and rise highest.
        const img = this.imgs.r_peak;
        if (img) rise(c, this.stand(img, c.sx, c.sy + TH * 0.12, TW * 1.08 * (img.height / img.width), c.sy + TH * DEPTH.ground, 0, fog));
      } else if (!cityAt.has(c.k)) {
        const feat = tname("FeatureType", map.feature[c.k]);
        const wood = feat && TREE_FEATURES[feat];
        if (wood) {
          woods.push({ c, wood, held: unitAt.has(c.k) || built, fog });
        } else if (feat && FEATURE_SPRITE[feat]) {
          const key = FEATURE_SPRITE[feat];
          const img = this.imgs[`f_${key}`];
          if (img) rise(c, this.stand(img, c.sx, c.sy + TH * 0.18, TW * FEATURE_SCALE[key] * (img.height / img.width), c.sy + TH * DEPTH.ground, 0, fog));
        }
      }
      if (built) {
        // Sized per class, so a farm lies flat and a mana obelisk towers.
        const key = improvementSprite(tname("ImprovementType", imp));
        const img = this.imgs[`i_${key}`];
        if (img) hold(c.k, this.stand(img, c.sx, c.sy + TH * 0.16, TW * (IMPROVEMENT_SCALE[key] ?? 0.72), c.sy + TH * DEPTH.structure, 2, fog));
      }
      if (marked) {
        // Drawn small at the front of the plot but sorted at its middle: in front of the plot's
        // structure, so a worked resource still shows, yet never over a unit standing there.
        const img = this.imgs[`b_${resourceSprite(tname("BonusType", bonus))}`];
        if (img) hold(c.k, this.stand(img, c.sx, c.sy + TH * 0.3, TW * 0.42 * (img.height / img.width), c.sy + TH * DEPTH.resource, 3, fog));
      }
    }
    for (const w of woods) this.collectTrees(w.c, w.wood, w.held, w.fog, keep, items);
  }

  /** A wooded plot's trees: one per slot of its layout (TREE_SLOTS), jittered per plot -- a stable
   *  hash of its index -- in position, size, mirroring and variant, so neighbouring forests never
   *  repeat. Each tree sorts on its own base: the back and side trees stand behind a unit at the
   *  plot centre, the front pair before its feet. The centre and the spot just in front of it clear
   *  for a unit or structure holding the plot (`held`), so what stands there is framed by trees
   *  rather than buried in them. A structure or resource icon is shorter than the trees, so any tree
   *  that would hide much of one -- on this plot or a plot behind it, `keep` -- is left out, opening
   *  a glade in its line of sight. Zoomed far out, only the core slots are drawn. */
  collectTrees(c, wood, held, fog, keep, items) {
    const map = this.view.map;
    const TW = this.view.cam.ts;
    const TH = TW / 2;
    const far = TW < 40;
    const slots = wood.sparse ? ANCIENT_SLOTS : TREE_SLOTS;
    const behind = SIGHT.flatMap(([dx, dy]) => keep.get(T.plotIndex(map, c.x + dx, c.y + dy)) ?? []);
    for (let i = 0; i < slots.length; i++) {
      const s = slots[i];
      if (far && !s.core) continue;
      if (s.clear && held) continue;
      const h = hash32(c.k * 32 + i);
      const img = this.imgs[`w_${wood.kind}_${h & 1 ? "b" : "a"}`];
      if (!img) continue;
      const u = s.u + (((h >>> 8) & 255) / 255 - 0.5) * 0.16;
      const w = s.v + (((h >>> 16) & 255) / 255 - 0.5) * 0.12;
      const size = wood.scale * s.s * (0.88 + ((h >>> 24) / 255) * 0.24);
      const bx = c.sx + (u * TW) / 2;
      const by = c.sy + (w * TH) / 2;
      const tree = this.stand(img, bx, by, TW * TREE_HEIGHT[wood.kind] * size, by, 1, fog, (h & 2) !== 0);
      if (!behind.some((b) => buries(tree, b))) items.push(tree);
    }
  }

  /** An upright sprite standing with its bottom centre at (bx, by), `h` tall (its width from its
   *  aspect), as a depth-sorted item at depth `d`, ties broken by `o`, carrying its screen box
   *  (x, y, w, h). */
  stand(img, bx, by, h, d, o, fog = false, flip = false) {
    const w = h * (img.width / img.height);
    const src = this.variant(img, flip, fog);
    const box = { x: Math.round(bx - w / 2), y: Math.round(by - h), w: Math.round(w), h: Math.round(h) };
    return { d, o, ...box, f: (g) => g.drawImage(src, box.x, box.y, box.w, box.h) };
  }

  /** `img` mirrored and/or darkened to the fog's tone (a remembered plot out of sight), made once
   *  and cached. The darkening paints over the sprite's opaque pixels only, so overlapping fogged
   *  trees stay solid where a lowered alpha would turn them see-through. */
  variant(img, flip, fog) {
    if (!flip && !fog) return img;
    let made = this.variants.get(img);
    if (!made) this.variants.set(img, (made = []));
    const key = (flip ? 1 : 0) + (fog ? 2 : 0);
    if (!made[key]) {
      const cv = document.createElement("canvas");
      cv.width = img.width;
      cv.height = img.height;
      const cg = cv.getContext("2d");
      if (flip) {
        cg.translate(img.width, 0);
        cg.scale(-1, 1);
      }
      cg.drawImage(img, 0, 0);
      if (fog) {
        cg.setTransform(1, 0, 0, 1, 0, 0);
        cg.globalCompositeOperation = "source-atop";
        cg.fillStyle = "rgba(6,10,16,0.45)"; // the tile fog's tone (drawTile)
        cg.fillRect(0, 0, cv.width, cv.height);
      }
      made[key] = cv;
    }
    return made[key];
  }

  /** The Political view: each owner's colour washed over its plots (unowned land a faint grey),
   *  with a bold territory outline along every edge to a differently-owned plot. */
  drawPolitical(g, cells) {
    const v = this.view;
    const map = v.map;
    const TW = v.cam.ts;
    const hw = TW / 2;
    const hh = TW / 4;
    const PLOT = this.plotTypes();
    for (const c of cells) {
      if (map.vis[c.k] === 0) continue;
      const owner = map.owner[c.k];
      if (owner >= 0) {
        this.diamond(g, c.sx, c.sy);
        g.fillStyle = ownerColor(owner);
        g.globalAlpha = 0.5;
        g.fill();
        g.globalAlpha = 1;
      } else if (map.plot_type[c.k] !== PLOT.ocean) {
        this.diamond(g, c.sx, c.sy);
        g.fillStyle = "rgba(150,150,150,0.3)";
        g.fill();
      }
    }
    g.lineWidth = Math.max(2, TW * 0.05);
    g.lineCap = "round";
    for (const c of cells) {
      if (map.vis[c.k] === 0) continue;
      const owner = map.owner[c.k];
      if (owner < 0) continue;
      const nb = (dx, dy) => {
        const n = T.plotIndex(map, c.x + dx, c.y + dy);
        return n >= 0 ? map.owner[n] : -2;
      };
      g.strokeStyle = ownerColor(owner);
      g.beginPath();
      if (nb(0, 1) !== owner) (g.moveTo(c.sx, c.sy - hh), g.lineTo(c.sx + hw, c.sy)); // N edge: top->right
      if (nb(1, 0) !== owner) (g.moveTo(c.sx, c.sy + hh), g.lineTo(c.sx + hw, c.sy)); // E edge: bottom->right
      if (nb(0, -1) !== owner) (g.moveTo(c.sx - hw, c.sy), g.lineTo(c.sx, c.sy + hh)); // S edge: left->bottom
      if (nb(-1, 0) !== owner) (g.moveTo(c.sx - hw, c.sy), g.lineTo(c.sx, c.sy - hh)); // W edge: left->top
      g.stroke();
    }
  }

  /** The Yields view: the board darkened, with the food/production/commerce of every in-sight plot
   *  drawn in their colours near the plot centre. */
  drawYields(g, cells) {
    const v = this.view;
    const map = v.map;
    const TW = v.cam.ts;
    for (const c of cells) {
      if (map.vis[c.k] === 0) continue;
      this.diamond(g, c.sx, c.sy);
      g.fillStyle = "rgba(0,0,0,0.5)";
      g.fill();
    }
    if (TW < 28) return; // too small for legible numbers
    g.font = `bold ${Math.max(9, Math.round(TW * 0.2))}px system-ui, sans-serif`;
    g.textAlign = "center";
    g.textBaseline = "middle";
    for (const c of cells) {
      if (map.vis[c.k] !== 2) continue; // only fully-visible plots show yields
      const y = T.unpackYield(map.yield[c.k]);
      if (!y) continue;
      const parts = [];
      y.forEach((val, j) => {
        if (val > 0) parts.push([String(val), T.YIELD_COLORS[j]]);
      });
      if (!parts.length) continue;
      const spacing = TW * 0.22;
      parts.forEach(([val, col], i) => {
        const ox = (i - (parts.length - 1) / 2) * spacing;
        g.fillStyle = "rgba(0,0,0,0.65)";
        g.fillText(val, c.sx + ox + 1, c.sy + 1);
        g.fillStyle = col;
        g.fillText(val, c.sx + ox, c.sy);
      });
    }
  }

  /** The cities in view, each with its plot cell. */
  cityList(cells) {
    const byPlot = new Map(cells.map((c) => [c.k, c]));
    const w = this.view.map.w;
    const out = [];
    for (const city of S.cities.values()) {
      const c = byPlot.get(city.y * w + city.x);
      if (c) out.push({ city, c });
    }
    return out;
  }

  /** The unit stacks in view: their units, the one shown on top, its plot's centre and index, and
   *  whether it is selected. */
  unitStacks() {
    const v = this.view;
    const TW = v.cam.ts;
    const TH = TW / 2;
    const mine = me()?.player;
    const sel = v.overlay.units ?? [];
    const out = [];
    for (const units of S.unitsAt.values()) {
      const u0 = units[0];
      const c = this.center(u0.x, u0.y);
      if (c.sx < -TW || c.sx > v.vw + TW || c.sy < -TH * 3 || c.sy > v.vh + TH * 2) continue;
      const top = T.topUnit(units, mine, sel);
      out.push({ units, top, c, k: u0.y * v.map.w + u0.x, picked: sel.includes(top.id) });
    }
    return out;
  }

  /** The team rings (iso ellipses) under cities and unit stacks, flat on the ground, back to front:
   *  gold-rimmed for a capital, bright-rimmed for the selected stack. */
  drawRings(g, stacks, cities) {
    const TW = this.view.cam.ts;
    const TH = TW / 2;
    const ring = (c, r, fill, rim, width) => {
      g.beginPath();
      g.ellipse(c.sx, c.sy, TW * r, TH * r, 0, 0, Math.PI * 2);
      g.fillStyle = fill;
      g.globalAlpha = 0.9;
      g.fill();
      g.globalAlpha = 1;
      g.lineWidth = width;
      g.strokeStyle = rim;
      g.stroke();
    };
    const bySy = (a, b) => a.c.sy - b.c.sy;
    for (const { city, c } of [...cities].sort(bySy)) {
      ring(c, 0.42, ownerColor(city.owner), city.capital ? "#ffe9a8" : "rgba(10,10,12,0.5)", Math.max(2, TW * 0.055));
    }
    for (const s of [...stacks].sort(bySy)) {
      ring(s.c, 0.36, ownerColor(s.top.owner), s.picked ? "#fff7a0" : "rgba(10,10,12,0.5)", Math.max(2, TW * 0.05));
    }
  }

  /** Each city's settlement sprite and owner banner (a labelled box until the sprite loads), as a
   *  depth-sorted item. */
  collectCities(cities, items) {
    const TW = this.view.cam.ts;
    const TH = TW / 2;
    const img = this.imgs.c_town;
    for (const { city, c } of cities) {
      const col = ownerColor(city.owner);
      items.push({ d: c.sy + TH * DEPTH.city, o: 4, f: (g) => {
        if (!img) {
          const s = TW * 0.5;
          g.fillStyle = "rgba(12,14,20,0.82)";
          roundRect(g, c.sx - s / 2, c.sy - s * 0.3, s, s * 0.5, 3);
          g.fill();
          g.strokeStyle = col;
          g.lineWidth = 2;
          g.stroke();
          return;
        }
        // The settlement, rising from just below the tile centre.
        const cw = TW * 1.02;
        const ch = cw * (img.height / img.width);
        const top = c.sy + TH * DEPTH.city - ch;
        g.drawImage(img, Math.round(c.sx - cw / 2), Math.round(top), Math.round(cw), Math.round(ch));
        // Owner banner: a per-team pennant (the neutral flag tinted to the owner's colour) flying
        // from a short pole at the settlement's shoulder, reinforcing the team ring.
        const px = c.sx - cw * 0.22;
        const poleTop = top + ch * 0.1;
        g.strokeStyle = "#2c241a";
        g.lineWidth = Math.max(1.5, TW * 0.022);
        g.beginPath();
        g.moveTo(px, poleTop);
        g.lineTo(px, top + ch * 0.46);
        g.stroke();
        const pw = TW * 0.22;
        const ph = TW * 0.15;
        g.beginPath();
        g.moveTo(px, poleTop);
        g.lineTo(px + pw, poleTop + ph * 0.38);
        g.lineTo(px, poleTop + ph);
        g.closePath();
        g.fillStyle = col;
        g.fill();
        g.strokeStyle = "rgba(0,0,0,0.55)";
        g.lineWidth = 1;
        g.stroke();
      } });
    }
  }

  /** The capital star, size badge and name chip of each city, drawn over everything. */
  drawCityLabels(g, cities) {
    const TW = this.view.cam.ts;
    const TH = TW / 2;
    const mine = me()?.player;
    for (const { city, c } of cities) {
      const col = ownerColor(city.owner);
      // Capital crown pip above the settlement.
      if (city.capital) {
        g.fillStyle = "#ffdd55";
        g.strokeStyle = "rgba(10,10,12,0.6)";
        g.lineWidth = 1;
        star(g, c.sx, c.sy - TW * 0.92, 5, Math.max(3, TW * 0.08), Math.max(1.5, TW * 0.035));
        g.fill();
        g.stroke();
      }
      // Size badge at the lower-left of the base.
      if (city.size != null) {
        const r = Math.max(7, TW * 0.15);
        const bx = c.sx - TW * 0.34;
        const by = c.sy + TH * 0.02;
        g.beginPath();
        g.arc(bx, by, r, 0, Math.PI * 2);
        g.fillStyle = "rgba(16,13,12,0.86)";
        g.fill();
        g.lineWidth = Math.max(1.5, TW * 0.025);
        g.strokeStyle = col;
        g.stroke();
        g.fillStyle = city.owner === mine ? "#ffe9a8" : "#f4f0e6";
        g.font = `bold ${Math.round(r * 1.1)}px system-ui, sans-serif`;
        g.textAlign = "center";
        g.textBaseline = "middle";
        g.fillText(String(city.size), bx, by + 0.5);
      }
      // City name chip below the ring, only when there is room.
      if (city.name && TW >= 52) {
        const fs = Math.max(10, Math.round(TW * 0.17));
        g.font = `600 ${fs}px system-ui, sans-serif`;
        const tw2 = g.measureText(city.name).width;
        const pad = fs * 0.5;
        const bw = tw2 + pad * 2;
        const bh = fs + 6;
        const bx = c.sx - bw / 2;
        const by = c.sy + TH * 0.42;
        g.fillStyle = "rgba(12,14,20,0.8)";
        roundRect(g, bx, by, bw, bh, 3);
        g.fill();
        g.fillStyle = city.owner === mine ? "#ffe9a8" : "#e8e4da";
        g.textAlign = "center";
        g.textBaseline = "middle";
        g.fillText(city.name, c.sx, by + bh / 2 + 0.5);
      }
    }
  }

  /** Each stack's figure -- its top unit's sprite when the type maps to one, else a labelled token --
   *  as a depth-sorted item standing at the plot centre. */
  collectUnits(stacks, items) {
    const TW = this.view.cam.ts;
    const TH = TW / 2;
    for (const { top, c, picked } of stacks) {
      const d = c.sy + TH * DEPTH.unit;
      const key = unitSpriteKey(tname("UnitType", top.type));
      const img = key && this.imgs[`u_${key}`];
      if (img) {
        items.push(this.stand(img, c.sx, d, TW * 0.98, d, 5));
        continue;
      }
      items.push({ d, o: 5, f: (g) => {
        const s = TW * 0.42;
        g.fillStyle = ownerColor(top.owner);
        g.strokeStyle = picked ? "#fff7a0" : "rgba(0,0,0,0.85)";
        g.lineWidth = 1.5;
        roundRect(g, c.sx - s / 2, c.sy - TH * 0.2 - s, s, s, Math.max(2, s / 6));
        g.fill();
        g.stroke();
        g.fillStyle = "#101010";
        g.font = `bold ${Math.round(s * 0.42)}px system-ui, sans-serif`;
        g.textAlign = "center";
        g.textBaseline = "middle";
        g.fillText(T.abbrev(dname("UnitType", top.type)), c.sx, c.sy - TH * 0.2 - s / 2 + 0.5);
      } });
    }
  }

  /** Each stack's count badge and, for the player's own, its moves pip, drawn over everything; and
   *  the front arc of its team ring again, so whose unit it is (and which is selected) still shows
   *  through the trees before it. Only the front arc: the back one passes behind the figure's legs,
   *  and on open ground the front arc lands on the one already drawn. */
  drawUnitBadges(g, stacks) {
    const TW = this.view.cam.ts;
    const TH = TW / 2;
    const mine = me()?.player;
    for (const { units, top, c, picked } of stacks) {
      g.beginPath();
      g.ellipse(c.sx, c.sy, TW * 0.36, TH * 0.36, 0, 0, Math.PI);
      g.lineWidth = Math.max(1.5, TW * 0.03);
      g.strokeStyle = picked ? "#fff7a0" : ownerColor(top.owner);
      g.stroke();
      // Count badge.
      if (units.length > 1) {
        const t = String(units.length);
        g.font = `bold ${Math.max(10, Math.round(TW * 0.2))}px system-ui, sans-serif`;
        const wch = g.measureText(t).width + 6;
        const bx = c.sx - TW * 0.34 - wch;
        const by = c.sy - TW * 0.7;
        g.fillStyle = "rgba(16,13,12,0.85)";
        roundRect(g, bx, by, wch, Math.max(13, TW * 0.22), 3);
        g.fill();
        g.fillStyle = "#f4f0e6";
        g.textAlign = "center";
        g.textBaseline = "middle";
        g.fillText(t, bx + wch / 2, by + Math.max(13, TW * 0.22) / 2 + 0.5);
      }
      // Moves pip for own units.
      if (top.owner === mine) {
        const r = Math.max(2.5, TW * 0.05);
        g.fillStyle = top.moves > 0 ? "#3fdc5a" : "#d03a2a";
        g.beginPath();
        g.arc(c.sx + TW * 0.28, c.sy - TW * 0.62, r, 0, Math.PI * 2);
        g.fill();
      }
    }
  }

  drawOverlays(g) {
    const o = this.view.overlay;
    const ring = (x, y, color, width, dash = []) => {
      const c = this.center(x, y);
      this.diamond(g, c.sx, c.sy);
      g.strokeStyle = color;
      g.lineWidth = width;
      g.setLineDash(dash);
      g.stroke();
      g.setLineDash([]);
    };
    if (Array.isArray(o.targets)) for (const [x, y] of o.targets) ring(x, y, "#4fe3ff", 2, [4, 3]);
    if (o.sel) ring(o.sel.x, o.sel.y, "#ffe14d", 2.5);
    if (o.hover) ring(o.hover.x, o.hover.y, o.targets ? "#4fe3ff" : "rgba(255,255,255,0.7)", 1.5);
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

  minimap() {
    return null; // MapView falls back to its own top-down minimap.
  }
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

/** Whether upright item `a` is drawn after `b` and hides more than a fifth of it (by their boxes). */
function buries(a, b) {
  if (a.d <= b.d) return false;
  const w = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
  const h = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
  return w > 0 && h > 0 && w * h > 0.2 * b.w * b.h;
}

/** A well-mixed 32-bit hash of an integer: stable per-plot randomness for tree placement. */
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
