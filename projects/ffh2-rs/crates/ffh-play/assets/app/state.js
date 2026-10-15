// The client's state and the pure helpers over it (tested by assets/test/state.test.mjs; no
// DOM). The server computes everything: this holds what it sent (status, view, map, static
// data) with lookups, the selection, and what the player has set aside this turn.

import { humanize, plotIndex } from "./tiles.js";

export const S = {
  /** `/api/data`: `{kinds, tables: {Kind: [{type, name, ...}]}}`. */
  data: null,
  /** `/api/setup`: the new-game lists. */
  setup: null,
  /** The last status (design §4.3). */
  status: null,
  /** The last view and map (design §4.4) and lookups built from them. */
  view: null,
  map: null,
  units: new Map(),
  groups: new Map(),
  cities: new Map(),
  players: new Map(),
  unitsAt: new Map(),
  /** The selection: own units on one plot, `{units: [ids], x, y}` or null. */
  sel: null,
  /** Messages `{i, t, text}` read with the `log` query, and the next index. */
  messages: [],
  logNext: 0,
  /** The turn report of the current turn (`report` query). */
  report: null,
  /** Deferred popups set aside this turn ("Later"), by key. */
  dismissed: new Set(),
  /** The turn the dismissals belong to. */
  dismissedTurn: null,
};

// ---------------------------------------------------------------------------------------------
// Events: a tiny bus between the modules.

const listeners = new Map();

export function on(ev, fn) {
  if (!listeners.has(ev)) listeners.set(ev, []);
  listeners.get(ev).push(fn);
}

export function emit(ev, ...args) {
  for (const fn of listeners.get(ev) ?? []) {
    try {
      fn(...args);
    } catch (e) {
      console.error(`listener of ${ev}:`, e);
    }
  }
}

// ---------------------------------------------------------------------------------------------
// Static data: type names and display names by index.

const typeIndex = new Map();

export function setData(data) {
  S.data = data;
  typeIndex.clear();
  for (const [kind, list] of Object.entries(data?.tables ?? {})) {
    list.forEach((e, i) => typeIndex.set(`${kind}:${e.type}`, i));
  }
}

/** Entry `i` of table `kind` (`UnitType`, ...), or null. */
export function info(kind, i) {
  if (i === null || i === undefined || i < 0) return null;
  return S.data?.tables?.[kind]?.[i] ?? null;
}

/** The display name of entry `i` of `kind` ("Warrior"), or "-" for none. */
export function dname(kind, i) {
  if (i === null || i === undefined || i < 0) return "-";
  const e = info(kind, i);
  return e ? e.name || humanize(e.type) : `${kind} ${i}`;
}

/** The type name of entry `i` of `kind` ("UNIT_WARRIOR"). */
export function tname(kind, i) {
  return info(kind, i)?.type ?? null;
}

/** The index of a type name in `kind`, or -1. */
export function tidx(kind, type) {
  return typeIndex.get(`${kind}:${type}`) ?? -1;
}

/** The order kinds of the production queue (OrderType index -> the table its data indexes). */
export const ORDER_KINDS = ["UnitType", "BuildingType", "ProjectType", "ProcessType"];

export function orderName(order, type) {
  return dname(ORDER_KINDS[order] ?? "UnitType", type);
}

// ---------------------------------------------------------------------------------------------
// The view and the map, with lookups.

export function plotKey(x, y) {
  return `${x},${y}`;
}

export function setView(view) {
  S.view = view;
  S.units = new Map();
  S.groups = new Map();
  S.cities = new Map();
  S.players = new Map();
  S.unitsAt = new Map();
  if (!view) return;
  for (const u of view.units) {
    S.units.set(u.id, u);
    const k = plotKey(u.x, u.y);
    if (!S.unitsAt.has(k)) S.unitsAt.set(k, []);
    S.unitsAt.get(k).push(u);
  }
  for (const g of view.groups) S.groups.set(g.id, g);
  for (const c of view.cities) S.cities.set(c.id, c);
  for (const p of view.players) S.players.set(p.id, p);
}

export function setMap(map) {
  S.map = map;
}

/** The human's player id and team. */
export function me() {
  return S.view?.me ?? null;
}

/** The units the viewer sees on a plot. */
export function unitsAt(x, y) {
  return S.unitsAt.get(plotKey(x, y)) ?? [];
}

/** The viewer's own units on a plot. */
export function ownUnitsAt(x, y) {
  const h = me()?.player;
  return unitsAt(x, y).filter((u) => u.owner === h);
}

/** The city on a plot (from the map's `city` array, then the view's list), or null. */
export function cityAt(x, y) {
  if (S.map) {
    const i = plotIndex(S.map, x, y);
    const id = i >= 0 ? S.map.city[i] : -1;
    if (id >= 0) return S.cities.get(id) ?? null;
  }
  for (const c of S.cities.values()) if (c.x === x && c.y === y) return c;
  return null;
}

/** A player's name: "you", a met player's leader name, or "Player N". */
export function playerName(p) {
  const m = me();
  if (m && p === m.player) return m.name ?? "you";
  return S.players.get(p)?.name ?? `Player ${p}`;
}

export function isBarbarian(p) {
  return S.players.get(p)?.barbarian ?? false;
}

/** A team's name: its members' names, or "Team N". */
export function teamName(t) {
  const m = me();
  if (m && t === m.team) return "your team";
  const names = [...S.players.values()].filter((p) => p.team === t).map((p) => p.name);
  return names.length ? names.join(" & ") : `Team ${t}`;
}

/** The groups waiting for orders (`pending.ready_groups`), in the server's order. */
export function readyGroups() {
  return S.view?.pending?.ready_groups ?? [];
}

/** The next ready group after `current` (the first one when `current` is not ready or none),
 *  preferring the group nearest (`dist`) to the plot given, as Civ4 cycles near the last
 *  selection. `skip` lists groups to leave out. */
export function nextReadyGroup(current, near = null, dist = null, skip = []) {
  const ready = readyGroups().filter((g) => g !== current && !skip.includes(g) && S.groups.has(g));
  if (ready.length === 0) return null;
  if (!near || !dist) {
    const after = readyGroups().indexOf(current);
    if (after >= 0) {
      const later = readyGroups().slice(after + 1).find((g) => ready.includes(g));
      if (later !== undefined) return later;
    }
    return ready[0];
  }
  let best = null;
  let bestD = Infinity;
  for (const g of ready) {
    const head = S.units.get(S.groups.get(g).head);
    if (!head) continue;
    const d = dist(near.x, near.y, head.x, head.y);
    if (d < bestD) {
      bestD = d;
      best = g;
    }
  }
  return best ?? ready[0];
}

/** W, Tab and "Next": the next waiting group in a cycle that selects every ready group before
 *  any one again (P5 review: nearest-first alone went back and forth between the two groups
 *  nearest each other). `visited` (a Set, updated here) holds the groups this cycle selected;
 *  the nearest (`dist`) ready group to `near` not yet visited is chosen, and when every ready
 *  group was, a new cycle starts from `current`. Returns the group, or null when no other
 *  group waits. */
export function cycleNext(current, visited, near = null, dist = null) {
  const cur = current ?? -1;
  if (cur >= 0) visited.add(cur);
  let g = nextReadyGroup(cur, near, dist, [...visited]);
  if (g === null) {
    visited.clear();
    if (cur >= 0) visited.add(cur);
    g = nextReadyGroup(cur, near, dist);
  }
  if (g !== null) visited.add(g);
  return g;
}

// ---------------------------------------------------------------------------------------------
// Deferred popups (design §2.3, §5.1): what the player answers at a control point, in C++
// order: events, votes, research, idle cities. A popup set aside ("Later") stays aside for the
// rest of the turn; the end-turn gate still counts it.

/** Keys of the deferred popups. */
export function popupKey(p, turn) {
  switch (p.kind) {
    case "event":
      return `event:${p.event.triggered}`;
    case "vote":
      return `vote:${p.vote.vote}`;
    case "research":
      return `research:${turn}`;
    case "city":
      return `city:${p.city}:${turn}`;
  }
  return "?";
}

/** The deferred popups to show now: `[{kind, key, ...}]`. */
export function popupQueue(pending, dismissed, turn) {
  if (!pending) return [];
  const out = [];
  for (const e of pending.events ?? []) out.push({ kind: "event", event: e });
  for (const v of pending.votes ?? []) out.push({ kind: "vote", vote: v });
  if (pending.research_needed) out.push({ kind: "research" });
  for (const c of pending.idle_cities ?? []) out.push({ kind: "city", city: c });
  return out.map((p) => ({ ...p, key: popupKey(p, turn) })).filter((p) => !dismissed.has(p.key));
}

/** Starts a turn's dismissals afresh when the turn changes. */
export function resetDismissed(turn) {
  if (S.dismissedTurn !== turn) {
    S.dismissed = new Set();
    S.dismissedTurn = turn;
  }
}

/** Forgets every dismissal: another game (new, loaded, or started elsewhere) shows its deferred
 *  popups again, also at the same turn (C++ saves the popup queue with the player). */
export function clearDismissed() {
  S.dismissed = new Set();
  S.dismissedTurn = null;
}

/** The label of a vote choice (`PlayerVoteTypes`: YES -2, NO -3, ABSTAIN -4, NEVER -5, or a
 *  team id in a team vote). */
export function voteChoiceLabel(choice, team = teamName) {
  switch (choice) {
    case -2:
      return "Yes";
    case -3:
      return "No";
    case -4:
      return "Abstain";
    case -5:
      return "Never (defy)";
    default:
      return choice >= 0 ? `Vote for ${team(choice)}` : `Choice ${choice}`;
  }
}

/** The answer to the transport picker (`confirm load_unit`): `do_command COMMAND_LOAD_UNIT` with
 *  `data1` = the transport's owner and `data2` = its id, as the LOADUNIT popup sends it
 *  (CvDLLButtonPopup.cpp `OnOkClicked`). A transport may be a teammate's: its owner comes from
 *  the view. */
export function loadUnitCommand(units, transport) {
  const owner = S.units.get(transport)?.owner ?? me()?.player ?? -1;
  return { c: "do_command", units, command: tidx("CommandType", "COMMAND_LOAD_UNIT"), data1: owner, data2: transport, alt: false, confirmed: false };
}

/** The lines of the end-turn gate (`confirm end_turn`). */
export function gateLines(c) {
  const out = [];
  const n = (k, one, many) => (k === 1 ? one : many.replace("%", k));
  if (c.events) out.push(n(c.events, "an event is waiting for your answer", "% events are waiting for your answer"));
  if (c.votes) out.push(n(c.votes, "a vote is open", "% votes are open"));
  if (c.research) out.push("no research is chosen");
  if (c.ready_units) out.push(n(c.ready_units, "a unit is waiting for orders", "% units are waiting for orders"));
  return out;
}

/** "3 turns", "1 turn", "-" (null or negative). */
export function turns(n) {
  if (n === null || n === undefined || n < 0) return "-";
  return n === 1 ? "1 turn" : `${n} turns`;
}

/** Moves for people: C++ counts 60 per move (`MOVE_DENOMINATOR`). */
export function moves(m) {
  const whole = Math.floor(m / 60);
  const rest = m % 60;
  if (rest === 0) return `${whole}`;
  const frac = rest % 20 === 0 ? `${rest / 20}/3` : rest % 30 === 0 ? "1/2" : `${rest}/60`;
  return whole ? `${whole} ${frac}` : frac;
}

/** The name of a save as `load` takes it, from a crash report's or status's path:
 *  ".../GAME/auto/T0012.ffhsave" -> "GAME/auto/T0012"; ".../GAME/name.ffhsave" -> "GAME/name". */
export function saveNameOfPath(path) {
  if (!path) return null;
  const parts = String(path).split(/[\\/]/).filter(Boolean);
  const file = parts.pop()?.replace(/\.ffhsave$/, "");
  if (!file) return null;
  const dir = parts.pop();
  if (dir === "auto") {
    const game = parts.pop();
    return game ? `${game}/auto/${file}` : null;
  }
  return dir ? `${dir}/${file}` : null;
}
