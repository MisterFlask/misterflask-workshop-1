// node --test: the client state's lookups and the pure helpers (app/state.js): names from the
// data tables, the view's indices, the deferred popups' order, vote labels, the end-turn gate,
// Civ4's group cycle, the waiting groups on a plot, options, trade items, save names.
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

test("Civ4's group cycle: the next waiting group after the selection's place, going round (CvPlayer::cycleSelectionGroups)", () => {
  const cycle = [5, 1, 9, 3];
  const ready = new Set([1, 3, 9]);
  assert.deepEqual(St.cycleGroups(cycle, ready, 1), { group: 9, wrapped: false });
  assert.deepEqual(St.cycleGroups(cycle, ready, 9), { group: 3, wrapped: false });
  assert.deepEqual(St.cycleGroups(cycle, ready, 3), { group: 1, wrapped: true });
  assert.deepEqual(St.cycleGroups(cycle, ready, 1, false), { group: 3, wrapped: true });
  assert.deepEqual(St.cycleGroups(cycle, ready, 3, false), { group: 9, wrapped: false });
  // A selected group that does not wait (5) still has its place in the cycle.
  assert.deepEqual(St.cycleGroups(cycle, ready, 5), { group: 1, wrapped: false });
  // No selection: from the head, no wrap; a group not in the cycle: from the head, a wrap.
  assert.deepEqual(St.cycleGroups(cycle, ready, null), { group: 1, wrapped: false });
  assert.deepEqual(St.cycleGroups(cycle, ready, 42), { group: 1, wrapped: true });
  // Only the selected group waits: it again (a wrap); none waits: nothing.
  assert.deepEqual(St.cycleGroups(cycle, new Set([9]), 9), { group: 9, wrapped: true });
  assert.deepEqual(St.cycleGroups(cycle, new Set(), 9), { group: null, wrapped: true });
  assert.deepEqual(St.cycleGroups([], ready, null), { group: null, wrapped: true });
  // W again and again visits every waiting group before any one again (the P5 review's case:
  // never back and forth between the two nearest).
  let cur = 1;
  const seen = [];
  for (let k = 0; k < 6; k++) {
    cur = St.cycleGroups(cycle, ready, cur).group;
    seen.push(cur);
  }
  assert.deepEqual(seen, [9, 3, 1, 9, 3, 1]);
});

test("the waiting groups on a plot (updateSelectionList), player options", () => {
  St.setData({ ...data, tables: { ...data.tables, PlayerOptionType: [{ type: "PLAYEROPTION_ADVISOR_POPUPS" }, { type: "PLAYEROPTION_NO_UNIT_CYCLING" }] } });
  St.setView({ ...view, me: { ...view.me, options: [false, true] } });
  assert.deepEqual(St.readyGroupsOnPlot(3, 3), [100]); // 101 does not wait
  assert.deepEqual(St.readyGroupsOnPlot(3, 3, 100), []);
  assert.deepEqual(St.readyGroupsOnPlot(4, 4), [103]);
  assert.deepEqual(St.readyGroupsOnPlot(0, 0), []);
  assert.ok(St.option("PLAYEROPTION_NO_UNIT_CYCLING"));
  assert.ok(!St.option("PLAYEROPTION_ADVISOR_POPUPS"));
  assert.ok(!St.option("PLAYEROPTION_NOT_THERE"));
  St.setData(data);
});

test("trade items for people", () => {
  St.setData({
    ...data,
    tables: {
      ...data.tables,
      TradeableItem: ["TRADE_GOLD", "TRADE_GOLD_PER_TURN", "TRADE_MAPS", "TRADE_TECHNOLOGIES", "TRADE_CITIES", "TRADE_PEACE", "TRADE_OPEN_BORDERS"].map((type) => ({ type })),
    },
  });
  St.setView(view);
  assert.equal(St.tradeItemLabel({ item: 0, data: 120 }), "120 gold");
  assert.equal(St.tradeItemLabel({ item: 0, data: 0 }), "Gold");
  assert.equal(St.tradeItemLabel({ item: 1, data: 3 }), "3 gold per turn");
  assert.equal(St.tradeItemLabel({ item: 2, data: 0 }), "Maps");
  assert.equal(St.tradeItemLabel({ item: 3, data: 0 }), "Ex");
  assert.equal(St.tradeItemLabel({ item: 4, data: 4 }), "Home");
  assert.equal(St.tradeItemLabel({ item: 4, data: 9, name: "Their city" }), "Their city");
  assert.equal(St.tradeItemLabel({ item: 5, data: 1 }), "Make peace with Basium & Hyborem");
  assert.ok(St.DUAL_ITEMS.includes("TRADE_OPEN_BORDERS"));
  St.setData(data);
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
