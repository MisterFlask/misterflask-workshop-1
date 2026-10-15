// node --test: the map maths of app/tiles.js (wrap, camera, encodings, the terrain cache's
// dirty chunks, path markers).
import test from "node:test";
import assert from "node:assert/strict";
import * as T from "../app/tiles.js";

const map = (w, h, wrap_x = true) => {
  const n = w * h;
  const arr = (v) => new Array(n).fill(v);
  return { w, h, wrap_x, wrap_y: false, vis: arr(2), plot_type: arr(2), terrain: arr(0), feature: arr(-1), bonus: arr(-1), improvement: arr(-1), route: arr(-1), owner: arr(-1), river: arr(0), city: arr(-1), yield: arr(0) };
};

test("wrap and plot indices", () => {
  const m = map(10, 6);
  assert.equal(T.wrapCoord(-1, 10, true), 9);
  assert.equal(T.wrapCoord(10, 10, true), 0);
  assert.equal(T.wrapCoord(-21, 10, true), 9);
  assert.equal(T.wrapCoord(-1, 10, false), null);
  assert.equal(T.wrapCoord(10, 10, false), null);
  assert.equal(T.plotIndex(m, 3, 2), 23);
  assert.equal(T.plotIndex(m, -1, 0), 9);
  assert.equal(T.plotIndex(m, 0, 6), -1);
  assert.equal(T.plotIndex(m, 0, -1), -1);
  assert.equal(T.plotIndex(map(10, 6, false), -1, 0), -1);
  assert.deepEqual(T.plotXY(m, 23), { x: 3, y: 2 });
});

test("plot distance is Civ4's, with wrap", () => {
  const m = map(20, 10);
  assert.equal(T.plotDistance(m, 0, 0, 3, 0), 3);
  assert.equal(T.plotDistance(m, 0, 0, 3, 2), 4); // 3 + 2/2
  assert.equal(T.plotDistance(m, 0, 0, 19, 0), 1); // across the seam
  assert.equal(T.plotDistance(map(20, 10, false), 0, 0, 19, 0), 19);
});

test("screen and map coordinates agree, north up, across the seam", () => {
  const m = map(10, 6);
  const cam = { cx: 5, cy: 3, ts: 20 };
  const vw = 200;
  const vh = 120;
  // The centre of the viewport is the camera's centre: plot (5, 3).
  assert.deepEqual(T.screenToMap(cam, m, vw, vh, 100, 61), { x: 5, y: 2 });
  assert.deepEqual(T.screenToMap(cam, m, vw, vh, 101, 59), { x: 5, y: 3 });
  assert.deepEqual(T.screenToMap(cam, m, vw, vh, 99, 59), { x: 4, y: 3 });
  // North is up: a smaller screen y is a larger plot y.
  assert.deepEqual(T.screenToMap(cam, m, vw, vh, 101, 1), { x: 5, y: 5 });
  assert.equal(T.screenToMap(cam, m, vw, vh, 101, -30), null);
  // Every plot's top-left corner maps back to it.
  for (let x = 0; x < 10; x++) {
    for (let y = 0; y < 6; y++) {
      const p = T.mapToScreen(cam, m, vw, vh, x, y);
      assert.deepEqual(T.screenToMap(cam, m, vw, vh, p.sx + 1, p.sy + 1), { x, y }, `${x},${y}`);
    }
  }
  // Near the seam the nearest copy is used: camera at column 0.5, plot 9 is just west.
  const seam = { cx: 0.5, cy: 3, ts: 20 };
  const p = T.mapToScreen(seam, m, vw, vh, 9, 3);
  assert.equal(p.sx, 100 + (-1 - 0.5) * 20);
  assert.deepEqual(T.screenToMap(seam, m, vw, vh, p.sx + 1, p.sy + 1), { x: 9, y: 3 });
});

test("copies of a column across a narrow wrapped map", () => {
  const m = map(4, 4);
  const cam = { cx: 2, cy: 2, ts: 10 };
  // 200 px = 20 columns visible: each column shows about 5 times.
  const c = T.copiesOf(cam, m, 200, 1);
  assert.ok(c.length >= 5, String(c));
  for (const ux of c) assert.equal(((ux % 4) + 4) % 4, 1);
  assert.deepEqual(T.copiesOf(cam, map(4, 4, false), 200, 1), [1]);
});

test("camera clamp, wrap and zoom about a point", () => {
  const m = map(10, 6);
  assert.deepEqual(T.clampCamera({ cx: -1, cy: 9, ts: 8 }, m), { cx: 9, cy: 6, ts: 8 });
  assert.deepEqual(T.clampCamera({ cx: 12, cy: -2, ts: 8 }, map(10, 6, false)), { cx: 10, cy: 0, ts: 8 });
  const cam = { cx: 5, cy: 3, ts: 20 };
  const z = T.zoomAt(cam, 200, 120, 40, 150, 30);
  // The map point under (150, 30) stays there.
  const before = T.screenToMap(cam, m, 200, 120, 150, 30);
  const after = T.screenToMap(z, m, 200, 120, 150, 30);
  assert.deepEqual(after, before);
  assert.equal(z.ts, 40);
});

test("river bits and packed yields", () => {
  assert.deepEqual(T.riverEdges(0), { south: false, east: false, ns: -1, we: -1 });
  assert.deepEqual(T.riverEdges(1 | (3 << 2)), { south: true, east: false, ns: 2, we: -1 });
  assert.deepEqual(T.riverEdges(2 | (1 << 5)), { south: false, east: true, ns: -1, we: 0 });
  assert.deepEqual(T.riverEdges(-1), { south: false, east: false, ns: -1, we: -1 });
  assert.deepEqual(T.unpackYield(2 | (1 << 8) | (3 << 16)), [2, 1, 3]);
  assert.equal(T.unpackYield(-1), null);
});

test("dirty chunks: the changed plot's chunk and its neighbours'", () => {
  const a = map(24, 16);
  assert.equal(T.dirtyChunks(null, a), "all");
  assert.equal(T.dirtyChunks(a, map(16, 16)), "all");
  const b = structuredClone(a);
  assert.equal(T.dirtyChunks(a, b).size, 0);
  // A plot inside chunk (1, 1): that chunk only.
  b.owner[12 * 24 + 12] = 3;
  assert.deepEqual([...T.dirtyChunks(a, b)].sort(), ["1,1"]);
  // A plot on a chunk corner: up to four chunks (its borders and roads reach the neighbours).
  const c = structuredClone(a);
  c.route[8 * 24 + 8] = 0;
  assert.deepEqual([...T.dirtyChunks(a, c)].sort(), ["0,0", "0,1", "1,0", "1,1"]);
  // Across the seam: plot (0, 0) touches the last column's chunk.
  const d = structuredClone(a);
  d.vis[0] = 1;
  assert.ok(T.dirtyChunks(a, d).has("2,0"));
  // The yields are drawn in the cache too (the yield map layer, P6).
  const e = structuredClone(a);
  e.yield[5] = 7;
  assert.ok(T.dirtyChunks(a, e).has("0,0"));
});

test("labels, colours and the top unit", () => {
  assert.equal(T.abbrev("Warrior"), "Wa");
  assert.equal(T.abbrev("Great Commander"), "GC");
  assert.equal(T.abbrev("Adept of the Order"), "AO");
  assert.equal(T.abbrev(""), "?");
  assert.equal(T.humanize("IMPROVEMENT_GOBLIN_FORT"), "Goblin Fort");
  assert.equal(T.terrainColor("TERRAIN_GRASS"), T.TERRAIN_COLORS.TERRAIN_GRASS);
  assert.equal(T.terrainColor("TERRAIN_NEW"), T.terrainColor("TERRAIN_NEW"));
  assert.match(T.terrainColor("TERRAIN_NEW"), /^hsl\(/);
  assert.equal(T.playerColor(7, true), "#5a1010");
  assert.equal(T.playerColor(0), T.PLAYER_COLORS[0]);
  assert.equal(T.playerColor(T.PLAYER_COLORS.length), T.PLAYER_COLORS[0]);
  assert.ok(T.isLair("IMPROVEMENT_BARROW"));
  assert.ok(T.isLair("IMPROVEMENT_X", { unique: true }));
  assert.ok(!T.isLair("IMPROVEMENT_FARM", { unique: false }));
  const units = [
    { id: 1, owner: 2, moves: 60 },
    { id: 2, owner: 0, moves: 0 },
    { id: 3, owner: 0, moves: 60 },
  ];
  assert.equal(T.topUnit(units, 0).id, 3);
  assert.equal(T.topUnit(units, 0, [2]).id, 2);
  assert.equal(T.topUnit(units, 5).id, 1);
  assert.equal(T.topUnit([], 0), null);
});

test("turn markers along a path", () => {
  const nodes = [
    { x: 0, y: 0, turn: 1 },
    { x: 1, y: 0, turn: 1 },
    { x: 2, y: 0, turn: 2 },
    { x: 3, y: 0, turn: 2 },
    { x: 4, y: 0, turn: 3 },
  ];
  assert.deepEqual(T.pathTurnMarks(nodes), [
    { i: 1, turn: 1, last: false },
    { i: 3, turn: 2, last: false },
    { i: 4, turn: 3, last: true },
  ]);
  assert.deepEqual(T.pathTurnMarks([nodes[0]]), []);
});

test("map deltas: the changed tiles on a copy, or the whole map", () => {
  const layers = T.MAP_LAYERS;
  const full = (rev, f) => {
    const m = { w: 4, h: 3, wrap_x: true, wrap_y: false, full: true, epoch: "e1", rev, map_rev: `r${rev}`, reveal: false };
    for (const k of layers) m[k] = Array.from({ length: 12 }, (_, i) => f(k, i));
    return m;
  };
  const prev = full(5, () => 0);
  // A full reply is the map.
  const f = full(9, () => 1);
  assert.deepEqual(T.applyMapDelta(prev, f), { map: f, changed: "all" });
  // A delta: tiles 2 and 11 changed.
  const delta = { w: 4, h: 3, wrap_x: true, wrap_y: false, full: false, since: 5, tiles: [2, 11], epoch: "e1", rev: 7, map_rev: "r7", reveal: false };
  for (const k of layers) delta[k] = k === "owner" ? [3, 4] : [0, 0];
  const { map, changed } = T.applyMapDelta(prev, delta);
  assert.deepEqual(changed, [2, 11]);
  assert.equal(map.full, true);
  assert.equal(map.rev, 7);
  assert.equal(map.since, undefined);
  assert.equal(map.tiles, undefined);
  assert.deepEqual(map.owner, [0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 4]);
  assert.deepEqual(prev.owner, Array(12).fill(0), "the map held is not changed");
  // A delta that does not fit the map held: another epoch, another revision, a short layer.
  assert.throws(() => T.applyMapDelta({ ...prev, epoch: "e0" }, delta));
  assert.throws(() => T.applyMapDelta({ ...prev, rev: 6 }, delta));
  assert.throws(() => T.applyMapDelta(null, delta));
  assert.throws(() => T.applyMapDelta(prev, { ...delta, owner: [3] }));
  // The chunks to redraw: the changed tiles' and their neighbours'.
  const chunks = T.chunksOfTiles(map, [2]);
  assert.ok(chunks.has("0,0"));
});
