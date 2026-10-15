// Units (design §5.1 "Units"): the selection, the stack list, the actions panel from the `group`
// query, moving and attacking with the path and combat previews, interface modes (go-to,
// route-to, range attack: pick a plot), grouping, and cycling over the ready groups.

import * as api from "./api.js";
import { S, emit, on, dname, tname, tidx, me, unitsAt, ownUnitsAt, cityAt, playerName, readyGroups, nextReadyGroup, moves, turns, info } from "./state.js";
import { h, fill, toast } from "./dom.js";
import { run, actionCommand } from "./commands.js";
import { plotDistance, plotIndex, humanize } from "./tiles.js";
import { render as markup } from "./markup.js";

let mapView = null;
let panel = null;
/** The last `group` query reply for the selection, and the key it answers. */
let group = null;
let groupKey = "";
/** An interface mode: `{kind: "move"}` or `{kind: "target", action}`. */
let mode = null;
/** After a command on the selection, move on to the next ready group once it is done. */
let advance = false;
/** Hover previews: the plot, and the replies. */
let hover = null;
let hoverTimer = null;
let preview = { path: null, combat: null, plot: null };

export function initUnits(view, panelEl) {
  mapView = view;
  panel = panelEl;
  on("applied", (c) => {
    if (c.units && S.sel && c.units.some((u) => S.sel.units.includes(u))) advance = true;
  });
}

// ---------------------------------------------------------------------------------------------
// Selection

/** Selects own units `ids` (on one plot), or nothing. */
export function select(ids, { center = false } = {}) {
  const units = (ids ?? []).map((id) => S.units.get(id)).filter((u) => u && u.owner === me()?.player);
  if (units.length === 0) {
    S.sel = null;
  } else {
    const { x, y } = units[0];
    S.sel = { units: units.filter((u) => u.x === x && u.y === y).map((u) => u.id), x, y };
    if (center) mapView.reveal(x, y);
  }
  setMode(null);
  advance = false;
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

/** Selects the next ready group (Civ4's cycle: the nearest to the last selection). */
export function selectNext({ skipCurrent = false } = {}) {
  const cur = currentGroup();
  const near = S.sel ? { x: S.sel.x, y: S.sel.y } : null;
  const g = nextReadyGroup(skipCurrent ? cur : -1, near, near && S.map ? (ax, ay, bx, by) => plotDistance(S.map, ax, ay, bx, by) : null, skipCurrent && cur !== null ? [cur] : []);
  if (g !== null && g !== undefined) {
    selectGroup(g, { center: true });
    return true;
  }
  return false;
}

/** The group the selection is (its first unit's). */
function currentGroup() {
  if (!S.sel) return null;
  return S.units.get(S.sel.units[0])?.group ?? null;
}

/** Called when a new view arrived: keeps the selection on the units that still exist (they may
 *  have moved), or moves on to the next ready group after a command finished with it. */
export function viewChanged() {
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
    advance = false;
    const g = currentGroup();
    if (!S.sel || g === null || !readyGroups().includes(g)) selectNext({ skipCurrent: true }) || (S.sel && refreshGroup());
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
  if (S.sel) moveTo(x, y, ev?.shiftKey ?? false);
}

export async function moveTo(x, y, append = false) {
  if (!S.sel || (S.sel.x === x && S.sel.y === y && !append)) return;
  await run({ c: "move", units: S.sel.units, x, y, append });
}

/** Sends the action (a `group` query action); a `target` action first asks for a plot. */
export async function doAction(a) {
  if (!a.enabled) return;
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

/** Runs the first enabled action of a kind the hotkey names (`MISSION_FORTIFY`, ...). */
export async function hotkey(type) {
  const a = findAction(type);
  if (a) await doAction(a);
  return !!a;
}

function findAction(type) {
  if (!group?.actions) return null;
  const m = tidx("MissionType", type);
  const c = tidx("CommandType", type);
  return group.actions.find((a) => a.enabled && ((m >= 0 && a.mission === m && a.kind === "mission") || (c >= 0 && a.command === c))) ?? null;
}

/** Enters an interface mode: `{kind: "move"}` (move to the plot clicked) or `{kind: "target",
 *  action}`; null leaves it. */
export function setMode(m) {
  mode = m;
  const targets = m?.kind === "target" ? (m.action.any ? null : m.action.targets) : null;
  mapView?.setOverlay({ targets: targets ?? (m ? [] : null) });
  document.body.classList.toggle("picking", !!m);
  emit("mode", m);
}

export function currentMode() {
  return mode;
}

async function pickPlot(x, y, ev) {
  const m = mode;
  setMode(null);
  if (!S.sel) return;
  if (m.kind === "move") {
    await moveTo(x, y, ev?.shiftKey ?? false);
    return;
  }
  const a = m.action;
  if (!a.any && !(a.targets ?? []).some(([tx, ty]) => tx === x && ty === y)) {
    toast("Not a valid target.", "error");
    return;
  }
  await run(actionCommand(a, x, y));
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
  mapView?.setOverlay({ path: null });
}

/** The mouse is over (x, y) (null: left the map). */
export function hoverAt(x, y) {
  hover = x === null ? null : { x, y };
  clearTimeout(hoverTimer);
  preview = { path: null, combat: null, plot: null };
  mapView.setOverlay({ path: null });
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
      jobs.push(api.query("combat", { units: sel.units, x, y }).then((r) => still() && (preview.combat = r)).catch(() => {}));
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
    lines.push(h("div", `Yields ${p.yields[0]} food, ${p.yields[1]} production, ${p.yields[2]} commerce · defence ${p.defense}% · move cost ${p.move_cost}`));
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
  box.append(h("div.odds", c.odds === null ? "Air strike" : `Odds ${pct(c.odds)}`, c.withdraw ? h("span.dim", ` · retreat ${pct(c.withdraw)}`) : null));
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

/** Re-reads the selection's actions (`group` query) and redraws the panel. */
export async function refreshGroup() {
  if (!S.sel) {
    group = null;
    groupKey = "";
    renderPanel();
    return;
  }
  const key = `${S.sel.units.join(",")}@${S.view?.view_version}:${S.view?.turn}`;
  if (key === groupKey && group) return renderPanel();
  groupKey = key;
  renderPanel();
  try {
    const r = await api.query("group", { units: S.sel.units });
    if (key !== groupKey) return;
    group = r;
  } catch (e) {
    if (key !== groupKey) return;
    group = { error: e.detail ?? e.message };
  }
  renderPanel();
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
      return dname("MissionType", a.mission);
    }
    case "target":
      return humanize(a.mode.replace(/^INTERFACEMODE_/, "MODE_"));
    case "command":
      return dname("CommandType", a.command);
    case "load":
      return "Load";
    case "spell":
      return dname("SpellType", a.spell);
    case "promotion":
      return dname("PromotionType", a.promotion);
    case "upgrade":
      return `Upgrade: ${dname("UnitType", a.unit)} (${a.price} gold)`;
  }
  return a.kind;
}

const HOTKEYS = {
  MISSION_FORTIFY: "F",
  MISSION_SLEEP: "S",
  MISSION_SKIP: "Space",
  MISSION_FOUND: "B",
  MISSION_HEAL: "H",
  MISSION_SENTRY: "",
  INTERFACEMODE_GO_TO: "M",
  COMMAND_DELETE: "Del",
};

function hotkeyOf(a) {
  if (a.kind === "mission") return HOTKEYS[tname("MissionType", a.mission)] ?? "";
  if (a.kind === "target") return HOTKEYS[a.mode] ?? "";
  if (a.kind === "command") return HOTKEYS[tname("CommandType", a.command)] ?? "";
  return "";
}

const SECTIONS = [
  ["Orders", (a) => (a.kind === "mission" && tname("MissionType", a.mission) !== "MISSION_BUILD") || a.kind === "target" || a.kind === "command" || a.kind === "load"],
  ["Build", (a) => a.kind === "mission" && tname("MissionType", a.mission) === "MISSION_BUILD"],
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
      "data-type": a.kind === "mission" ? tname("MissionType", a.mission) : a.kind === "target" ? a.mode : a.kind === "command" ? tname("CommandType", a.command) : a.kind === "spell" ? tname("SpellType", a.spell) : "",
      title: [key ? `[${key}]` : "", help ? markup(help).textContent : ""].filter(Boolean).join(" "),
      onclick: () => doAction(a),
    },
    actionLabel(a) + extra,
    key ? h("kbd", key) : null,
  );
  return btn;
}

function unitRow(u, selected) {
  const busy = u.activity !== null && u.activity !== undefined && tname("ActivityType", u.activity) !== "ACTIVITY_AWAKE";
  const row = h(
    "div.unit-row",
    {
      "data-unit": u.id,
      class: selected ? "selected" : "",
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
    h("span.unit-name", dname("UnitType", u.type), u.promo_ready ? h("span.badge.promo", "promote") : null),
    h(
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
  const details = new Map((group?.units ?? []).map((u) => [u.id, u]));
  const foreign = unitsAt(x, y).filter((u) => u.owner !== me()?.player);
  const g = currentGroup();
  const grp = g !== null ? S.groups.get(g) : null;
  const head = h(
    "div.panel-head",
    h("h3", own.length === 1 ? dname("UnitType", own[0].type) : `${S.sel.units.length} of ${own.length} units`),
    h("span.dim", ` (${x}, ${y})`),
  );
  const list = h("div.unit-list", own.map((u) => unitRow({ ...u, ...(details.get(u.id) ?? {}) }, S.sel.units.includes(u.id))));
  const tools = h(
    "div.row",
    h("button", { onclick: () => select(own.map((u) => u.id)), disabled: own.length < 2, title: "Select every unit here" }, "All"),
    h("button", { onclick: groupAll, disabled: own.length < 2, title: "Join the units here into one group" }, "Group"),
    h("button", { onclick: splitAll, disabled: S.sel.units.length < 2, title: "Each selected unit its own group" }, "Split"),
    h("button", { onclick: () => selectNext({ skipCurrent: true }), title: "Next unit waiting for orders [W]" }, "Next ⟳"),
  );
  const city = cityAt(x, y);
  if (city && city.owner === me()?.player) {
    tools.append(h("button", { onclick: () => emit("open-city", city.id), title: "Open the city (or double-click it)" }, `${city.name} ▸`));
  }
  const status = [];
  if (grp?.missions?.length) status.push(h("div.dim", `Orders: ${grp.missions.map((m) => dname("MissionType", m.m)).join(", ")}`));
  if (grp && !grp.ready) status.push(h("div.dim", grp.busy ? "Busy" : "Done for this turn"));
  const actions = [];
  if (group?.error) actions.push(h("p.warn", group.error));
  else if (!group?.actions) actions.push(h("p.dim", "…"));
  else {
    for (const [title, test] of SECTIONS) {
      const list = group.actions.filter(test);
      if (list.length) actions.push(h("div.section", h("h4", title), h("div.actions", list.map(actionButton))));
    }
  }
  const others = foreign.length ? h("div.section", h("h4", "Others here"), foreign.map((u) => h("div.dim", `${dname("UnitType", u.type)} (${playerName(u.owner)})`))) : null;
  const hint = mode ? h("p.mode-hint", mode.kind === "move" ? "Click a plot to move there (Esc cancels)." : `Pick a plot: ${actionLabel(mode.action)} (Esc cancels).`) : null;
  fill(panel, head, hint, list, tools, status, actions, others);
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
