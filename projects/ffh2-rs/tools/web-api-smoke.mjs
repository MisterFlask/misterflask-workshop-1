#!/usr/bin/env node
// The node smoke client of the browser client's protocol (docs/PLAY_DESIGN.md §7 P5): starts
// `ffh-play serve` on a free port, checks the page and its modules are served, then plays a game
// for 20 turns through `fetch` with the client's own api.js and state.js, the way the page does:
// long-polls the status, reads the cached view and map, answers the deferred popups (events,
// votes, research, idle cities) and blocking decisions, founds cities, gives orders from the
// `group` query's actions, moves units along `path` previews, ends turns through the gate;
// then saves, lists, loads, and plays a staged capture (`ffh-play stage capture`): the move's
// request waits while the keep/raze decision is answered on another request; and a staged load
// (`ffh-play stage load`): the transport picker answered with the command the page builds;
// then quits. Every refusal of something the server offered is a failure, as are
// inconsistent views (units on unrevealed plots, a stale cached view).
//
//   node tools/web-api-smoke.mjs [--bin PATH] [--url URL] [--turns 20] [--seed 5]
//        [--world WORLDSIZE_TINY] [--opponents 2] [--keep] [--verbose]
//
// --url uses a running server (its saves are left alone; no quit); --keep keeps the saves dir.
// Exit 0 on success, 1 on a failure, 2 on usage errors.

import fs from "node:fs";
import path from "node:path";
import * as api from "../crates/ffh-play/assets/app/api.js";
import { popupQueue, setData, setView, loadUnitCommand } from "../crates/ffh-play/assets/app/state.js";
import { plotIndex, plotDistance } from "../crates/ffh-play/assets/app/tiles.js";
import { parseArgs, findBin, tempDir, startServer, rng, stage, until } from "./web-lib.mjs";

let args;
try {
  args = parseArgs(process.argv.slice(2), { bin: "v", url: "v", turns: "v", seed: "v", world: "v", opponents: "v", keep: "flag", verbose: "flag" });
} catch (e) {
  console.error(`web-api-smoke: ${e.message}`);
  process.exit(2);
}
const TURNS = Number(args.turns ?? 20);
const SEED = Number(args.seed ?? 5);
const verbose = !!args.verbose;
const log = (...a) => verbose && console.log(...a);
const random = rng(SEED * 7919 + 1);

const stats = { turns: 0, commands: 0, refused: 0, confirms: 0, popups: 0, decisions: 0, moves: 0, founded: 0, polls: 0 };
const failures = [];
function fail(msg) {
  failures.push(msg);
  console.error(`FAIL: ${msg}`);
}

let server = null;
let saves = null;
let bin = null;
let data = null;
const T = (kind, i) => data.tables[kind][i]?.type;
const idx = (kind, type) => data.tables[kind].findIndex((e) => e.type === type);

async function http(path, init) {
  const r = await fetch(`${baseUrl}${path}`, init);
  return { status: r.status, type: r.headers.get("content-type") ?? "", etag: r.headers.get("etag"), text: await r.text() };
}
let baseUrl = "";

// ------------------------------------------------------------------------------------------
// The page and its modules.

async function checkAssets() {
  const page = await http("/");
  if (page.status !== 200 || !page.type.startsWith("text/html")) fail(`GET / -> ${page.status} ${page.type}`);
  const refs = [...page.text.matchAll(/(?:src|href)="(\/app\/[^"]+)"/g)].map((m) => m[1]);
  if (!refs.includes("/app/main.js") || !refs.includes("/app/style.css")) fail(`the page does not load /app/main.js and /app/style.css: ${refs}`);
  const seen = new Set();
  const queue = [...refs];
  while (queue.length) {
    const p = queue.shift();
    if (seen.has(p)) continue;
    seen.add(p);
    const r = await http(p);
    const want = p.endsWith(".css") ? "text/css" : "text/javascript";
    if (r.status !== 200 || !r.type.startsWith(want)) {
      fail(`GET ${p} -> ${r.status} ${r.type}`);
      continue;
    }
    for (const m of r.text.matchAll(/^\s*import\s+(?:[^"']*?\s+from\s+)?["'](\.\/[^"']+)["']/gm)) queue.push(`/app/${m[1].slice(2)}`);
  }
  log(`assets: ${[...seen].join(" ")}`);
  if (seen.size < 10) fail(`only ${seen.size} client files reached from the page`);
  const d1 = await http("/api/data");
  const d2 = await http("/api/data", { headers: { "If-None-Match": d1.etag } });
  if (d1.status !== 200 || !d1.etag || d2.status !== 304) fail(`/api/data: ${d1.status} etag ${d1.etag}, then ${d2.status}`);
  data = JSON.parse(d1.text);
  const setup = await api.get("/api/setup");
  if (!setup.worlds?.length || !setup.civilizations?.length) fail("the setup lists are empty");
  return setup;
}

// ------------------------------------------------------------------------------------------
// Playing.

/** The status once the game no longer runs. */
async function settle() {
  let st = await api.status();
  while (st.phase === "running") {
    stats.polls++;
    st = await api.wait(st.version, 20000);
  }
  return st;
}

async function command(c, { offered = true } = {}) {
  stats.commands++;
  const { c: name, ...fields } = c;
  const r = await api.send({ k: "cmd", c: name, ...fields });
  if (!r.ok) {
    stats.refused++;
    if (offered || r.error === "invalid") fail(`${JSON.stringify(c)} refused: ${r.error}: ${r.detail}`);
    return null;
  }
  if (r.r.confirm) stats.confirms++;
  return r;
}

/** Checks the cached view and map against each other and the status. */
async function checkView(st) {
  const view = await api.get("/api/view");
  const map = await api.get("/api/map");
  if (!view || !map) {
    fail(`no cached view or map in phase ${st.phase}`);
    return null;
  }
  if (view.view_version !== st.view_version) fail(`cached view_version ${view.view_version} != status ${st.view_version}`);
  if (map.map_rev !== st.map_rev) fail(`cached map_rev ${map.map_rev} != status ${st.map_rev}`);
  const n = map.w * map.h;
  for (const k of ["vis", "terrain", "owner", "city", "yield"]) if (map[k].length !== n) fail(`map.${k} has ${map[k].length} values, not ${n}`);
  const me = view.me.player;
  for (const u of view.units) {
    const i = plotIndex(map, u.x, u.y);
    if (i < 0) fail(`unit ${u.id} off the map`);
    else if (u.owner === me ? map.vis[i] === 0 : map.vis[i] !== 2) fail(`unit ${u.id} of ${u.owner} on a plot with vis ${map.vis[i]}`);
    if (!data.tables.UnitType[u.type]) fail(`unit ${u.id}: unknown type ${u.type}`);
  }
  for (const c of view.cities) {
    const i = plotIndex(map, c.x, c.y);
    if (i < 0 || map.city[i] !== c.id) fail(`city ${c.id} at (${c.x}, ${c.y}) not in the map's city array`);
  }
  return { view, map };
}

async function answerPopups(view, turn) {
  const dismissed = new Set();
  for (let guard = 0; guard < 30; guard++) {
    const q = popupQueue(view.pending, dismissed, turn);
    if (!q.length) return view;
    const p = q[0];
    dismissed.add(p.key);
    stats.popups++;
    if (p.kind === "event") {
      const o = p.event.options.find((x) => x.enabled);
      if (o) await command({ c: "event", triggered: p.event.triggered, event: o.event });
    } else if (p.kind === "vote") {
      await command({ c: "vote", vote: p.vote.vote, choice: p.vote.choices[Math.floor(random() * p.vote.choices.length)] });
    } else if (p.kind === "research") {
      const t = await api.query("techs");
      const can = t.techs.filter((x) => x.can_research && !x.has && !x.disabled);
      if (!can.length) fail("research needed but no tech can be researched");
      else {
        can.sort((a, b) => a.cost - b.cost);
        await command({ c: "research", tech: can[0].tech, shift: false });
      }
    } else if (p.kind === "city") {
      const c = await api.query("city", { city: p.city });
      const choices = c.buildable.filter((b) => b.enabled);
      const pick = choices.find((b) => T("UnitType", b.type) === "UNIT_WARRIOR" && b.order === 0 && random() < 0.4) ?? choices[Math.floor(random() * choices.length)];
      if (pick) await command({ c: "push_order", city: p.city, order: pick.order, data: pick.type, alt: false, shift: false, ctrl: false });
    }
    const st = await settle();
    if (st.phase !== "human") return null;
    view = await api.get("/api/view");
  }
  fail("the deferred popups did not go away");
  return view;
}

/** Orders for one ready group: found a city, build, or move toward a random reachable plot. */
async function orderGroup(view, map, gid) {
  const g = view.groups.find((x) => x.id === gid);
  if (!g) return;
  const r = await api.query("group", { units: g.units });
  const enabled = r.actions.filter((a) => a.enabled);
  const mission = (name) => enabled.find((a) => a.kind === "mission" && T("MissionType", a.mission) === name);
  const found = mission("MISSION_FOUND");
  const head = view.units.find((u) => u.id === g.head);
  const nearCity = view.cities.some((c) => c.owner === view.me.player && plotDistance(map, c.x, c.y, head.x, head.y) < 4);
  if (found && (!nearCity || view.cities.length === 0)) {
    if (await command(found.cmd)) stats.founded++;
    return;
  }
  const build = enabled.find((a) => a.kind === "mission" && T("MissionType", a.mission) === "MISSION_BUILD");
  if (build && random() < 0.7) {
    await command(build.cmd);
    return;
  }
  // Move: a few random plots within 4, the first with a path that does not start a war.
  for (let k = 0; k < 10; k++) {
    const x = head.x + Math.floor(random() * 9) - 4;
    const y = head.y + Math.floor(random() * 9) - 4;
    const i = plotIndex(map, x, y);
    if (i < 0 || map.vis[i] === 0 || (x === head.x && y === head.y)) continue;
    const wx = ((x % map.w) + map.w) % map.w;
    const p = await api.query("path", { units: g.units, x: wx, y });
    if (!p.ok || p.declare_war_team !== null) continue;
    const reply = await command({ c: "move", units: g.units, x: wx, y, append: false }, { offered: false });
    if (reply?.r?.confirm) continue;
    if (reply) {
      stats.moves++;
      return;
    }
  }
  const rest = mission("MISSION_SKIP") ?? mission("MISSION_FORTIFY") ?? mission("MISSION_SLEEP");
  if (rest) await command(rest.cmd);
}

async function playTurn(st) {
  let checked = await checkView(st);
  if (!checked) return;
  let { view, map } = checked;
  view = await answerPopups(view, st.turn);
  if (!view) return;
  // Orders, at most 25 per turn; the AutoMoves rule may run in between (phase `running`).
  for (let n = 0; n < 25; n++) {
    st = await settle();
    if (st.phase !== "human") return;
    view = await api.get("/api/view");
    map = (await api.get("/api/map")) ?? map;
    const ready = view.pending.ready_groups;
    if (!ready.length) break;
    await orderGroup(view, map, ready[0]);
  }
  // Messages: indices are contiguous.
  const msgs = await api.query("log", { since: 0 });
  msgs.entries.forEach((e, k) => e.i !== msgs.from + k && fail(`log entry ${k} has index ${e.i}`));
  // End the turn: the gate first, then forced.
  st = await settle();
  if (st.phase !== "human") return;
  const r = await command({ c: "end_turn" });
  if (r?.r?.confirm) {
    if (r.r.confirm.kind !== "end_turn") fail(`end_turn confirm ${JSON.stringify(r.r.confirm)}`);
    await command({ c: "end_turn", force: true });
  }
}

/** A capture's keep/raze decision (design §2.4): the move's request waits inside the rules call
 *  while the long-poll shows the decision, queries are answered, and the answer (on another
 *  request) lets the move finish. */
async function stagedCapture() {
  const staged = await stage(bin, "capture", path.join(saves, "staged"));
  await api.session("load", { name: "staged/capture" });
  let st = await settle();
  if (st.phase !== "human" || st.turn !== staged.turn) return fail(`staged capture loaded in phase ${st.phase}, turn ${st.turn}`);
  const move = api.send({ k: "cmd", c: "move", units: [staged.unit], x: staged.x, y: staged.y, append: false });
  let settled = false;
  move.then(() => (settled = true));
  const dec = await until(async () => {
    const s = await api.status();
    return s.phase === "decision" ? s : null;
  }, "the keep/raze decision", 20000);
  stats.decisions++;
  if (settled) fail("the move's reply came before the decision was answered");
  if (dec.decision.kind !== "keep_city") fail(`decision kind ${dec.decision.kind}`);
  const view = await api.get("/api/view");
  if (!view || view.view_version !== dec.view_version) fail("no cached view of the decision");
  const busy = await api.send({ k: "cmd", c: "end_turn" });
  if (busy.error !== "decision_pending") fail(`a command inside the decision: ${busy.error}`);
  const keep = dec.decision.options.findIndex((o) => o.key === "keep");
  await api.answer(dec.decision.id, keep);
  const reply = await move;
  if (!reply.ok || !reply.r.applied) fail(`the move after the decision: ${JSON.stringify(reply)}`);
  st = await settle();
  const after = await api.get("/api/view");
  const city = after.cities.find((c) => c.x === staged.x && c.y === staged.y);
  if (city?.owner !== after.me.player) fail("the captured city is not ours");
  if (!after.pending.idle_cities.includes(city?.id)) fail("the captured city is not idle");
  log(`staged capture: kept ${city?.name}`);
}

/** The transport picker (`ffh-play stage load`): the warrior's `load` action asks which of two
 *  triremes, and the command the page builds from the answer (state.js `loadUnitCommand`)
 *  loads it into the one picked. */
async function stagedLoad() {
  const staged = await stage(bin, "load", path.join(saves, "staged-load"));
  await api.session("load", { name: "staged-load/load" });
  const st = await settle();
  if (st.phase !== "human" || st.turn !== staged.turn) return fail(`staged load loaded in phase ${st.phase}, turn ${st.turn}`);
  const group = await api.query("group", { units: [staged.unit] });
  const load = group.actions.find((a) => a.kind === "load");
  if (!load?.enabled || JSON.stringify(load.transports) !== JSON.stringify(staged.transports)) return fail(`the load action: ${JSON.stringify(load)}`);
  const asked = await command(load.cmd);
  if (asked?.r?.confirm?.kind !== "load_unit") return fail(`load: ${JSON.stringify(asked?.r)}`);
  setData(data);
  setView(await api.get("/api/view"));
  const pick = asked.r.confirm.transports[1];
  if (!(await command(loadUnitCommand([staged.unit], pick)))) return;
  await settle();
  const after = await api.get("/api/view");
  const u = after.units.find((x) => x.id === staged.unit);
  if (u?.transport !== pick) fail(`after the picker the warrior is in ${u?.transport}, not ${pick}`);
  log(`staged load: into ${pick}`);
}

async function main() {
  let setup;
  if (args.url) {
    baseUrl = args.url.replace(/\/$/, "");
  } else {
    bin = findBin(args.bin);
    saves = tempDir("api-smoke");
    server = await startServer({ bin, saves });
    baseUrl = server.url;
  }
  api.configure({ url: baseUrl });
  console.log(`web-api-smoke: ${baseUrl}, ${TURNS} turns, seed ${SEED}`);
  setup = await checkAssets();
  const st0 = await api.status();
  if (!args.url) {
    if (st0.phase !== "no_game") fail(`a fresh server is in phase ${st0.phase}`);
    if ((await api.get("/api/view")) !== null) fail("GET /api/view without a game is not 409");
  }
  const world = args.world ?? "WORLDSIZE_TINY";
  if (!setup.worlds.some((w) => w.type === world)) throw new Error(`unknown world ${world}`);
  await api.session("new", { setup: { seed: SEED, world, opponents: Number(args.opponents ?? 2) } });
  const t0 = Date.now();
  let st = await settle();
  const start = st.turn;
  while (stats.turns < TURNS) {
    if (st.phase === "decision") {
      stats.decisions++;
      await api.answer(st.decision.id, null);
    } else if (st.phase === "human") {
      const before = st.turn;
      await playTurn(st);
      st = await settle();
      if (st.turn > before) stats.turns++;
      log(`turn ${st.turn}: ${JSON.stringify(stats)}`);
      if (failures.length > 10) break;
      continue;
    } else if (st.phase === "over") {
      console.log(`the game ended at turn ${st.turn}`);
      break;
    } else {
      fail(`phase ${st.phase}: ${st.stuck ?? JSON.stringify(st.crash ?? {})}`);
      break;
    }
    st = await settle();
  }
  const ms = Date.now() - t0;
  // Save, list, load.
  if (st.phase === "human") {
    const saved = await api.session("save", { name: "smoke" });
    if (!fs.existsSync(saved.r.save)) fail(`the save ${saved.r.save} is missing`);
    const list = (await api.session("saves")).r;
    const game = list.games.find((g) => g.game === list.current);
    if (!game?.saves.some((s) => s.name === "smoke")) fail("the save is not listed");
    const before = await api.get("/api/view");
    await api.session("load", { name: `${list.current}/smoke` });
    const after = await settle();
    const view = await api.get("/api/view");
    if (after.phase !== "human" || after.turn !== st.turn) fail(`after the load: phase ${after.phase}, turn ${after.turn} (saved at ${st.turn})`);
    if (view.units.length !== before.units.length || view.cities.length !== before.cities.length) fail("the loaded game differs from the saved one");
  }
  const cities = (await api.get("/api/view"))?.cities.filter((c) => c.owner === st.human).length;
  if (server) await stagedCapture();
  if (server) await stagedLoad();
  console.log(`web-api-smoke: turns ${start}-${st.turn} in ${(ms / 1000).toFixed(1)} s; ${cities} cities; ${JSON.stringify(stats)}`);
  if (server) {
    await api.session("quit");
    const code = await server.exited();
    if (code !== 0) fail(`after quit the server ${code === "timeout" ? "is still running" : `exited with ${code}`}`);
    await server.stop();
  }
}

try {
  await main();
} catch (e) {
  fail(`${e.stack ?? e}`);
  if (server) await server.stop();
} finally {
  if (saves && !args.keep) fs.rmSync(saves, { recursive: true, force: true });
  else if (saves) console.log(`saves kept in ${saves}`);
}
if (failures.length) {
  console.error(`web-api-smoke: ${failures.length} failure(s)`);
  process.exit(1);
}
console.log("web-api-smoke: ok");
