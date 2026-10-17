// The rules-testing tools (design §5.1 "Rules-testing tools", §7 P6): the raw inspector
// (`inspect`), debug reveal (`session debug`), AI autoplay (`autoplay`), taking over a slot of an
// AI save (`session take_over`), the player options (`set_option`), the raw request box, and the
// optional spectator mode: the AI plays every civilization (the human slot too, through FfH's
// AIAutoPlay one turn at a time) with the whole map shown, paused or stepped at will.

import * as api from "./api.js";
import { S, emit, dname, tidx, me } from "./state.js";
import { h, toast } from "./dom.js";
import { openModal, closeTag } from "./modals.js";
import { run, sending } from "./commands.js";
import { rawRequest, loadScreen } from "./screens.js";
import { plotIndex } from "./tiles.js";
import { look, setRendererChoice, EXPERIMENTAL_3D } from "./look.js";

/** The tools menu. */
export function openTools() {
  const st = S.status ?? {};
  const item = (label, help, f, extra = {}) => h("button.tool", { onclick: () => (closeTag("tools"), f()), title: help, ...extra }, h("b", label), h("span.dim", help));
  return openModal({
    title: "Tools",
    tag: "tools",
    body: [
      h(
        "div.tools",
        item(st.reveal ? "Debug reveal: on" : "Debug reveal: off", "Show the whole map and every unit and city (marked DEBUG REVEAL)", () => setReveal(!st.reveal), { id: "tool-reveal" }),
        item("Inspector…", "The rules engine's own data of a unit, group, city, plot, player, team, deal or info entry", () => openInspector(), { id: "tool-inspect" }),
        item("AI autoplay…", "The AI plays your civilization for a number of turns", () => openAutoplay(), { id: "tool-autoplay" }),
        item(spectating() ? "Stop spectating" : "Spectate", "The AI plays every civilization, one turn at a time, with the whole map shown", () => (spectating() ? stopSpectating() : startSpectating()), { id: "tool-spectate" }),
        item("Take over…", "Play another slot of an AI-only save", () => openTakeOver(), { id: "tool-takeover" }),
        item("Options…", "Player options: quick moves, stack attack, no unit cycling, …", () => openOptions(), { id: "tool-options" }),
        item("Raw request…", "Send any protocol request", () => rawRequest(), { id: "tool-raw" }),
      ),
    ],
    buttons: [{ label: "Close", value: null }],
  });
}

/** Debug reveal on or off. */
export async function setReveal(on) {
  try {
    await api.session("debug", { reveal: on });
    toast(on ? "Debug reveal: the whole map is shown." : "Debug reveal off.");
  } catch (e) {
    toast(e.detail ?? e.message, "error");
  }
  emit("kick");
}

// ---------------------------------------------------------------------------------------------
// The inspector.

/** The objects the selection, the open city and the hovered plot give the inspector. */
function quickPicks() {
  const out = [];
  const sel = S.sel;
  if (sel) {
    out.push(["unit", sel.units[0], `the selected ${dname("UnitType", S.units.get(sel.units[0])?.type)}`]);
    const g = S.units.get(sel.units[0])?.group;
    if (g !== undefined && g !== null) out.push(["group", g, "its group"]);
    const city = [...S.cities.values()].find((c) => c.x === sel.x && c.y === sel.y && c.owner === me()?.player);
    if (city) out.push(["city", city.id, city.name]);
    if (S.status?.reveal && S.map) out.push(["plot", plotIndex(S.map, sel.x, sel.y), `the plot (${sel.x}, ${sel.y})`]);
  }
  if (me()) {
    out.push(["player", me().player, "your player"]);
    out.push(["team", me().team, "your team"]);
  }
  return out;
}

/** The inspector: `{:#?}` of a rules object (`inspect {kind, oid}`). Your own objects, any
 *  under debug reveal (a plot only then); an info entry by its kind (`UnitType`) and index. */
export function openInspector(kind = "unit", oid = null) {
  const kinds = ["unit", "group", "city", "plot", "player", "team", "deal", ...(S.data?.kinds ?? [])];
  const kindSel = h("select#inspect-kind", kinds.map((k) => h("option", { value: k, selected: k === kind }, k)));
  const oidIn = h("input#inspect-oid", { type: "number", min: 0, value: oid ?? quickPicks().find(([k]) => k === kind)?.[1] ?? 0 });
  const out = h("pre.report.inspect", "");
  const go = async () => {
    try {
      const r = await api.query("inspect", { kind: kindSel.value, oid: Number(oidIn.value) });
      out.textContent = r.debug;
    } catch (e) {
      out.textContent = `${e.code ?? "error"}: ${e.detail ?? e.message}${e.code === "not_owner" ? " (turn debug reveal on to inspect other players' objects and plots)" : ""}`;
    }
  };
  const picks = quickPicks().map(([k, id, label]) => h("button.small", { onclick: () => ((kindSel.value = k), (oidIn.value = String(id)), go()) }, label));
  const m = openModal({
    title: "Inspector",
    tag: "inspect",
    wide: true,
    body: [h("div.row", h("label.field", h("span", "Kind"), kindSel), h("label.field", h("span", "Id or index"), oidIn)), picks.length ? h("div.row", picks) : null, out],
    buttons: [{ label: "Inspect", primary: true, onClick: go }, { label: "Close", value: null }],
  });
  if (oid !== null) go();
  oidIn.addEventListener("keydown", (ev) => ev.key === "Enter" && go());
  return m;
}

// ---------------------------------------------------------------------------------------------
// Autoplay and the spectator mode.

/** AI autoplay for N turns (`autoplay {turns}`, FfH `setAIAutoPlay`): the AI plays your slot
 *  meanwhile and gives it back at the start of the turn after the last. While it runs the top
 *  bar counts the turns left and offers Stop (`autoplayBar`). */
export function openAutoplay() {
  const n = h("input#autoplay-turns", { type: "number", min: 1, max: 10000, value: 5 });
  const err = h("p.error");
  return openModal({
    title: "AI autoplay",
    tag: "autoplay",
    body: [h("p", "The AI plays your civilization for these turns (popups and decisions included), then you have control again. The turns run without a pause (Stop in the top bar ends them); use Spectate to step through them."), h("label.field", h("span", "Turns"), n), err],
    buttons: [
      {
        label: "Run",
        primary: true,
        onClick: async (m) => {
          const turnsN = Number(n.value);
          const reply = await run({ c: "autoplay", turns: turnsN });
          if (reply) {
            toast(`The AI plays ${turnsN} turn${turnsN > 1 ? "s" : ""}.`);
            m.close("run");
          } else err.textContent = "Refused (1 to 10000 turns, on your turn).";
        },
      },
      { label: "Cancel", value: null },
    ],
  });
}

/** The spectator mode: `{playing, delay, reveal}` while on (the reveal setting to restore). */
let spect = null;

/** The phases where the game does not go on (the spectator mode pauses). */
const TERMINAL = ["over", "crashed", "no_game", "stuck"];

export function spectating() {
  return spect !== null;
}

/** Whether the spectator mode is playing (not paused). */
export function spectatorPlaying() {
  return !!spect?.playing;
}

/** Whether a spectated turn is under way. */
export function spectatorBusy() {
  return !!spect?.busy || looping;
}

/** Starts the spectator mode: debug reveal on, the AI plays every civilization one turn at a
 *  time (`autoplay {turns: 1}` at each of your turns) until paused or stopped. For a new game
 *  (`newGame`) the reveal is off to start with (the status may still be the old game's). */
export async function startSpectating({ playing = true, newGame = false } = {}) {
  if (spect && !newGame) return;
  const wasOn = !newGame && !!S.status?.reveal;
  spect = { playing, delay: 400, reveal: wasOn, busy: false };
  if (!wasOn) await setReveal(true);
  // The old game's status may still be over (or no game): wait for the new one.
  for (let i = 0; newGame && i < 100 && TERMINAL.includes(S.status?.phase); i++) await new Promise((r) => setTimeout(r, 50));
  emit("spectate");
  if (playing) stepLoop();
}

export function pauseSpectating() {
  if (!spect) return;
  spect.playing = false;
  emit("spectate");
}

export function resumeSpectating() {
  if (!spect) return;
  spect.playing = true;
  emit("spectate");
  stepLoop();
}

/** One turn of the spectator mode. */
export async function stepSpectating() {
  if (!spect) await startSpectating({ playing: false });
  if (!spect || spect.busy) return;
  spect.busy = true;
  try {
    await waitHuman();
    if (S.status?.phase === "human") await run({ c: "autoplay", turns: 1 }, { quiet: true });
    await waitHuman();
  } finally {
    if (spect) spect.busy = false;
  }
}

export async function stopSpectating() {
  if (!spect) return;
  const restore = spect.reveal;
  spect = null;
  emit("spectate");
  if (!restore && S.status?.reveal) await setReveal(false);
}

/** The playing loop (one at a time: resuming or a new game while it runs keeps it). */
let looping = false;

async function stepLoop() {
  if (looping) return;
  looping = true;
  try {
    while (spect?.playing) {
      if (TERMINAL.includes(S.status?.phase)) {
        pauseSpectating();
        break;
      }
      await stepSpectating();
      await new Promise((r) => setTimeout(r, spect?.delay ?? 0));
    }
  } finally {
    looping = false;
  }
}

/** Waits until the status says it is the human's turn (or the game cannot go on). */
async function waitHuman() {
  for (let i = 0; i < 600; i++) {
    const st = S.status;
    if (st && !["running", "decision"].includes(st.phase) && !sending()) return;
    await new Promise((r) => setTimeout(r, 50));
  }
}

// ---------------------------------------------------------------------------------------------
// Take over, options.

/** Take over a slot of an AI-only save (a debugging aid: `session take_over`). */
export async function openTakeOver() {
  const player = h("input#takeover-player", { type: "number", min: 0, max: 63, value: 0 });
  const name = h("input#takeover-name", { type: "text", placeholder: "GAME/auto/T0010", title: "A save of the saves directory: GAME/NAME or GAME/auto/NAME" });
  const err = h("p.error");
  return openModal({
    title: "Take over a slot",
    tag: "takeover",
    body: [
      h("p", "Load an AI-only save at a turn boundary (an autosave of a game played with `ffh-play soak run --saves DIR`, or `ffh` autoplay saves copied into the saves directory) and play one of its slots."),
      h("label.field", h("span", "Save"), name),
      h("label.field", h("span", "Player"), player),
      h("button.link", { onclick: () => loadScreen() }, "Browse the saves…"),
      err,
    ],
    buttons: [
      {
        label: "Take over",
        primary: true,
        onClick: async (m) => {
          try {
            await api.session("take_over", { player: Number(player.value), name: name.value });
            m.close("taken");
            emit("game-started");
          } catch (e) {
            err.textContent = e.detail ?? e.message;
          }
        },
      },
      { label: "Cancel", value: null },
    ],
  });
}

/** The player options `set_option` may change. */
export const OPTIONS = [
  ["PLAYEROPTION_QUICK_MOVES", "Quick moves (units jump to their destination)"],
  ["PLAYEROPTION_STACK_ATTACK", "Stack attack (the whole stack attacks)"],
  ["PLAYEROPTION_NO_UNIT_CYCLING", "No unit cycling (no automatic selection of the next unit)"],
  ["PLAYEROPTION_SAFE_AUTOMATION", "Safe automation (automated workers keep improvements)"],
  ["PLAYEROPTION_WAIT_END_TURN", "Wait at the end of the turn"],
];

export function openOptions() {
  const my = me();
  const rows = OPTIONS.map(([type, label]) => {
    const i = tidx("PlayerOptionType", type);
    return h(
      "label.toggle",
      h("input", {
        type: "checkbox",
        "data-option": type,
        checked: !!my?.options?.[i],
        onchange: async (ev) => {
          await run({ c: "set_option", option: i, value: ev.target.checked });
          emit("kick");
        },
      }),
      ` ${label}`,
    );
  });
  // The map's renderer (not a game option: remembered by this browser). The flat one is the P5
  // renderer, kept for debugging; `?renderer=flat` in the address does the same for one page.
  const current = globalThis.ffh?.mapView?.renderer?.() ?? "art";
  const artFailed = look.art ? "" : `The art did not load${look.error ? `: ${look.error.message ?? look.error}` : ""}`;
  const renderer = [
    ["art", "Pixel art (2D)", artFailed || "The 2D pixel-art map"],
    ["iso", "Isometric 2.5D", "Songs-of-Conquest-style isometric map (docs/art/SOC_DIRECTION.md)"],
    ["flat", "Flat (debug)", "The flat debugging renderer (colours, glyphs, labels)"],
    ...(EXPERIMENTAL_3D ? [["3d", "3D painterly (experimental)", "The experimental 3D renderer (docs/R3D.md); needs WebGL2"]] : []),
  ].map(([val, label, title]) =>
    h(
      "label.toggle",
      { title },
      h("input", {
        type: "radio",
        id: `option-${val}-renderer`,
        name: "option-renderer",
        value: val,
        disabled: val === "art" && !look.art,
        checked: current === val,
        onchange: (ev) => {
          if (!ev.target.checked) return;
          setRendererChoice(val);
          emit("renderer", val);
        },
      }),
      ` ${label}`,
    ),
  );
  return openModal({
    title: "Options",
    tag: "options",
    body: [h("div.checks.single", rows), h("h4", "This browser"), h("div.checks.single", renderer)],
    buttons: [{ label: "Close", value: null }],
  });
}

/** The spectator bar's buttons (the top bar shows them while spectating). */
/** The top bar while an AI autoplay runs (not the spectator mode's own): the turns left and
 *  Stop, which sends `autoplay {turns: 0}` (FfH `setAIAutoPlay(0)`; the server stops it before
 *  its next turn slice and you have control at your next turn, or at once during yours). */
export function autoplayBar() {
  const st = S.status ?? {};
  if (spect || st.phase !== "running" || !(st.ai_auto_play > 0)) return null;
  const left = st.ai_auto_play;
  return h(
    "span.spectate",
    h("b", "AI autoplay"),
    h("span.dim", ` ${left} turn${left === 1 ? "" : "s"} left `),
    h(
      "button.small",
      {
        id: "autoplay-stop",
        title: "Stop the AI autoplay: you have control again at your next turn",
        onclick: async (ev) => {
          ev.currentTarget.disabled = true;
          try {
            await api.cmd("autoplay", { turns: 0 });
            toast("The AI autoplay stops.");
          } catch (e) {
            toast(e.detail ?? e.message ?? String(e), "error");
          }
        },
      },
      "■ Stop",
    ),
  );
}

export function spectatorBar() {
  if (!spect) return null;
  const st = S.status ?? {};
  return h(
    "span.spectate",
    h("b", "Spectating"),
    spect.playing
      ? h("button.small", { id: "spectate-pause", onclick: pauseSpectating, title: "Pause after this turn" }, "⏸ Pause")
      : h("button.small", { id: "spectate-play", onclick: resumeSpectating, title: "Play turn after turn" }, "▶ Play"),
    h("button.small", { id: "spectate-step", onclick: stepSpectating, disabled: spect.playing, title: "One turn" }, "⏭ Step"),
    h("button.small", { id: "spectate-stop", onclick: stopSpectating, title: "Stop spectating (debug reveal goes back off)" }, "■ Stop"),
    st.phase === "running" ? h("span.dim", " playing…") : null,
  );
}
