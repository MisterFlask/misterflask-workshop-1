// node --test: what the art renderer draws, from app/stacks.js (zoom steps, the terrain cache's
// chunks in the art's row order, stacks and their state, city signatures, facing).
import test from "node:test";
import assert from "node:assert/strict";
import * as St from "../app/stacks.js";

const map = (w, h, wrap_x = true) => ({ w, h, wrap_x, wrap_y: false });

test("zoom steps are whole device pixels per art pixel", () => {
  assert.deepEqual(St.artZooms(1), [8, 16, 32, 64, 96, 128]);
  assert.equal(St.defaultArtZoom(1), 64);
  assert.deepEqual(St.artZooms(2), [8, 16, 32, 64, 96, 128]);
  // dpr 1.5: 1x is 2 device px an art px (round 1.5), so 42.67 CSS px a tile; 2x is 3.
  const z = St.artZooms(1.5);
  assert.equal(z.length, 6);
  assert.ok(Math.abs(z[2] - 64 / 1.5) < 1e-9 && Math.abs(z[3] - 96 / 1.5) < 1e-9, String(z));
  for (const ts of z.slice(2)) assert.equal((St.artScale(ts, 1.5) * 32) % 32, 0);
  assert.equal(St.artScale(16, 1), 0, "a chart step");
  assert.equal(St.artScale(8, 2), 0);
  assert.equal(St.artScale(64, 1), 2);
  assert.equal(St.artScale(64, 2), 4);
  assert.equal(St.artScale(32, 1), 1);
});

test("chunks in row order: a tile touches its own and its neighbours' chunks", () => {
  const m = map(16, 12);
  assert.equal(St.rowOf(m, 0), 11);
  assert.equal(St.rowOf(m, 11), 0);
  const C = St.CHUNK;
  // A tile in the middle of a chunk: only that chunk (row 12 - 1 - 5 = 6 -> chunk row 1 for C = 4).
  const mid = 5 * 16 + 5;
  assert.deepEqual([...St.rowChunksOfTiles(m, [mid])], [`${Math.floor(5 / C)},${Math.floor(6 / C)}`]);
  // The west edge of a wrapping map: the east chunk too.
  const west = 5 * 16 + 0;
  const keys = St.rowChunksOfTiles(m, [west]);
  assert.ok(keys.has(`${Math.floor(15 / C)},${Math.floor(6 / C)}`), [...keys].join(" "));
  // Not without wrap.
  const flat = St.rowChunksOfTiles(map(16, 12, false), [west]);
  assert.ok(![...flat].some((k) => k.startsWith(`${Math.floor(15 / C)},`)), [...flat].join(" "));
  // The north edge (y = h - 1) has no row above.
  assert.ok([...St.rowChunksOfTiles(m, [11 * 16 + 5])].every((k) => Number(k.split(",")[1]) >= 0));
});

test("changed tiles between two maps", () => {
  const a = { w: 2, h: 2, wrap_x: false, vis: [2, 2, 1, 0], owner: [-1, -1, -1, -1] };
  const b = { ...a, vis: [2, 2, 2, 0], owner: [-1, 3, -1, -1] };
  assert.deepEqual(St.changedTiles(a, b, ["vis", "owner"]), [1, 2]);
  assert.equal(St.changedTiles(null, b, ["vis"]), "all");
  assert.equal(St.changedTiles({ ...a, w: 4 }, b, ["vis"]), "all");
});

test("the head of a stack and its state", () => {
  const info = { 1: { hero: false, combat: "MELEE", strength: 3 }, 2: { hero: true, combat: "MELEE", strength: 6 }, 3: { hero: false, combat: "NONE", strength: 0 } };
  const acts = ["ACTIVITY_AWAKE", "ACTIVITY_HOLD", "ACTIVITY_SLEEP", "ACTIVITY_HEAL", "ACTIVITY_SENTRY"];
  const ctx = (extra = {}) => ({ me: 0, selected: [], activity: (i) => acts[i], unitInfo: (t) => info[t], automated: () => false, facing: (id) => (id === 11 ? "left" : "right"), ...extra });
  // Own units: the first that can move; moves, mark, hero, count.
  const own = [
    { id: 10, owner: 0, type: 1, hp: 100, max_hp: 100, moves: 0, activity: 2, group: 1 },
    { id: 11, owner: 0, type: 2, hp: 50, max_hp: 100, moves: 60, activity: 0, group: 2 },
  ];
  let st = St.stackState(own, ctx());
  assert.equal(st.head.id, 11);
  assert.deepEqual(st.state, { own: true, selected: false, hp: 50, count: 2, hero: true, facing: "left", moves: true, mark: null });
  // The selected unit leads; a fortified fighter shows the shield, a sleeping worker the crescent.
  st = St.stackState(own, ctx({ selected: [10] }));
  assert.equal(st.head.id, 10);
  assert.equal(st.state.selected, true);
  assert.equal(st.state.moves, false);
  assert.equal(st.state.mark, "fortify");
  st = St.stackState([{ id: 12, owner: 0, type: 3, hp: 100, max_hp: 100, moves: 60, activity: 2, group: 3 }], ctx());
  assert.equal(st.state.mark, "sleep");
  // A promotion waiting takes precedence; automated groups show the gear.
  st = St.stackState([{ ...own[0], promo_ready: true }], ctx());
  assert.equal(st.state.mark, "promote");
  st = St.stackState([own[1]], ctx({ automated: () => true }));
  assert.equal(st.state.mark, "auto");
  // Foreign stacks: the strongest stands in front; no moves, no mark.
  const foreign = [
    { id: 20, owner: 1, type: 1, hp: 100, max_hp: 100 },
    { id: 21, owner: 1, type: 2, hp: 80, max_hp: 100 },
  ];
  st = St.stackState(foreign, ctx());
  assert.equal(st.head.id, 21);
  assert.equal(st.state.own, false);
  assert.equal(st.state.moves, undefined);
  assert.equal(st.state.mark, undefined);
  assert.equal(St.stackState([], ctx()), null);
});

test("marks from Civ4's activities", () => {
  assert.equal(St.markOf("ACTIVITY_SLEEP", { canFight: true }), "fortify");
  assert.equal(St.markOf("ACTIVITY_SLEEP"), "sleep");
  assert.equal(St.markOf("ACTIVITY_SENTRY"), "sentry");
  assert.equal(St.markOf("ACTIVITY_HEAL"), "heal");
  assert.equal(St.markOf("ACTIVITY_AWAKE"), null);
  assert.equal(St.markOf("ACTIVITY_HEAL", { promote: true }), "promote");
});

test("city tiers and signatures", () => {
  assert.deepEqual([1, 3, 4, 7, 8, 12, 13, 19, 20, 40].map(St.tierOf), [1, 1, 2, 2, 3, 3, 4, 4, 5, 5]);
  const c = { owner: 2, size: 5, capital: true, visible: true, disorder: false, walls: false };
  const sig = St.citySignature(c);
  assert.equal(St.citySignature({ ...c, size: 7 }), sig, "the same tier");
  for (const change of [{ size: 8 }, { owner: 3 }, { capital: false }, { visible: false }, { disorder: true }, { walls: true }]) {
    assert.notEqual(St.citySignature({ ...c, ...change }), sig, JSON.stringify(change));
  }
});

test("facing follows the last move, across the seam", () => {
  let f = St.updateFacing(new Map(), [{ id: 1, x: 5 }, { id: 2, x: 0 }], 20, true);
  assert.equal(f.get(1).facing, "right");
  f = St.updateFacing(f, [{ id: 1, x: 4 }, { id: 2, x: 19 }], 20, true);
  assert.equal(f.get(1).facing, "left");
  assert.equal(f.get(2).facing, "left", "0 -> 19 on a wrapping map is a step west");
  f = St.updateFacing(f, [{ id: 1, x: 4 }, { id: 2, x: 0 }], 20, true);
  assert.equal(f.get(1).facing, "left", "standing still keeps the facing");
  assert.equal(f.get(2).facing, "right");
  assert.equal(f.has(3), false);
});

test("unitMoves: net displacement of the units that changed tile, wrap-aware", () => {
  let { positions, moved } = St.unitMoves(new Map(), [{ id: 1, x: 5, y: 7 }, { id: 2, x: 0, y: 0 }], 20, true);
  assert.equal(moved.length, 0, "no previous positions: nothing moved");
  assert.deepEqual(positions.get(1), { x: 5, y: 7 });
  // A straight step east and a step north; a unit standing still is not moved.
  ({ positions, moved } = St.unitMoves(positions, [{ id: 1, x: 6, y: 8 }, { id: 2, x: 0, y: 0 }], 20, true));
  assert.deepEqual(moved, [{ id: 1, dx: 1, dy: 1 }]);
  // A step west across the seam (19 -> 0) is +1, not +19 - 0.
  ({ positions, moved } = St.unitMoves(positions, [{ id: 1, x: 6, y: 8 }, { id: 2, x: 19, y: 0 }], 20, true));
  assert.deepEqual(moved, [{ id: 2, dx: -1, dy: 0 }]);
  // A brand-new unit (id 3) is never "moved"; a vanished unit drops out of positions.
  ({ positions, moved } = St.unitMoves(positions, [{ id: 3, x: 1, y: 1 }], 20, true));
  assert.equal(moved.length, 0);
  assert.equal(positions.has(1), false);
  assert.deepEqual(positions.get(3), { x: 1, y: 1 });
});

test("heraldry: a tincture plain, then its divisions, kept for the game", () => {
  const T = { CIVILIZATION_BANNOR: "Argent", CIVILIZATION_ELOHIM: "Argent", CIVILIZATION_LANUN: "Azure", CIVILIZATION_MERCURIANS: "Argent" };
  const tinctureOf = (civ) => T[civ] ?? "Argent";
  const memo = new Map();
  let specs = St.assignHeraldry([{ id: 0, civ: "CIVILIZATION_ELOHIM" }, { id: 3, civ: "CIVILIZATION_LANUN" }, { id: 9, civ: "CIVILIZATION_BARBARIAN", barbarian: true }], memo, tinctureOf);
  assert.deepEqual(specs.map((s) => [s.id, s.division, s.barbarian]), [[0, "plain", false], [3, "plain", false], [9, "plain", true]]);
  // Meeting slot 1 (Argent too) later: it takes the next division; the Elohim keep theirs.
  specs = St.assignHeraldry([{ id: 0, civ: "CIVILIZATION_ELOHIM" }, { id: 1, civ: "CIVILIZATION_BANNOR" }, { id: 3, civ: "CIVILIZATION_LANUN" }, { id: 5, civ: "CIVILIZATION_MERCURIANS" }], memo, tinctureOf);
  assert.deepEqual(specs.map((s) => [s.id, s.division]), [[0, "plain"], [1, "pale"], [3, "plain"], [5, "fess"]]);
  // A new game: a new memo.
  assert.equal(St.assignHeraldry([{ id: 1, civ: "CIVILIZATION_BANNOR" }], new Map(), tinctureOf)[0].division, "plain");
});

test("owner styles from heraldry specs, the lozenge for unknown players", () => {
  const calls = [];
  const art = { ownerStyle: (civ, opts) => (calls.push([civ, opts.division ?? null, !!opts.barbarian]), { civ, ...opts }) };
  const o = new St.OwnerStyles(art, [{ id: 0, civ: "CIVILIZATION_BANNOR", division: "pale", barbarian: false }, { id: 7, civ: "CIVILIZATION_BARBARIAN", division: "plain", barbarian: true }]);
  assert.equal(o.get(0).division, "pale");
  assert.equal(o.get(0), o.get(0), "made once");
  assert.equal(o.get(7).barbarian, true);
  assert.equal(o.get(4).civ, "CIVILIZATION_UNKNOWN");
  assert.equal(o.get(-1), undefined);
  assert.equal(o.get(null), undefined);
  assert.deepEqual(calls, [["CIVILIZATION_BANNOR", "pale", false], ["CIVILIZATION_BARBARIAN", null, true], ["CIVILIZATION_UNKNOWN", null, false]]);
});
