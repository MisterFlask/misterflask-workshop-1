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

/** The groups waiting for orders (`pending.ready_groups`: C++ `readyToMove(bAny = true)`, some
 *  unit can move), in the server's order: what the end-turn gate counts and what
 *  `updateSelectionList` asks of the selected group ("does it still wait"). */
export function readyGroups() {
  return S.view?.pending?.ready_groups ?? [];
}

/** The groups Civ4's cycle selects (the view's `selectable`: C++ `readyToSelect()`, whose
 *  default `bAny = false` needs every unit able to move), in the server's order (the player's
 *  groups). A stack in which one unit has used its moves waits (`readyGroups`) but is skipped
 *  by W, Enter, the plot cycle and the nearest-group search, as in Civ4 (P6 review). */
export function selectableGroups() {
  return (S.view?.groups ?? []).filter((g) => g.selectable).map((g) => g.id);
}

/** The next selectable group after `current` (the first one when `current` is not selectable
 *  or none), or, given a plot and `dist`, the one nearest to it (C++ `CvMap::findSelectionGroup`
 *  with `bReadyToSelect`: the first of the nearest in the player's group order). `skip` lists
 *  groups to leave out. */
export function nextSelectableGroup(current, near = null, dist = null, skip = []) {
  const all = selectableGroups();
  const ready = all.filter((g) => g !== current && !skip.includes(g) && S.groups.has(g));
  if (ready.length === 0) return null;
  if (!near || !dist) {
    const after = all.indexOf(current);
    if (after >= 0) {
      const later = all.slice(after + 1).find((g) => ready.includes(g));
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

/** C++ `CvPlayer::cycleSelectionGroups(pUnit, bForward)` (P6: Civ4's group cycle): the first
 *  selectable group (`ready`, a Set: `readyToSelect()`) after `current`'s place in the
 *  player's group cycle (`cycle`, the view's `group_cycle`), going round. `wrapped` when the
 *  search went past the end of the cycle (or started there, `current` being last or not in
 *  it) or came back to `current`: the exe then sends AutoMoves when a group has orders to
 *  continue (CvGame::cycleSelectionGroups). Returns `{group, wrapped}` (`group` null when none
 *  waits). */
export function cycleGroups(cycle, ready, current, forward = true) {
  const n = cycle.length;
  if (n === 0) return { group: null, wrapped: true };
  const at = current === null || current === undefined ? -1 : cycle.indexOf(current);
  let wrapped = false;
  let start;
  if (at < 0) {
    // No selection, or a group not in the cycle: from the head (C++: the node is NULL).
    start = forward ? 0 : n - 1;
    wrapped = current !== null && current !== undefined;
  } else {
    start = forward ? at + 1 : at - 1;
    if (start >= n || start < 0) {
      start = forward ? 0 : n - 1;
      wrapped = true;
    }
  }
  for (let k = 0; k < n; k++) {
    const raw = forward ? start + k : start - k;
    if (raw >= n || raw < 0) wrapped = true;
    const g = cycle[((raw % n) + n) % n];
    if (ready.has(g)) {
      if (g === current) wrapped = true;
      return { group: g, wrapped };
    }
  }
  return { group: null, wrapped: true };
}

/** The selectable groups on plot (x, y), in the plot's unit order (C++ `cyclePlotUnits(..,
 *  bAuto)`, which tests `readyToSelect()`): the own units' groups there, without `skip`. */
export function selectableGroupsOnPlot(x, y, skip = null) {
  const ready = new Set(selectableGroups());
  const out = [];
  for (const u of ownUnitsAt(x, y)) {
    if (u.group !== skip && ready.has(u.group) && !out.includes(u.group)) out.push(u.group);
  }
  return out;
}

/** C++ `CvGame::cycleCities(bForward)` for Home / End (CONTROL_NEXTCITY / PREVCITY): with a
 *  city selected (`current`, an id), the next / previous of `cities` (the player's, in its
 *  order), going round; with none, the city at the look-at plot (the exe's
 *  `selectLookAtCity(true)`, not in the DLL source: here the one nearest `near` by `dist`, the
 *  first of the nearest; the first city without a plot). Null when there is no city. */
export function cycleCity(cities, current, forward = true, near = null, dist = null) {
  if (!cities.length) return null;
  const i = current === null || current === undefined ? -1 : cities.findIndex((c) => c.id === current);
  if (i >= 0) return cities[(i + (forward ? 1 : -1) + cities.length) % cities.length];
  if (!near || !dist) return cities[0];
  let best = cities[0];
  let bestD = Infinity;
  for (const c of cities) {
    const d = dist(near.x, near.y, c.x, c.y);
    if (d < bestD) {
      bestD = d;
      best = c;
    }
  }
  return best;
}

/** C++ `CvGame::updateTestEndTurn`'s end-turn message ("press Enter to end the turn"): the turn
 *  is the human's and no group waits, `!hasReadyUnit(bAny)` with `bAny` when a unit is selected
 *  and unit cycling is on (`readyGroups`, else `selectableGroups`: a selected stack with a unit
 *  that can still move keeps the message off). Enter and Space end the turn only then
 *  (CvGame::canDoControl CONTROL_ENDTURN / _ALT); otherwise Enter cycles (CONTROL_CYCLEUNIT). */
export function endTurnMessage(selected = !!S.sel) {
  if (S.status?.phase !== "human") return false;
  const any = selected && !option("PLAYEROPTION_NO_UNIT_CYCLING");
  return !(any ? readyGroups() : selectableGroups()).some((g) => S.groups.has(g));
}

/** The PlayerOptionType index of an option, and whether the human has it on (`me.options`). */
export function option(name) {
  const i = tidx("PlayerOptionType", name);
  return i >= 0 && !!me()?.options?.[i];
}

// ---------------------------------------------------------------------------------------------
// Deferred popups (design §2.3, §5.1): what the player answers at a control point, in C++
// order: events, votes, elections, research, idle cities. A popup set aside ("Later") stays aside for the
// rest of the turn; the end-turn gate still counts it.

/** Keys of the deferred popups. */
export function popupKey(p, turn) {
  switch (p.kind) {
    case "event":
      return `event:${p.event.triggered}`;
    case "vote":
      return `vote:${p.vote.vote}`;
    case "election":
      return `election:${p.election.selection}`;
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
  for (const e of pending.elections ?? []) out.push({ kind: "election", election: e });
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

/** A trade item for people (`{item, data, name?}` of the `trade` query, design §4.6): the
 *  tech, resource, city (by its name), team, civic or religion it names, or the item's own
 *  name; gold with its amount. */
export function tradeItemLabel(it) {
  const type = tname("TradeableItem", it.item) ?? "";
  const n = it.data ?? 0;
  switch (type) {
    case "TRADE_GOLD":
      return n > 0 ? `${n} gold` : "Gold";
    case "TRADE_GOLD_PER_TURN":
      return n > 0 ? `${n} gold per turn` : "Gold per turn";
    case "TRADE_TECHNOLOGIES":
      return dname("TechType", n);
    case "TRADE_RESOURCES":
      return dname("BonusType", n);
    case "TRADE_CITIES":
      return it.name ?? S.cities.get(n)?.name ?? `City ${n}`;
    case "TRADE_CIVIC":
      return dname("CivicType", n);
    case "TRADE_RELIGION":
      return dname("ReligionType", n);
    case "TRADE_PEACE":
      return `Make peace with ${teamName(n)}`;
    case "TRADE_WAR":
      return `Declare war on ${teamName(n)}`;
    case "TRADE_EMBARGO":
      return `Stop trading with ${teamName(n)}`;
  }
  return dname("TradeableItem", it.item);
}

/** Trade items that go on both sides of a deal (C++ `CvDeal::isDual`): peace treaty, open
 *  borders, defensive pact, permanent alliance. */
export const DUAL_ITEMS = ["TRADE_PEACE_TREATY", "TRADE_OPEN_BORDERS", "TRADE_DEFENSIVE_PACT", "TRADE_PERMANENT_ALLIANCE"];

/** Why trade item `it` cannot go into a deal, or null: its own refusal (`denial`, a DenialType),
 *  or, for an item that goes on both sides (DUAL_ITEMS), its twin's on the other side (`other`,
 *  that side's table): the deal would hold both and be refused (P6 review: the AI's refusal of,
 *  say, open borders is on its own side only). `{denial, twin}`, `twin` when the refusal is the
 *  other side's. */
export function tradeRefusal(it, other) {
  const refused = (x) => x && x.denial !== null && x.denial !== undefined;
  if (refused(it)) return { denial: it.denial, twin: false };
  if (DUAL_ITEMS.includes(tname("TradeableItem", it.item))) {
    const twin = (other ?? []).find((o) => o.item === it.item);
    if (refused(twin)) return { denial: twin.denial, twin: true };
  }
  return null;
}

/** The lines of the end-turn gate (`confirm end_turn`). */
export function gateLines(c) {
  const out = [];
  const n = (k, one, many) => (k === 1 ? one : many.replace("%", k));
  if (c.events) out.push(n(c.events, "an event is waiting for your answer", "% events are waiting for your answer"));
  if (c.votes) out.push(n(c.votes, "a vote is open", "% votes are open"));
  if (c.elections) out.push(n(c.elections, "an election waits for your choice", "% elections wait for your choice"));
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
