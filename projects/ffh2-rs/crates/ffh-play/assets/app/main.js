// The browser client (design §5.1, §7 P5-P6): boots, follows the server's status (long-poll),
// refreshes the view and the map when they change (the map by tile deltas), and runs what the
// phase asks for: the map and the deferred popups on the player's turn, the blocking decision,
// the progress line during the AI turns, and the new-game, game-over, crash and stuck screens;
// the keys (Civ4's controls here, the selection's action hotkeys in units.js) and the menu's
// screens (city, techs, civics, religion, diplomacy, finances, tools).
//
// The server computes everything (legal actions, odds, turns, fog); the client renders it and
// sends commands (commands.js). The look (look.js) is the original pixel art the server embeds
// (docs/ART.md): the map's art renderer and the interface's bitmap type, frames and icons, loaded
// beside the game data; until it has loaded (or when it cannot, or with `?renderer=flat` for the
// map) the client draws as before. `window.ffh` exposes the state for the headless smoke test.

import * as api from "./api.js";
import { S, setData, setView, setMap, on, emit, resetDismissed, clearDismissed, popupQueue, endTurnMessage, cycleCity } from "./state.js";
import { MapView } from "./map.js";
import * as units from "./units.js";
import { openCity, ownCities, stepCityScreen } from "./city.js";
import * as empire from "./empire.js";
import { openDiplomacy } from "./diplo.js";
import * as tools from "./tools.js";
import { applyMapDelta, plotDistance } from "./tiles.js";
import * as K from "./hotkeys.js";
import * as screens from "./screens.js";
import { showDecision, modalOpen, closeTag, closeAll, onEscape, openTags, openModal } from "./modals.js";
import { renderTop, renderBottom, setTab, currentTab } from "./hud.js";
import { endTurn, sending } from "./commands.js";
import { h, $, toast } from "./dom.js";
import { dname, turns, me } from "./state.js";
import { look, loadLook, setLookData, rendererChoice } from "./look.js";

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
  mapView.art?.setMap(mapView.map, null, "all");
  mapView.map = null;
  mapView.chunks.clear();
  mapView.request();
  renderTop();
  renderBottom();
  units.renderPanel();
}

/** The dialogs that belong to one game (closed when another game arrives); the load, options,
 *  tools and crash dialogs stay. */
const GAME_DIALOGS = ["city", "decision", "election", "event", "research", "vote", "promotions", "spells", "trade", "diplomacy", "cities", "civics", "finance", "religion", "techtree", "inspect", "gate", "over", "stuck", "takeover", "save"];

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
    // The last game's dialogs are about its cities, units and questions, which this game does
    // not have: close them. A deferred popup still open (the city just founded asking what to
    // build, when a game is loaded over it) asked the server about a city that is gone ("no
    // such object") and, open, held back the new game's own popups. The popup loop then
    // carries on with this game's queue.
    for (const tag of GAME_DIALOGS) closeTag(tag);
  }
  setView(view);
  if (newGame || !S.map || st.map_rev !== S.map.map_rev) {
    // The tiles changed since the map held (`/api/map?since=R&epoch=E`, phase 6); the whole
    // map for a new game or when the delta does not fit (another epoch: a restarted server).
    let map = null;
    let changed = "all";
    if (!newGame && S.map?.epoch && S.map.rev) {
      const reply = await api.get(`/api/map?since=${S.map.rev}&epoch=${encodeURIComponent(S.map.epoch)}`);
      if (reply) {
        try {
          ({ map, changed } = applyMapDelta(S.map, reply));
        } catch (e) {
          console.warn("map delta:", e.message);
          map = null;
        }
      }
    }
    if (!map) map = await api.get("/api/map");
    if (map) {
      setMap(map);
      mapView.setMap(map, changed);
      if (newGame) centerOnStart();
    }
  }
  gameKey = gk;
  viewKey = vk;
  await refreshLog();
  units.viewChanged();
  empire.viewChanged();
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
  if (cap) mapView.centerOn(cap.x, cap.y, { animate: false });
  else if (u) mapView.centerOn(u.x, u.y, { animate: false });
  else if (S.map) mapView.centerOn(S.map.w / 2, S.map.h / 2, { animate: false });
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
    // Civ4 starts a turn on the first unit waiting for orders (not while spectating).
    const g = S.sel ? S.units.get(S.sel.units[0])?.group : null;
    if (!tools.spectating() && (!S.sel || !S.view.pending.ready_groups.includes(g))) units.selectNext();
  }
  runPopups();
}

/** Shows the deferred popups one at a time, in C++ order (events, votes, elections, research,
 *  idle cities); "Later" sets one aside for the turn. */
async function runPopups() {
  if (popupsRunning) return;
  popupsRunning = true;
  try {
    for (;;) {
      // The spectator mode leaves the questions to the AI (it plays every turn).
      if (S.status?.phase !== "human" || modalOpen() || sending() || tools.spectating()) break;
      const q = popupQueue(S.view?.pending, S.dismissed, S.status.turn);
      if (!q.length) break;
      const p = q[0];
      // Asked once per turn: answered, or set aside until the end-turn gate reopens it.
      S.dismissed.add(p.key);
      if (p.kind === "event") await screens.openEvent(p.event);
      else if (p.kind === "vote") await screens.openVote(p.vote);
      else if (p.kind === "election") await screens.openElection(p.election);
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
  if (!S.sel && S.status?.phase === "human" && !modalOpen() && !tools.spectating()) units.selectNext();
}

// ---------------------------------------------------------------------------------------------
// Cities list

function citiesList() {
  const mine = ownCities();
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

/** The screens of the menu and the F keys (Civ4's advisors: F1 cities, F2 finances, F3 civics,
 *  F4 diplomacy, F6 technology, F7 religion). */
function openScreen(name) {
  if (!S.view) return;
  switch (name) {
    case "cities":
      return citiesList();
    case "finance":
      return empire.openFinance();
    case "civics":
      return empire.openCivics();
    case "diplomacy":
      return openDiplomacy();
    case "techs":
      return empire.openTechTree();
    case "religion":
      return empire.openReligion();
    case "research":
      return screens.openResearch();
  }
}

const F_KEYS = { F1: "cities", F2: "finance", F3: "civics", F4: "diplomacy", F6: "techs", F7: "religion" };

function setLayer(layer) {
  mapView.setLayer(layer);
  for (const b of document.querySelectorAll("#mapwrap .layers button")) b.classList.toggle("active", b.dataset.layer === layer);
}

/** The city Home / End selected last (Civ4's city selection, without the city screen), or null;
 *  selecting units clears it, as in Civ4. */
let citySel = null;

/** C++ CONTROL_NEXTCITY / CONTROL_PREVCITY (Home / End, Num+ / Num-) outside the city screen:
 *  the next / previous of the player's cities after the one selected (`cycleCities`), else the
 *  city at the look-at plot (`selectLookAtCity(true)`); then the map looks at it. */
function stepCity(forward) {
  const near = mapView?.center?.() ?? null;
  const dist = near && S.map ? (ax, ay, bx, by) => plotDistance(S.map, ax, ay, bx, by) : null;
  const c = cycleCity(ownCities(), citySel, forward, near, dist);
  if (!c) return;
  if (S.sel) units.clearSelection();
  citySel = c.id;
  mapView.centerOn(c.x, c.y);
}

/** The keys (also under `?`). Civ4's controls (CIV4ControlInfos.xml, CvGame::doControl): Enter
 *  ends the turn when the end-turn message is up (no group waits) and otherwise cycles to the
 *  next group (CONTROL_ENDTURN, priority 1, before CONTROL_CYCLEUNIT); Shift+Enter ends it
 *  whatever waits; Space ends it like Enter when the selection's actions do not take it (no
 *  selection: CONTROL_ENDTURN_ALT); W cycles; C centres on the selection; Home / End the next /
 *  previous city; F10 the capital's screen; F1-F4, F6, F7 the advisors; Ctrl+S, Ctrl+L. Then
 *  ffh-play's own: Tab / Shift+Tab cycle forward / back (Civ4's Tab is chat), Ctrl+1/2/3 the
 *  layers, `?`, the arrows and `+`/`-`. Then the selection's action hotkeys from the XML
 *  (`units.hotkeyEvent`: F fortify or sleep, S sentry, Space skip, B found, G go to, R road, Del
 *  delete, Backspace cancel, L load, U unload, Shift+P pillage, ...). */
function onKey(ev) {
  const t = ev.target;
  const typing = t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement;
  if (ev.key === "Escape") {
    if (units.currentMode()) units.setMode(null);
    else if (units.clearWaypoints()) return;
    else if (!onEscape() && !modalOpen()) units.clearSelection();
    return;
  }
  const k = ev.key;
  const handled = () => ev.preventDefault();
  const ctrl = ev.ctrlKey || ev.metaKey;
  const plain = !ctrl && !ev.altKey && !ev.shiftKey;
  // Home / End in the city screen: its next / previous city (cycleCities with the screen up).
  const cityStep = { Home: 1, End: -1 }[k] ?? (ev.code === "NumpadAdd" ? 1 : ev.code === "NumpadSubtract" ? -1 : 0);
  if (cityStep && plain && !typing && stepCityScreen(cityStep, openTags().at(-1))) return handled();
  if (typing || modalOpen()) return;
  const human = S.status?.phase === "human";
  if (ctrl && !ev.altKey) {
    const lower = k.toLowerCase();
    if (lower === "s" && !ev.shiftKey) return handled(), S.view && screens.saveDialog();
    if (lower === "l" && !ev.shiftKey) return handled(), screens.loadScreen();
    const layer = { 1: "terrain", 2: "political", 3: "yield" }[k];
    if (layer) return handled(), setLayer(layer);
  }
  if (F_KEYS[k] && !ctrl && !ev.altKey) {
    handled();
    return openScreen(F_KEYS[k]);
  }
  if (cityStep && plain) {
    stepCity(cityStep > 0);
    return handled();
  }
  switch (k) {
    case "Enter":
      if (!ctrl && !ev.altKey) {
        // Shift+Enter: CONTROL_FORCEENDTURN. Enter: CONTROL_ENDTURN while the end-turn message
        // is up, else CONTROL_CYCLEUNIT.
        if (human && ev.shiftKey) endTurn(true);
        else if (human && endTurnMessage()) endTurn(false);
        else if (human) units.selectNext();
        return handled();
      }
      break;
    case "Tab":
      if (!ctrl && !ev.altKey) {
        units.selectNext({ forward: !ev.shiftKey });
        return handled();
      }
      break;
    case "w":
    case "W":
      if (plain) {
        units.selectNext();
        return handled();
      }
      break;
    case "c":
    case "C":
      if (plain) {
        if (S.sel) mapView.centerOn(S.sel.x, S.sel.y);
        return handled();
      }
      break;
    case "F10":
      if (plain) {
        // CONTROL_SELECTCAPITAL: the capital's city screen.
        const cap = ownCities().find((c) => c.capital);
        if (cap) {
          mapView.centerOn(cap.x, cap.y);
          openCity(cap.id);
        }
        return handled();
      }
      break;
    case " ":
      if (plain && !S.sel) {
        // CONTROL_ENDTURN_ALT: with no selection, Space ends the turn when nothing waits.
        if (human && endTurnMessage()) endTurn(false);
        return handled();
      }
      break;
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
      if (!ctrl) {
        mapView.zoom(1);
        return handled();
      }
      break;
    case "-":
      if (!ctrl) {
        mapView.zoom(-1);
        return handled();
      }
      break;
  }
  // The selection's action hotkeys (Civ4's, from the XML). With Ctrl only a key the selection's
  // actions use (the browser keeps Ctrl+C, Ctrl+R, ...). Space with no action taking it (the
  // selected group cannot skip) ends the turn when the end-turn message is up (ENDTURN_ALT).
  if (!S.sel || ["Shift", "Control", "Alt", "Meta"].includes(k)) return;
  if (ctrl ? K.boundTo(units.currentActions(), ev, S.data) : k.length === 1 || ["Delete", "Backspace"].includes(k) || /^(Numpad|Key|Digit)/.test(ev.code ?? "")) {
    handled();
    units.hotkeyEvent(ev).then((ran) => {
      if (!ran && k === " " && plain && S.status?.phase === "human" && endTurnMessage()) endTurn(false);
    });
  }
}

// ---------------------------------------------------------------------------------------------
// Boot

function wire() {
  const click = (id, f) => document.getElementById(id)?.addEventListener("click", f);
  click("end-turn", (ev) => endTurn(ev.shiftKey));
  click("btn-research", () => openScreen("research"));
  click("btn-techs", () => openScreen("techs"));
  click("btn-cities", () => openScreen("cities"));
  click("btn-civics", () => openScreen("civics"));
  click("btn-religion", () => openScreen("religion"));
  click("btn-diplo", () => openScreen("diplomacy"));
  click("btn-tools", () => tools.openTools());
  click("btn-save", () => S.view && screens.saveDialog());
  click("btn-load", () => screens.loadScreen());
  click("btn-new", () => screens.newGame());
  click("btn-keys", () => screens.keysHelp());
  click("btn-raw", () => screens.rawRequest());
  click("zoom-in", () => mapView.zoom(1));
  click("zoom-out", () => mapView.zoom(-1));
  for (const b of document.querySelectorAll("#mapwrap .layers button")) b.addEventListener("click", () => setLayer(b.dataset.layer));
  for (const b of document.querySelectorAll("#bottom-tabs button")) b.addEventListener("click", () => setTab(b.dataset.tab));
  click("bottom-toggle", () => document.body.classList.toggle("bottom-closed"));
  document.addEventListener("ffh-research", () => S.view && screens.openResearch());
  document.addEventListener("keydown", onKey);
  // Shift-right-click waypoints are sent when Shift is released (or the page loses the keys).
  document.addEventListener("keyup", (ev) => ev.key === "Shift" && units.shiftReleased());
  window.addEventListener("blur", () => units.shiftReleased());
  on("applied", () => refreshStatus());
  on("refused", () => refreshStatus());
  on("kick", () => refreshStatus());
  on("open-screen", (name) => openScreen(name));
  on("spectate", () => renderTop());
  on("game-started", (opts) => {
    if (opts?.spectate) tools.startSpectating({ newGame: true });
    else if (tools.spectating()) tools.stopSpectating();
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
  on("selection", () => {
    if (S.sel) citySel = null;
    units.renderPanel();
    renderTop();
  });
  on("mode", () => units.renderPanel());
  on("renderer", (r) => setRenderer(r));
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
  // The isometric renderer (docs/art/SOC_DIRECTION.md): an opt-in map renderer with PNG tiles and
  // unit sprites on team rings. It owns its own projection, so the art/flat switches leave it be.
  // Chosen with `?renderer=iso` or the remembered Tools → Options choice.
  if (rendererChoice() === "iso") mapView.useIso(true);
  // The experimental 3D renderer (docs/R3D.md): loaded on demand; without WebGL2 it falls back.
  if (rendererChoice() === "3d") use3d();
  // The art loads beside the data; the page works meanwhile (and without it). lookLoaded() builds
  // the art worker, which bakes in the engine's type ordering (from /api/data) once, so it must run
  // after setLookData(): otherwise, if the static art bundle wins the race against the server-
  // computed data, the worker draws terrain/units with the art's own ordering for the whole session.
  const art = loadLook();
  try {
    const [data, setup] = await Promise.all([api.get("/api/data"), api.get("/api/setup")]);
    setData(data);
    setLookData(data);
    S.setup = setup;
  } catch (e) {
    toast(`Cannot load the game data: ${e.message ?? e}`, "error", 60000);
    return;
  }
  api.watch(onStatus, onDisconnect);
  if (await art) lookLoaded();
  window.ffh.ready = true;
}

/** The art has loaded: the map draws with it unless the flat renderer was asked for, and the
 *  interface redraws with its type and icons. */
function lookLoaded() {
  if (!mapView.iso && rendererChoice() === "art") mapView.useArt(true);
  renderTop();
  renderBottom();
  units.renderPanel();
}

/** Switches the map's renderer ("art", "iso" or "flat"; Tools → Options). The isometric renderer
 *  owns the map on its own; leaving it turns it off and falls back to art or flat. */
function setRenderer(r) {
  if (r === "3d") {
    use3d();
    return;
  }
  if (mapView.r3d) mapView.useR3d(false);
  if (r === "iso") {
    mapView.useIso(true);
    return;
  }
  if (mapView.iso) mapView.useIso(false);
  mapView.useArt(r === "art" && !!look.art);
}

/** Turns the 3D renderer on; when it cannot run, says so and draws with the art (or flat). */
async function use3d() {
  if (await mapView.useR3d(true)) return;
  if (mapView.r3dError?.code === "software-webgl") {
    toast(`${mapView.r3dError.message} Enable hardware acceleration in your browser (or add ?force3d=1 to try anyway); using the 2D map.`, "error", 15000);
  } else {
    toast("The 3D renderer cannot run in this browser (it needs WebGL2); using the 2D map.", "error");
  }
  if (look.art) mapView.useArt(true);
}

// The 3D renderer lost its WebGL context (a driver reset): the 2D map until the page is reloaded.
window.addEventListener("ffh:r3d-lost", () => {
  if (!mapView.r3d) return;
  mapView.useR3d(false);
  if (look.art) mapView.useArt(true);
  toast("The 3D renderer lost its graphics context; using the 2D map until the page is reloaded.", "error");
});

window.ffh = { S, get mapView() { return mapView; }, units, screens, empire, tools, api, emit, ready: false, tab: currentTab, setLayer, openScreen, look, setRenderer };
boot();
