// node --test: the client state's lookups and the pure helpers (app/state.js): names from the
// data tables, the view's indices, the deferred popups' order, vote labels, the end-turn gate,
// cycling, save names.
import test from "node:test";
import assert from "node:assert/strict";
import * as St from "../app/state.js";

const data = {
  kinds: ["UnitType", "TechType", "OrderType", "BuildingType"],
  tables: {
    UnitType: [{ type: "UNIT_WARRIOR", name: "Warrior" }, { type: "UNIT_SETTLER", name: "" }],
    TechType: [{ type: "TECH_X", name: "Ex" }],
    BuildingType: [{ type: "BUILDING_PALACE", name: "Palace" }],
  },
};

const view = {
  view_version: 3,
  turn: 7,
  me: { player: 0, team: 0, name: "Cardith" },
  players: [
    { id: 1, team: 1, name: "Basium", barbarian: false },
    { id: 2, team: 1, name: "Hyborem", barbarian: false },
    { id: 7, team: 7, name: "Barbarian", barbarian: true },
  ],
  cities: [{ id: 4, owner: 0, x: 3, y: 3, name: "Home", capital: true }],
  units: [
    { id: 10, owner: 0, type: 0, x: 3, y: 3, group: 100 },
    { id: 11, owner: 0, type: 1, x: 3, y: 3, group: 101 },
    { id: 12, owner: 0, type: 0, x: 9, y: 3, group: 102 },
    { id: 13, owner: 0, type: 0, x: 4, y: 4, group: 103 },
    { id: 20, owner: 1, type: 0, x: 3, y: 3 },
  ],
  groups: [
    { id: 100, head: 10, units: [10] },
    { id: 101, head: 11, units: [11] },
    { id: 102, head: 12, units: [12] },
    { id: 103, head: 13, units: [13] },
  ],
  pending: { events: [], votes: [], research_needed: false, idle_cities: [], ready_groups: [100, 102, 103] },
};

test("names from the data tables", () => {
  St.setData(data);
  assert.equal(St.dname("UnitType", 0), "Warrior");
  assert.equal(St.dname("UnitType", 1), "Settler"); // no text: the type name, humanized
  assert.equal(St.dname("UnitType", -1), "-");
  assert.equal(St.dname("UnitType", null), "-");
  assert.equal(St.dname("UnitType", 99), "UnitType 99");
  assert.equal(St.tname("UnitType", 1), "UNIT_SETTLER");
  assert.equal(St.tidx("TechType", "TECH_X"), 0);
  assert.equal(St.tidx("TechType", "TECH_Y"), -1);
  assert.equal(St.orderName(1, 0), "Palace");
});

test("view lookups", () => {
  St.setData(data);
  St.setView(view);
  assert.equal(St.unitsAt(3, 3).length, 3);
  assert.deepEqual(St.ownUnitsAt(3, 3).map((u) => u.id), [10, 11]);
  assert.equal(St.cityAt(3, 3).name, "Home");
  assert.equal(St.cityAt(1, 1), null);
  assert.equal(St.playerName(0), "Cardith");
  assert.equal(St.playerName(2), "Hyborem");
  assert.equal(St.playerName(5), "Player 5");
  assert.equal(St.teamName(1), "Basium & Hyborem");
  assert.equal(St.teamName(0), "your team");
  assert.equal(St.teamName(9), "Team 9");
  assert.ok(St.isBarbarian(7));
  St.setView(null);
  assert.equal(St.S.units.size, 0);
});

test("cycling over the ready groups: next in order, or the nearest", () => {
  St.setView(view);
  assert.equal(St.nextReadyGroup(-1), 100);
  assert.equal(St.nextReadyGroup(100), 102);
  assert.equal(St.nextReadyGroup(103), 100);
  const dist = (ax, ay, bx, by) => Math.abs(ax - bx) + Math.abs(ay - by);
  assert.equal(St.nextReadyGroup(100, { x: 3, y: 3 }, dist), 103);
  assert.equal(St.nextReadyGroup(100, { x: 3, y: 3 }, dist, [103]), 102);
  St.setView({ ...view, pending: { ...view.pending, ready_groups: [100] } });
  assert.equal(St.nextReadyGroup(100), null);
});

test("W cycles over every waiting group, not between the two nearest (P5 review)", () => {
  // The review's game: settler g0 at (13,10), scout g1 at (13,12), warrior g2 at (15,10);
  // settler-scout 2, settler-warrior 2, scout-warrior 3 (Civ4's plot distance).
  const units = [
    { id: 0, owner: 0, type: 1, x: 13, y: 10, group: 0 },
    { id: 1, owner: 0, type: 0, x: 13, y: 12, group: 1 },
    { id: 2, owner: 0, type: 0, x: 15, y: 10, group: 2 },
  ];
  St.setView({
    ...view,
    units,
    groups: units.map((u) => ({ id: u.group, head: u.id, units: [u.id] })),
    pending: { ...view.pending, ready_groups: [0, 1, 2] },
  });
  const dist = (ax, ay, bx, by) => {
    const dx = Math.abs(ax - bx);
    const dy = Math.abs(ay - by);
    return Math.max(dx, dy) + Math.floor(Math.min(dx, dy) / 2);
  };
  const visited = new Set();
  let cur = 0;
  const seen = [cur];
  for (let k = 0; k < 8; k++) {
    const u = St.S.units.get(St.S.groups.get(cur).head);
    const next = St.cycleNext(cur, visited, { x: u.x, y: u.y }, dist);
    assert.notEqual(next, cur, "a press always moves on");
    cur = next;
    seen.push(cur);
  }
  // Every group within each press cycle: the first three presses reach all of them, and over
  // eight presses each one comes up at least twice.
  assert.deepEqual(new Set(seen.slice(0, 3)), new Set([0, 1, 2]), `${seen}`);
  for (const g of [0, 1, 2]) assert.ok(seen.filter((s) => s === g).length >= 2, `${seen}`);
  // One waiting group (the current one): nothing else to select.
  St.setView({ ...view, units, groups: units.map((u) => ({ id: u.group, head: u.id, units: [u.id] })), pending: { ...view.pending, ready_groups: [2] } });
  assert.equal(St.cycleNext(2, new Set()), null);
});

test("deferred popups in C++ order, and set aside per turn", () => {
  const pending = {
    events: [{ triggered: 5 }],
    votes: [{ vote: 2 }],
    research_needed: true,
    idle_cities: [4, 8],
  };
  const q = St.popupQueue(pending, new Set(), 12);
  assert.deepEqual(q.map((p) => p.kind), ["event", "vote", "research", "city", "city"]);
  assert.deepEqual(q.map((p) => p.key), ["event:5", "vote:2", "research:12", "city:4:12", "city:8:12"]);
  const later = St.popupQueue(pending, new Set(["event:5", "research:12", "city:8:12"]), 12);
  assert.deepEqual(later.map((p) => p.key), ["vote:2", "city:4:12"]);
  // A new turn asks again for research and cities.
  assert.equal(St.popupQueue(pending, new Set(["research:12"]), 13).filter((p) => p.kind === "research").length, 1);
  assert.deepEqual(St.popupQueue(null, new Set(), 1), []);
  St.resetDismissed(3);
  St.S.dismissed.add("x");
  St.resetDismissed(3);
  assert.ok(St.S.dismissed.has("x"));
  St.resetDismissed(4);
  assert.equal(St.S.dismissed.size, 0);
  // Another game at the same turn asks again (P5 review).
  St.S.dismissed.add("research:4");
  St.clearDismissed();
  St.resetDismissed(4);
  assert.equal(St.S.dismissed.size, 0);
});

test("vote choices, the end-turn gate, moves and turns", () => {
  St.setView(view);
  assert.equal(St.voteChoiceLabel(-2), "Yes");
  assert.equal(St.voteChoiceLabel(-3), "No");
  assert.equal(St.voteChoiceLabel(-4), "Abstain");
  assert.equal(St.voteChoiceLabel(-5), "Never (defy)");
  assert.equal(St.voteChoiceLabel(1), "Vote for Basium & Hyborem");
  assert.deepEqual(St.gateLines({ events: 2, votes: 0, research: true, ready_units: 1 }), [
    "2 events are waiting for your answer",
    "no research is chosen",
    "a unit is waiting for orders",
  ]);
  assert.deepEqual(St.gateLines({ events: 0, votes: 1, research: false, ready_units: 0 }), ["a vote is open"]);
  assert.equal(St.moves(120), "2");
  assert.equal(St.moves(80), "1 1/3");
  assert.equal(St.moves(40), "2/3");
  assert.equal(St.moves(0), "0");
  assert.equal(St.turns(1), "1 turn");
  assert.equal(St.turns(4), "4 turns");
  assert.equal(St.turns(-1), "-");
  assert.equal(St.turns(null), "-");
});

test("the transport picker's answer names the transport's owner and id (LOADUNIT OnOkClicked)", () => {
  St.setData({ kinds: [], tables: { ...data.tables, CommandType: [{ type: "COMMAND_LOAD" }, { type: "COMMAND_LOAD_UNIT" }] } });
  // Unit 20 is another player's (a teammate's transport): the owner comes from the view.
  St.setView(view);
  assert.deepEqual(St.loadUnitCommand([12], 20), { c: "do_command", units: [12], command: 1, data1: 1, data2: 20, alt: false, confirmed: false });
  assert.deepEqual(St.loadUnitCommand([12], 11), { c: "do_command", units: [12], command: 1, data1: 0, data2: 11, alt: false, confirmed: false });
});

test("save names from paths", () => {
  assert.equal(St.saveNameOfPath("/x/saves/game-3/auto/T0012.ffhsave"), "game-3/auto/T0012");
  assert.equal(St.saveNameOfPath("C:\\saves\\game-3\\auto\\start.ffhsave"), "game-3/auto/start");
  assert.equal(St.saveNameOfPath("/x/saves/game-3/before-war.ffhsave"), "game-3/before-war");
  assert.equal(St.saveNameOfPath(null), null);
  assert.equal(St.saveNameOfPath("auto/T1.ffhsave"), null);
});
