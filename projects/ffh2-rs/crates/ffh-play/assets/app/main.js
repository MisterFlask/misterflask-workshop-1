// The browser client (design §5.1 MVP, §7 P5): boots, follows the server's status (long-poll),
// refreshes the view and the map when they change, and runs what the phase asks for: the map
// and the deferred popups on the player's turn, the blocking decision, the progress line during
// the AI turns, and the new-game, game-over, crash and stuck screens.
//
// The server computes everything (legal actions, odds, turns, fog); the client renders it and
// sends commands (commands.js). `window.ffh` exposes the state for the headless smoke test.

import * as api from "./api.js";
import { S, setData, setView, setMap, on, emit, resetDismissed, clearDismissed, popupQueue } from "./state.js";
import { MapView } from "./map.js";
import * as units from "./units.js";
import { openCity } from "./city.js";
import * as screens from "./screens.js";
import { showDecision, modalOpen, closeTag, closeAll, onEscape, openTags, openModal } from "./modals.js";
import { renderTop, renderBottom, setTab, currentTab } from "./hud.js";
import { endTurn, sending } from "./commands.js";
import { h, $, toast } from "./dom.js";
import { dname, turns, me } from "./state.js";

let mapView = null;
let syncing = false;
let again = false;
/** The game the view belongs to (journal or directory), and the view's key. */
let gameKey = null;
let viewKey = null;
/** The turn of the last control point (a new one starts the turn's report and popups). */
let lastTurn = null;
let popupsRunning = false;
let overShown = null;
let stuckShown = null;
let connected = true;

// ---------------------------------------------------------------------------------------------
// Status and refresh

function onStatus(st) {
  if (!connected) {
    connected = true;
    toast("Connected.");
  }
  S.status = st;
  renderTop();
  sync();
}

function onDisconnect(e) {
  if (connected) {
    connected = false;
    toast(`Lost the server (${e.message ?? e}); retrying…`, "error");
  }
  const t = document.getElementById("phase-text");
  if (t) t.textContent = "disconnected; retrying…";
}

/** Reads the status at once (after a command: the long-poll may not have returned yet). */
async function refreshStatus() {
  try {
    const st = await api.status();
    if (st && (!S.status || st.version > (S.status.version ?? -1))) onStatus(st);
  } catch {
    /* the long-poll reports it */
  }
}

async function sync() {
  if (syncing) {
    again = true;
    return;
  }
  syncing = true;
  try {
    do {
      again = false;
      await syncOnce();
    } while (again);
  } catch (e) {
    console.warn("sync:", e);
  } finally {
    syncing = false;
  }
}

async function syncOnce() {
  const st = S.status;
  if (!st) return;
  const phase = st.phase;
  if (phase !== "crashed") closeTag("crash");
  if (phase !== "stuck") closeTag("stuck");
  if (phase !== "decision") showDecision(null);
  // The New game dialog the no-game screen forced (it cannot be closed) goes once there is a
  // game, also one started by another client or a restarted server (P5 review); so does the
  // game-over screen once the game is no longer over.
  if (phase !== "no_game") closeTag(screens.NEWGAME_REQUIRED);
  if (phase !== "over") closeTag("over");
  if (phase === "no_game" || phase === "crashed") {
    clearGame();
    if (phase === "crashed") {
      closeAll(["crash", "load", "newgame"]);
      if (!openTags().includes("crash")) screens.crashScreen(st);
    } else if (!openTags().some((t) => t === "newgame" || t === screens.NEWGAME_REQUIRED || t === "load")) {
      screens.newGame({ dismissible: false });
    }
    return;
  }
  if (phase === "running") return;
  await refreshView(st);
  if (S.status !== st && S.status.phase !== phase) return; // a newer status is being handled
  if (phase === "decision" && st.decision) showDecision(st.decision, (option) => api.answer(st.decision.id, option));
  if (phase === "over") {
    const key = `${gameKey}|${st.turn}`;
    if (overShown !== key) {
      overShown = key;
      closeAll();
      screens.gameOver(st);
    }
  }
  if (phase === "stuck") {
    const key = `${gameKey}|${st.turn}`;
    if (stuckShown !== key) {
      stuckShown = key;
      screens.stuckScreen(st);
    }
  }
  if (phase === "human") await onControl(st);
}

function clearGame() {
  if (!S.view && !S.map) return;
  setView(null);
  setMap(null);
  S.sel = null;
  S.messages = [];
  S.logNext = 0;
  S.report = null;
  gameKey = null;
  viewKey = null;
  lastTurn = null;
  clearDismissed();
  units.clearWaypoints();
  mapView.map = null;
  mapView.chunks.clear();
  mapView.request();
  renderTop();
  renderBottom();
  units.renderPanel();
}

/** Fetches the view (and the map when its revision changed) when the status says it changed. */
async function refreshView(st) {
  const gk = st.game?.journal ?? st.game?.dir ?? "memory";
  const vk = `${gk}|${st.view_version}|${st.turn}|${st.reveal}|${st.phase}`;
  if (vk === viewKey && gk === gameKey) return;
  const view = await api.get("/api/view");
  if (!view) return;
  const newGame = gk !== gameKey;
  if (newGame) {
    S.messages = [];
    S.logNext = 0;
    S.report = null;
    S.sel = null;
    lastTurn = null;
    // Another game (new, loaded, started elsewhere): its deferred popups are asked again,
    // also at the turn the last game's were set aside (P5 review); waypoints not sent yet
    // were the last game's.
    clearDismissed();
    units.clearWaypoints();
  }
  setView(view);
  if (newGame || !S.map || st.map_rev !== S.map.map_rev) {
    const map = await api.get("/api/map");
    if (map) {
      setMap(map);
      mapView.setMap(map);
      if (newGame) centerOnStart();
    }
  }
  gameKey = gk;
  viewKey = vk;
  await refreshLog();
  units.viewChanged();
  renderTop();
  renderBottom();
  mapView.request();
}

async function refreshLog() {
  const next = S.view?.log_next;
  if (next === null || next === undefined) return;
  if (next < S.logNext) {
    S.messages = [];
    S.logNext = 0;
  }
  if (next === S.logNext) return;
  try {
    const r = await api.query("log", { since: S.logNext });
    S.messages.push(...r.entries);
    if (S.messages.length > 2000) S.messages.splice(0, S.messages.length - 2000);
    S.logNext = r.next;
  } catch {
    /* busy inside a decision */
  }
}

function centerOnStart() {
  const m = me();
  const cap = [...S.cities.values()].find((c) => c.owner === m?.player && c.capital);
  const u = S.view?.units.find((x) => x.owner === m?.player);
  if (cap) mapView.centerOn(cap.x, cap.y);
  else if (u) mapView.centerOn(u.x, u.y);
  else if (S.map) mapView.centerOn(S.map.w / 2, S.map.h / 2);
}

/** The player has control: at a new turn, the turn report and the first ready group; then the
 *  deferred popups. */
async function onControl(st) {
  if (st.turn !== lastTurn) {
    lastTurn = st.turn;
    resetDismissed(st.turn);
    try {
      S.report = await api.query("report");
    } catch {
      S.report = null;
    }
    if (S.report && ((S.report.lines?.length ?? 0) > 0 || (S.report.messages?.length ?? 0) > 0)) setTab("report");
    else renderBottom();
    // Civ4 starts a turn on the first unit waiting for orders.
    const g = S.sel ? S.units.get(S.sel.units[0])?.group : null;
    if (!S.sel || !S.view.pending.ready_groups.includes(g)) units.selectNext();
  }
  runPopups();
}

/** Shows the deferred popups one at a time, in C++ order (events, votes, research, idle
 *  cities); "Later" sets one aside for the turn. */
async function runPopups() {
  if (popupsRunning) return;
  popupsRunning = true;
  try {
    for (;;) {
      if (S.status?.phase !== "human" || modalOpen() || sending()) break;
      const q = popupQueue(S.view?.pending, S.dismissed, S.status.turn);
      if (!q.length) break;
      const p = q[0];
      // Asked once per turn: answered, or set aside until the end-turn gate reopens it.
      S.dismissed.add(p.key);
      if (p.kind === "event") await screens.openEvent(p.event);
      else if (p.kind === "vote") await screens.openVote(p.vote);
      else if (p.kind === "research") await screens.openResearch({ popup: true });
      else {
        mapView.reveal(S.cities.get(p.city)?.x ?? 0, S.cities.get(p.city)?.y ?? 0);
        await openCity(p.city, { popup: true });
      }
      await new Promise((r) => setTimeout(r, 30));
    }
  } finally {
    popupsRunning = false;
  }
  if (!S.sel && S.status?.phase === "human" && !modalOpen()) units.selectNext();
}

// ---------------------------------------------------------------------------------------------
// Cities list

function citiesList() {
  const mine = [...S.cities.values()].filter((c) => c.owner === me()?.player);
  return openModal({
    title: "Your cities",
    tag: "cities",
    wide: true,
    body: [
      mine.length
        ? h(
            "table.scores",
            h("tr", h("th", "City"), h("th", "Size"), h("th", "Producing"), h("th", "Growth")),
            mine.map((c) =>
              h(
                "tr.clickable",
                { onclick: () => (closeTag("cities"), mapView.centerOn(c.x, c.y), openCity(c.id)) },
                h("td", `${c.capital ? "★ " : ""}${c.name}`),
                h("td", String(c.size)),
                h("td", c.production ? `${dname(["UnitType", "BuildingType", "ProjectType", "ProcessType"][c.production.order], c.production.type)} (${turns(c.production.turns)})` : h("span.warn", "nothing")),
                h("td", c.food_turns > 0 ? turns(c.food_turns) : "-"),
              ),
            ),
          )
        : h("p.dim", "No cities yet."),
    ],
    buttons: [{ label: "Close", value: null }],
  });
}

// ---------------------------------------------------------------------------------------------
// Keys

function onKey(ev) {
  const t = ev.target;
  const typing = t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement;
  if (ev.key === "Escape") {
    if (units.currentMode()) units.setMode(null);
    else if (units.clearWaypoints()) return;
    else if (!onEscape() && !modalOpen()) units.clearSelection();
    return;
  }
  if (typing || modalOpen() || ev.altKey || ev.ctrlKey || ev.metaKey) return;
  const human = S.status?.phase === "human";
  const k = ev.key;
  const handled = () => ev.preventDefault();
  switch (k) {
    case "Enter":
      if (human) endTurn(ev.shiftKey);
      return handled();
    case " ":
      units.hotkey("MISSION_SKIP");
      return handled();
    case "f":
    case "F":
      units.hotkey("MISSION_FORTIFY").then((ok) => ok || units.hotkey("MISSION_SLEEP"));
      return handled();
    case "s":
    case "S":
      units.hotkey("MISSION_SLEEP");
      return handled();
    case "h":
    case "H":
      units.hotkey("MISSION_HEAL");
      return handled();
    case "b":
    case "B":
      units.hotkey("MISSION_FOUND");
      return handled();
    case "Delete":
      units.hotkey("COMMAND_DELETE");
      return handled();
    case "m":
    case "M":
      if (S.sel) units.setMode({ kind: "move" });
      return handled();
    case "w":
    case "W":
    case "Tab":
      units.selectNext({ cycle: true });
      return handled();
    case "c":
    case "C":
      if (S.sel) mapView.centerOn(S.sel.x, S.sel.y);
      return handled();
    case "r":
    case "R":
      if (S.view) screens.openResearch();
      return handled();
    case "?":
      screens.keysHelp();
      return handled();
    case "ArrowLeft":
      mapView.pan(-2, 0);
      return handled();
    case "ArrowRight":
      mapView.pan(2, 0);
      return handled();
    case "ArrowUp":
      mapView.pan(0, 2);
      return handled();
    case "ArrowDown":
      mapView.pan(0, -2);
      return handled();
    case "+":
    case "=":
      mapView.zoom(1);
      return handled();
    case "-":
      mapView.zoom(-1);
      return handled();
  }
}

// ---------------------------------------------------------------------------------------------
// Boot

function wire() {
  const click = (id, f) => document.getElementById(id)?.addEventListener("click", f);
  click("end-turn", (ev) => endTurn(ev.shiftKey));
  click("btn-research", () => S.view && screens.openResearch());
  click("btn-cities", () => S.view && citiesList());
  click("btn-save", () => S.view && screens.saveDialog());
  click("btn-load", () => screens.loadScreen());
  click("btn-new", () => screens.newGame());
  click("btn-keys", () => screens.keysHelp());
  click("btn-raw", () => screens.rawRequest());
  click("zoom-in", () => mapView.zoom(1));
  click("zoom-out", () => mapView.zoom(-1));
  for (const b of document.querySelectorAll("#bottom-tabs button")) b.addEventListener("click", () => setTab(b.dataset.tab));
  click("bottom-toggle", () => document.body.classList.toggle("bottom-closed"));
  document.addEventListener("ffh-research", () => S.view && screens.openResearch());
  document.addEventListener("keydown", onKey);
  // Shift-right-click waypoints are sent when Shift is released (or the page loses the keys).
  document.addEventListener("keyup", (ev) => ev.key === "Shift" && units.flushWaypoints());
  window.addEventListener("blur", () => units.flushWaypoints());
  on("applied", () => refreshStatus());
  on("refused", () => refreshStatus());
  on("kick", () => refreshStatus());
  on("game-started", () => {
    closeAll();
    units.clearWaypoints();
    S.sel = null;
    viewKey = null;
    clearDismissed();
    refreshStatus();
  });
  on("reopen-popups", () => {
    S.dismissed = new Set();
    runPopups();
  });
  on("select-ready", () => units.selectNext());
  on("modals-closed", () => S.status?.phase === "human" && runPopups());
  on("open-city", (id) => openCity(id));
  on("center", (x, y) => mapView.centerOn(x, y));
  on("selection", () => units.renderPanel());
  on("mode", () => units.renderPanel());
}

async function boot() {
  wire();
  mapView = new MapView($("#mapwrap"), {
    click: (x, y, ev) => units.click(x, y, ev),
    rightClick: (x, y, ev) => units.rightClick(x, y, ev),
    hover: (x, y) => units.hoverAt(x, y),
    dblclick: (x, y) => {
      const c = [...S.cities.values()].find((q) => q.x === x && q.y === y && q.owner === me()?.player);
      if (c) openCity(c.id);
    },
  });
  units.initUnits(mapView, $("#unit-panel"));
  renderTop();
  renderBottom();
  try {
    const [data, setup] = await Promise.all([api.get("/api/data"), api.get("/api/setup")]);
    setData(data);
    S.setup = setup;
  } catch (e) {
    toast(`Cannot load the game data: ${e.message ?? e}`, "error", 60000);
    return;
  }
  api.watch(onStatus, onDisconnect);
  window.ffh.ready = true;
}

window.ffh = { S, get mapView() { return mapView; }, units, screens, api, emit, ready: false, tab: currentTab };
boot();
