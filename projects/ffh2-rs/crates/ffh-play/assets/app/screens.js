// Screens and deferred popups (design §5.1 "Setup and files", "Popups and decisions",
// "Empire"): new game, load, save, game over, crash and stuck screens, the research chooser,
// the event and vote popups, the keys, and a raw request box for testers.

import * as api from "./api.js";
import { S, emit, dname, info, turns, voteChoiceLabel, teamName, me, saveNameOfPath } from "./state.js";
import { h, toast } from "./dom.js";
import { openModal } from "./modals.js";
import { run } from "./commands.js";
import { render as markup, plain } from "./markup.js";
import { illuminate } from "./look.js";

// ---------------------------------------------------------------------------------------------
// Research (the CHOOSETECH popup and the research button).

/** The research chooser. Click: research it now; Shift-click: add it to the queue. With
 *  `popup` (research needed at a control point) it offers "Later" and closes once chosen.
 *  Resolves with "chosen", "later" or null. */
export async function openResearch({ popup = false } = {}) {
  const m = openModal({ title: popup ? "Choose research" : "Research", body: [h("p.dim", "…")], tag: "research", wide: true, buttons: popup ? [{ label: "Later", value: "later" }] : [{ label: "Close", value: null }] });
  let result = null;
  const load = async () => {
    let t;
    try {
      t = await api.query("techs");
    } catch (e) {
      return m.setBody(h("p.warn", e.detail ?? e.message));
    }
    if (m.closed) return;
    const can = t.techs.filter((x) => x.can_research && !x.has && !x.disabled);
    can.sort((a, b) => (a.turns ?? 1e9) - (b.turns ?? 1e9) || a.cost - b.cost);
    const cur = t.current !== null ? t.techs[t.current] : null;
    const head = h(
      "div.research-head",
      cur ? h("div", "Researching ", h("b", dname("TechType", t.current)), ` (${cur.progress}/${cur.cost}, ${turns(cur.turns)})`) : h("div.warn", "Nothing is being researched."),
      t.queue.length > 1 ? h("div.dim", `Queue: ${t.queue.map((q) => dname("TechType", q)).join(" → ")}`) : null,
      h("div.dim", `${t.rate} research per turn. Click: research now. Shift-click: add to the queue.`),
    );
    const list = h(
      "div.build-grid",
      can.map((x) => {
        const e = info("TechType", x.tech);
        const help = [e?.help, e?.strategy].filter(Boolean).map((s) => plain(s)).join("\n");
        return h(
          "button.build",
          {
            "data-tech": x.tech,
            class: x.researching ? "current" : x.queued !== null ? "queued" : "",
            title: help || dname("TechType", x.tech),
            onclick: async (ev) => {
              const reply = await run({ c: "research", tech: x.tech, shift: !!ev.shiftKey });
              if (!reply) return;
              if (popup && !ev.shiftKey) {
                result = "chosen";
                m.close("chosen");
              } else load();
            },
          },
          h("span", dname("TechType", x.tech)),
          h("span.dim", `${turns(x.turns)} · ${x.cost}${x.progress ? ` (${x.progress})` : ""}`),
        );
      }),
    );
    m.setBody(head, can.length ? list : h("p.dim", "No technology can be researched now."));
  };
  load();
  const v = await m.done;
  return result ?? v;
}

// ---------------------------------------------------------------------------------------------
// Events, votes and elections (deferred popups, design §2.3).

/** An event popup (`pending.events[]`). Resolves "chosen", "later" or null. */
export async function openEvent(ev) {
  let result = null;
  const err = h("p.error");
  const options = h(
    "div.options",
    ev.options.map((o) =>
      h(
        "button.option",
        {
          disabled: !o.enabled,
          "data-event": o.event,
          onclick: async () => {
            const reply = await run({ c: "event", triggered: ev.triggered, event: o.event });
            if (reply) {
              result = "chosen";
              m.close("chosen");
            }
          },
        },
        h("span", markup(dname("EventType", o.event))),
        o.effects?.length ? h("small", o.effects.map((e) => plain(e)).join(" · ")) : null,
      ),
    ),
  );
  const where = ev.plot ? h("button.link", { onclick: () => emit("center", ev.plot[0], ev.plot[1]) }, "Show on the map") : null;
  const m = openModal({
    title: dname("EventTriggerType", ev.trigger),
    body: [h("p.page-text", illuminate(markup(ev.text))), where, options, err],
    buttons: [{ label: "Later", value: "later", title: "Answer later (ending the turn without an answer drops the event)" }],
    tag: "event",
    className: "event",
  });
  const v = await m.done;
  return result ?? v;
}

/** A vote popup (`pending.votes[]`). Resolves "chosen", "later" or null. */
export async function openVote(v) {
  let result = null;
  const choices = h(
    "div.options",
    v.choices.map((c) =>
      h(
        "button.option",
        {
          "data-choice": c,
          onclick: async () => {
            const reply = await run({ c: "vote", vote: v.vote, choice: c });
            if (reply) {
              result = "chosen";
              m.close("chosen");
            }
          },
        },
        voteChoiceLabel(c),
      ),
    ),
  );
  const m = openModal({
    title: dname("VoteType", v.resolution),
    body: [h("p.dim", dname("VoteSourceType", v.source)), h("p.page-text", illuminate(markup(v.text))), choices],
    buttons: [{ label: "Later", value: "later", title: "Unanswered votes get one more turn, then abstain" }],
    tag: "vote",
    className: "event",
  });
  const r = await m.done;
  return result ?? r;
}

/** The secretary general's election (`pending.elections[]`, C++ BUTTONPOPUP_CHOOSEELECTION):
 *  one button per resolution, then "None". Resolves "chosen", "later" or null. */
export async function openElection(e) {
  let result = null;
  const err = h("p.error");
  const send = async (option) => {
    const reply = await run({ c: "election", selection: e.selection, option });
    if (reply) {
      result = "chosen";
      m.close("chosen");
    }
  };
  const options = h(
    "div.options",
    e.options.map((o, i) => h("button.option", { "data-option": i, onclick: () => send(i) }, h("span", markup(o.text || dname("VoteType", o.vote))))),
    h("button.option", { "data-option": "none", onclick: () => send(null) }, h("span", "None")),
  );
  const m = openModal({
    title: dname("VoteSourceType", e.source),
    body: [h("p", "As secretary general, choose the resolution to put to the vote."), options, err],
    buttons: [{ label: "Later", value: "later", title: "Answer later (ending the turn without an answer puts no resolution to the vote)" }],
    tag: "election",
  });
  const r = await m.done;
  return result ?? r;
}

// ---------------------------------------------------------------------------------------------
// New game, load, save.

function select(name, list, value, { none = null } = {}) {
  return h(
    "select",
    { name, id: `ng-${name}` },
    none ? h("option", { value: "" }, none) : null,
    list.map((e) => h("option", { value: e.type, selected: e.id === value }, e.name)),
  );
}

/** The tag of the New game dialog the no-game screen forces (not dismissible); main.js closes
 *  it once a game exists. A New game dialog opened by the player is tagged "newgame". */
export const NEWGAME_REQUIRED = "newgame-required";

/** The new-game screen (design §5.1): civilization and leader (or random), opponents, world
 *  size, speed, difficulty, era, climate, sea level, seed, game options and victories. */
export function newGame({ dismissible = true } = {}) {
  const st = S.setup;
  if (!st) return null;
  const d = st.defaults;
  const civ = select("civ", st.civilizations, d.civ, { none: "Random" });
  const leader = h("select", { name: "leader", id: "ng-leader" });
  const fillLeaders = () => {
    const c = st.civilizations.find((x) => x.type === civ.value);
    leader.replaceChildren(h("option", { value: "" }, "Random"), ...(c ? c.leaders.map((l) => h("option", { value: st.leaders[l].type }, st.leaders[l].name)) : []));
  };
  civ.addEventListener("change", fillLeaders);
  fillLeaders();
  const seed = h("input", { type: "number", id: "ng-seed", min: 0, max: 4294967295, value: d.seed });
  const dice = h("button.small", { type: "button", title: "A random seed", onclick: () => (seed.value = String(Math.floor(Math.random() * 1e6))) }, "⚄");
  const opponents = h("input", { type: "number", id: "ng-opponents", min: 1, max: st.max_opponents, value: d.opponents });
  const world = select("world", st.worlds, d.world);
  const speed = select("speed", st.speeds, d.speed);
  const handicap = select("handicap", st.handicaps, d.handicap);
  const era = select("era", st.eras, d.era);
  const climate = select("climate", st.climates, d.climate);
  const sea = select("sea_level", st.sea_levels, d.sea_level);
  const name = h("input", { type: "text", id: "ng-name", placeholder: "game-<seed>", pattern: "[A-Za-z0-9_.\\-]{1,64}", title: "The game's directory under the saves directory" });
  const options = h(
    "div.checks",
    st.options.map((o) => h("label", { title: o.type }, h("input", { type: "checkbox", value: o.type, checked: d.options.includes(o.id) }), ` ${o.name}`)),
  );
  const victories = h(
    "div.checks",
    st.victories.map((v) => h("label", h("input", { type: "checkbox", value: v.type, checked: !d.disabled_victories.includes(v.id) }), ` ${v.name}`)),
  );
  const spectate = h("input", { type: "checkbox", id: "ng-spectate" });
  const err = h("p.error");
  const field = (label, el, extra = null) => h("label.field", h("span", label), el, extra);
  const form = h(
    "div.newgame",
    h(
      "div.fields",
      field("Civilization", civ),
      field("Leader", leader),
      field("Opponents", opponents),
      field("World size", world),
      field("Speed", speed),
      field("Difficulty", handicap),
      field("Era", era),
      field("Climate", climate),
      field("Sea level", sea),
      field("Seed", h("div.inline", seed, dice)),
      field("Game name", name),
    ),
    h("details", h("summary", "Game options"), options),
    h("details", h("summary", "Victory conditions"), victories),
    h(
      "label.check",
      { title: "The AI plays every civilization (yours too), one turn at a time, with the whole map shown; pause, step or stop it from the top bar" },
      spectate,
      " Spectate: watch the AI play",
    ),
    err,
  );
  const start = async (m) => {
    const setup = {
      seed: Number(seed.value),
      opponents: Number(opponents.value),
      world: world.value,
      speed: speed.value,
      handicap: handicap.value,
      era: era.value,
      climate: climate.value,
      sea_level: sea.value,
      options: [...options.querySelectorAll("input:checked")].map((i) => i.value),
      disabled_victories: [...victories.querySelectorAll("input:not(:checked)")].map((i) => i.value),
    };
    if (civ.value) setup.civ = civ.value;
    if (leader.value) setup.leader = leader.value;
    err.textContent = "";
    try {
      await api.session("new", name.value ? { setup, name: name.value } : { setup });
      m.close("started");
      emit("game-started", { spectate: spectate.checked });
    } catch (e) {
      err.textContent = e.detail ?? e.message;
    }
  };
  return openModal({
    title: "New game",
    body: [form],
    tag: dismissible ? "newgame" : NEWGAME_REQUIRED,
    wide: true,
    dismissible,
    buttons: [
      { label: "Start", primary: true, onClick: start },
      { label: "Load a game…", onClick: () => loadScreen() },
    ],
  });
}

/** The load screen: every game directory with its saves (newest autosave first). */
export async function loadScreen() {
  const m = openModal({ title: "Load a game", body: [h("p.dim", "…")], tag: "load", wide: true, buttons: [{ label: "Close", value: null }] });
  let r;
  try {
    r = (await api.session("saves")).r;
  } catch (e) {
    m.setBody(h("p.warn", e.detail ?? e.message));
    return m.done;
  }
  const err = h("p.error");
  // One load at a time: a double click sends one request (P5 review).
  let loading = false;
  const load = async (name) => {
    if (loading) return;
    loading = true;
    const buttons = [...m.el.querySelectorAll("button.save")];
    buttons.forEach((b) => (b.disabled = true));
    err.textContent = "";
    try {
      await api.session("load", { name });
      m.close("loaded");
      emit("game-started");
    } catch (e) {
      err.textContent = e.detail ?? e.message;
    } finally {
      loading = false;
      if (!m.closed) buttons.forEach((b) => (b.disabled = false));
    }
  };
  const games = [...r.games].sort((a, b) => (a.game === r.current ? -1 : b.game === r.current ? 1 : a.game.localeCompare(b.game)));
  const list = games.map((g) => {
    const saves = [...g.saves].sort((a, b) => (a.auto === b.auto ? b.name.localeCompare(a.name) : a.auto ? 1 : -1));
    return h(
      "div.section",
      h("h4", g.game, g.game === r.current ? h("span.badge", "current") : null),
      saves.length
        ? h("div.save-list", saves.map((s) => h("button.save", { "data-save": `${g.game}/${s.name}`, title: s.path, onclick: () => load(`${g.game}/${s.name}`) }, s.name, s.auto ? h("span.dim", " auto") : null)))
        : h("p.dim", "No saves."),
    );
  });
  m.setBody(h("p.dim", `Saves directory: ${r.root}`), err, list.length ? list : h("p.dim", "No games yet."));
  return m.done;
}

/** The save dialog. */
export function saveDialog() {
  const name = h("input", { type: "text", id: "save-name", value: `turn-${S.status?.turn ?? 0}`, pattern: "[A-Za-z0-9_.\\-]{1,64}" });
  const err = h("p.error");
  const save = async (m) => {
    try {
      const r = (await api.session("save", { name: name.value })).r;
      toast(`Saved: ${r.save}`);
      m.close("saved");
    } catch (e) {
      err.textContent = e.detail ?? e.message;
    }
  };
  const m = openModal({
    title: "Save the game",
    body: [h("label.field", h("span", "Name"), name), h("p.dim", "Letters, digits, '.', '_' and '-'. Saved in the game's directory."), err],
    tag: "save",
    buttons: [
      { label: "Save", primary: true, onClick: save },
      { label: "Cancel", value: null },
    ],
  });
  // Enter is the Save button (one request at a time, as a click).
  name.addEventListener("keydown", (ev) => ev.key === "Enter" && m.el.querySelector(".modal-buttons button.primary")?.click());
  name.select();
  return m;
}

// ---------------------------------------------------------------------------------------------
// The end: game over, crash, stuck.

/** The game-over screen: winner, victory type, scores. */
export function gameOver(st) {
  const over = st.over ?? {};
  const mine = me();
  const won = over.winner_team !== null && over.winner_team !== undefined && mine && over.winner_team === mine.team;
  const title = won ? "Victory!" : mine?.alive === false ? "Your civilization has fallen" : "The game is over";
  const rows = [];
  if (mine) rows.push({ name: `${mine.name} (you)`, score: mine.score, alive: mine.alive });
  for (const p of S.players.values()) if (!p.barbarian) rows.push({ name: p.name, score: p.score, alive: p.alive });
  rows.sort((a, b) => b.score - a.score);
  return openModal({
    title,
    tag: "over",
    wide: true,
    body: [
      h("p", over.winner_team !== null && over.winner_team !== undefined ? `Winner: ${teamName(over.winner_team)}` : "No winner.", over.victory !== null && over.victory !== undefined ? ` · ${dname("VictoryType", over.victory)} victory` : "", ` · turn ${st.turn}`),
      h("table.scores", h("tr", h("th", "Player"), h("th", "Score")), rows.map((r) => h("tr", { class: r.alive ? "" : "dead" }, h("td", r.name), h("td", String(r.score))))),
    ],
    buttons: [
      { label: "New game", primary: true, onClick: () => newGame() },
      { label: "Load…", onClick: () => loadScreen() },
      { label: "View the map", value: null },
    ],
  });
}

/** The crash screen (design §2.8): the message and the bug report with the replay command. */
export function crashScreen(st) {
  const c = st.crash ?? {};
  const auto = saveNameOfPath(c.autosave);
  const report = [`ffh-play crash: ${c.message ?? "?"}`, `at ${c.location ?? "?"}`, `autosave: ${c.autosave ?? "-"}`, `journal: ${c.journal ?? "-"}`, `repro: ${c.repro ?? "-"}`].join("\n");
  const pre = h("pre.report", report);
  const copy = h("button.small", { onclick: () => navigator.clipboard?.writeText(report).then(() => toast("Copied.")).catch(() => toast("Select the text to copy it.", "error")) }, "Copy");
  return openModal({
    title: "The game crashed",
    tag: "crash",
    wide: true,
    dismissible: false,
    body: [h("p", "A rules error stopped the game. The server is still up: load the last autosave to go on, and please report the bug with the text below."), pre, copy],
    buttons: [
      auto ? { label: "Load the autosave", primary: true, onClick: async (m) => {
        try {
          await api.session("load", { name: auto });
          m.close("loaded");
          emit("game-started");
        } catch (e) {
          toast(e.detail ?? e.message, "error");
        }
      } } : null,
      { label: "Load…", onClick: () => loadScreen() },
      { label: "New game", onClick: () => newGame() },
    ].filter(Boolean),
  });
}

/** The stuck screen: the watchdog stopped a turn that did not finish. */
export function stuckScreen(st) {
  return openModal({
    title: "The turn did not finish",
    tag: "stuck",
    wide: true,
    dismissible: false,
    body: [h("p", "The AI turns ran too long without finishing (a rules or AI bug). Load an autosave to go on."), h("pre.report", String(st.stuck ?? ""))],
    buttons: [
      { label: "Load…", primary: true, onClick: () => loadScreen() },
      { label: "New game", onClick: () => newGame() },
    ],
  });
}

// ---------------------------------------------------------------------------------------------
// Help and testing tools.

/** The keys: Civ4's (the action hotkeys come from the game's XML, as each button's tooltip shows). */
export function keysHelp() {
  const keys = [
    ["Mouse", ""],
    ["Left click", "select units (again: the next group there), open your city, show a plot"],
    ["Right click", "move or attack with the selection; with Shift held, add waypoints (sent when Shift is released)"],
    ["Drag, wheel, arrows, + −", "pan and zoom the map; the minimap centres"],
    ["Turn and selection (Civ4's controls)", ""],
    ["Enter", "the next unit waiting for orders; when none waits (the End turn button glows), end the turn"],
    ["Shift+Enter", "end the turn whatever waits, without the checks"],
    ["W", "the next unit waiting for orders (Civ4's cycle: a stack whose units cannot all move is skipped; when it wraps, the automated units move)"],
    ["Space", "with no unit selected, end the turn when none waits"],
    ["C", "centre on the selection"],
    ["Home / End", "the next / previous city (in the city screen: its next / previous city)"],
    ["F10", "the capital's city screen"],
    ["Esc", "cancel a plot pick, close a dialog, deselect"],
    ["ffh-play's own", ""],
    ["Tab / Shift+Tab", "the next / previous unit waiting (in Civ4, Tab is chat)"],
    ["Orders (Civ4's hotkeys)", ""],
    ["Space", "skip the turn"],
    ["F / S / H", "fortify (sleep for units that cannot) / sentry / heal"],
    ["B", "found a city (bombard for siege units)"],
    ["G then click, Alt+R", "go to a plot (Shift-click: add waypoints, sent when Shift is released) / build a route to it"],
    ["R, I, M, …", "the worker's builds: road, farm, mine, … (each button shows its key)"],
    ["L / U / Shift+U", "load / unload / unload all"],
    ["Shift+P", "pillage"],
    ["Del / Backspace", "delete the units (asks first) / cancel the orders"],
    ["Screens", ""],
    ["F1 / F2 / F3 / F4", "cities / finances (the sliders) / civics / diplomacy and trade"],
    ["F6 / F7", "the tech tree / religion"],
    ["Ctrl+S / Ctrl+L", "save / load"],
    ["Ctrl+1 / 2 / 3", "map layer: terrain / political / yields (ffh-play's)"],
    ["?", "this list (ffh-play's)"],
  ];
  return openModal({
    title: "Keys",
    tag: "keys",
    body: [h("table.kv", keys.map(([k, v]) => (v ? h("tr", h("th", k), h("td", v)) : h("tr", h("th.group", { colspan: 2 }, k)))))],
    buttons: [{ label: "Close", value: null }],
  });
}

/** A box for raw protocol requests (design §5.1 "Rules-testing tools"). */
export function rawRequest() {
  const box = h("textarea.raw", { rows: 4, spellcheck: false }, '{"k":"q","q":"pending"}');
  const out = h("pre.report");
  const send = async () => {
    try {
      out.textContent = JSON.stringify(await api.send(JSON.parse(box.value)), null, 2);
    } catch (e) {
      out.textContent = String(e.message ?? e);
    }
    emit("kick");
  };
  return openModal({ title: "Raw request", tag: "raw", wide: true, body: [box, out], buttons: [{ label: "Send", primary: true, onClick: send }, { label: "Close", value: null }] });
}

