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

/** The last phase seen, to flare the crescent once when control returns to the player. */
let lastPhase = null;
/** The game the tween state belongs to; a new or loaded game resets it so numbers do not animate
 *  from the previous game's values. */
let hudGame = null;

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

// ---------------------------------------------------------------------------------------------
// Tweened numbers (the top bar's gold, score and research bar count/fill smoothly when they
// change, rather than snapping). renderTop() rebuilds the whole #empire bar each tick (fill()),
// so there is no stable node to put a CSS transition on; instead a module-level driver keeps the
// currently-shown value per key, renderTop re-creates each node (marked data-tween) showing that
// value, and one rAF loop eases the values and re-finds the live node by its marker each frame.
// Same shape as map.js's camera glide: an rAF loop that runs only while animating then stops,
// ease-out cubic, and a duration in the glide's own band. prefers-reduced-motion snaps.

const tweens = new Map(); // key -> { value, shown, from, to, t0, dur, lastPaint, kind, cost, next, tech }
let tweenRaf = 0;
const TWEEN_MS = 420;

function reduceMotion() {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

function ensureTweenLoop() {
  if (!tweenRaf) stepTweens();
}

function stepTweens() {
  tweenRaf = requestAnimationFrame(() => {
    tweenRaf = 0;
    const now = performance.now();
    let active = false;
    for (const [key, s] of tweens) {
      if (s.to === s.shown) continue;
      const t = Math.min(1, (now - s.t0) / s.dur);
      const e = 1 - Math.pow(1 - t, 3); // ease-out cubic, as map.js stepGlide
      s.shown = t >= 1 ? s.to : s.from + (s.to - s.from) * e;
      paintTween(key, s);
      if (t < 1) active = true;
    }
    if (active) stepTweens();
  });
}

/** Repaints the live node for `key` with its current eased value (the node may have just been
 *  rebuilt by renderTop; re-find it by its marker). */
function paintTween(key, s) {
  const node = document.querySelector(`#empire [data-tween="${key}"]`);
  if (!node) return;
  if (s.kind === "num") {
    const disp = String(Math.round(s.shown));
    if (disp === s.lastPaint) return;
    s.lastPaint = disp;
    node.replaceChildren(pix(disp, { font: "vigil" }));
  } else {
    const nb = barEl(Math.min(1, s.shown / Math.max(1, s.cost)), "research", { w: 30, next: s.next, title: `${Math.round(s.shown)}/${s.cost}` });
    nb.dataset.tween = key;
    node.replaceWith(nb);
  }
}

/** A big top-bar number (Vigil) that eases from its last value to `value`. */
function tweenNum(key, value, cls = "") {
  const node = h(`span.num${cls}`, { "data-tween": key });
  const s = tweens.get(key);
  if (!s || reduceMotion()) {
    tweens.set(key, { value, shown: value, to: value, kind: "num" });
    node.append(pix(String(value), { font: "vigil" }));
  } else if (value !== s.value) {
    Object.assign(s, { value, from: s.shown, to: value, t0: performance.now(), dur: TWEEN_MS, kind: "num" });
    node.append(pix(String(Math.round(s.shown)), { font: "vigil" }));
    ensureTweenLoop();
  } else {
    node.append(pix(String(Math.round(s.shown)), { font: "vigil" }));
  }
  return node;
}

/** The research bar, whose fill (and tooltip) eases as progress grows; a new tech resets the
 *  progress, so it snaps rather than sliding backwards. */
function tweenBar(key, progress, cost, next, tech) {
  const s = tweens.get(key);
  const techChanged = s && s.tech !== tech;
  const make = (p) => {
    const b = barEl(Math.min(1, p / Math.max(1, cost)), "research", { w: 30, next, title: `${Math.round(p)}/${cost}` });
    b.dataset.tween = key;
    return b;
  };
  if (!s || reduceMotion() || techChanged) {
    tweens.set(key, { value: progress, shown: progress, to: progress, cost, next, tech, kind: "bar" });
    return make(progress);
  }
  if (progress !== s.value) {
    Object.assign(s, { value: progress, from: s.shown, to: progress, t0: performance.now(), dur: TWEEN_MS, cost, next, tech, kind: "bar" });
    ensureTweenLoop();
  } else {
    Object.assign(s, { cost, next, tech });
  }
  return make(s.shown);
}

/** Redraws the top bar's empire summary and phase line. */
export function renderTop() {
  const st = S.status ?? {};
  // A new or loaded game: drop the tween and flare state so the numbers snap to this game's values
  // instead of counting from the last game's, and no stale crescent flare fires.
  const gameKey = st.game?.journal ?? st.game?.dir ?? "memory";
  if (gameKey !== hudGame) {
    hudGame = gameKey;
    tweens.clear();
    if (tweenRaf) {
      cancelAnimationFrame(tweenRaf);
      tweenRaf = 0;
    }
    lastPhase = null;
  }
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
    look.art ? h("span.turn-crescent", ic(yours ? "icon16/turn" : "icon16/turn_wait", "", yours ? "Your turn" : line)) : null,
    st.turn !== undefined ? h("b.turn", small("Turn", ".dim"), " ", big(st.turn)) : null,
    st.year !== undefined ? h("span.dim.year", " ", small(yearText(st.year)), " ") : null,
    h("span#phase-text", pixable(line) ? pix(line) : line),
    st.reveal ? h("span.badge.reveal", "DEBUG REVEAL") : null,
    spectatorBar(),
    autoplayBar(),
  );
  document.body.dataset.phase = st.phase ?? "";
  // A quiet one-shot flare on the crescent when control returns to the player (running -> human);
  // the breathe while the AIs move is pure CSS on the stable #phase (style.css). Added to #phase,
  // which fill() keeps (only its children are replaced), so it plays once, not on every render.
  if (phase && st.phase === "human" && lastPhase !== null && lastPhase !== "human" && !reduceMotion() && !phase.classList.contains("turn-arrived")) {
    phase.classList.add("turn-arrived");
    const done = () => phase.classList.remove("turn-arrived");
    phase.addEventListener("animationend", done, { once: true });
    setTimeout(done, 1200); // in case the animation is interrupted and animationend never fires
  }
  if (st.phase !== undefined) lastPhase = st.phase;
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
    h("span.stat.gold", { title: `Gold ${m.gold} (${m.gold_rate >= 0 ? "+" : ""}${m.gold_rate} a turn)` }, ic("icon16/gold", "¤", "Gold", ".gold"), tweenNum("gold", m.gold), small(`${m.gold_rate >= 0 ? "+" : ""}${m.gold_rate}`, m.gold_rate < 0 ? ".warn" : ".dim")),
    sep(),
    h(
      "button.stat.research",
      { id: "research-button", title: "Research (the tech tree: F6)", onclick: () => document.dispatchEvent(new CustomEvent("ffh-research")) },
      ic(r ? "icon16/research" : "icon16/research_needed", "⚗", "Research"),
      r ? small(dname("TechType", r.tech)) : h("span.warn", pix("choose research")),
      r ? tweenBar("research", r.progress, r.cost, next, r.tech) : null,
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
  items.push(sep(), h("span.stat.dim", { title: "Score" }, small("Score"), tweenNum("score", m.score)));
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
