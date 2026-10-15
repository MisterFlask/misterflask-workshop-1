// node --test: Civ4's hotkeys (app/hotkeys.js): key names, matching key events, labels, the
// action list's order and which action a key press runs.
import test from "node:test";
import assert from "node:assert/strict";
import * as K from "../app/hotkeys.js";

const key = (code, mods = {}) => ({ code, key: code.replace(/^Key/, "").toLowerCase(), shiftKey: false, altKey: false, ctrlKey: false, metaKey: false, ...mods });
const b = (k, mods = {}) => ({ key: k, alt: false, shift: false, ctrl: false, priority: 0, ...mods });

// MissionType 0 FORTIFY (F), 1 SLEEP (F), 2 SENTRY (S), 3 BUILD, 4 SKIP (Space), 5 FOUND (B, priority 1);
// CommandType 0 PILLAGE (Shift+P), 1 DELETE (Del); BuildType 0 ROAD (R), 1 WINDMILL (Shift+W);
// interface modes: GO_TO (G), ROUTE_TO (Alt+R), RANGE_ATTACK (B).
const data = {
  tables: {
    MissionType: [
      { type: "MISSION_FORTIFY", keys: [b("KB_F")], order: 5 },
      { type: "MISSION_SLEEP", keys: [b("KB_F")], order: 4 },
      { type: "MISSION_SENTRY", keys: [b("KB_S")], order: 3 },
      { type: "MISSION_BUILD", keys: [], order: 0 },
      { type: "MISSION_SKIP", keys: [b("KB_SPACE")], order: 6 },
      { type: "MISSION_FOUND", keys: [b("KB_B", { priority: 1 })], order: 9 },
    ],
    CommandType: [
      { type: "COMMAND_PILLAGE", keys: [b("KB_P", { shift: true })], order: 1 },
      { type: "COMMAND_DELETE", keys: [b("KB_DELETE")], order: 0 },
    ],
    BuildType: [
      { type: "BUILD_ROAD", keys: [b("KB_R")], order: 2 },
      { type: "BUILD_WINDMILL", keys: [b("KB_W", { shift: true })], order: 1 },
    ],
    AutomateType: [
      { type: "AUTOMATE_BUILD", keys: [b("KB_A")] },
      { type: "AUTOMATE_NETWORK", keys: [b("KB_N")] },
    ],
  },
  interface_modes: [
    { type: "INTERFACEMODE_GO_TO", keys: [b("KB_G")], order: 0 },
    { type: "INTERFACEMODE_ROUTE_TO", keys: [b("KB_R", { alt: true })], order: 0 },
    { type: "INTERFACEMODE_RANGE_ATTACK", keys: [b("KB_B")], order: 0 },
  ],
};
const mission = (m, enabled = true, extra = {}) => ({ kind: "mission", mission: m, enabled, ...extra });

test("Civ4 key names", () => {
  assert.deepEqual(K.kbCode("KB_F"), { code: "KeyF", key: "f" });
  assert.deepEqual(K.kbCode("KB_7"), { code: "Digit7", key: "7" });
  assert.deepEqual(K.kbCode("KB_SPACE"), { code: "Space", key: " " });
  assert.deepEqual(K.kbCode("KB_DELETE"), { code: "Delete", key: "Delete" });
  assert.equal(K.kbCode("KB_NUMPAD5").code, "Numpad5");
  assert.deepEqual(K.kbCode("KB_WHATEVER"), { code: null, key: null });
  assert.equal(K.label(b("KB_P", { shift: true })), "Shift+P");
  assert.equal(K.label(b("KB_R", { alt: true })), "Alt+R");
  assert.equal(K.label(b("KB_SPACE")), "Space");
  assert.equal(K.label(b("KB_DELETE")), "Del");
  assert.equal(K.label(null), "");
});

test("key events match bindings, modifiers exactly; events without a code by their key", () => {
  assert.ok(K.matches(b("KB_F"), key("KeyF")));
  assert.ok(!K.matches(b("KB_F"), key("KeyF", { shiftKey: true })));
  assert.ok(K.matches(b("KB_P", { shift: true }), key("KeyP", { shiftKey: true, key: "P" })));
  assert.ok(!K.matches(b("KB_P", { shift: true }), key("KeyP")));
  assert.ok(K.matches(b("KB_R", { alt: true }), key("KeyR", { altKey: true, key: "®" })), "the code, whatever the layout makes of the key");
  assert.ok(K.matches(b("KB_F"), { key: "F" }), "a script's event without a code");
  assert.ok(K.matches(b("KB_SPACE"), { code: "Space", key: " " }));
  assert.ok(!K.matches({ key: null }, key("KeyF")));
});

test("the action list's order and the action a key runs", () => {
  const actions = [
    mission(0),
    mission(2),
    mission(3, true, { data1: 0 }), // build a road
    mission(3, false, { data1: 1 }), // a windmill: not here
    mission(4),
    mission(5),
    { kind: "command", command: 0, enabled: true },
    { kind: "command", command: 1, enabled: true },
    { kind: "target", mode: "INTERFACEMODE_GO_TO", enabled: true },
    { kind: "target", mode: "INTERFACEMODE_ROUTE_TO", enabled: true },
    { kind: "target", mode: "INTERFACEMODE_RANGE_ATTACK", enabled: true },
  ];
  // iOrderPriority, highest first, else as listed.
  const name = (a) => (a.kind === "mission" ? (a.mission === 3 ? `build${a.data1}` : `m${a.mission}`) : a.kind === "command" ? `c${a.command}` : a.mode);
  assert.deepEqual(K.sortActions(actions, data).map(name), ["m5", "m4", "m0", "m2", "build0", "build1", "c0", "c1", "INTERFACEMODE_GO_TO", "INTERFACEMODE_ROUTE_TO", "INTERFACEMODE_RANGE_ATTACK"]);
  const run = (ev) => K.actionForKey(actions, ev, data);
  assert.equal(run(key("KeyF")).mission, 0);
  assert.equal(run(key("KeyS")).mission, 2);
  assert.equal(run(key("KeyR")).data1, 0, "R builds the road");
  assert.equal(run(key("KeyR", { altKey: true })).mode, "INTERFACEMODE_ROUTE_TO");
  assert.equal(run(key("KeyG")).mode, "INTERFACEMODE_GO_TO");
  assert.equal(run(key("KeyB")).mission, 5, "found a city (priority 1) before a range attack");
  assert.equal(run({ code: "Space", key: " " }).mission, 4);
  assert.equal(run({ code: "Delete", key: "Delete" }).command, 1);
  assert.equal(run(key("KeyP", { shiftKey: true })).command, 0);
  assert.equal(run(key("KeyP")), null);
  assert.equal(run(key("KeyW", { shiftKey: true })), null, "a disabled action does not run");
  assert.ok(K.boundTo(actions, key("KeyW", { shiftKey: true }), data), "but the key is the selection's");
  assert.ok(!K.boundTo(actions, key("KeyQ"), data));
  // Sleep when fortify is not offered (both on F).
  const sleeper = [mission(1), mission(2)];
  assert.equal(K.actionForKey(sleeper, key("KeyF"), data).mission, 1);
  // The automate buttons (CvAutomateInfo hotkeys).
  const worker = [{ kind: "automate", automate: 0, enabled: true }, { kind: "automate", automate: 1, enabled: true }];
  assert.equal(K.actionForKey(worker, key("KeyA"), data).automate, 0);
  assert.equal(K.actionForKey(worker, key("KeyN"), data).automate, 1);
  // Without the founding, B is the range attack.
  assert.equal(K.actionForKey(actions.filter((a) => a.mission !== 5), key("KeyB"), data).mode, "INTERFACEMODE_RANGE_ATTACK");
});
