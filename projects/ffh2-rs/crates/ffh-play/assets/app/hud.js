// The top bar (design §5.1 "Empire": gold, research, sliders, turn and year, the Armageddon
// counter, mana, anarchy, golden age, Stasis) with the phase and the progress line during the
// AI turns, and the bottom panel: the turn report, the messages by turn, the game's files.

import { S, dname, tname, turns, playerName, me, emit, endTurnMessage } from "./state.js";
import { h, fill } from "./dom.js";
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
  fill(
    phase,
    h("span.phase-dot", { "data-phase": st.phase ?? "" }),
    st.turn !== undefined ? h("b", `Turn ${st.turn}`) : null,
    st.year !== undefined ? h("span.dim", ` ${yearText(st.year)} · `) : null,
    h("span#phase-text", line),
    st.reveal ? h("span.badge.reveal", "DEBUG REVEAL") : null,
    spectatorBar(),
    autoplayBar(),
  );
  document.body.dataset.phase = st.phase ?? "";
  const end = document.getElementById("end-turn");
  if (end) {
    end.disabled = st.phase !== "human";
    const n = openCount();
    end.textContent = n ? `End turn (${n} open)` : "End turn";
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
  const items = [
    h("span.stat", { title: "Gold (per turn)" }, h("span.icon.gold", "¤"), ` ${m.gold} `, h("span.dim", `(${m.gold_rate >= 0 ? "+" : ""}${m.gold_rate})`)),
    h(
      "button.stat.research",
      { id: "research-button", title: "Research (the tech tree: F6)", onclick: () => document.dispatchEvent(new CustomEvent("ffh-research")) },
      h("span.icon", "⚗"),
      r ? ` ${dname("TechType", r.tech)} (${turns(r.turns)})` : h("span.warn", " choose research"),
      r ? h("span.bar", h("span.fill", { style: { width: `${Math.min(100, Math.round((100 * r.progress) / Math.max(1, r.cost)))}%` } })) : null,
    ),
    h(
      "button.stat.sliders",
      { id: "sliders-button", title: "Commerce sliders: click to change them [F2]", onclick: () => emit("open-screen", "finance") },
      m.commerce_percent
        .map((p, i) => [p, i])
        .filter(([p, i]) => p > 0 || m.flexible[i])
        .map(([p, i]) => `${dname("CommerceType", i).slice(0, 3)} ${p}%`)
        .join(" · "),
    ),
    h("span.stat.armageddon", { title: "Armageddon counter" }, h("span.icon", "☄"), ` ${m.armageddon}`),
  ];
  const mana = m.mana.filter((x) => x.count > 0);
  if (mana.length) {
    const list = mana.map((x) => `${dname("BonusType", x.bonus).replace(/ Mana$/, "")} ${x.count}`).join(", ");
    items.push(h("span.stat", { title: `Mana: ${list}` }, h("span.icon", "✦"), ` ${mana.reduce((n, x) => n + x.count, 0)} mana`));
  }
  if (m.anarchy > 0) items.push(h("span.stat.warn", `Anarchy ${m.anarchy}`));
  if (m.golden_age > 0) items.push(h("span.stat.good", `Golden age ${m.golden_age}`));
  const stasis = Object.entries(m.stasis ?? {}).filter(([, n]) => n > 0);
  if (stasis.length) items.push(h("span.stat.warn", `Stasis: ${stasis.map(([k, n]) => `${k} ${n}`).join(", ")}`));
  const idle = v.pending?.ai_groups_idle?.length ?? 0;
  if (idle) items.push(h("span.stat.warn", { title: "Automated and AI-controlled units wait for the AI automation (not ported yet)" }, `${idle} AI-controlled group${idle > 1 ? "s" : ""} idle`));
  items.push(h("span.stat.dim", `Score ${m.score}`));
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
    msgs.length ? h("div.section", h("h4", "Messages"), h("ul.messages", msgs.map((m) => h("li", m.text)))) : null,
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
    turnsDesc.map((t) => h("div.turn-group", h("h4", `Turn ${t}`), h("ul", byTurn.get(t).map((m) => h("li", m.text))))),
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
