// Sending commands (design §4.5) the way the Civ4 interface does: one at a time, with the
// confirmations the exe asks for (declare war to move, delete, global and war spells, which
// transport to load into, ending the turn with things open) answered in dialogs and the
// command resent with the answer.

import * as api from "./api.js";
import { S, emit, dname, teamName, gateLines, loadUnitCommand } from "./state.js";
import { ask } from "./modals.js";
import { toast, busy } from "./dom.js";

let inFlight = false;
/** The automatic (`quiet`) command on its way, if any: AutoMoves, a spectated turn, grouping. */
let automatic = null;

/** Whether a command is being sent (another one waits: the exe sends one at a time). */
export function sending() {
  return inFlight;
}

/** Sends command `c` (`{c: "move", ...}`), asks the confirmations, and returns the reply of
 *  the command that was applied, or null (refused, cancelled, or another command in flight).
 *  Messages the command produced are shown; `applied` is emitted with the reply. A command of
 *  the player's own while an automatic one is on its way (P6: the AutoMoves the group cycle
 *  sends when it comes round) waits for it rather than being dropped; a second click while the
 *  player's own command is on its way is still dropped (one request per click). */
export async function run(c, { quiet = false } = {}) {
  if (inFlight && automatic && !quiet) await automatic.catch(() => {});
  if (inFlight) return null;
  inFlight = true;
  const sent = busy(() => loop(c, quiet));
  if (quiet) automatic = sent;
  try {
    return await sent;
  } finally {
    inFlight = false;
    if (automatic === sent) automatic = null;
  }
}

async function loop(c, quiet) {
  let next = c;
  for (let round = 0; round < 4 && next; round++) {
    let reply;
    try {
      reply = await api.cmd(next.c, strip(next));
    } catch (e) {
      if (!quiet) toast(refusal(e), "error");
      emit("refused", next, e);
      return null;
    }
    const confirm = reply.r?.confirm;
    if (!confirm) {
      for (const line of reply.log ?? []) toast(line.text, "log", 7000);
      emit("applied", next, reply);
      return reply;
    }
    next = await answerConfirm(next, confirm);
  }
  return null;
}

function strip(c) {
  const { c: _name, ...rest } = c;
  return rest;
}

/** A refusal for people. */
export function refusal(e) {
  if (e instanceof api.ApiError) {
    const what = { not_your_turn: "Not your turn", decision_pending: "Answer the open decision first", invalid: "Invalid", not_owner: "Not yours", gone: "Gone", cannot: "Cannot" }[e.code] ?? e.code;
    return e.detail ? `${what}: ${e.detail}` : what;
  }
  return `Connection problem: ${e.message ?? e}`;
}

/** The command to send after the player answered `confirm` (null: cancelled). */
async function answerConfirm(c, confirm) {
  switch (confirm.kind) {
    case "declare_war": {
      const choices = [{ label: "Declare war", value: "war", primary: true }];
      if (confirm.move_anyway) choices.push({ label: "Move without declaring", value: "move" });
      choices.push({ label: "Cancel", value: null });
      const v = await ask("Declare war?", `This move means war with ${teamName(confirm.team)}.`, choices);
      if (!v) return null;
      // The exe answers the DECLAREWARMOVE popup with a move (design §4.5).
      const move = c.c === "move" ? c : { c: "move", units: c.units, x: c.data1, y: c.data2, append: c.append ?? false };
      return { ...move, declare_war: v === "war" };
    }
    case "command": {
      const name = dname("CommandType", confirm.command);
      const v = await ask(`${name}?`, `Are you sure? (${name})`, [
        { label: name, value: true, primary: true },
        { label: "Cancel", value: null },
      ]);
      return v ? { ...c, confirmed: true } : null;
    }
    case "cast_global": {
      const name = dname("SpellType", confirm.spell);
      const v = await ask(`Cast ${name}?`, "This is a world spell: it can be cast once per game.", [
        { label: `Cast ${name}`, value: true, primary: true },
        { label: "Cancel", value: null },
      ]);
      return v ? { ...c, confirmed: true } : null;
    }
    case "cast_war": {
      const name = dname("SpellType", confirm.spell);
      const v = await ask(`Cast ${name}?`, `Casting ${name} here declares war on ${teamName(confirm.team)}.`, [
        { label: "Cast and declare war", value: true, primary: true },
        { label: "Cancel", value: null },
      ]);
      return v ? { ...c, confirmed: true } : null;
    }
    case "load_unit": {
      const choices = confirm.transports.map((t) => {
        const u = S.units.get(t);
        return { label: u ? `${dname("UnitType", u.type)} (${t})` : `Unit ${t}`, value: t };
      });
      choices.push({ label: "Cancel", value: null });
      const t = await ask("Load into which transport?", "Several transports have room.", choices);
      if (t === null || t === undefined) return null;
      return loadUnitCommand(c.units, t);
    }
    case "end_turn": {
      const lines = gateLines(confirm);
      const choices = [];
      if (confirm.events || confirm.votes || confirm.research) choices.push({ label: "Answer them", value: "popups", primary: true });
      if (confirm.ready_units) choices.push({ label: "Select a unit", value: "units", primary: !choices.length });
      choices.push({ label: "End turn anyway", value: "force" });
      const v = await ask("End the turn?", lines.map((l) => `• ${l[0].toUpperCase()}${l.slice(1)}.`), choices, { tag: "gate" });
      if (v === "force") return { ...c, force: true };
      if (v === "popups") emit("reopen-popups");
      if (v === "units") emit("select-ready");
      return null;
    }
    default:
      toast(`Unknown confirmation ${confirm.kind}`, "error");
      return null;
  }
}

/** The command of a `group` query action, with the plot picked for a `target` action. */
export function actionCommand(action, x, y) {
  const c = { ...action.cmd };
  if (action.kind !== "target") return c;
  if (c.c === "move") return { ...c, x, y };
  return { ...c, data1: x, data2: y };
}

/** End turn (`force` skips the gate). */
export function endTurn(force = false) {
  return run(force ? { c: "end_turn", force: true } : { c: "end_turn" });
}

/** For the end-turn button's label: what is still open this turn. */
export function openCount() {
  const p = S.view?.pending;
  if (!p) return 0;
  return (p.events?.length ?? 0) + (p.votes?.length ?? 0) + (p.research_needed ? 1 : 0);
}
