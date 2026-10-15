// The top bar (design §5.1 "Empire": gold, research, sliders, turn and year, the Armageddon
// counter, mana, anarchy, golden age, Stasis) with the phase and the progress line during the
// AI turns, and the bottom panel: the turn report, the messages by turn, the game's files.

import { S, dname, tname, turns, playerName, me, emit, endTurnMessage } from "./state.js";
import { h, fill } from "./dom.js";
import { render as markup } from "./markup.js";
import { look, pix, pixable, iconEl, uiIcon, barEl } from "./look.js";
import { openCount } from "./commands.js";
import { spectatorBar, autoplayBar } from "./tools.js";

/** Year for people: -4000 -> "4000 BC". */
export function yearText(y) {
  if (y === null || y === undefined) return "";
  return y < 0 ? `${-y} BC` : `${y} AD`;
}

const PHASES = {
  human: "Your turn",
  running: "Other players are moving…",
  decision: "A decision is waiting",
  over: "Game over",
  stuck: "The turn did not finish",
  no_game: "No game",
  crashed: "Crashed",
};

/** An icon of the ui set (look.js), or the plain glyph before the art loaded. */
function ic(name, glyph, title, cls = "") {
  return iconEl(name, { title }) ?? h(`span.icon${cls}`, { title }, glyph);
}

/** A number in the top bar: Vigil (docs/ART.md 11.6: big numbers in the top bar). */
function big(text, cls = "") {
  return h(`span.num${cls}`, pix(String(text), { font: "vigil" }));
}

/** Short text in the top bar: Vigil Book. */
function small(text, cls = "") {
  return h(`span${cls}`, pix(String(text)));
}

const sep = () => h("span.sep");

/** Redraws the top bar's empire summary and phase line. */
export function renderTop() {
  const st = S.status ?? {};
  const v = S.view;
  const m = v?.me;
  const info = document.getElementById("empire");
  const phase = document.getElementById("phase");
  let line = PHASES[st.phase] ?? st.phase ?? "connecting…";
  if (st.phase === "running") {
    const active = (st.active_players ?? []).filter((p) => p !== st.human).map((p) => playerName(p));
    if (active.length) line = `Moving: ${active.join(", ")}`;
    if (st.ms_in_phase !== undefined) line += ` · ${(st.ms_in_phase / 1000).toFixed(1)} s`;
  }
  // The crescent is amber on your turn (the active-turn mark), else the waiting moon.
  const yours = st.phase === "human";
  fill(
    phase,
    h("span.phase-dot", { "data-phase": st.phase ?? "" }),
    look.art ? ic(yours ? "icon16/turn" : "icon16/turn_wait", "", yours ? "Your turn" : line) : null,
    st.turn !== undefined ? h("b.turn", small("Turn", ".dim"), " ", big(st.turn)) : null,
    st.year !== undefined ? h("span.dim.year", " ", small(yearText(st.year)), " ") : null,
    h("span#phase-text", pixable(line) ? pix(line) : line),
    st.reveal ? h("span.badge.reveal", "DEBUG REVEAL") : null,
    spectatorBar(),
    autoplayBar(),
  );
  document.body.dataset.phase = st.phase ?? "";
  const end = document.getElementById("end-turn");
  if (end) {
    end.disabled = st.phase !== "human";
    const n = openCount();
    const label = n ? `End turn (${n} open)` : "End turn";
    if (end.dataset.label !== `${label}|${!!look.art}`) {
      end.dataset.label = `${label}|${!!look.art}`;
      fill(end, look.art ? iconEl("icon16/turn") : null, look.art ? pix(label, { font: "vigil", smallCaps: true }) : label);
    }
    // Civ4's end-turn message: no group waits, so Enter (or Space) ends the turn; else Enter
    // selects the next group.
    const msg = endTurnMessage();
    end.classList.toggle("end-turn-message", msg);
    end.title = msg ? "End the turn [Enter, Space]; Shift: without the checks" : "End the turn [Shift+Enter: without the checks]; Enter selects the next unit waiting";
  }
  if (!m) {
    fill(info);
    return;
  }
  const r = m.research;
  const pct = r ? Math.min(1, r.progress / Math.max(1, r.cost)) : 0;
  const next = r ? Math.min(1, (r.progress + (r.rate ?? 0)) / Math.max(1, r.cost)) : null;
  const items = [
    h("span.stat.gold", { title: `Gold ${m.gold} (${m.gold_rate >= 0 ? "+" : ""}${m.gold_rate} a turn)` }, ic("icon16/gold", "¤", "Gold", ".gold"), big(m.gold), small(`${m.gold_rate >= 0 ? "+" : ""}${m.gold_rate}`, m.gold_rate < 0 ? ".warn" : ".dim")),
    sep(),
    h(
      "button.stat.research",
      { id: "research-button", title: "Research (the tech tree: F6)", onclick: () => document.dispatchEvent(new CustomEvent("ffh-research")) },
      ic(r ? "icon16/research" : "icon16/research_needed", "⚗", "Research"),
      r ? small(dname("TechType", r.tech)) : h("span.warn", pix("choose research")),
      r ? barEl(pct, "research", { w: 30, next, title: `${r.progress}/${r.cost}` }) : null,
      r ? small(turns(r.turns), ".dim") : null,
    ),
    sep(),
    h(
      "button.stat.sliders",
      { id: "sliders-button", title: "Commerce sliders: click to change them [F2]", onclick: () => emit("open-screen", "finance") },
      m.commerce_percent
        .map((p, i) => [p, i])
        .filter(([p, i]) => p > 0 || m.flexible[i])
        .map(([p, i]) => h("span.slider-item", look.art ? uiIcon("commerce", i, 8, { title: dname("CommerceType", i) }) : `${dname("CommerceType", i).slice(0, 3)} `, small(`${p}%`))),
    ),
    sep(),
    h("span.stat.armageddon", { title: `Armageddon counter ${m.armageddon}` }, look.art ? uiIcon("armageddon", m.armageddon, 16) : h("span.icon", "☄"), big(m.armageddon)),
  ];
  const mana = m.mana.filter((x) => x.count > 0);
  if (mana.length) {
    const list = mana.map((x) => `${dname("BonusType", x.bonus).replace(/ Mana$/, "")} ${x.count}`).join(", ");
    const spheres = look.art
      ? mana.map((x) => h("span.sphere", uiIcon("mana", tname("BonusType", x.bonus) ?? "", 8, { title: dname("BonusType", x.bonus) }) ?? null, small(String(x.count))))
      : [` ${mana.reduce((n, x) => n + x.count, 0)} mana`];
    items.push(h("span.stat.mana", { title: `Mana: ${list}` }, ic("icon16/mana", "✦", "Mana"), ...spheres));
  }
  if (m.anarchy > 0) items.push(sep(), h("span.stat.warn", { title: `Anarchy: ${m.anarchy} turns` }, ic("icon16/anarchy", "", "Anarchy"), small(`Anarchy ${m.anarchy}`)));
  if (m.golden_age > 0) items.push(sep(), h("span.stat.good", { title: `Golden age: ${m.golden_age} turns` }, ic("icon16/golden_age", "", "Golden age"), small(`Golden age ${m.golden_age}`)));
  const stasis = Object.entries(m.stasis ?? {}).filter(([, n]) => n > 0);
  if (stasis.length) items.push(sep(), h("span.stat.warn", ic("icon16/stasis", "", "Stasis"), small(`Stasis: ${stasis.map(([k, n]) => `${k} ${n}`).join(", ")}`)));
  const idle = v.pending?.ai_groups_idle?.length ?? 0;
  if (idle) items.push(sep(), h("span.stat.dim", { title: "Automated and FfH AI-controlled units: the AI moves them during your turn" }, ic("icon16/ai_idle", "", "Automated groups to move"), small(`${idle} automated group${idle > 1 ? "s" : ""} to move`)));
  items.push(sep(), h("span.stat.dim", { title: "Score" }, small("Score"), big(m.score)));
  fill(info, items);
}

// ---------------------------------------------------------------------------------------------
// The bottom panel.

let tab = "report";

export function setTab(t) {
  tab = t;
  for (const b of document.querySelectorAll("#bottom-tabs button")) b.classList.toggle("active", b.dataset.tab === t);
  renderBottom();
}

export function currentTab() {
  return tab;
}

/** Redraws the bottom panel's open tab. */
export function renderBottom() {
  const box = document.getElementById("bottom-body");
  if (!box) return;
  if (tab === "report") fill(box, reportView());
  else if (tab === "messages") fill(box, messagesView());
  else fill(box, gameView());
}

function reportView() {
  const r = S.report;
  if (!r) return h("p.dim", "The turn report appears at the start of each turn.");
  const lines = r.lines ?? [];
  const msgs = r.messages ?? [];
  return h(
    "div.report",
    h("h4", r.from_turn === r.turn ? `Turn ${r.turn}` : `Turns ${r.from_turn}-${r.turn}`),
    lines.length ? h("ul", lines.map((l) => h("li", l))) : h("p.dim", "Nothing to report."),
    msgs.length ? h("div.section", h("h4", "Messages"), h("ul.messages", msgs.map((m) => h("li", markup(m.text))))) : null,
  );
}

function messagesView() {
  if (!S.messages.length) return h("p.dim", "No messages yet.");
  const byTurn = new Map();
  for (const m of S.messages) {
    if (!byTurn.has(m.t)) byTurn.set(m.t, []);
    byTurn.get(m.t).push(m);
  }
  const turnsDesc = [...byTurn.keys()].sort((a, b) => b - a);
  return h(
    "div.messages",
    turnsDesc.map((t) => h("div.turn-group", h("h4", `Turn ${t}`), h("ul", byTurn.get(t).map((m) => h("li", markup(m.text)))))),
  );
}

function gameView() {
  const g = S.status?.game;
  const rows = [
    ["Game directory", g?.dir],
    ["Journal", g?.journal],
    ["Last autosave", g?.autosave],
    ["Map", S.map ? `${S.map.w} x ${S.map.h}${S.map.wrap_x ? ", wraps east-west" : ""}` : null],
    ["You", me() ? `${playerName(me().player)} (player ${me().player}, ${dname("CivilizationType", me().civ)})` : null],
    ["Leader", me() ? tname("LeaderHeadType", me().leader) : null],
  ].filter(([, v]) => v);
  if (!rows.length) return h("p.dim", "No game.");
  return h("table.kv", rows.map(([k, v]) => h("tr", h("th", k), h("td", h("code", String(v))))));
}
