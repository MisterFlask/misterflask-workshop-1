// Map maths without the DOM (tested by assets/test/tiles.test.mjs): wrap-aware coordinates, the
// camera, the map arrays' encodings, the terrain cache's chunks, and the colours and glyphs the
// renderer uses.
//
// Plots are `x` (east) and `y` (north): row 0 of the map is its south edge, and the map is drawn
// north up (design §5.1). A plot covers [x, x+1) x [y, y+1) in map units; the camera's centre
// (`cx`, `cy`) is in map units and `ts` is the tile size in CSS pixels.

/** The eight neighbours, as (dx, dy). */
export const DIRS8 = [
  [0, 1], [1, 1], [1, 0], [1, -1], [0, -1], [-1, -1], [-1, 0], [-1, 1],
];

/** `v` on an axis of `n` plots: wrapped when `wrap`, else `null` off the map. */
export function wrapCoord(v, n, wrap) {
  if (wrap) return ((v % n) + n) % n;
  return v >= 0 && v < n ? v : null;
}

/** The index of plot (x, y) in the map arrays (`y * w + x`, wrap applied), or -1 off the map. */
export function plotIndex(map, x, y) {
  const wx = wrapCoord(x, map.w, map.wrap_x);
  const wy = wrapCoord(y, map.h, map.wrap_y);
  return wx === null || wy === null ? -1 : wy * map.w + wx;
}

/** The plot of an index. */
export function plotXY(map, i) {
  return { x: i % map.w, y: Math.floor(i / map.w) };
}

/** Civ4 `plotDistance`: max(dx, dy) + min(dx, dy) / 2 (integer), with wrap. */
export function plotDistance(map, ax, ay, bx, by) {
  let dx = Math.abs(ax - bx);
  let dy = Math.abs(ay - by);
  if (map.wrap_x) dx = Math.min(dx, map.w - dx);
  if (map.wrap_y) dy = Math.min(dy, map.h - dy);
  return Math.max(dx, dy) + Math.floor(Math.min(dx, dy) / 2);
}

/** The plot under screen point (sx, sy) of a `vw` x `vh` viewport: `{x, y}` (x wrapped), or
 *  `null` off the map. */
export function screenToMap(cam, map, vw, vh, sx, sy) {
  const fx = cam.cx + (sx - vw / 2) / cam.ts;
  const fy = cam.cy - (sy - vh / 2) / cam.ts;
  const x = wrapCoord(Math.floor(fx), map.w, map.wrap_x);
  const y = wrapCoord(Math.floor(fy), map.h, map.wrap_y);
  return x === null || y === null ? null : { x, y };
}

/** The screen position of the top-left corner of plot (x, y) taken as unwrapped column `ux`
 *  (the map's copies east and west are columns x + k * w). */
export function columnToScreen(cam, vw, vh, ux, y) {
  return { sx: vw / 2 + (ux - cam.cx) * cam.ts, sy: vh / 2 - (y + 1 - cam.cy) * cam.ts };
}

/** The top-left corner of plot (x, y) on the screen: the copy nearest the camera's centre. */
export function mapToScreen(cam, map, vw, vh, x, y) {
  let ux = x;
  if (map.wrap_x) {
    const k = Math.round((cam.cx - (x + 0.5)) / map.w);
    ux = x + k * map.w;
  }
  return columnToScreen(cam, vw, vh, ux, y);
}

/** The unwrapped columns `ux` of plot column `x` that show in the viewport (a narrow map can
 *  show several copies). */
export function copiesOf(cam, map, vw, x) {
  const left = cam.cx - vw / 2 / cam.ts - 1;
  const right = cam.cx + vw / 2 / cam.ts + 1;
  if (!map.wrap_x) return x >= left && x <= right ? [x] : [];
  const out = [];
  const k0 = Math.floor((left - x) / map.w);
  for (let k = k0; x + k * map.w <= right; k++) {
    if (x + k * map.w >= left) out.push(x + k * map.w);
  }
  return out;
}

/** The plots in view: unwrapped columns [x0, x1] and rows [y0, y1] (rows clamped to the map). */
export function visibleRange(cam, map, vw, vh) {
  const x0 = Math.floor(cam.cx - vw / 2 / cam.ts) - 1;
  const x1 = Math.ceil(cam.cx + vw / 2 / cam.ts) + 1;
  const y0 = Math.max(0, Math.floor(cam.cy - vh / 2 / cam.ts) - 1);
  const y1 = Math.min(map.h - 1, Math.ceil(cam.cy + vh / 2 / cam.ts) + 1);
  return { x0: map.wrap_x ? x0 : Math.max(0, x0), x1: map.wrap_x ? x1 : Math.min(map.w - 1, x1), y0, y1 };
}

/** The camera kept on the map: the centre's column wraps (or is clamped), its row is clamped. */
export function clampCamera(cam, map) {
  const cx = map.wrap_x ? ((cam.cx % map.w) + map.w) % map.w : Math.min(Math.max(cam.cx, 0), map.w);
  const cy = Math.min(Math.max(cam.cy, 0), map.h);
  return { ...cam, cx, cy };
}

/** The camera zoomed to tile size `ts`, keeping the map point under (sx, sy) in place. */
export function zoomAt(cam, vw, vh, ts, sx, sy) {
  const fx = cam.cx + (sx - vw / 2) / cam.ts;
  const fy = cam.cy - (sy - vh / 2) / cam.ts;
  return { cx: fx - (sx - vw / 2) / ts, cy: fy + (sy - vh / 2) / ts, ts };
}

/** The `river` value of a plot (design §4.4): a river on the south edge (`n_of_river`), on the
 *  east edge (`w_of_river`), and the flows (CardinalDirectionType index, -1 none). */
export function riverEdges(v) {
  if (v < 0) return { south: false, east: false, ns: -1, we: -1 };
  return { south: (v & 1) !== 0, east: (v & 2) !== 0, ns: ((v >> 2) & 7) - 1, we: ((v >> 5) & 7) - 1 };
}

/** A packed `yield` value as [food, production, commerce], or `null` (-1: not visible). */
export function unpackYield(v) {
  return v < 0 ? null : [v & 255, (v >> 8) & 255, (v >> 16) & 255];
}

/** The terrain cache's chunks: CHUNK x CHUNK plots. */
export const CHUNK = 8;

export function chunkOf(x, y) {
  return { cx: Math.floor(x / CHUNK), cy: Math.floor(y / CHUNK) };
}

/** The chunks a plot's drawing depends on: its own and, for border and road lines drawn
 *  toward neighbours, those of its neighbours. */
function chunksAround(map, x, y, out) {
  for (const [dx, dy] of [[0, 0], ...DIRS8]) {
    const i = plotIndex(map, x + dx, y + dy);
    if (i < 0) continue;
    const p = plotXY(map, i);
    const c = chunkOf(p.x, p.y);
    out.add(`${c.cx},${c.cy}`);
  }
}

/** The map arrays of a `map` reply (design §4.4), in the server's order. */
export const MAP_LAYERS = ["vis", "plot_type", "terrain", "feature", "bonus", "improvement", "route", "owner", "river", "city", "yield"];

/** The map arrays the terrain cache draws from (every layer: the yield layer draws `yield`). */
export const STATIC_LAYERS = MAP_LAYERS;

/** The chunks to redraw between two maps (`"all"` for another map or none before), as a Set
 *  of `"cx,cy"`. */
export function dirtyChunks(prev, next) {
  if (!prev || prev.w !== next.w || prev.h !== next.h || prev.wrap_x !== next.wrap_x) return "all";
  const out = new Set();
  const n = next.w * next.h;
  for (let i = 0; i < n; i++) {
    if (STATIC_LAYERS.some((k) => prev[k][i] !== next[k][i])) {
      const { x, y } = plotXY(next, i);
      chunksAround(next, x, y, out);
    }
  }
  return out;
}

/** The chunks to redraw for the tiles `tiles` (indices) that changed, as a Set of `"cx,cy"`. */
export function chunksOfTiles(map, tiles) {
  const out = new Set();
  for (const i of tiles) {
    const { x, y } = plotXY(map, i);
    chunksAround(map, x, y, out);
  }
  return out;
}

/** A `map` reply applied to the map the client holds (design §4.4, phase 6 deltas): a full
 *  reply (`full: true`) is the new map; a delta (`full: false`, `tiles` = the indices of the
 *  changed tiles, each layer the values for them in that order) is applied to a copy of
 *  `prev`, which must be the map of revision `since` of the same `epoch`. Returns `{map,
 *  changed}`: `changed` is the tile indices, or "all". Throws when a delta does not fit. */
export function applyMapDelta(prev, reply) {
  if (reply.full !== false) return { map: reply, changed: "all" };
  if (!prev || prev.epoch !== reply.epoch || prev.rev !== reply.since || prev.w !== reply.w || prev.h !== reply.h) {
    throw new Error(`map delta since ${reply.since} (${reply.epoch}) does not fit the map held (${prev?.rev}, ${prev?.epoch})`);
  }
  const tiles = reply.tiles ?? [];
  const map = { ...reply, full: true };
  delete map.tiles;
  delete map.since;
  for (const k of MAP_LAYERS) {
    const a = prev[k].slice();
    const v = reply[k];
    if (!Array.isArray(v) || v.length !== tiles.length) throw new Error(`map delta: layer ${k} has ${v?.length} values for ${tiles.length} tiles`);
    for (let j = 0; j < tiles.length; j++) a[tiles[j]] = v[j];
    map[k] = a;
  }
  return { map, changed: tiles };
}

/** The yield colours (food, production, commerce) of the yield layer. */
export const YIELD_COLORS = ["#8fe07a", "#e0a050", "#f2dc5a"];

/** The map's layers (the map's own drawing modes, design §5.1 "Layers"). */
export const LAYERS = ["terrain", "political", "yield"];

/** A stable colour for any string. */
export function hashColor(s, sat = 40, light = 45) {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return `hsl(${h % 360}, ${sat}%, ${light}%)`;
}

/** Terrain colours keyed by type name (FfH hell terrains included); others hash. */
export const TERRAIN_COLORS = {
  TERRAIN_GRASS: "#4c7a33",
  TERRAIN_PLAINS: "#8c8a3e",
  TERRAIN_DESERT: "#cdb877",
  TERRAIN_MARSH: "#4b6a5c",
  TERRAIN_TUNDRA: "#7d8a78",
  TERRAIN_SNOW: "#dfe7ec",
  TERRAIN_BURNING_SANDS: "#b4532c",
  TERRAIN_BROKEN_LANDS: "#6c5848",
  TERRAIN_FIELDS_OF_PERDITION: "#5e2a3c",
  TERRAIN_SHALLOWS: "#3f8fa6",
  TERRAIN_COAST: "#2c6894",
  TERRAIN_OCEAN: "#1b3b68",
  TERRAIN_PEAK: "#6c6863",
  TERRAIN_HILL: "#7a6c4e",
};

export function terrainColor(type) {
  return TERRAIN_COLORS[type] ?? hashColor(type ?? "?", 30, 40);
}

/** Feature glyphs and colours by type name: `glyph` drawn on the plot, `tint` laid over it. */
export const FEATURES = {
  FEATURE_ICE: { glyph: "", tint: "rgba(235,245,255,0.75)" },
  FEATURE_JUNGLE: { glyph: "♠", color: "#1d4a1a", tint: "rgba(20,70,20,0.35)" },
  FEATURE_OASIS: { glyph: "●", color: "#3fa7d6" },
  FEATURE_FLOOD_PLAINS: { glyph: "≈", color: "#d7d39a" },
  FEATURE_FOREST: { glyph: "♣", color: "#1f4d22" },
  FEATURE_FOREST_BURNT: { glyph: "♣", color: "#3b2a1c" },
  FEATURE_FOREST_NEW: { glyph: "♣", color: "#5e8f3a" },
  FEATURE_FALLOUT: { glyph: "☢", color: "#9acd32", tint: "rgba(120,160,40,0.3)" },
  FEATURE_FOREST_ANCIENT: { glyph: "♣", color: "#0e3314", tint: "rgba(10,40,15,0.25)" },
  FEATURE_FLAMES: { glyph: "▲", color: "#ff7a1a", tint: "rgba(255,90,0,0.3)" },
  FEATURE_OBSIDIAN_PLAINS: { glyph: "", tint: "rgba(20,10,20,0.45)" },
  FEATURE_VOLCANO: { glyph: "▲", color: "#d0301c" },
  FEATURE_SCRUB: { glyph: "·", color: "#6b5f2e" },
  FEATURE_BLIZZARD: { glyph: "*", color: "#f4fbff", tint: "rgba(255,255,255,0.3)" },
  FEATURE_TORMENTED_SOULS: { glyph: "§", color: "#c6b1d8", tint: "rgba(90,40,90,0.3)" },
  FEATURE_WALLS: { glyph: "#", color: "#b0a89a" },
};

export function featureStyle(type) {
  if (!type) return null;
  return FEATURES[type] ?? { glyph: type.replace(/^FEATURE_/, "").slice(0, 1), color: "#ddd" };
}

/** Player colours by player id; the barbarians are dark red. */
export const PLAYER_COLORS = [
  "#e0c03c", "#3c7fe0", "#d8452f", "#3fb06a", "#a35bd6", "#e08a2c", "#3fc1c9", "#d659a3",
  "#8fb83a", "#6b6bd6", "#c97f5a", "#4fa3a3", "#b8b8b8", "#d6d65b", "#7f4fd6", "#5bd6a0",
];

export function playerColor(id, barbarian = false) {
  if (barbarian) return "#5a1010";
  if (id === null || id === undefined || id < 0) return "#777";
  return PLAYER_COLORS[id % PLAYER_COLORS.length];
}

/** A short label for a name: the initials of two or more words ("Great Commander" -> "GC"),
 *  else the first letters ("Warrior" -> "Wa"). */
export function abbrev(name, n = 2) {
  const words = String(name ?? "?").replace(/[^\p{L}\p{N} ]/gu, " ").split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  if (words.length >= 2) return words.slice(0, n).map((w) => w[0].toUpperCase()).join("");
  const w = words[0];
  return w[0].toUpperCase() + w.slice(1, n).toLowerCase();
}

/** A type name for people: "IMPROVEMENT_GOBLIN_FORT" -> "Goblin Fort". */
export function humanize(type) {
  return String(type ?? "")
    .replace(/^[A-Z]+_/, "")
    .toLowerCase()
    .split("_")
    .filter(Boolean)
    .map((w) => w[0].toUpperCase() + w.slice(1))
    .join(" ");
}

/** Improvements drawn as lairs (barbarian spawns and dungeons, which FfH marks unique or
 *  names so). */
export function isLair(type, info) {
  return /LAIR|BARROW|DEN$|DUNGEON|GOBLIN_FORT|RUINS|GRAVEYARD|HELLFIRE|PIRATE|SHIP_WRECK|TOMB|SEPULCHER|CAGE/.test(type ?? "") || (info?.unique ?? false);
}

/** How a plot's top unit is chosen: the selection's first, else the viewer's own unit that is
 *  not busy, else the first listed. `units` are the view's units on one plot. */
export function topUnit(units, me, selected = []) {
  const sel = units.find((u) => selected.includes(u.id));
  if (sel) return sel;
  const own = units.filter((u) => u.owner === me);
  if (own.length) return own.find((u) => (u.moves ?? 0) > 0) ?? own[0];
  return units[0] ?? null;
}

/** Turn markers along a path (`path` query nodes, the first being the start): `{i, turn,
 *  last}` for each node where a turn ends (the next node is reached a turn later) and for the
 *  last node. */
export function pathTurnMarks(nodes) {
  const out = [];
  for (let i = 1; i < nodes.length; i++) {
    const last = i === nodes.length - 1;
    if (last || nodes[i + 1].turn !== nodes[i].turn) out.push({ i, turn: nodes[i].turn, last });
  }
  return out;
}
