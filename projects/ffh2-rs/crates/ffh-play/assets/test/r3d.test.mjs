// node --test: the 3D renderer's pure parts (app/r3d, docs/R3D.md) -- the ground model, the
// recipes, the placement -- on the lab's synthetic scenes and the real type tables (the lab's
// data fixture). No WebGL here: what is checked is that the world is a function of what the player
// knows, deterministic, seamless on a wrapping map, and that every game type gets a model.
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import * as St from "../app/state.js";

const data = JSON.parse(fs.readFileSync(fileURLToPath(new URL("../../r3d-lab/data.json", import.meta.url)), "utf8"));
St.setData(data);
const { buildMap, buildView, showcase, fields, riverBits } = await import("../../r3d-lab/scenes.js");
const { World, KIND } = await import("../app/r3d/world.js");
const R = await import("../app/r3d/assets/registry.js");
await import("../app/r3d/assets/index.js");
const { CATALOG } = await import("../app/r3d/assets/catalog.js");
const { dressStatic, dressUnits } = await import("../app/r3d/dress.js");

const ROWS = ["..hh^^..~~--..", "..h^^h..~~-...", ".hhh.....--...", "..ff..,,..-...", "mm..dd..xxpp.."];

test("the ground meets itself across a wrapping map's seam", () => {
  const map = buildMap(data, ROWS, { wrap: true });
  const w = new World(map);
  for (let y = 0; y <= ROWS.length; y += 0.17) {
    for (const x of [0, 0.31, 2.9]) assert.ok(Math.abs(w.heightAt(x, y) - w.heightAt(x + map.w, y)) < 1e-9, `seam at (${x}, ${y})`);
  }
});

test("the ground is a pure function of the map", () => {
  const a = new World(buildMap(data, ROWS));
  const b = new World(buildMap(data, ROWS));
  for (let k = 0; k < 200; k++) {
    const x = (k * 0.137) % ROWS[0].length, y = (k * 0.071) % ROWS.length;
    assert.equal(a.heightAt(x, y), b.heightAt(x, y));
  }
});

test("unrevealed plots shape nothing: what is hidden does not leak through the ground", () => {
  const hidden = [[6, 2], [7, 2], [6, 3]];
  const base = buildMap(data, ROWS, { hidden });
  const leaked = buildMap(data, ROWS, { hidden });
  // Pretend the server had sent the hidden plots' real contents (it sends -1): a mountain and a river.
  const peak = data.tables.PlotType.findIndex((e) => e.type === "PLOT_PEAK");
  for (const [x, y] of hidden) {
    const i = y * leaked.w + x;
    leaked.plot_type[i] = peak;
    leaked.terrain[i] = 0;
    leaked.river[i] = 3;
  }
  const a = new World(base), b = new World(leaked);
  for (const [x, y] of hidden) assert.equal(a.kind[y * base.w + x], KIND.VOID);
  for (let y = 0; y < ROWS.length; y += 0.25) for (let x = 0; x < ROWS[0].length; x += 0.25) assert.equal(a.heightAt(x, y), b.heightAt(x, y));
  assert.equal(b.rivers.length, 0, "no river is traced through unrevealed plots");
});

test("river edges chain into continuous lines", () => {
  const rows = Array.from({ length: 6 }, () => ".".repeat(8));
  const map = buildMap(data, rows, { rivers: riverBits([[1, 5], [1, 4], [2, 4], [2, 3], [3, 3], [3, 2], [4, 2]]) });
  const w = new World(map);
  assert.equal(w.rivers.length, 1);
  const line = w.rivers[0];
  for (let k = 1; k < line.length; k++) assert.ok(Math.hypot(line[k][0] - line[k - 1][0], line[k][1] - line[k - 1][1]) < 0.6, "no jump along the river");
  // The channel is carved: the river runs lower than the land beside it.
  assert.ok(w.heightAt(2, 3.5) < w.heightAt(2.5, 3.5) - 0.03);
});

test("a small lake is a bowl of water; water touching the unknown is not a lake", () => {
  const rows = ["......", ".h....", "..-n..", "......", "......"];
  const w = new World(buildMap(data, rows));
  const i = (x, y) => y * 6 + x;
  // Rows are listed north first: row 2 is map y = 2.
  const ly = rows.length - 1 - 2;
  assert.equal(w.lake[i(2, ly)], 1);
  assert.ok(w.heightAt(2.5, ly + 0.5) < -0.2, `the lake's middle is deep (${w.heightAt(2.5, ly + 0.5)})`);
  // The same pond beside an unrevealed plot may be the edge of a sea: no bowl.
  const open = new World(buildMap(data, rows, { hidden: [[1, ly]] }));
  assert.equal(open.lake[i(2, ly)], 0);
});

test("recipes are deterministic, finite, and simplify for distance", () => {
  for (const name of R.recipeNames()) {
    const a = R.buildAsset(name, 1, "");
    R.clearBuilt();
    const b = R.buildAsset(name, 1, "");
    assert.equal(a.count, b.count, name);
    assert.ok(a.count > 0 && a.count % 3 === 0, `${name}: whole triangles`);
    for (let i = 0; i < a.data.length; i++) {
      if (!Number.isFinite(a.data[i])) assert.fail(`${name}: non-finite vertex data at ${i}`);
    }
    assert.deepEqual(Array.from(a.data.slice(0, 64)), Array.from(b.data.slice(0, 64)), name);
    const lod = R.buildAssetLOD(name, 1, "");
    if (lod) assert.ok(lod.count < a.count, `${name}: the simplified copy is smaller`);
  }
});

test("every unit type of the game gets a figure that builds", () => {
  const missing = [];
  for (const u of data.tables.UnitType) {
    const domain = St.tname("DomainType", u.domain);
    const combat = St.tname("UnitCombatType", u.unit_combat);
    const look = CATALOG.unit?.(u.type, { info: u, civ: null, domain, combat, family: "human", unit: { id: 1 }, r: () => 0.5, name: u.type });
    if (!look?.asset || !R.hasRecipe(look.asset)) missing.push(u.type);
    else R.buildAsset(look.asset, look.variant ?? 0, look.param ?? "");
  }
  assert.deepEqual(missing, []);
});

test("the dressing of a scene is deterministic and only uses what is known", () => {
  const scene = showcase(data);
  St.setView(scene.view);
  St.setMap(scene.map);
  const w = new World(scene.map);
  const a = dressStatic(w, scene.map);
  const b = dressStatic(new World(scene.map), scene.map);
  assert.equal(a.list.length, b.list.length);
  assert.deepEqual(a.list.slice(0, 50).map((i) => [i.asset, i.variant, i.x, i.z]), b.list.slice(0, 50).map((i) => [i.asset, i.variant, i.x, i.z]));
  assert.ok(a.list.length > 200, "a dressed world");
  for (const it of a.list) {
    assert.ok(R.hasRecipe(it.asset), `placed asset ${it.asset} has a recipe`);
    const px = Math.floor(it.x), py = Math.floor(-it.z);
    const i = w.index(px, py);
    if (i >= 0) assert.notEqual(scene.map.vis[i], 0, `${it.asset} placed on an unrevealed plot`);
  }
  const units = dressUnits(w, scene.map);
  assert.equal(units.stacks.length, St.S.unitsAt.size);
  const view2 = buildView(data, { players: [{ civ: "CIVILIZATION_BANNOR" }] });
  assert.ok(view2.players.length === 1);
});

test("a plot's dressing depends only on its neighbourhood: a moving unit redresses the plots around it", () => {
  const scene = showcase(data);
  St.setView(scene.view);
  St.setMap(scene.map);
  const w = new World(scene.map);
  const full = dressStatic(w, scene.map);
  const key = (d) => JSON.stringify((d?.list ?? []).map((i) => [i.asset, i.variant, i.param, +i.x.toFixed(5), +i.y.toFixed(5), +i.z.toFixed(5), +i.sx.toFixed(4), i.id]));
  // Dressing some plots alone gives what the full dressing gave them.
  const some = [5, 77, 140, 200, 301];
  const part = dressStatic(w, scene.map, some);
  for (const i of some) assert.equal(key(part.byPlot.get(i)), key(full.byPlot.get(i)), `plot ${i}`);
  // The elven archer walks two plots on through the great forest: only the plots within one of
  // where it was and where it is change (the glade follows it).
  const view = structuredClone(scene.view);
  const u = view.units.find((u) => u.owner === 1 && u.type === data.tables.UnitType.findIndex((t) => t.type === "UNIT_ARCHER"));
  const from = [u.x, u.y];
  u.x += 2;
  St.setView(view);
  const after = dressStatic(w, scene.map);
  const near = (i, [x, y]) => Math.abs((i % w.w) - x) <= 1 && Math.abs(Math.floor(i / w.w) - y) <= 1;
  let changed = 0;
  for (let i = 0; i < w.w * w.h; i++) {
    if (key(after.byPlot.get(i)) === key(full.byPlot.get(i))) continue;
    changed++;
    assert.ok(near(i, from) || near(i, [u.x, u.y]), `plot ${i} (${i % w.w}, ${Math.floor(i / w.w)}) changed though the unit was not next to it`);
  }
  assert.ok(changed > 0, "the move changed the dressing around the unit");
  St.setView(scene.view);
});

test("worked land keeps to a river's banks: no field, fence or crop in the water, no rock or reed on the crops", async () => {
  const { fieldLayout, inField, riversAround } = await import("../app/r3d/fields.js");
  const { RIVER_HALF, RIVER_WANDER } = await import("../app/r3d/world.js");
  const scene = fields(data);
  St.setView(scene.view);
  St.setMap(scene.map);
  const map = scene.map;
  const w = new World(map);
  const d = dressStatic(w, map);
  let worked = 0;
  for (let i = 0; i < map.w * map.h; i++) {
    if (map.improvement[i] < 0) continue;
    const x = i % map.w, y = (i - x) / map.w;
    const t = data.tables.ImprovementType[map.improvement[i]].type;
    const b = map.bonus[i] >= 0 ? data.tables.BonusType[map.bonus[i]].type : null;
    const rv = riversAround(w, map, x, y);
    const lay = fieldLayout(i, t, b, rv);
    assert.ok(lay, `${t} at (${x}, ${y}) has a layout`);
    if (/PASTURE|MUSHROOMS/.test(t)) assert.ok(lay.paddock, `${t} at (${x}, ${y}) has its paddock`);
    const dry = (lx, lz, what) => assert.ok(w.riverDist(x + 0.5 + lx, y + 0.5 - lz) > RIVER_HALF + RIVER_WANDER, `${what} of ${t} at (${x}, ${y}) in the river`);
    for (const f of lay.fields) for (let u = 0; u <= 4; u++) for (let v = 0; v <= 4; v++) dry(f.x0 + ((f.x1 - f.x0) * u) / 4, f.z0 + ((f.z1 - f.z0) * v) / 4, "a field");
    for (const [[ax, az], [bx, bz]] of lay.fences) for (let u = 0; u <= 4; u++) dry(ax + ((bx - ax) * u) / 4, az + ((bz - az) * u) / 4, "a fence");
    for (const it of d.byPlot.get(i).list) {
      const lx = it.x - (x + 0.5), lz = y + 0.5 + it.z;
      if (/^(imp|anim)\./.test(it.asset)) dry(lx, lz, it.asset);
      else if (/rock|reeds|plant\.grass/.test(it.asset)) assert.ok(!inField(i, t, b, rv, lx, lz), `${it.asset} on the fields of ${t} at (${x}, ${y})`);
    }
    worked++;
  }
  assert.ok(worked >= 10, "the scene's worked plots");
});

test("every panel switch has a default; plain turns the painting off but keeps the water's realism", async () => {
  const { PRESETS, DEFAULTS, PANEL } = await import("../app/r3d/config.js");
  for (const [key] of PANEL) assert.notEqual(DEFAULTS[key], undefined, `panel switch ${key} has no default`);
  // The brushwork on objects and the painted ink are stylisation: off in the plain comparison.
  for (const key of ["brush", "inkLost", "inkColour"]) {
    assert.ok(PRESETS.painterly[key] > 0, `${key} is on in the painterly look`);
    assert.equal(PRESETS.plain[key], 0, `${key} is off in the plain render`);
  }
  // Seeing into and mirrored in water is ordinary rendering: on in both.
  for (const key of ["reflections", "refraction", "caustics"]) {
    for (const name of ["painterly", "plain"]) assert.ok(PRESETS[name][key] > 0, `${key} is on in ${name}`);
  }
});

test("a look saved before a switch existed takes that switch from its own preset, not the defaults", async () => {
  const { loadConfig, PRESETS } = await import("../app/r3d/config.js");
  const saved = { ...PRESETS.plain, preset: "plain" };
  for (const key of ["brush", "inkLost", "inkColour"]) delete saved[key];
  const prev = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  Object.defineProperty(globalThis, "localStorage", { configurable: true, value: { getItem: () => JSON.stringify(saved), setItem() {} } });
  try {
    const cfg = loadConfig();
    for (const key of ["brush", "inkLost", "inkColour"]) assert.equal(cfg[key], 0, `${key} stays off in a saved plain look`);
  } finally {
    if (prev) Object.defineProperty(globalThis, "localStorage", prev);
    else delete globalThis.localStorage;
  }
});
