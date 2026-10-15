// Civ4's hotkeys for the selection's actions (design §7 P6 "hotkeys"), without the DOM (tested by
// assets/test/hotkeys.test.mjs). The bindings come from the XML through `/api/data`: each
// MissionType, CommandType and BuildType entry has `keys` (`[{key: "KB_F", alt, shift, ctrl,
// priority}]`, the `<HotKey>` and `<HotKeyAlt>` bindings) and `order` (`iOrderPriority`, by which
// the action list is sorted), and `interface_modes` lists the interface modes' bindings.
//
// The exe's dispatch of a key press is not in the DLL source: among the selection's enabled
// actions bound to the key, the one with the highest hotkey priority is taken, ties in the
// action list's order (iOrderPriority, highest first). F fortifies or sleeps (both are bound to
// F; a unit offers one of them), S is sentry, B founds a city (priority 1) before bombard or a
// range attack, R builds a road (a worker) or recons, Shift+P pillages, Alt+R routes to, G is
// go-to, Space skips, Del deletes, Backspace cancels, L loads, U unloads.

/** Civ4 key names (`KB_*`) to `KeyboardEvent.code` (layout independent, as Civ4's scan codes)
 *  and the `KeyboardEvent.key` used when an event has no code (synthetic events). */
export function kbCode(kb) {
  const k = String(kb ?? "").replace(/^KB_/, "");
  if (/^[A-Z]$/.test(k)) return { code: `Key${k}`, key: k.toLowerCase() };
  if (/^[0-9]$/.test(k)) return { code: `Digit${k}`, key: k };
  if (/^F\d{1,2}$/.test(k)) return { code: k, key: k };
  if (/^NUMPAD\d$/.test(k)) return { code: `Numpad${k.slice(6)}`, key: null };
  const special = {
    SPACE: ["Space", " "],
    RETURN: ["Enter", "Enter"],
    DELETE: ["Delete", "Delete"],
    BACKSPACE: ["Backspace", "Backspace"],
    ESCAPE: ["Escape", "Escape"],
    TAB: ["Tab", "Tab"],
    HOME: ["Home", "Home"],
    END: ["End", "End"],
    INSERT: ["Insert", "Insert"],
    NUMPADPERIOD: ["NumpadDecimal", null],
    NUMPADENTER: ["NumpadEnter", null],
    COMMA: ["Comma", ","],
    PERIOD: ["Period", "."],
    MINUS: ["Minus", "-"],
    EQUALS: ["Equal", "="],
    SLASH: ["Slash", "/"],
  }[k];
  return special ? { code: special[0], key: special[1] } : { code: null, key: null };
}

/** Whether key event `ev` (`code`, `key`, modifiers) is binding `b`. */
export function matches(b, ev) {
  if (!b?.key) return false;
  if (!!b.shift !== !!ev.shiftKey || !!b.alt !== !!ev.altKey || !!b.ctrl !== !!(ev.ctrlKey || ev.metaKey)) return false;
  const { code, key } = kbCode(b.key);
  if (ev.code) return ev.code === code;
  // No code (an event a script made): the key, letters in either case.
  return key !== null && String(ev.key ?? "").toLowerCase() === key.toLowerCase();
}

/** A binding for people: "Shift+P", "Space", "Del", "F". */
export function label(b) {
  if (!b?.key) return "";
  const k = String(b.key).replace(/^KB_/, "");
  const name = { SPACE: "Space", DELETE: "Del", BACKSPACE: "Backspace", RETURN: "Enter", NUMPADPERIOD: "Num .", ESCAPE: "Esc" }[k] ?? (k.startsWith("NUMPAD") ? `Num ${k.slice(6)}` : k.length === 1 ? k : k[0] + k.slice(1).toLowerCase());
  return [b.ctrl ? "Ctrl" : null, b.alt ? "Alt" : null, b.shift ? "Shift" : null, name].filter(Boolean).join("+");
}

/** The data entry an action's bindings and order come from: a build's BuildType, a mission's
 *  MissionType, a command's CommandType, an automation's AutomateType, an interface mode
 *  (`data.interface_modes`). */
function entryOf(a, data) {
  const t = data?.tables;
  if (!t) return null;
  const mission = t.MissionType?.[a.mission];
  if (a.kind === "mission" && mission?.type === "MISSION_BUILD") return t.BuildType?.[a.data1] ?? null;
  if (a.kind === "mission") return mission ?? null;
  if (a.kind === "target") return (data.interface_modes ?? []).find((m) => m.type === a.mode) ?? null;
  if (a.kind === "command" || a.kind === "load") return t.CommandType?.[a.command] ?? null;
  if (a.kind === "automate") return t.AutomateType?.[a.automate] ?? null;
  return null;
}

/** The bindings of a `group` query action. */
export function bindings(a, data) {
  return entryOf(a, data)?.keys ?? [];
}

/** The action list's order (`iOrderPriority`, 0 when none). */
export function order(a, data) {
  return entryOf(a, data)?.order ?? 0;
}

/** The actions in the action list's order: `iOrderPriority`, highest first (C++
 *  `SetGlobalActionInfo` sorts the action infos by it), else as the server listed them. */
export function sortActions(actions, data) {
  return actions.map((a, i) => ({ a, i, o: order(a, data) })).sort((x, y) => y.o - x.o || x.i - y.i).map((x) => x.a);
}

/** The action a key press runs: the enabled action bound to it with the highest hotkey
 *  priority, ties in the action list's order; null when none. */
export function actionForKey(actions, ev, data) {
  let best = null;
  let bestPriority = -Infinity;
  for (const a of sortActions(actions ?? [], data)) {
    if (!a.enabled) continue;
    for (const b of bindings(a, data)) {
      if (matches(b, ev) && (b.priority ?? 0) > bestPriority) {
        best = a;
        bestPriority = b.priority ?? 0;
      }
    }
  }
  return best;
}

/** Whether any action of the list is bound to the key (enabled or not): a key the selection
 *  uses is not also a global control. */
export function boundTo(actions, ev, data) {
  return (actions ?? []).some((a) => bindings(a, data).some((b) => matches(b, ev)));
}
