// Units (design §5.1 "Units"): the selection, the stack list, the actions panel from the `group`
// query, moving and attacking with the path and combat previews, Shift waypoints, interface
// modes (go-to, route-to, range attack: pick a plot), grouping, and cycling over the ready groups.

import * as api from "./api.js";
import { S, emit, on, dname, tname, tidx, me, unitsAt, ownUnitsAt, cityAt, playerName, readyGroups, selectableGroups, nextSelectableGroup, cycleGroups, selectableGroupsOnPlot, option, moves, turns, info } from "./state.js";
import * as K from "./hotkeys.js";
import { h, fill, toast } from "./dom.js";
import { openModal } from "./modals.js";
import { run, actionCommand, sending } from "./commands.js";
import { plotDistance, plotIndex, humanize } from "./tiles.js";
import { render as markup } from "./markup.js";
import { look, iconEl, unitIcon, barEl, pix, typeIcon, promotionIcon, uiIcon } from "./look.js";

/** An 8 px icon of the ui set, or the glyph before the art loaded. */
const icon8 = (name, glyph, title) => iconEl(`icon8/${name}`, { title }) ?? glyph;

/** A unit's owner as the art draws it (the map's heraldry), or null. */
const ownerLook = (p) => mapView?.art?.owners?.get(p) ?? null;

let mapView = null;
let panel = null;
/** The last `group` query reply, the units it answers (`groupFor`, as `unitsKey`), the key of
 *  the latest request (units, view version, turn) and that request while it runs. Only a reply
 *  for the selected units is used (`reply()`): the previous selection's actions are neither
 *  drawn nor sent while the new selection's reply is on its way (P5 review). */
let group = null;
let groupFor = "";
let groupKey = "";
let groupPending = null;
/** An interface mode: `{kind: "target", action, waypoints?}` (`waypoints`: go-to waypoints
 *  were added with Shift, the mode ends when Shift is released). */
let mode = null;
/** After a command on the selection: the plot the selection was on (C++ `getOriginalPlot`),
 *  to move on from once the group no longer waits for orders (`updateSelection`); else null. */
let advance = null;
/** An attack whose outcome is inferred from the view diff (the server sends no combat result):
 *  the target tile, the attacker's plot, and the defenders'/attackers' HP before the attack,
 *  snapshotted when the command is applied (S.units is still pre-combat then) and consumed in
 *  `viewChanged` to flash the struck tile. Null when the last command was not an attack. */
let pendingCombat = null;
/** Shift-right-click waypoints, held while Shift is down and sent when it is released
 *  (`flushWaypoints`): `{units, points: [[x, y]]}` or null. The exe sends each one at once but
 *  no AutoMoves while Shift is down (CvGame::updateTestEndTurn `if (!gDLL->shiftKey())`), and
 *  the session runs the AutoMoves rule right after a command that leaves nothing ready, so the
 *  client holds them instead (P5 review). */
let waypoints = null;
/** Hover previews: the plot, and the replies. */
let hover = null;
let hoverTimer = null;
let preview = { path: null, combat: null, plot: null };

/** A selection's units as a key. */
function unitsKey(units) {
  return (units ?? []).join(",");
}

/** The `group` reply when it answers the selected units, else null. */
function reply() {
  return group && S.sel && groupFor === unitsKey(S.sel.units) ? group : null;
}

export function initUnits(view, panelEl) {
  mapView = view;
  panel = panelEl;
  on("applied", (c) => {
    if (c?.units && S.sel && c.units.some((u) => S.sel.units.includes(u))) advance = { x: S.sel.x, y: S.sel.y };
    // Combat carries no result, so snapshot an attack now (S.units is still pre-combat) and infer
    // the outcome from the diff in viewChanged. An attack is a move onto a tile with a foreign
    // unit (the same gate the combat-odds preview uses).
    pendingCombat = null;
    if (c?.c === "move" && typeof c.x === "number" && typeof c.y === "number") {
      const mep = me()?.player;
      const defenders = unitsAt(c.x, c.y).filter((u) => u.owner !== mep);
      if (defenders.length) {
        pendingCombat = {
          tx: c.x,
          ty: c.y,
          ax: S.sel?.x,
          ay: S.sel?.y,
          defenders: defenders.map((u) => ({ id: u.id, hp: u.hp })),
          attackers: (c.units ?? []).map((id) => S.units.get(id)).filter(Boolean).map((u) => ({ id: u.id, hp: u.hp })),
        };
      }
    }
  });
}

/** Flashes the struck tile when a snapshotted attack (`pendingCombat`) resolved: a defender lost
 *  (death) or was hurt (hit), else the attacker if it took the loss. Best-effort; a stale or
 *  unmatched snapshot just clears. */
function flashCombat() {
  const pc = pendingCombat;
  pendingCombat = null;
  if (!pc || !mapView) return;
  try {
    const gone = (id) => !S.units.has(id);
    const hpNow = (id) => S.units.get(id)?.hp;
    const defDied = pc.defenders.some((d) => gone(d.id));
    const defHit = !defDied && pc.defenders.some((d) => hpNow(d.id) < d.hp);
    const atkDied = pc.attackers.some((a) => gone(a.id));
    const atkHit = !atkDied && pc.attackers.some((a) => hpNow(a.id) < a.hp);
    if (defDied) mapView.flashTile(pc.tx, pc.ty, "death");
    else if (defHit) mapView.flashTile(pc.tx, pc.ty, "hit");
    else if (atkDied && pc.ax != null) mapView.flashTile(pc.ax, pc.ay, "death");
    else if (atkHit && pc.ax != null) mapView.flashTile(pc.ax, pc.ay, "hit");
  } catch {
    /* a flash is never worth a thrown handler */
  }
}

// ---------------------------------------------------------------------------------------------
// Selection

/** Selects own units `ids` (on one plot), or nothing. */
export function select(ids, { center = false } = {}) {
  // Waypoints of other units are sent before the selection changes.
  if (waypoints && unitsKey(waypoints.units) !== unitsKey(ids)) flushWaypoints();
  const units = (ids ?? []).map((id) => S.units.get(id)).filter((u) => u && u.owner === me()?.player);
  if (units.length === 0) {
    S.sel = null;
  } else {
    const { x, y } = units[0];
    S.sel = { units: units.filter((u) => u.x === x && u.y === y).map((u) => u.id), x, y };
    if (center) mapView.reveal(x, y);
  }
  setMode(null);
  advance = null;
  clearPreview();
  syncOverlay();
  refreshGroup();
  emit("selection");
}

/** Selects group `g` (all its units). */
export function selectGroup(g, opts) {
  const grp = S.groups.get(g);
  if (grp) select(grp.units, opts);
}

export function clearSelection() {
  select([]);
}

/** The groups the cycle selects (C++ `readyToSelect()`, every unit can move), as a Set. */
function selectableSet() {
  return new Set(selectableGroups().filter((g) => S.groups.has(g)));
}

/** C++ `CvGame::cycleSelectionGroups(bClear, bForward)` (W, Enter, Next; P6): the next group in
 *  the player's group cycle (`view.group_cycle`, C++ `m_groupCycle`) after the selection's that
 *  the cycle selects (`readyToSelect()`: every unit can move), going round; with no selection,
 *  the selectable group nearest the centre of the map view (`findSelectionGroup` at the look-at
 *  plot). When the search went round and a group has orders to continue, the exe sends
 *  AutoMoves (they move now). */
export function selectNext({ forward = true } = {}) {
  const cur = currentGroup();
  if (cur === null) {
    const near = mapView?.center?.() ?? null;
    const dist = near && S.map ? (ax, ay, bx, by) => plotDistance(S.map, ax, ay, bx, by) : null;
    const g = nextSelectableGroup(-1, near, dist);
    if (g === null || g === undefined) return false;
    selectGroup(g, { center: true });
    return true;
  }
  const { group, wrapped } = cycleGroups(S.view?.group_cycle ?? selectableGroups(), selectableSet(), cur, forward);
  if (wrapped && S.view?.has_auto_unit) autoMoves();
  if (group === null) return false;
  selectGroup(group, { center: true });
  return true;
}

/** C++ `CvGame::updateSelectionList` (after a command, P6): once the selected group no longer
 *  waits for orders (`readyToSelect(true)`: no unit can move, `readyGroups`), the next
 *  selectable group (`readyToSelect()`) on the plot it started from, then on its plot, then the
 *  next in the group cycle (`cycleSelectionGroups`, AutoMoves on a wrap), and no selection when
 *  none is selectable; nothing when the player turned unit cycling off
 *  (PLAYEROPTION_NO_UNIT_CYCLING). */
function updateSelection(origin) {
  if (option("PLAYEROPTION_NO_UNIT_CYCLING")) return false;
  const cur = currentGroup();
  for (const p of [origin, S.sel ? { x: S.sel.x, y: S.sel.y } : null]) {
    if (!p) continue;
    const g = selectableGroupsOnPlot(p.x, p.y, cur)[0];
    if (g !== undefined) {
      selectGroup(g, { center: true });
      return true;
    }
  }
  if (cur === null) {
    // The selected units are gone (killed, joined a city): the selectable group nearest the
    // plot they were on (C++ `findSelectionGroup` at the look-at plot).
    const dist = origin && S.map ? (ax, ay, bx, by) => plotDistance(S.map, ax, ay, bx, by) : null;
    const g = nextSelectableGroup(-1, origin, dist);
    if (g === null || g === undefined) return false;
    selectGroup(g, { center: true });
    return true;
  }
  const { group, wrapped } = cycleGroups(S.view?.group_cycle ?? selectableGroups(), selectableSet(), cur);
  if (wrapped && S.view?.has_auto_unit) autoMoves();
  if (group === null || group === cur) {
    S.sel = null;
    syncOverlay();
    refreshGroup();
    emit("selection");
    return false;
  }
  selectGroup(group, { center: true });
  return true;
}

/** CvMessageControl::sendAutoMoves (a cycle that went round with orders waiting). */
function autoMoves() {
  setTimeout(async () => {
    for (let i = 0; sending() && i < 200; i++) await new Promise((r) => setTimeout(r, 20));
    if (S.status?.phase === "human") run({ c: "auto_moves" }, { quiet: true });
  }, 0);
}

/** The group the selection is (its first unit's). */
function currentGroup() {
  if (!S.sel) return null;
  return S.units.get(S.sel.units[0])?.group ?? null;
}

/** Called when a new view arrived: keeps the selection on the units that still exist (they may
 *  have moved), or moves on to the next ready group after a command finished with it. */
export function viewChanged() {
  if (pendingCombat) flashCombat();
  if (S.sel) {
    const alive = S.sel.units.map((id) => S.units.get(id)).filter((u) => u && u.owner === me()?.player);
    if (alive.length === 0) {
      S.sel = null;
    } else {
      const { x, y } = alive[0];
      S.sel = { units: alive.filter((u) => u.x === x && u.y === y).map((u) => u.id), x, y };
    }
  }
  if (advance) {
    const origin = advance;
    advance = null;
    const g = currentGroup();
    if (!S.sel || g === null || !readyGroups().includes(g)) updateSelection(origin) || (S.sel && refreshGroup());
    else refreshGroup();
  } else {
    refreshGroup();
  }
  clearPreview();
  syncOverlay();
}

/** A click on plot (x, y): picks the plot in an interface mode, else selects the units there
 *  (again on the same plot: the next group there), else opens the city or shows the plot. */
export function click(x, y, ev) {
  if (mode) {
    pickPlot(x, y, ev);
    return;
  }
  const own = ownUnitsAt(x, y);
  if (own.length) {
    const groups = [...new Set(own.map((u) => u.group))];
    let g = groups.find((gr) => readyGroups().includes(gr)) ?? groups[0];
    if (S.sel && S.sel.x === x && S.sel.y === y && groups.length > 1) {
      const i = groups.indexOf(currentGroup());
      g = groups[(i + 1) % groups.length];
    }
    if (ev?.shiftKey) select(own.map((u) => u.id));
    else selectGroup(g);
    return;
  }
  const city = cityAt(x, y);
  if (city && city.owner === me()?.player) {
    emit("open-city", city.id);
    return;
  }
  S.sel = null;
  syncOverlay();
  emit("selection");
  renderPlotInfo(x, y);
}

/** A right-click on (x, y): move the selection there (Shift: add a waypoint). */
export function rightClick(x, y, ev) {
  if (mode) {
    setMode(null);
    return;
  }
  if (!S.sel) return;
  if (ev?.shiftKey) addWaypoint(x, y);
  else moveTo(x, y);
}

/** Moves the selection to (x, y) now, replacing its orders (waypoints not sent yet are
 *  dropped: the move replaces them). */
export async function moveTo(x, y) {
  if (!S.sel) return;
  if (waypoints && unitsKey(waypoints.units) === unitsKey(S.sel.units)) clearWaypoints();
  if (S.sel.x === x && S.sel.y === y) return;
  await run({ c: "move", units: S.sel.units, x, y, append: false });
}

// ---------------------------------------------------------------------------------------------
// Shift waypoints

/** Adds waypoint (x, y) for the selection (Shift is down). */
export function addWaypoint(x, y) {
  if (!S.sel || S.status?.phase !== "human") return;
  if (!waypoints || unitsKey(waypoints.units) !== unitsKey(S.sel.units)) {
    flushWaypoints();
    waypoints = { units: [...S.sel.units], points: [] };
  }
  waypoints.points.push([x, y]);
  mapView?.setOverlay({ waypoints: waypoints.points });
  renderPanel();
}

/** The waypoints not sent yet (for the tests and the panel). */
export function pendingWaypoints() {
  return waypoints ? { units: [...waypoints.units], points: waypoints.points.map((p) => [...p]) } : null;
}

/** Drops the waypoints not sent yet (Esc). Returns whether there were any. */
export function clearWaypoints() {
  const had = !!waypoints;
  waypoints = null;
  mapView?.setOverlay({ waypoints: null });
  if (had) renderPanel();
  return had;
}

/** Shift was released (or the window lost the focus): sends the waypoints, each a `move` with
 *  `append`, as the exe sent them, waiting for the control point between them (the first may
 *  let the AutoMoves rule run the game on). A refused or cancelled one drops the rest. Once
 *  sent, the selection moves on when the group no longer waits for orders (the `applied`
 *  rule). */
export async function flushWaypoints() {
  const w = waypoints;
  clearWaypoints();
  if (!w?.points.length) return;
  let st = S.status;
  for (const [x, y] of w.points) {
    if (!(await atControl(st))) return;
    for (let i = 0; sending() && i < 500; i++) await new Promise((r) => setTimeout(r, 20));
    const r = await run({ c: "move", units: w.units, x, y, append: true });
    if (!r?.r?.applied) return;
    st = r.status ?? S.status;
  }
}

/** Waits (long-polling from status `st`) until the player has control again; false when the
 *  game left the player's turn (over, crashed, no game). */
async function atControl(st) {
  for (let i = 0; st && i < 1000; i++) {
    if (st.phase === "human") return true;
    if (st.phase !== "running" && st.phase !== "decision") return false;
    try {
      st = await api.wait(st.version, 20000);
    } catch {
      return false;
    }
  }
  return false;
}

/** Sends the action (a `group` query action); a `target` action first asks for a plot. An
 *  action of other units than the selected ones (a reply that came late) is not sent. */
export async function doAction(a) {
  if (!a.enabled) return;
  if (a.cmd?.units && unitsKey(a.cmd.units) !== unitsKey(S.sel?.units)) return;
  if (a.kind === "target") {
    if (!a.any && (a.targets ?? []).length === 0) {
      toast("No target in range.", "error");
      return;
    }
    setMode({ kind: "target", action: a });
    return;
  }
  await run(a.cmd);
}

/** A key press for the selection's actions (P6: Civ4's hotkeys, from the XML through
 *  `/api/data`): pressed just after the selection changed, it waits for the selection's own
 *  actions (P5 review: not the previous selection's), then runs the enabled action bound to the
 *  key with the highest hotkey priority (`hotkeys.js`). Resolves with whether one ran. */
export async function hotkeyEvent(ev) {
  const e = { code: ev.code, key: ev.key, shiftKey: ev.shiftKey, altKey: ev.altKey, ctrlKey: ev.ctrlKey, metaKey: ev.metaKey };
  for (let i = 0; i < 3 && S.sel && !reply() && groupPending; i++) await groupPending;
  const r = reply();
  if (!r?.actions) return false;
  const a = K.actionForKey(r.actions, e, S.data);
  if (!a) return false;
  await doAction(a);
  return true;
}

/** The selection's actions (the `group` reply held; none while it is on its way). */
export function currentActions() {
  return reply()?.actions ?? [];
}

/** Enters an interface mode, `{kind: "target", action}` (pick a plot for the action); null
 *  leaves it. */
export function setMode(m) {
  mode = m;
  const targets = m?.kind === "target" ? (m.action.any ? null : m.action.targets) : null;
  // The route-to mode's path preview is drawn in ochre (docs/ART.md 11.7).
  mapView?.setOverlay({ targets: targets ?? (m ? [] : null), routeTo: m?.kind === "target" && m.action.mode === "INTERFACEMODE_ROUTE_TO" });
  document.body.classList.toggle("picking", !!m);
  emit("mode", m);
}

export function currentMode() {
  return mode;
}

/** Whether action `a` is the go-to interface mode (INTERFACEMODE_GO_TO: a `move`). */
export function isGoTo(a) {
  return a?.kind === "target" && a.mode === "INTERFACEMODE_GO_TO" && a.cmd?.c === "move";
}

/** A plot picked in an interface mode. Go-to with Shift held adds a waypoint: C++ sends a
 *  go-to pick through `selectionListMove(pPlot, bAlt, bShift, bCtrl)`, which pushes
 *  MISSION_MOVE_TO with `bShift` as append (and keeps it through the DECLAREWARMOVE popup); the
 *  waypoints are held like Shift-right-click's and sent when Shift is released, and the mode
 *  stays on meanwhile so that more can be added (P6 review). */
async function pickPlot(x, y, ev) {
  const m = mode;
  if (!S.sel) {
    setMode(null);
    return;
  }
  const a = m.action;
  if (!a.any && !(a.targets ?? []).some(([tx, ty]) => tx === x && ty === y)) {
    setMode(null);
    toast("Not a valid target.", "error");
    return;
  }
  if (ev?.shiftKey && isGoTo(a)) {
    m.waypoints = true;
    addWaypoint(x, y);
    return;
  }
  setMode(null);
  if (isGoTo(a)) await moveTo(x, y);
  else await run(actionCommand(a, x, y));
}

/** Shift was released (or the window lost the focus): the held waypoints are sent, and a go-to
 *  mode that added them ends. */
export function shiftReleased() {
  if (mode?.waypoints) setMode(null);
  return flushWaypoints();
}

/** Merges the selected units into one group (`join_group` of each into the first's). */
export async function groupAll() {
  if (!S.sel) return;
  const own = ownUnitsAt(S.sel.x, S.sel.y);
  const head = S.sel.units[0];
  const headGroup = S.units.get(head)?.group;
  for (const u of own) {
    if (u.group !== headGroup) await run({ c: "join_group", unit: u.id, head }, { quiet: true });
  }
  select(own.map((u) => u.id));
}

/** Splits the selection: each selected unit its own group. */
export async function splitAll() {
  if (!S.sel) return;
  const ids = [...S.sel.units];
  for (const u of ids) await run({ c: "join_group", unit: u, head: null }, { quiet: true });
  select([ids[0]]);
}

// ---------------------------------------------------------------------------------------------
// Previews: the path and the attack odds under the mouse.

function clearPreview() {
  preview = { path: null, combat: null, plot: null };
  mapView?.setOverlay({ path: null, odds: null });
}

/** The mouse is over (x, y) (null: left the map). */
export function hoverAt(x, y) {
  hover = x === null ? null : { x, y };
  clearTimeout(hoverTimer);
  preview = { path: null, combat: null, plot: null };
  mapView.setOverlay({ path: null, odds: null });
  if (!hover) return;
  showTip();
  hoverTimer = setTimeout(() => loadPreview(x, y), 90);
}

async function loadPreview(x, y) {
  const key = `${x},${y}`;
  const still = () => hover && `${hover.x},${hover.y}` === key;
  const sel = S.sel;
  const jobs = [];
  if (S.status?.phase === "human" || S.status?.phase === "decision") {
    jobs.push(
      api.query("plot", { x, y }).then((r) => still() && (preview.plot = r)).catch(() => {}),
    );
  }
  if (sel && S.status?.phase === "human" && !(sel.x === x && sel.y === y)) {
    jobs.push(
      api.query("path", { units: sel.units, x, y }).then((r) => {
        if (!still()) return;
        preview.path = r;
        mapView.setOverlay({ path: r });
      }).catch(() => {}),
    );
    const foreign = unitsAt(x, y).some((u) => u.owner !== me()?.player);
    if (foreign) {
      jobs.push(
        api.query("combat", { units: sel.units, x, y }).then((r) => {
          if (!still()) return;
          preview.combat = r;
          // The odds chip at the end of an attack path (permille; air strikes have none).
          mapView.setOverlay({ odds: r?.valid && r.odds !== null && r.odds !== undefined ? r.odds : null });
        }).catch(() => {}),
      );
    }
  }
  await Promise.all(jobs);
  if (still()) showTip();
}

/** The tooltip of the hovered plot: what the map arrays and the view say, then the plot query,
 *  the path and the attack odds once they arrive. */
function showTip() {
  if (!hover || !S.map) return mapView.hideTip();
  const { x, y } = hover;
  const i = plotIndex(S.map, x, y);
  if (i < 0) return mapView.hideTip();
  const m = S.map;
  const lines = [];
  if (m.vis[i] === 0) {
    lines.push(h("div.tip-title", `(${x}, ${y}) unexplored`));
    return mapView.showTip(lines);
  }
  const terrain = dname("TerrainType", m.terrain[i]);
  const pt = tname("PlotType", m.plot_type[i]);
  const parts = [pt === "PLOT_PEAK" ? "Peak" : terrain];
  if (pt === "PLOT_HILLS") parts.push("Hills");
  if (m.feature[i] >= 0) parts.push(dname("FeatureType", m.feature[i]));
  lines.push(h("div.tip-title", parts.join(", "), h("span.dim", ` (${x}, ${y})${m.vis[i] === 1 ? " fogged" : ""}`)));
  const extra = [];
  if (m.bonus[i] >= 0) extra.push(dname("BonusType", m.bonus[i]));
  if (m.improvement[i] >= 0) extra.push(dname("ImprovementType", m.improvement[i]));
  if (m.route[i] >= 0) extra.push(dname("RouteType", m.route[i]));
  if (m.river[i] > 0 && (m.river[i] & 3)) extra.push("River");
  if (extra.length) lines.push(h("div", extra.join(" · ")));
  if (m.owner[i] >= 0) lines.push(h("div.dim", `Owner: ${playerName(m.owner[i])}`));
  const p = preview.plot;
  if (p && p.x === x && p.y === y && p.yields) {
    lines.push(
      look.art
        ? h(
            "div.tip-yields",
            p.yields.map((v, j) => h("span.yield-n", { title: dname("YieldType", j) }, uiIcon("yield", j, 8), String(v))),
            h("span.yield-n", uiIcon("icon", "defense", 8), `${p.defense}%`),
            h("span.yield-n", uiIcon("icon", "moves", 8), String(p.move_cost)),
            h("span.sr", `Yields ${p.yields[0]} food, ${p.yields[1]} production, ${p.yields[2]} commerce · defence ${p.defense}% · move cost ${p.move_cost}`),
          )
        : h("div", `Yields ${p.yields[0]} food, ${p.yields[1]} production, ${p.yields[2]} commerce · defence ${p.defense}% · move cost ${p.move_cost}`),
    );
  }
  const city = cityAt(x, y);
  if (city) lines.push(h("div.tip-city", `${city.capital ? "★ " : ""}${city.name}`, h("span.dim", ` (${playerName(city.owner)}${city.size !== null && city.size !== undefined ? `, size ${city.size}` : ""})`)));
  const units = unitsAt(x, y);
  if (units.length) {
    const shown = units.slice(0, 8).map((u) => h("div.tip-unit", `${dname("UnitType", u.type)}`, h("span.dim", ` ${playerName(u.owner)}${u.hp < u.max_hp ? `, ${Math.round((100 * u.hp) / u.max_hp)}% health` : ""}`)));
    lines.push(...shown);
    if (units.length > 8) lines.push(h("div.dim", `and ${units.length - 8} more`));
  }
  const c = preview.combat;
  if (c?.valid) lines.push(combatLines(c));
  const path = preview.path;
  if (path && S.sel) {
    if (!path.ok) lines.push(h("div.warn", "No path"));
    else {
      const bits = [`${turns(path.turns)}`];
      if (path.attack) bits.push("attack");
      if (path.declare_war_team !== null) bits.push("declares war");
      if (path.approximate) bits.push("approximate");
      lines.push(h("div.tip-path", `Move: ${bits.join(", ")} · right-click to go`));
    }
  }
  mapView.showTip(lines);
}

/** Labels of CombatDetails members (C++ `setCombatPlotHelp`). */
const MODIFIERS = {
  extra_combat_percent: "strength bonus",
  plot_defense_modifier: "terrain defence",
  fortify_modifier: "fortified",
  city_defense_modifier: "city defence",
  hills_attack_modifier: "hills attack",
  hills_defense_modifier: "hills defence",
  feature_attack_modifier: "feature attack",
  feature_defense_modifier: "feature defence",
  terrain_attack_modifier: "terrain attack",
  terrain_defense_modifier: "terrain defence (unit)",
  city_attack_modifier: "city attack",
  domain_defense_modifier: "domain defence",
  city_barbarian_defense_modifier: "city defence vs barbarians",
  class_defense_modifier: "vs unit class",
  class_attack_modifier: "vs unit class",
  combat_modifier_t: "combat modifier",
  combat_modifier_a: "combat modifier",
  domain_modifier_a: "domain",
  domain_modifier_t: "domain",
  animal_combat_modifier_a: "vs animals",
  animal_combat_modifier_t: "vs animals",
  animal_combat_modifier_ta: "animal",
  animal_combat_modifier_aa: "animal",
  barbarian_combat_modifier_tb: "barbarian",
  barbarian_combat_modifier_ab: "barbarian",
  river_attack_modifier: "across a river",
  amphib_attack_modifier: "amphibious",
  kamikaze_modifier: "kamikaze",
};

function modifierList(d) {
  return Object.entries(MODIFIERS)
    .filter(([k]) => d?.[k])
    .map(([k, label]) => `${d[k] > 0 ? "+" : ""}${d[k]}% ${label}`);
}

/** The attack preview: odds, strengths, the modifiers of both sides. */
export function combatLines(c) {
  const pct = (v) => `${(v / 10).toFixed(1)}%`;
  const str = (v) => (v / 100).toFixed(2);
  const att = S.units.get(c.attacker);
  const box = h("div.tip-combat");
  box.append(h(`div.odds${c.odds !== null && c.odds < 500 ? ".bad" : ""}`, c.odds === null ? "Air strike" : `Odds ${pct(c.odds)}`, c.withdraw ? h("span.dim", ` · retreat ${pct(c.withdraw)}`) : null));
  box.append(h("div", `${att ? dname("UnitType", att.type) : "Attacker"} ${str(c.attacker_strength)} vs ${dname("UnitType", c.defender_type)} ${str(c.defender_strength)}`, h("span.dim", ` (${playerName(c.defender_owner)})`)));
  const a = modifierList(c.attacker_details);
  const d = modifierList(c.defender_details);
  if (a.length) box.append(h("div.mods", h("b", "Attacker: "), a.join(", ")));
  if (d.length) box.append(h("div.mods", h("b", "Defender: "), d.join(", ")));
  if (c.declare_war_team !== null) box.append(h("div.warn", "Attacking declares war"));
  return box;
}

// ---------------------------------------------------------------------------------------------
// The panel

function syncOverlay() {
  mapView?.setOverlay({ sel: S.sel ? { x: S.sel.x, y: S.sel.y } : null, units: S.sel?.units ?? [] });
}

/** Re-reads the selection's actions (`group` query) and redraws the panel. Until the reply for
 *  newly selected units arrives the panel shows "…" and the hotkeys wait (a reply for the same
 *  units at an older view stays shown meanwhile). */
export async function refreshGroup() {
  if (!S.sel) {
    group = null;
    groupFor = "";
    groupKey = "";
    groupPending = null;
    renderPanel();
    return;
  }
  const units = [...S.sel.units];
  const uk = unitsKey(units);
  const key = `${uk}@${S.view?.view_version}:${S.view?.turn}`;
  if (key === groupKey && (groupPending || (group && groupFor === uk))) {
    renderPanel();
    return groupPending ?? undefined;
  }
  groupKey = key;
  if (groupFor !== uk) {
    group = null;
    groupFor = "";
  }
  renderPanel();
  const pending = (async () => {
    let r;
    try {
      r = await api.query("group", { units });
    } catch (e) {
      r = { error: e.detail ?? e.message };
    }
    if (key !== groupKey) return;
    group = r;
    groupFor = uk;
    groupPending = null;
    renderPanel();
  })();
  groupPending = pending;
  return pending;
}

function actionLabel(a) {
  switch (a.kind) {
    case "mission": {
      const m = tname("MissionType", a.mission);
      if (m === "MISSION_BUILD") return dname("BuildType", a.data1);
      if (m === "MISSION_SPREAD") return `Spread ${dname("ReligionType", a.data1)}`;
      if (m === "MISSION_JOIN") return `Join as ${dname("SpecialistType", a.data1)}`;
      if (m === "MISSION_CONSTRUCT") return `Build ${dname("BuildingType", a.data1)}`;
      if (m === "MISSION_LEAD") return `Lead ${dname("UnitType", S.units.get(a.data1)?.type)}`;
      if (m === "MISSION_DISCOVER") return "Discover a technology";
      if (m === "MISSION_HURRY") return "Hurry production";
      if (m === "MISSION_TRADE") return "Trade mission";
      if (m === "MISSION_GREAT_WORK") return "Great work";
      if (m === "MISSION_GOLDEN_AGE") return "Start a golden age";
      return dname("MissionType", a.mission);
    }
    case "target":
      return humanize(a.mode.replace(/^INTERFACEMODE_/, "MODE_"));
    case "command":
      return dname("CommandType", a.command);
    case "load":
      return "Load";
    case "automate":
      return AUTOMATE_LABELS[tname("AutomateType", a.automate)] ?? dname("AutomateType", a.automate);
    case "spell":
      return dname("SpellType", a.spell);
    case "promotion":
      return dname("PromotionType", a.promotion);
    case "upgrade": {
      const u = info("UnitType", a.unit);
      return `Upgrade: ${dname("UnitType", a.unit)} (${a.price} gold${u ? `; ${u.combat}/${u.combat_defense} strength, ${u.moves} moves` : ""})`;
    }
  }
  return a.kind;
}

function hotkeyOf(a) {
  return K.label(K.bindings(a, S.data)[0]);
}

const typeOf = (a) => (a.kind === "mission" ? tname("MissionType", a.mission) : a.kind === "command" || a.kind === "load" ? tname("CommandType", a.command) : "");
const GREAT_PERSON = ["MISSION_JOIN", "MISSION_CONSTRUCT", "MISSION_DISCOVER", "MISSION_HURRY", "MISSION_TRADE", "MISSION_GREAT_WORK", "MISSION_GOLDEN_AGE", "MISSION_LEAD", "MISSION_INFILTRATE"];
const RELIGION = ["MISSION_SPREAD", "MISSION_SPREAD_CORPORATION"];
const CARGO = ["COMMAND_LOAD", "COMMAND_LOAD_UNIT", "COMMAND_UNLOAD", "COMMAND_UNLOAD_ALL"];
/** The automate buttons (Civ4's TXT_KEY_ACTION_AUTOMATE_*, not in FfH's text files). */
const AUTOMATE_LABELS = {
  AUTOMATE_BUILD: "Automate: improve the land",
  AUTOMATE_NETWORK: "Automate: build the trade network",
  AUTOMATE_CITY: "Automate: improve this city",
  AUTOMATE_EXPLORE: "Explore (automated)",
  AUTOMATE_RELIGION: "Automate: spread the religion",
};

/** The actions panel's sections; each lists its actions in the action list's order
 *  (`iOrderPriority`, C++ `SetGlobalActionInfo`). */
const SECTIONS = [
  ["Orders", (a) => (a.kind === "mission" && ![...GREAT_PERSON, ...RELIGION, "MISSION_BUILD"].includes(typeOf(a))) || a.kind === "target" || (a.kind === "command" && !CARGO.includes(typeOf(a)))],
  ["Build", (a) => typeOf(a) === "MISSION_BUILD"],
  ["Great person", (a) => GREAT_PERSON.includes(typeOf(a))],
  ["Religion", (a) => RELIGION.includes(typeOf(a))],
  ["Cargo", (a) => a.kind === "load" || CARGO.includes(typeOf(a))],
  ["Automation", (a) => a.kind === "automate"],
  ["Spells", (a) => a.kind === "spell"],
  ["Promotions", (a) => a.kind === "promotion"],
  ["Upgrades", (a) => a.kind === "upgrade"],
];

function actionButton(a) {
  const key = hotkeyOf(a);
  const help = a.kind === "spell" && a.help ? a.help : a.kind === "promotion" ? (info("PromotionType", a.promotion)?.help ?? "") : "";
  const extra = a.turns !== undefined ? ` (${turns(a.turns)})` : "";
  const btn = h(
    "button.action",
    {
      disabled: !a.enabled,
      "data-kind": a.kind,
      "data-type": a.kind === "mission" ? tname("MissionType", a.mission) : a.kind === "target" ? a.mode : a.kind === "command" || a.kind === "load" ? tname("CommandType", a.command) : a.kind === "automate" ? tname("AutomateType", a.automate) : a.kind === "spell" ? tname("SpellType", a.spell) : a.kind === "promotion" ? tname("PromotionType", a.promotion) : a.kind === "upgrade" ? tname("UnitType", a.unit) : "",
      title: [key ? `[${key}]` : "", help ? markup(help).textContent : ""].filter(Boolean).join(" "),
      onclick: () => doAction(a),
    },
    a.kind === "promotion" ? promotionIcon(a.promotion, { u: 1 }) : null,
    actionLabel(a) + extra,
    key ? h("kbd", key) : null,
  );
  return btn;
}

/** "automated" (the AI moves the unit's group: Stop automation gives it back) or "AI control"
 *  (an FfH AI-controlled unit, e.g. enraged), else null. */
function automationBadge(u) {
  const grp = u.group !== null && u.group !== undefined ? S.groups.get(u.group) : null;
  if (!grp) return null;
  if (grp.automated !== null && grp.automated !== undefined) {
    const label = AUTOMATE_LABELS[tname("AutomateType", grp.automated)] ?? "Automated";
    return h("span.badge.auto", { title: `${label}: the AI moves this unit (Stop automation to take it back)` }, "automated");
  }
  return grp.ai_control ? h("span.badge.auto", { title: "AI-controlled (FfH): the AI moves this unit" }, "AI control") : null;
}

function unitRow(u, selected) {
  const busy = u.activity !== null && u.activity !== undefined && tname("ActivityType", u.activity) !== "ACTIVITY_AWAKE";
  const row = h(
    "div.unit-row",
    {
      "data-unit": u.id,
      class: [selected ? "selected" : "", u.transport !== null && u.transport !== undefined ? "cargo" : ""].filter(Boolean).join(" "),
      title: "Click: select its group. Ctrl-click: add or remove this unit.",
      onclick: (ev) => {
        if (ev.ctrlKey || ev.metaKey) {
          const set = new Set(S.sel?.units ?? []);
          set.has(u.id) ? set.delete(u.id) : set.add(u.id);
          select([...set]);
        } else {
          selectGroup(u.group);
        }
      },
    },
    h(
      "span.unit-name",
      u.transport !== null && u.transport !== undefined ? h("span.dim", "↳ ") : null,
      u.unit_combat !== undefined && u.unit_combat !== null ? typeIcon("unitcombat", u.unit_combat, 8, { title: dname("UnitCombatType", u.unit_combat) }) : null,
      u.name ?? dname("UnitType", u.type),
      u.promo_ready ? h("span.badge.promo", "promote") : null,
      automationBadge(u),
      u.summon_turns ? h("span.badge", { title: "A summoned unit: turns left" }, `${u.summon_turns}t`) : null,
      u.cargo?.length ? h("span.badge", { title: "Units on board" }, `${u.cargo.length} aboard`) : null,
    ),
    look.art
      ? h(
          "span.unit-stats",
          u.strength !== undefined ? h("span.stat-n", pix(String(u.strength)), icon8("strength", "⚔", "Strength")) : null,
          barEl(u.hp / Math.max(1, u.max_hp), u.hp * 4 < u.max_hp ? "hp_low" : "hp", { w: 14, title: `${Math.round((100 * u.hp) / u.max_hp)}% health` }),
          u.moves !== undefined ? h("span.stat-n", pix(`${moves(u.moves)}/${moves(u.max_moves)}`), icon8("moves", "➜", "Moves")) : null,
          u.level !== undefined ? h("span.stat-n", { title: `Level ${u.level}` }, pix(`L${u.level}`)) : null,
          busy ? h("span.dim", ` ${humanize(tname("ActivityType", u.activity))}`) : null,
        )
      : h(
          "span.unit-stats",
          `${u.strength ?? ""}${u.strength !== undefined ? "⚔ " : ""}`,
          `${Math.round((100 * u.hp) / u.max_hp)}% `,
          u.moves !== undefined ? `${moves(u.moves)}/${moves(u.max_moves)}➜ ` : "",
          u.level !== undefined ? `L${u.level}` : "",
          busy ? h("span.dim", ` ${humanize(tname("ActivityType", u.activity))}`) : null,
        ),
  );
  return row;
}

/** The side panel for the selection. */
export function renderPanel() {
  if (!panel) return;
  if (!S.sel) {
    if (!panel.dataset.plot) fill(panel, h("p.dim", "Select a unit or a city. Right-click moves the selected units."));
    return;
  }
  delete panel.dataset.plot;
  const { x, y } = S.sel;
  const own = ownUnitsAt(x, y);
  // Only a reply for the selected units: the previous selection's actions are not drawn under
  // the new selection's header while its reply is on its way.
  const r = reply();
  const details = new Map((r?.units ?? []).map((u) => [u.id, u]));
  const foreign = unitsAt(x, y).filter((u) => u.owner !== me()?.player);
  const g = currentGroup();
  const grp = g !== null ? S.groups.get(g) : null;
  const lead = S.units.get(S.sel.units[0]) ?? own[0];
  const head = h(
    "div.panel-head",
    lead ? unitIcon(lead.type, ownerLook(lead.owner), { title: dname("UnitType", lead.type) }) : null,
    h("h3", own.length === 1 ? dname("UnitType", own[0].type) : `${S.sel.units.length} of ${own.length} units`),
    h("span.dim", ` (${x}, ${y})`),
  );
  // Transports with their cargo under them (the units in a transport are listed after it).
  const ids = new Set(own.map((u) => u.id));
  const ordered = [];
  for (const u of own.filter((u) => u.transport === null || u.transport === undefined || !ids.has(u.transport))) {
    ordered.push(u);
    for (const c of own.filter((c) => c.transport === u.id)) ordered.push(c);
  }
  const list = h("div.unit-list", ordered.map((u) => unitRow({ ...u, ...(details.get(u.id) ?? {}) }, S.sel.units.includes(u.id))));
  const tools = h(
    "div.row",
    h("button", { onclick: () => select(own.map((u) => u.id)), disabled: own.length < 2, title: "Select every unit here" }, "All"),
    h("button", { onclick: groupAll, disabled: own.length < 2, title: "Join the units here into one group" }, "Group"),
    h("button", { onclick: splitAll, disabled: S.sel.units.length < 2, title: "Each selected unit its own group" }, "Split"),
    h("button", { onclick: () => selectNext(), title: "Next unit waiting for orders [W]" }, "Next ⟳"),
  );
  if (r?.actions?.some((a) => a.kind === "promotion" && a.enabled)) {
    tools.append(h("button.promote", { onclick: () => openPromotions(), title: "Choose a promotion" }, "Promote…"));
  }
  const city = cityAt(x, y);
  if (city && city.owner === me()?.player) {
    tools.append(h("button", { onclick: () => emit("open-city", city.id), title: "Open the city (or double-click it)" }, `${city.name} ▸`));
  }
  const status = [];
  if (grp?.missions?.length) status.push(h("div.dim", `Orders: ${grp.missions.map((m) => dname("MissionType", m.m)).join(", ")}`));
  if (grp && !grp.ready) status.push(h("div.dim", grp.busy ? "Busy" : "Done for this turn"));
  const actions = [];
  if (r?.error) actions.push(h("p.warn", r.error));
  else if (!r?.actions) actions.push(h("p.dim", "…"));
  else {
    for (const [title, test] of SECTIONS) {
      const list = K.sortActions(r.actions.filter(test), S.data);
      if (!list.length) continue;
      const more = title === "Spells" ? h("button.link.small", { onclick: () => openSpells(), title: "Every spell with its description" }, "details") : null;
      actions.push(h("div.section", h("h4", title, more ? " " : null, more), h("div.actions", list.map(actionButton))));
    }
  }
  const others = foreign.length ? h("div.section", h("h4", "Others here"), foreign.map((u) => h("div.dim", `${dname("UnitType", u.type)} (${playerName(u.owner)})`))) : null;
  const hint = mode ? h("p.mode-hint", isGoTo(mode.action) ? "Click a plot to go there (Shift: add waypoints; Esc cancels)." : `Pick a plot: ${actionLabel(mode.action)} (Esc cancels).`) : null;
  const way = waypoints && unitsKey(waypoints.units) === unitsKey(S.sel.units)
    ? h("p.mode-hint", `${waypoints.points.length === 1 ? "1 waypoint" : `${waypoints.points.length} waypoints`}: release Shift to send (Esc drops them).`)
    : null;
  // The lead unit's promotions as their seals (docs/ART.md 11.5), with the level as pips.
  const leadDetails = details.get(S.sel.units[0]);
  const seals = look.art && leadDetails?.promotions?.length
    ? h("div.seals", { title: leadDetails.promotions.map((p) => dname("PromotionType", p)).join(", ") }, leadDetails.promotions.map((p) => promotionIcon(p, { u: 1, title: dname("PromotionType", p) })))
    : null;
  fill(panel, head, hint, way, list, seals, tools, status, actions, others);
}

/** The promotions the selection can take, with their descriptions: a click promotes (every
 *  selected unit that can, as `CvNetDoCommand` with COMMAND_PROMOTION). */
export function openPromotions() {
  const r = reply();
  const list = K.sortActions((r?.actions ?? []).filter((a) => a.kind === "promotion"), S.data);
  const m = openModal({
    title: "Promotions",
    tag: "promotions",
    wide: true,
    body: [
      list.length
        ? h(
            "div.options",
            list.map((a) =>
              h(
                "button.option",
                { disabled: !a.enabled, "data-promotion": a.promotion, onclick: async () => (await doAction(a), m.close("promoted")) },
                h("span", dname("PromotionType", a.promotion)),
                info("PromotionType", a.promotion)?.help ? h("small", markup(info("PromotionType", a.promotion).help)) : null,
              ),
            ),
          )
        : h("p.dim", "No promotion now."),
    ],
    buttons: [{ label: "Close", value: null }],
  });
  return m;
}

/** The selection's spells with their help (the spell's XML help, as the button's tooltip has
 *  it): castable ones cast on a click. */
export function openSpells() {
  const r = reply();
  const list = K.sortActions((r?.actions ?? []).filter((a) => a.kind === "spell"), S.data);
  const m = openModal({
    title: "Spells",
    tag: "spells",
    wide: true,
    body: [
      h(
        "div.options",
        list.map((a) => {
          const e = info("SpellType", a.spell);
          const notes = [a.global ? "world spell (once per game)" : null, a.causes_war ? "may declare war" : null, e?.range ? `range ${e.range}` : null].filter(Boolean).join(", ");
          return h(
            "button.option",
            { disabled: !a.enabled, "data-spell": a.spell, onclick: async () => (m.close("cast"), await doAction(a)) },
            h("span", dname("SpellType", a.spell), notes ? h("span.dim", ` (${notes})`) : null, a.enabled ? null : h("span.dim", " — not now")),
            a.help || e?.help ? h("small", markup(a.help || e.help)) : null,
          );
        }),
      ),
    ],
    buttons: [{ label: "Close", value: null }],
  });
  return m;
}

/** The side panel for a plot without units of ours. */
async function renderPlotInfo(x, y) {
  panel.dataset.plot = `${x},${y}`;
  fill(panel, h("div.panel-head", h("h3", `Plot (${x}, ${y})`)), h("p.dim", "…"));
  let p;
  try {
    p = await api.query("plot", { x, y });
  } catch (e) {
    return fill(panel, h("p.warn", e.detail ?? e.message));
  }
  if (panel.dataset.plot !== `${x},${y}`) return;
  if (p.vis === 0) return fill(panel, h("div.panel-head", h("h3", `Plot (${x}, ${y})`)), h("p.dim", "Unexplored."));
  const rows = [
    ["Terrain", [dname("TerrainType", p.terrain), p.hills ? "Hills" : null, p.feature !== null ? dname("FeatureType", p.feature) : null].filter(Boolean).join(", ")],
    ["Bonus", p.bonus !== null ? dname("BonusType", p.bonus) : null],
    ["Improvement", p.improvement !== null ? dname("ImprovementType", p.improvement) : null],
    ["Route", p.route !== null ? dname("RouteType", p.route) : null],
    ["Owner", p.owner !== null ? playerName(p.owner) : null],
    ["City", p.city ? `${p.city.name} (${playerName(p.city.owner)})` : null],
    ["Yields", p.yields ? `${p.yields[0]} / ${p.yields[1]} / ${p.yields[2]}` : null],
    ["Defence", p.defense !== undefined ? `${p.defense}%` : null],
    ["Move cost", p.move_cost],
  ].filter(([, v]) => v !== null && v !== undefined);
  const units = (p.units ?? []).map((u) => h("div", `${dname("UnitType", u.type)} (${playerName(u.owner)}) ⚔${u.strength} ${Math.round((100 * u.hp) / u.max_hp)}%`));
  fill(
    panel,
    h("div.panel-head", h("h3", `Plot (${x}, ${y})`), p.vis === 1 ? h("span.dim", " fogged") : null),
    h("table.kv", rows.map(([k, v]) => h("tr", h("th", k), h("td", String(v))))),
    units.length ? h("div.section", h("h4", "Units"), units) : null,
  );
}
