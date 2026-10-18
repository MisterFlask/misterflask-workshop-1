// Synthetic scenes for the 3D lab (docs/R3D.md "Lab"): small hand-written maps in the shape of the
// server's `map` and `view` replies, so recipes and the pipeline can be looked at without a game.
// Real-game screenshots use the real client (`?renderer=3d`); these are for focused iteration.
//
// Map legend (one char per plot, the first row is the NORTH edge):
//   ~ ocean   - coast   . grassland   , plains   d desert   t tundra   s snow   m marsh
//   h grass hills   n plains hills   ^ peak (on tundra)
//   f forest (grass)   F forest on hills   y young forest   a ancient forest   j jungle
//   b burnt forest   x broken lands   X burning sands   p fields of perdition   P perdition hills
//   o oasis (desert)   l flood plains (desert)   i ice (on ocean)   v volcano (peak)

const LEGEND = {
  "~": ["TERRAIN_OCEAN", "PLOT_OCEAN"],
  "-": ["TERRAIN_COAST", "PLOT_OCEAN"],
  ".": ["TERRAIN_GRASS", "PLOT_LAND"],
  ",": ["TERRAIN_PLAINS", "PLOT_LAND"],
  d: ["TERRAIN_DESERT", "PLOT_LAND"],
  t: ["TERRAIN_TUNDRA", "PLOT_LAND"],
  s: ["TERRAIN_SNOW", "PLOT_LAND"],
  m: ["TERRAIN_MARSH", "PLOT_LAND"],
  h: ["TERRAIN_GRASS", "PLOT_HILLS"],
  n: ["TERRAIN_PLAINS", "PLOT_HILLS"],
  "^": ["TERRAIN_TUNDRA", "PLOT_PEAK"],
  f: ["TERRAIN_GRASS", "PLOT_LAND", "FEATURE_FOREST"],
  F: ["TERRAIN_GRASS", "PLOT_HILLS", "FEATURE_FOREST"],
  y: ["TERRAIN_GRASS", "PLOT_LAND", "FEATURE_FOREST_NEW"],
  a: ["TERRAIN_GRASS", "PLOT_LAND", "FEATURE_FOREST_ANCIENT"],
  j: ["TERRAIN_GRASS", "PLOT_LAND", "FEATURE_JUNGLE"],
  b: ["TERRAIN_PLAINS", "PLOT_LAND", "FEATURE_FOREST_BURNT"],
  x: ["TERRAIN_BROKEN_LANDS", "PLOT_LAND"],
  X: ["TERRAIN_BURNING_SANDS", "PLOT_LAND"],
  p: ["TERRAIN_FIELDS_OF_PERDITION", "PLOT_LAND"],
  P: ["TERRAIN_FIELDS_OF_PERDITION", "PLOT_HILLS"],
  o: ["TERRAIN_DESERT", "PLOT_LAND", "FEATURE_OASIS"],
  l: ["TERRAIN_DESERT", "PLOT_LAND", "FEATURE_FLOOD_PLAINS"],
  i: ["TERRAIN_OCEAN", "PLOT_OCEAN", "FEATURE_ICE"],
  v: ["TERRAIN_BROKEN_LANDS", "PLOT_PEAK", "FEATURE_VOLCANO"],
  // Mixed grounds and features (for judging the catalog's mixes in their real placement).
  w: ["TERRAIN_MARSH", "PLOT_LAND", "FEATURE_FOREST"],
  k: ["TERRAIN_TUNDRA", "PLOT_LAND", "FEATURE_FOREST"],
  K: ["TERRAIN_SNOW", "PLOT_HILLS", "FEATURE_FOREST"],
  z: ["TERRAIN_DESERT", "PLOT_LAND", "FEATURE_SCRUB"],
  q: ["TERRAIN_BROKEN_LANDS", "PLOT_LAND", "FEATURE_FOREST"],
  r: ["TERRAIN_BURNING_SANDS", "PLOT_LAND", "FEATURE_FLAMES"],
  O: ["TERRAIN_BROKEN_LANDS", "PLOT_LAND", "FEATURE_OBSIDIAN_PLAINS"],
  g: ["TERRAIN_FIELDS_OF_PERDITION", "PLOT_LAND", "FEATURE_TORMENTED_SOULS"],
  J: ["TERRAIN_GRASS", "PLOT_HILLS", "FEATURE_JUNGLE"],
};

/** A `map` reply from rows of legend chars (north first) and overlays:
 *  rivers [[x, y, bits]], routes [[x, y]], improvements [[x, y, "IMPROVEMENT_X"]], bonuses
 *  [[x, y, "BONUS_X"]], owners [[x0, y0, x1, y1, player]], fog [[x, y]] (fogged), hidden
 *  [[x, y]] (unrevealed). */
export function buildMap(data, rows, o = {}) {
  const idx = (kind, type) => (type ? data.tables[kind].findIndex((e) => e.type === type) : -1);
  const h = rows.length;
  const w = rows[0].length;
  const n = w * h;
  const map = { w, h, wrap_x: !!o.wrap, wrap_y: false, map_rev: "lab", full: true, reveal: true };
  for (const k of ["vis", "plot_type", "terrain", "feature", "bonus", "improvement", "route", "owner", "river", "city", "yield"]) map[k] = new Array(n).fill(-1);
  for (let row = 0; row < h; row++) {
    const y = h - 1 - row;
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      const c = rows[row][x];
      const L = LEGEND[c] ?? LEGEND["."];
      map.vis[i] = 2;
      map.terrain[i] = idx("TerrainType", L[0]);
      map.plot_type[i] = idx("PlotType", L[1]);
      map.feature[i] = idx("FeatureType", L[2]);
      map.river[i] = 0;
      map.yield[i] = 2 | (1 << 8) | (1 << 16);
    }
  }
  for (const [x, y, bits] of o.rivers ?? []) map.river[y * w + x] |= bits;
  for (const [x, y] of o.routes ?? []) map.route[y * w + x] = 0;
  for (const [x, y, t] of o.improvements ?? []) map.improvement[y * w + x] = idx("ImprovementType", t);
  for (const [x, y, t] of o.bonuses ?? []) map.bonus[y * w + x] = idx("BonusType", t);
  for (const [x0, y0, x1, y1, p] of o.owners ?? []) for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) map.owner[y * w + x] = p;
  for (const [x, y] of o.fog ?? []) map.vis[y * w + x] = 1;
  for (const [x, y] of o.hidden ?? []) {
    const i = y * w + x;
    for (const k of ["plot_type", "terrain", "feature", "bonus", "improvement", "route", "owner", "river", "city", "yield"]) map[k][i] = -1;
    map.vis[i] = 0;
  }
  for (const c of o.cities ?? []) map.city[c.y * w + c.x] = c.id;
  return map;
}

/** River bits from a path of plot corners (map coordinates, y from the south): a step along a row
 *  of corners is the south edge of the plot above it (bit 1), a step along a column the east edge
 *  of the plot to its left (bit 2). */
export function riverBits(corners) {
  const out = [];
  for (let k = 0; k + 1 < corners.length; k++) {
    const [ax, ay] = corners[k], [bx, by] = corners[k + 1];
    if (ay === by) out.push([Math.min(ax, bx), ay, 1]);
    else out.push([ax - 1, Math.min(ay, by), 2]);
  }
  return out;
}

/** A `view` reply: players, cities, units (the fields the renderers read). */
export function buildView(data, { players = [], cities = [], units = [], me = 0 } = {}) {
  const idx = (kind, type) => data.tables[kind].findIndex((e) => e.type === type);
  return {
    view_version: 1,
    turn: 1,
    reveal: true,
    me: { player: me, team: me },
    players: players.map((p, k) => ({ id: p.id ?? k, name: p.civ, civ: idx("CivilizationType", p.civ), team: p.id ?? k, barbarian: !!p.barbarian, alive: true })),
    cities: cities.map((c) => ({ visible: true, walls: false, ...c })),
    units: units.map((u, k) => ({ id: u.id ?? 1000 + k, owner: u.owner ?? 0, type: idx("UnitType", u.type), x: u.x, y: u.y, hp: u.hp ?? 100, max_hp: 100, moves: 1, max_moves: 1, group: k })),
    groups: [],
    group_cycle: [],
    pending: { ready_groups: [] },
    log_next: 0,
  };
}

/** The vertical slice: a coast, a river, forests of three kinds, hills and a mountain range, a
 *  human town, an elven city in its ancient wood, a corrupted infernal march, farms, mines,
 *  a lair, a mana node, armies and monsters. */
export function showcase(data) {
  const rows = [
    "~~~~~~----......hhh^^^tt",
    "~~~~---.....ffhh^^^^ht..",
    "~~~---..,,..ffFFh^^Fh..y",
    "~~---.,,,,...ffFhhFFf.yy",
    "~~--..,,,,....fffffaaa.y",
    "~~--..,.,,.....ffaaaaaa.",
    "~---..,,,......faaaaaaa.",
    "~--...,,.mm.....faaaa...",
    "~--..,,.mmm.....,.....xx",
    "~--..,..,.m...,,,...xxpX",
    "~~--..,,,,....,,,..xxpPX",
    "~~~--......,,,,...xxXppv",
    "~~~~---.......,..xxXXpPv",
    "~~~~~~---......xxXXXppvv",
  ];
  const h = rows.length;
  const Y = (row) => h - 1 - row;
  // River: from the hills down to the coast, along plot edges (see riverBits), skirting the town.
  const rivers = riverBits([[13, 11], [13, 10], [12, 10], [12, 9], [11, 9], [11, 8], [10, 8], [9, 8], [9, 7], [8, 7], [8, 6], [7, 6], [7, 5], [6, 5], [6, 4], [5, 4], [5, 3], [4, 3]]);
  const town = { id: 1, owner: 0, x: 7, y: Y(6), name: "Aldenhal", capital: true, size: 9, walls: true };
  const elves = { id: 2, owner: 1, x: 19, y: Y(5), name: "Evermore", capital: true, size: 7 };
  const hell = { id: 3, owner: 2, x: 21, y: Y(10), name: "Thrall's Gate", capital: false, size: 5 };
  const village = { id: 4, owner: 0, x: 4, y: Y(10), name: "Saltmere", capital: false, size: 3 };
  const routes = [];
  for (let x = 4; x <= 12; x++) routes.push([x, Y(8)]);
  for (const p of [[7, Y(7)], [7, Y(6)], [4, Y(9)], [4, Y(10)], [13, Y(7)], [14, Y(7)], [15, Y(6)], [16, Y(6)], [17, Y(5)], [18, Y(5)], [19, Y(5)]]) routes.push(p);
  const map = buildMap(data, rows, {
    rivers,
    routes,
    improvements: [
      [8, Y(8), "IMPROVEMENT_FARM"], [6, Y(8), "IMPROVEMENT_FARM"], [9, Y(7), "IMPROVEMENT_FARM"], [6, Y(5), "IMPROVEMENT_COTTAGE"],
      [8, Y(5), "IMPROVEMENT_HAMLET"], [5, Y(7), "IMPROVEMENT_VILLAGE"], [14, Y(2), "IMPROVEMENT_MINE"], [16, Y(3), "IMPROVEMENT_MINE"],
      [10, Y(4), "IMPROVEMENT_PASTURE"], [20, Y(1), "IMPROVEMENT_DUNGEON"], [14, Y(9), "IMPROVEMENT_MANA_FIRE"], [22, Y(4), "IMPROVEMENT_FORT"],
      [19, Y(12), "IMPROVEMENT_RUINS"], [2, Y(8), "IMPROVEMENT_FISHING_BOATS"], [12, Y(11), "IMPROVEMENT_TOWER"],
    ],
    bonuses: [
      [8, Y(8), "BONUS_WHEAT"], [10, Y(4), "BONUS_HORSE"], [14, Y(2), "BONUS_GOLD"], [16, Y(3), "BONUS_IRON"], [2, Y(8), "BONUS_FISH"],
      [14, Y(9), "BONUS_MANA"], [21, Y(6), "BONUS_MANA"], [11, Y(10), "BONUS_COW"], [2, Y(4), "BONUS_WHALE"], [20, Y(8), "BONUS_GEMS"],
    ],
    owners: [[2, Y(11), 11, Y(3), 0], [15, Y(7), 23, Y(2), 1], [18, Y(13), 23, Y(8), 2]],
    cities: [town, elves, hell, village],
  });
  const view = buildView(data, {
    players: [{ civ: "CIVILIZATION_BANNOR" }, { civ: "CIVILIZATION_LJOSALFAR" }, { civ: "CIVILIZATION_SHEAIM" }, { civ: "CIVILIZATION_BARBARIAN", barbarian: true }],
    cities: [town, elves, hell, village],
    units: [
      { owner: 0, type: "UNIT_SWORDSMAN", x: 9, y: Y(6) },
      { owner: 0, type: "UNIT_ARCHER", x: 9, y: Y(6) },
      { owner: 0, type: "UNIT_HORSEMAN", x: 11, y: Y(8) },
      { owner: 0, type: "UNIT_WORKER", x: 6, y: Y(8) },
      { owner: 0, type: "UNIT_ADEPT", x: 13, y: Y(8) },
      { owner: 0, type: "UNIT_TRIREME", x: 2, y: Y(7) },
      { owner: 1, type: "UNIT_ARCHER", x: 17, y: Y(6) },
      { owner: 1, type: "UNIT_HUNTER", x: 21, y: Y(3) },
      { owner: 2, type: "UNIT_IMP", x: 19, y: Y(9) },
      { owner: 2, type: "UNIT_WARRIOR", x: 20, y: Y(11) },
      { owner: 3, type: "UNIT_WOLF", x: 12, y: Y(1) },
      { owner: 3, type: "UNIT_GIANT_SPIDER", x: 15, y: Y(0) },
      { owner: 3, type: "UNIT_ACHERON", x: 22, y: Y(12) },
      { owner: 0, type: "UNIT_SETTLER", x: 10, y: Y(10) },
    ],
  });
  return { map, view, focus: [12, Y(7)] };
}

/** A great walled town by a river (down its west side, then along its front), a farm beside it, a
 *  cottage and farms round it, a unit on the plot behind it and its garrison, and a small walled
 *  town with its garrison: the towns keep out of the water and off their neighbours' ground, their
 *  garrisons stand on dry ground inside their walls (docs/R3D.md §6). Best framed at ts=150. */
export function riverTown(data) {
  const rows = ["..........", "....,,....", "..f.......", "..........", "..........", "...,,....."];
  const rivers = riverBits([[3, 6], [3, 5], [3, 4], [3, 3], [4, 3], [5, 3], [6, 3], [7, 3], [8, 3], [9, 3], [10, 3]]);
  const town = { id: 1, owner: 0, x: 3, y: 3, name: "Riverford", capital: true, size: 10, walls: true };
  const fort = { id: 2, owner: 0, x: 6, y: 4, name: "Stoneholt", capital: false, size: 3, walls: true };
  const routes = [[3, 3], [4, 3], [5, 3], [6, 4], [3, 2], [3, 1]];
  const map = buildMap(data, rows, {
    rivers,
    routes,
    improvements: [[4, 3, "IMPROVEMENT_FARM"], [4, 4, "IMPROVEMENT_COTTAGE"], [2, 4, "IMPROVEMENT_FARM"], [4, 2, "IMPROVEMENT_FARM"]],
    bonuses: [[4, 3, "BONUS_WHEAT"]],
    owners: [[0, 0, 9, 5, 0]],
    cities: [town, fort],
  });
  const view = buildView(data, {
    players: [{ civ: "CIVILIZATION_BANNOR" }],
    cities: [town, fort],
    units: [
      { type: "UNIT_SWORDSMAN", x: 3, y: 3 },
      { type: "UNIT_ARCHER", x: 3, y: 3 },
      { type: "UNIT_WARRIOR", x: 3, y: 3 },
      { type: "UNIT_HORSEMAN", x: 3, y: 4 },
      { type: "UNIT_ARCHER", x: 6, y: 4 },
      { type: "UNIT_WARRIOR", x: 6, y: 4 },
    ],
  });
  return { map, view, focus: [4.7, 3.5] };
}

/** The look target (docs/R3D.md §16): the composition of the owner's reference image -- a walled
 *  town on golden grass at the top, a river winding from the north-east to the south-west under two
 *  stone bridges, a crossroads of dirt roads, farms with a cottage and haystacks on the right, a
 *  wheat field, scattered trees thickening into woods at the right and a rocky rise at the bottom
 *  right. No units: the reference shows the land itself. Best framed at ts=150-170. */
export function reference(data) {
  const rows = [
    ",,,,..,,,,....ff",
    ",,,,..,,,,...f.f",
    ",,,,.,,,,,....ff",
    ",,,,,..,,.......",
    ",,.,,,.,,.....f.",
    "..,,,,.,......f.",
    "................",
    "......,,,....ff.",
    "......,,,,...fff",
    "....,,,,,,..fffh",
    "...,,,,,,,..ffhh",
  ];
  const h = rows.length;
  const w = rows[0].length;
  const rivers = riverBits([[16, 9], [15, 9], [14, 9], [14, 8], [13, 8], [12, 8], [12, 7], [11, 7], [10, 7], [10, 6], [9, 6], [8, 6], [7, 6], [7, 5], [6, 5], [5, 5], [4, 5], [4, 4], [3, 4], [2, 4], [2, 3], [1, 3], [0, 3]]);
  const town = { id: 1, owner: 0, x: 7, y: 8, name: "Aldenhal", capital: true, size: 9, walls: true };
  const routes = [[7, 8], [7, 7], [7, 6], [7, 5]];
  for (let x = 0; x < w; x++) routes.push([x, 4]);
  const map = buildMap(data, rows, {
    rivers,
    routes,
    improvements: [
      [14, 7, "IMPROVEMENT_FARM"], [15, 7, "IMPROVEMENT_FARM"], [13, 6, "IMPROVEMENT_COTTAGE"],
      [10, 5, "IMPROVEMENT_COTTAGE"], [11, 5, "IMPROVEMENT_FARM"], [10, 3, "IMPROVEMENT_FARM"], [11, 3, "IMPROVEMENT_FARM"],
    ],
    bonuses: [[10, 3, "BONUS_WHEAT"]],
    owners: [[0, 0, w - 1, h - 1, 0]],
    cities: [town],
  });
  const view = buildView(data, { players: [{ civ: "CIVILIZATION_BANNOR" }], cities: [town] });
  return { map, view, focus: [8, 5.6] };
}

/** A grid for inspecting recipes: `names` (asset names), each in a row of `variants`, on flat
 *  grass. Returns the scene and the instance list to show. */
export function grid(data, names, variants = 6, { ground = ".", spacing = 1 } = {}) {
  const w = Math.max(6, Math.ceil(variants * spacing) + 2);
  const h = Math.max(4, Math.ceil(names.length * spacing) + 2);
  const rows = Array.from({ length: h }, () => ground.repeat(w));
  const map = buildMap(data, rows);
  const view = buildView(data, { players: [{ civ: "CIVILIZATION_BANNOR" }] });
  const items = [];
  names.forEach((name, row) => {
    for (let v = 0; v < variants; v++) items.push({ asset: name, variant: v, row, mx: 1.5 + v * spacing, my: h - 1.5 - row * spacing });
  });
  return { map, view, items, focus: [w / 2, h / 2] };
}

/** Settlements side by side, for the architecture: a row per civilization (its cities at `sizes`,
 *  then its improvements `imps`, all on its own land so they take its family), two plots apart on
 *  flat grass. Capital on the first city; walls from size `walls` up. */
export function settlements(data, civs, sizes = [2, 6, 12], imps = [], { walls = 8, ground = ".", coast = false } = {}) {
  const cols = sizes.length + imps.length;
  const w = Math.max(6, cols * 2 + 1);
  const h = Math.max(4, civs.length * 2 + 1);
  // coast: a strip of coastal water just south of each civilization's row (harbours face it).
  const wet = new Set(coast ? civs.map((_, k) => h - 3 - k * 2) : []);
  const rows = Array.from({ length: h }, (_, row) => (wet.has(h - 1 - row) ? "-" : ground).repeat(w));
  const cities = [], improvements = [], owners = [];
  let id = 1;
  civs.forEach((civ, k) => {
    const y = h - 2 - k * 2;
    owners.push([0, y - 1 < 0 ? 0 : y - 1, w - 1, Math.min(h - 1, y + 1), k]);
    sizes.forEach((size, c) => cities.push({ id: id++, owner: k, x: 1 + c * 2, y, name: `${civ.replace("CIVILIZATION_", "")} ${size}`, capital: c === 0, size, walls: size >= walls }));
    imps.forEach((t, c) => improvements.push([1 + (sizes.length + c) * 2, y, t]));
  });
  const map = buildMap(data, rows, { improvements, owners, cities });
  const view = buildView(data, { players: civs.map((civ) => ({ civ })), cities });
  return { map, view, focus: [w / 2, h / 2] };
}

/** Any map from legend rows (north first, equal lengths), dressed for real: forests, flora and
 *  rocks as the game would place them. */
export function fromRows(data, rows) {
  const map = buildMap(data, rows);
  const view = buildView(data, { players: [{ civ: "CIVILIZATION_BANNOR" }] });
  return { map, view, focus: [rows[0].length / 2, rows.length / 2] };
}
